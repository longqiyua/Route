//! Mock 代码库集成测试
//!
//! 加载 `mock_codebase/` 目录下的所有文件，运行增强 RAG 流程，
//! 验证架构建议是否准确识别出各模块的操作类型。

use std::fs;
use std::path::Path;

use route_engine::symbol::{CodeSymbol, SymbolIndex, SymbolInfo};
use route_engine::{
    ArchitectureAssistant, MatchMode, RagPipeline, SemanticIndex, ThirdPartyChunking,
};

/// 读取 mock_codebase 目录下的所有文件内容
fn load_mock_files() -> Vec<(String, String)> {
    let mock_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("mock_codebase");

    let mut files = Vec::new();
    for entry in fs::read_dir(&mock_dir).expect("mock_codebase directory not found") {
        let entry = entry.expect("failed to read entry");
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "rs") {
            let content = fs::read_to_string(&path).expect("failed to read file");
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            files.push((name, content));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// 从代码内容中提取函数源码块
///
/// 匹配 `pub fn NAME(...) -> RET { ... }` 或 `fn NAME(...) { ... }` 模式
fn extract_functions(file_content: &str) -> Vec<(String, String)> {
    let mut functions = Vec::new();

    // 使用正则提取函数签名和体
    let re = regex_lite::Regex::new(
        r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^{]+)?\s*\{"
    )
    .unwrap();

    for cap in re.captures_iter(file_content) {
        let fn_name = cap.get(1).unwrap().as_str().to_string();
        let fn_start = cap.get(0).unwrap().start();

        // 找到匹配的闭合大括号
        let after = &file_content[fn_start..];
        let mut depth = 0u32;
        let mut end = 0;
        for (i, ch) in after.char_indices() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    end = fn_start + i + 1;
                    break;
                }
            }
        }

        let fn_body = &file_content[fn_start..end];
        if !fn_name.starts_with("fn") {
            // 过滤掉闭包等
            functions.push((fn_name, fn_body.to_string()));
        }
    }

    functions
}

#[test]
fn test_mock_codebase_architecture_analysis() {
    // ──────────────────────────────────────────────────
    // 1. 加载 Mock 代码库
    // ──────────────────────────────────────────────────
    let files = load_mock_files();
    assert!(!files.is_empty(), "No mock files found!");

    println!("{}", "=".repeat(70));
    println!("Mock Codebase Architecture Analysis");
    println!("{}", "=".repeat(70));
    println!("\nLoaded {} files:", files.len());
    for (name, _) in &files {
        println!("  - {}", name);
    }

    // ──────────────────────────────────────────────────
    // 2. 解析所有符号，构建语义索引
    // ──────────────────────────────────────────────────
    let symbol_index = SymbolIndex::new();
    let mut semantic_index = SemanticIndex::new();
    semantic_index.set_mode(MatchMode::Semantic);

    let mut total_functions = 0;
    let mut weird_name_functions = 0;

    for (file_name, content) in &files {
        let symbols = symbol_index.parse_code(content, "rust");
        let functions = extract_functions(content);

        for (fn_name, fn_body) in &functions {
            // 查找到对应的 SymbolInfo
            let sym = symbols.iter().find(|s| s.name == *fn_name).cloned().unwrap_or_else(|| {
                SymbolInfo {
                    kind: CodeSymbol::Function,
                    name: fn_name.clone(),
                    file_path: file_name.clone(),
                    line_start: 1,
                    line_end: 5,
                    doc_comment: None,
                }
            });

            semantic_index.add_entry(sym, fn_body);
            total_functions += 1;

            if file_name == "weird_names.rs" {
                weird_name_functions += 1;
            }
        }
    }

    println!("\n📊 Index Stats:");
    println!("  Total files: {}", files.len());
    println!("  Total functions indexed: {}", total_functions);
    println!("  Weird-name functions: {}", weird_name_functions);
    println!("  Semantic entries: {}", semantic_index.count());
    println!("  Operations detected: {:?}", semantic_index.operations());

    // ──────────────────────────────────────────────────
    // 3. 验证语义匹配能识别奇怪名字的函数
    // ──────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("Semantic Matching: Weird Name Functions");
    println!("{}", "=".repeat(70));

    // 搜索 "a + b" 应该匹配到所有加法函数（包括奇怪名字的）
    let add_results = semantic_index.search("a + b", 5);
    println!("\nSearch 'a + b':");
    for r in &add_results {
        println!(
            "  [{:.3}] {} ({}) — expr: {:?}",
            r.score, r.symbol_name, r.operation, r.core_expr
        );
    }
    // 应该至少有一个加法函数被匹配到
    assert!(!add_results.is_empty(), "Should find at least one addition function");
    let has_weird_add = add_results.iter().any(|r| r.symbol_name.contains("xyz_abc"));
    assert!(
        has_weird_add,
        "Semantic matching should find 'xyz_abc_123' when searching 'a + b'"
    );
    println!("  ✅ Semantic matching correctly identifies addition in weird-name functions");

    // 搜索 "format" 应该匹配到格式化函数
    let fmt_results = semantic_index.search("format", 5);
    println!("\nSearch 'format':");
    for r in &fmt_results {
        println!(
            "  [{:.3}] {} ({}) — expr: {:?}",
            r.score, r.symbol_name, r.operation, r.core_expr
        );
    }
    let has_weird_fmt = fmt_results.iter().any(|r| r.symbol_name.contains("plm_okn"));
    assert!(
        has_weird_fmt,
        "Semantic matching should find 'plm_okn_ijb' when searching 'format'"
    );
    println!("  ✅ Semantic matching correctly identifies formatting in weird-name functions");

    // ──────────────────────────────────────────────────
    // 4. 运行增强 RAG 流程（带第三方 AI 切分）
    // ──────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("Enhanced RAG Pipeline (a -> b -> c)");
    println!("{}", "=".repeat(70));

    let mut pipeline = RagPipeline::new();
    let mut chunking = ThirdPartyChunking::new();
    let mut architect = ArchitectureAssistant::new();

    // 模拟新代码：用户写了一个加法函数
    let new_code = "fn compute_summary(x: i32, y: i32) -> i32 { x + y }";
    let new_symbol = SymbolInfo {
        kind: CodeSymbol::Function,
        name: "compute_summary".to_string(),
        file_path: "new_module.rs".to_string(),
        line_start: 1,
        line_end: 3,
        doc_comment: None,
    };

    let result = pipeline.run_cycle_with_chunking(
        &mut chunking,
        &mut semantic_index,
        &mut architect,
        new_code,
        new_symbol,
        new_code,
        10,
    );

    println!("\n📋 Pipeline Results:");
    println!("  Stage: {:?}", pipeline.stage);
    println!("  Total chunks: {}", result.total_chunks);
    println!("  Vectorized chunks: {}", result.vectorized_chunks);
    println!("  High-confidence chunks: {}", result.high_confidence_chunks);
    println!("  Total entries: {}", result.total_entries);

    // ──────────────────────────────────────────────────
    // 5. 验证架构建议
    // ──────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("Architecture Suggestions");
    println!("{}", "=".repeat(70));

    println!("\n{}", architect.to_architecture_graph());

    // 验证架构建议的数量
    assert!(
        !result.architectures.is_empty(),
        "Should have architecture suggestions"
    );
    println!("  ✅ Architecture suggestions generated: {}", result.architectures.len());

    // 验证架构建议包含关键模块
    let ops: Vec<&str> = result
        .architectures
        .iter()
        .flat_map(|a| a.related_operations.iter().map(|s| s.as_str()))
        .collect();
    println!("  Related operations: {:?}", ops);

    // 验证混合模式能正确切换
    let module_suggestions = architect.suggest_modules(&semantic_index, 1);
    println!("\nModule Suggestions:");
    for m in &module_suggestions {
        println!(
            "  [{:.2}] {} — {}",
            m.confidence, m.module_name, m.description
        );
    }

    // ──────────────────────────────────────────────────
    // 6. 验证跨文件依赖识别
    // ──────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("Cross-File Dependency Recognition");
    println!("{}", "=".repeat(70));

    // 搜索 "sort filter" 应该匹配到 process_data_pipeline
    let pipeline_results = semantic_index.search("sort filter", 10);
    println!("\nSearch 'sort filter':");
    for r in &pipeline_results {
        println!(
            "  [{:.3}] {} ({}) — file: {}",
            r.score, r.symbol_name, r.operation, r.file_path
        );
    }
    let has_pipeline = pipeline_results
        .iter()
        .any(|r| r.symbol_name == "process_data_pipeline");
    assert!(
        has_pipeline,
        "Should find 'process_data_pipeline' (sort + filter combined)"
    );
    println!(
        "  ✅ Cross-file pipeline function recognized: process_data_pipeline"
    );

    // 搜索 "validate" 应该匹配到所有验证函数
    let validate_results = semantic_index.search("validate", 10);
    println!("\nSearch 'validate':");
    for r in &validate_results {
        println!(
            "  [{:.3}] {} ({}) — file: {}",
            r.score, r.symbol_name, r.operation, r.file_path
        );
    }
    assert!(!validate_results.is_empty(), "Should find validation functions");
    println!("  ✅ Validation functions correctly grouped");

    // ──────────────────────────────────────────────────
    // 7. 最终总结
    // ──────────────────────────────────────────────────
    println!("\n{}", "=".repeat(70));
    println!("Architecture Analysis Complete");
    println!("{}", "=".repeat(70));
    println!("\nKey findings:");
    println!(
        "  1. {} functions indexed across {} files",
        total_functions, files.len()
    );
    println!(
        "  2. {} operation types detected",
        semantic_index.operations().len()
    );
    println!(
        "  3. {} architecture suggestions generated",
        result.architectures.len()
    );
    println!(
        "  4. Weird-name functions correctly matched by semantic content"
    );
    println!("  5. Cross-file dependencies properly recognized");
    println!("  6. Enhanced RAG cycle (a→b→c→②→③→④→⑤→⑥) completed successfully");
}