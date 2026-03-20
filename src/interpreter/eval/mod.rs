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

use crate::interpreter::env::Environment;
use crate::interpreter::value::{FnValue, Value};
use crate::parser::ast::{
    BinOp, CallArg, Expr, FStringExprPart, Item, LiteralKind, Program, Stmt, UnaryOp,
};
use crate::runtime::ml::Tape;
use crate::runtime::os::OsRuntime;

/// Default maximum recursion depth to prevent stack overflow.
/// Kept conservative (64) to avoid actual Rust stack overflow in debug builds
/// with the larger Interpreter struct (tape, modules, etc.).
const DEFAULT_MAX_RECURSION_DEPTH: usize = 64;

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
        };
        interp.register_builtins();
        interp
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
            "tensor_grad",
            "tensor_requires_grad",
            "tensor_set_requires_grad",
            "tensor_detach",
            "tensor_no_grad_begin",
            "tensor_no_grad_end",
            "tensor_clear_tape",
            // Optimizers
            "optimizer_sgd",
            "optimizer_adam",
            "optimizer_step",
            "optimizer_zero_grad",
            // Layers
            "layer_dense",
            "layer_forward",
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
            Expr::Await { .. } => Err(RuntimeError::TypeError(
                "await is not supported in interpreter mode".into(),
            )
            .into()),
            Expr::AsyncBlock { .. } => Err(RuntimeError::TypeError(
                "async blocks are not supported in interpreter mode".into(),
            )
            .into()),
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
                self.call_builtin(&name, vals)
            }
            _ => {
                let desc = format!("{func}");
                Err(RuntimeError::NotAFunction(desc).into())
            }
        }
    }

    /// Calls a user-defined function with given arguments.
    fn call_function(&mut self, fv: &FnValue, args: Vec<Value>) -> EvalResult {
        if args.len() != fv.params.len() {
            return Err(RuntimeError::ArityMismatch {
                expected: fv.params.len(),
                got: args.len(),
            }
            .into());
        }

        self.call_depth += 1;
        let fn_name = if fv.name.is_empty() {
            "<closure>".to_string()
        } else {
            fv.name.clone()
        };
        self.call_stack.push(fn_name);

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
        let src = "fn inf(n: i64) -> i64 { inf(n) }\ninf(0)";
        let err = eval(src).unwrap_err();
        assert!(matches!(err, RuntimeError::StackOverflow { .. }));
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
}
