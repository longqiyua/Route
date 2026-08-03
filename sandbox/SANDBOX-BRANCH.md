# Route Sandbox Branch

> **sandbox 分支** — Route 自反测试演示分支

---

## 分支说明

`sandbox` 分支是 Route 项目的**演示与自反测试分支**，包含：

1. **`sandbox/` 目录** — 完整的沙盒测试环境
2. **DashBoard 项目** — 真实 Spring Boot 任务看板，用于 Route 自反测试
3. **自反测试报告** — Route 对自身能力的全路径白盒测试结果

## 用途

| 用途 | 说明 |
|------|------|
| **演示** | 展示 Route 如何管理、分析、搜索真实项目 |
| **回归验证** | 确保 Route 语义引擎、RAG 管线、记忆系统对第三方项目有效 |
| **CI/CD 集成** | 作为 Route 集成测试的基准项目 |

## 使用方法

### 克隆此分支

```bash
git clone -b sandbox https://github.com/longqiyua/route.git route-sandbox
cd route-sandbox
```

### 运行 Route 自反测试

```bash
# 构建 Route
cargo build --release

# 初始化 DashBoard 项目
./target/release/route init --path sandbox/DashBoard

# 运行自反测试
cd sandbox/DashBoard
../../target/release/route base status
../../target/release/route base search "task controller"
../../target/release/route base memory
../../target/release/route base self-manage introspect
../../target/release/route bench run default
```

### 查看测试报告

```
sandbox/results/self-ref-test-2026-08-04.md
```

## 沙盒内容

```
sandbox/
├── SANDBOX.md                          ← 沙盒主说明
├── SANDBOX-BRANCH.md                   ← 本文件（分支说明）
├── DashBoard/                          ← 测试项目
│   ├── src/main/java/...               ← Java 源码
│   ├── pom.xml                         ← Maven 配置
│   └── .route-basic/                   ← Route 初始化
└── results/
    └── self-ref-test-2026-08-04.md     ← 自反测试报告
```

---

*此分支由 Route 自身管理。*