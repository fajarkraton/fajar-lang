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

- [ ] Design macro expansion pipeline: parse → expand → re-parse
- [ ] Implement `#[derive()]` for common traits (Debug, Clone, PartialEq, Serialize)
- [ ] Implement function-like macros with token stream manipulation
- [ ] Hygiene: macro-generated names don't conflict with user names
- [ ] 30+ tests: derive macros, custom macros, hygiene
- [ ] **Deliverable:** Working macro system rivaling Rust's proc_macro

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

- [ ] Implement `macro_rules!` with pattern matching
- [ ] Support repetition patterns: `$($x:expr),*`
- [ ] Implement `include!()` for file inclusion
- [ ] Implement `stringify!()` and `concat!()`
- [ ] 20+ tests: vec!, hashmap!, format patterns
- [ ] **Deliverable:** Declarative macros for common patterns

---

## Phase 4: Ecosystem + Tooling (6 minggu)
> **Goal:** Go-level developer experience. ONE binary, everything works.

### Sprint 4.1: Package Registry — Real Remote (2 minggu)
- [ ] Build real registry server (Rust, PostgreSQL, S3)
- [ ] `fj publish` — upload package with signing
- [ ] `fj add` — download from registry with version resolution
- [ ] `fj search` — search packages
- [ ] Implement PubGrub-style dependency resolution
- [ ] Host at registry.fajarlang.org
- [ ] Seed with 20+ packages (stdlib ports, utilities)
- [ ] **Deliverable:** Working package registry like crates.io (minimal)

### Sprint 4.2: Online Playground (1 minggu)
- [ ] Build web playground: editor + compile + run
- [ ] WebAssembly backend: compile Fajar Lang to Wasm, run in browser
- [ ] Share snippets via URL
- [ ] Pre-loaded examples (hello world, ML demo, OS kernel snippet)
- [ ] Host at play.fajarlang.org
- [ ] **Deliverable:** "Try Fajar Lang in 5 seconds" experience

### Sprint 4.3: LSP Advanced Features (1 minggu)
- [ ] Implement rename (all references)
- [ ] Implement code actions (auto-fix suggestions)
- [ ] Implement semantic tokens (syntax highlighting via LSP)
- [ ] Implement inlay hints (type annotations, parameter names)
- [ ] Implement signature help (function parameter info)
- [ ] Test in VS Code, Neovim, Zed
- [ ] **Deliverable:** IDE experience rivaling rust-analyzer

### Sprint 4.4: Debugger — Real DAP (1 minggu)
- [ ] Generate DWARF debug info from Cranelift
- [ ] Implement DAP server with breakpoints, step, inspect
- [ ] Source-level debugging in VS Code
- [ ] Variable inspection, call stack, watch expressions
- [ ] Test: set breakpoint in .fj file, hit it, inspect variables
- [ ] **Deliverable:** Step-through debugging in VS Code

### Sprint 4.5: Documentation + Tutorial (1 minggu)
- [ ] Write "The Fajar Lang Book" — 20 chapters, beginner to advanced
- [ ] Interactive tutorial (like Rust by Example)
- [ ] API reference auto-generated from doc comments
- [ ] Migration guide: "Fajar Lang for Rust developers"
- [ ] Migration guide: "Fajar Lang for C++ developers"
- [ ] Migration guide: "Fajar Lang for Python/ML developers"
- [ ] **Deliverable:** World-class documentation

---

## Phase 5: Self-Hosting Complete (4 minggu)
> **Goal:** Compiler ditulis dalam Fajar Lang sendiri

### Sprint 5.1: Analyzer in Fajar Lang (2 minggu)
- [ ] Port type checker to Fajar Lang (`stdlib/analyzer.fj`)
- [ ] Port borrow checker to Fajar Lang
- [ ] Verify: analyze any .fj program, same errors as Rust analyzer
- [ ] **Deliverable:** Full frontend (lexer + parser + analyzer) in Fajar Lang

### Sprint 5.2: Codegen in Fajar Lang (2 minggu)
- [ ] Port Cranelift codegen to Fajar Lang (or emit C as intermediate)
- [ ] Bootstrap: compile self-hosted compiler with Rust compiler
- [ ] Verify: self-hosted compiler produces identical output
- [ ] 3-stage bootstrap verified: stage0 (Rust) → stage1 (Fj) → stage2 (Fj, compiled by stage1)
- [ ] **Deliverable:** Fajar Lang compiler written in Fajar Lang

---

## Phase 6: Performance + Benchmarks (4 minggu)
> **Goal:** Prove performance with numbers

### Sprint 6.1: Benchmarks Game (2 minggu)
- [ ] Implement all 13 Benchmarks Game programs in Fajar Lang
- [ ] Optimize each to be competitive with Rust/C/Go
- [ ] Submit to Computer Language Benchmarks Game
- [ ] Publish results: "Fajar Lang vs Rust vs Go vs C"
- [ ] Target: within 2x of C for all benchmarks, faster than Go
- [ ] **Deliverable:** Published benchmark results

### Sprint 6.2: Real-World Benchmarks (2 minggu)
- [ ] JSON parser: parse 1MB JSON in <10ms
- [ ] HTTP server: >500K req/s (hello world)
- [ ] Matrix multiply 1000x1000: within 2x of BLAS
- [ ] Compilation speed: 10K LOC in <3s (incremental <500ms)
- [ ] Binary size: hello world <100KB
- [ ] Cold start: <5ms
- [ ] Publish all results on website
- [ ] **Deliverable:** Performance claims backed by reproducible benchmarks

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
