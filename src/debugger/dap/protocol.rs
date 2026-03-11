//! DAP protocol implementation for Fajar Lang.
//!
//! Defines the request/response message types and a simulated DAP server
//! that dispatches debug commands without depending on external DAP crates.
//!
//! # Protocol Flow
//!
//! ```text
//! Client                         DapServer
//!   |── Initialize ──>              |
//!   |<── Capabilities ──            |
//!   |── Launch { program } ──>      |
//!   |── SetBreakpoints ──>          |
//!   |<── BreakpointList ──          |
//!   |── Continue / Next / StepIn ──>|
//!   |── StackTrace ──>              |
//!   |<── StackTraceResult ──        |
//!   |── Variables { scope } ──>     |
//!   |<── VariablesList ──           |
//!   |── Evaluate { expr } ──>      |
//!   |<── EvalResult ──              |
//!   |── Disconnect ──>              |
//! ```

use std::collections::HashMap;

use super::{DapDebugState, DapSourceLocation, DapStackFrame, DebugError, DebugInfo, VariableInfo};

// ═══════════════════════════════════════════════════════════════════════
// DapRequest
// ═══════════════════════════════════════════════════════════════════════

/// A debug adapter protocol request from the client (IDE).
#[derive(Debug, Clone, PartialEq)]
pub enum DapRequest {
    /// Initialize the debug adapter, negotiate capabilities.
    Initialize,
    /// Launch a program for debugging.
    Launch {
        /// Path to the `.fj` program to debug.
        program: String,
    },
    /// Set breakpoints for a file.
    SetBreakpoints {
        /// Source file path.
        file: String,
        /// Line numbers to set breakpoints on.
        lines: Vec<u32>,
    },
    /// Continue execution until next breakpoint or end.
    Continue,
    /// Step to the next statement (step over calls).
    Next,
    /// Step into the next function call.
    StepIn,
    /// Step out of the current function.
    StepOut,
    /// Request the current call stack.
    StackTrace,
    /// Request scopes for a given stack frame.
    Scopes {
        /// Stack frame ID.
        frame_id: u32,
    },
    /// Request variables for a given scope reference.
    Variables {
        /// Scope/variables reference ID.
        scope_id: u32,
    },
    /// Evaluate an expression in a given stack frame.
    Evaluate {
        /// Expression source text.
        expression: String,
        /// Stack frame context for evaluation.
        frame_id: u32,
    },
    /// Disconnect from the debug session.
    Disconnect,
}

// ═══════════════════════════════════════════════════════════════════════
// DapResponse
// ═══════════════════════════════════════════════════════════════════════

/// A debug adapter protocol response from the server.
#[derive(Debug, Clone, PartialEq)]
pub enum DapResponse {
    /// Capabilities supported by this debug adapter.
    Capabilities(DapCapabilities),
    /// List of verified breakpoints after SetBreakpoints.
    BreakpointList(Vec<DapBreakpointInfo>),
    /// Stack trace result.
    StackTraceResult(Vec<DapFrameInfo>),
    /// List of scopes for a frame.
    ScopesList(Vec<DapScope>),
    /// List of variables for a scope.
    VariablesList(Vec<VariableInfo>),
    /// Result of expression evaluation.
    EvalResult(String),
    /// Generic success response.
    Success,
    /// Error response.
    Error(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Supporting types
// ═══════════════════════════════════════════════════════════════════════

/// Breakpoint information returned in responses (serializable snapshot).
#[derive(Debug, Clone, PartialEq)]
pub struct DapBreakpointInfo {
    /// Breakpoint ID.
    pub id: u32,
    /// Source file.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Whether the breakpoint was verified (resolvable).
    pub verified: bool,
}

/// Stack frame information returned in responses (serializable snapshot).
#[derive(Debug, Clone, PartialEq)]
pub struct DapFrameInfo {
    /// Frame ID.
    pub id: u32,
    /// Function/scope name.
    pub name: String,
    /// Source file.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Column number.
    pub column: u32,
}

/// A scope within a stack frame (e.g., "Locals", "Globals").
#[derive(Debug, Clone, PartialEq)]
pub struct DapScope {
    /// Scope display name.
    pub name: String,
    /// Reference ID for requesting variables.
    pub variables_ref: u32,
    /// Whether fetching variables is expensive (e.g., globals).
    pub expensive: bool,
}

/// Capabilities advertised by the debug adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct DapCapabilities {
    /// Whether the adapter supports reverse stepping.
    pub supports_step_back: bool,
    /// Whether the adapter supports expression evaluation.
    pub supports_evaluate: bool,
    /// Whether the adapter supports conditional breakpoints.
    pub supports_conditional_breakpoints: bool,
    /// Whether the adapter supports hit-count breakpoints.
    pub supports_hit_count: bool,
}

impl Default for DapCapabilities {
    fn default() -> Self {
        Self {
            supports_step_back: false,
            supports_evaluate: true,
            supports_conditional_breakpoints: true,
            supports_hit_count: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DapMessage (JSON-RPC style serialization)
// ═══════════════════════════════════════════════════════════════════════

/// A JSON-RPC style message for DAP communication.
#[derive(Debug, Clone, PartialEq)]
pub struct DapMessage {
    /// Sequence number.
    pub seq: u32,
    /// Message type: "request", "response", or "event".
    pub msg_type: String,
    /// Command name or event name.
    pub command: String,
    /// Body content as key-value pairs.
    pub body: HashMap<String, String>,
    /// Whether this is a success response.
    pub success: bool,
}

impl DapMessage {
    /// Creates a request message.
    pub fn request(seq: u32, command: &str) -> Self {
        Self {
            seq,
            msg_type: "request".to_string(),
            command: command.to_string(),
            body: HashMap::new(),
            success: true,
        }
    }

    /// Creates a success response message.
    pub fn response(seq: u32, command: &str) -> Self {
        Self {
            seq,
            msg_type: "response".to_string(),
            command: command.to_string(),
            body: HashMap::new(),
            success: true,
        }
    }

    /// Creates an error response message.
    pub fn error_response(seq: u32, command: &str, message: &str) -> Self {
        let mut body = HashMap::new();
        body.insert("error".to_string(), message.to_string());
        Self {
            seq,
            msg_type: "response".to_string(),
            command: command.to_string(),
            body,
            success: false,
        }
    }

    /// Serializes the message to a JSON-like string.
    pub fn serialize(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("\"seq\":{}", self.seq));
        parts.push(format!("\"type\":\"{}\"", self.msg_type));
        parts.push(format!("\"command\":\"{}\"", self.command));
        parts.push(format!("\"success\":{}", self.success));
        if !self.body.is_empty() {
            let body_parts: Vec<String> = self
                .body
                .iter()
                .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
                .collect();
            parts.push(format!("\"body\":{{{}}}", body_parts.join(",")));
        }
        format!("{{{}}}", parts.join(","))
    }

    /// Deserializes a message from a simplified JSON string.
    ///
    /// This is a minimal parser for testing — not a full JSON parser.
    pub fn deserialize(input: &str) -> Result<Self, DebugError> {
        let trimmed = input.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(DebugError::JsonError {
                message: "expected JSON object".to_string(),
            });
        }
        // Extract seq
        let seq = extract_number(trimmed, "seq").unwrap_or(0);
        let msg_type = extract_string(trimmed, "type").unwrap_or_default();
        let command = extract_string(trimmed, "command").unwrap_or_default();
        let success = extract_bool(trimmed, "success").unwrap_or(true);

        Ok(Self {
            seq,
            msg_type,
            command,
            body: HashMap::new(),
            success,
        })
    }
}

/// Extracts a quoted string value for a key from a JSON-like string.
fn extract_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", key);
    let start = json.find(&pattern)?;
    let value_start = start + pattern.len();
    let rest = &json[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extracts a numeric value for a key from a JSON-like string.
fn extract_number(json: &str, key: &str) -> Option<u32> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let value_start = start + pattern.len();
    let rest = &json[value_start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Extracts a boolean value for a key from a JSON-like string.
fn extract_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let value_start = start + pattern.len();
    let rest = &json[value_start..];
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DapServer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated DAP server that processes debug requests.
///
/// Maintains the debug state, debug info, and dispatches requests
/// to the appropriate handler methods.
pub struct DapServer {
    /// Current debug execution state.
    pub debug_state: DapDebugState,
    /// Debug info (source maps, variables, breakpoints).
    pub debug_info: DebugInfo,
    /// Whether the server is actively running a session.
    pub running: bool,
    /// The program path being debugged.
    program: String,
    /// Next scope reference ID for variable requests.
    next_scope_ref: u32,
}

impl DapServer {
    /// Creates a new DAP server with empty state.
    pub fn new() -> Self {
        Self {
            debug_state: DapDebugState::new(),
            debug_info: DebugInfo::new(),
            running: false,
            program: String::new(),
            next_scope_ref: 1,
        }
    }

    /// Main dispatch: handles a request and returns a response.
    pub fn handle_request(&mut self, request: DapRequest) -> DapResponse {
        match request {
            DapRequest::Initialize => self.handle_initialize(),
            DapRequest::Launch { program } => self.handle_launch_request(&program),
            DapRequest::SetBreakpoints { file, lines } => {
                self.handle_set_breakpoints_request(&file, &lines)
            }
            DapRequest::Continue => self.handle_continue(),
            DapRequest::Next => self.handle_next(),
            DapRequest::StepIn => self.handle_step_in(),
            DapRequest::StepOut => self.handle_step_out(),
            DapRequest::StackTrace => self.handle_stack_trace(),
            DapRequest::Scopes { frame_id } => self.handle_scopes(frame_id),
            DapRequest::Variables { scope_id } => self.handle_variables(scope_id),
            DapRequest::Evaluate {
                expression,
                frame_id,
            } => self.handle_evaluate_request(&expression, frame_id),
            DapRequest::Disconnect => self.handle_disconnect(),
        }
    }

    /// Returns the adapter capabilities.
    pub fn handle_initialize(&self) -> DapResponse {
        DapResponse::Capabilities(DapCapabilities::default())
    }

    /// Loads a program and pauses at entry.
    fn handle_launch_request(&mut self, program: &str) -> DapResponse {
        self.program = program.to_string();
        self.debug_state = DapDebugState::new();
        self.debug_state.pause("entry");
        self.running = true;

        // Create an initial "main" stack frame
        let frame = DapStackFrame::new("main", DapSourceLocation::new(program, 1, 1));
        self.debug_state.push_frame(frame);
        DapResponse::Success
    }

    /// Sets breakpoints for a file and returns verified breakpoint info.
    fn handle_set_breakpoints_request(&mut self, file: &str, lines: &[u32]) -> DapResponse {
        // Clear existing breakpoints for this file
        let existing_ids: Vec<u32> = self
            .debug_info
            .breakpoint_manager
            .all_breakpoints()
            .filter(|bp| bp.file == file)
            .map(|bp| bp.id)
            .collect();
        for id in existing_ids {
            self.debug_info.breakpoint_manager.clear_breakpoint(id);
        }

        // Set new breakpoints
        let mut results = Vec::new();
        for &line in lines {
            let bp = self
                .debug_info
                .breakpoint_manager
                .set_breakpoint(file, line);
            results.push(DapBreakpointInfo {
                id: bp.id,
                file: file.to_string(),
                line,
                verified: true,
            });
        }
        DapResponse::BreakpointList(results)
    }

    /// Resumes execution.
    fn handle_continue(&mut self) -> DapResponse {
        self.debug_state.resume();
        DapResponse::Success
    }

    /// Steps to the next statement (step over).
    fn handle_next(&mut self) -> DapResponse {
        self.debug_state.instruction_ptr += 1;
        self.debug_state.pause("step");
        DapResponse::Success
    }

    /// Steps into the next function call.
    fn handle_step_in(&mut self) -> DapResponse {
        self.debug_state.instruction_ptr += 1;
        self.debug_state.pause("step_in");
        DapResponse::Success
    }

    /// Steps out of the current function.
    fn handle_step_out(&mut self) -> DapResponse {
        self.debug_state.pop_frame();
        self.debug_state.pause("step_out");
        DapResponse::Success
    }

    /// Returns the current call stack as frame info.
    fn handle_stack_trace(&self) -> DapResponse {
        let frames: Vec<DapFrameInfo> = self
            .debug_state
            .call_stack
            .iter()
            .map(|f| DapFrameInfo {
                id: f.id,
                name: f.name.clone(),
                file: f.source.file.clone(),
                line: f.source.line,
                column: f.source.column,
            })
            .collect();
        DapResponse::StackTraceResult(frames)
    }

    /// Returns scopes for a given stack frame.
    fn handle_scopes(&mut self, frame_id: u32) -> DapResponse {
        let frame_exists = self.debug_state.find_frame(frame_id).is_some();
        if !frame_exists {
            return DapResponse::Error(format!("frame {frame_id} not found"));
        }

        let locals_ref = self.next_scope_ref;
        self.next_scope_ref += 1;
        let globals_ref = self.next_scope_ref;
        self.next_scope_ref += 1;

        DapResponse::ScopesList(vec![
            DapScope {
                name: "Locals".to_string(),
                variables_ref: locals_ref,
                expensive: false,
            },
            DapScope {
                name: "Globals".to_string(),
                variables_ref: globals_ref,
                expensive: true,
            },
        ])
    }

    /// Returns variables for a given scope reference.
    fn handle_variables(&self, scope_id: u32) -> DapResponse {
        // Return locals from the current frame if scope matches
        if let Some(frame) = self.debug_state.current_frame() {
            if scope_id > 0 {
                return DapResponse::VariablesList(frame.locals.clone());
            }
        }
        DapResponse::VariablesList(Vec::new())
    }

    /// Evaluates an expression in the context of a stack frame.
    fn handle_evaluate_request(&self, expression: &str, frame_id: u32) -> DapResponse {
        // Look up the variable in the frame's locals
        if let Some(frame) = self.debug_state.find_frame(frame_id) {
            if let Some(var) = frame.find_local(expression) {
                return DapResponse::EvalResult(var.value.clone());
            }
        }
        // If not a simple variable lookup, return an error
        DapResponse::Error(format!("cannot evaluate '{expression}'"))
    }

    /// Disconnects the debug session.
    fn handle_disconnect(&mut self) -> DapResponse {
        self.running = false;
        self.debug_state = DapDebugState::new();
        DapResponse::Success
    }
}

impl Default for DapServer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::dap::VarScope;

    #[test]
    fn initialize_returns_capabilities() {
        let server = DapServer::new();
        let resp = server.handle_initialize();
        match resp {
            DapResponse::Capabilities(caps) => {
                assert!(caps.supports_evaluate);
                assert!(caps.supports_conditional_breakpoints);
                assert!(caps.supports_hit_count);
                assert!(!caps.supports_step_back);
            }
            other => panic!("expected Capabilities, got {other:?}"),
        }
    }

    #[test]
    fn launch_pauses_at_entry() {
        let mut server = DapServer::new();
        let resp = server.handle_request(DapRequest::Launch {
            program: "test.fj".to_string(),
        });
        assert_eq!(resp, DapResponse::Success);
        assert!(server.running);
        assert!(!server.debug_state.running);
        assert_eq!(server.debug_state.paused_reason.as_deref(), Some("entry"));
        assert_eq!(server.debug_state.depth(), 1);
    }

    #[test]
    fn set_breakpoints_returns_verified() {
        let mut server = DapServer::new();
        let resp = server.handle_request(DapRequest::SetBreakpoints {
            file: "main.fj".to_string(),
            lines: vec![5, 10, 15],
        });
        match resp {
            DapResponse::BreakpointList(bps) => {
                assert_eq!(bps.len(), 3);
                assert_eq!(bps[0].line, 5);
                assert_eq!(bps[1].line, 10);
                assert_eq!(bps[2].line, 15);
                assert!(bps.iter().all(|b| b.verified));
            }
            other => panic!("expected BreakpointList, got {other:?}"),
        }
    }

    #[test]
    fn continue_resumes_execution() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });
        assert!(!server.debug_state.running);

        server.handle_request(DapRequest::Continue);
        assert!(server.debug_state.running);
        assert!(server.debug_state.paused_reason.is_none());
    }

    #[test]
    fn next_increments_instruction_ptr() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });
        assert_eq!(server.debug_state.instruction_ptr, 0);

        server.handle_request(DapRequest::Next);
        assert_eq!(server.debug_state.instruction_ptr, 1);
        assert_eq!(server.debug_state.paused_reason.as_deref(), Some("step"));
    }

    #[test]
    fn step_in_increments_and_pauses() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });
        server.handle_request(DapRequest::StepIn);
        assert_eq!(server.debug_state.instruction_ptr, 1);
        assert_eq!(server.debug_state.paused_reason.as_deref(), Some("step_in"));
    }

    #[test]
    fn step_out_pops_frame() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });
        // Push a second frame
        let inner = DapStackFrame::new("inner", DapSourceLocation::new("t.fj", 5, 1));
        server.debug_state.push_frame(inner);
        assert_eq!(server.debug_state.depth(), 2);

        server.handle_request(DapRequest::StepOut);
        assert_eq!(server.debug_state.depth(), 1);
    }

    #[test]
    fn stack_trace_returns_frames() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "main.fj".to_string(),
        });

        let resp = server.handle_request(DapRequest::StackTrace);
        match resp {
            DapResponse::StackTraceResult(frames) => {
                assert_eq!(frames.len(), 1);
                assert_eq!(frames[0].name, "main");
                assert_eq!(frames[0].file, "main.fj");
            }
            other => panic!("expected StackTraceResult, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_finds_local_variable() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });

        // Add a local to the main frame
        let frame_id = server.debug_state.current_frame().map(|f| f.id).unwrap();
        server
            .debug_state
            .current_frame_mut()
            .unwrap()
            .add_local(VariableInfo::new("x", "i64", VarScope::Local, "42"));

        let resp = server.handle_request(DapRequest::Evaluate {
            expression: "x".to_string(),
            frame_id,
        });
        assert_eq!(resp, DapResponse::EvalResult("42".to_string()));

        // Non-existent variable
        let resp = server.handle_request(DapRequest::Evaluate {
            expression: "z".to_string(),
            frame_id,
        });
        match resp {
            DapResponse::Error(msg) => assert!(msg.contains("cannot evaluate")),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn disconnect_resets_state() {
        let mut server = DapServer::new();
        server.handle_request(DapRequest::Launch {
            program: "t.fj".to_string(),
        });
        assert!(server.running);

        server.handle_request(DapRequest::Disconnect);
        assert!(!server.running);
        assert_eq!(server.debug_state.depth(), 0);
    }

    #[test]
    fn dap_message_serialization_roundtrip() {
        let msg = DapMessage::request(1, "initialize");
        let serialized = msg.serialize();
        assert!(serialized.contains("\"seq\":1"));
        assert!(serialized.contains("\"command\":\"initialize\""));

        let deserialized = DapMessage::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.seq, 1);
        assert_eq!(deserialized.command, "initialize");
        assert_eq!(deserialized.msg_type, "request");
    }
}
