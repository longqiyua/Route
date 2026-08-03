//! 数学运算模块 — 基础算术操作

/// 加法：返回两个整数的和
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// 减法：返回两个整数的差
pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

/// 乘法：返回两个整数的积
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// 除法：返回两个整数的商，b 不为 0
pub fn divide(a: i32, b: i32) -> i32 {
    a / b
}

/// 幂运算：计算 a 的 b 次方（仅支持正指数）
pub fn power(base: i64, exp: u32) -> i64 {
    let mut result = 1i64;
    for _ in 0..exp {
        result *= base;
    }
    result
}

/// 计算绝对值
pub fn abs_value(x: i64) -> i64 {
    if x < 0 { -x } else { x }
}

/// 计算最大值
pub fn max_value(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}