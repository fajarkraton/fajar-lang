# Rules — Fajar Lang v1.0 (Production Grade)

> These rules are NON-NEGOTIABLE. Every line of code must comply.

---

## 1. Safety Rules (Absolute)

### 1.1 No Undefined Behavior
- **ZERO** `unsafe {}` blocks outside `src/codegen/` and `src/runtime/os/`
- Every `unsafe` block MUST have `// SAFETY:` comment with:
  - What invariants the caller must uphold
  - Why this is safe
  - What could go wrong if invariants are violated
- Prefer safe abstractions over unsafe code — always

### 1.2 Error Handling
- **NEVER** `.unwrap()` in `src/` — only allowed in `tests/` and `benches/`
- **NEVER** `panic!()` in library code — return `Result` or `Option`
- `.expect("reason")` allowed ONLY in `main.rs` with meaningful message
- ALL errors must have error codes (e.g., SE004, KE001)
- ALL errors must include source span for diagnostics
- COLLECT all errors, don't stop at first — show all at once

### 1.3 Memory Safety
- No raw pointer dereference outside `@kernel`/`@unsafe` context
- No buffer overflows — all array access bounds-checked
- No use-after-move — ownership system must reject
- No data races — shared state must use `Arc<Mutex<T>>` or `Atomic` (v0.3)
- `Rc<RefCell<>>` for single-threaded shared mutable state (closures, environments)

---

## 2. Code Quality Rules

### 2.1 Naming
```
Types/Traits/Enums:   PascalCase      → TokenKind, TensorValue
Functions/methods:    snake_case      → eval_expr, compile_fn
Constants/statics:    SCREAMING_CASE  → MAX_STACK_DEPTH, PAGE_SIZE
Modules:              snake_case      → type_check, borrow_lite
Lifetimes:            short lowercase → 'src, 'a, 'ctx
Type parameters:      PascalCase      → T, K, V
Error codes:          PREFIX + NUMBER → SE004, KE001, CE003
```

### 2.2 Function Size
- **Maximum 50 lines** per function (excluding comments/blank lines)
- If a function exceeds 50 lines → split into helper functions
- Exception: `match` dispatch functions (e.g., `eval_expr`) may be longer
  but each arm should delegate to a helper

### 2.3 Module Organization
```rust
//! Module-level doc comment (REQUIRED)
//! Description of what this module does

// Imports (std first, then external, then internal)
use std::collections::HashMap;
use thiserror::Error;
use crate::parser::ast::Expr;

// Constants
const MAX_DEPTH: usize = 1024;

// Public types and traits (exported API)
pub struct Compiler { ... }
pub enum Instruction { ... }

// Public functions (exported API)
pub fn compile(program: &Program) -> Result<Bytecode, CompileError> { ... }

// Private implementation
fn emit_instruction(&mut self, instr: Instruction) { ... }

// Tests at bottom
#[cfg(test)]
mod tests { ... }
```

### 2.4 Documentation
- ALL `pub` items MUST have `///` doc comments
- Complex functions: include `# Arguments`, `# Returns`, `# Errors`
- Internal helpers: at least one-line `///` comment
- No orphan `TODO:` comments — create a task in V1_TASKS.md instead
- Architecture decisions: document in `docs/` not in code comments

---

## 3. Architecture Rules

### 3.1 Dependency Direction (STRICT)
```
                    ┌──────────┐
                    │  main.rs │
                    └────┬─────┘
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
         ┌────────┐ ┌────────┐ ┌────────┐
         │codegen │ │  vm    │ │  lsp   │
         └───┬────┘ └───┬────┘ └───┬────┘
             │          │          │
             ▼          ▼          ▼
         ┌─────────────────────────────┐
         │       interpreter           │
         └──────────┬──────────────────┘
                    │
         ┌──────────┼──────────┐
         ▼                     ▼
    ┌──────────┐          ┌──────────┐
    │ analyzer │          │ runtime  │
    └────┬─────┘          ├──────────┤
         │                │ os/      │
         ▼                │ ml/      │
    ┌──────────┐          └──────────┘
    │  parser  │
    └────┬─────┘
         │
         ▼
    ┌──────────┐
    │  lexer   │
    └──────────┘
```

**FORBIDDEN:**
- lexer → parser (no upward deps)
- parser → analyzer
- runtime/os ↔ runtime/ml (siblings, no cross-deps)
- Any cycle

### 3.2 Module Contracts
| Module | Input | Output | Errors |
|--------|-------|--------|--------|
| Lexer | `&str` | `Vec<Token>` | `Vec<LexError>` |
| Parser | `Vec<Token>` | `Program` (AST) | `Vec<ParseError>` |
| Analyzer | `&Program` | `()` (modifies symbol table) | `Vec<SemanticError>` |
| Codegen | `&Program` | `CompiledModule` | `Vec<CodegenError>` |
| VM | `CompiledModule` | `Value` | `RuntimeError` |
| Interpreter | `&Program` | `Value` | `RuntimeError` |

### 3.3 No Cross-Domain Leakage
- OS types (`VirtAddr`, `PhysAddr`, `PageFlags`) MUST NOT appear in ML code
- ML types (`TensorValue`, `Tape`) MUST NOT appear in OS code
- The interpreter bridges both through `Value` enum — that's the only coupling point

---

## 4. Performance Rules

### 4.1 Allocation Awareness
- Prefer `&str` over `String` where lifetime allows
- Prefer `&[T]` over `Vec<T>` for read-only access
- Avoid cloning `Value` — pass references where possible
- Use `Cow<'a, str>` for strings that are sometimes owned
- Profile before optimizing — no premature optimization

### 4.2 Benchmark Requirements
- Every performance-critical path must have a criterion benchmark
- No merge if benchmark regresses >10%
- Benchmark categories: lexing, parsing, compilation, execution
- Target: `fibonacci(30)` < 50ms (native), < 500ms (tree-walk)

### 4.3 Binary Size
- Release build with `lto = true`
- Strip debug symbols for release: `strip = true`
- Target: `fj` binary < 10MB
- No unnecessary dependencies — audit with `cargo deny`

---

## 5. Testing Rules

### 5.1 Test-First Development
- Write tests BEFORE implementation (TDD)
- Every function must have at least 1 test
- Every error path must have a test
- Every example program must be a test

### 5.2 Test Naming
```rust
// Pattern: <what>_<when>_<expected>
fn lexer_produces_int_token_for_decimal_literal() { ... }
fn parser_returns_error_for_unclosed_brace() { ... }
fn analyzer_rejects_move_after_use() { ... }
fn codegen_emits_add_instruction_for_plus() { ... }
```

### 5.3 Test Categories
| Category | Location | Runs |
|----------|----------|------|
| Unit | `#[cfg(test)] mod tests` in source | `cargo test --lib` |
| Integration | `tests/*.rs` | `cargo test --test` |
| Property | `tests/property_tests.rs` | `cargo test --test property_tests` |
| Benchmark | `benches/*.rs` | `cargo bench` |
| Example | `examples/*.fj` | CI runs each with expected output |

### 5.4 Coverage Targets
| Component | Minimum | Target |
|-----------|---------|--------|
| Lexer | 95% | 100% |
| Parser | 90% | 100% |
| Analyzer | 90% | 95% |
| Codegen | 85% | 95% |
| VM | 85% | 95% |
| Runtime/OS | 80% | 90% |
| Runtime/ML | 80% | 90% |
| Overall | 85% | 90% |

---

## 6. Embedded ML + OS Specific Rules

### 6.1 Context Safety is Paramount
- @kernel code: NO heap allocation, NO tensor ops, NO floating point (soft-float only)
- @device code: NO raw pointers, NO IRQ handling, NO syscalls
- @safe code: CAN call @kernel and @device functions (bridge)
- @unsafe code: full access (must be audited manually)
- The compiler MUST reject violations at compile time — never at runtime

### 6.2 Tensor Operations
- All tensor ops must be shape-checked at compile time where possible
- Runtime shape errors must include both expected and actual shapes
- Tensor memory layout must be documented (row-major, C-contiguous)
- No implicit broadcasting without explicit `broadcast()` call

### 6.3 OS Primitives
- All memory operations must be bounds-checked
- IRQ handlers must be registered with priority levels
- Syscall numbers must be validated against defined table
- Page table operations must verify alignment

### 6.4 Cross-Domain Bridge Pattern
```fajar
// This is THE design pattern for Fajar Lang
@kernel fn read_sensor() -> [f32; 4] { ... }     // OS domain
@device fn infer(x: Tensor) -> Tensor { ... }     // ML domain
@safe fn bridge() -> Action {                      // bridges both
    let raw = read_sensor()
    let input = Tensor::from_slice(raw)
    let result = infer(input)
    Action::from_prediction(result)
}
```
- Every cross-domain example should demonstrate this pattern
- Tests must verify that the bridge pattern compiles and runs

---

## 7. Dependency Rules

### 7.1 Allowed Dependencies
```toml
# Core (always included)
thiserror = "2"         # Error types
miette = "7"            # Error display

# CLI
clap = "4"              # Argument parsing
rustyline = "14"        # REPL

# ML
ndarray = "0.16"        # Tensor backend
ndarray-rand = "0.15"   # Random tensors

# Codegen (Phase: native compilation)
cranelift = "0.110"     # Native code generation
cranelift-module = "0.110"
cranelift-jit = "0.110"
target-lexicon = "0.12" # Target triple parsing

# FFI
libloading = "0.8"      # Dynamic library loading
libffi = "3"            # C function calling

# Serialization
serde = "1"             # Config files
toml = "0.8"            # fj.toml

# Dev only
proptest = "1.4"        # Property testing
criterion = "0.5"       # Benchmarks
```

### 7.2 Forbidden Dependencies
- No `tokio` in core library (use feature-gated `lsp` feature for LSP server only)
- No `reqwest` or networking (security boundary)
- No `diesel`/`sqlx` or database (out of scope)
- No `rocket`/`actix` or web frameworks (out of scope)
- No `wasm-bindgen` (not an embedded target)

### 7.3 Optional Dependencies (Feature-Gated)
```toml
[features]
default = []
lsp = ["tower-lsp", "tokio"]        # LSP server (dev tool only)
gpu = ["wgpu"]                       # GPU compute (future)
llvm = ["inkwell"]                   # LLVM backend (future)
```

---

## 8. Concurrency Rules (v0.3)

### 8.1 Thread Safety
- ALL shared mutable data MUST be wrapped in `Mutex<T>`, `RwLock<T>`, or `Atomic`
- `Arc<T>` for shared ownership across threads — never raw pointers
- Thread entry functions: must take ownership of captured data, not references
- Thread join: always join or detach — no leaked threads

### 8.2 Atomics & Lock Ordering
- Default memory ordering: `SeqCst` unless explicitly documented otherwise
- Lock acquisition order: alphabetical by variable name to prevent deadlocks
- Atomic operations: prefer `fetch_add`/`fetch_sub` over manual CAS loops
- Channel preference: use channels over shared state when possible

### 8.3 Concurrency in Codegen
- Thread handles tracked in `cx.thread_handles: HashSet<String>`
- Mutex/Arc/Atomic handles tracked similarly for method dispatch
- All concurrency runtime functions: opaque `*mut u8` pointers to Rust structs
- JIT+AOT parity: every runtime function declared in BOTH sections

---

## 9. Inline Assembly Rules (v0.3)

### 9.1 Implementation Strategy
- Cranelift has NO native InlineAsm — use `extern "C"` runtime functions in Rust
- Common patterns (nop, fence, port I/O) implemented as `fj_rt_*` functions
- Raw byte emission deferred to v0.4
- Every `asm!` in runtime functions MUST have `// SAFETY:` comment

### 9.2 Testing
- All inline asm functionality must be tested on actual hardware OR QEMU
- Architecture-specific code: gate with `#[cfg(target_arch = "...")]`
- Provide no-op fallback for unsupported architectures

---

## 10. Bare Metal Rules (v0.3)

### 10.1 No-Std Compatibility
- Bare metal code: no heap allocation, no `std::` imports
- `#[panic_handler]` must be defined for bare-metal targets
- Stack size must be explicitly configured via linker script
- All volatile hardware access via `fj_rt_volatile_read/write`

### 10.2 QEMU Verification
- Every bare metal feature MUST boot on QEMU before marking done
- Test on all three targets: `qemu-system-x86_64`, `qemu-system-aarch64`, `qemu-system-riscv64`
- QEMU flags: `-nographic -serial mon:stdio` for headless testing

---

## 11. GPU Rules (v0.3)

### 11.1 Abstraction Layer
- Primary backend: `wgpu` (cross-platform: Vulkan + Metal + D3D12)
- CUDA FFI: optional, feature-gated for NVIDIA-specific optimizations
- All GPU operations must have CPU fallback — graceful degradation when no GPU
- GPU device selection: prefer discrete GPU, fall back to integrated

### 11.2 Safety
- No raw Vulkan/CUDA calls outside `src/codegen/gpu/` module
- GPU memory allocation tracked and freed on scope exit
- Shader compilation errors: surface as `CodegenError`, not panics

---

## 12. Invariants (Must Always Be True)

1. Every AST node has a valid `Span` (start <= end, within source)
2. EOF token is always last in token stream
3. Lexer produces no whitespace tokens (whitespace is skipped)
4. Parser never returns partial AST (either full Program or errors)
5. Analyzer runs BEFORE interpreter — semantic errors caught at compile time
6. `Value::Null` only appears for void-typed expressions
7. Tensor shape invariant: `shape.iter().product() == data.len()`
8. No overlapping allocated regions in MemoryManager
9. IRQ handlers are always called with interrupts disabled
10. Bytecode programs are always valid (verifier runs before execution)
11. Native code never contains null function pointers
12. @kernel functions never touch heap — enforced by compiler, not convention

---

*V1_RULES.md v1.1 — Updated 2026-03-09 (added v0.3 concurrency, asm, bare metal, GPU rules)*
