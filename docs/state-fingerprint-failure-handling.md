# 指纹采集错误处理（2026-10-11）

## 已复现与修复

三个定向故障测试在旧实现中全部失败，cargo 实际 exit=1：

- 已有 `.route-basic/config.json` 损坏，却退化成仅上下文指纹。
- 上下文文件含无效 UTF-8，默认 Constitution/Protocol 或省略的上下文仍参与成功指纹。
- Windows 源文件被本测试的独占句柄锁住，working_dir_status 报错，但外层仍返回指纹。

`compute_state_hash` 现在区分 Basic 元数据目录不存在和已有仓库读取失败；已有仓库的打开、历史和工作文件扫描所报告的错误都向上传播，不再省略相应组件。ContextSnapshot 的读取结果也必须成功。
ContextSnapshot 本身不再吞掉 Constitution、Protocol、活动 Profile 指针和所选 Profile store 的读取/解析错误；这影响其他调用者：损坏输入现在会返回错误，而不是生成默认上下文。真正可选文件缺失仍由已有 reader 返回默认值，不隐式写入文件。

## 执行与验收

- 执行前指纹不可读：命令不启动，调用返回错误。
- 执行后指纹不可读：已完成的命令不重跑；保存 CheckFail，state_stable=false、state_hash=None。metadata.state_hash_error 记录最多 512 字符的错误上下文，供本地审计。
- 子命令 exit=0 不能覆盖指纹故障。CLI 显示 UNKNOWN 并返回非零，不因空字符串的 `[..16]` 切片崩溃。
- verify/end-success 读取失败返回错误，不能验证成功或成功结束；未结束 Session 保持 Active。空的未知当前指纹总使 PASS 无效，不能借旧空指纹获得通过。

不改变 CommandEvidence 的字段/JSON，也不修改历史身份或 schema；Evidence metadata 是已有开放字典，仅增加可选错误信息。正常读取成功时的指纹算法和拼接顺序不变，不能据此宣称 CPU/RSS 改善。

## 回归与边界

[故障测试](../tool/route/crates/route-basic/src/state_hash_failure_tests.rs) 使用真实临时文件：损坏配置、五类上下文错误、Windows OS sharing violation、已有 PASS 的 verify/end 拒绝、空指纹拒绝及可选缺失只读。
[原生 CLI 回归](../tool/route/crates/route-cli/tests/session_verification_e2e.rs) 将独立子进程作为实际检查命令，在执行后制造配置读取故障，验证唯一失败记录、退出码 0 不等于 PASS、UNKNOWN 显示及无 panic。

范围是“采集层已经报告的错误不被这两个聚合入口吞掉”，不是全部 I/O 故障模型证明。基础 reader 的缺失判断、scanner 遍历边界、并发文件变动、跨 Evidence/Session 提交、恶意同 OS 用户修改文件仍需各自验收。
Q11 当时保留无 `.route-basic` 的仅 ContextSnapshot 语义，并列出 F16；后续 [Q12](sidecar-source-fingerprint.md) 已加入只读源文件摘要而不启用归档。扫描排除、外部资源及并发原子性仍有明确边界，不据此宣称全文件系统完整性。

首轮定向 3 passed；扩展故障用例及完整 `cargo test --workspace --exclude route-pyo3` 实际 exit=0。原生 Session CLI 2 passed / 1 ignored（夹具由父测试启动）；所有默认集成和文档测试通过。fmt、文档链接、PowerShell parser、diff 检查通过。未进行新的真实 AI Host 或付费模型验收。
