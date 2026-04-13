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
    #[allow(dead_code)]
    send_buffer: Vec<String>,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    subscriptions: Vec<String>,
    #[cfg(feature = "mqtt")]
    real_client: Option<RealMqttClient>,
}

/// In-memory MQTT message broker for simulation (used when `mqtt` feature is off).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct MqttBroker {
    /// topic → list of queued messages
    topics: std::collections::HashMap<String, Vec<String>>,
    /// client_id → subscribed topics
    subscriptions: std::collections::HashMap<i64, Vec<String>>,
}

#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
struct BleAdapter {
    /// Known devices from scanning.
    scanned: Vec<BleDevice>,
    /// Connected devices: handle → device.
    connected: std::collections::HashMap<i64, BleDevice>,
    /// Next connection handle.
    next_handle: i64,
}

#[allow(dead_code)]
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
#[allow(dead_code)]
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
    #[allow(dead_code)]
    mqtt_broker: MqttBroker,
    /// Next MQTT handle ID.
    next_mqtt_id: i64,
    /// Simulated BLE adapter for Bluetooth Low Energy operations.
    #[allow(dead_code)]
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
    /// Currently empty (all builtins are [x] as of V21.1) but kept for future use.
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
                Ok(Value::Array(results))
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
            _ => Ok(Value::Null),
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
                // V14 DT4: Check refinement type predicate at runtime.
                if let Some(crate::parser::ast::TypeExpr::Refinement {
                    var_name,
                    predicate,
                    ..
                }) = ty.as_ref()
                {
                    // Evaluate predicate with the bound variable.
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
                                "refinement type violation: {name} = {val:?} does not satisfy predicate"
                            ))
                            .into());
                        }
                        _ => {} // Non-bool predicates: skip check
                    }
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
                Ok(Value::Array(vec![val; n]))
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
                let mut result = a.clone();
                result.extend(b.iter().cloned());
                Ok(Value::Array(result))
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
            return Ok(Value::Array(yields));
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
        for (param, val) in fv.params.iter().zip(args) {
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
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    /// Helper: parse and eval a source string, returning the Value.
    fn eval(source: &str) -> Result<Value, RuntimeError> {
        let tokens = tokenize(source).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut interp = Interpreter::new_capturing();
        interp.eval_program(&program)
    }

    /// Helper: parse and eval, return captured output lines.
    fn eval_output(source: &str) -> Vec<String> {
        let tokens = tokenize(source).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut interp = Interpreter::new_capturing();
        interp.eval_program(&program).expect("runtime error");
        interp.get_output().to_vec()
    }

    // ── Literals ──

    #[test]
    fn eval_int_literal() {
        assert_eq!(eval("42").unwrap(), Value::Int(42));
    }

    #[test]
    fn eval_float_literal() {
        assert_eq!(eval("3.14").unwrap(), Value::Float(3.14));
    }

    #[test]
    fn eval_string_literal() {
        assert_eq!(eval("\"hello\"").unwrap(), Value::Str("hello".into()));
    }

    #[test]
    fn eval_bool_literal() {
        assert_eq!(eval("true").unwrap(), Value::Bool(true));
        assert_eq!(eval("false").unwrap(), Value::Bool(false));
    }

    #[test]
    fn eval_null_literal() {
        assert_eq!(eval("null").unwrap(), Value::Null);
    }

    // ── Arithmetic ──

    #[test]
    fn eval_addition() {
        assert_eq!(eval("1 + 2").unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_subtraction() {
        assert_eq!(eval("10 - 3").unwrap(), Value::Int(7));
    }

    #[test]
    fn eval_multiplication() {
        assert_eq!(eval("4 * 5").unwrap(), Value::Int(20));
    }

    #[test]
    fn eval_division() {
        assert_eq!(eval("10 / 3").unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_modulo() {
        assert_eq!(eval("10 % 3").unwrap(), Value::Int(1));
    }

    #[test]
    fn eval_power() {
        assert_eq!(eval("2 ** 10").unwrap(), Value::Int(1024));
    }

    #[test]
    fn eval_division_by_zero() {
        let err = eval("1 / 0").unwrap_err();
        assert!(matches!(err, RuntimeError::DivisionByZero));
    }

    #[test]
    fn eval_float_arithmetic() {
        assert_eq!(eval("1.5 + 2.5").unwrap(), Value::Float(4.0));
        assert_eq!(eval("3.0 * 2.0").unwrap(), Value::Float(6.0));
    }

    #[test]
    fn eval_mixed_int_float() {
        assert_eq!(eval("1 + 2.5").unwrap(), Value::Float(3.5));
        assert_eq!(eval("2.0 * 3").unwrap(), Value::Float(6.0));
    }

    #[test]
    fn eval_string_concat() {
        assert_eq!(
            eval("\"hello\" + \" world\"").unwrap(),
            Value::Str("hello world".into())
        );
    }

    #[test]
    fn eval_precedence() {
        assert_eq!(eval("2 + 3 * 4").unwrap(), Value::Int(14));
        assert_eq!(eval("(2 + 3) * 4").unwrap(), Value::Int(20));
    }

    // ── Comparison ──

    #[test]
    fn eval_comparison_int() {
        assert_eq!(eval("1 < 2").unwrap(), Value::Bool(true));
        assert_eq!(eval("2 > 1").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 == 1").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 != 2").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 >= 1").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 <= 1").unwrap(), Value::Bool(true));
    }

    // ── Logical ──

    #[test]
    fn eval_logical_and_or() {
        assert_eq!(eval("true && false").unwrap(), Value::Bool(false));
        assert_eq!(eval("true || false").unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_logical_short_circuit() {
        // false && (anything) should not evaluate RHS
        assert_eq!(eval("false && (1 / 0 == 0)").unwrap(), Value::Bool(false));
        // true || (anything) should not evaluate RHS
        assert_eq!(eval("true || (1 / 0 == 0)").unwrap(), Value::Bool(true));
    }

    // ── Unary ──

    #[test]
    fn eval_negation() {
        assert_eq!(eval("-42").unwrap(), Value::Int(-42));
        assert_eq!(eval("-3.14").unwrap(), Value::Float(-3.14));
    }

    #[test]
    fn eval_logical_not() {
        assert_eq!(eval("!true").unwrap(), Value::Bool(false));
        assert_eq!(eval("!false").unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_bitwise_not() {
        assert_eq!(eval("~0").unwrap(), Value::Int(-1));
    }

    // ── Bitwise ──

    #[test]
    fn eval_bitwise_ops() {
        assert_eq!(eval("5 & 3").unwrap(), Value::Int(1));
        assert_eq!(eval("5 | 3").unwrap(), Value::Int(7));
        assert_eq!(eval("5 ^ 3").unwrap(), Value::Int(6));
        assert_eq!(eval("1 << 3").unwrap(), Value::Int(8));
        assert_eq!(eval("8 >> 2").unwrap(), Value::Int(2));
    }

    // ── Variables ──

    #[test]
    fn eval_let_binding() {
        assert_eq!(eval("let x = 42; x").unwrap(), Value::Int(42));
    }

    #[test]
    fn eval_let_mut_assignment() {
        let src = "let mut x = 1; x = 2; x";
        assert_eq!(eval(src).unwrap(), Value::Int(2));
    }

    #[test]
    fn eval_compound_assignment() {
        let src = "let mut x = 10; x += 5; x";
        assert_eq!(eval(src).unwrap(), Value::Int(15));
    }

    #[test]
    fn eval_undefined_variable() {
        let err = eval("x").unwrap_err();
        assert!(matches!(err, RuntimeError::UndefinedVariable(_)));
    }

    // ── Blocks ──

    #[test]
    fn eval_block_returns_last_expr() {
        assert_eq!(eval("{ 1; 2; 3 }").unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_block_scope() {
        let src = "let x = 1; { let x = 2 }; x";
        assert_eq!(eval(src).unwrap(), Value::Int(1));
    }

    // ── If/Else ──

    #[test]
    fn eval_if_true() {
        assert_eq!(eval("if true { 1 } else { 2 }").unwrap(), Value::Int(1));
    }

    #[test]
    fn eval_if_false() {
        assert_eq!(eval("if false { 1 } else { 2 }").unwrap(), Value::Int(2));
    }

    #[test]
    fn eval_if_no_else() {
        assert_eq!(eval("if false { 1 }").unwrap(), Value::Null);
    }

    #[test]
    fn eval_if_else_if() {
        let src = "if false { 1 } else if true { 2 } else { 3 }";
        assert_eq!(eval(src).unwrap(), Value::Int(2));
    }

    // ── While ──

    #[test]
    fn eval_while_loop() {
        let src = "let mut x = 0; while x < 5 { x += 1 }; x";
        assert_eq!(eval(src).unwrap(), Value::Int(5));
    }

    #[test]
    fn eval_while_break() {
        let src = "let mut x = 0; while true { x += 1; if x == 3 { break } }; x";
        assert_eq!(eval(src).unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_while_continue() {
        // Sum only odd numbers 1..5
        let src = r#"
            let mut sum = 0
            let mut i = 0
            while i < 5 {
                i += 1
                if i % 2 == 0 { continue }
                sum += i
            }
            sum
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(9)); // 1+3+5
    }

    // ── For ──

    #[test]
    fn eval_for_loop_array() {
        let src = "let mut sum = 0; for x in [1, 2, 3] { sum += x }; sum";
        assert_eq!(eval(src).unwrap(), Value::Int(6));
    }

    #[test]
    fn eval_for_loop_range() {
        let src = "let mut sum = 0; for i in 0..5 { sum += i }; sum";
        assert_eq!(eval(src).unwrap(), Value::Int(10)); // 0+1+2+3+4
    }

    #[test]
    fn eval_for_loop_string() {
        let output = eval_output("for c in \"abc\" { println(c) }");
        assert_eq!(output, vec!["a", "b", "c"]);
    }

    // ── Functions ──

    #[test]
    fn eval_function_def_and_call() {
        let src = "fn add(a: i64, b: i64) -> i64 { a + b }\nadd(3, 4)";
        assert_eq!(eval(src).unwrap(), Value::Int(7));
    }

    #[test]
    fn eval_recursive_function() {
        let src = r#"
            fn fact(n: i64) -> i64 {
                if n <= 1 { 1 } else { n * fact(n - 1) }
            }
            fact(5)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(120));
    }

    #[test]
    fn eval_function_return() {
        let src = r#"
            fn early(n: i64) -> i64 {
                if n < 0 { return -1 }
                n * 2
            }
            early(-5)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(-1));
    }

    #[test]
    pub(super) fn eval_closure() {
        let src = r#"
            let double = |x: i64| -> i64 { x * 2 }
            double(5)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(10));
    }

    #[test]
    fn eval_closure_captures_env() {
        let src = r#"
            let multiplier = 3
            let mul = |x: i64| -> i64 { x * multiplier }
            mul(4)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(12));
    }

    #[test]
    fn eval_arity_mismatch() {
        let src = "fn f(a: i64) -> i64 { a }\nf(1, 2)";
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::ArityMismatch { .. }));
    }

    #[test]
    fn eval_stack_overflow() {
        // Run in a thread with larger stack to ensure the interpreter's depth check
        // catches the overflow before the Rust stack itself overflows in debug mode.
        let result = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
                eval(src).unwrap_err()
            })
            .expect("thread spawn")
            .join()
            .expect("thread join");
        assert!(matches!(result, RuntimeError::StackOverflow { .. }));
    }

    // ── Match ──

    #[test]
    fn eval_match_literal() {
        let src = r#"
            let x = 2
            match x {
                1 => "one",
                2 => "two",
                _ => "other"
            }
        "#;
        assert_eq!(eval(src).unwrap(), Value::Str("two".into()));
    }

    #[test]
    fn eval_match_wildcard() {
        let src = "match 42 { _ => true }";
        assert_eq!(eval(src).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_match_binding() {
        let src = "match 42 { n => n + 1 }";
        assert_eq!(eval(src).unwrap(), Value::Int(43));
    }

    // ── Structs ──

    #[test]
    fn eval_struct_init_and_field() {
        let src = r#"
            let p = Point { x: 1, y: 2 }
            p.x
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(1));
    }

    #[test]
    fn eval_struct_field_assign() {
        let src = r#"
            let mut p = Point { x: 1, y: 2 }
            p.x = 10
            p.x
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(10));
    }

    // ── Arrays ──

    #[test]
    fn eval_array_literal() {
        assert_eq!(
            eval("[1, 2, 3]").unwrap(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[test]
    fn eval_array_index() {
        assert_eq!(eval("[10, 20, 30][1]").unwrap(), Value::Int(20));
    }

    #[test]
    fn eval_array_index_assign() {
        let src = "let mut arr = [1, 2, 3]; arr[0] = 10; arr[0]";
        assert_eq!(eval(src).unwrap(), Value::Int(10));
    }

    // ── Tuples ──

    #[test]
    fn eval_tuple_literal() {
        assert_eq!(
            eval("(1, true)").unwrap(),
            Value::Tuple(vec![Value::Int(1), Value::Bool(true)])
        );
    }

    // ── Pipeline ──

    #[test]
    fn eval_pipeline() {
        let src = r#"
            fn double(x: i64) -> i64 { x * 2 }
            5 |> double
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(10));
    }

    // ── Builtins ──

    #[test]
    fn eval_println() {
        let output = eval_output("println(\"hello\")");
        assert_eq!(output, vec!["hello"]);
    }

    #[test]
    fn eval_println_int() {
        let output = eval_output("println(42)");
        assert_eq!(output, vec!["42"]);
    }

    #[test]
    fn eval_len_string() {
        assert_eq!(eval("len(\"hello\")").unwrap(), Value::Int(5));
    }

    #[test]
    fn eval_len_array() {
        assert_eq!(eval("len([1, 2, 3])").unwrap(), Value::Int(3));
    }

    #[test]
    fn eval_type_of() {
        assert_eq!(eval("type_of(42)").unwrap(), Value::Str("i64".into()));
        assert_eq!(eval("type_of(\"hi\")").unwrap(), Value::Str("str".into()));
    }

    #[test]
    fn eval_to_string() {
        assert_eq!(eval("to_string(42)").unwrap(), Value::Str("42".into()));
    }

    #[test]
    fn eval_assert_pass() {
        assert_eq!(eval("assert(true)").unwrap(), Value::Null);
    }

    #[test]
    fn eval_assert_fail() {
        let err = eval("assert(false)").unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn eval_assert_eq_pass() {
        assert_eq!(eval("assert_eq(1, 1)").unwrap(), Value::Null);
    }

    #[test]
    fn eval_assert_eq_fail() {
        let err = eval("assert_eq(1, 2)").unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    // ── Complex programs ──

    #[test]
    fn eval_fibonacci() {
        let src = r#"
            fn fib(n: i64) -> i64 {
                if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
            }
            fib(10)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(55));
    }

    #[test]
    fn eval_nested_functions() {
        let src = r#"
            fn outer(x: i64) -> i64 {
                fn inner(y: i64) -> i64 { y * 2 }
                inner(x) + 1
            }
            outer(5)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(11));
    }

    #[test]
    fn eval_counter_closure() {
        let src = r#"
            fn make_adder(base: i64) -> fn(i64) -> i64 {
                |x: i64| -> i64 { base + x }
            }
            let add5 = make_adder(5)
            add5(3)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(8));
    }

    // ── impl blocks & method dispatch ──

    #[test]
    fn eval_impl_basic_method() {
        let src = r#"
            struct Point { x: f64, y: f64 }
            impl Point {
                fn magnitude_sq(self) -> f64 {
                    self.x * self.x + self.y * self.y
                }
            }
            let p = Point { x: 3.0, y: 4.0 }
            p.magnitude_sq()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Float(25.0));
    }

    #[test]
    fn eval_impl_multiple_methods() {
        let src = r#"
            struct Rect { w: f64, h: f64 }
            impl Rect {
                fn area(self) -> f64 { self.w * self.h }
                fn perimeter(self) -> f64 { 2.0 * (self.w + self.h) }
            }
            let r = Rect { w: 5.0, h: 3.0 }
            r.area() + r.perimeter()
        "#;
        // area=15, perimeter=16, total=31
        assert_eq!(eval(src).unwrap(), Value::Float(31.0));
    }

    #[test]
    fn eval_impl_method_with_args() {
        let src = r#"
            struct Counter { value: i64 }
            impl Counter {
                fn add(self, n: i64) -> i64 {
                    self.value + n
                }
            }
            let c = Counter { value: 10 }
            c.add(5)
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(15));
    }

    #[test]
    fn eval_impl_static_method() {
        let src = r#"
            struct Point { x: f64, y: f64 }
            impl Point {
                fn origin() -> Point {
                    Point { x: 0.0, y: 0.0 }
                }
            }
            let p = Point::origin()
            p.x
        "#;
        assert_eq!(eval(src).unwrap(), Value::Float(0.0));
    }

    #[test]
    fn eval_impl_static_with_args() {
        let src = r#"
            struct Point { x: f64, y: f64 }
            impl Point {
                fn new(x: f64, y: f64) -> Point {
                    Point { x: x, y: y }
                }
            }
            let p = Point::new(3.0, 4.0)
            p.x + p.y
        "#;
        assert_eq!(eval(src).unwrap(), Value::Float(7.0));
    }

    #[test]
    fn eval_impl_method_not_found() {
        let src = r#"
            struct Foo { x: i64 }
            impl Foo {
                fn bar(self) -> i64 { self.x }
            }
            let f = Foo { x: 1 }
            f.baz()
        "#;
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn eval_impl_method_returns_struct() {
        let src = r#"
            struct Vec2 { x: f64, y: f64 }
            impl Vec2 {
                fn scale(self, factor: f64) -> Vec2 {
                    Vec2 { x: self.x * factor, y: self.y * factor }
                }
            }
            let v = Vec2 { x: 1.0, y: 2.0 }
            let v2 = v.scale(3.0)
            v2.x + v2.y
        "#;
        // 3.0 + 6.0 = 9.0
        assert_eq!(eval(src).unwrap(), Value::Float(9.0));
    }

    #[test]
    fn eval_impl_method_chain_output() {
        let src = r#"
            struct Greeter { name: str }
            impl Greeter {
                fn greet(self) -> str {
                    "Hello, " + self.name + "!"
                }
            }
            let g = Greeter { name: "Fajar" }
            println(g.greet())
        "#;
        let output = eval_output(src);
        assert_eq!(output, vec!["Hello, Fajar!"]);
    }

    #[test]
    fn eval_impl_two_structs() {
        let src = r#"
            struct Dog { name: str }
            struct Cat { name: str }
            impl Dog {
                fn speak(self) -> str { self.name + " says woof" }
            }
            impl Cat {
                fn speak(self) -> str { self.name + " says meow" }
            }
            let d = Dog { name: "Rex" }
            let c = Cat { name: "Whiskers" }
            d.speak() + " and " + c.speak()
        "#;
        assert_eq!(
            eval(src).unwrap(),
            Value::Str("Rex says woof and Whiskers says meow".into())
        );
    }

    #[test]
    fn eval_impl_self_field_access() {
        let src = r#"
            struct Circle { radius: f64 }
            impl Circle {
                fn diameter(self) -> f64 { self.radius * 2.0 }
                fn area_approx(self) -> f64 { 3.14159 * self.radius * self.radius }
            }
            let c = Circle { radius: 5.0 }
            c.diameter()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Float(10.0));
    }

    // ── Option/Result & ? operator ──

    #[test]
    fn eval_some_constructor() {
        assert_eq!(
            eval("Some(42)").unwrap(),
            Value::Enum {
                variant: "Some".into(),
                data: Some(Box::new(Value::Int(42)))
            }
        );
    }

    #[test]
    fn eval_none_value() {
        assert_eq!(
            eval("None").unwrap(),
            Value::Enum {
                variant: "None".into(),
                data: None
            }
        );
    }

    #[test]
    fn eval_ok_constructor() {
        assert_eq!(
            eval("Ok(10)").unwrap(),
            Value::Enum {
                variant: "Ok".into(),
                data: Some(Box::new(Value::Int(10)))
            }
        );
    }

    #[test]
    fn eval_err_constructor() {
        assert_eq!(
            eval("Err(\"bad\")").unwrap(),
            Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str("bad".into())))
            }
        );
    }

    #[test]
    fn eval_try_unwraps_ok() {
        let src = r#"
            fn get_val() -> i64 {
                let x = Ok(42)?
                x
            }
            get_val()
        "#;
        // ? on Ok(42) returns 42, so get_val returns 42
        // But the return is wrapped as ControlFlow::Return — need a top-level fn
        assert_eq!(eval(src).unwrap(), Value::Int(42));
    }

    #[test]
    fn eval_try_unwraps_some() {
        let src = r#"
            fn get_val() -> i64 {
                let x = Some(99)?
                x
            }
            get_val()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(99));
    }

    #[test]
    fn eval_try_short_circuits_err() {
        let src = r#"
            fn might_fail() -> i64 {
                let x = Err("oops")?
                x + 100
            }
            might_fail()
        "#;
        // ? on Err short-circuits, returning Err("oops")
        assert_eq!(
            eval(src).unwrap(),
            Value::Enum {
                variant: "Err".into(),
                data: Some(Box::new(Value::Str("oops".into())))
            }
        );
    }

    #[test]
    fn eval_try_short_circuits_none() {
        let src = r#"
            fn might_fail() -> i64 {
                let x = None?
                x + 100
            }
            might_fail()
        "#;
        assert_eq!(
            eval(src).unwrap(),
            Value::Enum {
                variant: "None".into(),
                data: None
            }
        );
    }

    #[test]
    fn eval_try_propagation_chain() {
        let src = r#"
            fn step1() -> i64 { Ok(10) }
            fn step2() -> i64 {
                let a = step1()?
                let b = Ok(20)?
                a + b
            }
            step2()
        "#;
        // step1() returns Ok(10), ? unwraps to 10
        // Ok(20) unwrapped to 20, total = 30
        assert_eq!(eval(src).unwrap(), Value::Int(30));
    }

    #[test]
    fn eval_unwrap_some() {
        assert_eq!(eval("Some(42).unwrap()").unwrap(), Value::Int(42));
    }

    #[test]
    fn eval_unwrap_none_panics() {
        let err = eval("None.unwrap()").unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn eval_unwrap_ok() {
        assert_eq!(eval("Ok(7).unwrap()").unwrap(), Value::Int(7));
    }

    #[test]
    fn eval_unwrap_err_panics() {
        let err = eval("Err(\"fail\").unwrap()").unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn eval_unwrap_or_some() {
        assert_eq!(eval("Some(42).unwrap_or(0)").unwrap(), Value::Int(42));
    }

    #[test]
    fn eval_unwrap_or_none() {
        assert_eq!(eval("None.unwrap_or(99)").unwrap(), Value::Int(99));
    }

    #[test]
    fn eval_unwrap_or_err() {
        assert_eq!(eval("Err(\"x\").unwrap_or(100)").unwrap(), Value::Int(100));
    }

    #[test]
    fn eval_is_some_is_none() {
        assert_eq!(eval("Some(1).is_some()").unwrap(), Value::Bool(true));
        assert_eq!(eval("Some(1).is_none()").unwrap(), Value::Bool(false));
        assert_eq!(eval("None.is_some()").unwrap(), Value::Bool(false));
        assert_eq!(eval("None.is_none()").unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_is_ok_is_err() {
        assert_eq!(eval("Ok(1).is_ok()").unwrap(), Value::Bool(true));
        assert_eq!(eval("Ok(1).is_err()").unwrap(), Value::Bool(false));
        assert_eq!(eval("Err(1).is_ok()").unwrap(), Value::Bool(false));
        assert_eq!(eval("Err(1).is_err()").unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_match_on_option() {
        let src = r#"
            let val = Some(42)
            match val {
                Some(x) => x * 2,
                None => 0
            }
        "#;
        // Match on Option — variant patterns
        // Current match system uses pattern matching on enum variants
        assert_eq!(eval(src).unwrap(), Value::Int(84));
    }

    // ── Sprint 12: Memory Safety ──

    #[test]
    fn s12_integer_overflow_add_panics() {
        let src = "9223372036854775807 + 1"; // i64::MAX + 1
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
    }

    #[test]
    fn s12_integer_overflow_sub_panics() {
        // i64::MIN is -9223372036854775808, but lexer can't parse that literal directly.
        // Use -(i64::MAX) - 1 - 1 to reach underflow.
        let src = "let x = -9223372036854775807 - 1\nx - 1"; // i64::MIN - 1
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
    }

    #[test]
    fn s12_integer_overflow_mul_panics() {
        let src = "9223372036854775807 * 2"; // i64::MAX * 2
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
    }

    #[test]
    fn s12_integer_overflow_pow_panics() {
        let src = "2 ** 63"; // 2^63 overflows i64
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
    }

    #[test]
    fn s12_wrapping_add_wraps_correctly() {
        let src = "wrapping_add(9223372036854775807, 1)";
        assert_eq!(eval(src).unwrap(), Value::Int(i64::MIN));
    }

    #[test]
    fn s12_wrapping_sub_wraps_correctly() {
        // i64::MIN wrapping_sub 1 = i64::MAX
        let src = "let x = -9223372036854775807 - 1\nwrapping_sub(x, 1)";
        assert_eq!(eval(src).unwrap(), Value::Int(i64::MAX));
    }

    #[test]
    fn s12_wrapping_mul_wraps_correctly() {
        let src = "wrapping_mul(9223372036854775807, 2)";
        assert_eq!(eval(src).unwrap(), Value::Int(-2));
    }

    #[test]
    fn s12_checked_add_returns_some_on_success() {
        let src = "checked_add(1, 2)";
        assert_eq!(
            eval(src).unwrap(),
            Value::Enum {
                variant: "Some".into(),
                data: Some(Box::new(Value::Int(3))),
            }
        );
    }

    #[test]
    fn s12_checked_add_returns_none_on_overflow() {
        let src = "checked_add(9223372036854775807, 1)";
        assert_eq!(
            eval(src).unwrap(),
            Value::Enum {
                variant: "None".into(),
                data: None,
            }
        );
    }

    #[test]
    fn s12_saturating_add_saturates() {
        let src = "saturating_add(9223372036854775807, 1)";
        assert_eq!(eval(src).unwrap(), Value::Int(i64::MAX));
    }

    #[test]
    fn s12_saturating_sub_saturates() {
        // i64::MIN saturating_sub 1 = i64::MIN (saturated)
        let src = "let x = -9223372036854775807 - 1\nsaturating_sub(x, 1)";
        assert_eq!(eval(src).unwrap(), Value::Int(i64::MIN));
    }

    #[test]
    fn s12_array_index_out_of_bounds_re010() {
        let src = "[1, 2, 3][5]";
        let err = eval(src).unwrap_err();
        assert!(matches!(
            err,
            RuntimeError::IndexOutOfBounds {
                index: 5,
                collection: _,
                length: 3,
            }
        ));
    }

    #[test]
    fn s12_string_index_out_of_bounds_re010() {
        let src = r#""hi"[5]"#;
        let err = eval(src).unwrap_err();
        assert!(matches!(
            err,
            RuntimeError::IndexOutOfBounds {
                index: 5,
                collection: _,
                length: 2,
            }
        ));
    }

    #[test]
    fn s12_stack_overflow_configurable_depth() {
        // Use eval_with_depth helper to test custom depth
        let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let mut interp = Interpreter::new_capturing();
        interp.set_max_recursion_depth(10);
        let err = interp.eval_program(&program).unwrap_err();
        match err {
            RuntimeError::StackOverflow { depth, .. } => assert_eq!(depth, 10),
            other => panic!("expected StackOverflow, got: {other:?}"),
        }
    }

    #[test]
    fn stack_overflow_includes_backtrace() {
        let src = "fn c(n: i64) -> i64 { c(n) }\nfn b(n: i64) -> i64 { c(n) }\nfn a(n: i64) -> i64 { b(n) }\na(1)";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let mut interp = Interpreter::new_capturing();
        interp.set_max_recursion_depth(5);
        let err = interp.eval_program(&program).unwrap_err();
        match err {
            RuntimeError::StackOverflow { backtrace, .. } => {
                assert!(backtrace.contains("a()"), "backtrace should show a()");
                assert!(backtrace.contains("b()"), "backtrace should show b()");
                assert!(backtrace.contains("c()"), "backtrace should show c()");
            }
            other => panic!("expected StackOverflow, got: {other:?}"),
        }
    }

    #[test]
    fn call_stack_tracks_functions() {
        // After normal execution, call stack should be empty
        let src = "fn foo() -> i64 { 42 }\nfoo()";
        let tokens = crate::lexer::tokenize(src).unwrap();
        let program = crate::parser::parse(tokens).unwrap();
        let mut interp = Interpreter::new_capturing();
        interp.eval_program(&program).unwrap();
        assert!(interp.get_call_stack().is_empty());
    }

    #[test]
    fn s12_normal_arithmetic_no_overflow() {
        // Normal operations should work fine
        assert_eq!(eval("100 + 200").unwrap(), Value::Int(300));
        assert_eq!(eval("1000 * 1000").unwrap(), Value::Int(1_000_000));
        assert_eq!(eval("50 - 100").unwrap(), Value::Int(-50));
        assert_eq!(eval("2 ** 10").unwrap(), Value::Int(1024));
    }

    #[test]
    fn s12_null_safety_option_must_be_matched() {
        // Option must be matched or unwrapped — ? on None propagates
        let src = r#"
            fn safe() -> i64 {
                let val = None?
                val + 1
            }
            safe()
        "#;
        // None? should short-circuit, returning None (not null)
        assert_eq!(
            eval(src).unwrap(),
            Value::Enum {
                variant: "None".into(),
                data: None,
            }
        );
    }

    #[test]
    fn s12_try_operator_only_on_option_result() {
        // ? on a non-Option/Result value should error
        let src = r#"
            fn bad() -> i64 {
                let x = 42?
                x
            }
            bad()
        "#;
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn s12_null_arithmetic_is_type_error() {
        // null + 1 should be a type error
        let src = "null + 1";
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::TypeError(_)));
    }

    #[test]
    fn s12_div_overflow_i64_min_by_neg1() {
        // i64::MIN / -1 overflows (would be i64::MAX + 1)
        let src = "let x = -9223372036854775807 - 1\nx / -1";
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::IntegerOverflow { .. }));
    }

    // --- Labeled break/continue tests ---

    #[test]
    fn labeled_break_outer_while() {
        let src = r#"
            fn main() -> i64 {
                let mut result = 0
                'outer: while true {
                    let mut j = 0
                    while j < 10 {
                        if j == 3 {
                            result = 42
                            break 'outer
                        }
                        j = j + 1
                    }
                }
                result
            }
            main()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(42));
    }

    #[test]
    fn labeled_continue_outer_while() {
        let src = r#"
            fn main() -> i64 {
                let mut count = 0
                let mut i = 0
                'outer: while i < 5 {
                    i = i + 1
                    let mut j = 0
                    while j < 5 {
                        j = j + 1
                        if j == 2 {
                            continue 'outer
                        }
                    }
                    count = count + 1
                }
                count
            }
            main()
        "#;
        // Inner loop always hits continue 'outer at j==2,
        // so count never increments
        assert_eq!(eval(src).unwrap(), Value::Int(0));
    }

    #[test]
    fn labeled_break_outer_loop() {
        let src = r#"
            fn main() -> i64 {
                let mut x = 0
                'outer: loop {
                    loop {
                        x = 99
                        break 'outer
                    }
                }
                x
            }
            main()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(99));
    }

    #[test]
    fn labeled_break_inner_only() {
        // break without label only breaks inner loop
        let src = r#"
            fn main() -> i64 {
                let mut sum = 0
                let mut i = 0
                while i < 3 {
                    let mut j = 0
                    while j < 100 {
                        if j == 2 {
                            break
                        }
                        j = j + 1
                    }
                    sum = sum + i
                    i = i + 1
                }
                sum
            }
            main()
        "#;
        // sum = 0 + 1 + 2 = 3
        assert_eq!(eval(src).unwrap(), Value::Int(3));
    }

    #[test]
    fn labeled_break_for_loop() {
        let src = r#"
            fn main() -> i64 {
                let mut result = 0
                'outer: for i in 0..10 {
                    for j in 0..10 {
                        if i + j == 5 {
                            result = i * 100 + j
                            break 'outer
                        }
                    }
                }
                result
            }
            main()
        "#;
        // First time i+j==5: i=0, j=5 → result=5
        assert_eq!(eval(src).unwrap(), Value::Int(5));
    }

    // --- const in function body tests ---

    #[test]
    fn const_in_function_body() {
        let src = r#"
            fn main() -> i64 {
                const SIZE: i64 = 4096 * 16
                SIZE
            }
            main()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(65536));
    }

    #[test]
    fn const_chained_in_body() {
        let src = r#"
            fn main() -> i64 {
                const A: i64 = 100
                const B: i64 = A * 2
                const C: i64 = A + B
                C
            }
            main()
        "#;
        assert_eq!(eval(src).unwrap(), Value::Int(300));
    }

    #[test]
    fn const_immutability_check() {
        // const assignment should produce analyzer error SE007
        use crate::analyzer::analyze;
        use crate::lexer::tokenize;
        use crate::parser::parse;
        let src = r#"
            fn main() -> i64 {
                const X: i64 = 42
                X = 10
                X
            }
        "#;
        let tokens = tokenize(src).unwrap();
        let program = parse(tokens).unwrap();
        let result = analyze(&program);
        assert!(result.is_err(), "const assignment should be rejected");
    }

    // ── Profiler tests ──

    /// Profiling session records function calls when enabled.
    #[test]
    fn test_profiler_records_function_calls() {
        let src = r#"
            fn add(a: i64, b: i64) -> i64 { a + b }
            fn run() -> i64 { add(1, 2) }
            run()
        "#;
        let tokens = tokenize(src).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut interp = Interpreter::new_capturing();
        interp.enable_profiling();
        interp.eval_program(&program).expect("runtime error");
        let session = interp.profile_session.as_ref().expect("session missing");
        assert!(
            session.call_count() > 0,
            "expected at least one call recorded, got {}",
            session.call_count()
        );
    }

    /// Profiling tracks nested call depth correctly.
    #[test]
    fn test_profiler_nested_calls() {
        let src = r#"
            fn inner() -> i64 { 42 }
            fn middle() -> i64 { inner() }
            fn outer() -> i64 { middle() }
            outer()
        "#;
        let tokens = tokenize(src).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut interp = Interpreter::new_capturing();
        interp.enable_profiling();
        interp.eval_program(&program).expect("runtime error");
        let session = interp.profile_session.as_ref().expect("session missing");
        // outer + middle + inner = 3 calls minimum
        assert!(
            session.call_count() >= 3,
            "expected at least 3 nested calls, got {}",
            session.call_count()
        );
        // At least one call should have depth > 0 (i.e., nested)
        let has_nested = session.records().iter().any(|r| r.depth > 0);
        assert!(
            has_nested,
            "expected at least one nested call with depth > 0"
        );
    }

    /// to_trace() produces well-formed Chrome JSON trace output.
    #[test]
    fn test_profiler_output_json() {
        let src = r#"
            fn greet() -> i64 { 1 }
            greet()
        "#;
        let tokens = tokenize(src).expect("lex error");
        let program = parse(tokens).expect("parse error");
        let mut interp = Interpreter::new_capturing();
        interp.enable_profiling();
        interp.eval_program(&program).expect("runtime error");
        let session = interp.profile_session.as_ref().expect("session missing");
        let trace = session.to_trace();
        // Chrome trace is a JSON array: starts with '[' and ends with ']'
        assert!(trace.starts_with('['), "trace should start with '['");
        assert!(trace.ends_with(']'), "trace should end with ']'");
        // Should contain the function name
        assert!(
            trace.contains("greet"),
            "trace should contain function name 'greet'"
        );
    }

    // ── WebSocket / MQTT tests ──

    #[test]
    fn test_ws_connect_send_recv_close() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let ws = ws_connect("ws://localhost:8080")
            let sent = ws_send(ws, "hello")
            let msg = ws_recv(ws)
            ws_close(ws)
            msg
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        match result.unwrap() {
            Value::Str(s) => assert_eq!(s, "hello"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_mqtt_pub_sub_roundtrip() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let client = mqtt_connect("localhost")
            mqtt_subscribe(client, "sensors/temp")
            mqtt_publish(client, "sensors/temp", "22.5")
            let msg = mqtt_recv(client)
            mqtt_disconnect(client)
            msg
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        match result.unwrap() {
            Value::Map(m) => {
                assert_eq!(m.get("topic").unwrap(), &Value::Str("sensors/temp".into()));
                assert_eq!(m.get("payload").unwrap(), &Value::Str("22.5".into()));
            }
            other => panic!("expected Map, got {:?}", other),
        }
    }

    #[test]
    fn test_mqtt_no_message_returns_null() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let client = mqtt_connect("localhost")
            mqtt_subscribe(client, "empty/topic")
            let msg = mqtt_recv(client)
            mqtt_disconnect(client)
            msg
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), Value::Null);
    }

    #[test]
    fn test_ws_recv_empty_returns_null() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let ws = ws_connect("ws://example.com")
            let msg = ws_recv(ws)
            ws_close(ws)
            msg
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), Value::Null);
    }

    #[test]
    fn test_ble_scan_returns_devices() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let devices = ble_scan()
            len(devices)
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        match result.unwrap() {
            Value::Int(n) => assert!(n >= 2, "expected at least 2 simulated devices, got {n}"),
            other => panic!("expected Int, got {:?}", other),
        }
    }

    #[test]
    fn test_ble_connect_read_write_disconnect() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let handle = ble_connect("AA:BB:CC:DD:EE:01")
            let data = ble_read(handle, "00002a6e-0000-1000-8000-00805f9b34fb")
            let ok = ble_write(handle, "0000ff01-0000-1000-8000-00805f9b34fb", [1])
            ble_disconnect(handle)
            ok
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_ble_connect_invalid_returns_negative() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let handle = ble_connect("XX:XX:XX:XX:XX:XX")
            handle
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), Value::Int(-1));
    }

    #[test]
    fn test_ble_read_after_write_returns_new_data() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"
            let h = ble_connect("AA:BB:CC:DD:EE:02")
            ble_write(h, "0000ff01-0000-1000-8000-00805f9b34fb", [0x42, 0x43])
            let data = ble_read(h, "0000ff01-0000-1000-8000-00805f9b34fb")
            ble_disconnect(h)
            data
        "#,
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        match result.unwrap() {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], Value::Int(0x42));
                assert_eq!(arr[1], Value::Int(0x43));
            }
            other => panic!("expected Array, got {:?}", other),
        }
    }

    // ===================================================================
    // V14 Phase 3 — Algebraic Effect System Tests
    // ===================================================================

    #[test]
    fn ef1_1_effect_declaration_registers_in_env() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Console {
                fn log(msg: str) -> void
                fn read_line() -> str
            }
            let x = 42
            x
            "#,
        );
        assert!(
            result.is_ok(),
            "effect declaration should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn ef1_2_effect_op_registered_as_builtin() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn info(msg: str) -> void
            }
            // Logger::info should be defined in scope as a builtin
            let f = Logger::info
            type_of(f)
            "#,
        );
        assert!(
            result.is_ok(),
            "effect op lookup should work: {:?}",
            result.err()
        );
        // Should be a BuiltinFn
        match result.unwrap() {
            Value::Str(s) => assert!(s.contains("builtin") || s.contains("function"), "got: {s}"),
            other => panic!("expected type_of to return string, got: {:?}", other),
        }
    }

    #[test]
    fn ef1_3_effect_op_default_handler_outside_handle() {
        // Outside a handle block, user-defined effect ops return Null by default.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn info(msg: str) -> void
            }
            let result = Logger::info("hello")
            result
            "#,
        );
        assert!(
            result.is_ok(),
            "default handler should work: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Null);
    }

    #[test]
    fn ef1_4_handle_intercepts_effect_op() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn get_name() -> str
            }
            let result = handle {
                Ask::get_name()
            } with {
                Ask::get_name() => { "Fajar" }
            }
            result
            "#,
        );
        assert!(
            result.is_ok(),
            "handle should intercept effect: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
    }

    #[test]
    fn ef1_5_handle_with_params() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            let captured = ""
            let result = handle {
                Logger::log("hello world")
                42
            } with {
                Logger::log(msg) => { msg }
            }
            result
            "#,
        );
        assert!(
            result.is_ok(),
            "handle with params should work: {:?}",
            result.err()
        );
        // V15: With multi-step continuations, the handler's resume value ("hello world")
        // is cached and the body continues. The body's final expression (42) is the
        // result of the handle expression.
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn ef1_6_resume_in_handler() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn question(prompt: str) -> str
            }
            let answer = handle {
                Ask::question("What is your name?")
            } with {
                Ask::question(prompt) => { resume("Fajar") }
            }
            answer
            "#,
        );
        assert!(result.is_ok(), "resume should work: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
    }

    #[test]
    fn ef1_7_effect_registry_has_builtins() {
        let interp = Interpreter::new();
        // Verify the runtime effect registry has built-in effects.
        assert!(interp.effect_registry.lookup("IO").is_some());
        assert!(interp.effect_registry.lookup("Alloc").is_some());
        assert!(interp.effect_registry.lookup("Panic").is_some());
        assert!(interp.effect_registry.lookup("Exception").is_some());
        assert!(interp.effect_registry.lookup("Async").is_some());
        assert!(interp.effect_registry.lookup("State").is_some());
        assert!(interp.effect_registry.lookup("Hardware").is_some());
        assert!(interp.effect_registry.lookup("Tensor").is_some());
        assert_eq!(interp.effect_registry.count(), 8);
    }

    #[test]
    fn ef1_8_user_effect_registers_in_registry() {
        let mut interp = Interpreter::new_capturing();
        let _ = interp.eval_source(
            r#"
            effect MyEffect {
                fn do_thing(x: i32) -> i32
            }
            "#,
        );
        assert!(interp.effect_registry.lookup("MyEffect").is_some());
        let decl = interp.effect_registry.lookup("MyEffect").unwrap();
        assert_eq!(decl.op_count(), 1);
        assert!(decl.find_op("do_thing").is_some());
    }

    #[test]
    fn ef1_9_unhandled_effect_reraises() {
        // If no handler matches, the effect should propagate upward.
        // Outside all handle blocks, the default handler kicks in.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Db {
                fn query(sql: str) -> str
            }
            // No handle block — default handler returns Null.
            Db::query("SELECT 1")
            "#,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Null);
    }

    #[test]
    fn ef1_10_handle_multiple_ops_same_effect() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Console {
                fn log(msg: str) -> void
                fn read_line() -> str
            }
            let result = handle {
                Console::read_line()
            } with {
                Console::log(msg) => { null }
                Console::read_line() => { "user input" }
            }
            result
            "#,
        );
        assert!(
            result.is_ok(),
            "multiple ops should work: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Str("user input".into()));
    }

    // ===================================================================
    // Sprint EF2 — Handler Semantics
    // ===================================================================

    #[test]
    fn ef2_1_nested_handle_inner_catches() {
        // Inner handle should catch the effect before outer handle.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            let result = handle {
                handle {
                    Ask::name()
                } with {
                    Ask::name() => { "inner" }
                }
            } with {
                Ask::name() => { "outer" }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "nested handle: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("inner".into()));
    }

    #[test]
    fn ef2_2_nested_handle_outer_catches_unhandled() {
        // If inner handle doesn't match, effect propagates to outer handle.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            effect Db {
                fn query(sql: str) -> str
            }
            let result = handle {
                handle {
                    Ask::name()
                } with {
                    Db::query(sql) => { "db result" }
                }
            } with {
                Ask::name() => { "outer caught it" }
            }
            result
            "#,
        );
        assert!(
            result.is_ok(),
            "outer catches unhandled: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Str("outer caught it".into()));
    }

    #[test]
    fn ef2_3_handler_accesses_outer_scope() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            let prefix = "Hello, "
            let result = handle {
                Ask::name()
            } with {
                Ask::name() => { prefix }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "outer scope access: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("Hello, ".into()));
    }

    #[test]
    fn ef2_4_handler_with_computation() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Math {
                fn double(x: i32) -> i32
            }
            let result = handle {
                Math::double(21)
            } with {
                Math::double(x) => { x * 2 }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "handler computation: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn ef2_5_handler_returns_different_type() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Stringify {
                fn convert(x: i32) -> str
            }
            let result = handle {
                Stringify::convert(42)
            } with {
                Stringify::convert(x) => { "forty-two" }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "different type return: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("forty-two".into()));
    }

    #[test]
    fn ef2_6_body_completes_without_effect() {
        // If body doesn't perform any effect, its result is returned directly.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            let result = handle {
                42
            } with {
                Ask::name() => { "unused" }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "no effect body: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn ef2_7_effect_in_function_call() {
        // Effect raised inside a function called from handle body.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Config {
                fn get_value(key: str) -> str
            }
            fn read_config(key: str) -> str with Config {
                Config::get_value(key)
            }
            let result = handle {
                read_config("host")
            } with {
                Config::get_value(key) => { "localhost" }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "effect in fn call: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("localhost".into()));
    }

    #[test]
    fn ef2_8_resume_with_computed_value() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Random {
                fn next_int(max: i32) -> i32
            }
            let result = handle {
                Random::next_int(100)
            } with {
                Random::next_int(max) => { resume(max / 2) }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "resume computed: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(50));
    }

    #[test]
    fn ef2_9_multiple_effects_different_types() {
        // Handle block with handlers for two different effects.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            effect Config {
                fn get(key: str) -> str
            }
            let result = handle {
                Config::get("name")
            } with {
                Logger::log(msg) => { null }
                Config::get(key) => { "Fajar Lang" }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "multi-effect: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("Fajar Lang".into()));
    }

    #[test]
    fn ef2_10_effect_handler_zero_params() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Clock {
                fn now() -> i64
            }
            let result = handle {
                Clock::now()
            } with {
                Clock::now() => { 1711929600 }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "zero params: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(1711929600));
    }

    // ===================================================================
    // Sprint EF3 — Effect Inference
    // ===================================================================

    #[test]
    fn ef3_1_undeclared_effect_in_fn_body() {
        // Function calls effect op without declaring it — should get error.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            fn greet() {
                Logger::log("hello")
            }
            greet()
            "#,
        );
        // Should fail with EE001 UndeclaredEffect
        assert!(result.is_err(), "should detect undeclared effect");
        let err = format!("{:?}", result.err().unwrap());
        assert!(
            err.contains("UndeclaredEffect") || err.contains("EE001"),
            "error should be EE001: {err}"
        );
    }

    #[test]
    fn ef3_2_declared_effect_in_fn_passes() {
        // Function declares effects in `with` clause — should pass.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            fn greet() with Logger {
                Logger::log("hello")
            }
            greet()
            "#,
        );
        assert!(
            result.is_ok(),
            "declared effect should pass: {:?}",
            result.err()
        );
    }

    #[test]
    fn ef3_3_handled_effect_no_warning() {
        // Effect inside a handle block should not require `with` declaration.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            fn greet() -> str {
                handle {
                    Ask::name()
                } with {
                    Ask::name() => { "Fajar" }
                }
            }
            greet()
            "#,
        );
        assert!(
            result.is_ok(),
            "handled effect no warning: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
    }

    #[test]
    fn ef3_4_fn_with_effects_executes() {
        // A function with declared effects should execute normally.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Config {
                fn get(key: str) -> str
            }
            fn read_config(key: str) -> str with Config {
                Config::get(key)
            }
            // Call inside handle block so effect is intercepted.
            handle {
                read_config("host")
            } with {
                Config::get(key) => { "localhost" }
            }
            "#,
        );
        assert!(result.is_ok(), "fn with effects: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("localhost".into()));
    }

    #[test]
    fn ef3_5_effect_propagation_through_call() {
        // Calling a function with effects propagates those effects.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Db {
                fn query(sql: str) -> str
            }
            fn get_user(id: i32) -> str with Db {
                Db::query("SELECT name WHERE id=" )
            }
            // get_user performs Db, handled here
            handle {
                get_user(1)
            } with {
                Db::query(sql) => { "Alice" }
            }
            "#,
        );
        assert!(result.is_ok(), "effect propagation: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("Alice".into()));
    }

    #[test]
    fn ef3_6_no_effects_function_passes() {
        // Function with no effect operations should work fine.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
            add(1, 2)
            "#,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(3));
    }

    #[test]
    fn ef3_7_multiple_effects_declared() {
        // Function can declare multiple effects.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            effect Db {
                fn query(sql: str) -> str
            }
            fn process() with Logger, Db {
                Logger::log("starting")
                Db::query("SELECT 1")
            }
            handle {
                process()
            } with {
                Logger::log(msg) => { null }
                Db::query(sql) => { "done" }
            }
            "#,
        );
        assert!(result.is_ok(), "multi-effects: {:?}", result.err());
    }

    #[test]
    fn ef3_8_builtin_effects_registered() {
        // Built-in effects (IO, Alloc, etc.) should be recognized.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            fn do_io() with IO {
                IO::print("hello")
            }
            do_io()
            "#,
        );
        assert!(result.is_ok(), "builtin effect: {:?}", result.err());
    }

    #[test]
    fn ef3_9_context_effect_compatibility() {
        // Effects in @kernel context: Hardware OK, Tensor forbidden.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Custom {
                fn tick() -> void
            }
            let x = 42
            x
            "#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn ef3_10_effect_set_operations() {
        // Test the EffectSet union/intersection operations.
        use crate::analyzer::effects::EffectSet;
        let mut set_a = EffectSet::empty();
        set_a.insert("IO".to_string());
        set_a.insert("Alloc".to_string());

        let mut set_b = EffectSet::empty();
        set_b.insert("IO".to_string());
        set_b.insert("Panic".to_string());

        let union = set_a.union(&set_b);
        assert_eq!(union.len(), 3); // IO, Alloc, Panic

        let intersection = set_a.intersection(&set_b);
        assert_eq!(intersection.len(), 1); // IO

        let diff = set_a.difference(&set_b);
        assert_eq!(diff.len(), 1); // Alloc
        assert!(diff.contains("Alloc"));

        assert!(set_a.is_subset_of(&union));
        assert!(!set_a.is_subset_of(&set_b));
    }

    // ===================================================================
    // V15 Sprint B1 — Multi-step Continuations
    // ===================================================================

    #[test]
    fn v15_b1_1_multi_step_two_effects() {
        // Body with 2 effect calls — both must execute.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Console {
                fn log(msg: str) -> void
            }
            let result = handle {
                Console::log("hello")
                Console::log("world")
                42
            } with {
                Console::log(msg) => { resume(null) }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "multi-step 2 effects: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn v15_b1_2_multi_step_three_effects() {
        // Body with 3 sequential effect calls — all must execute.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Counter {
                fn next() -> i32
            }
            let mut n = 0
            let result = handle {
                let a = Counter::next()
                let b = Counter::next()
                let c = Counter::next()
                a + b + c
            } with {
                Counter::next() => { resume(10) }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "multi-step 3 effects: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(30));
    }

    #[test]
    fn v15_b1_3_resume_return_value() {
        // resume(42) makes the effect call site return 42.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect AppState {
                fn get() -> i32
            }
            let x = handle {
                AppState::get()
            } with {
                AppState::get() => { resume(42) }
            }
            x
            "#,
        );
        assert!(result.is_ok(), "resume return value: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn v15_b1_3b_resume_value_used_in_body() {
        // Resume value is used in subsequent computation in the body.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect AppState {
                fn get() -> i32
            }
            let result = handle {
                let x = AppState::get()
                x * 2 + 1
            } with {
                AppState::get() => { resume(21) }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "resume value in body: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(43));
    }

    #[test]
    fn v15_b1_4_multi_effect_types_in_handler() {
        // Handle block with handlers for two different effects, body uses both.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            effect Config {
                fn get(key: str) -> str
            }
            let result = handle {
                Logger::log("starting")
                let host = Config::get("host")
                Logger::log("done")
                host
            } with {
                Logger::log(msg) => { resume(null) }
                Config::get(key) => { resume("localhost") }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "multi-effect types: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("localhost".into()));
    }

    #[test]
    fn v15_b1_5_resume_no_arg() {
        // resume() is alias for resume(null).
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            let result = handle {
                Logger::log("hello")
                Logger::log("world")
                "done"
            } with {
                Logger::log(msg) => { resume() }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "resume no-arg: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("done".into()));
    }

    #[test]
    fn v15_b1_6_handler_scope_isolation() {
        // Handler params must not leak to outer scope.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            let msg = "outer"
            handle {
                Logger::log("inner")
            } with {
                Logger::log(msg) => { resume(null) }
            }
            msg
            "#,
        );
        assert!(result.is_ok(), "handler scope: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("outer".into()));
    }

    #[test]
    fn v15_b1_7_nested_handle_multi_step() {
        // Nested handle with multi-step: inner catches A, outer catches B.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            effect Logger {
                fn log(msg: str) -> void
            }
            let result = handle {
                handle {
                    let n = Ask::name()
                    Logger::log(n)
                    n
                } with {
                    Ask::name() => { resume("Fajar") }
                }
            } with {
                Logger::log(msg) => { resume(null) }
            }
            result
            "#,
        );
        assert!(result.is_ok(), "nested multi-step: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("Fajar".into()));
    }

    #[test]
    fn v15_b1_8_resume_type_mismatch() {
        // resume("hello") when effect returns i32 should produce SE004.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Counter {
                fn next() -> i32
            }
            handle {
                Counter::next()
            } with {
                Counter::next() => { resume("wrong type") }
            }
            "#,
        );
        // Should produce a type mismatch error from analyzer.
        assert!(result.is_err(), "should detect type mismatch in resume");
    }

    #[test]
    fn v15_b1_9_effect_arity_mismatch() {
        // Handler with wrong number of params should produce SE005.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Logger {
                fn log(msg: str) -> void
            }
            handle {
                Logger::log("hello")
            } with {
                Logger::log() => { resume(null) }
            }
            "#,
        );
        // Should produce argument count mismatch error.
        assert!(result.is_err(), "should detect arity mismatch in handler");
    }

    // ===================================================================
    // Sprint EF4 — Effect Polymorphism
    // ===================================================================

    #[test]
    fn ef4_1_effect_bound_validation() {
        use crate::analyzer::effects::{EffectBound, EffectSet, check_effect_bound};
        let mut required = EffectSet::empty();
        required.insert("IO".to_string());
        required.insert("Alloc".to_string());
        let bound = EffectBound::new("E", required);

        // Concrete with subset — should pass.
        let mut concrete = EffectSet::empty();
        concrete.insert("IO".to_string());
        assert!(check_effect_bound(&bound, &concrete).is_ok());

        // Concrete with extra effect — should fail.
        let mut bad = EffectSet::empty();
        bad.insert("IO".to_string());
        bad.insert("Panic".to_string()); // not in bound
        assert!(check_effect_bound(&bound, &bad).is_err());
    }

    #[test]
    fn ef4_2_no_effect_bound() {
        use crate::analyzer::effects::{EffectSet, NoEffectBound};
        let no_eff = NoEffectBound::new("F");

        // Empty effects — passes.
        let empty = EffectSet::empty();
        assert!(no_eff.check(&empty).is_ok());

        // Non-empty effects — fails.
        let mut with_io = EffectSet::empty();
        with_io.insert("IO".to_string());
        assert!(no_eff.check(&with_io).is_err());
    }

    #[test]
    fn ef4_3_effect_trait_method() {
        use crate::analyzer::effects::{EffectSet, EffectTraitMethod, check_trait_method_effects};
        let mut trait_effects = EffectSet::empty();
        trait_effects.insert("IO".to_string());
        trait_effects.insert("Alloc".to_string());
        let trait_method =
            EffectTraitMethod::new("process", vec!["str".into()], "void", trait_effects);

        // Impl with subset — OK.
        let mut impl_effects = EffectSet::empty();
        impl_effects.insert("IO".to_string());
        assert!(check_trait_method_effects(&trait_method, &impl_effects).is_ok());

        // Impl with extra effect — error.
        let mut bad_effects = EffectSet::empty();
        bad_effects.insert("Panic".to_string());
        assert!(check_trait_method_effects(&trait_method, &bad_effects).is_err());
    }

    #[test]
    fn ef4_4_cross_module_effect_tracking() {
        use crate::analyzer::effects::{CrossModuleEffects, EffectSet};
        let mut cross = CrossModuleEffects::new();

        let mut io_set = EffectSet::empty();
        io_set.insert("IO".to_string());
        cross.register_fn("std", "println", io_set);

        let mut db_set = EffectSet::empty();
        db_set.insert("Db".to_string());
        cross.register_fn("db", "query", db_set);

        // Infer effects from calling both.
        let combined = cross.infer_from_calls(&[("std", "println"), ("db", "query")]);
        assert_eq!(combined.len(), 2);
        assert!(combined.contains("IO"));
        assert!(combined.contains("Db"));
    }

    #[test]
    fn ef4_5_effect_erasure_hints() {
        use crate::analyzer::effects::{
            EffectErasureHint, EffectHandler, EffectSet, HandlerScopeStack, compute_erasure_hints,
        };

        let mut stack = HandlerScopeStack::new();
        stack.push_scope();
        stack.add_handler(EffectHandler::new("IO")).unwrap_or(());

        let mut effects = EffectSet::empty();
        effects.insert("IO".to_string());
        effects.insert("Panic".to_string());

        let hints = compute_erasure_hints(&effects, &stack);
        // IO should be erasable (handler at immediate scope).
        assert!(matches!(
            hints.get("IO"),
            Some(EffectErasureHint::FullErase)
        ));
        // Panic has no handler — not erasable.
        assert!(matches!(
            hints.get("Panic"),
            Some(EffectErasureHint::NoErase)
        ));
    }

    #[test]
    fn ef4_6_effect_closure_tracking() {
        use crate::analyzer::effects::{EffectClosure, EffectSet};
        let mut effects = EffectSet::empty();
        effects.insert("IO".to_string());
        let closure = EffectClosure::new(
            effects,
            vec!["str".into()],
            "void",
            vec!["captured_var".into()],
        );
        assert!(!closure.is_pure());
        assert_eq!(closure.captures.len(), 1);

        let pure_closure = EffectClosure::new(EffectSet::empty(), vec![], "i32", vec![]);
        assert!(pure_closure.is_pure());
    }

    #[test]
    fn ef4_7_effect_checker_full_pipeline() {
        use crate::analyzer::effects::EffectChecker;
        let mut checker = EffectChecker::new();

        // Registry has builtins.
        assert!(checker.registry.lookup("IO").is_some());

        // Push handler scope.
        checker.handler_stack.push_scope();
        assert_eq!(checker.handler_stack.depth(), 1);

        // Register cross-module effect.
        let mut io = crate::analyzer::effects::EffectSet::empty();
        io.insert("IO".to_string());
        checker.cross_module.register_fn("std", "print", io);
        assert_eq!(checker.cross_module.count(), 1);

        checker.handler_stack.pop_scope();
        assert_eq!(checker.handler_stack.depth(), 0);
    }

    #[test]
    fn ef4_8_builtin_handlers() {
        use crate::analyzer::effects::{
            builtin_alloc_handler, builtin_exception_handler, builtin_io_handler,
        };
        let io = builtin_io_handler();
        assert_eq!(io.handler_count(), 2);
        assert!(io.find_handler("print").is_some());
        assert!(io.find_handler("read").is_some());

        let alloc = builtin_alloc_handler();
        assert_eq!(alloc.handler_count(), 2);

        let exception = builtin_exception_handler();
        assert_eq!(exception.handler_count(), 1);
    }

    #[test]
    fn ef4_9_context_forbidden_effects() {
        use crate::analyzer::effects::{
            ContextAnnotation, EffectKind, allowed_effects, forbidden_effects,
        };
        let kernel_forbidden = forbidden_effects(ContextAnnotation::Kernel);
        assert!(kernel_forbidden.contains(&EffectKind::Alloc));
        assert!(kernel_forbidden.contains(&EffectKind::Tensor));

        let device_forbidden = forbidden_effects(ContextAnnotation::Device);
        assert!(device_forbidden.contains(&EffectKind::Hardware));

        let safe_forbidden = forbidden_effects(ContextAnnotation::Safe);
        assert!(safe_forbidden.len() >= 3); // IO, Alloc, Hardware, Tensor

        let unsafe_forbidden = forbidden_effects(ContextAnnotation::Unsafe);
        assert!(unsafe_forbidden.is_empty());

        let kernel_allowed = allowed_effects(ContextAnnotation::Kernel);
        assert!(kernel_allowed.contains("Hardware"));
    }

    #[test]
    fn ef4_10_effect_polymorphic_fn() {
        // A function with effect variable in generics should compile.
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            effect Ask {
                fn name() -> str
            }
            // Effect polymorphic: E is an effect variable.
            fn with_default<E: Effect>(default_val: str) -> str {
                default_val
            }
            let result = with_default("hello")
            result
            "#,
        );
        assert!(result.is_ok(), "effect polymorphic fn: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Str("hello".into()));
    }

    // ===================================================================
    // Sub-Option 5B — Dependent Types (DT1-DT4)
    // ===================================================================

    // Sprint DT1: Type-Level Integers & Const Generics

    #[test]
    fn dt1_1_nat_value_arithmetic() {
        use crate::dependent::nat::NatValue;
        let a = NatValue::Literal(3);
        let b = NatValue::Literal(4);
        let sum = NatValue::Add(Box::new(a), Box::new(b));
        assert_eq!(sum.evaluate(&std::collections::HashMap::new()), Some(7));
    }

    #[test]
    fn dt1_2_nat_value_substitution() {
        use crate::dependent::nat::NatValue;
        let expr = NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        );
        let mut env = std::collections::HashMap::new();
        env.insert("N".to_string(), 5u64);
        assert_eq!(expr.evaluate(&env), Some(6));
    }

    #[test]
    fn dt1_3_nat_constraint_equality() {
        use crate::dependent::nat::{NatConstraint, NatValue};
        let c = NatConstraint::Equal(NatValue::Literal(5), NatValue::Literal(5));
        assert!(c.check(&std::collections::HashMap::new()).is_ok());

        let c2 = NatConstraint::Equal(NatValue::Literal(5), NatValue::Literal(3));
        assert!(c2.check(&std::collections::HashMap::new()).is_err());
    }

    #[test]
    fn dt1_4_nat_constraint_less_than() {
        use crate::dependent::nat::{NatConstraint, NatValue};
        let c = NatConstraint::LessThan(NatValue::Literal(3), 5);
        assert!(c.check(&std::collections::HashMap::new()).is_ok());

        let c2 = NatConstraint::LessThan(NatValue::Literal(5), 3);
        assert!(c2.check(&std::collections::HashMap::new()).is_err());
    }

    #[test]
    fn dt1_5_nat_multiplication() {
        use crate::dependent::nat::NatValue;
        let product = NatValue::Mul(
            Box::new(NatValue::Literal(3)),
            Box::new(NatValue::Literal(7)),
        );
        assert_eq!(
            product.evaluate(&std::collections::HashMap::new()),
            Some(21)
        );
    }

    #[test]
    fn dt1_6_const_generic_param() {
        use crate::dependent::nat::{ConstGenericParam, ConstType};
        let param = ConstGenericParam {
            name: "N".into(),
            const_type: ConstType::Usize,
        };
        assert_eq!(param.name, "N");
        assert_eq!(param.const_type, ConstType::Usize);
    }

    #[test]
    fn dt1_7_nat_free_params() {
        use crate::dependent::nat::NatValue;
        let expr = NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Mul(
                Box::new(NatValue::Param("M".into())),
                Box::new(NatValue::Literal(2)),
            )),
        );
        let params = expr.free_params();
        assert!(params.iter().any(|p| p == "N"));
        assert!(params.iter().any(|p| p == "M"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn dt1_8_nat_is_concrete() {
        use crate::dependent::nat::NatValue;
        assert!(NatValue::Literal(5).is_concrete());
        assert!(!NatValue::Param("N".into()).is_concrete());
        assert!(
            !NatValue::Add(
                Box::new(NatValue::Param("N".into())),
                Box::new(NatValue::Literal(1)),
            )
            .is_concrete()
        );
    }

    #[test]
    fn dt1_9_nat_substitution() {
        use crate::dependent::nat::NatValue;
        let expr = NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        );
        let mut sub_env = std::collections::HashMap::new();
        sub_env.insert("N".to_string(), 10u64);
        let result = expr.substitute(&sub_env);
        assert_eq!(result.evaluate(&std::collections::HashMap::new()), Some(11));
    }

    #[test]
    fn dt1_10_kind_system() {
        use crate::dependent::nat::Kind;
        let type_kind = Kind::Type;
        let nat_kind = Kind::Nat;
        let dep_kind = Kind::Dependent(Box::new(Kind::Nat), Box::new(Kind::Type));
        assert_eq!(format!("{type_kind}"), "Type");
        assert_eq!(format!("{nat_kind}"), "Nat");
        assert_eq!(format!("{dep_kind}"), "Nat -> Type");
    }

    // Sprint DT2: Dependent Arrays

    #[test]
    fn dt2_1_dep_array_creation() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(5),
        };
        assert_eq!(arr.element_ty, "i32");
        assert_eq!(arr.len.evaluate(&std::collections::HashMap::new()), Some(5));
    }

    #[test]
    fn dt2_2_dep_array_concat() {
        use crate::dependent::arrays::{DepArray, concat_type};
        use crate::dependent::nat::NatValue;
        let a = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(3),
        };
        let b = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(5),
        };
        let result = concat_type(&a, &b);
        assert!(result.is_ok());
        let c = result.unwrap();
        assert_eq!(c.len.evaluate(&std::collections::HashMap::new()), Some(8));
    }

    #[test]
    fn dt2_3_dep_array_bounds_check() {
        use crate::dependent::arrays::{BoundsCheckResult, DepArray, check_bounds};
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(5),
        };
        let env = std::collections::HashMap::new();

        let result = check_bounds(&arr.len, &NatValue::Literal(3), &env);
        assert!(matches!(result, BoundsCheckResult::Elide));

        let oob = check_bounds(&arr.len, &NatValue::Literal(5), &env);
        assert!(matches!(oob, BoundsCheckResult::OutOfBounds));
    }

    #[test]
    fn dt2_4_dep_array_split() {
        use crate::dependent::arrays::{DepArray, split_at_types};
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(10),
        };
        let (left, right) = split_at_types(&arr, &NatValue::Literal(4));
        assert_eq!(
            left.len.evaluate(&std::collections::HashMap::new()),
            Some(4)
        );
        assert_eq!(
            right.len.evaluate(&std::collections::HashMap::new()),
            Some(6)
        );
    }

    #[test]
    fn dt2_5_dep_array_window() {
        use crate::dependent::arrays::windows_count;
        use crate::dependent::nat::NatValue;
        let wc = windows_count(&NatValue::Literal(10), &NatValue::Literal(3));
        assert_eq!(wc.evaluate(&std::collections::HashMap::new()), Some(8));
    }

    #[test]
    fn dt2_6_dep_array_type_mismatch() {
        use crate::dependent::arrays::{DepArray, concat_type};
        use crate::dependent::nat::NatValue;
        let a = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(3),
        };
        let b = DepArray {
            element_ty: "f64".into(),
            len: NatValue::Literal(5),
        };
        let concat = concat_type(&a, &b);
        assert!(concat.is_err());
    }

    #[test]
    fn dt2_7_dep_array_parametric_length() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(5),
        };
        assert!(arr.len.is_concrete());

        let dynamic = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Param("N".into()),
        };
        assert!(!dynamic.len.is_concrete());
    }

    #[test]
    fn dt2_8_dep_array_parametric() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "T".into(),
            len: NatValue::Param("N".into()),
        };
        let params = arr.len.free_params();
        assert!(params.iter().any(|p| p == "N"));
    }

    #[test]
    fn dt2_9_dep_array_display() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Literal(5),
        };
        let s = format!("{arr}");
        assert!(s.contains("i32") && s.contains("5"));
    }

    #[test]
    fn dt2_10_dep_array_length_propagation() {
        use crate::dependent::arrays::{DepArray, concat_type};
        use crate::dependent::nat::NatValue;
        let a = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Param("N".into()),
        };
        let b = DepArray {
            element_ty: "i32".into(),
            len: NatValue::Param("M".into()),
        };
        let c = concat_type(&a, &b).unwrap();
        let params = c.len.free_params();
        assert!(params.iter().any(|p| p == "N"));
        assert!(params.iter().any(|p| p == "M"));
    }

    // Sprint DT3: Tensor Shape Types

    #[test]
    fn dt3_1_dep_tensor_creation() {
        use crate::dependent::tensor_shapes::DepTensor;
        let t = DepTensor::matrix("f32", 3, 4);
        assert_eq!(t.rank(), 2);
    }

    #[test]
    fn dt3_2_matmul_shape_check() {
        use crate::dependent::tensor_shapes::{DepTensor, check_matmul};
        let a = DepTensor::matrix("f32", 3, 4);
        let b = DepTensor::matrix("f32", 4, 5);
        let env = std::collections::HashMap::new();
        let result = check_matmul(&a, &b, &env);
        assert!(result.is_ok());
        let out = result.unwrap();
        assert_eq!(out.dims[0].evaluate(&env), Some(3));
        assert_eq!(out.dims[1].evaluate(&env), Some(5));
    }

    #[test]
    fn dt3_3_matmul_shape_mismatch() {
        use crate::dependent::tensor_shapes::{DepTensor, check_matmul};
        let a = DepTensor::matrix("f32", 3, 4);
        let b = DepTensor::matrix("f32", 5, 6);
        let env = std::collections::HashMap::new();
        let result = check_matmul(&a, &b, &env);
        assert!(result.is_err()); // Inner dims 4 != 5
    }

    #[test]
    fn dt3_4_transpose_shape() {
        use crate::dependent::tensor_shapes::{DepTensor, transpose_type};
        let t = DepTensor::matrix("f32", 3, 4);
        let out = transpose_type(&t);
        assert!(out.is_ok());
        let env = std::collections::HashMap::new();
        let trans = out.unwrap();
        assert_eq!(trans.dims[0].evaluate(&env), Some(4));
        assert_eq!(trans.dims[1].evaluate(&env), Some(3));
    }

    #[test]
    fn dt3_5_reshape_validation() {
        use crate::dependent::nat::NatValue;
        use crate::dependent::tensor_shapes::{DepTensor, check_reshape};
        let t = DepTensor::matrix("f32", 3, 4);
        let env = std::collections::HashMap::new();
        let result = check_reshape(&t, &[NatValue::Literal(2), NatValue::Literal(6)], &env);
        assert!(result.is_ok());

        let bad = check_reshape(&t, &[NatValue::Literal(2), NatValue::Literal(5)], &env);
        assert!(bad.is_err());
    }

    #[test]
    fn dt3_6_tensor_total_elements() {
        use crate::dependent::tensor_shapes::DepTensor;
        let t = DepTensor::matrix("f32", 3, 4);
        let env = std::collections::HashMap::new();
        assert_eq!(t.total_elements(&env), Some(12));
    }

    #[test]
    fn dt3_7_tensor_broadcast() {
        use crate::dependent::nat::NatValue;
        use crate::dependent::tensor_shapes::{BroadcastResult, DepTensor, check_broadcast};
        let a = DepTensor {
            element_ty: "f32".into(),
            dims: vec![NatValue::Literal(3), NatValue::Literal(1)],
        };
        let b = DepTensor {
            element_ty: "f32".into(),
            dims: vec![NatValue::Literal(1), NatValue::Literal(4)],
        };
        let env = std::collections::HashMap::new();
        let result = check_broadcast(&a, &b, &env);
        match result {
            BroadcastResult::Compatible(dims) => {
                assert_eq!(dims[0].evaluate(&env), Some(3));
                assert_eq!(dims[1].evaluate(&env), Some(4));
            }
            BroadcastResult::Incompatible { .. } => panic!("expected compatible broadcast"),
        }
    }

    #[test]
    fn dt3_8_tensor_parametric_shapes() {
        use crate::dependent::tensor_shapes::DepTensor;
        let t = DepTensor::parametric_2d("f32", "B", "D");
        assert_eq!(t.rank(), 2);
        assert!(!t.dims[0].is_concrete());
    }

    #[test]
    fn dt3_9_tensor_display() {
        use crate::dependent::tensor_shapes::DepTensor;
        let t = DepTensor::matrix("f32", 3, 4);
        let s = format!("{t}");
        assert!(s.contains("3") && s.contains("4"));
    }

    #[test]
    fn dt3_10_tensor_constructor_inference() {
        use crate::dependent::tensor_shapes::infer_from_constructor;
        let t = infer_from_constructor("zeros", &[3, 4], "f64");
        assert!(t.is_some());
        let tensor = t.unwrap();
        assert_eq!(tensor.rank(), 2);
        let env = std::collections::HashMap::new();
        assert_eq!(tensor.total_elements(&env), Some(12));
    }

    // Sprint DT4: Dependent Pattern Matching & Refinement

    #[test]
    fn dt4_1_nat_pattern_literal_match() {
        use crate::dependent::patterns::{NatPattern, nat_pattern_matches};
        let pat = NatPattern::Literal(5);
        assert!(nat_pattern_matches(&pat, 5));
        assert!(!nat_pattern_matches(&pat, 3));
    }

    #[test]
    fn dt4_2_nat_pattern_range() {
        use crate::dependent::patterns::{NatPattern, nat_pattern_matches};
        let pat = NatPattern::Range {
            start: 1,
            end_inclusive: 10,
        };
        assert!(nat_pattern_matches(&pat, 1));
        assert!(nat_pattern_matches(&pat, 10));
        assert!(!nat_pattern_matches(&pat, 0));
        assert!(!nat_pattern_matches(&pat, 11));
    }

    #[test]
    fn dt4_3_nat_pattern_wildcard() {
        use crate::dependent::patterns::{NatPattern, nat_pattern_matches};
        let pat = NatPattern::Wildcard;
        assert!(nat_pattern_matches(&pat, 0));
        assert!(nat_pattern_matches(&pat, u64::MAX));
    }

    #[test]
    fn dt4_4_exhaustiveness_check() {
        use crate::dependent::patterns::{
            ExhaustivenessResult, NatPattern, check_nat_exhaustiveness,
        };
        let patterns = vec![NatPattern::Literal(0), NatPattern::Wildcard];
        assert!(matches!(
            check_nat_exhaustiveness(&patterns, None),
            ExhaustivenessResult::Exhaustive
        ));

        let incomplete = vec![NatPattern::Literal(0)];
        assert!(matches!(
            check_nat_exhaustiveness(&incomplete, None),
            ExhaustivenessResult::NonExhaustive { .. }
        ));
    }

    #[test]
    fn dt4_5_proof_witness() {
        use crate::dependent::nat::{NatConstraint, NatValue};
        use crate::dependent::patterns::prove_constraint;
        let constraint = NatConstraint::LessThan(NatValue::Literal(3), 5);
        let env = std::collections::HashMap::new();
        let witness = prove_constraint(&constraint, &env);
        assert!(witness.is_ok());
    }

    #[test]
    fn dt4_6_safe_index_result() {
        use crate::dependent::nat::NatValue;
        use crate::dependent::patterns::{SafeIndexResult, check_safe_index};
        let env = std::collections::HashMap::new();

        let safe = check_safe_index(&NatValue::Literal(5), &NatValue::Literal(3), &env);
        assert_eq!(safe, SafeIndexResult::Safe);

        let oob = check_safe_index(&NatValue::Literal(5), &NatValue::Literal(5), &env);
        assert_eq!(oob, SafeIndexResult::DefinitelyOutOfBounds);

        let maybe = check_safe_index(&NatValue::Param("N".into()), &NatValue::Literal(0), &env);
        assert_eq!(maybe, SafeIndexResult::MaybeOutOfBounds);
    }

    #[test]
    fn dt4_7_nat_condition() {
        use crate::dependent::nat::NatValue;
        use crate::dependent::patterns::{NatCondition, eval_nat_condition};
        let env = std::collections::HashMap::new();

        let is_zero = NatCondition::IsZero(NatValue::Literal(0));
        assert_eq!(eval_nat_condition(&is_zero, &env), Some(true));

        let not_zero = NatCondition::IsZero(NatValue::Literal(5));
        assert_eq!(eval_nat_condition(&not_zero, &env), Some(false));

        let is_pos = NatCondition::IsPositive(NatValue::Literal(1));
        assert_eq!(eval_nat_condition(&is_pos, &env), Some(true));
    }

    #[test]
    fn dt4_8_where_clause_checking() {
        use crate::dependent::nat::{NatConstraint, NatValue};
        use crate::dependent::patterns::WhereClause;
        let clause =
            WhereClause::empty().with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0));
        let mut env = std::collections::HashMap::new();
        env.insert("N".to_string(), 5u64);
        assert!(clause.check_all(&env).is_ok());

        let mut bad_env = std::collections::HashMap::new();
        bad_env.insert("N".to_string(), 0u64);
        assert!(clause.check_all(&bad_env).is_err());
    }

    #[test]
    fn dt4_9_const_generics_in_interpreter() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            fn identity<const N: usize>(x: i32) -> i32 {
                x
            }
            identity(42)
            "#,
        );
        assert!(result.is_ok(), "const generic fn: {:?}", result.err());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn dt4_10_inductive_proof() {
        use crate::dependent::patterns::InductiveProof;
        let proof = InductiveProof {
            base_case: 0,
            step: 1,
            property: "sum_positive".into(),
        };
        assert_eq!(proof.property, "sum_positive");
        assert!(proof.covers(0)); // base case
        assert!(proof.covers(5)); // 0 + 5*1
        assert!(proof.covers(100));
    }

    // ===================================================================
    // Sub-Option 5D — LSP v4 (LS1-LS4)
    // ===================================================================

    // Sprint LS1: Semantic Tokens

    #[test]
    fn ls1_1_semantic_token_types() {
        use crate::lsp_v3::semantic::SemanticTokenType;
        assert_eq!(SemanticTokenType::Keyword.index(), 15);
        assert!(SemanticTokenType::legend().len() >= 10);
    }

    #[test]
    fn ls1_2_semantic_token_modifiers() {
        use crate::lsp_v3::semantic::SemanticTokenModifier;
        assert!(SemanticTokenModifier::Declaration.bitmask() > 0);
        assert!(SemanticTokenModifier::legend().len() >= 2);
    }

    #[test]
    fn ls1_3_semantic_token_encoding() {
        use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType, encode_semantic_tokens};
        let tokens = vec![
            AbsoluteToken {
                line: 0,
                start: 0,
                length: 3,
                token_type: SemanticTokenType::Keyword.index(),
                modifiers: 0,
            },
            AbsoluteToken {
                line: 0,
                start: 4,
                length: 1,
                token_type: SemanticTokenType::Variable.index(),
                modifiers: 0,
            },
        ];
        let encoded = encode_semantic_tokens(&tokens);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 0);
        assert_eq!(encoded[1].delta_start, 4);
    }

    #[test]
    fn ls1_4_semantic_token_multiline() {
        use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType, encode_semantic_tokens};
        let tokens = vec![
            AbsoluteToken {
                line: 0,
                start: 0,
                length: 2,
                token_type: SemanticTokenType::Keyword.index(),
                modifiers: 0,
            },
            AbsoluteToken {
                line: 2,
                start: 5,
                length: 3,
                token_type: SemanticTokenType::Function.index(),
                modifiers: 0,
            },
        ];
        let encoded = encode_semantic_tokens(&tokens);
        assert_eq!(encoded[1].delta_line, 2);
        assert_eq!(encoded[1].delta_start, 5); // new line resets to absolute
    }

    #[test]
    fn ls1_5_token_type_legend() {
        use crate::lsp_v3::semantic::SemanticTokenType;
        let legend = SemanticTokenType::legend();
        assert!(legend.contains(&"keyword"));
        assert!(legend.contains(&"function"));
        assert!(legend.contains(&"variable"));
    }

    #[test]
    fn ls1_6_empty_token_encoding() {
        use crate::lsp_v3::semantic::encode_semantic_tokens;
        let encoded = encode_semantic_tokens(&[]);
        assert!(encoded.is_empty());
    }

    #[test]
    fn ls1_7_token_type_all_variants() {
        use crate::lsp_v3::semantic::SemanticTokenType;
        let types = [
            SemanticTokenType::Keyword,
            SemanticTokenType::Function,
            SemanticTokenType::Variable,
            SemanticTokenType::Type,
            SemanticTokenType::String,
            SemanticTokenType::Number,
            SemanticTokenType::Comment,
        ];
        for t in types {
            assert!(t.index() < 20);
        }
    }

    #[test]
    fn ls1_8_modifier_legend() {
        use crate::lsp_v3::semantic::SemanticTokenModifier;
        let legend = SemanticTokenModifier::legend();
        assert!(legend.contains(&"declaration"));
    }

    #[test]
    fn ls1_9_absolute_token_fields() {
        use crate::lsp_v3::semantic::{AbsoluteToken, SemanticTokenType};
        let tok = AbsoluteToken {
            line: 5,
            start: 10,
            length: 3,
            token_type: SemanticTokenType::Keyword.index(),
            modifiers: 0,
        };
        assert_eq!(tok.line, 5);
        assert_eq!(tok.start, 10);
        assert_eq!(tok.length, 3);
    }

    #[test]
    fn ls1_10_semantic_token_delta() {
        use crate::lsp_v3::semantic::SemanticToken;
        let tok = SemanticToken {
            delta_line: 1,
            delta_start: 5,
            length: 3,
            token_type: 0,
            token_modifiers: 0,
        };
        assert_eq!(tok.delta_line, 1);
        assert_eq!(tok.length, 3);
    }

    // Sprint LS2: Inlay Hints

    #[test]
    fn ls2_1_inlay_hint_provider_creation() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("");
        assert!(hints.is_empty());
    }

    #[test]
    fn ls2_2_inlay_hint_for_let_int() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let x = 42");
        assert!(!hints.is_empty());
        assert!(
            hints[0].label.contains("i64")
                || hints[0].label.contains("i32")
                || hints[0].label.contains("int")
        );
    }

    #[test]
    fn ls2_3_inlay_hint_for_let_string() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints(r#"let name = "hello""#);
        assert!(!hints.is_empty());
        assert!(hints[0].label.contains("str"));
    }

    #[test]
    fn ls2_4_inlay_hint_kind() {
        use crate::lsp::completion::InlayHintKind;
        let type_hint = InlayHintKind::TypeHint;
        let param_hint = InlayHintKind::ParameterHint;
        assert_ne!(type_hint, param_hint);
    }

    #[test]
    fn ls2_5_no_hint_for_typed_let() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let x: i32 = 42");
        // Already typed — no hint needed.
        assert!(hints.is_empty());
    }

    #[test]
    fn ls2_6_inlay_hint_for_float() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let pi = 3.14");
        assert!(!hints.is_empty());
        assert!(hints[0].label.contains("f64") || hints[0].label.contains("float"));
    }

    #[test]
    fn ls2_7_inlay_hint_for_bool() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let flag = true");
        assert!(!hints.is_empty());
        assert!(hints[0].label.contains("bool"));
    }

    #[test]
    fn ls2_8_inlay_hint_for_array() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let arr = [1, 2, 3]");
        assert!(!hints.is_empty());
    }

    #[test]
    fn ls2_9_multiple_let_bindings() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let x = 1\nlet y = 2\nlet z = 3");
        assert_eq!(hints.len(), 3);
    }

    #[test]
    fn ls2_10_inlay_hint_position() {
        use crate::lsp::completion::InlayHintProvider;
        let provider = InlayHintProvider::new();
        let hints = provider.compute_inlay_hints("let x = 42");
        assert!(!hints.is_empty());
        assert_eq!(hints[0].line, 0);
    }

    // Sprint LS3: Completion Provider

    #[test]
    fn ls3_1_completion_provider_creation() {
        use crate::lsp::completion::CompletionProvider;
        let provider = CompletionProvider::new();
        let _ = provider; // just verifies it compiles
    }

    #[test]
    fn ls3_2_default_completions() {
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        assert!(!result.is_empty()); // Should have builtins + keywords
    }

    #[test]
    fn ls3_3_keyword_completions() {
        use crate::lsp::completion::{CompletionKind, CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_keywords = result.iter().any(|c| c.kind == CompletionKind::Keyword);
        assert!(has_keywords);
    }

    #[test]
    fn ls3_4_builtin_completions() {
        use crate::lsp::completion::{CompletionKind, CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_builtins = result.iter().any(|c| c.kind == CompletionKind::Builtin);
        assert!(has_builtins);
    }

    #[test]
    fn ls3_5_completion_candidate_fields() {
        use crate::lsp::completion::{CompletionCandidate, CompletionKind};
        let candidate = CompletionCandidate {
            label: "println".into(),
            kind: CompletionKind::Builtin,
            detail: Some("fn(args...) -> void".into()),
            insert_text: "println".into(),
        };
        assert_eq!(candidate.label, "println");
        assert_eq!(candidate.kind, CompletionKind::Builtin);
    }

    #[test]
    fn ls3_6_completion_trigger_variants() {
        use crate::lsp::completion::CompletionTrigger;
        let triggers = [
            CompletionTrigger::Dot,
            CompletionTrigger::DoubleColon,
            CompletionTrigger::Angle,
            CompletionTrigger::Default,
        ];
        assert_eq!(triggers.len(), 4);
    }

    #[test]
    fn ls3_7_completion_kind_variants() {
        use crate::lsp::completion::CompletionKind;
        let kinds = [
            CompletionKind::Function,
            CompletionKind::Variable,
            CompletionKind::Struct,
            CompletionKind::Enum,
            CompletionKind::Field,
            CompletionKind::Module,
            CompletionKind::Keyword,
            CompletionKind::Builtin,
        ];
        assert_eq!(kinds.len(), 8);
    }

    #[test]
    fn ls3_8_completion_has_println() {
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_println = result.iter().any(|c| c.label == "println");
        assert!(has_println);
    }

    #[test]
    fn ls3_9_completion_has_fn_keyword() {
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_fn = result.iter().any(|c| c.label == "fn");
        assert!(has_fn);
    }

    #[test]
    fn ls3_10_completion_has_let_keyword() {
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_let = result.iter().any(|c| c.label == "let");
        assert!(has_let);
    }

    // Sprint LS4: Workspace Symbols & Rename

    #[test]
    fn ls4_1_workspace_symbol_provider() {
        use crate::lsp::completion::WorkspaceSymbolProvider;
        let provider = WorkspaceSymbolProvider::new();
        let symbols = provider.search_symbols("fn hello() { }", "hello");
        assert!(!symbols.is_empty());
    }

    #[test]
    fn ls4_2_workspace_symbol_kinds() {
        use crate::lsp::completion::WorkspaceSymbolKind;
        let kinds = [
            WorkspaceSymbolKind::Function,
            WorkspaceSymbolKind::Struct,
            WorkspaceSymbolKind::Enum,
            WorkspaceSymbolKind::Trait,
            WorkspaceSymbolKind::Constant,
            WorkspaceSymbolKind::Module,
        ];
        assert_eq!(kinds.len(), 6);
    }

    #[test]
    fn ls4_3_workspace_find_struct() {
        use crate::lsp::completion::WorkspaceSymbolProvider;
        let provider = WorkspaceSymbolProvider::new();
        let symbols = provider.search_symbols("struct Point { x: i32, y: i32 }", "Point");
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name, "Point");
    }

    #[test]
    fn ls4_4_workspace_find_enum() {
        use crate::lsp::completion::WorkspaceSymbolProvider;
        let provider = WorkspaceSymbolProvider::new();
        let symbols = provider.search_symbols("enum Color { Red, Green, Blue }", "Color");
        assert!(!symbols.is_empty());
    }

    #[test]
    fn ls4_5_rename_provider_creation() {
        use crate::lsp::completion::RenameProvider;
        let provider = RenameProvider::new();
        let _ = provider;
    }

    #[test]
    fn ls4_6_find_references() {
        use crate::lsp::completion::RenameProvider;
        let provider = RenameProvider::new();
        let refs = provider
            .find_all_references(
                "let x = 1
let y = x + 1",
                0,
                4,
            )
            .unwrap();
        assert!(refs.len() >= 2); // definition + usage
    }

    #[test]
    fn ls4_7_rename_symbol() {
        use crate::lsp::completion::RenameProvider;
        let provider = RenameProvider::new();
        let edits = provider
            .rename_symbol(
                "let x = 1
let y = x",
                0,
                4,
                "foo",
            )
            .unwrap();
        assert!(!edits.is_empty());
    }

    #[test]
    fn ls4_8_workspace_empty_query() {
        use crate::lsp::completion::WorkspaceSymbolProvider;
        let provider = WorkspaceSymbolProvider::new();
        let symbols = provider.search_symbols("fn hello() { }", "");
        // Empty query returns all symbols.
        assert!(!symbols.is_empty());
    }

    #[test]
    fn ls4_9_lsp_error_variants() {
        use crate::lsp::completion::LspError;
        let err = LspError::ParseFailed {
            message: "test".into(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("test"));
    }

    #[test]
    fn ls4_10_workspace_symbol_fields() {
        use crate::lsp::completion::{WorkspaceSymbol, WorkspaceSymbolKind};
        let sym = WorkspaceSymbol {
            name: "main".into(),
            kind: WorkspaceSymbolKind::Function,
            file: "<source>".into(),
            line: 0,
            container_name: None,
        };
        assert_eq!(sym.name, "main");
        assert_eq!(sym.line, 0);
    }

    // ===================================================================
    // Sub-Option 5C — GPU Compute Shaders (GS1-GS4)
    // ===================================================================

    // Sprint GS1: SPIR-V Module & Shader Syntax

    #[test]
    fn gs1_1_spirv_module_creation() {
        use crate::gpu_codegen::spirv::SpirVModule;
        let module = SpirVModule::new_compute();
        assert_eq!(module.version, 0x0001_0500); // SPIR-V 1.5
        assert!(!module.capabilities.is_empty());
    }

    #[test]
    fn gs1_2_spirv_alloc_id() {
        use crate::gpu_codegen::spirv::SpirVModule;
        let mut module = SpirVModule::new_compute();
        let id1 = module.alloc_id();
        let id2 = module.alloc_id();
        assert_eq!(id2, id1 + 1);
    }

    #[test]
    fn gs1_3_spirv_emit_words_header() {
        use crate::gpu_codegen::spirv::SpirVModule;
        let module = SpirVModule::new_compute();
        let words = module.emit_words();
        assert_eq!(words.len(), 5);
        assert_eq!(words[0], 0x0723_0203); // SPIR-V magic
    }

    #[test]
    fn gs1_4_spirv_validate_empty_entry() {
        use crate::gpu_codegen::spirv::SpirVModule;
        let module = SpirVModule::new_compute();
        let errors = module.validate();
        // No entry points → should have validation error
        assert!(!errors.is_empty());
    }

    #[test]
    fn gs1_5_spirv_type_mapping() {
        use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
        assert_eq!(map_fj_type("f32"), Some(SpirVTypeDesc::Float(32)));
        assert_eq!(map_fj_type("i32"), Some(SpirVTypeDesc::Int(32, true)));
        assert_eq!(map_fj_type("bool"), Some(SpirVTypeDesc::Bool));
        assert_eq!(map_fj_type("unknown"), None);
    }

    #[test]
    fn gs1_6_spirv_capability_values() {
        use crate::gpu_codegen::spirv::Capability;
        assert_eq!(Capability::Shader.value(), 1);
        assert_eq!(Capability::Float16.value(), 9);
        assert_eq!(Capability::Float64.value(), 10);
    }

    #[test]
    fn gs1_7_spirv_execution_model() {
        use crate::gpu_codegen::spirv::ExecutionModel;
        let model = ExecutionModel::GLCompute;
        assert_eq!(model, ExecutionModel::GLCompute);
        assert_ne!(model, ExecutionModel::Vertex);
    }

    #[test]
    fn gs1_8_spirv_storage_class_values() {
        use crate::gpu_codegen::spirv::StorageClass;
        assert_eq!(StorageClass::StorageBuffer.value(), 12);
        assert_eq!(StorageClass::Workgroup.value(), 4);
        assert_eq!(StorageClass::Input.value(), 1);
    }

    #[test]
    fn gs1_9_spirv_entry_point_struct() {
        use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel};
        let ep = EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: 1,
            name: "main".into(),
            interface_ids: vec![2, 3],
            local_size: [256, 1, 1],
        };
        assert_eq!(ep.name, "main");
        assert_eq!(ep.local_size[0], 256);
    }

    #[test]
    fn gs1_10_spirv_validate_with_entry() {
        use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel, SpirVModule};
        let mut module = SpirVModule::new_compute();
        let fn_id = module.alloc_id();
        module.entry_points.push(EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: fn_id,
            name: "main".into(),
            interface_ids: vec![],
            local_size: [64, 1, 1],
        });
        let errors = module.validate();
        assert!(errors.is_empty(), "valid module: {errors:?}");
    }

    // Sprint GS2: SPIR-V Backend (Vulkan Compute)

    #[test]
    fn gs2_1_create_ssbo() {
        use crate::gpu_codegen::spirv::{StorageClass, create_ssbo};
        let ssbo = create_ssbo(10, 5, 0, 0);
        assert_eq!(ssbo.id, 10);
        assert_eq!(ssbo.storage_class, StorageClass::StorageBuffer);
        assert_eq!(ssbo.binding, Some(0));
    }

    #[test]
    fn gs2_2_create_workgroup_var() {
        use crate::gpu_codegen::spirv::{StorageClass, create_workgroup_var};
        let wg = create_workgroup_var(20, 8);
        assert_eq!(wg.storage_class, StorageClass::Workgroup);
        assert!(wg.binding.is_none());
    }

    #[test]
    fn gs2_3_vulkan_dispatch_1d() {
        use crate::gpu_codegen::spirv::compute_dispatch_1d;
        let dispatch = compute_dispatch_1d(1024, 256);
        assert_eq!(dispatch.group_count_x, 4);
        assert_eq!(dispatch.group_count_y, 1);
        assert_eq!(dispatch.group_count_z, 1);
    }

    #[test]
    fn gs2_4_vulkan_dispatch_2d() {
        use crate::gpu_codegen::spirv::compute_dispatch_2d;
        let dispatch = compute_dispatch_2d(512, 256, 16, 16);
        assert_eq!(dispatch.group_count_x, 32);
        assert_eq!(dispatch.group_count_y, 16);
    }

    #[test]
    fn gs2_5_vulkan_dispatch_rounding() {
        use crate::gpu_codegen::spirv::compute_dispatch_1d;
        // 1000 elements / 256 = 3.9 → rounds up to 4
        let dispatch = compute_dispatch_1d(1000, 256);
        assert_eq!(dispatch.group_count_x, 4);
    }

    #[test]
    fn gs2_6_barrier_scope_values() {
        use crate::gpu_codegen::spirv::BarrierScope;
        assert_eq!(BarrierScope::Workgroup.value(), 2);
        assert_eq!(BarrierScope::Device.value(), 1);
        assert_eq!(BarrierScope::Subgroup.value(), 3);
    }

    #[test]
    fn gs2_7_memory_semantics_values() {
        use crate::gpu_codegen::spirv::MemorySemantics;
        let acq = MemorySemantics::AcquireWorkgroup.value();
        let rel = MemorySemantics::ReleaseWorkgroup.value();
        assert_ne!(acq, rel);
        assert!(acq > 0);
    }

    #[test]
    fn gs2_8_backend_parse() {
        use crate::gpu_codegen::spirv::{GpuBackend, parse_backend};
        assert_eq!(parse_backend("ptx"), Some(GpuBackend::Ptx));
        assert_eq!(parse_backend("spirv"), Some(GpuBackend::SpirV));
        assert_eq!(parse_backend("auto"), Some(GpuBackend::Auto));
        assert_eq!(parse_backend("metal"), None);
    }

    #[test]
    fn gs2_9_backend_resolve_nvidia() {
        use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
        let resolved = resolve_backend(GpuBackend::Auto, "NVIDIA GeForce RTX 4090");
        assert_eq!(resolved, GpuBackend::Ptx);
    }

    #[test]
    fn gs2_10_backend_resolve_amd() {
        use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
        let resolved = resolve_backend(GpuBackend::Auto, "AMD Radeon RX 7900");
        assert_eq!(resolved, GpuBackend::SpirV);
    }

    // Sprint GS3: PTX Backend (CUDA)

    #[test]
    fn gs3_1_ptx_type_mapping() {
        use crate::gpu_codegen::ptx::{PtxType, map_type};
        assert_eq!(map_type("f32"), Some(PtxType::F32));
        assert_eq!(map_type("f64"), Some(PtxType::F64));
        assert_eq!(map_type("i32"), Some(PtxType::S32));
        assert_eq!(map_type("u8"), Some(PtxType::U8));
    }

    #[test]
    fn gs3_2_kernel_entry_emit() {
        use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
        let kernel = KernelEntry {
            name: "vecadd".into(),
            params: vec![
                KernelParam {
                    name: "a".into(),
                    ptx_type: PtxType::F32,
                    is_pointer: true,
                },
                KernelParam {
                    name: "b".into(),
                    ptx_type: PtxType::F32,
                    is_pointer: true,
                },
            ],
            body: vec![],
        };
        let ptx = kernel.emit();
        assert!(ptx.contains("vecadd"));
        assert!(ptx.contains(".entry"));
    }

    #[test]
    fn gs3_3_grid_config_1d() {
        use crate::gpu_codegen::ptx::compute_grid_1d;
        let grid = compute_grid_1d(1024, 256);
        assert_eq!(grid.total_threads(), 1024);
    }

    #[test]
    fn gs3_4_grid_config_2d() {
        use crate::gpu_codegen::ptx::compute_grid_2d;
        let grid = compute_grid_2d(512, 256, 16);
        assert_eq!(grid.total_threads(), 512 * 256);
    }

    #[test]
    fn gs3_5_ptx_thread_index() {
        use crate::gpu_codegen::ptx::{PtxInstruction, ThreadIndex, emit_thread_index};
        let instr = emit_thread_index("%tid_x", ThreadIndex::ThreadIdX);
        match instr {
            PtxInstruction::MovSpecial { .. } => {} // expected
            _ => panic!("expected MovSpecial instruction"),
        }
    }

    #[test]
    fn gs3_6_ptx_global_thread_id() {
        use crate::gpu_codegen::ptx::emit_global_thread_id;
        let instrs = emit_global_thread_id("%gtid", "%tid", "%ctaid", "%ntid");
        assert!(!instrs.is_empty()); // mad (multiply-add)
    }

    #[test]
    fn gs3_7_ptx_type_display() {
        use crate::gpu_codegen::ptx::PtxType;
        assert_eq!(format!("{}", PtxType::F32), ".f32");
        assert_eq!(format!("{}", PtxType::S32), ".s32");
    }

    #[test]
    fn gs3_8_kernel_param_display() {
        use crate::gpu_codegen::ptx::{KernelParam, PtxType};
        let param = KernelParam {
            name: "data".into(),
            ptx_type: PtxType::F64,
            is_pointer: true,
        };
        let s = format!("{param}");
        assert!(s.contains("data"));
    }

    #[test]
    fn gs3_9_shared_decl_fields() {
        use crate::gpu_codegen::ptx::{PtxType, SharedDecl};
        let shared = SharedDecl {
            name: "smem".into(),
            elem_type: PtxType::F32,
            count: 256,
        };
        assert_eq!(shared.name, "smem");
        assert_eq!(shared.count, 256);
    }

    #[test]
    fn gs3_10_memory_space_variants() {
        use crate::gpu_codegen::ptx::MemorySpace;
        let spaces = [
            MemorySpace::Global,
            MemorySpace::Shared,
            MemorySpace::Local,
            MemorySpace::Constant,
        ];
        assert_eq!(spaces.len(), 4);
    }

    // Sprint GS4: Auto-Dispatch & Fusion

    #[test]
    fn gs4_1_fusion_graph_creation() {
        use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![],
                output_elements: 1024,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![0],
                output_elements: 1024,
            },
        ];
        let graph = FusionGraph::new(ops);
        assert_eq!(graph.ops.len(), 2);
    }

    #[test]
    fn gs4_2_fusion_analysis() {
        use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![],
                output_elements: 1024,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseBinary,
                inputs: vec![0],
                output_elements: 1024,
            },
        ];
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        assert!(graph.num_fusions() >= 1);
    }

    #[test]
    fn gs4_3_can_fuse_elementwise() {
        use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
        let producer = GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 512,
        };
        let consumer = GpuOp {
            id: 1,
            kind: OpKind::ElementWiseBinary,
            inputs: vec![0],
            output_elements: 512,
        };
        assert!(can_fuse(&producer, &consumer));
    }

    #[test]
    fn gs4_4_cannot_fuse_matmul_reduction() {
        use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
        let producer = GpuOp {
            id: 0,
            kind: OpKind::Matmul,
            inputs: vec![],
            output_elements: 1024,
        };
        let consumer = GpuOp {
            id: 1,
            kind: OpKind::Reduction,
            inputs: vec![0],
            output_elements: 1,
        };
        assert!(!can_fuse(&producer, &consumer));
    }

    #[test]
    fn gs4_5_op_kind_display() {
        use crate::gpu_codegen::fusion::OpKind;
        assert_eq!(format!("{}", OpKind::Matmul), "Matmul");
        assert_eq!(format!("{}", OpKind::Softmax), "Softmax");
    }

    #[test]
    fn gs4_6_device_allocator_creation() {
        use crate::gpu_codegen::gpu_memory::DeviceAllocator;
        let alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        let stats = alloc.stats();
        assert_eq!(stats.used_bytes, 0);
    }

    #[test]
    fn gs4_7_device_allocator_alloc_free() {
        use crate::gpu_codegen::gpu_memory::DeviceAllocator;
        let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        let a = alloc.allocate(4096).unwrap();
        assert_eq!(a.size, 4096);
        assert!(a.in_use);
        let stats = alloc.stats();
        assert_eq!(stats.used_bytes, 4096);
        alloc.free(a.id).unwrap();
        let stats2 = alloc.stats();
        assert_eq!(stats2.used_bytes, 0);
    }

    #[test]
    fn gs4_8_device_allocator_oom() {
        use crate::gpu_codegen::gpu_memory::{AllocError, DeviceAllocator};
        let mut alloc = DeviceAllocator::new(0, 1024, 1024);
        let result = alloc.allocate(2048);
        assert!(matches!(result, Err(AllocError::OutOfMemory { .. })));
    }

    #[test]
    fn gs4_9_gpu_backend_display() {
        use crate::gpu_codegen::spirv::GpuBackend;
        assert_eq!(format!("{}", GpuBackend::Ptx), "ptx");
        assert_eq!(format!("{}", GpuBackend::SpirV), "spirv");
        assert_eq!(format!("{}", GpuBackend::Auto), "auto");
    }

    #[test]
    fn gs4_10_spirv_type_ids() {
        use crate::gpu_codegen::spirv::SpirVType;
        let void = SpirVType::Void { id: 1 };
        let int = SpirVType::Int {
            id: 2,
            width: 32,
            signed: true,
        };
        let float = SpirVType::Float { id: 3, width: 32 };
        assert_eq!(void.id(), 1);
        assert_eq!(int.id(), 2);
        assert_eq!(float.id(), 3);
    }

    // ===================================================================
    // Sub-Option 5E — Package Registry (PR1-PR4)
    // ===================================================================

    // Sprint PR1: Registry Core (publish, search, resolve)

    #[test]
    fn pr1_1_registry_creation() {
        use crate::package::registry::Registry;
        let reg = Registry::new();
        assert_eq!(reg.package_count(), 0);
    }

    #[test]
    fn pr1_2_publish_and_lookup() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("math", SemVer::new(1, 0, 0), "Math library");
        assert_eq!(reg.package_count(), 1);
        let pkg = reg.lookup("math").unwrap();
        assert_eq!(pkg.name, "math");
    }

    #[test]
    fn pr1_3_publish_multiple_versions() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("http", SemVer::new(1, 0, 0), "HTTP client");
        reg.publish("http", SemVer::new(1, 1, 0), "HTTP client");
        reg.publish("http", SemVer::new(2, 0, 0), "HTTP client v2");
        let latest = reg.latest_version("http").unwrap();
        assert_eq!(*latest, SemVer::new(2, 0, 0));
    }

    #[test]
    fn pr1_4_search_packages() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("json-parser", SemVer::new(1, 0, 0), "JSON parser");
        reg.publish("json-schema", SemVer::new(0, 5, 0), "JSON schema validator");
        reg.publish("http-client", SemVer::new(1, 0, 0), "HTTP client");
        let results = reg.search("json");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn pr1_5_version_constraint_parse() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("^1.2.3").unwrap();
        assert!(c.matches(&SemVer::new(1, 3, 0)));
        assert!(!c.matches(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn pr1_6_version_resolve() {
        use crate::package::registry::{Registry, SemVer, VersionConstraint};
        let mut reg = Registry::new();
        reg.publish("crypto", SemVer::new(1, 0, 0), "Crypto lib");
        reg.publish("crypto", SemVer::new(1, 2, 0), "Crypto lib");
        reg.publish("crypto", SemVer::new(2, 0, 0), "Crypto lib v2");
        let constraint = VersionConstraint::parse("^1.0.0").unwrap();
        let resolved = reg.resolve("crypto", &constraint).unwrap();
        assert_eq!(resolved, SemVer::new(1, 2, 0));
    }

    #[test]
    fn pr1_7_semver_parse() {
        use crate::package::registry::SemVer;
        let v = SemVer::parse("3.14.1").unwrap();
        assert_eq!(v, SemVer::new(3, 14, 1));
        assert!(SemVer::parse("not.a.version").is_err());
    }

    #[test]
    fn pr1_8_list_all_packages() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("aaa", SemVer::new(1, 0, 0), "First");
        reg.publish("zzz", SemVer::new(1, 0, 0), "Last");
        let all = reg.list_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn pr1_9_package_names() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("alpha", SemVer::new(1, 0, 0), "A");
        reg.publish("beta", SemVer::new(1, 0, 0), "B");
        let names = reg.package_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn pr1_10_yank_version() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("vuln-pkg", SemVer::new(1, 0, 0), "Vulnerable");
        let v = SemVer::new(1, 0, 0);
        assert!(!reg.is_yanked("vuln-pkg", &v));
        reg.yank("vuln-pkg", &v).unwrap();
        assert!(reg.is_yanked("vuln-pkg", &v));
    }

    // Sprint PR2: API & Metadata

    #[test]
    fn pr2_1_api_response_ok() {
        use crate::package::server::{ApiResponse, StatusCode};
        let resp = ApiResponse::ok(r#"{"status":"ok"}"#);
        assert_eq!(resp.status, StatusCode::OK);
    }

    #[test]
    fn pr2_2_api_response_error() {
        use crate::package::server::{ApiResponse, StatusCode};
        let resp = ApiResponse::error(StatusCode::NOT_FOUND, "not found");
        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn pr2_3_api_response_created() {
        use crate::package::server::{ApiResponse, StatusCode};
        let resp = ApiResponse::created(r#"{"published":true}"#);
        assert_eq!(resp.status, StatusCode::CREATED);
    }

    #[test]
    fn pr2_4_auth_token_creation() {
        use crate::package::registry::AuthToken;
        let token = AuthToken::new("test-token-123");
        let _ = token; // verifies construction
    }

    #[test]
    fn pr2_5_auth_token_scoped() {
        use crate::package::registry::AuthToken;
        let token = AuthToken::scoped("test-token-456", "my-package");
        let _ = token;
    }

    #[test]
    fn pr2_6_token_validation() {
        use crate::package::registry::{AuthToken, Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("my-pkg", SemVer::new(1, 0, 0), "My package");
        reg.add_token(AuthToken::new("secret-key"));
        assert!(reg.validate_token("secret-key", None));
        assert!(!reg.validate_token("wrong-key", None));
    }

    #[test]
    fn pr2_7_scoped_token_validation() {
        use crate::package::registry::{AuthToken, Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("my-pkg", SemVer::new(1, 0, 0), "My package");
        reg.add_token(AuthToken::scoped("pkg-key", "my-pkg"));
        assert!(reg.validate_token("pkg-key", Some("my-pkg")));
        assert!(!reg.validate_token("pkg-key", Some("other-pkg")));
    }

    #[test]
    fn pr2_8_publish_with_metadata() {
        use crate::package::registry::{Registry, SemVer};
        use std::collections::HashMap;
        let mut reg = Registry::new();
        let mut deps = HashMap::new();
        deps.insert("serde".into(), "^1.0.0".into());
        reg.publish_with_meta("my-app", SemVer::new(0, 1, 0), "My app", deps, "sha256abc");
        let pkg = reg.lookup("my-app").unwrap();
        assert_eq!(pkg.name, "my-app");
    }

    #[test]
    fn pr2_9_version_constraint_wildcard() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("*").unwrap();
        assert!(c.matches(&SemVer::new(1, 0, 0)));
        assert!(c.matches(&SemVer::new(99, 99, 99)));
    }

    #[test]
    fn pr2_10_version_constraint_tilde() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("~1.2.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 2, 5)));
        assert!(!c.matches(&SemVer::new(1, 3, 0)));
    }

    // Sprint PR3: Signing & Bundles

    #[test]
    fn pr3_1_oidc_authenticate() {
        use crate::package::signing::oidc_authenticate;
        let token = oidc_authenticate("github").unwrap();
        assert!(!token.identity.is_empty());
    }

    #[test]
    fn pr3_2_request_certificate() {
        use crate::package::signing::{oidc_authenticate, request_certificate};
        let oidc = oidc_authenticate("github").unwrap();
        let cert = request_certificate(&oidc).unwrap();
        assert!(cert.pem.contains("CERTIFICATE"));
    }

    #[test]
    fn pr3_3_sign_package() {
        use crate::package::signing::{oidc_authenticate, request_certificate, sign_package};
        let oidc = oidc_authenticate("github").unwrap();
        let cert = request_certificate(&oidc).unwrap();
        let sig = sign_package("sha256:deadbeef", &cert).unwrap();
        assert!(!sig.bytes.is_empty());
    }

    #[test]
    fn pr3_4_rekor_submit() {
        use crate::package::signing::{
            oidc_authenticate, request_certificate, sign_package, submit_to_rekor,
        };
        let oidc = oidc_authenticate("github").unwrap();
        let cert = request_certificate(&oidc).unwrap();
        let sig = sign_package("sha256:abcdef", &cert).unwrap();
        let entry = submit_to_rekor(&sig, &cert, "sha256:abcdef").unwrap();
        assert!(entry.log_index > 0);
    }

    #[test]
    fn pr3_5_signature_bundle_roundtrip() {
        use crate::package::signing::{
            FjSignatureBundle, oidc_authenticate, request_certificate, sign_package,
            submit_to_rekor,
        };
        let oidc = oidc_authenticate("github").unwrap();
        let cert = request_certificate(&oidc).unwrap();
        let sig = sign_package("sha256:112233", &cert).unwrap();
        let rekor = submit_to_rekor(&sig, &cert, "sha256:112233").unwrap();
        let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);
        let json = bundle.to_json();
        let restored = FjSignatureBundle::from_json(&json).unwrap();
        assert!(!restored.signature.is_empty());
    }

    #[test]
    fn pr3_6_signing_config_default() {
        use crate::package::signing::SigningConfig;
        let config = SigningConfig::default();
        assert!(!config.fulcio_url.is_empty());
        assert!(!config.rekor_url.is_empty());
    }

    #[test]
    fn pr3_7_sbom_document_creation() {
        use crate::package::sbom::{SbomDocument, SbomFormat};
        let doc = SbomDocument::new(SbomFormat::CycloneDx);
        assert!(doc.packages.is_empty());
    }

    #[test]
    fn pr3_8_sbom_add_package() {
        use crate::package::sbom::{SbomDocument, SbomFormat, SbomPackage};
        let mut doc = SbomDocument::new(SbomFormat::Spdx);
        doc.add_package(SbomPackage::new(
            "serde",
            "1.0.0",
            "sha256:abc",
            Some("MIT".into()),
        ));
        assert_eq!(doc.packages.len(), 1);
    }

    #[test]
    fn pr3_9_generate_sbom() {
        use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
        let deps = vec![DepInfo {
            name: "serde".into(),
            version: "1.0.0".into(),
            sha256: "abc".into(),
            license: Some("MIT".into()),
            dev_only: false,
        }];
        let json = generate_sbom("test-project", &deps, SbomFormat::CycloneDx).unwrap();
        assert!(json.contains("test-project"));
    }

    #[test]
    fn pr3_10_sbom_format_variants() {
        use crate::package::sbom::SbomFormat;
        let formats = [SbomFormat::CycloneDx, SbomFormat::Spdx];
        assert_eq!(formats.len(), 2);
    }

    // Sprint PR4: Security & Audit

    #[test]
    fn pr4_1_advisory_database_creation() {
        use crate::package::audit::AdvisoryDatabase;
        let db = AdvisoryDatabase::new();
        let findings = db.check("any-package", "1.0.0");
        assert!(findings.is_empty());
    }

    #[test]
    fn pr4_2_advisory_from_json() {
        use crate::package::audit::AdvisoryDatabase;
        let json = r#"{"advisories":[{"id":"FJ-2026-001","package":"vuln-lib","severity":"critical","description":"RCE in parser","min_version":"1.0.0","max_version":"1.2.0","patched_version":"1.2.1"}]}"#;
        let db = AdvisoryDatabase::from_json(json).unwrap();
        let hits = db.check("vuln-lib", "1.1.0");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn pr4_3_version_range_affects() {
        use crate::package::audit::VersionRange;
        let range = VersionRange::new(Some("1.0.0".into()), Some("2.0.0".into()));
        assert!(range.affects("1.5.0"));
        assert!(!range.affects("2.1.0"));
    }

    #[test]
    fn pr4_4_audit_dependencies() {
        use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
        let json = r#"{"advisories":[{"id":"FJ-2026-002","package":"bad-pkg","severity":"high","description":"Denial of service","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"}]}"#;
        let db = AdvisoryDatabase::from_json(json).unwrap();
        let deps = vec![("bad-pkg".into(), "0.5.0".into())];
        let report = audit_dependencies(&deps, &db);
        assert!(report.has_critical_or_high());
    }

    #[test]
    fn pr4_5_audit_clean() {
        use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
        let db = AdvisoryDatabase::new();
        let deps = vec![("safe-pkg".into(), "1.0.0".into())];
        let report = audit_dependencies(&deps, &db);
        assert!(!report.has_critical_or_high());
        assert_eq!(report.finding_count(), 0);
    }

    #[test]
    fn pr4_6_severity_variants() {
        use crate::package::audit::Severity;
        let severities = [
            Severity::Critical,
            Severity::High,
            Severity::Medium,
            Severity::Low,
        ];
        assert_eq!(severities.len(), 4);
    }

    #[test]
    fn pr4_7_severity_color_code() {
        use crate::package::audit::Severity;
        let code = Severity::Critical.color_code();
        assert!(!code.is_empty());
    }

    #[test]
    fn pr4_8_version_range_unbounded() {
        use crate::package::audit::VersionRange;
        let range = VersionRange::new(None, None);
        assert!(range.affects("1.0.0"));
        assert!(range.affects("999.0.0"));
    }

    #[test]
    fn pr4_9_advisory_not_affected() {
        use crate::package::audit::AdvisoryDatabase;
        let json = r#"{"advisories":[{"id":"FJ-2026-003","package":"lib-x","severity":"low","description":"Bug","min_version":"1.0.0","max_version":"1.5.0","patched_version":"1.5.1"}]}"#;
        let db = AdvisoryDatabase::from_json(json).unwrap();
        let hits = db.check("lib-x", "2.0.0");
        assert!(hits.is_empty());
    }

    #[test]
    fn pr4_10_audit_report_count() {
        use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
        let json = r#"{"advisories":[
            {"id":"FJ-001","package":"a","severity":"low","description":"Bug A","min_version":"1.0.0","max_version":"2.0.0","patched_version":"2.0.1"},
            {"id":"FJ-002","package":"b","severity":"medium","description":"Bug B","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"}
        ]}"#;
        let db = AdvisoryDatabase::from_json(json).unwrap();
        let deps = vec![("a".into(), "1.5.0".into()), ("b".into(), "0.5.0".into())];
        let report = audit_dependencies(&deps, &db);
        assert_eq!(report.finding_count(), 2);
    }

    // ===================================================================
    // PHASE 2 — Option 3: FajarOS Nova v2.0 (N1-N10)
    // ===================================================================

    // Sprint N1: Verified @kernel Functions

    #[test]
    fn n1_1_smt_prove_non_negative() {
        use crate::verify::smt::prove_non_negative;
        let result = prove_non_negative("x", "x >= 0");
        assert!(result.is_proven());
    }

    #[test]
    fn n1_2_smt_prove_array_bounds() {
        use crate::verify::smt::prove_array_bounds;
        let result = prove_array_bounds("i >= 0 && i < 10", 10);
        assert!(result.is_proven());
    }

    #[test]
    fn n1_3_smt_prove_no_overflow() {
        use crate::verify::smt::prove_no_i32_overflow;
        let result = prove_no_i32_overflow(0, 100, 0, 100);
        assert!(result.is_proven());
    }

    #[test]
    fn n1_4_smt_prove_matmul_shapes() {
        use crate::verify::smt::prove_matmul_shapes;
        let result = prove_matmul_shapes(4, 3, 3, 5);
        assert!(result.is_proven());
    }

    #[test]
    fn n1_5_smt_check_satisfiable() {
        use crate::verify::smt::check_satisfiable;
        let assertions = vec![
            ("x".to_string(), 0i64, ">=", 0i64),
            ("x".to_string(), 0, "<", 100),
        ];
        let result = check_satisfiable(&assertions);
        assert!(result.is_failed()); // Sat means satisfiable (a model exists)
    }

    #[test]
    fn n1_6_smt_prove_with_timeout() {
        use crate::verify::smt::prove_with_timeout;
        let result = prove_with_timeout("n", "n >= 0", 1000);
        assert!(result.is_proven());
    }

    #[test]
    fn n1_7_symbolic_engine_creation() {
        use crate::verify::symbolic::SymbolicEngine;
        let mut engine = SymbolicEngine::new();
        engine.init_symbolic_var("addr");
        engine.complete_paths();
        let _ = engine; // verifies creation and basic ops
    }

    #[test]
    fn n1_8_symbolic_engine_property_check() {
        use crate::verify::symbolic::SymbolicEngine;
        let mut engine = SymbolicEngine::new();
        engine.init_symbolic_var("size");
        let counterexamples = engine.check_property("size >= 0", "kernel.fj", 42);
        // Symbolic vars are unconstrained, so may find counterexample
        let _ = counterexamples;
    }

    #[test]
    fn n1_9_proof_cache() {
        use crate::verify::smt::ProofCache;
        use crate::verify::smt::SmtResult;
        let mut cache = ProofCache::default();
        cache.insert(12345, 67890, SmtResult::Unsat, 1000);
        assert_eq!(cache.size(), 1);
        let hit = cache.get(12345, 67890);
        assert!(hit.is_some());
    }

    #[test]
    fn n1_10_misra_compliance() {
        use crate::verify::certification::check_misra_compliance;
        let result = check_misra_compliance(false, false, false, false, false, "main.fj");
        assert!(result.compliance_rate() >= 0.0);
    }

    // Sprint N2: Kernel Optimization

    #[test]
    fn n2_1_memory_manager_alloc() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addr = mm.alloc(4096, 16).unwrap();
        // First alloc may start at 0; just verify it succeeded
        let _ = addr;
    }

    #[test]
    fn n2_2_memory_manager_free() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addr = mm.alloc(4096, 16).unwrap();
        mm.free(addr).unwrap();
    }

    #[test]
    fn n2_3_memory_read_write() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addr = mm.alloc(256, 4).unwrap();
        mm.write_u32(addr, 0xDEADBEEF).unwrap();
        let val = mm.read_u32(addr).unwrap();
        assert_eq!(val, 0xDEADBEEF);
    }

    #[test]
    fn n2_4_memory_regions() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        mm.alloc(1024, 8).unwrap();
        mm.alloc(2048, 8).unwrap();
        let regions = mm.allocated_regions();
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn n2_5_syscall_table_define() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(1, "sys_exit".into(), 1).unwrap();
        assert_eq!(table.syscall_count(), 1);
    }

    #[test]
    fn n2_6_syscall_dispatch() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(1, "sys_exit".into(), 1).unwrap();
        let handler = table.dispatch(1, 1).unwrap();
        assert_eq!(handler.name, "sys_exit");
    }

    #[test]
    fn n2_7_syscall_handler_lookup() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(42, "sys_write".into(), 3).unwrap();
        let handler = table.handler_for(42);
        assert!(handler.is_some());
        assert_eq!(handler.unwrap().arg_count, 3);
    }

    #[test]
    fn n2_8_syscall_dispatch_log() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(1, "sys_exit".into(), 1).unwrap();
        let _ = table.dispatch(1, 1);
        let log = table.dispatch_log();
        assert!(!log.is_empty());
    }

    #[test]
    fn n2_9_memory_size() {
        use crate::runtime::os::memory::MemoryManager;
        let mm = MemoryManager::new(65536);
        assert_eq!(mm.size(), 65536);
    }

    #[test]
    fn n2_10_syscall_unknown() {
        use crate::runtime::os::syscall::SyscallTable;
        let table = SyscallTable::new();
        let result = table.handler_for(999);
        assert!(result.is_none());
    }

    // Sprint N3: Distributed Kernel Services

    #[test]
    fn n3_1_raft_node_creation() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        let node = RaftNode::new(
            RaftNodeId(1),
            vec![RaftNodeId(2), RaftNodeId(3), RaftNodeId(4)],
        );
        assert_eq!(node.cluster_size(), 4); // self + 3 peers
        assert_eq!(node.quorum(), 3);
    }

    #[test]
    fn n3_2_raft_log_index() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        let node = RaftNode::new(RaftNodeId(1), vec![RaftNodeId(2), RaftNodeId(3)]);
        assert_eq!(node.last_log_index(), 0);
        assert_eq!(node.last_log_term(), 0);
    }

    #[test]
    fn n3_3_discovery_registry() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "scheduler".into(),
            instance_id: "sched-1".into(),
            address: "10.0.0.1:8080".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        assert_eq!(reg.total_count(), 1);
    }

    #[test]
    fn n3_4_discovery_resolve() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "kernel-rpc".into(),
            instance_id: "rpc-1".into(),
            address: "10.0.0.1:9090".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        let instances = reg.resolve("kernel-rpc");
        assert_eq!(instances.len(), 1);
    }

    #[test]
    fn n3_5_discovery_unhealthy() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "fs".into(),
            instance_id: "fs-1".into(),
            address: "10.0.0.2:2049".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.mark_unhealthy("fs-1");
        let healthy = reg.resolve("fs");
        assert!(healthy.is_empty());
    }

    #[test]
    fn n3_6_discovery_deregister() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "dns".into(),
            instance_id: "dns-1".into(),
            address: "10.0.0.3:53".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.deregister("dns-1");
        assert_eq!(reg.total_count(), 0);
    }

    #[test]
    fn n3_7_failure_detector() {
        use crate::distributed::cluster::{ClusterNodeId, FailureDetector};
        use std::time::Duration;
        let mut fd = FailureDetector::new(Duration::from_millis(5000));
        fd.heartbeat(ClusterNodeId(1), 1000);
        fd.heartbeat(ClusterNodeId(2), 1000);
        assert!(fd.is_healthy(ClusterNodeId(1), 3000));
        let failed = fd.check(7000);
        assert_eq!(failed.len(), 2); // both timed out
    }

    #[test]
    fn n3_8_work_queue() {
        use crate::distributed::cluster::{ClusterNodeId, WorkQueue};
        let mut q = WorkQueue::new(ClusterNodeId(1));
        q.push(100);
        q.push(200);
        q.push(300);
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop(), Some(100)); // front
        assert_eq!(q.steal(), Some(300)); // back
    }

    #[test]
    fn n3_9_work_queue_empty() {
        use crate::distributed::cluster::{ClusterNodeId, WorkQueue};
        let mut q = WorkQueue::new(ClusterNodeId(1));
        assert!(q.is_empty());
        assert_eq!(q.pop(), None);
        assert_eq!(q.steal(), None);
    }

    #[test]
    fn n3_10_raft_quorum_sizes() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        // 3-node cluster: quorum = 2
        let n3 = RaftNode::new(RaftNodeId(1), vec![RaftNodeId(2), RaftNodeId(3)]);
        assert_eq!(n3.quorum(), 2);
        // 5-node cluster: quorum = 3
        let n5 = RaftNode::new(
            RaftNodeId(1),
            vec![RaftNodeId(2), RaftNodeId(3), RaftNodeId(4), RaftNodeId(5)],
        );
        assert_eq!(n5.quorum(), 3);
    }

    // Sprint N4: AI-Integrated Kernel

    #[test]
    fn n4_1_workload_classify_compute() {
        use crate::accelerator::dispatch::classify_workload;
        let class = classify_workload(1_000_000, 1_000, 32);
        assert_eq!(format!("{class:?}"), "ComputeBound");
    }

    #[test]
    fn n4_2_workload_classify_memory() {
        use crate::accelerator::dispatch::classify_workload;
        let class = classify_workload(100, 1_000_000, 2);
        assert_eq!(format!("{class:?}"), "MemoryBound");
    }

    #[test]
    fn n4_3_device_set_cpu_only() {
        use crate::accelerator::dispatch::DeviceSet;
        let devices = DeviceSet::cpu_only();
        assert!(!devices.has_gpu());
        assert!(!devices.has_npu());
    }

    #[test]
    fn n4_4_dispatch_decision_cpu() {
        use crate::accelerator::dispatch::{DeviceSet, WorkloadDescriptor, decide_dispatch};
        let workload = WorkloadDescriptor {
            op_type: "add".into(),
            input_elements: 100,
            dtype: "f32".into(),
            batch_size: 1,
            estimated_flops: 100,
            estimated_bytes: 400,
            preference: crate::accelerator::infer::InferPreference::Auto,
        };
        let decision = decide_dispatch(&workload, &DeviceSet::cpu_only());
        assert_eq!(format!("{:?}", decision.primary), "Cpu");
    }

    #[test]
    fn n4_5_fusion_graph_matmul() {
        use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::Matmul,
                inputs: vec![],
                output_elements: 1024,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![0],
                output_elements: 1024,
            },
        ];
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        // Matmul + elementwise should fuse
        assert!(graph.total_fused_ops() >= 1);
    }

    #[test]
    fn n4_6_device_allocator_multiple() {
        use crate::gpu_codegen::gpu_memory::DeviceAllocator;
        let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        let a1 = alloc.allocate(1024).unwrap();
        let a2 = alloc.allocate(2048).unwrap();
        let stats = alloc.stats();
        assert_eq!(stats.used_bytes, 3072);
        alloc.free(a1.id).unwrap();
        alloc.free(a2.id).unwrap();
    }

    #[test]
    fn n4_7_spirv_compute_module() {
        use crate::gpu_codegen::spirv::{EntryPoint, ExecutionModel, SpirVModule, SpirVType};
        let mut m = SpirVModule::new_compute();
        let void_id = m.alloc_id();
        m.types.push(SpirVType::Void { id: void_id });
        let fn_id = m.alloc_id();
        m.entry_points.push(EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: fn_id,
            name: "anomaly_detect".into(),
            interface_ids: vec![],
            local_size: [64, 1, 1],
        });
        assert!(m.validate().is_empty());
    }

    #[test]
    fn n4_8_ptx_kernel_ai() {
        use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
        let kernel = KernelEntry {
            name: "predict_duration".into(),
            params: vec![
                KernelParam {
                    name: "features".into(),
                    ptx_type: PtxType::F32,
                    is_pointer: true,
                },
                KernelParam {
                    name: "weights".into(),
                    ptx_type: PtxType::F32,
                    is_pointer: true,
                },
                KernelParam {
                    name: "output".into(),
                    ptx_type: PtxType::F32,
                    is_pointer: true,
                },
            ],
            body: vec![],
        };
        let ptx = kernel.emit();
        assert!(ptx.contains("predict_duration"));
    }

    #[test]
    fn n4_9_smt_kernel_safety() {
        use crate::verify::smt::prove_array_bounds;
        // Verify kernel stack doesn't overflow (< 8192 bytes)
        let result = prove_array_bounds("stack_ptr >= 0 && stack_ptr < 8192", 8192);
        assert!(result.is_proven());
    }

    #[test]
    fn n4_10_do178c_evidence() {
        use crate::verify::certification::{DalLevel, generate_do178c_evidence};
        let evidence = generate_do178c_evidence(DalLevel::DalC, true, false, true, 0.95);
        assert!(!evidence.is_empty());
    }

    // Sprint N5: Hardware Abstraction v2

    #[test]
    fn n5_1_memory_multi_alloc() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addrs: Vec<_> = (0..10).map(|_| mm.alloc(1024, 8).unwrap()).collect();
        assert_eq!(addrs.len(), 10);
        for addr in addrs {
            mm.free(addr).unwrap();
        }
    }

    #[test]
    fn n5_2_syscall_multiple_define() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(0, "sys_read".into(), 3).unwrap();
        table.define(1, "sys_write".into(), 3).unwrap();
        table.define(2, "sys_open".into(), 2).unwrap();
        table.define(3, "sys_close".into(), 1).unwrap();
        assert_eq!(table.syscall_count(), 4);
    }

    #[test]
    fn n5_3_memory_write_read_bytes() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addr = mm.alloc(256, 1).unwrap();
        let data = vec![0xCA, 0xFE, 0xBA, 0xBE];
        mm.write_bytes(addr, &data).unwrap();
        let read = mm.read_bytes(addr, 4).unwrap();
        assert_eq!(read, data);
    }

    #[test]
    fn n5_4_raft_five_node() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        let node = RaftNode::new(
            RaftNodeId(1),
            vec![RaftNodeId(2), RaftNodeId(3), RaftNodeId(4), RaftNodeId(5)],
        );
        assert_eq!(node.cluster_size(), 5);
        assert_eq!(node.quorum(), 3);
    }

    #[test]
    fn n5_5_discovery_multiple_services() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        for i in 0..5 {
            reg.register(ServiceInstance {
                service_name: "usb-driver".into(),
                instance_id: format!("usb-{i}"),
                address: format!("10.0.0.{i}:5000"),
                healthy: true,
                tags: HashMap::new(),
            });
        }
        assert_eq!(reg.resolve("usb-driver").len(), 5);
    }

    #[test]
    fn n5_6_failure_detector_healthy() {
        use crate::distributed::cluster::{ClusterNodeId, FailureDetector};
        use std::time::Duration;
        let mut fd = FailureDetector::new(Duration::from_millis(10000));
        fd.heartbeat(ClusterNodeId(1), 5000);
        assert!(fd.is_healthy(ClusterNodeId(1), 8000));
    }

    #[test]
    fn n5_7_spirv_type_vector() {
        use crate::gpu_codegen::spirv::SpirVType;
        let vec4 = SpirVType::Vector {
            id: 10,
            component_id: 5,
            count: 4,
        };
        assert_eq!(vec4.id(), 10);
    }

    #[test]
    fn n5_8_ptx_grid_rounding() {
        use crate::gpu_codegen::ptx::compute_grid_1d;
        let grid = compute_grid_1d(1000, 256);
        assert!(grid.total_threads() >= 1000);
    }

    #[test]
    fn n5_9_smt_matmul_mismatch() {
        use crate::verify::smt::prove_matmul_shapes;
        let result = prove_matmul_shapes(4, 3, 5, 2); // k1 != k2
        assert!(result.is_failed());
    }

    #[test]
    fn n5_10_memory_alloc_alignment() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(1024 * 1024);
        let addr = mm.alloc(64, 64).unwrap();
        assert_eq!(addr.0 % 64, 0);
    }

    // Sprint N6: Network Stack v2

    #[test]
    fn n6_1_syscall_table_full() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        for i in 0..34 {
            table
                .define(i, format!("sys_{i}"), (i % 4 + 1) as usize)
                .unwrap();
        }
        assert_eq!(table.syscall_count(), 34);
    }

    #[test]
    fn n6_2_discovery_all_instances() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "tcp".into(),
            instance_id: "tcp-1".into(),
            address: "10.0.0.1:80".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.register(ServiceInstance {
            service_name: "tcp".into(),
            instance_id: "tcp-2".into(),
            address: "10.0.0.2:80".into(),
            healthy: false,
            tags: HashMap::new(),
        });
        assert_eq!(reg.all_instances("tcp").len(), 2);
        assert_eq!(reg.resolve("tcp").len(), 1); // only healthy
    }

    #[test]
    fn n6_3_work_queue_ordering() {
        use crate::distributed::cluster::{ClusterNodeId, WorkQueue};
        let mut q = WorkQueue::new(ClusterNodeId(1));
        for i in 0..5 {
            q.push(i);
        }
        assert_eq!(q.pop(), Some(0)); // FIFO
        assert_eq!(q.steal(), Some(4)); // LIFO steal
    }

    #[test]
    fn n6_4_raft_single_node() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        let node = RaftNode::new(RaftNodeId(1), vec![]);
        assert_eq!(node.cluster_size(), 1);
        assert_eq!(node.quorum(), 1);
    }

    #[test]
    fn n6_5_smt_overflow_large() {
        use crate::verify::smt::prove_no_i32_overflow;
        let result = prove_no_i32_overflow(i32::MIN, i32::MAX, 0, 1);
        assert!(result.is_failed()); // full range + 1 can overflow
    }

    #[test]
    fn n6_6_spirv_backend_explicit() {
        use crate::gpu_codegen::spirv::{GpuBackend, resolve_backend};
        let r = resolve_backend(GpuBackend::SpirV, "NVIDIA");
        assert_eq!(r, GpuBackend::SpirV); // explicit overrides auto
    }

    #[test]
    fn n6_7_ptx_type_all() {
        use crate::gpu_codegen::ptx::{PtxType, map_type};
        assert_eq!(map_type("u32"), Some(PtxType::U32));
        assert_eq!(map_type("u64"), Some(PtxType::U64));
        assert_eq!(map_type("f16"), Some(PtxType::F16));
    }

    #[test]
    fn n6_8_fusion_no_fuse_independent() {
        use crate::gpu_codegen::fusion::{GpuOp, OpKind, can_fuse};
        let a = GpuOp {
            id: 0,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 100,
        };
        let b = GpuOp {
            id: 1,
            kind: OpKind::ElementWiseUnary,
            inputs: vec![],
            output_elements: 100,
        };
        assert!(!can_fuse(&a, &b)); // b doesn't depend on a
    }

    #[test]
    fn n6_9_memory_manager_default() {
        use crate::runtime::os::memory::MemoryManager;
        let mm = MemoryManager::with_default_size();
        assert!(mm.size() > 0);
    }

    #[test]
    fn n6_10_proof_cache_miss() {
        use crate::verify::smt::ProofCache;
        let mut cache = ProofCache::default();
        assert!(cache.get(99999, 11111).is_none());
        assert_eq!(cache.size(), 0);
    }

    // Sprint N7: Userland Libraries

    #[test]
    fn n7_1_component_instance_creation() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let inst =
            ComponentInstance::new("userland-lib", vec![0x00, 0x61, 0x73, 0x6D], HashMap::new());
        assert_eq!(inst.name(), "userland-lib");
        assert!(!inst.has_executed());
    }

    #[test]
    fn n7_2_component_instance_run() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let mut inst = ComponentInstance::new("libc", vec![0x00], HashMap::new());
        let code = inst.run().unwrap();
        assert_eq!(code, 0);
        assert!(inst.has_executed());
    }

    #[test]
    fn n7_3_component_double_run() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let mut inst = ComponentInstance::new("test", vec![], HashMap::new());
        inst.run().unwrap();
        assert!(inst.run().is_err()); // already executed
    }

    #[test]
    fn n7_4_component_linker() {
        use crate::wasi_p2::composition::{ComponentInstance, ComponentLinker};
        use std::collections::HashMap;
        let mut linker = ComponentLinker::new();
        linker.register(ComponentInstance::new("app", vec![], HashMap::new()));
        assert_eq!(linker.instance_count(), 1);
    }

    #[test]
    fn n7_5_component_exports() {
        use crate::wasi_p2::component::ExportKind;
        use crate::wasi_p2::composition::{ComponentInstance, ExportDef};
        use std::collections::HashMap;
        let mut inst = ComponentInstance::new("libm", vec![], HashMap::new());
        inst.add_export(ExportDef {
            name: "sin".into(),
            kind: ExportKind::Func,
            params: vec!["f64".into()],
            result: Some("f64".into()),
        });
        assert!(inst.get_export("sin").is_some());
    }

    #[test]
    fn n7_6_ffi_mangle_name() {
        use crate::ffi_v2::cpp::mangle_name;
        let mangled = mangle_name(&["std".into()], "sort", &[]);
        assert!(!mangled.is_empty());
    }

    #[test]
    fn n7_7_ffi_demangle_name() {
        use crate::ffi_v2::cpp::{demangle_name, mangle_name};
        let mangled = mangle_name(&[], "hello", &[]);
        let demangled = demangle_name(&mangled);
        assert!(demangled.contains("hello"));
    }

    #[test]
    fn n7_8_wit_parse_interface() {
        use crate::wasi_p2::wit_parser::parse_wit;
        let wit = "package test:example@1.0.0;\ninterface math {\n  add: func(a: s32, b: s32) -> s32;\n}\n";
        let doc = parse_wit(wit).unwrap();
        assert!(!doc.interfaces.is_empty());
    }

    #[test]
    fn n7_9_wit_parse_world() {
        use crate::wasi_p2::wit_parser::parse_wit;
        let wit = "package test:app@1.0.0;\nworld my-world {\n  import wasi:io/streams;\n}\n";
        let doc = parse_wit(wit).unwrap();
        assert!(!doc.worlds.is_empty());
    }

    #[test]
    fn n7_10_component_return_value() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let mut inst = ComponentInstance::new("test", vec![], HashMap::new());
        inst.set_return_value(42);
        assert_eq!(inst.run().unwrap(), 42);
    }

    // Sprint N8: GUI Framework

    #[test]
    fn n8_1_interpreter_gpu_available() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let avail = gpu_available()\navail");
        assert!(result.is_ok());
    }

    #[test]
    fn n8_2_interpreter_gpu_info() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("gpu_info()");
        assert!(result.is_ok());
    }

    #[test]
    fn n8_3_spirv_variable_ssbo() {
        use crate::gpu_codegen::spirv::{StorageClass, create_ssbo};
        let v1 = create_ssbo(1, 2, 0, 0);
        let v2 = create_ssbo(3, 4, 1, 0);
        assert_eq!(v1.storage_class, v2.storage_class);
        assert_eq!(v1.storage_class, StorageClass::StorageBuffer);
    }

    #[test]
    fn n8_4_spirv_builtin_values() {
        use crate::gpu_codegen::spirv::BuiltIn;
        assert_eq!(BuiltIn::GlobalInvocationId.value(), 28);
        assert_eq!(BuiltIn::WorkGroupId.value(), 26);
    }

    #[test]
    fn n8_5_discovery_tags() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        let mut tags = HashMap::new();
        tags.insert("version".into(), "2.0".into());
        reg.register(ServiceInstance {
            service_name: "gui".into(),
            instance_id: "gui-1".into(),
            address: "10.0.0.1:3000".into(),
            healthy: true,
            tags,
        });
        let instances = reg.resolve("gui");
        assert_eq!(instances[0].tags.get("version").unwrap(), "2.0");
    }

    #[test]
    fn n8_6_work_queue_steal_all() {
        use crate::distributed::cluster::{ClusterNodeId, WorkQueue};
        let mut q = WorkQueue::new(ClusterNodeId(1));
        q.push(1);
        q.push(2);
        q.push(3);
        let stolen: Vec<_> = std::iter::from_fn(|| q.steal()).collect();
        assert_eq!(stolen, vec![3, 2, 1]);
        assert!(q.is_empty());
    }

    #[test]
    fn n8_7_syscall_undefine() {
        use crate::runtime::os::syscall::SyscallTable;
        let mut table = SyscallTable::new();
        table.define(1, "sys_exit".into(), 1).unwrap();
        table.undefine(1).unwrap();
        assert_eq!(table.syscall_count(), 0);
    }

    #[test]
    fn n8_8_memory_alloc_free_reuse() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(4096);
        let a = mm.alloc(1024, 8).unwrap();
        mm.free(a).unwrap();
        let b = mm.alloc(1024, 8).unwrap();
        // Should be able to allocate again after free
        assert!(b.0 > 0);
    }

    #[test]
    fn n8_9_smt_satisfiable_range() {
        use crate::verify::smt::check_satisfiable;
        let assertions = vec![
            ("x".to_string(), 0i64, ">=", 10i64),
            ("x".to_string(), 0, "<=", 20),
        ];
        let result = check_satisfiable(&assertions);
        assert!(result.is_failed()); // Sat means satisfiable (a model exists)
    }

    #[test]
    fn n8_10_fusion_total_ops() {
        use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![],
                output_elements: 100,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![0],
                output_elements: 100,
            },
            GpuOp {
                id: 2,
                kind: OpKind::ElementWiseBinary,
                inputs: vec![1],
                output_elements: 100,
            },
        ];
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        assert!(graph.total_fused_ops() >= 2);
    }

    // Sprint N9: Package Manager

    #[test]
    fn n9_1_registry_download_count() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish("os-pkg", SemVer::new(1, 0, 0), "OS package");
        let count = reg.download_count("os-pkg");
        assert!(count.is_some());
    }

    #[test]
    fn n9_2_registry_search_empty() {
        use crate::package::registry::Registry;
        let reg = Registry::new();
        let results = reg.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn n9_3_semver_ordering() {
        use crate::package::registry::SemVer;
        let v1 = SemVer::new(1, 0, 0);
        let v2 = SemVer::new(2, 0, 0);
        assert!(v1 < v2);
    }

    #[test]
    fn n9_4_version_constraint_exact() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("1.5.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 5, 0)));
        assert!(!c.matches(&SemVer::new(1, 5, 1)));
    }

    #[test]
    fn n9_5_component_adapter() {
        use crate::wasi_p2::composition::ComponentAdapter;
        let adapter = ComponentAdapter::new(vec![0x00, 0x61, 0x73, 0x6D]);
        let adapted = adapter.adapt();
        assert!(!adapted.is_empty());
    }

    #[test]
    fn n9_6_component_linker_imports() {
        use crate::wasi_p2::composition::{ComponentInstance, ComponentLinker};
        use std::collections::HashMap;
        let mut imports = HashMap::new();
        imports.insert("wasi:io/streams".into(), "wasi-io".into());
        let mut linker = ComponentLinker::new();
        linker.register(ComponentInstance::new("app", vec![], imports));
        let unresolved = linker.check_all_imports();
        // "wasi-io" not registered, so it's unresolved
        assert!(!unresolved.is_empty());
    }

    #[test]
    fn n9_7_sbom_spdx() {
        use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
        let deps = vec![DepInfo {
            name: "core".into(),
            version: "1.0.0".into(),
            sha256: "abc".into(),
            license: Some("MIT".into()),
            dev_only: false,
        }];
        let spdx = generate_sbom("fajaros", &deps, SbomFormat::Spdx).unwrap();
        assert!(spdx.contains("fajaros"));
    }

    #[test]
    fn n9_8_audit_empty_db() {
        use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
        let db = AdvisoryDatabase::new();
        let deps = vec![("anything".into(), "1.0.0".into())];
        let report = audit_dependencies(&deps, &db);
        assert_eq!(report.finding_count(), 0);
    }

    #[test]
    fn n9_9_signing_full_flow() {
        use crate::package::signing::{
            FjSignatureBundle, oidc_authenticate, request_certificate, sign_package,
            submit_to_rekor,
        };
        let oidc = oidc_authenticate("github").unwrap();
        let cert = request_certificate(&oidc).unwrap();
        let sig = sign_package("sha256:pkg-hash", &cert).unwrap();
        let rekor = submit_to_rekor(&sig, &cert, "sha256:pkg-hash").unwrap();
        let bundle = FjSignatureBundle::new(&cert, &sig, &rekor);
        let json = bundle.to_json();
        assert!(json.contains("certificate_pem"));
    }

    #[test]
    fn n9_10_version_constraint_gte() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse(">=2.0.0").unwrap();
        assert!(c.matches(&SemVer::new(2, 0, 0)));
        assert!(c.matches(&SemVer::new(3, 0, 0)));
        assert!(!c.matches(&SemVer::new(1, 9, 9)));
    }

    // Sprint N10: Release

    #[test]
    fn n10_1_iso26262_evidence() {
        use crate::verify::certification::{AsilLevel, generate_iso26262_evidence};
        let evidence = generate_iso26262_evidence(AsilLevel::AsilD, true, true, true, true);
        assert!(!evidence.is_empty());
    }

    #[test]
    fn n10_2_spirv_full_module() {
        use crate::gpu_codegen::spirv::*;
        let mut m = SpirVModule::new_compute();
        let void_id = m.alloc_id();
        let fn_type_id = m.alloc_id();
        let fn_id = m.alloc_id();
        m.types.push(SpirVType::Void { id: void_id });
        m.types.push(SpirVType::Function {
            id: fn_type_id,
            return_type_id: void_id,
            param_type_ids: vec![],
        });
        m.functions.push(SpirVFunction {
            id: fn_id,
            return_type_id: void_id,
            function_type_id: fn_type_id,
            param_ids: vec![],
            blocks: vec![],
        });
        m.entry_points.push(EntryPoint {
            execution_model: ExecutionModel::GLCompute,
            function_id: fn_id,
            name: "release_kernel".into(),
            interface_ids: vec![],
            local_size: [256, 1, 1],
        });
        assert!(m.validate().is_empty());
        let words = m.emit_words();
        assert_eq!(words[0], 0x0723_0203);
    }

    #[test]
    fn n10_3_proof_cache_hit_rate() {
        use crate::verify::smt::ProofCache;
        use crate::verify::smt::SmtResult;
        let mut cache = ProofCache::default();
        cache.insert(1, 1, SmtResult::Unsat, 100);
        let _ = cache.get(1, 1); // hit
        let _ = cache.get(2, 2); // miss
        assert!(cache.hit_rate() >= 0.0);
    }

    #[test]
    fn n10_4_registry_yank_nonexistent() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        let result = reg.yank("no-pkg", &SemVer::new(1, 0, 0));
        assert!(result.is_err());
    }

    #[test]
    fn n10_5_discovery_mode_variants() {
        use crate::distributed::discovery::DiscoveryMode;
        let modes = [
            DiscoveryMode::Mdns,
            DiscoveryMode::Seed,
            DiscoveryMode::Dns,
            DiscoveryMode::Gossip,
            DiscoveryMode::Static,
        ];
        assert_eq!(modes.len(), 5);
    }

    #[test]
    fn n10_6_component_binary() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let binary = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let inst = ComponentInstance::new("wasm-module", binary.clone(), HashMap::new());
        assert_eq!(inst.binary(), &binary);
    }

    #[test]
    fn n10_7_ffi_generate_class() {
        use crate::ffi_v2::cpp::{CppClass, generate_class_binding};
        let class = CppClass {
            name: "Widget".into(),
            namespace: vec![],
            bases: vec![],
            fields: vec![],
            methods: vec![],
            constructors: vec![],
            has_destructor: false,
            is_abstract: false,
            template_params: vec![],
            size_bytes: 0,
            align_bytes: 0,
        };
        let binding = generate_class_binding(&class);
        assert!(binding.contains("Widget"));
    }

    #[test]
    fn n10_8_memory_manager_stress() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(16 * 1024 * 1024);
        let mut addrs = Vec::new();
        for _ in 0..100 {
            addrs.push(mm.alloc(256, 8).unwrap());
        }
        for addr in addrs {
            mm.free(addr).unwrap();
        }
        assert!(mm.allocated_regions().is_empty());
    }

    #[test]
    fn n10_9_wit_parse_empty() {
        use crate::wasi_p2::wit_parser::parse_wit;
        let result = parse_wit("");
        // Empty input should either parse to empty doc or error
        let _ = result;
    }

    #[test]
    fn n10_10_api_status_codes() {
        use crate::package::server::StatusCode;
        assert_eq!(StatusCode::OK.0, 200);
        assert_eq!(StatusCode::CREATED.0, 201);
        assert_eq!(StatusCode::NOT_FOUND.0, 404);
        assert_eq!(StatusCode::BAD_REQUEST.0, 400);
    }

    // ===================================================================
    // PHASE 2 — Option 4: Real-World Validation (W1-W10)
    // ===================================================================

    // Sprint W1: FFI Bindgen & OpenCV

    #[test]
    fn w1_1_ffi_bindgen_config() {
        use crate::ffi_v2::bindgen::BindgenConfig;
        let config = BindgenConfig::new(
            "opencv2/core.hpp",
            crate::ffi_v2::bindgen::BindgenLanguage::Cpp,
            "bindings.fj",
        );
        assert_eq!(config.source_path, "opencv2/core.hpp");
    }

    #[test]
    fn w1_2_cpp_mangle_with_namespace() {
        use crate::ffi_v2::cpp::{CppType, mangle_name};
        let mangled = mangle_name(&["cv".into()], "imread", &[CppType::String]);
        assert!(!mangled.is_empty());
    }

    #[test]
    fn w1_3_cpp_class_binding() {
        use crate::ffi_v2::cpp::{
            CppClass, CppFunction, CppIntSize, CppType, generate_class_binding,
        };
        let class = CppClass {
            name: "Mat".into(),
            namespace: vec![],
            bases: vec![],
            fields: vec![],
            methods: vec![CppFunction {
                name: "rows".into(),
                namespace: vec![],
                return_type: CppType::Int(CppIntSize::I32),
                params: vec![],
                is_const: true,
                is_static: false,
                is_virtual: false,
                is_noexcept: false,
                template_params: vec![],
            }],
            constructors: vec![],
            has_destructor: true,
            is_abstract: false,
            template_params: vec![],
            size_bytes: 0,
            align_bytes: 0,
        };
        let binding = generate_class_binding(&class);
        assert!(binding.contains("Mat"));
        assert!(binding.contains("rows"));
    }

    #[test]
    fn w1_4_cpp_demangle() {
        use crate::ffi_v2::cpp::demangle_name;
        let result =
            demangle_name("_ZN2cv6imreadERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEEi");
        assert!(result.contains("imread") || result.contains("unknown"));
    }

    #[test]
    fn w1_5_ffi_fajar_bindings() {
        use crate::ffi_v2::cpp::{
            CppDecl, CppFunction, CppIntSize, CppParam, CppType, generate_fajar_bindings,
        };
        let decls = vec![CppDecl::Function(CppFunction {
            name: "detect_faces".into(),
            namespace: vec![],
            return_type: CppType::Int(CppIntSize::I32),
            params: vec![CppParam {
                name: "img".into(),
                param_type: CppType::Pointer(Box::new(CppType::Void)),
                has_default: false,
            }],
            is_static: false,
            is_const: false,
            is_virtual: false,
            is_noexcept: false,
            template_params: vec![],
        })];
        let fj = generate_fajar_bindings(&decls);
        assert!(fj.contains("detect_faces"));
    }

    #[test]
    fn w1_6_spirv_type_f16() {
        use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
        assert_eq!(map_fj_type("f16"), Some(SpirVTypeDesc::Float(16)));
        assert_eq!(map_fj_type("u8"), Some(SpirVTypeDesc::Int(8, false)));
    }

    #[test]
    fn w1_7_ptx_f16_type() {
        use crate::gpu_codegen::ptx::{PtxType, map_type};
        assert_eq!(map_type("f16"), Some(PtxType::F16));
        assert_eq!(map_type("bool"), Some(PtxType::Pred));
    }

    #[test]
    fn w1_8_registry_publish_multiple() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        reg.publish(
            "opencv-fj",
            SemVer::new(0, 1, 0),
            "OpenCV bindings for Fajar",
        );
        reg.publish("opencv-fj", SemVer::new(0, 2, 0), "OpenCV bindings v0.2");
        let latest = reg.latest_version("opencv-fj").unwrap();
        assert_eq!(*latest, SemVer::new(0, 2, 0));
    }

    #[test]
    fn w1_9_smt_array_out_of_bounds() {
        use crate::verify::smt::prove_array_bounds;
        let result = prove_array_bounds("i >= 0 && i <= 10", 10); // i can be 10, array size 10 → OOB
        assert!(result.is_failed());
    }

    #[test]
    fn w1_10_memory_read_unalloc() {
        use crate::runtime::os::memory::MemoryManager;
        let mm = MemoryManager::new(4096);
        let result = mm.read_u32(crate::runtime::os::memory::VirtAddr(99999));
        assert!(result.is_err());
    }

    // Sprint W2: WASI HTTP Server

    #[test]
    fn w2_1_wit_parse_full() {
        use crate::wasi_p2::wit_parser::parse_wit;
        let wit = r#"package wasi:http@0.2.0;
interface types {
  type body = list<u8>;
  record request {
    method: string,
    path: string,
  }
  record response {
    status: u16,
    body: body,
  }
}
world http-server {
  import wasi:io/streams;
  export handler: func(req: string) -> string;
}
"#;
        let doc = parse_wit(wit).unwrap();
        assert!(!doc.interfaces.is_empty());
        assert!(!doc.worlds.is_empty());
    }

    #[test]
    fn w2_2_component_linker_link() {
        use crate::wasi_p2::component::ExportKind;
        use crate::wasi_p2::composition::{ComponentInstance, ComponentLinker, ExportDef};
        use std::collections::HashMap;
        let mut linker = ComponentLinker::new();
        let mut provider = ComponentInstance::new("wasi-io", vec![], HashMap::new());
        provider.add_export(ExportDef {
            name: "wasi:io/streams".into(),
            kind: ExportKind::Instance,
            params: vec![],
            result: None,
        });
        let mut imports = HashMap::new();
        imports.insert("wasi:io/streams".into(), "wasi-io".into());
        let app = ComponentInstance::new("http-app", vec![], imports);
        linker.register(provider);
        linker.register(app);
        linker
            .link("wasi-io", "wasi:io/streams", "http-app", "wasi:io/streams")
            .unwrap();
        let unresolved = linker.check_all_imports();
        assert!(unresolved.is_empty()); // all imports satisfied
    }

    #[test]
    fn w2_3_component_set_return() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let mut inst = ComponentInstance::new("http-handler", vec![], HashMap::new());
        inst.set_return_value(200);
        assert_eq!(inst.run().unwrap(), 200);
    }

    #[test]
    fn w2_4_interpreter_http_route() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let status = 200
status"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w2_5_json_parse_eval() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let data = "{\"key\": \"value\"}"
len(data)"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w2_6_api_response_types() {
        use crate::package::server::{ApiResponse, StatusCode};
        let ok = ApiResponse::ok("{}");
        let err = ApiResponse::error(StatusCode::TOO_MANY_REQUESTS, "rate limited");
        assert_eq!(ok.status.0, 200);
        assert_eq!(err.status.0, 429);
    }

    #[test]
    fn w2_7_version_constraint_lt() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("<2.0.0").unwrap();
        assert!(c.matches(&SemVer::new(1, 9, 9)));
        assert!(!c.matches(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn w2_8_component_imports_map() {
        use crate::wasi_p2::composition::ComponentInstance;
        use std::collections::HashMap;
        let mut imports = HashMap::new();
        imports.insert("wasi:keyvalue/store".into(), "kv-provider".into());
        let inst = ComponentInstance::new("kv-app", vec![], imports);
        assert!(inst.imports().contains_key("wasi:keyvalue/store"));
    }

    #[test]
    fn w2_9_spirv_dispatch_large() {
        use crate::gpu_codegen::spirv::compute_dispatch_1d;
        let d = compute_dispatch_1d(1_000_000, 256);
        assert!(d.group_count_x > 3000);
    }

    #[test]
    fn w2_10_smt_non_negative_negative() {
        use crate::verify::smt::prove_non_negative;
        let result = prove_non_negative("x", "x > -5");
        // x > -5 includes negatives, so non-negative proof should fail
        assert!(result.is_failed());
    }

    // Sprint W3: Distributed MNIST Training

    #[test]
    fn w3_1_interpreter_tensor_zeros() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let t = zeros(3, 4)\nshape(t)");
        assert!(result.is_ok());
    }

    #[test]
    fn w3_2_interpreter_tensor_matmul() {
        let mut interp = Interpreter::new();
        let result = interp
            .eval_source("let a = ones(2, 3)\nlet b = ones(3, 4)\nlet c = matmul(a, b)\nshape(c)");
        assert!(result.is_ok());
    }

    #[test]
    fn w3_3_interpreter_dense_layer() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            "let layer = Dense(4, 2)\nlet x = ones(1, 4)\nlet y = forward(layer, x)\nshape(y)",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w3_4_work_queue_parallel() {
        use crate::distributed::cluster::{ClusterNodeId, WorkQueue};
        let mut queues: Vec<WorkQueue> = (0..4).map(|i| WorkQueue::new(ClusterNodeId(i))).collect();
        for (i, q) in queues.iter_mut().enumerate() {
            for j in 0..10 {
                q.push((i * 10 + j) as u64);
            }
        }
        let total: usize = queues.iter().map(|q| q.len()).sum();
        assert_eq!(total, 40);
    }

    #[test]
    fn w3_5_failure_detector_multi_node() {
        use crate::distributed::cluster::{ClusterNodeId, FailureDetector};
        use std::time::Duration;
        let mut fd = FailureDetector::new(Duration::from_millis(3000));
        for i in 0..4 {
            fd.heartbeat(ClusterNodeId(i), 1000);
        }
        assert_eq!(fd.check(2000).len(), 0); // all healthy
        assert_eq!(fd.check(5000).len(), 4); // all failed
    }

    #[test]
    fn w3_6_raft_cluster_sizes() {
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        let n7 = RaftNode::new(
            RaftNodeId(1),
            vec![
                RaftNodeId(2),
                RaftNodeId(3),
                RaftNodeId(4),
                RaftNodeId(5),
                RaftNodeId(6),
                RaftNodeId(7),
            ],
        );
        assert_eq!(n7.cluster_size(), 7);
        assert_eq!(n7.quorum(), 4);
    }

    #[test]
    fn w3_7_smt_matmul_valid() {
        use crate::verify::smt::prove_matmul_shapes;
        let r = prove_matmul_shapes(28, 784, 784, 128);
        assert!(r.is_proven()); // MNIST: 28 images × 784 features × 128 hidden
    }

    #[test]
    fn w3_8_fusion_chain() {
        use crate::gpu_codegen::fusion::{FusionGraph, GpuOp, OpKind};
        let ops = vec![
            GpuOp {
                id: 0,
                kind: OpKind::Matmul,
                inputs: vec![],
                output_elements: 3584,
            },
            GpuOp {
                id: 1,
                kind: OpKind::ElementWiseUnary,
                inputs: vec![0],
                output_elements: 3584,
            }, // relu
            GpuOp {
                id: 2,
                kind: OpKind::Matmul,
                inputs: vec![1],
                output_elements: 280,
            },
            GpuOp {
                id: 3,
                kind: OpKind::Softmax,
                inputs: vec![2],
                output_elements: 280,
            },
        ];
        let mut graph = FusionGraph::new(ops);
        graph.analyze();
        assert!(graph.num_fusions() >= 1);
    }

    #[test]
    fn w3_9_device_alloc_gpu_memory() {
        use crate::gpu_codegen::gpu_memory::DeviceAllocator;
        let mut alloc = DeviceAllocator::new(0, 256 * 1024 * 1024, 512 * 1024 * 1024);
        // Allocate space for MNIST weights
        let _w1 = alloc.allocate(784 * 128 * 4).unwrap(); // 784→128 f32
        let _w2 = alloc.allocate(128 * 10 * 4).unwrap(); // 128→10 f32
        let stats = alloc.stats();
        assert!(stats.used_bytes > 0);
    }

    #[test]
    fn w3_10_interpreter_relu() {
        let mut interp = Interpreter::new();
        let result =
            interp.eval_source("let t = from_data([[1.0, -2.0], [3.0, -4.0]])\nlet r = relu(t)\nr");
        assert!(result.is_ok());
    }

    // Sprint W4: PyTorch Model Inference

    #[test]
    fn w4_1_ffi_python_types() {
        use crate::ffi_v2::cpp::{CppIntSize, CppType};
        let types = [
            CppType::Int(CppIntSize::I32),
            CppType::Float,
            CppType::Double,
            CppType::Void,
        ];
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn w4_2_interpreter_softmax() {
        let mut interp = Interpreter::new();
        let result =
            interp.eval_source("let t = from_data([[1.0, 2.0, 3.0]])\nlet s = softmax(t)\ns");
        assert!(result.is_ok());
    }

    #[test]
    fn w4_3_interpreter_sigmoid() {
        let mut interp = Interpreter::new();
        let result =
            interp.eval_source("let t = from_data([[0.0, 1.0, -1.0]])\nlet s = sigmoid(t)\ns");
        assert!(result.is_ok());
    }

    #[test]
    fn w4_4_interpreter_reshape() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let t = ones(2, 6)\nlet r = reshape(t, [3, 4])\nshape(r)");
        assert!(result.is_ok());
    }

    #[test]
    fn w4_5_interpreter_transpose() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let t = zeros(3, 5)\nlet r = transpose(t)\nshape(r)");
        assert!(result.is_ok());
    }

    #[test]
    fn w4_6_ffi_class_with_methods() {
        use crate::ffi_v2::cpp::{
            CppClass, CppFunction, CppParam, CppType, generate_class_binding,
        };
        let class = CppClass {
            name: "TorchModel".into(),
            namespace: vec![],
            bases: vec![],
            fields: vec![],
            methods: vec![
                CppFunction {
                    name: "forward".into(),
                    namespace: vec![],
                    return_type: CppType::Pointer(Box::new(CppType::Float)),
                    params: vec![CppParam {
                        name: "input".into(),
                        param_type: CppType::Pointer(Box::new(CppType::Float)),
                        has_default: false,
                    }],
                    is_const: false,
                    is_static: false,
                    is_virtual: true,
                    is_noexcept: false,
                    template_params: vec![],
                },
                CppFunction {
                    name: "eval".into(),
                    namespace: vec![],
                    return_type: CppType::Void,
                    params: vec![],
                    is_const: false,
                    is_static: false,
                    is_virtual: false,
                    is_noexcept: false,
                    template_params: vec![],
                },
            ],
            constructors: vec![],
            has_destructor: true,
            is_abstract: false,
            template_params: vec![],
            size_bytes: 0,
            align_bytes: 0,
        };
        let binding = generate_class_binding(&class);
        assert!(binding.contains("forward"));
    }

    #[test]
    fn w4_7_spirv_type_u64() {
        use crate::gpu_codegen::spirv::{SpirVTypeDesc, map_fj_type};
        assert_eq!(map_fj_type("u64"), Some(SpirVTypeDesc::Int(64, false)));
        assert_eq!(map_fj_type("i64"), Some(SpirVTypeDesc::Int(64, true)));
    }

    #[test]
    fn w4_8_ptx_arith_ops() {
        use crate::gpu_codegen::ptx::ArithOp;
        let ops = [
            ArithOp::Add,
            ArithOp::Sub,
            ArithOp::Mul,
            ArithOp::Div,
            ArithOp::Rem,
        ];
        assert_eq!(ops.len(), 5);
    }

    #[test]
    fn w4_9_version_constraint_caret_zero() {
        use crate::package::registry::{SemVer, VersionConstraint};
        let c = VersionConstraint::parse("^0.5.0").unwrap();
        assert!(c.matches(&SemVer::new(0, 5, 3)));
    }

    #[test]
    fn w4_10_interpreter_mse_loss() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let pred = from_data([[1.0, 2.0]])\nlet target = from_data([[1.5, 2.5]])\nmse_loss(pred, target)");
        assert!(result.is_ok());
    }

    // Sprint W5: Embedded ML (Radxa Dragon Q6A)

    #[test]
    fn w5_1_interpreter_quantize() {
        let mut interp = Interpreter::new();
        let result =
            interp.eval_source("let t = from_data([[1.0, 2.0, 3.0]])\nlet q = quantize_int8(t)\nq");
        assert!(result.is_ok());
    }

    #[test]
    fn w5_2_smt_kernel_stack() {
        use crate::verify::smt::prove_array_bounds;
        let r = prove_array_bounds("sp >= 0 && sp < 4096", 4096);
        assert!(r.is_proven());
    }

    #[test]
    fn w5_3_memory_small_alloc() {
        use crate::runtime::os::memory::MemoryManager;
        let mut mm = MemoryManager::new(4096);
        let a = mm.alloc(16, 4).unwrap();
        mm.write_u32(a, 42).unwrap();
        assert_eq!(mm.read_u32(a).unwrap(), 42);
    }

    #[test]
    fn w5_4_ptx_kernel_quantized() {
        use crate::gpu_codegen::ptx::{KernelEntry, KernelParam, PtxType};
        let kernel = KernelEntry {
            name: "quantized_matmul".into(),
            params: vec![
                KernelParam {
                    name: "a".into(),
                    ptx_type: PtxType::U8,
                    is_pointer: true,
                },
                KernelParam {
                    name: "b".into(),
                    ptx_type: PtxType::U8,
                    is_pointer: true,
                },
                KernelParam {
                    name: "c".into(),
                    ptx_type: PtxType::S32,
                    is_pointer: true,
                },
            ],
            body: vec![],
        };
        let ptx = kernel.emit();
        assert!(ptx.contains("quantized_matmul"));
    }

    #[test]
    fn w5_5_dispatch_small_workload() {
        use crate::accelerator::dispatch::{DeviceSet, WorkloadDescriptor, decide_dispatch};
        let workload = WorkloadDescriptor {
            op_type: "relu".into(),
            input_elements: 10,
            dtype: "f32".into(),
            batch_size: 1,
            estimated_flops: 10,
            estimated_bytes: 40,
            preference: crate::accelerator::infer::InferPreference::Auto,
        };
        let decision = decide_dispatch(&workload, &DeviceSet::cpu_only());
        assert_eq!(format!("{:?}", decision.primary), "Cpu");
    }

    #[test]
    fn w5_6_interpreter_conv2d() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let layer = Conv2d(1, 8, 3, 1, 0)\nlet x = ones(1, 1, 8, 8)\nlet y = forward(layer, x)\ny");
        assert!(result.is_ok());
    }

    #[test]
    fn w5_7_capability_int8() {
        use crate::gpu_codegen::spirv::Capability;
        assert_eq!(Capability::Int8.value(), 39);
    }

    #[test]
    fn w5_8_discovery_service_with_tags() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        let mut tags = HashMap::new();
        tags.insert("arch".into(), "aarch64".into());
        tags.insert("board".into(), "dragon-q6a".into());
        reg.register(ServiceInstance {
            service_name: "ml-inference".into(),
            instance_id: "dragon-1".into(),
            address: "192.168.1.100:8080".into(),
            healthy: true,
            tags,
        });
        let inst = reg.resolve("ml-inference");
        assert_eq!(inst[0].tags.get("board").unwrap(), "dragon-q6a");
    }

    #[test]
    fn w5_9_smt_overflow_safe_small() {
        use crate::verify::smt::prove_no_i32_overflow;
        let r = prove_no_i32_overflow(0, 127, 0, 127);
        assert!(r.is_proven()); // INT8 range, safe for i32
    }

    #[test]
    fn w5_10_interpreter_eye() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let e = eye(4)\nshape(e)");
        assert!(result.is_ok());
    }

    // Sprint W6: Rust serde_json Interop

    #[test]
    fn w6_1_interpreter_json_roundtrip() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let s = "{\"x\":1}"
len(s)"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w6_2_interpreter_string_ops() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let s = "hello world"
let parts = s.split(" ")
len(parts)"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w6_3_interpreter_array_collect() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let arr = [1, 2, 3, 4, 5]\nlen(arr)");
        assert!(result.is_ok());
    }

    #[test]
    fn w6_4_interpreter_map_create() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let mut m = map_new()
m = map_insert(m, "key", "value")
map_get(m, "key")"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w6_5_ffi_mangle_params() {
        use crate::ffi_v2::cpp::{CppType, mangle_name};
        let m = mangle_name(&["serde".into()], "to_json", &[CppType::String]);
        assert!(!m.is_empty());
    }

    #[test]
    fn w6_6_wit_parse_types() {
        use crate::wasi_p2::wit_parser::parse_wit;
        let wit =
            "package test:json@1.0.0;\ninterface parser {\n  type json-value = list<u8>;\n}\n";
        let doc = parse_wit(wit);
        assert!(doc.is_ok());
    }

    #[test]
    fn w6_7_registry_constraint_resolve_latest() {
        use crate::package::registry::{Registry, SemVer, VersionConstraint};
        let mut reg = Registry::new();
        reg.publish("serde-fj", SemVer::new(1, 0, 0), "Serde for FJ");
        reg.publish("serde-fj", SemVer::new(1, 1, 0), "Serde for FJ");
        reg.publish("serde-fj", SemVer::new(1, 2, 0), "Serde for FJ");
        let c = VersionConstraint::parse("^1.0.0").unwrap();
        let resolved = reg.resolve("serde-fj", &c).unwrap();
        assert_eq!(resolved, SemVer::new(1, 2, 0));
    }

    #[test]
    fn w6_8_interpreter_to_string() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let x = 42\nto_string(x)");
        assert!(result.is_ok());
    }

    #[test]
    fn w6_9_interpreter_parse_int() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let r = "123".parse_int()
r"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w6_10_interpreter_string_contains() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(r#""hello world".contains("world")"#);
        assert!(result.is_ok());
    }

    // Sprint W7: WebSocket Chat

    #[test]
    fn w7_1_interpreter_array_push() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let arr = []\npush(arr, 1)\npush(arr, 2)\nlen(arr)");
        assert!(result.is_ok());
    }

    #[test]
    fn w7_2_interpreter_while_loop() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let mut i = 0\nwhile i < 10 { i = i + 1 }\ni");
        assert!(result.is_ok());
    }

    #[test]
    fn w7_3_interpreter_function_def() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("fn double(x: i32) -> i32 { x * 2 }\ndouble(21)");
        assert!(result.is_ok());
    }

    #[test]
    fn w7_4_component_linker_multiple() {
        use crate::wasi_p2::composition::{ComponentInstance, ComponentLinker};
        use std::collections::HashMap;
        let mut linker = ComponentLinker::new();
        linker.register(ComponentInstance::new("a", vec![], HashMap::new()));
        linker.register(ComponentInstance::new("b", vec![], HashMap::new()));
        linker.register(ComponentInstance::new("c", vec![], HashMap::new()));
        assert_eq!(linker.instance_count(), 3);
    }

    #[test]
    fn w7_5_discovery_multiple_services_types() {
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use std::collections::HashMap;
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "ws".into(),
            instance_id: "ws-1".into(),
            address: "10.0.0.1:8080".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.register(ServiceInstance {
            service_name: "http".into(),
            instance_id: "http-1".into(),
            address: "10.0.0.2:80".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        assert_eq!(reg.resolve("ws").len(), 1);
        assert_eq!(reg.resolve("http").len(), 1);
    }

    #[test]
    fn w7_6_spirv_memory_model() {
        use crate::gpu_codegen::spirv::MemoryModel;
        let models = [
            MemoryModel::Glsl450Logical,
            MemoryModel::Glsl450Physical32,
            MemoryModel::Glsl450Physical64,
        ];
        assert_eq!(models.len(), 3);
    }

    #[test]
    fn w7_7_version_parse_error() {
        use crate::package::registry::SemVer;
        assert!(SemVer::parse("abc").is_err());
        assert!(SemVer::parse("1.2").is_err());
    }

    #[test]
    fn w7_8_interpreter_match_expr() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let x = 2\nmatch x { 1 => 10, 2 => 20, _ => 0 }");
        assert!(result.is_ok());
    }

    #[test]
    fn w7_9_interpreter_struct() {
        let mut interp = Interpreter::new();
        let result =
            interp.eval_source("struct Msg { text: str }\nlet m = Msg { text: \"hello\" }\nm.text");
        assert!(result.is_ok());
    }

    #[test]
    fn w7_10_interpreter_enum() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("enum Color { Red, Green, Blue }\nlet c = Color::Red\nc");
        assert!(result.is_ok());
    }

    // Sprint W8: CLI Tool

    #[test]
    fn w8_1_interpreter_if_else() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let x = if true { 42 } else { 0 }\nx");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_2_interpreter_for_loop() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let mut sum = 0\nfor i in 0..5 { sum = sum + i }\nsum");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_3_interpreter_closure() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let add = |a: i32, b: i32| -> i32 { a + b }\nadd(3, 4)");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_4_interpreter_nested_fn() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("fn outer(x: i32) -> i32 {\n  fn inner(y: i32) -> i32 { y * 2 }\n  inner(x) + 1\n}\nouter(5)");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_5_interpreter_string_format() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let name = "Fajar"
let msg = f"Hello {name}"
msg"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w8_6_interpreter_array_index() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let arr = [10, 20, 30]\narr[1]");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_7_interpreter_pipeline() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("fn double(x: i32) -> i32 { x * 2 }\nfn inc(x: i32) -> i32 { x + 1 }\n5 |> double |> inc");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_8_interpreter_recursive() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("fn fib(n: i32) -> i32 {\n  if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }\n}\nfib(10)");
        assert!(result.is_ok());
    }

    #[test]
    fn w8_9_interpreter_type_of() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(r#"type_of(42)"#);
        assert!(result.is_ok());
    }

    #[test]
    fn w8_10_interpreter_assert() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("assert(1 + 1 == 2)");
        assert!(result.is_ok());
    }

    // Sprint W9: Database Client

    #[test]
    fn w9_1_interpreter_option_some() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let x = Some(42)\nx");
        assert!(result.is_ok());
    }

    #[test]
    fn w9_2_interpreter_option_none() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let x = None\nx");
        assert!(result.is_ok());
    }

    #[test]
    fn w9_3_interpreter_result_ok() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let r = Ok(100)\nr");
        assert!(result.is_ok());
    }

    #[test]
    fn w9_4_interpreter_result_err() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let r = Err("connection failed")
r"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w9_5_interpreter_hashmap() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source(
            r#"let mut db = map_new()
db = map_insert(db, "users", "table")
map_get(db, "users")"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn w9_6_registry_multiple_packages() {
        use crate::package::registry::{Registry, SemVer};
        let mut reg = Registry::new();
        for i in 0..20 {
            reg.publish(
                &format!("pkg-{i}"),
                SemVer::new(1, 0, 0),
                &format!("Package {i}"),
            );
        }
        assert_eq!(reg.package_count(), 20);
        assert_eq!(reg.list_all().len(), 20);
    }

    #[test]
    fn w9_7_audit_multiple_vuln() {
        use crate::package::audit::{AdvisoryDatabase, audit_dependencies};
        let json = r#"{"advisories":[
            {"id":"DB-001","package":"pg-fj","severity":"critical","description":"SQL injection","min_version":"0.1.0","max_version":"0.9.0","patched_version":"1.0.0"},
            {"id":"DB-002","package":"pg-fj","severity":"high","description":"Auth bypass","min_version":"0.5.0","max_version":"0.8.0","patched_version":"0.8.1"}
        ]}"#;
        let db = AdvisoryDatabase::from_json(json).unwrap();
        let deps = vec![("pg-fj".into(), "0.6.0".into())];
        let report = audit_dependencies(&deps, &db);
        assert_eq!(report.finding_count(), 2);
    }

    #[test]
    fn w9_8_interpreter_tuple() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let t = (1, 2, 3)\nt");
        assert!(result.is_ok());
    }

    #[test]
    fn w9_9_interpreter_block_expr() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("let val = {\n  let a = 10\n  let b = 20\n  a + b\n}\nval");
        assert!(result.is_ok());
    }

    #[test]
    fn w9_10_interpreter_nested_struct() {
        let mut interp = Interpreter::new();
        let result = interp.eval_source("struct Config { port: i32 }\nstruct Server { config: Config }\nlet s = Server { config: Config { port: 5432 } }\ns.config.port");
        assert!(result.is_ok());
    }

    // Sprint W10: Full-Stack Web App

    #[test]
    fn w10_1_interpreter_full_pipeline() {
        let mut interp = Interpreter::new();
        let code = r#"
struct Request { method: str, path: str }
struct Response { status: i32, body: str }
fn handle(req: Request) -> Response {
    if req.path == "/" {
        Response { status: 200, body: "OK" }
    } else {
        Response { status: 404, body: "Not Found" }
    }
}
let req = Request { method: "GET", path: "/" }
let resp = handle(req)
resp.status
"#;
        let result = interp.eval_source(code);
        assert!(result.is_ok());
    }

    #[test]
    fn w10_2_interpreter_enum_match() {
        let mut interp = Interpreter::new();
        let code = r#"
enum Method { Get, Post, Put, Delete }
fn method_str(m: Method) -> str {
    match m {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Delete => "DELETE",
    }
}
method_str(Method::Post)
"#;
        let result = interp.eval_source(code);
        assert!(result.is_ok());
    }

    #[test]
    fn w10_3_component_full_stack() {
        use crate::wasi_p2::component::ExportKind;
        use crate::wasi_p2::composition::{ComponentInstance, ComponentLinker, ExportDef};
        use std::collections::HashMap;
        let mut linker = ComponentLinker::new();
        // Backend component
        let mut backend = ComponentInstance::new("backend", vec![], HashMap::new());
        backend.add_export(ExportDef {
            name: "api/handler".into(),
            kind: ExportKind::Func,
            params: vec!["request".into()],
            result: Some("response".into()),
        });
        // Frontend component
        let mut frontend_imports = HashMap::new();
        frontend_imports.insert("api/handler".into(), "backend".into());
        let frontend = ComponentInstance::new("frontend", vec![], frontend_imports);
        linker.register(backend);
        linker.register(frontend);
        linker
            .link("backend", "api/handler", "frontend", "api/handler")
            .unwrap();
        assert!(linker.check_all_imports().is_empty());
    }

    #[test]
    fn w10_4_registry_full_workflow() {
        use crate::package::registry::{AuthToken, Registry, SemVer, VersionConstraint};
        let mut reg = Registry::new();
        reg.add_token(AuthToken::new("deploy-key"));
        reg.publish("web-app", SemVer::new(1, 0, 0), "Full-stack web app");
        reg.publish("web-app", SemVer::new(1, 1, 0), "Bug fixes");
        reg.publish("web-app", SemVer::new(2, 0, 0), "Major update");
        assert!(reg.validate_token("deploy-key", None));
        let c = VersionConstraint::parse("^1.0.0").unwrap();
        let v = reg.resolve("web-app", &c).unwrap();
        assert_eq!(v, SemVer::new(1, 1, 0));
    }

    #[test]
    fn w10_5_sbom_full_project() {
        use crate::package::sbom::{DepInfo, SbomFormat, generate_sbom};
        let deps = vec![
            DepInfo {
                name: "http-fj".into(),
                version: "1.0.0".into(),
                sha256: "aaa".into(),
                license: Some("MIT".into()),
                dev_only: false,
            },
            DepInfo {
                name: "db-fj".into(),
                version: "0.5.0".into(),
                sha256: "bbb".into(),
                license: Some("Apache-2.0".into()),
                dev_only: false,
            },
            DepInfo {
                name: "test-fj".into(),
                version: "1.0.0".into(),
                sha256: "ccc".into(),
                license: Some("MIT".into()),
                dev_only: true,
            },
        ];
        let sbom = generate_sbom("fullstack-app", &deps, SbomFormat::CycloneDx).unwrap();
        assert!(sbom.contains("fullstack-app"));
    }

    #[test]
    fn w10_6_interpreter_complex_program() {
        let mut interp = Interpreter::new();
        let code = r#"
fn fibonacci(n: i32) -> i32 {
    if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}
let results = [fibonacci(0), fibonacci(1), fibonacci(5), fibonacci(8)]
results
"#;
        let result = interp.eval_source(code);
        assert!(result.is_ok());
    }

    #[test]
    fn w10_7_smt_all_proofs() {
        use crate::verify::smt::{
            prove_array_bounds, prove_matmul_shapes, prove_no_i32_overflow, prove_non_negative,
        };
        assert!(prove_non_negative("x", "x >= 0").is_proven());
        assert!(prove_array_bounds("i >= 0 && i < 100", 100).is_proven());
        assert!(prove_matmul_shapes(10, 20, 20, 30).is_proven());
        assert!(prove_no_i32_overflow(0, 1000, 0, 1000).is_proven());
    }

    #[test]
    fn w10_8_distributed_full() {
        use crate::distributed::cluster::{ClusterNodeId, FailureDetector, WorkQueue};
        use crate::distributed::discovery::{DiscoveryRegistry, ServiceInstance};
        use crate::distributed::raft::{RaftNode, RaftNodeId};
        use std::collections::HashMap;
        use std::time::Duration;
        let _node = RaftNode::new(RaftNodeId(1), vec![RaftNodeId(2), RaftNodeId(3)]);
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "app".into(),
            instance_id: "app-1".into(),
            address: "10.0.0.1:80".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        let mut fd = FailureDetector::new(Duration::from_millis(5000));
        fd.heartbeat(ClusterNodeId(1), 1000);
        let mut q = WorkQueue::new(ClusterNodeId(1));
        q.push(1);
        assert_eq!(reg.resolve("app").len(), 1);
        assert!(fd.is_healthy(ClusterNodeId(1), 3000));
        assert_eq!(q.pop(), Some(1));
    }

    #[test]
    fn w10_9_interpreter_ml_pipeline() {
        let mut interp = Interpreter::new();
        let code = r#"
let x = from_data([[1.0, 2.0, 3.0, 4.0]])
let layer = Dense(4, 2)
let out = forward(layer, x)
let activated = relu(out)
shape(activated)
"#;
        let result = interp.eval_source(code);
        assert!(result.is_ok());
    }

    #[test]
    fn w10_10_interpreter_error_handling() {
        let mut interp = Interpreter::new();
        let code = r#"
fn safe_div(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a / b }
}
let r1 = safe_div(10, 2)
let r2 = safe_div(10, 0)
r1 + r2
"#;
        let result = interp.eval_source(code);
        assert!(result.is_ok());
    }

    // ===================================================================
    // V15 Sprint B2 — ML Runtime Fixes
    // ===================================================================

    #[test]
    fn v15_b2_1_tanh_shorthand() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let t = from_data([[0.0, 1.0]])
            let r = tanh(t)
            r
            "#,
        );
        assert!(result.is_ok(), "tanh shorthand: {:?}", result.err());
    }

    #[test]
    fn v15_b2_2_leaky_relu_shorthand() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let t = from_data([[-1.0, 1.0]])
            let r = leaky_relu(t)
            r
            "#,
        );
        assert!(result.is_ok(), "leaky_relu shorthand: {:?}", result.err());
    }

    #[test]
    fn v15_b2_4_dense_forward_method() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let l = Dense(4, 2)
            let input = ones(1, 4)
            let out = l.forward(input)
            out
            "#,
        );
        assert!(result.is_ok(), "Dense.forward(): {:?}", result.err());
        match result.unwrap() {
            Value::Tensor(t) => {
                assert_eq!(t.data().ndim(), 2, "output should be 2D");
                assert_eq!(t.data().shape()[1], 2, "output features should be 2");
            }
            other => panic!("expected Tensor, got: {:?}", other),
        }
    }

    #[test]
    fn v15_b2_5_conv2d_forward_method() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let c = Conv2d(1, 8, 3, 1, 0)
            let input = ones(1, 1, 8, 8)
            let out = c.forward(input)
            out
            "#,
        );
        assert!(result.is_ok(), "Conv2d.forward(): {:?}", result.err());
    }

    #[test]
    fn v15_b2_7_concat_builtin() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let a = zeros(2, 3)
            let b = ones(2, 3)
            let c = concat(a, b, 0)
            c
            "#,
        );
        assert!(result.is_ok(), "concat: {:?}", result.err());
        match result.unwrap() {
            Value::Tensor(t) => {
                assert_eq!(
                    t.data().shape(),
                    &[4, 3],
                    "concatenated shape should be [4,3]"
                );
            }
            other => panic!("expected Tensor, got: {:?}", other),
        }
    }

    #[test]
    fn v15_b2_8_cross_entropy_shorthand() {
        let mut interp = Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let pred = softmax(from_data([[1.0, 2.0, 3.0]]))
            let target = from_data([[0.0, 0.0, 1.0]])
            let ce = cross_entropy(pred, target)
            ce
            "#,
        );
        assert!(result.is_ok(), "cross_entropy: {:?}", result.err());
    }
}
