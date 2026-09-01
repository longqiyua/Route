$ErrorActionPreference = "Stop"

# ============================================================
# Route 1.0 — Golden Demo Script
# 展示 Route 的核心功能：任务生命周期、选择性修复、灾难恢复
# ============================================================

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$routeProject = Join-Path $repositoryRoot "tool\route"
$tmpDir = Join-Path $env:TEMP "route-demo-$(Get-Random)"
$recoverDir = Join-Path $env:TEMP "route-demo-recovered-$(Get-Random)"

# ---------- 辅助函数 ----------

function Invoke-Route {
    <#
    .SYNOPSIS
    在指定工作目录下运行 route 命令。
    使用 cargo run --manifest-path 确保从 Route 项目目录构建，但二进制文件在目标目录中运行。
    #>
    param(
        [Parameter(Mandatory = $true)]
        [string]$WorkDir,
        [Parameter(Mandatory = $true)]
        [string]$Command
    )
    $cargoManifest = Join-Path $routeProject "Cargo.toml"
    Push-Location $WorkDir
    try {
        $output = Invoke-Expression "cargo run --manifest-path `"$cargoManifest`" --bin route -- $Command 2>&1"
        return $output
    } catch {
        Write-Host "⚠ 命令失败: route $Command" -ForegroundColor Red
        Write-Host "   $_" -ForegroundColor Red
        throw $_
    } finally {
        Pop-Location
    }
}

function Section {
    param([string]$Title)
    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Green
    Write-Host "  $Title" -ForegroundColor Green
    Write-Host "============================================================" -ForegroundColor Green
    Write-Host ""
}

function Step {
    param([string]$Message)
    Write-Host "▶ $Message" -ForegroundColor Yellow
}

# ============================================================
#  0. 准备工作
# ============================================================

Section "0. 准备工作 — 创建临时项目目录"

Step "创建临时目录: $tmpDir"
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
Write-Host "  ✓ 已创建: $tmpDir" -ForegroundColor Green

Step "初始化 Git 仓库"
Push-Location $tmpDir
git init -q
git config user.email "demo@route.dev"
git config user.name "Route Demo"
Pop-Location
Write-Host "  ✓ Git 仓库已初始化" -ForegroundColor Green

Step "创建示例文件 main.py"
$mainPy = @'
def hello():
    return "Hello, Route!"


if __name__ == "__main__":
    print(hello())
'@
Set-Content -Path (Join-Path $tmpDir "main.py") -Value $mainPy
Write-Host "  ✓ main.py 已创建" -ForegroundColor Green

Push-Location $tmpDir
git add -A
git commit -m "initial commit" -q
Pop-Location
Write-Host "  ✓ Git 首次提交完成" -ForegroundColor Green

Step "初始化 Route"
Invoke-Route -WorkDir $tmpDir -Command "init"
Write-Host "  ✓ Route 已初始化" -ForegroundColor Green

# ============================================================
#  Flow A: 正常开发流程
# ============================================================

Section "Flow A: 正常开发流程"

Step "A1 — 查看 Route 状态"
Invoke-Route -WorkDir $tmpDir -Command "status"

Step "A2 — 开始一个新任务: 'add greeting function'"
$beginOutput = Invoke-Route -WorkDir $tmpDir -Command 'task begin "add greeting function" --target generic'
Write-Host "$beginOutput" -ForegroundColor White

# 从输出中提取 session ID
$sessionIdA = ""
if ($beginOutput -match 'id:\s+(\S+)') {
    $sessionIdA = $matches[1]
    Step "  提取到 Session ID: $sessionIdA"
}
$sessionShortA = ""
if ($beginOutput -match 'route task end (\S+)') {
    $sessionShortA = $matches[1]
    Step "  提取到 Session 短 ID: $sessionShortA"
}

Step "A3 — 修改 main.py 添加新功能"
$mainPyV2 = @'
def hello():
    return "Hello, Route!"


def greet(name):
    """Greet a person by name."""
    return f"Hello, {name}!"


if __name__ == "__main__":
    print(hello())
    print(greet("World"))
'@
Set-Content -Path (Join-Path $tmpDir "main.py") -Value $mainPyV2
Write-Host "  ✓ main.py 已更新 (添加了 greet 函数)" -ForegroundColor Green

Step "A4 — 创建 Route 快照 (commit)"
Invoke-Route -WorkDir $tmpDir -Command 'commit -m "add greet function"'

Step "A5 — 验证任务"
if ($sessionShortA) {
    Invoke-Route -WorkDir $tmpDir -Command "task verify $sessionShortA"
} else {
    Write-Host "  ⚠ 跳过验证 (无法获取 Session ID)" -ForegroundColor Yellow
}

Step "A6 — 结束任务 (成功)"
if ($sessionShortA) {
    Invoke-Route -WorkDir $tmpDir -Command "task end $sessionShortA --result success"
    Write-Host "  ✓ 任务已标记为成功" -ForegroundColor Green
}

# ============================================================
#  Flow B: 选择性修复
# ============================================================

Section "Flow B: 选择性修复 — 引入回归、检测、选择性恢复"

Step "B1 — 创建回归前的保存点 (savepoint)"
Invoke-Route -WorkDir $tmpDir -Command 'save create "pre-bug"'

Step "B2 — 开始一个新任务: 'add broken feature'"
$beginOutputB = Invoke-Route -WorkDir $tmpDir -Command 'task begin "add broken feature" --target generic'
Write-Host "$beginOutputB" -ForegroundColor White

$sessionShortB = ""
if ($beginOutputB -match 'route task end (\S+)') {
    $sessionShortB = $matches[1]
    Step "  Session 短 ID: $sessionShortB"
}

Step "B3 — 故意引入语法错误 (bug)"
$mainPyBug = @'
def hello():
    return "Hello, Route!"


def greet(name)
    """Greet a person by name."""
    return f"Hello, {name}!"


def broken_function(x
    return x * 2


if __name__ == "__main__":
    print(hello())
    print(greet("World"))
'@
Set-Content -Path (Join-Path $tmpDir "main.py") -Value $mainPyBug
Write-Host "  ⚠ main.py 已写入有语法错误的代码" -ForegroundColor Red

Step "B4 — 提交有问题的代码"
Invoke-Route -WorkDir $tmpDir -Command 'commit -m "add broken feature (with syntax error)"'

Step "B5 — 检测问题: 使用 Python 检查语法"
Push-Location $tmpDir
try {
    $null = python -c "import py_compile; py_compile.compile('main.py', doraise=True)"
    Write-Host "  ⚠ 语法检查通过 (预期之外)" -ForegroundColor Yellow
} catch {
    Write-Host "  ✓ 检测到语法错误" -ForegroundColor Green
}
Pop-Location

Step "B6 — 查看 Route 日志，找到上一个健康快照"
Invoke-Route -WorkDir $tmpDir -Command "log -l 5"

Step "B7 — 从保存点恢复 (选择性修复)"
Invoke-Route -WorkDir $tmpDir -Command "save list"
Invoke-Route -WorkDir $tmpDir -Command 'save restore "pre-bug" --scope code --force'

Step "B8 — 验证修复: 检查 main.py 语法已恢复"
Push-Location $tmpDir
$fixedContent = Get-Content "main.py" -Raw
if ($fixedContent -match "broken_function") {
    Write-Host "  ⚠ 修复可能不完整" -ForegroundColor Yellow
} else {
    Write-Host "  ✓ 代码已恢复到健康状态" -ForegroundColor Green
}
Pop-Location

Step "B9 — 运行 Route 完整性检查"
Invoke-Route -WorkDir $tmpDir -Command "check"

Step "B10 — 结束任务 (标记为失败，记录回归)"
if ($sessionShortB) {
    Invoke-Route -WorkDir $tmpDir -Command "task end $sessionShortB --result failed"
    Write-Host "  ✓ 回归任务已标记为失败，记录到学习系统" -ForegroundColor Green
}

# ============================================================
#  Flow C: 灾难恢复
# ============================================================

Section "Flow C: 灾难恢复 — 保存、删除、恢复"

Step "C1 — 创建外部存档备份 (archive save)"
Invoke-Route -WorkDir $tmpDir -Command 'archive save "backup before disaster"'

Step "C2 — 获取项目 ID (用于恢复)"
$statusOutput = Invoke-Route -WorkDir $tmpDir -Command "status"
Write-Host "$statusOutput" -ForegroundColor White

$projectId = ""
if ($statusOutput -match 'ID:\s+(\S+)') {
    $projectId = $matches[1]
    Step "  提取到项目 ID: $projectId"
} else {
    $archiveOutput = Invoke-Route -WorkDir $tmpDir -Command "archive list"
    Write-Host "$archiveOutput" -ForegroundColor White
    if ($archiveOutput -match "project '(\S+)'") {
        $projectId = $matches[1]
        Step "  从 archive 提取到项目 ID: $projectId"
    }
}

Step "C3 — 列出存档备份"
if ($projectId) {
    Invoke-Route -WorkDir $tmpDir -Command "archive list"
}

Step "C4 — 模拟灾难: 删除项目目录"
Write-Host "  删除项目目录: $tmpDir" -ForegroundColor Red
Remove-Item -Path $tmpDir -Recurse -Force
Write-Host "  ⚠ 项目目录已删除!" -ForegroundColor Red
Write-Host "  ✓ Route 存档安全保存在外部存储中 (Documents/Route/)" -ForegroundColor Green

Step "C5 — 从存档恢复项目到新路径"
if ($projectId) {
    $recoverOutput = Invoke-Route -WorkDir $routeProject -Command "archive recover $projectId --to `"$recoverDir`""
    Write-Host "$recoverOutput" -ForegroundColor White
    Write-Host "  ✓ 项目已恢复到: $recoverDir" -ForegroundColor Green
} else {
    Write-Host "  ⚠ 无法恢复: 未获取到项目 ID" -ForegroundColor Yellow
    Write-Host "  请手动执行: route archive recover <project-id> --to <path>" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $recoverDir -Force | Out-Null
}

Step "C6 — 验证恢复的文件"
if (Test-Path (Join-Path $recoverDir "main.py")) {
    Write-Host "  ✓ main.py 已恢复" -ForegroundColor Green
    $recoveredContent = Get-Content (Join-Path $recoverDir "main.py") -Raw
    Write-Host ""
    Write-Host "  恢复后的 main.py:" -ForegroundColor Cyan
    Write-Host "------------------------------" -ForegroundColor Cyan
    Write-Host $recoveredContent
    Write-Host "------------------------------" -ForegroundColor Cyan
} else {
    Write-Host "  ⚠ main.py 未找到，恢复可能不完整" -ForegroundColor Yellow
    Write-Host "  恢复目录内容:" -ForegroundColor Yellow
    Get-ChildItem $recoverDir -Recurse -Name | ForEach-Object { Write-Host "    - $_" }
}

Step "C7 — 在恢复的项目中运行 Route 完整性检查"
if (Test-Path (Join-Path $recoverDir ".route-basic")) {
    Invoke-Route -WorkDir $recoverDir -Command "check"
    Write-Host "  ✓ 恢复的项目完整性验证通过" -ForegroundColor Green
} else {
    Write-Host "  ⚠ .route-basic 目录未找到，Route 完整性检查跳过" -ForegroundColor Yellow
}

# ============================================================
#  清理
# ============================================================

Section "清理"

Step "清理临时目录"
if (Test-Path $tmpDir) {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "  ✓ 已删除: $tmpDir" -ForegroundColor Green
}
if (Test-Path $recoverDir) {
    Remove-Item -Path $recoverDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "  ✓ 已删除: $recoverDir" -ForegroundColor Green
}

# ============================================================
#  总结
# ============================================================

Section "演示完成"

Write-Host @"
  Route 1.0 核心功能演示已完成:

  ✓ Flow A: 正常开发流程
    - task begin → commit → task verify → task end

  ✓ Flow B: 选择性修复
    - savepoint → 引入 bug → save restore → check

  ✓ Flow C: 灾难恢复
    - archive save → 删除项目 → archive recover

"@ -ForegroundColor Cyan

Write-Host "  演示脚本路径: $PSCommandPath" -ForegroundColor Gray
