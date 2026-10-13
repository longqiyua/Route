# Evidence 并发存储修复（2026-10-11）

## 已复现的问题

在独立临时项目中，用 barrier 强制两个测试进程先读到同一空快照，再各自增加一条记录并保存。
修复前两个进程均 exit=0，磁盘仅有一条记录；断言期望 `[0, 73]`、实际 `[0, 0]`，父测试 exit=101。
这是已复现的丢失更新，不是推测。合成记录只用于存储回归，不作为真实开发验收 Evidence。

## 写入契约

- `EvidenceStore::save` 在项目内取得跨进程 OwnershipLock，再比较当前文件指纹与加载时指纹；过期快照返回 `EVIDENCE_WRITE_CONFLICT`，不替换历史。成功后更新自身指纹，支持同一实例连续保存。
- Clone 保留独立的加载版本，不能因另一个实例成功保存而自动获得覆盖权限。文件不存在与空 JSON 文件是不同版本；读取错误不会被当作空历史。
- `EvidenceStore::update` 在同一锁内重新加载、运行内存修改回调并原子提交，提交成功后才返回回调结果。回调失败不写入。回调不得重入存储或执行外部命令。
- `exec_command` 在子命令完成后才进入上述短事务保存结果；不跨命令运行持锁，也不因版本冲突重新执行命令。
- 其他直接 load/record/save 调用得到冲突保护，但尚未全部改成事务 API；冲突须调用方显式处理，不声称所有流程可自动续跑。
- 锁复用现有内核文件锁：最多 400 次、间隔 25 ms 的竞争等待，不按锁文件年龄杀进程。保留一个小锁文件；只读 load 不创建锁或写文件。锁内磁盘指纹使用流式哈希，但 JSON 仍整体加载/序列化，写入成本仍随历史增长。

## 兼容与边界

持久 JSON 仍只有 `evidence` 字段，记录身份、顺序、验收规则和原子替换机制不变；不迁移或修补用户历史。
Rust 类型新增不序列化的私有版本字段：外部结构体字面量 `EvidenceStore { evidence: ... }` 不再可用，应使用 `Default`（创建新存储）或 `load`（更新已有存储）。通过 serde 反序列化得到的实例没有磁盘加载版本，不能盲目覆盖已有文件。

保证适用于所有遵守这套 API 的新版本写者。旧二进制和直接编辑文件的外部程序不遵守此锁协议，不能与新版本并发写入并期待此保证。同一 OS 用户直接修改文件不属于新的安全边界。
这不是跨 Session/Development/Evidence 文件的原子事务，也没有实现持久 CheckRun 身份、任意崩溃点恢复或 exactly-once 执行；T2 整体仍未完成。

## Q9：精确执行收据

新增 `execution::exec_command_with_evidence` 返回 `(CommandEvidence, Evidence ID)`。ID 来自同一次事务的 record 结果，只在原子提交成功后向调用方返回；失败命令也有对应的失败 Evidence，而存储提交失败则整个 API 返回错误，不提供成功收据。
原 `exec_command` 委托新入口并保留原返回值，`CommandEvidence` 字段及现有 CLI/RPC JSON 不变，无需外部 Rust 构造调用迁移。

Team 验收不再通过“相同 session/check_id 的最后一条 PASS”选择结果，直接使用本次返回 ID。既有 Workflow/Integration 仍校验当前状态、来源、时序、权限及后续失败；精确关联不等于授权，也不能让旧 PASS 越过新 FAIL。
这样还移除每项检查后为找 ID 而额外全量读取 Evidence 的步骤，但未做端到端性能测量，不宣称具体加速比例。

独立进程回归改为两个真实命令共享 check_id，验证各自收据指向两条不同的已持久化记录；顺序复跑测试证明第二条 PASS 会改变旧搜索的结果，但第一条收据不会变化，非零退出收据对应 CheckFail。存储字段及请求重放协议没有改变，持久 CheckRun 和跨文件恢复仍未实现。

Q9 完整 `cargo test --workspace --exclude route-pyo3` 实际 exit=0：basic 440 passed / 6 ignored、core 24 passed、CLI lib 14 passed / main 9 passed，所有默认集成及文档测试通过。fmt、文档链接、PowerShell parser、diff 检查通过；没有运行新的 AI Host/付费模型验收，也不据此扩大模型路由能力声明。

## 回归覆盖

[独立进程测试](../tool/route/crates/route-basic/src/evidence_storage_tests.rs) 覆盖：

- 同旧快照的两个保存：一成功、一冲突，已提交结果不丢失。
- 两个事务写入：均成功、两条唯一身份记录保留，PASS/FAIL 的新旧关系仍按持久顺序判断。
- 两个独立进程调用真实 `exec_command`，各自执行一个输出命令：两条 check_id 和成功退出结果均保留。
- 连续保存、克隆版本隔离、删除后的版本冲突、旧 JSON 形状不变。
- 失败回调和损坏输入不覆盖磁盘内容。

已有 Cooperation 负向权限测试只调整测试夹具恢复方式：每次先加载当前版本，再恢复已知夹具内容；不绕过新冲突检查、不放宽权限断言。

完整 `cargo test --workspace --exclude route-pyo3` 实际 exit=0：route-basic 439 passed / 6 ignored（包含上述 5 个存储回归和父测试调用的 ignored 子进程夹具），route-core 24 passed，CLI lib 14 passed / main 9 passed，所有默认集成与文档测试通过。fmt、文档链接、PowerShell parser、diff 检查通过。既有 unused-variable 等编译警告未在本项扩大清理。
