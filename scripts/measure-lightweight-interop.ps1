param([string]$RouteExe = (Join-Path $PSScriptRoot '..\tool\route\target\debug\route.exe'))
$ErrorActionPreference = 'Stop'
$RouteExe = (Resolve-Path -LiteralPath $RouteExe).Path
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('route-weight-' + [guid]::NewGuid().ToString('N'))
$project = Join-Path $fixture 'project'
$hostData = Join-Path $fixture 'host'
$null = New-Item -ItemType Directory -Path $project -Force
$oldHome = $env:ROUTE_SIDECAR_HOME
$env:ROUTE_SIDECAR_HOME = $hostData
$script:peak = 0L
function Invoke-Route([string]$arguments) {
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $RouteExe
    $info.Arguments = $arguments
    $info.WorkingDirectory = $project
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $info
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $null = $process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    while (-not $process.HasExited) {
        try { $process.Refresh(); $script:peak = [Math]::Max($script:peak, $process.PeakWorkingSet64) } catch { }
        Start-Sleep -Milliseconds 1
    }
    $watch.Stop()
    $text = $stdout.GetAwaiter().GetResult()
    $errors = $stderr.GetAwaiter().GetResult()
    $code = $process.ExitCode
    $process.Dispose()
    if ($code -ne 0) { throw "Route failed ($arguments): $errors" }
    return @{ms=$watch.Elapsed.TotalMilliseconds; bytes=[Text.Encoding]::UTF8.GetByteCount($text); value=($text|ConvertFrom-Json)}
}
function Manifest {
    return @(Get-ChildItem -LiteralPath $fixture -Recurse -File | Sort-Object FullName | ForEach-Object {
        "$($_.FullName):$($_.Length):$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash)"
    }) -join "`n"
}
function Percentile($values,[double]$percent) {
    $sorted=@($values|Sort-Object)
    return [Math]::Round($sorted[[Math]::Max(0,[Math]::Ceiling($sorted.Count*$percent)-1)],2)
}
try {
    $attach=Invoke-Route 'attach'
    $before=Manifest
    Start-Sleep -Milliseconds 250
    $idle=Manifest
    $status=@(); $handoff=@()
    for($i=0;$i -lt 50;$i++) {
        $status += (Invoke-Route 'sidecar whoami').ms
        $handoff += (Invoke-Route 'handoff --shared').ms
    }
    $after=Manifest
    if ($before -ne $idle -or $before -ne $after) {throw 'Read-only or idle operation changed durable data'}
    $projection=Invoke-Route 'handoff --shared'
    [ordered]@{
        binary_bytes=(Get-Item -LiteralPath $RouteExe).Length
        build_kind= $(if($RouteExe -match '[\\/]release[\\/]'){'release'}else{'debug'})
        attach_ms=[Math]::Round($attach.ms,2)
        status_surface='sidecar whoami (identity/status); not legacy scanning route status'
        status_p50_ms=(Percentile $status 0.50); status_p95_ms=(Percentile $status 0.95)
        handoff_p50_ms=(Percentile $handoff 0.50); handoff_p95_ms=(Percentile $handoff 0.95)
        idle_durable_growth_bytes=0; reads=100; reads_durable_growth_bytes=0
        handoff_bytes=$projection.bytes; observed_peak_working_set_bytes=$script:peak
        memory_scope='Sampled transient CLI peak, not a daemon/idle-memory measurement'
        manual_configuration_steps=0; attach_commands=1
        fixture=$fixture
    } | ConvertTo-Json
} finally {
    if ($null -eq $oldHome) { Remove-Item Env:ROUTE_SIDECAR_HOME -ErrorAction SilentlyContinue }
    else { $env:ROUTE_SIDECAR_HOME=$oldHome }
}
