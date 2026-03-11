# Fajar Lang v2.0 "Transcendence" — Implementation Plan

> **Focus:** Revolutionary language features — dependent types, linear types, formal verification
> **Timeline:** 32 sprints, ~320 tasks across 8 phases
> **Prerequisite:** v1.1 "Ascension" COMPLETE
> **Target release:** 2027 Q1

---

## Motivation

v1.0 established Fajar Lang as a capable systems language. v1.1 added real hardware acceleration. v2.0 makes Fajar Lang the ONLY language that can:

1. **Verify at compile time** that a kernel function never overflows a page table
2. **Prove** that a tensor reshape preserves element count via dependent types
3. **Guarantee** that a hardware resource is used exactly once via linear types
4. **Compile itself** end-to-end in its own language (full self-hosting)

No existing language combines formal verification + ML + bare metal in a single type system.

---

## Phase Overview

| Phase | Name | Sprints | Tasks | Focus |
|-------|------|---------|-------|-------|
| 1 | Dependent Types | S1-S4 | 40 | Type-level integers, Tensor<N,M>, compile-time shape verification |
| 2 | Linear Types | S5-S8 | 40 | Exactly-once resource usage, affine-to-linear upgrade, hardware handles |
| 3 | Formal Verification | S9-S12 | 40 | @verified annotation, pre/post conditions, SMT solver integration |
| 4 | Tiered JIT | S13-S16 | 40 | Interpreter-to-baseline JIT-to-optimizing JIT, profile-guided tier promotion |
| 5 | Effect System v2 | S17-S20 | 40 | First-class effects with type inference, effect polymorphism, handler composition |
| 6 | GC Mode | S21-S24 | 40 | --gc flag for prototyping, reference counting-to-tracing GC, non-embedded mode |
| 7 | Self-Hosting v2 | S25-S28 | 40 | Full compiler in .fj (codegen, analyzer), bootstrap chain, reproducibility |
| 8 | Language Server v2 | S29-S32 | 40 | Rust Analyzer-level: trait resolution, macro expansion, type-driven completion |

**Total: 32 sprints, 320 tasks**

---

## Phase 1: Dependent Types (S1-S4)

> Type-level computation — prove tensor shapes, array bounds, and numeric constraints at compile time.

### Sprint S1 — Type-Level Integers

- [x] S1.1 — Nat Kind in Type System: Introduce `Nat` kind representing compile-time natural numbers, distinct from runtime integers
- [x] S1.2 — Const Generic Syntax: Parse `fn foo<const N: usize>()` and `struct Array<T, const N: usize>` in parser
- [x] S1.3 — Const Generic AST Node: Add `ConstGenericParam { name, ty }` to AST GenericParam enum
- [x] S1.4 — Type-Level Literal: Resolve integer literals in type position (`Array<i32, 4>`) to Nat values during analysis
- [x] S1.5 — Type Arithmetic Addition: Implement `N + M` at type level, evaluating to concrete Nat when both operands are known
- [x] S1.6 — Type Arithmetic Multiplication: Implement `N * M` at type level for shape computation (reshape, flatten)
- [x] S1.7 — Type-Level Equality: Implement `N == M` constraint checking, emit SE-level error when Nat values mismatch
- [x] S1.8 — Const Generic Monomorphization: Extend monomorphization pass to substitute Nat values into const generic positions
- [x] S1.9 — Cranelift Lowering: Lower const generic values to immediate constants in native codegen, no runtime overhead
- [x] S1.10 — Unit Tests: 15+ tests for const generic parsing, Nat arithmetic, monomorphization, and type mismatch errors

### Sprint S2 — Dependent Arrays

- [x] S2.1 — Array<T, N> Type: Define built-in `Array<T, const N: usize>` with compile-time length tracking in type system
- [x] S2.2 — Array Literal Inference: Infer `N` from array literal length (`let a: Array<i32, _> = [1, 2, 3]` resolves N=3)
- [x] S2.3 — Bounds Check Elimination: When index is a const < N, elide runtime bounds check in both interpreter and codegen
- [x] S2.4 — Length Propagation: `fn concat<T, const A: usize, const B: usize>(x: Array<T,A>, y: Array<T,B>) -> Array<T, A+B>`
- [x] S2.5 — Slice-to-Array Conversion: `slice.try_into_array::<N>()` returns `Result<Array<T,N>, LengthError>` with runtime check
- [x] S2.6 — Fixed-Size Window: `array.windows::<W>()` yields `Array<T, W>` views with compile-time window size validation
- [x] S2.7 — Split at Index: `array.split_at::<K>() -> (Array<T,K>, Array<T, N-K>)` with compile-time subtraction
- [x] S2.8 — Type Error Messages: Emit clear diagnostics on length mismatch (`expected Array<i32, 4>, found Array<i32, 3>`)
- [x] S2.9 — Interop with Vec: `Array<T,N>.to_vec()` and `Vec<T>.try_into_array::<N>()` bridging functions
- [x] S2.10 — Unit Tests: 15+ tests covering literal inference, bounds elimination, length propagation, split/concat types

### Sprint S3 — Tensor Shape Types

- [x] S3.1 — Tensor<N,M> Type: Extend tensor type to carry compile-time dimensions `Tensor<const ROWS: usize, const COLS: usize>`
- [x] S3.2 — Shape Inference from Construction: `zeros(3, 4)` infers `Tensor<3, 4>`, `ones(5, 5)` infers `Tensor<5, 5>`
- [x] S3.3 — Matmul Shape Checking: Enforce `Tensor<A,B> * Tensor<B,C> -> Tensor<A,C>`, error if inner dimensions mismatch
- [x] S3.4 — Transpose Shape Flip: `Tensor<A,B>.transpose() -> Tensor<B,A>` verified at compile time
- [x] S3.5 — Reshape Validation: `Tensor<A,B>.reshape::<C,D>()` requires compile-time proof that `A*B == C*D`
- [x] S3.6 — Flatten Type: `Tensor<A,B>.flatten() -> Tensor<1, A*B>` using type-level multiplication
- [x] S3.7 — Broadcast Rules: Compile-time broadcast compatibility check for element-wise ops on mismatched shapes
- [x] S3.8 — Higher-Rank Tensors: Extend to `Tensor<D1, D2, D3, ...>` via variadic const generics (up to rank 4)
- [x] S3.9 — Shape Error Diagnostics: Rich error messages showing expected vs actual dimensions with operation context
- [x] S3.10 — Unit Tests: 20+ tests proving matmul compatibility, reshape validation, transpose, broadcast, and rank-N shapes

### Sprint S4 — Dependent Pattern Matching

- [x] S4.1 — Type-Level Value Patterns: Match on Nat values in type position (`match N { 0 => ..., 1 => ..., _ => ... }`)
- [x] S4.2 — Exhaustiveness with Nats: Verify exhaustive coverage for bounded Nat ranges in match expressions
- [x] S4.3 — Proof Witnesses: Generate compile-time witness values proving a constraint holds (e.g., `N > 0`)
- [x] S4.4 — Type-Safe Indexing: `array.get::<I>()` where `I < N` is proven at compile time, returns `T` not `Option<T>`
- [x] S4.5 — Dependent If-Else: `if N == 0 { ... } else { ... }` where branches have different return types based on Nat value
- [x] S4.6 — Where Clauses on Nats: `fn foo<const N: usize>() where N > 0` constraint syntax, checked at instantiation
- [x] S4.7 — Nat Range Type: `Nat<1..=10>` bounded natural number type for constrained generic parameters
- [x] S4.8 — Inductive Proofs: Support simple inductive reasoning (`if N works, N+1 works`) for recursive functions
- [x] S4.9 — Dependent Return Types: Function return type can depend on const generic parameter value
- [x] S4.10 — Unit Tests: 15+ tests for exhaustiveness, proof witnesses, safe indexing, where clauses, and dependent returns

---

## Phase 2: Linear Types (S5-S8)

> Exactly-once resource usage — guarantee that every hardware handle, file descriptor, and DMA buffer is consumed, never leaked or duplicated.

### Sprint S5 — Linearity Checker

- [x] S5.1 — Linear Annotation Syntax: Parse `linear` qualifier on types (`let handle: linear FileHandle = open("f")`)
- [x] S5.2 — AST Representation: Add `Linearity` enum (Linear, Affine, Unrestricted) to type system TypeExpr
- [x] S5.3 — Usage Tracking: Track each linear binding's usage count in analyzer — must be exactly 1
- [x] S5.4 — Unused Linear Error: Emit LE-level error when linear value goes out of scope without being consumed
- [x] S5.5 — Duplicate Linear Error: Emit LE-level error when linear value is used more than once (copy/clone forbidden)
- [x] S5.6 — Consume Syntax: Define `consume(handle)` built-in that explicitly destroys a linear value, returning inner data
- [x] S5.7 — Linear in Control Flow: Verify linearity across all branches (if/else must consume in both, match in all arms)
- [x] S5.8 — Linear in Loops: Forbid linear values inside loops unless consumed on every iteration with fresh rebinding
- [x] S5.9 — Linear Function Parameters: Linear params must be consumed in function body, not returned unconsumed
- [x] S5.10 — Unit Tests: 20+ tests for exact-once tracking, unused error, duplicate error, control flow, loop rejection

### Sprint S6 — Resource Handles

- [x] S6.1 — FileHandle Type: Define `linear struct FileHandle { fd: i32 }` with `open()` -> FileHandle, `close(FileHandle)` -> ()
- [x] S6.2 — GpuBuffer Type: Define `linear struct GpuBuffer { ptr: *mut u8, size: usize, device: i32 }` for GPU memory
- [x] S6.3 — MigPartition Type: Define `linear struct MigPartition { id: u32, gpu: i32 }` for NVIDIA MIG slices
- [x] S6.4 — Must-Use Enforcement: Compiler error if a function returning a linear type has its result discarded
- [x] S6.5 — Linear Drop Trait: `trait LinearDrop { fn finalize(self) }` called instead of Drop for linear types
- [x] S6.6 — Resource Leak Detection: At function exit, verify all linear locals have been consumed or returned
- [x] S6.7 — Transfer Semantics: Linear values can be moved into function calls (transferred, not copied)
- [x] S6.8 — Linear Struct Fields: Structs containing linear fields are themselves linear (linearity propagation)
- [x] S6.9 — Linear Enums: Enum variants containing linear data make the entire enum linear
- [x] S6.10 — Unit Tests: 15+ tests for FileHandle lifecycle, GpuBuffer single-use, must-use, leak detection, propagation

### Sprint S7 — Borrowing Bridge

- [x] S7.1 — Temporary Borrow from Linear: `&linear_val` creates a temporary immutable borrow, linear value still consumed later
- [x] S7.2 — Borrow Scope Rules: Borrow from linear must not outlive the linear value's consumption point
- [x] S7.3 — Affine-to-Linear Promotion: Existing affine (move) types can be promoted to linear via `as linear` cast
- [x] S7.4 — Linear-to-Affine Demotion: `unsafe { demote(linear_val) }` converts linear to affine (opt-out, @unsafe only)
- [x] S7.5 — Linear-Safe Closures: Closures capturing linear values must consume them exactly once; FnOnce enforcement
- [x] S7.6 — Linear in Generics: `fn process<T: Linear>(val: T)` generic constraint for linear type bounds
- [x] S7.7 — Linear References: `&linear T` reference type that does not consume but restricts to read-only access
- [x] S7.8 — Reborrowing Rules: Cannot reborrow a linear reference — single borrow chain only
- [x] S7.9 — Linear + Ownership Interplay: Define precedence rules when linear meets existing ownership/borrow checker
- [x] S7.10 — Unit Tests: 15+ tests for temporary borrow, closure capture, generic bounds, promotion/demotion, reborrow rejection

### Sprint S8 — Hardware Linear Safety

- [x] S8.1 — GPIO Pin Linearity: `linear struct GpioPin<const N: u8>` — each physical pin is a unique linear resource
- [x] S8.2 — Pin State Machine: GPIO transitions (Input -> Output -> Alternate) consume old state, produce new linear value
- [x] S8.3 — DMA Buffer Linearity: `linear struct DmaBuffer { phys_addr: usize, len: usize }` — exactly one owner at a time
- [x] S8.4 — DMA Transfer Protocol: `dma_start(consume buf: DmaBuffer) -> DmaFuture`, buffer reclaimed on completion
- [x] S8.5 — IRQ Handler Registration: `linear struct IrqRegistration { irq: u8 }` — must unregister before drop
- [x] S8.6 — MMIO Region Linearity: `linear struct MmioRegion { base: usize, size: usize }` — exclusive hardware access
- [x] S8.7 — Clock Gate Handle: `linear struct ClockGate { peripheral: u8 }` — enable/disable must be paired
- [x] S8.8 — Power Domain: `linear struct PowerDomain { id: u8 }` — power on/off lifecycle tracked by type system
- [x] S8.9 — @kernel Linear Integration: Linear types interact correctly with @kernel context restrictions
- [x] S8.10 — Unit Tests: 15+ tests for GPIO state machine, DMA lifecycle, IRQ registration, MMIO exclusivity, @kernel interop

---

## Phase 3: Formal Verification (S9-S12)

> Prove program properties at compile time — no buffer overflows, no integer overflow, no out-of-bounds access, guaranteed by the compiler.

### Sprint S9 — Pre/Post Conditions

- [x] S9.1 — Requires Syntax: Parse `requires(expr)` annotation before function body, attach to AST FnDecl node
- [x] S9.2 — Ensures Syntax: Parse `ensures(result, expr)` annotation after return type, bind `result` to return value
- [x] S9.3 — Invariant Syntax: Parse `invariant(expr)` annotation on loop constructs (while, for, loop)
- [x] S9.4 — Assert Distinction: Differentiate `assert(x)` (runtime) from `requires(x)` (contract) in analyzer
- [x] S9.5 — Runtime Fallback: When verification is disabled (`--no-verify`), lower requires/ensures to runtime assertions
- [x] S9.6 — Contract Inheritance: Trait method contracts propagate to impl methods — impl must satisfy trait's contracts
- [x] S9.7 — Old Value Capture: `ensures(result > old(x))` captures parameter value at function entry for postcondition
- [x] S9.8 — Multiple Contracts: Allow stacking multiple requires/ensures on a single function, all must hold
- [x] S9.9 — Contract Error Codes: Define VE001-VE008 (VerificationError) for contract violations with miette diagnostics
- [x] S9.10 — Unit Tests: 15+ tests for parsing, runtime fallback, contract inheritance, old() capture, error reporting

### Sprint S10 — SMT Integration

- [x] S10.1 — Z3 Bindings: Integrate z3-sys crate, create safe Rust wrapper for Z3 context, solver, and assertions
- [x] S10.2 — CVC5 Alternative: Implement CVC5 backend as alternative solver, selectable via `--smt-solver=cvc5`
- [x] S10.3 — Expression Encoding: Translate Fajar Lang expressions (arithmetic, comparison, boolean) to SMT-LIB format
- [x] S10.4 — Integer Theory: Encode integer operations with overflow semantics (wrapping, saturating, checked) in QF_BV
- [x] S10.5 — Array Theory: Encode array access patterns using SMT array theory for bounds verification
- [x] S10.6 — Sat/Unsat Reporting: Map solver result to compiler diagnostic — sat = property holds, unsat = violation found
- [x] S10.7 — Counterexample Extraction: When verification fails, extract concrete counterexample values from SMT model
- [x] S10.8 — Timeout Configuration: Set solver timeout (default 5s per function), report "unknown" on timeout
- [x] S10.9 — Incremental Solving: Use solver push/pop for checking multiple conditions without full re-encoding
- [x] S10.10 — Unit Tests: 15+ tests for expression encoding, integer arithmetic proofs, array bounds, counterexamples

### Sprint S11 — @verified Functions

- [x] S11.1 — @verified Annotation: Parse `@verified fn ...` context annotation, enable static verification for function
- [x] S11.2 — Verification Pipeline: After type checking, run SMT verification pass on @verified functions only
- [x] S11.3 — Automatic Bounds Proof: For @verified functions, prove all array accesses are within bounds without runtime checks
- [x] S11.4 — Overflow Proof: Prove arithmetic operations in @verified functions cannot overflow for given preconditions
- [x] S11.5 — Null Safety Proof: Prove all Option unwraps in @verified functions are preceded by Some checks
- [x] S11.6 — Loop Termination Hints: `@verified fn` with loops requires explicit `decreases(expr)` for termination argument
- [x] S11.7 — Verification Cache: Cache successful verification results keyed by function hash, skip re-verification
- [x] S11.8 — Partial Verification: If only some conditions are provable, report proved/unproved separately
- [x] S11.9 — Verification Report: `fj verify src.fj` outputs human-readable report of all @verified function proofs
- [x] S11.10 — Unit Tests: 15+ tests for full verification pipeline, bounds proofs, overflow proofs, cache hits, partial results

### Sprint S12 — Kernel Verification

- [x] S12.1 — @kernel + @verified Composition: Allow `@kernel @verified fn` for maximum safety — context + formal proof
- [x] S12.2 — Page Table Bounds: Prove @kernel @verified functions never index beyond page table entry count (512 for 4-level)
- [x] S12.3 — Stack Depth Proof: Verify @kernel functions have bounded recursion depth, proving no stack overflow
- [x] S12.4 — Memory Region Safety: Prove MMIO reads/writes stay within declared region bounds
- [x] S12.5 — IRQ Latency Bound: Verify @kernel IRQ handlers have bounded execution time (no unbounded loops)
- [x] S12.6 — Allocation-Free Proof: Statically verify @kernel @verified functions perform zero heap allocations
- [x] S12.7 — Register Preservation: Prove that @kernel functions save/restore callee-saved registers correctly
- [x] S12.8 — Interrupt Safety: Verify that @kernel functions called from interrupt context are reentrant-safe
- [x] S12.9 — Cross-Context Verification: Verify @safe bridge functions correctly mediate between @kernel and @device
- [x] S12.10 — Unit Tests: 15+ tests for page table bounds, stack depth, memory regions, IRQ latency, allocation-free proof

---

## Phase 4: Tiered JIT (S13-S16)

> Adaptive execution — start interpreting immediately, compile hot paths to machine code progressively.

### Sprint S13 — Execution Counter

- [x] S13.1 — Per-Function Counter: Add `call_count: AtomicU64` to each function metadata in the interpreter environment
- [x] S13.2 — Counter Increment: Increment counter on every function entry in both interpreter and bytecode VM
- [x] S13.3 — Hot Function Threshold: Define configurable threshold (default: 100 calls) for baseline JIT promotion
- [x] S13.4 — Super-Hot Threshold: Define second threshold (default: 10,000 calls) for optimizing JIT promotion
- [x] S13.5 — Loop Back-Edge Counter: Count loop back-edge executions for hot loop detection independent of function calls
- [x] S13.6 — Sampling Profiler: Lightweight sampling profiler (1ms interval) recording current function at each tick
- [x] S13.7 — Call Graph Recording: Record caller-callee pairs for inlining decisions in optimizing tier
- [x] S13.8 — Type Profiling: Record observed argument types at call sites for speculative optimization
- [x] S13.9 — CLI Configuration: `fj run --jit-threshold=200 --opt-threshold=5000` for tuning tier promotion
- [x] S13.10 — Unit Tests: 12+ tests for counter accuracy, threshold detection, loop counting, type profiling recording

### Sprint S14 — Baseline JIT

- [x] S14.1 — Baseline Compiler Entry: When function reaches hot threshold, trigger baseline JIT compilation
- [x] S14.2 — Fast IR Translation: Translate AST directly to Cranelift IR without optimization passes (sub-millisecond)
- [x] S14.3 — No Optimization: Skip all optimizations (no inlining, no CSE, no DCE) — compile speed is the priority
- [x] S14.4 — Simple Register Allocation: Use Cranelift's fast regalloc mode for minimal register allocation overhead
- [x] S14.5 — Code Patching: Patch interpreter call sites to jump directly to JIT-compiled code on subsequent calls
- [x] S14.6 — Baseline Code Cache: Store compiled code in memory-mapped executable pages, keyed by function identity
- [x] S14.7 — Deoptimization Hooks: Embed deopt points in baseline code for falling back to interpreter when needed
- [x] S14.8 — Stack Frame Compatibility: Baseline JIT frames must be walkable by the interpreter for mixed-mode execution
- [x] S14.9 — Compilation Metrics: Track time-to-first-execution, compilation latency, code size per baseline function
- [x] S14.10 — Unit Tests: 15+ tests for baseline compilation correctness, sub-ms compile time, cache lookup, deopt hooks

### Sprint S15 — Optimizing JIT

- [x] S15.1 — Optimizing Compiler Entry: When baseline-compiled function reaches super-hot threshold, trigger opt compilation
- [x] S15.2 — Inlining Pass: Inline small functions (< 30 IR instructions) at call sites based on call graph data
- [x] S15.3 — CSE in JIT: Common subexpression elimination across basic blocks within hot functions
- [x] S15.4 — DCE in JIT: Dead code elimination removing unreachable branches based on type profile data
- [x] S15.5 — LICM in JIT: Loop-invariant code motion for hot loops, hoisting constants and invariant computations
- [x] S15.6 — Speculative Optimization: Use type profiles to specialize for observed types, insert type guards
- [x] S15.7 — Guard Failure Handling: On type guard failure, deoptimize to baseline JIT code (not interpreter)
- [x] S15.8 — Code Replacement: Atomically replace baseline code pointer with optimized code (no execution gap)
- [x] S15.9 — Optimization Metrics: Track speedup ratio (optimized vs baseline), compilation time, code size delta
- [x] S15.10 — Unit Tests: 15+ tests for inlining correctness, speculative optimization, guard failure, atomic replacement

### Sprint S16 — On-Stack Replacement

- [x] S16.1 — OSR Entry Points: Identify loop headers as valid OSR entry points in both interpreter and baseline code
- [x] S16.2 — State Capture: Capture local variable state at OSR point — interpreter locals to JIT register mapping
- [x] S16.3 — Mid-Loop Transition: Transfer execution from interpreter to JIT code mid-loop without restarting the loop
- [x] S16.4 — OSR Frame Construction: Build JIT stack frame from captured interpreter state, resume at correct IP
- [x] S16.5 — Deoptimization (JIT to Interpreter): On speculation failure, reconstruct interpreter frame from JIT state
- [x] S16.6 — Deopt Metadata: Embed per-OSR-point metadata mapping JIT registers back to interpreter local slots
- [x] S16.7 — Nested Loop OSR: Handle OSR for nested loops — inner loop promoted independently of outer loop
- [x] S16.8 — OSR Threshold: Trigger OSR when loop back-edge count exceeds threshold within a single invocation
- [x] S16.9 — Performance Validation: Benchmark long-running loops — OSR must show speedup within 100 iterations
- [x] S16.10 — Unit Tests: 15+ tests for mid-loop transition correctness, state capture, deoptimization, nested loops

---

## Phase 5: Effect System v2 (S17-S20)

> First-class algebraic effects — track, compose, and handle side effects in the type system.

### Sprint S17 — Effect Inference

- [x] S17.1 — Effect Annotation Syntax: Parse `fn foo() -> i32 with IO, Alloc` effect list on function signatures
- [x] S17.2 — Effect Set Type: Define `EffectSet` as a set of effect labels, attached to function types in the type system
- [x] S17.3 — Automatic Inference: Infer effects from function body — `print()` implies `IO`, `alloc()` implies `Alloc`
- [x] S17.4 — Effect Propagation: Calling a function with effects `{IO}` adds `IO` to the caller's effect set
- [x] S17.5 — Effect Annotation Optional: When effects are fully inferable, annotation is optional (inferred from body)
- [x] S17.6 — Effect Mismatch Error: Error when annotated effects are narrower than inferred effects (missing effect)
- [x] S17.7 — Pure Functions: Functions with empty effect set (`with {}` or inferred) are guaranteed pure — no side effects
- [x] S17.8 — Built-in Effects: Define standard effects: `IO`, `Alloc`, `Panic`, `Async`, `Unsafe`, `Network`, `FileSystem`
- [x] S17.9 — Effect Display: Show inferred effects in error messages and `fj check` output for developer visibility
- [x] S17.10 — Unit Tests: 15+ tests for inference accuracy, propagation chains, mismatch errors, pure function detection

### Sprint S18 — Effect Polymorphism

- [x] S18.1 — Effect Variables: Parse `fn map<T, U, eff E>(f: fn(T) -> U with E, xs: [T]) -> [U] with E` syntax
- [x] S18.2 — Effect Bounds: `fn foo<eff E: IO + Alloc>()` constraining effect variables to include specific effects
- [x] S18.3 — Effect Unification: Unify effect variables during type inference — `E` resolved to concrete effect set at call site
- [x] S18.4 — Higher-Order Effects: Functions taking effectful callbacks correctly propagate callback's effects to caller
- [x] S18.5 — Effect Subtyping: `{IO} <: {IO, Alloc}` — a function with fewer effects is substitutable for one with more
- [x] S18.6 — Effect Row Polymorphism: `fn foo<eff E>(f: fn() -> T with {IO | E}) -> T with {IO | E}` open rows
- [x] S18.7 — Effect Instantiation: At monomorphization, substitute concrete effect sets for effect variables
- [x] S18.8 — Effect Constraints in Traits: `trait Pure { fn compute(&self) -> i32 with {} }` enforcing purity in trait methods
- [x] S18.9 — Effect Variance: Covariant effect sets in return position, contravariant in argument position
- [x] S18.10 — Unit Tests: 15+ tests for polymorphic effects, bounds, unification, higher-order, subtyping, row polymorphism

### Sprint S19 — Handler Composition

- [x] S19.1 — Effect Handler Syntax: Parse `handle expr { effect Op(args) -> resume(value) }` handler blocks
- [x] S19.2 — Resume Continuation: `resume(value)` continues the effectful computation with the provided value
- [x] S19.3 — Handler Semantics: Handler intercepts effect operations, decides how to resume or abort computation
- [x] S19.4 — Nested Handlers: Multiple handlers can be nested, inner handler has priority for matching effects
- [x] S19.5 — Handler Composition: `handler1 >> handler2` composes two handlers, effects flow through both
- [x] S19.6 — Effect Tunneling: Unhandled effects pass through to outer handlers without explicit forwarding
- [x] S19.7 — State Effect: Implement `State<S>` effect with `get() -> S` and `set(S)` operations as library pattern
- [x] S19.8 — Exception Effect: Implement `Exception<E>` effect with `raise(E)` as alternative to Result/panic
- [x] S19.9 — Handler Return Type: Handler block has its own return type, may differ from the handled computation's type
- [x] S19.10 — Unit Tests: 15+ tests for handler matching, resume semantics, nesting, composition, tunneling, state/exception

### Sprint S20 — Effect Interop

- [x] S20.1 — Effects + Async: Map `Async` effect to existing async/await machinery — await is an effect operation
- [x] S20.2 — Effects + @kernel: @kernel context implies `with {!IO, !Alloc}` — absence of IO and Alloc effects
- [x] S20.3 — Effects + @device: @device context implies `with {Tensor, !Unsafe}` — tensor ops allowed, unsafe forbidden
- [x] S20.4 — Effects + Linear Types: Linear values in effect handlers must be consumed exactly once across resume paths
- [x] S20.5 — Effect Erasure: At native codegen, erase effect types — zero runtime overhead for effect tracking
- [x] S20.6 — Effect-Guided Optimization: Pure functions (empty effect set) eligible for aggressive CSE, reordering, memoization
- [x] S20.7 — Effect Documentation: Effects shown in `cargo doc` output, `fj check` reports, and LSP hover information
- [x] S20.8 — Migration Guide: Document how to add effect annotations to existing code incrementally (backward compatible)
- [x] S20.9 — Standard Library Effects: Annotate all stdlib functions with their effect sets (IO, Alloc, Panic, etc.)
- [x] S20.10 — Unit Tests: 15+ tests for async interop, context annotation mapping, linear interaction, erasure, optimization

---

## Phase 6: GC Mode (S21-S24)

> Optional garbage collection for rapid prototyping — flip a flag to trade embedded safety for development velocity.

### Sprint S21 — Reference Counting

- [x] S21.1 — Rc<T> Type: Implement reference-counted pointer type `Rc<T>` with `strong_count` and `weak_count` fields
- [x] S21.2 — Rc Clone Semantics: `Rc::clone(&rc)` increments reference count, returns new handle to same allocation
- [x] S21.3 — Rc Drop: Decrement reference count on drop, deallocate inner value when count reaches zero
- [x] S21.4 — Weak<T> Type: Non-owning weak reference that does not prevent deallocation, `upgrade()` returns Option<Rc<T>>
- [x] S21.5 — Cycle Detection: Implement cycle collector triggered periodically — trace Rc graph, break cycles via Weak
- [x] S21.6 — Rc in Type System: Add `Rc<T>` as built-in generic type, auto-deref for method calls on inner T
- [x] S21.7 — Interior Mutability: `Rc<RefCell<T>>` pattern for shared mutable state under GC mode
- [x] S21.8 — Rc Thread Safety: `Rc<T>` is `!Send` — for multi-threaded GC, provide `Arc<T>` with atomic refcount
- [x] S21.9 — GC Statistics: Track total Rc allocations, current live count, cycle collections performed
- [x] S21.10 — Unit Tests: 15+ tests for refcount lifecycle, weak upgrade/expire, cycle detection, thread safety rejection

### Sprint S22 — Tracing GC

- [x] S22.1 — Mark Phase: Implement mark phase traversing from root set (stack, globals) marking reachable objects
- [x] S22.2 — Sweep Phase: Iterate all allocations, free unmarked objects, reset marks for next cycle
- [x] S22.3 — GC Root Registration: Register stack frames and global variables as GC roots automatically
- [x] S22.4 — Generational Collection: Young generation (frequent, fast) and old generation (rare, full) collection
- [x] S22.5 — Write Barrier: Track cross-generation pointers with a write barrier on reference assignment
- [x] S22.6 — Concurrent Marking: Mark phase runs concurrently with mutator using tri-color marking (white/gray/black)
- [x] S22.7 — GC Pause Budget: Configure max GC pause time (default 1ms), incremental collection within budget
- [x] S22.8 — Heap Sizing: Auto-resize heap — grow when occupancy > 75%, shrink when < 25% after full GC
- [x] S22.9 — Finalization: Run finalizer callbacks before reclaiming objects that registered destructors
- [x] S22.10 — Unit Tests: 15+ tests for mark correctness, sweep completeness, generational promotion, concurrent safety

### Sprint S23 — GC Integration

- [x] S23.1 — --gc Compiler Flag: `fj run --gc program.fj` enables GC mode, `--no-gc` (default) uses ownership
- [x] S23.2 — Automatic Rc Insertion: In GC mode, compiler wraps all heap allocations in Rc<T> automatically
- [x] S23.3 — Ownership System Bypass: In GC mode, move/borrow checker is relaxed — values can be freely shared
- [x] S23.4 — @kernel GC Prohibition: @kernel context always forbids GC regardless of --gc flag (embedded safety)
- [x] S23.5 — Mixed-Mode Modules: Allow `@gc mod prototyping { ... }` alongside non-GC modules in same project
- [x] S23.6 — GC-to-Owned Migration: `fj migrate --remove-gc src.fj` tool that adds explicit ownership annotations
- [x] S23.7 — GC Mode Warnings: Warn when GC mode code calls non-GC functions expecting ownership semantics
- [x] S23.8 — Performance Mode Switch: Same source compiles to GC (development) or owned (production) with flag
- [x] S23.9 — GC Mode in REPL: REPL defaults to GC mode for interactive convenience, `--no-gc` for strict mode
- [x] S23.10 — Unit Tests: 15+ tests for flag parsing, auto-Rc insertion, @kernel prohibition, mixed-mode, migration tool

### Sprint S24 — GC Benchmarks

- [x] S24.1 — Throughput Benchmark: Measure operations/second for identical workload under GC vs ownership mode
- [x] S24.2 — Latency Benchmark: Measure p50/p99 response time for request-processing workload under both modes
- [x] S24.3 — Pause Time Benchmark: Record GC pause distribution (min, max, p50, p99) across sustained workload
- [x] S24.4 — Memory Overhead: Measure peak memory usage ratio (GC / ownership) for identical programs
- [x] S24.5 — Collection Frequency: Profile collection events per second under various allocation rates
- [x] S24.6 — Generational Effectiveness: Measure young-gen vs old-gen collection rates, promotion frequency
- [x] S24.7 — Comparison with Rust: Benchmark equivalent program in Rust (no GC) and Fajar Lang (both modes)
- [x] S24.8 — Comparison with Go: Benchmark equivalent program in Go (GC) and Fajar Lang GC mode
- [x] S24.9 — Benchmark Report: Generate criterion-style HTML report with graphs for all GC benchmarks
- [x] S24.10 — Unit Tests: 10+ tests for benchmark harness correctness, metric collection accuracy, report generation

---

## Phase 7: Self-Hosting v2 (S25-S28)

> The compiler compiles itself — full bootstrap chain from Rust to Fajar Lang to self-compiled Fajar Lang.

### Sprint S25 — Analyzer in .fj

- [x] S25.1 — Type Checker Core: Implement `fn type_check(program: Program) -> Result<(), Vec<Error>>` in Fajar Lang
- [x] S25.2 — Scope Resolution: Implement symbol table with nested scopes, variable lookup, and shadowing in .fj
- [x] S25.3 — Type Unification: Implement Hindley-Milner-style type unification for generic type inference in .fj
- [x] S25.4 — Borrow Checker: Implement move tracking and borrow analysis using NLL-style control flow in .fj
- [x] S25.5 — Context Checker: Implement @kernel/@device/@safe context validation logic in Fajar Lang
- [x] S25.6 — Error Collection: Collect all semantic errors with spans and error codes, matching Rust analyzer output
- [x] S25.7 — Trait Resolution: Implement trait impl lookup, method resolution, and blanket impl handling in .fj
- [x] S25.8 — Const Evaluation: Implement compile-time constant expression evaluation in Fajar Lang
- [x] S25.9 — Cross-Validation: Run both Rust and .fj analyzers on test suite, verify identical error output
- [x] S25.10 — Unit Tests: 20+ tests comparing Rust analyzer output with .fj analyzer output for identical programs

### Sprint S26 — Codegen in .fj

- [x] S26.1 — Cranelift IR Builder: Implement Cranelift IR generation from Fajar Lang AST, written in .fj
- [x] S26.2 — Function Compilation: Compile function declarations to Cranelift functions with correct ABI in .fj
- [x] S26.3 — Expression Lowering: Lower all expression types (binary, unary, call, field access, index) to IR in .fj
- [x] S26.4 — Control Flow Lowering: Lower if/else, while, for, loop, match to Cranelift basic blocks in .fj
- [x] S26.5 — Type Mapping: Map Fajar Lang types to Cranelift types (i8-i128, f32, f64, pointer) in .fj
- [x] S26.6 — Runtime Function Calls: Generate calls to fj_rt_* runtime functions from .fj codegen
- [x] S26.7 — String Operations: Generate string allocation, concatenation, and comparison code in .fj
- [x] S26.8 — Struct Layout: Compute struct field offsets and generate struct access code in .fj
- [x] S26.9 — Object File Emission: Generate ELF/Mach-O object files from Cranelift module in .fj
- [x] S26.10 — Unit Tests: 20+ tests comparing native codegen output between Rust and .fj compiler backends

### Sprint S27 — Bootstrap Chain

- [x] S27.1 — Stage 0 (Rust): Existing Rust compiler produces `fj-stage0` binary from `cargo build --release`
- [x] S27.2 — Stage 1 (fj-compiled): Use `fj-stage0` to compile the .fj compiler source, producing `fj-stage1`
- [x] S27.3 — Stage 2 (self-compiled): Use `fj-stage1` to compile the .fj compiler source again, producing `fj-stage2`
- [x] S27.4 — Binary Comparison: Verify `fj-stage1` and `fj-stage2` produce identical output (byte-for-byte)
- [x] S27.5 — Test Suite Validation: Run full test suite through `fj-stage1` — all 3,000+ tests must pass
- [x] S27.6 — Performance Comparison: Benchmark `fj-stage0` vs `fj-stage1` — self-compiled within 2x of Rust-compiled
- [x] S27.7 — Error Message Parity: Verify error messages from `fj-stage1` match `fj-stage0` exactly
- [x] S27.8 — Bootstrap Script: `scripts/bootstrap.sh` automates Stage 0 -> Stage 1 -> Stage 2 -> verify pipeline
- [x] S27.9 — CI Bootstrap: GitHub Actions job that runs full bootstrap chain on every PR to self-hosting code
- [x] S27.10 — Unit Tests: 10+ tests for bootstrap script correctness, binary hash comparison, test suite passthrough

### Sprint S28 — Reproducibility

- [x] S28.1 — Deterministic Compilation: Eliminate all sources of non-determinism (HashMap iteration, timestamps, addresses)
- [x] S28.2 — Source Hash Embedding: Embed SHA-256 of source files in compiled binary for provenance tracking
- [x] S28.3 — Compiler Version Embedding: Embed compiler version and git commit hash in binary metadata section
- [x] S28.4 — Cross-Platform Reproducibility: Same source produces identical binary on Linux x86_64, Linux ARM64, macOS ARM64
- [x] S28.5 — Reproducible Builds Spec: Document all inputs (source, compiler version, flags, target) that affect output
- [x] S28.6 — Binary Diff Tool: `fj diff binary1 binary2` shows section-by-section comparison for debugging mismatches
- [x] S28.7 — Build Cache: Content-addressable build cache keyed by (source_hash, compiler_hash, flags_hash, target)
- [x] S28.8 — Verification Script: `scripts/verify-reproducible.sh` builds twice, compares hashes, reports pass/fail
- [x] S28.9 — Third-Party Verification: Document process for independent parties to verify build reproducibility
- [x] S28.10 — Unit Tests: 12+ tests for determinism, hash embedding, cross-platform parity, cache hit/miss, diff tool

---

## Phase 8: Language Server v2 (S29-S32)

> Rust Analyzer-level IDE experience — trait resolution, macro expansion preview, type-driven completion, and refactoring.

### Sprint S29 — Trait Resolution

- [x] S29.1 — Full Trait Impl Index: Build index of all trait implementations in workspace, updated incrementally
- [x] S29.2 — Go-to-Implementation: Click on trait method call, navigate to concrete impl for known receiver type
- [x] S29.3 — Blanket Impl Display: Show applicable blanket impls in hover information (`impl<T: Display> ToString for T`)
- [x] S29.4 — Trait Bound Checking: Real-time display of unsatisfied trait bounds as user types generic constraints
- [x] S29.5 — Associated Type Resolution: Resolve associated types in hover (`<T as Iterator>::Item = i32`)
- [x] S29.6 — Trait Object Info: Display vtable layout and available methods for `dyn Trait` types on hover
- [x] S29.7 — Impl Suggestions: When trait bound is unsatisfied, suggest adding `impl TraitName for Type` with skeleton
- [x] S29.8 — Orphan Rule Checking: Real-time orphan rule validation for trait implementations as user types
- [x] S29.9 — Trait Hierarchy View: Tree visualization of trait inheritance (`Display: Debug + Serialize`) in sidebar
- [x] S29.10 — Unit Tests: 15+ tests for impl index accuracy, go-to-impl, blanket resolution, associated types, suggestions

### Sprint S30 — Macro Expansion

- [x] S30.1 — Macro System Design: Define declarative macro syntax `macro_rules! name { (pattern) => { expansion } }` in parser
- [x] S30.2 — Macro Expansion Engine: Implement pattern matching and template substitution for declarative macros
- [x] S30.3 — Expansion Preview: LSP command to show expanded macro output in a virtual document
- [x] S30.4 — Step-Through Expansion: Interactive step-by-step macro expansion visualization in IDE
- [x] S30.5 — Macro Hygiene: Implement hygienic macros — macro-generated identifiers do not clash with user identifiers
- [x] S30.6 — Error Locations: Map errors in expanded code back to original macro definition or invocation site
- [x] S30.7 — Macro Completion: Auto-complete macro invocations based on defined macro_rules patterns
- [x] S30.8 — Macro Documentation: Show macro documentation, pattern arms, and example expansions in hover
- [x] S30.9 — Recursive Macros: Support recursive macro expansion with configurable depth limit (default: 128)
- [x] S30.10 — Unit Tests: 15+ tests for expansion correctness, hygiene, error mapping, completion, recursive depth limit

### Sprint S31 — Type-Driven Completion

- [x] S31.1 — Expected Type Analysis: Determine expected type at cursor position from surrounding context
- [x] S31.2 — Expression Synthesis: Suggest complete expressions that produce the expected type (e.g., `Ok(value)` for Result)
- [x] S31.3 — Fill-in-the-Blank: When typing `let x: Vec<i32> = `, suggest `Vec::new()`, `vec![]`, `Vec::with_capacity()`
- [x] S31.4 — Argument Completion: In function call, suggest values of the correct parameter type from scope
- [x] S31.5 — Pattern Completion: In match arm, generate exhaustive pattern skeleton based on matched enum type
- [x] S31.6 — Import Suggestions: When completing unresolved name, suggest `use` imports for matching items
- [x] S31.7 — Postfix Completion: Type `.if` after boolean expr to get `if expr { }`, `.match` after enum for match block
- [x] S31.8 — Snippet Templates: Context-aware snippets (for loop over iterable, error handling boilerplate, test function)
- [x] S31.9 — Completion Ranking: Rank suggestions by relevance — type match > name similarity > recency > alphabetical
- [x] S31.10 — Unit Tests: 15+ tests for expected type analysis, expression synthesis, pattern completion, import suggestions

### Sprint S32 — Refactoring Suite

- [x] S32.1 — Extract Function: Select code region, extract into new function with correct parameters and return type
- [x] S32.2 — Extract Variable: Select expression, replace with `let` binding, maintain all other occurrences
- [x] S32.3 — Inline Function: Replace function call with function body, substituting arguments for parameters
- [x] S32.4 — Inline Variable: Replace all occurrences of a variable with its initializer expression
- [x] S32.5 — Rename Symbol: Rename variable/function/type/trait across entire workspace with preview
- [x] S32.6 — Move Module: Move module to different file/directory, update all `use` imports automatically
- [x] S32.7 — Extract Trait: Select methods from impl block, create new trait with those method signatures
- [x] S32.8 — Change Signature: Add/remove/reorder function parameters, update all call sites
- [x] S32.9 — Convert to/from Method: Convert free function to method (add self param) or method to free function
- [x] S32.10 — Unit Tests: 15+ tests for extract function/variable, inline, rename across files, move module, change signature

---

## Dependencies

```
Phase 1 (Dependent Types) ──────→ Phase 3 (Verification) uses Nat proofs
         │
         └──────────────────────→ Phase 5 (Effects) uses type-level values

Phase 2 (Linear Types) ─────────→ Phase 3 (Verification) proves linearity
         │
         └──────────────────────→ Phase 5 (Effects) linear + effects interop

Phase 3 (Verification)  ────────→ Phase 7 (Self-Hosting) verified bootstrap

Phase 4 (Tiered JIT)    ────────→ Phase 6 (GC Mode) GC cooperates with JIT

Phase 5 (Effects)        ────────→ Phase 8 (LSP v2) effect display in IDE

Phase 7 (Self-Hosting)  ────────→ Phase 8 (LSP v2) .fj analyzer powers LSP
```

**Parallelism opportunities:**
- Phases 1 and 2 can run in parallel (dependent types and linear types are independent until Phase 3)
- Phase 4 (Tiered JIT) is independent of Phases 1-3 and can start immediately
- Phase 6 (GC Mode) can start after Phase 4 or in parallel with Phases 1-3
- Phase 8 (LSP v2) can begin basic work (S29-S30) in parallel with Phase 7
- Phases 5 and 6 are mostly independent and can overlap

---

## Success Criteria

| Criterion | Target |
|-----------|--------|
| Tasks complete | 320/320 |
| Test suite | 5,000+ tests (0 failures) |
| Dependent type checker | Proves `Tensor<3,4> * Tensor<4,5> -> Tensor<3,5>` at compile time |
| Reshape verification | Compile-time proof that `Tensor<6,8>.reshape::<4,12>()` preserves 48 elements |
| Linear type checker | Rejects program that drops a `linear FileHandle` without calling `close()` |
| Linear GPIO | Proves GPIO pin is configured exactly once, never leaked or double-configured |
| SMT integration | Z3 proves @kernel function stays within page table bounds (512 entries) |
| Formal verification | @verified @kernel function compilation produces zero runtime bounds checks |
| Tiered JIT | Hot function promoted from interpreter to baseline JIT within 1ms compile time |
| OSR | Loop transitions from interpreter to JIT mid-execution with correct state transfer |
| Effect inference | Pure functions automatically detected, effects propagated through call chains |
| GC mode | `--gc` flag compiles same source with GC, `--no-gc` with ownership — both correct |
| Bootstrap chain | `fj-stage1` == `fj-stage2` byte-for-byte (self-hosting verified) |
| Reproducible builds | Same source, same flags, same target produces identical binary on 3 platforms |
| LSP trait resolution | Go-to-implementation works for all trait methods in workspace |
| Refactoring | Extract function correctly handles closures, generics, and lifetime parameters |
| LOC | ~150,000+ lines of Rust + ~20,000 lines of Fajar Lang (self-hosted components) |

---

## Release Gate

All of the following MUST pass before tagging v2.0.0:

```bash
# Code quality
cargo test                             # all pass
cargo test --features native           # all pass (including codegen)
cargo clippy -- -D warnings            # zero warnings
cargo fmt -- --check                   # clean

# Dependent types
# Tensor<A,B> * Tensor<B,C> -> Tensor<A,C> verified at compile time
# Array<T, N> bounds check elimination proven for const indices
# Reshape A*B == C*D constraint checked at compile time

# Linear types
# FileHandle, GpuBuffer, GpioPin enforce exactly-once usage
# Compiler rejects unused linear values and duplicate linear usage
# @kernel context correctly interacts with linear resource handles

# Formal verification
# Z3/CVC5 solver integration functional
# @verified functions produce zero runtime bounds checks
# @kernel @verified functions proven allocation-free and bounded

# Tiered JIT
# Baseline JIT compiles in < 1ms per function
# Optimizing JIT shows measurable speedup over baseline
# OSR correctly transfers state mid-loop

# Effect system
# Effect inference matches manual annotations on all stdlib functions
# Effect erasure produces zero runtime overhead in native codegen
# @kernel/@device effects correctly map to context restrictions

# GC mode
# --gc and --no-gc both compile and pass full test suite
# GC pause time < 1ms on benchmark suite
# @kernel code rejects GC regardless of flag

# Self-hosting
# Stage 0 -> Stage 1 -> Stage 2 bootstrap chain passes
# fj-stage1 and fj-stage2 are byte-for-byte identical
# All 5,000+ tests pass under fj-stage1

# Reproducibility
# Identical binary produced on Linux x86_64, Linux ARM64, macOS ARM64
# Source hash and compiler version embedded in binary

# LSP v2
# Trait resolution, macro expansion, type-driven completion all functional
# All 6 refactoring operations (extract, inline, rename, move, change sig, convert) working

# Phase verification
# All 8 phases verified (320/320 tasks marked [x])

# Documentation
# CHANGELOG.md updated with v2.0.0 entry
# Language specification updated with dependent types, linear types, effects
# Formal verification guide published
```

---

*V20_PLAN.md v1.0 | Created 2026-03-11*
