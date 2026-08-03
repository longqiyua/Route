//! 奇怪函数名模块 — 测试语义匹配的关键场景
//!
//! 这些函数的名字完全没有意义，但核心实现是有意义的操作。
//! 语义模糊匹配应该能通过实现内容正确识别出操作类型。

/// 名字奇怪，但实现是加法
pub fn xyz_abc_123(a: i32, b: i32) -> i32 {
    a + b
}

/// 名字奇怪，但实现是减法
pub fn qwerty_uio_456(a: i32, b: i32) -> i32 {
    a - b
}

/// 名字奇怪，但实现是乘法
pub fn asdf_ghjk_789(a: i32, b: i32) -> i32 {
    a * b
}

/// 名字奇怪，但实现是除法
pub fn zxcv_bnm_000(a: i32, b: i32) -> i32 {
    a / b
}

/// 名字奇怪，但实现是格式化
pub fn plm_okn_ijb(s: &str) -> String {
    format!("Processed: {}", s)
}

/// 名字奇怪，但实现是解析
pub fn rfv_tgb_yhn(s: &str) -> i32 {
    s.parse().unwrap_or(-1)
}

/// 名字奇怪，但实现是验证
pub fn cde_wsx_qaz(email: &str) -> bool {
    email.contains('@') && email.len() > 5
}

/// 名字奇怪，但实现是排序
pub fn vfr_ujm_iko(mut data: Vec<i32>) -> Vec<i32> {
    data.sort();
    data
}

/// 名字奇怪，但实现是过滤
pub fn bgt_nhy_mlp(data: Vec<i32>) -> Vec<i32> {
    data.into_iter().filter(|x| *x > 10).collect()
}

/// 名字奇怪，但实现是搜索
pub fn zse_xdr_cfv(data: &[i32], target: i32) -> Option<usize> {
    data.iter().position(|x| *x == target)
}