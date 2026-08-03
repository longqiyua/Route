//! 跨文件依赖模块 — 测试复杂调用链
//!
//! 这些函数调用其他模块的函数，形成复杂的跨文件依赖关系。
//! 语义匹配应该能识别出这些复合操作的本质。

/// 计算三个数的和（调用 add 语义）
pub fn compute_sum(a: i32, b: i32, c: i32) -> i32 {
    let step1 = a + b;
    step1 + c
}

/// 处理数据管道：先排序再过滤正数
pub fn process_data_pipeline(data: Vec<i32>) -> Vec<i32> {
    let mut sorted = data.clone();
    sorted.sort();
    sorted.into_iter().filter(|x| *x > 0).collect()
}

/// 处理用户信息：格式化姓名和年龄
pub fn format_user_info(name: &str, age: i32) -> String {
    let info = format!("{} ({})", name, age);
    format!("Hello, {}!", info)
}

/// 批量处理数据：先过滤偶数，再排序，再转字符串
pub fn batch_process(data: Vec<i32>) -> Vec<String> {
    let filtered: Vec<i32> = data.into_iter().filter(|x| x % 2 == 0).collect();
    let mut sorted = filtered;
    sorted.sort();
    sorted.iter().map(|x| x.to_string()).collect()
}

/// 统计验证：检查数据中是否有重复值
pub fn has_duplicates(data: &[i32]) -> bool {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for &val in data {
        if seen.contains(&val) {
            return true;
        }
        seen.insert(val);
    }
    false
}

/// 复杂计算：加权平均值
pub fn weighted_average(values: &[f64], weights: &[f64]) -> f64 {
    let mut sum_product = 0.0;
    let mut sum_weights = 0.0;
    for (v, w) in values.iter().zip(weights.iter()) {
        sum_product += v * w;
        sum_weights += w;
    }
    if sum_weights == 0.0 { 0.0 } else { sum_product / sum_weights }
}

/// 多字段验证：检查多个条件
pub fn validate_user_input(name: &str, email: &str, age: i32) -> Vec<String> {
    let mut errors = Vec::new();
    if name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if !email.contains('@') {
        errors.push("Invalid email".to_string());
    }
    if age < 0 || age > 150 {
        errors.push("Invalid age".to_string());
    }
    errors
}