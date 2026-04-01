# V15 "Delivery" — Task Tracking

> **Master Tracking Document** — All 120 tasks with checkboxes.
> **Rule:** `[ ]` = pending, `[w]` = in progress, `[x]` = done (verified by `fj run`), `[f]` = framework only
> **Verify:** `fj run <test_file.fj>` must succeed for [x]. `cargo test --lib` must pass.
> **Plan:** `docs/V15_DELIVERY_PLAN.md` — full context, rationale, and verification criteria.
> **Previous:** V14 "Infinity" — 205/500 [x], 160 [f], 135 [ ] (honest re-audit)

---

## Summary

| Option | Sprint | Tasks | [x] | [f] | [ ] |
|--------|--------|-------|-----|-----|-----|
| Bug Fixes | B1 Effect System | 10 | 0 | 0 | 10 |
| Bug Fixes | B2 ML Runtime | 10 | 0 | 0 | 10 |
| Bug Fixes | B3 Toolchain | 10 | 0 | 0 | 10 |
| Integration | I1 Real MNIST | 10 | 0 | 0 | 10 |
| Integration | I2 Real FFI | 10 | 0 | 0 | 10 |
| Integration | I3 Real CLI Tools | 10 | 0 | 0 | 10 |
| Hardening | P1 Fuzz Testing | 10 | 0 | 0 | 10 |
| Hardening | P2 Benchmarks | 10 | 0 | 0 | 10 |
| Hardening | P3 Security | 10 | 0 | 0 | 10 |
| Docs/Release | D1 Tutorials | 10 | 0 | 0 | 10 |
| Docs/Release | D2 Gap Analysis | 10 | 0 | 0 | 10 |
| Docs/Release | D3 Release v12.1.0 | 10 | 0 | 0 | 10 |
| **Total** | **12 sprints** | **120** | **0** | **0** | **120** |

---

# OPTION 1: BUG FIXES

## Sprint B1: Effect System Fixes

- [ ] B1.1 Fix effect multi-step continuation (body continues after resume)
- [ ] B1.2 Add resume stack tracking
- [ ] B1.3 Fix resume return value propagation
- [ ] B1.4 Handle multiple effect types in one handler
- [ ] B1.5 Add `resume()` as alias for `resume(null)`
- [ ] B1.6 Effect handler variable scoping fix
- [ ] B1.7 Nested handle expressions
- [ ] B1.8 Effect with typed return value
- [ ] B1.9 Effect operation arity check
- [ ] B1.10 End-to-end effect test suite (10 .fj programs)

## Sprint B2: ML Runtime Fixes

- [ ] B2.1 Register `tanh()` as builtin
- [ ] B2.2 Register `gelu()` as builtin
- [ ] B2.3 Register `leaky_relu()` as builtin
- [ ] B2.4 Fix `Dense.forward()` method dispatch
- [ ] B2.5 Fix `Conv2d.forward()` method dispatch
- [ ] B2.6 Register `flatten()` as builtin
- [ ] B2.7 Register `concat()` as builtin
- [ ] B2.8 Fix `cross_entropy()` as builtin
- [ ] B2.9 Add `accuracy()` metric builtin
- [ ] B2.10 End-to-end MNIST training test

## Sprint B3: Toolchain Fixes

- [ ] B3.1 Fix bindgen struct typedef output
- [ ] B3.2 Deepen context isolation in verify
- [ ] B3.3 Add `fj verify --strict` mode
- [ ] B3.4 Fix `fj build` error message without native feature
- [ ] B3.5 Add `fj run --check-only` flag
- [ ] B3.6 Fix LSP completion for effect keywords
- [ ] B3.7 Fix LSP semantic tokens for effects
- [ ] B3.8 Add `fj registry init` command
- [ ] B3.9 Add `fj publish --local` flag
- [ ] B3.10 End-to-end toolchain test

---

# OPTION 2: INTEGRATION COMPLETION

## Sprint I1: Real MNIST Training

- [ ] I1.1 Create MNIST data loader in .fj
- [ ] I1.2 Define CNN model in .fj
- [ ] I1.3 Training loop in .fj
- [ ] I1.4 Accuracy evaluation in .fj
- [ ] I1.5 Batch processing in .fj
- [ ] I1.6 Save/load model weights
- [ ] I1.7 Training progress output
- [ ] I1.8 Achieve 90%+ accuracy
- [ ] I1.9 GPU acceleration option
- [ ] I1.10 Tutorial document

## Sprint I2: Real FFI Integration

- [ ] I2.1 C math library FFI
- [ ] I2.2 Generate bindings for math.h
- [ ] I2.3 Use generated bindings
- [ ] I2.4 C string interop
- [ ] I2.5 C struct interop
- [ ] I2.6 Callback from C to Fajar
- [ ] I2.7 Error handling across FFI
- [ ] I2.8 Memory management across FFI
- [ ] I2.9 FFI performance benchmark
- [ ] I2.10 FFI integration test suite

## Sprint I3: Real CLI Tools in Fajar Lang

- [ ] I3.1 Word count tool
- [ ] I3.2 JSON pretty printer
- [ ] I3.3 CSV to JSON converter
- [ ] I3.4 File search tool (grep-like)
- [ ] I3.5 Calculator REPL
- [ ] I3.6 Fibonacci benchmark
- [ ] I3.7 Sorting algorithms
- [ ] I3.8 String manipulation tool
- [ ] I3.9 Matrix operations
- [ ] I3.10 CLI tools test suite

---

# OPTION 3: PRODUCTION HARDENING

## Sprint P1: Fuzz Testing

- [ ] P1.1 Lexer fuzz harness
- [ ] P1.2 Parser fuzz harness
- [ ] P1.3 Analyzer fuzz harness
- [ ] P1.4 Interpreter fuzz harness
- [ ] P1.5 Effect system fuzz
- [ ] P1.6 Tensor ops fuzz
- [ ] P1.7 FFI boundary fuzz
- [ ] P1.8 Format string fuzz
- [ ] P1.9 REPL fuzz
- [ ] P1.10 CI fuzz integration

## Sprint P2: Performance Benchmarks

- [ ] P2.1 fibonacci(30) benchmark
- [ ] P2.2 Sort 100K elements
- [ ] P2.3 Matrix multiply 256x256
- [ ] P2.4 String concat 10K
- [ ] P2.5 Lexer throughput
- [ ] P2.6 Parser throughput
- [ ] P2.7 Effect dispatch overhead
- [ ] P2.8 GPU vs CPU comparison
- [ ] P2.9 Startup time
- [ ] P2.10 Benchmark report

## Sprint P3: Security & Quality

- [ ] P3.1 cargo audit — 0 advisories
- [ ] P3.2 All unsafe blocks documented
- [ ] P3.3 No unwrap in src/
- [ ] P3.4 Recursion depth limits verified
- [ ] P3.5 Macro expansion limits verified
- [ ] P3.6 Input validation (path traversal)
- [ ] P3.7 Coverage report > 70%
- [ ] P3.8 Memory leak check (valgrind)
- [ ] P3.9 Update all dependencies
- [ ] P3.10 Security policy update

---

# OPTION 4: DOCUMENTATION & RELEASE

## Sprint D1: Examples & Tutorials

- [ ] D1.1 Effect system tutorial
- [ ] D1.2 ML training tutorial
- [ ] D1.3 FFI tutorial
- [ ] D1.4 GPU acceleration tutorial
- [ ] D1.5 CLI tool tutorial
- [ ] D1.6 Update examples/ directory (10 new .fj)
- [ ] D1.7 Update STDLIB_SPEC.md
- [ ] D1.8 Update ERROR_CODES.md
- [ ] D1.9 Update ARCHITECTURE.md
- [ ] D1.10 Update FAJAR_LANG_SPEC.md

## Sprint D2: Gap Analysis & Honesty

- [ ] D2.1 Update GAP_ANALYSIS_V2.md
- [ ] D2.2 Update CLAUDE.md
- [ ] D2.3 Update CHANGELOG.md
- [ ] D2.4 Verify all doc claims
- [ ] D2.5 Remove inflated statistics
- [ ] D2.6 Update website stats
- [ ] D2.7 Create KNOWN_LIMITATIONS.md
- [ ] D2.8 Update VS Code extension docs
- [ ] D2.9 Update book/ (mdBook)
- [ ] D2.10 Cross-reference audit

## Sprint D3: Release v12.1.0

- [ ] D3.1 Bump Cargo.toml to v12.1.0
- [ ] D3.2 Tag v12.1.0
- [ ] D3.3 GitHub Release v12.1.0
- [ ] D3.4 Update README badges
- [ ] D3.5 Update website
- [ ] D3.6 Run full test suite
- [ ] D3.7 Run full clippy
- [ ] D3.8 Run format check
- [ ] D3.9 Verify all examples run
- [ ] D3.10 Final honest assessment

---

*V15 Tasks — Version 1.0 | 120 tasks, all [ ] pending | 2026-04-01*
