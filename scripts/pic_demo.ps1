# PIC Demo — Code Mode logic simulation
# Proves PIC can work without loading full Route history.
# Flow: task → route context --json → checkpoint → filesystem work → verification → repair → evidence

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$RouteExecutable = Join-Path $ProjectRoot "tool\route\target\release\route.exe"

Write-Host "=== PIC Demo: Code Mode Logic ===" -ForegroundColor Cyan
Write-Host ""

# ---- Step 1: Route context --json ----
Write-Host "Step 1: route context task --json" -ForegroundColor Yellow
$contextJson = & $RouteExecutable context task "PIC demo: verify file integrity" --json 2>&1 | Out-String
$context = $contextJson | ConvertFrom-Json
Write-Host "  Task: $($context.task)"
Write-Host "  Profile: $($context.profile_id)"
Write-Host "  References: $($context.references.Count)"
Write-Host "  Protocol: present=$($context.protocol_body -ne $null)"
Write-Host ""

# ---- Step 2: Route status --json ----
Write-Host "Step 2: route status --json" -ForegroundColor Yellow
$statusJson = & $RouteExecutable status --json 2>&1 | Out-String
$status = $statusJson | ConvertFrom-Json
Write-Host "  Project ID: $($status.project.id)"
Write-Host "  Branch: $($status.project.branch)"
Write-Host "  Integrity: $($status.project.integrity)"
Write-Host "  Active sessions: $($status.work.active_sessions.Count)"
Write-Host ""

# ---- Step 3: Checkpoint ----
Write-Host "Step 3: route checkpoint --json" -ForegroundColor Yellow
$cpJson = & $RouteExecutable checkpoint "pic-demo-before-work" --json 2>&1 | Out-String
$cp = $cpJson | ConvertFrom-Json
Write-Host "  Checkpoint: $($cp.id)"
Write-Host "  Title: $($cp.title)"
Write-Host ""

# ---- Step 4: Filesystem / shell work (simulated) ----
Write-Host "Step 4: Simulated work (filesystem check)" -ForegroundColor Yellow
$routeDir = Join-Path $ProjectRoot ".route"
if (Test-Path $routeDir) {
    $items = Get-ChildItem $routeDir -Recurse -File | Select-Object -First 10
    Write-Host "  .route/ contains $($items.Count) files (showing first 10):"
    foreach ($item in $items) {
        Write-Host "    $($item.FullName.Replace($ProjectRoot, ''))"
    }
}
Write-Host ""

# ---- Step 5: Verification --json ----
Write-Host "Step 5: route check --json" -ForegroundColor Yellow
$checkJson = & $RouteExecutable check --json 2>&1 | Out-String
$check = $checkJson | ConvertFrom-Json
Write-Host "  Status: $($check.status)"
Write-Host "  Snapshots: $($check.snapshots_checked)"
Write-Host "  Findings: $($check.findings.Count)"
Write-Host ""

# ---- Step 6: Repair plan (if findings exist) ----
if ($check.findings.Count -gt 0) {
    Write-Host "Step 6: route repair-plan --json (findings detected)" -ForegroundColor Yellow
    $repairJson = & $RouteExecutable repair-plan --json 2>&1 | Out-String
    $repair = $repairJson | ConvertFrom-Json
    Write-Host "  Status: $($repair.status)"
    Write-Host "  Repairable: $($repair.repairable)"
    Write-Host "  Findings: $($repair.finding_count)"
    foreach ($f in $repair.findings) {
        Write-Host "    [$($f.severity)] $($f.code): $($f.repair)"
    }
} else {
    Write-Host "Step 6: No findings — repair not needed" -ForegroundColor Green
}
Write-Host ""

# ---- Step 7: Archive save (execution evidence) ----
Write-Host "Step 7: route archive save (execution evidence)" -ForegroundColor Yellow
$saveResult = & $RouteExecutable archive save "pic-demo-automated-evidence" 2>&1 | Out-String
Write-Host "  $saveResult"
Write-Host ""

# ---- Step 8: Archive list --json (evidence readback) ----
Write-Host "Step 8: route archive list --json (evidence readback)" -ForegroundColor Yellow
$listJson = & $RouteExecutable archive list --json 2>&1 | Out-String
$list = $listJson | ConvertFrom-Json
Write-Host "  Project: $($list.project_id)"
Write-Host "  Total saves: $($list.count)"
Write-Host ""

Write-Host "=== PIC Demo Complete ===" -ForegroundColor Cyan
Write-Host "Key takeaway: PIC never loaded full Route history." -ForegroundColor Green
Write-Host "All state was read on-demand via --json CLI bridge." -ForegroundColor Green
