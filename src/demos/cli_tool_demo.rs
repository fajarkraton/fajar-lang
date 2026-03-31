//! Sprint W8: CLI Tool — arg parser, subcommand router, file processor,
//! color output, progress bar, table formatter, config file, error reporter.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W8.1: ArgParser — Positional Args, Flags, Options
// ═══════════════════════════════════════════════════════════════════════

/// Parsed argument type.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    /// Boolean flag (present/absent).
    Flag(bool),
    /// String option value.
    Option(String),
    /// Positional argument.
    Positional(String),
}

/// Definition of a CLI flag or option.
#[derive(Debug, Clone)]
pub struct ArgDef {
    /// Long name (e.g., "verbose").
    pub long: String,
    /// Short name (e.g., 'v').
    pub short: Option<char>,
    /// Help text.
    pub help: String,
    /// Whether this takes a value (option) or is just a flag.
    pub takes_value: bool,
    /// Default value (for options).
    pub default: Option<String>,
}

/// Parsed CLI arguments.
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    /// Flag/option values indexed by long name.
    pub named: HashMap<String, ArgValue>,
    /// Positional arguments in order.
    pub positional: Vec<String>,
    /// The subcommand name (if any).
    pub subcommand: Option<String>,
    /// Remaining arguments after `--`.
    pub rest: Vec<String>,
}

impl ParsedArgs {
    /// Check if a flag is set.
    pub fn has_flag(&self, name: &str) -> bool {
        matches!(self.named.get(name), Some(ArgValue::Flag(true)))
    }

    /// Get an option value.
    pub fn get_option(&self, name: &str) -> Option<&str> {
        match self.named.get(name) {
            Some(ArgValue::Option(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get a positional argument by index.
    pub fn get_positional(&self, idx: usize) -> Option<&str> {
        self.positional.get(idx).map(|s| s.as_str())
    }
}

/// CLI argument parser.
#[derive(Debug, Clone)]
pub struct ArgParser {
    /// Program name.
    pub program: String,
    /// Program description.
    pub description: String,
    /// Defined arguments.
    defs: Vec<ArgDef>,
    /// Allowed subcommands.
    subcommands: Vec<String>,
}

impl ArgParser {
    /// Create a new argument parser.
    pub fn new(program: &str, description: &str) -> Self {
        Self {
            program: program.into(),
            description: description.into(),
            defs: Vec::new(),
            subcommands: Vec::new(),
        }
    }

    /// Add a flag definition.
    pub fn flag(mut self, long: &str, short: Option<char>, help: &str) -> Self {
        self.defs.push(ArgDef {
            long: long.into(),
            short,
            help: help.into(),
            takes_value: false,
            default: None,
        });
        self
    }

    /// Add an option definition (takes a value).
    pub fn option(
        mut self,
        long: &str,
        short: Option<char>,
        help: &str,
        default: Option<&str>,
    ) -> Self {
        self.defs.push(ArgDef {
            long: long.into(),
            short,
            help: help.into(),
            takes_value: true,
            default: default.map(|s| s.into()),
        });
        self
    }

    /// Add an allowed subcommand.
    pub fn subcommand(mut self, name: &str) -> Self {
        self.subcommands.push(name.into());
        self
    }

    /// Parse a list of argument strings.
    pub fn parse(&self, args: &[&str]) -> Result<ParsedArgs, String> {
        let mut named: HashMap<String, ArgValue> = HashMap::new();
        let mut positional = Vec::new();
        let mut subcommand = None;
        let mut rest = Vec::new();
        let mut in_rest = false;

        // Apply defaults
        for def in &self.defs {
            if def.takes_value {
                if let Some(ref default) = def.default {
                    named.insert(def.long.clone(), ArgValue::Option(default.clone()));
                }
            }
        }

        let mut i = 0;
        while i < args.len() {
            let arg = args[i];

            if in_rest {
                rest.push(arg.to_string());
                i += 1;
                continue;
            }

            if arg == "--" {
                in_rest = true;
                i += 1;
                continue;
            }

            if let Some(long_name) = arg.strip_prefix("--") {
                // Handle --key=value
                if let Some((key, value)) = long_name.split_once('=') {
                    let def = self
                        .defs
                        .iter()
                        .find(|d| d.long == key)
                        .ok_or_else(|| format!("unknown option: --{key}"))?;
                    if !def.takes_value {
                        return Err(format!("--{key} does not take a value"));
                    }
                    named.insert(key.into(), ArgValue::Option(value.into()));
                } else {
                    let def = self
                        .defs
                        .iter()
                        .find(|d| d.long == long_name)
                        .ok_or_else(|| format!("unknown option: --{long_name}"))?;
                    if def.takes_value {
                        i += 1;
                        let val = args
                            .get(i)
                            .ok_or_else(|| format!("--{long_name} requires a value"))?;
                        named.insert(long_name.into(), ArgValue::Option((*val).into()));
                    } else {
                        named.insert(long_name.into(), ArgValue::Flag(true));
                    }
                }
            } else if let Some(short_chars) = arg.strip_prefix('-') {
                if short_chars.is_empty() {
                    return Err("bare '-' is not allowed".into());
                }
                for ch in short_chars.chars() {
                    let def = self
                        .defs
                        .iter()
                        .find(|d| d.short == Some(ch))
                        .ok_or_else(|| format!("unknown option: -{ch}"))?;
                    if def.takes_value {
                        i += 1;
                        let val = args
                            .get(i)
                            .ok_or_else(|| format!("-{ch} requires a value"))?;
                        named.insert(def.long.clone(), ArgValue::Option((*val).into()));
                    } else {
                        named.insert(def.long.clone(), ArgValue::Flag(true));
                    }
                }
            } else if subcommand.is_none()
                && self.subcommands.iter().any(|s| s == arg)
            {
                subcommand = Some(arg.to_string());
            } else {
                positional.push(arg.to_string());
            }

            i += 1;
        }

        Ok(ParsedArgs {
            named,
            positional,
            subcommand,
            rest,
        })
    }

    /// Generate help text.
    pub fn help(&self) -> String {
        let mut out = format!("{} - {}\n\nUsage: {}", self.program, self.description, self.program);
        if !self.subcommands.is_empty() {
            out.push_str(" [COMMAND]");
        }
        out.push_str(" [OPTIONS] [ARGS...]\n\n");

        if !self.subcommands.is_empty() {
            out.push_str("Commands:\n");
            for cmd in &self.subcommands {
                out.push_str(&format!("  {cmd}\n"));
            }
            out.push('\n');
        }

        if !self.defs.is_empty() {
            out.push_str("Options:\n");
            for def in &self.defs {
                let short = def
                    .short
                    .map(|c| format!("-{c}, "))
                    .unwrap_or_else(|| "    ".into());
                let value_hint = if def.takes_value { " <VALUE>" } else { "" };
                out.push_str(&format!(
                    "  {short}--{}{value_hint}    {}\n",
                    def.long, def.help
                ));
            }
        }
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.2: SubcommandRouter — Route to Subcommand Handlers
// ═══════════════════════════════════════════════════════════════════════

/// Result from running a subcommand.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Output text.
    pub output: String,
}

/// Handler function type for subcommands.
pub type CommandHandler = fn(&ParsedArgs) -> CommandResult;

/// Routes parsed arguments to the appropriate subcommand handler.
pub struct SubcommandRouter {
    /// Registered handlers by command name.
    handlers: HashMap<String, CommandHandler>,
    /// Default handler when no subcommand is specified.
    default_handler: Option<CommandHandler>,
}

impl SubcommandRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: None,
        }
    }

    /// Register a handler for a subcommand.
    pub fn register(mut self, name: &str, handler: CommandHandler) -> Self {
        self.handlers.insert(name.into(), handler);
        self
    }

    /// Set the default handler.
    pub fn default(mut self, handler: CommandHandler) -> Self {
        self.default_handler = Some(handler);
        self
    }

    /// Route to the appropriate handler.
    pub fn dispatch(&self, args: &ParsedArgs) -> CommandResult {
        if let Some(ref cmd) = args.subcommand {
            if let Some(handler) = self.handlers.get(cmd) {
                return handler(args);
            }
            return CommandResult {
                exit_code: 1,
                output: format!("unknown command: {cmd}"),
            };
        }
        if let Some(handler) = self.default_handler {
            return handler(args);
        }
        CommandResult {
            exit_code: 0,
            output: "no command specified".into(),
        }
    }

    /// List registered command names.
    pub fn commands(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for SubcommandRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.3: FileProcessor — Read, Transform, Write with Progress
// ═══════════════════════════════════════════════════════════════════════

/// A transform operation to apply to file content.
#[derive(Debug, Clone, PartialEq)]
pub enum FileTransform {
    /// Convert to uppercase.
    Uppercase,
    /// Convert to lowercase.
    Lowercase,
    /// Replace occurrences.
    Replace { from: String, to: String },
    /// Prepend a header.
    PrependHeader(String),
    /// Append a footer.
    AppendFooter(String),
    /// Remove blank lines.
    RemoveBlankLines,
    /// Number each line.
    NumberLines,
}

/// Processing statistics.
#[derive(Debug, Clone, Default)]
pub struct ProcessStats {
    /// Bytes read.
    pub bytes_read: usize,
    /// Bytes written.
    pub bytes_written: usize,
    /// Lines processed.
    pub lines_processed: usize,
    /// Transforms applied.
    pub transforms_applied: usize,
}

/// Processes file content through a chain of transforms.
pub struct FileProcessor {
    /// Transform chain.
    transforms: Vec<FileTransform>,
}

impl FileProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Add a transform to the chain.
    pub fn add_transform(mut self, transform: FileTransform) -> Self {
        self.transforms.push(transform);
        self
    }

    /// Process content through all transforms, returning result and stats.
    pub fn process(&self, input: &str) -> (String, ProcessStats) {
        let mut stats = ProcessStats {
            bytes_read: input.len(),
            lines_processed: input.lines().count(),
            ..Default::default()
        };

        let mut content = input.to_string();

        for transform in &self.transforms {
            content = Self::apply_transform(&content, transform);
            stats.transforms_applied += 1;
        }

        stats.bytes_written = content.len();
        (content, stats)
    }

    /// Apply a single transform to content.
    fn apply_transform(content: &str, transform: &FileTransform) -> String {
        match transform {
            FileTransform::Uppercase => content.to_uppercase(),
            FileTransform::Lowercase => content.to_lowercase(),
            FileTransform::Replace { from, to } => content.replace(from.as_str(), to.as_str()),
            FileTransform::PrependHeader(header) => format!("{header}\n{content}"),
            FileTransform::AppendFooter(footer) => format!("{content}\n{footer}"),
            FileTransform::RemoveBlankLines => content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            FileTransform::NumberLines => content
                .lines()
                .enumerate()
                .map(|(i, line)| format!("{:4} | {line}", i + 1))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

impl Default for FileProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.4: ColorOutput — ANSI Color Codes
// ═══════════════════════════════════════════════════════════════════════

/// ANSI color codes for terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Red text.
    Red,
    /// Green text.
    Green,
    /// Yellow text.
    Yellow,
    /// Blue text.
    Blue,
    /// Magenta text.
    Magenta,
    /// Cyan text.
    Cyan,
    /// White text.
    White,
    /// Bold modifier.
    Bold,
    /// Dim modifier.
    Dim,
    /// Reset to default.
    Reset,
}

impl Color {
    /// Get the ANSI escape code for this color.
    pub fn code(self) -> &'static str {
        match self {
            Color::Red => "\x1b[31m",
            Color::Green => "\x1b[32m",
            Color::Yellow => "\x1b[33m",
            Color::Blue => "\x1b[34m",
            Color::Magenta => "\x1b[35m",
            Color::Cyan => "\x1b[36m",
            Color::White => "\x1b[37m",
            Color::Bold => "\x1b[1m",
            Color::Dim => "\x1b[2m",
            Color::Reset => "\x1b[0m",
        }
    }
}

/// Convenience functions for colorized output.
pub struct ColorOutput;

impl ColorOutput {
    /// Wrap text in a color.
    pub fn paint(text: &str, color: Color) -> String {
        format!("{}{}{}", color.code(), text, Color::Reset.code())
    }

    /// Red text (for errors).
    pub fn error(text: &str) -> String {
        Self::paint(text, Color::Red)
    }

    /// Green text (for success).
    pub fn success(text: &str) -> String {
        Self::paint(text, Color::Green)
    }

    /// Yellow text (for warnings).
    pub fn warning(text: &str) -> String {
        Self::paint(text, Color::Yellow)
    }

    /// Blue text (for info).
    pub fn info(text: &str) -> String {
        Self::paint(text, Color::Blue)
    }

    /// Bold text.
    pub fn bold(text: &str) -> String {
        format!("{}{}{}", Color::Bold.code(), text, Color::Reset.code())
    }

    /// Strip ANSI escape codes from a string.
    pub fn strip_ansi(text: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;
        for ch in text.chars() {
            if ch == '\x1b' {
                in_escape = true;
                continue;
            }
            if in_escape {
                if ch == 'm' {
                    in_escape = false;
                }
                continue;
            }
            result.push(ch);
        }
        result
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.5: ProgressBar — Animated Progress Indicator
// ═══════════════════════════════════════════════════════════════════════

/// A text-based progress bar.
#[derive(Debug, Clone)]
pub struct ProgressBar {
    /// Total number of steps.
    pub total: u64,
    /// Current progress.
    pub current: u64,
    /// Bar width in characters.
    pub width: usize,
    /// Fill character.
    pub fill_char: char,
    /// Empty character.
    pub empty_char: char,
    /// Label text.
    pub label: String,
}

impl ProgressBar {
    /// Create a new progress bar.
    pub fn new(total: u64) -> Self {
        Self {
            total,
            current: 0,
            width: 40,
            fill_char: '#',
            empty_char: '-',
            label: String::new(),
        }
    }

    /// Set bar width.
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Set label.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.into();
        self
    }

    /// Advance progress by one step.
    pub fn tick(&mut self) {
        if self.current < self.total {
            self.current += 1;
        }
    }

    /// Set progress to a specific value.
    pub fn set(&mut self, value: u64) {
        self.current = value.min(self.total);
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.current as f64 / self.total as f64
    }

    /// Progress as a percentage (0 to 100).
    pub fn percentage(&self) -> u64 {
        (self.fraction() * 100.0) as u64
    }

    /// Whether progress is complete.
    pub fn is_done(&self) -> bool {
        self.current >= self.total
    }

    /// Render the progress bar as a string.
    pub fn render(&self) -> String {
        let filled = (self.fraction() * self.width as f64) as usize;
        let empty = self.width.saturating_sub(filled);
        let bar: String = std::iter::repeat(self.fill_char)
            .take(filled)
            .chain(std::iter::repeat(self.empty_char).take(empty))
            .collect();
        if self.label.is_empty() {
            format!("[{bar}] {}%", self.percentage())
        } else {
            format!("{} [{bar}] {}%", self.label, self.percentage())
        }
    }
}

impl fmt::Display for ProgressBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.6: TableFormatter — Aligned Table Output
// ═══════════════════════════════════════════════════════════════════════

/// Column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    /// Left-aligned.
    Left,
    /// Right-aligned.
    Right,
    /// Center-aligned.
    Center,
}

/// A column definition.
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header.
    pub header: String,
    /// Alignment.
    pub alignment: Alignment,
    /// Minimum width.
    pub min_width: usize,
}

/// Formats data as an aligned table.
pub struct TableFormatter {
    /// Column definitions.
    columns: Vec<Column>,
    /// Row data.
    rows: Vec<Vec<String>>,
    /// Whether to draw borders.
    pub borders: bool,
}

impl TableFormatter {
    /// Create a new table.
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            borders: true,
        }
    }

    /// Add a column.
    pub fn column(mut self, header: &str, alignment: Alignment) -> Self {
        self.columns.push(Column {
            header: header.into(),
            alignment,
            min_width: 0,
        });
        self
    }

    /// Add a column with minimum width.
    pub fn column_min_width(mut self, header: &str, alignment: Alignment, min_width: usize) -> Self {
        self.columns.push(Column {
            header: header.into(),
            alignment,
            min_width,
        });
        self
    }

    /// Add a row of values.
    pub fn row(mut self, values: &[&str]) -> Self {
        self.rows.push(values.iter().map(|s| (*s).into()).collect());
        self
    }

    /// Render the table as a string.
    pub fn render(&self) -> String {
        if self.columns.is_empty() {
            return String::new();
        }

        // Calculate column widths
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|c| c.header.len().max(c.min_width))
            .collect();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        let mut out = String::new();

        if self.borders {
            // Top border
            out.push_str(&self.horizontal_line(&widths));
            out.push('\n');
        }

        // Header
        out.push_str(&self.format_row(
            &self.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(),
            &widths,
        ));
        out.push('\n');

        if self.borders {
            out.push_str(&self.horizontal_line(&widths));
            out.push('\n');
        }

        // Data rows
        for row in &self.rows {
            let cells: Vec<&str> = row.iter().map(|s| s.as_str()).collect();
            out.push_str(&self.format_row(&cells, &widths));
            out.push('\n');
        }

        if self.borders {
            out.push_str(&self.horizontal_line(&widths));
            out.push('\n');
        }

        out
    }

    /// Format a single row.
    fn format_row(&self, cells: &[&str], widths: &[usize]) -> String {
        let mut parts = Vec::new();
        for (i, &width) in widths.iter().enumerate() {
            let cell = cells.get(i).copied().unwrap_or("");
            let alignment = self
                .columns
                .get(i)
                .map(|c| c.alignment)
                .unwrap_or(Alignment::Left);
            parts.push(Self::align_cell(cell, width, alignment));
        }
        if self.borders {
            format!("| {} |", parts.join(" | "))
        } else {
            parts.join("  ")
        }
    }

    /// Generate a horizontal separator line.
    fn horizontal_line(&self, widths: &[usize]) -> String {
        let segments: Vec<String> = widths.iter().map(|&w| "-".repeat(w + 2)).collect();
        format!("+{}+", segments.join("+"))
    }

    /// Align a cell value within the given width.
    fn align_cell(text: &str, width: usize, alignment: Alignment) -> String {
        let padding = width.saturating_sub(text.len());
        match alignment {
            Alignment::Left => format!("{text}{}", " ".repeat(padding)),
            Alignment::Right => format!("{}{text}", " ".repeat(padding)),
            Alignment::Center => {
                let left_pad = padding / 2;
                let right_pad = padding - left_pad;
                format!("{}{text}{}", " ".repeat(left_pad), " ".repeat(right_pad))
            }
        }
    }
}

impl Default for TableFormatter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.7: ConfigFile — TOML/JSON Config Reading and Writing
// ═══════════════════════════════════════════════════════════════════════

/// Configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// String value.
    String(String),
    /// Integer value.
    Integer(i64),
    /// Float value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Array of values.
    Array(Vec<ConfigValue>),
    /// Nested table/section.
    Table(Vec<(String, ConfigValue)>),
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigValue::String(s) => write!(f, "\"{s}\""),
            ConfigValue::Integer(n) => write!(f, "{n}"),
            ConfigValue::Float(v) => write!(f, "{v}"),
            ConfigValue::Bool(b) => write!(f, "{b}"),
            ConfigValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            ConfigValue::Table(pairs) => {
                write!(f, "{{")?;
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k} = {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// Simple configuration file manager.
#[derive(Debug, Clone)]
pub struct ConfigFile {
    /// Configuration entries.
    entries: Vec<(String, ConfigValue)>,
    /// File format.
    pub format: ConfigFormat,
}

/// Configuration file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML format.
    Toml,
    /// JSON format.
    Json,
}

impl ConfigFile {
    /// Create a new empty config.
    pub fn new(format: ConfigFormat) -> Self {
        Self {
            entries: Vec::new(),
            format,
        }
    }

    /// Set a string value.
    pub fn set_string(&mut self, key: &str, value: &str) {
        self.set(key, ConfigValue::String(value.into()));
    }

    /// Set an integer value.
    pub fn set_int(&mut self, key: &str, value: i64) {
        self.set(key, ConfigValue::Integer(value));
    }

    /// Set a boolean value.
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set(key, ConfigValue::Bool(value));
    }

    /// Set any config value.
    pub fn set(&mut self, key: &str, value: ConfigValue) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value;
        } else {
            self.entries.push((key.into(), value));
        }
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Get a string value.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(ConfigValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get an integer value.
    pub fn get_int(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(ConfigValue::Integer(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get a boolean value.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(ConfigValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// Serialize to TOML-like string.
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.entries {
            match value {
                ConfigValue::Table(pairs) => {
                    out.push_str(&format!("[{key}]\n"));
                    for (k, v) in pairs {
                        out.push_str(&format!("{k} = {v}\n"));
                    }
                    out.push('\n');
                }
                _ => {
                    out.push_str(&format!("{key} = {value}\n"));
                }
            }
        }
        out
    }

    /// Number of top-level entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the config is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All keys.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8.8: ErrorReporter — User-Friendly Error Display
// ═══════════════════════════════════════════════════════════════════════

/// Severity level for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Error — stops execution.
    Error,
    /// Warning — continues execution.
    Warning,
    /// Hint — suggestion for improvement.
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic report for the user.
#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    /// Severity level.
    pub severity: Severity,
    /// Error code (e.g., "E0001").
    pub code: String,
    /// Primary message.
    pub message: String,
    /// File where the error occurred.
    pub file: Option<String>,
    /// Line number (1-based).
    pub line: Option<usize>,
    /// Column number (1-based).
    pub column: Option<usize>,
    /// Suggestions for fixing.
    pub suggestions: Vec<String>,
    /// Additional notes.
    pub notes: Vec<String>,
}

/// Formats error reports for user display.
pub struct ErrorReporter;

impl ErrorReporter {
    /// Format a diagnostic report as a readable string.
    pub fn format(report: &DiagnosticReport) -> String {
        let mut out = String::new();

        // Header line: error[E0001]: message
        out.push_str(&format!(
            "{}[{}]: {}\n",
            report.severity, report.code, report.message
        ));

        // Location
        if let Some(ref file) = report.file {
            let line = report.line.unwrap_or(0);
            let col = report.column.unwrap_or(0);
            out.push_str(&format!(" --> {file}:{line}:{col}\n"));
        }

        // Suggestions
        for suggestion in &report.suggestions {
            out.push_str(&format!("  help: {suggestion}\n"));
        }

        // Notes
        for note in &report.notes {
            out.push_str(&format!("  note: {note}\n"));
        }

        out
    }

    /// Format a diagnostic report with ANSI colors.
    pub fn format_colored(report: &DiagnosticReport) -> String {
        let severity_color = match report.severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Hint => Color::Cyan,
        };
        let mut out = String::new();

        out.push_str(&ColorOutput::paint(
            &format!("{}[{}]", report.severity, report.code),
            severity_color,
        ));
        out.push_str(&format!(
            ": {}\n",
            ColorOutput::bold(&report.message)
        ));

        if let Some(ref file) = report.file {
            let line = report.line.unwrap_or(0);
            let col = report.column.unwrap_or(0);
            out.push_str(&ColorOutput::paint(
                &format!(" --> {file}:{line}:{col}\n"),
                Color::Blue,
            ));
        }

        for suggestion in &report.suggestions {
            out.push_str(&format!(
                "  {}: {suggestion}\n",
                ColorOutput::paint("help", Color::Cyan)
            ));
        }

        for note in &report.notes {
            out.push_str(&format!(
                "  {}: {note}\n",
                ColorOutput::paint("note", Color::Blue)
            ));
        }

        out
    }

    /// Create a simple error report.
    pub fn simple_error(code: &str, message: &str) -> DiagnosticReport {
        DiagnosticReport {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            file: None,
            line: None,
            column: None,
            suggestions: Vec::new(),
            notes: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W8.1: ArgParser
    #[test]
    fn w8_1_parse_flags() {
        let parser = ArgParser::new("fj", "Fajar Lang CLI")
            .flag("verbose", Some('v'), "Enable verbose output")
            .flag("quiet", Some('q'), "Suppress output");
        let args = parser.parse(&["-v", "--quiet"]).unwrap();
        assert!(args.has_flag("verbose"));
        assert!(args.has_flag("quiet"));
    }

    #[test]
    fn w8_1_parse_options() {
        let parser = ArgParser::new("fj", "CLI")
            .option("output", Some('o'), "Output file", None);
        let args = parser.parse(&["--output", "out.fj"]).unwrap();
        assert_eq!(args.get_option("output"), Some("out.fj"));
    }

    #[test]
    fn w8_1_parse_option_equals() {
        let parser = ArgParser::new("fj", "CLI")
            .option("target", None, "Target", None);
        let args = parser.parse(&["--target=arm64"]).unwrap();
        assert_eq!(args.get_option("target"), Some("arm64"));
    }

    #[test]
    fn w8_1_parse_positional() {
        let parser = ArgParser::new("fj", "CLI")
            .flag("verbose", Some('v'), "Verbose");
        let args = parser.parse(&["-v", "file1.fj", "file2.fj"]).unwrap();
        assert_eq!(args.positional.len(), 2);
        assert_eq!(args.get_positional(0), Some("file1.fj"));
    }

    #[test]
    fn w8_1_parse_subcommand() {
        let parser = ArgParser::new("fj", "CLI")
            .subcommand("build")
            .subcommand("run")
            .flag("verbose", Some('v'), "Verbose");
        let args = parser.parse(&["build", "-v", "main.fj"]).unwrap();
        assert_eq!(args.subcommand, Some("build".into()));
        assert!(args.has_flag("verbose"));
        assert_eq!(args.get_positional(0), Some("main.fj"));
    }

    #[test]
    fn w8_1_parse_rest_args() {
        let parser = ArgParser::new("fj", "CLI")
            .flag("verbose", Some('v'), "Verbose");
        let args = parser.parse(&["-v", "--", "extra1", "extra2"]).unwrap();
        assert!(args.has_flag("verbose"));
        assert_eq!(args.rest, vec!["extra1", "extra2"]);
    }

    #[test]
    fn w8_1_parse_unknown_option() {
        let parser = ArgParser::new("fj", "CLI");
        assert!(parser.parse(&["--unknown"]).is_err());
    }

    #[test]
    fn w8_1_help_generation() {
        let parser = ArgParser::new("fj", "Fajar Lang CLI")
            .flag("verbose", Some('v'), "Enable verbose output")
            .subcommand("build");
        let help = parser.help();
        assert!(help.contains("fj"));
        assert!(help.contains("verbose"));
        assert!(help.contains("build"));
    }

    // W8.2: SubcommandRouter
    #[test]
    fn w8_2_router_dispatch() {
        fn handle_build(_args: &ParsedArgs) -> CommandResult {
            CommandResult {
                exit_code: 0,
                output: "built".into(),
            }
        }
        let router = SubcommandRouter::new().register("build", handle_build);
        let args = ParsedArgs {
            named: HashMap::new(),
            positional: Vec::new(),
            subcommand: Some("build".into()),
            rest: Vec::new(),
        };
        let result = router.dispatch(&args);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output, "built");
    }

    #[test]
    fn w8_2_router_unknown_command() {
        let router = SubcommandRouter::new();
        let args = ParsedArgs {
            named: HashMap::new(),
            positional: Vec::new(),
            subcommand: Some("foo".into()),
            rest: Vec::new(),
        };
        let result = router.dispatch(&args);
        assert_eq!(result.exit_code, 1);
    }

    // W8.3: FileProcessor
    #[test]
    fn w8_3_file_transform_chain() {
        let proc = FileProcessor::new()
            .add_transform(FileTransform::Uppercase)
            .add_transform(FileTransform::PrependHeader("// HEADER".into()));
        let (result, stats) = proc.process("hello world");
        assert!(result.starts_with("// HEADER"));
        assert!(result.contains("HELLO WORLD"));
        assert_eq!(stats.transforms_applied, 2);
        assert_eq!(stats.bytes_read, 11);
    }

    #[test]
    fn w8_3_file_replace() {
        let proc = FileProcessor::new().add_transform(FileTransform::Replace {
            from: "foo".into(),
            to: "bar".into(),
        });
        let (result, _) = proc.process("foo baz foo");
        assert_eq!(result, "bar baz bar");
    }

    #[test]
    fn w8_3_file_number_lines() {
        let proc = FileProcessor::new().add_transform(FileTransform::NumberLines);
        let (result, _) = proc.process("alpha\nbeta\ngamma");
        assert!(result.contains("   1 | alpha"));
        assert!(result.contains("   3 | gamma"));
    }

    #[test]
    fn w8_3_file_remove_blank_lines() {
        let proc = FileProcessor::new().add_transform(FileTransform::RemoveBlankLines);
        let (result, _) = proc.process("a\n\nb\n  \nc");
        assert_eq!(result, "a\nb\nc");
    }

    // W8.4: ColorOutput
    #[test]
    fn w8_4_color_codes() {
        let red = ColorOutput::error("oops");
        assert!(red.contains("\x1b[31m"));
        assert!(red.contains("\x1b[0m"));
        assert!(red.contains("oops"));
    }

    #[test]
    fn w8_4_strip_ansi() {
        let colored = ColorOutput::success("ok");
        let plain = ColorOutput::strip_ansi(&colored);
        assert_eq!(plain, "ok");
    }

    // W8.5: ProgressBar
    #[test]
    fn w8_5_progress_bar() {
        let mut bar = ProgressBar::new(10).with_width(20);
        assert_eq!(bar.percentage(), 0);
        bar.set(5);
        assert_eq!(bar.percentage(), 50);
        assert!(!bar.is_done());
        bar.set(10);
        assert_eq!(bar.percentage(), 100);
        assert!(bar.is_done());
    }

    #[test]
    fn w8_5_progress_render() {
        let mut bar = ProgressBar::new(4).with_width(8).with_label("Loading");
        bar.set(2);
        let rendered = bar.render();
        assert!(rendered.contains("Loading"));
        assert!(rendered.contains("50%"));
        assert!(rendered.contains("####"));
    }

    #[test]
    fn w8_5_progress_tick() {
        let mut bar = ProgressBar::new(3);
        bar.tick();
        bar.tick();
        bar.tick();
        assert!(bar.is_done());
        bar.tick(); // should not exceed total
        assert_eq!(bar.current, 3);
    }

    // W8.6: TableFormatter
    #[test]
    fn w8_6_table_basic() {
        let table = TableFormatter::new()
            .column("Name", Alignment::Left)
            .column("Score", Alignment::Right)
            .row(&["Alice", "95"])
            .row(&["Bob", "87"]);
        let out = table.render();
        assert!(out.contains("Name"));
        assert!(out.contains("Score"));
        assert!(out.contains("Alice"));
        assert!(out.contains("95"));
    }

    #[test]
    fn w8_6_table_alignment() {
        let table = TableFormatter::new()
            .column("X", Alignment::Center)
            .row(&["ab"]);
        let out = table.render();
        // Center-aligned: "X" should be centered within its column
        assert!(out.contains("X"));
        assert!(out.contains("ab"));
    }

    // W8.7: ConfigFile
    #[test]
    fn w8_7_config_set_get() {
        let mut config = ConfigFile::new(ConfigFormat::Toml);
        config.set_string("name", "fajar-lang");
        config.set_int("version", 14);
        config.set_bool("debug", true);
        assert_eq!(config.get_string("name"), Some("fajar-lang"));
        assert_eq!(config.get_int("version"), Some(14));
        assert_eq!(config.get_bool("debug"), Some(true));
        assert_eq!(config.len(), 3);
    }

    #[test]
    fn w8_7_config_to_toml() {
        let mut config = ConfigFile::new(ConfigFormat::Toml);
        config.set_string("name", "test");
        config.set_int("port", 8080);
        let toml = config.to_toml();
        assert!(toml.contains("name = \"test\""));
        assert!(toml.contains("port = 8080"));
    }

    #[test]
    fn w8_7_config_overwrite() {
        let mut config = ConfigFile::new(ConfigFormat::Toml);
        config.set_int("x", 1);
        config.set_int("x", 2);
        assert_eq!(config.get_int("x"), Some(2));
        assert_eq!(config.len(), 1);
    }

    // W8.8: ErrorReporter
    #[test]
    fn w8_8_error_report_format() {
        let report = DiagnosticReport {
            severity: Severity::Error,
            code: "SE004".into(),
            message: "type mismatch".into(),
            file: Some("main.fj".into()),
            line: Some(10),
            column: Some(5),
            suggestions: vec!["expected i32, found f64".into()],
            notes: vec!["declared on line 3".into()],
        };
        let out = ErrorReporter::format(&report);
        assert!(out.contains("error[SE004]"));
        assert!(out.contains("type mismatch"));
        assert!(out.contains("main.fj:10:5"));
        assert!(out.contains("help: expected i32"));
        assert!(out.contains("note: declared on line 3"));
    }

    #[test]
    fn w8_8_error_colored() {
        let report = ErrorReporter::simple_error("E001", "something failed");
        let colored = ErrorReporter::format_colored(&report);
        assert!(colored.contains("something failed"));
        // Should contain ANSI codes
        assert!(colored.contains("\x1b["));
    }

    #[test]
    fn w8_8_severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Hint), "hint");
    }
}
