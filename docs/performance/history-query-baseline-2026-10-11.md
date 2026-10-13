# 历史查询资源基线（2026-10-11）

这是 T0 的第一份最小实测，不是全部性能目标验收，也不是发布版性能承诺。
使用现有 `daily_use_e2e::resource_budget_real_binary`，未新增生产代码或基准框架。
原始 16 组结果见 [JSON 报告](history-query-baseline-2026-10-11.json)。
后续 Q7 在同一测试入口增加 `worker.list`，现运行会产生 20 组 / 400 次查询；
本报告保留扩展前的原始 16 组数据，不改写历史测量。新增路径对照见
[Worker 单快照报告](worker-list-single-snapshot-2026-10-11.md)。

## 可重复入口与范围

在 `tool/route` 执行：

```powershell
cargo test -p route-cli --features test-utils --test daily_use_e2e resource_budget_real_binary -- --ignored --nocapture
```

- Windows 原生进程，`debug/test-utils`、未优化且包含调试信息；本次命令 exit=0，测试 58.09 秒。构建时间不计入查询样本。
- 每个查询/数据集启动 20 个独立 Route 子进程；共 320 次查询。p50/p95 使用排序后第 10/19 个样本；峰值工作集为 20 个子进程中的最大值。
- 耗时包含新进程启动、RPC、查询、完整输出排空、等待退出与计量收尾，不是纯存储函数耗时。未清空 OS 文件缓存，不能称作冷缓存/热缓存对照；没有并发压力或机器负载归一化。
- 二进制 SHA-256：`7655f2e43f02dac633a01002c3705247f9b03911d25aca198b119cd6b4d2bf12`；44,045,824 bytes。
- 源码基于 HEAD `eb9751ba3d563e116ffcf0bf36a102c840cc0fca` 的当前未提交工作树（含 Q1–Q5），不是干净 HEAD 构建；以二进制 SHA 识别本次实际被测产物。
- 所有项目及 256 MiB 外部文件均为测试临时数据。合成历史使用现有测试专用 seed，经 canonical hash/verifier 检查；不是实际用户项目分布。
- 查询前后整个临时项目的路径、文件内容和修改时间一致，16 组 durable_growth 均为 0。响应正确性及事件查询 global_revision 断言通过。

## 关键结果

| 查询 | 历史事件数 | p50 ms | p95 ms | 子进程峰值工作集 MiB |
| --- | ---: | ---: | ---: | ---: |
| development.events.query | 0 | 52.00 | 70.58 | 13.99 |
| development.events.query | 1,000 | 92.82 | 103.67 | 16.31 |
| development.events.query | 10,000 | 491.63 | 515.90 | 34.71 |
| development.state | 0 | 168.12 | 181.69 | 15.16 |
| development.state | 1,000 | 208.54 | 228.55 | 16.80 |
| development.state | 10,000 | 614.75 | 708.97 | 34.73 |

10,000 条合成历史的 durable_bytes 为 5,697,084。事件查询返回约 41.8 KiB，
返回量受 limit 限制，但延迟和峰值工作集仍随总历史增长。
`cooperation.list` 即使返回 312 bytes，在 10,000 条历史下 p50 仍为 493.37 ms、
峰值 34.78 MiB；`reference.list` 在相同数据集 p50 为 46.54 ms、峰值 13.67 MiB。
这些差异支持优先调查 ledger 读取/校验/投影路径，而不是单凭返回长度判断成本。
不能从这组端到端数据独立分离 JSON 解析、哈希、投影和 RPC 各自的贡献。

注册 256 MiB 外部文件后的列表查询：`reference.list` p50 42.99 ms、峰值 14.48 MiB；
`cooperation.list` p50 42.88 ms、峰值 14.89 MiB。仅这些列表路径没有观察到与外部文件等大的
进程工作集；没有测量实际 I/O 字节，不能推广到内容加载、索引、检索或所有 Wiki 路径。

## 下一步与未覆盖项

下一小项先分离单次 ledger load/verify 与状态投影成本，或检查同一只读请求内的重复加载，
用此基线做同环境前后对照。不得以省略哈希校验、放宽预算检查或静默缓存过期状态换速度。

尚未测量：发布构建、100K 事件、真实复杂事件分布、同进程重复取消轮询、实际 team status、
读写字节、CPU 时间、并发写者/故障恢复、长期常驻占用、资料内容读取、1 GiB 输出以及旧新版本 RSS 对比。
本报告不证明先前约 400 MiB 的 Route 进程问题已解决，不升级 T0 整体为 PASS。
