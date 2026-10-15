#requires -Version 7
param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [ValidateRange(3,30)][int]$Samples=8,
    [ValidateRange(100,1000)][int]$Events=400,
    [switch]$LegacyLedger
)
# Isolated, bounded acceptance. Does not run AI, change PATH, install or clean.
$ErrorActionPreference='Stop'
$repo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$exe=(Resolve-Path -LiteralPath $Binary).Path
$audit=[IO.Path]::GetFullPath($OutputDirectory)
$allowed=[IO.Path]::GetFullPath((Join-Path $repo 'Release')).TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar
if(-not $audit.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Output must be a new child of ignored Release'}
if(Test-Path -LiteralPath $audit){throw 'Refusing to overwrite prior measurement'}
New-Item -ItemType Directory -Path $audit|Out-Null
$fixtureHost=Join-Path $audit 'host'
function Run([string]$root,[string[]]$arguments,[switch]$Text){
    $info=[Diagnostics.ProcessStartInfo]::new();$info.FileName=$exe;$info.WorkingDirectory=$root
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    $info.Environment['ROUTE_SIDECAR_HOME']=$fixtureHost
    [void]$info.Environment.Remove('ROUTE_WORKER_CREDENTIAL')
    foreach($arg in $arguments){$info.ArgumentList.Add($arg)}
    $p=[Diagnostics.Process]::new();$p.StartInfo=$info
    $timer=[Diagnostics.Stopwatch]::StartNew();[void]$p.Start()
    $stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync();$rss=0L
    while(-not $p.WaitForExit(2)){
        if($timer.Elapsed.TotalSeconds -gt 30){throw "Fixture process $($p.Id) still running; inspect it before retrying"}
        try{$p.Refresh();$rss=[Math]::Max($rss,$p.WorkingSet64)}catch [InvalidOperationException]{}
    }
    $timer.Stop();$out=$stdout.GetAwaiter().GetResult();$err=$stderr.GetAwaiter().GetResult();$code=$p.ExitCode;$p.Dispose()
    if($code){throw "CLI exit $code : $err"}
    [pscustomobject]@{ms=$timer.Elapsed.TotalMilliseconds;bytes=[Text.Encoding]::UTF8.GetByteCount($out);sampled_rss_bytes=$rss;text=$out;value=$(if($Text){$null}else{$out|ConvertFrom-Json})}
}
function Snapshot([string]$path){
    $hash=[ordered]@{};if(Test-Path -LiteralPath $path){foreach($f in Get-ChildItem -LiteralPath $path -Recurse -Force -File|Sort-Object FullName){$hash[$f.FullName.Substring($path.Length)]=(Get-FileHash -LiteralPath $f.FullName).Hash}}
    ConvertTo-Json -InputObject $hash -Compress
}
function SumBytes([string]$path){
    if(-not(Test-Path -LiteralPath $path)){return 0L}
    [long](Get-ChildItem -LiteralPath $path -Recurse -Force -File|Measure-Object Length -Sum).Sum
}
function Stats($values){$a=@($values|Sort-Object);[ordered]@{n=$a.Count;p50_ms=$a[[int][Math]::Ceiling($a.Count*.5)-1];p95_ms=$a[[int][Math]::Ceiling($a.Count*.95)-1]}}
function ActiveLedger([string]$root){
    $dir=Join-Path $root '.route/development'
    if(Test-Path (Join-Path $dir 'ledger.manifest.json')){return Join-Path $dir 'ledger-000001.rlog'}
    Join-Path $dir 'ledger.json'
}
function Footprint([string]$root){
    $door=Join-Path $root 'ROUTE.md';$ledger=ActiveLedger $root
    [ordered]@{doorway_bytes=$(if(Test-Path $door){(Get-Item $door).Length}else{0});ignored_bytes=((SumBytes (Join-Path $root '.route'))+(SumBytes (Join-Path $root '.route-basic')));external_bytes=(SumBytes $fixtureHost);ledger_bytes=$(if(Test-Path $ledger){(Get-Item $ledger).Length}else{0})}
}
$attach=@();$cold=@();$startup=@();$stages=[ordered]@{};$root=$null
for($i=0;$i -lt $Samples;$i++){
    $project=Join-Path $audit "project-$i";New-Item -ItemType Directory -Path $project|Out-Null
    & git -C $project init -q;if($LASTEXITCODE){throw 'Git init failed'}
    if($i -eq 0){$root=$project;$stages['empty']=Footprint $root}
    $missing=Run $project @('enter','--json');if($missing.value.code -ne 'ATTACH_REQUIRED'){throw 'Missing-state entry failed'}
    $template=Run $project @('doorway','--template') -Text
    if($template.bytes -gt 1024){throw 'Preferred template size exceeded'}
    if($i -eq 0){$stages['template_discovery']=Footprint $root}
    if($LegacyLedger){
        # Controlled compatibility fixture: same domain semantics and current
        # binary/profile, with the pre-migration writer explicitly selected.
        $state=Join-Path $project '.route';New-Item -ItemType Directory -Path (Join-Path $state 'development') -Force|Out-Null
        $pidFixture='prj_'+[guid]::NewGuid().ToString('N').Substring(0,26)
        @{schema=1;project_id=$pidFixture;workspace_id=('ws_'+[guid]::NewGuid().ToString('N').Substring(0,26));created_at=0;attached_from=$null;bound_root=$project.Replace('\','/')}|ConvertTo-Json|Set-Content (Join-Path $state 'project-identity.json')
        @{schema=2;project_id=$pidFixture;head_hash=('0'*64);events=@()}|ConvertTo-Json|Set-Content (Join-Path $state 'development/ledger.json')
    }
    $a=Run $project @('attach');$attach+=$a.ms
    if($i -eq 0){$stages['attach']=Footprint $root}
    $c=Run $project @('enter','--json');$cold+=$c.ms
    if($c.value.code -ne 'READY'){throw 'Attached entry not ready'}
    if($i -eq 0){$stages['first_enter']=Footprint $root;$freshBytes=$c.bytes}
    $v=Run $project @('--version') -Text;$startup+=$v.ms
}
# External byte counts are aggregate across the deliberately separate projects.
$before=Snapshot $root;$hostBefore=Snapshot $fixtureHost;$warm=@();$rss=@()
foreach($kind in @('enter','status','handoff')){
    for($i=1;$i -le 100;$i++){
        $args=if($kind -eq 'handoff'){@('handoff','--shared')}else{@($kind,'--json')}
        $r=Run $root $args
        if($kind -eq 'enter'){
            if($r.bytes -gt 16384 -or $r.value.code -ne 'READY'){throw 'Entry output invalid'}
            $warm+=$r.ms;$rss+=$r.sampled_rss_bytes
            if($i -eq 10){$stages['10_enters']=Footprint $root}
            if($i -eq 100){$stages['100_enters']=Footprint $root}
        }
    }
    if((Snapshot $root) -ne $before -or (Snapshot $fixtureHost) -ne $hostBefore){throw "$kind mutated project or credentials"}
}
$ledger=ActiveLedger $root;$initial=(Get-Item $ledger).Length
$incremental=$ledger.EndsWith('.rlog');$previousSize=$initial;$metadataWrites=0L
$writes=@();$rewrite=0L;$payload=0L;$scales=@();$first100=0L
for($i=1;$i -le $Events;$i++){
    $fact="Fixture progress $i; next verify boundary $i";$payload+=[Text.Encoding]::UTF8.GetByteCount($fact)
    $w=Run $root @('sidecar','note',$fact);$writes+=$w.ms;$size=(Get-Item $ledger).Length
    if($incremental){$rewrite+=($size-$previousSize);$metadataWrites+=(Get-Item (Join-Path $root '.route/development/ledger.manifest.json')).Length}else{$rewrite+=$size}
    $previousSize=$size
    if($i -eq 1){$stages['first_fact']=Footprint $root}
    if($i -eq 100){$stages['100_facts']=Footprint $root;$first100=$size-$initial}
    if($i -in @(1,100,$Events)){
        $times=@();$memory=@();$r=Run $root @('enter','--json');$first=$r.ms
        for($j=0;$j -lt 8;$j++){$r=Run $root @('enter','--json');$times+=$r.ms;$memory+=$r.sampled_rss_bytes}
        $window=@($writes|Select-Object -Last 20)
        $scales+=[ordered]@{facts=$i;ledger_bytes=$size;first_process_ms=$first;warm=(Stats $times);last_20_writes=(Stats $window);sampled_rss_max_bytes=($memory|Measure-Object -Maximum).Maximum;entry_bytes=$r.bytes}
    }
}
$post=Run $root @('enter','--json');$blind=Run $root @('enter','--json','--independent')
if(@($blind.value.base_units).Count -or @($blind.value.handoff.observations).Count){throw 'Independent projection leak'}
$richBytes=$null;$receipt=Join-Path $repo 'Release/local-tools/base-positive-chain-20261013/dogfood/dogfood-receipt.json'
if(Test-Path $receipt){
    $scene=Get-Content -Raw $receipt|ConvertFrom-Json;$richBefore=Snapshot $scene.project_path
    $rich=Run $scene.project_path @('enter','--json');$richBytes=$rich.bytes
    if($richBytes -gt 16384 -or (Snapshot $scene.project_path) -ne $richBefore){throw 'Rich entry changed retained fixture or exceeded budget'}
}
$door=Join-Path $root 'ROUTE.md';$doorText=[IO.File]::ReadAllText($door)
$report=[ordered]@{
    schema='route.delivery-measurement/1';status='PASS';binary=$exe;binary_sha256=(Get-FileHash $exe).Hash;project=$root;
    doorway_bytes=(Get-Item $door).Length;doorway_lines=($doorText.TrimEnd("`n") -split "`n").Count;
    stages=$stages;footprint_scope='Git metadata excluded; external home aggregate across Samples projects; each project owns one 64-byte key';
    startup=(Stats $startup);attach=(Stats $attach);enter_cold=(Stats $cold);enter_warm=(Stats $warm);
    cold_definition='First CLI process after attach or write; OS page cache is not flushed';
    fresh_entry_bytes=$freshBytes;rich_entry_bytes=$richBytes;post_events_entry_bytes=$post.bytes;
    read_checks=@('100 enter','100 status','100 bounded handoff');read_growth_bytes=0;project_and_host_hashes_unchanged=$true;
    facts=$Events;first_100_ledger_growth=$first100;ledger_growth=((Get-Item $ledger).Length-$initial);
    bytes_per_event=(((Get-Item $ledger).Length-$initial)/[double]$Events);payload_bytes=$payload;
    metadata_inclusive_growth_over_payload=(((Get-Item $ledger).Length-$initial)/[double]$payload);
    ledger_format=$(if($incremental){'FRAMED_JOURNAL'}else{'LEGACY_JSON'});
    cumulative_ledger_write_bytes=$rewrite;metadata_write_bytes=$metadataWrites;
    cumulative_ledger_rewrite_estimate=$(if($incremental){$null}else{$rewrite});
    rewrite_scope='Legacy: sum of resulting file lengths; journal: appended frame bytes plus separately reported manifest replacements. Excludes OS physical IO and lock/transport receipts';
    first_100_writes=(Stats @($writes|Select-Object -First 100));first_100_write_total_ms=(@($writes|Select-Object -First 100)|Measure-Object -Sum).Sum;
    scaling=$scales;sampled_rss_max_bytes=($rss|Measure-Object -Maximum).Maximum;
    rss_scope='Child process working set sampled about every 2ms: lower bound on peak, not exact peak RSS';
    storage_rewrite_performed=$false;archive_database_created=(Test-Path (Join-Path $root '.route-basic'));real_ai_golden_path='NOT_RUN'
}
$report|ConvertTo-Json -Depth 15|Set-Content -LiteralPath (Join-Path $audit 'measurement.json') -Encoding UTF8
$report|ConvertTo-Json -Depth 15
