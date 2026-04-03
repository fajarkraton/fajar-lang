# CLAUDE.md — Fajar Lang Master Reference

> Auto-loaded by Claude Code on every session. This is the **single source of truth** for all development decisions. Read this FIRST before any action.

---

## 1. Project Identity

- **Project:** Fajar Lang (`fj`) — A statically-typed systems programming language for embedded ML + OS integration
- **File extension:** `.fj`
- **Author:** Fajar (TaxPrime / PrimeCore.id)
- **Model:** Claude Opus 4.6 exclusively
- **Stack:** Rust (interpreter/compiler), ndarray (tensor backend), miette (error display), Cranelift (native codegen — v1.0)
- **Binary name:** `fj`

**Vision:** *"Bahasa terbaik untuk embedded ML + OS integration — the only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*

**Design Principles:**
1. **Explicitness over magic** — no hidden allocation or hidden cost
2. **Dual-context safety** — @kernel disables heap+tensor; @device disables raw pointers. Compiler enforces isolation
3. **Rust-inspired but simpler** — ownership lite without lifetime annotations
4. **Native tensor types** — Tensor is a first-class citizen in the type system, shape checked at compile time

**Target Audience:** Embedded AI engineers (drone, robot, IoT), OS research teams (AI-integrated kernels), Safety-critical ML systems (automotive, aerospace, medical)

---

## 2. Mandatory Session Protocol

Every Claude Code session MUST follow this order:

1. **READ** → `CLAUDE.md` (this file) [auto-loaded]
2. **READ** → `docs/HONEST_AUDIT_V17.md` [CRITICAL: V17 re-audit — true module/CLI status]
3. **READ** → `docs/GAP_ANALYSIS_V2.md` [module-level gap analysis, corrected by V17]
4. **READ** → `docs/V1_RULES.md` [coding conventions — still applies]
5. **ORIENT** → "What does the user want?" Check V17 audit for what's real vs framework.
6. **ACT** → Execute per TDD workflow
7. **VERIFY** → `cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check`
8. **UPDATE** → Mark task `[x]` ONLY if feature works end-to-end. Use `[f]` for framework-only.

### Completion Status (Honest Assessment — V17 Re-Audit, 2026-04-03)

> **Source of truth:** `docs/HONEST_AUDIT_V17.md` — full re-audit of every module and CLI command.
> Previous claims (V13-V15 "100% production") were **inflated by 40-55%**. Corrected below.

**Codebase Reality (56 modules):**
- **33 modules PRODUCTION [x]** — 368K LOC (77%) — user can `fj <command>` and it works
- **1 module PARTIAL [p]** — analyzer: type checking works, @kernel/@device enforcement broken
- **18 modules FRAMEWORK [f]** — 56K LOC (12%) — code exists, not wired to CLI
- **3 modules STUB [s]** — near-empty, superseded or placeholder
- **25/35 CLI commands** production, 8 partial, 2 stub

**Core Compiler (V1-V05): PRODUCTION — verified by code audit.**
- v1.0: 506 tasks — lexer, parser, analyzer, Cranelift, ML runtime ✅
- v0.2: Codegen type system, advanced types ✅
- v0.3: 739 tasks — concurrency, OS runtime, GPU, ML native, self-hosting, packages ✅
- v0.4: 40 tasks — generic enums, RAII/Drop, async, MNIST ✅
- v0.5: 80 tasks — test framework, doc gen, trait objects, iterators, f-strings ✅

**Advanced Features (V06-V12): Mixed production + framework.**
- V06-V07: Core gaps closed, but ~530 tasks were framework-only (types/traits, not E2E)
- V08-V10: Real networking (BLE, MQTT, WebSocket), async tokio, HTTP, regex, LSP ✅
- V11: Website, tutorials, VS Code, benchmarks, self-hosting, borrow checker ✅
- V12: LLVM, package registry, macros, generators, WASI, LSP — 6 options ✅

**V13 "Beyond" (710 tasks): ~390 real [x] (55%), rest framework.**
- Const fn, incremental compilation, WASI P2, FFI v2, SMT verification, distributed, self-hosting

**V14 "Infinity" (500 tasks): ~302 real [x] (60%), not 500/500 as previously claimed.**
- Effects, dependent types, GPU codegen, LSP, package registry, FajarOS Nova

**V15 "Delivery" (120 tasks): ~55 real [x] (46%), not 120/120 as previously claimed.**
- Bug fixes, MNIST pipeline, FFI interop, benchmarks, docs

**V17 Bug Fixes (9 critical): ALL FIXED.** See `docs/HONEST_AUDIT_V17.md` §4.

### Key Documents

| Document | When to Read | Purpose |
|----------|-------------|---------|
| `docs/HONEST_AUDIT_V17.md` | **EVERY SESSION** | V17 re-audit — 33/56 modules production, 9 bugs fixed |
| `docs/GAP_ANALYSIS_V2.md` | **EVERY SESSION** | Module-level gap analysis, corrected by V17 |
| `docs/V1_RULES.md` | Every session | Safety, code quality, architecture rules |
| `docs/V12_TRANSCENDENCE_PLAN.md` | Reference | V12 plan (6 options) |
| `docs/V12_GAP_CLOSURE_PLAN.md` | Reference | 40 tasks that wired V12 into pipeline |
| `docs/V05_PLAN.md` | Reference | v0.5 plan (COMPLETE, verified real) |
| `docs/V04_PLAN.md` | Reference | v0.4 plan (COMPLETE, verified real) |
| `docs/V03_TASKS.md` | Reference | v0.3 tasks (739, COMPLETE, verified real) |
| `docs/V1_TASKS.md` | Reference | v1.0 tasks (506, COMPLETE, verified real) |
| `docs/V1_IMPLEMENTATION_PLAN.md` | Reference | Original 6-month plan (completed) |

---

## 3. Current Status

### Core Compiler (v1.0-v0.5): ALL COMPLETE
- v1.0: 506 tasks (lexer, parser, analyzer, Cranelift, ML runtime) ✅
- v0.2: Codegen type system ✅ | v0.3: 739 tasks (concurrency, GPU, ML, self-hosting) ✅
- v0.4: 40 tasks (generic enums, RAII, async) ✅ | v0.5: 80 tasks (test framework, iterators, f-strings) ✅

### Current Totals (V19 "Precision" COMPLETE, 2026-04-03)

```
Tests:     9,550 default (8,285 lib + 1,265 integ) | ~9,700 with LLVM — 0 failures
LOC:       ~476,000 lines of Rust (441+ files)
Examples:  213 .fj programs | Binary: 13 MB release | MSRV: Rust 1.87
Modules:   ~40 production, 1 partial, 12 framework, 3 stub (56 total)
CLI:       28 production, 5 partial, 2 stub (35 total)
CI:        6 GitHub Actions workflows
Feature Flags: websocket, mqtt, ble, gui, https, native (Cranelift), llvm, registry
```

### V19 "Precision" (2026-04-03) — ALL 6 PHASES COMPLETE (42/42 tasks)
- Plan: `docs/V19_PLAN.md` + `docs/V19_V21_COMPLETE_56_PLAN.md`
- **Phase 1-4:** user macro_rules! with $x metavariable substitution (nested, multi-arg),
  macro_rules! inside blocks, real async_sleep (tokio), async_spawn/join with user functions,
  async_timeout, pattern match destructuring verified E2E (Ok/Err/Some/None)
- **Phase 5:** 4 new .fj demos (database, web, embedded-ml, cli), `fj demo --list` (13 demos),
  LSP v3 QuickFixKind wired into code_action, suggest_cast for SE004,
  const_type_name + const_field_names builtins, map_get_or builtin
- **Phase 6:** fj test verified, fj watch verified, f-strings in all positions
- Integration tests: 148 context_safety + 15 V19 E2E tests
- Examples: `examples/macros.fj`, `examples/pattern_match.fj`, `examples/async_demo.fj`,
  `examples/database_demo.fj`, `examples/web_demo.fj`, `examples/embedded_ml_demo.fj`,
  `examples/cli_tool_demo.fj`

### V18 "Integrity" (2026-04-03) — 35/37 tasks, 18 new real features
- Plan: `docs/V18_HONEST_COMPLETION_PLAN.md`
- New: http_get/post, tcp_connect, dns_resolve, ffi_load_library/call, gen fn + yield,
  channels, @requires, MultiHeadAttention, fj deploy, fj demo, fj build (ELF),
  fj bindgen (FFI output), const fn, LLVM backend fixed, LSP v2 completion
- Context enforcement: 132 integration tests (was 85)

### FajarOS (two platforms)
- **FajarOS v3.0 "Surya"** (ARM64): MMU, EL0, IPC, 65+ commands. Verified on Radxa Dragon Q6A.
- **FajarOS Nova v1.4.0 "Zenith"** (x86_64): 21K lines, 240+ commands, CoW fork, TCP/IP, GPU, SMP, GDB stub. QEMU verified.

---

## 4. Architecture Overview

### 4.1 Compilation Pipeline

```
Source (.fj)
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
    Method: Recursive Descent + Pratt for expressions
    | AST
    v
SEMANTIC ANALYZER (src/analyzer/)     [ACTIVE — integrated into pipeline]
    Input:  &Program
    Output: () or Vec<SemanticError>
    Checks: types, scope, context, mutability
    | analyzed AST
    v
    +-------------------+-------------------+
    |                   |                   |
    v                   v                   v
INTERPRETER         BYTECODE VM         (v1.0) NATIVE COMPILER
(tree-walking)      (45 opcodes)        Cranelift backend
    |                   |                   |
    v                   v                   v
RUNTIME
+-- OS Runtime (memory.rs, irq.rs, syscall.rs, port_io)
+-- ML Runtime (tensor.rs, autograd.rs, ops.rs, optim.rs, metrics.rs)
```

### 4.2 Module Contracts

| Module | Public API | Input -> Output |
|--------|-----------|-----------------|
| Lexer | `tokenize(source: &str)` | `&str` -> `Result<Vec<Token>, Vec<LexError>>` |
| Parser | `parse(tokens: Vec<Token>)` | `Vec<Token>` -> `Result<Program, Vec<ParseError>>` |
| Analyzer | `analyze(program: &Program)` | `&Program` -> `Result<(), Vec<SemanticError>>` |
| Analyzer | `analyze_with_known(prog, names)` | REPL mode with pre-defined names |
| Interpreter | `eval_source(&mut self, src)` | Lex + Parse + Analyze + Eval in one call |
| Interpreter | `eval_program(&mut self, prog)` | `&Program` -> `Result<Value, RuntimeError>` |
| VM | `compile(&Program)` + `vm.run()` | AST -> Bytecode -> Execute |

### 4.3 Top-Level Error Type

```rust
pub enum FjError {
    Lex(Vec<LexError>),
    Parse(Vec<ParseError>),
    Semantic(Vec<SemanticError>),
    Runtime(RuntimeError),
}
```

### 4.4 Value Enum (All Runtime Types)

```rust
pub enum Value {
    Null, Int(i64), Float(f64), Bool(bool), Char(char), Str(String),
    Array(Vec<Value>), Tuple(Vec<Value>), Tensor(TensorValue),
    Map(HashMap<String, Value>),  // HashMap support
    Struct { name: String, fields: HashMap<String, Value> },
    Enum { variant: String, data: Option<Box<Value>> },
    Function(FnValue), BuiltinFn(String), Pointer(PointerValue),
    Optimizer(OptimizerValue), Layer(LayerValue),
}
```

### 4.5 Dependency Direction (STRICT)

```
ALLOWED:  main.rs -> interpreter -> analyzer -> parser -> lexer
          main.rs -> vm -> parser -> lexer
          interpreter -> runtime/os
          interpreter -> runtime/ml
          main.rs -> codegen (v1.0)

FORBIDDEN: lexer -> parser (no upward deps)
           parser -> interpreter
           runtime/os <-> runtime/ml (siblings, no cross-deps)
           Any cycle
```

### 4.6 Key Architectural Details

- `eval_source()` runs full pipeline: lex -> parse -> analyze -> eval
- Analyzer in REPL mode uses `analyze_with_known()` to see prior definitions
- Warnings (SE009 UnusedVariable, SE010 UnreachableCode) do NOT block execution
- `EvalError::Control` is boxed to avoid large_enum_variant clippy warning
- `loss` is a Fajar Lang keyword — cannot use as variable name
- `parse_int`/`parse_float` return `Value::Enum { Ok/Err }`, not RuntimeError

---

## 5. Language Essentials (Quick Reference)

### 5.1 Keywords

```
Control:      if else match while for in return break continue loop
Declarations: let mut fn struct enum impl trait type const
Types:        bool i8-i128 u8-u128 isize usize f32 f64 str char void never
ML:           tensor grad loss layer model
OS:           ptr addr page region irq syscall
Module:       use mod pub extern as
Literals:     true false null
Annotations:  @kernel @device @safe @unsafe @ffi
```

### 5.2 Operator Precedence
19 levels (lowest→highest): Assignment → Pipeline(`|>`) → Logic(`||`,`&&`) → Bitwise → Equality → Comparison → Range → Shift → Add → Mul → Power(`**`) → Cast(`as`) → Unary → Try(`?`) → Postfix(`.`,`()`,`[]`) → Primary. Full table: `docs/GRAMMAR_REFERENCE.md`.

### 5.3 Context Annotations (Unique Feature)

```
@unsafe --> Full access to all features
@kernel --> OS primitives, no heap, no tensor
@device --> Tensor ops, no raw pointer, no IRQ
@safe   --> Default; no hardware, no raw pointer, no direct tensor (safest subset)
```

| Operation | @safe | @kernel | @device | @unsafe |
|-----------|-------|---------|---------|---------|
| `let x = 42` | OK | OK | OK | OK |
| `String::new()` | OK | ERROR KE001 | OK | OK |
| `zeros(3,4)` / `relu()` | ERROR | ERROR KE002 | OK | OK |
| `alloc!(4096)` | ERROR | OK | ERROR DE002 | OK |
| `*mut T` dereference | ERROR | OK | ERROR DE001 | OK |
| `irq_register!()` | ERROR | OK | ERROR DE002 | OK |
| Call `@device` function | OK | ERROR KE003 | OK | OK |
| Call `@kernel` function | OK | OK | ERROR DE002 | OK |

### 5.4 Fajar Lang Syntax Samples

```fajar
// Variables
let x: i32 = 42
let mut counter = 0
const MAX: usize = 1024

// Functions
fn add(a: i32, b: i32) -> i32 { a + b }

// Structs & Enums
struct Point { x: f64, y: f64 }
enum Shape { Circle(f64), Rect(f64, f64) }

// Control flow (expressions!)
let max = if a > b { a } else { b }
let label = match x { 0 => "zero", _ => "other" }

// Pipeline operator
5 |> double |> add_one  // = add_one(double(5))

// Error handling
let val = risky_fn()?   // propagate with ?

// Cross-domain bridge pattern
@kernel fn read_sensor() -> [f32; 4] { ... }
@device fn infer(x: Tensor) -> Tensor { ... }
@safe fn bridge() -> Action {
    let raw = read_sensor()
    let result = infer(Tensor::from_slice(raw))
    Action::from_prediction(result)
}
```

---

## 6. Coding Rules (Non-Negotiable)

> **Full production rules:** `docs/V1_RULES.md`

### 6.1 Core Principles

```
CORRECTNESS > SAFETY > USABILITY > PERFORMANCE
"If it compiles in Fajar Lang, it's safe to deploy on hardware."
```

1. **CORRECTNESS** first — no undefined behavior, no incorrect results
2. **EXPLICIT** over implicit — no hidden behavior
3. **ERRORS are values** — never panic in library code
4. **TESTS before implementation** — TDD always
5. **SMALL functions** — max 50 lines per function
6. **ONE concern per module** — strict single responsibility

### 6.2 Rust Code Style

```
Types/Traits/Enums:  PascalCase     -> TokenKind, FjError
Functions/vars/mods: snake_case     -> tokenize(), token_count
Constants/statics:   SCREAMING_CASE -> MAX_RECURSION_DEPTH
Lifetimes:           short lowercase -> 'src, 'a, 'ctx
Type params:         PascalCase      -> T, U
Error codes:         PREFIX + NUMBER -> SE004, KE001, CE003
```

### 6.3 Error Handling Rules

- **NEVER** use `.unwrap()` in `src/` — only allowed in `tests/` and `benches/`
- **NEVER** `panic!()` in library code — return `Result` or `Option`
- **ALLOWED:** `.expect("reason")` with meaningful message in `main.rs` only
- **USE** `thiserror` for all error types
- **COLLECT** all errors, don't stop at first — show all at once
- **ALL** errors must have error codes and source spans

### 6.4 Safety Rules

- **ZERO** `unsafe {}` blocks outside `src/codegen/` and `src/runtime/os/`
- Every `unsafe` block MUST have `// SAFETY:` comment
- No raw pointer dereference outside `@kernel`/`@unsafe` context

### 6.5 Code Review Checklist (Before Marking Task Done)

- [ ] No `.unwrap()` in `src/` (only in tests)
- [ ] No `unsafe` without `// SAFETY:` comment
- [ ] All `pub` items have doc comments
- [ ] `cargo test` — all pass
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo fmt` — formatted
- [ ] New functions have at least 1 test
- [ ] Task file updated

### 6.6 Documentation Integrity Rules (Non-Negotiable)

These rules exist because of GAP_ANALYSIS_V2 findings. They prevent inflated claims.

1. **[x] means END-TO-END working.** A task is only [x] if a user can actually USE the feature. Type definitions with passing unit tests are `[f]` (framework), not `[x]`.

2. **Every task needs a verification method.** "Verify: send HTTP request and receive response" not "Verify: unit test passes".

3. **No inflated statistics.** Documentation must match actual code capability. Reference GAP_ANALYSIS_V2.md for accurate LOC/status.

4. **No stub plans.** Every option in a plan must have full task tables. No `*(placeholder)*` lines.

5. **Audit before building.** Before creating new plans, verify previous plan claims are backed by real code.

6. **Distinguish real vs framework.** When a module has type definitions but no external integration (no networking, no FFI, no solver calls), document it honestly as "framework — needs X integration".

---

## 7. Error Code System

```
Format: [PREFIX][NUMBER]

LE = Lex Error        (LE001-LE008)  -- 8 tokenization problems
PE = Parse Error      (PE001-PE010)  -- 10 syntax problems
SE = Semantic Error   (SE001-SE012)  -- 12 type/scope problems
KE = Kernel Error     (KE001-KE004)  -- 4 @kernel context violations
DE = Device Error     (DE001-DE003)  -- 3 @device context violations
TE = Tensor Error     (TE001-TE008)  -- 8 shape/type problems
RE = Runtime Error    (RE001-RE008)  -- 8 execution problems
ME = Memory Error     (ME001-ME008)  -- 8 ownership/borrow problems
CE = Codegen Error    (CE001-CE010)  -- 10 native compilation problems (v1.0)

Total: 71 error codes across 9 categories
```

Key errors:
- **SE004** TypeMismatch | **KE001** HeapAllocInKernel | **KE002** TensorInKernel
- **DE001** RawPointerInDevice | **ME001** UseAfterMove | **RE003** StackOverflow

> **Full catalog:** `docs/ERROR_CODES.md`

---

## 8. TDD Workflow (Per Task)

> **Full workflow:** `docs/V1_WORKFLOW.md`

```
+-- 1. THINK   -> Read task from V1_TASKS.md
|               -> Check V1_SKILLS.md for implementation patterns
|
+-- 2. DESIGN  -> Write PUBLIC INTERFACE first (fn signatures, structs, enums)
|
+-- 3. TEST    -> Write tests BEFORE implementation (RED phase)
|
+-- 4. IMPL    -> Write MINIMAL code to make tests pass (GREEN phase)
|
+-- 5. VERIFY  -> cargo test && cargo clippy -- -D warnings && cargo fmt
|
+-- 6. UPDATE  -> Mark task [x] in V1_TASKS.md, move to next task
```

### Quality Gates

**Per-Task:** All tests pass, no unwrap in src, pub items documented, clippy clean
**Per-Sprint:** No regressions, benchmarks compared, at least 1 new example
**Per-Milestone:** Full suite passes, all examples run, cargo doc compiles, release notes

---

## 9. Testing Strategy

### 9.1 Test Suite: 8,317 tests (8,280 lib + 24 integ + 13 doc)

### 9.2 Test Naming Convention

```rust
// Pattern: <what>_<when>_<expected>
fn lexer_produces_int_token_for_decimal_literal() { ... }
fn s1_1_eval_source_runs_analyzer() { ... }
```

### 9.3 Coverage Targets (v1.0)

| Component | Minimum | Target |
|-----------|---------|--------|
| Lexer | 95% | 100% |
| Parser | 90% | 100% |
| Analyzer | 90% | 95% |
| Codegen | 85% | 95% |
| Interpreter | 85% | 95% |
| Runtime | 80% | 90% |
| Overall | 85% | 90% |

---

## 10. Git & Contributing

### 10.1 Branch Strategy

```
main          <- stable releases only (tagged v0.X.Y)
develop       <- integration branch (PR target)
feat/XXX      <- feature branches (1 per sprint task)
fix/XXX       <- bugfix branches
release/v0.X  <- release preparation
```

### 10.2 Commit Convention

```
Format: <type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: lexer, parser, analyzer, interp, runtime, vm, codegen, cli, stdlib

Examples:
  feat(analyzer): integrate analyzer into eval_source pipeline
  fix(analyzer): resolve module-qualified paths in type checker
  test(eval): add S1.1 analyzer integration tests
```

### 10.3 Milestone Tags

```
v0.2.0  -- Month 1  -- Native compilation (Cranelift MVP)        ✅ DONE
v0.3.0  -- Month 2  -- Generics + Traits + FFI                   ✅ DONE
v0.4.0  -- Month 3  -- Ownership system + borrow checker         ✅ DONE
v0.5.0  -- Month 4  -- Full ML runtime + quantization            ✅ DONE
v0.6.0  -- Month 5  -- Cross-compilation + embedded targets      ✅ DONE
v1.0.0  -- Month 6  -- Production release                        ✅ DONE

v0.2    --         -- Codegen type system, self-hosting prep            ✅ DONE
v0.3.0  -- 2026-03-10 -- "Dominion": concurrency, ML, bare metal      ✅ DONE (52 sprints, 739 tasks)
v0.4.0  -- 2026-03-10 -- "Sovereignty": generic enums, RAII, async     ✅ DONE (6 sprints, 40 tasks)
```

---

## 11. Standard Library Overview

| Module | Domain | Key Items |
|--------|--------|-----------|
| `std::io` | General | `print`, `println`, `eprintln`, `read_file`, `write_file`, `append_file`, `file_exists` |
| `std::collections` | General | `Array` (15+ methods), `HashMap` (8 builtins + 7 methods) |
| `std::string` | General | 15 methods: `trim`, `split`, `replace`, `contains`, `starts_with`, `parse_int`, `parse_float`, etc. |
| `std::math` | General | `PI`, `E`, `abs`, `sqrt`, `pow`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `clamp`, `min`, `max` |
| `std::convert` | General | `to_string`, `to_int`, `to_float`, `as` cast |
| `os::memory` | OS | `mem_alloc`, `mem_free`, `mem_read/write`, `page_map/unmap`, `memory_copy/set/compare` |
| `os::irq` | OS | `irq_register`, `irq_unregister`, `irq_enable`, `irq_disable` |
| `os::syscall` | OS | `syscall_define`, `syscall_dispatch` |
| `os::io` | OS | `port_read`, `port_write` |
| `nn::tensor` | ML | `zeros`, `ones`, `randn`, `eye`, `xavier`, `from_data`, `arange`, `linspace` |
| `nn::ops` | ML | `add`, `sub`, `mul`, `div`, `matmul`, `transpose`, `reshape`, `flatten`, `squeeze`, `split`, `concat` |
| `nn::activation` | ML | `relu`, `sigmoid`, `tanh`, `softmax`, `gelu`, `leaky_relu` |
| `nn::loss` | ML | `mse_loss`, `cross_entropy`, `bce_loss`, `l1_loss` |
| `nn::layer` | ML | `Dense`, `Conv2d`, `MultiHeadAttention`, `BatchNorm`, `Dropout`, `Embedding` |
| `nn::autograd` | ML | `backward()`, `grad()`, `requires_grad`, `set_requires_grad` |
| `nn::optim` | ML | `SGD` (lr + momentum), `Adam` (lr), `step()`, `zero_grad()` |
| `nn::metrics` | ML | `accuracy`, `precision`, `recall`, `f1_score` |

**Built-in constructors:** `Some(v)`, `None`, `Ok(v)`, `Err(e)`
**Built-in globals:** `print`, `println`, `len`, `type_of`, `assert`, `assert_eq`, `panic`, `todo`, `dbg`
**Constants:** `PI`, `E`

---

## 12. Security Model Summary

**Philosophy:** "Security by Construction" -- if it compiles, it's safe.

| Pillar | Mechanism | Enforcement |
|--------|-----------|-------------|
| Memory Safety | No use-after-free, no null deref, no buffer overflow | Compiler (ownership + borrow) |
| Context Isolation | @kernel != @device, no heap in kernel, no tensor in kernel | Compiler (context analyzer) |
| Type Safety | PhysAddr != VirtAddr, tensor shape check, no implicit cast | Compiler (type checker) |

Key features: ownership lite (no lifetime annotations), borrow rules (many &T OR one &mut T), null safety (Option<T>), no implicit type conversions, exhaustive match, integer overflow checking.

---

## 13. Performance Targets

Priority: **CORRECTNESS > SAFETY > PERFORMANCE**

| Benchmark | v0.1 (actual) | v1.0 (target) |
|-----------|--------------|---------------|
| Lex 3000 tokens | ~120us | < 50us |
| Parse 300 stmts | ~190us | < 100us |
| fibonacci(20) tree-walk | ~26ms | < 50ms (native) |
| Loop 1000 iterations | ~293us | < 100us (native) |
| String concat 100 | ~73us | < 30us |
| fibonacci(30) | ~500ms | < 50ms (native) |
| Binary size | N/A | < 10MB |

---

## 14. Cargo.toml Dependencies

```toml
[dependencies]
thiserror = "2.0"           # Error types
miette = "7.0"              # Beautiful error display
clap = { version = "4.5", features = ["derive"] }  # CLI
rustyline = "14.0"          # REPL
ndarray = "0.16"            # Tensor backend
ndarray-rand = "0.15"       # Random tensors
serde = { version = "1.0", features = ["derive"] }  # Config
serde_json = "1.0"
toml = "0.8"                # fj.toml
indexmap = "2.0"            # Ordered HashMap
tokio = { version = "1", features = ["full"] }  # LSP server
tower-lsp = "0.20"          # LSP protocol

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"            # Property testing
pretty_assertions = "1.4"

# v1.0 additions (feature-gated):
# cranelift-codegen, cranelift-frontend, cranelift-module
# cranelift-jit, cranelift-object, target-lexicon
# libloading, libffi
```

---

## 15. Key Design Decisions

Interpreter: tree-walking + bytecode VM. Codegen: Cranelift (embedded) + LLVM (production). Tensors: ndarray. Errors: collect-all + miette display. Env: `Rc<RefCell<>>` for closures. Parser: Pratt (19 levels). Generics: monomorphization. Borrow: NLL-like without lifetimes. Full table: see git history.

---

## 16. Quick Commands

```bash
# Build & Run
cargo build                           # build project
cargo build --release                 # release build (optimized)
cargo run -- run examples/hello.fj    # execute Fajar Lang program
cargo run -- repl                     # start REPL (with analyzer)
cargo run -- run --vm examples/hello.fj  # run with bytecode VM
cargo run -- check examples/hello.fj  # type-check only (no execution)

# Testing & Quality
cargo test                            # run default tests (non-native)
cargo test --features native          # run all 2,650 tests (including native codegen)
cargo test --test eval_tests          # run integration tests
cargo test -- s1_1_                   # run sprint-specific tests
cargo clippy -- -D warnings           # linting (MUST pass before commit)
cargo fmt                             # format code
cargo fmt -- --check                  # check formatting

# Documentation & Benchmarks
cargo doc --open                      # generate + view docs
cargo bench                           # run criterion benchmarks

# Project Management
cargo run -- new my_project           # create new Fajar Lang project
cargo run -- build                    # build from fj.toml
cargo run -- fmt file.fj              # format .fj source
cargo run -- lsp                      # start LSP server
cargo run -- gui examples/gui_hello.fj # launch GUI window (--features gui)
cargo run -- dump-tokens file.fj      # inspect lexer output
cargo run -- dump-ast file.fj         # inspect parser output

# Feature-gated builds
cargo run --features websocket -- run file.fj   # real WebSocket (tungstenite)
cargo run --features mqtt -- run file.fj        # real MQTT (rumqttc)
cargo run --features ble -- run file.fj         # real BLE (btleplug)
cargo run --features https -- run file.fj       # HTTPS server (native-tls)
cargo build --features gui                      # GUI windowing (winit)
```

---

## 17. Repository Structure

```
src/
  main.rs (CLI, 5.4K LOC) | lib.rs (module decls)
  lexer/ (tokenize) | parser/ (parse, ast, pratt) | analyzer/ (type_check, scope, effects)
  interpreter/ (eval, env, value) | vm/ (bytecode compiler+engine)
  codegen/ (cranelift/, llvm/, types, abi, linker, analysis)
  runtime/os/ (memory, IRQ, syscall) | runtime/ml/ (tensor, autograd, ops, layers)
  gpu_codegen/ (spirv, ptx, metal, hlsl, fusion, gpu_memory)
  dependent/ (nat, arrays, tensor_shapes, patterns) | verify/ (smt, pipeline, properties)
  lsp/ (server, completion, advanced) | package/ (registry, server, signing, sbom)
  distributed/ | wasi_p2/ | ffi_v2/ | formatter/ | selfhost/
docs/ (44 documents) | tests/ | examples/ (216+ .fj) | fuzz/ (8 targets)
editors/vscode/ | book/ | benches/ | website/ | .github/workflows/ (6 workflows)
```

---

## 18. Document Quick-Reference Index

| When You Need... | Read This |
|---|---|
| **Honest codebase audit** | **`docs/GAP_ANALYSIS_V2.md`** — Tier 1/2/3 breakdown, per-module status |
| **Current plan (V8)** | **`docs/NEXT_IMPLEMENTATION_PLAN_V8.md`** — 810 tasks, Option 0 (Gap Closure) first |
| **Coding rules** | **`docs/V1_RULES.md`** — safety, quality, architecture (still applies) |
| **Completed core tasks** | `docs/V05_PLAN.md` + `docs/V04_PLAN.md` + `docs/V03_TASKS.md` + `docs/V1_TASKS.md` |
| **Implementation plans** | `docs/NEXT_IMPLEMENTATION_PLAN_V{2-8}.md` — all with detailed task tables |
| Language syntax, keywords, types | `docs/FAJAR_LANG_SPEC.md` |
| Formal EBNF grammar | `docs/GRAMMAR_REFERENCE.md` |
| Component contracts, data flow | `docs/ARCHITECTURE.md` |
| Error code catalog | `docs/ERROR_CODES.md` |
| Standard library API | `docs/STDLIB_SPEC.md` |
| Security model | `docs/SECURITY.md` |
| Example programs | `docs/EXAMPLES.md` |
| Git workflow | `docs/CONTRIBUTING.md` |
| OS plans | `docs/V30_PLAN.md` + `docs/COMPILER_ENHANCEMENT_PLAN.md` |

---

## 19. Troubleshooting Quick Reference

| Problem | Solution |
|---------|----------|
| `cargo build` fails: linker not found | `sudo apt-get install build-essential` |
| Test timeout / infinite loop | MAX_RECURSION_DEPTH = 64 (debug) / 1024 (release) |
| Token kind wrong | Use `dbg!(&tokens)` or `fj dump-tokens file.fj` |
| Random test failures | Ensure each test creates fresh `Interpreter::new()` |
| Gradient mismatch | Use epsilon `1e-4`, not exact equality |
| eval_source returns Semantic error | Check that builtins are registered in type_check.rs |
| Module path not found by analyzer | Check Expr::Path resolves qualified name (`mod::fn`) |
| REPL variable not found | `eval_source()` uses `analyze_with_known()` for cross-call state |
| Slow compilation | Use `cargo check` (no codegen) for quick validation |
| Claude forgot context | Re-orient: "Read V1_TASKS.md and find next uncompleted task" |

---

*CLAUDE.md Version: 15.0 | V19 "Precision" COMPLETE — 9,550 tests, ~476K LOC, 0 failures | ~40/56 modules production (71%) | macros, pattern match, async, demos, LSP v3, const reflect*
*Last Updated: 2026-04-03*
