//! 数据操作模块 — 排序、过滤、搜索

/// 对整数列表进行升序排序
pub fn sort_data(mut data: Vec<i32>) -> Vec<i32> {
    data.sort();
    data
}

/// 过滤出所有正数
pub fn filter_positive(data: Vec<i32>) -> Vec<i32> {
    data.into_iter().filter(|x| *x > 0).collect()
}

/// 过滤出所有偶数
pub fn filter_even(data: Vec<i32>) -> Vec<i32> {
    data.into_iter().filter(|x| x % 2 == 0).collect()
}

/// 在列表中搜索目标值，返回索引
pub fn search_value(data: &[i32], target: i32) -> Option<usize> {
    data.iter().position(|x| *x == target)
}

/// 统计各值的出现次数
pub fn count_frequencies(data: &[i32]) -> std::collections::HashMap<i32, usize> {
    let mut freq = std::collections::HashMap::new();
    for &val in data {
        *freq.entry(val).or_insert(0) += 1;
    }
    freq
}

/// 将数据转换为字符串列表
pub fn transform_to_strings(data: &[i32]) -> Vec<String> {
    data.iter().map(|x| x.to_string()).collect()
}