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

Every session: **READ** `CLAUDE.md` + `docs/HONEST_STATUS_V26.md` → **ORIENT**
on what user wants vs what's real → **ACT** per TDD workflow (§8) → **VERIFY**
`cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check` →
**UPDATE** task to `[x]` only if E2E works (use `[f]` for framework-only).

### Completion Status (V26, 2026-04-11)

**54 modules: 54 [x] / 0 [sim] / 0 [f] / 0 [s].** Zero framework, zero stubs.
Every public mod has a callable surface from `.fj` or `fj` CLI. 23 CLI subcommands,
all production.

> **Source of truth:** `docs/HONEST_STATUS_V26.md` — current per-module status.
> Audit trail: `docs/HONEST_AUDIT_V26.md`. Older snapshots: `HONEST_STATUS_V20_5.md`,
> `HONEST_AUDIT_V17.md`. Historical V13-V15 "100% production" claims were inflated
> 40-55% per V17 re-audit; V26 closed the remaining gap.

**Core compiler (v1.0 → v0.5):** ALL COMPLETE — 506 + 739 + 40 + 80 + 130 tasks across
lexer, parser, analyzer, Cranelift, ML runtime, concurrency, OS runtime, generic enums,
RAII, async, test framework, iterators, f-strings.

**V06-V26 history:** see §3 Version History table or `CHANGELOG.md` (root) for
detailed entries.

**V17 critical bugs (9):** ALL FIXED. See `docs/HONEST_AUDIT_V17.md` §4 for the list.

### Key Documents

- **`docs/HONEST_STATUS_V26.md`** — read every session, source of truth for module status
- **`docs/V26_PRODUCTION_PLAN.md`** — current 6-week plan (v1.1 with §10.5 Plan Hygiene)
- **`docs/HONEST_AUDIT_V26.md`** — V26 hands-on verification, corrections to prior counts
- `docs/HONEST_AUDIT_V17.md` — historical baseline re-audit
- `docs/V1_RULES.md` — coding conventions (mostly subsumed by §6 below)
- `docs/V0{1..5}_*.md`, `docs/V1_TASKS.md` — completed task plans (reference only)
- See §18 for full document index.

---

## 3. Current Status

### Core Compiler (v1.0-v0.5): ALL COMPLETE
- v1.0: 506 tasks (lexer, parser, analyzer, Cranelift, ML runtime) ✅
- v0.2: Codegen type system ✅ | v0.3: 739 tasks (concurrency, GPU, ML, self-hosting) ✅
- v0.4: 40 tasks (generic enums, RAII, async) ✅ | v0.5: 80 tasks (test framework, iterators, f-strings) ✅

### Current Totals (V26 "Final" partial, 2026-04-11)

```
Tests:     7,581 lib + 2,374 integ (in 46 test files) + 14 doc + 1 ignored
           ≈ 9,969 total | 0 failures, 0 flakes
           Stress: 80/80 consecutive runs at `cargo test --lib -- --test-threads=64`
LOC:       ~446,000 lines of Rust (394 files in src/)
Examples:  231 .fj programs in examples/ (was 228, +3 V26 const_*+gui demos)
           Binary: 14 MB release | MSRV: Rust 1.87
Modules:   42 lib.rs pub mods | 54 [x], 0 [sim], 0 [f], 0 [s] (54 logical)
           Source of truth: docs/HONEST_STATUS_V26.md
           V26 Phase A3 closed all 5 framework + 2 stub modules. 0 remaining.
CLI:       23 subcommands declared in src/main.rs, all production
CI:        6 GitHub Actions workflows + new flake-stress job (V26 A1.4)
Feature Flags: websocket, mqtt, ble, gui, https, native (Cranelift), llvm (30 enhancements), registry, cuda
Quality:   0 clippy warnings | 0 production .unwrap() (verified by scripts/audit_unwrap.py)
           0 fmt diffs | 0 test failures (7,581/7,581) | 0 flakes (80 stress runs)
Threading: Real std::thread actors + Arc<Mutex> throughout interpreter
GPU:       RTX 4090 CUDA (9 PTX kernels, tiled matmul, async streams, 3x speedup)
Hooks:     Pre-commit rejects fmt drift (scripts/git-hooks/pre-commit, V26 A1.2)

Labeling: [x] = production (tested, works E2E)
          [sim] = simulated — NONE REMAINING (all upgraded to [x] in V21)
          [f] = framework (code exists, not callable from .fj)
          [s] = stub (near-empty placeholder)

Numbers verified by runnable commands as of 2026-04-11. CLAUDE.md no longer
trusts inflated counts. Audit corrections in V26:
  - prior 11,395 tests was inflated; real is 7,581 lib + 2,374 integ + 14 doc
  - prior 285 examples was inflated; real is 231
  - prior "0 unwraps" was aspirational; real before V26 was 3 (now 0)
  - prior "5 [f] modules" was outdated; A3 closed all (now 0)
  - prior "2 [s] modules" was outdated; V24+V20.8 closed all (now 0)
  - prior "8 const_* modules" was inflated; real is 3
```

### Version History (V18 → V26)

> **Detailed entries:** `CHANGELOG.md` (root) — has V20.8 → V26 with full
> Added/Changed/Fixed/Removed/Stats sections. V18-V20 history lives in
> git log (`git log --oneline --grep="V1[89]\|V20"`).

| Version | Date | Highlight |
|---|---|---|
| **V26** "Final" (Phase A) | 2026-04-11 | 80/80 stress, 0 unwraps, 0 [f], 0 [s], pre-commit hook, §6.7 rule |
| V25 "Production" | 2026-04-07 | Hands-on re-audit, K8s deploy, FajarQuant Phase C real Gemma 4 E2B, @kernel transitive fix |
| V24 "Quantum" | 2026-04-07 | CUDA RTX 4090 (9 PTX kernels, ~3x matmul), AVX2 + AES-NI inline asm, FajarQuant Phase 5-7 |
| V23 "Boot" | 2026-04-06 | FajarOS boots to shell, 16 LLVM/runtime fixes, NVMe + GUI + ACPI working |
| V22 "Hardened" | 2026-04-06 | 30 LLVM enhancements (E1-I6 batches), 690→0 codegen errors |
| V21 "Production" | 2026-04-04 | Real threaded actors (std::thread + mpsc), 5 [sim]→[x], LLVM JIT/AOT runtime |
| V20.8 "Cleanup" | 2026-04-04 | Rc→Arc migration, removed 21.4K LOC dead code (rtos/iot/rt_pipeline/etc) |
| V20 "Completeness" | 2026-04-03 | Debugger v2 record/replay, package v2 build scripts, accelerator dispatch |
| V19 "Precision" | 2026-04-03 | macro_rules! with $x metavar, async_sleep tokio, pattern match destructure E2E |
| V18 "Integrity" | 2026-04-03 | http/tcp/dns, ffi_load, gen+yield, @requires, MultiHeadAttention, const fn |

### FajarOS (two platforms)
- **FajarOS v3.0 "Surya"** (ARM64): Verified on Radxa Dragon Q6A. 65+ commands.
- **FajarOS Nova** (x86_64): 47,821 LOC, V26 LLM E2E (SmolLM-135M v5/v6), 14 LLM shell commands. Boot to `nova>` reliably in QEMU.

---

## 4. Architecture Overview

> **Full architecture:** `docs/ARCHITECTURE.md` — module contracts, data flow, dependency graph.

### 4.1 Compilation Pipeline (one-line summary)

`source.fj → lexer → parser → analyzer → {interpreter | vm | cranelift | llvm} → {os runtime | ml runtime}`

- **Lexer** (`src/lexer/`): `&str → Vec<Token>` (LE001-LE008)
- **Parser** (`src/parser/`): `Vec<Token> → Program` (recursive descent + Pratt, 19 levels)
- **Analyzer** (`src/analyzer/`): `&Program → Result<(), Vec<SemanticError>>` (types, scope, @kernel/@device contexts)
- **Backends:** tree-walking interpreter, bytecode VM (45 opcodes), Cranelift (embedded), LLVM (production w/ 30 enhancements)

### 4.2 Top-Level Types

```rust
enum FjError { Lex, Parse, Semantic, Runtime }
enum Value { Null, Int, Float, Bool, Char, Str, Array, Tuple, Tensor,
             Map, Struct, Enum, Function, BuiltinFn, Pointer, Optimizer, Layer }
```

### 4.3 Dependency Direction (STRICT)

`main → interpreter → analyzer → parser → lexer` ; `interpreter → runtime/{os,ml}` ; `main → codegen`. **Forbidden:** any upward dep, parser → interpreter, runtime/os ↔ runtime/ml, any cycle.

### 4.4 Key Architectural Details

- `eval_source()` runs full pipeline; REPL uses `analyze_with_known()` for prior names
- Warnings (SE009 UnusedVariable, SE010 UnreachableCode) do NOT block execution
- `EvalError::Control` is boxed (avoids large_enum_variant clippy warning)
- `loss` is a keyword — cannot use as variable name
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
- [ ] No wall-clock `assert!(elapsed < threshold)` in unit tests (see §6.7)
- [ ] All `pub` items have doc comments
- [ ] `cargo test` — all pass
- [ ] `cargo test --lib -- --test-threads=64` — passes 5x in a row (stress test)
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

### 6.7 Test Hygiene Rules (No Wall-Clock Assertions in Unit Tests)

> **Reason:** V26 A1.3 found 14 tests asserting `elapsed < threshold` on
> microsecond-scale work. They flaked under `cargo test --test-threads=64`
> because scheduler jitter parks threads for 100s of ms — far above the
> assertion threshold. Pre-fix flake rate was ~20% per full test run.
> Fixed by 10x threshold bump + noise floor (commit `13aa9e3`).

1. **NEVER** write `assert!(elapsed < N_ms)` in unit tests when the work
   measured is microsecond-scale or contains a no-op simulation. Wall-clock
   timing is unreliable under parallel test load.

2. **DO** put performance regression detection in **criterion benchmarks**
   under `benches/`, not unit tests. Criterion handles statistical noise.

3. **IF** a unit test must check timing (e.g., for asynchronous behavior),
   set the threshold to **at least 10x** the actual expected value, OR use
   a noise floor pattern that treats sub-millisecond differences as passing.

4. **CI safeguard:** the `flake-stress` job in `.github/workflows/ci.yml`
   runs `cargo test --lib -- --test-threads=64` 5x to catch new wall-clock
   flakes before they're observed in production.

5. **Antipattern example (DO NOT WRITE):**
   ```rust
   #[test]
   fn fast_operation_is_fast() {
       let start = Instant::now();
       compute_thing();              // takes microseconds
       assert!(start.elapsed() < Duration::from_millis(50));  // FLAKY
   }
   ```

6. **Acceptable pattern:**
   ```rust
   #[test]
   fn fast_operation_is_fast() {
       let start = Instant::now();
       compute_thing();
       // Target: <50ms; test allows <500ms (10x) for parallel jitter immunity
       assert!(start.elapsed() < Duration::from_millis(500));
   }
   ```

### 6.8 Plan Hygiene Rules (No Inflated Estimates, No Skipped Decisions)

> **Reason:** V26 Phase A1+A2+A3 surfaced 6 systemic patterns that, if left
> unchecked, distort future plans the same way prior plans were distorted.
> A2.1 found "174 unwraps" was actually **3** (58× inflation). A1.3 found
> "1 flaky test" was actually **14** (14× scope expansion). These are not
> outliers — they are the default outcome of trusting baselines without
> hands-on verification. Full evidence: `docs/V26_PRODUCTION_PLAN.md` §10.5.

When writing or reviewing any plan, audit, or status doc:

1. **Pre-flight audit mandatory.** Every Phase starts with a B0/C0/D0
   subphase that hands-on verifies the baseline via runnable commands
   and produces a `docs/V26_<phase>_FINDINGS.md`. Downstream subphases
   cannot start until findings are committed. (Lesson: A2.1 inflated
   174→3; A3 found 5 [f] modules already deleted.)

2. **Verification columns must be runnable commands.** Every task table
   has a "Verification" column. That column must contain a literal command
   whose output can be checked, not prose like "test passes" or "feature
   works". (Lesson: CLAUDE.md drift 11,395 tests claim → real 7,581.)

3. **Prevention layer per phase.** Every fix that closes a class of bugs
   must spawn at least one prevention mechanism: a pre-commit hook, a
   CI job, or a CLAUDE.md rule. One-time fixes are forbidden — the
   prevention layer is the deliverable, not the patch. (Lesson: A1.2
   added pre-commit hook; A1.4 added flake-stress CI + §6.7.)

4. **Multi-agent audit cross-check mandatory.** Numbers produced by
   parallel sub-agents must be manually re-verified with `Bash` before
   being committed. Single-source agent claims are inadmissible.
   (Lesson: V26 audit agent claimed "4,062 unwraps" + "no LLM shell
   commands" — both wrong by huge factors.)

5. **Surprise budget +25% minimum, tracked per commit.** Every Phase
   allocates an explicit surprise budget. Default +25%; high-uncertainty
   phases use +30%. Commit messages tag actual variance:
   `feat(v26-b1): fork() PID return [actual 3h, est 2h, +50%]`. If
   average variance exceeds budget, the next Phase escalates to +40%.
   (Lesson: A1.3 hypothesized 1 flaky test, found 14.)

6. **Decision gates must be mechanical.** "Decision required before X"
   prose markers get skipped under execution pressure. Every decision
   must produce a committed file (e.g., `docs/V26_B5_DECISION.md`)
   that pre-commit hooks check, mechanically blocking downstream work
   until the file exists. (Lesson: A1.4 mechanical CI job worked where
   prose hadn't.)

**Self-check before any plan/audit commit:**
```
[ ] Pre-flight audit (B0/C0/D0) exists for the Phase?         (Rule 1)
[ ] Every task has a runnable verification command?           (Rule 2)
[ ] At least one prevention mechanism added (hook/CI/rule)?   (Rule 3)
[ ] Agent-produced numbers cross-checked with Bash?           (Rule 4)
[ ] Effort variance tagged in commit message?                 (Rule 5)
[ ] Decisions are committed files, not prose paragraphs?      (Rule 6)
```
Six NO answers = revert. Six YES answers = ship.

---

## 7. Error Code System

```
Format: [PREFIX][NUMBER]

LE = Lex Error        (LE001-LE008)     --  8 tokenization problems
PE = Parse Error      (PE001-PE010)     -- 10 syntax problems
SE = Semantic Error   (SE001-SE016)     -- 16 type/scope problems
KE = Kernel Error     (KE001-KE004)     --  4 @kernel context violations
DE = Device Error     (DE001-DE003)     --  3 @device context violations
TE = Tensor Error     (TE001-TE009)     --  9 shape/type problems
RE = Runtime Error    (RE001-RE008)     --  8 execution problems
ME = Memory Error     (ME001-ME010)     -- 10 ownership/borrow problems
CE = Codegen Error    (CE001-CE010)     -- 10 native compilation problems

Total: 78 error codes across 9 categories (verified by grep on docs/ERROR_CODES.md)
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

### 9.1 Test Suite: ~9,969 tests (7,581 lib + 2,374 integ in 46 files + 14 doc)

> Numbers verified 2026-04-11 via `cargo test --lib`, `ls tests/*.rs | wc -l`,
> `grep -h '^#\[test\]' tests/*.rs | wc -l`, `cargo test --doc`. Stress test
> (V26 A1.4) runs `cargo test --lib -- --test-threads=64 × 5` per push.

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

> **Full guide:** `docs/CONTRIBUTING.md`. CHANGELOG: root `CHANGELOG.md`.

- **Branches:** `main` (stable, tagged) | `feat/XXX` | `fix/XXX` | `release/vX.Y`
- **Commits:** `<type>(<scope>): <desc>` — types: feat/fix/test/refactor/docs/perf/ci/chore; scopes: lexer/parser/analyzer/interp/runtime/vm/codegen/cli/stdlib + V26 phase scopes (`v26-a1`..`v26-c4`)
- **Milestones:** v0.2-v1.0 (6 monthly Cranelift+ML+ownership) ✅ DONE; v0.3 "Dominion", v0.4 "Sovereignty", v0.5 "Apex" ✅ DONE

---

## 11. Standard Library Overview

> **Full API:** `docs/STDLIB_SPEC.md`. Discover dynamically via REPL `:help` or grep `src/interpreter/builtins.rs`.

- **`std::io`**: print, println, eprintln, read_file, write_file, append_file, file_exists
- **`std::collections`**: `Array` (15+ methods), `HashMap` (8 builtins + 7 methods)
- **`std::string`**: 15 methods (trim, split, replace, contains, starts_with, parse_int, parse_float, …)
- **`std::math`**: PI, E, abs, sqrt, pow, sin/cos/tan, floor, ceil, round, clamp, min, max
- **`std::convert`**: to_string, to_int, to_float, `as` cast
- **`os::*`**: memory (alloc/free/read/write, page_map/unmap), irq (register/enable), syscall, io (port_read/write)
- **`nn::tensor`**: zeros, ones, randn, eye, xavier, from_data, arange, linspace
- **`nn::ops`**: add, sub, mul, div, matmul, transpose, reshape, flatten, squeeze, split, concat
- **`nn::activation`**: relu, sigmoid, tanh, softmax, gelu, leaky_relu
- **`nn::loss`**: mse_loss, cross_entropy, bce_loss, l1_loss
- **`nn::layer`**: Dense, Conv2d, MultiHeadAttention, BatchNorm, Dropout, Embedding
- **`nn::autograd`**: backward, grad, requires_grad, set_requires_grad
- **`nn::optim`**: SGD (lr + momentum), Adam (lr), step, zero_grad
- **`nn::metrics`**: accuracy, precision, recall, f1_score

**Built-in:** `Some/None/Ok/Err` constructors; `print/println/len/type_of/assert/assert_eq/panic/todo/dbg` globals; `PI/E` constants.

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

> **Source of truth:** `Cargo.toml` itself. Keys: `thiserror` (errors), `miette` (display),
> `clap` (CLI), `ndarray` (tensors), `tokio` + `tower-lsp` (LSP), `cranelift-*` + `inkwell` (codegen,
> feature-gated), `criterion` (benches), `proptest` (property tests).

---

## 15. Key Design Decisions

Interpreter: tree-walking + bytecode VM. Codegen: Cranelift (embedded) + LLVM (production). Tensors: ndarray. Errors: collect-all + miette display. Env: `Rc<RefCell<>>` for closures. Parser: Pratt (19 levels). Generics: monomorphization. Borrow: NLL-like without lifetimes. Full table: see git history.

---

## 16. Quick Commands

```bash
# Build & test (mandatory before commit)
cargo build [--release]
cargo test --lib                                 # 7,581 lib tests
cargo test --lib -- --test-threads=64            # stress (V26 §6.7 rule)
cargo clippy --lib -- -D warnings                # MUST pass
cargo fmt -- --check                             # MUST pass

# Run Fajar Lang programs
cargo run -- run examples/hello.fj               # default (interpreter)
cargo run -- run --vm file.fj                    # bytecode VM
cargo run -- check file.fj                       # type-check only
cargo run -- repl                                # interactive REPL
cargo run -- dump-tokens|dump-ast file.fj        # debug

# Project lifecycle
cargo run -- new <name> | build | fmt | lsp | doc | demo | watch

# Feature flags (cargo run --features X -- run file.fj)
#   websocket | mqtt | ble | https | gui | native (Cranelift) | llvm | cuda
```

---

## 17. Repository Structure

`src/`: lexer, parser, analyzer, interpreter, vm, codegen/{cranelift,llvm}, runtime/{os,ml}, gpu_codegen, dependent, verify, lsp, package, distributed, wasi_p2, ffi_v2, formatter, selfhost, const_*, gui (gated). **Glob discovery preferred** — use `Glob "src/**/mod.rs"` rather than reading this map. Companion dirs: `tests/` (46 files), `examples/` (231 .fj), `docs/` (157), `benches/`, `fuzz/`, `audit/`, `scripts/`, `.github/workflows/`.

---

## 18. Document Quick-Reference Index

| When You Need... | Read This |
|---|---|
| **Current per-module status** | **`docs/HONEST_STATUS_V26.md`** — V26 (54 [x], 0 [f], 0 [s]) |
| **Current plan (V26)** | **`docs/V26_PRODUCTION_PLAN.md`** — Phase A nearly done, B+C remaining |
| **V26 audit trail** | **`docs/HONEST_AUDIT_V26.md`** — corrections to prior counts |
| **Honest codebase audit (older)** | `docs/HONEST_AUDIT_V17.md` (V17 baseline) |
| **Coding rules** | CLAUDE.md §6 (V1_RULES.md is archived in docs/archive/) |
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

*CLAUDE.md Version: 25.0 | V26 "Final" Phase A done — 7,581 lib + 2,374 integ + 14 doc tests, 0 flakes (80/80 stress), 231 examples, 0 production .unwrap(), 0 [f]/[s] modules | §6.8 Plan Hygiene Rules added, doc trimmed from 885→~600 lines for context efficiency*
*Last Updated: 2026-04-11*
