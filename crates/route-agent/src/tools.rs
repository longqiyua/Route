//! Agent 工具系统：read_file, write_file, list_dir, search, git_ops 等。
//!
//! Agent 在 plan → act 循环中通过 ToolRegistry 调用工具。

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u128,
}

pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameter_schema(&self) -> serde_json::Value;
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String>;
}

pub struct ToolRegistry {
    tools: std::collections::BTreeMap<String, Box<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { Self { tools: Default::default() } }

    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        for t in builtin_tools() {
            r.register(t);
        }
        r
    }

    pub fn register(&mut self, tool: Box<dyn AgentTool>) {
        info!(
            "[tool] registering tool: name={}, description_len={}",
            tool.name(), tool.description().len()
        );
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn AgentTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn list(&self) -> Vec<(&str, &str, serde_json::Value)> {
        self.tools.values().map(|t| (t.name(), t.description(), t.parameter_schema())).collect()
    }

    pub fn execute(&self, project_path: Option<&Path>, call: &ToolCall) -> ToolResult {
        let start = std::time::Instant::now();
        info!(
            "[tool] executing: name={}, call_id={}, args_keys={:?}, has_project={}",
            call.tool, call.id,
            call.args.as_object().map(|o| o.keys().collect::<Vec<_>>()),
            project_path.is_some()
        );
        match self.get(&call.tool) {
            Some(tool) => {
                debug!("[tool] found tool: {}, calling...", call.tool);
                match tool.call(project_path, call.args.clone()) {
                    Ok(output) => {
                        let dur = start.elapsed().as_millis();
                        info!(
                            "[tool] success: name={}, output_len={}, duration={}ms",
                            call.tool, output.len(), dur
                        );
                        ToolResult {
                            call_id: call.id.clone(),
                            success: true,
                            output,
                            error: None,
                            duration_ms: dur,
                        }
                    }
                    Err(e) => {
                        let dur = start.elapsed().as_millis();
                        error!(
                            "[tool] FAILED: name={}, error={}, duration={}ms",
                            call.tool, e, dur
                        );
                        ToolResult {
                            call_id: call.id.clone(),
                            success: false,
                            output: String::new(),
                            error: Some(e.to_string()),
                            duration_ms: dur,
                        }
                    }
                }
            }
            None => {
                error!("[tool] not found: name={}", call.tool);
                ToolResult {
                    call_id: call.id.clone(),
                    success: false,
                    output: String::new(),
                    error: Some(format!("tool not found: {}", call.tool)),
                    duration_ms: start.elapsed().as_millis(),
                }
            }
        }
    }
}

impl Default for ToolRegistry { fn default() -> Self { Self::with_builtins() } }

// ---------- 内置工具 ----------

pub fn builtin_tools() -> Vec<Box<dyn AgentTool>> {
    vec![
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(ListDirTool),
        Box::new(SearchContentTool),
        Box::new(GlobTool),
    ]
}

struct ReadFileTool;
impl AgentTool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read a text file from the project. Args: {path: string, max_chars?: number}" }
    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["path"],"properties":{"path":{"type":"string"},"max_chars":{"type":"number","default":20000}}})
    }
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String> {
        let p = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("read_file requires 'path'"))?;
        let max = args["max_chars"].as_u64().unwrap_or(20000) as usize;
        let full = resolve(project_path, p)?;
        debug!(
            "[tool:read_file] resolving path: input={}, resolved={}, max_chars={}",
            p, full.display(), max
        );
        let content = std::fs::read_to_string(&full)
            .map_err(|e| {
                error!("[tool:read_file] failed to read {}: {}", full.display(), e);
                anyhow::anyhow!("read {}: {e}", full.display())
            })?;
        let original_len = content.len();
        let result = if content.len() > max {
            let mut t: String = content.chars().take(max).collect();
            t.push_str(&format!("\n... [truncated at {max} chars, total {}]", content.len()));
            t
        } else { content };
        debug!(
            "[tool:read_file] read {} chars from {} (original={} chars)",
            result.len(), full.display(), original_len
        );
        Ok(result)
    }
}

struct WriteFileTool;
impl AgentTool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Create or overwrite a text file. Args: {path: string, content: string}" }
    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["path","content"],"properties":{"path":{"type":"string"},"content":{"type":"string"}}})
    }
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String> {
        let p = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("write_file requires 'path'"))?;
        let content = args["content"].as_str().ok_or_else(|| anyhow::anyhow!("write_file requires 'content'"))?;
        let full = resolve(project_path, p)?;
        let size = content.len();
        info!(
            "[tool:write_file] writing: path={}, content_len={}, creating_dirs={}",
            full.display(), size, full.parent().map(|_| true).unwrap_or(false)
        );
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;
        info!("[tool:write_file] successfully wrote {} bytes to {}", size, full.display());
        Ok(format!("wrote {} bytes to {}", size, full.display()))
    }
}

struct ListDirTool;
impl AgentTool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str { "List directory entries. Args: {path?: string, max_items?: number}" }
    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"path":{"type":"string","default":"."},"max_items":{"type":"number","default":200}}})
    }
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String> {
        let p = args["path"].as_str().unwrap_or(".");
        let max = args["max_items"].as_u64().unwrap_or(200) as usize;
        let full = resolve(project_path, p)?;
        debug!(
            "[tool:list_dir] listing: path={}, max_items={}",
            full.display(), max
        );
        let mut entries: Vec<String> = Vec::new();
        for (i, entry) in std::fs::read_dir(&full)?.flatten().take(max).enumerate() {
            let meta = entry.metadata().ok();
            let ty = if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) { "/" } else { "" };
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            entries.push(format!("{:3}  {}{}  [{}B]", i, entry.file_name().to_string_lossy(), ty, size));
        }
        debug!(
            "[tool:list_dir] found {} entries in {}",
            entries.len(), full.display()
        );
        Ok(entries.join("\n"))
    }
}

struct SearchContentTool;
impl AgentTool for SearchContentTool {
    fn name(&self) -> &str { "search_content" }
    fn description(&self) -> &str { "Search file contents for a regex pattern. Args: {pattern: string, path?: string, max_results?: number}" }
    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["pattern"],"properties":{"pattern":{"type":"string"},"path":{"type":"string","default":"."},"max_results":{"type":"number","default":50}}})
    }
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String> {
        let pattern = args["pattern"].as_str().ok_or_else(|| anyhow::anyhow!("search_content requires 'pattern'"))?;
        let p = args["path"].as_str().unwrap_or(".");
        let max = args["max_results"].as_u64().unwrap_or(50) as usize;
        let full = resolve(project_path, p)?;
        debug!(
            "[tool:search_content] searching: pattern={}, path={}, max_results={}",
            pattern, full.display(), max
        );
        let re = regex_lite(pattern)?;
        use std::cell::RefCell;
        let results: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let file_count = std::sync::atomic::AtomicUsize::new(0);
        visit_files(&full, &|file| {
            file_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut out = results.borrow_mut();
            if out.len() >= max { return; }
            if let Ok(content) = std::fs::read_to_string(file) {
                for (lineno, line) in content.lines().enumerate() {
                    if out.len() >= max { break; }
                    if re.is_match(line) {
                        let rel = file.strip_prefix(&full).unwrap_or(file);
                        out.push(format!("{}:{}:{}", rel.to_string_lossy(), lineno + 1, line.trim()));
                    }
                }
            }
        }, 0);
        let result_count = results.borrow().len();
        debug!(
            "[tool:search_content] searched {} files, found {} matches",
            file_count.load(std::sync::atomic::Ordering::Relaxed), result_count
        );
        Ok(results.into_inner().join("\n"))
    }
}

struct GlobTool;
impl AgentTool for GlobTool {
    fn name(&self) -> &str { "glob" }
    fn description(&self) -> &str { "Find files by glob pattern (e.g. src/**/*.rs). Args: {pattern: string, max_results?: number}" }
    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","required":["pattern"],"properties":{"pattern":{"type":"string"},"max_results":{"type":"number","default":200}}})
    }
    fn call(&self, project_path: Option<&Path>, args: serde_json::Value) -> anyhow::Result<String> {
        let pat = args["pattern"].as_str().ok_or_else(|| anyhow::anyhow!("glob requires 'pattern'"))?;
        let max = args["max_results"].as_u64().unwrap_or(200) as usize;
        let base = project_path.unwrap_or_else(|| Path::new("."));
        let full_pat = base.join(pat).to_string_lossy().to_string();
        debug!(
            "[tool:glob] globbing: pattern={}, max_results={}, base={}",
            pat, max, base.display()
        );
        let mut matches: Vec<String> = Vec::new();
        if let Ok(glob) = glob_lite(&full_pat) {
            for (i, path) in glob.iter().take(max).enumerate() {
                matches.push(format!("{:3}  {}", i, path));
            }
        }
        debug!("[tool:glob] found {} matches for pattern '{}'", matches.len(), pat);
        Ok(matches.join("\n"))
    }
}

// ---------- 辅助函数（不引入额外依赖的极简实现） ----------

fn resolve(base: Option<&Path>, rel: &str) -> anyhow::Result<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() { Ok(p.to_path_buf()) } else {
        match base {
            Some(b) => Ok(b.join(p)),
            None => Ok(p.to_path_buf()),
        }
    }
}

fn visit_files(dir: &Path, cb: &dyn Fn(&Path), depth: usize) {
    if depth > 10 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    let skip = [".git", ".route", "node_modules", "target", "dist", "build", ".next"];
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if skip.iter().any(|s| *s == name) { continue; }
            visit_files(&path, cb, depth + 1);
        } else if path.is_file() {
            cb(&path);
        }
    }
}

fn regex_lite(pattern: &str) -> anyhow::Result<LiteRegex> {
    Ok(LiteRegex { pat: pattern.to_string() })
}

struct LiteRegex { pat: String }
impl LiteRegex {
    fn is_match(&self, line: &str) -> bool {
        let mut p = self.pat.as_str();
        let prefix = p.starts_with('^');
        if prefix { p = &p[1..]; }
        let suffix = p.ends_with('$');
        if suffix { p = &p[..p.len()-1]; }
        if prefix && suffix { line == p }
        else if prefix { line.starts_with(p) }
        else if suffix { line.ends_with(p) }
        else { line.contains(p) }
    }
}

fn glob_lite(pattern: &str) -> anyhow::Result<Vec<String>> {
    use std::cell::RefCell;
    let mut base = Path::new(pattern);
    while base.file_name().map(|f| f.to_string_lossy().contains('*')).unwrap_or(false) {
        base = base.parent().unwrap_or(Path::new(""));
    }
    let result = RefCell::new(Vec::new());
    if base.exists() {
        visit_files(base, &|p| {
            if let Some(s) = p.to_str() {
                if let Some(first_star) = pattern.rfind('*') {
                    let after = &pattern[first_star+1..];
                    if s.ends_with(after) { result.borrow_mut().push(s.to_string()); }
                }
            }
        }, 0);
    }
    Ok(result.into_inner())
}
