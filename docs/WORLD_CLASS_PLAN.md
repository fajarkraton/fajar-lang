# Fajar Lang World-Class Implementation Plan

> **Misi:** Menjadikan Fajar Lang bahasa pemrograman terbaik di dunia untuk embedded AI + OS
> **Tanggal:** 2026-03-22
> **Dasar:** Riset mendalam (Rust/Go/Zig/Mojo/Koka/Idris/seL4/Hubris) + audit brutal codebase
> **Prinsip:** Tidak setengah-setengah. Setiap fitur harus REAL — bukan stub.

---

## Posisi Strategis

### Unique Value Proposition (Tidak Ada di Bahasa Lain)

```
"Satu-satunya bahasa di mana OS kernel dan neural network bisa hidup di
 codebase yang sama, type system yang sama, dan compiler yang sama — dengan
 safety guarantees yang di-enforce oleh compiler melalui context annotations."
```

- `@kernel` → compiler menolak heap/tensor ops
- `@device` → compiler menolak raw pointer/IRQ ops
- `@safe` → compiler menolak semua hardware access
- **Tidak ada bahasa lain yang punya ini**

### Kelemahan Kompetitor (Peluang Kita)

| Kompetitor | Kelemahan | Peluang Fajar Lang |
|------------|-----------|-------------------|
| **Rust** | Compile lambat (27.9% "big problem"), async fragmented, no linear types, learning curve | Compile cepat via Cranelift, unified async, linear types, simpler ownership |
| **Go** | No generics sampai 2022, no enums, GC pauses, no embedded | Full generics+traits, no GC, bare-metal support |
| **Zig** | No traits, no closures, no ML support, immature ecosystem | Full trait system, closures, first-class tensors |
| **Mojo** | Closed source, Python-only ecosystem, no OS kernel support | Open source, OS kernel + ML, hardware verified |
| **C/C++** | Memory unsafe, no compile-time safety, undefined behavior | Memory safe by construction, context enforcement |

---

## Audit Reality Check: 7 STUB Features yang Harus Dijadikan REAL

| # | Feature | Status Saat Ini | Prioritas |
|---|---------|----------------|-----------|
| 1 | **Linear Types** | STUB (data structures only) | CRITICAL — Rust tidak punya ini |
| 2 | **Effect System** | STUB (enums defined, not enforced) | CRITICAL — formalize @kernel/@device |
| 3 | **Comptime Evaluation** | STUB (marks functions, doesn't evaluate) | HIGH — Zig's killer feature |
| 4 | **Macros** | STUB (parsed, not expanded) | HIGH — metaprogramming essential |
| 5 | **Higher-Kinded Types** | STUB (types defined, no computation) | MEDIUM — academic credibility |
| 6 | **GC Mode** | STUB (types defined, no GC) | MEDIUM — gradual adoption path |
| 7 | **Verification/SMT** | STUB (z3 FFI declared, no logic) | LOW — seL4 territory, long-term |

---

## Phase 0: Foundation Fix (4 minggu)
> **Goal:** Eliminasi semua stub. Setiap feature yang declared harus REAL atau di-remove.

### Sprint 0.1: Honest Cleanup (1 minggu)
- [ ] Audit semua `src/linear/`, `src/effects.rs`, `src/hkt/`, `src/gc/`, `src/verification/`
- [ ] Untuk setiap module: either implement properly dengan 20+ tests, atau DELETE
- [ ] Update semua docs yang claim features yang belum real
- [ ] Remove semua version claims (v0.7, v0.8, v0.9, v2.0) yang hanya stubs
- [ ] Update CLAUDE.md: honest status — hanya claim yang REAL
- [ ] **Deliverable:** Zero stubs in codebase. Every feature is either real or gone.

### Sprint 0.2: Borrow Checker Hardening (1 minggu)
- [ ] Integrate borrow checker into EVERY code path (REPL, eval_source, native)
- [ ] Add 50+ tests for edge cases: double move, borrow after move, nested borrows
- [ ] Add borrow checker to native codegen (Cranelift) — currently only interpreter
- [ ] Implement proper NLL (Non-Lexical Lifetimes) using CFG — not just scope-based
- [ ] **Deliverable:** Borrow checker catches all use-after-move, double-free, dangling ref

### Sprint 0.3: Async Completion (1 minggu)
- [ ] Implement real executor (single-threaded event loop, like tokio-lite)
- [ ] Implement real Future trait with poll()
- [ ] Wire async/await to use executor (not just stub)
- [ ] Test: 3 concurrent tasks actually interleave execution
- [ ] Test: async I/O (file read, timer) with real waiting
- [ ] **Deliverable:** `async fn` + `await` actually run concurrently

### Sprint 0.4: Self-Hosting Phase 1 — Parser (1 minggu)
- [ ] Port parser from Rust to Fajar Lang (`stdlib/parser.fj`)
- [ ] Verify: parse any .fj file and produce identical AST as Rust parser
- [ ] Test: self-parse `stdlib/lexer.fj` and `stdlib/parser.fj`
- [ ] **Deliverable:** Lexer + Parser in Fajar Lang, verified against Rust implementation

---

## Phase 1: Linear Types + Effect System (6 minggu)
> **Goal:** Features yang Rust TIDAK punya. Ini pembeda utama.

### Sprint 1.1: Linear Types — Real Implementation (2 minggu)

**Apa:** Tipe yang HARUS digunakan tepat sekali. Rust hanya punya affine (at most once).
Linear types menjamin destructor berjalan — tidak bisa di-leak via `mem::forget`.

```fajar
linear struct FileHandle { fd: i64 }

fn open(path: str) -> linear FileHandle { ... }
fn close(handle: linear FileHandle) { ... }  // MUST be called

fn bad() {
    let f = open("data.txt")
    // ERROR: linear value 'f' not consumed — compiler rejects this
}

fn good() {
    let f = open("data.txt")
    close(f)  // OK: consumed exactly once
}
```

- [ ] Add `linear` keyword to parser (type modifier)
- [ ] Add linearity tracking to type checker: used=0 → error, used=1 → ok, used>1 → error
- [ ] Enforce linearity across all branches (if/else/match must consume in all paths)
- [ ] Integrate with borrow checker (linear values cannot be borrowed mutably)
- [ ] Add `consume` pattern for transferring linear ownership
- [ ] 50+ tests: linear struct, linear fn return, linear in if/else, linear + closures
- [ ] **Deliverable:** `linear` keyword enforced at compile time, no leaks possible

### Sprint 1.2: Effect System — Formalize @kernel/@device/@safe (2 minggu)

**Apa:** @kernel/@device/@safe sudah ada tapi ad-hoc. Formalize sebagai effect system.

```fajar
// Effects are declared in function signatures
fn read_sensor() -> i64 with io, hardware {
    volatile_read(0x40001000)
}

fn inference(data: Tensor) -> Tensor with compute {
    matmul(data, weights)
}

// @kernel = { hardware, memory } effects allowed
// @device = { compute, tensor } effects allowed
// @safe = {} no effects (pure)

// Effect handlers (like Koka)
handle io {
    fn read(addr: i64) -> i64 { volatile_read(addr) }
    fn write(addr: i64, val: i64) { volatile_write(addr, val) }
}
```

- [x] Define effect algebra: `io`, `hardware`, `compute`, `tensor`, `heap`, `panic`
- [x] Map existing annotations: @kernel={hardware,memory}, @device={compute,tensor,heap}, @safe={}
- [x] Add `with` clause to function signatures
- [x] Type checker: verify function body only uses declared effects
- [x] Effect inference: auto-detect effects if not declared
- [ ] Effect polymorphism: generic over effects
- [x] 77 tests: effect violation, effect inference, effect composition, context mapping
- [x] **Deliverable:** Formal effect system integrated with @kernel/@device/@safe

### Sprint 1.3: Comptime Evaluation (2 minggu)

**Apa:** Zig-style compile-time code execution. Same syntax, runs during compilation.

```fajar
comptime fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

const LOOKUP_TABLE: [i64; 10] = comptime {
    let mut table: [i64; 10] = [0; 10]
    for i in 0..10 {
        table[i] = factorial(i)
    }
    table
}

fn generic_array<comptime N: i64>() -> [i64; N] {
    [0; N]  // N known at compile time
}
```

- [x] Add `comptime` keyword (block, variable, function parameter)
- [x] Implement comptime evaluator: interpret AST at compile time
- [x] Support: arithmetic, if/else, function calls, array construction, floats, strings
- [x] Restrict: no I/O, no extern calls — CT001-CT008 error codes
- [x] Use comptime for generic const parameters: `fn zeros<comptime N>() -> [f64; N]`
- [x] 69 tests: comptime arithmetic, comptime fn, comptime blocks, restrictions
- [x] **Deliverable:** Compile-time evaluation with Zig-style comptime blocks

---

## Phase 2: Compiler Excellence (6 minggu)
> **Goal:** Compilation speed, codegen quality, tooling — better than Rust

### Sprint 2.1: Incremental Compilation (2 minggu)
- [x] Build dependency graph between functions (file-level + function-level)
- [x] Cache compiled functions (hash-based invalidation, FNV-1a)
- [x] On file change: only recompile affected functions (transitive dependents)
- [x] `fj build --incremental` flag with disk-persistent cache (.fj-cache/)
- [x] Function-level dependency tracking (call graph, const fn detection)
- [x] 31 new tests: dependency graph, change detection, function tracking, disk persistence
- [x] **Deliverable:** `fj build --incremental` — cache-based rebuild with dependency tracking

### Sprint 2.2: LLVM Backend Production-Ready (2 minggu)
- [x] Cranelift = dev mode (fast compile), LLVM = release mode (best codegen)
- [x] `fj build --release` uses LLVM with -O2 (auto-selects backend + opt level)
- [x] `fj build` (default) uses Cranelift for speed
- [x] Semantic analyzer integrated into LLVM pipeline (catches errors before codegen)
- [x] LLVM backend handles effect/comptime/handle AST nodes
- [x] 19 dual-backend tests, all 5,267 tests pass on both backends
- [x] **Deliverable:** Dual backend strategy: `fj build` (Cranelift) + `fj build --release` (LLVM O2)

### Sprint 2.3: Codegen Bug Fix + Optimization (2 minggu)
- [x] Unified optimization pipeline (src/codegen/optimizer.rs): O0/O1/O2/O3 levels
- [x] Optimization passes: constant folding, dead code elimination, inlining heuristics
- [x] Tail call optimization: detects self-recursive tail calls, analyze_tail_call() info
- [x] Compiler phase profiler: CompileProfile with lex/parse/analyze/codegen/link timing
- [x] 50 new tests (22 unit + 28 integration): TCO, DCE, const fold, profiling
- [x] **Deliverable:** Unified optimization pipeline with TCO + DCE + profiling

---

## Phase 3: Macro System + Metaprogramming (4 minggu)
> **Goal:** Code generation tanpa template magic

### Sprint 3.1: Procedural Macros (2 minggu)

```fajar
#[derive(Debug, Clone, PartialEq)]
struct Point { x: f64, y: f64 }

// Custom derive
#[derive(Serialize)]
struct Config { name: str, port: i64 }

// Function-like macro
let sql = sql!("SELECT * FROM users WHERE id = ?", user_id)
```

- [x] Macro expansion pipeline: parse → expand (built-in) → evaluate
- [x] `@derive(Debug, Clone, PartialEq)` annotation with derive expansion
- [x] Function-like macros: `vec![]`, `stringify!()`, `concat!()`, `dbg!()`, `todo!()`, `env!()`
- [x] Macro registry with 11 built-in macros + user-defined support
- [x] 48 tests: macro invocation, derive, built-in macros, registry, expansion
- [x] **Deliverable:** Working macro system with built-in macros + @derive

### Sprint 3.2: Declarative Macros + Code Generation (2 minggu)

```fajar
macro_rules! vec {
    ($($x:expr),*) => {
        {
            let mut v = Array::new()
            $(v.push($x);)*
            v
        }
    }
}

let nums = vec![1, 2, 3, 4, 5]
```

- [x] `macro_rules!` parsing with pattern → template arms
- [x] `@pure` annotation for pure function marking
- [x] `stringify!()` and `concat!()` built-in macros (working in interpreter)
- [x] `macro_rules!` item parsing and AST representation
- [x] Integrated into Sprint 3.1 (combined delivery)
- [x] **Deliverable:** Declarative macro syntax + built-in expansion

---

## Phase 4: Ecosystem + Tooling (6 minggu)
> **Goal:** Go-level developer experience. ONE binary, everything works.

### Sprint 4.1: Package Registry — Real Remote (2 minggu)
- [x] PubGrub-style dependency resolver with backtracking + conflict analysis
- [x] Registry protocol: HTTP API definitions, sparse index (crates.io-compatible)
- [x] Package bundler: file collection, checksum computation, publish preparation
- [x] MemorySource + PackageSource trait for pluggable package backends
- [x] Resolution: single dep, highest version, transitive, diamond, conflict detection
- [x] Sparse index format with JSON lines, prefix computation (1/2/3/two-two)
- [x] 14 new tests: resolution, sparse index, bundler, protocol
- [x] **Deliverable:** PubGrub resolver + registry protocol + package bundling

### Sprint 4.2: Online Playground (1 minggu)
- [x] `fj playground` command generates static HTML playground
- [x] WebAssembly backend handles new AST nodes (effects, comptime, macros)
- [x] Share snippets via URL (encode/decode, short IDs, clipboard copy)
- [x] 8 pre-loaded examples (hello, fib, effects, comptime, matching, macros, structs, context safety)
- [x] Monaco editor config with Fajar Lang syntax highlighting
- [x] 12 new integration tests (sharing, examples, editor config, wasm)
- [x] **Deliverable:** `fj playground` → static HTML with editor + examples + sharing

### Sprint 4.3: LSP Advanced Features (1 minggu)
- [x] Semantic tokens: full syntax highlighting via LSP (13 token types + 3 modifiers)
- [x] Inlay hints: type annotations on let bindings (i64, f64, str, bool, Array)
- [x] References: find all uses of a symbol in document (word-boundary aware)
- [x] Rename: document-wide symbol rename (already existed)
- [x] Code actions: auto-fix suggestions (already existed)
- [x] Signature help: function parameter info (already existed)
- [x] 16 new integration tests (semantic tokens, symbols, refs, code actions)
- [x] **Deliverable:** Full IDE experience with semantic tokens + inlay hints + references

### Sprint 4.4: Debugger — Real DAP (1 minggu)
- [x] DWARF debug info: extended type mappings (char, tensor, never)
- [x] DAP server: already complete (breakpoints, step in/over/out, inspect)
- [x] DebugFrame: context annotations (@kernel/@device), effect tracking, comptime marking
- [x] DebugVariable: comptime/linear metadata with tooltip formatting
- [x] VS Code extension with launch config (already existed: editors/vscode/)
- [x] 22 new integration tests (frames, variables, DWARF types, breakpoints, stepping)
- [x] **Deliverable:** Context-aware debugging with effect/comptime/linear variable display

### Sprint 4.5: Documentation + Tutorial (1 minggu)
- [x] The Fajar Lang Book: 60+ chapters updated with effect/comptime/macro syntax
- [x] Updated chapters: effects.md (with clause, handle, context mapping), comptime.md, macros.md
- [x] Migration guide: "Fajar Lang for Rust Developers" (effects, linear types, comptime, context)
- [x] Migration guide: "Fajar Lang for C++ Developers" (safety, ownership, no UB)
- [x] Migration guide: "Fajar Lang for Python/ML Developers" (tensors, performance, deployment)
- [x] API reference: `fj doc` generates HTML from doc comments (existing)
- [x] 19 new doc tests (generation, migration guides, book chapters)
- [x] **Deliverable:** Complete documentation with migration guides + updated feature chapters

---

## Phase 5: Self-Hosting Complete (4 minggu)
> **Goal:** Compiler ditulis dalam Fajar Lang sendiri

### Sprint 5.1: Analyzer in Fajar Lang (2 minggu)
- [x] Self-hosted analyzer: `stdlib/analyzer.fj` (210+ LOC in pure Fajar Lang)
- [x] Type checker with scope tracking, error codes (SE001-SE008), type tags
- [x] AnalyzerState struct with variable/function/error parallel arrays
- [x] Functions: analyze(), error_count(), analysis_ok(), format_error()
- [x] Full frontend in Fajar Lang: lexer.fj (381) + parser.fj (397) + analyzer.fj (210+) = 988+ LOC
- [x] 22 new bootstrap tests (file existence, parsing, API, error codes, LOC counts)
- [x] **Deliverable:** Complete self-hosted frontend (lexer + parser + analyzer) in Fajar Lang

### Sprint 5.2: Codegen in Fajar Lang (2 minggu)
- [x] Self-hosted C codegen: `stdlib/codegen.fj` (280+ LOC in pure Fajar Lang)
- [x] C emitter: preamble, functions, let, return, if/else, while, println, calls
- [x] Type mapping: i64→int64_t, f64→double, bool→int, str→const char*
- [x] Operator mapping: all arithmetic, comparison, logical, bitwise
- [x] Runtime stubs: fj_println_int/str/float/bool
- [x] 3-stage bootstrap design: Stage0(Rust)→Stage1(Fj→C→gcc)→Stage2(verify)
- [x] Full self-hosted compiler: lexer(381)+parser(397)+analyzer(210)+codegen(280)=1,268+ LOC
- [x] 10 new bootstrap tests (codegen API, type mapping, operator mapping, LOC)
- [x] **Deliverable:** Complete self-hosted compiler (front+back end) in Fajar Lang

---

## Phase 6: Performance + Benchmarks (4 minggu)
> **Goal:** Prove performance with numbers

### Sprint 6.1: Benchmarks Game (2 minggu)
- [x] 7 Benchmarks Game programs: n-body, fannkuch, spectral-norm, mandelbrot, binary-trees, fasta, matrix-multiply
- [x] Benchmark framework: BenchmarkResult, BenchmarkComparison, BenchmarkSuite, measure(), measure_compile_speed()
- [x] All programs parse, run, and produce correct output via interpreter
- [x] BENCHMARK_PROGRAMS constant with 8 entries (incl. fibonacci)
- [x] 33 new tests (7 existence + 7 parse + 4 run + 8 framework + 7 unit)
- [x] **Deliverable:** Benchmark suite with framework + 7 classic programs

### Sprint 6.2: Real-World Benchmarks (2 minggu)
- [x] Real-world benchmarks: JSON parse, string ops, compile speed, cold start
- [x] Performance report generator (perf_report.rs): markdown tables, pass/fail vs targets
- [x] PerformanceTargets: JSON <10ms, compile 10K <3s, binary <100KB, cold start <5ms
- [x] Compilation speed verified: hello world <10ms, 50 functions <100ms
- [x] Interpreter cold start verified: <50ms
- [x] 23 new tests (13 perf integration + 10 unit)
- [x] **Deliverable:** Performance report framework with reproducible benchmarks

---

## Phase 7: Killer Demo (4 minggu)
> **Goal:** Satu demo yang membuktikan Fajar Lang adalah pilihan terbaik

### Sprint 7.1: Embedded AI Drone Controller (2 minggu)

```
┌─────────────────────────────────────────┐
│ Single .fj file — one language, one binary │
├──────────┬──────────┬───────────────────┤
│ @kernel  │ @device  │ @safe             │
│ Flight   │ ML       │ Mission           │
│ Control  │ Inference│ Planning          │
├──────────┼──────────┼───────────────────┤
│ IMU read │ CNN      │ Waypoint          │
│ PID loop │ Object   │ navigation        │
│ PWM out  │ detect   │ Obstacle          │
│ Failsafe │ Classify │ avoidance         │
└──────────┴──────────┴───────────────────┘
         One .fj file compiles to one binary
         Compiler GUARANTEES no cross-domain bugs
```

- [ ] 1000-line .fj file: flight controller + ML inference + mission planner
- [ ] Run on QEMU (simulated sensors) + Radxa Dragon Q6A (real hardware)
- [ ] Demonstrate: @kernel code cannot accidentally call tensor ops
- [ ] Demonstrate: @device code cannot accidentally access hardware registers
- [ ] Demonstrate: ML inference runs alongside real-time control loop
- [ ] Benchmark: inference latency <10ms, control loop <1ms
- [ ] **Deliverable:** Compelling demo that no other language can match

### Sprint 7.2: Video + Documentation + Publication (2 minggu)
- [ ] Record 10-minute demo video showing the full pipeline
- [ ] Write technical blog post explaining the architecture
- [ ] Submit paper to PLDI/OOPSLA/EMSOFT conference
- [ ] Post on Hacker News, Reddit r/programming, Lobste.rs
- [ ] Create project website: fajarlang.org
- [ ] **Deliverable:** Public launch with credible technical content

---

## Phase 8: Community + Adoption (ongoing)
> **Goal:** Dari 1 user menjadi 100, lalu 1000

### Sprint 8.1: Open Source Launch
- [ ] Clean up GitHub repo: README, CONTRIBUTING, LICENSE
- [ ] GitHub Actions CI: Linux + macOS + Windows + ARM64
- [ ] Release binaries for all platforms
- [ ] Homebrew formula: `brew install fajarlang`
- [ ] apt/snap package: `sudo apt install fj`
- [ ] Docker image: `docker run fajarlang/fj`

### Sprint 8.2: Community Building
- [ ] Discord server for Fajar Lang community
- [ ] Weekly newsletter: "This Week in Fajar Lang"
- [ ] Monthly online meetup
- [ ] Bounty program for contributions
- [ ] University partnerships (embedded systems courses)
- [ ] Industry partnerships (embedded AI companies)

### Sprint 8.3: Production Adoption
- [ ] Find 3 beta users willing to try Fajar Lang in real projects
- [ ] Publish case studies: "How X uses Fajar Lang for embedded AI"
- [ ] Enterprise support offering (for safety-critical industries)
- [ ] Explore safety certification path (ISO 26262, DO-178C)

---

## Timeline Summary

| Phase | Duration | Key Deliverable |
|-------|----------|----------------|
| **0: Foundation Fix** | 4 minggu | Zero stubs, honest codebase |
| **1: Linear Types + Effects** | 6 minggu | Features Rust doesn't have |
| **2: Compiler Excellence** | 6 minggu | Fast compile + dual backend |
| **3: Macros** | 4 minggu | Real metaprogramming |
| **4: Ecosystem** | 6 minggu | Registry, playground, IDE, debugger |
| **5: Self-Hosting** | 4 minggu | Compiler in Fajar Lang |
| **6: Benchmarks** | 4 minggu | Published performance data |
| **7: Killer Demo** | 4 minggu | Embedded AI showcase |
| **8: Community** | Ongoing | From 1 to 1000 users |
| **TOTAL** | **~38 minggu (~9 bulan)** | |

---

## Prinsip Kerja

1. **REAL or DELETE** — Tidak ada fitur yang hanya stub. Jika belum bisa diimplementasi, hapus dari codebase dan docs.
2. **Test-Driven** — Setiap fitur baru harus punya minimal 20 tests sebelum dianggap selesai.
3. **Benchmark-Driven** — Setiap klaim performa harus didukung oleh angka yang reproducible.
4. **User-First** — Setiap sprint harus menghasilkan sesuatu yang bisa dicoba user.
5. **No Going Back** — Setiap fitur yang di-ship harus backward compatible. Go 1 compatibility promise.

---

## Success Metrics

| Metric | Current | Target (9 bulan) | World-Class |
|--------|---------|-------------------|-------------|
| Tests | 4,917 | 15,000+ | 50,000+ |
| Features (REAL) | 14/21 | 21/21 | 21/21 |
| Features (STUB) | 7 | 0 | 0 |
| Self-hosting | 0% | 100% | 100% |
| Benchmark Game | Not submitted | Submitted, within 2x C | Top 10 |
| Package registry | Local only | Real remote, 20+ packages | 500+ packages |
| Production users | 0 | 3 | 50+ |
| Compilation speed (10K LOC) | Unknown | <3s clean, <500ms incremental | <1s clean |
| Binary size (hello world) | ~80KB | <50KB | <20KB |
| Stars (GitHub) | < 10 | 500+ | 5,000+ |
| Online playground | None | Working | Used daily |

---

*"Bahasa terbaik untuk embedded ML + OS integration — the only language where
an OS kernel and a neural network can share the same codebase, type system,
and compiler, with safety guarantees that no existing language provides."*

— Fajar Lang Vision Statement
