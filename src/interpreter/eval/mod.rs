//! Expression and statement evaluation.
//!
//! Core dispatch functions: `eval_expr`, `eval_stmt`, `eval_item`.
//! The interpreter is a tree-walking evaluator over the untyped AST.
//!
//! Split into submodules:
//! - `builtins.rs` — built-in function dispatch and implementations
//! - `methods.rs` — method call evaluation

mod builtins;
mod methods;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::interpreter::env::Environment;
use crate::interpreter::value::{FnValue, Value};
use crate::parser::ast::{
    BinOp, CallArg, Expr, FStringExprPart, Item, LiteralKind, Program, Stmt, UnaryOp,
};
use crate::runtime::ml::Tape;
use crate::runtime::os::OsRuntime;

/// Async HTTP GET using tokio::net::TcpStream (no external HTTP crate needed).
async fn async_http_get_impl(url: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (host, port, path) = parse_http_url(url)?;
    let addr = format!("{host}:{port}");
    let mut stream = tokio::net::TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("connect {addr}: {e}"))?;

    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("write: {e}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .map_err(|e| format!("read: {e}"))?;

    // Extract body after \r\n\r\n.
    if let Some(body_start) = response.find("\r\n\r\n") {
        Ok(response[body_start + 4..].to_string())
    } else {
        Ok(response)
    }
}

/// Async HTTP POST using tokio::net::TcpStream.
async fn async_http_post_impl(url: &str, body: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (host, port, path) = parse_http_url(url)?;
    let addr = format!("{host}:{port}");
    let mut stream = tokio::net::TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("connect {addr}: {e}"))?;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("write: {e}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .map_err(|e| format!("read: {e}"))?;

    if let Some(body_start) = response.find("\r\n\r\n") {
        Ok(response[body_start + 4..].to_string())
    } else {
        Ok(response)
    }
}

/// Parse "http://host:port/path" → (host, port, path).
fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let stripped = url
        .strip_prefix("http://")
        .ok_or_else(|| "expected http:// URL".to_string())?;
    let (host_port, path) = if let Some(slash) = stripped.find('/') {
        (&stripped[..slash], format!("/{}", &stripped[slash + 1..]))
    } else {
        (stripped, "/".to_string())
    };
    let (host, port) = if let Some(colon) = host_port.find(':') {
        let h = &host_port[..colon];
        let p = host_port[colon + 1..]
            .parse::<u16>()
            .map_err(|_| "invalid port".to_string())?;
        (h.to_string(), p)
    } else {
        (host_port.to_string(), 80)
    };
    Ok((host, port, path))
}

/// A single GUI widget created by gui_* builtins.
#[derive(Debug, Clone)]
pub struct GuiWidget {
    /// Widget type: "label", "button", "rect".
    pub kind: String,
    /// Display text (for label/button).
    pub text: String,
    /// Position and size.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Fill color (0xAARRGGBB).
    pub color: u32,
    /// Optional callback function name invoked on button click.
    pub on_click: Option<String>,
}

/// Accumulated GUI state from gui_* builtin calls.
#[derive(Debug, Clone, Default)]
pub struct GuiState {
    /// Window title.
    pub title: String,
    /// Window width.
    pub width: u32,
    /// Window height.
    pub height: u32,
    /// Widgets to render.
    pub widgets: Vec<GuiWidget>,
    /// Layout mode: "none" (manual xy), "row" (horizontal flex), "column" (vertical flex).
    pub layout_mode: String,
    /// Gap between flex-layout items in pixels.
    pub layout_gap: u32,
    /// Padding inside the flex container.
    pub layout_padding: u32,
}

/// WebSocket connection state.
///
/// With `--features websocket`: holds a real `tungstenite::WebSocket` socket.
/// Without the feature: in-memory echo simulation for testing.
struct WsConnection {
    #[allow(dead_code)]
    url: String,
    connected: bool,
    /// Simulation buffers (used when `websocket` feature is disabled).
    send_buffer: Vec<String>,
    #[cfg_attr(feature = "websocket", allow(dead_code))]
    recv_buffer: std::collections::VecDeque<String>,
    /// Real WebSocket socket (used when `websocket` feature is enabled).
    #[cfg(feature = "websocket")]
    socket:
        Option<tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>>,
}

/// Real MQTT client backed by `rumqttc` (feature-gated).
#[cfg(feature = "mqtt")]
struct RealMqttClient {
    client: rumqttc::Client,
    receiver: std::sync::mpsc::Receiver<(String, String)>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

/// MQTT client state.
///
/// With `--features mqtt`: holds a real `rumqttc::Client` + background connection thread.
/// Without the feature: in-memory broker simulation for testing.
struct MqttClientState {
    #[allow(dead_code)]
    broker_addr: String,
    connected: bool,
    subscriptions: Vec<String>,
    #[cfg(feature = "mqtt")]
    real_client: Option<RealMqttClient>,
}

/// In-memory MQTT message broker for simulation (used when `mqtt` feature is off).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "mqtt", allow(dead_code))]
struct MqttBroker {
    /// topic → list of queued messages
    topics: std::collections::HashMap<String, Vec<String>>,
    /// client_id → subscribed topics
    subscriptions: std::collections::HashMap<i64, Vec<String>>,
}

#[cfg_attr(feature = "mqtt", allow(dead_code))]
impl MqttBroker {
    fn new() -> Self {
        Self {
            topics: std::collections::HashMap::new(),
            subscriptions: std::collections::HashMap::new(),
        }
    }

    fn subscribe(&mut self, client_id: i64, topic: &str) {
        self.subscriptions
            .entry(client_id)
            .or_default()
            .push(topic.to_string());
    }

    fn publish(&mut self, topic: &str, payload: &str) {
        self.topics
            .entry(topic.to_string())
            .or_default()
            .push(payload.to_string());
    }

    fn receive(&mut self, client_id: i64) -> Option<(String, String)> {
        let subs = self.subscriptions.get(&client_id)?;
        for topic in subs.clone() {
            if let Some(messages) = self.topics.get_mut(&topic) {
                if !messages.is_empty() {
                    let msg = messages.remove(0);
                    return Some((topic, msg));
                }
            }
        }
        None
    }

    fn unsubscribe_all(&mut self, client_id: i64) {
        self.subscriptions.remove(&client_id);
    }
}

/// An HTTP server framework instance (V10 P3).
///
/// Stores routes and middleware registered by .fj code. The serving loop
/// dispatches incoming requests to the matching handler function by name.
pub struct HttpFrameworkServer {
    /// Listening port.
    pub port: u16,
    /// Registered routes: (method, pattern, handler_fn_name).
    pub routes: Vec<(String, String, String)>,
    /// Middleware function names, executed in order.
    pub middlewares: Vec<String>,
}

/// A real async operation to be executed via tokio (V10).
///
/// These are created by `async_sleep`, `async_http_get`, etc. and resolved
/// when `.await` is applied to the resulting `Value::Future`.
pub enum AsyncOperation {
    /// Sleep for the given duration.
    Sleep(std::time::Duration),
    /// HTTP GET request to the given URL.
    HttpGet(String),
    /// HTTP POST request to the given URL with body.
    HttpPost(String, String),
    /// Spawn: execute a function body as a concurrent task.
    Spawn(
        Box<crate::parser::ast::Expr>,
        crate::interpreter::env::EnvRef,
    ),
    /// Join: wait for multiple futures to complete.
    Join(Vec<u64>),
    /// Select: wait for the first future to complete.
    Select(Vec<u64>),
}

/// Simulated BLE (Bluetooth Low Energy) device.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ble", allow(dead_code))]
struct BleDevice {
    /// Device address (e.g., "AA:BB:CC:DD:EE:FF").
    addr: String,
    /// Device name.
    name: String,
    /// Whether currently connected.
    connected: bool,
    /// Characteristic data: UUID → value bytes.
    characteristics: std::collections::HashMap<String, Vec<u8>>,
}

/// Simulated BLE adapter managing scanned and connected devices.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ble", allow(dead_code))]
struct BleAdapter {
    /// Known devices from scanning.
    scanned: Vec<BleDevice>,
    /// Connected devices: handle → device.
    connected: std::collections::HashMap<i64, BleDevice>,
    /// Next connection handle.
    next_handle: i64,
}

#[cfg_attr(feature = "ble", allow(dead_code))]
impl BleAdapter {
    fn new() -> Self {
        Self {
            scanned: vec![
                BleDevice {
                    addr: "AA:BB:CC:DD:EE:01".into(),
                    name: "FajarSensor-1".into(),
                    connected: false,
                    characteristics: {
                        let mut m = std::collections::HashMap::new();
                        m.insert(
                            "00002a6e-0000-1000-8000-00805f9b34fb".into(),
                            vec![0x16, 0x09],
                        ); // temp 23.26°C
                        m.insert(
                            "00002a6f-0000-1000-8000-00805f9b34fb".into(),
                            vec![0x2C, 0x19],
                        ); // humidity 64.6%
                        m
                    },
                },
                BleDevice {
                    addr: "AA:BB:CC:DD:EE:02".into(),
                    name: "FajarActuator-1".into(),
                    connected: false,
                    characteristics: {
                        let mut m = std::collections::HashMap::new();
                        m.insert("0000ff01-0000-1000-8000-00805f9b34fb".into(), vec![0x00]); // relay off
                        m
                    },
                },
            ],
            connected: std::collections::HashMap::new(),
            next_handle: 1,
        }
    }

    fn scan(&self) -> Vec<(String, String)> {
        self.scanned
            .iter()
            .map(|d| (d.addr.clone(), d.name.clone()))
            .collect()
    }

    fn connect(&mut self, addr: &str) -> Option<i64> {
        let device = self.scanned.iter().find(|d| d.addr == addr)?.clone();
        let handle = self.next_handle;
        self.next_handle += 1;
        let mut dev = device;
        dev.connected = true;
        self.connected.insert(handle, dev);
        Some(handle)
    }

    fn read(&self, handle: i64, uuid: &str) -> Option<Vec<u8>> {
        let dev = self.connected.get(&handle)?;
        dev.characteristics.get(uuid).cloned()
    }

    fn write(&mut self, handle: i64, uuid: &str, data: Vec<u8>) -> bool {
        if let Some(dev) = self.connected.get_mut(&handle) {
            dev.characteristics.insert(uuid.to_string(), data);
            true
        } else {
            false
        }
    }

    fn disconnect(&mut self, handle: i64) {
        self.connected.remove(&handle);
    }
}

/// Default maximum recursion depth to prevent stack overflow.
/// Default recursion depth limit.
/// Debug builds: 64 (Rust stack is ~2MB, each eval frame is large).
/// Release builds: 1024 (optimized frames are smaller).
/// SQ11.7: Release mode handles 500+ statement programs.
/// Use `set_max_recursion_depth()` or `--stack-depth N` to adjust.
#[cfg(debug_assertions)]
const DEFAULT_MAX_RECURSION_DEPTH: usize = 64;
#[cfg(not(debug_assertions))]
const DEFAULT_MAX_RECURSION_DEPTH: usize = 1024;

/// A runtime error produced during interpretation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RuntimeError {
    /// RE001: Division by zero.
    #[error("RE001: division by zero")]
    DivisionByZero,

    /// RE002: Type error (e.g., adding string and int).
    #[error("RE002: type error: {0}")]
    TypeError(String),

    /// RE003: Stack overflow (recursion too deep).
    #[error("RE003: stack overflow (max recursion depth {depth})\n{backtrace}")]
    StackOverflow {
        /// The recursion depth limit that was exceeded.
        depth: usize,
        /// Call stack backtrace at the point of overflow.
        backtrace: String,
    },

    /// RE004: Undefined variable.
    #[error("RE004: undefined variable '{0}'")]
    UndefinedVariable(String),

    /// RE005: Not a function.
    #[error("RE005: '{0}' is not a function")]
    NotAFunction(String),

    /// RE006: Wrong number of arguments.
    #[error("RE006: expected {expected} arguments, got {got}")]
    ArityMismatch {
        /// Expected count.
        expected: usize,
        /// Actual count.
        got: usize,
    },

    /// RE007: Cannot assign to target.
    #[error("RE007: cannot assign to this expression")]
    InvalidAssignTarget,

    /// RE008: Unsupported operation.
    #[error("RE008: {0}")]
    Unsupported(String),

    /// RE009: Integer overflow.
    #[error("RE009: integer overflow in {op}: {lhs} {op} {rhs}")]
    IntegerOverflow {
        /// The operation that overflowed.
        op: String,
        /// Left-hand side operand.
        lhs: i64,
        /// Right-hand side operand.
        rhs: i64,
    },

    /// RE010: Index out of bounds.
    #[error("RE010: index {index} out of bounds for {collection} of length {length}")]
    IndexOutOfBounds {
        /// The index that was out of bounds.
        index: i64,
        /// The collection type ("array", "string", "tuple").
        collection: String,
        /// The collection length.
        length: usize,
    },
}

/// Control flow signals that propagate through the interpreter.
///
/// These are not errors — they represent structured control flow
/// (return, break, continue) that needs to unwind the call/loop stack.
#[derive(Debug, Clone)]
pub enum ControlFlow {
    /// A `return` statement with an optional value.
    Return(Value),
    /// A `break` statement with an optional value and optional label.
    Break(Value, Option<String>),
    /// A `continue` statement with an optional label.
    Continue(Option<String>),
    /// An algebraic effect operation was performed and needs a handler.
    /// Contains: (effect_name, op_name, arguments, resume_id).
    EffectPerformed {
        /// Effect name (e.g., `"Console"`).
        effect: String,
        /// Operation name (e.g., `"log"`).
        op: String,
        /// Evaluated argument values.
        args: Vec<Value>,
    },
}

/// Result type for interpreter operations.
///
/// `Ok(Value)` for normal evaluation, `Err` for runtime errors or control flow.
pub type EvalResult = Result<Value, EvalError>;

/// Combined error type for evaluation: runtime errors or control flow signals.
///
/// `Control` is boxed to keep the error type small (Value can contain large
/// TensorValue with `ArrayD<f64>`).
#[derive(Debug, Clone)]
pub enum EvalError {
    /// A runtime error (true error).
    Runtime(RuntimeError),
    /// A runtime error with source location attached.
    RuntimeWithSpan(RuntimeError, crate::lexer::token::Span),
    /// A control flow signal (not an error, but needs unwinding).
    Control(Box<ControlFlow>),
}

impl From<RuntimeError> for EvalError {
    fn from(e: RuntimeError) -> Self {
        EvalError::Runtime(e)
    }
}

impl From<ControlFlow> for EvalError {
    fn from(cf: ControlFlow) -> Self {
        EvalError::Control(Box::new(cf))
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Runtime(e) | EvalError::RuntimeWithSpan(e, _) => write!(f, "{e}"),
            EvalError::Control(_) => write!(f, "unexpected control flow"),
        }
    }
}

impl RuntimeError {
    /// Attach a source span to this runtime error.
    pub fn with_span(self, span: crate::lexer::token::Span) -> EvalError {
        EvalError::RuntimeWithSpan(self, span)
    }
}

impl EvalError {
    /// Extract the runtime error and optional span, if this is a runtime error.
    pub fn into_runtime(self) -> Option<(RuntimeError, Option<crate::lexer::token::Span>)> {
        match self {
            EvalError::Runtime(e) => Some((e, None)),
            EvalError::RuntimeWithSpan(e, span) => Some((e, Some(span))),
            EvalError::Control(_) => None,
        }
    }
}

impl std::error::Error for EvalError {}

/// Tree-walking interpreter for Fajar Lang.
///
/// Evaluates a parsed AST (`Program`) and produces runtime `Value`s.
/// Uses an environment chain (`crate::interpreter::env::EnvRef`) for scoping.
/// V15: A single level in the effect replay stack.
/// `(cache_entries, current_index)` where each cache entry is
/// `(effect_name, op_name, resume_value)`.
type EffectReplayLevel = (Vec<(String, String, Value)>, usize);

/// Handle to a real OS-threaded actor (V21).
///
/// Each actor runs in its own `std::thread`, receiving messages via an
/// `mpsc::Sender<Value>`. Dropping the sender signals the actor to shut down.
struct ActorHandle {
    /// Actor name (for debugging/status).
    name: String,
    /// Channel sender — messages are delivered to the actor's thread (bounded).
    tx: std::sync::mpsc::SyncSender<Value>,
    /// Thread join handle — used for graceful shutdown and supervision.
    join: Option<JoinHandle<()>>,
    /// Supervision strategy (from concurrency_v2).
    strategy: crate::concurrency_v2::actors::SupervisionStrategy,
    /// Handler function name (for restart).
    handler_fn: String,
    /// Handler function's closure environment (for restart).
    #[allow(dead_code)]
    handler_env: crate::interpreter::env::EnvRef,
}

pub struct Interpreter {
    /// The global environment.
    env: crate::interpreter::env::EnvRef,
    /// Current call depth for recursion protection.
    call_depth: usize,
    /// Maximum recursion depth (configurable, default 64).
    max_recursion_depth: usize,
    /// Call stack for backtrace on runtime errors.
    call_stack: Vec<String>,
    /// Captured output for testing (if `capture_output` is true).
    output: Vec<String>,
    /// Whether to capture print output instead of writing to stdout.
    capture_output: bool,
    /// OS runtime subsystem (memory, IRQ, syscall, port I/O).
    os: OsRuntime,
    /// Registry of impl methods: `(type_name, method_name)` → `FnValue`.
    impl_methods: HashMap<(String, String), FnValue>,
    /// Trait definitions: `trait_name` → list of method names.
    trait_defs: HashMap<String, Vec<String>>,
    /// Trait impl registry: `(trait_name, type_name)` set.
    trait_impls: HashSet<(String, String)>,
    /// Module symbol tables: `module_name` → { `symbol_name` → `Value` }.
    modules: HashMap<String, HashMap<String, Value>>,
    /// Public symbols per module: `module_name` → set of public symbol names.
    module_pub_items: HashMap<String, HashSet<String>>,
    /// Autograd computation tape for backward pass.
    tape: Tape,
    /// Gradient results from the last backward pass (TensorId → gradient).
    last_grads: HashMap<crate::runtime::ml::TensorId, ndarray::ArrayD<f64>>,
    /// Directory to resolve file-based modules from (e.g., `mod name;` → `name.fj`).
    source_dir: Option<PathBuf>,
    /// Set of module names currently being loaded (for circular dependency detection).
    loading_modules: HashSet<String>,
    /// Debug state for breakpoints and stepping (None = no debugging).
    debug_state: Option<crate::debugger::DebugState>,
    /// Source code for debug hook location tracking.
    debug_source: String,
    /// Source file name for debug hook.
    debug_file: String,
    /// Simulated GPIO pin states: pin_number → (direction: 0=in/1=out, level: 0/1).
    gpio_pins: HashMap<i64, (i64, i64)>,
    /// Simulated UART port states: port_number → (baud_rate, tx_buffer).
    uart_ports: HashMap<i64, (i64, Vec<u8>)>,
    /// Simulated PWM channel states: channel → (frequency_hz, duty_percent, enabled).
    pwm_channels: HashMap<i64, (i64, i64, bool)>,
    /// Simulated SPI bus states: bus_number → (speed_hz, rx_buffer).
    spi_buses: HashMap<i64, (i64, Vec<u8>)>,
    /// Loaded NPU model handles: model_id → model_path.
    npu_models: HashMap<i64, String>,
    /// QNN buffer store: buffer_id → QnnBuffer (for quantize/dequantize round-trip).
    qnn_buffers: HashMap<i64, crate::runtime::ml::npu::QnnBuffer>,
    /// Inference result cache: key (string) → cached result (string).
    inference_cache: HashMap<String, String>,
    /// Async task queue: task_id → (function body expr, captured env).
    async_tasks: HashMap<u64, (Box<Expr>, crate::interpreter::env::EnvRef)>,
    /// Real async operations pending execution (V10).
    async_ops: HashMap<u64, AsyncOperation>,
    /// Next async task ID.
    next_task_id: u64,
    /// Lazily-initialized tokio runtime for real async I/O operations.
    tokio_runtime: Option<tokio::runtime::Runtime>,
    /// SQLite database connection manager (TQ12.2).
    db_manager: crate::stdlib_v3::database::DbManager,
    /// Active profiling session (None = profiling disabled).
    pub profile_session: Option<crate::profiler::instrument::ProfileSession>,
    /// WebSocket connections: handle → (send_buffer, recv_buffer, connected).
    ws_connections: std::collections::HashMap<i64, WsConnection>,
    /// Next WebSocket handle ID.
    next_ws_id: i64,
    /// MQTT clients: handle → MqttClient state.
    mqtt_clients: std::collections::HashMap<i64, MqttClientState>,
    /// In-memory MQTT broker for simulation (unused when `mqtt` feature active).
    #[cfg_attr(feature = "mqtt", allow(dead_code))]
    mqtt_broker: MqttBroker,
    /// Next MQTT handle ID.
    next_mqtt_id: i64,
    /// Simulated BLE adapter for Bluetooth Low Energy operations (unused when `ble` feature active).
    #[cfg_attr(feature = "ble", allow(dead_code))]
    ble_adapter: BleAdapter,
    /// GUI state accumulated by gui_* builtins.
    gui_state: GuiState,
    /// HTTP framework servers: handle → server state.
    http_servers: HashMap<i64, HttpFrameworkServer>,
    /// Next HTTP server handle.
    next_http_server_id: i64,
    /// V12: User-defined macro expander for macro_rules! definitions.
    macro_expander: crate::macros_v12::MacroExpander,
    /// V14: Effect registry — tracks declared effects and their operations.
    effect_registry: crate::analyzer::effects::EffectRegistry,
    /// V14 EF4.9: Runtime effect usage statistics.
    effect_statistics: crate::analyzer::effects::EffectStatistics,
    /// V14: Effect handler stack depth — tracks active `handle` blocks.
    /// Each entry: (effect_name, op_name) → handler_index for quick lookup.
    effect_handler_depth: usize,
    /// V15: Stack of effect replay caches — one entry per active `handle` expression.
    /// Each entry is `(cache, index)` where `cache` holds tagged resume values and `index`
    /// tracks consumption during replay. When an effect fires, the dispatch walks the stack
    /// from innermost to outermost looking for a cached entry that matches the effect identity.
    effect_replay_stack: Vec<EffectReplayLevel>,
    /// V18: TCP connections: fd → TcpStream.
    tcp_connections: HashMap<usize, std::net::TcpStream>,
    /// V18: Next TCP file descriptor.
    next_tcp_fd: usize,
    /// V18: FFI manager for loading shared libraries and calling C functions.
    ffi_manager: crate::interpreter::ffi::FfiManager,
    /// V18: Generator yield collector — when Some, yield pushes here instead of returning.
    generator_yields: Option<Vec<Value>>,
    /// V18: User-defined macro bodies: name → Vec<(param_names, body)>.
    #[allow(clippy::type_complexity)]
    user_macros: HashMap<String, Vec<(Vec<String>, Box<Expr>)>>,
    /// V18: Channel pairs for actor-style message passing.
    channels: HashMap<
        i64,
        (
            std::sync::mpsc::Sender<Value>,
            Option<std::sync::mpsc::Receiver<Value>>,
        ),
    >,
    /// V18: Next channel ID.
    next_channel_id: i64,
    /// V20: Event log for debug recording (None = recording disabled).
    pub record_log: Option<crate::debugger_v2::recording::EventLog>,
    /// V20.5: Set of simulated builtins that have already printed a warning.
    /// Currently empty (all builtins are `[x]` as of V21.1) but kept for future use.
    #[allow(dead_code)]
    sim_warned: HashSet<String>,
    /// V20.5: Source span from the last runtime error (for diagnostic display).
    last_error_span: Option<crate::lexer::token::Span>,
    /// V20.7: Strict mode — reject simulated builtins with an error.
    strict_mode: bool,
    /// V21: Real threaded actor registry: actor_id → ActorHandle.
    actor_registry: HashMap<i64, ActorHandle>,
    /// V21: Next actor ID counter.
    next_actor_id: i64,
}

impl Interpreter {
    /// Creates a new interpreter with a fresh global environment.
    pub fn new() -> Self {
        let env = Arc::new(Mutex::new(Environment::new()));
        let mut interp = Interpreter {
            env,
            call_depth: 0,
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
            call_stack: Vec::new(),
            output: Vec::new(),
            capture_output: false,
            os: OsRuntime::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashSet::new(),
            modules: HashMap::new(),
            module_pub_items: HashMap::new(),
            tape: Tape::new(),
            last_grads: HashMap::new(),
            source_dir: None,
            loading_modules: HashSet::new(),
            debug_state: None,
            debug_source: String::new(),
            debug_file: String::new(),
            gpio_pins: HashMap::new(),
            uart_ports: HashMap::new(),
            pwm_channels: HashMap::new(),
            spi_buses: HashMap::new(),
            npu_models: HashMap::new(),
            qnn_buffers: HashMap::new(),
            inference_cache: HashMap::new(),
            async_tasks: HashMap::new(),
            async_ops: HashMap::new(),
            next_task_id: 1,
            tokio_runtime: None,
            db_manager: crate::stdlib_v3::database::DbManager::new(),
            profile_session: None,
            ws_connections: std::collections::HashMap::new(),
            next_ws_id: 1,
            mqtt_clients: std::collections::HashMap::new(),
            mqtt_broker: MqttBroker::new(),
            next_mqtt_id: 1,
            ble_adapter: BleAdapter::new(),
            gui_state: GuiState::default(),
            http_servers: HashMap::new(),
            next_http_server_id: 1,
            macro_expander: crate::macros_v12::MacroExpander::new(),
            effect_registry: crate::analyzer::effects::EffectRegistry::with_builtins(),
            effect_statistics: crate::analyzer::effects::EffectStatistics::new(),
            effect_handler_depth: 0,
            effect_replay_stack: Vec::new(),
            tcp_connections: HashMap::new(),
            next_tcp_fd: 100,
            ffi_manager: crate::interpreter::ffi::FfiManager::new(),
            generator_yields: None,
            user_macros: HashMap::new(),
            channels: HashMap::new(),
            next_channel_id: 1,
            record_log: None,
            sim_warned: HashSet::new(),
            last_error_span: None,
            strict_mode: false,
            actor_registry: HashMap::new(),
            next_actor_id: 1,
        };
        interp.register_builtins();
        interp
    }

    /// Creates an interpreter with a given environment as its root scope.
    ///
    /// Used by actor threads — each actor gets a fresh interpreter sharing
    /// the handler function's closure environment. Output goes to stdout.
    pub fn new_from_env(env: crate::interpreter::env::EnvRef) -> Self {
        let mut interp = Interpreter {
            env,
            call_depth: 0,
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
            call_stack: Vec::new(),
            output: Vec::new(),
            capture_output: false,
            os: OsRuntime::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashSet::new(),
            modules: HashMap::new(),
            module_pub_items: HashMap::new(),
            tape: Tape::new(),
            last_grads: HashMap::new(),
            source_dir: None,
            loading_modules: HashSet::new(),
            debug_state: None,
            debug_source: String::new(),
            debug_file: String::new(),
            gpio_pins: HashMap::new(),
            uart_ports: HashMap::new(),
            pwm_channels: HashMap::new(),
            spi_buses: HashMap::new(),
            npu_models: HashMap::new(),
            qnn_buffers: HashMap::new(),
            inference_cache: HashMap::new(),
            async_tasks: HashMap::new(),
            async_ops: HashMap::new(),
            next_task_id: 1,
            tokio_runtime: None,
            db_manager: crate::stdlib_v3::database::DbManager::new(),
            profile_session: None,
            ws_connections: std::collections::HashMap::new(),
            next_ws_id: 1,
            mqtt_clients: std::collections::HashMap::new(),
            mqtt_broker: MqttBroker::new(),
            next_mqtt_id: 1,
            ble_adapter: BleAdapter::new(),
            gui_state: GuiState::default(),
            http_servers: HashMap::new(),
            next_http_server_id: 1,
            macro_expander: crate::macros_v12::MacroExpander::new(),
            effect_registry: crate::analyzer::effects::EffectRegistry::with_builtins(),
            effect_statistics: crate::analyzer::effects::EffectStatistics::new(),
            effect_handler_depth: 0,
            effect_replay_stack: Vec::new(),
            tcp_connections: HashMap::new(),
            next_tcp_fd: 100,
            ffi_manager: crate::interpreter::ffi::FfiManager::new(),
            generator_yields: None,
            user_macros: HashMap::new(),
            channels: HashMap::new(),
            next_channel_id: 1,
            record_log: None,
            sim_warned: HashSet::new(),
            last_error_span: None,
            strict_mode: false,
            actor_registry: HashMap::new(),
            next_actor_id: 1,
        };
        interp.register_builtins();
        interp
    }

    /// Returns the effect usage statistics collected during execution.
    pub fn effect_stats(&self) -> &crate::analyzer::effects::EffectStatistics {
        &self.effect_statistics
    }

    /// Returns the source span from the last runtime error, if available.
    pub fn last_error_span(&self) -> Option<crate::lexer::token::Span> {
        self.last_error_span
    }

    /// Enable strict mode: simulated builtins are rejected with an error.
    pub fn set_strict_mode(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    /// Creates an interpreter that captures output (for testing).
    pub fn new_capturing() -> Self {
        let env = Arc::new(Mutex::new(Environment::new()));
        let mut interp = Interpreter {
            env,
            call_depth: 0,
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
            call_stack: Vec::new(),
            output: Vec::new(),
            capture_output: true,
            os: OsRuntime::new(),
            impl_methods: HashMap::new(),
            trait_defs: HashMap::new(),
            trait_impls: HashSet::new(),
            modules: HashMap::new(),
            module_pub_items: HashMap::new(),
            tape: Tape::new(),
            last_grads: HashMap::new(),
            source_dir: None,
            loading_modules: HashSet::new(),
            debug_state: None,
            debug_source: String::new(),
            debug_file: String::new(),
            gpio_pins: HashMap::new(),
            uart_ports: HashMap::new(),
            pwm_channels: HashMap::new(),
            spi_buses: HashMap::new(),
            npu_models: HashMap::new(),
            qnn_buffers: HashMap::new(),
            inference_cache: HashMap::new(),
            async_tasks: HashMap::new(),
            async_ops: HashMap::new(),
            next_task_id: 1,
            tokio_runtime: None,
            db_manager: crate::stdlib_v3::database::DbManager::new(),
            profile_session: None,
            ws_connections: std::collections::HashMap::new(),
            next_ws_id: 1,
            mqtt_clients: std::collections::HashMap::new(),
            mqtt_broker: MqttBroker::new(),
            next_mqtt_id: 1,
            ble_adapter: BleAdapter::new(),
            gui_state: GuiState::default(),
            http_servers: HashMap::new(),
            next_http_server_id: 1,
            macro_expander: crate::macros_v12::MacroExpander::new(),
            effect_registry: crate::analyzer::effects::EffectRegistry::with_builtins(),
            effect_statistics: crate::analyzer::effects::EffectStatistics::new(),
            effect_handler_depth: 0,
            effect_replay_stack: Vec::new(),
            tcp_connections: HashMap::new(),
            next_tcp_fd: 100,
            ffi_manager: crate::interpreter::ffi::FfiManager::new(),
            generator_yields: None,
            user_macros: HashMap::new(),
            channels: HashMap::new(),
            next_channel_id: 1,
            record_log: None,
            sim_warned: HashSet::new(),
            last_error_span: None,
            strict_mode: false,
            actor_registry: HashMap::new(),
            next_actor_id: 1,
        };
        interp.register_builtins();
        interp
    }

    /// Takes the accumulated GUI state, leaving a default in place.
    pub fn take_gui_state(&mut self) -> GuiState {
        std::mem::take(&mut self.gui_state)
    }

    /// Get or create the tokio runtime for real async I/O operations.
    fn ensure_tokio_runtime(&mut self) -> &tokio::runtime::Runtime {
        if self.tokio_runtime.is_none() {
            self.tokio_runtime = Some(
                tokio::runtime::Runtime::new().expect("failed to create tokio runtime for async"),
            );
        }
        self.tokio_runtime.as_ref().expect("runtime just created")
    }

    /// Execute a real async operation via tokio::block_on.
    fn execute_async_op(&mut self, op: AsyncOperation) -> Result<Value, RuntimeError> {
        let rt = self.ensure_tokio_runtime();
        match op {
            AsyncOperation::Sleep(dur) => {
                rt.block_on(async {
                    tokio::time::sleep(dur).await;
                });
                Ok(Value::Null)
            }
            AsyncOperation::HttpGet(url) => {
                let result = rt.block_on(async { async_http_get_impl(&url).await });
                match result {
                    Ok(body) => Ok(Value::Str(body)),
                    Err(e) => Err(RuntimeError::TypeError(format!("async_http_get: {e}"))),
                }
            }
            AsyncOperation::HttpPost(url, body) => {
                let result = rt.block_on(async { async_http_post_impl(&url, &body).await });
                match result {
                    Ok(resp) => Ok(Value::Str(resp)),
                    Err(e) => Err(RuntimeError::TypeError(format!("async_http_post: {e}"))),
                }
            }
            AsyncOperation::Spawn(body, env) => {
                // Execute the spawned task body immediately (cooperative).
                // True thread-level concurrency requires Arc<Mutex<>> refactor (V11).
                let prev_env = self.env.clone();
                self.env = env;
                let result = self.eval_expr(&body);
                self.env = prev_env;
                match result {
                    Ok(val) => Ok(val),
                    Err(e) => Err(RuntimeError::TypeError(format!("async_spawn: {e}"))),
                }
            }
            AsyncOperation::Join(task_ids) => {
                // Execute all pending tasks and collect results.
                let mut results = Vec::new();
                for tid in task_ids {
                    if let Some(op) = self.async_ops.remove(&tid) {
                        match self.execute_async_op(op) {
                            Ok(val) => results.push(val),
                            Err(e) => results.push(Value::Str(format!("error: {e}"))),
                        }
                    } else if let Some((body, env)) = self.async_tasks.remove(&tid) {
                        let prev_env = self.env.clone();
                        self.env = env;
                        match self.eval_expr(&body) {
                            Ok(val) => results.push(val),
                            Err(e) => results.push(Value::Str(format!("error: {e}"))),
                        }
                        self.env = prev_env;
                    } else {
                        results.push(Value::Null);
                    }
                }
                Ok(Value::array_from_vec(results))
            }
            AsyncOperation::Select(task_ids) => {
                // Execute tasks sequentially, return the first successful result.
                for tid in task_ids {
                    if let Some(op) = self.async_ops.remove(&tid) {
                        if let Ok(val) = self.execute_async_op(op) {
                            return Ok(val);
                        }
                    } else if let Some((body, env)) = self.async_tasks.remove(&tid) {
                        let prev_env = self.env.clone();
                        self.env = env;
                        let result = self.eval_expr(&body);
                        self.env = prev_env;
                        if let Ok(val) = result {
                            return Ok(val);
                        }
                    }
                }
                Ok(Value::Null)
            }
        }
    }

    /// Enables profiling for this interpreter session.
    ///
    /// After execution, inspect `self.profile_session` for results.
    pub fn enable_profiling(&mut self) {
        self.profile_session = Some(crate::profiler::instrument::ProfileSession::new());
    }

    /// Attaches a debug state to enable debugging (breakpoints, stepping).
    pub fn set_debug_state(&mut self, state: crate::debugger::DebugState) {
        self.debug_state = Some(state);
    }

    /// V20.5: Print one-time warning for simulated builtin.
    /// List of builtin names that are simulated (not backed by real hardware/threading).
    const SIMULATED_BUILTINS: &'static [&'static str] = &[
        // V21.1: All builtins are now production [x].
        // const_alloc creates correct ConstAllocation descriptors;
        // .rodata placement handled by codegen @section infrastructure.
    ];

    /// Check if a builtin name is simulated.
    pub fn is_simulated(name: &str) -> bool {
        Self::SIMULATED_BUILTINS.contains(&name)
    }

    #[allow(dead_code)]
    fn warn_simulated(&mut self, name: &str) {
        if !self.sim_warned.contains(name) {
            self.sim_warned.insert(name.to_string());
            if self.strict_mode {
                // In strict mode, simulated builtins are recorded but execution continues
                // (the error is returned by the builtin dispatch)
                return;
            }
            if !self.capture_output {
                eprintln!("[sim] {name}() is simulated — underlying mechanism is not real");
            }
        }
    }

    /// V20: Enables event recording for debug record/replay.
    pub fn enable_recording(&mut self) {
        self.record_log = Some(crate::debugger_v2::recording::EventLog::new());
    }

    /// V20: Records a function entry event if recording is enabled.
    pub fn record_fn_entry(&mut self, name: &str) {
        if let Some(ref mut log) = self.record_log {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            log.record(
                ts,
                0,
                crate::debugger_v2::recording::EventKind::FnEntry {
                    name: name.to_string(),
                    location: String::new(),
                },
            );
        }
    }

    /// V20: Records a function exit event if recording is enabled.
    pub fn record_fn_exit(&mut self, name: &str, return_val: Option<&str>) {
        if let Some(ref mut log) = self.record_log {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            log.record(
                ts,
                0,
                crate::debugger_v2::recording::EventKind::FnExit {
                    name: name.to_string(),
                    return_value: return_val.map(|s| s.to_string()),
                },
            );
        }
    }

    /// V20: Records an output (println) event if recording is enabled.
    pub fn record_output(&mut self, text: &str) {
        if let Some(ref mut log) = self.record_log {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            log.record(
                ts,
                0,
                crate::debugger_v2::recording::EventKind::IoOp {
                    op: crate::debugger_v2::recording::IoOpKind::StdoutWrite,
                    data: text.as_bytes().to_vec(),
                },
            );
        }
    }

    /// Returns a mutable reference to the debug state (if debugging is enabled).
    pub fn debug_state_mut(&mut self) -> Option<&mut crate::debugger::DebugState> {
        self.debug_state.as_mut()
    }

    /// Returns the current call depth (for debug step-over/step-out).
    pub fn call_depth(&self) -> usize {
        self.call_depth
    }

    /// Sets the maximum recursion depth (default: 64).
    pub fn set_max_recursion_depth(&mut self, depth: usize) {
        self.max_recursion_depth = depth;
    }

    /// Returns the current call stack (function names, innermost last).
    pub fn get_call_stack(&self) -> &[String] {
        &self.call_stack
    }

    /// Formats the call stack as a human-readable backtrace string.
    fn format_backtrace(&self) -> String {
        if self.call_stack.is_empty() {
            return String::from("  <empty call stack>");
        }
        let max_frames = 16;
        let head_frames = 3;
        let total = self.call_stack.len();
        let mut lines = Vec::new();
        lines.push("backtrace (most recent call last):".to_string());
        if total > max_frames {
            // Show first `head_frames` + last `(max_frames - head_frames)` frames
            let tail_count = max_frames - head_frames;
            for (i, name) in self.call_stack.iter().take(head_frames).enumerate() {
                lines.push(format!("  {:>3}: {name}()", i));
            }
            let skip = total - tail_count;
            lines.push(format!(
                "  ... {skip} frames total ({} omitted) ...",
                skip - head_frames
            ));
            for (i, name) in self.call_stack.iter().skip(skip).enumerate() {
                lines.push(format!("  {:>3}: {name}()", skip + i));
            }
        } else {
            for (i, name) in self.call_stack.iter().enumerate() {
                lines.push(format!("  {:>3}: {name}()", i));
            }
        }
        lines.join("\n")
    }

    /// Sets the source directory for resolving file-based modules.
    pub fn set_source_dir(&mut self, dir: PathBuf) {
        self.source_dir = Some(dir);
    }

    /// Returns captured output lines.
    pub fn get_output(&self) -> &[String] {
        &self.output
    }

    /// Registers built-in functions in the global environment.
    fn register_builtins(&mut self) {
        let mut all = Vec::with_capacity(320);
        all.extend(Self::core_builtins());
        all.extend(Self::ml_builtins());
        all.extend(Self::os_builtins());
        all.extend(Self::hal_builtins());
        all.extend(Self::hw_device_builtins());
        all.extend(Self::x86_builtins());
        all.extend(Self::storage_net_builtins());
        all.extend(Self::display_process_builtins());
        all.extend(Self::gui_builtins());
        all.extend(Self::regex_builtins());
        all.extend(Self::async_builtins());
        all.extend(Self::http_framework_builtins());

        for name in &all {
            self.env
                .lock()
                .expect("env lock")
                .define(name.to_string(), Value::BuiltinFn(name.to_string()));
        }

        // Register Option/Result constructors
        self.env.lock().expect("env lock").define(
            "None".to_string(),
            Value::Enum {
                variant: "None".to_string(),
                data: None,
            },
        );
        self.env
            .lock()
            .expect("env lock")
            .define("Some".to_string(), Value::BuiltinFn("Some".to_string()));
        self.env
            .lock()
            .expect("env lock")
            .define("Ok".to_string(), Value::BuiltinFn("Ok".to_string()));
        self.env
            .lock()
            .expect("env lock")
            .define("Err".to_string(), Value::BuiltinFn("Err".to_string()));

        // Math constants
        self.env
            .lock()
            .expect("env lock")
            .define("PI".to_string(), Value::Float(std::f64::consts::PI));
        self.env
            .lock()
            .expect("env lock")
            .define("E".to_string(), Value::Float(std::f64::consts::E));
    }

    /// I/O, math, error, integer overflow, file I/O, collections, cache/env builtins.
    fn core_builtins() -> Vec<&'static str> {
        vec![
            "print",
            "println",
            "len",
            "type_of",
            "const_type_name",
            "const_field_names",
            "push",
            "pop",
            "to_string",
            "to_int",
            "to_float",
            "format",
            "assert",
            "assert_eq",
            // Integer overflow control
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "checked_add",
            "checked_sub",
            "checked_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
            // Error/debug
            "panic",
            "todo",
            "dbg",
            "eprint",
            "eprintln",
            // Async ecosystem (AA2)
            "join",
            "timeout",
            "spawn",
            // Math
            "abs",
            "sqrt",
            "pow",
            "log",
            "log2",
            "log10",
            "sin",
            "cos",
            "tan",
            "floor",
            "ceil",
            "round",
            "clamp",
            "min",
            "max",
            // String free functions
            "split",
            "trim",
            "contains",
            "starts_with",
            "ends_with",
            "replace",
            // File I/O
            "read_file",
            "read_file_text",
            "write_file",
            "append_file",
            "read_binary",
            "write_binary",
            "run_command",
            "file_exists",
            // MNIST builtins
            "mnist_load_images",
            "mnist_load_labels",
            // GPU builtins
            "thread_idx",
            "block_idx",
            "block_dim",
            "grid_dim",
            "gpu_sync",
            // Collections
            "map_new",
            "map_insert",
            "map_get",
            "map_get_or",
            "map_remove",
            "map_contains_key",
            "map_keys",
            "map_values",
            "map_len",
            // Cache / file utilities
            "cache_set",
            "cache_get",
            "cache_clear",
            "file_size",
            "dir_list",
            "env_var",
        ]
    }

    /// Tensor, activations, loss, autograd, optimizer, layer, metrics, model export builtins.
    fn ml_builtins() -> Vec<&'static str> {
        vec![
            // Tensor creation
            "tensor_zeros",
            "tensor_ones",
            "tensor_randn",
            "zeros",
            "ones",
            "randn",
            "tensor_rand",
            "tensor_eye",
            "tensor_full",
            "tensor_from_data",
            // Tensor info/reshape
            "tensor_shape",
            "tensor_reshape",
            "tensor_numel",
            // Tensor arithmetic
            "tensor_add",
            "tensor_sub",
            "tensor_mul",
            "tensor_div",
            "tensor_neg",
            "tensor_matmul",
            "tensor_transpose",
            "tensor_flatten",
            "tensor_squeeze",
            "tensor_unsqueeze",
            // Tensor reduction
            "tensor_sum",
            "tensor_mean",
            "tensor_max",
            "tensor_min",
            "tensor_argmax",
            // Tensor generation
            "tensor_arange",
            "tensor_linspace",
            "tensor_xavier",
            "tensor_free",
            "tensor_rows",
            "tensor_cols",
            "tensor_set",
            "tensor_row",
            "tensor_normalize",
            "tensor_scale",
            // Activations
            "tensor_relu",
            "tensor_sigmoid",
            "tensor_tanh",
            "tensor_softmax",
            "tensor_gelu",
            "tensor_leaky_relu",
            // Loss functions
            "tensor_mse_loss",
            "tensor_cross_entropy",
            "tensor_bce_loss",
            "tensor_l1_loss",
            // Short aliases (match native codegen names)
            "matmul",
            "relu",
            "sigmoid",
            "tanh",
            "softmax",
            "gelu",
            "leaky_relu",
            "argmax",
            "transpose",
            "flatten",
            "concat",
            "xavier",
            "from_data",
            "shape",
            "reshape",
            "eye",
            "mse_loss",
            "cross_entropy_loss",
            "cross_entropy",
            "accuracy",
            "quantize_int8",
            "quantize",
            "dequantize",
            "quantized_bits",
            "quantized_shape",
            "quantized_scale",
            "quantized_numel",
            "quantized_size_bytes",
            // Hadamard
            "hadamard",
            "hadamard_inverse",
            "hadamard_quantize",
            "matmul_quantized",
            "kv_cache_create",
            "kv_cache_update",
            "kv_cache_get_keys",
            "kv_cache_get_values",
            "kv_cache_len",
            "kv_cache_size_bytes",
            // v3 Phase A: per-axis stats + quant modes
            "var_axis",
            "std_axis",
            "kurtosis_axis",
            "skewness_axis",
            "abs_max_axis",
            "channel_cv",
            "svd_ratio",
            "select_dim",
            "topk_indices",
            "quantize_per_channel",
            "quantize_residual",
            "quantize_asymmetric",
            // Calibration (B5.L3)
            "load_calibration",
            "save_calibration",
            "verify_orthogonal",
            // Autograd
            "tensor_backward",
            "backward",
            "tensor_grad",
            "grad",
            "tensor_requires_grad",
            "tensor_set_requires_grad",
            "set_requires_grad",
            "tensor_detach",
            "tensor_no_grad_begin",
            "tensor_no_grad_end",
            "tensor_clear_tape",
            // Optimizers
            "optimizer_sgd",
            "SGD",
            "optimizer_adam",
            "Adam",
            "optimizer_step",
            "optim_step",
            "optimizer_zero_grad",
            "zero_grad",
            // Layers
            "layer_dense",
            "Dense",
            "layer_conv2d",
            "Conv2d",
            // V18: Multi-head attention
            "MultiHeadAttention",
            "attention",
            "layer_forward",
            "forward",
            "layer_params",
            // V20: ML Advanced — Diffusion + RL
            "diffusion_create",
            "diffusion_denoise",
            "rl_agent_create",
            "rl_agent_step",
            // Metrics
            "metric_accuracy",
            "metric_precision",
            "metric_recall",
            "metric_f1_score",
            // Model export
            "model_save",
            "model_save_quantized",
            // GPU discovery
            "gpu_discover",
            // V20 Phase 4: RT Pipeline
            "pipeline_create",
            "pipeline_add_stage",
            "pipeline_run",
            // V20 Phase 5: Accelerator
            "accelerate",
            // V21: Real threaded actors
            "actor_spawn",
            "actor_send",
            "actor_supervise",
            "actor_stop",
            "actor_status",
            // V20 Phase 7: Const modules
            "const_alloc",
            "const_size_of",
            "const_align_of",
            // V26 A3.1: wire serialize_const() from src/const_alloc.rs
            "const_serialize",
            // V26 A3.2: wire parse_nat_expr() + eval_nat() from src/const_generics.rs
            "const_eval_nat",
            // V26 A3.3: wire ConstTraitRegistry from src/const_traits.rs
            "const_trait_list",
            "const_trait_implements",
            "const_trait_resolve",
            // V20.5 Tier 4: New tensor/scalar ops
            "sign",
            "argmin",
            "norm",
            "dot",
            "exp_tensor",
            "log_tensor",
            "sqrt_tensor",
            "abs_tensor",
            "exp",
            "gamma",
            "clamp_tensor",
            "where_tensor",
            // FajarQuant Phase 1: TurboQuant
            "turboquant_create",
            "turboquant_encode",
            "turboquant_decode",
            "turboquant_inner_product",
            // FajarQuant Phase 2: Adaptive
            "fajarquant_compare",
            // FajarQuant Phase 3: Fused attention
            "gpu_fq_codebook_dot",
            "fq_kv_cache_create",
            "fq_kv_cache_append",
            "fq_fused_attention",
            // FajarQuant Phase 4: Hierarchical
            "fq_schedule_create",
            "fq_hierarchical_stats",
            // AVX2/AES-NI (LLVM-only, interpreter returns clear error)
            "avx2_dot_f32",
            "avx2_add_f32",
            "avx2_mul_f32",
            "avx2_relu_f32",
            "aesni_encrypt_block",
            "aesni_decrypt_block",
        ]
    }

    /// mem_*, page_*, irq_*, port_*, syscall_* builtins.
    fn os_builtins() -> Vec<&'static str> {
        vec![
            "mem_alloc",
            "mem_free",
            "mem_read_u8",
            "mem_read_u32",
            "mem_read_u64",
            "mem_write_u8",
            "mem_write_u32",
            "mem_write_u64",
            "page_map",
            "page_unmap",
            "irq_register",
            "irq_unregister",
            "irq_enable",
            "irq_disable",
            "port_read",
            "port_write",
            "syscall_define",
            "syscall_dispatch",
        ]
    }

    /// Phase 3 bare-metal HAL builtins (v3.0 FajarOS).
    fn hal_builtins() -> Vec<&'static str> {
        vec![
            "gpio_config",
            "gpio_set_output",
            "gpio_set_input",
            "gpio_set_pull",
            "gpio_set_irq",
            "uart_init",
            "uart_available",
            "spi_init",
            "spi_cs_set",
            "i2c_init",
            "i2c_write",
            "i2c_read",
            "timer_get_ticks",
            "timer_get_freq",
            "timer_set_deadline",
            "timer_enable_virtual",
            "timer_disable_virtual",
            "sleep_us",
            "time_since_boot",
            "timer_mark_boot",
            "dma_alloc",
            "dma_free",
            "dma_config",
            "dma_start",
            "dma_wait",
            "dma_status",
            "dma_barrier",
        ]
    }

    /// Hardware detection, GPIO/UART/PWM/SPI (v2.0), NPU, GPU, edge AI, watchdog builtins.
    fn hw_device_builtins() -> Vec<&'static str> {
        vec![
            // Hardware detection (v1.1)
            "hw_cpu_vendor",
            "hw_cpu_arch",
            "hw_has_avx2",
            "hw_has_avx512",
            "hw_has_amx",
            "hw_has_neon",
            "hw_has_sve",
            "hw_simd_width",
            // Accelerator registry (v1.1 S4)
            "hw_gpu_count",
            "hw_npu_count",
            "hw_best_accelerator",
            // GPIO (v2.0 Q6A)
            "gpio_open",
            "gpio_close",
            "gpio_set_direction",
            "gpio_write",
            "gpio_read",
            "gpio_toggle",
            // UART (v2.0 Q6A)
            "uart_open",
            "uart_close",
            "uart_write_byte",
            "uart_read_byte",
            "uart_write_str",
            // PWM (v2.0 Q6A)
            "pwm_open",
            "pwm_close",
            "pwm_set_frequency",
            "pwm_set_duty",
            "pwm_enable",
            "pwm_disable",
            // SPI (v2.0 Q6A)
            "spi_open",
            "spi_close",
            "spi_transfer",
            "spi_write",
            // NPU (v2.0 Q6A)
            "npu_available",
            "npu_info",
            "npu_load",
            "npu_infer",
            "qnn_quantize",
            "qnn_dequantize",
            "qnn_version",
            // Timing (v2.0)
            "delay_ms",
            "delay_us",
            // GPU/OpenCL (v2.0 Q6A)
            "gpu_available",
            "gpu_info",
            "gpu_matmul",
            "gpu_add",
            "gpu_relu",
            "gpu_sigmoid",
            "gpu_mul",
            "gpu_transpose",
            "gpu_sum",
            // Edge AI / production (v2.0 Q6A)
            "cpu_temp",
            "cpu_freq",
            "mem_usage",
            "sys_uptime",
            "log_to_file",
            // Watchdog / deployment (v2.0 Q6A)
            "watchdog_start",
            "watchdog_kick",
            "watchdog_stop",
            "process_id",
            "sleep_ms",
        ]
    }

    /// x86_64 port I/O, CPUID, PIC, PIT, MSR, process scheduler builtins (FajarOS Nova).
    fn x86_builtins() -> Vec<&'static str> {
        vec![
            "port_outb",
            "port_inb",
            "x86_serial_init",
            "set_uart_mode_x86",
            "cpuid_eax",
            "cpuid_ebx",
            "cpuid_ecx",
            "cpuid_edx",
            "sse_enable",
            "read_cr0",
            "read_cr4",
            "idt_init",
            "pic_remap",
            "pic_eoi",
            "pit_init",
            "read_timer_ticks",
            "str_byte_at",
            "str_len",
            // Process scheduler (Phase 4)
            "proc_table_addr",
            "get_current_pid",
            "set_current_pid",
            "get_proc_count",
            "proc_create",
            "yield_proc",
            "tss_init",
            "syscall_init",
            "proc_create_user",
            "kb_read_scancode",
            "kb_has_data",
            "pci_read32",
            "pci_write32",
            "volatile_read_u64",
            "volatile_write_u64",
            "buffer_read_u16_le",
            "buffer_read_u32_le",
            "buffer_read_u64_le",
            "buffer_write_u16_le",
            "buffer_write_u32_le",
            "buffer_write_u64_le",
            "buffer_read_u16_be",
            "buffer_read_u32_be",
            "buffer_read_u64_be",
            "buffer_write_u16_be",
            "buffer_write_u32_be",
            "buffer_write_u64_be",
            "acpi_shutdown",
            "acpi_find_rsdp",
            "acpi_get_cpu_count",
            "rdtsc",
            "read_msr",
            "write_msr",
            "write_cr4",
            "invlpg",
            "fxsave",
            "fxrstor",
            "iretq_to_user",
            "rdrand",
            // FajarOS Nova v0.2 system builtins
            "hlt",
            "cli",
            "sti",
            "cpuid",
            "rdmsr",
            "wrmsr",
            // FajarOS Nova v0.3 Stage A: Extended Port I/O
            "port_inw",
            "port_ind",
            "port_outw",
            "port_outd",
            // FajarOS Nova v0.3 Stage A: CPU Control
            "ltr",
            "lgdt_mem",
            "lidt_mem",
            "swapgs",
            "int_n",
            "pause",
            "stac",
            "clac",
            // FajarOS Nova v0.3 Stage A: Buffer Operations
            "memcmp_buf",
            "memcpy_buf",
            "memset_buf",
        ]
    }

    /// NVMe, SD, VFS, Ethernet, network builtins (v3.0 FajarOS).
    fn storage_net_builtins() -> Vec<&'static str> {
        vec![
            // Storage (Phase 4)
            "nvme_init",
            "nvme_read",
            "nvme_write",
            "sd_init",
            "sd_read_block",
            "sd_write_block",
            "vfs_mount",
            "vfs_open",
            "vfs_read",
            "vfs_write",
            "vfs_close",
            "vfs_stat",
            // Network (Phase 5)
            "eth_init",
            "net_socket",
            "net_bind",
            "net_listen",
            "net_accept",
            "net_connect",
            "net_send",
            "net_recv",
            "net_close",
            // HTTP server (TQ12.1)
            "http_listen",
            // Database (TQ12.2)
            "db_open",
            "db_execute",
            "db_query",
            "db_close",
            "db_begin",
            "db_commit",
            "db_rollback",
            // CQ1.4 (2026-05-09): Crypto signing builtins
            "rsa_generate_2048",
            "rsa_sign",
            "rsa_verify",
            "ed25519_generate",
            "ed25519_sign",
            "ed25519_verify",
            "sha256",
            // v35.3.0 Batch 1 (2026-05-09): trivial crypto wrappers
            "sha384",
            "sha512",
            "hex_encode_str",
            "hex_decode_str",
            "base64_encode_str",
            "base64_decode_str",
            "constant_time_eq",
            "random_u64_range",
            "argon2_hash",
            "argon2_verify",
            // v35.3.0 Batch 2 (2026-05-09): MAC + KDF + RNG bytes
            "hmac_sha256",
            "hmac_sha256_verify",
            "pbkdf2_sha256",
            "hkdf_sha256",
            "random_bytes",
            // v35.3.0 Batch 3 (2026-05-09): AES-128/256 GCM + CBC
            "aes128_gcm_encrypt",
            "aes128_gcm_decrypt",
            "aes256_gcm_encrypt",
            "aes256_gcm_decrypt",
            "aes128_cbc_encrypt",
            "aes128_cbc_decrypt",
            "aes256_cbc_encrypt",
            "aes256_cbc_decrypt",
            // v35.3.0 Batch 4 (2026-05-09): X25519 key exchange
            "x25519_generate",
            // v35.3.1 (2026-05-09): X25519 DH shared-secret derivation
            "x25519_dh",
            // WebSocket builtins
            "ws_connect",
            "ws_send",
            "ws_recv",
            "ws_close",
            // MQTT builtins
            "mqtt_connect",
            "mqtt_publish",
            "mqtt_subscribe",
            "mqtt_recv",
            "mqtt_disconnect",
            "ble_scan",
            "ble_connect",
            "ble_read",
            "ble_write",
            "ble_disconnect",
        ]
    }

    /// Display, keyboard, process management, system power builtins (v3.0 FajarOS).
    fn display_process_builtins() -> Vec<&'static str> {
        vec![
            // Display & Input (Phase 6)
            "fb_init",
            "fb_write_pixel",
            "fb_fill_rect",
            "fb_width",
            "fb_height",
            "fb_set_base",
            "fb_scroll",
            "kb_init",
            "kb_read",
            "kb_available",
            // OS Services (Phase 8)
            "proc_spawn",
            "proc_wait",
            "proc_kill",
            "proc_self",
            "proc_yield",
            "sys_poweroff",
            "sys_reboot",
            "sys_cpu_temp",
            "sys_ram_total",
            "sys_ram_free",
            // V27.5 P1.2: AI scheduler
            "tensor_workload_hint",
            "schedule_ai_task",
            // V27.5 P4.2: Capability builtins
            "cap_new",
            "cap_unwrap",
            "cap_is_valid",
        ]
    }

    /// Async builtins for real I/O operations via tokio.
    fn async_builtins() -> Vec<&'static str> {
        vec![
            "async_sleep",
            "async_http_get",
            "async_http_post",
            "async_spawn",
            "async_join",
            "async_select",
        ]
    }

    /// HTTP framework builtins (V10 P3).
    fn http_framework_builtins() -> Vec<&'static str> {
        vec![
            "http_server",
            "http_route",
            "http_middleware",
            "http_start",
            "http_start_tls",
            "request_json",
            "response_json",
            // V18: Synchronous HTTP client
            "http_get",
            "http_post",
            // V18: TCP sockets
            "tcp_connect",
            "tcp_send",
            "tcp_recv",
            "tcp_close",
            // V18: DNS
            "dns_resolve",
            // V18: FFI
            "ffi_load_library",
            "ffi_register",
            "ffi_call",
            // V18: Channels (actor message passing)
            "channel_create",
            "channel_send",
            "channel_recv",
        ]
    }

    /// Regex builtins for pattern matching.
    fn regex_builtins() -> Vec<&'static str> {
        vec![
            "regex_match",
            "regex_find",
            "regex_find_all",
            "regex_replace",
            "regex_replace_all",
            "regex_captures",
        ]
    }

    /// GUI widget builtins: create windows, labels, buttons, and rectangles.
    fn gui_builtins() -> Vec<&'static str> {
        vec![
            "gui_window",
            "gui_label",
            "gui_button",
            "gui_rect",
            "gui_layout",
        ]
    }

    /// Evaluates a complete program.
    ///
    /// Processes all top-level items in order and returns the last value
    /// (or `Value::Null` if the program is empty).
    pub fn eval_program(&mut self, program: &Program) -> Result<Value, RuntimeError> {
        let mut last = Value::Null;
        for item in &program.items {
            match self.eval_item(item) {
                Ok(v) => last = v,
                Err(EvalError::Runtime(e)) => return Err(e),
                Err(EvalError::RuntimeWithSpan(e, span)) => {
                    self.last_error_span = Some(span);
                    return Err(e);
                }
                Err(EvalError::Control(cf)) if matches!(*cf, ControlFlow::Return(_)) => {
                    return Err(RuntimeError::Unsupported(
                        "return outside of function".into(),
                    ));
                }
                Err(EvalError::Control(_)) => {
                    return Err(RuntimeError::Unsupported(
                        "break/continue outside of loop".into(),
                    ));
                }
            }
        }
        Ok(last)
    }

    /// Convenience method: lex, parse, analyze, and evaluate source code.
    ///
    /// Combines tokenization, parsing, semantic analysis, and evaluation
    /// in one call. The analyzer catches type errors, undefined variables,
    /// and context violations before execution begins.
    ///
    /// Names already defined in the interpreter's environment are passed to
    /// the analyzer so that REPL-style multi-line usage works correctly.
    ///
    /// # Examples
    ///
    /// ```
    /// use fajar_lang::interpreter::Interpreter;
    ///
    /// let mut interp = Interpreter::new();
    /// let result = interp.eval_source("1 + 2").expect("eval failed");
    /// assert_eq!(format!("{result}"), "3");
    /// ```
    pub fn eval_source(&mut self, source: &str) -> Result<Value, crate::FjError> {
        let tokens = crate::lexer::tokenize(source)?;
        let program = crate::parser::parse(tokens)?;
        // Run semantic analysis with known names from the environment
        let known_names = self.env.lock().expect("env lock").all_names();
        if let Err(errors) = crate::analyzer::analyze_with_known(&program, &known_names) {
            let real_errors: Vec<_> = errors.into_iter().filter(|e| !e.is_warning()).collect();
            if !real_errors.is_empty() {
                return Err(crate::FjError::Semantic(real_errors));
            }
        }
        // Store source for debug hooks
        if self.debug_state.is_some() {
            self.debug_source = source.to_string();
        }
        self.eval_program(&program).map_err(crate::FjError::from)
    }

    /// Calls a named function with the given arguments.
    ///
    /// The function must already be defined in the global environment.
    pub fn call_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        let func = self
            .env
            .lock()
            .expect("env lock")
            .lookup(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()))?;
        match func {
            Value::Function(fv) => match self.call_function(&fv, args) {
                Ok(v) => Ok(v),
                Err(EvalError::Runtime(e)) => Err(e),
                Err(EvalError::RuntimeWithSpan(e, span)) => {
                    self.last_error_span = Some(span);
                    Err(e)
                }
                Err(EvalError::Control(_)) => Ok(Value::Null),
            },
            _ => Err(RuntimeError::NotAFunction(name.to_string())),
        }
    }

    /// Calls the `main()` function if it exists in the global scope.
    ///
    /// This is called after `eval_program` to run the program's entry point.
    /// If no `main` function is defined, this is a no-op.
    pub fn call_main(&mut self) -> Result<Value, RuntimeError> {
        let main_fn = self.env.lock().expect("env lock").lookup("main");
        match main_fn {
            Some(Value::Function(fv)) => match self.call_function(&fv, vec![]) {
                Ok(v) => Ok(v),
                Err(EvalError::Runtime(e)) => Err(e),
                Err(EvalError::RuntimeWithSpan(e, span)) => {
                    self.last_error_span = Some(span);
                    Err(e)
                }
                Err(EvalError::Control(_)) => Ok(Value::Null),
            },
            Some(_non_fn) => Err(RuntimeError::TypeError(
                "main is defined but is not a function".to_string(),
            )),
            None => Ok(Value::Null),
        }
    }

    /// Evaluates a top-level item.
    fn eval_item(&mut self, item: &Item) -> EvalResult {
        match item {
            Item::FnDef(fndef) => {
                let fn_val = FnValue {
                    name: fndef.name.clone(),
                    params: fndef.params.clone(),
                    body: fndef.body.clone(),
                    closure_env: Arc::clone(&self.env),
                    is_async: fndef.is_async,
                    is_gen: fndef.is_gen,
                    requires: fndef.requires.clone(),
                };
                self.env
                    .lock()
                    .expect("env lock")
                    .define(fndef.name.clone(), Value::Function(fn_val));
                Ok(Value::Null)
            }
            Item::StructDef(sdef) => {
                // Store struct name for later use with StructInit
                // In Phase 1, structs are duck-typed — just store the name
                self.env
                    .lock()
                    .expect("env lock")
                    .define(sdef.name.clone(), Value::Null);
                Ok(Value::Null)
            }
            Item::UnionDef(udef) => {
                self.env
                    .lock()
                    .expect("env lock")
                    .define(udef.name.clone(), Value::Null);
                Ok(Value::Null)
            }
            Item::EnumDef(edef) => {
                // Register each variant as a constructor function or value
                for variant in &edef.variants {
                    if variant.fields.is_empty() {
                        // Unit variant — register as enum value
                        self.env.lock().expect("env lock").define(
                            variant.name.clone(),
                            Value::Enum {
                                variant: variant.name.clone(),
                                data: None,
                            },
                        );
                    } else {
                        // Tuple variant — register as builtin constructor
                        self.env.lock().expect("env lock").define(
                            variant.name.clone(),
                            Value::BuiltinFn(format!("__enum_{}_{}", edef.name, variant.name)),
                        );
                    }
                }
                Ok(Value::Null)
            }
            Item::ConstDef(cdef) => {
                let val = self.eval_expr(&cdef.value)?;
                self.env
                    .lock()
                    .expect("env lock")
                    .define(cdef.name.clone(), val);
                Ok(Value::Null)
            }
            Item::StaticDef(sdef) => {
                // Static mut: define as mutable global variable
                let val = self.eval_expr(&sdef.value)?;
                self.env
                    .lock()
                    .expect("env lock")
                    .define(sdef.name.clone(), val);
                Ok(Value::Null)
            }
            Item::ServiceDef(svc) => {
                // Register each handler as a regular function
                for handler in &svc.handlers {
                    let fn_val = FnValue {
                        name: handler.name.clone(),
                        params: handler.params.clone(),
                        body: handler.body.clone(),
                        closure_env: Arc::clone(&self.env),
                        is_async: false,
                        is_gen: false,
                        requires: vec![],
                    };
                    self.env
                        .lock()
                        .expect("env lock")
                        .define(handler.name.clone(), Value::Function(fn_val));
                }
                Ok(Value::Null)
            }
            Item::Stmt(stmt) => self.eval_stmt(stmt),
            Item::ImplBlock(impl_block) => {
                self.eval_impl_block(impl_block)?;
                Ok(Value::Null)
            }
            Item::ModDecl(mod_decl) => self.eval_mod_decl(mod_decl),
            Item::UseDecl(use_decl) => self.eval_use_decl(use_decl),
            Item::TraitDef(td) => {
                // Register trait method names for dynamic dispatch (dyn Trait).
                let methods: Vec<String> = td.methods.iter().map(|m| m.name.clone()).collect();
                self.trait_defs.insert(td.name.clone(), methods);
                Ok(Value::Null)
            }
            Item::ExternFn(efn) => {
                // Register extern function as a builtin placeholder.
                // Actual dynamic loading happens via ffi.rs (S7.2).
                self.env.lock().expect("env lock").define(
                    efn.name.clone(),
                    Value::BuiltinFn(format!("__ffi_{}", efn.name)),
                );
                Ok(Value::Null)
            }
            Item::TypeAlias(_) => {
                // Type aliases are resolved at analysis time; no runtime effect.
                Ok(Value::Null)
            }
            Item::GlobalAsm(_) => {
                // Global assembly is only meaningful in native codegen; no-op in interpreter.
                Ok(Value::Null)
            }
            Item::EffectDecl(ed) => {
                // V14: Register effect declaration in interpreter's runtime registry.
                // This enables handle expressions to intercept effect operations at runtime.
                let kind = crate::analyzer::effects::effect_kind_from_name(&ed.name)
                    .unwrap_or(crate::analyzer::effects::EffectKind::State);
                let ops: Vec<crate::analyzer::effects::EffectOp> = ed
                    .operations
                    .iter()
                    .map(|op| {
                        crate::analyzer::effects::EffectOp::new(
                            op.name.clone(),
                            op.params.iter().map(|(_, ty)| format!("{ty:?}")).collect(),
                            op.return_type
                                .as_ref()
                                .map(|t| format!("{t:?}"))
                                .unwrap_or_else(|| "void".to_string()),
                        )
                    })
                    .collect();
                let decl = crate::analyzer::effects::EffectDecl::new(ed.name.clone(), kind, ops);
                // Ignore duplicate registration (analyzer already validates).
                let _ = self.effect_registry.register(decl);

                // Register effect operations as BuiltinFn in the environment.
                // When called, these raise ControlFlow::EffectPerformed to be caught
                // by the nearest enclosing `handle` expression.
                for op in &ed.operations {
                    let qualified = format!("{}::{}", ed.name, op.name);
                    self.env.lock().expect("env lock").define(
                        qualified,
                        Value::BuiltinFn(format!("__effect__{}::{}", ed.name, op.name)),
                    );
                }
                Ok(Value::Null)
            }
            Item::EffectComposition(ec) => {
                // V14: Resolve composed effect and register merged decl in runtime.
                let comp = crate::analyzer::effects::EffectComposition::new(
                    &ec.name,
                    ec.components.clone(),
                );
                match comp.resolve(&self.effect_registry) {
                    Ok(merged) => {
                        // Register all component operations under the composed name.
                        for op in &merged.operations {
                            let qualified = format!("{}::{}", ec.name, op.name);
                            self.env.lock().expect("env lock").define(
                                qualified,
                                Value::BuiltinFn(format!("__effect__{}::{}", ec.name, op.name)),
                            );
                        }
                        let _ = self.effect_registry.register(merged);
                    }
                    Err(_) => {
                        // Component effect not declared yet — skip silently.
                        // The analyzer catches this as EE002.
                    }
                }
                Ok(Value::Null)
            }
            Item::MacroRulesDef(mdef) => {
                // V18: Store user macro arms for runtime expansion
                let mut arms = Vec::new();
                for arm in &mdef.arms {
                    // Extract parameter names from pattern: ($x:expr) → ["x"]
                    // Pattern is stored as raw string with spaces between tokens,
                    // e.g. "$ x : expr" — so trim leading whitespace after splitting on $.
                    let params: Vec<String> = arm
                        .pattern
                        .split('$')
                        .skip(1)
                        .filter_map(|s| {
                            let trimmed = s.trim_start();
                            let name: String = trimmed
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            if name.is_empty() { None } else { Some(name) }
                        })
                        .collect();
                    arms.push((params, arm.body.clone()));
                }
                self.user_macros.insert(mdef.name.clone(), arms);

                // Also register in expander for compatibility
                let mut compiled = crate::macros_v12::CompiledMacro::new(&mdef.name);
                for arm in &mdef.arms {
                    compiled.add_rule(
                        vec![crate::macros_v12::TokenTree::Literal(arm.pattern.clone())],
                        vec![crate::macros_v12::TokenTree::Ident("body".into())],
                    );
                }
                self.macro_expander.register(compiled);
                Ok(Value::Null)
            }
        }
    }

    /// Evaluates a statement.
    fn eval_stmt(&mut self, stmt: &Stmt) -> EvalResult {
        // Debug hook: check breakpoints and stepping before execution
        if self.debug_state.is_some() {
            let span_start = match stmt {
                Stmt::Let { span, .. }
                | Stmt::Const { span, .. }
                | Stmt::Expr { span, .. }
                | Stmt::Return { span, .. }
                | Stmt::Break { span, .. }
                | Stmt::Continue { span, .. } => span.start,
                Stmt::Item(_) => 0, // Items don't need debug hooks
            };
            if !matches!(stmt, Stmt::Item(_)) {
                let file = self.debug_file.clone();
                let source = self.debug_source.clone();
                let depth = self.call_depth;
                if let Some(ref mut ds) = self.debug_state {
                    ds.debug_hook(&file, &source, span_start, depth);
                }
            }
        }
        match stmt {
            Stmt::Let {
                name, value, ty, ..
            } => {
                let val = self.eval_expr(value)?;
                // Coerce to trait object if type annotation is `dyn Trait`
                let val =
                    if let Some(crate::parser::ast::TypeExpr::DynTrait { trait_name, .. }) =
                        ty.as_ref()
                    {
                        self.coerce_to_trait_object(val, trait_name)?
                    } else {
                        val
                    };
                // V14 DT4 / V27.5 P4.1: Check refinement type predicate.
                if let Some(t) = ty.as_ref() {
                    self.check_refinement(t, &val, &format!("let {name}"))?;
                }
                self.env.lock().expect("env lock").define(name.clone(), val);
                Ok(Value::Null)
            }
            Stmt::Const { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.env.lock().expect("env lock").define(name.clone(), val);
                Ok(Value::Null)
            }
            Stmt::Expr { expr, .. } => self.eval_expr(expr),
            Stmt::Return { value, .. } => {
                let val = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Null,
                };
                Err(ControlFlow::Return(val).into())
            }
            Stmt::Break { label, value, .. } => {
                let val = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Null,
                };
                Err(ControlFlow::Break(val, label.clone()).into())
            }
            Stmt::Continue { label, .. } => Err(ControlFlow::Continue(label.clone()).into()),
            Stmt::Item(item) => self.eval_item(item),
        }
    }

    /// Evaluates an expression.
    pub fn eval_expr(&mut self, expr: &Expr) -> EvalResult {
        match expr {
            Expr::Literal { kind, .. } => Ok(self.eval_literal(kind)),
            Expr::Ident { name, .. } => self.eval_ident(name),
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => self.eval_binary(left, *op, right, *span),
            Expr::Unary { op, operand, .. } => self.eval_unary(*op, operand),
            Expr::Call { callee, args, span } => {
                self.eval_call(callee, args).map_err(|e| match e {
                    EvalError::Runtime(re) => re.with_span(*span),
                    other => other,
                })
            }
            Expr::Block { stmts, expr, .. } => self.eval_block(stmts, expr),
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.eval_if(condition, then_branch, else_branch),
            Expr::While {
                label,
                condition,
                body,
                ..
            } => self.eval_while(condition, body, label.as_deref()),
            Expr::For {
                label,
                variable,
                iterable,
                body,
                ..
            } => self.eval_for(variable, iterable, body, label.as_deref()),
            Expr::Loop { label, body, .. } => self.eval_loop(body, label.as_deref()),
            Expr::Assign {
                target, op, value, ..
            } => self.eval_assign(target, *op, value),
            Expr::Match { subject, arms, .. } => self.eval_match(subject, arms),
            Expr::Array { elements, .. } => self.eval_array(elements),
            Expr::ArrayRepeat { value, count, .. } => {
                let val = self.eval_expr(value)?;
                let n = match self.eval_expr(count)? {
                    Value::Int(n) => n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "array repeat count must be integer".into(),
                        )
                        .into());
                    }
                };
                Ok(Value::array_from_vec(vec![val; n]))
            }
            Expr::Tuple { elements, .. } => self.eval_tuple(elements),
            Expr::Pipe { left, right, .. } => self.eval_pipe(left, right),
            Expr::StructInit { name, fields, .. } => self.eval_struct_init(name, fields),
            Expr::Field { object, field, .. } => self.eval_field(object, field),
            Expr::Index {
                object,
                index,
                span,
            } => self.eval_index(object, index).map_err(|e| match e {
                EvalError::Runtime(re) => re.with_span(*span),
                other => other,
            }),
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => self.eval_range(start, end, *inclusive),
            Expr::Grouped { expr, .. } => self.eval_expr(expr),
            Expr::Closure { params, body, .. } => self.eval_closure(params, body),
            Expr::Path { segments, .. } => {
                // Try qualified name first (e.g., "Point::new"), then last segment
                let qualified = segments.join("::");
                if let Some(val) = self.env.lock().expect("env lock").lookup(&qualified) {
                    return Ok(val);
                }
                let name = segments.last().map_or("", |s| s.as_str());
                self.eval_ident(name)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => self.eval_method_call(receiver, method, args),
            Expr::Try { expr, .. } => self.eval_try(expr),
            Expr::Cast { expr, ty, .. } => self.eval_cast(expr, ty),
            Expr::Await { expr, .. } => {
                // Evaluate the expression — should produce a Future value.
                let val = self.eval_expr(expr)?;
                match val {
                    Value::Future { task_id } => {
                        // V10: Check if this is a real async operation first.
                        if let Some(op) = self.async_ops.remove(&task_id) {
                            return self.execute_async_op(op).map_err(EvalError::Runtime);
                        }
                        // Fallback: cooperative execution (user-defined async fn).
                        if let Some((body, task_env)) = self.async_tasks.remove(&task_id) {
                            let prev_env = self.env.clone();
                            self.env = task_env;
                            let result = self.eval_expr(&body);
                            self.env = prev_env;
                            result
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    other => Ok(other),
                }
            }
            Expr::AsyncBlock { body, .. } => {
                // Create a Future by capturing the body and environment
                let task_id = self.next_task_id;
                self.next_task_id += 1;
                let captured_env = self.env.clone();
                self.async_tasks
                    .insert(task_id, (body.clone(), captured_env));
                Ok(Value::Future { task_id })
            }
            Expr::InlineAsm { .. } => Err(RuntimeError::TypeError(
                "inline assembly is not supported in interpreter mode".into(),
            )
            .into()),
            Expr::FString { parts, .. } => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        FStringExprPart::Literal(s) => result.push_str(s),
                        FStringExprPart::Expr(expr) => {
                            let val = self.eval_expr(expr)?;
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::Str(result))
            }
            Expr::HandleEffect { body, handlers, .. } => {
                // V15: Multi-step continuation via replay with stack-based caching.
                //
                // Each `handle` expression pushes a replay cache onto a shared stack.
                // When an effect fires, the dispatch walks the stack from innermost to
                // outermost looking for a cached resume value. This correctly handles
                // nested `handle` expressions where an inner handle may not match and
                // the effect propagates to an outer handle.
                //
                // Algorithm:
                // 1. Push a new empty cache for this handle level.
                // 2. Evaluate body. On EffectPerformed:
                //    a. If handler matches: run handler, cache resume value, replay body.
                //    b. If no handler: pop cache, re-raise to outer handle.
                // 3. On body completion: pop cache, return result.

                self.effect_replay_stack.push((Vec::new(), 0));
                let stack_level = self.effect_replay_stack.len() - 1;

                let max_replays = 1000;
                let mut replay_count = 0;

                let final_result = loop {
                    replay_count += 1;
                    if replay_count > max_replays {
                        break Err(RuntimeError::Unsupported(
                            "effect handler exceeded maximum replay count (possible infinite effect loop)".into(),
                        ).into());
                    }

                    // Reset replay index for this level (keep cache entries).
                    self.effect_replay_stack[stack_level].1 = 0;

                    self.effect_handler_depth += 1;
                    let result = self.eval_expr(body);
                    self.effect_handler_depth -= 1;

                    match result {
                        Err(EvalError::Control(ref cf))
                            if matches!(**cf, ControlFlow::EffectPerformed { .. }) =>
                        {
                            let (effect, op, args) = match *cf.clone() {
                                ControlFlow::EffectPerformed { effect, op, args } => {
                                    (effect, op, args)
                                }
                                _ => unreachable!(),
                            };
                            // Record effect statistics.
                            self.effect_statistics.record_op(&effect, &op);
                            self.effect_statistics
                                .update_depth(self.effect_handler_depth);
                            // Find matching handler arm.
                            let handler = handlers
                                .iter()
                                .find(|h| h.effect_name == effect && h.op_name == op);
                            if let Some(arm) = handler {
                                // Bind parameters in a new scope.
                                let prev_env = self.env.clone();
                                let handler_env = Arc::new(Mutex::new(
                                    Environment::new_with_parent(Arc::clone(&self.env)),
                                ));
                                self.env = handler_env;
                                for (i, pname) in arm.param_names.iter().enumerate() {
                                    let val = args.get(i).cloned().unwrap_or(Value::Null);
                                    self.env
                                        .lock()
                                        .expect("env lock")
                                        .define(pname.clone(), val);
                                }
                                let handler_result = self.eval_expr(&arm.body);
                                self.env = prev_env;

                                match handler_result {
                                    Ok(resume_val) => {
                                        self.effect_statistics.record_resume();
                                        // Cache the resume value tagged with effect identity.
                                        self.effect_replay_stack[stack_level].0.push((
                                            effect.clone(),
                                            op.clone(),
                                            resume_val,
                                        ));
                                        continue;
                                    }
                                    Err(e) => break Err(e),
                                }
                            } else {
                                // No handler found — re-raise to outer handler.
                                break result;
                            }
                        }
                        other => break other,
                    }
                };

                // Pop this handle's cache.
                self.effect_replay_stack.pop();

                final_result
            }
            Expr::ResumeExpr { value, .. } => {
                // V14: Resume evaluates its argument and returns it as the
                // result that will replace the effect operation call site.
                // In the shallow handler model, this is simply the identity —
                // the handler body's return value IS the resume value.
                self.eval_expr(value)
            }
            Expr::Comptime { body, .. } => {
                // In interpreter mode, comptime blocks are evaluated eagerly
                // just like normal expressions.
                self.eval_expr(body)
            }
            Expr::MacroInvocation { name, args, .. } => {
                // Evaluate macro arguments
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                // V18: Check user-defined macros first
                if let Some(arms) = self.user_macros.get(name).cloned() {
                    // Find first arm where param count matches
                    for (params, body) in &arms {
                        if params.len() == arg_vals.len() || params.is_empty() {
                            // Bind parameters in a new scope
                            let macro_env = Arc::new(Mutex::new(Environment::new_with_parent(
                                Arc::clone(&self.env),
                            )));
                            for (param, val) in params.iter().zip(arg_vals.iter()) {
                                macro_env
                                    .lock()
                                    .expect("env lock")
                                    .define(param.clone(), val.clone());
                            }
                            let prev_env = Arc::clone(&self.env);
                            self.env = macro_env;
                            let result = self.eval_expr(body);
                            self.env = prev_env;
                            return result;
                        }
                    }
                    // No matching arm — return first arg or Null
                    return Ok(arg_vals.into_iter().next().unwrap_or(Value::Null));
                }
                // Dispatch to built-in macro handler
                match crate::macros::eval_builtin_macro(name, &arg_vals) {
                    Ok(val) => Ok(val),
                    Err(msg) => Err(RuntimeError::TypeError(msg).into()),
                }
            }
            // Yield expression in generator (V18 gen fn semantics)
            Expr::Yield { value, .. } => {
                let val = if let Some(expr) = value {
                    self.eval_expr(expr)?
                } else {
                    Value::Null
                };
                // V18: If inside a generator call, collect the yielded value
                if let Some(ref mut yields) = self.generator_yields {
                    yields.push(val);
                    return Ok(Value::Null); // Continue execution
                }
                // Outside generator — just return the value
                Ok(val)
            }
            // V19: Macro metavariable — look up in environment (bound during macro expansion)
            Expr::MacroVar { name, .. } => self
                .env
                .lock()
                .expect("env lock")
                .lookup(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(format!("${name}")).into()),
        }
    }

    /// Evaluates a literal expression.
    fn eval_literal(&self, kind: &LiteralKind) -> Value {
        match kind {
            LiteralKind::Int(v) => Value::Int(*v),
            LiteralKind::Float(v) => Value::Float(*v),
            LiteralKind::String(s) | LiteralKind::RawString(s) => Value::Str(s.clone()),
            LiteralKind::Char(c) => Value::Char(*c),
            LiteralKind::Bool(b) => Value::Bool(*b),
            LiteralKind::Null => Value::Null,
        }
    }

    /// Evaluates an identifier by looking it up in the environment.
    fn eval_ident(&self, name: &str) -> EvalResult {
        self.env
            .lock()
            .expect("env lock")
            .lookup(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()).into())
    }

    /// Evaluates a binary expression.
    fn eval_binary(
        &mut self,
        left: &Expr,
        op: BinOp,
        right: &Expr,
        span: crate::lexer::token::Span,
    ) -> EvalResult {
        // Short-circuit for logical operators
        if op == BinOp::And {
            let lv = self.eval_expr(left)?;
            if !lv.is_truthy() {
                return Ok(lv);
            }
            return self.eval_expr(right);
        }
        if op == BinOp::Or {
            let lv = self.eval_expr(left)?;
            if lv.is_truthy() {
                return Ok(lv);
            }
            return self.eval_expr(right);
        }

        let lv = self.eval_expr(left)?;
        let rv = self.eval_expr(right)?;

        // Attach source span to any runtime error from binary operations
        let attach_span = |r: EvalResult| -> EvalResult {
            r.map_err(|e| match e {
                EvalError::Runtime(re) => re.with_span(span),
                other => other,
            })
        };
        match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => attach_span(self.eval_int_binop(*a, op, *b)),
            (Value::Float(a), Value::Float(b)) => attach_span(self.eval_float_binop(*a, op, *b)),
            (Value::Int(a), Value::Float(b)) => {
                attach_span(self.eval_float_binop(*a as f64, op, *b))
            }
            (Value::Float(a), Value::Int(b)) => {
                attach_span(self.eval_float_binop(*a, op, *b as f64))
            }
            (Value::Str(a), Value::Str(b)) => self.eval_str_binop(a, op, b),
            // Pointer arithmetic: ptr + offset, offset + ptr, ptr - offset
            (Value::Pointer(addr), Value::Int(offset)) => match op {
                BinOp::Add => Ok(Value::Pointer(addr.wrapping_add(*offset as u64))),
                BinOp::Sub => Ok(Value::Pointer(addr.wrapping_sub(*offset as u64))),
                _ => self.eval_comparison(&lv, op, &rv),
            },
            (Value::Int(offset), Value::Pointer(addr)) if op == BinOp::Add => {
                Ok(Value::Pointer(addr.wrapping_add(*offset as u64)))
            }
            // V16: Array concatenation with +
            (Value::Array(a), Value::Array(b)) if op == BinOp::Add => {
                let mut result = (**a).clone();
                result.extend(b.iter().cloned());
                Ok(Value::array_from_vec(result))
            }
            // Tensor arithmetic: dispatch to tensor_binop builtins
            (Value::Tensor(_), Value::Tensor(_)) => match op {
                BinOp::Add => self.builtin_tensor_binop(vec![lv, rv], "add"),
                BinOp::Sub => self.builtin_tensor_binop(vec![lv, rv], "sub"),
                BinOp::Mul => self.builtin_tensor_binop(vec![lv, rv], "mul"),
                BinOp::Div => self.builtin_tensor_binop(vec![lv, rv], "div"),
                _ => self.eval_comparison(&lv, op, &rv),
            },
            (Value::Bool(_), Value::Bool(_)) => self.eval_comparison(&lv, op, &rv),
            _ => self.eval_comparison(&lv, op, &rv),
        }
    }

    /// Evaluates a binary operation on two integers.
    ///
    /// Overflow behavior: checked arithmetic returns RE009 on overflow.
    /// Use `wrapping_add`/`checked_add`/`saturating_add` builtins for explicit control.
    fn eval_int_binop(&self, a: i64, op: BinOp, b: i64) -> EvalResult {
        match op {
            BinOp::Add => a.checked_add(b).map(Value::Int).ok_or_else(|| {
                RuntimeError::IntegerOverflow {
                    op: "+".into(),
                    lhs: a,
                    rhs: b,
                }
                .into()
            }),
            BinOp::Sub => a.checked_sub(b).map(Value::Int).ok_or_else(|| {
                RuntimeError::IntegerOverflow {
                    op: "-".into(),
                    lhs: a,
                    rhs: b,
                }
                .into()
            }),
            BinOp::Mul => a.checked_mul(b).map(Value::Int).ok_or_else(|| {
                RuntimeError::IntegerOverflow {
                    op: "*".into(),
                    lhs: a,
                    rhs: b,
                }
                .into()
            }),
            BinOp::Div => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero.into());
                }
                a.checked_div(b).map(Value::Int).ok_or_else(|| {
                    RuntimeError::IntegerOverflow {
                        op: "/".into(),
                        lhs: a,
                        rhs: b,
                    }
                    .into()
                })
            }
            BinOp::Rem => {
                if b == 0 {
                    return Err(RuntimeError::DivisionByZero.into());
                }
                Ok(Value::Int(a % b))
            }
            BinOp::Pow => {
                if b < 0 {
                    return Ok(Value::Float((a as f64).powf(b as f64)));
                }
                a.checked_pow(b as u32).map(Value::Int).ok_or_else(|| {
                    RuntimeError::IntegerOverflow {
                        op: "**".into(),
                        lhs: a,
                        rhs: b,
                    }
                    .into()
                })
            }
            BinOp::BitAnd => Ok(Value::Int(a & b)),
            BinOp::BitOr => Ok(Value::Int(a | b)),
            BinOp::BitXor => Ok(Value::Int(a ^ b)),
            BinOp::Shl => Ok(Value::Int(a.wrapping_shl(b as u32))),
            BinOp::Shr => Ok(Value::Int(a.wrapping_shr(b as u32))),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::Le => Ok(Value::Bool(a <= b)),
            BinOp::Ge => Ok(Value::Bool(a >= b)),
            BinOp::And | BinOp::Or => {
                // Already handled by short-circuit above
                unreachable!()
            }
            BinOp::MatMul => {
                Err(RuntimeError::TypeError("matmul not supported on integers".into()).into())
            }
        }
    }

    /// Evaluates a binary operation on two floats.
    fn eval_float_binop(&self, a: f64, op: BinOp, b: f64) -> EvalResult {
        match op {
            BinOp::Add => Ok(Value::Float(a + b)),
            BinOp::Sub => Ok(Value::Float(a - b)),
            BinOp::Mul => Ok(Value::Float(a * b)),
            BinOp::Div => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero.into());
                }
                Ok(Value::Float(a / b))
            }
            BinOp::Rem => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero.into());
                }
                Ok(Value::Float(a % b))
            }
            BinOp::Pow => Ok(Value::Float(a.powf(b))),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::Le => Ok(Value::Bool(a <= b)),
            BinOp::Ge => Ok(Value::Bool(a >= b)),
            _ => {
                Err(RuntimeError::TypeError(format!("unsupported operator {op} for floats")).into())
            }
        }
    }

    /// Evaluates a binary operation on two strings.
    fn eval_str_binop(&self, a: &str, op: BinOp, b: &str) -> EvalResult {
        match op {
            BinOp::Add => Ok(Value::Str(format!("{a}{b}"))),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::Le => Ok(Value::Bool(a <= b)),
            BinOp::Ge => Ok(Value::Bool(a >= b)),
            _ => Err(
                RuntimeError::TypeError(format!("unsupported operator {op} for strings")).into(),
            ),
        }
    }

    /// Evaluates comparison operators for general values.
    fn eval_comparison(&self, lv: &Value, op: BinOp, rv: &Value) -> EvalResult {
        match op {
            BinOp::Eq => Ok(Value::Bool(lv == rv)),
            BinOp::Ne => Ok(Value::Bool(lv != rv)),
            _ => Err(RuntimeError::TypeError(format!(
                "unsupported operator {op} for {} and {}",
                lv.type_name(),
                rv.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a unary expression.
    fn eval_unary(&mut self, op: UnaryOp, operand: &Expr) -> EvalResult {
        let val = self.eval_expr(operand)?;
        match (op, &val) {
            (UnaryOp::Neg, Value::Int(v)) => Ok(Value::Int(v.wrapping_neg())),
            (UnaryOp::Neg, Value::Float(v)) => Ok(Value::Float(-v)),
            (UnaryOp::Not, Value::Bool(v)) => Ok(Value::Bool(!v)),
            (UnaryOp::Not, _) => Ok(Value::Bool(!val.is_truthy())),
            (UnaryOp::BitNot, Value::Int(v)) => Ok(Value::Int(!v)),
            (UnaryOp::Deref, Value::Pointer(addr)) => {
                // Dereference pointer: read i64 at address
                use crate::runtime::os::memory::VirtAddr;
                match self.os.memory.read_u64(VirtAddr(*addr)) {
                    Ok(val) => Ok(Value::Int(val as i64)),
                    Err(_) => Err(RuntimeError::TypeError(format!(
                        "cannot dereference invalid pointer 0x{addr:x}"
                    ))
                    .into()),
                }
            }
            _ => Err(RuntimeError::TypeError(format!(
                "unsupported unary {op} for {}",
                val.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a function call.
    fn eval_call(&mut self, callee: &Expr, args: &[CallArg]) -> EvalResult {
        let func = self.eval_expr(callee)?;

        // Evaluate arguments
        let has_named = args.iter().any(|a| a.name.is_some());
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push((arg.name.clone(), self.eval_expr(&arg.value)?));
        }

        match func {
            Value::Function(fv) => {
                let ordered = if has_named {
                    self.reorder_named_args(&fv.params, arg_vals)?
                } else {
                    arg_vals.into_iter().map(|(_, v)| v).collect()
                };
                self.call_function(&fv, ordered)
            }
            Value::BuiltinFn(name) => {
                let vals: Vec<Value> = arg_vals.into_iter().map(|(_, v)| v).collect();
                if let Some(ref mut session) = self.profile_session {
                    session.enter_fn(&name, "", 0);
                }
                let result = self.call_builtin(&name, vals);
                if let Some(ref mut session) = self.profile_session {
                    session.exit_fn();
                }
                result
            }
            _ => {
                let desc = format!("{func}");
                Err(RuntimeError::NotAFunction(desc).into())
            }
        }
    }

    /// Calls a user-defined function with given arguments.
    /// Call a Value as a function (works with closures and named functions)
    fn call_value(&mut self, func: &Value, args: Vec<Value>) -> EvalResult {
        match func {
            Value::Function(fv) => self.call_function(fv, args),
            Value::BuiltinFn(name) => self.call_builtin(name, args),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot call value of type {} as function",
                func.type_name()
            ))
            .into()),
        }
    }

    /// V27.5 P4.1: Check a refinement type predicate against a value.
    /// Returns Ok(()) if the predicate holds (or the type isn't a refinement),
    /// or Err(TypeError) describing the violation.
    ///
    /// Called from 4 sites: let-binding (Stmt::Let), function call (param),
    /// function return, and mutable assignment.
    fn check_refinement(
        &mut self,
        ty: &crate::parser::ast::TypeExpr,
        val: &Value,
        context: &str,
    ) -> Result<(), EvalError> {
        use crate::parser::ast::TypeExpr;
        if let TypeExpr::Refinement {
            var_name,
            predicate,
            ..
        } = ty
        {
            let pred_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                &self.env,
            ))));
            pred_env
                .lock()
                .expect("env lock")
                .define(var_name.clone(), val.clone());
            let prev_env = self.env.clone();
            self.env = pred_env;
            let pred_result = self.eval_expr(predicate);
            self.env = prev_env;
            match pred_result {
                Ok(Value::Bool(true)) => {}
                Ok(Value::Bool(false)) => {
                    return Err(RuntimeError::TypeError(format!(
                        "refinement violation at {context}: {val:?} does not satisfy predicate"
                    ))
                    .into());
                }
                _ => {} // Non-bool predicate: skip
            }
        }
        Ok(())
    }

    fn call_function(&mut self, fv: &FnValue, args: Vec<Value>) -> EvalResult {
        if args.len() != fv.params.len() {
            return Err(RuntimeError::ArityMismatch {
                expected: fv.params.len(),
                got: args.len(),
            }
            .into());
        }

        // AA1: If async fn, capture as Future instead of executing immediately
        if fv.is_async {
            let task_id = self.next_task_id;
            self.next_task_id += 1;
            // Create a new environment with arguments bound
            let call_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                &fv.closure_env,
            ))));
            for (param, val) in fv.params.iter().zip(args) {
                call_env
                    .lock()
                    .expect("env lock")
                    .define(param.name.clone(), val);
            }
            self.async_tasks
                .insert(task_id, (fv.body.clone(), call_env));
            return Ok(Value::Future { task_id });
        }

        // V18: If gen fn, eagerly collect all yielded values into an array
        if fv.is_gen {
            let prev_yields = self.generator_yields.take();
            self.generator_yields = Some(Vec::new());

            // Execute the generator body in a new scope
            self.call_depth += 1;
            let fn_name = if fv.name.is_empty() {
                "<gen>".to_string()
            } else {
                fv.name.clone()
            };
            self.call_stack.push(fn_name.clone());

            let call_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
                &fv.closure_env,
            ))));
            for (param, val) in fv.params.iter().zip(args) {
                call_env
                    .lock()
                    .expect("env lock")
                    .define(param.name.clone(), val);
            }
            let prev_env = std::mem::replace(&mut self.env, call_env);
            let _ = self.eval_expr(&fv.body); // ignore final return value
            self.env = prev_env;

            self.call_stack.pop();
            self.call_depth -= 1;

            let yields = self.generator_yields.take().unwrap_or_default();
            self.generator_yields = prev_yields;
            return Ok(Value::array_from_vec(yields));
        }

        self.call_depth += 1;
        let fn_name = if fv.name.is_empty() {
            "<closure>".to_string()
        } else {
            fv.name.clone()
        };
        self.call_stack.push(fn_name.clone());

        if self.call_depth > self.max_recursion_depth {
            let backtrace = self.format_backtrace();
            self.call_stack.pop();
            self.call_depth -= 1;
            return Err(RuntimeError::StackOverflow {
                depth: self.max_recursion_depth,
                backtrace,
            }
            .into());
        }

        // Record function entry in profiling session (if active).
        if let Some(ref mut session) = self.profile_session {
            session.enter_fn(&fn_name, "", 0);
        }
        // V20: Record function entry for debug recording.
        self.record_fn_entry(&fn_name);

        // Create new scope with closure's environment as parent
        let call_env = Arc::new(Mutex::new(Environment::new_with_parent(Arc::clone(
            &fv.closure_env,
        ))));

        // Bind parameters
        // V27.5 P4.1: check refinement predicates on each parameter.
        for (param, val) in fv.params.iter().zip(args) {
            self.check_refinement(&param.ty, &val, &format!("param {}", param.name))?;
            call_env
                .lock()
                .expect("env lock")
                .define(param.name.clone(), val);
        }

        // Save and swap environment
        let prev_env = Arc::clone(&self.env);
        self.env = call_env;

        // V18 4.4: Check @requires preconditions at call time
        for req_expr in &fv.requires {
            match self.eval_expr(req_expr) {
                Ok(Value::Bool(true)) => {} // precondition satisfied
                Ok(Value::Bool(false)) => {
                    self.env = prev_env;
                    self.call_stack.pop();
                    self.call_depth -= 1;
                    return Err(RuntimeError::TypeError(format!(
                        "@requires precondition failed in '{}'",
                        fv.name
                    ))
                    .into());
                }
                Ok(_) => {}  // non-bool @requires — skip
                Err(_) => {} // evaluation error — skip
            }
        }

        let result = match self.eval_expr(&fv.body) {
            Ok(v) => Ok(v),
            Err(EvalError::Control(cf)) if matches!(*cf, ControlFlow::Return(_)) => match *cf {
                ControlFlow::Return(v) => Ok(v),
                _ => unreachable!(),
            },
            Err(e) => Err(e),
        };

        // Restore environment
        self.env = prev_env;
        self.call_stack.pop();
        self.call_depth -= 1;

        // Record function exit in profiling session (if active).
        if let Some(ref mut session) = self.profile_session {
            session.exit_fn();
        }
        // V20: Record function exit for debug recording.
        {
            let ret_str = result.as_ref().ok().map(|v| format!("{v}"));
            self.record_fn_exit(&fn_name, ret_str.as_deref());
        }

        result
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
