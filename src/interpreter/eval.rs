//! Expression and statement evaluation.
//!
//! Core dispatch functions: `eval_expr`, `eval_stmt`, `eval_item`.
//! The interpreter is a tree-walking evaluator over the untyped AST.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::interpreter::env::Environment;
use crate::interpreter::value::{FnValue, IteratorValue, LayerValue, OptimizerValue, Value};
use crate::parser::ast::{
    AssignOp, BinOp, CallArg, Expr, FStringExprPart, FieldInit, Item, LiteralKind, MatchArm,
    ModDecl, Pattern, Program, Stmt, TypeExpr, UnaryOp, UseDecl, UseKind,
};
use crate::runtime::ml::{tensor_ops, Tape, TensorValue};
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
    /// A `break` statement with an optional value.
    Break(Value),
    /// A `continue` statement.
    Continue,
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
        let builtins = [
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
            // Integer overflow control builtins
            "wrapping_add",
            "wrapping_sub",
            "wrapping_mul",
            "checked_add",
            "checked_sub",
            "checked_mul",
            "saturating_add",
            "saturating_sub",
            "saturating_mul",
            // Error/debug builtins
            "panic",
            "todo",
            "dbg",
            "eprint",
            "eprintln",
            // Math builtins
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
            // OS runtime builtins
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
            // ML runtime builtins
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
            "tensor_shape",
            "tensor_reshape",
            "tensor_numel",
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
            "tensor_sum",
            "tensor_mean",
            "tensor_max",
            "tensor_min",
            "tensor_argmax",
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
            // Activation functions
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
            // Autograd builtins
            "tensor_backward",
            "tensor_grad",
            "tensor_requires_grad",
            "tensor_set_requires_grad",
            "tensor_detach",
            "tensor_no_grad_begin",
            "tensor_no_grad_end",
            "tensor_clear_tape",
            // Optimizer builtins
            "optimizer_sgd",
            "optimizer_adam",
            "optimizer_step",
            "optimizer_zero_grad",
            // Layer builtins
            "layer_dense",
            "layer_forward",
            "layer_params",
            // Metrics builtins
            "metric_accuracy",
            "metric_precision",
            "metric_recall",
            "metric_f1_score",
            // File I/O builtins
            "read_file",
            "write_file",
            "append_file",
            "file_exists",
            // Collection builtins
            "map_new",
            "map_insert",
            "map_get",
            "map_remove",
            "map_contains_key",
            "map_keys",
            "map_values",
            "map_len",
            // Hardware detection builtins (v1.1)
            "hw_cpu_vendor",
            "hw_cpu_arch",
            "hw_has_avx2",
            "hw_has_avx512",
            "hw_has_amx",
            "hw_has_neon",
            "hw_has_sve",
            "hw_simd_width",
            // Accelerator registry builtins (v1.1 S4)
            "hw_gpu_count",
            "hw_npu_count",
            "hw_best_accelerator",
            // GPIO builtins (v2.0 Q6A)
            "gpio_open",
            "gpio_close",
            "gpio_set_direction",
            "gpio_write",
            "gpio_read",
            "gpio_toggle",
            // UART builtins (v2.0 Q6A)
            "uart_open",
            "uart_close",
            "uart_write_byte",
            "uart_read_byte",
            "uart_write_str",
            // PWM builtins (v2.0 Q6A)
            "pwm_open",
            "pwm_close",
            "pwm_set_frequency",
            "pwm_set_duty",
            "pwm_enable",
            "pwm_disable",
            // SPI builtins (v2.0 Q6A)
            "spi_open",
            "spi_close",
            "spi_transfer",
            "spi_write",
            // NPU builtins (v2.0 Q6A)
            "npu_available",
            "npu_info",
            "npu_load",
            "npu_infer",
            // Timing builtins (v2.0)
            "delay_ms",
            "delay_us",
        ];
        for name in &builtins {
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
            Stmt::Break { value, .. } => {
                let val = match value {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Null,
                };
                Err(ControlFlow::Break(val).into())
            }
            Stmt::Continue { .. } => Err(ControlFlow::Continue.into()),
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
                condition, body, ..
            } => self.eval_while(condition, body),
            Expr::For {
                variable,
                iterable,
                body,
                ..
            } => self.eval_for(variable, iterable, body),
            Expr::Loop { body, .. } => self.eval_loop(body),
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

    /// Calls a built-in function.
    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> EvalResult {
        match name {
            "print" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    print!("{output}");
                }
                Ok(Value::Null)
            }
            "println" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    println!("{output}");
                }
                Ok(Value::Null)
            }
            "len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(s) => Ok(Value::Int(s.len() as i64)),
                    Value::Array(a) => Ok(Value::Int(a.len() as i64)),
                    Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
                    Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                    _ => Err(RuntimeError::TypeError(format!(
                        "len() not supported for {}",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "type_of" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Str(args[0].type_name().to_string()))
            }
            "push" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Array(a) => {
                        let mut new_arr = a.clone();
                        new_arr.push(args[1].clone());
                        Ok(Value::Array(new_arr))
                    }
                    _ => Err(RuntimeError::TypeError("push() requires an array".into()).into()),
                }
            }
            "pop" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Array(a) => {
                        if a.is_empty() {
                            Ok(Value::Null)
                        } else {
                            Ok(a.last().cloned().unwrap_or(Value::Null))
                        }
                    }
                    _ => Err(RuntimeError::TypeError("pop() requires an array".into()).into()),
                }
            }
            "to_string" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Str(format!("{}", args[0])))
            }
            "format" => {
                // format("Hello {}, age {}", name, age) → "Hello Alice, age 30"
                if args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: 0,
                    }
                    .into());
                }
                let template = match &args[0] {
                    Value::Str(s) => s.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "format() first argument must be a string".into(),
                        )
                        .into())
                    }
                };
                let mut result = String::new();
                let mut arg_idx = 1;
                let mut chars = template.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '{' && chars.peek() == Some(&'}') {
                        chars.next(); // consume '}'
                        if arg_idx < args.len() {
                            result.push_str(&format!("{}", args[arg_idx]));
                            arg_idx += 1;
                        } else {
                            result.push_str("{}");
                        }
                    } else {
                        result.push(c);
                    }
                }
                Ok(Value::Str(result))
            }
            "to_int" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Int(v) => Ok(Value::Int(*v)),
                    Value::Float(v) => Ok(Value::Int(*v as i64)),
                    Value::Str(s) => s.parse::<i64>().map(Value::Int).map_err(|_| {
                        RuntimeError::TypeError(format!("cannot convert '{s}' to int")).into()
                    }),
                    Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                    _ => Err(RuntimeError::TypeError(format!(
                        "cannot convert {} to int",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "to_float" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Float(v) => Ok(Value::Float(*v)),
                    Value::Int(v) => Ok(Value::Float(*v as f64)),
                    Value::Str(s) => s.parse::<f64>().map(Value::Float).map_err(|_| {
                        RuntimeError::TypeError(format!("cannot convert '{s}' to float")).into()
                    }),
                    _ => Err(RuntimeError::TypeError(format!(
                        "cannot convert {} to float",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "assert" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                if !args[0].is_truthy() {
                    return Err(RuntimeError::TypeError("assertion failed".into()).into());
                }
                Ok(Value::Null)
            }
            "assert_eq" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                if args[0] != args[1] {
                    return Err(RuntimeError::TypeError(format!(
                        "assertion failed: {} != {}",
                        args[0], args[1]
                    ))
                    .into());
                }
                Ok(Value::Null)
            }
            // ── Error/debug builtins ──
            "panic" => {
                let msg = if args.is_empty() {
                    "explicit panic".to_string()
                } else {
                    format!("{}", args[0])
                };
                Err(RuntimeError::TypeError(format!("panic: {msg}")).into())
            }
            "todo" => {
                let msg = if args.is_empty() {
                    "not yet implemented".to_string()
                } else {
                    format!("{}", args[0])
                };
                Err(RuntimeError::TypeError(format!("todo: {msg}")).into())
            }
            "dbg" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                let output = format!("[dbg] {}", args[0]);
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprintln!("{output}");
                }
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            }
            "eprint" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprint!("{output}");
                }
                Ok(Value::Null)
            }
            "eprintln" => {
                let text: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                let output = text.join(" ");
                if self.capture_output {
                    self.output.push(output);
                } else {
                    eprintln!("{output}");
                }
                Ok(Value::Null)
            }
            // ── Math builtins ──
            "abs" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Int(n) => Ok(Value::Int(n.abs())),
                    Value::Float(f) => Ok(Value::Float(f.abs())),
                    _ => Err(RuntimeError::TypeError(format!(
                        "abs() not supported for {}",
                        args[0].type_name()
                    ))
                    .into()),
                }
            }
            "sqrt" => self.math_f64_unary(args, f64::sqrt),
            "log" => self.math_f64_unary(args, f64::ln),
            "log2" => self.math_f64_unary(args, f64::log2),
            "log10" => self.math_f64_unary(args, f64::log10),
            "sin" => self.math_f64_unary(args, f64::sin),
            "cos" => self.math_f64_unary(args, f64::cos),
            "tan" => self.math_f64_unary(args, f64::tan),
            "floor" => self.math_f64_unary(args, f64::floor),
            "ceil" => self.math_f64_unary(args, f64::ceil),
            "round" => self.math_f64_unary(args, f64::round),
            "pow" => self.math_f64_binary(args, f64::powf),
            "clamp" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Float(v), Value::Float(lo), Value::Float(hi)) => {
                        Ok(Value::Float(v.clamp(*lo, *hi)))
                    }
                    (Value::Int(v), Value::Int(lo), Value::Int(hi)) => {
                        Ok(Value::Int((*v).clamp(*lo, *hi)))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "clamp() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            "min" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
                    _ => Err(RuntimeError::TypeError(
                        "min() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            "max" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                    (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
                    _ => Err(RuntimeError::TypeError(
                        "max() requires matching numeric types".into(),
                    )
                    .into()),
                }
            }
            // ── Integer overflow control builtins ──
            "wrapping_add" => self.int_binop_builtin(args, "wrapping_add", i64::wrapping_add),
            "wrapping_sub" => self.int_binop_builtin(args, "wrapping_sub", i64::wrapping_sub),
            "wrapping_mul" => self.int_binop_builtin(args, "wrapping_mul", i64::wrapping_mul),
            "saturating_add" => self.int_binop_builtin(args, "saturating_add", i64::saturating_add),
            "saturating_sub" => self.int_binop_builtin(args, "saturating_sub", i64::saturating_sub),
            "saturating_mul" => self.int_binop_builtin(args, "saturating_mul", i64::saturating_mul),
            "checked_add" => self.checked_int_builtin(args, "checked_add", i64::checked_add),
            "checked_sub" => self.checked_int_builtin(args, "checked_sub", i64::checked_sub),
            "checked_mul" => self.checked_int_builtin(args, "checked_mul", i64::checked_mul),
            // ── OS runtime builtins ──
            "mem_alloc" => self.builtin_mem_alloc(args),
            "mem_free" => self.builtin_mem_free(args),
            "mem_read_u8" => self.builtin_mem_read_u8(args),
            "mem_read_u32" => self.builtin_mem_read_u32(args),
            "mem_read_u64" => self.builtin_mem_read_u64(args),
            "mem_write_u8" => self.builtin_mem_write_u8(args),
            "mem_write_u32" => self.builtin_mem_write_u32(args),
            "mem_write_u64" => self.builtin_mem_write_u64(args),
            "page_map" => self.builtin_page_map(args),
            "page_unmap" => self.builtin_page_unmap(args),
            "irq_register" => self.builtin_irq_register(args),
            "irq_unregister" => self.builtin_irq_unregister(args),
            "irq_enable" => self.builtin_irq_enable(args),
            "irq_disable" => self.builtin_irq_disable(args),
            "port_read" => self.builtin_port_read(args),
            "port_write" => self.builtin_port_write(args),
            "syscall_define" => self.builtin_syscall_define(args),
            "syscall_dispatch" => self.builtin_syscall_dispatch(args),
            // ML runtime builtins
            "tensor_zeros" | "zeros" => self.builtin_tensor_zeros(args),
            "tensor_ones" | "ones" => self.builtin_tensor_ones(args),
            "tensor_randn" | "tensor_rand" | "randn" => self.builtin_tensor_randn(args),
            "tensor_eye" => self.builtin_tensor_eye(args),
            "tensor_full" => self.builtin_tensor_full(args),
            "tensor_from_data" => self.builtin_tensor_from_data(args),
            "tensor_shape" => self.builtin_tensor_shape(args),
            "tensor_reshape" => self.builtin_tensor_reshape(args),
            "tensor_numel" => self.builtin_tensor_numel(args),
            "tensor_add" => self.builtin_tensor_binop(args, "add"),
            "tensor_sub" => self.builtin_tensor_binop(args, "sub"),
            "tensor_mul" => self.builtin_tensor_binop(args, "mul"),
            "tensor_div" => self.builtin_tensor_binop(args, "div"),
            "tensor_neg" => self.builtin_tensor_neg(args),
            "tensor_matmul" => self.builtin_tensor_matmul(args),
            "tensor_transpose" => self.builtin_tensor_transpose(args),
            "tensor_flatten" => self.builtin_tensor_unary(args, "flatten"),
            "tensor_squeeze" => self.builtin_tensor_squeeze(args),
            "tensor_unsqueeze" => self.builtin_tensor_unsqueeze(args),
            "tensor_sum" => self.builtin_tensor_reduce(args, "sum"),
            "tensor_mean" => self.builtin_tensor_reduce(args, "mean"),
            "tensor_max" => self.builtin_tensor_reduce(args, "max"),
            "tensor_min" => self.builtin_tensor_reduce(args, "min"),
            "tensor_argmax" => self.builtin_tensor_argmax(args),
            "tensor_arange" => self.builtin_tensor_arange(args),
            "tensor_linspace" => self.builtin_tensor_linspace(args),
            "tensor_xavier" => self.builtin_tensor_xavier(args),
            "tensor_free" => Ok(Value::Null), // no-op in interpreter (GC handles cleanup)
            "tensor_rows" => self.builtin_tensor_rows(args),
            "tensor_cols" => self.builtin_tensor_cols(args),
            "tensor_set" => self.builtin_tensor_set(args),
            "tensor_row" => self.builtin_tensor_row(args),
            "tensor_normalize" => self.builtin_tensor_normalize(args),
            "tensor_scale" => self.builtin_tensor_scale(args),
            // Activation functions
            "tensor_relu" => self.builtin_tensor_activation(args, "relu"),
            "tensor_sigmoid" => self.builtin_tensor_activation(args, "sigmoid"),
            "tensor_tanh" => self.builtin_tensor_activation(args, "tanh"),
            "tensor_softmax" => self.builtin_tensor_activation(args, "softmax"),
            "tensor_gelu" => self.builtin_tensor_activation(args, "gelu"),
            "tensor_leaky_relu" => self.builtin_tensor_leaky_relu(args),
            // Loss functions
            "tensor_mse_loss" => self.builtin_tensor_loss(args, "mse"),
            "tensor_cross_entropy" => self.builtin_tensor_loss(args, "cross_entropy"),
            "tensor_bce_loss" => self.builtin_tensor_loss(args, "bce"),
            "tensor_l1_loss" => self.builtin_tensor_loss(args, "l1"),
            // ── Autograd builtins ──
            "tensor_backward" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        if let Some(tid) = t.id() {
                            let grads = self.tape.backward(tid, t.shape()).map_err(|e| {
                                RuntimeError::TypeError(format!("backward failed: {e}"))
                            })?;
                            self.last_grads = grads;
                        } else {
                            // No tape id — store ones as gradient (seed)
                            let seed = ndarray::ArrayD::ones(t.shape());
                            // Use a placeholder id of 0
                            self.last_grads.clear();
                            self.last_grads.insert(0, seed);
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(
                        RuntimeError::TypeError("tensor_backward requires a tensor".into()).into(),
                    ),
                }
            }
            "tensor_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        // Check last_grads by tensor id
                        let found = t.id().and_then(|tid| self.last_grads.get(&tid).cloned());
                        if let Some(grad_data) = found {
                            Ok(Value::Tensor(TensorValue::from_ndarray(grad_data)))
                        } else if let Some(g) = t.grad() {
                            Ok(Value::Tensor(TensorValue::from_ndarray(g.clone())))
                        } else {
                            // Fallback: return any grad available
                            if let Some(g) = self.last_grads.values().next() {
                                Ok(Value::Tensor(TensorValue::from_ndarray(g.clone())))
                            } else {
                                Ok(Value::Null)
                            }
                        }
                    }
                    _ => {
                        Err(RuntimeError::TypeError("tensor_grad requires a tensor".into()).into())
                    }
                }
            }
            "tensor_requires_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Bool(t.requires_grad())),
                    _ => Err(RuntimeError::TypeError(
                        "tensor_requires_grad requires a tensor".into(),
                    )
                    .into()),
                }
            }
            "tensor_set_requires_grad" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(t), Value::Bool(b)) => {
                        let mut t2 = t.clone();
                        t2.set_requires_grad(*b);
                        if *b && t2.id().is_none() {
                            let id = self.tape.fresh_id();
                            t2.set_id(id);
                        }
                        Ok(Value::Tensor(t2))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "tensor_set_requires_grad(tensor, bool)".into(),
                    )
                    .into()),
                }
            }
            "tensor_detach" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Tensor(t.detach())),
                    _ => Err(
                        RuntimeError::TypeError("tensor_detach requires a tensor".into()).into(),
                    ),
                }
            }
            "tensor_no_grad_begin" => {
                self.tape.set_recording(false);
                Ok(Value::Null)
            }
            "tensor_no_grad_end" => {
                self.tape.set_recording(true);
                Ok(Value::Null)
            }
            "tensor_clear_tape" => {
                self.tape.clear();
                Ok(Value::Null)
            }
            // ── Optimizer builtins ──
            "optimizer_sgd" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let lr = match &args[0] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_sgd: lr must be a number".into(),
                        )
                        .into())
                    }
                };
                let momentum = match &args[1] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_sgd: momentum must be a number".into(),
                        )
                        .into())
                    }
                };
                Ok(Value::Optimizer(OptimizerValue::Sgd(
                    crate::runtime::ml::optim::SGD::new(lr, momentum),
                )))
            }
            "optimizer_adam" => {
                if args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: 0,
                    }
                    .into());
                }
                let lr = match &args[0] {
                    Value::Float(f) => *f,
                    Value::Int(n) => *n as f64,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_adam: lr must be a number".into(),
                        )
                        .into())
                    }
                };
                Ok(Value::Optimizer(OptimizerValue::Adam(
                    crate::runtime::ml::optim::Adam::new(lr),
                )))
            }
            "optimizer_step" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let mut opt = match args[0].clone() {
                    Value::Optimizer(o) => o,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_step: first arg must be an optimizer".into(),
                        )
                        .into())
                    }
                };
                let mut tensor = match args[1].clone() {
                    Value::Tensor(t) => t,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "optimizer_step: second arg must be a tensor".into(),
                        )
                        .into())
                    }
                };
                // Apply gradient stored from last backward
                if let Some(tid) = tensor.id() {
                    if let Some(grad_data) = self.last_grads.get(&tid) {
                        tensor.set_grad(grad_data.clone());
                    }
                }
                let mut params = vec![tensor];
                match &mut opt {
                    OptimizerValue::Sgd(sgd) => sgd.step(&mut params),
                    OptimizerValue::Adam(adam) => adam.step(&mut params),
                }
                Ok(Value::Tensor(params.into_iter().next().unwrap()))
            }
            "optimizer_zero_grad" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        let mut t2 = t.clone();
                        t2.zero_grad();
                        Ok(Value::Tensor(t2))
                    }
                    _ => Err(RuntimeError::TypeError(
                        "optimizer_zero_grad requires a tensor".into(),
                    )
                    .into()),
                }
            }
            // ── Layer builtins ──
            "layer_dense" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let in_f = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_dense: in_features must be int".into(),
                        )
                        .into())
                    }
                };
                let out_f = match &args[1] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_dense: out_features must be int".into(),
                        )
                        .into())
                    }
                };
                Ok(Value::Layer(Box::new(LayerValue::Dense(
                    crate::runtime::ml::layers::Dense::new(in_f, out_f),
                ))))
            }
            "layer_forward" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let layer = match &args[0] {
                    Value::Layer(l) => l,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_forward: first arg must be a layer".into(),
                        )
                        .into())
                    }
                };
                let input = match &args[1] {
                    Value::Tensor(t) => t,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "layer_forward: second arg must be a tensor".into(),
                        )
                        .into())
                    }
                };
                match layer.as_ref() {
                    LayerValue::Dense(dense) => {
                        let output = dense
                            .forward(input)
                            .map_err(|e| RuntimeError::TypeError(format!("forward failed: {e}")))?;
                        Ok(Value::Tensor(output))
                    }
                }
            }
            "layer_params" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Layer(layer) => match layer.as_ref() {
                        LayerValue::Dense(dense) => {
                            let params: Vec<Value> = dense
                                .parameters()
                                .into_iter()
                                .map(|p| Value::Tensor(p.clone()))
                                .collect();
                            Ok(Value::Array(params))
                        }
                    },
                    _ => {
                        Err(RuntimeError::TypeError("layer_params requires a layer".into()).into())
                    }
                }
            }
            // Metrics builtins
            "metric_accuracy" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_accuracy")?;
                let labels = self.extract_i64_array(&args[1], "metric_accuracy")?;
                Ok(Value::Float(crate::runtime::ml::metrics::accuracy(
                    &preds, &labels,
                )))
            }
            "metric_precision" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_precision")?;
                let labels = self.extract_i64_array(&args[1], "metric_precision")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::precision(
                    &preds, &labels, class,
                )))
            }
            "metric_recall" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_recall")?;
                let labels = self.extract_i64_array(&args[1], "metric_recall")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::recall(
                    &preds, &labels, class,
                )))
            }
            "metric_f1_score" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let preds = self.extract_i64_array(&args[0], "metric_f1_score")?;
                let labels = self.extract_i64_array(&args[1], "metric_f1_score")?;
                let class = match &args[2] {
                    Value::Int(n) => *n,
                    _ => return Err(RuntimeError::TypeError("class must be integer".into()).into()),
                };
                Ok(Value::Float(crate::runtime::ml::metrics::f1_score(
                    &preds, &labels, class,
                )))
            }
            // File I/O builtins
            "read_file" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => match std::fs::read_to_string(path) {
                        Ok(content) => Ok(Value::Enum {
                            variant: "Ok".into(),
                            data: Some(Box::new(Value::Str(content))),
                        }),
                        Err(e) => Ok(Value::Enum {
                            variant: "Err".into(),
                            data: Some(Box::new(Value::Str(e.to_string()))),
                        }),
                    },
                    _ => Err(
                        RuntimeError::TypeError("read_file() requires a string path".into()).into(),
                    ),
                }
            }
            "write_file" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(path), Value::Str(content)) => {
                        match std::fs::write(path, content) {
                            Ok(()) => Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Null)),
                            }),
                            Err(e) => Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(e.to_string()))),
                            }),
                        }
                    }
                    _ => Err(
                        RuntimeError::TypeError("write_file(path: str, content: str)".into())
                            .into(),
                    ),
                }
            }
            "append_file" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Str(path), Value::Str(content)) => {
                        use std::io::Write;
                        let result = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .and_then(|mut f| f.write_all(content.as_bytes()));
                        match result {
                            Ok(()) => Ok(Value::Enum {
                                variant: "Ok".into(),
                                data: Some(Box::new(Value::Null)),
                            }),
                            Err(e) => Ok(Value::Enum {
                                variant: "Err".into(),
                                data: Some(Box::new(Value::Str(e.to_string()))),
                            }),
                        }
                    }
                    _ => Err(RuntimeError::TypeError(
                        "append_file(path: str, content: str)".into(),
                    )
                    .into()),
                }
            }
            "file_exists" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Str(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
                    _ => Err(RuntimeError::TypeError(
                        "file_exists() requires a string path".into(),
                    )
                    .into()),
                }
            }
            // Collection builtins — HashMap
            "map_new" => {
                if !args.is_empty() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 0,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Map(HashMap::new()))
            }
            "map_insert" => {
                if args.len() != 3 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 3,
                        got: args.len(),
                    }
                    .into());
                }
                let mut args = args.into_iter();
                let map_val = args.next().unwrap();
                let key_val = args.next().unwrap();
                let value = args.next().unwrap();
                match (map_val, key_val) {
                    (Value::Map(mut m), Value::Str(k)) => {
                        m.insert(k, value);
                        Ok(Value::Map(m))
                    }
                    _ => Err(
                        RuntimeError::TypeError("map_insert(map, key: str, value)".into()).into(),
                    ),
                }
            }
            "map_get" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Map(m), Value::Str(k)) => match m.get(k) {
                        Some(v) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(v.clone())),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    },
                    _ => Err(RuntimeError::TypeError("map_get(map, key: str)".into()).into()),
                }
            }
            "map_remove" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                let mut args = args.into_iter();
                let map_val = args.next().unwrap();
                let key_val = args.next().unwrap();
                match (map_val, key_val) {
                    (Value::Map(mut m), Value::Str(k)) => {
                        m.remove(&k);
                        Ok(Value::Map(m))
                    }
                    _ => Err(RuntimeError::TypeError("map_remove(map, key: str)".into()).into()),
                }
            }
            "map_contains_key" => {
                if args.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: args.len(),
                    }
                    .into());
                }
                match (&args[0], &args[1]) {
                    (Value::Map(m), Value::Str(k)) => Ok(Value::Bool(m.contains_key(k))),
                    _ => Err(
                        RuntimeError::TypeError("map_contains_key(map, key: str)".into()).into(),
                    ),
                }
            }
            "map_keys" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => {
                        let keys: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                        Ok(Value::Array(keys))
                    }
                    _ => Err(RuntimeError::TypeError("map_keys(map)".into()).into()),
                }
            }
            "map_values" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => {
                        let vals: Vec<Value> = m.values().cloned().collect();
                        Ok(Value::Array(vals))
                    }
                    _ => Err(RuntimeError::TypeError("map_values(map)".into()).into()),
                }
            }
            "map_len" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                match &args[0] {
                    Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                    _ => Err(RuntimeError::TypeError("map_len(map)".into()).into()),
                }
            }
            // Option/Result constructors
            "Some" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Some".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            "Ok" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Ok".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            "Err" => {
                if args.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: args.len(),
                    }
                    .into());
                }
                Ok(Value::Enum {
                    variant: "Err".to_string(),
                    data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                })
            }
            // Hardware detection builtins (v1.1)
            "hw_cpu_vendor" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Str(cpu.vendor.to_string()))
            }
            "hw_cpu_arch" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Str(cpu.arch.clone()))
            }
            "hw_has_avx2" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.avx2))
            }
            "hw_has_avx512" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_avx512()))
            }
            "hw_has_amx" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_amx()))
            }
            "hw_has_neon" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.neon))
            }
            "hw_has_sve" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Bool(cpu.has_sve()))
            }
            "hw_simd_width" => {
                let cpu = crate::hw::CpuFeatures::cached();
                Ok(Value::Int(cpu.best_simd_width() as i64))
            }
            // Accelerator registry builtins (v1.1 S4)
            "hw_gpu_count" => {
                let profile = crate::hw::HardwareProfile::detect();
                Ok(Value::Int(profile.gpu.devices.len() as i64))
            }
            "hw_npu_count" => {
                let profile = crate::hw::HardwareProfile::detect();
                Ok(Value::Int(profile.npu.devices.len() as i64))
            }
            "hw_best_accelerator" => {
                let profile = crate::hw::HardwareProfile::detect();
                let best = profile.select_best(crate::hw::TaskType::General);
                Ok(Value::Str(best.to_string()))
            }
            // GPIO builtins (v2.0 Q6A)
            "gpio_open" => self.builtin_gpio_open(args),
            "gpio_close" => self.builtin_gpio_close(args),
            "gpio_set_direction" => self.builtin_gpio_set_direction(args),
            "gpio_write" => self.builtin_gpio_write(args),
            "gpio_read" => self.builtin_gpio_read(args),
            "gpio_toggle" => self.builtin_gpio_toggle(args),
            // UART builtins (v2.0 Q6A)
            "uart_open" => self.builtin_uart_open(args),
            "uart_close" => self.builtin_uart_close(args),
            "uart_write_byte" => self.builtin_uart_write_byte(args),
            "uart_read_byte" => self.builtin_uart_read_byte(args),
            "uart_write_str" => self.builtin_uart_write_str(args),
            // PWM builtins (v2.0 Q6A)
            "pwm_open" => self.builtin_pwm_open(args),
            "pwm_close" => self.builtin_pwm_close(args),
            "pwm_set_frequency" => self.builtin_pwm_set_frequency(args),
            "pwm_set_duty" => self.builtin_pwm_set_duty(args),
            "pwm_enable" => self.builtin_pwm_enable(args),
            "pwm_disable" => self.builtin_pwm_disable(args),
            // SPI builtins (v2.0 Q6A)
            "spi_open" => self.builtin_spi_open(args),
            "spi_close" => self.builtin_spi_close(args),
            "spi_transfer" => self.builtin_spi_transfer(args),
            "spi_write" => self.builtin_spi_write(args),
            // NPU builtins (v2.0 Q6A)
            "npu_available" => self.builtin_npu_available(args),
            "npu_info" => self.builtin_npu_info(args),
            "npu_load" => self.builtin_npu_load(args),
            "npu_infer" => self.builtin_npu_infer(args),
            // Timing builtins (v2.0)
            "delay_ms" => self.builtin_delay_ms(args),
            "delay_us" => self.builtin_delay_us(args),
            _ => {
                // Check for enum constructor builtins
                if name.starts_with("__enum_") {
                    if args.len() == 1 {
                        let variant_name = name.rsplit('_').next().unwrap_or(name);
                        return Ok(Value::Enum {
                            variant: variant_name.to_string(),
                            data: Some(Box::new(args.into_iter().next().unwrap_or(Value::Null))),
                        });
                    }
                    // Multiple args — wrap in tuple
                    let variant_name = name.rsplit('_').next().unwrap_or(name);
                    return Ok(Value::Enum {
                        variant: variant_name.to_string(),
                        data: Some(Box::new(Value::Tuple(args))),
                    });
                }
                Err(RuntimeError::Unsupported(format!("unknown builtin '{name}'")).into())
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // OS runtime builtins
    // ═══════════════════════════════════════════════════════════════════

    /// `mem_alloc(size, align)` → Pointer
    fn builtin_mem_alloc(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let size = match &args[0] {
            Value::Int(n) => *n as usize,
            _ => return Err(RuntimeError::TypeError("mem_alloc: size must be int".into()).into()),
        };
        let align = match &args[1] {
            Value::Int(n) => *n as usize,
            _ => return Err(RuntimeError::TypeError("mem_alloc: align must be int".into()).into()),
        };
        match self.os.memory.alloc(size, align) {
            Ok(addr) => Ok(Value::Pointer(addr.0)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_free(ptr)` → null
    fn builtin_mem_free(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let addr = match &args[0] {
            Value::Pointer(a) => crate::runtime::os::VirtAddr(*a),
            Value::Int(n) => crate::runtime::os::VirtAddr(*n as u64),
            _ => return Err(RuntimeError::TypeError("mem_free: expected pointer".into()).into()),
        };
        match self.os.memory.free(addr) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u8(ptr)` → Int
    fn builtin_mem_read_u8(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u8", &args, 1)?;
        match self.os.memory.read_u8(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u32(ptr)` → Int
    fn builtin_mem_read_u32(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u32", &args, 1)?;
        match self.os.memory.read_u32(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_read_u64(ptr)` → Int
    fn builtin_mem_read_u64(&self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("mem_read_u64", &args, 1)?;
        match self.os.memory.read_u64(addr) {
            Ok(v) => Ok(Value::Int(v as i64)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u8(ptr, value)` → null
    fn builtin_mem_write_u8(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u8", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u8: value must be int".into()).into(),
                )
            }
        };
        match self.os.memory.write_u8(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u32(ptr, value)` → null
    fn builtin_mem_write_u32(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u32", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u32,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u32: value must be int".into()).into(),
                )
            }
        };
        match self.os.memory.write_u32(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `mem_write_u64(ptr, value)` → null
    fn builtin_mem_write_u64(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let addr = self.val_to_addr("mem_write_u64", &args[0])?;
        let val = match &args[1] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("mem_write_u64: value must be int".into()).into(),
                )
            }
        };
        match self.os.memory.write_u64(addr, val) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `page_map(virt_addr, phys_addr, flags)` → null
    fn builtin_page_map(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let va = self.val_to_addr("page_map", &args[0])?;
        let pa = match &args[1] {
            Value::Pointer(a) => crate::runtime::os::PhysAddr(*a),
            Value::Int(a) => crate::runtime::os::PhysAddr(*a as u64),
            _ => {
                return Err(RuntimeError::TypeError(
                    "page_map: phys_addr must be int/pointer".into(),
                )
                .into())
            }
        };
        let flags_val = match &args[2] {
            Value::Int(n) => *n as u8,
            _ => return Err(RuntimeError::TypeError("page_map: flags must be int".into()).into()),
        };
        let flags = crate::runtime::os::PageFlags::from_bits(flags_val);
        match self.os.memory.page_table.map_page(va, pa, flags) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `page_unmap(virt_addr)` → null
    fn builtin_page_unmap(&mut self, args: Vec<Value>) -> EvalResult {
        let addr = self.extract_addr("page_unmap", &args, 1)?;
        match self.os.memory.page_table.unmap_page(addr) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_register(num, handler_name)` → null
    fn builtin_irq_register(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let irq_num = match &args[0] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_register: irq num must be int".into()).into(),
                )
            }
        };
        let handler = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_register: handler must be string".into()).into(),
                )
            }
        };
        match self.os.irq.register(irq_num, handler) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_unregister(num)` → null
    fn builtin_irq_unregister(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let irq_num = match &args[0] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("irq_unregister: irq num must be int".into()).into(),
                )
            }
        };
        match self.os.irq.unregister(irq_num) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `irq_enable()` → null
    fn builtin_irq_enable(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        self.os.irq.enable();
        Ok(Value::Null)
    }

    /// `irq_disable()` → null
    fn builtin_irq_disable(&mut self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        self.os.irq.disable();
        Ok(Value::Null)
    }

    /// `port_read(port)` → Int
    fn builtin_port_read(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n as u16,
            _ => return Err(RuntimeError::TypeError("port_read: port must be int".into()).into()),
        };
        Ok(Value::Int(self.os.port_io.read(port) as i64))
    }

    /// `port_write(port, value)` → null
    fn builtin_port_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n as u16,
            _ => return Err(RuntimeError::TypeError("port_write: port must be int".into()).into()),
        };
        let value = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(RuntimeError::TypeError("port_write: value must be int".into()).into())
            }
        };
        self.os.port_io.write(port, value);
        Ok(Value::Null)
    }

    /// `syscall_define(num, handler_name, arg_count)` → null
    fn builtin_syscall_define(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let num = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("syscall_define: num must be int".into()).into(),
                )
            }
        };
        let handler_name = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "syscall_define: handler must be string".into(),
                )
                .into())
            }
        };
        let arg_count = match &args[2] {
            Value::Int(n) => *n as usize,
            _ => {
                return Err(
                    RuntimeError::TypeError("syscall_define: arg_count must be int".into()).into(),
                )
            }
        };
        match self.os.syscall.define(num, handler_name, arg_count) {
            Ok(()) => Ok(Value::Null),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `syscall_dispatch(num, ...args)` → handler name (string) for the interpreter to resolve
    fn builtin_syscall_dispatch(&mut self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: 0,
            }
            .into());
        }
        let num = match &args[0] {
            Value::Int(n) => *n as u64,
            _ => {
                return Err(
                    RuntimeError::TypeError("syscall_dispatch: num must be int".into()).into(),
                )
            }
        };
        let syscall_args = args.len() - 1;
        match self.os.syscall.dispatch(num, syscall_args) {
            Ok(handler) => Ok(Value::Str(handler.name.clone())),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    // ── GPIO builtins (v2.0 Q6A) ──

    /// `gpio_open(pin: i64) -> i64` — Open a GPIO pin; returns pin handle (the pin number).
    /// On x86_64 host, operates in simulation mode.
    fn builtin_gpio_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_open: pin must be int".into()).into()),
        };
        // Store pin state in gpio_pins map: pin -> (direction: 0=in/1=out, level: 0/1)
        self.gpio_pins.insert(pin, (0, 0));
        Ok(Value::Int(pin))
    }

    /// `gpio_close(pin: i64) -> null` — Close/release a GPIO pin.
    fn builtin_gpio_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_close: pin must be int".into()).into()),
        };
        self.gpio_pins.remove(&pin);
        Ok(Value::Null)
    }

    /// `gpio_set_direction(pin: i64, dir: str) -> null` — Set pin direction ("in" or "out").
    fn builtin_gpio_set_direction(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("gpio_set_direction: pin must be int".into()).into(),
                )
            }
        };
        let dir = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "gpio_set_direction: direction must be string (\"in\" or \"out\")".into(),
                )
                .into())
            }
        };
        let dir_val = match dir.as_str() {
            "in" | "input" => 0,
            "out" | "output" => 1,
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_set_direction: invalid direction '{}' (use \"in\" or \"out\")",
                    dir
                ))
                .into())
            }
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            state.0 = dir_val;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "gpio_set_direction: pin {} not opened",
                pin
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `gpio_write(pin: i64, level: i64) -> null` — Write 0 (low) or 1 (high) to an output pin.
    fn builtin_gpio_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_write: pin must be int".into()).into()),
        };
        let level = match &args[1] {
            Value::Int(n) => {
                if *n != 0 {
                    1
                } else {
                    0
                }
            }
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => {
                return Err(
                    RuntimeError::TypeError("gpio_write: level must be int or bool".into()).into(),
                )
            }
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            if state.0 != 1 {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_write: pin {} is not set to output",
                    pin
                ))
                .into());
            }
            state.1 = level;
        } else {
            return Err(
                RuntimeError::TypeError(format!("gpio_write: pin {} not opened", pin)).into(),
            );
        }
        Ok(Value::Null)
    }

    /// `gpio_read(pin: i64) -> i64` — Read current pin level (0 or 1).
    fn builtin_gpio_read(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_read: pin must be int".into()).into()),
        };
        if let Some(state) = self.gpio_pins.get(&pin) {
            Ok(Value::Int(state.1))
        } else {
            Err(RuntimeError::TypeError(format!("gpio_read: pin {} not opened", pin)).into())
        }
    }

    /// `gpio_toggle(pin: i64) -> null` — Toggle output pin level (0→1 or 1→0).
    fn builtin_gpio_toggle(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let pin = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("gpio_toggle: pin must be int".into()).into()),
        };
        if let Some(state) = self.gpio_pins.get_mut(&pin) {
            if state.0 != 1 {
                return Err(RuntimeError::TypeError(format!(
                    "gpio_toggle: pin {} is not set to output",
                    pin
                ))
                .into());
            }
            state.1 = if state.1 == 0 { 1 } else { 0 };
        } else {
            return Err(
                RuntimeError::TypeError(format!("gpio_toggle: pin {} not opened", pin)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── UART builtins (v2.0 Q6A) ──

    /// `uart_open(port: i64, baud: i64) -> i64` — Open UART port; returns port handle.
    fn builtin_uart_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_open: port must be int".into()).into()),
        };
        let baud = match &args[1] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_open: baud must be int".into()).into()),
        };
        // Store UART state: port -> (baud, tx_buffer)
        self.uart_ports.insert(port, (baud, Vec::new()));
        Ok(Value::Int(port))
    }

    /// `uart_close(port: i64) -> null` — Close UART port.
    fn builtin_uart_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("uart_close: port must be int".into()).into()),
        };
        self.uart_ports.remove(&port);
        Ok(Value::Null)
    }

    /// `uart_write_byte(port: i64, byte: i64) -> null` — Write a byte to UART.
    fn builtin_uart_write_byte(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_byte: port must be int".into()).into(),
                )
            }
        };
        let byte = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_byte: byte must be int".into()).into(),
                )
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            state.1.push(byte);
        } else {
            return Err(RuntimeError::TypeError(format!(
                "uart_write_byte: port {} not opened",
                port
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `uart_read_byte(port: i64) -> i64` — Read a byte from UART TX buffer (simulation: reads back written bytes).
    fn builtin_uart_read_byte(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_read_byte: port must be int".into()).into(),
                )
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            if state.1.is_empty() {
                Ok(Value::Int(-1)) // No data available
            } else {
                let byte = state.1.remove(0); // FIFO
                Ok(Value::Int(byte as i64))
            }
        } else {
            Err(RuntimeError::TypeError(format!("uart_read_byte: port {} not opened", port)).into())
        }
    }

    /// `uart_write_str(port: i64, s: str) -> null` — Write string bytes to UART.
    fn builtin_uart_write_str(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let port = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_str: port must be int".into()).into(),
                )
            }
        };
        let s = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(
                    RuntimeError::TypeError("uart_write_str: data must be string".into()).into(),
                )
            }
        };
        if let Some(state) = self.uart_ports.get_mut(&port) {
            state.1.extend_from_slice(s.as_bytes());
        } else {
            return Err(RuntimeError::TypeError(format!(
                "uart_write_str: port {} not opened",
                port
            ))
            .into());
        }
        Ok(Value::Null)
    }

    // ── Timing builtins (v2.0) ──

    /// `delay_ms(ms: i64) -> null` — Sleep for the given number of milliseconds.
    fn builtin_delay_ms(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ms = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("delay_ms: argument must be int".into()).into())
            }
        };
        if ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(ms as u64));
        }
        Ok(Value::Null)
    }

    /// `delay_us(us: i64) -> null` — Sleep for the given number of microseconds.
    fn builtin_delay_us(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let us = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("delay_us: argument must be int".into()).into())
            }
        };
        if us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(us as u64));
        }
        Ok(Value::Null)
    }

    // ── PWM builtins (v2.0 Q6A) ──

    /// `pwm_open(channel: i64) -> i64` — Open PWM channel; returns handle.
    fn builtin_pwm_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("pwm_open: channel must be int".into()).into())
            }
        };
        // Store PWM state: channel -> (frequency_hz, duty_percent, enabled)
        self.pwm_channels.insert(ch, (1000, 0, false));
        Ok(Value::Int(ch))
    }

    /// `pwm_close(channel: i64) -> null`
    fn builtin_pwm_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("pwm_close: channel must be int".into()).into())
            }
        };
        self.pwm_channels.remove(&ch);
        Ok(Value::Null)
    }

    /// `pwm_set_frequency(channel: i64, hz: i64) -> null`
    fn builtin_pwm_set_frequency(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError(
                    "pwm_set_frequency: channel must be int".into(),
                )
                .into())
            }
        };
        let hz = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_frequency: hz must be int".into()).into(),
                )
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.0 = hz;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_frequency: channel {} not opened",
                ch
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `pwm_set_duty(channel: i64, percent: i64) -> null` — Set duty cycle (0-100).
    fn builtin_pwm_set_duty(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_duty: channel must be int".into()).into(),
                )
            }
        };
        let duty = match &args[1] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_set_duty: percent must be int".into()).into(),
                )
            }
        };
        if !(0..=100).contains(&duty) {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_duty: percent must be 0-100, got {}",
                duty
            ))
            .into());
        }
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.1 = duty;
        } else {
            return Err(RuntimeError::TypeError(format!(
                "pwm_set_duty: channel {} not opened",
                ch
            ))
            .into());
        }
        Ok(Value::Null)
    }

    /// `pwm_enable(channel: i64) -> null`
    fn builtin_pwm_enable(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_enable: channel must be int".into()).into(),
                )
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.2 = true;
        } else {
            return Err(
                RuntimeError::TypeError(format!("pwm_enable: channel {} not opened", ch)).into(),
            );
        }
        Ok(Value::Null)
    }

    /// `pwm_disable(channel: i64) -> null`
    fn builtin_pwm_disable(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let ch = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("pwm_disable: channel must be int".into()).into(),
                )
            }
        };
        if let Some(state) = self.pwm_channels.get_mut(&ch) {
            state.2 = false;
        } else {
            return Err(
                RuntimeError::TypeError(format!("pwm_disable: channel {} not opened", ch)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── SPI builtins (v2.0 Q6A) ──

    /// `spi_open(bus: i64, speed_hz: i64) -> i64` — Open SPI bus; returns handle.
    fn builtin_spi_open(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_open: bus must be int".into()).into()),
        };
        let speed = match &args[1] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_open: speed must be int".into()).into()),
        };
        // Store SPI state: bus -> (speed_hz, rx_buffer)
        self.spi_buses.insert(bus, (speed, Vec::new()));
        Ok(Value::Int(bus))
    }

    /// `spi_close(bus: i64) -> null`
    fn builtin_spi_close(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_close: bus must be int".into()).into()),
        };
        self.spi_buses.remove(&bus);
        Ok(Value::Null)
    }

    /// `spi_transfer(bus: i64, byte: i64) -> i64` — Full-duplex: send byte, receive byte.
    fn builtin_spi_transfer(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(RuntimeError::TypeError("spi_transfer: bus must be int".into()).into())
            }
        };
        let byte = match &args[1] {
            Value::Int(n) => *n as u8,
            _ => {
                return Err(RuntimeError::TypeError("spi_transfer: byte must be int".into()).into())
            }
        };
        if let Some(state) = self.spi_buses.get_mut(&bus) {
            // Simulation: loopback — received byte = sent byte (MOSI→MISO)
            state.1.push(byte);
            Ok(Value::Int(byte as i64))
        } else {
            Err(RuntimeError::TypeError(format!("spi_transfer: bus {} not opened", bus)).into())
        }
    }

    /// `spi_write(bus: i64, data: str) -> null` — Write string bytes to SPI.
    fn builtin_spi_write(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let bus = match &args[0] {
            Value::Int(n) => *n,
            _ => return Err(RuntimeError::TypeError("spi_write: bus must be int".into()).into()),
        };
        let data = match &args[1] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("spi_write: data must be string".into()).into())
            }
        };
        if let Some(state) = self.spi_buses.get_mut(&bus) {
            state.1.extend_from_slice(data.as_bytes());
        } else {
            return Err(
                RuntimeError::TypeError(format!("spi_write: bus {} not opened", bus)).into(),
            );
        }
        Ok(Value::Null)
    }

    // ── NPU builtins (v2.0 Q6A) ──

    /// `npu_available() -> bool` — Check if NPU (Hexagon 770) is available.
    fn builtin_npu_available(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        // Check for FastRPC device node (real hardware detection)
        let available = std::path::Path::new("/dev/fastrpc-cdsp").exists();
        Ok(Value::Bool(available))
    }

    /// `npu_info() -> str` — Return NPU info string.
    fn builtin_npu_info(&self, args: Vec<Value>) -> EvalResult {
        if !args.is_empty() {
            return Err(RuntimeError::ArityMismatch {
                expected: 0,
                got: args.len(),
            }
            .into());
        }
        let available = std::path::Path::new("/dev/fastrpc-cdsp").exists();
        if available {
            Ok(Value::Str("Hexagon 770 V68, 12 TOPS INT8, QNN SDK".into()))
        } else {
            Ok(Value::Str("NPU not available (simulation mode)".into()))
        }
    }

    /// `npu_load(path: str) -> i64` — Load NPU model; returns model handle.
    fn builtin_npu_load(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let path = match &args[0] {
            Value::Str(s) => s.clone(),
            _ => {
                return Err(RuntimeError::TypeError("npu_load: path must be string".into()).into())
            }
        };
        // Simulation: assign incrementing model ID
        let model_id = self.npu_models.len() as i64 + 1;
        self.npu_models.insert(model_id, path);
        Ok(Value::Int(model_id))
    }

    /// `npu_infer(model: i64, input_data: i64) -> i64` — Run inference; returns result class index.
    fn builtin_npu_infer(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let model_id = match &args[0] {
            Value::Int(n) => *n,
            _ => {
                return Err(
                    RuntimeError::TypeError("npu_infer: model must be int handle".into()).into(),
                )
            }
        };
        let _input = match &args[1] {
            Value::Int(n) => *n,
            v => {
                return Err(RuntimeError::TypeError(format!(
                    "npu_infer: input must be int, got {:?}",
                    v
                ))
                .into())
            }
        };
        if !self.npu_models.contains_key(&model_id) {
            return Err(RuntimeError::TypeError(format!(
                "npu_infer: model {} not loaded",
                model_id
            ))
            .into());
        }
        // Simulation: return class 0 (placeholder for real QNN inference)
        Ok(Value::Int(0))
    }

    // ── ML runtime builtins ──

    /// Helper: extract a shape (Vec<usize>) from a Value::Array of ints.
    fn extract_shape(args: &[Value], idx: usize) -> Result<Vec<usize>, EvalError> {
        match &args[idx] {
            Value::Array(arr) => {
                let mut shape = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Int(n) if *n >= 0 => shape.push(*n as usize),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "shape elements must be non-negative integers".into(),
                            )
                            .into())
                        }
                    }
                }
                Ok(shape)
            }
            _ => Err(RuntimeError::TypeError("expected array for shape".into()).into()),
        }
    }

    /// Helper: resolve tensor shape from args.
    /// Accepts either `(rows, cols)` as two ints or `([dim1, dim2, ...])` as one array.
    fn resolve_tensor_shape(&self, args: Vec<Value>) -> Result<Vec<usize>, EvalError> {
        if args.len() == 1 {
            Self::extract_shape(&args, 0)
        } else if args.len() >= 2 && args.iter().all(|a| matches!(a, Value::Int(_))) {
            let mut shape = Vec::with_capacity(args.len());
            for a in &args {
                if let Value::Int(n) = a {
                    if *n >= 0 {
                        shape.push(*n as usize);
                    } else {
                        return Err(RuntimeError::TypeError(
                            "shape dimensions must be non-negative".into(),
                        )
                        .into());
                    }
                }
            }
            Ok(shape)
        } else {
            Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into())
        }
    }

    /// `tensor_zeros(rows, cols)` or `tensor_zeros([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_zeros(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::zeros(&shape)))
    }

    /// `tensor_ones(rows, cols)` or `tensor_ones([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_ones(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::ones(&shape)))
    }

    /// `tensor_randn(rows, cols)` or `tensor_randn([dim1, dim2, ...])` → Tensor
    fn builtin_tensor_randn(&self, args: Vec<Value>) -> EvalResult {
        let shape = self.resolve_tensor_shape(args)?;
        Ok(Value::Tensor(TensorValue::randn(&shape)))
    }

    /// `tensor_eye(n)` → Tensor (identity matrix)
    fn builtin_tensor_eye(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let n = match &args[0] {
            Value::Int(n) if *n > 0 => *n as usize,
            _ => return Err(RuntimeError::TypeError("eye: n must be positive int".into()).into()),
        };
        Ok(Value::Tensor(TensorValue::eye(n)))
    }

    /// `tensor_full([dim1, dim2, ...], value)` → Tensor
    fn builtin_tensor_full(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let shape = Self::extract_shape(&args, 0)?;
        let val = match &args[1] {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => return Err(RuntimeError::TypeError("full: value must be numeric".into()).into()),
        };
        Ok(Value::Tensor(TensorValue::full(&shape, val)))
    }

    /// `tensor_from_data([d1, d2, ...], [dim1, dim2, ...])` → Tensor
    fn builtin_tensor_from_data(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let data = match &args[0] {
            Value::Array(arr) => {
                let mut data = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Float(f) => data.push(*f),
                        Value::Int(i) => data.push(*i as f64),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "tensor data must be numeric".into(),
                            )
                            .into())
                        }
                    }
                }
                data
            }
            _ => return Err(RuntimeError::TypeError("expected array for data".into()).into()),
        };
        let shape = Self::extract_shape(&args, 1)?;
        match TensorValue::from_data(data, &shape) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `tensor_shape(tensor)` → Array of ints
    fn builtin_tensor_shape(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Array(
                t.shape().iter().map(|&d| Value::Int(d as i64)).collect(),
            )),
            _ => Err(RuntimeError::TypeError("tensor_shape: expected tensor".into()).into()),
        }
    }

    /// `tensor_reshape(tensor, [new_shape])` → Tensor
    fn builtin_tensor_reshape(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let tensor = match &args[0] {
            Value::Tensor(t) => t,
            _ => {
                return Err(
                    RuntimeError::TypeError("tensor_reshape: expected tensor".into()).into(),
                )
            }
        };
        let new_shape = Self::extract_shape(&args, 1)?;
        let new_numel: usize = new_shape.iter().product();
        if new_numel != tensor.numel() {
            return Err(RuntimeError::TypeError(format!(
                "cannot reshape {:?} ({} elements) to {:?} ({} elements)",
                tensor.shape(),
                tensor.numel(),
                new_shape,
                new_numel
            ))
            .into());
        }
        let data = tensor.to_vec();
        match TensorValue::from_data(data, &new_shape) {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// `tensor_numel(tensor)` → Int
    fn builtin_tensor_numel(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Int(t.numel() as i64)),
            _ => Err(RuntimeError::TypeError("tensor_numel: expected tensor".into()).into()),
        }
    }

    /// Binary tensor operation: add/sub/mul/div.
    fn builtin_tensor_binop(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "tensor_{op}: first arg must be tensor"
                ))
                .into())
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "tensor_{op}: second arg must be tensor"
                ))
                .into())
            }
        };
        // Use tracked ops when either input requires grad and tape is recording
        let use_tracked = (a.requires_grad() || b.requires_grad()) && self.tape.is_recording();
        let result = if use_tracked {
            match op {
                "add" => tensor_ops::add_tracked(&a, &b, &mut self.tape),
                "sub" => tensor_ops::sub_tracked(&a, &b, &mut self.tape),
                "mul" => tensor_ops::mul_tracked(&a, &b, &mut self.tape),
                "div" => tensor_ops::div_tracked(&a, &b, &mut self.tape),
                _ => unreachable!(),
            }
        } else {
            match op {
                "add" => tensor_ops::add(&a, &b),
                "sub" => tensor_ops::sub(&a, &b),
                "mul" => tensor_ops::mul(&a, &b),
                "div" => tensor_ops::div(&a, &b),
                _ => unreachable!(),
            }
        };
        match result {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// Unary tensor negation.
    fn builtin_tensor_neg(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::neg(t))),
            _ => Err(RuntimeError::TypeError("tensor_neg: expected tensor".into()).into()),
        }
    }

    /// Matrix multiplication.
    fn builtin_tensor_matmul(&mut self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "tensor_matmul: first arg must be tensor".into(),
                )
                .into())
            }
        };
        let b = match &args[1] {
            Value::Tensor(t) => t.clone(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "tensor_matmul: second arg must be tensor".into(),
                )
                .into())
            }
        };
        let use_tracked = (a.requires_grad() || b.requires_grad()) && self.tape.is_recording();
        let result = if use_tracked {
            tensor_ops::matmul_tracked(&a, &b, &mut self.tape)
        } else {
            tensor_ops::matmul(&a, &b)
        };
        match result {
            Ok(t) => Ok(Value::Tensor(t)),
            Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
        }
    }

    /// Tensor transpose.
    fn builtin_tensor_transpose(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => match tensor_ops::transpose(t) {
                Ok(r) => Ok(Value::Tensor(r)),
                Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
            },
            _ => Err(RuntimeError::TypeError("tensor_transpose: expected tensor".into()).into()),
        }
    }

    /// Reduction operation: sum/mean.
    fn builtin_tensor_reduce(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let use_tracked = t.requires_grad() && self.tape.is_recording();
                let result = match op {
                    "sum" if use_tracked => tensor_ops::sum_tracked(t, &mut self.tape),
                    "sum" => tensor_ops::sum(t),
                    "mean" => tensor_ops::mean(t),
                    "max" => tensor_ops::max(t),
                    "min" => tensor_ops::min(t),
                    "argmax" => tensor_ops::argmax(t),
                    _ => unreachable!(),
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// Unary tensor operation (flatten, etc.).
    fn builtin_tensor_unary(&self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let result = match op {
                    "flatten" => tensor_ops::flatten(t),
                    _ => unreachable!(),
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// `tensor_squeeze(t, axis)` — remove dimension of size 1.
    fn builtin_tensor_squeeze(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(axis)) => tensor_ops::squeeze(t, *axis as usize)
                .map(Value::Tensor)
                .map_err(|e| RuntimeError::TypeError(e.to_string()).into()),
            _ => {
                Err(RuntimeError::TypeError("tensor_squeeze: expected (tensor, int)".into()).into())
            }
        }
    }

    /// `tensor_unsqueeze(t, axis)` — insert dimension of size 1.
    fn builtin_tensor_unsqueeze(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(axis)) => tensor_ops::unsqueeze(t, *axis as usize)
                .map(Value::Tensor)
                .map_err(|e| RuntimeError::TypeError(e.to_string()).into()),
            _ => Err(
                RuntimeError::TypeError("tensor_unsqueeze: expected (tensor, int)".into()).into(),
            ),
        }
    }

    /// `tensor_arange(start, end, step)` — range tensor.
    fn builtin_tensor_arange(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let to_f64 = |v: &Value, name: &str| -> Result<f64, EvalError> {
            match v {
                Value::Float(f) => Ok(*f),
                Value::Int(i) => Ok(*i as f64),
                _ => Err(
                    RuntimeError::TypeError(format!("tensor_arange: {name} must be number")).into(),
                ),
            }
        };
        let start = to_f64(&args[0], "start")?;
        let end = to_f64(&args[1], "end")?;
        let step = to_f64(&args[2], "step")?;
        tensor_ops::arange(start, end, step)
            .map(Value::Tensor)
            .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
    }

    /// `tensor_linspace(start, end, steps)` — evenly spaced tensor.
    fn builtin_tensor_linspace(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 3 {
            return Err(RuntimeError::ArityMismatch {
                expected: 3,
                got: args.len(),
            }
            .into());
        }
        let to_f64 = |v: &Value, name: &str| -> Result<f64, EvalError> {
            match v {
                Value::Float(f) => Ok(*f),
                Value::Int(i) => Ok(*i as f64),
                _ => Err(RuntimeError::TypeError(format!(
                    "tensor_linspace: {name} must be number"
                ))
                .into()),
            }
        };
        let start = to_f64(&args[0], "start")?;
        let end = to_f64(&args[1], "end")?;
        let steps = match &args[2] {
            Value::Int(i) => *i as usize,
            _ => {
                return Err(
                    RuntimeError::TypeError("tensor_linspace: steps must be int".into()).into(),
                )
            }
        };
        tensor_ops::linspace(start, end, steps)
            .map(Value::Tensor)
            .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
    }

    /// `tensor_xavier(rows, cols)` — Xavier initialization.
    fn builtin_tensor_xavier(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(rows), Value::Int(cols)) => Ok(Value::Tensor(tensor_ops::xavier(
                *rows as usize,
                *cols as usize,
            ))),
            _ => Err(RuntimeError::TypeError("tensor_xavier: expected (int, int)".into()).into()),
        }
    }

    /// `tensor_argmax(tensor)` — Returns the index of the maximum element as an integer.
    fn builtin_tensor_argmax(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let result = tensor_ops::argmax(t);
                // Convert scalar tensor to integer
                let idx = result.to_scalar().unwrap_or(0.0) as i64;
                Ok(Value::Int(idx))
            }
            _ => Err(RuntimeError::TypeError("tensor_argmax: expected tensor".into()).into()),
        }
    }

    /// `tensor_rows(tensor)` — Returns the number of rows.
    fn builtin_tensor_rows(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let shape = t.shape();
                let rows = if shape.is_empty() { 0 } else { shape[0] as i64 };
                Ok(Value::Int(rows))
            }
            _ => Err(RuntimeError::TypeError("tensor_rows: expected tensor".into()).into()),
        }
    }

    /// `tensor_cols(tensor)` — Returns the number of columns.
    fn builtin_tensor_cols(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let shape = t.shape();
                let cols = if shape.len() >= 2 {
                    shape[1] as i64
                } else if shape.len() == 1 {
                    shape[0] as i64
                } else {
                    0
                };
                Ok(Value::Int(cols))
            }
            _ => Err(RuntimeError::TypeError("tensor_cols: expected tensor".into()).into()),
        }
    }

    /// `tensor_set(tensor, row, col, value_bits)` — Set a tensor element (value as f64 bits).
    fn builtin_tensor_set(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 4 {
            return Err(RuntimeError::ArityMismatch {
                expected: 4,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1], &args[2], &args[3]) {
            (Value::Tensor(t), Value::Int(row), Value::Int(col), Value::Int(val_bits)) => {
                let value = f64::from_bits(*val_bits as u64);
                let mut new_data = t.data().to_owned();
                let r = *row as usize;
                let c = *col as usize;
                if let Some(elem) = new_data.get_mut([r, c]) {
                    *elem = value;
                }
                // tensor_set is a mutation, but in interpreter we return Null
                // (the original tensor is immutable; this is a semantic no-op
                // unless we clone — native codegen mutates in place)
                Ok(Value::Null)
            }
            _ => Err(RuntimeError::TypeError(
                "tensor_set: expected (tensor, int, int, int)".into(),
            )
            .into()),
        }
    }

    /// `tensor_row(tensor, index)` — Extract a single row as a new tensor.
    fn builtin_tensor_row(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(row_idx)) => {
                let shape = t.shape();
                if shape.len() != 2 {
                    return Err(
                        RuntimeError::TypeError("tensor_row: expected 2D tensor".into()).into(),
                    );
                }
                let cols = shape[1];
                let row = *row_idx as usize;
                if row >= shape[0] {
                    return Err(RuntimeError::TypeError(
                        "tensor_row: row index out of bounds".into(),
                    )
                    .into());
                }
                let row_data: Vec<f64> = (0..cols)
                    .map(|c| *t.data().get([row, c]).unwrap_or(&0.0))
                    .collect();
                match TensorValue::from_data(row_data, &[1, cols]) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_row: expected (tensor, int)".into()).into()),
        }
    }

    /// `tensor_normalize(tensor)` — Normalize tensor values to [0, 1] range.
    fn builtin_tensor_normalize(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let nd = t.data();
                let min_val = nd.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_val = nd.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let range = max_val - min_val;
                let normalized: Vec<f64> = if range == 0.0 {
                    vec![0.0; nd.len()]
                } else {
                    nd.iter().map(|&v| (v - min_val) / range).collect()
                };
                let shape = t.shape().to_vec();
                match TensorValue::from_data(normalized, &shape) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_normalize: expected tensor".into()).into()),
        }
    }

    /// `tensor_scale(tensor, scalar_bits)` — Scale tensor by a scalar (f64 bits as i64).
    fn builtin_tensor_scale(&self, args: Vec<Value>) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(t), Value::Int(scalar_bits)) => {
                let scalar = f64::from_bits(*scalar_bits as u64);
                let nd = t.data();
                let scaled: Vec<f64> = nd.iter().map(|&v| v * scalar).collect();
                let shape = t.shape().to_vec();
                match TensorValue::from_data(scaled, &shape) {
                    Ok(tv) => Ok(Value::Tensor(tv)),
                    Err(e) => Err(RuntimeError::TypeError(e.to_string()).into()),
                }
            }
            _ => Err(RuntimeError::TypeError("tensor_scale: expected (tensor, int)".into()).into()),
        }
    }

    /// Unary activation function: relu/sigmoid/tanh/softmax/gelu.
    fn builtin_tensor_activation(&mut self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        match &args[0] {
            Value::Tensor(t) => {
                let use_tracked = t.requires_grad() && self.tape.is_recording();
                let result = if use_tracked {
                    match op {
                        "relu" => tensor_ops::relu_tracked(t, &mut self.tape),
                        "sigmoid" => tensor_ops::sigmoid_tracked(t, &mut self.tape),
                        "tanh" => tensor_ops::tanh_tracked(t, &mut self.tape),
                        "softmax" => tensor_ops::softmax(t), // no tracked version yet
                        "gelu" => tensor_ops::gelu(t),       // no tracked version yet
                        _ => unreachable!(),
                    }
                } else {
                    match op {
                        "relu" => tensor_ops::relu(t),
                        "sigmoid" => tensor_ops::sigmoid(t),
                        "tanh" => tensor_ops::tanh_act(t),
                        "softmax" => tensor_ops::softmax(t),
                        "gelu" => tensor_ops::gelu(t),
                        _ => unreachable!(),
                    }
                };
                Ok(Value::Tensor(result))
            }
            _ => Err(RuntimeError::TypeError(format!("tensor_{op}: expected tensor")).into()),
        }
    }

    /// Leaky ReLU with optional alpha parameter.
    fn builtin_tensor_leaky_relu(&self, args: Vec<Value>) -> EvalResult {
        if args.is_empty() || args.len() > 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let alpha = if args.len() == 2 {
            match &args[1] {
                Value::Float(f) => *f,
                Value::Int(i) => *i as f64,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "tensor_leaky_relu: alpha must be a number".into(),
                    )
                    .into())
                }
            }
        } else {
            0.01 // default alpha
        };
        match &args[0] {
            Value::Tensor(t) => Ok(Value::Tensor(tensor_ops::leaky_relu(t, alpha))),
            _ => Err(RuntimeError::TypeError("tensor_leaky_relu: expected tensor".into()).into()),
        }
    }

    /// Loss function: mse/cross_entropy/bce.
    fn builtin_tensor_loss(&self, args: Vec<Value>, op: &str) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Tensor(pred), Value::Tensor(target)) => {
                let result = match op {
                    "mse" => tensor_ops::mse_loss(pred, target),
                    "cross_entropy" => tensor_ops::cross_entropy(pred, target),
                    "bce" => tensor_ops::bce_loss(pred, target),
                    "l1" => tensor_ops::l1_loss(pred, target),
                    _ => unreachable!(),
                };
                result
                    .map(Value::Tensor)
                    .map_err(|e| RuntimeError::TypeError(e.to_string()).into())
            }
            _ => Err(
                RuntimeError::TypeError(format!("tensor_{op}_loss: expected two tensors")).into(),
            ),
        }
    }

    /// Helper: extract a VirtAddr from args (validates length and type).
    fn extract_addr(
        &self,
        fn_name: &str,
        args: &[Value],
        expected: usize,
    ) -> Result<crate::runtime::os::VirtAddr, EvalError> {
        if args.len() != expected {
            return Err(RuntimeError::ArityMismatch {
                expected,
                got: args.len(),
            }
            .into());
        }
        self.val_to_addr(fn_name, &args[0])
    }

    /// Helper: convert Value to VirtAddr.
    fn val_to_addr(
        &self,
        fn_name: &str,
        val: &Value,
    ) -> Result<crate::runtime::os::VirtAddr, EvalError> {
        match val {
            Value::Pointer(a) => Ok(crate::runtime::os::VirtAddr(*a)),
            Value::Int(n) => Ok(crate::runtime::os::VirtAddr(*n as u64)),
            _ => Err(RuntimeError::TypeError(format!("{fn_name}: expected pointer/int")).into()),
        }
    }

    /// Evaluates a block expression.
    fn eval_block(&mut self, stmts: &[Stmt], tail_expr: &Option<Box<Expr>>) -> EvalResult {
        // Create a new scope
        let block_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
            &self.env,
        ))));
        let prev_env = Rc::clone(&self.env);
        self.env = block_env;

        // Evaluate statements
        for stmt in stmts {
            self.eval_stmt(stmt)?;
        }

        // Evaluate tail expression (the block's value)
        let result = match tail_expr {
            Some(e) => self.eval_expr(e),
            None => Ok(Value::Null),
        };

        // Drop owned locals at scope exit (simulates destructors)
        self.env.borrow_mut().drop_locals();

        // Restore scope
        self.env = prev_env;
        result
    }

    /// Evaluates an if expression.
    fn eval_if(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Option<Box<Expr>>,
    ) -> EvalResult {
        let cond = self.eval_expr(condition)?;
        if cond.is_truthy() {
            self.eval_expr(then_branch)
        } else if let Some(else_e) = else_branch {
            self.eval_expr(else_e)
        } else {
            Ok(Value::Null)
        }
    }

    /// Evaluates a while loop.
    fn eval_while(&mut self, condition: &Expr, body: &Expr) -> EvalResult {
        loop {
            let cond = self.eval_expr(condition)?;
            if !cond.is_truthy() {
                break;
            }
            match self.eval_expr(body) {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v) => return Ok(v),
                    ControlFlow::Continue => continue,
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
        Ok(Value::Null)
    }

    /// Evaluates an infinite loop: `loop { body }`.
    fn eval_loop(&mut self, body: &Expr) -> EvalResult {
        loop {
            match self.eval_expr(body) {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v) => return Ok(v),
                    ControlFlow::Continue => continue,
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
    }

    /// Evaluates a for loop.
    fn eval_for(&mut self, variable: &str, iterable: &Expr, body: &Expr) -> EvalResult {
        let iter_val = self.eval_expr(iterable)?;

        // If iterable is already an Iterator, use iterator protocol
        if let Value::Iterator(iter_rc) = iter_val {
            return self.for_loop_iterator(variable, iter_rc, body);
        }

        // Convert value to iterator or eagerly collect
        let items: Vec<Value> = match iter_val {
            Value::Array(arr) => arr,
            Value::Tuple(t) => t,
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            Value::Map(m) => m
                .into_iter()
                .map(|(k, v)| Value::Tuple(vec![Value::Str(k), v]))
                .collect(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot iterate over {}",
                    iter_val.type_name()
                ))
                .into());
            }
        };

        for item in items {
            let loop_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                &self.env,
            ))));
            loop_env.borrow_mut().define(variable.to_string(), item);

            let prev_env = Rc::clone(&self.env);
            self.env = loop_env;

            let result = self.eval_expr(body);

            self.env = prev_env;

            match result {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v) => return Ok(v),
                    ControlFlow::Continue => continue,
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }

        Ok(Value::Null)
    }

    /// Runs a for loop using the iterator protocol (call next() until None).
    fn for_loop_iterator(
        &mut self,
        variable: &str,
        iter_rc: Rc<RefCell<IteratorValue>>,
        body: &Expr,
    ) -> EvalResult {
        loop {
            let item = self.iter_next(&iter_rc)?;
            let item = match item {
                Some(v) => v,
                None => break,
            };

            let loop_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                &self.env,
            ))));
            loop_env.borrow_mut().define(variable.to_string(), item);

            let prev_env = Rc::clone(&self.env);
            self.env = loop_env;

            let result = self.eval_expr(body);

            self.env = prev_env;

            match result {
                Ok(_) => {}
                Err(EvalError::Control(cf)) => match *cf {
                    ControlFlow::Break(v) => return Ok(v),
                    ControlFlow::Continue => continue,
                    cf => return Err(EvalError::Control(Box::new(cf))),
                },
                Err(e) => return Err(e),
            }
        }
        Ok(Value::Null)
    }

    /// Advances an iterator, handling combinators that need function calls.
    fn iter_next(
        &mut self,
        iter_rc: &Rc<RefCell<IteratorValue>>,
    ) -> Result<Option<Value>, EvalError> {
        let mut iter = iter_rc.borrow_mut();
        match &mut *iter {
            IteratorValue::MappedIter { inner, func } => {
                let inner_clone = inner.clone();
                let func_clone = func.clone();
                drop(iter); // Release borrow before calling function
                let inner_rc = Rc::new(RefCell::new(*inner_clone));
                let val = self.iter_next(&inner_rc)?;
                // Write back the advanced inner iterator
                let advanced = inner_rc.borrow().clone();
                let mut iter = iter_rc.borrow_mut();
                if let IteratorValue::MappedIter { inner, .. } = &mut *iter {
                    **inner = advanced;
                }
                match val {
                    Some(v) => {
                        drop(iter);
                        let result = self.call_function(&func_clone, vec![v])?;
                        Ok(Some(result))
                    }
                    None => Ok(None),
                }
            }
            IteratorValue::FilterIter { inner, func } => {
                let inner_clone = inner.clone();
                let func_clone = func.clone();
                drop(iter);
                let inner_rc = Rc::new(RefCell::new(*inner_clone));
                loop {
                    let val = self.iter_next(&inner_rc)?;
                    // Write back
                    let advanced = inner_rc.borrow().clone();
                    let mut iter = iter_rc.borrow_mut();
                    if let IteratorValue::FilterIter { inner, .. } = &mut *iter {
                        **inner = advanced.clone();
                    }
                    drop(iter);
                    match val {
                        Some(v) => {
                            let pred = self.call_function(&func_clone, vec![v.clone()])?;
                            if matches!(pred, Value::Bool(true)) {
                                return Ok(Some(v));
                            }
                            // Update inner_rc for next iteration
                            let iter = iter_rc.borrow();
                            if let IteratorValue::FilterIter { inner, .. } = &*iter {
                                *inner_rc.borrow_mut() = *inner.clone();
                            }
                        }
                        None => return Ok(None),
                    }
                }
            }
            _ => Ok(iter.next_simple()),
        }
    }

    /// Evaluates an assignment expression.
    fn eval_assign(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> EvalResult {
        let new_val = self.eval_expr(value)?;

        match target {
            Expr::Ident { name, .. } => {
                let final_val = if op == AssignOp::Assign {
                    new_val
                } else {
                    let old = self.eval_ident(name)?;
                    self.apply_compound_assign(&old, op, &new_val)?
                };
                if !self.env.borrow_mut().assign(name, final_val) {
                    return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                }
                Ok(Value::Null)
            }
            Expr::Index { object, index, .. } => {
                let idx_val = self.eval_expr(index)?;
                let obj_val = self.eval_expr(object)?;

                match (&obj_val, &idx_val) {
                    (Value::Array(arr), Value::Int(i)) => {
                        let idx = *i as usize;
                        if idx >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds {
                                index: *i,
                                collection: "array".into(),
                                length: arr.len(),
                            }
                            .into());
                        }
                        let mut new_arr = arr.clone();
                        let final_val = if op == AssignOp::Assign {
                            new_val
                        } else {
                            self.apply_compound_assign(&arr[idx], op, &new_val)?
                        };
                        new_arr[idx] = final_val;
                        // Re-assign the whole array back
                        if let Expr::Ident { name, .. } = object.as_ref() {
                            if !self.env.borrow_mut().assign(name, Value::Array(new_arr)) {
                                return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                            }
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(RuntimeError::InvalidAssignTarget.into()),
                }
            }
            Expr::Field { object, field, .. } => {
                let obj_val = self.eval_expr(object)?;
                match obj_val {
                    Value::Struct {
                        name: sname,
                        mut fields,
                    } => {
                        let final_val = if op == AssignOp::Assign {
                            new_val
                        } else {
                            let old = fields
                                .get(field)
                                .ok_or(RuntimeError::TypeError(format!(
                                    "struct '{sname}' has no field '{field}'"
                                )))?
                                .clone();
                            self.apply_compound_assign(&old, op, &new_val)?
                        };
                        fields.insert(field.clone(), final_val);
                        // Re-assign struct
                        if let Expr::Ident { name, .. } = object.as_ref() {
                            let new_struct = Value::Struct {
                                name: sname,
                                fields,
                            };
                            if !self.env.borrow_mut().assign(name, new_struct) {
                                return Err(RuntimeError::UndefinedVariable(name.clone()).into());
                            }
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(RuntimeError::InvalidAssignTarget.into()),
                }
            }
            _ => Err(RuntimeError::InvalidAssignTarget.into()),
        }
    }

    /// Applies a compound assignment operator (+=, -=, etc.).
    fn apply_compound_assign(&self, old: &Value, op: AssignOp, new_val: &Value) -> EvalResult {
        let binop = match op {
            AssignOp::AddAssign => BinOp::Add,
            AssignOp::SubAssign => BinOp::Sub,
            AssignOp::MulAssign => BinOp::Mul,
            AssignOp::DivAssign => BinOp::Div,
            AssignOp::RemAssign => BinOp::Rem,
            AssignOp::BitAndAssign => BinOp::BitAnd,
            AssignOp::BitOrAssign => BinOp::BitOr,
            AssignOp::BitXorAssign => BinOp::BitXor,
            AssignOp::ShlAssign => BinOp::Shl,
            AssignOp::ShrAssign => BinOp::Shr,
            AssignOp::Assign => unreachable!(),
        };
        match (old, new_val) {
            (Value::Int(a), Value::Int(b)) => self.eval_int_binop(*a, binop, *b),
            (Value::Float(a), Value::Float(b)) => self.eval_float_binop(*a, binop, *b),
            (Value::Int(a), Value::Float(b)) => self.eval_float_binop(*a as f64, binop, *b),
            (Value::Float(a), Value::Int(b)) => self.eval_float_binop(*a, binop, *b as f64),
            (Value::Str(a), Value::Str(b)) if binop == BinOp::Add => {
                Ok(Value::Str(format!("{a}{b}")))
            }
            // Pointer compound assignment: ptr += offset, ptr -= offset
            (Value::Pointer(addr), Value::Int(offset)) if binop == BinOp::Add => {
                Ok(Value::Pointer(addr.wrapping_add(*offset as u64)))
            }
            (Value::Pointer(addr), Value::Int(offset)) if binop == BinOp::Sub => {
                Ok(Value::Pointer(addr.wrapping_sub(*offset as u64)))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "unsupported compound assignment for {} and {}",
                old.type_name(),
                new_val.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a match expression.
    fn eval_match(&mut self, subject: &Expr, arms: &[MatchArm]) -> EvalResult {
        let subject_val = self.eval_expr(subject)?;

        for arm in arms {
            if let Some(bindings) = self.match_pattern(&arm.pattern, &subject_val) {
                // Check guard if present
                if let Some(guard) = &arm.guard {
                    // Create scope with bindings for guard evaluation
                    let guard_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                        &self.env,
                    ))));
                    for (k, v) in &bindings {
                        guard_env.borrow_mut().define(k.clone(), v.clone());
                    }
                    let prev = Rc::clone(&self.env);
                    self.env = guard_env;
                    let guard_val = self.eval_expr(guard)?;
                    self.env = prev;
                    if !guard_val.is_truthy() {
                        continue;
                    }
                }

                // Create scope with pattern bindings and evaluate body
                let arm_env = Rc::new(RefCell::new(Environment::new_with_parent(Rc::clone(
                    &self.env,
                ))));
                for (k, v) in bindings {
                    arm_env.borrow_mut().define(k, v);
                }
                let prev = Rc::clone(&self.env);
                self.env = arm_env;
                let result = self.eval_expr(&arm.body);
                self.env = prev;
                return result;
            }
        }

        // No arm matched
        Ok(Value::Null)
    }

    /// Attempts to match a value against a pattern.
    ///
    /// Returns `Some(bindings)` if the pattern matches, `None` otherwise.
    fn match_pattern(&self, pattern: &Pattern, value: &Value) -> Option<HashMap<String, Value>> {
        match pattern {
            Pattern::Wildcard { .. } => Some(HashMap::new()),
            Pattern::Ident { name, .. } => {
                // Check if this is a known unit enum variant (e.g., None)
                if let Some(Value::Enum {
                    variant,
                    data: None,
                }) = self.env.borrow().lookup(name)
                {
                    // Compare as unit variant match
                    return if let Value::Enum {
                        variant: v,
                        data: d,
                    } = value
                    {
                        if &variant == v && d.is_none() {
                            Some(HashMap::new())
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                }
                let mut bindings = HashMap::new();
                bindings.insert(name.clone(), value.clone());
                Some(bindings)
            }
            Pattern::Literal { kind, .. } => {
                let pat_val = match kind {
                    LiteralKind::Int(v) => Value::Int(*v),
                    LiteralKind::Float(v) => Value::Float(*v),
                    LiteralKind::String(s) | LiteralKind::RawString(s) => Value::Str(s.clone()),
                    LiteralKind::Char(c) => Value::Char(*c),
                    LiteralKind::Bool(b) => Value::Bool(*b),
                    LiteralKind::Null => Value::Null,
                };
                if &pat_val == value {
                    Some(HashMap::new())
                } else {
                    None
                }
            }
            Pattern::Tuple { elements, .. } => {
                if let Value::Tuple(vals) = value {
                    if elements.len() != vals.len() {
                        return None;
                    }
                    let mut bindings = HashMap::new();
                    for (pat, val) in elements.iter().zip(vals.iter()) {
                        let sub = self.match_pattern(pat, val)?;
                        bindings.extend(sub);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Enum {
                variant, fields, ..
            } => {
                if let Value::Enum {
                    variant: v,
                    data: d,
                } = value
                {
                    if variant != v {
                        return None;
                    }
                    if fields.is_empty() {
                        // Unit variant pattern
                        if d.is_none() {
                            return Some(HashMap::new());
                        }
                        return None;
                    }
                    // Variant with data
                    if let Some(inner) = d {
                        if fields.len() == 1 {
                            return self.match_pattern(&fields[0], inner);
                        }
                        // Multiple fields — match against tuple
                        if let Value::Tuple(vals) = inner.as_ref() {
                            if fields.len() != vals.len() {
                                return None;
                            }
                            let mut bindings = HashMap::new();
                            for (pat, val) in fields.iter().zip(vals.iter()) {
                                let sub = self.match_pattern(pat, val)?;
                                bindings.extend(sub);
                            }
                            return Some(bindings);
                        }
                    }
                    None
                } else {
                    None
                }
            }
            Pattern::Struct {
                name: _,
                fields: pat_fields,
                ..
            } => {
                if let Value::Struct { fields, .. } = value {
                    let mut bindings = HashMap::new();
                    for fp in pat_fields {
                        let val = fields.get(&fp.name)?;
                        if let Some(ref pat) = fp.pattern {
                            let sub = self.match_pattern(pat, val)?;
                            bindings.extend(sub);
                        } else {
                            bindings.insert(fp.name.clone(), val.clone());
                        }
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Range { .. } => {
                // Phase 1: basic range matching not yet supported
                None
            }
        }
    }

    /// Evaluates an array literal.
    fn eval_array(&mut self, elements: &[Expr]) -> EvalResult {
        let mut vals = Vec::with_capacity(elements.len());
        for e in elements {
            vals.push(self.eval_expr(e)?);
        }
        Ok(Value::Array(vals))
    }

    /// Evaluates a tuple literal.
    fn eval_tuple(&mut self, elements: &[Expr]) -> EvalResult {
        let mut vals = Vec::with_capacity(elements.len());
        for e in elements {
            vals.push(self.eval_expr(e)?);
        }
        Ok(Value::Tuple(vals))
    }

    /// Evaluates a pipeline expression: `x |> f` → `f(x)`.
    fn eval_pipe(&mut self, left: &Expr, right: &Expr) -> EvalResult {
        let arg = self.eval_expr(left)?;
        let func = self.eval_expr(right)?;
        match func {
            Value::Function(fv) => self.call_function(&fv, vec![arg]),
            Value::BuiltinFn(name) => self.call_builtin(&name, vec![arg]),
            _ => Err(RuntimeError::NotAFunction(format!("{func}")).into()),
        }
    }

    /// Evaluates struct initialization: `Point { x: 1, y: 2 }`.
    fn eval_struct_init(&mut self, name: &str, fields: &[FieldInit]) -> EvalResult {
        let mut field_map = HashMap::new();
        for fi in fields {
            let val = self.eval_expr(&fi.value)?;
            field_map.insert(fi.name.clone(), val);
        }
        Ok(Value::Struct {
            name: name.to_string(),
            fields: field_map,
        })
    }

    /// Evaluates field access: `obj.field`.
    fn eval_field(&mut self, object: &Expr, field: &str) -> EvalResult {
        let obj = self.eval_expr(object)?;
        match &obj {
            Value::Struct { name, fields } => fields.get(field).cloned().ok_or_else(|| {
                RuntimeError::TypeError(format!("struct '{name}' has no field '{field}'")).into()
            }),
            Value::Tuple(elems) => {
                // Support tuple.0, tuple.1, etc.
                if let Ok(idx) = field.parse::<usize>() {
                    elems.get(idx).cloned().ok_or_else(|| {
                        RuntimeError::IndexOutOfBounds {
                            index: idx as i64,
                            collection: "tuple".into(),
                            length: elems.len(),
                        }
                        .into()
                    })
                } else {
                    Err(
                        RuntimeError::TypeError(format!("cannot access field '{field}' on tuple"))
                            .into(),
                    )
                }
            }
            _ => Err(RuntimeError::TypeError(format!(
                "cannot access field '{field}' on {}",
                obj.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates index access: `arr[i]`.
    fn eval_index(&mut self, object: &Expr, index: &Expr) -> EvalResult {
        let obj = self.eval_expr(object)?;
        let idx = self.eval_expr(index)?;

        match (&obj, &idx) {
            (Value::Array(arr), Value::Int(i)) => {
                let idx_usize = *i as usize;
                arr.get(idx_usize).cloned().ok_or_else(|| {
                    RuntimeError::IndexOutOfBounds {
                        index: *i,
                        collection: "array".into(),
                        length: arr.len(),
                    }
                    .into()
                })
            }
            (Value::Str(s), Value::Int(i)) => {
                let idx_usize = *i as usize;
                let char_len = s.chars().count();
                s.chars().nth(idx_usize).map(Value::Char).ok_or_else(|| {
                    RuntimeError::IndexOutOfBounds {
                        index: *i,
                        collection: "string".into(),
                        length: char_len,
                    }
                    .into()
                })
            }
            _ => Err(RuntimeError::TypeError(format!(
                "cannot index {} with {}",
                obj.type_name(),
                idx.type_name()
            ))
            .into()),
        }
    }

    /// Evaluates a range expression, producing an Array of integers.
    fn eval_range(
        &mut self,
        start: &Option<Box<Expr>>,
        end: &Option<Box<Expr>>,
        inclusive: bool,
    ) -> EvalResult {
        let start_val = match start {
            Some(e) => match self.eval_expr(e)? {
                Value::Int(v) => v,
                _ => {
                    return Err(
                        RuntimeError::TypeError("range bounds must be integers".into()).into(),
                    )
                }
            },
            None => 0,
        };

        let end_val = match end {
            Some(e) => match self.eval_expr(e)? {
                Value::Int(v) => v,
                _ => {
                    return Err(
                        RuntimeError::TypeError("range bounds must be integers".into()).into(),
                    )
                }
            },
            None => {
                return Err(RuntimeError::TypeError("range must have an end bound".into()).into())
            }
        };

        let items: Vec<Value> = if inclusive {
            (start_val..=end_val).map(Value::Int).collect()
        } else {
            (start_val..end_val).map(Value::Int).collect()
        };

        Ok(Value::Array(items))
    }

    /// Evaluates a closure expression.
    fn eval_closure(
        &mut self,
        params: &[crate::parser::ast::ClosureParam],
        body: &Expr,
    ) -> EvalResult {
        let closure_params: Vec<crate::parser::ast::Param> = params
            .iter()
            .map(|cp| crate::parser::ast::Param {
                name: cp.name.clone(),
                ty: cp
                    .ty
                    .clone()
                    .unwrap_or(crate::parser::ast::TypeExpr::Simple {
                        name: "any".to_string(),
                        span: crate::lexer::token::Span::new(0, 0),
                    }),
                span: cp.span,
            })
            .collect();

        Ok(Value::Function(FnValue {
            name: String::new(),
            params: closure_params,
            body: Box::new(body.clone()),
            closure_env: Rc::clone(&self.env),
        }))
    }

    /// Evaluates the `?` (try) operator.
    ///
    /// Unwraps `Ok(v)` or `Some(v)` to `v`.
    /// For `Err(e)` or `None`, early-returns from the enclosing function.
    /// Reorders named arguments to match parameter order.
    fn reorder_named_args(
        &self,
        params: &[crate::parser::ast::Param],
        args: Vec<(Option<String>, Value)>,
    ) -> Result<Vec<Value>, EvalError> {
        let mut result = vec![Value::Null; params.len()];
        let mut filled = vec![false; params.len()];
        let mut positional_idx = 0;

        for (name, val) in args {
            if let Some(arg_name) = name {
                // Named argument: find matching parameter
                let pos = params
                    .iter()
                    .position(|p| p.name == arg_name)
                    .ok_or_else(|| {
                        RuntimeError::TypeError(format!("unknown parameter name '{arg_name}'"))
                    })?;
                result[pos] = val;
                filled[pos] = true;
            } else {
                // Positional argument: fill next unfilled slot
                while positional_idx < params.len() && filled[positional_idx] {
                    positional_idx += 1;
                }
                if positional_idx >= params.len() {
                    return Err(RuntimeError::ArityMismatch {
                        expected: params.len(),
                        got: positional_idx + 1,
                    }
                    .into());
                }
                result[positional_idx] = val;
                filled[positional_idx] = true;
                positional_idx += 1;
            }
        }
        Ok(result)
    }

    /// Helper for unary math functions that take and return f64.
    fn math_f64_unary(&self, args: Vec<Value>, f: fn(f64) -> f64) -> EvalResult {
        if args.len() != 1 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }
            .into());
        }
        let v = match &args[0] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                )
            }
        };
        Ok(Value::Float(f(v)))
    }

    /// Helper for binary math functions that take and return f64.
    fn math_f64_binary(&self, args: Vec<Value>, f: fn(f64, f64) -> f64) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        let a = match &args[0] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                )
            }
        };
        let b = match &args[1] {
            Value::Float(x) => *x,
            Value::Int(x) => *x as f64,
            _ => {
                return Err(
                    RuntimeError::TypeError("math function requires a number".into()).into(),
                )
            }
        };
        Ok(Value::Float(f(a, b)))
    }

    /// Helper for wrapping/saturating integer binary builtins.
    fn int_binop_builtin(
        &self,
        args: Vec<Value>,
        name: &str,
        f: fn(i64, i64) -> i64,
    ) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(f(*a, *b))),
            _ => Err(RuntimeError::TypeError(format!("{name}() requires two integers")).into()),
        }
    }

    /// Helper for checked integer binary builtins (returns Option-like Enum).
    fn checked_int_builtin(
        &self,
        args: Vec<Value>,
        name: &str,
        f: fn(i64, i64) -> Option<i64>,
    ) -> EvalResult {
        if args.len() != 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 2,
                got: args.len(),
            }
            .into());
        }
        match (&args[0], &args[1]) {
            (Value::Int(a), Value::Int(b)) => match f(*a, *b) {
                Some(result) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(Value::Int(result))),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            _ => Err(RuntimeError::TypeError(format!("{name}() requires two integers")).into()),
        }
    }

    /// Evaluates a type cast expression: `expr as Type`.
    fn eval_cast(&mut self, expr: &Expr, target_ty: &TypeExpr) -> EvalResult {
        let val = self.eval_expr(expr)?;
        let type_name = match target_ty {
            TypeExpr::Simple { name, .. } => name.as_str(),
            _ => {
                return Err(
                    RuntimeError::TypeError("cast to complex types not supported".into()).into(),
                )
            }
        };
        match (&val, type_name) {
            // Int → Float
            (Value::Int(n), "f64" | "f32") => Ok(Value::Float(*n as f64)),
            // Float → Int (truncate to target width)
            (Value::Float(f), "i64") => Ok(Value::Int(*f as i64)),
            (Value::Float(f), "i32") => Ok(Value::Int((*f as i32) as i64)),
            (Value::Float(f), "i16") => Ok(Value::Int((*f as i16) as i64)),
            (Value::Float(f), "i8") => Ok(Value::Int((*f as i8) as i64)),
            (Value::Float(f), "u8") => Ok(Value::Int((*f as u8) as i64)),
            (Value::Float(f), "u16") => Ok(Value::Int((*f as u16) as i64)),
            (Value::Float(f), "u32") => Ok(Value::Int((*f as u32) as i64)),
            (Value::Float(f), "u64") => Ok(Value::Int(*f as i64)),
            // Int → Int (narrowing casts truncate, widening preserves)
            (Value::Int(n), "u8") => Ok(Value::Int((*n as u8) as i64)),
            (Value::Int(n), "u16") => Ok(Value::Int((*n as u16) as i64)),
            (Value::Int(n), "u32") => Ok(Value::Int((*n as u32) as i64)),
            (Value::Int(n), "i8") => Ok(Value::Int((*n as i8) as i64)),
            (Value::Int(n), "i16") => Ok(Value::Int((*n as i16) as i64)),
            (Value::Int(n), "i32") => Ok(Value::Int((*n as i32) as i64)),
            (Value::Int(_), "i64" | "u64" | "isize" | "usize") => Ok(val),
            // Float → Float (stored as f64 internally)
            (Value::Float(_), "f64" | "f32") => Ok(val),
            // Bool → Int
            (Value::Bool(b), "i64" | "i32" | "i16" | "i8") => {
                Ok(Value::Int(if *b { 1 } else { 0 }))
            }
            // Int → Bool
            (Value::Int(n), "bool") => Ok(Value::Bool(*n != 0)),
            _ => Err(RuntimeError::TypeError(format!(
                "cannot cast {} to {type_name}",
                val.type_name()
            ))
            .into()),
        }
    }

    fn eval_try(&mut self, expr: &Expr) -> EvalResult {
        let val = self.eval_expr(expr)?;
        match &val {
            Value::Enum { variant, data } => match variant.as_str() {
                "Ok" | "Some" => Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null)),
                "Err" | "None" => Err(ControlFlow::Return(val).into()),
                _ => Err(RuntimeError::TypeError(
                    "? operator requires Option or Result value".into(),
                )
                .into()),
            },
            _ => Err(
                RuntimeError::TypeError("? operator requires Option or Result value".into()).into(),
            ),
        }
    }

    /// Evaluates an impl block: registers methods in the impl_methods registry.
    fn eval_impl_block(&mut self, impl_block: &crate::parser::ast::ImplBlock) -> EvalResult {
        let type_name = &impl_block.target_type;
        // Track trait impls for dynamic dispatch
        if let Some(ref trait_name) = impl_block.trait_name {
            self.trait_impls
                .insert((trait_name.clone(), type_name.clone()));
        }
        for method in &impl_block.methods {
            let fn_val = FnValue {
                name: method.name.clone(),
                params: method.params.clone(),
                body: method.body.clone(),
                closure_env: Rc::clone(&self.env),
            };

            // Check if this is a static method (no `self` param) — also register globally
            let is_static = method.params.first().is_none_or(|p| p.name != "self");
            if is_static {
                // Register as `TypeName::method_name` in global env for path access
                let qualified = format!("{}::{}", type_name, method.name);
                self.env
                    .borrow_mut()
                    .define(qualified, Value::Function(fn_val.clone()));
            }

            self.impl_methods
                .insert((type_name.clone(), method.name.clone()), fn_val);
        }
        Ok(Value::Null)
    }

    /// Evaluates a method call on an iterator value.
    fn eval_iterator_method(
        &mut self,
        iter_rc: Rc<RefCell<IteratorValue>>,
        method: &str,
        args: Vec<Value>,
    ) -> EvalResult {
        match method {
            "next" => {
                let val = self.iter_next(&iter_rc)?;
                Ok(match val {
                    Some(v) => Value::Enum {
                        variant: "Some".into(),
                        data: Some(Box::new(v)),
                    },
                    None => Value::Enum {
                        variant: "None".into(),
                        data: None,
                    },
                })
            }
            "map" => {
                let func = match args.into_iter().next() {
                    Some(Value::Function(fv)) => fv,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".map() requires a function argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let mapped = IteratorValue::MappedIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(mapped))))
            }
            "filter" => {
                let func = match args.into_iter().next() {
                    Some(Value::Function(fv)) => fv,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".filter() requires a function argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let filtered = IteratorValue::FilterIter {
                    inner: Box::new(inner),
                    func,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(filtered))))
            }
            "take" => {
                let n = match args.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".take() requires an integer argument".into(),
                        )
                        .into());
                    }
                };
                let inner = iter_rc.borrow().clone();
                let taken = IteratorValue::TakeIter {
                    inner: Box::new(inner),
                    remaining: n,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(taken))))
            }
            "enumerate" => {
                let inner = iter_rc.borrow().clone();
                let enumerated = IteratorValue::EnumerateIter {
                    inner: Box::new(inner),
                    index: 0,
                };
                Ok(Value::Iterator(Rc::new(RefCell::new(enumerated))))
            }
            "collect" => {
                let mut result = Vec::new();
                while let Some(v) = self.iter_next(&iter_rc)? {
                    result.push(v);
                }
                Ok(Value::Array(result))
            }
            "sum" => {
                let mut total: i64 = 0;
                while let Some(v) = self.iter_next(&iter_rc)? {
                    match v {
                        Value::Int(n) => total += n,
                        Value::Float(f) => total += f as i64,
                        _ => {
                            return Err(RuntimeError::TypeError(
                                ".sum() requires numeric iterator".into(),
                            )
                            .into());
                        }
                    }
                }
                Ok(Value::Int(total))
            }
            "count" => {
                let mut n: i64 = 0;
                while self.iter_next(&iter_rc)?.is_some() {
                    n += 1;
                }
                Ok(Value::Int(n))
            }
            "fold" => {
                if args.len() < 2 {
                    return Err(RuntimeError::TypeError(
                        ".fold() requires init value and function".into(),
                    )
                    .into());
                }
                let mut acc = args[0].clone();
                let func = match &args[1] {
                    Value::Function(fv) => fv.clone(),
                    _ => {
                        return Err(RuntimeError::TypeError(
                            ".fold() second argument must be a function".into(),
                        )
                        .into());
                    }
                };
                while let Some(v) = self.iter_next(&iter_rc)? {
                    acc = self.call_function(&func, vec![acc, v])?;
                }
                Ok(acc)
            }
            _ => Err(RuntimeError::TypeError(format!("no method '{method}' on iterator")).into()),
        }
    }

    /// Coerces a concrete value into a trait object (`dyn Trait`).
    ///
    /// Builds a vtable by looking up all trait methods in impl_methods for the
    /// concrete type.
    fn coerce_to_trait_object(&self, val: Value, trait_name: &str) -> EvalResult {
        let concrete_type = match &val {
            Value::Struct { name, .. } => name.clone(),
            _ => {
                return Err(RuntimeError::TypeError(format!(
                    "cannot coerce {} to dyn {trait_name} (only structs can be trait objects)",
                    match &val {
                        Value::Int(_) => "int",
                        Value::Float(_) => "float",
                        Value::Bool(_) => "bool",
                        Value::Str(_) => "str",
                        _ => "value",
                    }
                ))
                .into());
            }
        };

        // Look up trait method names
        let method_names = match self.trait_defs.get(trait_name) {
            Some(names) => names.clone(),
            None => {
                return Err(
                    RuntimeError::TypeError(format!("unknown trait '{trait_name}'")).into(),
                );
            }
        };

        // Verify this type implements the trait
        if !self
            .trait_impls
            .contains(&(trait_name.to_string(), concrete_type.clone()))
        {
            return Err(RuntimeError::TypeError(format!(
                "type '{concrete_type}' does not implement trait '{trait_name}'"
            ))
            .into());
        }

        // Build vtable from impl_methods
        let mut vtable = HashMap::new();
        for method in &method_names {
            let key = (concrete_type.clone(), method.clone());
            if let Some(fv) = self.impl_methods.get(&key) {
                vtable.insert(method.clone(), fv.clone());
            }
        }

        Ok(Value::TraitObject {
            trait_name: trait_name.to_string(),
            concrete: Box::new(val),
            concrete_type,
            vtable,
        })
    }

    /// Evaluates a module declaration: `mod name { items }` or `mod name;`.
    ///
    /// For inline modules (body=Some), items are evaluated directly.
    /// For file-based modules (body=None), resolves `name.fj` from the source
    /// directory, parses it, and evaluates the resulting items.
    ///
    /// Each symbol is registered in the global environment under its qualified
    /// name (e.g., `math::square`) and stored in `self.modules[name]`.
    fn eval_mod_decl(&mut self, mod_decl: &ModDecl) -> EvalResult {
        let mod_name = &mod_decl.name;

        // For file-based modules, check/track loading state across the full lifecycle
        let is_file_module = mod_decl.body.is_none();
        if is_file_module {
            if self.loading_modules.contains(mod_name) {
                return Err(RuntimeError::Unsupported(format!(
                    "circular module dependency detected: '{mod_name}'"
                ))
                .into());
            }
            self.loading_modules.insert(mod_name.to_string());
        }

        let items = match &mod_decl.body {
            Some(items) => items.clone(),
            None => self.resolve_file_module(mod_name)?,
        };

        let result = self.eval_mod_items(mod_name, &items);

        if is_file_module {
            self.loading_modules.remove(mod_name);
        }

        result
    }

    /// Resolves a file-based module (`mod name;`) to its parsed items.
    ///
    /// Searches for `name.fj` in the source directory and stdlib path.
    /// Detects circular dependencies.
    fn resolve_file_module(&mut self, mod_name: &str) -> Result<Vec<Item>, EvalError> {
        let source_dir = self
            .source_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let file_path = source_dir.join(format!("{mod_name}.fj"));
        if !file_path.exists() {
            return Err(RuntimeError::Unsupported(format!(
                "[PE011] module file not found: '{}'",
                file_path.display()
            ))
            .into());
        }

        let source = std::fs::read_to_string(&file_path).map_err(|e| {
            RuntimeError::Unsupported(format!("cannot read module '{}': {e}", file_path.display()))
        })?;

        let tokens = crate::lexer::tokenize(&source).map_err(|errors| {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::Unsupported(format!(
                "lex error in module '{}': {msg}",
                file_path.display()
            ))
        })?;

        let program = crate::parser::parse(tokens).map_err(|errors| {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            RuntimeError::Unsupported(format!(
                "parse error in module '{}': {msg}",
                file_path.display()
            ))
        })?;

        // Set source_dir to module file's directory for nested module resolution
        let old_dir = self.source_dir.clone();
        if let Some(parent) = file_path.parent() {
            self.source_dir = Some(parent.to_path_buf());
        }

        let items = program.items;

        // Restore source_dir
        self.source_dir = old_dir;

        Ok(items)
    }

    /// Evaluates a list of items belonging to a module.
    fn eval_mod_items(&mut self, mod_name: &str, items: &[Item]) -> EvalResult {
        let mut mod_symbols: HashMap<String, Value> = HashMap::new();
        let mut pub_items: HashSet<String> = HashSet::new();

        for item in items {
            match item {
                Item::FnDef(fndef) => {
                    let fn_val = FnValue {
                        name: fndef.name.clone(),
                        params: fndef.params.clone(),
                        body: fndef.body.clone(),
                        closure_env: Rc::clone(&self.env),
                    };
                    let val = Value::Function(fn_val);
                    mod_symbols.insert(fndef.name.clone(), val.clone());
                    if fndef.is_pub {
                        pub_items.insert(fndef.name.clone());
                    }
                    let qualified = format!("{}::{}", mod_name, fndef.name);
                    self.env.borrow_mut().define(qualified, val);
                }
                Item::StructDef(sdef) => {
                    let val = Value::Str(format!("struct:{}", sdef.name));
                    mod_symbols.insert(sdef.name.clone(), val);
                    if sdef.is_pub {
                        pub_items.insert(sdef.name.clone());
                    }
                }
                Item::ConstDef(cdef) => {
                    let val = self.eval_expr(&cdef.value)?;
                    mod_symbols.insert(cdef.name.clone(), val.clone());
                    if cdef.is_pub {
                        pub_items.insert(cdef.name.clone());
                    }
                    let qualified = format!("{}::{}", mod_name, cdef.name);
                    self.env.borrow_mut().define(qualified, val);
                }
                Item::ModDecl(inner_mod) => {
                    // Nested module: evaluate and store with nested qualified names
                    self.eval_mod_decl(inner_mod)?;
                    if let Some(inner_symbols) = self.modules.get(&inner_mod.name).cloned() {
                        let nested_name = format!("{}::{}", mod_name, inner_mod.name);
                        for (sym_name, sym_val) in &inner_symbols {
                            let qualified = format!("{}::{}", nested_name, sym_name);
                            self.env.borrow_mut().define(qualified, sym_val.clone());
                        }
                        self.modules.insert(nested_name, inner_symbols);
                    }
                }
                Item::EnumDef(edef) => {
                    if edef.is_pub {
                        for variant in &edef.variants {
                            pub_items.insert(variant.name.clone());
                        }
                    }
                    for variant in &edef.variants {
                        if variant.fields.is_empty() {
                            let val = Value::Enum {
                                variant: variant.name.clone(),
                                data: None,
                            };
                            mod_symbols.insert(variant.name.clone(), val.clone());
                            let qualified = format!("{}::{}", mod_name, variant.name);
                            self.env.borrow_mut().define(qualified, val);
                        }
                    }
                }
                Item::ImplBlock(impl_block) => {
                    self.eval_impl_block(impl_block)?;
                }
                _ => {
                    self.eval_item(item)?;
                }
            }
        }

        self.modules.insert(mod_name.to_string(), mod_symbols);
        self.module_pub_items
            .insert(mod_name.to_string(), pub_items);
        Ok(Value::Null)
    }

    /// Checks if a symbol is accessible from outside a module.
    ///
    /// If the module has any `pub` items, only `pub` items are accessible.
    /// If the module has NO `pub` items (legacy), all items are accessible.
    fn is_item_visible(&self, mod_path: &str, item_name: &str) -> bool {
        match self.module_pub_items.get(mod_path) {
            Some(pub_set) if !pub_set.is_empty() => pub_set.contains(item_name),
            _ => true, // legacy: no pub markers → everything visible
        }
    }

    /// Evaluates a use declaration: `use path::item`, `use path::*`, `use path::{a, b}`.
    ///
    /// Imports symbols from a registered module into the current scope.
    /// Respects `pub` visibility: only public items can be imported.
    fn eval_use_decl(&mut self, use_decl: &UseDecl) -> EvalResult {
        let path = &use_decl.path;

        match &use_decl.kind {
            UseKind::Simple => {
                // `use math::square` — import the last segment
                if path.len() >= 2 {
                    let mod_path = path[..path.len() - 1].join("::");
                    let item_name = &path[path.len() - 1];
                    if !self.is_item_visible(&mod_path, item_name) {
                        return Err(RuntimeError::TypeError(format!(
                            "'{item_name}' is private in module '{mod_path}'"
                        ))
                        .into());
                    }
                    let qualified = format!("{}::{}", mod_path, item_name);
                    let resolved = self.env.borrow().lookup(&qualified).or_else(|| {
                        self.modules
                            .get(&mod_path)
                            .and_then(|m| m.get(item_name).cloned())
                    });
                    if let Some(val) = resolved {
                        self.env.borrow_mut().define(item_name.clone(), val);
                    }
                }
                Ok(Value::Null)
            }
            UseKind::Glob => {
                // `use math::*` — import all PUBLIC symbols from module
                let mod_path = path.join("::");
                if let Some(mod_syms) = self.modules.get(&mod_path).cloned() {
                    for (name, val) in mod_syms {
                        if self.is_item_visible(&mod_path, &name) {
                            self.env.borrow_mut().define(name, val);
                        }
                    }
                }
                Ok(Value::Null)
            }
            UseKind::Group(names) => {
                // `use math::{square, cube}` — import specific items
                let mod_path = path.join("::");
                let mut imports = Vec::new();
                for name in names {
                    if !self.is_item_visible(&mod_path, name) {
                        return Err(RuntimeError::TypeError(format!(
                            "'{name}' is private in module '{mod_path}'"
                        ))
                        .into());
                    }
                    let qualified = format!("{}::{}", mod_path, name);
                    let resolved = self.env.borrow().lookup(&qualified).or_else(|| {
                        self.modules
                            .get(&mod_path)
                            .and_then(|m| m.get(name).cloned())
                    });
                    if let Some(val) = resolved {
                        imports.push((name.clone(), val));
                    }
                }
                for (name, val) in imports {
                    self.env.borrow_mut().define(name, val);
                }
                Ok(Value::Null)
            }
        }
    }

    /// Extracts an array of i64 values from a Value::Array.
    fn extract_i64_array(&self, val: &Value, fn_name: &str) -> Result<Vec<i64>, EvalError> {
        match val {
            Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for v in arr {
                    match v {
                        Value::Int(n) => result.push(*n),
                        _ => {
                            return Err(RuntimeError::TypeError(format!(
                                "{fn_name} requires array of integers"
                            ))
                            .into())
                        }
                    }
                }
                Ok(result)
            }
            _ => Err(RuntimeError::TypeError(format!("{fn_name} requires array argument")).into()),
        }
    }

    /// Evaluates a method call: `obj.method(args)`.
    fn eval_method_call(&mut self, receiver: &Expr, method: &str, args: &[CallArg]) -> EvalResult {
        let obj = self.eval_expr(receiver)?;
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.eval_expr(&arg.value)?);
        }

        // Check impl methods first — look up by struct name
        if let Value::Struct { name, .. } = &obj {
            let key = (name.clone(), method.to_string());
            if let Some(fv) = self.impl_methods.get(&key).cloned() {
                // Instance method: prepend receiver as `self` argument
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![obj];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
        }

        // Trait object dynamic dispatch — look up method in vtable
        if let Value::TraitObject {
            vtable, concrete, ..
        } = &obj
        {
            if let Some(fv) = vtable.get(method).cloned() {
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![*concrete.clone()];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
            return Err(
                RuntimeError::TypeError(format!("no method '{method}' on trait object")).into(),
            );
        }

        // Iterator methods on collections: .iter()
        if method == "iter" {
            let iter_val = match obj {
                Value::Array(arr) => IteratorValue::Array { items: arr, pos: 0 },
                Value::Str(s) => IteratorValue::Chars {
                    chars: s.chars().collect(),
                    pos: 0,
                },
                Value::Map(m) => IteratorValue::Map {
                    entries: m.into_iter().collect(),
                    pos: 0,
                },
                _ => {
                    return Err(RuntimeError::TypeError(format!(
                        "cannot call .iter() on {}",
                        obj.type_name()
                    ))
                    .into());
                }
            };
            return Ok(Value::Iterator(Rc::new(RefCell::new(iter_val))));
        }

        // Iterator combinator/consumer methods
        if let Value::Iterator(iter_rc) = obj {
            return self.eval_iterator_method(iter_rc, method, arg_vals);
        }

        // Check impl methods for enum values
        if let Value::Enum { variant, .. } = &obj {
            let key = (variant.clone(), method.to_string());
            if let Some(fv) = self.impl_methods.get(&key).cloned() {
                let has_self = fv.params.first().is_some_and(|p| p.name == "self");
                let call_args = if has_self {
                    let mut all = vec![obj];
                    all.extend(arg_vals);
                    all
                } else {
                    arg_vals
                };
                return self.call_function(&fv, call_args);
            }
        }

        // Option/Result utility methods
        if let Value::Enum { variant, data } = &obj {
            match method {
                "unwrap" => {
                    return match variant.as_str() {
                        "Some" | "Ok" => {
                            Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null))
                        }
                        "None" => Err(RuntimeError::TypeError(
                            "called unwrap() on None value".into(),
                        )
                        .into()),
                        "Err" => Err(RuntimeError::TypeError(format!(
                            "called unwrap() on Err({})",
                            data.as_ref().map(|d| format!("{d}")).unwrap_or_default()
                        ))
                        .into()),
                        _ => Err(RuntimeError::TypeError(format!(
                            "no method 'unwrap' on variant '{variant}'"
                        ))
                        .into()),
                    };
                }
                "unwrap_or" => {
                    return match variant.as_str() {
                        "Some" | "Ok" => {
                            Ok(data.as_ref().map(|d| *d.clone()).unwrap_or(Value::Null))
                        }
                        "None" | "Err" => Ok(arg_vals.into_iter().next().unwrap_or(Value::Null)),
                        _ => Err(RuntimeError::TypeError(format!(
                            "no method 'unwrap_or' on variant '{variant}'"
                        ))
                        .into()),
                    };
                }
                "is_some" => return Ok(Value::Bool(variant == "Some")),
                "is_none" => return Ok(Value::Bool(variant == "None")),
                "is_ok" => return Ok(Value::Bool(variant == "Ok")),
                "is_err" => return Ok(Value::Bool(variant == "Err")),
                _ => {}
            }
        }

        // Built-in methods on primitive types
        match (&obj, method) {
            // String methods
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Str(s), "contains") => {
                if let Some(Value::Str(sub)) = arg_vals.first() {
                    Ok(Value::Bool(s.contains(sub.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("contains() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "trim") => Ok(Value::Str(s.trim().to_string())),
            (Value::Str(s), "trim_start") => Ok(Value::Str(s.trim_start().to_string())),
            (Value::Str(s), "trim_end") => Ok(Value::Str(s.trim_end().to_string())),
            (Value::Str(s), "to_uppercase") => Ok(Value::Str(s.to_uppercase())),
            (Value::Str(s), "to_lowercase") => Ok(Value::Str(s.to_lowercase())),
            (Value::Str(s), "starts_with") => {
                if let Some(Value::Str(prefix)) = arg_vals.first() {
                    Ok(Value::Bool(s.starts_with(prefix.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("starts_with() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "ends_with") => {
                if let Some(Value::Str(suffix)) = arg_vals.first() {
                    Ok(Value::Bool(s.ends_with(suffix.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("ends_with() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "replace") => {
                if let (Some(Value::Str(from)), Some(Value::Str(to))) =
                    (arg_vals.first(), arg_vals.get(1))
                {
                    Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
                } else {
                    Err(
                        RuntimeError::TypeError("replace() requires two string arguments".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "split") => {
                if let Some(Value::Str(sep)) = arg_vals.first() {
                    let parts: Vec<Value> = s
                        .split(sep.as_str())
                        .map(|p| Value::Str(p.to_string()))
                        .collect();
                    Ok(Value::Array(parts))
                } else {
                    Err(RuntimeError::TypeError("split() requires a string argument".into()).into())
                }
            }
            (Value::Str(s), "repeat") => {
                if let Some(Value::Int(n)) = arg_vals.first() {
                    Ok(Value::Str(s.repeat(*n as usize)))
                } else {
                    Err(
                        RuntimeError::TypeError("repeat() requires an integer argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "chars") => {
                let chars: Vec<Value> = s.chars().map(Value::Char).collect();
                Ok(Value::Array(chars))
            }
            (Value::Str(s), "substring") => {
                let start = match arg_vals.first() {
                    Some(Value::Int(n)) => *n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "substring() requires integer arguments".into(),
                        )
                        .into())
                    }
                };
                let end = match arg_vals.get(1) {
                    Some(Value::Int(n)) => *n as usize,
                    _ => s.len(),
                };
                let result: String = s
                    .chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect();
                Ok(Value::Str(result))
            }
            (Value::Str(s), "parse_int") => match s.trim().parse::<i64>() {
                Ok(n) => Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Int(n))),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("parse error: {}", e)))),
                }),
            },
            (Value::Str(s), "parse_float") => match s.trim().parse::<f64>() {
                Ok(f) => Ok(Value::Enum {
                    variant: "Ok".into(),
                    data: Some(Box::new(Value::Float(f))),
                }),
                Err(e) => Ok(Value::Enum {
                    variant: "Err".into(),
                    data: Some(Box::new(Value::Str(format!("parse error: {}", e)))),
                }),
            },
            (Value::Str(s), "is_empty") => Ok(Value::Bool(s.is_empty())),
            (Value::Str(s), "index_of") => {
                if let Some(Value::Str(needle)) = arg_vals.first() {
                    match s.find(needle.as_str()) {
                        Some(pos) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(Value::Int(pos as i64))),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    }
                } else {
                    Err(
                        RuntimeError::TypeError("index_of() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Str(s), "rev") => Ok(Value::Str(s.chars().rev().collect())),
            (Value::Str(s), "bytes") => Ok(Value::Array(
                s.bytes().map(|b| Value::Int(b as i64)).collect(),
            )),
            // Array methods
            (Value::Array(a), "len") => Ok(Value::Int(a.len() as i64)),
            (Value::Array(a), "push") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let mut new_arr = a.clone();
                new_arr.push(arg_vals.into_iter().next().unwrap_or(Value::Null));
                Ok(Value::Array(new_arr))
            }
            (Value::Array(a), "is_empty") => Ok(Value::Bool(a.is_empty())),
            (Value::Array(a), "first") => match a.first() {
                Some(v) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(v.clone())),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            (Value::Array(a), "last") => match a.last() {
                Some(v) => Ok(Value::Enum {
                    variant: "Some".into(),
                    data: Some(Box::new(v.clone())),
                }),
                None => Ok(Value::Enum {
                    variant: "None".into(),
                    data: None,
                }),
            },
            // Map methods
            (Value::Map(m), "len") => Ok(Value::Int(m.len() as i64)),
            (Value::Map(m), "is_empty") => Ok(Value::Bool(m.is_empty())),
            (Value::Map(m), "contains_key") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    Ok(Value::Bool(m.contains_key(k)))
                } else {
                    Err(
                        RuntimeError::TypeError("contains_key() requires a string argument".into())
                            .into(),
                    )
                }
            }
            (Value::Map(m), "get") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    match m.get(k) {
                        Some(v) => Ok(Value::Enum {
                            variant: "Some".into(),
                            data: Some(Box::new(v.clone())),
                        }),
                        None => Ok(Value::Enum {
                            variant: "None".into(),
                            data: None,
                        }),
                    }
                } else {
                    Err(RuntimeError::TypeError("get() requires a string argument".into()).into())
                }
            }
            (Value::Map(m), "keys") => {
                let keys: Vec<Value> = m.keys().map(|k| Value::Str(k.clone())).collect();
                Ok(Value::Array(keys))
            }
            (Value::Map(m), "values") => {
                let vals: Vec<Value> = m.values().cloned().collect();
                Ok(Value::Array(vals))
            }
            (Value::Map(m), "insert") => {
                if arg_vals.len() != 2 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 2,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let mut args_iter = arg_vals.into_iter();
                let key = args_iter.next().unwrap();
                let val = args_iter.next().unwrap();
                if let Value::Str(k) = key {
                    let mut new_map = m.clone();
                    new_map.insert(k, val);
                    Ok(Value::Map(new_map))
                } else {
                    Err(RuntimeError::TypeError("insert() requires a string key".into()).into())
                }
            }
            (Value::Map(m), "remove") => {
                if let Some(Value::Str(k)) = arg_vals.first() {
                    let mut new_map = m.clone();
                    new_map.remove(k);
                    Ok(Value::Map(new_map))
                } else {
                    Err(
                        RuntimeError::TypeError("remove() requires a string argument".into())
                            .into(),
                    )
                }
            }
            // Array methods continued
            (Value::Array(a), "join") => {
                if let Some(Value::Str(sep)) = arg_vals.first() {
                    let joined: String = a
                        .iter()
                        .map(|v| match v {
                            Value::Str(s) => s.clone(),
                            other => format!("{other}"),
                        })
                        .collect::<Vec<_>>()
                        .join(sep.as_str());
                    Ok(Value::Str(joined))
                } else {
                    Err(RuntimeError::TypeError(
                        "join() requires a string separator argument".into(),
                    )
                    .into())
                }
            }
            (Value::Array(a), "reverse") => {
                let mut reversed = a.clone();
                reversed.reverse();
                Ok(Value::Array(reversed))
            }
            (Value::Array(a), "contains") => {
                if arg_vals.len() != 1 {
                    return Err(RuntimeError::ArityMismatch {
                        expected: 1,
                        got: arg_vals.len(),
                    }
                    .into());
                }
                let needle = &arg_vals[0];
                let found = a.iter().any(|v| v == needle);
                Ok(Value::Bool(found))
            }
            _ => Err(RuntimeError::TypeError(format!(
                "no method '{method}' on type {}",
                obj.type_name()
            ))
            .into()),
        }
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
    fn eval_closure() {
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
}
