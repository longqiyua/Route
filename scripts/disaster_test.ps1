$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$cargoManifest = Join-Path $repositoryRoot "tool\route\Cargo.toml"
$tmpDir = Join-Path $env:TEMP "route-real-disaster-$(Get-Random)"
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

Write-Host "=== P0: REAL DISASTER TEST ==="
Write-Host "1. Creating test project at $tmpDir"

Push-Location $tmpDir

# Init
$out = cargo run --manifest-path "$cargoManifest" --bin route -- init 2>&1
Write-Host "Init: $out"

# Create a test file
Set-Content -Path (Join-Path $tmpDir "test.txt") -Value "HELLO WORLD" -Encoding utf8

# Task begin + end for verified save
$out = cargo run --manifest-path "$cargoManifest" --bin route -- task begin "disaster prep" 2>&1
Write-Host "$out"
$sid = ($out | Select-String -Pattern "id:\s+(\S+)").Matches.Groups[1].Value
$out = cargo run --manifest-path "$cargoManifest" --bin route -- task end $sid --result success 2>&1
Write-Host "$out"

# Verify archive exists
$out = cargo run --manifest-path "$cargoManifest" --bin route -- archive list 2>&1
Write-Host "Archive list: $out"

# Read the archive dir for the project ID
$archiveBase = Join-Path ([Environment]::GetFolderPath("MyDocuments")) "Route" "projects"
$projectDirs = Get-ChildItem $archiveBase -Directory
if ($projectDirs.Count -eq 0) {
    Write-Host "ERROR: No archive dirs found!"
    Pop-Location
    exit 1
}
$archiveDir = $projectDirs[0].FullName
$projectId = $projectDirs[0].Name

# Get hash of test.txt before deletion
$testFileHashBefore = (Get-FileHash (Join-Path $tmpDir "test.txt") -Algorithm SHA256).Hash
Write-Host "Project ID: $projectId"
Write-Host "test.txt hash before: $testFileHashBefore"

# 2. DELETE the entire project directory
$recoverPath = Join-Path $env:TEMP "route-recovered-$(Get-Random)"
Write-Host "2. Deleting project directory..."
Remove-Item -Path $tmpDir -Recurse -Force
if (Test-Path $tmpDir) {
    Write-Host "ERROR: Project dir still exists after deletion!"
    Pop-Location
    exit 1
}
Write-Host "   Deleted: $(Test-Path $tmpDir)"

# 3. Archive recover to new path
Write-Host "3. Recovering from archive..."
$out = cargo run --manifest-path "$cargoManifest" --bin route -- archive recover $projectId --path $recoverPath 2>&1
Write-Host "Recover: $out"

# 4. Check recovered directory
Write-Host "4. Verifying recovered project..."
if (-not (Test-Path $recoverPath)) {
    Write-Host "ERROR: Recovered path doesn't exist!"
    Pop-Location
    exit 1
}

$routeBasicDir = Join-Path $recoverPath ".route-basic"
if (-not (Test-Path $routeBasicDir)) {
    Write-Host "ERROR: .route-basic doesn't exist in recovered project!"
    Pop-Location
    exit 1
}
Write-Host "   .route-basic exists: YES"

# 5. Check file hash
$testFileHashAfter = (Get-FileHash (Join-Path $recoverPath "test.txt") -Algorithm SHA256).Hash
Write-Host "   test.txt hash after: $testFileHashAfter"
if ($testFileHashBefore -eq $testFileHashAfter) {
    Write-Host "   File hash MATCH: YES"
} else {
    Write-Host "   File hash MISMATCH!"
    Pop-Location
    exit 1
}

# 6. route status works
Push-Location $recoverPath
$out = cargo run --manifest-path "$cargoManifest" --bin route -- status 2>&1
Write-Host "Status: $out"

# 7. New task can be started
$out = cargo run --manifest-path "$cargoManifest" --bin route -- task begin "post-disaster" 2>&1
Write-Host "New task: $out"
$sid2 = ($out | Select-String -Pattern "id:\s+(\S+)").Matches.Groups[1].Value
$out = cargo run --manifest-path "$cargoManifest" --bin route -- task end $sid2 --result success 2>&1
Write-Host "$out"

Pop-Location
Write-Host "`nP0 PASSED: REAL DISASTER RECOVERY VERIFIED"
