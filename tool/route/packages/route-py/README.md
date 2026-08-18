# Route — Python Bindings

> **From Route to Routine.**
> 轻量级版本管理器，专为 Vibe Coding 设计。

Python 原生绑定，基于 [PyO3](https://pyo3.rs/) 构建，性能接近 C 语言。
你可以在 Python 脚本中直接调用 Route 的核心功能：仓库管理、分支操作、标签管理、对话跟踪等。

---

## 目录

- [系统要求](#系统要求)
- [安装](#安装)
- [快速开始](#快速开始)
- [API 参考](#api-参考)
- [完整示例](#完整示例)
- [常见问题](#常见问题)
- [License](#license)

---

## 系统要求

| 依赖 | 版本要求 |
|------|----------|
| Python | ≥ 3.11 |
| Rust 工具链 | ≥ 1.70（仅本地构建时需要） |
| OS | Windows / macOS / Linux |

Python 包使用 `abi3-py311` 稳定 ABI，编译一次即可在 Python 3.11/3.12/3.13 上运行。

---

## 安装

### 方式一：从源码构建（推荐开发阶段使用）

```bash
# 1. 克隆仓库
git clone https://github.com/longqiyua/route.git
cd route

# 2. 安装 maturin（构建工具，仅需一次）
pip install maturin

# 3. 构建并安装到当前 Python 环境
maturin develop -m packages/route-py/pyproject.toml

# 4. 验证安装
python -c "import route; print(route.__version__)"
```

### 方式二：直接使用编译产物

如果你不想安装 Rust 工具链，也可以直接使用编译好的 `.pyd` / `.so` 文件：

```bash
# 先构建
cargo build --release -p route-pyo3

# 复制到项目目录（Windows）
copy target\release\route.dll .\route.pyd

# 或者复制到 Python 的 site-packages
copy target\release\route.dll "$(python -c "import site; print(site.getsitepackages()[0])")\route.pyd"
```

### 方式三：pip 安装（发布后）

```bash
pip install route-vc
```

---

## 快速开始

### 1. 初始化仓库

```python
import route

# 在现有项目目录中初始化 Route 仓库
route.init("/path/to/your/project")

# 如果已有 Route 仓库，直接打开
# route.open("/path/to/your/project")
```

### 2. 查看状态

```python
status = route.status()
print(f"项目路径: {status['project_path']}")
print(f"当前分支: {status['current_branch']}")
print(f"模式: {status['mode']}")

# 分支列表
for b in status['branches']:
    marker = "← 当前" if b['is_current'] else ""
    print(f"  {b['name']} ({b['kind']}) {marker}")
```

### 3. 提交变更

```python
# 基本提交
route.commit("修复登录页面的样式问题")

# 详细提交
route.commit(
    "重构用户认证模块",
    author="developer",
    body="将 JWT 验证逻辑抽取为独立服务",
    is_checkpoint=True,   # 标记为检查点
)
```

### 4. 查看历史

```python
# 最近 10 条提交
for c in route.log(limit=10):
    tag = "📌" if c['is_checkpoint'] else ""
    diff = c.get('diff_summary', {})
    summary = f"+{diff.get('added', 0)} ~{diff.get('modified', 0)} -{diff.get('removed', 0)}" if diff else ""
    print(f"  {tag} {c['id'][:12]} {c['message']}  {summary}")
```

### 5. 分支管理

```python
# 创建分支
route.branch_create("feature-search", kind="inherited")
# kind 可选: "main" / "inherited" / "sandbox"

# 切换分支
route.branch_switch("feature-search")

# 查看所有分支
for b in route.branch_list():
    print(f"  {b['name']} ({b['kind']})")

# 删除分支
route.branch_delete("old-feature")
```

### 6. 标签管理

```python
# 创建标签
route.tag_create("v1.0", message="首个正式版本")

# 列出所有标签
for t in route.tag_list():
    print(f"  {t['name']} → {t['snapshot_id'][:12]}")

# 删除标签
route.tag_delete("v0.9-beta")
```

### 7. 对话跟踪

```python
# 创建对话会话
session = route.conversation_new("讨论代码重构方案")

# 记录对话
route.conversation_record(session, "user", "这段代码如何优化性能？")
route.conversation_record(session, "ai", "建议使用缓存策略，以下是具体方案...")

# 查看会话详情
detail = route.conversation_show(session)
for m in detail['messages']:
    print(f"  [{m['role']}] {m['content'][:50]}...")

# 回滚到指定消息（会恢复对应的快照）
route.conversation_rollback(session, "msg-xxxxx", reason="用户选择回退")

# 归档会话
route.conversation_archive(session)

# 删除会话
route.conversation_delete(session)
```

### 8. 回滚操作

```python
# 查看快照历史
snapshots = route.history(limit=5)
for s in snapshots:
    print(f"  {s['id'][:12]} 分支:{s['branch']}")

# 回滚到指定快照
route.rollback("snap-xxxxx", reason="用户主动回滚")

# 撤销 / 重做
route.undo()
route.redo()
```

### 9. 备份与注释

```python
# 备份当前 HEAD 到指定目录
backup_path = route.backup("/path/to/backup/dir")
print(f"备份完成: {backup_path}")

# 获取最近一次提交的 ID
commits = route.log(limit=1)
cid = commits[0]['id']

# 给提交添加注释
ann = route.annotate(cid, "此次提交重构了用户认证模块")
print(f"注释已添加: {ann['id'][:12]}")

# 查看提交的所有注释
for a in route.annotations(cid):
    print(f"  [{a['id'][:12]}] {a['text']}")
```

### 10. 自动打开当前目录仓库

```python
# 自动检测当前目录是否为 Route 仓库
if route.auto_open():
    print("已打开当前目录仓库")
else:
    print("当前目录没有 Route 仓库")
```

---

## API 参考

### 仓库操作

| 函数 | 说明 | 返回值 |
|------|------|--------|
| `init(path=None)` | 初始化新仓库 | `str` (项目路径) |
| `open(path=None)` | 打开已有仓库 | `str` (项目路径) |
| `status()` | 查看仓库状态 | `dict` |
| `commit(message, ...)` | 提交变更 | `dict` |
| `log(limit=20, branch=None)` | 查看提交历史 | `list[dict]` |
| `rollback(snapshot_id, reason=None)` | 回滚到指定快照 | `dict` |
| `changes()` | 查看工作区变更 | `list[dict]` |
| `diff(from, to)` | 比较两个快照差异 | `list[dict]` |
| `undo()` | 撤销上次提交 | `dict` |
| `redo()` | 重做上次撤销 | `dict` |
| `history(limit=20)` | 查看快照历史 | `list[dict]` |
| `checkpoint(title, body=None)` | 创建检查点 | `dict` |
| `stats()` | 仓库统计信息 | `dict` |
| `all_snapshots()` | 所有快照 | `list[dict]` |
| `backup(target)` | 完整备份当前 HEAD 到目录 | `str` (备份路径) |
| `annotate(commit_id, text)` | 给提交附加文本注释 | `dict` |
| `annotations(commit_id)` | 列出提交的所有注释 | `list[dict]` |

### 分支操作

| 函数 | 说明 | 返回值 |
|------|------|--------|
| `branch_list()` | 列出所有分支 | `list[dict]` |
| `branch_create(name, kind="inherited", from_branch=None)` | 创建分支 | `dict` |
| `branch_switch(name)` | 切换分支 | `None` |
| `branch_delete(name)` | 删除分支 | `None` |

### 标签操作

| 函数 | 说明 | 返回值 |
|------|------|--------|
| `tag_list()` | 列出所有标签 | `list[dict]` |
| `tag_create(name, message=None)` | 创建标签 | `dict` |
| `tag_delete(name)` | 删除标签 | `None` |

### 对话跟踪

| 函数 | 说明 | 返回值 |
|------|------|--------|
| `conversation_new(title)` | 创建对话会话 | `str` (session ID) |
| `conversation_list()` | 列出所有会话 | `list[dict]` |
| `conversation_show(session_id, limit=None)` | 查看会话消息 | `dict` |
| `conversation_record(session_id, role, content, snapshot_id=None)` | 记录消息 | `dict` |
| `conversation_rollback(session_id, message_id, reason=None)` | 回滚到指定消息 | `dict` |
| `conversation_archive(session_id)` | 归档会话 | `None` |
| `conversation_delete(session_id)` | 删除会话 | `None` |

### 便捷函数

| 函数 | 说明 | 返回值 |
|------|------|--------|
| `get_project_path()` | 获取当前仓库路径 | `str` 或 `None` |
| `require_repo()` | 检查仓库是否打开 | `None` (无仓库时抛异常) |
| `auto_open()` | 自动打开当前目录仓库 | `bool` |

---

## 完整示例

以下是一个完整的 Python 脚本，演示从初始化到对话跟踪的完整流程：

```python
#!/usr/bin/env python3
"""Route 使用示例 — 完整的 Vibe Coding 工作流。"""

import route
import os
import sys


def main():
    project = os.getcwd()

    # 1. 初始化或打开仓库
    if route.auto_open():
        print(f"✅ 已打开仓库: {project}")
    else:
        route.init(project)
        print(f"✅ 已初始化仓库: {project}")

    # 2. 查看状态
    s = route.status()
    print(f"\n📊 当前状态:")
    print(f"   分支: {s['current_branch']}")
    print(f"   快照数: {len(route.all_snapshots())}")

    # 3. 提交当前变更
    changed = route.changes()
    if changed:
        print(f"\n📝 检测到 {len(changed)} 个文件变更:")
        for f in changed:
            print(f"   {f['change']:>8}  {f['path']}")

        route.commit(
            f"自动提交: {len(changed)} 个文件变更",
            operator="auto",
            is_ai=True,
            body="AI 自动提交",
        )
        print("✅ 提交成功")

    # 4. 创建分支
    branches = route.branch_list()
    has_feature = any(b['name'] == 'feature-ai' for b in branches)
    if not has_feature:
        route.branch_create("feature-ai", kind="inherited")
        print(f"\n🌿 创建分支: feature-ai")
    route.branch_switch("feature-ai")
    print(f"🌿 切换到: feature-ai")

    # 5. 记录对话
    session = route.conversation_new("AI 辅助开发")
    route.conversation_record(session, "user", "帮我分析项目结构并优化代码")
    route.conversation_record(session, "ai", "分析完成，建议优化以下模块...")
    print(f"\n💬 对话已记录 (session: {session[:16]}...)")

    # 6. 查看统计
    stats = route.stats()
    print(f"\n📈 仓库统计:")
    print(f"   分支: {stats['branches']['total']}")
    print(f"   快照: {stats['snapshots']}")
    print(f"   提交: {stats['commits']['total']}")

    # 7. 创建标签
    route.tag_create(f"v{stats['commits']['total']}", message="自动标签")
    print(f"🏷️  标签已创建")

    # 8. 备份当前状态
    backup_dir = os.path.join(os.path.expanduser("~"), ".route-backups")
    os.makedirs(backup_dir, exist_ok=True)
    path = route.backup(backup_dir)
    print(f"💾 备份已保存: {path}")

    # 9. 给最新提交加注释
    commits = route.log(limit=1)
    if commits:
        route.annotate(commits[0]['id'], "AI 自动备份 + 标签")
        print(f"🏷️  注释已附加到提交 {commits[0]['id'][:12]}")

    print(f"\n🎉 完成! 使用 route.status() 查看最新状态")


if __name__ == "__main__":
    main()
```

---

## 常见问题

### Q: 导入时报 `ModuleNotFoundError: No module named 'route'`

**原因**：Python 找不到编译后的原生模块。

**解决方案**：

```bash
# 方案 1：设置 PYTHONPATH
$env:PYTHONPATH = "target/release"    # Windows PowerShell
export PYTHONPATH=target/release      # macOS / Linux

# 方案 2：使用 maturin develop 安装到当前环境
pip install maturin
maturin develop -m packages/route-py/pyproject.toml

# 方案 3：手动复制到 site-packages
python -c "
import site, shutil
shutil.copy('target/release/route.dll', f'{site.getsitepackages()[0]}/route.pyd')
"
```

### Q: 报 `ImportError: DLL load failed` 错误

**原因**：Windows 上缺少 VC++ 运行时库。

**解决方案**：安装 [Microsoft Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe)。

### Q: 报 `OSError: %1 is not a valid Win32 application`

**原因**：Rust 编译的目标架构（64位）与 Python 解释器（32位）不匹配。

**解决方案**：确保两者架构一致，建议都使用 64 位。

```bash
# 检查 Python 架构
python -c "import struct; print('32-bit' if struct.calcsize('P') * 8 == 32 else '64-bit')"

# 使用 64 位 Rust 工具链
rustup default stable-x86_64-pc-windows-msvc
```

### Q: 如何确认模块已正确安装？

```bash
python -c "
import route
print(f'版本: {route.__version__}')
print(f'路径: {route.__file__ if hasattr(route, \"__file__\") else \"原生模块\"}')
print(f'API 总数: {len([x for x in dir(route) if not x.startswith(\"_\")])}')
"
```

### Q: 编译时提示 `ToPyObject` 已弃用

这是 PyO3 0.23 的 deprecation warning，不影响功能。将在后续版本中迁移到 `IntoPyObject`。

---

## License

AGPL-3.0 — See [LICENSE](../../LICENSE) for details.