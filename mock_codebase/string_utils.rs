//! 字符串工具模块 — 格式化、解析、验证

/// 格式化问候语
pub fn format_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}

/// 格式化带时间戳的消息
pub fn format_with_timestamp(msg: &str, ts: &str) -> String {
    format!("[{}] {}", ts, msg)
}

/// 解析字符串为整数，出错返回 0
pub fn parse_number(s: &str) -> i32 {
    s.parse().unwrap_or(0)
}

/// 解析字符串为浮点数
pub fn parse_float(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

/// 验证邮箱格式（简单版：包含 @ 和 .）
pub fn validate_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

/// 验证非空字符串
pub fn validate_not_empty(s: &str) -> bool {
    !s.trim().is_empty()
}

/// 转换为大写
pub fn transform_uppercase(s: &str) -> String {
    s.to_uppercase()
}

/// 去除首尾空格
pub fn transform_trim(s: &str) -> &str {
    s.trim()
}