//! VS Code debug extension configuration and integration.
//!
//! Provides launch configuration parsing, debug console commands,
//! variable presentation formatting, and hover info generation
//! for the Fajar Lang VS Code debug adapter.
//!
//! # Components
//!
//! - `LaunchConfig`: Parses `launch.json` configuration
//! - `DebugAdapterDescriptor`: Adapter metadata for VS Code
//! - `VariablePresentation`: Display formatting for debug variables
//! - `CallStackPresentation`: Formatted call stack entries
//! - `DebugConsole`: Interactive debug console with commands
//! - `HoverInfo`: IDE hover tooltip content

use super::{DapDebugState, DebugError, VariableInfo};

// ═══════════════════════════════════════════════════════════════════════
// LaunchConfig
// ═══════════════════════════════════════════════════════════════════════

/// VS Code launch configuration for debugging Fajar Lang programs.
///
/// Mirrors the structure of a `.vscode/launch.json` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchConfig {
    /// Debug adapter type (always "fj").
    pub type_: String,
    /// Request kind (always "launch").
    pub request: String,
    /// Path to the `.fj` program to debug.
    pub program: String,
    /// Command-line arguments for the program.
    pub args: Vec<String>,
    /// Whether to stop at the first statement.
    pub stop_on_entry: bool,
    /// Console type ("internalConsole", "integratedTerminal", "externalTerminal").
    pub console: String,
}

impl LaunchConfig {
    /// Creates a default launch config for the given program.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            type_: "fj".to_string(),
            request: "launch".to_string(),
            program: program.into(),
            args: Vec::new(),
            stop_on_entry: true,
            console: "internalConsole".to_string(),
        }
    }

    /// Parses a launch config from a JSON string.
    ///
    /// Expects a flat JSON object with keys: type, request, program,
    /// args, stopOnEntry, console.
    pub fn from_json(json: &str) -> Result<Self, DebugError> {
        let trimmed = json.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(DebugError::JsonError {
                message: "expected JSON object".to_string(),
            });
        }

        let type_ = extract_json_string(trimmed, "type").unwrap_or_else(|| "fj".to_string());
        let request =
            extract_json_string(trimmed, "request").unwrap_or_else(|| "launch".to_string());
        let program = extract_json_string(trimmed, "program").ok_or_else(|| {
            DebugError::LaunchConfigError {
                message: "missing 'program' field".to_string(),
            }
        })?;
        let stop_on_entry = extract_json_bool(trimmed, "stopOnEntry").unwrap_or(true);
        let console = extract_json_string(trimmed, "console")
            .unwrap_or_else(|| "internalConsole".to_string());

        // Parse args array (simple: extract comma-separated quoted strings)
        let args = extract_json_string_array(trimmed, "args");

        Ok(Self {
            type_,
            request,
            program,
            args,
            stop_on_entry,
            console,
        })
    }

    /// Serializes the launch config to a JSON string.
    pub fn to_json(&self) -> String {
        let args_json = if self.args.is_empty() {
            "[]".to_string()
        } else {
            let items: Vec<String> = self.args.iter().map(|a| format!("\"{}\"", a)).collect();
            format!("[{}]", items.join(", "))
        };

        format!(
            concat!(
                "{{\n",
                "  \"type\": \"{}\",\n",
                "  \"request\": \"{}\",\n",
                "  \"program\": \"{}\",\n",
                "  \"args\": {},\n",
                "  \"stopOnEntry\": {},\n",
                "  \"console\": \"{}\"\n",
                "}}"
            ),
            self.type_, self.request, self.program, args_json, self.stop_on_entry, self.console,
        )
    }

    /// Validates the launch configuration.
    ///
    /// Checks that required fields are present and have valid values.
    pub fn validate(&self) -> Result<(), DebugError> {
        if self.type_ != "fj" {
            return Err(DebugError::LaunchConfigError {
                message: format!("unsupported type '{}', expected 'fj'", self.type_),
            });
        }
        if self.request != "launch" {
            return Err(DebugError::LaunchConfigError {
                message: format!("unsupported request '{}', expected 'launch'", self.request),
            });
        }
        if self.program.is_empty() {
            return Err(DebugError::LaunchConfigError {
                message: "program path is empty".to_string(),
            });
        }
        if !self.program.ends_with(".fj") {
            return Err(DebugError::LaunchConfigError {
                message: format!("program '{}' does not have .fj extension", self.program),
            });
        }
        let valid_consoles = ["internalConsole", "integratedTerminal", "externalTerminal"];
        if !valid_consoles.contains(&self.console.as_str()) {
            return Err(DebugError::LaunchConfigError {
                message: format!("invalid console type '{}'", self.console),
            });
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// JSON extraction helpers
// ═══════════════════════════════════════════════════════════════════════

/// Extracts a quoted string value for a key from a JSON-like string.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let key_start = json.find(&pattern)?;
    let after_key = &json[key_start + pattern.len()..];
    // Skip whitespace and colon
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_space = after_colon.trim_start();
    // Extract quoted value
    let value_start = after_space.strip_prefix('"')?;
    let end = value_start.find('"')?;
    Some(value_start[..end].to_string())
}

/// Extracts a boolean value for a key from a JSON-like string.
fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", key);
    let key_start = json.find(&pattern)?;
    let after_key = &json[key_start + pattern.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_space = after_colon.trim_start();
    if after_space.starts_with("true") {
        Some(true)
    } else if after_space.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Extracts a string array value for a key from a JSON-like string.
fn extract_json_string_array(json: &str, key: &str) -> Vec<String> {
    let pattern = format!("\"{}\"", key);
    let key_start = match json.find(&pattern) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let after_key = &json[key_start + pattern.len()..];
    let after_colon = match after_key.trim_start().strip_prefix(':') {
        Some(s) => s,
        None => return Vec::new(),
    };
    let after_space = after_colon.trim_start();
    let array_content = match after_space.strip_prefix('[') {
        Some(s) => s,
        None => return Vec::new(),
    };
    let end = match array_content.find(']') {
        Some(e) => e,
        None => return Vec::new(),
    };
    let inner = &array_content[..end];
    inner
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim().trim_matches('"');
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// DebugAdapterDescriptor
// ═══════════════════════════════════════════════════════════════════════

/// Metadata describing the Fajar Lang debug adapter for VS Code.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugAdapterDescriptor {
    /// Adapter identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Arguments passed to the runtime when starting the adapter.
    pub runtime_args: Vec<String>,
}

impl Default for DebugAdapterDescriptor {
    fn default() -> Self {
        Self {
            id: "fj-debug".to_string(),
            label: "Fajar Lang Debug".to_string(),
            runtime_args: vec!["debug".to_string(), "--dap".to_string()],
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// VariablePresentation
// ═══════════════════════════════════════════════════════════════════════

/// Display formatting for a debug variable.
#[derive(Debug, Clone, PartialEq)]
pub struct VariablePresentation {
    /// Kind of variable (e.g., "property", "method", "data").
    pub kind: String,
    /// Display attributes (e.g., "readOnly", "rawString").
    pub attributes: Vec<String>,
}

impl VariablePresentation {
    /// Creates a presentation for a given Fajar Lang type.
    pub fn for_type(type_name: &str) -> Self {
        match type_name {
            "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
            | "isize" | "usize" => Self {
                kind: "data".to_string(),
                attributes: vec!["number".to_string()],
            },
            "f32" | "f64" => Self {
                kind: "data".to_string(),
                attributes: vec!["number".to_string(), "float".to_string()],
            },
            "bool" => Self {
                kind: "data".to_string(),
                attributes: vec!["boolean".to_string()],
            },
            "str" => Self {
                kind: "data".to_string(),
                attributes: vec!["rawString".to_string()],
            },
            _ if type_name.starts_with('[') => Self {
                kind: "data".to_string(),
                attributes: vec!["indexed".to_string()],
            },
            _ => Self {
                kind: "property".to_string(),
                attributes: Vec::new(),
            },
        }
    }

    /// Formats a variable value for display in the IDE.
    pub fn format_value(var: &VariableInfo) -> String {
        match var.type_name.as_str() {
            "str" => format!("\"{}\"", var.value),
            "bool" => var.value.clone(),
            "char" => format!("'{}'", var.value),
            _ => var.value.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CallStackPresentation
// ═══════════════════════════════════════════════════════════════════════

/// Formatted call stack entry for IDE display.
#[derive(Debug, Clone, PartialEq)]
pub struct CallStackEntry {
    /// Display string (e.g., "main at test.fj:10:1").
    pub display: String,
    /// Frame ID for reference.
    pub frame_id: u32,
}

/// Formats stack frames for call stack display.
pub fn format_call_stack(state: &DapDebugState) -> Vec<CallStackEntry> {
    state
        .call_stack
        .iter()
        .rev()
        .map(|frame| CallStackEntry {
            display: format!(
                "{} at {}:{}:{}",
                frame.name, frame.source.file, frame.source.line, frame.source.column,
            ),
            frame_id: frame.id,
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// DebugConsole
// ═══════════════════════════════════════════════════════════════════════

/// Interactive debug console for evaluating commands during debugging.
///
/// Supports commands: `print <expr>`, `locals`, `stack`, `break <file>:<line>`, `continue`.
#[derive(Debug, Default)]
pub struct DebugConsole {
    /// Output history.
    output: Vec<String>,
}

impl DebugConsole {
    /// Creates a new debug console.
    pub fn new() -> Self {
        Self { output: Vec::new() }
    }

    /// Executes a debug console command and returns the output.
    pub fn execute_command(&mut self, cmd: &str, state: &DapDebugState) -> String {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let result = match parts.first().copied() {
            Some("locals") => self.cmd_locals(state),
            Some("stack") => self.cmd_stack(state),
            Some("print") => {
                let expr = parts.get(1).unwrap_or(&"");
                self.cmd_print(expr, state)
            }
            Some("break") => {
                let arg = parts.get(1).unwrap_or(&"");
                self.cmd_break(arg)
            }
            Some("continue") => self.cmd_continue(),
            Some("help") => self.cmd_help(),
            Some(other) => format!("unknown command: '{other}'"),
            None => String::new(),
        };
        self.output.push(result.clone());
        result
    }

    /// Returns the output history.
    pub fn history(&self) -> &[String] {
        &self.output
    }

    /// Prints local variables of the current frame.
    fn cmd_locals(&self, state: &DapDebugState) -> String {
        match state.current_frame() {
            Some(frame) => {
                if frame.locals.is_empty() {
                    return "no local variables".to_string();
                }
                frame
                    .locals
                    .iter()
                    .map(|v| format!("{}: {} = {}", v.name, v.type_name, v.value))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            None => "no active frame".to_string(),
        }
    }

    /// Prints the call stack.
    fn cmd_stack(&self, state: &DapDebugState) -> String {
        if state.call_stack.is_empty() {
            return "empty call stack".to_string();
        }
        let entries = format_call_stack(state);
        entries
            .iter()
            .enumerate()
            .map(|(i, e)| format!("#{i} {}", e.display))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Evaluates and prints an expression.
    fn cmd_print(&self, expr: &str, state: &DapDebugState) -> String {
        if expr.is_empty() {
            return "usage: print <expression>".to_string();
        }
        // Try to find as a local variable in the current frame
        if let Some(frame) = state.current_frame() {
            if let Some(var) = frame.find_local(expr) {
                return format!("{} = {}", var.name, var.value);
            }
        }
        format!("cannot evaluate '{expr}'")
    }

    /// Parses and acknowledges a break command.
    fn cmd_break(&self, arg: &str) -> String {
        if arg.is_empty() {
            return "usage: break <file>:<line>".to_string();
        }
        // Parse file:line
        if let Some((file, line_str)) = arg.rsplit_once(':') {
            if let Ok(line) = line_str.parse::<u32>() {
                return format!("breakpoint set at {}:{}", file, line);
            }
        }
        format!("invalid breakpoint format: '{arg}'")
    }

    /// Acknowledges a continue command.
    fn cmd_continue(&self) -> String {
        "continuing execution...".to_string()
    }

    /// Shows available commands.
    fn cmd_help(&self) -> String {
        concat!(
            "Available commands:\n",
            "  print <expr>        - evaluate and print expression\n",
            "  locals              - show local variables\n",
            "  stack               - show call stack\n",
            "  break <file>:<line> - set breakpoint\n",
            "  continue            - resume execution\n",
            "  help                - show this help"
        )
        .to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HoverInfo
// ═══════════════════════════════════════════════════════════════════════

/// Information displayed when hovering over a variable in the IDE.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInfo {
    /// Variable name.
    pub variable_name: String,
    /// Type name.
    pub type_name: String,
    /// Preview of the value.
    pub value_preview: String,
}

impl HoverInfo {
    /// Creates a new hover info entry.
    pub fn new(
        variable_name: impl Into<String>,
        type_name: impl Into<String>,
        value_preview: impl Into<String>,
    ) -> Self {
        Self {
            variable_name: variable_name.into(),
            type_name: type_name.into(),
            value_preview: value_preview.into(),
        }
    }

    /// Formats the hover info as a markdown-like tooltip string.
    pub fn to_tooltip(&self) -> String {
        format!(
            "**{}**: {} = {}",
            self.variable_name, self.type_name, self.value_preview
        )
    }
}

/// Attempts to get hover info for a variable at a given source location.
///
/// Searches the debug state's current frame locals for a matching variable.
/// In a real implementation, this would use the source text and column
/// to identify the token under the cursor.
pub fn get_hover_info(variable_name: &str, state: &DapDebugState) -> Option<HoverInfo> {
    let frame = state.current_frame()?;
    let var = frame.find_local(variable_name)?;
    Some(HoverInfo::new(
        &var.name,
        &var.type_name,
        VariablePresentation::format_value(var),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::dap::{DapDebugState, DapSourceLocation, DapStackFrame, VarScope};

    fn make_debug_state() -> DapDebugState {
        let mut state = DapDebugState::new();
        let mut frame = DapStackFrame::new("main", DapSourceLocation::new("test.fj", 5, 1));
        frame.add_local(VariableInfo::new("x", "i64", VarScope::Local, "42"));
        frame.add_local(VariableInfo::new("name", "str", VarScope::Local, "hello"));
        frame.add_local(VariableInfo::new("flag", "bool", VarScope::Local, "true"));
        state.push_frame(frame);
        state
    }

    #[test]
    fn launch_config_from_json() {
        let json = r#"{
            "type": "fj",
            "request": "launch",
            "program": "examples/hello.fj",
            "stopOnEntry": false,
            "console": "integratedTerminal"
        }"#;

        let config = LaunchConfig::from_json(json).unwrap();
        assert_eq!(config.type_, "fj");
        assert_eq!(config.request, "launch");
        assert_eq!(config.program, "examples/hello.fj");
        assert!(!config.stop_on_entry);
        assert_eq!(config.console, "integratedTerminal");
    }

    #[test]
    fn launch_config_to_json_roundtrip() {
        let config = LaunchConfig::new("test.fj");
        let json = config.to_json();
        assert!(json.contains("\"program\": \"test.fj\""));
        assert!(json.contains("\"type\": \"fj\""));
        assert!(json.contains("\"stopOnEntry\": true"));
    }

    #[test]
    fn launch_config_validate_success() {
        let config = LaunchConfig::new("main.fj");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn launch_config_validate_bad_type() {
        let mut config = LaunchConfig::new("main.fj");
        config.type_ = "python".to_string();
        let err = config.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("unsupported type"));
    }

    #[test]
    fn launch_config_validate_bad_extension() {
        let config = LaunchConfig::new("main.py");
        let err = config.validate().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains(".fj extension"));
    }

    #[test]
    fn debug_adapter_descriptor_defaults() {
        let desc = DebugAdapterDescriptor::default();
        assert_eq!(desc.id, "fj-debug");
        assert_eq!(desc.label, "Fajar Lang Debug");
        assert_eq!(desc.runtime_args, vec!["debug", "--dap"]);
    }

    #[test]
    fn variable_presentation_for_types() {
        let int_pres = VariablePresentation::for_type("i64");
        assert_eq!(int_pres.kind, "data");
        assert!(int_pres.attributes.contains(&"number".to_string()));

        let str_pres = VariablePresentation::for_type("str");
        assert!(str_pres.attributes.contains(&"rawString".to_string()));

        let struct_pres = VariablePresentation::for_type("Point");
        assert_eq!(struct_pres.kind, "property");

        let array_pres = VariablePresentation::for_type("[i64; 3]");
        assert!(array_pres.attributes.contains(&"indexed".to_string()));
    }

    #[test]
    fn variable_presentation_format_value() {
        let str_var = VariableInfo::new("s", "str", VarScope::Local, "hello");
        assert_eq!(VariablePresentation::format_value(&str_var), "\"hello\"");

        let int_var = VariableInfo::new("n", "i64", VarScope::Local, "42");
        assert_eq!(VariablePresentation::format_value(&int_var), "42");

        let char_var = VariableInfo::new("c", "char", VarScope::Local, "A");
        assert_eq!(VariablePresentation::format_value(&char_var), "'A'");
    }

    #[test]
    fn debug_console_locals() {
        let state = make_debug_state();
        let mut console = DebugConsole::new();
        let output = console.execute_command("locals", &state);
        assert!(output.contains("x: i64 = 42"));
        assert!(output.contains("name: str = hello"));
    }

    #[test]
    fn debug_console_stack() {
        let state = make_debug_state();
        let mut console = DebugConsole::new();
        let output = console.execute_command("stack", &state);
        assert!(output.contains("main at test.fj:5:1"));
    }

    #[test]
    fn debug_console_print() {
        let state = make_debug_state();
        let mut console = DebugConsole::new();
        let output = console.execute_command("print x", &state);
        assert_eq!(output, "x = 42");

        let output = console.execute_command("print z", &state);
        assert!(output.contains("cannot evaluate"));
    }

    #[test]
    fn debug_console_break() {
        let state = make_debug_state();
        let mut console = DebugConsole::new();
        let output = console.execute_command("break test.fj:10", &state);
        assert!(output.contains("breakpoint set at test.fj:10"));

        let output = console.execute_command("break", &state);
        assert!(output.contains("usage"));
    }

    #[test]
    fn debug_console_history() {
        let state = make_debug_state();
        let mut console = DebugConsole::new();
        console.execute_command("locals", &state);
        console.execute_command("stack", &state);
        assert_eq!(console.history().len(), 2);
    }

    #[test]
    fn hover_info_found() {
        let state = make_debug_state();
        let info = get_hover_info("x", &state).expect("should find x");
        assert_eq!(info.variable_name, "x");
        assert_eq!(info.type_name, "i64");
        assert_eq!(info.value_preview, "42");
        assert!(info.to_tooltip().contains("**x**"));
    }

    #[test]
    fn hover_info_not_found() {
        let state = make_debug_state();
        assert!(get_hover_info("nonexistent", &state).is_none());
    }

    #[test]
    fn call_stack_presentation() {
        let mut state = DapDebugState::new();
        let f1 = DapStackFrame::new("main", DapSourceLocation::new("main.fj", 1, 1));
        let f2 = DapStackFrame::new("helper", DapSourceLocation::new("lib.fj", 20, 5));
        state.push_frame(f1);
        state.push_frame(f2);

        let entries = format_call_stack(&state);
        // Reversed: most recent first
        assert_eq!(entries.len(), 2);
        assert!(entries[0].display.contains("helper"));
        assert!(entries[1].display.contains("main"));
    }
}
