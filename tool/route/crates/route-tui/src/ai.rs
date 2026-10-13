//! AI integration for route-tui — Claude-Code-style AI assistant.
//!
//! Adds an `ai` command that loads project context, sends it to the
//! configured AI provider, and displays the response. Also detects
//! dangerous operations (rollback, reset, hard push, etc.) and
//! proactively warns the user.
//!
//! ## Configuration
//!
//! Set these environment variables to configure the AI provider:
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `ROUTE_AI_PROVIDER` | `openai` | Display label; endpoint must speak OpenAI-compatible chat |
//! | `ROUTE_AI_ENDPOINT` | `https://api.openai.com/v1` | API endpoint |
//! | `ROUTE_AI_KEY` | — | API key (required) |
//! | `ROUTE_AI_MODEL` | `gpt-4o` | Model name |

use std::path::Path;

use crate::fmt::{accent, dim, err, faint, warn};

// ---------------------------------------------------------------------------
// AI Config
// ---------------------------------------------------------------------------

/// AI provider configuration read from environment variables.
#[derive(Clone)]
pub struct AiConfig {
    pub provider: String,
    pub endpoint: String,
    pub key: String,
    pub model: String,
}

impl AiConfig {
    /// Read AI config from environment variables.
    /// Returns `None` when the key is not set (AI is disabled).
    pub fn from_env() -> Option<Self> {
        let config = Self::read_env();
        (!config.key.is_empty()).then_some(config)
    }
    fn read_env() -> Self {
        let key = std::env::var("ROUTE_AI_KEY").unwrap_or_default();
        Self {
            provider: std::env::var("ROUTE_AI_PROVIDER").unwrap_or_else(|_| "openai".to_string()),
            endpoint: std::env::var("ROUTE_AI_ENDPOINT")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            key,
            model: std::env::var("ROUTE_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string()),
        }
    }

    /// Print the current config (masked key).
    pub fn print(&self) {
        let masked = if self.key.is_empty() {
            "not configured"
        } else {
            "[configured; hidden]"
        };
        println!("  {} {}", dim("Provider:"), accent(&self.provider));
        // URL userinfo/query may contain credentials; never echo either.
        let endpoint = reqwest::Url::parse(&self.endpoint)
            .ok()
            .map(|mut url| {
                let _ = url.set_username("");
                let _ = url.set_password(None);
                url.set_query(None);
                url.set_fragment(None);
                url.to_string()
            })
            .unwrap_or_else(|| "[invalid endpoint; hidden]".into());
        println!("  {} {}", dim("Endpoint:"), faint(&endpoint));
        println!("  {} {}", dim("Model:"), accent(&self.model));
        println!("  {} {}", dim("Key:"), faint(masked));
    }
}

// ---------------------------------------------------------------------------
// Project Context
// ---------------------------------------------------------------------------

/// Load project context from the project path.
/// Returns a formatted text block for AI injection.
pub fn load_project_context(project_path: &Path) -> String {
    let mut sections: Vec<String> = Vec::new();

    // 1. Project structure
    let tree = build_tree(project_path);
    sections.push(format!("## Project Structure\n{tree}"));

    // 2. Current branch
    let branch = get_current_branch(project_path);
    sections.push(format!("## Current Branch\n{branch}"));

    // 3. Recent commits
    let commits = get_recent_commits(project_path);
    sections.push(format!("## Recent Commits\n{commits}"));

    // 4. References
    let refs = get_references(project_path);
    sections.push(format!("## References\n{refs}"));

    sections.join("\n\n")
}

fn build_tree(project_path: &Path) -> String {
    let mut lines: Vec<String> = Vec::new();
    let skip_dirs = [
        ".route",
        ".git",
        "node_modules",
        "target",
        ".next",
        "dist",
        "build",
    ];

    if let Ok(entries) = std::fs::read_dir(project_path) {
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || skip_dirs.contains(&name.as_str()) {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }

        dirs.sort();
        files.sort();

        for d in &dirs {
            lines.push(format!("  📁 {d}/"));
        }
        for f in &files {
            lines.push(format!("  📄 {f}"));
        }
    }

    if lines.is_empty() {
        lines.push("  (empty project)".to_string());
    }
    lines.join("\n")
}

fn get_current_branch(project_path: &Path) -> String {
    use std::process::Command;
    let output = Command::new("git")
        .args([
            "-C",
            &project_path.to_string_lossy(),
            "branch",
            "--show-current",
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "(unknown)".to_string(),
    }
}

fn get_recent_commits(project_path: &Path) -> String {
    use std::process::Command;
    let output = Command::new("git")
        .args([
            "-C",
            &project_path.to_string_lossy(),
            "log",
            "--oneline",
            "-10",
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = text.lines().collect();
            if lines.is_empty() {
                "  (no commits)".to_string()
            } else {
                lines
                    .iter()
                    .map(|l| format!("  - {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        _ => "  (no git history)".to_string(),
    }
}

fn get_references(project_path: &Path) -> String {
    let refs_dir = project_path.join(".route").join("references");
    if !refs_dir.exists() {
        return "  (no references)".to_string();
    }

    let mut lines: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&refs_dir) {
        let mut files: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let preview: String = content.chars().take(300).collect();
                let trunc = if content.len() > 300 { "..." } else { "" };
                files.push(format!("  - {name}:\n    {preview}{trunc}"));
            }
        }
        files.sort();
        if files.is_empty() {
            return "  (no reference files)".to_string();
        }
        lines = files;
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// AI Chat
// ---------------------------------------------------------------------------

/// Send a chat message to the configured AI provider.
/// Returns the response text.
pub fn ai_chat(
    config: &AiConfig,
    system_prompt: &str,
    user_message: &str,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|_| "cannot initialize AI client".to_string())?;
    let url = format!("{}/chat/completions", config.endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "temperature": 0.7,
        "max_tokens": 4096,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "AI_REQUEST_TIMEOUT".to_string()
            } else {
                "AI_REQUEST_FAILED (endpoint and credentials hidden)".to_string()
            }
        })?;

    let status = response.status();
    let body_text = read_bounded_response(response)?;

    if !status.is_success() {
        return Err(format!("API error ({}) (body hidden)", status.as_u16()));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body_text).map_err(|_| "invalid AI response JSON".to_string())?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "no content in AI response".to_string())?;

    Ok(content.to_string())
}

// ---------------------------------------------------------------------------
// Danger Detection
// ---------------------------------------------------------------------------

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
fn read_bounded_response(reader: impl std::io::Read) -> Result<String, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "AI_RESPONSE_READ_FAILED".to_string())?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("AI_RESPONSE_TOO_LARGE".into());
    }
    String::from_utf8(bytes).map_err(|_| "AI_RESPONSE_INVALID_UTF8".into())
}
fn wrap_utf8(mut line: &str, width: usize) -> Vec<&str> {
    let mut parts = Vec::new();
    while !line.is_empty() {
        let boundary = line
            .char_indices()
            .nth(width.max(1))
            .map(|(index, _)| index)
            .unwrap_or(line.len());
        parts.push(&line[..boundary]);
        line = &line[boundary..];
    }
    if parts.is_empty() {
        parts.push("");
    }
    parts
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    #[test]
    fn unicode_wrap_never_slices_inside_character_or_loops_at_zero_width() {
        let text = "你好Route🦀é世界";
        for width in [0, 1, 2, 5, 80] {
            assert_eq!(wrap_utf8(text, width).concat(), text);
        }
    }
    #[test]
    fn response_reader_rejects_oversize_and_invalid_utf8() {
        assert_eq!(read_bounded_response(&b"hello"[..]).unwrap(), "hello");
        assert!(read_bounded_response(std::io::repeat(b'x'))
            .unwrap_err()
            .contains("TOO_LARGE"));
        assert!(read_bounded_response(&[0xff][..])
            .unwrap_err()
            .contains("INVALID_UTF8"));
    }
    #[test]
    fn help_and_config_are_local_commands() {
        assert!(handle_ai_command(Path::new("."), &["help".into()]));
        assert!(handle_ai_command(Path::new("."), &["config".into()]));
    }
}

/// List of dangerous operations that the AI should warn about.
const DANGEROUS_OPERATIONS: &[(&str, &str, &str)] = &[
    (
        "rollback",
        "rb",
        "Rolls back to a previous snapshot — irreversible.",
    ),
    ("reset", "git reset", "Discards commits — data loss risk."),
    (
        "--hard",
        "hard",
        "Discards working-tree changes permanently.",
    ),
    (
        "push --force",
        "force push",
        "Overwrites remote history — affects collaborators.",
    ),
    (
        "delete branch",
        "branch delete",
        "Removes a branch and its commits.",
    ),
    ("tag delete", "tag delete", "Removes a tag reference."),
    (
        "revert",
        "git revert",
        "Creates an inverse commit — can be disruptive.",
    ),
    (
        "stash drop",
        "stash drop",
        "Permanently removes a stash entry.",
    ),
    (
        "stash clear",
        "stash clear",
        "Permanently removes ALL stashes.",
    ),
    (
        "checkout",
        "git checkout",
        "Can be destructive with --force.",
    ),
    (
        "clean -f",
        "git clean",
        "Removes untracked files permanently.",
    ),
    ("rm -rf", "rm -rf", "Deletes files from disk and index."),
];

/// Check if the input contains a dangerous operation.
/// Returns a warning message if found, `None` otherwise.
pub fn detect_danger(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();
    for (keyword, _alias, warning) in DANGEROUS_OPERATIONS {
        if lower.contains(keyword) {
            return Some(warning);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Main AI command handler
// ---------------------------------------------------------------------------

/// Handle the `ai` command in the TUI.
/// `project_path` is the project directory.
/// `args` is everything after `ai`.
pub fn handle_ai_command(project_path: &Path, args: &[String]) -> bool {
    let config = AiConfig::read_env();
    if args.is_empty() || (args.len() == 1 && (args[0] == "help" || args[0] == "?")) {
        print_ai_help(&config);
        return true;
    }
    if args.len() == 1 && args[0] == "config" {
        config.print();
        return true;
    }
    if config.key.is_empty() {
        eprintln!(
                "{}",
                err("✗ AI not configured. Set ROUTE_AI_KEY (and optionally ROUTE_AI_ENDPOINT, ROUTE_AI_MODEL, ROUTE_AI_PROVIDER).")
            );
        println!(
            "  {} {}",
            dim("Example:"),
            faint("set ROUTE_AI_KEY=sk-... && route-tui ai tell me about this project")
        );
        return false;
    }

    // Check for danger
    let user_message = args.join(" ");
    if let Some(warning) = detect_danger(&user_message) {
        println!();
        println!("  {} {}", warn("⚠ DANGER DETECTED:"), warning);
        print!("  {} ", dim("Continue? (y/N): "));
        use std::io::{Read, Write};
        let mut buf = [0u8; 1];
        let _ = std::io::stdout().flush();
        let _ = std::io::stdin().read_exact(&mut buf);
        let answer = (buf[0] as char).to_ascii_lowercase();
        if answer != 'y' {
            println!("  {}", dim("Cancelled."));
            return true;
        }
    }

    // Load project context
    println!(
        "  {} {}",
        dim("Loading project context..."),
        faint("(this may take a moment)")
    );
    let context = load_project_context(project_path);

    // Build system prompt
    let system_prompt = format!(
        r#"You are an AI assistant integrated with the Route version management system.

You have access to the project's full context below. Use this information to provide accurate, context-aware assistance.

=== Project Context ===
{context}

Key responsibilities:
1. Help the user understand their project structure and history.
2. Suggest improvements and best practices.
3. WARN about potentially dangerous operations.
4. Answer questions about the project's code, commits, and tracking status.
5. Be concise and actionable in your responses.

When you detect a potentially dangerous operation being discussed, explicitly warn the user with "⚠️ DANGER:" prefix."#,
        context = context
    );

    // Send to AI
    println!("  {} {} {}", dim("AI"), accent("⟫"), faint("thinking..."));
    match ai_chat(&config, &system_prompt, &user_message) {
        Ok(response) => {
            println!();
            // Print response with word wrapping at terminal width
            let width = terminal_width().unwrap_or(80);
            for line in response.lines() {
                for part in wrap_utf8(line, width) {
                    println!("  {}", part);
                }
            }
            println!();
        }
        Err(e) => {
            eprintln!("  {} {}", err("✗ AI error:"), faint(&e));
        }
    }

    true
}

fn print_ai_help(config: &AiConfig) {
    println!();
    println!("{}", crate::fmt::head("Route — AI Assistant"));
    println!();
    println!(
        "  {}",
        dim("Ask questions about your project. The AI has full context:")
    );
    println!("  {}  {}", dim("•"), faint("project structure"));
    println!(
        "  {}  {}",
        dim("•"),
        faint("current branch and recent commits")
    );
    println!("  {}  {}", dim("•"), faint("references (project memory)"));
    println!();
    println!("  {}", dim("Usage:"));
    println!(
        "  {}  {}",
        accent("ai <question>"),
        dim("Ask a question about the project")
    );
    println!("  {}  {}", accent("ai help"), dim("Show this help"));
    println!(
        "  {}  {}",
        accent("ai config"),
        dim("Show current AI configuration")
    );
    println!();
    println!("  {}", dim("Examples:"));
    println!("  {}", faint("  ai what is the current branch status?"));
    println!("  {}", faint("  ai suggest improvements for this project"));
    println!("  {}", faint("  ai explain the recent commits"));
    println!();
    println!("  {}", dim("Current configuration:"));
    config.print();
    println!();
}

/// Get the terminal width for output formatting.
fn terminal_width() -> Option<usize> {
    #[cfg(windows)]
    {
        use std::mem::zeroed;
        use std::os::raw::c_void;
        extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
            fn GetConsoleScreenBufferInfo(
                hConsoleOutput: *mut c_void,
                lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
            ) -> i32;
        }
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
        const INVALID_HANDLE_VALUE: isize = -1;

        #[allow(non_snake_case)]
        #[repr(C)]
        struct CONSOLE_SCREEN_BUFFER_INFO {
            dwSize: COORD,
            dwCursorPosition: COORD,
            wAttributes: u16,
            srWindow: SMALL_RECT,
            dwMaximumWindowSize: COORD,
        }
        #[allow(non_snake_case)]
        #[repr(C)]
        struct COORD {
            X: i16,
            Y: i16,
        }
        #[allow(non_snake_case)]
        #[repr(C)]
        struct SMALL_RECT {
            Left: i16,
            Top: i16,
            Right: i16,
            Bottom: i16,
        }

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle as isize != INVALID_HANDLE_VALUE {
                let mut info: CONSOLE_SCREEN_BUFFER_INFO = zeroed();
                if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
                    return Some((info.srWindow.Right - info.srWindow.Left + 1) as usize);
                }
            }
        }
        None
    }
    #[cfg(unix)]
    {
        None
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}
