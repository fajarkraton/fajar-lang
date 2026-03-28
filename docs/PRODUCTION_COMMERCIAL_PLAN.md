# PRODUCTION_COMMERCIAL_PLAN.md — 100% Production & Commercial Ready

> **Date:** 2026-03-28
> **Author:** Claude Opus 4.6 exclusively
> **Scope:** Fajar Lang V0-V8 + FajarOS Nova — close ALL gaps to production/commercial level
> **Based on:** Full re-audit of 342 files, 334,821 LOC, 5,554 tests, 21,187-line kernel
> **Rule:** Only Claude Opus 4.6. No other models. No exceptions.

---

## Executive Summary

After auditing every module from V0 to V8 plus FajarOS Nova, there are **7 categories
of gaps** preventing true 100% production/commercial readiness. This plan addresses
ALL of them across **12 phases, 200 tasks**.

| Category | Gap Count | Impact |
|----------|-----------|--------|
| A. Type Checker Limitations | 214 kernel errors | Kernel can't pass `fj check` clean |
| B. Missing Interpreter Builtins | ~15 builtins | Templates reference non-existent functions |
| C. Compiler Pipeline Integration | ~20 items | Security/profiler exist but not auto-injected |
| D. External Integration | 3 modules | No real libclang/pyo3/windowing |
| E. Playground WASM | 15 items | Browser playground doesn't work |
| F. Benchmark Suite | 30 items | No cross-language performance comparison |
| G. FajarOS Nova Production | ~40 items | Kernel needs native compile, CI, verification |

---

## Phase 1: Type Checker Fixes (25 tasks)

**Goal:** Fix the 6 type inference patterns that cause 214 false-positive errors on
real kernel code. This is the #1 blocker — the kernel should pass `fj check` with 0 errors.

### P1.1: Void-then-integer pattern (122 errors)

Pattern: `while cond { ... return x } -1` — the `-1` after a void-typed while
is flagged as "cannot use `-` between void and integer".

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P1.1.1 | Analyze while/if/for expression typing in type checker | `src/analyzer/type_check/mod.rs` | Understand current logic |
| P1.1.2 | When a block ends with a bare expression after while/if, treat the while/if as a statement (void), and the bare expr as the block's return type | `type_check/mod.rs` | Pattern compiles |
| P1.1.3 | Add test: `fn f() -> i64 { while false {} -1 }` should return i64, not error | `type_check/mod.rs` tests | Test passes |
| P1.1.4 | Add test: `fn f() -> i64 { if false { return 0 } -1 }` should return i64 | tests | Test passes |
| P1.1.5 | Verify kernel void-then-integer errors reduced to 0 | `fj check kernel.fj` | 0 SE004 of this pattern |

### P1.2: i64-vs-str coercion (42 errors)

Pattern: `cprint("text", WHITE_ON_BLACK)` where analyzer can't determine function param types
because `cprint` is user-defined in the same file.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P1.2.1 | Improve forward-declaration handling: scan all `fn` signatures before checking bodies | `type_check/mod.rs` | Forward refs resolve |
| P1.2.2 | Register user-defined function signatures in first pass | `type_check/check.rs` | fn types available |
| P1.2.3 | Verify kernel str/i64 mismatch errors reduced to 0 | `fj check kernel.fj` | 0 SE004 of this pattern |

### P1.3: Bitwise-AND-vs-comparison (30 errors)

Pattern: `if flags & 0x80 == 0` — parsed as `flags & (0x80 == 0)` due to precedence,
resulting in "i64 and bool" type error.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P1.3.1 | Analyze if this is a parser or type checker issue | `src/parser/pratt.rs` | Determine root cause |
| P1.3.2 | If parser: adjust `&` vs `==` precedence (C-style: `&` binds tighter than `==`) | `pratt.rs` | `a & 0x80 == 0` parsed as `(a & 0x80) == 0` |
| P1.3.3 | If type checker: add coercion from `bool` to `i64` for bitwise context | `type_check/mod.rs` | Expression type-checks |
| P1.3.4 | Add tests for `x & 0xFF == 0`, `x | 0x01 != 0` | tests | All pass |
| P1.3.5 | Verify kernel bitwise errors reduced to 0 | `fj check kernel.fj` | 0 SE004 of this pattern |

### P1.4: Void-in-if-else branches (6 errors)

Pattern: `if cond { port_outb(...) } else if cond2 { ... }` — void function in if branch.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P1.4.1 | Allow if/else branches to have void type when used as statements | `type_check/mod.rs` | Void branches OK as stmt |
| P1.4.2 | Only error when if/else is used as expression AND branches disagree | `type_check/mod.rs` | `let x = if ... {}` still errors |
| P1.4.3 | Verify kernel void-branch errors reduced to 0 | `fj check kernel.fj` | 0 SE004 of this pattern |

### P1.5: Arity mismatches (8 errors)

Pattern: Functions called with wrong number of arguments.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P1.5.1 | Fix `tcp_connect` call: kernel uses 2 args, builtin expects 4 — add overload or fix kernel | `kernel.fj` or `register.rs` | Arity matches |
| P1.5.2 | Fix `sys_gpu_dispatch` call: kernel uses 3 args, definition has 4 — fix kernel | `kernel.fj` | Arity matches |
| P1.5.3 | Fix remaining 2 arity mismatches | `kernel.fj` or `register.rs` | All SE005 resolved |
| P1.5.4 | Verify 0 SE005 errors on kernel | `fj check kernel.fj` | Clean |

### P1.6: Final kernel verification

| Task | Description | Verify |
|------|-------------|--------|
| P1.6.1 | Run `fj check examples/fajaros_nova_kernel.fj` — must show "OK: no errors found" | 0 errors |
| P1.6.2 | Run `fj check` on all Phoenix/Aurora .fj files | All clean |
| P1.6.3 | Run `fj check` on all 126+ example .fj files | All clean or known warnings only |

---

## Phase 2: Interpreter Builtins (15 tasks)

**Goal:** Implement WebSocket, MQTT, BLE builtins so app templates run end-to-end.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P2.1 | `ws_connect(url)` → handle | `eval/builtins.rs` | Returns handle |
| P2.2 | `ws_send(handle, msg)` → bytes sent | `eval/builtins.rs` | Echo simulation |
| P2.3 | `ws_recv(handle)` → msg or null | `eval/builtins.rs` | Returns message |
| P2.4 | `ws_close(handle)` | `eval/builtins.rs` | Connection closed |
| P2.5 | `mqtt_connect(broker)` → handle | `eval/builtins.rs` | Returns handle |
| P2.6 | `mqtt_publish(handle, topic, payload)` | `eval/builtins.rs` | Message queued |
| P2.7 | `mqtt_subscribe(handle, topic)` | `eval/builtins.rs` | Subscription active |
| P2.8 | `mqtt_recv(handle)` → {topic, payload} | `eval/builtins.rs` | Returns message map |
| P2.9 | `mqtt_disconnect(handle)` | `eval/builtins.rs` | Cleanup |
| P2.10 | `ble_scan()` → device list | `eval/builtins.rs` | Simulated devices |
| P2.11 | `ble_connect(addr)` → handle | `eval/builtins.rs` | Connection |
| P2.12 | `ble_read(handle, uuid)` → bytes | `eval/builtins.rs` | Data returned |
| P2.13 | `ble_write(handle, uuid, data)` | `eval/builtins.rs` | Data written |
| P2.14 | Register all in type checker | `type_check/register.rs` | Analyzer accepts |
| P2.15 | Tests: ws roundtrip, mqtt pub/sub, ble scan/connect | `tests/` | 6 tests pass |

---

## Phase 3: Compiler Pipeline Integration (15 tasks)

**Goal:** Security checks, profiler, optimizer produce real effects in compiled output.

> Note: Phase 1 of GAP_CLOSURE_PLAN_V9 partially completed this. Remaining:

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P3.1 | Security linter auto-runs on `fj build` (not just `--lint`) in debug mode | `cranelift/mod.rs` | Lint warnings on build |
| P3.2 | Bounds check calls emitted in `compile_index` when security enabled | `cranelift/compile/arrays.rs` | Array OOB trapped |
| P3.3 | Overflow check calls emitted in `compile_expr` for `+`/`-`/`*` when security enabled | `cranelift/compile/expr.rs` | Overflow trapped |
| P3.4 | Stack canary prologue/epilogue in native compiled functions | `cranelift/compile/mod.rs` | Canary verified |
| P3.5 | Profiler enter/exit calls in native compiled functions (when `--profile`) | `cranelift/compile/call.rs` | Profile recorded |
| P3.6 | `fj build --opt-level 2` runs real optimization pipeline | `cranelift/mod.rs` | O2 passes applied |
| P3.7 | Optimizer report printed with `--verbose` | `cranelift/mod.rs` | Report shown |
| P3.8 | Tests: build with security → trap on OOB | `cranelift/tests.rs` | 3 tests |
| P3.9 | Tests: build with profile → JSON output | tests | 2 tests |
| P3.10 | Tests: build with O2 → optimization count > 0 | tests | 2 tests |
| P3.11 | Benchmark: security overhead < 15% | benches | Measured |
| P3.12 | Benchmark: profiler overhead < 5% | benches | Measured |
| P3.13 | Document `--security`, `--profile`, `--opt-level` in help text | `main.rs` | `fj build --help` shows |
| P3.14 | Document security features in `book/src/guides/security.md` | `book/` | Guide exists |
| P3.15 | Document profiler in `book/src/guides/performance.md` | `book/` | Guide exists |

---

## Phase 4: External Integration (20 tasks)

**Goal:** Make feature-gated code work end-to-end with real external dependencies.

### P4.1: GUI OS Windowing via winit

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P4.1.1 | Add `winit = { version = "0.30", optional = true }` | `Cargo.toml` | Compiles |
| P4.1.2 | Add `softbuffer = { version = "0.4", optional = true }` | `Cargo.toml` | Compiles |
| P4.1.3 | Add `gui = ["dep:winit", "dep:softbuffer"]` feature | `Cargo.toml` | Feature defined |
| P4.1.4 | `WinitBackend::open_window(config)` → real OS window | `gui/platform.rs` | Window visible |
| P4.1.5 | Event loop translating winit events → GUI events | `gui/platform.rs` | Mouse/key work |
| P4.1.6 | `SoftbufferRenderer` blits Canvas to OS window | `gui/platform.rs` | Pixels on screen |
| P4.1.7 | `fj gui app.fj` command in CLI | `main.rs` | Command works |
| P4.1.8 | DPI scaling via winit::dpi | `gui/platform.rs` | HiDPI correct |
| P4.1.9 | Example: `gui_hello.fj` with button + label | `examples/` | Window opens |
| P4.1.10 | Tests (headless via feature gate) | `gui/` tests | Pass in CI |

### P4.2: CI for Feature-Gated Code

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P4.2.1 | CI job: `cargo test --features smt` with libz3-dev | `ci.yml` | Job passes |
| P4.2.2 | CI job: `cargo test --features cpp-ffi` with libclang-dev | `ci.yml` | Job passes |
| P4.2.3 | CI job: `cargo test --features python-ffi` with python3-dev | `ci.yml` | Job passes |
| P4.2.4 | CI job: `cargo test --features gui` (headless) | `ci.yml` | Job passes |
| P4.2.5 | Document all feature flags in README | `README.md` | Flags listed |

### P4.3: Platform Detection Real Implementation

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P4.3.1 | Query real display count via winit (when gui feature) | `gui/platform.rs` | Real count |
| P4.3.2 | Query real monitor resolution via winit | `gui/platform.rs` | Real resolution |
| P4.3.3 | Probe for Vulkan/OpenGL via libloading | `gui/platform.rs` | Real GPU detection |
| P4.3.4 | Detect real installed GPU (CUDA/Vulkan/OpenCL) | `hw/gpu.rs` | Real detection |
| P4.3.5 | Tests for platform detection | `gui/platform.rs` tests | Pass |

---

## Phase 5: Playground WASM (15 tasks)

**Goal:** Build a real working browser playground that compiles and runs .fj code.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P5.1 | Add `wasm-bindgen = "0.2"` to Cargo.toml (wasm32 target) | `Cargo.toml` | Dep resolves |
| P5.2 | Create `src/playground/wasm_api.rs` with `#[wasm_bindgen]` exports | `src/playground/` | Compiles |
| P5.3 | Export `eval_source(code) → String` for wasm | wasm_api.rs | Works |
| P5.4 | Export `tokenize(code) → String` for wasm | wasm_api.rs | Works |
| P5.5 | Export `format_code(code) → String` for wasm | wasm_api.rs | Works |
| P5.6 | Build script for wasm-pack | `build-playground.sh` | Produces pkg/ |
| P5.7 | Build `playground/pkg/` with wasm-pack | `playground/pkg/` | .wasm exists |
| P5.8 | Wire playground JS to import from `../pkg/` | `playground/src/` | JS works |
| P5.9 | Test: playground runs `let x = 42; println(x)` | manual | Output "42" |
| P5.10 | Test: playground shows parse errors | manual | Errors shown |
| P5.11 | Add wasm32 build to CI | `ci.yml` | CI builds |
| P5.12 | Deploy playground with docs | `docs.yml` | Deployed |
| P5.13 | Update "try in playground" links in book | `book/` | Links work |
| P5.14 | Test 5 example programs in playground | manual | All correct |
| P5.15 | Document playground in `playground/README.md` | `playground/` | Documented |

---

## Phase 6: Benchmark Suite (30 tasks)

**Goal:** Cross-language performance comparison: Fajar Lang vs Rust vs C vs Python.

### P6.1: Microbenchmarks (10 tasks)

| Task | Description | Verify |
|------|-------------|--------|
| P6.1.1 | Fibonacci recursive (fj vs Rust vs C vs Python) | Results table |
| P6.1.2 | Fibonacci iterative | Results table |
| P6.1.3 | Quicksort N=100K | Results table |
| P6.1.4 | String concatenation 100K iterations | Results table |
| P6.1.5 | HashMap insert/lookup 100K entries | Results table |
| P6.1.6 | Matrix multiply 128x128 | Results table |
| P6.1.7 | Tokenize 10K lines | Results table |
| P6.1.8 | Pattern matching 100 branches | Results table |
| P6.1.9 | Closure call overhead 1M calls | Results table |
| P6.1.10 | Compile time for 5K-line program | Results table |

### P6.2: Application Benchmarks (10 tasks)

| Task | Description | Verify |
|------|-------------|--------|
| P6.2.1 | Binary trees (alloc/dealloc) | Results table |
| P6.2.2 | N-body simulation | Results table |
| P6.2.3 | Mandelbrot fractal | Results table |
| P6.2.4 | JSON parsing 1MB | Results table |
| P6.2.5 | HTTP request throughput | Results table |
| P6.2.6 | MNIST training 1 epoch | Results table |
| P6.2.7 | Regex matching 1MB text | Results table |
| P6.2.8 | File I/O 100MB | Results table |
| P6.2.9 | Channel throughput | Results table |
| P6.2.10 | Peak memory usage | Results table |

### P6.3: Infrastructure (10 tasks)

| Task | Description | Verify |
|------|-------------|--------|
| P6.3.1 | Benchmark harness with warm-up + iterations | Script works |
| P6.3.2 | Statistical analysis (mean, median, stddev, percentiles) | Stats computed |
| P6.3.3 | Comparison charts (bar charts) | Charts generated |
| P6.3.4 | CI integration (run on each release) | CI job works |
| P6.3.5 | Historical tracking across versions | Data stored |
| P6.3.6 | Regression detection (alert if >10% worse) | Alert works |
| P6.3.7 | `BENCHMARKS.md` with formatted results | Doc exists |
| P6.3.8 | Website benchmark page | Page deployed |
| P6.3.9 | Blog: "Fajar Lang Performance" | Blog published |
| P6.3.10 | Optimization guide for users | Guide in book |

---

## Phase 7: FajarOS Nova Production (40 tasks)

**Goal:** Make FajarOS Nova kernel genuinely production-ready — native compile,
automated testing, QEMU CI, hardware verification.

### P7.1: Native Kernel Compilation (10 tasks)

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P7.1.1 | `fj build --target x86_64-none-elf kernel.fj` produces flat binary | `cranelift/` | Binary output |
| P7.1.2 | Bare-metal startup code (boot.S + linker script) for kernel | `examples/` | Boots in QEMU |
| P7.1.3 | Link kernel .fj → ELF via Cranelift ObjectCompiler | `cranelift/mod.rs` | ELF produced |
| P7.1.4 | Multiboot2 header in compiled kernel | linker script | GRUB loads it |
| P7.1.5 | VGA output from native compiled kernel | runtime_bare | Text on screen |
| P7.1.6 | Serial output from native compiled kernel | runtime_bare | Serial log |
| P7.1.7 | Interrupt handling (IDT) in native compiled kernel | runtime_bare | Timer ticks |
| P7.1.8 | Memory management in native compiled kernel | runtime_bare | alloc/free work |
| P7.1.9 | `make run` builds and boots kernel in QEMU | Makefile | Boots |
| P7.1.10 | Document native kernel build process | docs | Documented |

### P7.2: QEMU CI Pipeline (10 tasks)

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P7.2.1 | GitHub Actions job that boots kernel in QEMU | `ci.yml` | Job passes |
| P7.2.2 | Serial output capture and assertion | CI script | Output verified |
| P7.2.3 | Test: boot to shell prompt | CI | "nova>" seen |
| P7.2.4 | Test: basic commands (help, uname, ps) | CI | Correct output |
| P7.2.5 | Test: file operations (touch, cat, ls) | CI | Files created |
| P7.2.6 | Test: process management (fork, exec) | CI | Processes run |
| P7.2.7 | Test: network stack (ping, HTTP) | CI | Packets flow |
| P7.2.8 | Test: SMP (4 cores) | CI | All cores active |
| P7.2.9 | Timeout: 60 seconds max per test | CI | No hangs |
| P7.2.10 | Report: pass/fail summary | CI | Summary posted |

### P7.3: Kernel Code Quality (10 tasks)

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P7.3.1 | Fix all 4 SE005 arity mismatches in kernel | `kernel.fj` | 0 SE005 |
| P7.3.2 | Fix all SE004 after Phase 1 type checker improvements | `kernel.fj` | 0 SE004 |
| P7.3.3 | Add doc comments to all 819 @kernel functions | `kernel.fj` | `///` on each fn |
| P7.3.4 | Split kernel into modules (memory.fj, process.fj, fs.fj, net.fj, shell.fj) | `examples/nova/` | Modular structure |
| P7.3.5 | Add kernel unit tests in .fj (using `@test` framework) | `examples/nova/tests/` | Tests run |
| P7.3.6 | Code review: remove dead code, unused variables | `kernel.fj` | 0 SE009 warnings |
| P7.3.7 | Verify all 240+ shell commands work | test script | All pass |
| P7.3.8 | Verify all 34 syscalls work | test script | All pass |
| P7.3.9 | Memory leak audit: track alloc/free balance | kernel analysis | No leaks |
| P7.3.10 | Stack overflow protection in all recursive functions | kernel code | Guards present |

### P7.4: Hardware Verification (10 tasks)

| Task | Description | Verify |
|------|-------------|--------|
| P7.4.1 | Boot on real x86_64 hardware (Lenovo Legion) | Boots |
| P7.4.2 | Serial output on real UART | Serial visible |
| P7.4.3 | VGA text mode on real display | Text on screen |
| P7.4.4 | NVMe detection on real hardware | Drive detected |
| P7.4.5 | USB detection on real hardware | Devices listed |
| P7.4.6 | Network on real hardware (if NIC supported) | Ping works |
| P7.4.7 | ARM64 build + Q6A boot (FajarOS Surya) | Boots on Q6A |
| P7.4.8 | Q6A GPIO test | LED blinks |
| P7.4.9 | Q6A QNN inference test | Model runs |
| P7.4.10 | Document hardware test results | Results documented |

---

## Phase 8: Self-Hosting Completeness (10 tasks)

**Goal:** Self-hosted compiler (in .fj) can compile itself.

| Task | Description | File | Verify |
|------|-------------|------|--------|
| P8.1 | `stdlib/ast.fj` — AST node types | `stdlib/ast.fj` | Types defined |
| P8.2 | Upgrade `stdlib/parser.fj` to return AST (not positions) | `stdlib/parser.fj` | AST returned |
| P8.3 | `stdlib/analyzer.fj` — basic type checking | `stdlib/analyzer.fj` | Type errors caught |
| P8.4 | `stdlib/codegen.fj` — emit Cranelift IR (or bytecode) | `stdlib/codegen.fj` | Code emitted |
| P8.5 | Bootstrap test: self-hosted lexer tokenizes itself | test | Output matches |
| P8.6 | Bootstrap test: self-hosted parser parses itself | test | AST matches |
| P8.7 | Bootstrap test: compile hello.fj via self-hosted pipeline | test | "Hello" printed |
| P8.8 | Document bootstrap process | docs | Documented |
| P8.9 | Measure self-hosted vs Rust-hosted performance | bench | Comparison |
| P8.10 | Blog: "Fajar Lang compiles itself" | docs/BLOG | Published |

---

## Phase 9: Package Ecosystem (10 tasks)

**Goal:** Package registry usable by external developers.

| Task | Description | Verify |
|------|-------------|--------|
| P9.1 | `fj publish` uploads to registry | Package published |
| P9.2 | `fj install pkg` downloads + installs | Package installed |
| P9.3 | `fj search query` finds packages | Results shown |
| P9.4 | `fj update` checks for newer versions | Updates found |
| P9.5 | `fj audit` checks for known vulnerabilities | Audit runs |
| P9.6 | Registry web UI (basic HTML) | Page loads |
| P9.7 | 10 published packages (fj-math through fj-crypto) | All installable |
| P9.8 | Dependency resolution (pubgrub) works | Resolved correctly |
| P9.9 | Lock file (`fj.lock`) generated | Lock file exists |
| P9.10 | Document package publishing in book | Guide in book |

---

## Phase 10: Documentation Completeness (10 tasks)

**Goal:** Every feature has documentation that matches actual capability.

| Task | Description | Verify |
|------|-------------|--------|
| P10.1 | Verify all 14 tutorials work (code samples run) | All execute |
| P10.2 | Verify all 26 reference docs match current API | All accurate |
| P10.3 | Verify all 15 guides are up-to-date | All current |
| P10.4 | Generate API docs with `cargo doc` — 0 warnings | Clean build |
| P10.5 | Error catalog (80+ codes) matches implementation | All match |
| P10.6 | Update CLAUDE.md test counts, LOC, feature list | Accurate |
| P10.7 | Update README.md badges and stats | Accurate |
| P10.8 | Build mdBook — all pages render | No broken links |
| P10.9 | Verify all example .fj files lex and parse | 0 errors |
| P10.10 | Release notes for v7.0.0 "Integrity" | Written |

---

## Phase 11: Commercial Readiness (10 tasks)

**Goal:** Ready for external users and commercial deployment.

| Task | Description | Verify |
|------|-------------|--------|
| P11.1 | MIT license in all source files | Headers present |
| P11.2 | SBOM (Software Bill of Materials) generated | `fj sbom` works |
| P11.3 | Reproducible builds verified | Same hash twice |
| P11.4 | Security audit checklist completed | All items checked |
| P11.5 | Binary releases for Linux/macOS/Windows | 5 targets built |
| P11.6 | Homebrew/Snap/Chocolatey packages work | Install tested |
| P11.7 | Docker image runs | `docker run fj` works |
| P11.8 | VS Code extension published to marketplace | Installable |
| P11.9 | Website live at fajarlang.dev | Site loads |
| P11.10 | First external user can `cargo install fajar-lang` and run hello.fj | End-to-end works |

---

## Phase 12: Release v7.0.0 "Integrity" (5 tasks)

| Task | Description | Verify |
|------|-------------|--------|
| P12.1 | Bump version to 7.0.0 in Cargo.toml | Version correct |
| P12.2 | Update CHANGELOG.md with all changes | Changelog complete |
| P12.3 | Tag `v7.0.0` in git | Tag created |
| P12.4 | Create GitHub release with binaries | Release published |
| P12.5 | Announce: blog post + social media | Announced |

---

## Dependency Graph

```
Phase 1 (Type Checker) ──────┐
Phase 2 (Builtins) ──────────┤
Phase 3 (Pipeline) ──────────┤
Phase 4 (External) ──────────┼──→ Phase 10 (Docs) ──→ Phase 12 (Release)
Phase 5 (Playground) ────────┤            ↑
Phase 6 (Benchmarks) ────────┤            │
Phase 7 (Nova) ──────────────┤    Phase 11 (Commercial)
Phase 8 (Self-Host) ─────────┤
Phase 9 (Packages) ──────────┘
```

Phase 1 is the **critical path** — type checker fixes unblock Phase 7 (Nova)
and Phase 10 (docs). All other phases can run in parallel.

---

## Priority Order

| Priority | Phase | Why |
|----------|-------|-----|
| 1 | **Phase 1: Type Checker** | Unblocks kernel verification + everything downstream |
| 2 | **Phase 2: Builtins** | Unblocks templates + Phase 9 packages |
| 3 | **Phase 7: Nova Production** | Most visible: OS boots on real hardware |
| 4 | **Phase 3: Pipeline** | Makes security/profiler/optimizer real in output |
| 5 | **Phase 4: External** | Real GUI windows, CI for features |
| 6 | **Phase 5: Playground** | Adoption: try in browser |
| 7 | **Phase 6: Benchmarks** | Marketing: performance comparison |
| 8 | **Phase 8: Self-Host** | Technical milestone |
| 9 | **Phase 9: Packages** | Ecosystem growth |
| 10 | **Phase 10: Docs** | Quality assurance |
| 11 | **Phase 11: Commercial** | Launch preparation |
| 12 | **Phase 12: Release** | Ship it |

---

## Summary

| Phase | Tasks | Effort Estimate |
|-------|-------|-----------------|
| 1. Type Checker Fixes | 25 | HIGH — core compiler changes |
| 2. Interpreter Builtins | 15 | MEDIUM — well-defined interface |
| 3. Compiler Pipeline | 15 | HIGH — Cranelift IR injection |
| 4. External Integration | 20 | HIGH — winit + CI setup |
| 5. Playground WASM | 15 | MEDIUM — wasm-bindgen |
| 6. Benchmark Suite | 30 | MEDIUM — testing + charts |
| 7. FajarOS Nova | 40 | HIGH — native compile + QEMU CI |
| 8. Self-Hosting | 10 | HIGH — parser rewrite in .fj |
| 9. Package Ecosystem | 10 | MEDIUM — registry + CLI |
| 10. Documentation | 10 | LOW — verification + update |
| 11. Commercial Ready | 10 | MEDIUM — packaging + audit |
| 12. Release | 5 | LOW — tag + announce |
| **TOTAL** | **205** | |

**When all 205 tasks are complete, Fajar Lang + FajarOS Nova will be genuinely,
provably, 100% production-ready and commercially deployable.**

---

*PRODUCTION_COMMERCIAL_PLAN.md — Version 1.0 — 2026-03-28*
*Written by Claude Opus 4.6 — no other models used*
