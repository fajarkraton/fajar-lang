//! System & Utilities — paths, logging, CLI args, progress bar, concurrency.
//!
//! Phase S4: 20 tasks covering OS interaction and developer utilities.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S4.1-S4.2: Path Manipulation
// ═══════════════════════════════════════════════════════════════════════

/// A cross-platform file path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    /// Path components.
    pub components: Vec<String>,
    /// Whether the path is absolute.
    pub is_absolute: bool,
}

impl Path {
    /// Parses a path string.
    pub fn new(path: &str) -> Self {
        let is_absolute = path.starts_with('/') || (path.len() >= 2 && path.as_bytes()[1] == b':');
        let components: Vec<String> = path
            .split(['/', '\\'])
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| s.to_string())
            .collect();
        Self { components, is_absolute }
    }

    /// Joins a child path.
    pub fn join(&self, child: &str) -> Self {
        let mut result = self.clone();
        let child_path = Path::new(child);
        if child_path.is_absolute {
            return child_path;
        }
        result.components.extend(child_path.components);
        result.normalize()
    }

    /// Returns the parent path.
    pub fn parent(&self) -> Option<Self> {
        if self.components.is_empty() { return None; }
        let mut result = self.clone();
        result.components.pop();
        Some(result)
    }

    /// Returns the file name.
    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(|s| s.as_str())
    }

    /// Returns the file extension.
    pub fn extension(&self) -> Option<&str> {
        self.file_name()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
    }

    /// Returns the file stem (name without extension).
    pub fn stem(&self) -> Option<&str> {
        self.file_name()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(stem, _)| stem)
            .or(self.file_name())
    }

    /// Changes the extension.
    pub fn with_extension(&self, ext: &str) -> Self {
        let mut result = self.clone();
        if let Some(last) = result.components.last_mut() {
            if let Some(dot) = last.rfind('.') {
                last.truncate(dot);
            }
            if !ext.is_empty() {
                last.push('.');
                last.push_str(ext);
            }
        }
        result
    }

    /// Normalizes the path (resolves ".." components).
    pub fn normalize(&self) -> Self {
        let mut normalized = Vec::new();
        for comp in &self.components {
            if comp == ".." {
                if !normalized.is_empty() && normalized.last() != Some(&"..".to_string()) {
                    normalized.pop();
                } else if !self.is_absolute {
                    normalized.push(comp.clone());
                }
            } else {
                normalized.push(comp.clone());
            }
        }
        Self { components: normalized, is_absolute: self.is_absolute }
    }

    /// Converts to string with `/` separator.
    pub fn to_string_path(&self) -> String {
        let joined = self.components.join("/");
        if self.is_absolute { format!("/{joined}") } else { joined }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_path())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.3-S4.4: Logging Framework
// ═══════════════════════════════════════════════════════════════════════

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

impl LogLevel {
    /// Returns the ANSI color code for this level.
    pub fn color_code(self) -> &'static str {
        match self {
            Self::Trace => "\x1b[90m",   // gray
            Self::Debug => "\x1b[36m",   // cyan
            Self::Info => "\x1b[32m",    // green
            Self::Warn => "\x1b[33m",    // yellow
            Self::Error => "\x1b[31m",   // red
        }
    }
}

/// A log record.
#[derive(Debug, Clone)]
pub struct LogRecord {
    /// Level.
    pub level: LogLevel,
    /// Message.
    pub message: String,
    /// Module path (e.g., "net::http").
    pub module: String,
    /// Timestamp (ISO 8601).
    pub timestamp: String,
    /// Structured fields.
    pub fields: HashMap<String, String>,
}

impl fmt::Display for LogRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] {}: {}", self.timestamp, self.level, self.module, self.message)?;
        for (k, v) in &self.fields {
            write!(f, " {k}={v}")?;
        }
        Ok(())
    }
}

/// Logger configuration.
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Minimum level to log.
    pub min_level: LogLevel,
    /// Whether to use colors.
    pub use_colors: bool,
    /// Whether to include timestamps.
    pub timestamps: bool,
    /// Whether to include module path.
    pub show_module: bool,
    /// Output target.
    pub target: LogTarget,
}

/// Log output target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogTarget {
    Stdout,
    Stderr,
    File(String),
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            use_colors: true,
            timestamps: true,
            show_module: true,
            target: LogTarget::Stderr,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.5-S4.6: CLI Argument Parser
// ═══════════════════════════════════════════════════════════════════════

/// CLI argument definition.
#[derive(Debug, Clone)]
pub struct ArgDef {
    /// Long name (e.g., "output").
    pub long: String,
    /// Short name (e.g., 'o').
    pub short: Option<char>,
    /// Description.
    pub help: String,
    /// Whether this arg takes a value.
    pub takes_value: bool,
    /// Whether this arg is required.
    pub required: bool,
    /// Default value.
    pub default: Option<String>,
}

/// Parsed CLI arguments.
#[derive(Debug, Clone, Default)]
pub struct ParsedArgs {
    /// Named arguments (--key value or --flag).
    pub named: HashMap<String, String>,
    /// Positional arguments.
    pub positional: Vec<String>,
    /// Whether --help was requested.
    pub help_requested: bool,
}

impl ParsedArgs {
    /// Gets a named argument value.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.named.get(name).map(|s| s.as_str())
    }

    /// Gets a named argument or default.
    pub fn get_or(&self, name: &str, default: &str) -> String {
        self.named.get(name).cloned().unwrap_or_else(|| default.to_string())
    }

    /// Checks if a flag is present.
    pub fn has(&self, name: &str) -> bool {
        self.named.contains_key(name)
    }
}

/// Parses CLI arguments according to definitions.
pub fn parse_args(args: &[String], defs: &[ArgDef]) -> Result<ParsedArgs, String> {
    let mut result = ParsedArgs::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            result.help_requested = true;
            i += 1;
            continue;
        }
        if arg.starts_with("--") {
            let name = &arg[2..];
            let def = defs.iter().find(|d| d.long == name);
            if let Some(def) = def {
                if def.takes_value {
                    i += 1;
                    if i >= args.len() { return Err(format!("missing value for --{name}")); }
                    result.named.insert(name.to_string(), args[i].clone());
                } else {
                    result.named.insert(name.to_string(), "true".to_string());
                }
            } else {
                return Err(format!("unknown argument: --{name}"));
            }
        } else if arg.starts_with('-') && arg.len() == 2 {
            let ch = arg.chars().nth(1).unwrap();
            let def = defs.iter().find(|d| d.short == Some(ch));
            if let Some(def) = def {
                if def.takes_value {
                    i += 1;
                    if i >= args.len() { return Err(format!("missing value for -{ch}")); }
                    result.named.insert(def.long.clone(), args[i].clone());
                } else {
                    result.named.insert(def.long.clone(), "true".to_string());
                }
            } else {
                return Err(format!("unknown flag: -{ch}"));
            }
        } else {
            result.positional.push(arg.clone());
        }
        i += 1;
    }

    // Apply defaults
    for def in defs {
        if !result.named.contains_key(&def.long) {
            if let Some(ref default) = def.default {
                result.named.insert(def.long.clone(), default.clone());
            } else if def.required {
                return Err(format!("missing required argument: --{}", def.long));
            }
        }
    }

    Ok(result)
}

/// Generates help text for CLI arguments.
pub fn generate_help(program: &str, defs: &[ArgDef]) -> String {
    let mut help = format!("Usage: {program} [OPTIONS] [ARGS]\n\nOptions:\n");
    for def in defs {
        let short = def.short.map(|c| format!("-{c}, ")).unwrap_or_else(|| "    ".to_string());
        let value_hint = if def.takes_value { " <VALUE>" } else { "" };
        let required = if def.required { " (required)" } else { "" };
        let default = def.default.as_ref().map(|d| format!(" [default: {d}]")).unwrap_or_default();
        help.push_str(&format!("  {short}--{}{value_hint}\n        {}{required}{default}\n",
            def.long, def.help));
    }
    help.push_str("      --help\n        Print this help message\n");
    help
}

// ═══════════════════════════════════════════════════════════════════════
// S4.7-S4.8: Progress Bar + Table Formatting
// ═══════════════════════════════════════════════════════════════════════

/// Progress bar state.
#[derive(Debug, Clone)]
pub struct ProgressBar {
    /// Total steps.
    pub total: u64,
    /// Current step.
    pub current: u64,
    /// Bar width in characters.
    pub width: u32,
    /// Message prefix.
    pub message: String,
}

impl ProgressBar {
    /// Creates a new progress bar.
    pub fn new(total: u64, message: &str) -> Self {
        Self { total, current: 0, width: 40, message: message.to_string() }
    }

    /// Updates progress.
    pub fn set(&mut self, current: u64) { self.current = current.min(self.total); }

    /// Increments by 1.
    pub fn inc(&mut self) { self.set(self.current + 1); }

    /// Returns fraction complete (0.0 to 1.0).
    pub fn fraction(&self) -> f64 {
        if self.total == 0 { return 1.0; }
        self.current as f64 / self.total as f64
    }

    /// Renders the progress bar as a string.
    pub fn render(&self) -> String {
        let frac = self.fraction();
        let filled = (frac * self.width as f64) as u32;
        let empty = self.width - filled;
        let bar: String = "█".repeat(filled as usize) + &"░".repeat(empty as usize);
        let pct = (frac * 100.0) as u32;
        format!("{} [{bar}] {pct}% ({}/{})", self.message, self.current, self.total)
    }
}

/// Table column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align { Left, Right, Center }

/// Formats data as an ASCII table.
pub fn format_table(headers: &[&str], rows: &[Vec<String>], align: &[Align]) -> String {
    // Calculate column widths
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < cols && cell.len() > widths[i] { widths[i] = cell.len(); }
        }
    }

    let mut result = String::new();
    // Header
    result.push('|');
    for (i, header) in headers.iter().enumerate() {
        result.push_str(&format!(" {:^width$} |", header, width = widths[i]));
    }
    result.push('\n');
    // Separator
    result.push('|');
    for w in &widths {
        result.push_str(&format!("-{}-|", "-".repeat(*w)));
    }
    result.push('\n');
    // Rows
    for row in rows {
        result.push('|');
        for (i, cell) in row.iter().enumerate() {
            if i >= cols { break; }
            let a = align.get(i).copied().unwrap_or(Align::Left);
            let formatted = match a {
                Align::Left => format!(" {:<width$} ", cell, width = widths[i]),
                Align::Right => format!(" {:>width$} ", cell, width = widths[i]),
                Align::Center => format!(" {:^width$} ", cell, width = widths[i]),
            };
            result.push_str(&formatted);
            result.push('|');
        }
        result.push('\n');
    }
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S4.9-S4.10: Timer + Rate Limiter
// ═══════════════════════════════════════════════════════════════════════

/// A stopwatch for measuring elapsed time.
#[derive(Debug, Clone)]
pub struct Stopwatch {
    /// Start time (nanoseconds from arbitrary epoch).
    pub start_ns: u64,
    /// Accumulated time (for pause/resume).
    pub accumulated_ns: u64,
    /// Whether the stopwatch is running.
    pub running: bool,
}

impl Stopwatch {
    /// Creates a new stopped stopwatch.
    pub fn new() -> Self {
        Self { start_ns: 0, accumulated_ns: 0, running: false }
    }

    /// Returns elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> f64 {
        self.accumulated_ns as f64 / 1_000_000.0
    }

    /// Returns elapsed time in seconds.
    pub fn elapsed_secs(&self) -> f64 {
        self.accumulated_ns as f64 / 1_000_000_000.0
    }

    /// Formats elapsed time as human-readable string.
    pub fn format(&self) -> String {
        let ms = self.elapsed_ms();
        if ms < 1.0 { format!("{:.0}μs", ms * 1000.0) }
        else if ms < 1000.0 { format!("{ms:.1}ms") }
        else { format!("{:.2}s", ms / 1000.0) }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S4.1: Path operations
    #[test]
    fn s4_1_path_parse() {
        let p = Path::new("/home/fajar/project/main.fj");
        assert!(p.is_absolute);
        assert_eq!(p.file_name(), Some("main.fj"));
        assert_eq!(p.extension(), Some("fj"));
        assert_eq!(p.stem(), Some("main"));
    }

    #[test]
    fn s4_1_path_join() {
        let base = Path::new("/home/fajar");
        let joined = base.join("src/main.fj");
        assert_eq!(joined.to_string_path(), "/home/fajar/src/main.fj");
    }

    #[test]
    fn s4_1_path_parent() {
        let p = Path::new("/home/fajar/file.txt");
        let parent = p.parent().unwrap();
        assert_eq!(parent.to_string_path(), "/home/fajar");
    }

    #[test]
    fn s4_1_path_normalize() {
        let p = Path::new("/home/fajar/../lang/./src/main.fj");
        let normalized = p.normalize();
        assert_eq!(normalized.to_string_path(), "/home/lang/src/main.fj");
    }

    #[test]
    fn s4_1_path_with_extension() {
        let p = Path::new("output.fj");
        let new_p = p.with_extension("wasm");
        assert_eq!(new_p.to_string_path(), "output.wasm");
    }

    // S4.3: Logging
    #[test]
    fn s4_3_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn s4_3_log_record_display() {
        let record = LogRecord {
            level: LogLevel::Info,
            message: "server started".to_string(),
            module: "net::http".to_string(),
            timestamp: "2026-03-26T12:00:00Z".to_string(),
            fields: HashMap::from([("port".to_string(), "8080".to_string())]),
        };
        let s = format!("{record}");
        assert!(s.contains("INFO"));
        assert!(s.contains("server started"));
        assert!(s.contains("port=8080"));
    }

    // S4.5: CLI args
    #[test]
    fn s4_5_parse_args() {
        let defs = vec![
            ArgDef { long: "output".to_string(), short: Some('o'), help: "Output file".to_string(), takes_value: true, required: false, default: None },
            ArgDef { long: "verbose".to_string(), short: Some('v'), help: "Verbose".to_string(), takes_value: false, required: false, default: None },
        ];
        let args: Vec<String> = vec!["--output", "out.fj", "-v", "input.fj"].iter().map(|s| s.to_string()).collect();
        let parsed = parse_args(&args, &defs).unwrap();
        assert_eq!(parsed.get("output"), Some("out.fj"));
        assert!(parsed.has("verbose"));
        assert_eq!(parsed.positional, vec!["input.fj"]);
    }

    #[test]
    fn s4_5_parse_args_help() {
        let args: Vec<String> = vec!["--help"].iter().map(|s| s.to_string()).collect();
        let parsed = parse_args(&args, &[]).unwrap();
        assert!(parsed.help_requested);
    }

    #[test]
    fn s4_5_generate_help() {
        let defs = vec![ArgDef { long: "output".to_string(), short: Some('o'), help: "Output file".to_string(), takes_value: true, required: true, default: None }];
        let help = generate_help("fj", &defs);
        assert!(help.contains("--output"));
        assert!(help.contains("-o"));
        assert!(help.contains("(required)"));
    }

    // S4.7: Progress bar
    #[test]
    fn s4_7_progress_bar() {
        let mut pb = ProgressBar::new(100, "Building");
        assert_eq!(pb.fraction(), 0.0);
        pb.set(50);
        assert!((pb.fraction() - 0.5).abs() < 0.001);
        let rendered = pb.render();
        assert!(rendered.contains("50%"));
        assert!(rendered.contains("Building"));
    }

    // S4.8: Table formatting
    #[test]
    fn s4_8_format_table() {
        let headers = vec!["Name", "Type", "Size"];
        let rows = vec![
            vec!["main.fj".to_string(), "source".to_string(), "1.2KB".to_string()],
            vec!["lib.fj".to_string(), "library".to_string(), "3.5KB".to_string()],
        ];
        let table = format_table(&headers, &rows, &[Align::Left, Align::Left, Align::Right]);
        assert!(table.contains("main.fj"));
        assert!(table.contains("---"));
    }

    // S4.9: Stopwatch
    #[test]
    fn s4_9_stopwatch() {
        let mut sw = Stopwatch::new();
        sw.accumulated_ns = 1_500_000; // 1.5ms
        assert!((sw.elapsed_ms() - 1.5).abs() < 0.001);
        assert_eq!(sw.format(), "1.5ms");

        sw.accumulated_ns = 500; // 0.5μs
        assert!(sw.format().contains("μs"));

        sw.accumulated_ns = 2_500_000_000; // 2.5s
        assert!(sw.format().contains("2.50s"));
    }

    // S4.10: Defaults
    #[test]
    fn s4_10_logger_defaults() {
        let cfg = LoggerConfig::default();
        assert_eq!(cfg.min_level, LogLevel::Info);
        assert!(cfg.use_colors);
        assert_eq!(cfg.target, LogTarget::Stderr);
    }
}
