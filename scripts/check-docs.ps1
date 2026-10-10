# check-docs.ps1 — lightweight documentation validation
#
# No external dependencies. Verifies:
#   1. Every relative markdown link in README/docs/ROUTE* points to a real file.
#   2. Referenced example paths exist.
#   3. CLI commands/aliases mentioned in docs exist in the current parser
#      (tool/route/crates/route-cli/src/main.rs).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/check-docs.ps1
# Exit code is 0 on success, 1 if any check fails.

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $root

$failures = 0
function Fail([string]$msg) {
    Write-Host "[FAIL] $msg"
    $script:failures++
}

# --- 1. Relative links in markdown docs ---
$docs = @()
$docs += Get-ChildItem -Path $root -Filter '*.md' -File -ErrorAction SilentlyContinue
$docs += Get-ChildItem -Path (Join-Path $root 'docs') -Filter '*.md' -File -ErrorAction SilentlyContinue

foreach ($doc in $docs) {
    $text = Get-Content -Raw -LiteralPath $doc.FullName
    # relative links: ](path) or ](path#anchor), skip http(s)/mailto/data/absolute
    $matches = [regex]::Matches($text, '\]\(([^)]+)\)')
    foreach ($m in $matches) {
        $target = $m.Groups[1].Value
        if ($target -match '^(https?://|mailto:|data:|#|\\\\|/|>)') { continue }
        $pathOnly = ($target -split '#')[0]
        if ($pathOnly -eq '') { continue }
        $resolved = Join-Path $doc.DirectoryName $pathOnly
        if (-not (Test-Path -LiteralPath $resolved)) {
            Fail("broken link in $($doc.Name): $target")
        }
    }
}

# --- 2. Example paths referenced in docs ---
$exampleRefs = @(
    'examples/basic/constitution.md',
    'examples/basic/protocol.md',
    'examples/basic/reference-entry.json',
    'examples/basic/README.md'
)
foreach ($p in $exampleRefs) {
    if (-not (Test-Path -LiteralPath (Join-Path $root $p))) {
        Fail("missing example path: $p")
    }
}

# --- 3. CLI commands in docs exist in the parser ---
$parser = Get-Content -Raw -LiteralPath (Join-Path $root 'tool/route/crates/route-cli/src/main.rs')
# Known top-level command variants (from the Commands enum).
function To-Kebab([string]$s) {
    # PascalCase "AgentPlan" -> "agent-plan" (note: -creplace is case-sensitive)
    $s = $s -creplace '([a-z0-9])([A-Z])', '$1-$2'
    return $s.ToLowerInvariant()
}
$knownCommands = [regex]::Matches($parser, '(?m)^\s{4}(\w+)\s*(?:\{|\(|,)')
$known = @{}
foreach ($c in $knownCommands) {
    $v = $c.Groups[1].Value
    $known[$v.ToLowerInvariant()] = $true
    $known[(To-Kebab $v)] = $true
}
# Subcommand action names (begin/save/verify/...) are matched loosely; only
# top-level names are validated strictly.
$topLevelMentions = @(
    'route init', 'route status', 'route commit', 'route log', 'route rollback',
    'route task', 'route archive', 'route apply', 'route context', 'route reference',
    'route constitution', 'route protocol', 'route workflow', 'route profile',
    'route evolve', 'route emerge', 'route learn', 'route study', 'route memory',
    'route save', 'route check', 'route health', 'route guardian', 'route brain',
    'route strategy', 'route plan', 'route agent-plan', 'route study-apply'
)
foreach ($cmd in $topLevelMentions) {
    $name = $cmd -replace '^route ', ''
    if (-not $known.ContainsKey($name)) {
        Fail("doc references '$cmd' but parser has no top-level '$name' command")
    }
}

# --- 4. Terminology consistency (spot checks) ---
# 'disaster recovery' as an unqualified promise is not allowed in user-facing docs.
foreach ($doc in $docs) {
    $text = Get-Content -Raw -LiteralPath $doc.FullName
    if ($text -match 'guaranteed disaster recovery' -or
        $text -match 'fully supported' -or
        $text -match 'automatically self-improves') {
        Fail("over-claim in $($doc.Name)")
    }
}

if ($failures -eq 0) {
    Write-Host "check-docs: OK"
    exit 0
} else {
    Write-Host "check-docs: $failures failure(s)"
    exit 1
}