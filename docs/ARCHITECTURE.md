# Architecture — Fajar Lang

> Dokumen ini menjelaskan desain sistem, komponen boundaries, dan contract antar modul.

## 1. System Overview

```
┌─────────────────────────────────────────────────────┐
│                  Source (.fj file)                   │
└──────────────────────┬──────────────────────────────┘
                       │ raw text
                       ▼
┌─────────────────────────────────────────────────────┐
│                 LEXER (src/lexer/)                   │
│  Input:  &str                                       │
│  Output: Vec<Token>                                 │
│  Errors: LexError { line, col, message }            │
└──────────────────────┬──────────────────────────────┘
                       │ token stream
                       ▼
┌─────────────────────────────────────────────────────┐
│                PARSER (src/parser/)                  │
│  Input:  Vec<Token>                                 │
│  Output: AST (Program node)                         │
│  Errors: ParseError { span, expected, got }         │
│  Method: Recursive Descent + Pratt for expressions  │
└──────────────────────┬──────────────────────────────┘
                       │ AST
                       ▼
┌─────────────────────────────────────────────────────┐
│           SEMANTIC ANALYZER (src/analyzer/)          │
│  Input:  AST                                        │
│  Output: Typed AST + Symbol Table                   │
│  Tasks:  type checking, scope resolution,           │
│          annotation validation, tensor shape check  │
└──────────────────────┬──────────────────────────────┘
                       │ typed AST
                 ┌─────┴─────┐
                 ▼           ▼
          ┌──────────┐  ┌─────────────────┐
          │INTERPRETER│  │ (Future) COMPILER│
          │(Phase 1–4)│  │  LLVM backend   │
          │Tree-walk  │  │  (Phase 5+)     │
          └─────┬─────┘  └─────────────────┘
                │
     ┌──────────┴──────────────────────────┐
     │              RUNTIME                 │
     │  ┌─────────────┐  ┌──────────────┐  │
     │  │  OS Runtime  │  │  ML Runtime  │  │
     │  │  memory.rs   │  │  tensor.rs   │  │
     │  │  irq.rs      │  │  autograd.rs │  │
     │  │  syscall.rs  │  │  ops.rs      │  │
     │  └─────────────┘  └──────────────┘  │
     └─────────────────────────────────────┘
```

## 2. Module Contracts

### 2.1 Lexer (`src/lexer/`)

**Responsibility:** Convert raw source text into a flat list of tokens.

```rust
// Public API
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>;

// Token type
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,    // byte offset (start, end)
    pub line: u32,
    pub col: u32,
}

pub struct Span {
    pub start: usize,
    pub end: usize,
}
```

**Key invariants:**
- All source bytes are covered (no gaps in spans)
- Whitespace and comments produce no tokens (consumed silently)
- EOF token is always the last token
- Errors are non-fatal: collect all errors, don't stop at first

### 2.2 Parser (`src/parser/`)

**Responsibility:** Convert token stream into Abstract Syntax Tree.

```rust
// Public API
pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<ParseError>>;

// AST root
pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}
```

**Key invariants:**
- Every AST node has a valid Span
- Parser never returns partial AST (either full program or error)
- Operator precedence follows spec exactly (11 levels)
- Error recovery: sync to next statement boundary

### 2.3 Semantic Analyzer (`src/analyzer/`)

**Responsibility:** Type checking, name resolution, annotation validation.

```rust
// Public API
pub fn analyze(program: Program) -> Result<TypedProgram, Vec<SemanticError>>;

// Symbol table
pub struct SymbolTable {
    scopes: Vec<Scope>,
}

pub struct Scope {
    symbols: HashMap<String, Symbol>,
    kind: ScopeKind,  // Function | Block | Module | Kernel | Device | Unsafe
}
```

**Key invariants:**
- All identifiers resolved before interpreter runs
- @kernel scope: tensor ops are type errors
- @device scope: raw pointers are type errors
- Tensor shapes verified at compile time where possible

### 2.4 Interpreter (`src/interpreter/`)

**Responsibility:** Evaluate typed AST and produce runtime values.

```rust
// Public API
pub struct Interpreter {
    env: Environment,
    os_rt: OsRuntime,
    ml_rt: MlRuntime,
}

impl Interpreter {
    pub fn new() -> Self;
    // Phase 1: accepts untyped &Program (no analyzer yet)
    // Phase 2+: accepts &TypedProgram after semantic analysis
    pub fn eval_program(&mut self, program: &Program) -> Result<Value, RuntimeError>;
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError>;
    pub fn eval_source(&mut self, source: &str) -> Result<Value, FjError>;
    pub fn call_fn(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RuntimeError>;
}

// Value type — all runtime values
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(String),
    Array(Vec<Value>),
    Tuple(Vec<Value>),
    Struct { name: String, fields: HashMap<String, Value> },
    Enum { variant: String, data: Option<Box<Value>> },
    Function(FnValue),
    BuiltinFn(String),       // built-in functions (print, println, len, etc.)
    Tensor(TensorValue),
    Pointer(PointerValue),
    Null,  // only for void-typed expressions
}
```

## 3. Data Flow

```
FjPipeline::run(source: &str)
│
├── 1. Lexer::tokenize(source)
│      → Result<Vec<Token>, Vec<LexError>>
│
├── 2. Parser::parse(tokens)
│      → Result<Program, Vec<ParseError>>
│
├── 3. Analyzer::analyze(program)
│      → Result<TypedProgram, Vec<SemanticError>>
│
└── 4. Interpreter::eval_program(typed_program)
       → Result<Value, RuntimeError>
```

## 4. Runtime Components

### 4.1 OS Runtime (`src/runtime/os/`)

- `memory.rs` — MemoryManager: heap simulation, page tables, protection flags
- `irq.rs` — IrqTable: handler registration, enable/disable, dispatch
- `syscall.rs` — SyscallTable: definition, dispatch, standard syscall numbers

### 4.2 ML Runtime (`src/runtime/ml/`)

- `tensor.rs` — TensorValue: data (ndarray), shape, dtype, grad tracking
- `autograd.rs` — GradFn trait, computation graph, backward pass
- `ops.rs` — All tensor operations: matmul, relu, sigmoid, softmax, etc.
- `optim.rs` — SGD, Adam optimizers

## 5. Cargo.toml Dependencies

```toml
[package]
name = "fajar-lang"
version = "0.1.0"
edition = "2021"

[dependencies]
# Error handling & formatting
thiserror = "2.0"
miette = "7.0"           # beautiful error reporting

# CLI
clap = { version = "4.5", features = ["derive"] }

# REPL
rustyline = "14.0"

# ML Runtime
ndarray = "0.16"

# Serialization (AST for debugging)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Utilities
indexmap = "2.0"         # ordered HashMap (for symbol table)

[dev-dependencies]
criterion = "0.5"        # benchmarking
pretty_assertions = "1.4" # better test output

[profile.release]
opt-level = 3
lto = true
```

## 6. Key Design Decisions

| # | Decision | Rationale | Trade-off |
|---|----------|-----------|-----------|
| 1 | Tree-walking interpreter (not bytecode VM) | Simplest path to working implementation | Slower execution; upgrade in Phase 5 |
| 2 | ndarray for tensor backend | Mature, well-tested, supports SIMD via BLAS | No GPU yet; add wgpu in Phase 5 |
| 3 | Collect-all errors (not fail-fast) | Better DX — show all errors at once like Rust | Slightly more complex error handling |
| 4 | `Rc<RefCell<>>` for environment | Closures need shared mutable access to parent scope | Not thread-safe; upgrade to `Arc<Mutex<>>` later |
| 5 | miette for error display | Beautiful, Rust-compiler-style error output | Additional dependency |

---

*Architecture Version: 0.2 | Last Updated: 2026-03-05 (Gap Analysis fixes)*
