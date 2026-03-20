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
2. **READ** → Current task list (`docs/V06_PLAN.md` for v0.6; check user intent)
3. **READ** → `docs/V1_RULES.md` + `docs/V06_WORKFLOW.md` [coding conventions + workflow]
4. **ORIENT** → "What does the user want?" (v0.5 complete — all planned tasks done)
5. **ACT** → Execute per TDD workflow (see `docs/V06_WORKFLOW.md`)
6. **VERIFY** → `cargo test --features native && cargo clippy -- -D warnings && cargo fmt -- --check`
7. **UPDATE** → Mark task `[x]` in relevant task file if applicable

**v1.0 STATUS: ALL 506 ACTIONABLE TASKS COMPLETE.**
**v0.2 STATUS: Phases A, B, E, F COMPLETE.**
**v0.3 STATUS: "Dominion" — 52 sprints, 739 tasks. ALL COMPLETE.**
**v0.4 STATUS: "Sovereignty" — 6 sprints, 40 tasks. ALL COMPLETE.**
**v0.5 STATUS: "Ascendancy" — 8 sprints, 80 tasks. ALL COMPLETE.**
**v0.6 STATUS: "Horizon" — 28 sprints, ~280 tasks. PLANNING COMPLETE, implementation not started.**

### Key Documents (Read on Demand)

| Document | When to Read | Purpose |
|----------|-------------|---------|
| `docs/V06_PLAN.md` | **Current sprint** | v0.6 "Horizon" plan (28 sprints, ~280 tasks) |
| `docs/V06_WORKFLOW.md` | Session start | Sprint cycle, quality gates, feature gate rules |
| `docs/V06_SKILLS.md` | Before complex tasks | Patterns: LLVM, DAP, BSP, PubGrub, lifetimes, RTOS, LSTM |
| `docs/V05_PLAN.md` | Reference | v0.5 "Ascendancy" plan (8 sprints, COMPLETE) |
| `docs/V04_PLAN.md` | Reference | v0.4 "Sovereignty" plan (6 sprints, COMPLETE) |
| `docs/V03_TASKS.md` | Reference | v0.3 task checkboxes (739 tasks, ALL COMPLETE) |
| `docs/V03_IMPLEMENTATION_PLAN.md` | Reference | 12-month plan: concurrency, OS, GPU, ML, self-hosting |
| `docs/V03_WORKFLOW.md` | Reference | v0.3 sprint cycle, quality gates, rules |
| `docs/V03_SKILLS.md` | Reference | Patterns: fn pointers, threads, async, GPU, bare metal |
| `docs/V1_TASKS.md` | Reference | v1.0 + v0.2 completed tasks (historical) |
| `docs/V1_RULES.md` | Every session | Safety, code quality, architecture rules (still applies) |
| `docs/V1_SKILLS.md` | Reference | Cranelift, monomorphization, borrow checker patterns |
| `docs/V1_IMPLEMENTATION_PLAN.md` | Reference | Original 6-month plan (completed) |

---

## 3. Current Status

### v1.0 — COMPLETE

```
Month 1: FOUNDATION    — Analyzer + Cranelift JIT/AOT               ✅ COMPLETE
Month 2: TYPE SYSTEM   — Generics + Traits + FFI (C interop)        ✅ COMPLETE
Month 3: SAFETY        — Move semantics + NLL borrow checker        ✅ COMPLETE
Month 4: ML RUNTIME    — Autograd + Conv2d/Attention + INT8 quant   ✅ COMPLETE
Month 5: EMBEDDED      — ARM64/RISC-V cross-compile + no_std + HAL  ✅ COMPLETE
Month 6: PRODUCTION    — Docs + package ecosystem + release          ✅ COMPLETE

Tasks:     506 complete | 49 deferred to v0.2 | 0 remaining
Tests:     1,430 default + 133 native codegen = 1,563 total (v1.0 baseline)
LOC:       ~45,000 lines of Rust (v1.0 baseline)
Examples:  15 .fj programs | Benchmarks: 12 criterion
Sprints:   24/26 complete (S11 tensor shapes + S23 self-hosting → v0.2)
Note:      Current totals higher — see v0.3/v0.4 status below
```

### Sprint Progress (All 26)

| Month | Sprints | Status |
|-------|---------|--------|
| 1 — Foundation | S1: Pipeline, CI/CD, modules; S2: Cranelift JIT; S3: control flow; S4: strings, arrays, CLI | ✅ |
| 2 — Type System | S5: generics/mono; S6: traits; S7: FFI/C interop; S8: type inference, enums | ✅ |
| 3 — Safety | S9: move semantics; S10: borrow checker (NLL); S12: overflow/null/bounds; S13: safety audit | ✅ |
| 4 — ML Runtime | S14: autograd/tape; S15: Conv2d/attention/embedding; S16: training/MNIST; S17: INT8 quantization | ✅ |
| 5 — Embedded | S18: cross-compile ARM64/RISC-V; S19: no_std/bare-metal; S20: HAL traits; S21: drone pipeline; S22: testing | ✅ |
| 6 — Production | S24: mdBook docs; S25: package ecosystem; S26: release workflows | ✅ |
| Deferred | S11: tensor shape safety (needs dependent types); S23: self-hosting (needs codegen maturity) | → v0.2 |

### v0.2 — COMPLETE

| Phase | Focus | Status |
|-------|-------|--------|
| A | Codegen type system | ✅ COMPLETE |
| B | Advanced types | ✅ COMPLETE |
| E | Parity/correctness | ✅ COMPLETE |
| F | Production polish | ✅ COMPLETE |
| C | Self-hosting | ✅ Moved to v0.3, COMPLETE |
| D | Dead code elim | ✅ Moved to v0.3, COMPLETE |

### v0.3 "Dominion" — COMPLETE

```
Concurrency:   Threads, channels, mutexes, atomics, async/await     ✅ COMPLETE
Low-level:     Inline asm, volatile I/O, allocators, bare metal     ✅ COMPLETE
GPU:           CUDA + Vulkan backends, SIMD vector types             ✅ COMPLETE
ML Native:     Tensor ops, autograd, training, MNIST 90%+, ONNX     ✅ COMPLETE
Self-hosting:  Lexer + parser in .fj, bootstrap verified             ✅ COMPLETE
Optimization:  Dead fn elim, LICM, CSE, inlining                    ✅ COMPLETE
Tooling:       40-page mdBook, 7 packages, VS Code extension        ✅ COMPLETE

Tasks:     739/739 complete | 0 deferred
Sprints:   52/52 complete
```

### v0.4 "Sovereignty" — COMPLETE

```
Generic enums: Option<T>, Result<T,E> with typed payloads            ✅ COMPLETE
RAII/Drop:     Scope-level cleanup, Drop trait, MutexGuard           ✅ COMPLETE
Future/Poll:   Formal generic enum types, async return checking      ✅ COMPLETE
Lazy async:    State machines, waker, round-robin executor           ✅ COMPLETE

Tasks:     40/40 complete | 0 deferred
Sprints:   6/6 complete
```

### v0.5 "Ascendancy" — COMPLETE

```
Test Framework:    @test/@should_panic/@ignore + fj test CLI          ✅ COMPLETE
Doc Generation:    /// doc comments + fj doc HTML generation          ✅ COMPLETE
Trait Objects:     dyn Trait, vtable dispatch, object safety          ✅ COMPLETE
Iterators:         .iter()/.map()/.filter()/.collect() protocol       ✅ COMPLETE
String Interp:     f"Hello {name}" with expression evaluation         ✅ COMPLETE
Error Recovery:    Multi-error parser, suggestions, type hints        ✅ COMPLETE
Developer Tools:   fj watch, fj bench, REPL multi-line, LSP rename   ✅ COMPLETE

Tasks:     80/80 complete | 0 deferred
Sprints:   8/8 complete
```

### Current Totals

```
Tests:     4,903 lib + 566 integration = 5,469 total (0 failures)
LOC:       ~152,000 lines of Rust (220+ files)
Examples:  126 .fj programs (incl. fajaros_nova_kernel, fajaros_kernel, q6a_showcase)
Packages:  7 standard (fj-math, fj-nn, fj-hal, fj-drivers, fj-http, fj-json, fj-crypto)
Builtins:  90+ bare-metal runtime functions + tensor short aliases
CI:        15 jobs green (Linux/macOS/Windows, stable/nightly, 5 cross targets)
Release:   v3.2.0 "Surya Rising" (2026-03-20)
```

### FajarOS v3.0 "Surya" — OS written 100% in Fajar Lang (ARM64)

```
Features:  MMU, EL0 user space, 10 syscalls, IPC, preemptive scheduler, 65+ shell commands
Hardware:  Verified on Radxa Dragon Q6A (QCS6490) — JIT, GPIO, QNN CPU+GPU inference
Repo:      github.com/fajarkraton/fajar-os
```

### FajarOS Nova v0.2.0 "Perseverance" — x86_64 bare-metal OS (100% Fajar Lang)

```
Kernel:    examples/fajaros_nova_kernel.fj — 7,313 lines, 197KB ELF
Commands:  122 shell commands (system, files, process, AI, network, storage)
Storage:   NVMe driver (admin+IO queues, sector R/W) + FAT32 (mount, ls, cat)
VFS:       / (ramfs), /dev (null/zero/random), /proc (version/uptime), /mnt (fat32)
Network:   Ethernet + ARP + IPv4 + ICMP (ping)
SMP:       AP trampoline (16-bit→64-bit), INIT-SIPI-SIPI, per-CPU tracking
ELF:       ELF64 parser, PT_LOAD loader, 8 syscalls (exit, write, read, mmap, etc.)
Plan:      docs/FAJAROS_NOVA_V2_PLAN.md — ALL 6 PHASES COMPLETE
Test:      QEMU verified: NVMe + FAT32 + VFS + NET + Syscall
Target:    Intel Core i9-14900HX (Lenovo Legion Pro)
```

> **Task lists:** `docs/V05_PLAN.md` (v0.5), `docs/V03_TASKS.md` (v0.3), `docs/V04_PLAN.md` (v0.4), `docs/V1_TASKS.md` (v1.0)
> **Implementation plans:** `docs/V03_IMPLEMENTATION_PLAN.md`, `docs/V1_IMPLEMENTATION_PLAN.md`
> **OS plans:** `docs/V30_PLAN.md`, `docs/COMPILER_ENHANCEMENT_PLAN.md`, `docs/NEXT_STEPS_PLAN.md`

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

### 5.2 Operator Precedence (lowest -> highest, 19 levels)

| Level | Name | Operators | Assoc |
|-------|------|-----------|-------|
| 1 | Assignment | `= += -= *= /= %= &= \|= ^= <<= >>=` | Right |
| 2 | Pipeline | `\|>` | Left |
| 3 | Logical OR | `\|\|` | Left |
| 4 | Logical AND | `&&` | Left |
| 5 | Bitwise OR | `\|` | Left |
| 6 | Bitwise XOR | `^` | Left |
| 7 | Bitwise AND | `&` | Left |
| 8 | Equality | `== !=` | Left |
| 9 | Comparison | `< > <= >=` | Left |
| 10 | Range | `.. ..=` | None |
| 11 | Bit Shift | `<< >>` | Left |
| 12 | Addition | `+ -` | Left |
| 13 | Multiply | `* / % @` | Left |
| 14 | Power | `**` | Right |
| 15 | Type Cast | `as` | Left |
| 16 | Unary | `! - ~ & &mut` | Right |
| 17 | Try | `?` | Postfix |
| 18 | Postfix | `. () [] .method()` | Left |
| 19 | Primary | Literals, idents | - |

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
- [ ] `docs/V1_TASKS.md` updated

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

### 9.1 Current Test Suite (2,650 tests)

| Category | Location | Count | Description |
|----------|----------|-------|-------------|
| Unit + Native | `#[cfg(test)] mod tests` | 2,267 | Per-function tests (incl ~700 native codegen with `--features native`) |
| Integration | `tests/eval_tests.rs` | 181 | Full pipeline (lex -> parse -> analyze -> eval) |
| ML | `tests/ml_tests.rs` | 39 | Tensor ops, autograd, optimizers, layers |
| OS | `tests/os_tests.rs` | 16 | Memory, IRQ, syscall, port I/O |
| Autograd | `tests/autograd_tests.rs` | 13 | Numerical gradient checks |
| Property | `tests/property_tests.rs` | 33 | proptest invariants |
| Safety | `tests/safety_tests.rs` | 76 | Move, borrow, overflow, bounds, type errors |
| Cross | `tests/cross_compile_tests.rs` | 9 | ARM64 + RISC-V cross-compilation |
| Doc | `src/**/*.rs` | 8 | Doctest examples |

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

| # | Decision | Rationale | Status |
|---|----------|-----------|--------|
| 1 | Tree-walking interpreter | Simplest path to working v0.1 | DONE |
| 2 | Bytecode VM (45 opcodes) | Faster than tree-walk | DONE |
| 3 | Cranelift for native codegen | Lighter than LLVM, good embedded support | DONE (S2) |
| 4 | ndarray for tensors | Mature, SIMD via BLAS | DONE |
| 5 | Collect-all errors | Show all errors at once like Rust | DONE |
| 6 | `Rc<RefCell<>>` for env | Closures need shared mutable parent scope | DONE |
| 7 | miette for errors | Beautiful Rust-style error output | DONE |
| 8 | Pratt parser for exprs | Elegant precedence handling (19 levels) | DONE |
| 9 | Monomorphization for generics | Static dispatch, no vtables, embedded-friendly | DONE (S5) |
| 10 | NLL-like borrow checker | Simpler than Rust (no lifetime annotations) | DONE (S10) |
| 11 | INT8 quantization | Embedded inference without FPU | DONE (S17) |
| 12 | Analyzer in eval_source | Catch errors before execution, REPL-aware | DONE (S1.1) |
| 13 | pub visibility enforcement | Module privacy with backward compat (legacy = all public) | DONE (S1.4) |
| 14 | Contextual keywords | OS/ML keywords usable as identifiers in params/exprs | DONE |
| 15 | Trait body-less methods | Signature-only methods in traits (empty block default) | DONE (S20) |

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
cargo run -- dump-tokens file.fj      # inspect lexer output
cargo run -- dump-ast file.fj         # inspect parser output
```

---

## 17. Repository Structure

```
fajar-lang/
+-- CLAUDE.md                 <- YOU ARE HERE (Master reference)
+-- Cargo.toml
+-- Cargo.lock
|
+-- docs/                     <- 44 documents
|   +-- V04_PLAN.md               <- v0.4 "Sovereignty" (6 sprints, COMPLETE)
|   +-- V03_TASKS.md              <- v0.3 task checkboxes (739 tasks, ALL COMPLETE)
|   +-- V03_IMPLEMENTATION_PLAN.md <- v0.3 plan (52 sprints, 12 months)
|   +-- V03_WORKFLOW.md           <- v0.3 sprint workflow
|   +-- V03_SKILLS.md             <- v0.3 implementation patterns
|   +-- V1_IMPLEMENTATION_PLAN.md <- v1.0 plan (26 sprints, COMPLETE)
|   +-- V1_TASKS.md              <- v1.0 task checkboxes (506 tasks, COMPLETE)
|   +-- V1_RULES.md              <- Coding rules (production grade, still applies)
|   +-- V1_WORKFLOW.md           <- TDD workflow
|   +-- V1_SKILLS.md             <- Cranelift, monomorphization patterns
|   +-- FAJAR_LANG_SPEC.md       <- Language spec & grammar (AUTHORITATIVE)
|   +-- ARCHITECTURE.md          <- System design & contracts
|   +-- GRAMMAR_REFERENCE.md     <- Formal EBNF grammar
|   +-- ERROR_CODES.md           <- 71 error codes across 9 categories
|   +-- STDLIB_SPEC.md           <- Standard library API
|   +-- SECURITY.md              <- Security model
|   +-- CHANGELOG.md             <- Version history (v0.3.0, v0.4.0)
|   +-- GAP_ANALYSIS.md          <- Cross-doc conflict audit
|   +-- ROADMAP_V1.1.md          <- Future roadmap (GPU, LLVM, etc.)
|   +-- (and more...)
|
+-- src/                      <- ~98,000 LOC across 97 .rs files
|   +-- lib.rs                <- Module decls + FjError + FjDiagnostic
|   +-- main.rs               <- CLI entry point (clap: run, repl, check, build, fmt, lsp, new)
|   +-- lexer/
|   |   +-- mod.rs            <- pub fn tokenize()
|   |   +-- token.rs          <- Token, TokenKind (82+ kinds), Span
|   |   +-- cursor.rs         <- Cursor struct (peek, advance, is_eof)
|   +-- parser/
|   |   +-- mod.rs            <- pub fn parse() (4,520 LOC)
|   |   +-- ast.rs            <- Expr (25+ variants), Stmt, Item, TypeExpr, Pattern
|   |   +-- pratt.rs          <- Pratt expression parser (19 precedence levels)
|   +-- analyzer/
|   |   +-- mod.rs            <- pub fn analyze(), analyze_with_known()
|   |   +-- type_check.rs     <- TypeChecker (6,616 LOC — types, scope, context, NLL)
|   |   +-- scope.rs          <- SymbolTable, Scope, ScopeKind
|   |   +-- cfg.rs            <- NLL control flow analysis
|   |   +-- borrow_lite.rs    <- Ownership/move/borrow analysis
|   +-- interpreter/
|   |   +-- mod.rs            <- Interpreter struct
|   |   +-- env.rs            <- Environment (scope chain, Rc<RefCell<>>)
|   |   +-- eval.rs           <- eval_expr, eval_stmt, eval_source (5,737 LOC)
|   |   +-- value.rs          <- Value enum (17 variants)
|   |   +-- ffi.rs            <- C interop
|   +-- vm/
|   |   +-- compiler.rs       <- AST -> Bytecode compiler
|   |   +-- engine.rs         <- Bytecode VM (45 opcodes)
|   +-- codegen/              <- Cranelift native backend (~40K LOC)
|   |   +-- cranelift/
|   |   |   +-- mod.rs        <- CraneliftCompiler + ObjectCompiler (10,242 LOC)
|   |   |   +-- tests.rs      <- ~700 native codegen tests (14,991 LOC)
|   |   |   +-- runtime_fns.rs <- 150+ extern "C" fj_rt_* functions (7,115 LOC)
|   |   |   +-- context.rs    <- CodegenCtx (56 fields)
|   |   |   +-- closures.rs   <- Free var analysis
|   |   |   +-- generics.rs   <- Monomorphization
|   |   |   +-- compile/
|   |   |       +-- mod.rs    <- Core expr/stmt/call + builtins (6,113 LOC)
|   |   |       +-- expr.rs   <- Expression compilation
|   |   |       +-- control.rs <- if/while/loop/for/match
|   |   |       +-- stmt.rs   <- Statement compilation
|   |   |       +-- builtins.rs <- Math, assert, file builtins
|   |   |       +-- arrays.rs <- Array operations
|   |   |       +-- strings.rs <- String operations
|   |   |       +-- structs.rs <- Struct operations
|   |   +-- types.rs          <- Type -> Cranelift mapping
|   |   +-- abi.rs            <- ABI handling
|   |   +-- linker.rs         <- Linker script generation
|   |   +-- analysis.rs       <- Dead code elimination
|   +-- runtime/
|   |   +-- os/               <- 20+ files: memory, IRQ, syscall, paging, GDT/IDT, serial, VGA
|   |   +-- ml/               <- tensor, autograd, ops, optim, layers, metrics, quantize, ONNX
|   +-- formatter/            <- fj fmt (AST-based pretty-printing)
|   +-- lsp/                  <- Language Server Protocol (tower-lsp)
|   +-- package/              <- fj.toml, fj new, fj build, registry
|   +-- stdlib/               <- Rust-side stdlib bindings
|
+-- stdlib/                   <- Fajar Lang stdlib (.fj: core, nn, os, hal, drivers, lexer)
+-- packages/                 <- 7 standard packages (fj-math/nn/hal/drivers/http/json/crypto)
+-- tests/
|   +-- eval_tests.rs         <- 181 integration tests
|   +-- ml_tests.rs           <- 39 ML tests
|   +-- os_tests.rs           <- 16 OS tests
|   +-- autograd_tests.rs     <- 13 autograd gradient checks
|   +-- property_tests.rs     <- 33 proptest invariants
|   +-- safety_tests.rs       <- 76 safety tests (move/borrow/overflow/bounds)
|   +-- cross_compile_tests.rs <- 9 ARM64/RISC-V cross-compilation tests
+-- examples/                 <- 24 example .fj programs
+-- editors/vscode/           <- VS Code extension (syntax, snippets, LSP)
+-- book/                     <- mdBook documentation (40+ pages)
+-- benches/
    +-- interpreter_bench.rs  <- Interpreter benchmarks
    +-- embedded_bench.rs     <- Embedded benchmarks
    +-- concurrency_bench.rs  <- Concurrency benchmarks (native)
```

---

## 18. Document Quick-Reference Index

| When You Need... | Read This |
|---|---|
| **v0.6 plan (CURRENT)** | **`docs/V06_PLAN.md`** — 28 sprints, ~280 tasks, 7 phases |
| **v0.6 implementation patterns** | **`docs/V06_SKILLS.md`** — LLVM, DAP, BSP, PubGrub, lifetimes, RTOS, LSTM |
| **v0.6 development workflow** | **`docs/V06_WORKFLOW.md`** — sprint cycle, quality gates, feature gates |
| **Completed task lists** | `docs/V05_PLAN.md` (v0.5) + `docs/V04_PLAN.md` (v0.4) + `docs/V03_TASKS.md` (v0.3) |
| **Coding rules** | **`docs/V1_RULES.md`** |
| **Previous patterns** | `docs/V03_SKILLS.md` + `docs/V1_SKILLS.md` |
| **Future roadmap** | **`docs/ROADMAP_V1.1.md`** |
| Language syntax, keywords, types | `docs/FAJAR_LANG_SPEC.md` |
| Formal EBNF grammar | `docs/GRAMMAR_REFERENCE.md` |
| Component contracts, data flow | `docs/ARCHITECTURE.md` |
| Error code catalog | `docs/ERROR_CODES.md` |
| Standard library API | `docs/STDLIB_SPEC.md` |
| Security model | `docs/SECURITY.md` |
| Example programs | `docs/EXAMPLES.md` |
| Git workflow | `docs/CONTRIBUTING.md` |
| Performance targets | `docs/BENCHMARKS.md` |

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

*CLAUDE.md Version: 5.0 | v0.4 COMPLETE — 2,650 tests, ~98K LOC, 0 failures | Auto-loaded by Claude Code*
*Last Updated: 2026-03-10*
