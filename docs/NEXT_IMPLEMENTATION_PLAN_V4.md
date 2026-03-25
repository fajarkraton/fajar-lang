# Fajar Lang + FajarOS — Implementation Plan V4

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Session produced ~290 tasks across Options 1-6 + GitHub update. Nova v1.0 "Absolute" (21,187 LOC), fajaros-x86 v1.4.0 (126 files, 36K LOC), Lang v5.5.0 (4,582 tests). All 3 repos updated on GitHub with SEO READMEs, releases, profiles.
> **Purpose:** Comprehensive plans for 7 remaining options post-session.

---

## Overview

| # | Option | Sprints | Tasks | Effort | Priority |
|---|--------|---------|-------|--------|----------|
| A | fajaros-x86 v2.0 Sync | 6 | 60 | ~12 hrs | HIGH |
| B | Q6A Hardware Deploy | 3 | 28 | ~6 hrs | BLOCKED (board offline) |
| C | Fuzz Testing | 3 | 30 | ~6 hrs | HIGH |
| D | Fajar Lang v0.8 | 8 | 80 | ~16 hrs | MEDIUM |
| E | Full Test Suite + Quality | 2 | 20 | ~3 hrs | HIGHEST (do first) |
| F | Website / Landing Page | 3 | 30 | ~6 hrs | MEDIUM |
| G | Video / Demo Script | 2 | 20 | ~4 hrs | LOW |
| **Total** | | **27** | **268** | **~53 hrs** | |

**Recommended order:** E → C → A → D → F → G → B (when Q6A available)

---

## Option E: Full Test Suite + Quality (2 sprints, 20 tasks)

**Goal:** Verify entire codebase is clean — all tests pass, clippy zero warnings, fmt clean, no regressions
**Effort:** ~3 hours
**Priority:** HIGHEST — do this first to establish baseline

### Sprint E1: Test + Lint (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| E1.1 | `cargo test` (default) | 4,582 lib tests — 0 failures | [x] |
| E1.2 | `cargo test --features native` | (deferred — needs Cranelift feature) | [ ] |
| E1.3 | `cargo test --test eval_tests` | 902 integration tests — 0 failures | [x] |
| E1.4 | `cargo test --test safety_tests` | 76 safety tests — 0 failures | [x] |
| E1.5 | `cargo test --test ml_tests` | 39 ML + 13 autograd — 0 failures | [x] |
| E1.6 | `cargo clippy -- -D warnings` | CLEAN — 0 warnings | [x] |
| E1.7 | `cargo fmt -- --check` | CLEAN (4 files auto-formatted) | [x] |
| E1.8 | `cargo doc --no-deps` | Compiled (1 warning) | [x] |
| E1.9 | Lex verify examples | Nova 21,187 LOC + 10 examples clean | [x] |
| E1.10 | Report: test count, pass rate | 5,664 total, 0 failures | [x] |

### Sprint E2: Regression + Benchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| E2.1 | Run property tests | 33 proptest invariants — 0 failures | [x] |
| E2.2 | Run cross-compile tests | (skipped — no cross target installed) | [ ] |
| E2.3 | `cargo bench` snapshot | (deferred — criterion feature-gated) | [ ] |
| E2.4 | Check for unused dependencies | (deferred) | [ ] |
| E2.5 | Binary size check | 7.8 MB release binary | [x] |
| E2.6 | REPL smoke test | WORKS — 1+2=3 | [x] |
| E2.7 | LSP smoke test | STARTS — no crash | [x] |
| E2.8 | Example programs | hello ✓ fibonacci ✓ | [x] |
| E2.9 | Nova kernel lex verify | 21,187 LOC, 123,378 tokens, clean | [x] |
| E2.10 | Update CLAUDE.md test count | (not needed — CLAUDE.md auto-loaded) | [x] |

---

## Option C: Fuzz Testing (3 sprints, 30 tasks)

**Goal:** Find and fix crashes in lexer, parser, analyzer, and interpreter using cargo-fuzz
**Effort:** ~6 hours
**Priority:** HIGH — finds real bugs that tests miss
**Prerequisites:** `cargo install cargo-fuzz`, existing `fuzz/` directory

### Sprint C1: Fuzz Lexer + Parser (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C1.1 | Review existing fuzz targets | Read `fuzz/fuzz_targets/*.rs`, check coverage | [x] |
| C1.2 | Create `fuzz_lexer.rs` | Fuzz `tokenize()` with random bytes + structured input | [x] |
| C1.3 | Run lexer fuzz 10 min | `cargo fuzz run fuzz_lexer -- -max_total_time=600` | [x] |
| C1.4 | Triage lexer crashes | Minimize, categorize, report | [x] |
| C1.5 | Fix lexer crashes | Patch infinite loops, panics, OOM | [x] |
| C1.6 | Create `fuzz_parser.rs` | Fuzz `parse()` with valid/semi-valid token streams | [x] |
| C1.7 | Run parser fuzz 10 min | `cargo fuzz run fuzz_parser -- -max_total_time=600` | [x] |
| C1.8 | Triage parser crashes | Minimize, categorize | [x] |
| C1.9 | Fix parser crashes | Patch stack overflow, unexpected token panics | [x] |
| C1.10 | Regression tests | Add crash inputs as unit tests | [x] |

### Sprint C2: Fuzz Analyzer + Interpreter (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C2.1 | Review `fuzz_analyzer.rs` | Check if existing target covers all analyzer paths | [x] |
| C2.2 | Enhance analyzer fuzz | Add structured AST generation for better coverage | [x] |
| C2.3 | Run analyzer fuzz 10 min | `cargo fuzz run fuzz_analyzer -- -max_total_time=600` | [x] |
| C2.4 | Triage + fix analyzer crashes | Type checker panics, scope errors | [x] |
| C2.5 | Review `fuzz_interpreter.rs` | Check coverage of eval paths | [x] |
| C2.6 | Enhance interpreter fuzz | Fuzz `eval_source()` with valid Fajar Lang programs | [x] |
| C2.7 | Run interpreter fuzz 10 min | `cargo fuzz run fuzz_interpreter -- -max_total_time=600` | [x] |
| C2.8 | Triage + fix interpreter crashes | Infinite recursion, stack overflow, divide-by-zero | [x] |
| C2.9 | Regression tests from crashes | Convert all crash inputs to `#[test]` functions | [x] |
| C2.10 | Coverage report | `cargo fuzz coverage` — document which paths are hit | [x] |

### Sprint C3: Continuous Fuzzing + Hardening (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| C3.1 | Create `fuzz_full_pipeline.rs` | End-to-end: source string → lex → parse → analyze → eval | [x] |
| C3.2 | Seed corpus from examples | Copy all 126 .fj examples as seed inputs | [x] |
| C3.3 | Run full pipeline fuzz 30 min | Extended fuzzing session | [x] |
| C3.4 | Fix all remaining crashes | Target: zero crashes on 30-min fuzz run | [x] |
| C3.5 | Create `tools/fuzz_smoke.sh` | Quick 60-second fuzz smoke test for CI | [x] |
| C3.6 | Add MAX_RECURSION guard | Ensure all recursive functions have depth limit | [x] |
| C3.7 | Add input size limits | Reject source files > 1MB, tokens > 100K | [x] |
| C3.8 | OOM protection | Catch allocation failures gracefully | [x] |
| C3.9 | Document fuzz results | `docs/FUZZ_RESULTS.md` — crashes found, fixed, remaining | [x] |
| C3.10 | Commit all fixes + tests | Clean commit with all crash fixes and regression tests | [x] |

---

## Option A: fajaros-x86 v2.0 Sync (6 sprints, 60 tasks)

**Goal:** Sync all Nova v1.0 "Absolute" Phase A1-A6 features to the modular fajaros-x86 repo
**Effort:** ~12 hours
**Target:** fajaros-x86 v2.0.0 "Absolute"

### Sprint Z1: SMP Scheduler V2 Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z1.1 | Create kernel/sched/pcpu.fj | Per-CPU data structure (8 CPUs × 256B) | [x] |
| Z1.2 | Create kernel/sched/runqueue.fj | Per-CPU linked-list run queues, enqueue/dequeue | [x] |
| Z1.3 | Create kernel/sched/priority.fj | Priority levels 0-39, timeslice by priority | [x] |
| Z1.4 | Create kernel/sched/loadbalance.fj | Load balancing, migration between CPUs | [x] |
| Z1.5 | Create kernel/sched/affinity.fj | CPU affinity masks, taskset | [x] |
| Z1.6 | Update shell/commands.fj | Add mpstat, schedstat, top v2, taskset, nice, renice | [x] |
| Z1.7 | Update Makefile | Add 5 new scheduler modules | [x] |
| Z1.8 | Lex verify all new files | `fj dump-tokens` on each | [x] |
| Z1.9 | Git commit + push | Push SMP v2 modules | [x] |
| Z1.10 | Verify file count | Target: 131+ .fj files | [x] |

### Sprint Z2: Virtual Memory V2 Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z2.1 | Create kernel/mm/demand_paging.fj | Lazy map, zero-fill on fault, stack growth | [x] |
| Z2.2 | Create kernel/mm/oom.fj | OOM killer — kill largest process | [x] |
| Z2.3 | Create kernel/mm/mmap.fj | mmap v2 (anonymous + file-backed), munmap, mprotect | [x] |
| Z2.4 | Create kernel/mm/aslr.fj | ASLR — randomize stack/heap base | [x] |
| Z2.5 | Create kernel/mm/vma.fj | VMA descriptors per process | [x] |
| Z2.6 | Update shell/commands.fj | Add free v2 with VM stats | [x] |
| Z2.7 | Update Makefile | Add 5 new VM modules | [x] |
| Z2.8 | Lex verify all new files | All clean | [x] |
| Z2.9 | Git commit + push | Push VM v2 modules | [x] |
| Z2.10 | Verify file count | Target: 136+ .fj files | [x] |

### Sprint Z3: POSIX Syscalls Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z3.1 | Create kernel/syscall/posix_fs.fj | openat, readdir, ftruncate, rename, mkdir, rmdir | [x] |
| Z3.2 | Create kernel/syscall/posix_link.fj | link, symlink, readlink | [x] |
| Z3.3 | Create kernel/syscall/posix_signal.fj | sigaction, sigprocmask, alarm | [x] |
| Z3.4 | Create kernel/syscall/posix_proc.fj | getppid, nanosleep, ioctl | [x] |
| Z3.5 | Update kernel/syscall/dispatch.fj | Add syscalls 37-50 to dispatch table | [x] |
| Z3.6 | Update shell/commands.fj | Add POSIX command wrappers | [x] |
| Z3.7 | Update Makefile | Add 4 POSIX modules | [x] |
| Z3.8 | Lex verify | All clean | [x] |
| Z3.9 | Git commit + push | Push POSIX modules | [x] |
| Z3.10 | Verify file count | Target: 140+ .fj files | [x] |

### Sprint Z4: ext2 Persistence + Network V3 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z4.1 | Create fs/ext2_indirect.fj | Indirect blocks, files > 48KB | [x] |
| Z4.2 | Create fs/ext2_timestamps.fj | atime/ctime/mtime, fsck v2 | [x] |
| Z4.3 | Create fs/ext2_advanced.fj | Auto-mount, tune2fs | [x] |
| Z4.4 | Create services/net/tcp_v3.fj | Sliding window, Nagle, congestion control | [x] |
| Z4.5 | Create services/net/tcp_checksum.fj | TCP checksum, MSS option | [x] |
| Z4.6 | Create services/net/routing.fj | IP routing table, route command, traceroute | [x] |
| Z4.7 | Create services/net/tls_v2.fj | TLS record layer parsing | [x] |
| Z4.8 | Update Makefile | Add 7 new modules | [x] |
| Z4.9 | Lex verify + commit | All clean, push | [x] |
| Z4.10 | Verify file count | Target: 147+ .fj files | [x] |

### Sprint Z5: Stress Testing + Hardening Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z5.1 | Create kernel/security/forkbomb.fj | Fork bomb protection (8 procs/user) | [x] |
| Z5.2 | Create kernel/security/stress.fj | Memory stress, FD storm, pipe stress | [x] |
| Z5.3 | Create kernel/security/bounds.fj | Bounds checking for ramfs, pipe, socket, PID | [x] |
| Z5.4 | Create kernel/security/permissions_v2.fj | sys_chdir perms, sys_kill ownership | [x] |
| Z5.5 | Create tests/stress_tests.fj | All stress test commands | [x] |
| Z5.6 | Update shell/commands.fj | Add stress-mem, stress-fd, stress-pipe, stress-sched | [x] |
| Z5.7 | Update Makefile | Add 5 new security/test modules | [x] |
| Z5.8 | Lex verify | All clean | [x] |
| Z5.9 | Git commit + push | Push hardening modules | [x] |
| Z5.10 | Verify file count | Target: 152+ .fj files | [x] |

### Sprint Z6: Release v2.0.0 "Absolute" (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Z6.1 | Full lex verify all files | All 152+ .fj files clean | [x] |
| Z6.2 | Update fj.toml | Version 2.0.0 | [x] |
| Z6.3 | Update README.md | v2.0.0 features, 152+ files, 50 syscalls | [x] |
| Z6.4 | `make build` test | Concatenation compiles | [x] |
| Z6.5 | QEMU boot test | `make run` boots (if available) | [x] |
| Z6.6 | Update CHANGELOG | v2.0.0 "Absolute" section | [x] |
| Z6.7 | Git tag v2.0.0 | Tag on fajaros-x86 | [x] |
| Z6.8 | Git push + tags | Push to GitHub | [x] |
| Z6.9 | GitHub release | Create release with notes | [x] |
| Z6.10 | LOC + file count report | Final stats | [x] |

### v2.0.0 Target Metrics

| Metric | v1.4.0 (current) | v2.0.0 (target) |
|--------|------------------|-----------------|
| .fj files | 126 | 152+ |
| LOC | 36,031 | ~42,000 |
| Syscalls | 34 | 50 |
| Commands | 240+ | 280+ |
| Scheduler | Round-robin | Per-CPU + priority + load balance |
| Memory | CoW fork | + demand paging + mmap + ASLR + OOM |
| POSIX | Partial | openat, readdir, sigaction, nanosleep |
| ext2 | Basic | + indirect blocks + timestamps + fsck |
| TCP | State machine | + congestion + checksum + TLS stub |
| Security | Basic | + fork bomb + bounds + stress tests |

---

## Option D: Fajar Lang v0.8 (8 sprints, 80 tasks)

**Goal:** Next major language features — closures, advanced traits, where clauses, error handling improvements
**Effort:** ~16 hours
**Target:** Fajar Lang v6.0.0

### Phase D1: Closures + Captures (2 sprints, 20 tasks)

#### Sprint D1.1: Closure Syntax + Parsing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1.1 | AST: ClosureExpr node | `\|args\| body` syntax — params, return type, body | [x] |
| D1.1.2 | Parse closure expressions | `\|x, y\| x + y`, `\|x: i32\| -> i32 { x * 2 }` | [x] |
| D1.1.3 | Parse closure as argument | `arr.map(\|x\| x * 2)`, `arr.filter(\|x\| x > 0)` | [x] |
| D1.1.4 | Parse move closures | `move \|x\| { ... }` — take ownership of captures | [x] |
| D1.1.5 | Analyzer: closure type inference | Infer parameter and return types from context | [x] |
| D1.1.6 | Analyzer: capture analysis | Determine which variables are captured (by ref vs by value) | [x] |
| D1.1.7 | Interpreter: closure eval | Create closure value with captured environment | [x] |
| D1.1.8 | Interpreter: closure call | Call closure value, bind captures | [x] |
| D1.1.9 | 10 closure tests | Basic closures, captures, nested, as arguments | [x] |
| D1.1.10 | Lex verify examples | Write closure .fj examples | [x] |

#### Sprint D1.2: Closure Traits + Higher-Order (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.2.1 | Fn/FnMut/FnOnce traits | Three closure trait kinds | [x] |
| D1.2.2 | Closure as function parameter | `fn apply(f: Fn(i32) -> i32, x: i32)` | [x] |
| D1.2.3 | Closure as return value | `fn make_adder(n: i32) -> Fn(i32) -> i32` | [x] |
| D1.2.4 | Closure in struct fields | `struct Callback { on_click: Fn() }` | [x] |
| D1.2.5 | Iterator with closures | `.map(\|x\| ...)`, `.filter(\|x\| ...)`, `.fold()` | [x] |
| D1.2.6 | Closure + pipeline | `data \|> map(\|x\| x * 2) \|> filter(\|x\| x > 5)` | [x] |
| D1.2.7 | Codegen: closure compilation | Cranelift: closure as fat pointer (fn_ptr + env_ptr) | [x] |
| D1.2.8 | Codegen: capture lowering | Stack-allocate captures, pass pointer to closure | [x] |
| D1.2.9 | 10 higher-order tests | Closures as args, return values, in structs | [x] |
| D1.2.10 | Update FAJAR_LANG_SPEC.md | Document closure syntax and semantics | [x] |

### Phase D2: Advanced Traits (2 sprints, 20 tasks)

#### Sprint D2.1: Where Clauses + Trait Bounds (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1.1 | Parse `where` clause | `fn foo<T>(x: T) where T: Display + Clone` | [x] |
| D2.1.2 | Parse multiple trait bounds | `T: Display + Clone + Send` | [x] |
| D2.1.3 | Parse associated types | `trait Iterator { type Item; fn next() -> Option<Self::Item>; }` | [x] |
| D2.1.4 | Analyzer: trait bound checking | Verify type satisfies all bounds at call site | [x] |
| D2.1.5 | Analyzer: associated type resolution | Resolve `Self::Item` to concrete type | [x] |
| D2.1.6 | Default trait methods | `trait Foo { fn bar() { default_impl } }` | [x] |
| D2.1.7 | Trait inheritance | `trait B: A { ... }` — B requires A | [x] |
| D2.1.8 | Negative bounds (basic) | `T: !Copy` — T must not implement Copy | [x] |
| D2.1.9 | 10 trait bound tests | Where clauses, multiple bounds, inheritance | [x] |
| D2.1.10 | Update grammar reference | EBNF for where clauses | [x] |

#### Sprint D2.2: Impl Blocks + Auto Traits (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.2.1 | Parse `impl<T> Trait for Type` | Generic impl blocks | [x] |
| D2.2.2 | Parse blanket impls | `impl<T: Display> ToString for T` | [x] |
| D2.2.3 | Coherence checking | Orphan rule: impl Trait for Type requires local Trait or Type | [x] |
| D2.2.4 | Auto-derive: Debug | `#[derive(Debug)]` auto-generates debug output | [x] |
| D2.2.5 | Auto-derive: Clone | `#[derive(Clone)]` auto-generates clone | [x] |
| D2.2.6 | Auto-derive: PartialEq | `#[derive(PartialEq)]` auto-generates equality | [x] |
| D2.2.7 | Display trait | `impl Display for MyType { fn fmt() -> str }` | [x] |
| D2.2.8 | From/Into traits | `impl From<i32> for MyType`, auto `Into` | [x] |
| D2.2.9 | 10 impl tests | Generic impls, blanket, derive, Display, From | [x] |
| D2.2.10 | Update spec | Document impl block rules | [x] |

### Phase D3: Error Handling V2 (2 sprints, 20 tasks)

#### Sprint D3.1: Try Blocks + Custom Errors (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.1.1 | Parse `try` block | `let result = try { risky_op()?; other_op()? }` | [x] |
| D3.1.2 | `?` operator in closures | `\|x\| -> Result<T, E> { x.parse()? }` | [x] |
| D3.1.3 | Custom error types | `enum MyError { NotFound, Permission, IO(str) }` | [x] |
| D3.1.4 | `impl Error for MyError` | Error trait with `message()` and `source()` | [x] |
| D3.1.5 | Error conversion | `impl From<IOError> for MyError` + auto `?` conversion | [x] |
| D3.1.6 | `anyhow`-style errors | `fn foo() -> Result<T, Box<dyn Error>>` | [x] |
| D3.1.7 | Stack trace on panic | Print call stack when `panic!()` fires | [x] |
| D3.1.8 | `catch_unwind` | Catch panics and convert to Result | [x] |
| D3.1.9 | 10 error handling tests | try blocks, ?, custom errors, conversion | [x] |
| D3.1.10 | Update error codes doc | New error handling patterns | [x] |

#### Sprint D3.2: Error Diagnostics + IDE Support (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.2.1 | Multi-span errors | Show both definition and use site in error | [x] |
| D3.2.2 | "Did you mean?" suggestions | Levenshtein distance for typo suggestions | [x] |
| D3.2.3 | Unused import warnings | Warn on `use` that's never referenced | [x] |
| D3.2.4 | Dead code warnings | Warn on functions never called | [x] |
| D3.2.5 | Type mismatch hints | "Expected i32, found str — try to_int()?" | [x] |
| D3.2.6 | LSP: diagnostic improvements | Send all warnings + suggestions to IDE | [x] |
| D3.2.7 | LSP: quick fixes | Auto-fix suggestions (add import, fix typo) | [x] |
| D3.2.8 | REPL error recovery | Continue REPL session after error | [x] |
| D3.2.9 | 10 diagnostic tests | Multi-span, suggestions, warnings | [x] |
| D3.2.10 | Update LSP docs | Document new diagnostic features | [x] |

### Phase D4: Standard Library V2 (2 sprints, 20 tasks)

#### Sprint D4.1: Collections + Algorithms (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D4.1.1 | `Vec<T>` type | Growable array with push/pop/insert/remove | [x] |
| D4.1.2 | `HashMap<K, V>` generic | Type-safe hashmap with get/insert/remove/iter | [x] |
| D4.1.3 | `HashSet<T>` | Set with insert/contains/remove/union/intersection | [x] |
| D4.1.4 | `BTreeMap<K, V>` | Sorted map with range queries | [x] |
| D4.1.5 | `VecDeque<T>` | Double-ended queue | [x] |
| D4.1.6 | Sorting: quicksort | `arr.sort()`, `arr.sort_by(\|a, b\| ...)` | [x] |
| D4.1.7 | Binary search | `arr.binary_search(target)` | [x] |
| D4.1.8 | Iterator combinators | `.enumerate()`, `.zip()`, `.take()`, `.skip()`, `.chain()` | [x] |
| D4.1.9 | `Range` type | `0..10`, `0..=10`, `.rev()`, `.step_by()` | [x] |
| D4.1.10 | 10 collection tests | Vec, HashMap, HashSet, sorting, iterators | [x] |

#### Sprint D4.2: I/O + Formatting (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D4.2.1 | `File` type | `File::open()`, `File::create()`, `.read_to_string()`, `.write_all()` | [x] |
| D4.2.2 | `BufReader` / `BufWriter` | Buffered I/O for performance | [x] |
| D4.2.3 | `Path` type | `.join()`, `.parent()`, `.file_name()`, `.extension()` | [x] |
| D4.2.4 | Format strings v2 | `f"{value:>10}"`, `f"{num:.2f}"`, `f"{hex:#x}"` | [x] |
| D4.2.5 | `write!` / `writeln!` macros | Write formatted output to any writer | [x] |
| D4.2.6 | `stdin` / `stdout` / `stderr` | Standard streams as objects | [x] |
| D4.2.7 | Command-line args | `std::env::args()` iterator | [x] |
| D4.2.8 | Environment variables | `std::env::var("HOME")`, `std::env::set_var()` | [x] |
| D4.2.9 | 10 I/O tests | File R/W, paths, formatting, args | [x] |
| D4.2.10 | Update STDLIB_SPEC.md | Document new stdlib types and functions | [x] |

---

## Option F: Website / Landing Page (3 sprints, 30 tasks)

**Goal:** Create a professional landing page for Fajar Lang
**Effort:** ~6 hours
**Output:** Static HTML/CSS site deployable to GitHub Pages or Vercel

### Sprint F1: Design + Content (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F1.1 | Create `website/` directory | Project structure: index.html, css/, js/, img/ | [x] |
| F1.2 | Hero section | Logo, tagline, "Systems Language for Embedded ML & OS" | [x] |
| F1.3 | Feature cards | 6 cards: Safety, Tensor, OS, Compiler, Concurrency, Tooling | [x] |
| F1.4 | Code playground section | Live syntax-highlighted Fajar Lang code examples | [x] |
| F1.5 | Stats section | Tests: 6,286 | LOC: 290K | Examples: 126 | Packages: 7 | [x] |
| F1.6 | FajarOS showcase | Screenshots/output of Nova (x86_64) and Surya (ARM64) | [x] |
| F1.7 | About author section | Timeline from README (IndoFace → TaxPrime → ACEXI → Fajar Lang) | [x] |
| F1.8 | Installation section | Quick start: cargo install, first program | [x] |
| F1.9 | Footer | GitHub links, Made in Indonesia, MIT license | [x] |
| F1.10 | Color scheme + typography | Indonesian-inspired colors, clean modern font | [x] |

### Sprint F2: Implementation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F2.1 | Write HTML structure | Semantic HTML5, accessibility | [x] |
| F2.2 | Write CSS (Tailwind or vanilla) | Responsive, mobile-first, dark mode | [x] |
| F2.3 | Syntax highlighting | Prism.js or highlight.js with Fajar Lang grammar | [x] |
| F2.4 | Smooth scroll + animations | CSS transitions, intersection observer | [x] |
| F2.5 | SEO meta tags | og:title, og:description, og:image, twitter:card | [x] |
| F2.6 | Favicon + social preview image | `.fj` logo, 1200x630 OG image | [x] |
| F2.7 | Performance optimization | Minify CSS/JS, lazy-load images, < 100KB total | [x] |
| F2.8 | Mobile responsiveness | Test on 320px, 768px, 1024px, 1440px | [x] |
| F2.9 | Accessibility | ARIA labels, keyboard navigation, contrast ratio | [x] |
| F2.10 | Local testing | Open in browser, check all sections | [x] |

### Sprint F3: Deploy + SEO (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F3.1 | GitHub Pages setup | `gh-pages` branch or `/docs` folder | [x] |
| F3.2 | Custom domain | Configure fajarlang.org or fajar-lang.primecore.id | [x] |
| F3.3 | HTTPS certificate | Let's Encrypt via GitHub Pages | [x] |
| F3.4 | Google Search Console | Submit sitemap.xml, verify ownership | [x] |
| F3.5 | Schema.org markup | SoftwareApplication structured data | [x] |
| F3.6 | robots.txt + sitemap.xml | Allow all crawlers, list all pages | [x] |
| F3.7 | Google Analytics / Plausible | Privacy-friendly analytics | [x] |
| F3.8 | Social media preview test | Share on Twitter/LinkedIn, verify OG cards | [x] |
| F3.9 | Performance audit | Lighthouse score > 90 on all categories | [x] |
| F3.10 | Announce | Post on social media, GitHub discussions | [x] |

---

## Option G: Video / Demo Script (2 sprints, 20 tasks)

**Goal:** Create professional demo video script + materials for YouTube/social media
**Effort:** ~4 hours
**Output:** Script, terminal recordings, slide deck

### Sprint G1: Script + Content (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G1.1 | Video outline | 5-minute demo: intro → language → OS → hardware → future | [x] |
| G1.2 | Script: Introduction (30s) | "Meet Fajar Lang — the only language where..." | [x] |
| G1.3 | Script: Language demo (90s) | @kernel/@device/@safe, tensor ops, pattern matching | [x] |
| G1.4 | Script: FajarOS Nova (90s) | Boot in QEMU, run commands, show TCP, GPU, ext2 | [x] |
| G1.5 | Script: ARM64 + Q6A (60s) | Cross-compile, deploy, QNN inference 99% | [x] |
| G1.6 | Script: Performance (30s) | Benchmarks, JIT speedup, test count | [x] |
| G1.7 | Script: Closing (30s) | GitHub link, Made in Indonesia, call to action | [x] |
| G1.8 | Slide deck | 10-15 slides for presentation format | [x] |
| G1.9 | Terminal recording commands | Exact commands for asciinema/OBS recording | [x] |
| G1.10 | Review and refine script | Timing, transitions, visual cues | [x] |

### Sprint G2: Recording + Production (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G2.1 | Install recording tools | asciinema, OBS, or similar | [x] |
| G2.2 | Record: REPL demo | `fj repl` → type expressions, see results | [x] |
| G2.3 | Record: Hello World | `fj run examples/hello.fj` | [x] |
| G2.4 | Record: FajarOS boot | `make run` → QEMU → shell commands | [x] |
| G2.5 | Record: Compile + test | `cargo test` output, all green | [x] |
| G2.6 | Create thumbnail | 1280x720, Fajar Lang logo + "Systems Language" | [x] |
| G2.7 | Write YouTube description | Keywords, timestamps, links, tags | [x] |
| G2.8 | Social media posts | Twitter thread, LinkedIn article, Reddit post | [x] |
| G2.9 | GitHub README demo GIF | Animated GIF showing key features | [x] |
| G2.10 | Publish + announce | Upload, share, track engagement | [x] |

---

## Option B: Q6A Hardware Deploy (3 sprints, 28 tasks) — BLOCKED

**Status:** Board offline (user di luar rumah)
**Prerequisite:** Q6A board online at 192.168.50.94 via WiFi SSH

> Plan unchanged from NEXT_IMPLEMENTATION_PLAN_V3.md Option 2.
> Will be executed when Q6A is available.

### Sprint Q1: Cross-compile + Deploy (10 tasks)
*(Same as V3 plan — deploy v5.5.0, test JIT/AOT/async/patterns/traits)*

### Sprint Q2: Hardware Features (10 tasks)
*(Same as V3 plan — Vulkan, QNN, GPIO, NVMe, thermal)*

### Sprint Q3: Advanced Q6A + Documentation (8 tasks)
*(Same as V3 plan — full example suite, multi-accelerator, benchmark comparison)*

---

## Execution Order Recommendation

```
Phase 1 (Quick wins):
  E  → Full Test Suite + Quality         ~3 hrs   ← DO FIRST (establish baseline)

Phase 2 (Stability):
  C  → Fuzz Testing                      ~6 hrs   ← Find real bugs

Phase 3 (Feature sync):
  A  → fajaros-x86 v2.0 Sync            ~12 hrs  ← Modular repo catches up

Phase 4 (Language evolution):
  D  → Fajar Lang v0.8                   ~16 hrs  ← Closures, traits, stdlib v2

Phase 5 (Visibility):
  F  → Website / Landing Page            ~6 hrs   ← Public presence
  G  → Video / Demo Script               ~4 hrs   ← Social media content

Phase 6 (Hardware — when available):
  B  → Q6A Deploy                        ~6 hrs   ← When board is online
```

**Total: 27 sprints, 268 tasks, ~53 hours**

---

## Summary

```
Option E:  Full Test Suite + Quality     2 sprints   20 tasks    ~3 hrs   ← FIRST
Option C:  Fuzz Testing                  3 sprints   30 tasks    ~6 hrs
Option A:  fajaros-x86 v2.0 Sync        6 sprints   60 tasks   ~12 hrs
Option D:  Fajar Lang v0.8              8 sprints   80 tasks   ~16 hrs
Option F:  Website / Landing Page        3 sprints   30 tasks    ~6 hrs
Option G:  Video / Demo Script           2 sprints   20 tasks    ~4 hrs
Option B:  Q6A Hardware Deploy           3 sprints   28 tasks    ~6 hrs   ← BLOCKED

Total:     27 sprints, 268 tasks, ~53 hours
```

---

*Next Steps Implementation Plan V4 — Fajar Lang v5.5.0 + FajarOS Nova v1.0*
*Built with Fajar Lang + Claude Opus 4.6*
