# API REFERENCE

> Internal Rust API Reference — Fajar Lang Compiler Component APIs

---

## 1. Pipeline API

Top-level pipeline yang menggabungkan semua komponen:

```rust
// src/lib.rs
pub struct FjPipeline;

impl FjPipeline {
    /// Run complete pipeline: lex -> parse -> analyze -> eval
    pub fn run(source: &str) -> Result<Value, FjError>;

    /// Run pipeline until analysis (no eval)
    pub fn check(source: &str) -> Result<TypedProgram, FjError>;

    /// Lex only
    pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>;

    /// Lex + Parse only
    pub fn parse(source: &str) -> Result<Program, FjError>;
}

/// Top-level error type
#[derive(Debug, Error)]
pub enum FjError {
    #[error("Lex errors")]
    Lex(Vec<LexError>),
    #[error("Parse errors")]
    Parse(Vec<ParseError>),
    #[error("Semantic errors")]
    Semantic(Vec<SemanticError>),
    #[error("Runtime error: {0}")]
    Runtime(RuntimeError),
}
```

---

## 2. Lexer API

```rust
// src/lexer/mod.rs
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>;

// src/lexer/token.rs
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub line: u32,
    pub col: u32,
}

pub struct Span {
    pub start: usize,  // byte offset
    pub end: usize,
}

pub enum TokenKind {
    // Literals
    IntLit(i64), FloatLit(f64), StringLit(String), BoolLit(bool), CharLit(char),
    // Identifiers
    Ident(String),
    // Keywords (40+ variants)
    Let, Mut, Fn, Struct, Enum, Impl, Trait, Type, Const,
    If, Else, Match, While, For, In, Return, Break, Continue,
    // ... (lihat FAJAR_LANG_SPEC.md Section 2.1 untuk daftar lengkap)
    // Operators (30+ variants)
    Plus, Minus, Star, Slash, Percent, At, Pipe, PipeArrow,
    // Special
    Eof,
}
```

### 2.1 LexError

```rust
#[derive(Debug, Error)]
pub enum LexError {
    #[error("[LE001] unexpected char '{ch}' at {line}:{col}")]
    UnexpectedChar { ch: char, line: u32, col: u32, span: Span },
    #[error("[LE002] unterminated string at {line}")]
    UnterminatedString { line: u32, span: Span },
    #[error("[LE003] unterminated comment")]
    UnterminatedComment { span: Span },
    #[error("[LE004] invalid number literal")]
    InvalidNumber { span: Span },
    // ... dll
}
```

---

## 3. Parser API

```rust
// src/parser/mod.rs
pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<ParseError>>;

// src/parser/ast.rs
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}

pub enum Item {
    FnDef(FnDef),
    StructDef(StructDef),
    EnumDef(EnumDef),
    TraitDef(TraitDef),
    ImplBlock(ImplBlock),
    UseDecl(UseDecl),
    ModDecl(ModDecl),
    ConstDef(ConstDef),
}

pub enum Expr {
    Literal(LiteralKind, Span),
    Ident(String, Span),
    Binary(Box<Expr>, BinOp, Box<Expr>, Span),
    Unary(UnaryOp, Box<Expr>, Span),
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    If { cond: Box<Expr>, then_: Box<Expr>, else_: Option<Box<Expr>>, span: Span },
    Match { subject: Box<Expr>, arms: Vec<MatchArm>, span: Span },
    Block(Vec<Stmt>, Option<Box<Expr>>, Span),
    Pipe(Box<Expr>, Box<Expr>, Span),
    // ... (20+ variants total)
}
```

---

## 4. Analyzer API

```rust
// src/analyzer/mod.rs
pub fn analyze(program: Program) -> Result<TypedProgram, Vec<SemanticError>>;

// src/analyzer/scope.rs
pub struct SymbolTable {
    scopes: Vec<Scope>,
}
pub struct Scope {
    symbols: HashMap<String, Symbol>,
    kind: ScopeKind,
}
pub enum ScopeKind {
    Module, Function, Block, Kernel, Device, Unsafe,
}

// src/analyzer/type_check.rs
pub struct TypeChecker { ... }
impl TypeChecker {
    pub fn check_expr(&mut self, expr: &Expr) -> Result<Type, SemanticError>;
    pub fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), SemanticError>;
    pub fn check_context(&self, op: &str, ctx: Context) -> Result<(), SemanticError>;
}
```

---

## 5. Interpreter API

```rust
// src/interpreter/mod.rs
pub struct Interpreter {
    env: Environment,
    os_rt: OsRuntime,
    ml_rt: MlRuntime,
}
impl Interpreter {
    pub fn new() -> Self;
    // Phase 1: accepts &Program; Phase 2+: accepts &TypedProgram
    pub fn eval_program(&mut self, prog: &Program) -> Result<Value, RuntimeError>;
    pub fn eval_source(&mut self, source: &str) -> Result<Value, FjError>;
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError>;
    pub fn call_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError>;
}

// src/interpreter/value.rs
pub enum Value {
    Null, Int(i64), Float(f64), Bool(bool), Char(char), Str(String),
    Array(Vec<Value>), Tuple(Vec<Value>), Tensor(TensorValue),
    Struct { name: String, fields: HashMap<String, Value> },
    Enum { variant: String, data: Option<Box<Value>> },
    Function(FnValue), BuiltinFn(String),
    Pointer(PointerValue),
}

// src/interpreter/env.rs
pub struct Environment {
    bindings: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}
```

---

## 6. Runtime APIs

### 6.1 OS Runtime

```rust
// src/runtime/os/memory.rs
pub struct MemoryManager {
    heap: Vec<u8>,
    regions: Vec<MemRegion>,
    page_table: HashMap<VirtAddr, PhysAddr>,
}
impl MemoryManager {
    pub fn alloc(&mut self, size: usize) -> Result<*mut u8, MemError>;
    pub fn free(&mut self, ptr: *mut u8, size: usize) -> Result<(), MemError>;
    pub fn map_page(&mut self, va: VirtAddr, pa: PhysAddr, flags: PageFlags) -> Result<(), MapError>;
}

// src/runtime/os/irq.rs
pub struct IrqTable {
    handlers: [Option<fn()>; 256],
    enabled: bool,
}
```

### 6.2 ML Runtime

```rust
// src/runtime/ml/tensor.rs
pub struct TensorValue {
    data: ndarray::ArrayD<f32>,
    shape: Vec<usize>,
    requires_grad: bool,
    grad: Option<Box<TensorValue>>,
    grad_fn: Option<Box<dyn GradFn>>,
}

// src/runtime/ml/autograd.rs
pub trait GradFn: std::fmt::Debug {
    fn backward(&self, grad_output: &TensorValue) -> Vec<TensorValue>;
}

// src/runtime/ml/ops.rs
pub fn matmul(a: &TensorValue, b: &TensorValue) -> Result<TensorValue, TensorError>;
pub fn relu(x: &TensorValue) -> TensorValue;
pub fn sigmoid(x: &TensorValue) -> TensorValue;
pub fn softmax(x: &TensorValue, dim: usize) -> TensorValue;
```

---

*API Reference Version: 0.2 | Updated: 2026-03-05 (Gap Analysis fixes)*
