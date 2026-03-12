# Architecture — Fajar Lang

> Desain sistem, komponen boundaries, dan contract antar modul.

## 1. System Overview

```
Source (.fj file)
    | raw text
    v
LEXER (src/lexer/)
    Input:  &str
    Output: Vec<Token>
    Errors: LexError (LE001-LE008)
    | token stream
    v
PARSER (src/parser/)
    Input:  Vec<Token>
    Output: AST (Program node)
    Method: Recursive Descent + Pratt (19 precedence levels)
    | AST
    v
SEMANTIC ANALYZER (src/analyzer/)
    Input:  &Program
    Output: () or Vec<SemanticError>
    Checks: types, scope, context, mutability, lifetimes,
            effects, linear types, borrow checking (NLL)
    | analyzed AST
    v
    +-------------------+-------------------+-------------------+
    |                   |                   |                   |
    v                   v                   v                   v
INTERPRETER         BYTECODE VM         CRANELIFT           LLVM
(tree-walking)      (45 opcodes)        (JIT + AOT)         (AOT)
    |                   |                   |                   |
    +-------------------+-------------------+-------------------+
                        |
              +---------+---------+
              |                   |
              v                   v
        OS RUNTIME           ML RUNTIME
        memory, IRQ,         tensor, autograd,
        syscall, paging,     layers, optim,
        serial, VGA,         quantization, ONNX,
        bus, DMA             GPU compute
```

## 2. Module Contracts

### 2.1 Lexer (`src/lexer/`)

**Responsibility:** Convert raw source text into a flat list of tokens.

```rust
pub fn tokenize(source: &str) -> Result<Vec<Token>, Vec<LexError>>;

pub struct Token {
    pub kind: TokenKind,  // 82+ token kinds
    pub span: Span,
    pub line: u32,
    pub col: u32,
}
```

**Key invariants:**
- All source bytes are covered (no gaps in spans)
- Whitespace and comments produce no tokens
- EOF token is always last
- Errors are non-fatal: collect all errors

### 2.2 Parser (`src/parser/`)

**Responsibility:** Convert token stream into Abstract Syntax Tree.

```rust
pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<ParseError>>;

pub struct Program {
    pub items: Vec<Item>,
    pub span: Span,
}
```

**Key invariants:**
- Every AST node has a valid Span
- Operator precedence: 19 levels (Pratt parser)
- Error recovery: sync to next statement boundary
- Supports: generics, traits, impl blocks, async, macros, effects, lifetimes

### 2.3 Semantic Analyzer (`src/analyzer/`)

**Responsibility:** Type checking, name resolution, annotation validation, borrow checking.

```rust
pub fn analyze(program: &Program) -> Result<(), Vec<SemanticError>>;
pub fn analyze_with_known(prog: &Program, names: &HashSet<String>) -> Result<(), Vec<SemanticError>>;
```

**Key invariants:**
- All identifiers resolved before execution
- @kernel scope: tensor/heap ops are type errors (KE001-KE004)
- @device scope: raw pointers are type errors (DE001-DE003)
- NLL borrow checker: move semantics, borrow rules
- Lifetime analysis with elision rules
- Effect tracking and verification
- Linear type consumption checking

### 2.4 Interpreter (`src/interpreter/`)

**Responsibility:** Evaluate AST and produce runtime values (tree-walking mode).

```rust
pub struct Interpreter { ... }

impl Interpreter {
    pub fn eval_source(&mut self, src: &str) -> Result<Value, FjError>;
    pub fn eval_program(&mut self, prog: &Program) -> Result<Value, RuntimeError>;
}

pub enum Value {
    Null, Int(i64), Float(f64), Bool(bool), Char(char), Str(String),
    Array(Vec<Value>), Tuple(Vec<Value>), Tensor(TensorValue),
    Map(HashMap<String, Value>),
    Struct { name: String, fields: HashMap<String, Value> },
    Enum { variant: String, data: Option<Box<Value>> },
    Function(FnValue), BuiltinFn(String), Pointer(PointerValue),
    Optimizer(OptimizerValue), Layer(LayerValue),
}
```

### 2.5 Bytecode VM (`src/vm/`)

**Responsibility:** Compile AST to bytecode and execute on stack-based VM.

- 45 opcodes (arithmetic, control flow, function calls, closures)
- Stack-based execution model
- Faster than tree-walking for compute-intensive programs

### 2.6 Cranelift Backend (`src/codegen/cranelift/`)

**Responsibility:** Native code generation via Cranelift.

```rust
pub struct CraneliftCompiler { ... }   // JIT compilation
pub struct ObjectCompiler { ... }       // AOT compilation (.o files)
```

**Features:**
- JIT: compile and execute in-memory
- AOT: produce native executables
- Monomorphization for generics
- 150+ runtime functions (`fj_rt_*`)
- Cross-compilation: x86_64, aarch64, riscv64, thumbv7em, wasm32

### 2.7 LLVM Backend (`src/codegen/llvm/`)

**Responsibility:** Production-quality native code generation via LLVM.

- Optimization passes (O0-O3, LTO)
- Platform-specific intrinsics
- Debug info (DWARF) generation
- GPU codegen (PTX for NVIDIA, SPIR-V for Vulkan)

## 3. Data Flow

```
eval_source(source: &str)
│
├── 1. Lexer::tokenize(source)       → Result<Vec<Token>, Vec<LexError>>
├── 2. Parser::parse(tokens)          → Result<Program, Vec<ParseError>>
├── 3. Analyzer::analyze(&program)    → Result<(), Vec<SemanticError>>
└── 4. Backend execution:
        ├── Interpreter::eval_program()  → Result<Value, RuntimeError>
        ├── VM::compile() + vm.run()     → Bytecode execution
        ├── CraneliftCompiler::jit()     → Native execution (JIT)
        └── ObjectCompiler::compile()    → Native executable (AOT)
```

## 4. Runtime Components

### 4.1 OS Runtime (`src/runtime/os/`)

| File | Responsibility |
|------|---------------|
| `memory.rs` | MemoryManager: heap, page tables, protection flags |
| `irq.rs` | IrqTable: handler registration, enable/disable, dispatch |
| `syscall.rs` | SyscallTable: definition, dispatch, standard numbers |
| `paging.rs` | 4-level page table (PML4, PDPT, PD, PT) |
| `gdt.rs` | Global Descriptor Table |
| `idt.rs` | Interrupt Descriptor Table |
| `vga.rs` | VGA text mode buffer |
| `serial.rs` | Serial UART I/O |
| `pit.rs` | Programmable Interval Timer |
| `keyboard.rs` | PS/2 keyboard driver |
| `shell.rs` | Interactive shell |
| `bus.rs` | I2C, SPI, DMA drivers |
| `aarch64.rs` | ARM64 architecture support |
| `riscv.rs` | RISC-V architecture support |

### 4.2 ML Runtime (`src/runtime/ml/`)

| File | Responsibility |
|------|---------------|
| `tensor.rs` | TensorValue: ndarray backend, shape, dtype, grad |
| `autograd.rs` | GradFn trait, computation graph, backward pass |
| `ops.rs` | Tensor operations: matmul, relu, sigmoid, softmax, etc. |
| `optim.rs` | SGD, Adam, AdamW optimizers |
| `layers.rs` | Dense, Conv2d, MultiHeadAttention, LSTM, GRU, BatchNorm |
| `metrics.rs` | accuracy, precision, recall, f1_score |
| `quantize.rs` | INT8 quantization, FJMQ format |
| `onnx.rs` | ONNX import/export |
| `distributed.rs` | Distributed training (data/model parallelism) |

### 4.3 Advanced Type System (`src/analyzer/`)

| Component | File | Responsibility |
|-----------|------|---------------|
| Type checker | `type_check.rs` | Full type checking (6,616 LOC) |
| Scope | `scope.rs` | Symbol table, scope chain, visibility |
| CFG | `cfg.rs` | Control flow graph for NLL analysis |
| Borrow checker | `borrow_lite.rs` | Ownership, move, borrow analysis |

### 4.4 Tooling

| Tool | Location | Responsibility |
|------|----------|---------------|
| Formatter | `src/formatter/` | AST-based pretty-printing (`fj fmt`) |
| LSP Server | `src/lsp/` | Language Server Protocol (tower-lsp) |
| Package Manager | `src/package/` | fj.toml, registry, dependencies |
| Debugger | `src/debugger/` | DAP protocol, breakpoints, stepping |

## 5. Dependency Direction (STRICT)

```
ALLOWED:  main.rs -> interpreter -> analyzer -> parser -> lexer
          main.rs -> vm -> parser -> lexer
          main.rs -> codegen -> parser -> lexer
          interpreter -> runtime/os
          interpreter -> runtime/ml
          codegen -> runtime (via extern "C" fns)

FORBIDDEN: lexer -> parser (no upward deps)
           parser -> interpreter
           runtime/os <-> runtime/ml (siblings, no cross-deps)
           Any cycle
```

## 6. Top-Level Error Type

```rust
pub enum FjError {
    Lex(Vec<LexError>),
    Parse(Vec<ParseError>),
    Semantic(Vec<SemanticError>),
    Runtime(RuntimeError),
}
```

All errors implement `miette::Diagnostic` for beautiful, Rust-style error output with source highlighting.

## 7. Key Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Tree-walking interpreter | Simplest path to working v0.1 |
| 2 | Bytecode VM (45 opcodes) | Faster than tree-walk, portable |
| 3 | Cranelift for native codegen | Lighter than LLVM, good embedded support |
| 4 | LLVM for production codegen | Industrial-strength optimization |
| 5 | ndarray for tensors | Mature, SIMD via BLAS |
| 6 | Collect-all errors | Show all errors at once like Rust |
| 7 | `Rc<RefCell<>>` for env | Closures need shared mutable parent scope |
| 8 | miette for errors | Beautiful Rust-style error output |
| 9 | Pratt parser (19 levels) | Elegant precedence handling |
| 10 | Monomorphization | Static dispatch, no vtables, embedded-friendly |
| 11 | NLL borrow checker | Simpler than Rust (no lifetime annotations in most cases) |
| 12 | Context annotations | Compiler-enforced domain isolation |

---

*Architecture Version: 3.0 | Last Updated: 2026-03-12 (v3.0 — full pipeline with all backends)*
