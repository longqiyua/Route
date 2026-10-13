# Worker 列表单快照优化（2026-10-11）

## 改动边界

原 `worker.list` 分别调用 Worker 描述和 presence 查询，各自完整加载、校验 ledger。
改为调用新增 Rust 辅助函数 `development::worker_descriptors_and_presences`，在一次完整
校验之后，从同一份事件快照生成两份原有投影。单项 Rust 查询 API 保留；RPC 的
`workers` / `presences` 字段及 `WORKER_STATE_UNAVAILABLE` 错误码不变。
这是 additive Rust API，不修改持久化 schema、权限、预算写入检查或默认配置。

没有缓存、不跳过历史哈希、不新增索引/后台服务；每个新请求仍重新加载并验证。
完整历史读取/验证的代码路径由两次变为一次，实际磁盘 I/O 字节未测。
`development.state` 原本已复用一次加载，并不是本次被优化的路径；Context Pack 的
多次加载也尚未处理，不能宣称全局一致快照已完成。

## 可重复测量

扩展现有 `daily_use_e2e::resource_budget_real_binary` 的查询列表，加入 `worker.list`：

```powershell
cargo test -p route-cli --features test-utils --test daily_use_e2e resource_budget_real_binary -- --ignored --nocapture
```

优化前、后均真实 exit=0，分别运行 80.08 / 71.48 秒（这不是纯 worker.list 耗时）。
每次测量 4 个数据集、5 种查询、每组 20 个独立进程，共 400 次查询。
原始报告：[优化前](worker-list-before-2026-10-11.json)、[优化后](worker-list-after-2026-10-11.json)。
方法和局限沿用 [历史查询基线](history-query-baseline-2026-10-11.md)：Windows，
未优化 debug/test-utils，包含进程启动/RPC/结果排空，不清理 OS 缓存、不控制背景负载。
这是同机顺序测量，不是随机交错 A/B 或发布版性能承诺。

优化前二进制 SHA-256：`3d1381330dacfefe525c8e6b7026153601cc6158843c0e22840d970085052d8b`。
优化后二进制 SHA-256：`19e0b34a4d36d8f23853b0743593a1189000ef75dd9642557902e43fca0f0efa`。
两次均基于 HEAD `eb9751ba3d563e116ffcf0bf36a102c840cc0fca` 的未提交工作树，
不是干净 HEAD 构建；以实际产物 SHA 区分。合成历史为 Finding，Worker 列表为空，
不能推广为大量真实 Worker/复杂事件的投影压力测试。

## worker.list 对照

| 历史数据集 | 前 p50/p95 ms | 后 p50/p95 ms | 前峰值 MiB | 后峰值 MiB |
| --- | ---: | ---: | ---: | ---: |
| 0 事件 | 39.16 / 48.02 | 38.00 / 47.79 | 13.93 | 13.95 |
| 1,000 事件 | 124.68 / 133.30 | 99.01 / 101.80 | 16.30 | 16.26 |
| 10,000 事件 | 928.45 / 965.27 | 478.96 / 502.17 | 36.52 | 34.68 |
| 2 事件 + 256 MiB 外部资料 | 38.68 / 46.06 | 37.91 / 51.20 | 14.69 | 14.71 |

10K 场景 p50 下降约 48.4%，p95 下降约 48.0%，峰值工作集约减少 1.84 MiB。
小历史内存几乎无变化，LARGE 场景 p95 反而略升；不称作所有场景都改善。
本场景耗时收益明显，但内存收益有限，仍保留一次全量解析/校验成本。
所有基准查询检查整个临时项目内容和修改时间不变，两份报告 durable_growth 均为 0。

## 正确性验证与待办

- 新单元测试比较组合结果与原单项投影，覆盖 attached workspace、新元数据可见性、
  旧事件篡改拒绝、失败读取不自动重置历史。
- 新原生 RPC 测试使用非空 Worker/presence，确认输出完全等于原结构，读前后状态一致；
  篡改旧事件后仍返回原错误码，损坏文件保持原样。
- 完整工作区回归（排除 route-pyo3，包含 core/basic/CLI）真实 exit=0；basic 434 passed / 5 ignored，core 24 passed，CLI 单元 9 passed。fmt、文档链接、PowerShell parser 与 diff 检查通过；原有 warnings / ignored 不计作本轮解决。

未测 release、100K、真实复杂 Worker 历史、长期进程、并发读写竞态压力、实际 I/O/CPU 分项。
不证明先前约 400 MiB 常驻 Route 问题已解决，不将 T0/T5 整体升级为 PASS。
