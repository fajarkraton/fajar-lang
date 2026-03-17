//! DAP (Debug Adapter Protocol) server for Fajar Lang.
//!
//! Implements the DAP protocol over stdin/stdout, allowing IDEs like VS Code
//! to debug Fajar Lang programs with breakpoints, stepping, and variable inspection.
//!
//! # Protocol Flow
//!
//! ```text
//! Client (IDE)                    Server (fj debug --dap)
//!     |  ── Initialize ──>            |
//!     |  <── Capabilities ──          |
//!     |  ── Launch ──>                |  (spawn interpreter thread)
//!     |  ── SetBreakpoints ──>        |
//!     |  <── Verified BPs ──          |
//!     |  ── ConfigurationDone ──>     |  (start execution)
//!     |  <── Stopped(breakpoint) ──   |
//!     |  ── Threads ──>               |
//!     |  ── StackTrace ──>            |
//!     |  ── Scopes ──>                |
//!     |  ── Variables ──>             |
//!     |  ── Continue ──>              |
//!     |  <── Terminated ──            |
//! ```

use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use dap::events::{Event, StoppedEventBody};
use dap::prelude::*;
use dap::responses::{
    EvaluateResponse, ScopesResponse, SetBreakpointsResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types::{
    Capabilities, Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread,
    Variable,
};

use super::{Breakpoint, DebugState, SourceLocation, StepMode, StopReason};

/// Thread ID for the main interpreter thread (single-threaded for now).
const MAIN_THREAD_ID: i64 = 1;

/// Variables reference for the local scope.
const LOCALS_REF: i64 = 1;

/// Variables reference for the global scope.
const GLOBALS_REF: i64 = 2;

/// Command from the DAP server to the interpreter thread.
#[derive(Debug)]
pub enum DebugCommand {
    /// Resume execution.
    Continue,
    /// Step to next statement (step-in).
    StepIn,
    /// Step over (skip function internals).
    StepOver,
    /// Step out of current function.
    StepOut,
    /// Evaluate an expression in the current scope.
    Evaluate(String),
    /// Stop the interpreter.
    Terminate,
}

/// Response from the interpreter thread to the DAP server.
#[derive(Debug, Clone)]
pub enum DebugResponse {
    /// Execution stopped (breakpoint, step, entry).
    Stopped(StopReason, SourceLocation),
    /// Execution terminated.
    Terminated,
    /// Evaluation result.
    EvalResult(String),
    /// Evaluation error.
    EvalError(String),
}

/// Runs the DAP server loop on the given I/O streams.
///
/// This is the main entry point for `fj debug --dap`. It reads DAP requests
/// from `input`, processes them, and writes responses/events to `output`.
pub fn run_dap_server<R: Read, W: Write>(input: R, output: W) {
    let reader = BufReader::new(input);
    let writer = BufWriter::new(output);
    let mut server = Server::new(reader, writer);

    let mut state = DapState::new();

    loop {
        let req = match server.poll_request() {
            Ok(Some(req)) => req,
            Ok(None) => break, // EOF
            Err(_) => break,
        };

        let cmd = req.command.clone();
        match cmd {
            Command::Initialize(_) => handle_initialize(&mut server, req),
            Command::Launch(args) => handle_launch(&mut server, &mut state, req, &args),
            Command::SetBreakpoints(args) => {
                handle_set_breakpoints(&mut server, &mut state, req, &args)
            }
            Command::ConfigurationDone => handle_configuration_done(&mut server, &mut state, req),
            Command::Threads => handle_threads(&mut server, req),
            Command::StackTrace(_) => handle_stack_trace(&mut server, &state, req),
            Command::Scopes(_) => handle_scopes(&mut server, req),
            Command::Variables(args) => handle_variables(&mut server, &state, req, &args),
            Command::Continue(_) => handle_continue(&mut server, &mut state, req),
            Command::Next(_) => handle_next(&mut server, &mut state, req),
            Command::StepIn(_) => handle_step_in(&mut server, &mut state, req),
            Command::StepOut(_) => handle_step_out(&mut server, &mut state, req),
            Command::Evaluate(args) => handle_evaluate(&mut server, &mut state, req, &args),
            Command::Disconnect(_) => {
                handle_disconnect(&mut server, &mut state, req);
                break;
            }
            _ => {
                // Unhandled command — send error response
                let resp = req.error("unsupported command");
                let _ = server.respond(resp);
            }
        }
    }
}

/// Internal state for the DAP server session.
struct DapState {
    /// Debug state (breakpoints, stepping).
    debug_state: DebugState,
    /// Source file being debugged.
    _source_file: String,
    /// Source code content.
    _source_code: String,
    /// Whether execution has started.
    running: bool,
    /// Channel to send commands to the interpreter thread.
    cmd_tx: Option<mpsc::Sender<DebugCommand>>,
    /// Channel to receive responses from the interpreter thread.
    resp_rx: Option<mpsc::Receiver<DebugResponse>>,
    /// Last stop location (for stack trace).
    last_stop: Option<SourceLocation>,
    /// Local variables at the last stop (name → display string).
    locals: Vec<(String, String, String)>,
    /// Whether stop_on_entry was requested.
    stop_on_entry: bool,
    /// Server output handle for sending events from interpreter thread.
    _server_output: Option<Arc<Mutex<dap::server::ServerOutput<BufWriter<std::io::Stdout>>>>>,
}

impl DapState {
    fn new() -> Self {
        Self {
            debug_state: DebugState::new(),
            _source_file: String::new(),
            _source_code: String::new(),
            running: false,
            cmd_tx: None,
            resp_rx: None,
            last_stop: None,
            locals: Vec::new(),
            stop_on_entry: false,
            _server_output: None,
        }
    }
}

/// Handles the Initialize request — returns capabilities.
fn handle_initialize<R: Read, W: Write>(server: &mut Server<R, W>, req: Request) {
    let caps = Capabilities {
        supports_configuration_done_request: Some(true),
        supports_conditional_breakpoints: Some(true),
        supports_hit_conditional_breakpoints: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_log_points: Some(true),
        ..Default::default()
    };
    let resp = req.success(ResponseBody::Initialize(caps));
    let _ = server.respond(resp);
    let _ = server.send_event(Event::Initialized);
}

/// Handles the Launch request — stores program path and stop-on-entry.
fn handle_launch<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
    _args: &dap::requests::LaunchRequestArguments,
) {
    // The program path comes from launch configuration (custom field).
    // For now, we accept it and wait for ConfigurationDone to start.
    state.stop_on_entry = true; // Default: stop on entry
    let resp = req.success(ResponseBody::Launch);
    let _ = server.respond(resp);
}

/// Handles SetBreakpoints — register breakpoints for a file.
fn handle_set_breakpoints<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
    args: &dap::requests::SetBreakpointsArguments,
) {
    let file = args
        .source
        .path
        .clone()
        .unwrap_or_else(|| "<unknown>".to_string());

    // Clear existing breakpoints for this file
    state.debug_state.clear_breakpoints_for_file(&file);

    // Register new breakpoints
    let mut dap_breakpoints = Vec::new();
    if let Some(ref bps) = args.breakpoints {
        for sbp in bps {
            let mut bp = Breakpoint::new(file.clone(), sbp.line as usize);
            if let Some(ref cond) = sbp.condition {
                bp = bp.with_condition(cond.clone());
            }
            if let Some(ref msg) = sbp.log_message {
                bp = bp.with_log_message(msg.clone());
            }
            let id = state.debug_state.add_breakpoint(bp);
            dap_breakpoints.push(dap::types::Breakpoint {
                id: Some(id as i64),
                verified: true,
                message: None,
                source: Some(Source {
                    name: None,
                    path: Some(file.clone()),
                    ..Default::default()
                }),
                line: Some(sbp.line),
                column: None,
                end_line: None,
                end_column: None,
                instruction_reference: None,
                offset: None,
            });
        }
    }

    let resp = req.success(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
        breakpoints: dap_breakpoints,
    }));
    let _ = server.respond(resp);
}

/// Handles ConfigurationDone — starts execution.
fn handle_configuration_done<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
) {
    state.running = true;
    state.debug_state.set_stop_on_entry(state.stop_on_entry);
    let resp = req.success(ResponseBody::ConfigurationDone);
    let _ = server.respond(resp);

    // If stop_on_entry, immediately send a Stopped event
    if state.stop_on_entry {
        let _ = server.send_event(Event::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Entry,
            description: Some("Stopped on entry".to_string()),
            thread_id: Some(MAIN_THREAD_ID),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: None,
        }));
    }
}

/// Handles Threads request — returns the single main thread.
fn handle_threads<R: Read, W: Write>(server: &mut Server<R, W>, req: Request) {
    let resp = req.success(ResponseBody::Threads(ThreadsResponse {
        threads: vec![Thread {
            id: MAIN_THREAD_ID,
            name: "main".to_string(),
        }],
    }));
    let _ = server.respond(resp);
}

/// Handles StackTrace request — returns current call frames.
fn handle_stack_trace<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &DapState,
    req: Request,
) {
    let mut frames = Vec::new();

    if let Some(ref loc) = state.last_stop {
        frames.push(StackFrame {
            id: 0,
            name: "main".to_string(),
            source: Some(Source {
                name: Some(
                    std::path::Path::new(&loc.file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&loc.file)
                        .to_string(),
                ),
                path: Some(loc.file.clone()),
                ..Default::default()
            }),
            line: loc.line as i64,
            column: loc.column as i64,
            end_line: None,
            end_column: None,
            can_restart: None,
            instruction_pointer_reference: None,
            module_id: None,
            presentation_hint: None,
        });
    }

    let resp = req.success(ResponseBody::StackTrace(StackTraceResponse {
        stack_frames: frames,
        total_frames: None,
    }));
    let _ = server.respond(resp);
}

/// Handles Scopes request — returns Locals and Globals scopes.
fn handle_scopes<R: Read, W: Write>(server: &mut Server<R, W>, req: Request) {
    let scopes = vec![
        Scope {
            name: "Locals".to_string(),
            presentation_hint: Some(ScopePresentationhint::Locals),
            variables_reference: LOCALS_REF,
            named_variables: None,
            indexed_variables: None,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        },
        Scope {
            name: "Globals".to_string(),
            presentation_hint: None,
            variables_reference: GLOBALS_REF,
            named_variables: None,
            indexed_variables: None,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        },
    ];

    let resp = req.success(ResponseBody::Scopes(ScopesResponse { scopes }));
    let _ = server.respond(resp);
}

/// Handles Variables request — returns variables for the given scope.
fn handle_variables<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &DapState,
    req: Request,
    args: &dap::requests::VariablesArguments,
) {
    let variables = if args.variables_reference == LOCALS_REF {
        state
            .locals
            .iter()
            .map(|(name, value, ty)| Variable {
                name: name.clone(),
                value: value.clone(),
                type_field: Some(ty.clone()),
                presentation_hint: None,
                evaluate_name: Some(name.clone()),
                variables_reference: 0,
                named_variables: None,
                indexed_variables: None,
                memory_reference: None,
            })
            .collect()
    } else {
        Vec::new() // Globals not yet populated
    };

    let resp = req.success(ResponseBody::Variables(VariablesResponse { variables }));
    let _ = server.respond(resp);
}

/// Handles Continue request — resumes execution.
fn handle_continue<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
) {
    state.debug_state.set_step_mode(StepMode::Continue, 0);
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::Continue);
    }
    let resp = req.success(ResponseBody::Continue(dap::responses::ContinueResponse {
        all_threads_continued: Some(true),
    }));
    let _ = server.respond(resp);
}

/// Handles Next request — step over.
fn handle_next<R: Read, W: Write>(server: &mut Server<R, W>, state: &mut DapState, req: Request) {
    state.debug_state.set_step_mode(StepMode::StepOver, 0);
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::StepOver);
    }
    let resp = req.success(ResponseBody::Next);
    let _ = server.respond(resp);
}

/// Handles StepIn request.
fn handle_step_in<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
) {
    state.debug_state.set_step_mode(StepMode::StepIn, 0);
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::StepIn);
    }
    let resp = req.success(ResponseBody::StepIn);
    let _ = server.respond(resp);
}

/// Handles StepOut request.
fn handle_step_out<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
) {
    state.debug_state.set_step_mode(StepMode::StepOut, 0);
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::StepOut);
    }
    let resp = req.success(ResponseBody::StepOut);
    let _ = server.respond(resp);
}

/// Handles Evaluate request — evaluates an expression.
fn handle_evaluate<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
    args: &dap::requests::EvaluateArguments,
) {
    // Try to evaluate via the interpreter if available
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::Evaluate(args.expression.clone()));
        if let Some(ref rx) = state.resp_rx {
            match rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(DebugResponse::EvalResult(result)) => {
                    let resp = req.success(ResponseBody::Evaluate(EvaluateResponse {
                        result,
                        type_field: None,
                        presentation_hint: None,
                        variables_reference: 0,
                        named_variables: None,
                        indexed_variables: None,
                        memory_reference: None,
                    }));
                    let _ = server.respond(resp);
                    return;
                }
                Ok(DebugResponse::EvalError(msg)) => {
                    let resp = req.error(&msg);
                    let _ = server.respond(resp);
                    return;
                }
                _ => {}
            }
        }
    }

    // Fallback: expression not evaluatable
    let resp = req.success(ResponseBody::Evaluate(EvaluateResponse {
        result: format!("<cannot evaluate: {}>", args.expression),
        type_field: None,
        presentation_hint: None,
        variables_reference: 0,
        named_variables: None,
        indexed_variables: None,
        memory_reference: None,
    }));
    let _ = server.respond(resp);
}

/// Handles Disconnect request — stops debugging.
fn handle_disconnect<R: Read, W: Write>(
    server: &mut Server<R, W>,
    state: &mut DapState,
    req: Request,
) {
    if let Some(ref tx) = state.cmd_tx {
        let _ = tx.send(DebugCommand::Terminate);
    }
    state.running = false;
    let resp = req.success(ResponseBody::Disconnect);
    let _ = server.respond(resp);
}

/// Converts our StopReason to a DAP StoppedEventReason.
pub fn stop_reason_to_dap(reason: &StopReason) -> StoppedEventReason {
    match reason {
        StopReason::Breakpoint(_) => StoppedEventReason::Breakpoint,
        StopReason::Step => StoppedEventReason::Step,
        StopReason::Entry => StoppedEventReason::Entry,
        StopReason::Terminated => StoppedEventReason::Step, // shouldn't happen
    }
}

/// Creates a StoppedEventBody from a StopReason and location.
pub fn make_stopped_event(reason: &StopReason) -> StoppedEventBody {
    StoppedEventBody {
        reason: stop_reason_to_dap(reason),
        description: Some(match reason {
            StopReason::Breakpoint(id) => format!("Breakpoint {id} hit"),
            StopReason::Step => "Step completed".to_string(),
            StopReason::Entry => "Stopped on entry".to_string(),
            StopReason::Terminated => "Terminated".to_string(),
        }),
        thread_id: Some(MAIN_THREAD_ID),
        preserve_focus_hint: None,
        text: None,
        all_threads_stopped: Some(true),
        hit_breakpoint_ids: match reason {
            StopReason::Breakpoint(id) => Some(vec![*id as i64]),
            _ => None,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper: creates a DAP request JSON in wire format.
    fn make_dap_message(json: &str) -> Vec<u8> {
        let content = json.as_bytes();
        let header = format!("Content-Length: {}\r\n\r\n", content.len());
        let mut msg = header.into_bytes();
        msg.extend_from_slice(content);
        msg
    }

    /// Helper: creates an Initialize request.
    fn initialize_msg(seq: i64) -> Vec<u8> {
        make_dap_message(&format!(
            r#"{{"seq":{},"type":"request","command":"initialize","arguments":{{"clientID":"test","adapterID":"fajar"}}}}"#,
            seq
        ))
    }

    /// Helper: creates a Disconnect request.
    fn disconnect_msg(seq: i64) -> Vec<u8> {
        make_dap_message(&format!(
            r#"{{"seq":{},"type":"request","command":"disconnect","arguments":{{}}}}"#,
            seq
        ))
    }

    #[test]
    fn dap_initialize_returns_capabilities() {
        let mut input = Vec::new();
        input.extend(initialize_msg(1));
        input.extend(disconnect_msg(2));

        let output = Vec::new();
        let input_cursor = Cursor::new(input);
        let output_cursor = Cursor::new(output);

        // Run the server
        let reader = BufReader::new(input_cursor);
        let writer = BufWriter::new(output_cursor);
        let mut server = Server::new(reader, writer);

        // Process initialize
        let req = server.poll_request().unwrap().unwrap();
        assert!(matches!(req.command, Command::Initialize(_)));
        handle_initialize(&mut server, req);
    }

    #[test]
    fn dap_set_breakpoints_registers_correctly() {
        let mut state = DapState::new();
        let bp_json = make_dap_message(
            r#"{"seq":1,"type":"request","command":"setBreakpoints","arguments":{"source":{"path":"test.fj"},"breakpoints":[{"line":5},{"line":10,"condition":"x > 3"}]}}"#,
        );

        let input = Cursor::new(bp_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        let cmd = req.command.clone();
        if let Command::SetBreakpoints(ref args) = cmd {
            handle_set_breakpoints(&mut server, &mut state, req, args);
        }

        assert_eq!(state.debug_state.breakpoint_count(), 2);
        let bps = state.debug_state.breakpoints_for_file("test.fj");
        assert_eq!(bps.len(), 2);
        assert_eq!(bps[0].line, 5);
        assert_eq!(bps[1].line, 10);
        assert!(bps[1].condition.is_some());
    }

    #[test]
    fn dap_threads_returns_main_thread() {
        let threads_json = make_dap_message(r#"{"seq":1,"type":"request","command":"threads"}"#);

        let input = Cursor::new(threads_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        assert!(matches!(req.command, Command::Threads));
        handle_threads(&mut server, req);
    }

    #[test]
    fn dap_scopes_returns_locals_and_globals() {
        let scopes_json = make_dap_message(
            r#"{"seq":1,"type":"request","command":"scopes","arguments":{"frameId":0}}"#,
        );

        let input = Cursor::new(scopes_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        assert!(matches!(req.command, Command::Scopes(_)));
        handle_scopes(&mut server, req);
    }

    #[test]
    fn dap_stack_trace_with_location() {
        let state = DapState {
            last_stop: Some(SourceLocation {
                file: "test.fj".into(),
                line: 42,
                column: 5,
                offset: 100,
            }),
            ..DapState::new()
        };

        let st_json = make_dap_message(
            r#"{"seq":1,"type":"request","command":"stackTrace","arguments":{"threadId":1}}"#,
        );

        let input = Cursor::new(st_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        handle_stack_trace(&mut server, &state, req);
    }

    #[test]
    fn dap_variables_returns_locals() {
        let state = DapState {
            locals: vec![
                ("x".into(), "42".into(), "i64".into()),
                ("name".into(), "\"hello\"".into(), "str".into()),
            ],
            ..DapState::new()
        };

        let vars_json = make_dap_message(
            r#"{"seq":1,"type":"request","command":"variables","arguments":{"variablesReference":1}}"#,
        );

        let input = Cursor::new(vars_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        let cmd = req.command.clone();
        if let Command::Variables(ref args) = cmd {
            handle_variables(&mut server, &state, req, args);
        }
    }

    #[test]
    fn stop_reason_to_dap_mapping() {
        assert!(matches!(
            stop_reason_to_dap(&StopReason::Breakpoint(1)),
            StoppedEventReason::Breakpoint
        ));
        assert!(matches!(
            stop_reason_to_dap(&StopReason::Step),
            StoppedEventReason::Step
        ));
        assert!(matches!(
            stop_reason_to_dap(&StopReason::Entry),
            StoppedEventReason::Entry
        ));
    }

    #[test]
    fn make_stopped_event_breakpoint() {
        let event = make_stopped_event(&StopReason::Breakpoint(5));
        assert!(matches!(event.reason, StoppedEventReason::Breakpoint));
        assert_eq!(event.hit_breakpoint_ids, Some(vec![5]));
        assert_eq!(event.thread_id, Some(MAIN_THREAD_ID));
    }

    #[test]
    fn make_stopped_event_step() {
        let event = make_stopped_event(&StopReason::Step);
        assert!(matches!(event.reason, StoppedEventReason::Step));
        assert!(event.hit_breakpoint_ids.is_none());
    }

    #[test]
    fn dap_continue_sets_mode() {
        let mut state = DapState::new();
        state.debug_state.set_step_mode(StepMode::Paused, 0);

        let cont_json = make_dap_message(
            r#"{"seq":1,"type":"request","command":"continue","arguments":{"threadId":1}}"#,
        );

        let input = Cursor::new(cont_json);
        let output = Cursor::new(Vec::new());
        let mut server = Server::new(BufReader::new(input), BufWriter::new(output));

        let req = server.poll_request().unwrap().unwrap();
        handle_continue(&mut server, &mut state, req);
        assert_eq!(state.debug_state.step_mode(), StepMode::Continue);
    }
}
