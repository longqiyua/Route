# Route Execution Trust I — 实现与验收记录

日期：2026-10-12。仓库：Route；分支：fruit。
起始 HEAD：`eb9751ba3d563e116ffcf0bf36a102c840cc0fca`。

本轮保留既有混合工作树，不把前轮存储迁移、性能修复、TUI 外观和手册冒充本阶段成果。
本阶段状态为 **PARTIAL**：检查与收尾恢复已有真实进程测试；AI Host 重连、完整进程树隔离和真实模型重启 dogfood 尚未闭环。
不得据此升级 CrossHost、CrossModel 或 DAILY_USE_VERIFIED。

## 1. Q17：先诊断，再 RED，再修复

原测试在首次检查返回 exit=1，尚未到执行计数断言。
直接捕获原 argv 的 stderr 得到：`The filename, directory name, or volume label syntax is incorrect.`
根因是 Windows `cmd.exe /d /c` 重定向和路径引号的测试 fixture 组合错误，分类 **TEST_FIXTURE_DEFECT**。

仅将计数 fixture 换成原生 libtest 子进程后，旧生产代码完成两个标签并关闭 Goal，
随后真正 RED：实际执行次数 **2**，期望 **1**。
修复后 GREEN：实际执行次数 **1**，两个标签分别拥有不同 Evidence ID，
共同引用一个 `check_run_id`；两个 Workflow 步骤仍独立验证，最终完成门 PASS。
没有删除需求、弱化断言或把 Worker 自报当 Evidence。

## 2. 身份和等价边界

逻辑 obligation != 物理 CheckRun != System Evidence != Completion。

CheckRun 是最小进度记录，不是第二套验证权威。权威仍是现有 Evidence store、Work Integration、Workflow CompletionGate、Outcome 和 Goal。

执行合约包含：

- 明确 argv；程序解析到绝对路径并计算内容哈希，不由显示标签决定等价。
- canonical cwd、继承环境的 framed digest（不持久化原始环境值）。
- verification policy digest、明确兼容的 obligation 集合、deadline。
- 运行记录中的执行前 source/world fingerprint。

同一次批准的验证操作按相同 argv 分组。每个兼容标签生成独立 System Evidence，
记录共享运行 ID 和合约哈希；不复用其他 Goal 或不相关检查的 PASS。
同键不同合法合约拒绝 `IDEMPOTENCY_CONFLICT`；执行环境/程序变化拒绝旧合约。
源状态变化拒绝旧证明，需要明确批准新观察。
失败观察的同键重放仍然是失败；真正重试使用新批准的观察，确实重新启动命令。
已不确定的执行不能通过换键绕过，Goal 内其他新检查也会被保守拒绝。

程序哈希检查不是敌对文件系统的原子可执行映像锁定，不声称消除了替换程序的 TOCTOU。
全环境摘要比“只比较少量环境变量”更保守，启动终端环境变化也可能要求重新批准。

## 3. 持久生命周期与锁

进度路径：`.route/execution/check-runs/<operation-key-sha256>.json`。
字段包括 run_id、intent_ref、owner nonce、attempt、created_at、finished_at、合约、world、结果、Evidence ID 映射和诊断。

状态：`RESERVED`、`POSSIBLY_RUNNING`、`RESULT_PERSISTED`、`EVIDENCE_COMMITTED`、`RECOVERY_REQUIRED`。
真实命令失败由持久结果的非零 exit_code 和 CheckFail 区分；超时/取消不假造 exit code 或 CheckFail。
未实现可重连 PID/started_at 协议；`POSSIBLY_RUNNING` 明确不是已确认的 RUNNING。

全局检查 owner 锁和每键 owner 锁使用已有内核文件锁，覆盖执行、结果和 Evidence 写入。
不能依据时间推断进程死亡。恢复修改 owner/attempt；结果和 Evidence 提交前检查 fencing。
这是一条保守串行控制路径，不是新增并行调度器。

## 4. 六个崩溃窗口

| 窗口 | 已持久事实 | 外部事实与恢复 |
| --- | --- | --- |
| A：reservation 后、spawn 前 | RESERVED | 代码尚未走到允许 spawn 的持久状态；死进程内核锁释放后同键可继续，只执行一次 |
| B：spawn 后、启动确认前 | spawn 前已写 POSSIBLY_RUNNING | 实际子进程可能存活；同键/换键都不能自动再启动，需人工调查 |
| C：exit 后、result 前 | POSSIBLY_RUNNING | 即使实际已退出也不能凭猜测恢复结果；保持不确定，不自动重跑 |
| D：result 后、Evidence 前 | RESULT_PERSISTED + 精确输出摘要/状态 | 同键恢复原观察，无需重新执行；Evidence 写入成功前不能声称已验证 |
| E：Evidence 后、映射前 | 原运行和 canonical Evidence | 按运行/标签 dedup key 找回原 ID，不重复执行或重复写 Evidence |
| F：Integration 到 Goal 关闭中断 | finalization journal 的原始参数、revision、逐步回执 | 同键重放原 canonical domain 操作；完成门与 Integration 分别检查，不跳步制造成功 |

测试 failpoint 仅存在于测试构建或内部测试 seam，不提供可由生产环境变量触发的退出开关。
测试用真实子进程 `exit(23)`，不调用 Drop，避免把正常解锁误当崩溃恢复。
B 窗口在真实 spawn 后退出；不假定子进程随 Route 一同死亡。

## 5. 有界 runner 与取消真值

Windows 使用独占匿名管道读端的 PeekNamedPipe；Unix 实现非阻塞读端。
Windows 的管道语义参考 [Microsoft PeekNamedPipe 文档](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-peeknamedpipe)。
本轮 Windows 实测；Unix 实现未在 Unix 主机运行，不能写成跨平台验收 PASS。

- argv + 显式 cwd，无隐式 shell 拼接；脚本需要明确批准解释器。
- stdout/stderr 各保留前 2048 个原始字节的 UTF-8 lossily decoded 预览；预览字符编码后字节数可能因替换字符略大。
- 对完整流计算 SHA-256，不把预览哈希冒充全流哈希。
- 默认 900 秒 deadline；CheckContract 支持 1 毫秒到 1 小时。CLI 当前没有新添 deadline 设置项。
- Team cancel 状态约每 250ms 检查；已自然退出的直接进程优先保留真实退出事实。
- 只 kill 本次持有的 Child；确认回收等待最多 5 秒；不查杀未知 PID。
- 父进程退出但管道仍被子孙持有时，最多约 1 秒后返回 OUTPUT_INCOMPLETE，不无限阻塞读取线程。
- 超时/取消/输出不完整均保持 RECOVERY_REQUIRED，持久结构化 code、父进程终止确认、descendants=UNKNOWN、full_hashes=false 和下一步安全动作。

**没有 Windows Job Object/Unix process-group 进程树隔离。**
终止父进程 != 终止全部子孙；失败诊断不得写成整个任务已取消。
自退出子孙进程继承管道的真实 fixture 已证明不会假造完整输出。
既有 AI Host broker 尚未统一迁入该 runner，也没有活跃 Host 重连协议。

## 6. Evidence 和 finalization

外部 exit=0 不等于 Evidence。结果先持久化，随后在原 EvidenceStore 事务内提交。
已存在的物理运行/标签记录必须匹配 kind、payload hash、metadata 和 state hash，才允许取回原 ID。
Evidence 文件拒绝读取/写入时保留 RESULT_PERSISTED，返回 EVIDENCE_COMMIT_FAILED；同键重试不用再启动检查。
后续 CheckFail 会使旧 PASS 不能重新作为有效证明。

收尾路径：`.route/execution/finalizations/<operation-key-sha256>.json`。
先持久化原 mutation 参数（含 expected_revision），再调用既有 keyed Operator ingress，最后持久化结果。
覆盖 Workflow/Plan 创建、逐标签验证、Integration、Workflow completion、Artifact、Outcome、Goal close。
同键重试保持原参数和原回执；不同 payload 冲突。不是跨存储分布式事务。

特别修复：Goal close 已生效、但 journal 尚未记成功时，Work 的 active projection 不再返回原条目。
恢复不得误判 WORK_NOT_COMPLETED，更不能只看 Goal 投影就补写成功；
必须先验证原 journal 完整前序、canonical Integration 和 CompletionGate，再重放原 Goal close 回执。

使用：`route team verify <goal-id> --checks-file <path> --operation-key <approved-stable-key>`。
自动生成键的旧入口仍存在，会输出 verification_operation；恢复应保存并使用该键。
新 key 是新观察，不是解决未知运行状态的手段。
Work 只声明目录 scope 而无显式文件 scope 时，既有 Artifact 发布缺口仍未处理。

## 7. 本轮真实证据

已通过的专项：

- Q17：1 个物理执行、2 个独立逻辑 Evidence/Workflow 义务、Goal SUCCEEDED。
- 同键进程竞争只执行一次，不同合法键真的执行两次。
- changed-world 拒绝旧证明，新批准观察真实重跑。
- 非零退出始终 CheckFail；后续失败使旧 PASS 失效。
- 六个真实进程崩溃窗口；D/E 恢复原 run_id/原 Evidence，无重复执行。
- Evidence 文件暂时拒绝、恢复后同键提交原观察。
- stale attempt 提交前 fencing，无权威 Evidence 写入。
- deadline/cancel 真实 Child 终止确认；子孙管道未关闭时有界失败；16MiB 双流完整哈希回归随完整 basic 回归执行。
- Integration 前后、Workflow completion 后、Artifact 后、Outcome 后、Goal close 前后共 7 个收尾崩溃点；同键恢复并发重放不追加事件。

这些 fixture 是实际 OS 进程测试，不是实际 AI Host dogfood。
本机 `codex --version` 返回 `0.162.0-alpha.17.2`；`codex login status` 返回 exit=1、`Not logged in`。
未更改认证，真实 AI Team/restart dogfood **NOT_RUN**；未生成伪 Goal/Evidence ID 冒充 dogfood 成果。
测试 Evidence ID 位于一次性 temp fixture，测试结束清理，不是可长期复查的线上验收 ID。

## 8. TUI AI P1 小修复

UTF-8 字符边界换行；连接 timeout=10s，总请求 timeout=90s；响应体上限 1MiB，超过即拒绝。
help/config 为本地操作，即使未配置 key 也不会发模型请求。
不再显示 key 首尾，不派生含 key 的 Debug；请求/HTTP 错误不回显 URL 或原始响应体。
endpoint 展示去除 userinfo/query/fragment；provider 仍只是 OpenAI-compatible endpoint 的展示标签，不增加原生多 provider adapter。
3 项安全单元测试通过；实际服务端超时集成与 Unicode 终端列宽/组合字形排版没有被额外认证。

## 9. 性能采样

以下为 Windows 本机单次样本，不是 p95/吞吐容量承诺。使用真实测试可执行程序，程序内容哈希也计入操作耗时。
中型含 256 个源文件、128 条其他 Goal 的合成进度记录；现实形状 fixture 只读复制当前 185 个 Rust/TOML tracked 源文件并加入 512 条合成记录，不修改实际仓库运行历史。

| Fixture | 创建含子进程 ms | Evidence 提交 ms | 直接 keyed lookup ms | 去重含保守 scan ms | 恢复含 scan ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 1185.481 | 27.786 | 0.165 | 560.297 | 677.383 |
| medium synthetic | 1812.457 | 181.862 | 0.273 | 837.315 | 863.240 |
| current source snapshot | 2233.907 | 354.834 | 0.277 | 1060.438 | 1137.723 |

没有新增 authoritative index。保守 recovery scan 为 O(历史 CheckRun 数量)，全文件 Evidence 和 ledger 全量回放/重写的规模问题没有解决。
性能成本可见但不宣称已经足够快；后续优化必须保留未知运行不能换键绕过这一不变量。

收尾恢复另在小型真实进程 fixture 明确以 `--nocapture` 测量，exit=0：
Integration 前/后分别 1397.375/1386.998ms，Workflow completion 后 1459.649ms，
Artifact 后 1299.446ms，Outcome 后 1268.068ms，Goal close 前/后 1481.413/230.962ms。
这些包含新 CLI 测试进程启动和重新核验，不是纯 journal 写入延迟；中/大历史 finalization resume 尚未单独计时。
原始输出见最终日志目录的 `finalization-resume.log`。

## 10. 回归与 readiness

完整回归日志（ignored、本地审计产物，不进入仓库）：

- 首轮：`Release/local-tools/execution-trust-20261012-215432/`。
  fmt/core/CLI exit=0；basic/workspace exit=101，8 项新测试受同进程备份测试临时变更环境变量影响。
  生产 EXECUTION_CONTEXT_CHANGED 校验没有放宽；新测试改为独立进程运行，保持完整环境合约不变量。
- 最终重跑：`Release/local-tools/execution-trust-retry-20261012-215850/`。

| 实际命令 | 最终 exit | 结果 |
| --- | ---: | --- |
| cargo fmt --manifest-path tool/route/Cargo.toml --all -- --check | 0 | PASS |
| cargo test --manifest-path tool/route/Cargo.toml -p route-core | 0 | 25 unit + 1 doc test PASS |
| cargo test --manifest-path tool/route/Cargo.toml -p route-basic | 0 | 467 passed / 13 ignored；独立进程嵌套结果不重复计数 |
| cargo test --manifest-path tool/route/Cargo.toml -p route-cli | 0 | library、binary、全部 integration/doc targets PASS；ignored 保持 ignored |
| cargo test --manifest-path tool/route/Cargo.toml --workspace --exclude route-pyo3 | 0 | PASS；不含 pyo3 或默认 ignored 测试 |
| powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-docs.ps1 | 0 | check-docs: OK |
| scripts 下递归 PowerShell Parser.ParseFile | 0 | 11 文件、0 parse errors |
| git diff --check | 0 | PASS；新 untracked phase 文件另检尾随空白 0 |

Cargo test 实际同时使用 `--offline --locked`，设置 CARGO_PROFILE_TEST_DEBUG=0、CARGO_INCREMENTAL=0，复用现有 target。
专项性能测试虽带 ignored 标记，本轮已明确 `--ignored --exact` 执行且 exit=0；其他资源/fixture ignored 项不因此升级为 PASS。
新增阶段文件与 TUI AI 的凭据模式检查未发现匹配；这不是对整个既有仓库的无秘密证明。

最终 fetch --prune exit=0；fruit ahead/behind=0/0，local/remote SHA 均为起始 HEAD。

| 能力 | 当前证据级别 |
| --- | --- |
| CheckRunIdentity / PhysicalCheckDedup | LIBRARY_TESTED + 真实 fixture 进程验收；非 DAILY_USE_VERIFIED |
| RunRecovery | 检查的 A/D/E 可恢复，B/C fail-closed；AI Host 活跃重连 PARTIAL |
| RunFencing | 本地内核 owner 锁 + attempt 提交前测试；不含 Host 重连接管 |
| BoundedRunner | Windows 检查进程实测；Unix NOT_RUN；Host broker 尚未统一 |
| CancellationSemantics | 已知直接 Child 有界终止；子孙 UNKNOWN，PARTIAL |
| EvidenceCommitRecovery | 真实拒写与跨进程崩溃恢复测试通过 |
| FinalizationRecovery | 7 个进程崩溃点与并发同键重放通过 |
| RestartContinuity | 非 AI fixture 通过；真实授权 Host dogfood NOT_RUN |

阶段未 PASS，因此未 commit/push；main、Yuich、tag、Release 发布和历史均未改变。
工作树仍为混合 dirty baseline（最终核对 174 个 porcelain status entries），不是 clean release state。
没有递归清理旧工作、未 stage 既有变更，Release/ 的本地日志仍被 ignore 且 git ls-files Release 为空。
