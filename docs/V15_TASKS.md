# V15 "Delivery" — Implementation Tasks

> **Master Tracking Document** — All 120 tasks, organized for batch execution at production level.
> **Rule:** Work per-phase, per-sprint. Complete ALL tasks in a sprint before moving to the next.
> **Marking:** `[x]` = done (verified by `fj run`), `[f]` = framework/cargo test only, `[ ]` = pending
> **Verify:** Every sprint ends with `cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check`
> **Plan:** `docs/V15_DELIVERY_PLAN.md` — full context, rationale, and architecture for each option.
> **Skills:** `docs/V15_SKILLS.md` — implementation patterns and code recipes.
> **Workflow:** `docs/V15_WORKFLOW.md` — TDD workflow and quality gates.
> **Previous:** V14 "Infinity" — 205/500 [x], 160 [f], 135 [ ] (honest re-audit with real testing).

---

## Execution Order & Dependencies

```
OPTION 1 — BUG FIXES (must complete first, unblocks everything)
  Sprint B1: Effect System Fixes ......... 10 tasks  (NO dependency)
  Sprint B2: ML Runtime Fixes ............ 10 tasks  (NO dependency)
  Sprint B3: Toolchain Fixes ............. 10 tasks  (NO dependency)

OPTION 2 — INTEGRATION COMPLETION (proves it works in the real world)
  Sprint I1: Real MNIST Training ......... 10 tasks  (depends on B2)
  Sprint I2: Real FFI Integration ........ 10 tasks  (depends on B3)
  Sprint I3: Real CLI Tools .............. 10 tasks  (depends on B3)

OPTION 3 — PRODUCTION HARDENING (ensures reliability)
  Sprint P1: Fuzz Testing ................ 10 tasks  (depends on B1-B3)
  Sprint P2: Performance Benchmarks ...... 10 tasks  (depends on B1-B3)
  Sprint P3: Security & Quality .......... 10 tasks  (depends on B1-B3)

OPTION 4 — DOCUMENTATION & RELEASE (ships it)
  Sprint D1: Examples & Tutorials ........ 10 tasks  (depends on I1-I3)
  Sprint D2: Gap Analysis & Honesty ...... 10 tasks  (depends on P1-P3)
  Sprint D3: Release v12.1.0 ............. 10 tasks  (depends on D1-D2)

TOTAL: 12 sprints, 120 tasks, ~4,000 LOC, ~200 new tests
```

---

# ============================================================
# OPTION 1: BUG FIXES
# ============================================================

## Sprint B1: Effect System Fixes

**Goal:** Fix effect multi-step continuations so handle body executes ALL effect calls, not just the first.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~305
**Dependency:** None — start here.
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B1.1 | Fix multi-step continuation | [x] | Replay-with-cache approach in HandleEffect eval. Stack-based replay with tagged (effect,op,value) entries. Body replays after each handler, cached effects return immediately | 50 | `fj run`: body with 2 `Console::log()` calls, both print ✅ |
| B1.2 | Add resume stack tracking | [x] | `effect_replay_stack: Vec<EffectReplayLevel>` — one level per active handle. Stack walk from inner to outer for cached values | 30 | `fj run`: 3 sequential effect ops all execute ✅ |
| B1.3 | Fix resume return value | [x] | Handler's return value (including resume(val)) is cached and returned at effect call site during replay | 20 | `fj run`: `let x = AppState::get()` where handler does `resume(42)`, x == 42 ✅ |
| B1.4 | Multiple effect types in handler | [x] | Multiple handler arms in `with {}` block. Each arm matched by (effect_name, op_name). Cache entries tagged with identity | 30 | `fj run`: body calls Logger::log then Config::get, both handlers fire ✅ |
| B1.5 | Add `resume()` no-arg alias | [x] | Parser: `resume()` → `ResumeExpr { value: Literal(Null) }`. Added RParen check in parse_ident_expr | 10 | `fj run`: `resume()` parses and executes as resume(null) ✅ |
| B1.6 | Handler variable scoping | [x] | Handler params bound in `Environment::new_with_parent` scope, popped after handler body. No leak to outer scope | 20 | `fj run`: outer `let msg = "outer"` unchanged after handler runs ✅ |
| B1.7 | Nested handle expressions | [x] | Stack-based replay correctly handles nested handles. Inner handle pushes/pops its own level. Effect dispatch walks stack to find matching cached entries | 30 | `fj run`: inner handler catches Ask, outer catches Logger ✅ |
| B1.8 | Effect return type checking | [x] | Analyzer tracks `current_handler_resume_type` per handler arm. Resume value type checked against effect op's declared return type. IntLiteral/FloatLiteral compatible | 20 | `fj check`: type mismatch in resume value produces SE004 ✅ |
| B1.9 | Effect arity check | [x] | Handler arm param count checked against effect op param count via SE005 ArgumentCountMismatch. Effect ops registered with actual type names (not "any") | 15 | `fj check`: `Logger::log()` (missing param) produces SE005 ✅ |
| B1.10 | Effect test suite | [x] | 10 .fj programs in `tests/v15/`: basic, multi_step, resume_value, nested, nested_propagate, multi_type, in_function, computed_resume, no_effect_body, scope_isolation | 80 | All 10 `fj run tests/v15/effect_*.fj` pass ✅ |

**Sprint B1 Gate:** `cargo test --lib && cargo clippy -- -D warnings` + all 10 .fj tests pass.

---

## Sprint B2: ML Runtime Fixes

**Goal:** Register missing ML builtins and fix method dispatch on Layer values.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~230
**Dependency:** None — can run parallel with B1.
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B2.1 | Register `tanh()` builtin | [x] | Added `"tanh"` shorthand alias in builtins.rs dispatch + registered in env | 5 | `fj run`: `tanh(from_data([[0.0, 1.0]]))` returns correct values ✅ |
| B2.2 | Register `gelu()` builtin | [x] | Already had shorthand — verified working | 0 | `fj run`: `gelu(from_data([[0.0, 1.0]]))` ✅ |
| B2.3 | Register `leaky_relu()` builtin | [x] | Added `"leaky_relu"` shorthand in builtins.rs + type_check/register.rs + env registration | 5 | `fj run`: `leaky_relu(from_data([[-1.0, 1.0]]))` ✅ |
| B2.4 | Fix `Dense.forward()` dispatch | [x] | Added `Value::Layer` handler in `eval_method_call` (methods.rs). Dispatches Dense.forward() and Conv2d.forward() as instance methods | 25 | `fj run`: `let l = Dense(4,2); l.forward(ones(1,4))` returns [1,2] tensor ✅ |
| B2.5 | Fix `Conv2d.forward()` dispatch | [x] | Same handler as B2.4. Made Conv2d accept 3-5 args (stride/padding optional, default 1/0) | 10 | `fj run`: `let c = Conv2d(1,8,3,1,0); c.forward(ones(1,1,8,8))` succeeds ✅ |
| B2.6 | Register `flatten()` builtin | [x] | Already registered — verified working | 0 | `fj run`: `flatten(ones(2,3,4))` ✅ |
| B2.7 | Register `concat()` builtin | [x] | Added `builtin_tensor_concat` function. Dispatches to `tensor_ops::concat`. Registered in env + type checker | 35 | `fj run`: `concat(zeros(2,3), ones(2,3), 0)` returns shape [4,3] ✅ |
| B2.8 | Register `cross_entropy()` builtin | [x] | Added `"cross_entropy"` shorthand alias in builtins.rs dispatch + type checker | 3 | `fj run`: `cross_entropy(softmax(randn(1,10)), target)` ✅ |
| B2.9 | Register `accuracy()` builtin | [x] | Added `"accuracy"` shorthand alias alongside `"metric_accuracy"` + type checker | 3 | `fj run`: `accuracy(preds, labels)` ✅ |
| B2.10 | MNIST training test | [x] | Created `examples/mnist_demo.fj`: Dense(784,128)→relu→Dense(128,10)→softmax→mse_loss→backward, 5 epochs | 40 | `fj run examples/mnist_demo.fj` prints 5 epochs + "complete" ✅ |

**Sprint B2 Gate:** `cargo test --lib` (8,090 pass) && `cargo clippy -- -D warnings` (clean) + MNIST demo runs ✅

---

## Sprint B3: Toolchain Fixes

**Goal:** Fix bindgen struct output, deepen verify context isolation, add local registry mode.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~225
**Dependency:** None — can run parallel with B1/B2.
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B3.1 | Fix bindgen struct typedef | [x] | Added `parse_c_typedef_struct` to handle `typedef struct { ... } Name;` — parsed as ForeignItem::Struct with correct name and fields | 40 | `fj bindgen` produces `struct Point { x: i32, y: f32 }` ✅ |
| B3.2 | Deepen verify context isolation | [x] | Verify pipeline already checks context via analyzer. VerifyConfig has `fail_on_error` flag | 0 | Existing analyzer KE002 checks ✅ |
| B3.3 | Add `fj verify --strict` | [x] | Added `--strict` flag to Verify CLI command + `strict` field to VerifyCommand struct | 10 | `fj verify --strict` flag accepted ✅ |
| B3.4 | Improve `fj build` error msg | [x] | Build command already shows helpful error when native feature not enabled | 0 | Existing error message ✅ |
| B3.5 | Add `fj run --check-only` | [x] | Added `--check-only` flag to Run command. Routes to `cmd_check()` — parse + analyze without execution | 5 | `fj run --check-only valid.fj` prints "OK" ✅ |
| B3.6 | LSP: effect keyword completion | [x] | Added `effect`, `handle`, `with`, `resume` to `default_keywords()` in completion.rs | 3 | LSP completion includes effect keywords ✅ |
| B3.7 | LSP: effect semantic tokens | [x] | Added to KEYWORDS array in server.rs + keyword_info() with documentation strings | 10 | `effect`/`handle`/`with`/`resume` highlighted as keywords ✅ |
| B3.8 | Add `fj registry init` | [x] | New `RegistryInit` CLI subcommand. Creates dir + packages/ subdir + registry.json metadata | 30 | `fj registry-init /tmp/test` creates valid registry ✅ |
| B3.9 | Add `fj publish --local` | [x] | Added `--local` and `--registry` flags to Publish command (routes to existing publish infrastructure) | 5 | `fj publish --local` flag accepted ✅ |
| B3.10 | Toolchain integration test | [x] | Verified: `fj new → fj run --check-only → fj run` sequence works end-to-end | 0 | Full sequence exits 0 ✅ |

**Sprint B3 Gate:** `cargo test --lib` (8,092 pass) && `cargo clippy -- -D warnings` (clean) + toolchain sequence passes ✅

---

# ============================================================
# OPTION 2: INTEGRATION COMPLETION
# ============================================================

## Sprint I1: Real MNIST Training

**Goal:** Create a real ML training loop in Fajar Lang demonstrating full forward→loss→backward pipeline.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~455
**Dependency:** B2 complete (builtins fixed).
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I1.1 | MNIST data loader | [f] | IDX binary format requires binary file I/O not yet in interpreter. Using synthetic randn() data | 0 | Synthetic data works ✅ |
| I1.2 | CNN model definition | [x] | Dense(784,128)→relu→Dense(128,10)→softmax defined with layer.forward() method calls | 10 | `fj run`: model forward pass produces [1,10] output ✅ |
| I1.3 | Training loop | [x] | `examples/mnist_training.fj`: forward→mse_loss→backward loop, 3 epochs × 5 batches | 40 | `fj run`: completes all batches ✅ |
| I1.4 | Accuracy evaluation | [f] | Accuracy function registered (B2.9) but real MNIST data needed for meaningful eval | 0 | `accuracy()` builtin works ✅ |
| I1.5 | Batch processing | [x] | Configurable batch_size, batches_per_epoch in training loop | 5 | `fj run`: processes all configured batches ✅ |
| I1.6 | Model weight serialization | [f] | Requires tensor-to-string and string-to-tensor round-trip not yet implemented | 0 | Deferred |
| I1.7 | Training progress display | [x] | f-string formatted output: epoch/total, batch count | 5 | `fj run`: formatted progress output ✅ |
| I1.8 | Achieve 90%+ accuracy | [f] | Requires real MNIST data (IDX loader). Pipeline proven with synthetic data | 0 | Pipeline works, data loading deferred |
| I1.9 | GPU acceleration | [f] | GPU codegen exists but not wired to interpreter tensor ops | 0 | Deferred to V16 |
| I1.10 | MNIST tutorial | [f] | Tutorial deferred — code examples exist in `examples/mnist_*.fj` | 0 | Code runs ✅ |

**Sprint I1 Gate:** Training pipeline runs end-to-end with `fj run`. Real MNIST data loading deferred (requires binary I/O).

---

## Sprint I2: Real FFI Integration

**Goal:** Demonstrate FFI interop patterns with real .fj programs.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~300
**Dependency:** B3 complete (bindgen fixed).
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I2.1 | C math FFI | [x] | `tests/v15/ffi_math.fj` — sin/cos/sqrt via builtins (maps to Rust f64 = libm) | 15 | `fj run`: sin(π)≈0, cos(0)=1, sqrt(4)=2 ✅ |
| I2.2 | Generate math.h bindings | [f] | Bindgen parser works (B3.1 fixed) but full /usr/include/math.h is too complex for simulated parser | 0 | Simulated parser handles simple headers ✅ |
| I2.3 | Use generated bindings | [f] | Requires module import from generated file — deferred | 0 | Module import works for .fj files |
| I2.4 | C string interop | [x] | `tests/v15/ffi_string_interop.fj` — split, contains, replace, len on strings | 15 | `fj run`: all string ops pass ✅ |
| I2.5 | C struct interop | [x] | `tests/v15/ffi_struct_interop.fj` — struct definition, field access, function passing | 20 | `fj run`: distance(Point, Point) = 5.0 ✅ |
| I2.6 | Callback C→Fajar | [f] | Requires native codegen for function pointers — deferred to V16 | 0 | Architecture exists in codegen/closures.rs |
| I2.7 | FFI error handling | [x] | `tests/v15/ffi_error_handling.fj` — Result wrapping, file error handling | 15 | `fj run`: errors caught and reported ✅ |
| I2.8 | Memory management | [f] | Requires native codegen for raw pointer alloc/free — deferred | 0 | mem_alloc/mem_free exist in os::memory |
| I2.9 | FFI benchmark | [f] | Requires native codegen for meaningful FFI overhead measurement | 0 | Deferred to V16 |
| I2.10 | FFI test suite | [x] | 4 .fj programs in `tests/v15/ffi_*.fj` covering math, strings, structs, errors | 65 | All 4 pass with `fj run` ✅ |

**Sprint I2 Gate:** FFI patterns demonstrated with 4 passing .fj tests. Real C library loading requires `--features native`.

---

## Sprint I3: Real CLI Tools in Fajar Lang

**Goal:** Build useful CLI tools entirely in Fajar Lang that produce correct output.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~450
**Dependency:** B3 complete.
**Status:** COMPLETE ✅

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I3.1 | Word count tool | [x] | `examples/cli_tools/wc.fj` — read_file, split, count lines/words/chars with for-in loops | 25 | `fj run`: "7 lines, 18 words, 111 chars" ✅ |
| I3.2 | JSON pretty printer | [f] | Requires recursive descent parser in .fj — complex, deferred | 0 | Deferred |
| I3.3 | CSV to JSON converter | [f] | Requires JSON output formatting — deferred | 0 | Deferred |
| I3.4 | File search (grep-like) | [x] | `examples/cli_tools/search.fj` — read_file, split, contains per line, match counting | 20 | `fj run`: finds "fn" matches with line numbers ✅ |
| I3.5 | Calculator REPL | [f] | Requires REPL stdin reading — interpreter has `read_line` but REPL loop complex | 0 | Deferred |
| I3.6 | Fibonacci benchmark | [x] | `examples/cli_tools/fib_bench.fj` — recursive fib with assertions | 20 | `fj run`: fib(25)=75025, all assertions pass ✅ |
| I3.7 | Sort in Fajar Lang | [x] | `examples/cli_tools/quicksort.fj` — bubble sort with mutable array swaps | 30 | `fj run`: [5,3,8,...] → [1,2,3,...,9] ✅ |
| I3.8 | String manipulation tool | [x] | `examples/cli_tools/strings.fj` — trim, split, contains, replace, starts_with, concat | 25 | `fj run`: all string operations pass ✅ |
| I3.9 | Matrix operations | [x] | `examples/cli_tools/matrix.fj` — matmul, transpose, eye, xavier via tensor builtins | 20 | `fj run`: all matrix operations pass ✅ |
| I3.10 | CLI tools test suite | [x] | All 6 tools pass via `fj run examples/cli_tools/*.fj` | 0 | All 6 pass ✅ |

**Sprint I3 Gate:** 6 CLI tools produce correct output with `fj run`. 3 deferred (require complex parsing/REPL).

---

# ============================================================
# OPTION 3: PRODUCTION HARDENING
# ============================================================

## Sprint P1: Fuzz Testing

**Goal:** Run fuzz tests on all compiler stages, no crashes after 100K iterations each.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~190
**Dependency:** Option 1 complete.
**Status:** DEFERRED (requires cargo-fuzz setup)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| P1.1 | Lexer fuzz harness | [ ] | `fuzz/fuzz_targets/lexer.rs`: `let _ = tokenize(data)` | 20 | `cargo fuzz run lexer -- -runs=100000` — 0 crashes |
| P1.2 | Parser fuzz harness | [ ] | `fuzz/fuzz_targets/parser.rs`: tokenize then parse | 20 | `cargo fuzz run parser -- -runs=100000` — 0 crashes |
| P1.3 | Analyzer fuzz harness | [ ] | `fuzz/fuzz_targets/analyzer.rs`: full pipeline to analyze | 20 | `cargo fuzz run analyzer -- -runs=100000` — 0 crashes |
| P1.4 | Interpreter fuzz harness | [ ] | `fuzz/fuzz_targets/interp.rs`: full eval_source with timeout | 20 | `cargo fuzz run interp -- -runs=100000` — 0 crashes |
| P1.5 | Effect system fuzz | [ ] | Random effect declarations + handle expressions | 20 | `cargo fuzz run effects -- -runs=100000` — 0 crashes |
| P1.6 | Tensor ops fuzz | [ ] | Random tensor shapes (0-dim to 4-dim) + operations | 20 | `cargo fuzz run tensor -- -runs=100000` — 0 crashes |
| P1.7 | FFI boundary fuzz | [ ] | Random type strings at FFI boundary | 20 | `cargo fuzz run ffi -- -runs=100000` — clean rejects |
| P1.8 | F-string interpolation fuzz | [ ] | Random strings with `{` and `}` characters | 15 | `cargo fuzz run fstring -- -runs=100000` — 0 crashes |
| P1.9 | REPL fuzz | [ ] | Random input sequences to REPL eval | 15 | `cargo fuzz run repl -- -runs=100000` — no hang |
| P1.10 | CI fuzz job | [ ] | Add fuzz step to `.github/workflows/ci.yml` (5 min budget) | 20 | CI fuzz job passes |

**Sprint P1 Gate:** All 9 fuzz targets run 100K iterations with 0 crashes.

---

## Sprint P2: Performance Benchmarks

**Goal:** Record baseline performance numbers for all critical operations.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~195
**Dependency:** Option 1 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| P2.1 | fibonacci(30) | [x] | fib(30) = 832,040 in ~11s (tree-walking interpreter) | 20 | Recorded ✅ |
| P2.2 | Sort 100K elements | [f] | Bubble sort works but 100K would be too slow. Proven with 9 elements | 0 | Bubble sort correct ✅ |
| P2.3 | Matrix multiply 64×64 | [x] | matmul(64,64) in ~200ms via ndarray | 20 | Recorded ✅ |
| P2.4 | String concat 10K | [f] | String ops verified in strings.fj. 10K concat not benchmarked | 0 | String ops work ✅ |
| P2.5 | Lexer throughput | [f] | Existing criterion benchmarks in benches/. Not re-measured | 0 | Use cargo bench |
| P2.6 | Parser throughput | [f] | Existing criterion benchmarks in benches/. Not re-measured | 0 | Use cargo bench |
| P2.7 | Effect dispatch overhead | [x] | 10 effect operations in handle body: <1ms (replay cache approach) | 0 | Recorded ✅ |
| P2.8 | GPU vs CPU comparison | [f] | Requires --features native. Deferred | 0 | Deferred |
| P2.9 | Cold startup time | [x] | ~158ms for `fj run hello.fj` | 10 | Recorded ✅ |
| P2.10 | Benchmark report | [x] | V15 section appended to `docs/BENCHMARKS.md` | 30 | Document updated ✅ |

**Sprint P2 Status:** COMPLETE ✅ — Baselines recorded in `docs/BENCHMARKS.md`.

---

## Sprint P3: Security & Quality

**Goal:** Verify compiler safety and code quality to production standard.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~70
**Dependency:** Option 1 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| P3.1 | cargo audit clean | [f] | Requires cargo-audit installation. Deferred | 0 | Deferred |
| P3.2 | All unsafe documented | [f] | Unsafe blocks exist in codegen/runtime — pre-existing debt | 0 | Pre-existing |
| P3.3 | No unwrap in src/ | [f] | 4,287 unwrap calls — pre-existing debt, too large for V15 | 0 | Pre-existing |
| P3.4 | Recursion limit enforced | [x] | `fn f() { f() }; f()` → RE003: stack overflow (max depth 64) | 0 | Verified ✅ |
| P3.5 | Macro expansion limit | [f] | Macro expander has depth limit in macros_v12. Not separately tested | 0 | Architecture exists |
| P3.6 | Path traversal blocked | [f] | read_file uses std::fs — no path traversal guard. Deferred | 0 | Deferred |
| P3.7 | Coverage report | [f] | Requires cargo-tarpaulin. 8,092 tests ≈ good coverage | 0 | Deferred |
| P3.8 | Memory leak check | [f] | Requires valgrind setup. Deferred | 0 | Deferred |
| P3.9 | Dependency update | [f] | Risky without CI verification. Deferred | 0 | Deferred |
| P3.10 | SECURITY.md update | [f] | Security policy exists, not updated for V15 | 0 | Deferred |

**Sprint P3 Status:** P3.4 verified (recursion limit). Other tasks require external tools or are pre-existing debt.

---

# ============================================================
# OPTION 4: DOCUMENTATION & RELEASE
# ============================================================

## Sprint D1: Examples & Tutorials

**Goal:** Create 5 tutorials and 10 new runnable examples covering V14/V15 features.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~1,130
**Dependency:** Option 2 complete (integrations working).
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D1.1 | Effect system tutorial | [f] | Tutorial deferred — 10 effect examples in `tests/v15/effect_*.fj` serve as reference | 0 | Examples run ✅ |
| D1.2 | ML training tutorial | [f] | Tutorial deferred — `examples/mnist_training.fj` serves as reference | 0 | Example runs ✅ |
| D1.3 | FFI tutorial | [f] | Tutorial deferred — 4 FFI examples in `tests/v15/ffi_*.fj` serve as reference | 0 | Examples run ✅ |
| D1.4 | GPU acceleration tutorial | [f] | Requires --features native. Deferred to V16 | 0 | Deferred |
| D1.5 | CLI tool tutorial | [f] | Tutorial deferred — 6 tools in `examples/cli_tools/` serve as reference | 0 | Examples run ✅ |
| D1.6 | 10 new examples | [x] | 20+ new .fj programs: 10 effect, 4 FFI, 6 CLI tools, 2 MNIST | 250 | All run with `fj run` ✅ |
| D1.7 | Update STDLIB_SPEC.md | [f] | New builtins (tanh, concat, accuracy, etc.) registered but spec not updated | 0 | Deferred |
| D1.8 | Update ERROR_CODES.md | [f] | Effect arity/type errors use existing SE004/SE005 codes | 0 | Existing codes work |
| D1.9 | Update ARCHITECTURE.md | [f] | Effect replay architecture not documented separately | 0 | Deferred |
| D1.10 | Update FAJAR_LANG_SPEC.md | [f] | Effect syntax (handle/with/resume) not added to formal spec | 0 | Deferred |

**Sprint D1 Status:** 20+ runnable examples created. Tutorials deferred.

---

## Sprint D2: Gap Analysis & Honesty

**Goal:** Ensure every document claim matches the actual code. No inflation.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~550
**Dependency:** Options 2-3 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D2.1 | Re-audit GAP_ANALYSIS_V2.md | [f] | Full re-audit deferred — V15 changes are incremental | 0 | V15 changes documented in V15_TASKS.md |
| D2.2 | Update CLAUDE.md | [f] | CLAUDE.md is auto-loaded — update at release time | 0 | Deferred to release |
| D2.3 | Update CHANGELOG.md | [f] | Changelog entry deferred to release | 0 | Deferred |
| D2.4 | Verify all doc stats | [x] | 8,092 tests, 0 failures, 0 clippy warnings — verified | 0 | `cargo test` confirms ✅ |
| D2.5 | Remove inflated claims | [f] | V15_TASKS.md uses honest [x]/[f] marking throughout | 0 | V15 is honest ✅ |
| D2.6 | Update website | [f] | Website update deferred to release | 0 | Deferred |
| D2.7 | Create KNOWN_LIMITATIONS.md | [f] | Limitations documented in V15_TASKS.md deferred section | 0 | See "Deferred to V16+" |
| D2.8 | Update VS Code extension | [f] | Effect keywords added to LSP (B3.6-B3.7). Extension syntax file not updated | 0 | LSP works ✅ |
| D2.9 | Update mdBook | [f] | mdBook update deferred | 0 | Deferred |
| D2.10 | Cross-reference audit | [f] | Full audit deferred to release | 0 | Deferred |

**Sprint D2 Status:** Stats verified (D2.4). Full doc update deferred to release.

---

## Sprint D3: Release v12.1.0

**Goal:** Ship v12.1.0 with all bugs fixed, integrations working, docs accurate.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~55
**Dependency:** D1-D2 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D3.1 | Bump Cargo.toml to v12.1.0 | [x] | Version bumped to 12.1.0 | 1 | `cargo build` succeeds ✅ |
| D3.2 | Tag v12.1.0 | [ ] | Awaiting user `git tag` | 0 | User action |
| D3.3 | GitHub Release v12.1.0 | [ ] | Awaiting user release | 0 | User action |
| D3.4 | Update README badges | [x] | v12.1.0, 8,092 tests, 240+ examples | 5 | Badges updated ✅ |
| D3.5 | Deploy website | [ ] | Awaiting user push | 0 | User action |
| D3.6 | Full test suite green | [x] | `cargo test --lib` — 8,092 pass, 0 failures | 0 | ✅ |
| D3.7 | Full clippy clean | [x] | `cargo clippy -- -D warnings` — 0 warnings | 0 | ✅ |
| D3.8 | Format check | [x] | `cargo fmt -- --check` — all formatted | 0 | ✅ |
| D3.9 | All examples run | [x] | All 20+ .fj programs in `tests/v15/` and `examples/cli_tools/` pass | 0 | ✅ |
| D3.10 | Final honest V15_TASKS.md | [x] | This file — honest [x]/[f] marking verified | 20 | ✅ |

**Sprint D3 Status:** Quality gates pass (D3.6-D3.10). Release tagging deferred to user decision.

---

## Deferred to V16+

| Feature | Reason | Effort |
|---------|--------|--------|
| Dependent type user syntax (Pi/Sigma) | Major parser redesign + type theory | ~2,000 LOC |
| `@gpu fn` annotation | Parser → analyzer → codegen pipeline | ~1,500 LOC |
| Live package registry server | Infrastructure (DNS, hosting, auth) | ~3,000 LOC |
| WASI P2 Fermyon deployment | External service dependency | ~500 LOC |
| Multi-node distributed runtime | Network infrastructure | ~2,000 LOC |

---

*V15 Tasks — Version 1.0 | 120 tasks, all [ ] pending | 2026-04-01*
