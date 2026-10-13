# 无 Basic 的源文件指纹（2026-10-11）

## 问题及实现

旧 sidecar 模式没有 `.route-basic` 时只计算 ContextSnapshot。临时项目的 source.rs 从 `fn before() {}` 改成同长度的 `fn after_() {}`，前后得到完全相同的指纹；红测试实际 exit=1。

现在该分支只读扫描项目内文件，复用已有 ProjectScanner 的规则和流式文件 SHA-256，按规范相对路径排序，以长度分帧方式依次摘要路径和文件哈希。域标记 `route-sidecar-source-v1` 和组件 `source-tree-v1` 区分旧的仅上下文语义。
ContextSnapshot 仍参与最终指纹。既有 Basic 分支保留原快照及工作文件变更算法，不改变其 TrackConfig。

没有隐式 init、归档或常驻进程，不写源文件、Git、索引、数据库或源文件 manifest。仅在明确调用现有状态指纹/检查/验收路径时执行，不给 handoff 等只读入口新增后台扫描。

## 资源及扫描边界

- 文件内容流式哈希，不分配与文件大小相等的内容缓冲。
- 路径和文件摘要复用 scanner 的内存结果，按引用排序；manifest 哈希逐字段输入，无额外整份 JSON 或拼接字符串。元数据仍为 O(文件数)，不是总内存恒定。
- 完整读盘成本随纳入扫描的文件字节数增长；检查前后各有一次扫描。未测端到端 RSS/延迟，不将正确性修复称为性能提升。未来增量缓存必须仍有权威失效校验。
- 复用当前扫描策略：排除 `.route`、`.route-basic`、`.git`、Wiki、Release、node_modules、target/target-*、dist/build，遵守 ignore/Git ignore 规则并跳过符号链接。不是“磁盘上所有文件都受证明保护”，被排除的构建输入/外部资源仍需独立 Artifact/资源验证。
- scanner 报告读取失败时错误向上传播，沿用 Q11 的执行前拒绝/执行后 CheckFail 处理。不保证一次遍历是文件系统原子快照，也不能发现检查期间修改后又恢复原内容的瞬时变化。

## 兼容

无 Basic 的旧 PASS 使用 context-only 指纹，升级后不能冒充源文件已验证，应重新运行检查。旧记录仍可审计，不删除、不改 ID、不自动补成有效 PASS；已有历史成功状态不被偷偷改写。
CommandEvidence/CLI JSON、Evidence schema、权限和调用参数不变。查询指纹会有新的扫描 I/O，但不再要求为了正确绑定源文件而启用 Basic 归档。

## 验证

[单元测试](../tool/route/crates/route-basic/src/state_hash_failure_tests.rs) 覆盖同大小编辑、新增、删除、重命名、生成物排除和无隐式元数据创建；[哈希测试](../tool/route/crates/route-core/src/hash.rs) 验证字段边界与顺序。
[原生回归](../tool/route/crates/route-cli/tests/lightweight_interop.rs) 使用真实 Git 临时仓库，经 ensure_identity 和 goal.create 的现有 domain 入口建立仅元数据 Goal，再运行 CLI：产生 PASS 后编辑源文件，旧 PASS 失效，重跑生成新的有效证据；验证无 `.route-basic`，Git 文件内容未改。
注意：现有 `attach` 会调用 BasicRepository::init 创建 Basic 元数据（不是源文件归档），所以本测试不能用 attach 当作无 Basic 前提。第一次全回归因该新测试的错误前提 exit=1；没有放宽无 Basic 断言或修改 attach，而是改为真实无 Basic 入口，修正后原生定向 exit=0。
这不是新的真实 AI Host/模型验收，不扩大 AI 路由或金额预算能力声明；跨文件恢复和终结事务仍待办。

最终完整 `cargo test --workspace --exclude route-pyo3` 实际 exit=0：basic 452 passed / 6 ignored、core 25 passed、lightweight_interop 4 passed，全部默认集成和文档测试通过。fmt、文档链接、PowerShell parser、diff 检查通过；第一次错误前提导致的失败记录保留在上文，不伪装成首轮全绿。
