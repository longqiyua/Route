//! Terminal UI adaptation — detects the terminal environment and adapts
//! output formatting for different terminal emulators and capabilities.
//!
//! ## What this module does
//!
//! 1. **Terminal Detection** — identifies the terminal emulator (Windows
//!    Terminal, iTerm2, Kitty, Alacritty, WezTerm, etc.) via environment
//!    variables.
//! 2. **Color Support** — detects truecolor (24-bit), 256-color, 16-color,
//!    or no-color support.
//! 3. **Unicode/Emoji Support** — detects whether the terminal can render
//!    Unicode box-drawing characters and emoji.
//! 4. **Output Formatting** — provides helpers to format output based on
//!    the detected capabilities (colors, styles, box-drawing, progress bars).
//!
//! ## Usage
//!
//! ```rust,ignore
//! use tui_adapt::{TerminalInfo, detect_terminal};
//! let info = detect_terminal();
//! if info.supports_truecolor() {
//!     println!("{}", info.style("Hello, Route!").green().bold());
//! }
//! ```

use std::env;

// ---------------------------------------------------------------------------
// Terminal emulator detection
// ---------------------------------------------------------------------------

/// Known terminal emulators with their capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalEmulator {
    /// Windows Terminal (modern, good support)
    WindowsTerminal,
    /// Windows Console Host (legacy cmd.exe / PowerShell 5)
    WindowsConsole,
    /// iTerm2 (macOS, excellent support)
    ITerm2,
    /// Kitty (cross-platform, excellent support)
    Kitty,
    /// Alacritty (cross-platform, good support)
    Alacritty,
    /// WezTerm (cross-platform, good support)
    WezTerm,
    /// GNOME Terminal / VTE-based
    GnomeTerminal,
    /// macOS Terminal.app
    AppleTerminal,
    /// VS Code integrated terminal
    VSCodeTerminal,
    /// JetBrains IDE terminal
    JetBrainsTerminal,
    /// tmux (multiplexer, depends on parent terminal)
    Tmux,
    /// Trae IDE integrated terminal
    TraeTerminal,
    /// Codex CLI terminal
    CodexTerminal,
    /// Claude Code terminal
    ClaudeCodeTerminal,
    /// GitHub Codespaces terminal
    CodespacesTerminal,
    /// Warp terminal (modern, macOS with good GPU rendering)
    WarpTerminal,
    /// Hyper terminal (Electron-based)
    HyperTerminal,
    /// Tabby terminal (modern, cross-platform)
    TabbyTerminal,
    /// Rio terminal (hardware-accelerated)
    RioTerminal,
    /// Unknown terminal
    Unknown,
}

impl TerminalEmulator {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            TerminalEmulator::WindowsTerminal => "Windows Terminal",
            TerminalEmulator::WindowsConsole => "Windows Console Host",
            TerminalEmulator::ITerm2 => "iTerm2",
            TerminalEmulator::Kitty => "Kitty",
            TerminalEmulator::Alacritty => "Alacritty",
            TerminalEmulator::WezTerm => "WezTerm",
            TerminalEmulator::GnomeTerminal => "GNOME Terminal",
            TerminalEmulator::AppleTerminal => "Apple Terminal",
            TerminalEmulator::VSCodeTerminal => "VS Code Terminal",
            TerminalEmulator::JetBrainsTerminal => "JetBrains Terminal",
            TerminalEmulator::Tmux => "tmux",
            TerminalEmulator::TraeTerminal => "Trae IDE Terminal",
            TerminalEmulator::CodexTerminal => "Codex CLI Terminal",
            TerminalEmulator::ClaudeCodeTerminal => "Claude Code Terminal",
            TerminalEmulator::CodespacesTerminal => "GitHub Codespaces",
            TerminalEmulator::WarpTerminal => "Warp",
            TerminalEmulator::HyperTerminal => "Hyper",
            TerminalEmulator::TabbyTerminal => "Tabby",
            TerminalEmulator::RioTerminal => "Rio",
            TerminalEmulator::Unknown => "Unknown Terminal",
        }
    }

    /// Whether this terminal supports truecolor (24-bit RGB).
    pub fn has_truecolor(&self) -> bool {
        matches!(
            self,
            TerminalEmulator::WindowsTerminal
                | TerminalEmulator::ITerm2
                | TerminalEmulator::Kitty
                | TerminalEmulator::Alacritty
                | TerminalEmulator::WezTerm
                | TerminalEmulator::VSCodeTerminal
                | TerminalEmulator::JetBrainsTerminal
                | TerminalEmulator::TraeTerminal
                | TerminalEmulator::CodexTerminal
                | TerminalEmulator::ClaudeCodeTerminal
                | TerminalEmulator::CodespacesTerminal
                | TerminalEmulator::WarpTerminal
                | TerminalEmulator::HyperTerminal
                | TerminalEmulator::TabbyTerminal
                | TerminalEmulator::RioTerminal
        )
    }

    /// Whether this terminal supports Unicode box-drawing characters.
    pub fn has_unicode(&self) -> bool {
        !matches!(
            self,
            TerminalEmulator::WindowsConsole
        )
    }

    /// Whether this terminal supports emoji rendering.
    pub fn has_emoji(&self) -> bool {
        matches!(
            self,
            TerminalEmulator::WindowsTerminal
                | TerminalEmulator::ITerm2
                | TerminalEmulator::Kitty
                | TerminalEmulator::Alacritty
                | TerminalEmulator::WezTerm
                | TerminalEmulator::VSCodeTerminal
                | TerminalEmulator::JetBrainsTerminal
                | TerminalEmulator::GnomeTerminal
                | TerminalEmulator::AppleTerminal
                | TerminalEmulator::TraeTerminal
                | TerminalEmulator::CodexTerminal
                | TerminalEmulator::ClaudeCodeTerminal
                | TerminalEmulator::CodespacesTerminal
                | TerminalEmulator::WarpTerminal
                | TerminalEmulator::HyperTerminal
                | TerminalEmulator::TabbyTerminal
                | TerminalEmulator::RioTerminal
        )
    }
}

/// Color support level detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorLevel {
    /// No color support (--no-color, dumb terminals)
    None,
    /// ANSI 16 colors (basic)
    Ansi16,
    /// ANSI 256 colors (extended)
    Ansi256,
    /// Truecolor (24-bit RGB, 16.7M colors)
    TrueColor,
}

impl ColorLevel {
    pub fn supports_ansi(&self) -> bool {
        *self >= ColorLevel::Ansi16
    }

    pub fn supports_256(&self) -> bool {
        *self >= ColorLevel::Ansi256
    }

    pub fn supports_truecolor(&self) -> bool {
        *self >= ColorLevel::TrueColor
    }
}

// ---------------------------------------------------------------------------
// Terminal information
// ---------------------------------------------------------------------------

/// Comprehensive terminal capability information.
#[derive(Debug, Clone)]
pub struct TerminalInfo {
    /// Detected terminal emulator.
    pub emulator: TerminalEmulator,
    /// Color support level.
    pub color_level: ColorLevel,
    /// Whether the terminal is interactive (has a TTY).
    pub is_interactive: bool,
    /// Terminal width in columns (0 if unknown).
    pub width: u16,
    /// Terminal height in rows (0 if unknown).
    pub height: u16,
    /// Whether NO_COLOR env var is set.
    pub no_color: bool,
    /// Whether FORCE_COLOR env var is set.
    pub force_color: bool,
    /// The TERM env var value.
    pub term: String,
    /// The TERM_PROGRAM env var value.
    pub term_program: String,
}

impl TerminalInfo {
    /// Create a default (unknown) terminal info.
    pub fn unknown() -> Self {
        Self {
            emulator: TerminalEmulator::Unknown,
            color_level: ColorLevel::None,
            is_interactive: false,
            width: 0,
            height: 0,
            no_color: false,
            force_color: false,
            term: String::new(),
            term_program: String::new(),
        }
    }

    /// Whether truecolor is available.
    pub fn supports_truecolor(&self) -> bool {
        if self.no_color { return false; }
        if self.force_color { return true; }
        self.color_level.supports_truecolor() || self.emulator.has_truecolor()
    }

    /// Whether ANSI colors are available.
    pub fn supports_color(&self) -> bool {
        if self.no_color { return false; }
        if self.force_color { return true; }
        self.color_level.supports_ansi()
    }

    /// Whether Unicode box-drawing is supported.
    pub fn supports_unicode(&self) -> bool {
        self.emulator.has_unicode()
    }

    /// Whether emoji rendering is supported.
    pub fn supports_emoji(&self) -> bool {
        self.emulator.has_emoji()
    }

    /// Get a safe box-drawing character for this terminal.
    pub fn box_char(&self, kind: BoxChar) -> &'static str {
        if self.supports_unicode() {
            kind.unicode()
        } else {
            kind.ascii()
        }
    }

    /// Create a styled text builder for this terminal.
    pub fn style<'a>(&self, text: &'a str) -> StyledText<'a> {
        StyledText {
            text,
            color: self.supports_color(),
            styles: Vec::new(),
        }
    }
}

/// Box-drawing character types.
#[derive(Debug, Clone, Copy)]
pub enum BoxChar {
    Horizontal,
    Vertical,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Cross,
    TeeRight,
    TeeLeft,
    TeeDown,
    TeeUp,
}

impl BoxChar {
    fn unicode(&self) -> &'static str {
        match self {
            BoxChar::Horizontal => "─",
            BoxChar::Vertical => "│",
            BoxChar::TopLeft => "┌",
            BoxChar::TopRight => "┐",
            BoxChar::BottomLeft => "└",
            BoxChar::BottomRight => "┘",
            BoxChar::Cross => "┼",
            BoxChar::TeeRight => "├",
            BoxChar::TeeLeft => "┤",
            BoxChar::TeeDown => "┬",
            BoxChar::TeeUp => "┴",
        }
    }

    fn ascii(&self) -> &'static str {
        match self {
            BoxChar::Horizontal => "-",
            BoxChar::Vertical => "|",
            BoxChar::TopLeft => "+",
            BoxChar::TopRight => "+",
            BoxChar::BottomLeft => "+",
            BoxChar::BottomRight => "+",
            BoxChar::Cross => "+",
            BoxChar::TeeRight => "+",
            BoxChar::TeeLeft => "+",
            BoxChar::TeeDown => "+",
            BoxChar::TeeUp => "+",
        }
    }
}

// ---------------------------------------------------------------------------
// Styled text builder
// ---------------------------------------------------------------------------

/// Builder for styled terminal text.
pub struct StyledText<'a> {
    text: &'a str,
    color: bool,
    styles: Vec<&'static str>,
}

impl<'a> StyledText<'a> {
    pub fn bold(mut self) -> Self { self.styles.push("1"); self }
    pub fn dim(mut self) -> Self { self.styles.push("2"); self }
    pub fn italic(mut self) -> Self { self.styles.push("3"); self }
    pub fn underline(mut self) -> Self { self.styles.push("4"); self }

    pub fn red(mut self) -> Self { self.styles.push("31"); self }
    pub fn green(mut self) -> Self { self.styles.push("32"); self }
    pub fn yellow(mut self) -> Self { self.styles.push("33"); self }
    pub fn blue(mut self) -> Self { self.styles.push("34"); self }
    pub fn magenta(mut self) -> Self { self.styles.push("35"); self }
    pub fn cyan(mut self) -> Self { self.styles.push("36"); self }
    pub fn white(mut self) -> Self { self.styles.push("37"); self }
    pub fn gray(mut self) -> Self { self.styles.push("90"); self }

    pub fn truecolor(mut self, _r: u8, _g: u8, _b: u8) -> Self {
        self.styles.push("38;2");
        // We'll handle the RGB values in the build method
        self
    }

    /// Build the styled string with ANSI escape codes.
    pub fn build(&self) -> String {
        if !self.color || self.styles.is_empty() {
            return self.text.to_string();
        }
        let codes = self.styles.join(";");
        format!("\x1b[{}m{}\x1b[0m", codes, self.text)
    }
}

impl<'a> std::fmt::Display for StyledText<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.build())
    }
}

// ---------------------------------------------------------------------------
// Detection logic
// ---------------------------------------------------------------------------

/// Detect the terminal emulator from environment variables.
pub fn detect_emulator() -> TerminalEmulator {
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
    let term = env::var("TERM").unwrap_or_default();
    let wt_session = env::var("WT_SESSION").unwrap_or_default();
    let kitty_instance = env::var("KITTY_WINDOW_ID").unwrap_or_default();
    let alacritty = env::var("ALACRITTY_LOG").unwrap_or_default();
    let wezterm = env::var("WEZTERM_EXECUTABLE").unwrap_or_default();
    let tmux = env::var("TMUX").unwrap_or_default();
    let vscode = env::var("VSCODE_PID").unwrap_or_default();
    let jetbrains = env::var("TERMINAL_EMULATOR").unwrap_or_default();
    let trae = env::var("TRAE_PID").unwrap_or_default();
    let codex = env::var("CODEX_SESSION").unwrap_or_default();
    let claude_code = env::var("CLAUDE_CODE_SESSION").unwrap_or_default();
    let codespaces = env::var("CODESPACES").unwrap_or_default();
    let warp = env::var("WARP_TERM_SESSION_ID").unwrap_or_default();
    let hyper = env::var("HYPER_PID").unwrap_or_default();
    let tabby = env::var("TABBY_CONFIG_DIRECTORY").unwrap_or_default();
    let rio = env::var("RIO_CONFIG").unwrap_or_default();

    if !wt_session.is_empty() {
        TerminalEmulator::WindowsTerminal
    } else if !kitty_instance.is_empty() {
        TerminalEmulator::Kitty
    } else if !alacritty.is_empty() {
        TerminalEmulator::Alacritty
    } else if !wezterm.is_empty() {
        TerminalEmulator::WezTerm
    } else if !tmux.is_empty() {
        TerminalEmulator::Tmux
    } else if !trae.is_empty() {
        TerminalEmulator::TraeTerminal
    } else if !codex.is_empty() {
        TerminalEmulator::CodexTerminal
    } else if !claude_code.is_empty() {
        TerminalEmulator::ClaudeCodeTerminal
    } else if !codespaces.is_empty() {
        TerminalEmulator::CodespacesTerminal
    } else if !warp.is_empty() {
        TerminalEmulator::WarpTerminal
    } else if !hyper.is_empty() {
        TerminalEmulator::HyperTerminal
    } else if !tabby.is_empty() {
        TerminalEmulator::TabbyTerminal
    } else if !rio.is_empty() {
        TerminalEmulator::RioTerminal
    } else if !vscode.is_empty() {
        TerminalEmulator::VSCodeTerminal
    } else if jetbrains.contains("JetBrains") {
        TerminalEmulator::JetBrainsTerminal
    } else if term_program == "iTerm.app" {
        TerminalEmulator::ITerm2
    } else if term_program == "Apple_Terminal" {
        TerminalEmulator::AppleTerminal
    } else if term.contains("gnome") || term.contains("xterm-256color") {
        TerminalEmulator::GnomeTerminal
    } else if cfg!(windows) {
        // On Windows, if we're not in Windows Terminal, we're likely in
        // the legacy console host.
        TerminalEmulator::WindowsConsole
    } else {
        TerminalEmulator::Unknown
    }
}

/// Detect the color support level from environment variables.
pub fn detect_color_level(emulator: TerminalEmulator) -> ColorLevel {
    let colorterm = env::var("COLORTERM").unwrap_or_default();

    if colorterm == "truecolor" || colorterm == "24bit" {
        return ColorLevel::TrueColor;
    }

    if emulator.has_truecolor() {
        // Check if TERM supports 256 colors
        let term = env::var("TERM").unwrap_or_default();
        if term.contains("256color") {
            return ColorLevel::Ansi256;
        }
        return ColorLevel::TrueColor;
    }

    let term = env::var("TERM").unwrap_or_default();
    if term.contains("256color") {
        ColorLevel::Ansi256
    } else if term.contains("color") || term.contains("ansi") {
        ColorLevel::Ansi16
    } else {
        ColorLevel::None
    }
}

/// Detect terminal dimensions from environment or OS APIs.
pub fn detect_dimensions() -> (u16, u16) {
    // Try term_size crate or environment variables
    if let Ok(cols) = env::var("COLUMNS") {
        if let Ok(rows) = env::var("LINES") {
            if let (Ok(w), Ok(h)) = (cols.parse::<u16>(), rows.parse::<u16>()) {
                return (w, h);
            }
        }
    }

    // Fallback: try OS-specific terminal size
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
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11
        const INVALID_HANDLE_VALUE: isize = -1;

        #[repr(C)]
        #[allow(non_snake_case)]
        struct CONSOLE_SCREEN_BUFFER_INFO {
            dwSize: COORD,
            dwCursorPosition: COORD,
            wAttributes: u16,
            srWindow: SMALL_RECT,
            dwMaximumWindowSize: COORD,
        }
        #[repr(C)]
        struct COORD { X: i16, Y: i16 }
        #[repr(C)]
        struct SMALL_RECT { Left: i16, Top: i16, Right: i16, Bottom: i16 }

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle as isize != INVALID_HANDLE_VALUE {
                let mut info: CONSOLE_SCREEN_BUFFER_INFO = zeroed();
                if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
                    let w = (info.srWindow.Right - info.srWindow.Left + 1) as u16;
                    let h = (info.srWindow.Bottom - info.srWindow.Top + 1) as u16;
                    return (w, h);
                }
            }
        }
    }

    #[cfg(unix)]
    {
        // Try libc::ioctl with TIOCGWINSZ
        // This requires the libc crate. For now, fallback to defaults.
    }

    (80, 24) // Default fallback
}

/// Detect whether the terminal is interactive (has a TTY).
pub fn detect_interactive() -> bool {
    #[cfg(windows)]
    {
        use std::os::raw::c_void;
        extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> *mut c_void;
            fn GetConsoleMode(hConsoleHandle: *mut c_void, lpMode: *mut u32) -> i32;
        }
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // -11
        const INVALID_HANDLE_VALUE: isize = -1;

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle as isize == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut mode: u32 = 0;
            GetConsoleMode(handle, &mut mode) != 0
        }
    }

    #[cfg(unix)]
    {
        unsafe { libc::isatty(1) != 0 }
    }

    #[cfg(not(any(windows, unix)))]
    {
        false
    }
}

/// Full terminal detection — returns a complete `TerminalInfo`.
pub fn detect_terminal() -> TerminalInfo {
    let no_color = env::var("NO_COLOR").is_ok();
    let force_color = env::var("FORCE_COLOR").is_ok();
    let emulator = detect_emulator();
    let color_level = if no_color {
        ColorLevel::None
    } else if force_color {
        ColorLevel::TrueColor
    } else {
        detect_color_level(emulator)
    };
    let is_interactive = detect_interactive();
    let (width, height) = detect_dimensions();
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    TerminalInfo {
        emulator,
        color_level,
        is_interactive,
        width,
        height,
        no_color,
        force_color,
        term,
        term_program,
    }
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

/// Print a horizontal separator line adapted to the terminal.
pub fn print_separator(info: &TerminalInfo) {
    let ch = info.box_char(BoxChar::Horizontal);
    let width = if info.width > 0 { info.width as usize } else { 80 };
    let line: String = std::iter::repeat(ch).take(width).collect();
    if info.supports_color() {
        println!("\x1b[90m{}\x1b[0m", line);
    } else {
        println!("{}", line);
    }
}

/// Print a section header adapted to the terminal.
pub fn print_header(info: &TerminalInfo, title: &str) {
    let ch = info.box_char(BoxChar::Horizontal);
    let sep: String = std::iter::repeat(ch).take(3).collect();
    if info.supports_color() {
        println!("\x1b[1;36m{}{}{} {}\x1b[0m", sep, ch, ch, title);
    } else {
        println!("--- {}", title);
    }
}

/// Print a status line with an icon (adapts emoji based on terminal).
pub fn print_status(info: &TerminalInfo, icon: &str, message: &str, success: bool) {
    let icon_text = if info.supports_emoji() {
        icon.to_string()
    } else {
        match icon {
            "✅" => "[OK]".to_string(),
            "❌" => "[FAIL]".to_string(),
            "⚠️" => "[WARN]".to_string(),
            "ℹ️" => "[INFO]".to_string(),
            "🔧" => "[FIX]".to_string(),
            "🚀" => "[RUN]".to_string(),
            _ => format!("[{}]", icon),
        }
    };

    if info.supports_color() {
        let color = if success { "32" } else { "31" };
        println!("  {} \x1b[{}m{}\x1b[0m", icon_text, color, message);
    } else {
        println!("  {} {}", icon_text, message);
    }
}

/// Print a bullet list item.
pub fn print_bullet(info: &TerminalInfo, text: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let bullet = if info.supports_unicode() { "•" } else { "*" };
    if info.supports_color() {
        println!("{}\x1b[90m{}\x1b[0m {}", indent, bullet, text);
    } else {
        println!("{}{} {}", indent, bullet, text);
    }
}

/// Print a simple progress bar.
/// `current` and `total` are the progress values,
/// `width` is the desired bar width in characters.
pub fn print_progress_bar(info: &TerminalInfo, current: usize, total: usize, width: usize) {
    if total == 0 { return; }
    let ratio = (current as f64 / total as f64).clamp(0.0, 1.0);
    let filled = (ratio * width as f64) as usize;
    let empty = width.saturating_sub(filled);

    let fill_char = if info.supports_unicode() { "█" } else { "#" };
    let empty_char = if info.supports_unicode() { "░" } else { "-" };
    let bar: String = std::iter::repeat(fill_char).take(filled)
        .chain(std::iter::repeat(empty_char).take(empty))
        .collect();

    let pct = (ratio * 100.0) as usize;
    if info.supports_color() {
        let color = if ratio < 0.5 { "33" } else if ratio < 0.9 { "36" } else { "32" };
        print!("\r  \x1b[{color}m[{bar}]\x1b[0m {pct}% ({current}/{total})");
    } else {
        print!("\r  [{bar}] {pct}% ({current}/{total})");
    }
}

/// Print a simple table with headers and rows.
/// Each row should have the same number of columns as `headers`.
pub fn print_table(
    info: &TerminalInfo,
    headers: &[&str],
    rows: &[Vec<String>],
    col_widths: &[usize],
) {
    let sep = info.box_char(BoxChar::Horizontal);
    let vert = info.box_char(BoxChar::Vertical);
    let tee_down = info.box_char(BoxChar::TeeDown);
    let tee_up = info.box_char(BoxChar::TeeUp);
    let tee_right = info.box_char(BoxChar::TeeRight);
    let tee_left = info.box_char(BoxChar::TeeLeft);
    let cross = info.box_char(BoxChar::Cross);
    let top_left = info.box_char(BoxChar::TopLeft);
    let top_right = info.box_char(BoxChar::TopRight);
    let bottom_left = info.box_char(BoxChar::BottomLeft);
    let bottom_right = info.box_char(BoxChar::BottomRight);

    // Build separator lines.
    let top_sep = format!(
        "{top_left}{}{top_right}",
        col_widths.iter().enumerate().map(|(i, &w)| {
            let seg: String = std::iter::repeat(sep).take(w + 2).collect();
            if i == 0 { seg } else { format!("{tee_down}{seg}") }
        }).collect::<Vec<_>>().join("")
    );
    let mid_sep = format!(
        "{tee_right}{}{tee_left}",
        col_widths.iter().enumerate().map(|(i, &w)| {
            let seg: String = std::iter::repeat(sep).take(w + 2).collect();
            if i == 0 { seg } else { format!("{cross}{seg}") }
        }).collect::<Vec<_>>().join("")
    );
    let bot_sep = format!(
        "{bottom_left}{}{bottom_right}",
        col_widths.iter().enumerate().map(|(i, &w)| {
            let seg: String = std::iter::repeat(sep).take(w + 2).collect();
            if i == 0 { seg } else { format!("{tee_up}{seg}") }
        }).collect::<Vec<_>>().join("")
    );

    // Print top border.
    if info.supports_color() {
        println!("\x1b[90m{top_sep}\x1b[0m");
    } else {
        println!("{top_sep}");
    }

    // Print header row.
    let header_str: String = headers.iter().enumerate().map(|(i, h)| {
        let padded = if info.supports_color() {
            format!("\x1b[1m{:width$}\x1b[0m", h, width = col_widths[i])
        } else {
            format!("{:width$}", h, width = col_widths[i])
        };
        if i == 0 {
            format!("{vert} {padded} ")
        } else {
            format!("{vert} {padded} ")
        }
    }).collect::<Vec<_>>().join("") + &vert;
    println!("{header_str}");

    // Print separator.
    if info.supports_color() {
        println!("\x1b[90m{mid_sep}\x1b[0m");
    } else {
        println!("{mid_sep}");
    }

    // Print data rows.
    for row in rows {
        let row_str: String = row.iter().enumerate().map(|(i, cell)| {
            let width = col_widths.get(i).copied().unwrap_or(10);
            let truncated: String = cell.chars().take(width).collect();
            let padded = format!("{:width$}", truncated, width = width);
            if i == 0 {
                format!("{vert} {padded} ")
            } else {
                format!("{vert} {padded} ")
            }
        }).collect::<Vec<_>>().join("") + &vert;
        println!("{row_str}");
    }

    // Print bottom border.
    if info.supports_color() {
        println!("\x1b[90m{bot_sep}\x1b[0m");
    } else {
        println!("{bot_sep}");
    }
}

/// Print a diff line with appropriate coloring.
/// `line_type` is one of: "added", "removed", "context", "header".
pub fn print_diff_line(info: &TerminalInfo, line_type: &str, text: &str) {
    if info.supports_color() {
        match line_type {
            "added" => println!("\x1b[32m+ {}\x1b[0m", text),
            "removed" => println!("\x1b[31m- {}\x1b[0m", text),
            "header" => println!("\x1b[1;36m{}\x1b[0m", text),
            "hunk" => println!("\x1b[34m{}\x1b[0m", text),
            _ => println!("  {}", text),
        }
    } else {
        match line_type {
            "added" => println!("+ {}", text),
            "removed" => println!("- {}", text),
            _ => println!("  {}", text),
        }
    }
}

/// Format a commit graph line with branch topology.
/// Uses Unicode box-drawing if supported, falls back to ASCII.
pub fn format_commit_graph(
    info: &TerminalInfo,
    sha: &str,
    message: &str,
    author: &str,
    date: &str,
    graph_line: &str,
) -> String {
    let short_sha = &sha[..sha.len().min(7)];
    if info.supports_color() {
        format!(
            "\x1b[33m{graph_line}\x1b[0m \x1b[1;33m{short_sha}\x1b[0m \x1b[32m{message}\x1b[0m \x1b[90m({author}, {date})\x1b[0m",
        )
    } else {
        format!("{graph_line} {short_sha} {message} ({author}, {date})")
    }
}

/// Format a status badge (e.g., "CLEAN", "DIRTY", "OK", "FAIL").
pub fn format_badge(info: &TerminalInfo, label: &str, status: &str) -> String {
    let (color, icon) = match status.to_lowercase().as_str() {
        "clean" | "ok" | "pass" | "success" => ("32", if info.supports_emoji() { "✅" } else { "[OK]" }),
        "dirty" | "modified" | "warn" => ("33", if info.supports_emoji() { "⚠️" } else { "[WARN]" }),
        "fail" | "error" => ("31", if info.supports_emoji() { "❌" } else { "[FAIL]" }),
        _ => ("37", ""),
    };
    if info.supports_color() {
        format!("\x1b[{color}m{icon} {label}\x1b[0m")
    } else {
        format!("{icon} {label}")
    }
}

/// Format a key-value pair for display.
pub fn format_kv(info: &TerminalInfo, key: &str, value: &str) -> String {
    if info.supports_color() {
        format!("\x1b[90m{key}:\x1b[0m {value}")
    } else {
        format!("{key}: {value}")
    }
}

/// Format a numbered list item.
pub fn format_list_item(info: &TerminalInfo, num: usize, text: &str) -> String {
    if info.supports_color() {
        format!("\x1b[90m{num:>3}.\x1b[0m {text}")
    } else {
        format!("{num:>3}. {text}")
    }
}

/// Print a banner / title bar with a centered title.
pub fn print_banner(info: &TerminalInfo, title: &str) {
    let width = if info.width > 0 { info.width as usize } else { 80 };
    let title_len = title.chars().count();
    let side = width.saturating_sub(title_len + 2) / 2;
    let ch = info.box_char(BoxChar::Horizontal);
    let left: String = std::iter::repeat(ch).take(side).collect();
    let right: String = std::iter::repeat(ch).take(side).collect();
    if info.supports_color() {
        println!("\x1b[1;37;44m{left} {title} {right}\x1b[0m");
    } else {
        println!("{left} {title} {right}");
    }
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// Return terminal capability information to the frontend.
/// The frontend can use this to decide whether to use emoji, ANSI colors,
/// or Unicode box-drawing in the embedded terminal view.
#[tauri::command]
pub fn tui_detect_terminal() -> serde_json::Value {
    let info = detect_terminal();
    serde_json::json!({
        "emulator": info.emulator.name(),
        "color_level": format!("{:?}", info.color_level),
        "supports_color": info.supports_color(),
        "supports_truecolor": info.supports_truecolor(),
        "supports_unicode": info.supports_unicode(),
        "supports_emoji": info.supports_emoji(),
        "is_interactive": info.is_interactive,
        "width": info.width,
        "height": info.height,
        "no_color": info.no_color,
        "force_color": info.force_color,
        "term": info.term,
        "term_program": info.term_program,
    })
}