param([string]$TargetDir)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$workspace = Join-Path $repo 'tool\route'
if (-not (Test-Path -LiteralPath (Join-Path $workspace 'Cargo.toml'))) {
    throw 'Route Rust workspace not found'
}
if ($env:RUSTFLAGS -or $env:CARGO_ENCODED_RUSTFLAGS) {
    throw 'Release build requires explicit review of existing Rust flags before path remapping'
}

# Rust can embed source locations in a release executable. Keep developer home
# and checkout paths out of the distributed binary, including dependency paths.
$flags = @(
    "--remap-path-prefix=$repo=/route-src",
    "--remap-path-prefix=$env:USERPROFILE=/build-home"
)
$oldTarget = $env:CARGO_TARGET_DIR
$env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
try {
    if ($TargetDir) {
        $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($TargetDir)
    }
    Push-Location -LiteralPath $workspace
    try {
        & cargo build --release -p route-cli --locked
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed: $LASTEXITCODE" }
        $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR }
                  else { Join-Path $workspace 'target' }
        $binary = Join-Path $target 'release\route.exe'
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw 'Expected route.exe not found'
        }
        $item = Get-Item -LiteralPath $binary
        "BINARY=$binary"
        "BYTES=$($item.Length)"
        "SHA256=$((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash)"
    } finally {
        Pop-Location
    }
} finally {
    Remove-Item Env:\CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
    if ($null -eq $oldTarget) {
        Remove-Item Env:\CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_TARGET_DIR = $oldTarget
    }
}
