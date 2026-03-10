# Planning — Fajar Lang Roadmap

> Update this file when: phase completes, sprint changes, blockers found, scope changes.

---

## Project Status

```
Current Phase:   ALL PHASES COMPLETE
Current Sprint:  —
Status:          Phase 1+2+3+4+Gap+5+6+7 COMPLETE
Started:         2026-03-05
```

---

## Master Roadmap

```
Phase 0: Project Scaffolding          ✅ COMPLETE
Phase 1: Core Language Foundation     ✅ COMPLETE (incl. Sprint 1.7 gap fixes)
Phase 2: Type System                  ✅ COMPLETE
Phase 3: OS Runtime                   ✅ COMPLETE (incl. Sprint 3.10 gap fixes)
Phase 4: ML/AI Runtime                ✅ COMPLETE (incl. Sprint 4.11 gap fixes)
Pre-5:   Gap Fixes (G.1-G.7)         ✅ COMPLETE
Phase 5: Tooling & Compiler Backend   ✅ COMPLETE (formatter, VM, LSP, package manager, LLVM/GPU assessed)
Phase 6: Standard Library             ✅ COMPLETE (string, collections, file I/O, metrics, HashMap)
Phase 7: Production Hardening         ✅ COMPLETE (proptest, criterion, security audit, examples)
```

> See `docs/IMPLEMENTATION_PLAN.md` for detailed step-by-step execution guide.

---

## Phase 0 — Project Scaffolding

**Goal:** Cargo project with all module stubs compiling cleanly.
**Duration:** 1 session. **Status:** ✅ COMPLETE (2026-03-05)

- [x] Cargo.toml with dependencies (NO logos — hand-written lexer)
- [x] Directory structure (src/lexer/, src/parser/, src/analyzer/, src/interpreter/, src/runtime/)
- [x] Placeholder module files (`mod.rs` for each)
- [x] src/lib.rs + src/main.rs
- [x] Example files (hello.fj, fibonacci.fj)
- [x] `cargo build` succeeds
- [x] `cargo test` runs (0 tests)
- [x] `cargo clippy -- -D warnings` clean

---

## Phase 1 — Core Language Foundation

**Goal:** Working tree-walking interpreter that can run basic Fajar Lang programs.
**Status:** 🟡 Core done (Sprint 1.1-1.6), gap fixes remaining (Sprint 1.7).

### Done (Sprint 1.1-1.6)
All milestones complete: Lexer (82 tests), AST (33 tests), Parser (94 tests), Environment & Values (33 tests), Interpreter (69 tests), CLI & REPL (3 integration tests). 314 tests, all passing.

- [x] `examples/hello.fj` runs and prints correctly
- [x] `examples/fibonacci.fj` computes fibonacci sequence
- [x] `examples/factorial.fj` computes factorials
- [x] REPL evaluates expressions interactively
- [x] CLI commands: run, repl, check, dump-tokens, dump-ast

### Sprint 1.7 — Gap Fixes ✅
- [x] Error codes aligned to ERROR_CODES.md (LE001-LE008, PE001-PE010)
- [x] Span::merge() utility added
- [x] Exit codes: 0=success, 1=runtime, 2=compile, 3=usage
- [x] Integration tests: 12 E2E tests in tests/eval_tests.rs
- [x] Interpreter API: eval_source(), call_fn() added
- [x] FjError: typed variants (Vec<LexError>, Vec<ParseError>, etc.)
- [x] Dependency cleanup: removed serde, serde_json, indexmap, criterion
- [x] CHANGELOG updated with all Phase 1+2 completions

### Phase 1 Exit Criteria
- [x] `examples/hello.fj` runs: prints "Hello from Fajar Lang!" ✅
- [x] `examples/fibonacci.fj` computes fibonacci correctly ✅
- [x] REPL evaluates expressions interactively ✅
- [ ] All error messages have source highlighting via miette ← **Sprint 2.9**
- [x] `cargo test` — 364 tests, all passing ✅
- [x] `cargo clippy -- -D warnings` — clean ✅
- [x] `cargo fmt -- --check` — clean ✅
- [x] CHANGELOG updated ✅

---

## Phase 2 — Type System

**Goal:** Static type checking catches errors before runtime.
**Status:** ✅ COMPLETE — All sprints (2.1-2.10) done.

### Done (Sprint 2.1-2.5)
- [x] Type enum (14 variants) + Type::is_compatible() with Unknown/Never handling
- [x] SemanticError enum (9 error codes: SE001-SE008, SE012)
- [x] SymbolTable with lexical scoping (stack of scopes)
- [x] Two-pass TypeChecker: register items then check types
- [x] Check all 24 expression types + all statement/item types
- [x] 11 builtin function signatures registered
- [x] Wired into CLI `check` and `run` commands

### Remaining (Sprint 2.7-2.10) — Gap Fixes
- [x] **Sprint 2.6**: Distinct integer/float types (i8≠i16≠i32≠i64, f32≠f64) + IntLiteral/FloatLiteral inference ✅
- [x] **Sprint 2.7**: Missing SE codes (SE009 UnusedVariable, SE010 UnreachableCode, SE011 NonExhaustiveMatch) ✅
- [x] **Sprint 2.8**: ScopeKind enum + break/continue/return context validation ✅
- [x] **Sprint 2.9**: miette integration — beautiful error display with source highlighting ✅
- [x] **Sprint 2.10**: Phase 2 exit gate — verify all gaps closed ✅

### Deferred to Later Phases
- [~] Generic type parameters — monomorphization → Phase 5
- [~] Tensor type + shape checking (TE001-TE008) → Phase 4
- [~] Context annotation enforcement (@kernel/@device) → Phase 3/4
- [~] borrow_lite.rs — ownership/move semantics (ME001-ME008) → Phase 3
- [~] Type Inference (Hindley-Milner Lite) → Phase 5
- [~] TypedProgram output → Phase 5 (compiler backend)

### Phase 2 Exit Criteria ✅
- [x] Type errors caught at compile time
- [x] `let x: i32 = 42; let y: i64 = x` → COMPILE ERROR (SE004)
- [x] All 12 SE codes (SE001-SE012) implemented
- [x] Error output uses miette with source highlighting
- [x] break/continue validated inside loop scope only
- [x] return validated inside function scope only

---

## Phase 3 — OS Runtime ✅ COMPLETE

**Goal:** OS-level programming capabilities.
**Duration:** 8–10 weeks. **Status:** ✅ COMPLETE (Sprint 3.1-3.10)

### Done (Sprint 3.1-3.9)
- [x] MemoryManager (heap simulation, bump allocator)
- [x] VirtAddr, PhysAddr distinct newtype structs
- [x] Virtual memory mapping (page tables)
- [x] PageFlags (READ, WRITE, EXEC, USER)
- [x] IRQ table and handler dispatch
- [x] Syscall table definition and dispatch
- [x] Pointer(u64) runtime value type
- [x] Port I/O (simulated x86 ports)
- [x] @kernel/@device annotation enforcement (KE003, DE001, DE002)
- [x] 16 OS builtins wired into interpreter
- [x] examples/memory_map.fj runs correctly
- [x] 10 OS integration tests + 7 context enforcement tests

### Sprint 3.10 — Gap Fixes ✅
- [x] KE001 (HeapAllocInKernel) enforcement — heap_builtins set (push, pop, to_string) checked in @kernel
- [x] KE002 (TensorInKernel) enforcement — tensor_builtins set ready (empty until Phase 4)
- [x] Syscall builtins (syscall_define, syscall_dispatch) wired into interpreter + type checker
- [x] stdlib/os.fj + src/stdlib/os.rs + src/stdlib/mod.rs created
- [x] Integration tests: kernel init sequence, IRQ lifecycle, syscall from .fj
- [x] Final exit gate: 496 tests, clippy clean, fmt clean

---

## Phase 4 — ML/AI Runtime ✅ COMPLETE

**Goal:** Native tensor operations and automatic differentiation.
**Status:** ✅ COMPLETE (2026-03-05)

- [x] TensorValue struct (data, shape, grad tracking, TensorId for autograd)
- [x] Basic ops: add, sub, mul, div, neg, matmul, transpose, flatten, sum, mean
- [x] Activation functions: relu, sigmoid, tanh, softmax, gelu, leaky_relu
- [x] Computation graph (dynamic, tape-based reverse-mode autograd)
- [x] Backward pass: tracked ops for add/sub/mul/div/matmul/relu/sigmoid/tanh/sum/mean
- [x] Loss functions: mse_loss, cross_entropy, bce_loss (with autograd support)
- [x] Optimizers: SGD (with momentum), Adam (with bias correction)
- [x] Layers: Dense (Xavier init), Dropout (inverted scaling), BatchNorm
- [x] @device annotation dispatch: 27 tensor builtins wired to interpreter + type checker
- [x] KE002 enforcement: all 27 tensor builtins in tensor_builtins set
- [x] ML stdlib: stdlib/nn.fj + src/stdlib/nn.rs
- [x] Integration test: MNIST forward pass (784→128→10, relu + softmax)
- [x] Integration test: gradient flow (relu backward, mul chain)
- [x] Gradient correctness test (numerical vs analytical, all ops)
- [x] Gap fixes: squeeze/unsqueeze, max/min/argmax, arange/linspace, xavier, l1_loss, flatten
- [x] examples/mnist_forward.fj working example
- [x] 660 total tests (598 unit + 12 eval + 31 ml + 16 os + 3 doc)

---

## Pre-Phase 5 — Gap Fixes (G.1-G.7) ← CURRENT

**Goal:** Fix language features documented in spec but not yet implemented.
**Duration:** 13-21 sessions. **Status:** ⏳ NOT STARTED

### Sprint G.1 — impl Blocks & Method Dispatch ✅
- [x] Interpreter: evaluate ImplBlock, register methods per type
- [x] Method dispatch: `obj.method()` looks up impl methods
- [x] Type checker: register impl methods in SymbolTable
- [x] Tests: struct + impl, method calls, field access via self

### Sprint G.2 — Option/Result Types & ? Operator ✅
- [x] Register Some/None/Ok/Err as built-in enum constructors
- [x] Implement ? operator: unwrap Ok/Some, early-return Err/None
- [x] Utility methods: .unwrap(), .unwrap_or(), .is_some(), .is_ok()
- [x] Tests: ? propagation chain, match on Option/Result

### Sprint G.3 — Module System (use/mod) ✅
- [x] Inline modules: `mod math { fn square() {} }`
- [x] Use statements: `use math::square`, `use math::*`, `use math::{a, b}`
- [x] Nested modules: `outer::inner::symbol`
- [x] Type checker: module-scoped name registration + use import resolution
- [x] Tests: 7 integration tests (qualified access, simple/glob/group import, struct, const, nesting)
- [~] Visibility (pub/private): deferred — all items public by default
- [~] File-based modules: deferred to Phase 6

### Sprint G.4 — Cast Expression & Minor Gaps ✅
- [x] `as` cast: int↔float, widening/narrowing, bool↔int
- [x] Named arguments: `greet(times: 2, name: "hello")`
- [x] Tests: 7 integration tests
- [~] @device parameter parsing: deferred (low priority)

### Sprint G.5 — Missing Global Builtins & Math Functions ✅
- [x] Error builtins: panic, todo, dbg, eprint, eprintln
- [x] Math: abs, sqrt, pow, log, log2, log10, sin, cos, tan, floor, ceil, round, clamp, min, max
- [x] Constants: PI, E
- [x] Tests: 12 integration tests
- [~] read_line(): deferred (requires stdin)

### Sprint G.6 — NN Runtime Builtin Exposure ✅
- [x] Autograd: tensor_backward, tensor_grad, tensor_requires_grad, tensor_set_requires_grad
- [x] Optimizers: optimizer_sgd, optimizer_adam, optimizer_step, optimizer_zero_grad
- [x] Layers: layer_dense, layer_forward, layer_params
- [x] Value::Optimizer (SGD | Adam), Value::Layer (Box<Dense>)
- [x] Tests: 8 integration tests

### Sprint G.7 — Parser & Analyzer Cleanup ✅
- [x] `loop` expression: lexer keyword + AST + parser + interpreter + type checker
- [x] Dead code cleanup: no warnings, clippy clean
- [x] Created stdlib/core.fj with utility functions
- [x] Tests: 3 integration tests (break, continue, value)

> **Full details:** `docs/PHASE5_PLAN.md`

---

## Phase 5 — Tooling & Compiler Backend

**Goal:** Developer experience + native compilation.
**Duration:** 18-25 sessions. **Status:** ⏳ IN PROGRESS (Sprint 5.1+5.2 done)

- [x] Sprint 5.1: Code formatter (`fj fmt`) ✅
- [x] Sprint 5.2: Bytecode VM (`fj run --vm`) ✅ — 45 opcodes, stack-based, 15 VM tests
- [x] Sprint 5.2.1: VM Gap Fixes ✅ — 25 VM tests, closures, short-circuit, structs/enums/arrays
- [x] Sprint 5.3: LSP server + VS Code extension ✅ — diagnostics, hover, completions, go-to-def, 14 tests
- [x] Sprint 5.4: Package manager ✅ — fj.toml, `fj new`, `fj build`, project-mode `fj run`
- [x] Sprint 5.5: LLVM Backend ✅ — assessed, DEFERRED to Phase 7 (llvm-dev not installed, tree-walker+VM sufficient)
- [x] Sprint 5.6: GPU Backend ✅ — researched, DEFERRED to Phase 7 (wgpu best option, CPU ndarray sufficient)

> **Full details:** `docs/PHASE5_PLAN.md`

---

## Phase 6 — Standard Library ✅ COMPLETE

**Goal:** Complete standard library per STDLIB_SPEC.md.
**Status:** ✅ COMPLETE (2026-03-05)

- [x] Sprint 6.1: std::string & std::convert — 15 string methods + parse_int/parse_float + as cast + to_string
- [x] Sprint 6.2: std::collections — HashMap (8 builtins + 7 methods + for-in iteration) + array methods (join, reverse, contains)
- [x] Sprint 6.3: std::io & File I/O — read_file, write_file, append_file, file_exists (all Result-based)
- [x] Sprint 6.4: OS & NN stdlib completion — memory_copy/set/compare + metrics module (accuracy, precision, recall, f1_score)
- [~] Advanced NN (Conv2d, Attention, DataLoader) — deferred to Phase 7

> **Full details:** `docs/PHASE5_PLAN.md` Part 3

---

## Phase 7 — Production Hardening ✅ COMPLETE

**Goal:** Production-ready quality and documentation.
**Status:** ✅ COMPLETE (2026-03-05)

- [x] Sprint 7.1: Property testing — 15 proptest invariants (lexer, parser, interpreter, value)
- [x] Sprint 7.2: Benchmarks — criterion suite (5 benchmarks: lex, parse, fib, loop, string)
- [x] Sprint 7.3: Security audit — zero unsafe blocks, 6 security tests, context isolation verified
- [x] Sprint 7.5: Examples — 3 new examples (collections, file_io, ml_metrics), all builtins in type checker
- [~] Sprint 7.4: mdBook documentation site — deferred to v0.2
- [~] Sprint 7.6: LLVM & GPU — deferred to v0.2

> **Full details:** `docs/PHASE5_PLAN.md` Part 4

---

## Metrics & Progress

| Phase | Tasks Total | Done | Coverage | Status |
|-------|-------------|------|----------|--------|
| 1 | 53 | 53 | 100% | ✅ Complete |
| 2 | 39 | 39 | 100% | ✅ Complete |
| 3 | 35 | 35 | 100% | ✅ Complete |
| 4 | 42 | 42 | 100% | ✅ Complete |
| Gap | 45 | 45 | 100% | ✅ Complete |
| 5 | 35 | 35 | 100% | ✅ Complete |
| 6 | 20 | 17 | 85% | ✅ Complete (3 advanced NN deferred) |
| 7 | 30 | 22 | 73% | ✅ Complete (8 deferred to v0.2) |

## Blockers & Decisions Log

| Date | Item | Decision / Resolution |
|------|------|----------------------|
| 2026-03-05 | Tree-walking vs bytecode VM | Phase 1-4: tree-walking; Phase 5.2: stack-based bytecode VM |
| 2026-03-05 | GPU backend | Phase 4: CPU only; Phase 5.6: wgpu research; Phase 7.6: full impl |
| 2026-03-05 | Lifetime annotations | Omit from language v0.1, add in v0.3 |
| 2026-03-05 | Async/await | Deferred indefinitely; not needed for core use cases |
| 2026-03-05 | LSP framework | tower-lsp (standard Rust LSP framework) |
| 2026-03-05 | LLVM bindings | inkwell crate (supports LLVM 11-21) |
| 2026-03-05 | Formatter approach | AST-based + comment reattachment |
| 2026-03-05 | Collections strategy | Builtin HashMap<String,Value> first; full generics later |
| 2026-03-05 | Gap analysis | Expanded from 4 sprints (G.1-G.4) to 7 (G.1-G.7) after deep audit |

---

*Last Updated: 2026-03-05 — Phase 4 COMPLETE. 660 tests. Deep gap analysis revealed 7 sprints of fixes needed (G.1-G.7). Full roadmap through Phase 7. See docs/PHASE5_PLAN.md for comprehensive plan.*
