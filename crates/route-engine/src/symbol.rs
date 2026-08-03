//! 代码符号解析器
//!
//! 提供基于正则表达式的代码符号提取（tree-sitter 不可用时的回退方案），
//! 支持函数、变量、类、接口、结构体、枚举、方法、模块等符号类型。

use std::fs;
use std::path::Path;

/// 代码符号类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CodeSymbol {
    Function,
    Variable,
    Class,
    Interface,
    Struct,
    Enum,
    Method,
    Module,
}

impl CodeSymbol {
    /// 返回符号类型的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            CodeSymbol::Function => "function",
            CodeSymbol::Variable => "variable",
            CodeSymbol::Class => "class",
            CodeSymbol::Interface => "interface",
            CodeSymbol::Struct => "struct",
            CodeSymbol::Enum => "enum",
            CodeSymbol::Method => "method",
            CodeSymbol::Module => "module",
        }
    }
}

/// 符号信息
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub kind: CodeSymbol,
    pub name: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub doc_comment: Option<String>,
}

/// 符号索引
#[derive(Debug, Clone, Default)]
pub struct SymbolIndex {
    pub symbols: Vec<SymbolInfo>,
}

impl SymbolIndex {
    /// 创建一个空的符号索引
    pub fn new() -> Self {
        SymbolIndex {
            symbols: Vec::new(),
        }
    }

    /// 解析文件的符号
    pub fn parse_file(&mut self, path: &str) -> anyhow::Result<Vec<SymbolInfo>> {
        let content = fs::read_to_string(path)?;
        let language = Self::detect_language(path);
        let symbols = self.parse_code(&content, &language);
        Ok(symbols)
    }

    /// 解析代码字符串，返回符号列表
    pub fn parse_code(&self, code: &str, language: &str) -> Vec<SymbolInfo> {
        match language {
            "rust" => self.parse_rust(code),
            "python" => self.parse_python(code),
            "javascript" | "typescript" => self.parse_javascript(code),
            "go" => self.parse_go(code),
            "java" => self.parse_java(code),
            _ => self.parse_generic(code),
        }
    }

    /// 检测文件语言
    fn detect_language(path: &str) -> String {
        let path = Path::new(path);
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust".to_string(),
            Some("py") => "python".to_string(),
            Some("js") => "javascript".to_string(),
            Some("ts") => "typescript".to_string(),
            Some("go") => "go".to_string(),
            Some("java") => "java".to_string(),
            Some("c" | "h") => "c".to_string(),
            Some("cpp" | "hpp" | "cc" | "cxx") => "cpp".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// 解析 Rust 代码
    fn parse_rust(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        // 提取 fn 定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*[<(]"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Function,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 struct 定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?struct\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Struct,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 enum 定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?enum\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Enum,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 trait 定义（作为 interface）
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?trait\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Interface,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 impl 块（模块）
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?impl\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Module,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 mod 定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:pub\s+)?mod\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Module,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        symbols
    }

    /// 解析 Python 代码
    fn parse_python(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        // 提取 def 函数定义
        let re = regex_lite::Regex::new(r"(?m)^\s*def\s+(\w+)\s*\(").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Function,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 class 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*class\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Class,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        symbols
    }

    /// 解析 JavaScript/TypeScript 代码
    fn parse_javascript(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = code.lines().collect();

        // 提取 function 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Function,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 class 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:export\s+)?class\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Class,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 const/let/var 变量定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:export\s+)?(?:const|let|var)\s+(\w+)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Variable,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取 interface 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:export\s+)?interface\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Interface,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        // 提取 enum 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:export\s+)?enum\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Enum,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: Self::extract_doc_comment(lines.as_slice(), line_no),
                    });
                }
            }
        }

        symbols
    }

    /// 解析 Go 代码
    fn parse_go(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();

        // 提取 func 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*func\s+(?:\(\w+\s+\*?\w+\)\s+)?(\w+)\s*\(").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Function,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取 type struct 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*type\s+(\w+)\s+struct").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Struct,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取 type interface 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*type\s+(\w+)\s+interface").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Interface,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        symbols
    }

    /// 解析 Java 代码
    fn parse_java(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();

        // 提取 class 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:public\s+|private\s+|protected\s+)?(?:abstract\s+|final\s+)?class\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Class,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取 interface 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:public\s+)?interface\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Interface,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取 enum 定义
        let re = regex_lite::Regex::new(r"(?m)^\s*(?:public\s+)?enum\s+(\w+)").ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Enum,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        // 提取方法定义
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(?:public|private|protected|static|final|abstract|synchronized|native)\s+(?:\w+\s+)*(\w+)\s*\([^)]*\)\s*(?:throws\s+\w+(?:\s*,\s*\w+)*\s*)?\{"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    // 过滤掉不是方法的（如控制流关键字）
                    let name = m.as_str().to_string();
                    if name == "if" || name == "for" || name == "while" || name == "switch" {
                        continue;
                    }
                    let line_no = code[..m.start()].lines().count();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Method,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        symbols
    }

    /// 通用解析（兜底）
    fn parse_generic(&self, code: &str) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();

        // 尝试匹配常见的函数定义模式
        let re = regex_lite::Regex::new(
            r"(?m)^\s*(\w+)\s*\([^)]*\)\s*(?:\{|=>|do|where)"
        ).ok();
        if let Some(re) = re {
            for cap in re.captures_iter(code) {
                if let Some(m) = cap.get(1) {
                    let line_no = code[..m.start()].lines().count();
                    let name = m.as_str().to_string();
                    symbols.push(SymbolInfo {
                        kind: CodeSymbol::Function,
                        name,
                        file_path: String::new(),
                        line_start: line_no,
                        line_end: line_no,
                        doc_comment: None,
                    });
                }
            }
        }

        symbols
    }

    /// 提取前面的文档注释
    fn extract_doc_comment(lines: &[&str], line_no: usize) -> Option<String> {
        let mut comment_lines = Vec::new();
        // 从当前行往上找连续注释
        let mut idx = line_no.saturating_sub(1); // 0-indexed
        while idx > 0 {
            let trimmed = lines[idx - 1].trim();
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                comment_lines.push(trimmed);
                idx -= 1;
            } else if trimmed.starts_with("/*") || trimmed.starts_with("/**") {
                comment_lines.push(trimmed);
                break;
            } else if trimmed.is_empty() {
                break;
            } else {
                break;
            }
        }
        if comment_lines.is_empty() {
            None
        } else {
            comment_lines.reverse();
            Some(comment_lines.join("\n"))
        }
    }

    /// 添加符号
    pub fn add_symbol(&mut self, info: SymbolInfo) {
        self.symbols.push(info);
    }

    /// 获取所有符号
    pub fn symbols(&self) -> &[SymbolInfo] {
        &self.symbols
    }

    /// 按类型筛选符号
    pub fn filter_by_kind(&self, kind: &CodeSymbol) -> Vec<&SymbolInfo> {
        self.symbols.iter().filter(|s| s.kind == *kind).collect()
    }

    /// 按名称搜索符号
    pub fn search_by_name(&self, name: &str) -> Vec<&SymbolInfo> {
        self.symbols
            .iter()
            .filter(|s| s.name.contains(name) || s.name.eq_ignore_ascii_case(name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_symbol_as_str() {
        assert_eq!(CodeSymbol::Function.as_str(), "function");
        assert_eq!(CodeSymbol::Class.as_str(), "class");
        assert_eq!(CodeSymbol::Struct.as_str(), "struct");
        assert_eq!(CodeSymbol::Enum.as_str(), "enum");
        assert_eq!(CodeSymbol::Method.as_str(), "method");
        assert_eq!(CodeSymbol::Module.as_str(), "module");
        assert_eq!(CodeSymbol::Interface.as_str(), "interface");
        assert_eq!(CodeSymbol::Variable.as_str(), "variable");
    }

    #[test]
    fn test_parse_rust_functions() {
        let index = SymbolIndex::new();
        let code = r#"
fn hello() {
    println!("hello");
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub unsafe fn dangerous() {}
"#;
        let symbols = index.parse_code(code, "rust");
        let fns: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Function).collect();
        assert_eq!(fns.len(), 3);
        assert!(fns.iter().any(|s| s.name == "hello"));
        assert!(fns.iter().any(|s| s.name == "add"));
        assert!(fns.iter().any(|s| s.name == "dangerous"));
    }

    #[test]
    fn test_parse_rust_structs() {
        let index = SymbolIndex::new();
        let code = r#"
struct Point {
    x: i32,
    y: i32,
}

pub struct Config {
    pub name: String,
}
"#;
        let symbols = index.parse_code(code, "rust");
        let structs: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Struct).collect();
        assert_eq!(structs.len(), 2);
        assert!(structs.iter().any(|s| s.name == "Point"));
        assert!(structs.iter().any(|s| s.name == "Config"));
    }

    #[test]
    fn test_parse_rust_enums() {
        let index = SymbolIndex::new();
        let code = r#"
enum Color {
    Red,
    Green,
    Blue,
}

pub enum Option<T> {
    Some(T),
    None,
}
"#;
        let symbols = index.parse_code(code, "rust");
        let enums: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Enum).collect();
        assert_eq!(enums.len(), 2);
    }

    #[test]
    fn test_parse_python() {
        let index = SymbolIndex::new();
        let code = r#"
def hello():
    print("hello")

class MyClass:
    def method(self):
        pass
"#;
        let symbols = index.parse_code(code, "python");
        let fns: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Function).collect();
        let classes: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Class).collect();
        assert_eq!(fns.len(), 2);
        assert_eq!(classes.len(), 1);
    }

    #[test]
    fn test_parse_javascript() {
        let index = SymbolIndex::new();
        let code = r#"
function hello() {
    console.log("hello");
}

const name = "test";

class MyClass {
    method() {}
}

export function add(a, b) {
    return a + b;
}
"#;
        let symbols = index.parse_code(code, "javascript");
        let fns: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Function).collect();
        let vars: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Variable).collect();
        let classes: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Class).collect();
        assert_eq!(fns.len(), 2);
        assert_eq!(vars.len(), 1);
        assert_eq!(classes.len(), 1);
    }

    #[test]
    fn test_parse_go() {
        let index = SymbolIndex::new();
        let code = r#"
func hello() {
    fmt.Println("hello")
}

type Point struct {
    X int
    Y int
}

type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;
        let symbols = index.parse_code(code, "go");
        let fns: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Function).collect();
        let structs: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Struct).collect();
        let ifaces: Vec<&SymbolInfo> = symbols.iter().filter(|s| s.kind == CodeSymbol::Interface).collect();
        assert_eq!(fns.len(), 1);
        assert_eq!(structs.len(), 1);
        assert_eq!(ifaces.len(), 1);
    }

    #[test]
    fn test_search_by_name() {
        let mut index = SymbolIndex::new();
        index.add_symbol(SymbolInfo {
            kind: CodeSymbol::Function,
            name: "hello".to_string(),
            file_path: "test.rs".to_string(),
            line_start: 1,
            line_end: 3,
            doc_comment: None,
        });
        index.add_symbol(SymbolInfo {
            kind: CodeSymbol::Function,
            name: "hello_world".to_string(),
            file_path: "test.rs".to_string(),
            line_start: 5,
            line_end: 7,
            doc_comment: None,
        });

        let results = index.search_by_name("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_filter_by_kind() {
        let mut index = SymbolIndex::new();
        index.add_symbol(SymbolInfo {
            kind: CodeSymbol::Function,
            name: "f1".to_string(),
            file_path: "test.rs".to_string(),
            line_start: 1,
            line_end: 1,
            doc_comment: None,
        });
        index.add_symbol(SymbolInfo {
            kind: CodeSymbol::Struct,
            name: "S1".to_string(),
            file_path: "test.rs".to_string(),
            line_start: 3,
            line_end: 5,
            doc_comment: None,
        });

        assert_eq!(index.filter_by_kind(&CodeSymbol::Function).len(), 1);
        assert_eq!(index.filter_by_kind(&CodeSymbol::Struct).len(), 1);
        assert_eq!(index.filter_by_kind(&CodeSymbol::Class).len(), 0);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(SymbolIndex::detect_language("main.rs"), "rust");
        assert_eq!(SymbolIndex::detect_language("main.py"), "python");
        assert_eq!(SymbolIndex::detect_language("main.js"), "javascript");
        assert_eq!(SymbolIndex::detect_language("main.ts"), "typescript");
        assert_eq!(SymbolIndex::detect_language("main.go"), "go");
        assert_eq!(SymbolIndex::detect_language("main.java"), "java");
        assert_eq!(SymbolIndex::detect_language("main.c"), "c");
        assert_eq!(SymbolIndex::detect_language("main.cpp"), "cpp");
    }

    #[test]
    fn test_empty_code() {
        let index = SymbolIndex::new();
        let symbols = index.parse_code("", "rust");
        assert!(symbols.is_empty());
    }
}