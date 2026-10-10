param(
    [Parameter(Mandatory=$true)][string]$RouteExe,
    [Parameter(Mandatory=$true)][string]$ProjectRoot,
    [Parameter(Mandatory=$true)][string]$GoalId,
    [Parameter(Mandatory=$true)][string]$WorkId,
    [int]$Samples = 20
)
$ErrorActionPreference = 'Stop'
if ($Samples -lt 1 -or $Samples -gt 100) { throw 'Samples must be 1..100' }
$RouteExe = (Resolve-Path -LiteralPath $RouteExe).Path
$ProjectRoot = (Resolve-Path -LiteralPath $ProjectRoot).Path
function StateFingerprint {
    $rows = @()
    foreach ($name in @('.route', '.route-basic')) {
        $statePath = Join-Path $ProjectRoot $name
        if (Test-Path -LiteralPath $statePath) {
            $rows += Get-ChildItem -LiteralPath $statePath -Recurse -File -Force | Sort-Object FullName | ForEach-Object {
                '{0}:{1}:{2}' -f $_.FullName, $_.Length, (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            }
        }
    }
    return ($rows -join "`n")
}
function MeasureRead([string]$Name, [string[]]$CommandArgs, [string]$InputJson = '') {
    $values = @()
    $last = ''
    for ($i = 0; $i -lt $Samples; $i++) {
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        if ($InputJson) { $last = $InputJson | & $RouteExe @CommandArgs }
        else { $last = & $RouteExe @CommandArgs }
        if ($LASTEXITCODE -ne 0) { throw "$Name read failed" }
        if ($InputJson -and -not (($last -join "`n") | ConvertFrom-Json).ok) { throw "$Name protocol read failed" }
        $watch.Stop()
        $values += $watch.Elapsed.TotalMilliseconds
    }
    $sorted = @($values | Sort-Object)
    $result = [ordered]@{
        name=$Name; samples=$Samples
        p50_ms=[Math]::Round($sorted[[int][Math]::Floor(($Samples-1)*0.5)],2)
        p95_ms=[Math]::Round($sorted[[int][Math]::Ceiling(($Samples-1)*0.95)],2)
        response_bytes=[Text.Encoding]::UTF8.GetByteCount(($last -join "`n"))
    }
    return $result
}
Push-Location -LiteralPath $ProjectRoot
try {
    $before = StateFingerprint
    $results = @(
        MeasureRead 'routing_projection_including_cli_and_io' @('team','status',$GoalId)
        MeasureRead 'implementer_context_including_cli_and_io' @('team','context',$GoalId,'--work-id',$WorkId)
        MeasureRead 'independent_context_including_cli_and_io' @('team','context',$GoalId,'--work-id',$WorkId,'--reviewer')
        MeasureRead 'knowledge_detector_including_cli_and_io' @('team','detect',$GoalId)
    )
    $baseRequest = @{protocol='route/1';request_id='base-performance-read';method='base.query';context=@{};params=@{component='duration parser';environment=@();independent_first=$false}} | ConvertTo-Json -Compress
    $results += MeasureRead 'base_query_including_cli_and_io' @('rpc') $baseRequest
    $after = StateFingerprint
    if ($before -cne $after) { throw 'Read-only state changed' }
    [ordered]@{schema='route.team.performance/1';scope='control plane only; no AI/model calls';reads=$Samples*5;durable_growth_bytes=0;state_fingerprint_unchanged=$true;results=$results} | ConvertTo-Json -Depth 8
} finally { Pop-Location }
