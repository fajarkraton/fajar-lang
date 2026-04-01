# V15 "Delivery" — Close V14 Gaps to Production

> **Previous:** V14 "Infinity" — 205/500 [x], 160 [f], 135 [ ] (honest audit with real testing)
> **Version:** Fajar Lang v12.0.0 → v12.1.0 "Delivery"
> **Goal:** Close the 10 known gaps from V14 so every feature is [x] end-to-end
> **Scale:** 4 options, 12 sprints, 120 tasks, ~4,000 LOC
> **Principle:** NO new features. Only finish what's started. Every task verified by `fj run`.
> **Date:** 2026-04-01
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6

---

## V14 Gaps (the ONLY things V15 addresses)

| # | Gap | Current State | What's Needed |
|---|-----|--------------|---------------|
| 1 | Effect multi-step continuations | Body stops after first resume() | Fix interpreter to continue body after resume |
| 2 | `tanh()` not registered | Missing from builtin dispatch | Register in builtins.rs + type_check |
| 3 | `Dense.forward()` fails | Layer value exists, method not dispatched | Wire .forward() method on Layer values |
| 4 | Dependent type user syntax | Framework in src/dependent/, no parser syntax | NOT in V15 scope — requires major parser work |
| 5 | `@gpu fn` annotation | GPU via builtins only, no annotation | NOT in V15 scope — requires codegen pipeline |
| 6 | Package registry live server | In-memory API works, no server | Add local file-based registry mode |
| 7 | Real-world projects are mocks | Demo files simulate behavior | Replace 5 key demos with real integration tests |
| 8 | Context isolation depth | verify doesn't catch tensor-in-kernel | Add builtin blacklist check in verify pipeline |
| 9 | `resume(void)` parse error | Must use resume(null) | Document as intended behavior (void = null) |
| 10 | Bindgen struct typedef | Malformed output | Fix format string in bindgen emit |

**Scope decision:** Gaps #4 and #5 (dependent type syntax, @gpu annotation) are DEFERRED — they require
major parser/codegen work that warrants their own version. V15 focuses on bugs and wiring.

---

## Execution Order

```
Option 1: Bug Fixes .................. 3 sprints,  30 tasks  (NO dependency)
Option 2: Integration Completion ..... 3 sprints,  30 tasks  (depends on 1)
Option 3: Production Hardening ....... 3 sprints,  30 tasks  (depends on 2)
Option 4: Documentation & Release .... 3 sprints,  30 tasks  (depends on 3)

TOTAL: 12 sprints, 120 tasks, ~4,000 LOC, ~200 tests
```

---

## Option 1: Bug Fixes (3 sprints, 30 tasks)

### Context

These are concrete bugs found during real end-to-end testing. Each task has a specific
failing test case and a clear fix location in the codebase.

### Sprint B1: Effect System Fixes (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| B1.1 | Fix effect multi-step continuation | After `resume(val)`, interpreter must return to body's next statement instead of exiting handle | 50 | `fj run` with 2 effect calls in one body, both execute |
| B1.2 | Add resume stack tracking | Track resume point so body continues after handler returns | 30 | Test: `Console::log("a"); Console::log("b")` both print |
| B1.3 | Fix resume return value propagation | `resume(42)` should make the effect call site return 42 | 20 | `let x = State::get()` returns resumed value |
| B1.4 | Handle multiple effect types in one handler | `handle { } with { IO::log(...) => ..., State::get() => ... }` | 30 | Test: both handlers fire |
| B1.5 | Add `resume(void)` as alias for `resume(null)` | Parser should accept `resume()` or `resume(void)` | 10 | `fj run` with `resume()` works |
| B1.6 | Effect handler variable scoping fix | Handler params should shadow outer scope correctly | 20 | Test: handler param `msg` doesn't conflict with outer `msg` |
| B1.7 | Nested handle expressions | `handle { handle { inner } with { ... } } with { ... }` | 30 | Inner handler catches inner effect, outer catches outer |
| B1.8 | Effect with return type | `effect State { fn get() -> i32 }` returns typed value | 20 | `let x = State::get()` has correct type |
| B1.9 | Effect operation arity check | Calling effect op with wrong arg count gives clear error | 15 | Error message includes expected vs actual count |
| B1.10 | End-to-end effect test suite | 10 .fj example programs covering all effect patterns | 50 | All 10 programs run correctly |

### Sprint B2: ML Runtime Fixes (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| B2.1 | Register `tanh()` as builtin | Add to builtins.rs dispatch + type_check register | 10 | `fj run` with `tanh(tensor)` works |
| B2.2 | Register `gelu()` as builtin | Add to builtins.rs dispatch + type_check register | 10 | `fj run` with `gelu(tensor)` works |
| B2.3 | Register `leaky_relu()` as builtin | Add to builtins.rs dispatch + type_check register | 10 | `fj run` with `leaky_relu(tensor)` works |
| B2.4 | Fix `Dense.forward()` method dispatch | Wire `.forward()` on Layer value type via method resolution | 30 | `let y = layer.forward(x)` works |
| B2.5 | Fix `Conv2d.forward()` method dispatch | Same as B2.4 for Conv2d layers | 20 | `let y = conv.forward(x)` works |
| B2.6 | Register `flatten()` as builtin | Add to builtins.rs if missing | 10 | `fj run` with `flatten(tensor)` works |
| B2.7 | Register `concat()` as builtin | Add to builtins.rs if missing | 10 | `fj run` with `concat(t1, t2, axis)` works |
| B2.8 | Fix `cross_entropy()` as builtin | Ensure loss function is registered | 10 | `fj run` with `cross_entropy(pred, target)` works |
| B2.9 | Add `accuracy()` metric builtin | Register from metrics module | 10 | `fj run` with `accuracy(pred, target)` works |
| B2.10 | End-to-end MNIST training test | Full training loop: data → model → loss → backward → step | 80 | `fj run examples/mnist_train.fj` trains and prints accuracy |

### Sprint B3: Toolchain Fixes (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| B3.1 | Fix bindgen struct typedef output | Fix format string for struct → type generation | 15 | `fj bindgen struct.h` produces valid Fajar code |
| B3.2 | Deepen context isolation in verify | Add tensor builtin blacklist check for @kernel functions | 30 | `fj verify` catches `zeros()` inside @kernel |
| B3.3 | Add `fj verify --strict` mode | Strict mode fails on warnings too | 20 | `fj verify --strict` returns non-zero on warnings |
| B3.4 | Fix `fj build` without native feature | Show helpful error message instead of crash | 10 | Clear message: "install with --features native" |
| B3.5 | Add `fj run --check-only` flag | Parse + analyze without executing | 10 | `fj run --check-only file.fj` exits 0 on valid code |
| B3.6 | Fix LSP completion for effect keywords | Add `effect`, `handle`, `with`, `resume` to completion | 15 | VS Code shows effect keywords in autocomplete |
| B3.7 | Fix LSP semantic tokens for effects | Highlight `effect` declarations and `handle/with` blocks | 20 | VS Code colors effect code correctly |
| B3.8 | Add `fj registry init` command | Initialize local file-based package registry | 40 | `fj registry init ~/.fj/registry` creates registry dir |
| B3.9 | Add `fj publish --local` flag | Publish to local registry instead of remote | 30 | `fj publish --local` stores package in local registry |
| B3.10 | End-to-end toolchain test | Run fj new → fj check → fj test → fj fmt → fj verify → fj run | 20 | Full workflow works in sequence |

---

## Option 2: Integration Completion (3 sprints, 30 tasks)

### Context

Replace mock demos with real integration tests. Focus on the 5 most impactful
projects that exercise different language features.

### Sprint I1: Real MNIST Training (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I1.1 | Create MNIST data loader in .fj | Read IDX format files, normalize to f32 | 80 | `fj run mnist_loader.fj` loads 60K images |
| I1.2 | Define CNN model in .fj | Conv2d → relu → Dense → softmax | 40 | Model creates without error |
| I1.3 | Training loop in .fj | Forward → loss → backward → SGD step | 50 | Loss decreases over 10 batches |
| I1.4 | Accuracy evaluation in .fj | Test set accuracy computation | 30 | Prints accuracy percentage |
| I1.5 | Batch processing in .fj | Process multiple images per forward pass | 20 | Batch of 32 processes correctly |
| I1.6 | Save/load model weights | Serialize model state to file | 40 | Save then load produces same predictions |
| I1.7 | Training progress output | Print loss/accuracy per epoch | 15 | Formatted output each epoch |
| I1.8 | Achieve 90%+ accuracy | Train enough epochs on MNIST | 0 | Test accuracy ≥ 90% |
| I1.9 | GPU acceleration option | Use gpu_matmul when available | 20 | Training faster with GPU |
| I1.10 | Tutorial document | Step-by-step guide for MNIST in Fajar Lang | 100 | docs/tutorials/mnist.md complete |

### Sprint I2: Real FFI Integration (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I2.1 | C math library FFI | Call `sin`, `cos`, `sqrt` from libc via FFI | 30 | `fj run` with FFI calls returns correct values |
| I2.2 | Generate bindings for math.h | `fj bindgen /usr/include/math.h` | 0 | Bindings file generated |
| I2.3 | Use generated bindings | Import and call generated functions | 20 | Results match expected values |
| I2.4 | C string interop | Pass strings between Fajar and C | 30 | Round-trip string preserved |
| I2.5 | C struct interop | Pass struct data to C function | 30 | Struct fields accessible in C |
| I2.6 | Callback from C to Fajar | C function calls Fajar callback | 40 | Callback executes with correct args |
| I2.7 | Error handling across FFI | C returns error, Fajar handles it | 20 | Error propagated as Result::Err |
| I2.8 | Memory management across FFI | Allocate in C, free in Fajar (and vice versa) | 30 | No leaks (verified by test) |
| I2.9 | FFI performance benchmark | Compare FFI call overhead | 20 | Overhead < 100ns per call |
| I2.10 | FFI integration test suite | 10 test cases covering all FFI patterns | 50 | All 10 pass |

### Sprint I3: Real CLI Tool in Fajar Lang (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| I3.1 | Word count tool | Read file, count lines/words/chars | 40 | `fj run wc.fj input.txt` matches `wc` |
| I3.2 | JSON pretty printer | Parse JSON string, format with indentation | 60 | `fj run json_fmt.fj '{"a":1}'` outputs formatted |
| I3.3 | CSV to JSON converter | Read CSV, output JSON array | 60 | `fj run csv2json.fj data.csv` outputs valid JSON |
| I3.4 | File search tool (grep-like) | Search for pattern in files | 50 | `fj run search.fj pattern file.txt` finds matches |
| I3.5 | Calculator REPL | Interactive arithmetic calculator | 40 | Evaluates expressions correctly |
| I3.6 | Fibonacci benchmark | Compute fib(35), time it | 20 | Runs in < 2 seconds |
| I3.7 | Sorting algorithms | Implement quicksort + mergesort in .fj | 60 | Sort 10K elements correctly |
| I3.8 | String manipulation tool | Reverse, uppercase, base64 encode | 40 | All operations produce correct output |
| I3.9 | Matrix operations | Matrix multiply, transpose, determinant | 50 | Results match known values |
| I3.10 | CLI tools test suite | Run all 9 tools, verify output | 30 | All pass |

---

## Option 3: Production Hardening (3 sprints, 30 tasks)

### Sprint P1: Fuzz Testing (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| P1.1 | Lexer fuzz harness | cargo-fuzz target for tokenize() | 20 | 100K iterations, no crash |
| P1.2 | Parser fuzz harness | cargo-fuzz target for parse() | 20 | 100K iterations, no crash |
| P1.3 | Analyzer fuzz harness | cargo-fuzz target for analyze() | 20 | 100K iterations, no crash |
| P1.4 | Interpreter fuzz harness | cargo-fuzz target for eval_source() | 20 | 100K iterations, no crash |
| P1.5 | Effect system fuzz | Random effect declarations + handles | 20 | 100K iterations, no crash |
| P1.6 | Tensor ops fuzz | Random tensor shapes + operations | 20 | 100K iterations, no crash |
| P1.7 | FFI boundary fuzz | Random types at FFI boundary | 20 | 100K iterations, clean reject |
| P1.8 | Format string fuzz | Random f-string interpolation | 15 | 100K iterations, no injection |
| P1.9 | REPL fuzz | Random input sequences | 15 | 100K iterations, no hang |
| P1.10 | CI fuzz integration | Run fuzzers in GitHub Actions (5 min budget) | 20 | CI job passes |

### Sprint P2: Performance Benchmarks (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| P2.1 | fibonacci(30) benchmark | Tree-recursive, measure time | 20 | Time recorded, < 1 second |
| P2.2 | Sort 100K elements | Quicksort in Fajar Lang | 20 | Time recorded |
| P2.3 | Matrix multiply 256x256 | Using tensor matmul | 20 | Time recorded |
| P2.4 | String concat 10K | Repeated string concatenation | 15 | Time recorded |
| P2.5 | Lexer throughput | Lex 100K lines of .fj code | 20 | LOC/sec recorded |
| P2.6 | Parser throughput | Parse 10K statements | 20 | Stmts/sec recorded |
| P2.7 | Effect dispatch overhead | 10K effect operations | 20 | Overhead per operation |
| P2.8 | GPU vs CPU comparison | matmul on CPU vs GPU | 20 | Speedup recorded |
| P2.9 | Startup time | Cold start measurement | 10 | < 50ms |
| P2.10 | Benchmark report | Record all baselines in docs/BENCHMARKS.md | 30 | Document complete |

### Sprint P3: Security & Quality (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| P3.1 | cargo audit — 0 advisories | Run and fix any vulnerabilities | 0 | `cargo audit` clean |
| P3.2 | All unsafe blocks documented | Every `unsafe` has SAFETY comment | 0 | Grep confirms |
| P3.3 | No unwrap in src/ | Verify no .unwrap() in production code | 0 | Grep confirms |
| P3.4 | Recursion depth limits | Verify MAX_RECURSION_DEPTH enforced | 10 | `fj run` with deep recursion hits limit |
| P3.5 | Macro expansion limits | Verify no infinite macro expansion | 10 | Recursive macro hits limit |
| P3.6 | Input validation | File paths, CLI args validated | 20 | Path traversal blocked |
| P3.7 | Coverage report | tarpaulin coverage > 70% | 10 | Coverage percentage recorded |
| P3.8 | Memory leak check | Run valgrind on 5 programs | 0 | No leaks |
| P3.9 | Update all dependencies | `cargo update`, verify no breakage | 0 | All tests still pass |
| P3.10 | Security policy | Update SECURITY.md with V15 status | 20 | Policy current |

---

## Option 4: Documentation & Release (3 sprints, 30 tasks)

### Sprint D1: Examples & Tutorials (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D1.1 | Effect system tutorial | Step-by-step guide with 5 examples | 200 | docs/tutorials/effects.md |
| D1.2 | ML training tutorial | MNIST from scratch | 200 | docs/tutorials/mnist.md |
| D1.3 | FFI tutorial | Calling C from Fajar Lang | 150 | docs/tutorials/ffi.md |
| D1.4 | GPU acceleration tutorial | Using gpu_* builtins | 100 | docs/tutorials/gpu.md |
| D1.5 | CLI tool tutorial | Building a word count tool | 100 | docs/tutorials/cli_tool.md |
| D1.6 | Update examples/ directory | Add 10 new runnable .fj examples | 200 | All 10 run with `fj run` |
| D1.7 | Update STDLIB_SPEC.md | Add new builtins (tanh, gelu, etc.) | 50 | Spec matches implementation |
| D1.8 | Update ERROR_CODES.md | Add EE001-EE008 effect errors | 30 | All codes documented |
| D1.9 | Update ARCHITECTURE.md | Add effect system architecture | 50 | Diagram accurate |
| D1.10 | Update FAJAR_LANG_SPEC.md | Add effect syntax specification | 50 | Spec matches parser |

### Sprint D2: Gap Analysis & Honesty (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D2.1 | Update GAP_ANALYSIS_V2.md | Re-audit every module honestly | 200 | All claims match reality |
| D2.2 | Update CLAUDE.md to v11.0 | Accurate status, test counts, feature list | 100 | No inflated claims |
| D2.3 | Update CHANGELOG.md | v12.0.0 and v12.1.0 entries | 100 | Complete changelog |
| D2.4 | Verify all doc claims | Cross-check every doc stat against code | 0 | All accurate |
| D2.5 | Remove inflated statistics | Fix any remaining overcounts | 0 | All stats honest |
| D2.6 | Update website stats | Reflect actual verified capabilities | 20 | Website accurate |
| D2.7 | Create KNOWN_LIMITATIONS.md | Document what doesn't work yet | 100 | Honest limitations |
| D2.8 | Update VS Code extension docs | Match current LSP capabilities | 30 | Extension docs accurate |
| D2.9 | Update book/ (mdBook) | Add V15 features to documentation book | 100 | Book builds, content accurate |
| D2.10 | Cross-reference audit | Every doc references match each other | 0 | No contradictions |

### Sprint D3: Release v12.1.0 (10 tasks)

| # | Task | Detail | LOC | Verify |
|---|------|--------|-----|--------|
| D3.1 | Bump Cargo.toml to v12.1.0 | Version update | 5 | `cargo build` succeeds |
| D3.2 | Tag v12.1.0 | Annotated git tag | 0 | Tag exists |
| D3.3 | GitHub Release v12.1.0 | Release notes with honest changelog | 0 | Release published |
| D3.4 | Update README badges | v12.1.0, correct test count | 10 | Badges accurate |
| D3.5 | Update website | New stats, feature descriptions | 20 | Page shows v12.1.0 |
| D3.6 | Run full test suite | All tests pass, 0 failures | 0 | `cargo test --lib` green |
| D3.7 | Run full clippy | 0 warnings | 0 | `cargo clippy` clean |
| D3.8 | Run format check | All code formatted | 0 | `cargo fmt --check` clean |
| D3.9 | Verify all examples run | Every .fj example executes | 0 | All examples pass |
| D3.10 | Final honest assessment | Update V15_TASKS.md with actual status | 20 | All tasks marked honestly |

---

## What V15 Does NOT Include (Deferred to V16+)

| Feature | Reason | When |
|---------|--------|------|
| Dependent type user syntax (Pi/Sigma) | Requires major parser redesign | V16 |
| `@gpu fn` annotation | Requires codegen pipeline from parser to SPIR-V | V16 |
| Live package registry server | Infrastructure requirement beyond language | V16 |
| WASI P2 real deployment (Fermyon) | External service dependency | V16 |
| Distributed MNIST (multi-node) | Requires real network infra | V16 |

---

## Quality Gates

### Per-Task
- [ ] Test file created before implementation (TDD)
- [ ] `fj run test_file.fj` produces expected output
- [ ] No .unwrap() in src/
- [ ] cargo clippy clean

### Per-Sprint
- [ ] All 10 tasks verified end-to-end
- [ ] No regressions (full test suite passes)
- [ ] At least 1 new .fj example program
- [ ] Commit with proper message

### Per-Option
- [ ] All sprints complete
- [ ] Integration test covers the option's scope
- [ ] Documentation updated

### Release (v12.1.0)
- [ ] All 120 tasks [x]
- [ ] 0 known [f] tasks remaining
- [ ] All examples run
- [ ] All docs accurate
- [ ] Website updated

---

*V15 "Delivery" Plan — Version 1.0 | 120 tasks, 12 sprints, 4 options | 2026-04-01*
*Author: Fajar (PrimeCore.id) + Claude Opus 4.6*
