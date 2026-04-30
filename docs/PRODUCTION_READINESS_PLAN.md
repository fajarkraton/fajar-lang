# Production Readiness Plan — Honest Path to v7.0.0

> **ARCHIVED 2026-04-30** — superseded by [`docs/PRODUCTION_AUDIT_V1.md`](PRODUCTION_AUDIT_V1.md).
> Live tracker is now PRODUCTION_AUDIT_V1.md. This v7.0.0-era plan (2026-03-26) is preserved
> for historical reference; many items have been completed in V8-V31, and the V1 audit
> consolidates remaining gaps with current verification commands.

> **Date:** 2026-03-26
> **Status:** Assessment + Plan (ARCHIVED)
> **Goal:** Make Fajar Lang genuinely production-ready for target use cases
> **Principle:** Quality > Speed. Every task verified before marked done.

---

## What "Production Level" Means for Fajar Lang

Fajar Lang targets three specific use cases. "Production level" means a real user
can build a real project in each use case without hitting blockers:

1. **Embedded ML inference** — Deploy a neural network on edge hardware (Q6A, RPi)
2. **OS development** — Write a bare-metal kernel in pure Fajar Lang
3. **Systems programming** — Build CLI tools, servers, data pipelines

---

## Current Honest Assessment

### What IS Production-Ready (Verified)

| Component | LOC | Tests | Verified By |
|-----------|-----|-------|-------------|
| Lexer | 2,200 | 96 | Every `fj run` uses it |
| Parser | 4,800 | 195 | Every `fj run` uses it |
| Analyzer | 8,000 | 429 | Every `fj run` uses it |
| Interpreter | 6,000 | 177 | 168 examples run successfully |
| Cranelift JIT/AOT | 40,000 | 700+ | Q6A hardware verified |
| ML Runtime | 3,000 | 50 | MNIST training works |
| CLI | 2,500 | — | Used daily in development |
| FajarOS Nova (.fj) | 22,000 | — | QEMU boot verified |

### What Was Fixed in V8 (Real, but Needs Hardening)

| Component | What's Real | What Needs Work |
|-----------|-------------|-----------------|
| Crypto (GC1) | SHA/AES/Ed25519/Argon2 with NIST vectors | More algorithm variants, performance testing |
| Networking (GC1) | TCP/UDP/HTTP/DNS via std::net | Connection pooling, timeouts, TLS |
| C++ FFI (GC2) | libclang parsing, function extraction | Template support, error handling |
| Python FFI (GC2) | pyo3 calls, numpy bridge | GIL management, exception mapping |
| Distributed (GC3) | tokio TCP transport, actor mailboxes | Cluster discovery, fault tolerance |
| Z3 SMT (GC4) | Proof for bounds/shapes | Integration with analyzer |
| Formats (GC5) | JSON/TOML/CSV parsers | Streaming, error recovery |
| System (GC5) | process spawn, path ops | File watching, signal handling |
| Plugins (GC5) | CompilerPlugin trait, 2 lints | Plugin loading, API stability |
| Self-hosted (SH) | 1,754 lines lexer+parser+analyzer | Real AST, stage 2 bootstrap |

### What's NOT Production-Ready (Honest Gaps)

| Gap | Impact | Why It Matters |
|-----|--------|---------------|
| **No production users** | No real-world validation | Bugs hide until someone builds something real |
| **Borrow checker over-relaxed** | Array/Struct/Tuple are Copy | May hide ownership bugs in native codegen |
| **Error messages** | Functional but terse | Users need helpful "did you mean?" suggestions |
| **LLVM backend** | Basic, not optimized | Production binaries need -O2 quality code |
| **Package ecosystem** | Registry code exists, never served | Can't `fj install` anything real |
| **Cross-platform** | Linux primary, macOS/Windows untested | CI files exist but may not pass |
| **Documentation** | Extensive but outdated in places | CLAUDE.md had stale info until this session |
| **Performance** | Benchmarks exist, not regularly run | No regression tracking |

---

## Production Readiness Phases

### Phase 1: Hardening (Priority: HIGHEST)

*Fix the things that would bite a real user on day one.*

**1.1 Borrow Checker Correctness (10 tasks)**
- Revert Array/Tuple/Struct to non-Copy
- Implement proper implicit borrowing for function args (auto-&T)
- Add `Clone` trait so users can explicitly `.clone()` when needed
- Fix `x = f(x)` pattern without making everything Copy
- Integration tests: 20 ownership scenarios from real code patterns
- Verify FajarOS Nova still compiles after changes
- Verify self-hosted compiler still compiles
- Verify all 3 application templates still run
- Verify all 168 examples still pass
- Document ownership rules in FAJAR_LANG_SPEC.md

**1.2 Error Message Quality (10 tasks)**
- Audit all 71 error codes for clarity and helpfulness
- Add "did you mean?" suggestions for undefined variables (Levenshtein)
- Add type mismatch hints ("expected i64, found str — use to_int()")
- Add context-specific help ("return outside function — did you forget fn?")
- Test every error code has at least one test that triggers it
- Compare error output with Rust's error messages for 10 common mistakes
- Add source code snippets in error display (miette already does this)
- Test error display on Windows terminal (no ANSI escape issues)
- Measure: user can fix any error within 30 seconds of reading it
- Document all error codes with examples in ERROR_CODES.md

**1.3 Test Coverage Gaps (10 tasks)**
- Run tarpaulin, identify modules below 80% coverage
- Add tests for untested public functions (audit every `pub fn`)
- Add edge case tests: empty input, max values, Unicode, nested structures
- Add regression tests for every bug fixed in this session
- Property tests for parser (random valid programs should parse)
- Property tests for interpreter (deterministic execution)
- Fuzz lexer with 1M iterations, fix any crashes
- Fuzz parser with 1M iterations, fix any crashes
- Test every example in `examples/` via CI script
- Measure: 0 panics in production code paths

### Phase 2: Real-World Validation (Priority: HIGH)

*Build something real and fix what breaks.*

**2.1 Build a Real CLI Tool in Fajar Lang (10 tasks)**
- Choose a real utility (e.g., JSON formatter, line counter, file searcher)
- Implement it entirely in .fj (~200-500 lines)
- Test on Linux, macOS, Windows (via cross-compile)
- Document every language limitation encountered
- Fix at least 5 bugs found during development
- Benchmark vs equivalent Python/Rust tool
- Publish as example with documentation
- Get at least 1 external person to try building it
- Collect feedback on error messages, documentation, tooling
- Write a blog post about the experience

**2.2 Build a Real ML Pipeline in Fajar Lang (10 tasks)**
- Extend template_ml_pipeline.fj to train on real data (not synthetic)
- Load CSV data from file, preprocess, normalize
- Train a model that achieves meaningful accuracy
- Export model, deploy to Q6A for inference
- Measure end-to-end latency (data load → prediction)
- Compare accuracy with Python scikit-learn equivalent
- Document the ML workflow for new users
- Test on at least 3 different datasets
- Identify and fix pain points in the ML API
- Write tutorial: "Your First ML Model in Fajar Lang"

**2.3 Build a Real IoT Application (10 tasks)**
- Deploy template_iot_edge.fj to Q6A hardware
- Read real sensor data (GPIO, I2C temperature sensor)
- Run real inference on sensor data
- Transmit telemetry over real network (MQTT or HTTP)
- Measure power consumption and battery life impact
- Run for 24 hours without crash or memory leak
- Monitor memory usage over time (no growth = no leaks)
- Test recovery from network disconnection
- Test recovery from sensor failure
- Document deployment procedure for Q6A

### Phase 3: Ecosystem Completion (Priority: MEDIUM)

*Make it easy for others to use.*

**3.1 Package Registry (10 tasks)**
- Verify `fj new` creates valid project structure
- Verify `fj build` compiles a project with dependencies
- Verify `fj publish` to a local registry server
- Verify `fj install` downloads and installs a package
- Publish the 7 standard packages to the registry
- Verify dependency resolution with PubGrub solver
- Test version conflicts and resolution
- Test offline mode (install from cache)
- Test lock file reproducibility
- Document: "Publishing Your First Package"

**3.2 IDE Experience (10 tasks)**
- Verify VS Code extension installs and activates
- Verify syntax highlighting works for all token kinds
- Verify go-to-definition works for functions and structs
- Verify hover shows type information
- Verify completion suggests relevant items
- Verify rename works across a single file
- Verify diagnostics show on-type errors
- Test with a real 500+ line .fj project
- Fix any performance issues (LSP response < 100ms)
- Document: "Setting Up VS Code for Fajar Lang"

**3.3 CI/CD (10 tasks)**
- Verify GitHub Actions CI passes on push
- Test on Ubuntu 22.04 and 24.04
- Test on macOS (ARM64 via cross-compile)
- Test on Windows (via cross-compile)
- Verify `cargo clippy` passes in CI
- Verify `cargo test` passes in CI (including native feature)
- Verify `cargo fmt --check` passes in CI
- Add binary release job (upload artifacts)
- Test release binary runs on fresh system
- Document CI setup for contributors

### Phase 4: Documentation & Polish (Priority: MEDIUM)

**4.1 Documentation Accuracy (10 tasks)**
- Audit CLAUDE.md against actual codebase
- Audit FAJAR_LANG_SPEC.md against actual parser/interpreter
- Audit STDLIB_SPEC.md against actual builtins
- Audit ERROR_CODES.md against actual error types
- Update GAP_ANALYSIS_V2.md with current state
- Verify every code example in docs actually runs
- Remove or update stale version references
- Add "Getting Started in 5 Minutes" guide
- Add "Fajar Lang vs Rust" comparison guide
- Spell-check all documentation

**4.2 Performance Baseline (10 tasks)**
- Run criterion benchmarks and record baselines
- Benchmark: fibonacci(30) interpreter vs JIT vs AOT
- Benchmark: sort 10K elements
- Benchmark: tensor matmul 128x128
- Benchmark: compilation speed (1000-line program)
- Benchmark: LSP response time on 500-line file
- Benchmark: binary size (release build)
- Benchmark: startup time (REPL launch)
- Compare with Rust/Python/Node for same tasks
- Publish benchmark results in docs/BENCHMARKS.md

---

## Summary

```
Phase 1: Hardening              30 tasks    ~30 hours
Phase 2: Real-World Validation  30 tasks    ~30 hours
Phase 3: Ecosystem Completion   30 tasks    ~30 hours
Phase 4: Documentation & Polish 20 tasks    ~20 hours

Total: 110 tasks, ~110 hours
```

### Verification Rule

Every task in this plan has a **concrete verification method**:
- "Verify X works" → run a specific command, check output
- "Fix Y" → write a test that fails before, passes after
- "Document Z" → the document exists and is accurate
- "Build W" → the program compiles and runs

No task is marked [x] until the verification passes.

---

*This plan replaces the remaining V8 Options 2,3,5-10 with an honest,
verifiable path to production readiness.*
