# Implementation Plan — Fajar Lang v1.0

> Target: "Bahasa terbaik untuk embedded ML + OS integration"
> Timeline: 6 bulan (26 sprints, 1 sprint = ~1 minggu)
> Start: March 2026

---

## Overview

```
Month 1: FOUNDATION    — Native codegen + analyzer integration          ✅ COMPLETE
Month 2: TYPE SYSTEM   — Generics + Traits + FFI                       ✅ COMPLETE
Month 3: SAFETY        — Ownership + borrow checker                    ✅ COMPLETE (S11 deferred)
Month 4: ML RUNTIME    — Autograd + advanced layers + quantization     ✅ COMPLETE
Month 5: EMBEDDED      — Cross-compilation + no_std + embedded targets ✅ COMPLETE
Month 6: PRODUCTION    — Self-hosting + docs + release                 ✅ COMPLETE (S23 deferred)

Start:   27K LOC, 866 tests, tree-walking interpreter
Result:  ~45K LOC, 1,563 tests, native compiler (Cranelift JIT+AOT), 15 examples
```

---

## Month 1 — Foundation (Sprint 1-4)

### Sprint 1: Pipeline Integration & Infrastructure

**Goal:** Analyzer runs in pipeline. CI/CD. README.

```
S1.1  Integrate analyzer into eval_source() pipeline
      - analyzer::analyze() called before eval_program()
      - Semantic errors caught at compile time
      - Fix any tests broken by stricter checking
      Files: src/interpreter/eval.rs, tests/

S1.2  GitHub Actions CI/CD
      - .github/workflows/ci.yml
      - Matrix: ubuntu + macos, stable + nightly
      - cargo fmt, clippy, test, bench --no-run
      Files: .github/workflows/ci.yml

S1.3  README.md + Installation guide
      - Project description, features, quickstart
      - cargo install instructions
      - Example programs
      Files: README.md

S1.4  File-based modules (mod name; → loads name.fj)
      - Parser: `mod name;` resolves to file
      - Module search path: current dir, then stdlib/
      - Integration test: multi-file project
      Files: src/parser/mod.rs, src/interpreter/eval.rs
```

### Sprint 2: Cranelift Setup & Basic Codegen

**Goal:** Compile `fn add(a: i64, b: i64) -> i64 { a + b }` to native.

```
S2.1  Add cranelift dependencies
      - cranelift-codegen, cranelift-frontend, cranelift-module
      - cranelift-jit for development, cranelift-object for release
      - Feature-gate: [features] native = ["cranelift-*"]
      Files: Cargo.toml

S2.2  Codegen module structure
      - src/codegen/cranelift.rs — CraneliftCompiler struct
      - src/codegen/types.rs — Type lowering (Fajar Type → Cranelift types)
      - src/codegen/abi.rs — Calling convention, value representation
      - src/codegen/mod.rs — pub fn compile(program) → CompiledModule
      Files: src/codegen/*

S2.3  Integer arithmetic codegen
      - Compile: i64 add, sub, mul, div, mod, neg
      - Compile: comparison ops (eq, ne, lt, gt, le, ge)
      - Compile: bool ops (and, or, not)
      - Test: assert native(1+2) == 3
      Files: src/codegen/cranelift.rs, tests/codegen_tests.rs

S2.4  Function definition & calls
      - Compile fn definitions → Cranelift functions
      - Compile fn calls with argument passing
      - Compile return statements
      - Test: fibonacci(20) native vs interpreter
      Files: src/codegen/cranelift.rs
```

### Sprint 3: Control Flow & Variables

**Goal:** Compile if/while/for to native code.

```
S3.1  Local variables
      - Stack slot allocation for locals
      - Variable declaration → stack store
      - Variable read → stack load
      - Mutable variables → reassignment
      Files: src/codegen/cranelift.rs

S3.2  If/else expressions
      - Branch to true_block / false_block
      - Phi nodes for if-expression values
      - Nested if/else chains
      Files: src/codegen/cranelift.rs

S3.3  While loops
      - Loop header block, body block, exit block
      - Break → jump to exit
      - Continue → jump to header
      Files: src/codegen/cranelift.rs

S3.4  For loops & ranges
      - Range iterator → counter variable
      - For-in over arrays → index + bounds check
      - Loop unrolling hint for small known ranges
      Files: src/codegen/cranelift.rs
```

### Sprint 4: Strings, Arrays & CLI Integration

**Goal:** `fj build` produces native binary.

```
S4.1  String representation in native code
      - String as struct { ptr, len, cap }
      - String literals → static data section
      - String concatenation → runtime alloc + memcpy
      - print/println → call to C printf or custom runtime
      Files: src/codegen/cranelift.rs, src/codegen/runtime.rs

S4.2  Array representation
      - Fixed arrays → stack allocation
      - Dynamic arrays → heap allocation (runtime)
      - Bounds checking → trap on out-of-bounds
      Files: src/codegen/cranelift.rs

S4.3  CLI: `fj build` → native binary
      - `fj build` reads fj.toml, compiles to object file
      - Link with system linker (cc)
      - `fj run --native file.fj` for JIT compilation
      - Benchmark: native vs tree-walk vs VM
      Files: src/main.rs, src/codegen/mod.rs

S4.4  Runtime library (libfj_rt)
      - Memory allocator (malloc/free wrapper)
      - Print functions (stdout, stderr)
      - Panic handler
      - GC stubs (for future reference counting)
      Files: src/codegen/runtime.rs, runtime/libfj_rt.c
```

---

## Month 2 — Type System (Sprint 5-8)

### Sprint 5: Generics — Monomorphization

```
S5.1  Generic function parsing (already done, verify)
      - fn max<T>(a: T, b: T) -> T
      - Verify AST stores GenericParam correctly
      Files: src/parser/ast.rs, src/parser/mod.rs

S5.2  Type inference at call site
      - Infer T from actual argument types
      - Unification algorithm for type variables
      - Error: "cannot infer type T" when ambiguous
      Files: src/analyzer/type_check.rs, src/analyzer/inference.rs (NEW)

S5.3  Monomorphization in interpreter
      - At call site: substitute type params → specialized body
      - Cache specialized functions: (name, [types]) → body
      - Test: max(1, 2), max(1.0, 2.0) both work
      Files: src/interpreter/eval.rs

S5.4  Monomorphization in codegen
      - Generate separate native function per specialization
      - max_i64, max_f64 etc.
      - Dead code elimination: only generate used specializations
      Files: src/codegen/cranelift.rs
```

### Sprint 6: Trait System

```
S6.1  Trait definition evaluation
      - Store trait method signatures in symbol table
      - Validate: no duplicate method names
      - Validate: methods have proper self parameter
      Files: src/analyzer/type_check.rs, src/interpreter/eval.rs

S6.2  Impl trait for type
      - Verify all trait methods implemented
      - Verify method signatures match trait
      - Error SE014: missing trait method implementation
      - Error SE015: method signature mismatch
      Files: src/analyzer/type_check.rs

S6.3  Trait bounds on generics
      - `fn sort<T: Ord>(arr: [T])` — verify T implements Ord
      - Error SE016: type does not implement trait
      - Static dispatch (monomorphized — no vtables)
      Files: src/analyzer/type_check.rs, src/interpreter/eval.rs

S6.4  Built-in traits
      - Copy — types that are copied, not moved
      - Display — types that can be printed
      - Ord — types that can be compared
      - Default — types with default values
      - From/Into — type conversion
      Files: src/analyzer/traits.rs (NEW), src/interpreter/eval.rs
```

### Sprint 7: FFI — C Interop

```
S7.1  @ffi("C") extern declarations
      - Parse: extern fn name(params) -> ret
      - Store in symbol table as foreign function
      - Type checking: only C-compatible types (i32, f64, *T, etc.)
      Files: src/parser/mod.rs, src/analyzer/type_check.rs

S7.2  Dynamic library loading
      - libloading for .so/.dylib/.dll
      - Symbol lookup by name
      - Type marshaling: Value ↔ C types
      Files: src/interpreter/ffi.rs (NEW)

S7.3  Native FFI in codegen
      - Cranelift: declare imported function
      - Call convention: C ABI
      - Pointer marshaling (VirtAddr → raw pointer)
      Files: src/codegen/cranelift.rs

S7.4  libc bindings
      - Provide: malloc, free, printf, memcpy, memset
      - Used by runtime library
      - Test: call printf from Fajar Lang
      Files: stdlib/ffi/libc.fj (NEW)
```

### Sprint 8: Type System Polish

```
S8.1  Type inference improvements
      - Bidirectional type inference
      - Let-binding inference from right-hand side
      - Closure parameter inference from usage
      Files: src/analyzer/type_check.rs

S8.2  Enum with associated data types
      - Option<T> as proper generic enum
      - Result<T, E> as proper generic enum
      - Pattern matching with type-checked destructuring
      Files: src/analyzer/type_check.rs, src/interpreter/eval.rs

S8.3  Type aliases
      - type Meters = f64
      - type Matrix = Tensor<f64>
      - Transparent aliases (no runtime cost)
      Files: src/parser/mod.rs, src/analyzer/type_check.rs

S8.4  Never type (!) and exhaustiveness
      - Functions that never return → type !
      - Match exhaustiveness checking (proper algorithm)
      - Unreachable code after diverging expressions
      Files: src/analyzer/type_check.rs
```

---

## Month 3 — Safety (Sprint 9-13)

### Sprint 9: Move Semantics

```
S9.1  Copy vs Move classification
      - Primitives (i64, f64, bool, char) → Copy
      - Compound types (String, Array, Struct) → Move
      - Annotate types in symbol table
      Files: src/analyzer/borrow_lite.rs

S9.2  Move tracking
      - Track variable states: Owned | Moved | Partially_Moved
      - Assignment: if non-Copy, mark source as Moved
      - Function call: non-Copy args are moved
      - Error ME001: use after move
      Files: src/analyzer/borrow_lite.rs

S9.3  Drop insertion
      - At scope exit, drop all Owned variables
      - Drop order: reverse declaration order
      - For codegen: emit destructor calls
      Files: src/analyzer/borrow_lite.rs, src/codegen/cranelift.rs

S9.4  Move semantics in pattern matching
      - Match arms may move the subject
      - Partial moves in struct destructuring
      - Error ME002: partial move
      Files: src/analyzer/borrow_lite.rs
```

### Sprint 10: Borrow Checker

```
S10.1  Immutable borrows (&T)
       - Track borrow count per variable
       - Multiple immutable borrows OK
       - Error ME003: cannot move while borrowed
       Files: src/analyzer/borrow_lite.rs

S10.2  Mutable borrows (&mut T)
       - Exclusive: no other borrows while &mut active
       - Error ME004: cannot borrow mutably while immutably borrowed
       - Error ME005: cannot borrow immutably while mutably borrowed
       Files: src/analyzer/borrow_lite.rs

S10.3  Borrow scoping
       - Borrows last until last use (NLL-like)
       - Scope analysis: find last use of each borrow
       - Allow reborrow after last use
       Files: src/analyzer/borrow_lite.rs

S10.4  Context boundary checking
       - Error ME006: cannot borrow across @kernel/@device boundary
       - @kernel function cannot hold borrow to @device data
       - Validates the "domain isolation" property
       Files: src/analyzer/borrow_lite.rs, src/analyzer/type_check.rs
```

### Sprint 11: Tensor Shape Safety

```
S11.1  Static shape tracking
       - Tensor<f32, [M, N]> carries shape in type
       - Shape variables (M, N) resolved at compile time where possible
       - Fallback: runtime shape checking
       Files: src/analyzer/type_check.rs

S11.2  Matmul shape checking
       - [M, K] @ [K, N] → [M, N] — K must match
       - Error TE002: matmul dimension mismatch
       - Compile-time when shapes are literals
       Files: src/analyzer/type_check.rs

S11.3  Reshape validation
       - reshape(Tensor<[M, N]>, [P, Q]) requires M*N == P*Q
       - Compile-time when all dimensions known
       - Runtime check when dimensions are variables
       Files: src/analyzer/type_check.rs

S11.4  Shape inference through operations
       - add/sub/mul: broadcast rules
       - transpose: swap last two dims
       - flatten: [M, N, ...] → [M*N*...]
       - conv2d: output shape from kernel/stride/padding
       Files: src/analyzer/type_check.rs
```

### Sprint 12: Memory Safety Polish

```
S12.1  Integer overflow checking
       - Debug mode: panic on overflow
       - Release mode: wrapping behavior (documented)
       - Explicit: wrapping_add, checked_add, saturating_add
       Files: src/interpreter/eval.rs, src/codegen/cranelift.rs

S12.2  Null safety enforcement
       - No null values outside Option<T>
       - Option<T> must be matched or unwrapped
       - ? operator only on Option and Result
       Files: src/analyzer/type_check.rs

S12.3  Array bounds checking
       - Compile-time: known index vs known length
       - Runtime: trap with helpful error message
       - Unsafe: unchecked_index() in @unsafe only
       Files: src/codegen/cranelift.rs

S12.4  Stack overflow protection
       - Recursion depth limit (configurable, default 1024)
       - Stack size estimation per function
       - Warning for deeply recursive functions
       Files: src/codegen/cranelift.rs, src/analyzer/type_check.rs
```

### Sprint 13: Safety Testing & Audit

```
S13.1  Comprehensive safety test suite
       - 50+ tests for move semantics
       - 50+ tests for borrow checking
       - 20+ tests for tensor shape safety
       - 20+ tests for context isolation
       Files: tests/safety_tests.rs (NEW)

S13.2  Fuzzing setup
       - cargo-fuzz targets: lexer, parser, analyzer
       - Run fuzzer for 1 hour, fix all crashes
       - Add crash cases as regression tests
       Files: fuzz/

S13.3  Safety audit checklist
       - Review all unsafe blocks
       - Review all FFI boundary crossings
       - Review all runtime error paths
       - Document known limitations
       Files: docs/SAFETY_AUDIT.md (NEW)
```

---

## Month 4 — ML Runtime (Sprint 14-17)

### Sprint 14: Autograd — Full Implementation

```
S14.1  Computation graph (Tape)
       - Record all tensor operations with inputs/outputs
       - Track requires_grad flag per tensor
       - Detach: create non-tracked copy
       Files: src/runtime/ml/autograd.rs

S14.2  Backward pass
       - Reverse-mode automatic differentiation
       - Gradient rules: add, mul, matmul, relu, sigmoid, tanh, softmax
       - Gradient accumulation for shared tensors
       Files: src/runtime/ml/autograd.rs

S14.3  Gradient correctness verification
       - Numerical gradient checking (finite differences)
       - Assert: |analytical - numerical| < epsilon for all ops
       - Test with known-correct gradients
       Files: tests/autograd_tests.rs (NEW)

S14.4  No-grad context
       - no_grad { } block disables gradient tracking
       - Used for inference, reduces memory usage
       - Nested no_grad contexts supported
       Files: src/runtime/ml/autograd.rs, src/interpreter/eval.rs
```

### Sprint 15: Neural Network Layers

```
S15.1  Conv2d layer
       - Kernel, stride, padding parameters
       - Forward: im2col + matmul approach
       - Backward: gradient through convolution
       - Xavier initialization
       Files: src/runtime/ml/layers.rs

S15.2  Attention mechanism
       - Scaled dot-product attention
       - Multi-head attention (num_heads parameter)
       - Forward + backward with gradient support
       Files: src/runtime/ml/layers.rs

S15.3  Normalization layers
       - BatchNorm (running mean/var)
       - LayerNorm
       - Forward + backward
       Files: src/runtime/ml/layers.rs

S15.4  Dropout & Embedding
       - Dropout with inverted scaling
       - Embedding lookup table
       - Both with gradient support
       Files: src/runtime/ml/layers.rs
```

### Sprint 16: Data Loading & Training

```
S16.1  DataLoader
       - Load CSV data
       - Batch iteration
       - Shuffle support
       Files: src/runtime/ml/data.rs (NEW)

S16.2  Training loop builtins
       - Optimizer step (actually update parameters)
       - Learning rate scheduling
       - Gradient clipping
       Files: src/runtime/ml/optim.rs

S16.3  Model serialization
       - Save model weights to binary file
       - Load model weights from file
       - Version-tagged format
       Files: src/runtime/ml/model.rs (NEW)

S16.4  MNIST end-to-end example
       - Load MNIST data
       - Train 784→128→10 network
       - Report accuracy per epoch
       - Export trained model
       Files: examples/mnist_train.fj (NEW)
```

### Sprint 17: Quantization & Embedded Inference

```
S17.1  INT8 quantization
       - Post-training quantization (PTQ)
       - Per-tensor scale factors
       - INT8 matmul using i32 accumulation
       Files: src/runtime/ml/quantize.rs (NEW)

S17.2  Model export for embedded
       - Serialize quantized weights
       - Generate C header with model structure
       - No-alloc inference function
       Files: src/runtime/ml/export.rs (NEW)

S17.3  Fixed-point arithmetic
       - Q8.8, Q16.16 fixed-point types
       - All operations without floating point
       - For MCU targets without FPU
       Files: src/runtime/ml/fixed_point.rs (NEW)

S17.4  Embedded inference example
       - Load pre-trained model
       - Run inference with no heap allocation
       - @kernel context (no floats, no heap)
       Files: examples/embedded_inference.fj (NEW)
```

---

## Month 5 — Embedded (Sprint 18-22)

### Sprint 18: Cross-Compilation

```
S18.1  Target triple support
       - Parse target triple from CLI: fj build --target aarch64-unknown-none
       - Configure Cranelift for target ISA
       - ABI selection per target
       Files: src/codegen/target.rs (NEW), src/main.rs

S18.2  ARM64 backend
       - aarch64 instruction selection
       - ARM calling convention
       - Test: compile fibonacci for ARM64
       Files: src/codegen/cranelift.rs

S18.3  RISC-V backend
       - riscv64 instruction selection
       - RISC-V calling convention
       - Test: compile fibonacci for RISC-V
       Files: src/codegen/cranelift.rs

S18.4  Linker integration
       - Use system linker (ld/lld) for AOT
       - Generate proper ELF/Mach-O sections
       - Bare-metal linker scripts
       Files: src/codegen/linker.rs (NEW)
```

### Sprint 19: no_std & Bare Metal

```
S19.1  no_std runtime
       - Compile without std library
       - Static memory allocation only
       - No heap, no filesystem, no stdio
       Files: src/codegen/nostd.rs (NEW)

S19.2  @kernel function compilation
       - No floating point instructions (soft-float)
       - No heap allocation (stack only)
       - Interrupt-safe (no locks, no allocation)
       Files: src/codegen/cranelift.rs

S19.3  Stack-only tensor operations
       - Fixed-size tensors on stack
       - Shape known at compile time
       - No dynamic memory
       Files: src/runtime/ml/stack_tensor.rs (NEW)

S19.4  Bare-metal hello world
       - Write to UART (memory-mapped I/O)
       - No OS, no libc
       - Test: QEMU aarch64 or riscv64
       Files: examples/bare_metal.fj (NEW)
```

### Sprint 20: Hardware Abstraction Layer

```
S20.1  HAL trait definitions
       - trait Gpio { fn set_high(), fn set_low(), fn read() -> bool }
       - trait Uart { fn write(data: &[u8]), fn read() -> u8 }
       - trait Spi { fn transfer(data: &[u8]) -> [u8] }
       - trait I2c { fn write(addr, data), fn read(addr, len) -> [u8] }
       Files: stdlib/hal.fj (NEW)

S20.2  Interrupt handling
       - IRQ handler registration from Fajar Lang
       - Priority levels
       - Nested interrupt support
       - Critical section macros
       Files: stdlib/hal.fj, src/runtime/os/irq.rs

S20.3  DMA support
       - DMA transfer descriptors
       - Memory-to-peripheral, peripheral-to-memory
       - Completion callbacks via IRQ
       Files: src/runtime/os/dma.rs (NEW)

S20.4  Timer/PWM
       - Hardware timer configuration
       - PWM output for motor control
       - Input capture for sensor reading
       Files: stdlib/hal.fj
```

### Sprint 21: Sensor → ML → Actuator Pipeline

```
S21.1  Sensor driver abstraction
       - trait Sensor { fn read() -> [f32; N] }
       - IMU (accelerometer + gyroscope)
       - Temperature, pressure, humidity
       Files: stdlib/drivers.fj (NEW)

S21.2  Real-time inference pipeline
       - Read sensor → preprocess → infer → postprocess → actuate
       - Fixed timing constraints
       - No heap allocation in hot path
       Files: examples/realtime_pipeline.fj (NEW)

S21.3  Actuator control
       - trait Actuator { fn set(value: f32) }
       - Motor control (PWM-based)
       - Servo control
       - LED/display output
       Files: stdlib/drivers.fj

S21.4  Complete drone example
       - IMU sensor reading (@kernel)
       - Attitude estimation (Kalman filter, @device)
       - PID controller output (@kernel)
       - Bridge pattern (@safe)
       Files: examples/drone_control.fj (NEW)
```

### Sprint 22: Embedded Testing

```
S22.1  QEMU-based testing
       - Run tests on QEMU aarch64
       - Automated CI with QEMU
       - Test: all examples run on QEMU
       Files: .github/workflows/embedded.yml

S22.2  Hardware-in-loop testing framework
       - Test framework for embedded targets
       - UART-based test result reporting
       - Timeout detection
       Files: tests/embedded/ (NEW)

S22.3  Memory usage analysis
       - Stack usage per function (compile-time)
       - Static memory map report
       - Warning for excessive stack depth
       Files: src/codegen/analysis.rs (NEW)

S22.4  Performance benchmarks on target
       - Inference latency on ARM64
       - Power consumption estimation
       - Memory footprint analysis
       Files: benches/embedded_bench.rs (NEW)
```

---

## Month 6 — Production (Sprint 23-26)

### Sprint 23: Self-Hosting Preparation

```
S23.1  Fajar Lang lexer in Fajar Lang
       - Port src/lexer/ to Fajar Lang
       - Same token types, same algorithms
       - Test: self-lexer produces same output as Rust lexer
       Files: self/lexer.fj (NEW)

S23.2  Fajar Lang parser in Fajar Lang
       - Port src/parser/ to Fajar Lang
       - Same AST types, Pratt parser
       - Test: self-parser produces same AST
       Files: self/parser.fj (NEW)

S23.3  Bootstrap test
       - Compile self-hosted compiler with Rust compiler
       - Use self-hosted compiler to compile itself
       - Verify: output matches
       Files: self/bootstrap.sh (NEW)
```

### Sprint 24: Documentation & Tutorials

```
S24.1  mdBook documentation site
       - Getting Started guide
       - Language tour (with runnable examples)
       - Installation instructions
       Files: book/ (NEW)

S24.2  Embedded ML tutorial
       - "Your first ML model on bare metal"
       - Step-by-step: train → quantize → deploy
       - Working code at each step
       Files: book/src/tutorials/embedded_ml.md

S24.3  OS Development tutorial
       - "Write a kernel module in Fajar Lang"
       - Memory management, interrupts, syscalls
       - QEMU-testable
       Files: book/src/tutorials/os_dev.md

S24.4  API reference generation
       - cargo doc with full coverage
       - All pub items documented
       - Examples in doc comments
       Files: src/**/*.rs
```

### Sprint 25: Package Ecosystem

```
S25.1  Package registry design
       - fj.toml dependency syntax
       - Version resolution algorithm
       - Registry API (simple file-based first)
       Files: src/package/registry.rs (NEW)

S25.2  Core packages
       - fj-hal: Hardware abstraction layer
       - fj-nn: Neural network layers
       - fj-drivers: Common sensor/actuator drivers
       - fj-math: Extended math functions
       Files: packages/ (NEW)

S25.3  Package publishing
       - `fj publish` command
       - Package validation (tests pass, docs present)
       - Semantic versioning enforcement
       Files: src/main.rs, src/package/

S25.4  Dependency resolution
       - SAT solver for version constraints
       - Lock file (fj.lock)
       - Offline mode support
       Files: src/package/resolver.rs (NEW)
```

### Sprint 26: Release

```
S26.1  Release candidate testing
       - Run full test suite on all targets
       - Run all examples
       - Run benchmarks, compare to baseline
       - Manual testing: 10 real-world programs
       Files: tests/

S26.2  Binary distribution
       - Build for: linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64
       - GitHub Release with binaries
       - curl-based installer script
       Files: .github/workflows/release.yml

S26.3  Announcement & documentation
       - Blog post: "Introducing Fajar Lang v1.0"
       - Feature comparison with Rust, Zig, C++ for embedded ML
       - Benchmark results
       Files: docs/

S26.4  Post-release plan
       - Bug tracker setup
       - Community contribution guidelines
       - v1.1 roadmap
       Files: CONTRIBUTING.md, docs/ROADMAP_V1.1.md
```

---

## Milestone Summary — Actual Results

| Milestone | Sprint | Tests | Key Deliverable | Status |
|-----------|--------|-------|-----------------|--------|
| v0.2.0 | S1-S4 | 1,100+ | Cranelift JIT+AOT, arrays, strings, CI/CD | ✅ DONE |
| v0.3.0 | S5-S8 | 1,200+ | Generics (monomorphization), traits, FFI, type inference | ✅ DONE |
| v0.4.0 | S9-S13 | 1,350+ | Move semantics, borrow checker (NLL), safety audit | ✅ DONE |
| v0.5.0 | S14-S17 | 1,400+ | Autograd, Conv2d, attention, quantization (INT8) | ✅ DONE |
| v0.6.0 | S18-S22 | 1,430+ | ARM64+RISC-V cross-compile, no_std, HAL, drone example | ✅ DONE |
| v1.0.0 | S24-S26 | 1,563 | mdBook docs, package ecosystem, release workflows | ✅ DONE |

### Final Statistics

```
Tasks:     506 complete | 49 deferred to v0.2 | 0 remaining
Tests:     1,430 default + 133 native codegen = 1,563 total
LOC:       ~45,000 lines of Rust
Examples:  15 .fj programs (all passing)
Benchmarks: 12 criterion benchmarks
Quality:   clippy zero warnings, fmt clean, cargo doc clean
Stdlib:    6 .fj files (all pass `fj check`)
Sprints:   24/26 complete (S11 tensor shapes + S23 self-hosting deferred)
```

---

## Risk Register — Post-mortem

| Risk | Predicted | Actual | Outcome |
|------|-----------|--------|---------|
| Cranelift API breaking changes | Medium | Low | Pinned 0.129.1, no issues |
| Borrow checker too complex | High | Medium | Scope-based + NLL hybrid, works well |
| Self-hosting infeasible in timeline | Medium | **Hit** | Deferred to v0.2 — needs string/array/enum in codegen |
| Embedded testing requires hardware | Low | Low | QEMU covers all test cases |
| Performance not competitive | Medium | Low | Native codegen beats targets |
| Single developer bottleneck | High | **Hit** | Mitigated by ruthless prioritization (49 tasks deferred) |

---

## v0.2 Roadmap

> **Audit date:** 2026-03-07 (comprehensive Phase A audit by 4 parallel agents)
> **Total deferred from v1.0:** 49 tasks
> **Phase A status:** A.1-A.7 implemented, audit found 9 critical bugs + 8 high-priority gaps + 7 medium gaps

---

### Phase A: Codegen Type System (enables most other features)

#### Completed Sub-Phases (A.1-A.7)

```
A.1  Type tracking in native codegen                              DONE
     - var_types, fn_return_types, last_expr_type tracking
     - f64 arithmetic, comparisons, unary neg, compound assign
     - Type-aware if/else merge blocks (infer_expr_type)
     - 16 native f64 tests
     Files: src/codegen/cranelift.rs

A.2  String values + runtime concat                               DONE
     - String as (ptr, len) pairs, intern_string for static data
     - fj_rt_str_concat runtime function (heap-allocated result)
     - Compile-time literal folding, mixed concat, chain concat
     - 4 native string tests
     Files: src/codegen/cranelift.rs

A.3  Dynamic arrays (heap-backed)                                 DONE
     - fj_rt_array_{new,push,get,set,len,pop,free} runtime
     - Vec<i64> as Box opaque pointer, heap_arrays tracking
     - Method dispatch (.push/.pop/.len), for-in iteration
     - 9 native heap array tests
     Files: src/codegen/cranelift.rs

A.4  Enum/match in native codegen                                 DONE
     - Tagged union: i64 tag + i64 payload
     - enum_defs, compile_path, bare variant lookup
     - Match: branch tree with test/body/merge blocks
     - Wildcard, literal, ident, enum patterns
     - Option/Result built-in tag mappings
     - 17 native enum/match tests
     Files: src/codegen/cranelift.rs

A.5  Struct codegen                                               DONE
     - Stack slot layout (8 bytes/field), compile_struct_init
     - compile_field_access, compile_field_assign
     - struct_defs, struct_slots, last_struct_init side-channel
     - 8 native struct tests
     Files: src/codegen/cranelift.rs

A.6  Impl blocks + method dispatch                                DONE
     - Methods mangled as TypeName_method
     - impl_methods HashMap, self passed as pointer
     - Static methods (Type::method), instance methods (obj.method)
     - 5 native impl tests
     Files: src/codegen/cranelift.rs

A.7  Tuple + as cast + pipeline                                   DONE
     - Tuple: stack slot 8 bytes/elem, .0/.1 index access
     - Cast: i64<->f64, i64->bool via compile_cast
     - Pipeline |>: desugar x |> f to f(x)
     - 8 native tests (3 tuple + 3 cast + 2 pipeline)
     Files: src/codegen/cranelift.rs
```

#### Gap-Fix Sub-Phases (A.8-A.12) — From Audit 2026-03-07

> **Audit findings:** 9 critical bugs (C1-C9), 8 high gaps (H1-H8), 7 medium gaps (M1-M7)
> **Root cause:** last_expr_type propagation incomplete, pattern matching partial,
>   memory management absent, all struct/tuple fields hardcoded as i64

```
A.8  Type Propagation Completeness (Wave 1 — CRITICAL)
     Fixes: C1-C7 — compile_* functions that don't set last_expr_type
     Impact: f64 values silently treated as i64 in downstream expressions

     A.8.1  compile_unary: set last_expr_type after Not/Neg ops
            - Not → bool type; Neg → preserve operand type (f64 or i64)
            File: src/codegen/cranelift.rs (compile_unary, ~line 3004-3028)

     A.8.2  compile_index: set last_expr_type after array index
            - Both heap and stack arrays → default_int_type (i64-only arrays)
            File: src/codegen/cranelift.rs (compile_index, ~line 2829-2890)

     A.8.3  compile_method_call: set last_expr_type for push/pop/len
            - push → void/i64; pop → element type; len → i64
            - Struct method calls → look up fn_return_types for mangled name
            File: src/codegen/cranelift.rs (compile_method_call, ~line 2220-2328)

     A.8.4  compile_while/compile_loop/compile_for: set last_expr_type
            - while/for → i64 (unit); loop with break value → break expr type
            File: src/codegen/cranelift.rs (~line 3484-3812)

     A.8.5  compile_match: set last_expr_type from merge block param
            - Infer merge type from first arm body, set after merge block
            File: src/codegen/cranelift.rs (compile_match, ~line 3570-3694)

     A.8.6  compile_block (in compile_expr): propagate tail expr type
            - After compiling tail expression, last_expr_type already set by tail
            - Verify it's not overwritten; add explicit propagation if needed
            File: src/codegen/cranelift.rs (~line 1774-1787)

     A.8.7  compile_pipe: set last_expr_type from fn_return_types
            - Look up called function's return type, same as compile_call
            File: src/codegen/cranelift.rs (~line 1883-1906)

     A.8.8  Extend infer_expr_type to handle all 25 expression variants
            - Currently: 8/25 handled (Literal, Ident, Call, Binary, Unary,
              Grouped, Block, If)
            - Add: MethodCall, Field, Index, Cast, Match, StructInit, Tuple,
              Array, Path, Pipe, While, Loop, For, Range, Assign, Try, Closure
            - Fallback: default_int_type only for truly unknown cases
            File: src/codegen/cranelift.rs (infer_expr_type, ~line 3428-3470)

     A.8.9  Tests: type propagation correctness
            - Test: if true { -3.14 } else { 0.0 } → must be f64
            - Test: let x = match y { 0 => 1.5, _ => 2.5 } → must be f64
            - Test: let x = { 1; 2.5 } → must be f64
            - Test: let x = loop { break 3.14; } → must be f64
            - Test: f64 pipeline, f64 method return
            File: src/codegen/cranelift.rs (tests module)

A.9  Pattern Matching Completeness (Wave 2 — HIGH)
     Fixes: H1-H3, C8 — silent pattern skips + unsafe tag fallback
     Impact: Match arms silently ignored; wrong variant tag on lookup failure

     A.9.1  Return CodegenError for unsupported patterns instead of silent skip
            - Replace jump-to-next with CodegenError::NotImplemented
            - Patterns: Tuple, Struct, Range → explicit error message
            File: src/codegen/cranelift.rs (compile_match, ~line 3667-3670)

     A.9.2  Implement Tuple pattern: (a, b, c)
            - Load subject as stack slot pointer
            - Bind each field: a = load(ptr, offset=0), b = load(ptr, offset=8)...
            - Support nested: (a, _) with wildcard
            File: src/codegen/cranelift.rs (compile_match pattern dispatch)

     A.9.3  Implement Struct pattern: Point { x, y }
            - Look up struct_defs for field names + order
            - Load from subject pointer at field offsets
            - Bind named variables in arm body scope
            File: src/codegen/cranelift.rs (compile_match pattern dispatch)

     A.9.4  Implement Range pattern: 1..10, 1..=10
            - Compile as: subject >= start AND subject < end (or <= for ..=)
            - Branch to body if both conditions true
            File: src/codegen/cranelift.rs (compile_match pattern dispatch)

     A.9.5  Fix resolve_variant_tag: return CodegenError on unknown variant
            - Replace fallback `0` at line ~3715 with error
            - Error: CodegenError::UndefinedVariant(name)
            File: src/codegen/cranelift.rs (resolve_variant_tag, ~line 3700-3716)

     A.9.6  Multi-field enum payloads (stretch goal)
            - Current: single i64 payload
            - Target: stack-allocated payload struct for multi-field variants
            - Enum repr: { tag: i64, payload_ptr: *mut u8 } or inline fields
            - Enables: Shape::Rect(w, h), Result<(i64, i64), String>
            File: src/codegen/cranelift.rs (enum representation redesign)

     A.9.7  Tests: pattern matching completeness
            - Test: match (1, 2) { (a, b) => a + b }
            - Test: match point { Point { x, y } => x + y }
            - Test: match n { 1..10 => "small", _ => "big" }
            - Test: unknown variant → compile error (not silent 0)
            File: src/codegen/cranelift.rs (tests module)

A.10 Memory Management (Wave 3 — HIGH)
     Fixes: C9, M6 — string concat leaks, no cleanup on early return
     Impact: Long-running programs exhaust heap memory

     A.10.1 Owned pointer tracking in CodegenCtx
            - Add owned_ptrs: Vec<(Variable, OwnedKind)> to CodegenCtx
            - OwnedKind: String | Array | StructHeap
            - Track which variables hold heap-allocated data
            File: src/codegen/cranelift.rs (CodegenCtx struct)

     A.10.2 Scope-exit cleanup: emit fj_rt_free for owned pointers
            - At function return: iterate owned_ptrs, call __free for each
            - Handle both explicit return and implicit (fall-through)
            - Exclude: pointers returned from function (transfer ownership)
            File: src/codegen/cranelift.rs (compile_fn epilogue)

     A.10.3 Early return cleanup
            - Before each `return` statement, emit cleanup for all owned_ptrs
            - in current scope and parent scopes up to function boundary
            - Skip the value being returned
            File: src/codegen/cranelift.rs (compile_return)

     A.10.4 String concat cleanup integration
            - After fj_rt_str_concat, register result ptr in owned_ptrs
            - On reassignment (s = s + "x"), free old ptr before storing new
            File: src/codegen/cranelift.rs (compile_str_concat)

     A.10.5 Heap array cleanup integration
            - fj_rt_array_new results → register in owned_ptrs
            - Function exit → call fj_rt_array_free for each heap array
            File: src/codegen/cranelift.rs (compile_heap_array_init)

     A.10.6 Tests: memory management
            - Test: string concat in loop doesn't crash (leak detection)
            - Test: early return after string concat (cleanup emitted)
            - Test: heap array freed on function exit
            - Test: returned string NOT freed (ownership transfer)
            File: src/codegen/cranelift.rs (tests module)

A.11 Type-Aware Struct & Tuple Fields (Wave 4 — MEDIUM)
     Fixes: H5-H8 — all fields hardcoded as i64, heap init incomplete
     Impact: f64/bool struct fields produce wrong values

     A.11.1 Expand struct_defs to include field types
            - Change: HashMap<String, Vec<String>>
              To:     HashMap<String, Vec<(String, ClifType)>>
            - Populate from AST struct definition type annotations
            - Default to i64 when type annotation absent
            File: src/codegen/cranelift.rs (struct_defs, compile_program)

     A.11.2 Type-aware compile_field_access
            - Look up field type from struct_defs instead of default_int_type
            - Use correct Cranelift load type (I64, F64, I8 for bool)
            - Set last_expr_type based on field type
            File: src/codegen/cranelift.rs (compile_field_access)

     A.11.3 Type-aware compile_field_assign
            - Look up field type for store operations
            - Type-check RHS value matches field type
            - Use correct Cranelift store type
            File: src/codegen/cranelift.rs (compile_field_assign)

     A.11.4 Type-aware compile_struct_init
            - Validate field value types match struct_defs types
            - Use correct store types per field
            File: src/codegen/cranelift.rs (compile_struct_init)

     A.11.5 Heterogeneous tuple support
            - Track element types in last_struct_init or new tuple_types map
            - Tuple index access uses correct load type per element position
            - Enable (i64, f64, bool) mixed tuples
            File: src/codegen/cranelift.rs (compile_tuple, compile_field_access)

     A.11.6 Fix heap array initial elements
            - Verify compile_heap_array_init loop pushes elements
            - If stubbed: implement fj_rt_array_push calls for each element
            - Test: let arr = [1, 2, 3]; assert arr.len() == 3
            File: src/codegen/cranelift.rs (compile_heap_array_init)

     A.11.7 Fix analysis.rs string size mismatch
            - Change estimate_type_size("String") from PTR_SIZE*3 to PTR_SIZE*2
            - Or: implement cap tracking (depends on A.10 decision)
            - Update test_type_size_string assertion
            File: src/codegen/analysis.rs (~line 99, ~line 587)

     A.11.8 Tests: type-aware fields
            - Test: struct with f64 field → correct value roundtrip
            - Test: struct with bool field → correct value
            - Test: struct with mixed i64 + f64 fields
            - Test: tuple (i64, f64) → correct element access
            - Test: heap array [1, 2, 3] has length 3 and correct elements
            File: src/codegen/cranelift.rs (tests module)

A.12 Codegen Completeness Polish (Wave 5 — MEDIUM)
     Fixes: M1-M5, M7 — missing compound ops, pipeline to calls, casts
     Impact: Minor feature gaps, some operations return errors

     A.12.1 Implement missing compound assignment operators
            - Add: /=, %=, &=, |=, ^=, <<=, >>= in compile_field_assign
            - Match existing pattern from += -= *=
            File: src/codegen/cranelift.rs (compile_field_assign, ~line 2105-2109)

     A.12.2 Pipeline to function calls: x |> f(y) → f(x, y)
            - Detect Expr::Call on RHS of pipe
            - Prepend LHS value as first argument
            - Preserve existing ident-only path
            File: src/codegen/cranelift.rs (compile_expr Pipe case)

     A.12.3 Additional cast operations
            - f64 → bool: fcmp NotEqual with 0.0
            - bool → f64: uextend + fcvt_from_sint
            - Explicit error for unsupported cast instead of silent pass-through
            File: src/codegen/cranelift.rs (compile_cast, ~line 2161-2215)

     A.12.4 Nested enum patterns (stretch goal)
            - Support Some(Some(x)) via recursive pattern compilation
            - Load payload, re-match inner pattern
            File: src/codegen/cranelift.rs (compile_match)

     A.12.5 Match exhaustiveness warning
            - After all arms compiled, check if fallthrough reachable
            - Emit CodegenError::Warning for non-exhaustive match
            - Or: require _ wildcard arm (simpler approach)
            File: src/codegen/cranelift.rs (compile_match)

     A.12.6 Tests: completeness polish
            - Test: struct field /= and %= compound assign
            - Test: 5 |> add(10) → 15
            - Test: 1.5 as bool → true; 0.0 as bool → false
            - Test: match without _ arm → error or warning
            File: src/codegen/cranelift.rs (tests module)
```

#### Phase A — Summary Table

| Sub-Phase | Description | Tasks | Tests | Status |
|-----------|-------------|-------|-------|--------|
| A.1 | Type tracking + f64 | 13 | 16 | DONE |
| A.2 | String values + concat | 7 | 4 | DONE |
| A.3 | Dynamic arrays (heap) | 12 | 9 | DONE |
| A.4 | Enum/match | 10 | 17 | DONE |
| A.5 | Struct codegen | 8 | 8 | DONE |
| A.6 | Impl blocks + methods | 4 | 5 | DONE |
| A.7 | Tuple + cast + pipeline | 6 | 8 | DONE |
| **A.8** | **Type propagation fixes** | **9** | **~10** | **TODO** |
| **A.9** | **Pattern matching completeness** | **7** | **~8** | **TODO** |
| **A.10** | **Memory management** | **6** | **~6** | **TODO** |
| **A.11** | **Type-aware struct/tuple** | **8** | **~10** | **TODO** |
| **A.12** | **Codegen polish** | **6** | **~8** | **TODO** |
| | **TOTAL** | **96** | **~129** | **60 done / 36 new** |

#### Phase A — Dependency Graph

```
A.1-A.7 (DONE)
    |
    +---> A.8 Type Propagation (no deps, start first)
    |         |
    |         +---> A.9 Pattern Matching (needs A.8 for match type inference)
    |         |
    |         +---> A.11 Type-Aware Fields (needs A.8 for field type propagation)
    |                   |
    |                   +---> A.12 Codegen Polish (needs A.11 for compound assigns)
    |
    +---> A.10 Memory Management (independent, can parallel with A.8)
```

#### Phase A — Estimated Effort

| Wave | Sub-Phase | Complexity | Est. Sessions |
|------|-----------|-----------|---------------|
| 1 | A.8 Type propagation | Low-Medium | 1-2 |
| 2 | A.9 Pattern matching | Medium-High | 2-3 |
| 3 | A.10 Memory management | High | 2-3 |
| 4 | A.11 Type-aware fields | Medium | 1-2 |
| 5 | A.12 Polish | Low | 1 |
| | **TOTAL** | | **7-11 sessions** |

---

### Phase B: Advanced Type System ✅ DONE (B.1-B.3) + Audit Fixes (B.4-B.7)

> **Prerequisite:** Phase A.8-A.12 complete (type tracking must be solid)

```
B.1  Const generics & tensor shape types               ✅ DONE (Session 25)
     - Type::Tensor { element, dims } with compile-time shapes
     - Type::dynamic_tensor() for unknown-shape tensors
     - Shape algebra: matmul_shape(), elementwise_shape()
     - BinOp::MatMul (@) compile-time shape checking, TE001 error
     - Tensor builtin return types: Unknown → dynamic_tensor()
     - Tensor builtins non-consuming (exempt from move tracking)
     - 18 tests
     Files: src/analyzer/type_check.rs, src/lib.rs, src/lsp/server.rs

B.2  Full type-checked destructuring                    ✅ DONE (Session 24)
     - enum_variant_types, last_enum_payload_type, enum_vars with payload type
     - dfg.value_type() for runtime-correct pattern binding across arms
     - Tuple, struct, ident pattern type-awareness
     - infer_expr_type for Match: all arms, define_function error recovery
     - 8 tests
     Files: src/codegen/cranelift.rs

B.3  Static dispatch for traits in codegen              ✅ DONE (Session 24)
     - trait_defs, trait_impls collection in JIT + AOT compile_program
     - obj.method() dispatch, Trait::method(obj) qualified calls
     - Inherent + trait impls coexist, no vtables
     - 6 tests
     Files: src/codegen/cranelift.rs
```

#### Phase B Audit (2026-03-07) — 18 Findings, 4 Fix Waves

Post-completion audit by 3 parallel agents found **5 CRITICAL, 5 HIGH, 8 MEDIUM**
issues across B.1-B.3. Organized into 4 fix waves: B.4-B.7.

```
B.4  B.1 Tensor Shape Hardening (Wave 1 — P0 CRITICAL)
     Fixes: C1 (@ operator untested), C2 (elementwise dead code)
     Impact: Shape checking claimed but partially unverified
     Depends on: B.1 done

     B.4.1 @ operator matmul shape tests
           - Test: a @ b where both have known compatible shapes → OK
           - Test: a @ b where K dimensions mismatch → TE001 emitted
           - Test: a @ b where both dynamic → no error, returns dynamic tensor
           - Test: a @ b where one operand is 1D → TE001 (not 2D)
           Note: @ operator currently returns dynamic_tensor() when operands
                 are from tensor_zeros (empty dims). Tests must use annotated
                 parameters like fn f(a: Tensor<f64>[3,4], b: Tensor<f64>[4,5])
                 to exercise static shape checking.
           File: src/analyzer/type_check.rs (tests module)

     B.4.2 Integrate elementwise_shape() into analyzer
           - Option A: Add check_tensor_elementwise() called for BinOp::Add/Sub/Mul/Div
                       when BOTH operands are Type::Tensor with known dims
           - Option B: Document that elementwise shape checking is runtime-only
                       (dynamic tensors from builtins have empty dims → always compatible)
           - Decision: Option A only fires when BOTH tensors have non-empty dims
                       (i.e., from annotated params). Otherwise skip (runtime check).
           - Emit TE001 on mismatch: "elementwise: Tensor[3,4] + Tensor[5,6]"
           File: src/analyzer/type_check.rs (check_binary)

     B.4.3 Reject nested tensor types
           - In resolve_type() for TypeExpr::Tensor: if element resolves to
             Type::Tensor, emit SE004 "tensor element type cannot be Tensor"
           - Test: Tensor<Tensor<f64>[3]>[2] → error
           File: src/analyzer/type_check.rs (resolve_type)

     B.4.4 TE001 documentation
           - Add TE001 entry to docs/ERROR_CODES.md
           - Include: error message, explanation, example code, fix suggestion
           File: docs/ERROR_CODES.md

     B.4.5 Tests: tensor shape hardening
           - Test: @ with annotated tensor params (valid + invalid shapes)
           - Test: elementwise ops with annotated tensor params (valid + invalid)
           - Test: nested tensor type rejected
           - Test: dynamic tensors bypass shape checking
           Expected: 6-8 new tests
           File: src/analyzer/type_check.rs (tests module)

B.5  B.3 Trait Dispatch Correctness (Wave 2 — P0 CRITICAL)
     Fixes: C3 (receiver type), C4 (method validation), C5 (error fallthrough),
            H4 (HashMap ordering), H5 (JIT/AOT dedup)
     Impact: Trait-qualified calls silently call wrong implementation
     Depends on: B.3 done

     B.5.1 Validate receiver type for Trait::method(obj)
           - After finding impl via trait_impls scan, verify first arg type
             matches type_n (the implementing type)
           - Track struct type from compile_expr on first arg
           - If no matching impl for receiver type → error CE011
           - This also fixes H4 (HashMap ordering) because we no longer
             pick the first match — we pick the one matching the receiver
           File: src/codegen/cranelift.rs (compile_call, trait path resolution)

     B.5.2 Validate method exists in trait definition
           - Before calling mangled TypeName_method, check:
             cx.trait_defs[trait_name].contains(method_name)
           - If not → error CE012 "method 'X' not defined in trait 'T'"
           File: src/codegen/cranelift.rs (compile_call, trait path resolution)

     B.5.3 Error fallthrough for unmatched trait calls
           - After trait_impls iteration loop, if no impl found:
             return Err(CodegenError::NotImplemented(
               "no implementation of trait 'T' found for method 'M'"
             ))
           - Currently falls through silently to regular function call
           File: src/codegen/cranelift.rs (compile_call)

     B.5.4 Extract shared trait collection logic
           - Create fn collect_trait_info(program) -> (trait_defs, trait_impls, impl_methods)
           - Called from both CraneliftCompiler::compile_program and
             ObjectCompiler::compile_program
           - Eliminates duplication, ensures fixes apply to both paths
           File: src/codegen/cranelift.rs (new shared function)

     B.5.5 Tests: trait dispatch correctness
           - Test: two types implement same trait, Trait::method(type_a) calls correct impl
           - Test: two types implement same trait, Trait::method(type_b) calls correct impl
           - Test: Trait::non_existent_method(obj) → error CE012
           - Test: Trait::method(non_implementing_type) → error CE011
           - Test: multiple impls iterate deterministically
           Expected: 5-6 new tests
           File: src/codegen/cranelift.rs (tests module)

B.6  B.2 Destructuring Robustness (Wave 3 — P1 HIGH)
     Fixes: H1 (multi-field enum), H2 (dead code), H3 (tuple literal subject),
            M3 (merge type), M5 (no-payload binding), M6 (wildcard patterns)
     Impact: Edge cases in pattern matching silently miscompile
     Depends on: B.2 done

     B.6.1 Document multi-field enum variant limitation
           - Multi-field variants (V(i64, f64, bool)) are NOT supported in codegen
           - Only single-payload variants work: V(i64), V(f64)
           - Add comment in compile_program enum collection explaining this
           - Add test that multi-field variant match gracefully errors (or defers to v0.3)
           - Decision: document as known limitation, not a v0.2 fix target
                       (full support needs stack layout for variant payloads)
           File: src/codegen/cranelift.rs (compile_program enum collection)

     B.6.2 Remove or repurpose enum_variant_types dead code
           - Option A: Remove #[allow(dead_code)] and enum_variant_types entirely
                       (actual types come from dfg.value_type)
           - Option B: Use enum_variant_types as validation:
                       compare dfg.value_type(payload_val) vs enum_variant_types
                       → emit warning on mismatch (debug aid)
           - Decision: Option A (remove). dfg.value_type is the source of truth.
           File: src/codegen/cranelift.rs

     B.6.3 Tuple pattern for literal subjects
           - Handle match (expr1, expr2) { (a, b) => ... }
           - When subject is Expr::Tuple, compile each element and record types
             in a temporary Vec<ClifType> (not relying on tuple_types map)
           - Fallback: if subject_tuple_types is None, use dfg.value_type for
             each element loaded from the tuple slot
           File: src/codegen/cranelift.rs (compile_match, tuple pattern)

     B.6.4 Match merge type unification
           - Instead of "first non-default type wins", collect all arm body types
           - If all non-default types agree → use that type
           - If types conflict (f64 vs i64) → use wider type (f64) or emit error
           - Also update compile_match merge block creation accordingly
           File: src/codegen/cranelift.rs (infer_expr_type Match, compile_match)

     B.6.5 No-payload variant binding guard
           - In Pattern::Enum binding: if variant has no payload fields
             (checked via enum_variant_types or enum_defs), skip binding
           - Currently: Red(x) on no-payload Red silently binds x = 0 (tag i64)
           - After fix: Red(x) on no-payload Red → emit error or skip bind
           File: src/codegen/cranelift.rs (compile_match, enum pattern)

     B.6.6 Tests: destructuring robustness
           - Test: match (1, 2) { (a, b) => a + b } (literal tuple subject)
           - Test: match with conflicting arm types (f64 arm + i64 arm) → wider type
           - Test: wildcard in tuple pattern: (a, _, c) → only a and c bound
           - Test: wildcard in struct pattern: Point { x, .. } → only x bound
           - Test: nested match: match x { 1 => match y { 2 => 3, _ => 4 }, _ => 5 }
           - Test: no-payload enum variant with binding attempt → graceful error
           Expected: 6-8 new tests
           File: src/codegen/cranelift.rs (tests module)

B.7  Phase B Documentation & Polish (Wave 4 — P2 MEDIUM)
     Fixes: M1 (nested tensor), M2 (docs), M4 (nested match), M7 (multi-impl),
            M8 (trait return types)
     Impact: Documentation, edge case polish

     B.7.1 Trait method return type tracking
           - When compiling trait method call, look up return type from
             fn_return_types using mangled name (TypeName_method)
           - If not found, infer from trait_defs (requires storing return types)
           - Update last_expr_type with actual return type (not default i64)
           File: src/codegen/cranelift.rs (compile_method_call)

     B.7.2 Update docs/ERROR_CODES.md
           - Add TE001: tensor shape mismatch (from B.4.4)
           - Add CE011: trait impl not found for receiver type (from B.5.1)
           - Add CE012: method not defined in trait (from B.5.2)
           - Review existing CE001-CE010 for accuracy
           File: docs/ERROR_CODES.md

     B.7.3 Update CLAUDE.md error code count
           - Current: 71 error codes across 9 categories
           - Add TE001, CE011, CE012 → 74 error codes
           - Update CE category count: CE001-CE012 (was CE001-CE010)
           File: CLAUDE.md (Section 7)

     B.7.4 Phase B summary in V1_IMPLEMENTATION_PLAN.md
           - Update summary table with actual task/test counts
           - Mark B.1-B.3 DONE, B.4-B.7 status
           File: docs/V1_IMPLEMENTATION_PLAN.md
```

#### Phase B Audit — Dependency Graph

```
B.1-B.3 (DONE)
    |
    +---> B.4 Tensor Shape Hardening (no deps, start first)
    |         |
    |         +---> B.7.2 docs/ERROR_CODES.md update (needs TE001 from B.4)
    |
    +---> B.5 Trait Dispatch Correctness (no deps, parallel with B.4)
    |         |
    |         +---> B.7.1 Trait return type tracking (needs B.5.4 shared logic)
    |         |
    |         +---> B.7.2 docs/ERROR_CODES.md update (needs CE011, CE012 from B.5)
    |
    +---> B.6 Destructuring Robustness (no deps, parallel with B.4/B.5)
    |
    +---> B.7 Polish (needs B.4 + B.5 for error codes)
```

#### Phase B Audit — Estimated Effort

| Wave | Sub-Phase | Complexity | Est. Sessions | Priority |
|------|-----------|-----------|---------------|----------|
| 1 | B.4 Tensor shape hardening | Low-Medium | 1 | P0 |
| 2 | B.5 Trait dispatch correctness | Medium-High | 1-2 | P0 |
| 3 | B.6 Destructuring robustness | Medium | 1-2 | P1 |
| 4 | B.7 Documentation + polish | Low | 0.5 | P2 |
| | **TOTAL** | | **3.5-5.5 sessions** | |

#### Phase B — Summary Table (Updated)

| Sub-Phase | Description | Tasks | Tests | Status |
|-----------|-------------|-------|-------|--------|
| B.1 | Const generics & tensor shapes | 16 | 18 | ✅ DONE |
| B.2 | Type-checked destructuring | 12 | 8 | ✅ DONE |
| B.3 | Static trait dispatch | 12 | 6 | ✅ DONE |
| B.4 | Tensor shape hardening | 5 | 10 | ✅ DONE |
| B.5 | Trait dispatch correctness | 5 | 5 | ✅ DONE |
| B.6 | Destructuring robustness | 6 | 6 | ✅ DONE |
| B.7 | Documentation & polish | 4 | 0 | ✅ DONE |
| | **TOTAL** | **60** | **53** | **ALL DONE** |

### Phase F: A/B Hardening (requires Phase A + B)

> **Prerequisite:** Phase A + B complete
> **Motivation:** Post-completion audit (4 agents, 28 findings) revealed 3 critical, 5 high,
> and 4 medium-severity gaps in codegen safety and type checker completeness.
> These MUST be fixed before Phase C (self-hosting) or E (parity) to prevent
> silent miscompilation and panics in production codegen paths.

#### F.1 — Stack Slot Overflow Guards `P0 CRITICAL`

> **Problem:** `(num as u32) * 8` in struct/tuple/array slot allocation can overflow u32,
> causing Cranelift to silently allocate undersized stack slots → memory corruption.
> **Files:** `src/codegen/cranelift.rs`

```
F.1.1  Struct slot: checked_mul guard
       - Line ~2178: `(num_fields as u32) * 8`
       - Replace with checked_mul, return CodegenError on overflow
       - Test: struct with 0 fields, 1 field, normal, pathological name

F.1.2  Tuple slot: checked_mul guard
       - Line ~2425: `num * 8`
       - Replace with checked_mul, return CodegenError on overflow
       - Test: empty tuple, large tuple (20+ elements)

F.1.3  Array slot: checked_mul guard
       - Line ~3081: `(len as u32) * elem_size`
       - Replace with checked_mul, return CodegenError on overflow
       - Test: array with 0 elements, normal, very large

F.1.4  Field offset bounds validation
       - In compile_field_access/compile_struct_init: assert offset < slot_size
       - Prevents out-of-bounds stack_store/stack_load
       - Test: access last field of struct, out-of-range field name → error
```

#### F.2 — TrapCode Safety `P0 CRITICAL`

> **Problem:** `TrapCode::user(1).unwrap()` in 4 production paths can panic if
> Cranelift API changes or input validation tightens.
> **Files:** `src/codegen/cranelift.rs`

```
F.2.1  Define constant for user trap code
       - Create: `const TRAP_BOUNDS_CHECK: TrapCode = ...;`
       - If TrapCode::user(1) returns None, fall back to TrapCode::USER0 or
         use .expect("user trap code 1 is valid per Cranelift API")
       - Replace all 4 occurrences (lines ~929, ~1622, ~3205, ~3302)
       - Test: compile program with array bounds check → no panic
```

#### F.3 — Silent Type Loss (default_int_type fallbacks) `P0 CRITICAL`

> **Problem:** 16 places use `unwrap_or(clif_types::default_int_type())` which silently
> treats f64/bool/ptr/char as i64 when type inference fails → wrong native code.
> **Strategy:** Fix the 6 highest-impact sites; remaining 10 are safe (param types
> always resolve via lower_type, or context guarantees i64).
> **Files:** `src/codegen/cranelift.rs`

```
F.3.1  Block tail expression type (line ~1895)
       - cx.last_expr_type.unwrap_or(default_int_type()) after compile_expr
       - Fix: compile_expr always sets last_expr_type; add defensive log if None
       - Impact: block expressions like `let x = { 3.14 }` return wrong type
       - Test: `fn main() -> f64 { let x = { 3.14 }; x }` → correct f64

F.3.2  For-loop iterator element type (line ~1922)
       - Element type for for-in loop falls back to i64
       - Fix: infer from array's tracked elem_type or range expression type
       - Impact: `for x in [1.0, 2.0, 3.0]` → x treated as i64
       - Test: for-in over f64 array → accumulate sum → correct f64

F.3.3  Tuple element type in compile_tuple (line ~2433)
       - cx.last_expr_type after compiling each element
       - Fix: compile_expr sets this; verify tuple with mixed types
       - Impact: `(1, 3.14)` → second element stored as i64
       - Test: `let t = (42, 3.14); t.1` → correct f64

F.3.4  Match arm ident binding type (line ~4172)
       - cx.last_expr_type for wildcard/ident pattern match
       - Fix: use subject's tracked type, not last_expr_type
       - Impact: `match x { n => n }` where x is f64 → n bound as i64
       - Test: `let x: f64 = 3.14; match x { n => n }` → correct f64

F.3.5  Enum payload type in match (lines ~3852-3976, 5 sites)
       - Variant payload type unknown → default i64
       - Fix: most already use dfg.value_type(payload_val); verify remaining
       - Impact: `Some(3.14)` → payload read as i64 in match
       - Test: `match Some(3.14) { Some(v) => v, None => 0.0 }` → f64

F.3.6  Array element type in compile_index (line ~2255)
       - Tuple element type lookup falls back to i64
       - Already addressed in A.11 for most cases; verify edge cases
       - Test: `let arr = [1.0, 2.0]; arr[0]` → correct f64
```

#### F.4 — Missing Builtin Registrations in Type Checker `P1 HIGH`

> **Problem:** 4 autograd builtins exist in interpreter but are NOT registered in the
> type checker, causing type mismatches to be silently missed.
> **Files:** `src/analyzer/type_check.rs`

```
F.4.1  Register tensor_detach
       - Signature: (Tensor) -> Tensor (dynamic)
       - Add to register_builtins() after other autograd entries

F.4.2  Register tensor_clear_tape
       - Signature: () -> Void
       - Add to register_builtins()

F.4.3  Register tensor_no_grad_begin / tensor_no_grad_end
       - Signature: () -> Void each
       - Add to register_builtins()

F.4.4  Tests (~4 new)
       - Test: tensor_detach(t) type-checks OK
       - Test: tensor_clear_tape() with no args → OK
       - Test: tensor_no_grad_begin() / end() → OK
       - Test: tensor_detach(42) → SE004 TypeMismatch
```

#### F.5 — Cast Expression Type Validation `P1 HIGH`

> **Problem:** `Expr::Cast { .. }` returns `Type::Unknown` in type checker without
> validating cast compatibility or propagating the target type.
> **Files:** `src/analyzer/type_check.rs`

```
F.5.1  Implement check_cast()
       - Extract target type from TypeExpr
       - Validate: numeric↔numeric OK, bool→int OK, int→bool OK
       - Reject: String as i64, Tensor as f64, struct as int
       - Return the target type (not Unknown)

F.5.2  Wire into check_expr()
       - Replace `Expr::Cast { .. } => Type::Unknown` with check_cast() call
       - Expr::Try remains Unknown (correct — needs Result type tracking)

F.5.3  Tests (~6 new)
       - Test: `42 as f64` → Type::F64
       - Test: `3.14 as i64` → Type::I64
       - Test: `1 as bool` → Type::Bool
       - Test: `true as i64` → Type::I64
       - Test: `"hello" as i64` → SE004 TypeMismatch
       - Test: `x as UnknownType` → error
```

#### F.6 — Missing Method Registrations `P1 HIGH`

> **Problem:** Several string and array methods exist in the interpreter but are
> not registered in the type checker's method dispatch, causing false negatives.
> **Files:** `src/analyzer/type_check.rs` (check_method_call)

```
F.6.1  Register missing string methods
       - chars() -> Array (element: Char)
       - repeat(n: I64) -> Str
       - trim_start() -> Str
       - trim_end() -> Str

F.6.2  Register missing array method
       - join(separator: Str) -> Str

F.6.3  Tests (~5 new)
       - Test: "hello".chars() → type Array
       - Test: "abc".repeat(3) → type Str
       - Test: "  hi  ".trim_start() → type Str
       - Test: [1, 2, 3].join(", ") → type Str
       - Test: "hello".nonexistent() → SE error
```

#### F.7 — Fragile `self` Struct Type Lookup `P1 HIGH`

> **Problem:** `compile_field_access` for `self.field` scans ALL impl_methods keys
> with `.find()` to determine struct type. If multiple structs have impl methods,
> it may pick the wrong struct. HashMap iteration order is non-deterministic.
> **Files:** `src/codegen/cranelift.rs`

```
F.7.1  Track current struct type in CodegenCtx
       - Add field: `current_impl_type: Option<String>` to CodegenCtx
       - Set it when entering an impl method compilation
       - Clear it when leaving

F.7.2  Use current_impl_type in compile_field_access
       - Replace impl_methods.keys().find() with cx.current_impl_type lookup
       - Fall back to scan only if current_impl_type is None (backward compat)

F.7.3  Tests (~3 new)
       - Test: two structs with impl, each accessing self.field → correct values
       - Test: nested method call (self.x in struct A, self.x in struct B) → no cross-talk
       - Test: method accessing self.field returns correct type (f64 vs i64)
```

#### F.8 — Missing Operator Tests `P2 MEDIUM`

> **Problem:** Bare bitwise operators and some comparison operators lack direct tests.
> They work indirectly (via field assignment, loops) but should have explicit coverage.
> **Files:** `src/codegen/cranelift.rs` (test section)

```
F.8.1  Bare bitwise operator tests
       - Test: `a & b` (AND)
       - Test: `a | b` (OR)
       - Test: `a ^ b` (XOR)
       - Test: `a << 2` (left shift)
       - Test: `a >> 1` (right shift)

F.8.2  Missing comparison operator tests
       - Test: `a != b` → correct bool/branch
       - Test: `a <= b` → correct bool/branch
       - Test: `a >= b` → correct bool/branch

F.8.3  Block expression tests
       - Test: `let x = { let y = 10; y * 2 }` → 20
       - Test: `let x: f64 = { 3.14 }` → correct f64
```

---

#### Phase F — Dependency Graph

```
(no cross-dependencies — all waves can run in parallel)

F.1 Stack Slot Guards ──────────────> (standalone, P0)
F.2 TrapCode Safety ───────────────> (standalone, P0)
F.3 Silent Type Loss ──────────────> (standalone, P0, largest)
F.4 Missing Builtins ──────────────> (standalone, P1)
F.5 Cast Type Validation ──────────> (standalone, P1)
F.6 Missing Methods ───────────────> (standalone, P1)
F.7 Self Struct Lookup ────────────> (standalone, P1)
F.8 Missing Tests ─────────────────> (standalone, P2, after F.1-F.7)
```

#### Phase F — Estimated Effort

| Wave | Sub-Phase | Complexity | Est. Time | Priority | Tests |
|------|-----------|-----------|-----------|----------|-------|
| 1 | F.1 Stack slot overflow guards | Low | 30 min | P0 | ~4 |
| 2 | F.2 TrapCode safety | Low | 15 min | P0 | ~1 |
| 3 | F.3 Silent type loss (6 sites) | Medium-High | 1.5 hr | P0 | ~6 |
| 4 | F.4 Missing builtin registrations | Low | 20 min | P1 | ~4 |
| 5 | F.5 Cast expression validation | Medium | 30 min | P1 | ~6 |
| 6 | F.6 Missing method registrations | Low | 20 min | P1 | ~5 |
| 7 | F.7 Self struct type lookup | Medium | 30 min | P1 | ~3 |
| 8 | F.8 Missing operator tests | Low | 20 min | P2 | ~11 |
| | **TOTAL** | | **~4 hours** | | **~40** |

#### Phase F — Summary Table

| Sub-Phase | Description | Tasks | Tests | Status |
|-----------|-------------|-------|-------|--------|
| F.1 | Stack slot overflow guards | 4 | ~4 | TODO |
| F.2 | TrapCode safety | 1 | ~1 | TODO |
| F.3 | Silent type loss fixes | 6 | ~6 | TODO |
| F.4 | Missing builtin registrations | 4 | ~4 | TODO |
| F.5 | Cast expression type validation | 3 | ~6 | TODO |
| F.6 | Missing method registrations | 3 | ~5 | TODO |
| F.7 | Self struct type lookup fix | 3 | ~3 | TODO |
| F.8 | Missing operator tests | 3 | ~11 | TODO |
| | **TOTAL** | **27** | **~40** | **TODO** |

---

### Phase C: Self-Hosting (requires Phase A + B + F)

> **Prerequisite:** Phase F complete (codegen safety hardened),
> A.8-A.12 (string/array/enum in native), B.2 (destructuring for parser)

```
C.1  Fajar Lang lexer in .fj
     - Port src/lexer/ to Fajar Lang syntax
     - Requires: string methods, char iteration, enum/match, file I/O
     - Verify: self-lexer produces same output as Rust lexer
     Files: self/lexer.fj

C.2  Fajar Lang parser in .fj
     - Port src/parser/ to Fajar Lang syntax
     - Requires: recursive data structures, dynamic arrays, pattern matching
     - Verify: self-parser produces same AST
     Files: self/parser.fj

C.3  Bootstrap test
     - Compile self-hosted compiler with Rust compiler
     - Use self-hosted compiler to compile itself
     - Verify output matches
     Files: self/bootstrap.sh
```

### Phase D: Production Polish

> **Prerequisite:** Phase A.10 (memory management for long-running programs)

```
D.1  Optimization pass
     - Dead code elimination in codegen
     - Loop unrolling for small known ranges
     - Constant folding beyond string literals
     - Performance benchmarks vs C (fibonacci, matrix multiply, inference)
     Files: src/codegen/cranelift.rs

D.2  External infrastructure
     - GitHub Pages documentation site (mdbook deploy)
     - Package registry hosting (fj publish backend)
     - Installer testing on clean machines (CI matrix)
     Files: .github/workflows/, docs/

D.3  Extended FFI
     - Variadic function support (printf)
     - Pointer argument marshaling (VirtAddr <-> raw pointer)
     - Module system integration for FFI imports
     Files: src/codegen/cranelift.rs, src/interpreter/ffi.rs
```

### Phase E: Interpreter-Codegen Parity (ongoing)

> **Goal:** Native codegen should handle everything the interpreter handles.
> **Track:** Features added to interpreter that don't yet work in native.

```
E.1  Parity audit (run after each phase)
     - Compare interpreter eval_tests.rs coverage vs native codegen
     - Identify expressions/statements that work in interp but fail in native
     - Prioritize by usage frequency in example programs

E.2  Closure support in codegen
     - Closure capture (by value initially)
     - Anonymous function compilation
     - Closure as function argument

E.3  String methods in codegen
     - .len(), .contains(), .starts_with(), .trim(), .split()
     - Requires: A.10 (memory management for returned strings)

E.4  Generic function compilation
     - Monomorphize generic functions in native codegen
     - Reuse interpreter's monomorphization cache
     - Requires: A.11 (type-aware codegen)
```

---

### v0.2 Phase Summary

| Phase | Focus | Sub-Phases | Tasks | Tests | Depends On | Status |
|-------|-------|------------|-------|-------|------------|--------|
| **A** | Codegen type system | A.1-A.12 | 151 | 135 | — | ✅ DONE |
| **B** | Advanced type system | B.1-B.7 | 60 | 53 | A.8-A.12 | ✅ DONE |
| **F** | A/B Hardening | F.1-F.8 | 27 | ~40 | A + B | TODO |
| **C** | Self-hosting | C.1-C.3 | ~12 | ~10 | A + B + F | NOT STARTED |
| **D** | Production polish | D.1-D.3 | ~10 | ~6 | A.10 | NOT STARTED |
| **E** | Interp-codegen parity | E.1-E.4 | ~12 | ~20 | A.8+ | NOT STARTED |
| | **TOTAL** | | **~272** | **~264** | | |

### v0.2 Phase Execution Order

```
Phase A (DONE) ──> Phase B (DONE) ──> Phase F (Hardening)
                                           │
                                           ├──> Phase C (Self-Hosting)
                                           │
                                           ├──> Phase E (Parity)
                                           │
                                           └──> Phase D (Polish)
```

---

*V1_IMPLEMENTATION_PLAN.md v5.0 — v1.0 Complete + v0.2 Comprehensive Roadmap (post-Phase F audit) | Updated 2026-03-07*
