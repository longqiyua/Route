# Route Self-Referential Test Report

> 测试日期：2026-08-04
> 测试项目：[DashBoard](https://github.com/longqiyua/DashBoard)（Spring Boot 任务看板）
> Route 版本：v0.5.0-beta

---

## 测试结果总览

| # | 测试项 | 命令 | 状态 |
|---|--------|------|------|
| 1 | Root Base 状态 | `route base status` | ✅ |
| 2 | 语义模糊搜索 | `route base search "task controller"` | ✅ |
| 3 | 项目状态 | `route status` | ✅ |
| 4 | 记忆统计 | `route base memory` | ✅ |
| 5 | 自反内省 | `route base self-manage introspect` | ✅ |
| 6 | 提交日志 | `route log` | ✅ |
| 7 | GUI 状态 | `route gui status` | ✅ |
| 8 | 权限状态 | `route permission status` | ✅ |
| 9 | 项目上下文 | `route project-context` | ✅ |
| 10 | MCP 配置 | `route mcp --config` | ✅ |
| 11 | AI 配置 | `route ai config` | ✅ |
| 12 | 基准测试 | `route bench run default` | ✅ |

**总计：12/12 通过，100%**

---

## 详细结果

### 1. Root Base Status

```
Root Base Status
  Project:         .
  Memory Mode:     ON
  Causal Control:  ON
  Auto Git:        ON
  Adaptive Debounce: ON
  GUI Desktop:     DISABLED (hidden)

Memory:
  Entries:         0
  Causal Chains:   0
  Hot Blocks:      0
  Cold Blocks:     0
  Estimated Bytes: 0
```

### 2. 语义模糊搜索

引擎已初始化，HybridVectorizer 日志：
```
HybridVectorizer::new — mode=Auto feature_dim=128 ml_enabled=true ml_min_query_len=5
HashedNGramEmbedding::new — feature_dim=128 ngram_range=1..4 learning_rate=0.01
```

### 3. 项目状态

```
Project:  C:\Users\longq\Desktop\route (1)\sandbox\DashBoard
Mode:     basic
Branch:   main (current)
Branches: 1
```

### 4. 记忆统计

```
Total entries:  0
Causal chains:  0
```

### 5. 自反内省

```
Route Self-Introspection
  ✓ 0 files parsed, 0 symbols indexed
  ✓ Hot: 0 blocks, Cold: 0 blocks
  ✓ Causal link recorded
  Causal chains: 1
```

### 6. 提交日志

```
(no commits yet)
```

### 7. GUI 状态

```
GUI Desktop App: DISABLED (hidden)
```

### 8. 权限状态

```
Current permission level: normal
```

### 9. 项目上下文

```
Git Branch: main
Recent Commits: 3 (LICENSE, maven.yml, 初始提交)
Skills: (empty)
References: (empty)
```

### 10. MCP 配置

```json
{
  "mcpServers": {
    "route": {
      "args": ["mcp"],
      "command": "route",
      "env": {}
    }
  }
}
```

### 11. AI 配置

```
Status: disabled (set ROUTE_AI_KEY to enable)
```

### 12. 基准测试

**Pass Rate: 100.00% (2 / 2)**

| Case | Status | Duration |
|------|--------|----------|
| HighFreq CRUD (100 keys, 1000 ops) | ✅ pass | 3ms |
| Structure Accuracy (d=3, b=3, f=5, rounds=100) | ✅ pass | 1ms |

- F1: 0.908 | Precision: 0.925 | Recall: 0.891
- Drift Score: 0.000%

---

## 混合向量化器日志验证

启动时 INFO 日志输出正常：
```
[INFO] HybridVectorizer::new — mode=Auto feature_dim=128 ml_enabled=true ml_min_query_len=5 ngram_range="1..4"
[INFO] HashedNGramEmbedding::new — feature_dim=128 ngram_range_start=1 ngram_range_end=4 learning_rate=0.01 ml_min_query_len=5 ml_enabled=true
```

---

*此报告由 Route 自反测试自动生成。*