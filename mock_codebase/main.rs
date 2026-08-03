//! 主入口 — 编排所有模块
//!
//! 这个文件模拟了真实项目的主入口，调用各个模块的函数。
//! 语义匹配应该能识别出这个文件的"编排"性质。

mod math_ops;
mod string_utils;
mod data_ops;
mod weird_names;
mod cross_file;

fn main() {
    // 数学运算
    let sum = math_ops::add(1, 2);
    let diff = math_ops::subtract(5, 3);
    let product = math_ops::multiply(4, 5);
    let quotient = math_ops::divide(10, 2);
    let pow = math_ops::power(2, 10);

    // 字符串处理
    let greeting = string_utils::format_greeting("World");
    let num = string_utils::parse_number("42");
    let is_valid = string_utils::validate_email("test@example.com");

    // 数据处理
    let data = vec![3, -1, 5, 0, -2, 8];
    let sorted = data_ops::sort_data(data.clone());
    let positives = data_ops::filter_positive(data.clone());
    let found = data_ops::search_value(&data, 5);

    // 奇怪名字的函数
    let weird_sum = weird_names::xyz_abc_123(1, 2);
    let weird_diff = weird_names::qwerty_uio_456(5, 3);
    let weird_product = weird_names::asdf_ghjk_789(4, 5);
    let weird_greeting = weird_names::plm_okn_ijb("test");

    // 跨文件依赖
    let total = cross_file::compute_sum(1, 2, 3);
    let processed = cross_file::process_data_pipeline(data);
    let user_info = cross_file::format_user_info("Alice", 30);
    let errors = cross_file::validate_user_input("", "invalid", 200);

    println!("Sum: {}, Product: {}, Greeting: {}", sum, product, greeting);
    println!("Weird sum: {}, Weird product: {}", weird_sum, weird_product);
    println!("Total: {}, Processed: {:?}", total, processed);
    println!("User info: {}, Errors: {:?}", user_info, errors);
}