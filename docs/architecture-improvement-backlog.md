# Route 架构改进待办（2026-10-11）

审计基线：`eb9751ba3d563e116ffcf0bf36a102c840cc0fca`，fruit。
状态：持续维护，已开始小范围修复。初次审计为源码检查；后续实现与验证逐项记录在下方，未完成项仍为待办。
此前单 Host dogfood PASS 的范围保持不变；它不能代替大历史、崩溃恢复、知识正向提取和安全并行验收。

## 结论与目标结构

优先解决资源随工作量增长、运行无法可靠恢复、验收流程缺少稳定事务边界的问题。
保留现有 Goal / Work / Principal / Evidence / CooperationKnowledge 语义，逐步调整实现边界：

```text
CLI / RPC / MCP
      |
      v
Application services：启动、恢复、验收、知识提取
      |                       |
      v                       v
Domain commands          Bounded process runner
权限、预算、状态机          生命周期、输出、超时、取消
      |
      v
Canonical event/evidence storage
      |
      v
可重建增量投影：Goal / Work / Run / Evidence 索引
      |
      v
同一 revision 的 Query / Context Pack
```

先在现有 crate 中建立接口与模块，证明边界后再决定是否拆 crate。
不要求常驻 daemon；直接 CLI 仍可使用同一套服务。

## 当前证据

| 编号 | 代码事实或风险 | 实际影响 |
| --- | --- | --- |
| F1 | 初始实现使用 `Command::output()`，完成后才截取 2 KiB；Q1 已改成双管道流式采集 | 输出缓冲不再随日志总量增长；显式超时、取消和进程树管理仍待办，尚未做整机 RSS 对比 |
| F2 | `development.rs::load_ledger_at` 全量读取、反序列化、校验 ledger；append 全量序列化替换。`context_pack::build` 又分别调用多个会加载状态的查询 | Context 输出有界，但构建成本随历史增长；独立查询可能跨 revision，不能视为统一快照 |
| F3 | 已确定性复现两个旧快照保存均成功却仅保留一条；Q8 增加跨进程锁内版本检查，并将 exec_command 记录改为锁内重读事务 | 新 API 写者不再静默覆盖其他提交；直接保存过期快照明确冲突。持久运行身份、跨存储关联及恢复仍待办 |
| F4 | `team_commands::execute` 拒绝有 active run 的恢复，并排除所有已尝试过的 Work；失败 Planner 没有完整恢复路径 | 保守拒绝能避免重复启动，但 broker 异常退出后可能留下长期占位；领域 retry 不等于 CLI 可恢复 |
| F5 | 验收、进程启动和调度主要在 CLI 模块；`team_verify` 分多次事件执行 Workflow/Plan/Integration/Artifact/Outcome/Close | 中间失败后的续跑需要稳定操作身份和逐步恢复；新增入口容易复制业务流程 |
| F6 | `team_verify` 仅保留 `root.join(scope).is_file()` 的 scope 作为最终 Artifact | Work 若使用目录范围，且没有显式文件范围，可能在已有验收副作用后遇到 PUBLISHED_ARTIFACT_REQUIRED |
| F7 | Start 将同一批准 argv 复制给所有 AI 生成的 requirement 文本 | 多条 requirement 会重复执行同一测试；标签覆盖不能证明每项需求都被验证 |
| F8 | 默认每 Goal 最多 3 个 Worker；Knowledge 调用共用此预算；detector 需要 Finding.evidence_refs，而普通 Worker 发布接口拒绝非空 evidence_refs | 常见 Planner+Implementer+Reviewer 用完预算后无法再启动提取；自主发布 Finding 到可信 Evidence 的关联缺少顺畅正向路径 |
| F9 | HostAdapter 将 stdout/stderr 丢弃，生命周期主要持有当前 Child；实际 scheduler 串行 | 失败诊断信息有限，进程树管理和重连不完整；并发预算不是已有并行吞吐能力 |
| F10 | route-basic 汇集多个领域；CLI 直接依赖 TUI、同步、搜索等组件 | 隔离测试和可选能力边界值得改善；依赖数量本身不能证明运行时内存高，需要测量 |
| F11 | `general_work::artifact_current` 原先全量读取文件后计算哈希；Q2 已复用现有流式文件哈希 | Artifact 内容校验不再要求分配与文件等大的缓冲，路径检查及全内容哈希语义保留 |
| F12 | Q3 并行回归中 `test_p7_guard_restores_env_even_on_panic` 一次失败，单独复跑通过；源码在 guard 释放环境锁后读取全局环境变量作断言 | Q4 将同类三个恢复测试放入各自独立进程，隔离断言窗口及前值 sentinel；不改变生产归档逻辑，不掩盖历史首次回归失败 |
| F13 | `worker.list` 原先独立调用两个全量 ledger 查询 | Q7 改为一次完整校验后生成两份投影，避免同一请求重复历史读取及跨 revision 拼接；全局 Context 快照仍待办 |
| F14 | Session 验证原先未要求 required_checks 匹配记录为 PASS；成功结束另用仅判断种类存在的弱策略 | Q10 已复现 CheckFail→Verified、旧 PASS 掩盖真实 FAIL；统一结束与验证判定，拒绝非正向结果及已被后续失败取代的 PASS |
| F15 | `compute_state_hash` 原先对 open/history/working_dir_status/ContextSnapshot 错误使用 if-let 跳过 | Q11 已用配置损坏、无效 UTF-8 上下文及 Windows 独占源文件复现并改为传播错误；执行前不启动、执行后保存失败证据；空未知指纹不能验证 PASS |
| F16 | 无 `.route-basic` 时，旧 compute_state_hash 只包含 ContextSnapshot；同大小文件修改的指纹相同已复现 | Q12 加入只读源文件 manifest 摘要，不初始化 Basic、不归档；按既有 scanner 排除规则绑定路径及内容，旧 context-only 证据需重跑，不等于全文件系统完整性 |
| F17 | team.decide 原先接受 split=false + IMPLEMENTER；scheduler / 子 Worker 预约按 role 和 Work 匹配而忽略 split | Q15 收紧新决策模式一致性，调度和最终预约拒绝旧矛盾授权；保留历史、合法 self-work / 委派及独立知识提取 |

源码入口：

- [execution.rs](../tool/route/crates/route-basic/src/execution.rs)
- [development.rs](../tool/route/crates/route-basic/src/development.rs)
- [context_pack.rs](../tool/route/crates/route-basic/src/context_pack.rs)
- [team_commands.rs](../tool/route/crates/route-cli/src/team_commands.rs)
- [team_verify.rs](../tool/route/crates/route-cli/src/team_verify.rs)
- [team.rs](../tool/route/crates/route-basic/src/team.rs)
- [principal.rs](../tool/route/crates/route-basic/src/principal.rs)
- [passive_base.rs](../tool/route/crates/route-basic/src/passive_base.rs)
- [host_adapter.rs](../tool/route/crates/route-cli/src/host_adapter.rs)

## 小修复记录

### 2026-10-11：双通道 1 GiB 输出资源实测

- [x] **Q16 / F1 / T0 子项**：新增 Windows 独立进程资源夹具，直接调用现有流式 capture；每通道分别 16 MiB / 1 GiB，固定块生成、流式预期摘要、各 2 KiB 预览、不同尾部及真实 exit=7。大输出不落盘，仅临时 43 bytes 测量元数据；不改生产行为或扩大权限。
- 显式测量实际 exit=0，75.94 秒：1 GiB/通道采集 25.68 秒，采集测试进程峰值 7,606,272 bytes、输出子进程 7,421,952 bytes；完整哈希/尾部/退出码通过，未死锁。每规模单样本，不声称 release、整进程上限、旧新 RSS 对照或原 418 MiB 问题已解决；[方法和原始报告](performance/bounded-capture-baseline-2026-10-11.md)。
- 新夹具全部默认忽略，外层显式入口负责传参及看门狗；生产 deadline、cancel、进程树、管道后代和恢复仍待办，T0/T1 总项不勾选。
- 完整 workspace（排除 route-pyo3）实际 exit=0：basic 456 passed / 10 ignored（新增三个资源入口默认忽略，外层显式实测已通过）、core 25 passed，全部默认集成及文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。未提交、未替换发布版、未改 Yuich；原始测试二进制测量身份保存在 JSON，未要求保留构建缓存作为交付。

### 2026-10-11：AI 角色路由的拆分意图一致性

- [x] **Q15 / F17**：真实认证 Planner 发布 split=false + IMPLEMENTER 的红测试实际 exit=1，旧逻辑接受了矛盾模式。现新决策只接受 false+PLANNER、true+IMPLEMENTER、true+INDEPENDENT_REVIEWER；其余 INVALID_ROUTING_MODE。旧记录仍可读，但不能用作新子 Worker 的预约或调度授权；不重写历史。
- 修复后两项定向测试实际 exit=0：八组合矩阵、五种认证拒绝且 ledger 不变、合法授权及旧矛盾记录的最终预约拒绝。新增纯校验 helper 为 additive Rust API；持久 schema / JSON 字段不变，新输入校验有明确收紧。
- [路由边界审计](routing-mode-consistency.md)：当前仅实现批准并 pin 的 Codex Host、AI 角色/Work 决策和串行调度；没有模型/Provider 选择或故障转移，模型 UNKNOWN，token/金额 NOT_TRACKED。本轮无真实付费调用，不冒充第二 Host 或模型路由验收。
- 完整 workspace（排除 route-pyo3）实际 exit=0：basic 456 passed / 7 ignored、core 25 passed，全部默认集成及文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。未提交、未替换发布版、未改 Yuich；既有无关变更保留。模型路由、金额计量、旧决策修改/恢复仍不是本项完成内容。

### 2026-10-11：项目并发统计轻量单次重放

- [x] **Q14 / F2 / T5 子项**：项目并发计算不再为每个 Goal 重放并保留完整 TeamView；单次遍历只保留预算上限、借用的 Goal/run ID 和运行状态，不复制路由说明、完整 Reservation 或知识任务。预约门和预算摘要仍共用同一计算，不引入缓存、持久化或新配置。
- 旧完整重放保留为测试 oracle；单/16 Goal、三种候选上限、每个历史前缀逐字段对比，覆盖所有 TeamAction、终态/取消/重配置/重复 run/未知 run/无关事件。真实预约拒绝/释放与只读 CLI 兼容测试继续保留。
- 本地 debug 纯投影 10K 事件/16 Goal、各 30 次：旧 p50/p95=6.801/7.968 ms，新=1.330/1.456 ms，显式测量实际 exit=0。仅证明该夹具统计阶段收益，不代表 CLI 总耗时、RSS、release 或原 418 MiB 问题已修复；[测量边界和原始输出](performance/project-capacity-projection-2026-10-11.md)。
- 完整 workspace（排除 route-pyo3）实际 exit=0：basic 454 passed / 7 ignored（新增显式 timing 测试默认忽略，但已单独运行通过）、core 25 passed，全部默认集成及文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。默认及可选 CLI 输出合同保持 Q13 状态。未提交、未替换发布版、未改 Yuich，既有无关变更保留。

### 2026-10-11：项目级并发预算只读摘要

- [x] **Q13 / T5 子项**：可选 `team status GOAL --budget` 从同一次已验证 ledger 加载，显示项目活动 Worker、实际有效并发上限、剩余额度和 revision；与 Reserve 共用既有计算逻辑。不改变默认 status、持久化 schema、授权/取消/重试规则。
- 这是 Q5 的可用性补齐，不是新复现的 correctness defect。其他 Goal 占满项目额度时，本 Goal 的本地余额仍可为正；两类原因分别显示。空闲低上限 Goal 不限制别人，未知进程不被假定已经退出。
- 可选摘要的 scope / project_global_capacity 值有明确变化，见 [输出契约](project-budget-snapshot.md)。仍非启动许可，角色/深度/Work/重试资格 NOT_EVALUATED，token / 金额 NOT_TRACKED。没有模型路由或性能加速声明；逐 Goal 投影的成本仍存在。
- 新核心测试验证与真实 Reserve 拒绝/接受一致、终态释放、空闲低上限不干扰、revision、损坏 ledger 返回错误和查询不改写历史；原生 CLI 测试验证跨 Goal 占用、释放及默认输出兼容。完整 workspace（排除 route-pyo3）实际 exit=0：basic 453 passed / 6 ignored、core 25 passed，全部默认集成及文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。既有编译 warnings 未列为本项修复。未提交、未替换发布版、未改 Yuich。

### 2026-10-11：无 Basic 的源文件绑定

- [x] **Q12 / F16 / T2 子项**：红测试实际 exit=1，同大小 source.rs 修改前后得到相同指纹。现无 Basic 时复用 ProjectScanner，只读计算按路径排序的文件哈希 manifest，再加入 `source-tree-v1` 指纹组件；不创建 Basic/数据库/快照或存储源文件内容。
- 新的长度分帧流式摘要避免构造整份 manifest 字符串，防止字段边界歧义；文件内容沿用流式哈希，路径及摘要元数据仍为 O(文件数)。既有 Basic 指纹分支不变。
- 原定向复现用例修复后通过；补增/删/重命名、生成物排除、字段边界测试，以及仅身份/Goal→CLI 检查→源文件修改→旧 PASS 失效→重新检查的回归。第一次全回归 exit=1，原因是新测试错误假设 attach 不创建 Basic 元数据；源码确认 attach 既有行为会初始化 Basic，故保持该生产行为、改用真实无 Basic 的 domain 入口，原生定向实际 exit=0。完整 workspace 重跑（排除 route-pyo3）exit=0：basic 452 passed / 6 ignored、core 25 passed、lightweight_interop 4 passed，全部默认集成和文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。未提交、未替换发布版、未改 Yuich。
- 这使旧 sidecar context-only PASS 不再匹配当前指纹，应重跑检查；不重写历史记录或自动迁移为有效 PASS。扫描有真实 I/O 成本，不声称性能加速；范围/兼容边界见 [说明](sidecar-source-fingerprint.md)。

### 2026-10-11：指纹采集失败不再伪装成成功

- [x] **Q11 / F15 / T2 子项**：三个红测试实际 exit=1：既有 Basic 配置损坏、无效 UTF-8 上下文、Windows 源文件独占读锁都被旧实现吞掉并返回指纹。现已有 Basic 时 open/history/working_dir_status 错误向上传播；ContextSnapshot 不再把 Constitution/Protocol/活动 Profile 的读取错误替换成默认值。
- 命令执行前采集失败直接返回错误，不启动；执行后采集失败仍保存这次命令的 CheckFail，state_stable=false、无 state_hash，附最多 512 字符的 state_hash_error。CLI 显示 UNKNOWN，不再对空指纹做固定切片；既有命令输出 JSON 不变，metadata 增加可选错误字段。
- 会话 verify/end-success 读取失败返回错误，不能给 Verified/Succeeded；Session 保持 Active 以便故障恢复后重试。未知空指纹不能匹配旧空 PASS。无元数据的只读 Context 默认仍可收集；没有强制初始化 Basic 或新建归档。
- 首轮定向 3 passed；另补空指纹、可选缺失只读及原生 CLI 执行后故障测试。完整 workspace（排除 route-pyo3）实际 exit=0，新增故障测试全部通过、原生 Session CLI 2 passed / 1 ignored，全部默认集成和文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。[故障契约和限制](state-fingerprint-failure-handling.md) 已记录。正常输入的指纹拼接/算法、历史、权限不改变；跨文件事务、无 Basic 的源文件绑定（F16）、所有底层 scanner 故障模型仍不在本项完成范围。未提交、未替换发布版、未改 Yuich。

### 2026-10-11：Session 验证与成功结束一致性

- [x] **Q10 / F14 / T2 子项**：两个红测试实际 exit=1，证明 CheckFail 单独就可被判 Verified，以及真实命令 PASS→FAIL 仍 Verified；不是推测。`task end success` 原先另用弱策略，忽略 required_checks 与当前指纹。
- required_checks 现按持久顺序选同检查最新 System 正/负结果，仅当前指纹的 PASS 有效；Manual/Agent 不能满足检查。保留旧 argv 匹配入口，最新失败后重新 PASS 可恢复。
- success_requires 的 PASS 按 check_id、旧 argv 或匿名记录规则排除后续 System 失败。匿名旧证据无法区分检查，因此任何后续 System 检查失败都使其失效，须重新产生当前证据。失败索引只在策略要求正向检查种类时建立，避免历史 PASS 的重复后缀扫描。
- `end_session(success)` 复用 `verify_session`，无策略保持允许成功；显式 force-end 的 ForcedUnverified 语义不变。不改历史或 JSON；旧 hashless PASS 不能再借弱结束路径通过。
- 定向 5 passed，包含相同时间戳、无关检查及 Agent 隔离、legacy argv、失败后重新通过、源文件漂移与无策略兼容。原有成功测试夹具补当前 state_hash，而不是放宽验收。新增原生 CLI PASS→FAIL→verify→end 回归通过；完整 workspace（排除 route-pyo3）实际 exit=0：basic 445 passed / 6 ignored、core 24 passed、CLI main 9 passed + lib 14 passed，全部默认集成和文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。[行为与兼容边界](session-verification-consistency.md) 已记录。未提交、未替换发布版、未修改 Yuich。
- 此修复不是跨 Evidence/Session 原子事务；并发终结 fencing、持久 CheckRun 和恢复仍未实现。

### 2026-10-11：验证结果精确关联

- [x] **Q9 / T2 子项**：`team_verify` 原先在执行后重新搜索同 session/check_id 的最后一条 PASS，可能选中另一物理执行；现新增 `execution::exec_command_with_evidence`，在存储事务提交成功后返回本次 Evidence ID，Team 直接传入 Workflow 和 Integration，不再全量重载并猜测“最后一条”。
- 原 `exec_command` 签名、`CommandEvidence` Rust 字段和 JSON 输出不变；新入口是 additive API。仍由原领域逻辑检查状态、权限、时序与后续失败，不因持有准确 ID 跳过验证。
- 独立进程真实命令使用相同 check_id，验证双方返回不同且实际存在的 ID；新增顺序复跑及非零退出用例，证明最新 PASS 与第一条收据不同、FAIL 也准确关联、后续失败仍使旧 PASS 失效。完整 workspace（排除 route-pyo3）实际 exit=0：basic 440 passed / 6 ignored、core 24 passed、CLI main 9 passed + lib 14 passed，默认集成及文档测试通过；fmt、文档链接、PowerShell parser、diff 检查通过。未提交、未替换发布版、未修改 Yuich。
- 不把“返回 Evidence ID”包装成持久 CheckRun / request replay / 跨存储事务；T2 的恢复和关联事务仍待办。

### 2026-10-11：Evidence 并发丢失更新

- [x] **Q8 / F3 / T2 子项**：独立进程 barrier 复现旧实现两个成功写者仅保留一条（父测试 exit=101）；实现锁内版本比较及 `EvidenceStore::update`，验证命令完成后重读并提交，不重跑命令。
- [写入契约、兼容影响与测试覆盖](evidence-storage-concurrency.md)：JSON 不变；Rust 结构体新增私有版本字段，外部字面量构造需改用 Default/load。旧二进制不遵守新锁，禁止混用并发写入；不是跨文件事务或 exactly-once。
- 首轮定向存储回归 4 passed / 1 ignored；随后增加真实命令并发回归，完整 workspace（排除 route-pyo3）exit=0：basic 439 passed / 6 ignored、core 24 passed、CLI 单元 9 passed + lib 14 passed，各集成测试通过。fmt、文档链接、PowerShell parser、diff 检查通过。未提交或替换发布版，未改 Yuich；T2 整体不勾选。

### 2026-10-11：Worker 列表单快照

- [x] **Q7 / F13：Worker 列表单快照**。新增 additive Rust 辅助函数，`worker.list` 单次加载/完整验证 ledger，再生成 Worker 描述和 presence；原单项 API、RPC JSON、错误码、权限和持久化 schema 不变。无跨请求缓存，不影响权威写入检查。
- 新单元及原生 RPC 定向测试通过：静态投影兼容、attached workspace、新元数据、非空 presence、历史篡改拒绝及只读性。优化前/后各 400 次查询基准真实 exit=0；10K 合成历史 worker.list p50 928.45→478.96 ms、p95 965.27→502.17 ms，峰值 36.52→34.68 MiB。
- [方法、全部原始数据与局限](performance/worker-list-single-snapshot-2026-10-11.md) 已保存；debug 本机顺序测量，不能推广到发布版、真实复杂 Worker 或其他查询。LARGE p95 略升和近零小数据内存收益亦保留记录。
- `development.state` 原本未重复读取，不伪造该路径修复；Context Pack 多次加载仍待办。完整工作区回归（排除 route-pyo3）exit=0，basic 434 passed / 5 ignored、core 24 passed、CLI 单元 9 passed；fmt、文档链接、PowerShell parser 和 diff 检查通过。仅收口 Q7，未提交、未替换发布版、不升级 T0/T5 整体。

### 2026-10-11：最小历史查询资源基线

- [x] **Q6 / T0 子项：真实进程查询实测**。复用现有 ignored 基准，0/1K/10K 历史与 256 MiB 外部资料，4 条查询路径各 20 次，共 320 个独立 Route 子进程。测试实际 exit=0，原始指标和方法已保存至 [基线报告](performance/history-query-baseline-2026-10-11.md) 与 [原始 JSON](performance/history-query-baseline-2026-10-11.json)。
- 10K 历史下事件查询 p50=491.63 ms、状态查询 p50=614.75 ms，峰值约 35 MiB；reference.list 仍约 46.54 ms。返回量小不等于历史处理成本小，下一步调查全量读取/校验及请求内重复加载；不先增加跨请求缓存。
- 限定为本机 debug/test-utils、合成事件和启动+RPC 端到端成本；I/O 字节、纯函数阶段成本、冷/热缓存、实际 team 等待轮询和 release 性能未测。16 组查询均验证临时项目内容/修改时间不变，无生产历史写入。未新增源码；本轮仅保存测量证据与清单。
- T0 整体仍未完成：100K、并发/崩溃、真实事件分布与其他资源路径保留待办，不把最小基线当作全部目标验收。

### 2026-10-11：只读预算摘要（长线任务第一项）

- [x] **Q5：只读预算摘要**。新增可选 CLI 参数 `route team status GOAL --budget`；无参数时原始 JSON 保持不变，无持久化 schema / RPC 变化。摘要从同一次 Goal 状态加载计算，不额外读取全量历史，不创建身份、不发起 AI、不预约额度、不增加 daemon。
- 展示启动尝试、去重 Worker 数、Reserved/Running 活动槽位及各项剩余数字；失败及终态启动仍消耗次数，Cancelled 仍计入历史用量。剩余使用饱和减法，异常/超上限数据不会整数回绕。列出取消与 Goal 内新 Worker 额度耗尽原因。
- Q5 当时的边界：不是启动许可；项目全局并发、角色/深度/Work/重试资格明确 NOT_EVALUATED。后续 Q13 补充项目并发 snapshot，其他资格仍不评估，空原因列表不代表可以启动。同一 Worker 的重试不能仅看新 Worker 剩余数。token / 金额均为 NOT_TRACKED，不冒充费用管理或模型路由。
- 验证：两项摘要测试及一项真实 CLI 子进程测试通过；默认输出等于原 TeamView，查询前后 ledger 字节不变。完整工作区回归（排除 route-pyo3，含 core/basic/CLI）exit=0，basic 433 passed / 5 ignored，CLI 单元 9 passed；fmt、文档链接、PowerShell parser 与 diff 检查通过。保留原有 ignored/warnings，不宣称价格、模型能力或性能收益已验证。
- 长线顺序保持串行：Q5 收口后，优先一个最小历史读取性能基线；再根据实测选择低侵入优化；模型路由和费用控制单独定义可验证边界。不把长线目标或文档更新当作其他待办已完成。

### 2026-10-11：环境恢复测试隔离

- [x] **Q4 / F12：环境恢复测试隔离**。P6-A（原先未设置）、P6-B（恢复前值）与 P7（panic 后恢复）的环境断言发生在 guard Drop 释放共享锁之后，其他并行测试可能已改变全局环境。重新加锁不能证明刚才的恢复结果，因此这三个用例各自在只运行该用例的子进程中执行；不将整个测试集改成串行。
- 保留原始恢复、临时目录清理及真实 panic/catch_unwind 断言；父测试检查子进程真实退出状态，不能吞掉断言失败。P6-B 的 sentinel 不再影响其他测试。仅修改 `#[cfg(test)]` 范围，无生产 API、配置、数据或依赖变更。
- 验证：定向 P6（3 passed）、P7（1 passed）通过；P7 预期 panic 已被捕获，不能把预期 panic 日志视作失败。完整工作区回归（排除 route-pyo3，含 core/basic/CLI）exit=0，basic 为 433 passed / 5 ignored；fmt、文档链接、PowerShell parser 及 diff 检查通过。未忽略原失败用例，未降低断言，未通过全局单线程规避问题。
- 本轮只完成 Q4，未提交；不宣称生产性能改善。下一小项回到只读预算可用性或最小性能基线，不在本轮继续扩展。

### 2026-10-11：实际预算驱动 Planner 路由

- [x] **Q3：实际预算驱动 Planner 提示**。Planner 原提示词硬编码 3 Workers / depth 1，现改为读取当前 Goal 的 `AgentBudget`，显示 Worker 总数、并发、启动次数、深度、重试和超时上限。初始 Planner 自身消耗一个 Worker 和一次启动；可委派数量取剩余 Worker 与启动次数的较小值，depth=0 时为零；不能委派时明确要求 `split=false`。上限不代表已经获得名额，权威事务校验不变。
- 低侵入边界：仅调整现有 Planner 提示与纯函数，不改配置、事件格式、权限、Provider、默认预算或调度方式；不发起真实付费 AI 调用。
- 预算真实性：当前是 Worker / 进程资源预算，并非 token / 金额硬上限。模型价格、token 使用量、跨 Provider 路由均不能据此宣称已支持。
- 本轮只收口 Q3；性能、可靠、好用、低侵入、预算和 AI 路由作为后续逐项验收维度，不同时展开。
- 验证：新增定向测试通过（默认、自定义、单 Worker、单次启动、depth=0）；fmt、文档链接、PowerShell parser、diff 检查通过。首次工作区回归 exit=101（F12）；该用例单独复跑通过，随后完整工作区（排除 route-pyo3，包含 core/basic/CLI）复跑 exit=0。保留 F12 待办，不以复跑通过冒充测试竞态已修复；不宣称实际 AI 决策效果或性能提升已实测。

### 2026-10-11：输出采集与 Artifact 哈希

- [x] **Q1 / F1：命令输出有界采集**。新增内部 `command_output` 模块，stdout/stderr 同时排空；完整流计算 SHA-256，每路仅保留前 2048 原始字节，继续使用原有 UTF-8 有损预览转换。保留空 stdin、真实退出码和 Evidence 完成门。读取失败不能作为完整输出成功返回，不新增日志落盘。
- [x] **Q2 / F11：Artifact 内容校验流式化**。使用已有 `content_hash_file`，保持原来的路径校验、全文件 SHA-256 比较和错误拒绝语义。
- 定向验证：完整流尾部参与哈希；空流/零预览；Interrupted 重试、读取错误传播、无效 UTF-8 字节；真实子进程双管道各 16 MiB，保留退出码 7；3 MiB Artifact 尾部篡改、缺失和越界路径拒绝。
- 完整回归状态：PASS（2026-10-11）。`cargo fmt --all -- --check`、core（24 passed）、basic（433 passed / 5 ignored）、CLI、`cargo test --workspace --exclude route-pyo3`、文档链接检查、PowerShell parser 检查及 `git diff --check` 均通过；CLI 与工作区回归进程真实退出码为 0。既有 ignored 用例未计为通过，原有 unused-variable/unused-mut 警告未在本轮扩散处理。
- 本轮收口：只完成 Q1、Q2，保留为未提交的可审阅改动；不继续展开其他待办。后续每轮优先一个可独立验证的小项，完成验证与清单更新后再推进下一项。
- 资源结论：代码和用例确认采集缓冲/文件哈希缓冲有界；未进行旧新二进制峰值 RSS 对比，不能据此宣称先前整机内存问题全部解决。线程栈、管道及历史状态占用仍需分别测量。
- T1 仅输出采集子项完成；T0、T2–T9 不因本次修复自动升级状态。

## 待办清单

以下规模是相对复杂度，不是工期承诺。每项先复现，再实施，再记录可用范围。

后续按用户最新优先级逐项收口，不并行扩项：

1. 性能与可靠：先做 T0 的一个最小基线，测量 Worker 等待时反复读取历史的成本，再决定是否优化；不以缓存削弱取消或预算检查。
2. 预算好用：只读展示已用/剩余 Worker、启动次数及限制原因；不增加必需配置或后台服务。
3. AI 路由：先核对现有 Host/角色选择的能力边界和预算拒绝路径；模型/Provider 路由仍待设计与实测，不能把角色分配冒充模型路由。
4. token/金额预算：当前未实现；需先明确计量来源、未知用量处理及启动前预留/结束后结算的可验证合同，再单独实施。没有可靠用量证据时不得显示“费用受控”。

- [ ] **T0 / P0：建立可重复的资源与恢复基线（小）**
  - 依据：F1–F9。覆盖 0/1K/10K/100K 事件、真实大目录、1 GiB 子进程输出、两个并发写者、各收尾步骤崩溃。
  - Q16 已显式验证每通道 1 GiB 输出及独立采集/输出进程峰值工作集；仅 debug、每规模单样本，无旧新 RSS 对照。其余规模、真实目录和恢复矩阵仍不完整。
  - 记录 p50/p95、Route 自身峰值内存、子进程内存、读写字节、历史增长、二进制 SHA；分开冷启动与热查询。
  - 验收：可以重复产生相同类别的结果；未知/未测保持 UNKNOWN/NOT_RUN；不把小夹具 PASS 推广到全部场景。
  - 依赖：无。后续性能指标以此为基线。

- [ ] **T1 / P0：统一有界进程执行器（中）**
  - 依据：F1、F9。Host 和 verification 共用执行机制，保持不同的权限策略。
  - 实现：并行排空 stdout/stderr、流式 hash、有界首尾预览、输出计数、deadline、cancel token、退出原因；完整日志仅显式开启并受磁盘配额约束。
  - 原始 AI 输出默认不落盘；只提取允许的生命周期字段和脱敏诊断。
  - Windows 评估 Job Object 管理本轮进程树；验证与现有 Codex sandbox/nested job 的兼容性。未知进程不纳管，无法确认退出时保持占位。
  - 验收：双通道 1 GiB 输出不死锁，输出缓冲内存固定；建议新增缓冲预算 <=8 MiB。超时/取消可收敛且不遗留已纳管子进程；权限与当前 sandbox 不减弱。
  - 依赖：T0。最高直接收益：减少验证阶段内存峰值与无限等待。

- [ ] **T2 / P0：Evidence 并发提交与验收关联（中）**
  - 依据：F3。Q8 已用 barrier 复现并修复同旧版本覆盖；存储锁/CAS 与 exec_command 锁内重读已实现，其他写入仍可能返回需显式处理的冲突。
  - 实现：统一 EvidenceRepository 的 append/update；以项目锁或版本 CAS 保护读改写，锁内重读；物理 CheckRun 有稳定唯一 ID，直接返回产生的 Evidence ID。
  - Q9 已实现 additive 执行入口返回已提交的精确 Evidence ID，并接入 Team 验收；持久 CheckRun、恢复操作记录仍未实现。
  - Q10 修复 Session 正向结果/后续失败与结束判定一致性；Q11 处理 F15 已报告的指纹采集错误；Q12 处理 F16 无 Basic 的源文件绑定。底层故障覆盖和跨文件终结事务仍待办，不能只凭结果种类判断整个验收链已安全。
  - 不再通过“相同 check_id 的最后一条 Evidence”猜测当前执行结果。跨存储的关联使用持久操作记录和恢复协议，不宣称两个独立文件具有单事务原子性。
  - 验收：并发 N 个检查保留 N 条结果；当前 FAIL 不能被旧 PASS 覆盖；任意提交崩溃点可恢复、重试不重复；旧 Evidence 保留。
  - 依赖：T0，可与 T1 分开实施。

- [ ] **T3 / P0：把 Run 变成可恢复的执行记录（大）**
  - 依据：F4。加入 attempt_id、owner token、PID 与启动时间、心跳/租约、fencing epoch、终态原因和恢复进度。
  - 实现 resume/reconcile：确认已退出后回收预算；未知状态保持 UNKNOWN/RECOVERY_REQUIRED；新 owner 接管后旧 owner 不能继续提交。
  - 租约过期只触发调查，不自动证明进程死亡。PID 必须结合启动时间等身份信息，避免误认 PID 复用。
  - 失败 attempt 显式派生受限的新 attempt；同时处理关联 Work claim，区分“重连”与“重新执行”。
  - 验收：分别在预约、发凭据、spawn、完成事件前崩溃；重启后无重复 Worker、无预算超发、无永久孤立 claim；旧 owner 写入被拒绝。
  - 依赖：T1、T2 的持久身份与提交基础。

- [ ] **T4 / P0：抽出应用服务，建立可续跑的交付事务（大）**
  - 依据：F5–F7。先将 TeamService / VerificationService 从 CLI 移至可复用模块，CLI/RPC 负责解析与展示。
  - Requirement、CheckDefinition、CheckRun 分离：一条实际测试可关联多项需求，但只执行一次；覆盖关系、人工审查项和未覆盖项可见。
  - 生成明确 Artifact manifest，支持文件、新增、删除及有界目录展开；使用已有受保护路径和链接规则，检查前预检可交付性。
  - 验收过程以固定 operation_id 和步骤进度续跑，绑定 source revision、manifest hash、Evidence 集合；关闭前再次校验，保留不可伪造的完成门。
  - 验收：目录范围任务可收口；相同批准检查不因七个标签执行七次；每一步崩溃后可继续；缺失覆盖/源文件漂移不 PASS。
  - 依赖：T2、T3。保持现有命令可用，通过服务层接入。

- [ ] **T5 / P1：一致读快照与增量查询投影（大，性能主要收益）**
  - 依据：F2。先引入 ProjectReadSnapshot，使一次 Context/Status 使用同一 revision，避免层层重载与重复校验。
  - Q13 已让 Goal / 项目并发摘要使用同一 ledger；Q14 将项目并发统计改为轻量单次重放。仍不是通用 ProjectReadSnapshot 或持久增量索引，ledger 全量加载/校验及其他重复查询尚未收口。
  - 后续用已有 SQLite 技术建立可丢弃投影，按 Goal/Work/Run/Evidence 建索引；记录 schema_version、last_sequence、head_hash，只处理增量。
  - 权限、预算最终写入检查仍在权威事务边界，不能信任过期缓存。查询明确返回 revision/滞后状态；只读查询不隐式创建或修复索引，缺失时安全回退或要求显式维护。
  - 验收：投影与完整重放逐字段一致；删除缓存可重建；跨进程变更不会造成混合 revision；100 次只读零持久增长。
  - 建议目标：受控机器上热查询 100K 事件 p95 <=200 ms、元数据峰值 <=128 MiB；这是待验证目标，不是当前承诺。
  - 依赖：T0、T2；先做单请求快照，再做持久投影。

- [ ] **T6 / P1：分段追加存储与可校验历史维护（大）**
  - 依据：F2。投影解决读取，分段日志解决每次追加重写全历史。
  - 旧 ledger 保持可读且不改已有事件身份/哈希；新格式显式切换，manifest 连接旧历史和新段。旧版写者必须被拒绝或明确只读，禁止同时向两套 canonical store 写入。
  - 写入协议覆盖截断尾、fsync、manifest 提交及恢复；sealed segments 可做校验后压缩，清理以引用和保留策略为依据。
  - Canonical history、可重建投影、运行临时输出、build cache 分别设预算。维护默认 dry-run，删除需明确目标与既有授权。
  - 验收：旧新读取结果一致，历史哈希不变；损坏不得静默丢失已确认事件；单次追加写入量不再随全部历史增长；压缩可验证还原。
  - 依赖：T5；保持为独立迁移阶段，先 shadow-read 比对再切换。

- [ ] **T7 / P1：补齐真实知识提取的正向路径（中）**
  - 依据：F8。增加“引用已有可信 Evidence”的验证入口，与“创建 System Evidence”严格分离；关联 Goal、Work、Finding、检查运行与资源版本。
  - Packet 提供完整可验证来源及发布所需字段，Knowledge Worker 无需重读仓库猜 resource_id。泛化和质量门继续保守处理。
  - 明确批准前台/维护任务预算：预留一个可选维护槽或设有总上限的独立维护预算，仍受项目并发上限约束；预算不足时保留可重试的 DEFERRED，不消费成无法恢复的终态。
  - 验收：真实有意义问题的 FAIL→FIX→PASS→独立提取→候选→验证→再次使用可追溯；完成三 Worker 工作后仍能按批准预算提取；无信号仍为零，提取失败不阻塞 Goal。
  - 依赖：T2–T4。真实 AI 正向链路通过前，不升级为全面 DAILY_USE_VERIFIED。

- [ ] **T8 / P2：安全 DAG 并行（大）**
  - 依据：F9。在可恢复执行基础上调度可同时进行的 Work，依赖、冲突范围、review 独立性与全局预算一起判定。
  - 涉及写入的 Worker 使用隔离工作区；提交候选 patch/manifest，经集成与重新验证进入最终工作区。初期保留默认串行。
  - 验收：两个无依赖任务确实重叠运行；重叠写路径按策略串行/拒绝；依赖不提前执行；取消、崩溃、集成冲突不覆盖用户修改。
  - 依赖：T3–T5。不能只把现有 launch 放进线程池。

- [ ] **T9 / P2：收敛组件依赖和长期验收（中到大）**
  - 依据：F10。按 application/domain/storage/host/transport 整理模块，先建立依赖方向测试，再按测量决定拆 crate。
  - 可选 TUI/同步等能力评估 Cargo features 或扩展二进制；保留完整分发能力，不以删除功能换小体积。
  - 增加真实中型仓库三轮失败恢复、多 Goal、长历史、退出重进的验收；记录 CPU/RSS/磁盘曲线。第二 Host 可用后再开展跨 Host 实测。
  - 验收：CLI 与 RPC 执行相同业务用例得到同义结果；可选依赖不污染核心服务；构建/启动收益有实测；文档 readiness 与测试证据一致。
  - 依赖：T4–T6；无需等待第二 Host 完成核心改进。

## 推荐批次

1. **资源与正确性**：T0、T1、T2。先解决失控输出、无界等待和 Evidence 并发风险。
2. **可恢复交付**：T3、T4。让中断任务可以继续，验收与产物收口可重试。
3. **规模与知识**：T5，再分别推进 T6、T7。
4. **吞吐与边界**：T8、T9。安全恢复与一致性通过后再扩大并发。

每批单独提供：失败用例、实现、兼容策略、资源预算、回归及真实场景验收。
实现进度以“小修复记录”和各待办状态为准；本文件不授权迁移 canonical store、清理文件或扩大运行权限。

## 外部实现依据

- [Rust Command::output](https://doc.rust-lang.org/std/process/struct.Command.html#method.output)：等待进程结束并收集输出；流式实现应采用 spawn 与有界管道消费。
- [SQLite isolation](https://www.sqlite.org/isolation.html)：WAL 提供读快照，仍只有单写者；投影设计需短事务和忙等待预算。
- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)：进程组生命周期控制；接入前需验证嵌套作业与 sandbox 兼容性。

## 2026-10-12：Execution Trust I 增量记录

Q17 已得到真实 RED（fixture 修复后 2 次物理执行）和 GREEN（1 次执行、2 份独立逻辑证明）。
新增 CheckRun 进度与 keyed finalization journal；A–F 崩溃、拒写、并发重放和有界直接子进程测试见
[执行可信性报告](execution-trust-i.md)。不据此把整个 T0/T3/T4 或阶段写成 PASS。

后续保持最小闭环优先：

1. 活跃 AI Host 运行的可检查身份、重连/接管和旧 attempt fencing；不能凭 elapsed time 或 PID 存在直接判断归属。
2. 实测 Windows Job Object 与既有 sandbox 的兼容性，明确子孙取消语义；不查杀未知进程。
3. 在已有授权 Host 可用时完成一次真实 AI Team 中断/重启 dogfood，再考虑 DAILY_USE_VERIFIED。
4. CheckRun recovery scan 为 O(n)，程序 pin/hash 和完整 source fingerprint 仍有成本；只在保持未知执行不可绕过的不变量下优化。
5. 目录 Work scope 的 Artifact 发布缺口、ledger 全量回放/重写、Evidence whole-file、Context 多查询 revision 一致性仍待办。

本轮没有模型路由、额外 Worker、知识自动化、GUI 或 Yuich 改动。

## 2026-10-13：Run Continuity I 增量记录

上面的历史待办不删除。Windows OS 进程身份、原子 Job 归属、重启检查、
整树取消和安全 keyed recovery 已实现，证据与最终回归见
[Run Continuity I](run-continuity-i.md)。这是 OS substrate，不是 AI Host 会话重连。

- 未知仍隔离容量；Job 名消失不等于已观察到零成员。没有终止证明时拒绝释放。
- BEFORE_SPAWN 可显式恢复原 CheckRun reservation，重试保留旧 launch receipt。
- 真实 AI Host 重启仍 NOT_RUN：Codex 未登录，不更改用户认证。
- Linux/macOS 进程连续性仍 NOT_RUN/UNSUPPORTED；不从 Windows 测试推断。
- 可执行文件 pin/hash 与 O(n) 扫描成本保留为后续有测量依据的优化项。
- 不增加 daemon、模型/预算产品功能，不修改 Yuich 或 Git 历史。

## 2026-10-13：Base Positive Chain I 当前状态核对

保留上文历史 T0–T9/Q1–Q17 文字。以下仅核对本批要求的 13 个范围，不代表整个
产品的完成百分比。证据见 [Base 正向链报告](base-positive-chain-i.md)、
[Execution Trust](execution-trust-i.md)、[Run Continuity](run-continuity-i.md)。

| 优先级 | 项目 | 当前状态 | 已有证据与未完成边界 |
|---|---|---|---|
| P0 | Q17 重复检查 | DONE | 兼容批准命令单次物理执行、独立逻辑证明与 RED/GREEN |
| P0 | Run 中断恢复 | PARTIAL | Windows OS 归属/重启/取消通过；真实 AI Host 重启 NOT_RUN |
| P0 | Evidence/Workflow/Integration finalization | DONE | journal、崩溃恢复、幂等重放测试；不声称跨文件原子事务 |
| P0 | bounded runner | PARTIAL | 有界输出/超时/Windows owned-tree；其他 OS、Host 全路径尚未认证 |
| P1 | TUI AI 稳定性/输出界限 | DONE | UTF-8 安全、deadline、响应大小边界回归 |
| P1 | Artifact 文件/目录/删除 | PARTIAL | 文件与指纹有效；目录/删除完整发布闭环仍缺 |
| P1 | 同 revision 查询/增量投影 | PARTIAL | 并发摘要/本批 Base 使用同载入 ledger；全局 Context 快照/增量索引未实现 |
| P1 | Knowledge 正向提取 | PARTIAL | 确定性/人工/fixture 正向闭环；真实 AI 合成 NOT_RUN，不升级整个 T7 |
| P1 | 文档真值收敛 | PARTIAL | 本批逐项核对；既有历史文档仍保留，不能当所有旧愿景均已认证 |
| P2 | Model/Provider routing | OPEN | 本批禁止实现 |
| P2 | token/cost budget | OPEN | Worker 数量/超时预算不等于 token 账本 |
| P2 | safe DAG parallelism | OPEN | 本批禁止实现；不把串行 launch 包装成安全 DAG |
| P2 | 应用服务/依赖收敛 | PARTIAL | 部分执行能力已下沉；CLI/服务分层及可选依赖还未全量整理 |

P0：4 total / 2 DONE / 2 PARTIAL / 0 OPEN。
P1：5 total / 1 DONE / 4 PARTIAL / 0 OPEN。
P2：4 total / 0 DONE / 1 PARTIAL / 3 OPEN。
合计：13 total / 3 DONE / 7 PARTIAL / 3 OPEN / 0 NOT_RUN / 0 BLOCKED_EXTERNAL。
严格完成率：3/13 = 23.08%。
加权口径：P0=3、P1=2、P2=1；DONE=1、PARTIAL=0.5、其余=0。
分母 26，得分 15.5，加权完成率 59.62%。这是明确的管理折算，不是质量置信度；
PARTIAL 内部不假定恰好完成一半。真实 AI 的 NOT_RUN 单列，不伪装成代码 blocker。

### 聊天愿景与实际交付差距（15 项）

| 曾讨论能力 | 状态 | 当前边界 |
|---|---|---|
| real cross-tool handoff | NOT_RUN | 通用协议 fixture 不等于第二真实 Host 日用 |
| autonomous Goal decomposition | PARTIAL | Work/Goal 结构与有限协调；非通用自治规划 |
| dynamic Worker routing | PARTIAL | 固定角色/现有预算；非动态模型路由 |
| model/effort profiles | DEFERRED | 本批不实现 |
| user-defined workflow | PARTIAL | 现有 contract/验证面；非完整自定义工作流产品 |
| role-specific Context Pack | IMPLEMENTED | 有界角色投影与独立 reviewer 策略 |
| Route Console timeline | DEFERRED | 只有既有 events/history 与 Base query |
| contribution provenance UX | PARTIAL | canonical refs 可追溯，非完整可视交互 |
| Route Base positive learning | IMPLEMENTED | 本批 deterministic/manual/deferred/fixture；AI 合成 NOT_RUN |
| Default vs Full policy | DEFERRED | 非本批范围 |
| first-run demo | DEFERRED | 非本批范围 |
| token/cost routing | DEFERRED | 非本批范围 |
| safe DAG parallelism | DEFERRED | 非本批范围 |
| cross-project Personal Base | DEFERRED | 只有项目范围 |
| Benchmark/Arena | DEFERRED | 非本批范围 |

汇总：IMPLEMENTED=2 / PARTIAL=4 / NOT_RUN=1 / DEFERRED=8 /
BLOCKED_EXTERNAL=0。上述 IMPLEMENTED 指限定实现，不表示真实 AI 日用已认证。
