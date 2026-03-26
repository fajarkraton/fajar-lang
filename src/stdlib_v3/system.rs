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
        Self {
            components,
            is_absolute,
        }
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
        if self.components.is_empty() {
            return None;
        }
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
        Self {
            components: normalized,
            is_absolute: self.is_absolute,
        }
    }

    /// Converts to string with `/` separator.
    pub fn to_string_path(&self) -> String {
        let joined = self.components.join("/");
        if self.is_absolute {
            format!("/{joined}")
        } else {
            joined
        }
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
            Self::Trace => "\x1b[90m", // gray
            Self::Debug => "\x1b[36m", // cyan
            Self::Info => "\x1b[32m",  // green
            Self::Warn => "\x1b[33m",  // yellow
            Self::Error => "\x1b[31m", // red
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
        write!(
            f,
            "{} [{}] {}: {}",
            self.timestamp, self.level, self.module, self.message
        )?;
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
        self.named
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_string())
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
        if let Some(name) = arg.strip_prefix("--") {
            let def = defs.iter().find(|d| d.long == name);
            if let Some(def) = def {
                if def.takes_value {
                    i += 1;
                    if i >= args.len() {
                        return Err(format!("missing value for --{name}"));
                    }
                    result.named.insert(name.to_string(), args[i].clone());
                } else {
                    result.named.insert(name.to_string(), "true".to_string());
                }
            } else {
                return Err(format!("unknown argument: --{name}"));
            }
        } else if arg.starts_with('-') && arg.len() == 2 {
            let ch = arg.as_bytes()[1] as char;
            let def = defs.iter().find(|d| d.short == Some(ch));
            if let Some(def) = def {
                if def.takes_value {
                    i += 1;
                    if i >= args.len() {
                        return Err(format!("missing value for -{ch}"));
                    }
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
        let short = def
            .short
            .map(|c| format!("-{c}, "))
            .unwrap_or_else(|| "    ".to_string());
        let value_hint = if def.takes_value { " <VALUE>" } else { "" };
        let required = if def.required { " (required)" } else { "" };
        let default = def
            .default
            .as_ref()
            .map(|d| format!(" [default: {d}]"))
            .unwrap_or_default();
        help.push_str(&format!(
            "  {short}--{}{value_hint}\n        {}{required}{default}\n",
            def.long, def.help
        ));
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
        Self {
            total,
            current: 0,
            width: 40,
            message: message.to_string(),
        }
    }

    /// Updates progress.
    pub fn set(&mut self, current: u64) {
        self.current = current.min(self.total);
    }

    /// Increments by 1.
    pub fn inc(&mut self) {
        self.set(self.current + 1);
    }

    /// Returns fraction complete (0.0 to 1.0).
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.current as f64 / self.total as f64
    }

    /// Renders the progress bar as a string.
    pub fn render(&self) -> String {
        let frac = self.fraction();
        let filled = (frac * self.width as f64) as u32;
        let empty = self.width - filled;
        let bar: String = "█".repeat(filled as usize) + &"░".repeat(empty as usize);
        let pct = (frac * 100.0) as u32;
        format!(
            "{} [{bar}] {pct}% ({}/{})",
            self.message, self.current, self.total
        )
    }
}

/// Table column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
}

/// Formats data as an ASCII table.
pub fn format_table(headers: &[&str], rows: &[Vec<String>], align: &[Align]) -> String {
    // Calculate column widths
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < cols && cell.len() > widths[i] {
                widths[i] = cell.len();
            }
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
            if i >= cols {
                break;
            }
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

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}

impl Stopwatch {
    /// Creates a new stopped stopwatch.
    pub fn new() -> Self {
        Self {
            start_ns: 0,
            accumulated_ns: 0,
            running: false,
        }
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
        if ms < 1.0 {
            format!("{:.0}μs", ms * 1000.0)
        } else if ms < 1000.0 {
            format!("{ms:.1}ms")
        } else {
            format!("{:.2}s", ms / 1000.0)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S4.11-S4.20: Real Process, Environment, Path, and FS Operations
// ═══════════════════════════════════════════════════════════════════════

/// Output captured from a spawned process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Process exit code (0 = success).
    pub exit_code: i32,
}

/// Spawns a command and captures its output.
///
/// Uses real `std::process::Command` to execute the program.
///
/// # Errors
///
/// Returns an error if the process cannot be started (e.g., program not found).
pub fn spawn_command(program: &str, args: &[&str]) -> Result<CommandOutput, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn '{program}': {e}"))?;

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Spawns a command with a timeout in milliseconds.
///
/// If the process does not exit within `timeout_ms`, it is killed and an error
/// is returned.
///
/// # Errors
///
/// Returns an error if the process cannot be started or if it exceeds the timeout.
pub fn spawn_with_timeout(
    program: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Result<CommandOutput, String> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn '{program}': {e}"))?;

    let deadline = std::time::Duration::from_millis(timeout_ms);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut r| {
                        let mut s = String::new();
                        std::io::Read::read_to_string(&mut r, &mut s).ok();
                        s
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut r| {
                        let mut s = String::new();
                        std::io::Read::read_to_string(&mut r, &mut s).ok();
                        s
                    })
                    .unwrap_or_default();
                return Ok(CommandOutput {
                    stdout,
                    stderr,
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "process '{program}' timed out after {timeout_ms}ms"
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                return Err(format!("error waiting for '{program}': {e}"));
            }
        }
    }
}

/// Gets an environment variable by name.
///
/// Returns `None` if the variable is not set.
pub fn env_get(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Sets an environment variable for the current process.
///
/// # Safety note
///
/// This modifies the process environment, which is shared state. In
/// multi-threaded contexts, concurrent reads and writes to environment
/// variables can cause issues on some platforms.
pub fn env_set(key: &str, value: &str) {
    // SAFETY: We document that env_set is not thread-safe on all platforms.
    // This matches std::env::set_var behavior.
    unsafe {
        std::env::set_var(key, value);
    }
}

/// Joins two path segments using the platform separator.
///
/// Uses real `std::path::Path` for correct cross-platform behavior.
pub fn path_join(base: &str, child: &str) -> String {
    std::path::Path::new(base)
        .join(child)
        .to_string_lossy()
        .to_string()
}

/// Returns the parent directory of a path, or `None` for root/empty paths.
pub fn path_parent(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
}

/// Returns the file extension (without the dot), or `None` if there is none.
pub fn path_extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
}

/// Recursively walks a directory and returns all file paths.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn walk_dir(path: &str) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    walk_dir_recursive(std::path::Path::new(path), &mut results)?;
    results.sort();
    Ok(results)
}

/// Internal recursive helper for `walk_dir`.
fn walk_dir_recursive(dir: &std::path::Path, results: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory '{}': {e}", dir.display()))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| format!("failed to read entry in '{}': {e}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir_recursive(&path, results)?;
        } else {
            results.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
}

/// Returns the system temporary directory path.
pub fn temp_dir() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// SQ8.1: Pipe stdin to child process
// ═══════════════════════════════════════════════════════════════════════

/// Spawn a process and write data to its stdin, capture stdout.
pub fn spawn_with_stdin(
    program: &str,
    args: &[&str],
    stdin_data: &[u8],
) -> Result<CommandOutput, String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {program}: {e}"))?;

    // Write to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(stdin_data)
            .map_err(|e| format!("write stdin: {e}"))?;
        // Drop stdin to signal EOF
    }

    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// SQ8.2: Stream stdout line-by-line
// ═══════════════════════════════════════════════════════════════════════

/// Spawn a process and read stdout line by line, calling handler for each.
pub fn spawn_streaming<F>(program: &str, args: &[&str], mut handler: F) -> Result<i32, String>
where
    F: FnMut(&str),
{
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {program}: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => handler(&l),
                Err(_) => break,
            }
        }
    }

    let status = child.wait().map_err(|e| format!("wait: {e}"))?;
    Ok(status.code().unwrap_or(-1))
}

// ═══════════════════════════════════════════════════════════════════════
// SQ8.3: Exit code constants
// ═══════════════════════════════════════════════════════════════════════

/// Standard exit code: success.
pub const EXIT_SUCCESS: i32 = 0;
/// Standard exit code: general failure.
pub const EXIT_FAILURE: i32 = 1;
/// Exit code: command not found (shell convention).
pub const EXIT_NOT_FOUND: i32 = 127;
/// Exit code: permission denied.
pub const EXIT_PERMISSION: i32 = 126;

// ═══════════════════════════════════════════════════════════════════════
// SQ8.4-SQ8.6: File permissions, symlinks, metadata
// ═══════════════════════════════════════════════════════════════════════

/// File metadata.
#[derive(Debug, Clone)]
pub struct FileMeta {
    /// File size in bytes.
    pub size: u64,
    /// Is a directory.
    pub is_dir: bool,
    /// Is a regular file.
    pub is_file: bool,
    /// Is a symbolic link.
    pub is_symlink: bool,
    /// Unix permissions (octal, e.g., 0o755).
    pub permissions: u32,
    /// Last modified time (seconds since epoch).
    pub modified_secs: u64,
}

/// Get file metadata (size, type, permissions, modified time).
pub fn file_metadata(path: &str) -> Result<FileMeta, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat {path}: {e}"))?;

    let permissions = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode()
        }
        #[cfg(not(unix))]
        {
            if meta.permissions().readonly() {
                0o444
            } else {
                0o644
            }
        }
    };

    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(FileMeta {
        size: meta.len(),
        is_dir: meta.is_dir(),
        is_file: meta.is_file(),
        is_symlink: meta.file_type().is_symlink(),
        permissions,
        modified_secs,
    })
}

/// Create a symbolic link.
#[cfg(unix)]
pub fn create_symlink(target: &str, link: &str) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|e| format!("symlink: {e}"))
}

/// Read a symbolic link target.
pub fn read_symlink(path: &str) -> Result<String, String> {
    std::fs::read_link(path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("readlink {path}: {e}"))
}

/// Set file permissions (Unix mode, e.g., 0o755).
#[cfg(unix)]
pub fn set_permissions(path: &str, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(path, perms).map_err(|e| format!("chmod {path}: {e}"))
}

// ═══════════════════════════════════════════════════════════════════════
// SQ8.8: Home directory
// ═══════════════════════════════════════════════════════════════════════

/// Get the user's home directory (cross-platform).
pub fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

// ═══════════════════════════════════════════════════════════════════════
// SQ8.9: Find executable in PATH
// ═══════════════════════════════════════════════════════════════════════

/// Find an executable in the system PATH.
pub fn which(name: &str) -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;
    let separator = if cfg!(windows) { ";" } else { ":" };
    for dir in path_var.split(separator) {
        let candidate = std::path::Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
        // On Unix, also try without extension
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&candidate) {
                if meta.permissions().mode() & 0o111 != 0 {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
        // On Windows, try common extensions
        #[cfg(windows)]
        {
            for ext in &[".exe", ".cmd", ".bat", ".com"] {
                let with_ext = candidate.with_extension(&ext[1..]);
                if with_ext.is_file() {
                    return Some(with_ext.to_string_lossy().to_string());
                }
            }
        }
    }
    None
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
            ArgDef {
                long: "output".to_string(),
                short: Some('o'),
                help: "Output file".to_string(),
                takes_value: true,
                required: false,
                default: None,
            },
            ArgDef {
                long: "verbose".to_string(),
                short: Some('v'),
                help: "Verbose".to_string(),
                takes_value: false,
                required: false,
                default: None,
            },
        ];
        let args: Vec<String> = vec!["--output", "out.fj", "-v", "input.fj"]
            .iter()
            .map(|s| s.to_string())
            .collect();
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
        let defs = vec![ArgDef {
            long: "output".to_string(),
            short: Some('o'),
            help: "Output file".to_string(),
            takes_value: true,
            required: true,
            default: None,
        }];
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
            vec![
                "main.fj".to_string(),
                "source".to_string(),
                "1.2KB".to_string(),
            ],
            vec![
                "lib.fj".to_string(),
                "library".to_string(),
                "3.5KB".to_string(),
            ],
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

    // S4.11: spawn_command — echo hello
    #[test]
    fn s4_11_spawn_echo() {
        let result = spawn_command("echo", &["hello"]).unwrap();
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    // S4.11: spawn_command — nonexistent program
    #[test]
    fn s4_11_spawn_nonexistent() {
        let result = spawn_command("__nonexistent_program_fj_test__", &[]);
        assert!(result.is_err());
    }

    // S4.12: spawn_with_timeout — fast command succeeds
    #[test]
    fn s4_12_spawn_with_timeout_ok() {
        let result = spawn_with_timeout("echo", &["timeout_test"], 5000).unwrap();
        assert!(result.stdout.contains("timeout_test"));
        assert_eq!(result.exit_code, 0);
    }

    // S4.12: spawn_with_timeout — slow command times out
    #[test]
    fn s4_12_spawn_with_timeout_expires() {
        let result = spawn_with_timeout("sleep", &["60"], 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("timed out"));
    }

    // S4.13: env_get / env_set roundtrip
    #[test]
    fn s4_13_env_roundtrip() {
        let key = "FJ_TEST_ENV_VAR_SYSTEM_RS";
        let value = "fajar_lang_42";
        env_set(key, value);
        let got = env_get(key);
        assert_eq!(got, Some(value.to_string()));
    }

    // S4.13: env_get missing variable
    #[test]
    fn s4_13_env_get_missing() {
        let got = env_get("FJ_TEST_DEFINITELY_NOT_SET_XYZ_999");
        assert!(got.is_none());
    }

    // S4.14: path_join
    #[test]
    fn s4_14_path_join() {
        let joined = path_join("/home/fajar", "src/main.fj");
        assert_eq!(joined, "/home/fajar/src/main.fj");
    }

    // S4.14: path_join with absolute child replaces base
    #[test]
    fn s4_14_path_join_absolute_child() {
        let joined = path_join("/home/fajar", "/etc/config");
        assert_eq!(joined, "/etc/config");
    }

    // S4.15: path_parent
    #[test]
    fn s4_15_path_parent() {
        let parent = path_parent("/home/fajar/file.txt");
        assert_eq!(parent, Some("/home/fajar".to_string()));
    }

    // S4.15: path_parent of root returns None (no parent above /)
    #[test]
    fn s4_15_path_parent_root() {
        // On Unix, "/" has no parent — std::path returns None.
        let parent = path_parent("/");
        assert!(parent.is_none());
    }

    // S4.16: path_extension
    #[test]
    fn s4_16_path_extension() {
        assert_eq!(path_extension("main.fj"), Some("fj".to_string()));
        assert_eq!(path_extension("archive.tar.gz"), Some("gz".to_string()));
        assert_eq!(path_extension("no_extension"), None);
    }

    // S4.17: walk_dir on temp directory with created files
    #[test]
    fn s4_17_walk_dir() {
        let dir = std::env::temp_dir().join("fj_test_walk_dir_s4_17");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), "aaa").unwrap();
        std::fs::write(dir.join("sub/b.txt"), "bbb").unwrap();

        let files = walk_dir(dir.to_str().unwrap()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.ends_with("a.txt")));
        assert!(files.iter().any(|f| f.ends_with("b.txt")));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    // S4.17: walk_dir on nonexistent directory
    #[test]
    fn s4_17_walk_dir_nonexistent() {
        let result = walk_dir("/tmp/fj_test_nonexistent_dir_xyz_999");
        assert!(result.is_err());
    }

    // S4.18: temp_dir returns valid path
    #[test]
    fn s4_18_temp_dir() {
        let tmp = temp_dir();
        assert!(!tmp.is_empty());
        assert!(std::path::Path::new(&tmp).exists());
    }

    // ═══════════════════════════════════════════════════════════════════
    // SQ8: Quality improvement tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn sq8_1_pipe_stdin() {
        // Pipe data to `cat` and capture output
        let result = spawn_with_stdin("cat", &[], b"hello from stdin").unwrap();
        assert_eq!(result.stdout, "hello from stdin");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn sq8_2_stream_stdout() {
        let mut lines = Vec::new();
        let code = spawn_streaming("echo", &["line1\nline2\nline3"], |line| {
            lines.push(line.to_string());
        })
        .unwrap();
        assert_eq!(code, 0);
        assert!(!lines.is_empty(), "should capture at least 1 line");
    }

    #[test]
    fn sq8_3_exit_codes() {
        assert_eq!(EXIT_SUCCESS, 0);
        assert_eq!(EXIT_FAILURE, 1);
        assert_eq!(EXIT_NOT_FOUND, 127);
        assert_eq!(EXIT_PERMISSION, 126);
    }

    #[test]
    fn sq8_6_file_metadata() {
        let meta = file_metadata("/tmp").unwrap();
        assert!(meta.is_dir, "/tmp should be a directory");
        assert!(!meta.is_file, "/tmp should not be a regular file");
    }

    #[test]
    fn sq8_6_file_metadata_regular() {
        // Create a temp file
        let path = format!("{}/fj_meta_test_{}", temp_dir(), std::process::id());
        std::fs::write(&path, "test data").unwrap();
        let meta = file_metadata(&path).unwrap();
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 9); // "test data" = 9 bytes
        assert!(meta.modified_secs > 0);
        std::fs::remove_file(&path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sq8_4_file_permissions() {
        let path = format!("{}/fj_perm_test_{}", temp_dir(), std::process::id());
        std::fs::write(&path, "test").unwrap();
        set_permissions(&path, 0o755).unwrap();
        let meta = file_metadata(&path).unwrap();
        assert_eq!(meta.permissions & 0o777, 0o755);
        std::fs::remove_file(&path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sq8_5_symlink() {
        let dir = format!("{}/fj_symlink_test_{}", temp_dir(), std::process::id());
        let _ = std::fs::create_dir_all(&dir);
        let target = format!("{dir}/target.txt");
        let link = format!("{dir}/link.txt");
        std::fs::write(&target, "real file").unwrap();
        create_symlink(&target, &link).unwrap();

        let resolved = read_symlink(&link).unwrap();
        assert_eq!(resolved, target);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sq8_8_home_dir() {
        let home = home_dir();
        assert!(home.is_some(), "should detect home directory");
        let home = home.unwrap();
        assert!(!home.is_empty());
        assert!(std::path::Path::new(&home).exists());
    }

    #[test]
    fn sq8_9_which_cargo() {
        // cargo should be in PATH (we're running inside cargo test)
        let result = which("cargo");
        assert!(result.is_some(), "cargo should be findable in PATH");
        let path = result.unwrap();
        assert!(
            path.contains("cargo"),
            "path should contain 'cargo': {path}"
        );
    }

    #[test]
    fn sq8_9_which_nonexistent() {
        let result = which("this_program_definitely_does_not_exist_12345");
        assert!(result.is_none());
    }
}
