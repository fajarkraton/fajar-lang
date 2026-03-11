# Fajar Lang v1.0.0 "Genesis" — Implementation Plan

> First stable production release. 7 phases, 28 sprints, 280 tasks.
> Focus: stability, performance, cross-platform, LSP, documentation, ecosystem, release engineering.

---

## Overview

v1.0.0 "Genesis" is the first stable production release of Fajar Lang. The goal is not to add features but to harden, polish, optimize, document, and ship what exists.

---

## Phase 1 — Stability & Conformance Testing (S1-S4)

### Sprint 1: Fuzz Testing Infrastructure `P0`
- [x] S1.1 — `tests/fuzz/fuzz_lexer.rs`: Fuzz harness for `tokenize()` — arbitrary byte strings, assert no panics
- [x] S1.2 — `tests/fuzz/fuzz_parser.rs`: Fuzz harness for `parse()` — random token streams, assert no panics
- [x] S1.3 — `tests/fuzz/fuzz_analyzer.rs`: Fuzz harness for `analyze()` — random ASTs, assert no panics
- [x] S1.4 — `tests/fuzz/fuzz_interpreter.rs`: Grammar-aware fuzzing for `eval_source()` with timeout
- [x] S1.5 — `tests/fuzz/grammar_gen.rs`: Grammar-aware input generator from EBNF
- [x] S1.6 — `tests/fuzz/fuzz_formatter.rs`: Fuzz formatter — assert idempotency `format(format(x)) == format(x)`
- [x] S1.7 — `tests/fuzz/fuzz_vm.rs`: Fuzz bytecode VM — compile random ASTs and run
- [x] S1.8 — `tests/fuzz/corpus/`: Seed corpus from all example `.fj` files
- [x] S1.9 — `.github/workflows/fuzz.yml`: CI job running fuzz targets for 60s per PR
- [x] S1.10 — `docs/FUZZING.md`: Fuzzing documentation + 10 tests

### Sprint 2: Conformance Test Suite `P0`
- [x] S2.1 — `tests/conformance/runner.rs`: Test runner with `// expect:` annotation matching
- [x] S2.2 — `tests/conformance/lexer/`: 20 lexer conformance tests (all LE errors, literals, keywords)
- [x] S2.3 — `tests/conformance/parser/`: 20 parser conformance tests (all PE errors, all stmt/expr forms)
- [x] S2.4 — `tests/conformance/types/`: 20 type system conformance tests (all SE errors, generics, traits)
- [x] S2.5 — `tests/conformance/context/`: 15 context system tests (KE001-KE004, DE001-DE003)
- [x] S2.6 — `tests/conformance/ownership/`: 15 ownership tests (ME001-ME008, NLL, RAII)
- [x] S2.7 — `tests/conformance/tensor/`: 15 tensor system tests (TE001-TE008, autograd)
- [x] S2.8 — `tests/conformance/runtime/`: 15 runtime error tests (RE001-RE008)
- [x] S2.9 — `tests/conformance/features/`: 20 language feature tests (closures, async, effects, macros)
- [x] S2.10 — `tests/conformance/README.md`: Conformance test format documentation

### Sprint 3: Regression Test Harness `P0`
- [x] S3.1 — `tests/regression/runner.rs`: Regression runner with `.expected` snapshot comparison
- [x] S3.2 — `tests/regression/cases/`: 30 regression test cases from v0.3-v0.9 bug fixes
- [x] S3.3 — `tests/regression/snapshot.rs`: Snapshot update mode (`BLESS=1`)
- [x] S3.4 — `benches/baselines/recorder.rs`: Baseline performance recorder (JSON)
- [x] S3.5 — `benches/baselines/comparator.rs`: Performance comparator with threshold
- [x] S3.6 — `tests/regression/compile_time.rs`: Compilation time regression tests
- [x] S3.7 — `tests/regression/memory.rs`: Memory usage regression tests
- [x] S3.8 — CI update: regression test step with baseline comparison
- [x] S3.9 — `scripts/bisect.sh`: Git bisect helper script
- [x] S3.10 — 10 tests validating regression harness

### Sprint 4: Error Message Polish `P0`
- [x] S4.1 — Improve all 8 LE error messages with `help:` suggestions
- [x] S4.2 — Improve all 10 PE error messages with "did you mean?" suggestions
- [x] S4.3 — Improve all 12 SE error messages with type display
- [x] S4.4 — Context errors (KE/DE): specific suggestions for context violations
- [x] S4.5 — Tensor errors (TE): shape visualization in error messages
- [x] S4.6 — Runtime errors (RE): stack trace, value display, assertion diffs
- [x] S4.7 — Memory errors (ME): lifetime visualization, borrow origins
- [x] S4.8 — `FjDiagnostic` wrapper with consistent miette formatting
- [x] S4.9 — `docs/ERROR_CODES.md`: comprehensive update with all v0.3-v0.9 errors
- [x] S4.10 — 10 tests per error category verifying improved messages

---

## Phase 2 — Performance Engineering (S5-S8)

### Sprint 5: Interpreter Performance `P1`
- [x] S5.1 — String interning table for identifier lookups (`Symbol` indices)
- [x] S5.2 — Replace `HashMap<String, Value>` with `Vec<Value>` indexed by Symbol
- [x] S5.3 — Inline caching for method dispatch
- [x] S5.4 — Fast-path for common binary ops (Int+Int, Float+Float)
- [x] S5.5 — Specialize `eval_expr` for `Expr::Ident` fast path
- [x] S5.6 — Optimize `Value` enum size — box large variants
- [x] S5.7 — Tail-call optimization for recursive functions
- [x] S5.8 — Constant folding pass before interpretation
- [x] S5.9 — New benchmarks for each optimization
- [x] S5.10 — 10 tests verifying identical results after optimizations

### Sprint 6: Cranelift Codegen Optimization `P1`
- [x] S6.1 — Register allocation hints for hot variables
- [x] S6.2 — Branch layout optimization for loops
- [x] S6.3 — Strength reduction (`x*2` → `x<<1`, `x%pow2` → `x&(pow-1)`)
- [x] S6.4 — Inline small builtins (abs, min, max, clamp) via Cranelift `select`
- [x] S6.5 — Bounds check elimination for sequential array access in loops
- [x] S6.6 — Switch-to-jump-table for dense integer match (`br_table`)
- [x] S6.7 — Function inlining heuristic (<20 IR instructions, hot loops)
- [x] S6.8 — Common subexpression elimination within basic blocks
- [x] S6.9 — `--opt-level` support (O0/O1/O2) mapped to Cranelift settings
- [x] S6.10 — 10 tests verifying optimized codegen correctness

### Sprint 7: Memory Allocator Optimization `P1`
- [x] S7.1 — String interner for token text (SymbolId references)
- [x] S7.2 — Replace `String` fields in Token with `SymbolId`
- [x] S7.3 — Arena allocator for AST nodes
- [x] S7.4 — Convert `Box<Expr>` to arena references
- [x] S7.5 — Pool allocator for Scope objects
- [x] S7.6 — Small-string optimization for `Value::Str` (SSO ≤22 bytes)
- [x] S7.7 — Reuse CodegenCtx between function compilations
- [x] S7.8 — Memory-mapped artifact cache (`memmap2`)
- [x] S7.9 — Allocator benchmark suite
- [x] S7.10 — 10 tests verifying allocator correctness

### Sprint 8: Compilation Speed `P1`
- [x] S8.1 — Parallel lexing for multi-file projects (`rayon`)
- [x] S8.2 — Lazy analysis (only reachable from main/@test)
- [x] S8.3 — Module-level incremental compilation
- [x] S8.4 — Parallel codegen for independent functions
- [x] S8.5 — Type checking cache (generic instantiation memoization)
- [x] S8.6 — Lazy parsing for IDE mode
- [x] S8.7 — Object code caching per function hash
- [x] S8.8 — Compilation speed benchmark (10K-line program)
- [x] S8.9 — `--timings` flag for per-phase timing breakdown
- [x] S8.10 — 10 tests verifying parallel/incremental correctness

---

## Phase 3 — Cross-Platform & Distribution (S9-S12)

### Sprint 9: Build Matrix `P0`
- [x] S9.1 — CI: Linux/macOS/Windows matrix with stable+nightly Rust
- [x] S9.2 — Platform abstraction via `#[cfg(target_os)]`
- [x] S9.3 — Windows path/line-ending fixes in interpreter
- [x] S9.4 — Formatter: normalize `\r\n` on all platforms
- [x] S9.5 — LSP: Windows URI handling (`file:///C:/...`)
- [x] S9.6 — DAP: Windows-compatible protocol handling
- [x] S9.7 — Platform-specific test suite
- [x] S9.8 — Cross-platform `fj.toml` path handling
- [x] S9.9 — Native feature testing on all platforms
- [x] S9.10 — 10 platform-specific tests

### Sprint 10: Static Binary Distribution `P0`
- [x] S10.1 — musl static Linux builds (x86_64 + aarch64)
- [x] S10.2 — `[profile.release-dist]` with LTO, strip, codegen-units=1
- [x] S10.3 — macOS universal binary (x86_64 + aarch64 lipo)
- [x] S10.4 — Windows static CRT build
- [x] S10.5 — `scripts/build-static.sh` for local builds
- [x] S10.6 — SHA256 checksums for all release artifacts
- [x] S10.7 — Automatic GitHub Release creation from CHANGELOG
- [x] S10.8 — Binary size targets in README
- [x] S10.9 — RISC-V64 cross-compilation target
- [x] S10.10 — 10 tests verifying static binary correctness

### Sprint 11: Installation Scripts `P1`
- [x] S11.1 — `scripts/install.sh`: curl installer (detect OS/arch, download, install)
- [x] S11.2 — `scripts/install.ps1`: PowerShell installer for Windows
- [x] S11.3 — `packaging/homebrew/fj.rb`: Homebrew formula
- [x] S11.4 — `packaging/apt/`: Debian package generation
- [x] S11.5 — Verify `cargo install fajar-lang` works
- [x] S11.6 — Version management (`fj-up update`, `fj-up install 1.0.0`)
- [x] S11.7 — `fj --version` detailed output (git hash, build date, target)
- [x] S11.8 — Shell completions (bash, zsh, fish, PowerShell)
- [x] S11.9 — Unix man page `man/fj.1`
- [x] S11.10 — 10 tests verifying installer scripts

### Sprint 12: Platform-Specific Optimizations `P1`
- [x] S12.1 — Platform memory page size detection
- [x] S12.2 — Thread count detection for parallel compilation
- [x] S12.3 — Linux io_uring capability detection
- [x] S12.4 — macOS GCD detection for thread pool hints
- [x] S12.5 — x86_64 CPU feature detection (SSE4.2, AVX2, AVX-512)
- [x] S12.6 — ARM64 NEON detection
- [x] S12.7 — Platform-specific virtual memory ops (mmap/VirtualAlloc)
- [x] S12.8 — REPL terminal width/color detection
- [x] S12.9 — Unified `PlatformInfo` struct
- [x] S12.10 — 10 platform detection tests

---

## Phase 4 — Language Server Completion (S13-S16)

### Sprint 13: Go-to-Definition & Find References `P0`
- [x] S13.1 — `SymbolIndex` struct mapping names to definition locations
- [x] S13.2 — `IndexBuilder` walking AST for fn/let/const/struct/enum/trait/impl
- [x] S13.3 — Improved `goto_definition` using SymbolIndex
- [x] S13.4 — Cross-scope resolution (parent scopes, closures, modules)
- [x] S13.5 — `find_all_references(symbol, span)` returning all usage spans
- [x] S13.6 — `textDocument/references` handler
- [x] S13.7 — Struct field go-to-definition
- [x] S13.8 — Method go-to-definition (impl block resolution)
- [x] S13.9 — Module path resolution (`mod::func()`)
- [x] S13.10 — 10 tests for go-to-def and find-references

### Sprint 14: Code Actions `P1`
- [x] S14.1 — `CodeActionProvider` with `provide_actions(diagnostics, range)`
- [x] S14.2 — "Add missing import" action for SE002
- [x] S14.3 — "Make variable mutable" action for SE007
- [x] S14.4 — "Extract function" action
- [x] S14.5 — "Inline variable" action
- [x] S14.6 — "Add type annotation" action
- [x] S14.7 — "Convert to match" action (if/else chain → match)
- [x] S14.8 — "Add missing fields" action for SE012
- [x] S14.9 — Wire `textDocument/codeAction` handler
- [x] S14.10 — 10 tests verifying code action correctness

### Sprint 15: Semantic Tokens `P0`
- [x] S15.1 — `SemanticTokensProvider` walking AST
- [x] S15.2 — Token types: function, variable, parameter, type, struct, enum, property, keyword, etc.
- [x] S15.3 — Token modifiers: declaration, readonly, static, deprecated, async, mutable
- [x] S15.4 — Distinguish declarations vs usages, calls vs definitions
- [x] S15.5 — Context annotation highlighting (@kernel, @device, @safe, @unsafe)
- [x] S15.6 — Effect annotation highlighting (perform, handle, resume)
- [x] S15.7 — Macro invocation highlighting
- [x] S15.8 — Register SemanticTokensOptions in server capabilities
- [x] S15.9 — Implement `semanticTokens/full` and `semanticTokens/range`
- [x] S15.10 — 10 tests verifying token types and modifiers

### Sprint 16: Signature Help & Call Hierarchy `P1`
- [x] S16.1 — `SignatureProvider` resolving function signatures at cursor
- [x] S16.2 — Active parameter tracking (comma counting)
- [x] S16.3 — Overload display for trait methods
- [x] S16.4 — Builtin function signatures (30+ builtins)
- [x] S16.5 — Doc comment integration in signature help
- [x] S16.6 — `CallHierarchyProvider` resolving callers and callees
- [x] S16.7 — Incoming calls (who calls this function)
- [x] S16.8 — Outgoing calls (what does this function call)
- [x] S16.9 — Wire signatureHelp and callHierarchy handlers
- [x] S16.10 — 10 tests for signatures and call hierarchy

---

## Phase 5 — Documentation & Learning (S17-S20)

### Sprint 17: Language Reference Manual `P0`
- [x] S17.1 — `book/src/reference/types.md`: Complete type reference
- [x] S17.2 — `book/src/reference/operators.md`: All 19 precedence levels
- [x] S17.3 — `book/src/reference/expressions.md`: All 25+ expression variants
- [x] S17.4 — `book/src/reference/statements.md`: All statement types
- [x] S17.5 — `book/src/reference/ownership.md`: Ownership, borrowing, NLL, RAII
- [x] S17.6 — `book/src/reference/contexts.md`: @safe/@kernel/@device/@unsafe
- [x] S17.7 — `book/src/reference/effects.md`: Effect system reference
- [x] S17.8 — `book/src/reference/macros.md`: Macro system reference
- [x] S17.9 — `book/src/reference/concurrency.md`: Threads, channels, async
- [x] S17.10 — Update `SUMMARY.md` with all new chapters

### Sprint 18: Tutorial Series `P0`
- [x] S18.1 — Tutorial 1: Hello World (install, compile, run)
- [x] S18.2 — Tutorial 2: Variables & Types
- [x] S18.3 — Tutorial 3: Control Flow (if, match, loops)
- [x] S18.4 — Tutorial 4: Functions & Closures (pipeline |>)
- [x] S18.5 — Tutorial 5: Structs & Enums (Option/Result, ?)
- [x] S18.6 — Tutorial 6: Generics & Traits
- [x] S18.7 — Tutorial 7: Ownership & Borrowing
- [x] S18.8 — Tutorial 8: ML & Tensors (MNIST walkthrough)
- [x] S18.9 — Tutorial 9: Embedded (@kernel, @device, HAL)
- [x] S18.10 — Tutorial 10: Project Structure (fj.toml, testing, docs)

### Sprint 19: API Documentation Improvements `P1`
- [x] S19.1 — Cross-reference resolver for `[TypeName]` links
- [x] S19.2 — Full-text search index (JSON)
- [x] S19.3 — Module hierarchy navigation (sidebar tree)
- [x] S19.4 — Source code view with line numbers
- [x] S19.5 — Version selector dropdown
- [x] S19.6 — Example rendering with syntax highlighting
- [x] S19.7 — Trait implementation list per type
- [x] S19.8 — CSS theme (dark/light mode, responsive)
- [x] S19.9 — Deprecation display with warning banners
- [x] S19.10 — 10 tests for doc generation

### Sprint 20: Interactive Playground `P1`
- [x] S20.1 — Wasm entry point (`eval_source_wasm`)
- [x] S20.2 — Output capture (println → buffer)
- [x] S20.3 — Timeout enforcement (5s)
- [x] S20.4 — HTML layout (editor + output panes)
- [x] S20.5 — CodeMirror 6 editor with syntax highlighting
- [x] S20.6 — Wasm loader and runtime bridge
- [x] S20.7 — Example selector dropdown
- [x] S20.8 — Share functionality (base64 URL)
- [x] S20.9 — Build script (wasm-pack + wasm-opt)
- [x] S20.10 — 10 tests for playground

---

## Phase 6 — Ecosystem & Interop (S21-S24)

### Sprint 21: C/C++ Header Generation `P1`
- [x] S21.1 — `CBindgenContext` collecting @ffi items
- [x] S21.2 — Type mapping table (Fajar → C types)
- [x] S21.3 — Struct layout generation with padding/alignment
- [x] S21.4 — Enum generation (simple → C enum, data → tagged union)
- [x] S21.5 — Function declaration generation
- [x] S21.6 — Include guard generation
- [x] S21.7 — Doc comment passthrough (`///` → `/** */`)
- [x] S21.8 — `fj cbindgen` subcommand
- [x] S21.9 — C++ compatibility (`extern "C"`)
- [x] S21.10 — 10 tests for header generation

### Sprint 22: Python Bindings Generator `P1`
- [x] S22.1 — `PyBindgenContext` collecting @python items
- [x] S22.2 — Type mapping (Fajar → Python types)
- [x] S22.3 — Function wrapper generation (ctypes)
- [x] S22.4 — Class wrapper for structs
- [x] S22.5 — Enum wrapper (enum.Enum / dataclass)
- [x] S22.6 — Tensor bridge (Fajar Tensor ↔ NumPy ndarray)
- [x] S22.7 — Error bridge (Result → exceptions)
- [x] S22.8 — `__init__.py` + `.pyi` type stubs
- [x] S22.9 — `fj pybindgen` subcommand
- [x] S22.10 — 10 tests for Python bindings

### Sprint 23: Wasm Component Model `P1`
- [x] S23.1 — `WasmComponentCompiler` wrapping existing backend
- [x] S23.2 — WIT generation from @wasm pub fn declarations
- [x] S23.3 — Interface type mapping (str→string, Array→list, Result→result)
- [x] S23.4 — Resource type generation (structs with methods)
- [x] S23.5 — Import interface generation
- [x] S23.6 — Export interface generation
- [x] S23.7 — Component linking metadata
- [x] S23.8 — `--component` flag for `fj build --backend wasm`
- [x] S23.9 — `examples/wasm_component.fj` example
- [x] S23.10 — 10 tests for component model

### Sprint 24: Package Registry Improvements `P1`
- [x] S24.1 — Vulnerability scanning against advisory database
- [x] S24.2 — `fj audit` subcommand
- [x] S24.3 — Advisory database update mechanism
- [x] S24.4 — SBOM generation (CycloneDX JSON)
- [x] S24.5 — `fj sbom` subcommand
- [x] S24.6 — Rate limiting for registry operations
- [x] S24.7 — Package integrity verification (SHA256 + signature)
- [x] S24.8 — License compliance checking
- [x] S24.9 — Yanked version support
- [x] S24.10 — 10 tests for registry improvements

---

## Phase 7 — Release Engineering (S25-S28)

### Sprint 25: Release Automation `P0`
- [x] S25.1 — Complete release pipeline (test → build → publish)
- [x] S25.2 — Artifact signing (GPG)
- [x] S25.3 — Release notes extraction from CHANGELOG
- [x] S25.4 — Homebrew formula auto-update
- [x] S25.5 — crates.io auto-publish
- [x] S25.6 — Nightly build pipeline
- [x] S25.7 — PR quality gates (tests, clippy, coverage, benchmarks)
- [x] S25.8 — `scripts/release.sh` manual preparation
- [x] S25.9 — Post-release smoke testing
- [x] S25.10 — 10 tests for release pipeline

### Sprint 26: Binary Size Optimization `P1`
- [x] S26.1 — `strip = "symbols"` in release profile
- [x] S26.2 — LTO profiles (thin for dev, fat for release)
- [x] S26.3 — `codegen-units = 1` for release
- [x] S26.4 — `panic = "abort"` for release
- [x] S26.5 — Feature-gate optional modules (bsp, iot, rtos, llvm, wasm)
- [x] S26.6 — Binary size analysis script (`cargo bloat`)
- [x] S26.7 — Binary size tracking in CI
- [x] S26.8 — Minimize ndarray footprint
- [x] S26.9 — Dead code elimination at link time
- [x] S26.10 — 10 binary size tests

### Sprint 27: Backwards Compatibility `P0`
- [x] S27.1 — Edition stability tests (2025, 2026)
- [x] S27.2 — Syntax stability: 50 golden tests
- [x] S27.3 — Semantic stability: 50 golden tests
- [x] S27.4 — Runtime behavior stability: 50 golden tests
- [x] S27.5 — Stdlib API stability tests
- [x] S27.6 — Deprecation migration testing
- [x] S27.7 — `docs/STABILITY.md`: stability policy document
- [x] S27.8 — Error code stability tests
- [x] S27.9 — API surface snapshot for diff detection
- [x] S27.10 — 10 tests for stability infrastructure

### Sprint 28: Release Notes & Announcement `P0`
- [x] S28.1 — v1.0.0 CHANGELOG entry
- [x] S28.2 — Migration guide (v0.9 → v1.0)
- [x] S28.3 — Release announcement draft
- [x] S28.4 — README update to v1.0.0
- [x] S28.5 — Version bump to `1.0.0`
- [x] S28.6 — Language comparison benchmarks
- [x] S28.7 — 5 showcase examples
- [x] S28.8 — Book introduction update
- [x] S28.9 — CLAUDE.md update
- [x] S28.10 — Final verification (all tests pass, clippy clean, fmt clean)

---

## Summary

| Phase | Sprints | Focus | Tasks |
|-------|---------|-------|-------|
| 1 — Stability & Conformance | S1-S4 | Fuzz testing, conformance, regression, error polish | 40 |
| 2 — Performance Engineering | S5-S8 | Interpreter, codegen, allocator, compilation speed | 40 |
| 3 — Cross-Platform & Distribution | S9-S12 | Build matrix, static binaries, installers, platform opts | 40 |
| 4 — Language Server Completion | S13-S16 | Go-to-def, code actions, semantic tokens, signatures | 40 |
| 5 — Documentation & Learning | S17-S20 | Reference manual, tutorials, doc generator, playground | 40 |
| 6 — Ecosystem & Interop | S21-S24 | C headers, Python bindings, Wasm components, registry | 40 |
| 7 — Release Engineering | S25-S28 | CI/CD, binary size, stability, release notes | 40 |
| **Total** | **28** | | **280** |
