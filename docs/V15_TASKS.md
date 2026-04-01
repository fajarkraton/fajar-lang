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
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B1.1 | Fix multi-step continuation | [ ] | After `resume(val)`, interpreter must return to body's next statement instead of exiting handle expression. Modify `eval_handle_effect` in `eval/mod.rs` to loop | 50 | `fj run`: body with 2 `Console::log()` calls, both print |
| B1.2 | Add resume stack tracking | [ ] | Track which statement in the body we're resuming from. Store continuation index in handle eval state | 30 | `fj run`: 3 sequential effect ops all execute |
| B1.3 | Fix resume return value | [ ] | `resume(42)` should make the effect call site return 42, not null. Thread resumed value back to body | 20 | `fj run`: `let x = State::get()` where handler does `resume(42)`, x == 42 |
| B1.4 | Multiple effect types in handler | [ ] | `handle {} with { IO::log(m) => ..., State::get() => ... }` — both handler arms must match | 30 | `fj run`: body calls IO::log then State::get, both handlers fire |
| B1.5 | Add `resume()` no-arg alias | [ ] | Parser should accept `resume()` as alias for `resume(null)`. Add case in `parse_ident_expr` | 10 | `fj run`: `resume()` parses and executes as resume(null) |
| B1.6 | Handler variable scoping | [ ] | Handler params (e.g., `msg` in `IO::log(msg)`) must not leak to outer scope or conflict | 20 | `fj run`: outer `let msg = "outer"` unchanged after handler runs |
| B1.7 | Nested handle expressions | [ ] | `handle { handle { inner() } with { A::op() => ... } } with { B::op() => ... }` | 30 | `fj run`: inner handler catches A, outer catches B |
| B1.8 | Effect return type checking | [ ] | `effect State { fn get() -> i32 }` — analyzer verifies `resume(val)` type matches return type | 20 | `fj check`: type mismatch in resume value produces SE004 |
| B1.9 | Effect arity check | [ ] | Calling effect op with wrong argument count gives clear error with expected vs actual | 15 | `fj run`: `Console::log()` (missing arg) produces EE error |
| B1.10 | Effect test suite | [ ] | Create 10 .fj example programs in `tests/v15/` covering all effect patterns: basic, multi-step, nested, typed resume, multi-effect | 80 | All 10 `fj run tests/v15/effect_*.fj` pass |

**Sprint B1 Gate:** `cargo test --lib && cargo clippy -- -D warnings` + all 10 .fj tests pass.

---

## Sprint B2: ML Runtime Fixes

**Goal:** Register missing ML builtins and fix method dispatch on Layer values.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~230
**Dependency:** None — can run parallel with B1.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B2.1 | Register `tanh()` builtin | [ ] | Add to `builtins.rs` dispatch + `type_check/register.rs`. Implementation: `tv.data.mapv(\|x\| x.tanh())` | 10 | `fj run`: `tanh(from_data([[0.0, 1.0]]))` returns correct values |
| B2.2 | Register `gelu()` builtin | [ ] | GELU approximation: `x * 0.5 * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))` | 15 | `fj run`: `gelu(from_data([[0.0, 1.0]]))` returns correct values |
| B2.3 | Register `leaky_relu()` builtin | [ ] | `if x > 0 { x } else { 0.01 * x }`. Takes optional alpha parameter | 15 | `fj run`: `leaky_relu(from_data([[-1.0, 1.0]]))` returns [-0.01, 1.0] |
| B2.4 | Fix `Dense.forward()` dispatch | [ ] | Add `.forward()` method resolution for `Value::Layer` in method call evaluation. Delegate to `builtin_layer_forward` | 30 | `fj run`: `let l = Dense(4,2); l.forward(ones(1,4))` returns [1,2] tensor |
| B2.5 | Fix `Conv2d.forward()` dispatch | [ ] | Same as B2.4 for Conv2d layers. Must handle 4D tensor input [batch, channels, h, w] | 20 | `fj run`: `let c = Conv2d(1,8,3); c.forward(ones(1,1,8,8))` succeeds |
| B2.6 | Register `flatten()` builtin | [ ] | Reshape tensor to 2D: [batch, features]. Check if already registered, wire if not | 10 | `fj run`: `flatten(ones(2,3,4))` returns shape [2, 12] |
| B2.7 | Register `concat()` builtin | [ ] | Concatenate tensors along axis. Use ndarray `concatenate` | 20 | `fj run`: `concat(zeros(2,3), ones(2,3), 0)` returns shape [4, 3] |
| B2.8 | Register `cross_entropy()` builtin | [ ] | Cross-entropy loss: `-sum(target * log(pred))`. Check if already registered | 15 | `fj run`: `cross_entropy(softmax(randn(1,10)), from_data([[1,0,0,0,0,0,0,0,0,0]]))` returns scalar |
| B2.9 | Register `accuracy()` builtin | [ ] | `argmax(pred) == argmax(target)` averaged. From metrics module | 15 | `fj run`: `accuracy(from_data([[0.1,0.9]]), from_data([[0,1]]))` returns 1.0 |
| B2.10 | MNIST training test | [ ] | Create `examples/mnist_demo.fj`: Dense(784,128) → relu → Dense(128,10) → softmax → loss → backward → SGD step. Must show loss decreasing | 80 | `fj run examples/mnist_demo.fj` prints decreasing loss over 5 iterations |

**Sprint B2 Gate:** `cargo test --lib && cargo clippy -- -D warnings` + MNIST demo runs.

---

## Sprint B3: Toolchain Fixes

**Goal:** Fix bindgen struct output, deepen verify context isolation, add local registry mode.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~225
**Dependency:** None — can run parallel with B1/B2.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| B3.1 | Fix bindgen struct typedef | [ ] | Fix format string in `ffi_v2/bindgen.rs` that produces `type { = struct` instead of proper struct definition | 15 | `fj bindgen struct_header.h` produces valid Fajar struct |
| B3.2 | Deepen verify context isolation | [ ] | Add `TENSOR_BUILTINS` blacklist to verify pipeline. Check function calls against list when in @kernel context | 30 | `fj verify` on `@kernel fn f() { zeros(3,3) }` reports KE002 |
| B3.3 | Add `fj verify --strict` | [ ] | Strict mode: warnings become errors, return non-zero exit code | 20 | `fj verify --strict file_with_warning.fj` exits 1 |
| B3.4 | Improve `fj build` error msg | [ ] | When native feature not enabled, show: "Native compilation requires: cargo install fajar-lang --features native" | 10 | `fj build file.fj` shows helpful message |
| B3.5 | Add `fj run --check-only` | [ ] | Parse + analyze without executing. Print "OK" or errors. Add flag to clap Run command | 10 | `fj run --check-only valid.fj` prints "OK", exits 0 |
| B3.6 | LSP: effect keyword completion | [ ] | Add `effect`, `handle`, `with`, `resume` to LSP completion provider keyword list | 15 | LSP completion request returns effect keywords |
| B3.7 | LSP: effect semantic tokens | [ ] | Add `effect` declaration highlighting (token type: keyword) and handle/with block highlighting | 20 | LSP semantic tokens request includes effect tokens |
| B3.8 | Add `fj registry init` | [ ] | New CLI subcommand: creates local file-based registry directory with `registry.json` metadata | 40 | `fj registry init /tmp/test-reg` creates dir with valid metadata |
| B3.9 | Add `fj publish --local` | [ ] | Publish package to local registry (created by B3.8). Pack source, compute checksum, write to registry | 40 | `fj publish --local --registry /tmp/test-reg` succeeds |
| B3.10 | Toolchain integration test | [ ] | Script: `fj new proj && cd proj && fj check src/main.fj && fj test src/main.fj && fj fmt src/main.fj && fj verify src/main.fj && fj run src/main.fj` | 25 | Full sequence exits 0 |

**Sprint B3 Gate:** `cargo test --lib && cargo clippy -- -D warnings` + toolchain sequence passes.

---

# ============================================================
# OPTION 2: INTEGRATION COMPLETION
# ============================================================

## Sprint I1: Real MNIST Training

**Goal:** Create a real ML training loop in Fajar Lang that achieves 90%+ accuracy on MNIST.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~455
**Dependency:** B2 complete (builtins fixed).
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I1.1 | MNIST data loader | [ ] | Read IDX binary format (train-images, train-labels). Parse header, extract images as f64 arrays, normalize to [0,1] | 80 | `fj run`: loads 60,000 images, prints first 10 labels |
| I1.2 | CNN model definition | [ ] | Define: Dense(784,128) → relu → Dense(128,10) → softmax. Use `let` bindings for layers | 40 | `fj run`: model forward pass produces [1,10] output |
| I1.3 | Training loop | [ ] | For each batch: forward → cross_entropy → backward → SGD step. Print loss per batch | 50 | `fj run`: loss decreases over 10 batches |
| I1.4 | Accuracy evaluation | [ ] | After training, run on test set, compute accuracy = correct/total | 30 | `fj run`: prints "Accuracy: XX.X%" |
| I1.5 | Batch processing | [ ] | Reshape data into batches of 32. Iterate batches in training loop | 20 | `fj run`: processes 1875 batches per epoch |
| I1.6 | Model weight serialization | [ ] | Save weights to file using `write_file`. Load using `read_file` + `from_data` | 40 | `fj run`: save then load produces same predictions |
| I1.7 | Training progress display | [ ] | Print epoch, batch, loss, and running accuracy per epoch | 15 | `fj run`: formatted output each epoch |
| I1.8 | Achieve 90%+ accuracy | [ ] | Tune learning rate, add epochs until test accuracy ≥ 90% | 0 | `fj run`: final accuracy ≥ 90% |
| I1.9 | GPU acceleration | [ ] | Replace `matmul` with `gpu_matmul` when `gpu_available()`. Measure speedup | 20 | `fj run`: training completes, faster with GPU |
| I1.10 | MNIST tutorial | [ ] | Write `docs/tutorials/mnist.md`: step-by-step guide with code, output, explanations | 160 | Tutorial document complete, code runs |

**Sprint I1 Gate:** MNIST trains to 90%+ accuracy with `fj run`.

---

## Sprint I2: Real FFI Integration

**Goal:** Call real C library functions from Fajar Lang via FFI, not mocks.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~300
**Dependency:** B3 complete (bindgen fixed).
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I2.1 | C math FFI | [ ] | Call `sin`, `cos`, `sqrt` from libm via `@ffi extern fn`. Use `libloading` crate | 30 | `fj run`: `sin(3.14159)` ≈ 0.0, `sqrt(4.0)` = 2.0 |
| I2.2 | Generate math.h bindings | [ ] | `fj bindgen /usr/include/math.h -o math_bindings.fj` | 0 | Bindings file generated with sin/cos/sqrt |
| I2.3 | Use generated bindings | [ ] | Import generated bindings, call functions, verify results | 20 | `fj run`: results match C library output |
| I2.4 | C string interop | [ ] | Pass Fajar string → C (null-terminated), C string → Fajar. Test with `strlen` | 30 | `fj run`: string round-trip preserves content |
| I2.5 | C struct interop | [ ] | Define matching struct in C and Fajar, pass across FFI boundary | 30 | `fj run`: struct fields accessible after FFI call |
| I2.6 | Callback C→Fajar | [ ] | Register Fajar function as C callback. C calls it with arguments | 40 | `fj run`: callback fires with correct args |
| I2.7 | FFI error handling | [ ] | C function returns error code, Fajar wraps in Result | 20 | `fj run`: error propagated as Err("message") |
| I2.8 | Memory management | [ ] | Allocate buffer in C, fill data, read in Fajar, free in C | 30 | `fj run`: no leak (test with valgrind) |
| I2.9 | FFI benchmark | [ ] | Measure FFI call overhead: 1M calls, compute avg latency | 20 | `fj run`: overhead printed, < 100ns/call |
| I2.10 | FFI test suite | [ ] | 10 .fj programs in `tests/v15/ffi_*.fj` covering all patterns | 80 | All 10 pass with `fj run` |

**Sprint I2 Gate:** Real C FFI calls work end-to-end, measured overhead < 100ns.

---

## Sprint I3: Real CLI Tools in Fajar Lang

**Goal:** Build 5 useful CLI tools entirely in Fajar Lang that produce correct output.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~450
**Dependency:** B3 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I3.1 | Word count tool | [ ] | Read file, count lines/words/chars. Use `read_file`, `split`, `len` | 40 | `fj run wc.fj README.md` matches `wc README.md` output |
| I3.2 | JSON pretty printer | [ ] | Parse JSON string (recursive descent), output indented. Handle objects, arrays, strings, numbers, booleans, null | 80 | `fj run json_fmt.fj '{"a":1,"b":[2,3]}'` outputs formatted JSON |
| I3.3 | CSV to JSON converter | [ ] | Read CSV (first row = headers), output JSON array of objects | 60 | `fj run csv2json.fj data.csv` outputs valid JSON |
| I3.4 | File search (grep-like) | [ ] | Read file line by line, print lines containing pattern | 40 | `fj run search.fj "fn " src/main.fj` finds function defs |
| I3.5 | Calculator REPL | [ ] | Interactive: read expression, parse, evaluate, print result. Support +, -, *, /, parentheses | 60 | `fj run calc.fj` with input "2+3*4" outputs "14" |
| I3.6 | Fibonacci benchmark | [ ] | Compute fib(35) with timing. Use naive recursive implementation | 20 | `fj run fib_bench.fj` prints result and time |
| I3.7 | Quicksort in Fajar Lang | [ ] | Sort array of integers using quicksort. Print sorted result | 50 | `fj run quicksort.fj` sorts [5,3,8,1,2] → [1,2,3,5,8] |
| I3.8 | String manipulation tool | [ ] | Reverse, uppercase (manual char mapping), repeat, pad | 40 | `fj run strings.fj` all operations produce correct output |
| I3.9 | Matrix operations | [ ] | Matrix multiply, transpose using tensor ops. Print results | 30 | `fj run matrix.fj` results match expected values |
| I3.10 | CLI tools test suite | [ ] | Run all 9 tools, verify output matches expected | 30 | All 9 pass |

**Sprint I3 Gate:** All 9 tools produce correct output with `fj run`.

---

# ============================================================
# OPTION 3: PRODUCTION HARDENING
# ============================================================

## Sprint P1: Fuzz Testing

**Goal:** Run fuzz tests on all compiler stages, no crashes after 100K iterations each.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~190
**Dependency:** Option 1 complete.
**Status:** PENDING

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
| P2.1 | fibonacci(30) | [ ] | Naive recursive fib(30) via `fj run`. Record wall time | 20 | Time recorded in BENCHMARKS.md |
| P2.2 | Sort 100K elements | [ ] | Quicksort 100K random integers via `fj run`. Record time | 20 | Time recorded |
| P2.3 | Matrix multiply 256×256 | [ ] | `matmul(randn(256,256), randn(256,256))` via `fj run`. Record time | 20 | Time recorded |
| P2.4 | String concat 10K | [ ] | Concatenate 10,000 strings via `fj run`. Record time | 15 | Time recorded |
| P2.5 | Lexer throughput | [ ] | Lex 100K lines of .fj source. Record LOC/second | 20 | LOC/sec recorded |
| P2.6 | Parser throughput | [ ] | Parse 10K statements. Record stmts/second | 20 | Stmts/sec recorded |
| P2.7 | Effect dispatch overhead | [ ] | 10K effect operations in a loop. Record per-op overhead | 20 | ns/op recorded |
| P2.8 | GPU vs CPU comparison | [ ] | 256×256 matmul on CPU vs `gpu_matmul`. Record speedup | 20 | Speedup ratio recorded |
| P2.9 | Cold startup time | [ ] | `time fj run hello.fj`. Record wall time | 10 | Time < 100ms |
| P2.10 | Benchmark report | [ ] | Write `docs/BENCHMARKS.md` with all baseline numbers, system specs, methodology | 30 | Document complete with all 9 measurements |

**Sprint P2 Gate:** All baselines recorded in BENCHMARKS.md.

---

## Sprint P3: Security & Quality

**Goal:** Verify compiler safety and code quality to production standard.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~70
**Dependency:** Option 1 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| P3.1 | cargo audit clean | [ ] | Run `cargo audit`. Fix or document any advisories | 0 | `cargo audit` exits 0 |
| P3.2 | All unsafe documented | [ ] | Every `unsafe {}` block has `// SAFETY:` comment explaining why it's safe | 0 | `grep -r "unsafe" src/ \| grep -v SAFETY` returns 0 results (except in test/) |
| P3.3 | No unwrap in src/ | [ ] | Verify no `.unwrap()` in production code (src/ except tests) | 0 | `grep -r "\.unwrap()" src/ \| grep -v "test\|#\[test\]"` returns 0 |
| P3.4 | Recursion limit enforced | [ ] | `fj run` with `fn f() { f() }; f()` hits MAX_RECURSION_DEPTH, does not crash | 10 | Error message: "RE003: stack overflow" |
| P3.5 | Macro expansion limit | [ ] | Recursive macro definition hits expansion limit, does not loop forever | 10 | Error message includes "expansion limit" |
| P3.6 | Path traversal blocked | [ ] | `read_file("../../etc/passwd")` returns error, not file contents | 10 | Error: path traversal detected |
| P3.7 | Coverage report | [ ] | Run `cargo tarpaulin --lib` and record coverage percentage | 10 | Coverage ≥ 70% recorded |
| P3.8 | Memory leak check | [ ] | Run 5 .fj programs under valgrind, verify no leaks | 0 | Valgrind: "All heap blocks were freed" |
| P3.9 | Dependency update | [ ] | `cargo update`, run full test suite, verify no breakage | 0 | All tests pass after update |
| P3.10 | SECURITY.md update | [ ] | Update security policy with V15 status, disclosure process | 30 | Policy is current and accurate |

**Sprint P3 Gate:** All security checks pass, coverage ≥ 70%.

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
| D1.1 | Effect system tutorial | [ ] | `docs/tutorials/effects.md`: motivation, syntax, 5 examples, common patterns, comparison to monads | 200 | Document complete, all examples run with `fj run` |
| D1.2 | ML training tutorial | [ ] | `docs/tutorials/mnist.md`: data loading, model, training loop, evaluation, GPU | 200 | Document complete, code produces 90%+ accuracy |
| D1.3 | FFI tutorial | [ ] | `docs/tutorials/ffi.md`: calling C, bindgen, strings, structs, callbacks, memory | 150 | Document complete, all examples run |
| D1.4 | GPU acceleration tutorial | [ ] | `docs/tutorials/gpu.md`: gpu_available, gpu_matmul, gpu_relu, benchmarks | 100 | Document complete, all examples run |
| D1.5 | CLI tool tutorial | [ ] | `docs/tutorials/cli_tool.md`: building a word count tool step by step | 100 | Document complete, tool runs correctly |
| D1.6 | 10 new examples | [ ] | Add `examples/v15_*.fj`: effects, mnist, ffi, gpu, cli, json, csv, sort, matrix, calc | 200 | All 10 run with `fj run` |
| D1.7 | Update STDLIB_SPEC.md | [ ] | Add new builtins: tanh, gelu, leaky_relu, concat, accuracy, cross_entropy | 50 | Spec matches implementation |
| D1.8 | Update ERROR_CODES.md | [ ] | Add EE001-EE008 (effect errors) if not already documented | 30 | All effect error codes documented |
| D1.9 | Update ARCHITECTURE.md | [ ] | Add effect system data flow diagram, handle dispatch, resume mechanism | 50 | Architecture diagram accurate |
| D1.10 | Update FAJAR_LANG_SPEC.md | [ ] | Add effect declaration syntax, handle/with/resume syntax to language spec | 50 | Spec matches parser behavior |

**Sprint D1 Gate:** All tutorials complete, all examples run.

---

## Sprint D2: Gap Analysis & Honesty

**Goal:** Ensure every document claim matches the actual code. No inflation.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~550
**Dependency:** Options 2-3 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D2.1 | Re-audit GAP_ANALYSIS_V2.md | [ ] | Go through every module, verify real vs claimed. Test with `fj run` | 200 | Every claim backed by working test |
| D2.2 | Update CLAUDE.md | [ ] | Accurate version, test count, feature list. Remove any inflated claims | 100 | All stats match `cargo test` output |
| D2.3 | Update CHANGELOG.md | [ ] | Add v12.0.0 "Infinity" and v12.1.0 "Delivery" entries with honest changelogs | 100 | Changelog complete |
| D2.4 | Verify all doc stats | [ ] | Cross-check every LOC/test/file count in all docs against reality | 0 | All numbers match |
| D2.5 | Remove inflated claims | [ ] | Find and fix any remaining "100% production" claims that aren't verified | 0 | No unverified "100%" claims |
| D2.6 | Update website | [ ] | `website/index.html`: correct stats, version, feature descriptions | 20 | Website matches reality |
| D2.7 | Create KNOWN_LIMITATIONS.md | [ ] | Document: dep type syntax (V16), @gpu annotation (V16), live registry (V16) | 50 | Honest limitations documented |
| D2.8 | Update VS Code extension | [ ] | `editors/vscode/`: add effect keywords to syntax, snippets | 30 | Extension highlights effects |
| D2.9 | Update mdBook | [ ] | `book/src/`: add V15 features chapter | 50 | `mdbook build` succeeds |
| D2.10 | Cross-reference audit | [ ] | Verify CLAUDE.md, README, website, GAP_ANALYSIS all agree with each other | 0 | No contradictions between docs |

**Sprint D2 Gate:** All documentation is accurate and consistent.

---

## Sprint D3: Release v12.1.0

**Goal:** Ship v12.1.0 with all bugs fixed, integrations working, docs accurate.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~55
**Dependency:** D1-D2 complete.
**Status:** PENDING

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D3.1 | Bump Cargo.toml to v12.1.0 | [ ] | Update version field | 5 | `cargo build` succeeds |
| D3.2 | Tag v12.1.0 | [ ] | `git tag -a v12.1.0 -m "V15 Delivery — bugs fixed, integrations real, docs honest"` | 0 | Tag exists |
| D3.3 | GitHub Release v12.1.0 | [ ] | Release notes with honest changelog, link to KNOWN_LIMITATIONS.md | 0 | Release published |
| D3.4 | Update README badges | [ ] | v12.1.0, correct test count, correct LOC | 10 | Badges accurate |
| D3.5 | Deploy website | [ ] | Push updated `website/index.html` to trigger docs.yml workflow | 20 | Website shows v12.1.0 |
| D3.6 | Full test suite green | [ ] | `cargo test --lib` — all pass, 0 failures | 0 | Test result: ok. XXXX passed |
| D3.7 | Full clippy clean | [ ] | `cargo clippy -- -D warnings` — 0 warnings | 0 | Clippy clean |
| D3.8 | Format check | [ ] | `cargo fmt -- --check` — all formatted | 0 | Format clean |
| D3.9 | All examples run | [ ] | Every .fj in `examples/` and `tests/v15/` executes with `fj run` | 0 | All pass |
| D3.10 | Final honest V15_TASKS.md | [ ] | Update this file: all tasks marked [x] only if truly verified | 20 | Every [x] backed by real test |

**Sprint D3 Gate:** v12.1.0 released, all docs accurate, all examples run.

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
