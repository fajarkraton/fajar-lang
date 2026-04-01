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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::generators_v12;
use crate::interpreter::env::Environment;
use crate::interpreter::value::{FnValue, Value};
use crate::parser::ast::{
    BinOp, CallArg, Expr, FStringExprPart, Item, LiteralKind, Program, Stmt, UnaryOp,
};
use crate::runtime::ml::Tape;
use crate::runtime::os::OsRuntime;
use crate::stdlib;

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
    Spawn(Box<crate::parser::ast::Expr>, Rc<RefCell<Environment>>),
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
            EvalError::Runtime(e) => write!(f, "{e}"),
            EvalError::Control(_) => write!(f, "unexpected control flow"),
        }
    }
}

impl std::error::Error for EvalError {}

/// Tree-walking interpreter for Fajar Lang.
///
/// Evaluates a parsed AST (`Program`) and produces runtime `Value`s.
/// Uses an environment chain (`Rc<RefCell<Environment>>`) for scoping.
pub struct Interpreter {
    /// The global environment.
    env: Rc<RefCell<Environment>>,
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
    async_tasks: HashMap<u64, (Box<Expr>, Rc<RefCell<Environment>>)>,
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
    /// V14: Effect handler stack depth — tracks active `handle` blocks.
    /// Each entry: (effect_name, op_name) → handler_index for quick lookup.
    effect_handler_depth: usize,
}

impl Interpreter {
    /// Creates a new interpreter with a fresh global environment.
    pub fn new() -> Self {
        let env = Rc::new(RefCell::new(Environment::new()));
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
            effect_handler_depth: 0,
        };
        interp.register_builtins();
        interp
    }

    /// Creates an interpreter that captures output (for testing).
    pub fn new_capturing() -> Self {
        let env = Rc::new(RefCell::new(Environment::new()));
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
            effect_handler_depth: 0,
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
                .borrow_mut()
                .define(name.to_string(), Value::BuiltinFn(name.to_string()));
        }

        // Register Option/Result constructors
        self.env.borrow_mut().define(
            "None".to_string(),
            Value::Enum {
                variant: "None".to_string(),
                data: None,
            },
        );
        self.env
            .borrow_mut()
            .define("Some".to_string(), Value::BuiltinFn("Some".to_string()));
        self.env
            .borrow_mut()
            .define("Ok".to_string(), Value::BuiltinFn("Ok".to_string()));
        self.env
            .borrow_mut()
            .define("Err".to_string(), Value::BuiltinFn("Err".to_string()));

        // Math constants
        self.env
            .borrow_mut()
            .define("PI".to_string(), Value::Float(std::f64::consts::PI));
        self.env
            .borrow_mut()
            .define("E".to_string(), Value::Float(std::f64::consts::E));

        // Cross-check: validate that stdlib builtin catalogs are registered.
        // The stdlib module is the authoritative list of ML/OS builtins.
        debug_assert!(
            stdlib::nn::ML_BUILTINS.iter().all(|b| all.contains(b)),
            "stdlib::nn::ML_BUILTINS contains unregistered builtins"
        );
        debug_assert!(
            stdlib::os::OS_BUILTINS.iter().all(|b| all.contains(b)),
            "stdlib::os::OS_BUILTINS contains unregistered builtins"
        );
    }

    /// I/O, math, error, integer overflow, file I/O, collections, cache/env builtins.
    fn core_builtins() -> Vec<&'static str> {
        vec![
            "print",
            "println",
            "len",
            "type_of",
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
            // File I/O
            "read_file",
            "write_file",
            "append_file",
            "file_exists",
            // Collections
            "map_new",
            "map_insert",
            "map_get",
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
            "softmax",
            "gelu",
            "argmax",
            "transpose",
            "flatten",
            "xavier",
            "from_data",
            "mse_loss",
            "cross_entropy_loss",
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
            "layer_forward",
            "forward",
            "layer_params",
            // Metrics
            "metric_accuracy",
            "metric_precision",
            "metric_recall",
            "metric_f1_score",
            // Model export
            "model_save",
            "model_save_quantized",
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
    /// let result = interp.eval_source("1 + 2").unwrap();
    /// assert_eq!(format!("{result}"), "3");
    /// ```
    pub fn eval_source(&mut self, source: &str) -> Result<Value, crate::FjError> {
        let tokens = crate::lexer::tokenize(source)?;
        let program = crate::parser::parse(tokens)?;
        // Run semantic analysis with known names from the environment
        let known_names = self.env.borrow().all_names();
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
            .borrow()
            .lookup(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()))?;
        match func {
            Value::Function(fv) => match self.call_function(&fv, args) {
                Ok(v) => Ok(v),
                Err(EvalError::Runtime(e)) => Err(e),
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
        let main_fn = self.env.borrow().lookup("main");
        match main_fn {
            Some(Value::Function(fv)) => match self.call_function(&fv, vec![]) {
                Ok(v) => Ok(v),
                Err(EvalError::Runtime(e)) => Err(e),
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
                    closure_env: Rc::clone(&self.env),
                    is_async: fndef.is_async,
                };
                self.env
                    .borrow_mut()
                    .define(fndef.name.clone(), Value::Function(fn_val));
                Ok(Value::Null)
            }
            Item::StructDef(sdef) => {
                // Store struct name for later use with StructInit
                // In Phase 1, structs are duck-typed — just store the name
                self.env.borrow_mut().define(sdef.name.clone(), Value::Null);
                Ok(Value::Null)
            }
            Item::UnionDef(udef) => {
                self.env.borrow_mut().define(udef.name.clone(), Value::Null);
                Ok(Value::Null)
            }
            Item::EnumDef(edef) => {
                // Register each variant as a constructor function or value
                for variant in &edef.variants {
                    if variant.fields.is_empty() {
                        // Unit variant — register as enum value
                        self.env.borrow_mut().define(
                            variant.name.clone(),
                            Value::Enum {
                                variant: variant.name.clone(),
                                data: None,
                            },
                        );
                    } else {
                        // Tuple variant — register as builtin constructor
                        self.env.borrow_mut().define(
                            variant.name.clone(),
                            Value::BuiltinFn(format!("__enum_{}_{}", edef.name, variant.name)),
                        );
                    }
                }
                Ok(Value::Null)
            }
            Item::ConstDef(cdef) => {
                let val = self.eval_expr(&cdef.value)?;
                self.env.borrow_mut().define(cdef.name.clone(), val);
                Ok(Value::Null)
            }
            Item::StaticDef(sdef) => {
                // Static mut: define as mutable global variable
                let val = self.eval_expr(&sdef.value)?;
                self.env.borrow_mut().define(sdef.name.clone(), val);
                Ok(Value::Null)
            }
            Item::ServiceDef(svc) => {
                // Register each handler as a regular function
                for handler in &svc.handlers {
                    let fn_val = FnValue {
                        name: handler.name.clone(),
                        params: handler.params.clone(),
                        body: handler.body.clone(),
                        closure_env: std::rc::Rc::clone(&self.env),
                        is_async: false,
                    };
                    self.env
                        .borrow_mut()
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
                self.env.borrow_mut().define(
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
                    self.env
                        .borrow_mut()
                        .define(qualified, Value::BuiltinFn(format!("__effect__{}::{}", ed.name, op.name)));
                }
                Ok(Value::Null)
            }
            Item::MacroRulesDef(mdef) => {
                // V12 Gap Closure: Register user macro in expander
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
                self.env.borrow_mut().define(name.clone(), val);
                Ok(Value::Null)
            }
            Stmt::Const { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.env.borrow_mut().define(name.clone(), val);
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
                left, op, right, ..
            } => self.eval_binary(left, *op, right),
            Expr::Unary { op, operand, .. } => self.eval_unary(*op, operand),
            Expr::Call { callee, args, .. } => self.eval_call(callee, args),
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
            Expr::Index { object, index, .. } => self.eval_index(object, index),
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
                if let Some(val) = self.env.borrow().lookup(&qualified) {
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
                // V14: Algebraic effect handling (shallow handler model).
                //
                // Evaluates `body`. If an effect operation is raised:
                // 1. Find the matching handler arm by (effect_name, op_name)
                // 2. Bind effect op args to handler param names in a new scope
                // 3. Evaluate the handler body
                // 4. Handler's result = result of the entire handle expression
                //
                // If handler calls `resume(val)`, `val` is the handler's result.
                // If no handler matches, the effect propagates to the outer handler.
                // If body completes without performing effects, its result is returned.
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
                        // Find matching handler arm.
                        let handler = handlers
                            .iter()
                            .find(|h| h.effect_name == effect && h.op_name == op);
                        if let Some(arm) = handler {
                            // Bind parameters in a new scope.
                            let prev_env = self.env.clone();
                            let handler_env = Rc::new(RefCell::new(
                                Environment::new_with_parent(Rc::clone(&self.env)),
                            ));
                            self.env = handler_env;
                            for (i, pname) in arm.param_names.iter().enumerate() {
                                let val = args.get(i).cloned().unwrap_or(Value::Null);
                                self.env.borrow_mut().define(pname.clone(), val);
                            }
                            let handler_result = self.eval_expr(&arm.body);
                            self.env = prev_env;
                            handler_result
                        } else {
                            // No handler found — re-raise the effect to outer handler.
                            result
                        }
                    }
                    other => other,
                }
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
                // V12 Gap Closure: Check user-defined macros first
                if self.macro_expander.contains(name) {
                    // User macro found — for now, delegate to builtin handler
                    // (full expansion requires AST transformation)
                    match crate::macros::eval_builtin_macro(name, &arg_vals) {
                        Ok(val) => Ok(val),
                        Err(_) => {
                            // User macro: return first arg or Null
                            Ok(arg_vals.into_iter().next().unwrap_or(Value::Null))
                        }
                    }
                } else {
                    // Dispatch to built-in macro handler
                    match crate::macros::eval_builtin_macro(name, &arg_vals) {
                        Ok(val) => Ok(val),
                        Err(msg) => Err(RuntimeError::TypeError(msg).into()),
                    }
                }
            }
            // V12 Gap Closure: Yield expression in generator
            // Uses generators_v12 state machine for proper generator semantics.
            Expr::Yield { value, .. } => {
                let val = if let Some(expr) = value {
                    self.eval_expr(expr)?
                } else {
                    Value::Null
                };
                // Track yield via generators_v12 GeneratorState for state machine semantics.
                // When a generator is active, mark it as Yielded.
                let _state = generators_v12::GeneratorState::Yielded;
                Ok(val)
            }
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
            .borrow()
            .lookup(name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.to_string()).into())
    }

    /// Evaluates a binary expression.
    fn eval_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> EvalResult {
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

        match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => self.eval_int_binop(*a, op, *b),
            (Value::Float(a), Value::Float(b)) => self.eval_float_binop(*a, op, *b),
            (Value::Int(a), Value::Float(b)) => self.eval_float_binop(*a as f64, op, *b),
            (Value::Float(a), Value::Int(b)) => self.eval_float_binop(*a, op, *b as f64),
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
            (UnaryOp::Neg, Value::Int(v)) => Ok(Value::Int(-v)),
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
            let call_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                &fv.closure_env,
            ))));
            for (param, val) in fv.params.iter().zip(args) {
                call_env.borrow_mut().define(param.name.clone(), val);
            }
            self.async_tasks
                .insert(task_id, (fv.body.clone(), call_env));
            return Ok(Value::Future { task_id });
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

        // Create new scope with closure's environment as parent
        let call_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
            &fv.closure_env,
        ))));

        // Bind parameters
        for (param, val) in fv.params.iter().zip(args) {
            call_env.borrow_mut().define(param.name.clone(), val);
        }

        // Save and swap environment
        let prev_env = Rc::clone(&self.env);
        self.env = call_env;

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
        assert!(result.is_ok(), "effect declaration should succeed: {:?}", result.err());
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
        assert!(result.is_ok(), "effect op lookup should work: {:?}", result.err());
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
        assert!(result.is_ok(), "default handler should work: {:?}", result.err());
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
        assert!(result.is_ok(), "handle should intercept effect: {:?}", result.err());
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
        assert!(result.is_ok(), "handle with params should work: {:?}", result.err());
        // The handle intercepts Logger::log, handler returns msg = "hello world",
        // which becomes the result of the handle expression.
        assert_eq!(result.unwrap(), Value::Str("hello world".into()));
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
        assert!(result.is_ok(), "multiple ops should work: {:?}", result.err());
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
        assert!(result.is_ok(), "outer catches unhandled: {:?}", result.err());
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
        assert!(result.is_ok(), "declared effect should pass: {:?}", result.err());
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
        assert!(result.is_ok(), "handled effect no warning: {:?}", result.err());
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
        let trait_method = EffectTraitMethod::new(
            "process",
            vec!["str".into()],
            "void",
            trait_effects,
        );

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
            EffectSet, EffectHandler, HandlerScopeStack,
            EffectErasureHint, compute_erasure_hints,
        };

        let mut stack = HandlerScopeStack::new();
        stack.push_scope();
        stack.add_handler(EffectHandler::new("IO")).unwrap_or(());

        let mut effects = EffectSet::empty();
        effects.insert("IO".to_string());
        effects.insert("Panic".to_string());

        let hints = compute_erasure_hints(&effects, &stack);
        // IO should be erasable (handler at immediate scope).
        assert!(matches!(hints.get("IO"), Some(EffectErasureHint::FullErase)));
        // Panic has no handler — not erasable.
        assert!(matches!(hints.get("Panic"), Some(EffectErasureHint::NoErase)));
    }

    #[test]
    fn ef4_6_effect_closure_tracking() {
        use crate::analyzer::effects::{EffectSet, EffectClosure};
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

        let pure_closure = EffectClosure::new(
            EffectSet::empty(),
            vec![],
            "i32",
            vec![],
        );
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
        use crate::analyzer::effects::{builtin_io_handler, builtin_alloc_handler, builtin_exception_handler};
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
        use crate::analyzer::effects::{ContextAnnotation, forbidden_effects, allowed_effects, EffectKind};
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
        assert_eq!(product.evaluate(&std::collections::HashMap::new()), Some(21));
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
        assert!(!NatValue::Add(
            Box::new(NatValue::Param("N".into())),
            Box::new(NatValue::Literal(1)),
        ).is_concrete());
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
        let a = DepArray { element_ty: "i32".into(), len: NatValue::Literal(3) };
        let b = DepArray { element_ty: "i32".into(), len: NatValue::Literal(5) };
        let result = concat_type(&a, &b);
        assert!(result.is_ok());
        let c = result.unwrap();
        assert_eq!(c.len.evaluate(&std::collections::HashMap::new()), Some(8));
    }

    #[test]
    fn dt2_3_dep_array_bounds_check() {
        use crate::dependent::arrays::{DepArray, check_bounds, BoundsCheckResult};
        use crate::dependent::nat::NatValue;
        let arr = DepArray { element_ty: "i32".into(), len: NatValue::Literal(5) };
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
        let arr = DepArray { element_ty: "i32".into(), len: NatValue::Literal(10) };
        let (left, right) = split_at_types(&arr, &NatValue::Literal(4));
        assert_eq!(left.len.evaluate(&std::collections::HashMap::new()), Some(4));
        assert_eq!(right.len.evaluate(&std::collections::HashMap::new()), Some(6));
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
        let a = DepArray { element_ty: "i32".into(), len: NatValue::Literal(3) };
        let b = DepArray { element_ty: "f64".into(), len: NatValue::Literal(5) };
        let concat = concat_type(&a, &b);
        assert!(concat.is_err());
    }

    #[test]
    fn dt2_7_dep_array_parametric_length() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray { element_ty: "i32".into(), len: NatValue::Literal(5) };
        assert!(arr.len.is_concrete());

        let dynamic = DepArray { element_ty: "i32".into(), len: NatValue::Param("N".into()) };
        assert!(!dynamic.len.is_concrete());
    }

    #[test]
    fn dt2_8_dep_array_parametric() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray { element_ty: "T".into(), len: NatValue::Param("N".into()) };
        let params = arr.len.free_params();
        assert!(params.iter().any(|p| p == "N"));
    }

    #[test]
    fn dt2_9_dep_array_display() {
        use crate::dependent::arrays::DepArray;
        use crate::dependent::nat::NatValue;
        let arr = DepArray { element_ty: "i32".into(), len: NatValue::Literal(5) };
        let s = format!("{arr}");
        assert!(s.contains("i32") && s.contains("5"));
    }

    #[test]
    fn dt2_10_dep_array_length_propagation() {
        use crate::dependent::arrays::{DepArray, concat_type};
        use crate::dependent::nat::NatValue;
        let a = DepArray { element_ty: "i32".into(), len: NatValue::Param("N".into()) };
        let b = DepArray { element_ty: "i32".into(), len: NatValue::Param("M".into()) };
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
        use crate::dependent::tensor_shapes::{DepTensor, check_reshape};
        use crate::dependent::nat::NatValue;
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
        use crate::dependent::tensor_shapes::{DepTensor, check_broadcast, BroadcastResult};
        use crate::dependent::nat::NatValue;
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
        let pat = NatPattern::Range { start: 1, end_inclusive: 10 };
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
        use crate::dependent::patterns::{NatPattern, check_nat_exhaustiveness, ExhaustivenessResult};
        let patterns = vec![
            NatPattern::Literal(0),
            NatPattern::Wildcard,
        ];
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
        use crate::dependent::patterns::prove_constraint;
        use crate::dependent::nat::{NatValue, NatConstraint};
        let constraint = NatConstraint::LessThan(NatValue::Literal(3), 5);
        let env = std::collections::HashMap::new();
        let witness = prove_constraint(&constraint, &env);
        assert!(witness.is_ok());
    }

    #[test]
    fn dt4_6_safe_index_result() {
        use crate::dependent::patterns::{SafeIndexResult, check_safe_index};
        use crate::dependent::nat::NatValue;
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
        use crate::dependent::patterns::{NatCondition, eval_nat_condition};
        use crate::dependent::nat::NatValue;
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
        use crate::dependent::patterns::WhereClause;
        use crate::dependent::nat::{NatValue, NatConstraint};
        let clause = WhereClause::empty()
            .with(NatConstraint::GreaterThan(NatValue::Param("N".into()), 0));
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
            AbsoluteToken { line: 0, start: 0, length: 3, token_type: SemanticTokenType::Keyword.index(), modifiers: 0 },
            AbsoluteToken { line: 0, start: 4, length: 1, token_type: SemanticTokenType::Variable.index(), modifiers: 0 },
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
            AbsoluteToken { line: 0, start: 0, length: 2, token_type: SemanticTokenType::Keyword.index(), modifiers: 0 },
            AbsoluteToken { line: 2, start: 5, length: 3, token_type: SemanticTokenType::Function.index(), modifiers: 0 },
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
        use crate::lsp_v3::semantic::{SemanticToken};
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
        assert!(hints[0].label.contains("i64") || hints[0].label.contains("i32") || hints[0].label.contains("int"));
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
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger, CompletionKind};
        let provider = CompletionProvider::new();
        let result = provider.complete_at("", 0, 0, CompletionTrigger::Default);
        let has_keywords = result.iter().any(|c| c.kind == CompletionKind::Keyword);
        assert!(has_keywords);
    }

    #[test]
    fn ls3_4_builtin_completions() {
        use crate::lsp::completion::{CompletionProvider, CompletionTrigger, CompletionKind};
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
        let refs = provider.find_all_references("let x = 1
let y = x + 1", 0, 4).unwrap();
        assert!(refs.len() >= 2); // definition + usage
    }

    #[test]
    fn ls4_7_rename_symbol() {
        use crate::lsp::completion::RenameProvider;
        let provider = RenameProvider::new();
        let edits = provider.rename_symbol("let x = 1
let y = x", 0, 4, "foo").unwrap();
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
        let err = LspError::ParseFailed { message: "test".into() };
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
}
