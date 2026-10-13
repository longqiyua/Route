# Session 验证一致性（2026-10-11）

## 真实缺陷与范围

修复前两个定向测试均失败、cargo 实际 exit=1：仅有匹配 check_id 的 System CheckFail 就返回 Verified；真实命令先 exit=0、再 exit=7，验证仍返回 Verified。
原因是 required_checks 仅判断匹配、System 来源和“非 stale”，而 `is_pass_stale` 对非 PASS 返回 false；此外扫描任意旧 PASS 时没有考虑后来失败。
成功结束使用另一套只判断 success_requires 种类存在的逻辑，忽略 required_checks 与当前指纹。上述路径均可产生不实成功状态。

## 当前判定

- required_checks：在本 Session 的持久记录中，从后往前找到同检查最新的 System CheckPass/TestPass/CheckFail/TestFail。只有匹配当前工作指纹的正向结果有效。Agent、Manual、其他 Session 不能替代检查。
- 精确 check_id 优先；旧记录没有 check_id 时保留 argv 包含 required 字符串的原兼容入口，不将 argv 执行或解释为指令。保留现有匹配范围，不宣称旧 argv 文本是稳定 CheckRun 身份。
- success_requires：原有非 PASS 种类判断保持不变；PASS 不能被后续同 check_id 的 System 失败推翻后继续使用。无 check_id 时采用完整 argv 相等；两者均缺失的匿名旧 PASS 遇到任何后续 System 检查失败即失效，因为无法证明失败属于其他检查。
- 顺序以持久 vector 顺序为准，不以 created_at 排序；相同时间戳也不能掩盖失败。重新执行并产生当前 PASS 后可重新满足要求。
- 失败记录按一次遍历索引；只要求 required_checks 时不构造这些索引，避免额外分配。不是新缓存，查询仍读取当前权威文件；未做 RSS/延迟实测，不宣称数值性能提升。
- `end_session(success)` 使用与显式 verify 相同的策略判定，不再有 hashless/stale 或仅有 required_checks 时的弱成功出口。无策略仍可正常成功结束；显式 force-end 仍只产生 ForcedUnverified。

缺少证据返回 Incomplete，有证据但失败/非正向/失效返回 Failed；两类条件同时存在时保留原 Incomplete 优先级。CLI JSON 形状和进程退出码约定不变，应读取 result 或持久 Session 状态，不能以命令 exit=0 作为业务 VERIFIED。

## 兼容及未完成项

不修改历史、身份、文件 schema 或权限。可读旧历史仍可审计，但不能依赖旧验收漏洞保持成功；应重新执行所需检查。原成功测试夹具补充当前指纹，以证明正确的正向路径，而不是给匿名 hashless PASS 开例外。

本项不实现 Evidence 与 Session 的跨文件原子事务。verify 后另一进程再写入失败、同时结束/撤销会话、崩溃恢复等仍需持久操作记录和 fencing，不把单次查询一致性包装成并发最终提交保证。independent_review 的更完整流程亦未因本修复升级。
Q10 当时把底层 `compute_state_hash` 忽略采集错误的风险另列 F15；后续 [Q11 故障注入与错误处理](state-fingerprint-failure-handling.md) 已覆盖损坏配置、上下文错误与 Windows 源文件锁，[Q12](sidecar-source-fingerprint.md) 又补无 Basic 源文件绑定。仍不能把这些小项当作全部扫描故障或跨文件并发终结的证明。

回归位于 [Session 单元测试](../tool/route/crates/route-basic/src/session_verification_tests.rs) 和 [原生 CLI 测试](../tool/route/crates/route-cli/tests/session_verification_e2e.rs)，分别覆盖判定边界和真实 task exec/verify/end 入口。

定向 5 passed；完整 `cargo test --workspace --exclude route-pyo3` 实际 exit=0：basic 445 passed / 6 ignored，core 24 passed，CLI lib 14 passed / main 9 passed，原生 CLI 新回归及所有默认集成/文档测试通过。fmt、文档链接、PowerShell parser、diff 检查通过。未运行付费模型/真实 Host 新验收，不扩大 AI 路由能力声明。
