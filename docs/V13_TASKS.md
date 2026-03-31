# V13 "Beyond" — Implementation Tasks

> **Master Tracking Document** — All 710 tasks, organized for batch execution at production level.
> **Rule:** Work per-phase, per-sprint. Complete ALL tasks in a sprint before moving to the next.
> **Marking:** `[ ]` = pending, `[w]` = work-in-progress, `[x]` = done (end-to-end verified), `[f]` = framework only
> **Verify:** Every sprint ends with `cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check`
> **Plan:** `docs/V13_BEYOND_PLAN.md` — full context, rationale, and architecture for each option.
> **Previous:** V12 "Transcendence" — 600 tasks ALL COMPLETE, released as v10.0.0.

---

## Execution Order & Dependencies

```
PHASE 1 — FOUNDATION (must complete first, unblocks everything)
  Option A: CI Green ..................... 1 sprint,  10 tasks  (NO dependency)
  Option H: Const Fn + Compile-Time ..... 10 sprints, 100 tasks (depends on A)
  Option C: Incremental Compilation ..... 10 sprints, 100 tasks (depends on A)

PHASE 2 — ECOSYSTEM (expands targets and interop)
  Option B: WASI P2 + Component Model .. 10 sprints, 100 tasks (H improves W8)
  Option E: FFI v2 Full Integration ..... 10 sprints, 100 tasks (C improves E4)

PHASE 3 — DIFFERENTIATION (unique value, world-class)
  Option F: SMT Formal Verification ..... 10 sprints, 100 tasks (C improves V5)
  Option D: Distributed Runtime ......... 10 sprints, 100 tasks (E improves D5)
  Option G: Self-Hosting Compiler ....... 10 sprints, 100 tasks (H improves S4)

TOTAL: 71 sprints, 710 tasks, ~52,779 LOC, ~700 tests
```

### Batch Execution Protocol

Each sprint is ONE atomic batch. Instructions for Claude:

1. **READ** the sprint section below (tasks + verification)
2. **IMPLEMENT** all tasks in the sprint sequentially (they build on each other within a sprint)
3. **TEST** the entire sprint: `cargo test --lib && cargo clippy -- -D warnings`
4. **MARK** all tasks `[x]` only when verified end-to-end
5. **COMMIT** with message: `feat(v13): complete sprint [ID] — [summary]`
6. **MOVE** to next sprint. Do NOT go back to a completed sprint.

If a task fails verification, fix it IN THE SAME SPRINT before proceeding.

---

# ============================================================
# PHASE 1: FOUNDATION
# ============================================================

---

## Option A: CI Green — Cross-Platform Stability

**Goal:** All CI jobs green across all platforms, nightly + stable, all feature flags.
**Sprints:** 1 | **Tasks:** 10 | **LOC:** ~79
**Dependency:** None — do this FIRST.
**Status:** ALL 10 TASKS COMPLETE (2026-03-31)

### Sprint A1: CI Stability & Hardening

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| A1.1 | Fix nightly `collapsible_match` | [x] | Collapse `if` into match guard in `type_check/check.rs` | 5 | Nightly clippy passes |
| A1.2 | Fix `gen` keyword conflict | [x] | Rename `gen` variable in `n2_inode_generation` to `generation` | 5 | `cargo test --test eval_tests n2_inode_generation` passes |
| A1.3 | Fix LLVM `unused-mut` | [x] | Remove `mut` from `compiler` in `llvm/mod.rs:6995` | 1 | `cargo test --lib --features llvm` passes |
| A1.4 | Fix cpp-ffi `useless_format` | [x] | Replace `format!("{e}")` with `e.to_string()` in `ffi_v2/cpp.rs` | 1 | `cargo clippy --features cpp-ffi -- -D warnings` passes |
| A1.5 | Audit keyword-as-identifier | [x] | Grep for `let gen`, `let yield`, `let async` in tests + examples; rename all | 20 | No keyword conflicts in any .fj or test file |
| A1.6 | Pin nightly clippy allow-list | [x] | Add `#![allow()]` for known nightly-only lints with `// TODO: remove when stable` | 10 | Nightly + stable both pass clippy |
| A1.7 | Feature flag matrix test | [x] | Verify all 8 feature flags compile independently: `cargo check --features X` for each | 30 | All 8 feature flags compile |
| A1.8 | Coverage job fix | [x] | Ensure tarpaulin runs after test fixes | 5 | Codecov upload succeeds |
| A1.9 | Add CI badge to README | [x] | Add `![CI](https://github.com/.../badge.svg)` to README.md | 2 | Badge shows in GitHub |
| A1.10 | Verify full CI green | [x] | Push and confirm all 11 jobs pass | 0 | GitHub Actions: 11/11 green |

**Sprint A1 Gate:** `cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check` + all CI green.

---

## Option H: Const Fn + Compile-Time Eval

**Goal:** Full const generics, const trait bounds, compile-time allocation, const stdlib.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~6,000
**Dependency:** Option A complete (CI green).
**Existing:** 713 LOC, 13 tests (comptime blocks, arithmetic, control flow).

### Sprint K1: Const Generics Foundation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K1.1 | Const parameter syntax | [x] | `fn zeros<const N: usize>() -> [f64; N]` parsed | 80 | Parser handles const generics |
| K1.2 | Const param in type system | [x] | `N` usable as type-level integer, `[T; N]` has type `Array<T, N>` | 100 | Type system resolves const params |
| K1.3 | Const param monomorphization | [x] | Instantiate `zeros<3>()` -> concrete function | 80 | Monomorphized function correct |
| K1.4 | Const expressions in types | [x] | `[T; N + 1]`, `[T; N * 2]` in type positions | 60 | Arithmetic in type expressions |
| K1.5 | Const parameter bounds | [x] | `where N > 0` compile-time constraint | 60 | `zeros<0>()` -> compile error |
| K1.6 | Const parameter inference | [x] | Infer N from context: `let arr: [i32; 3] = zeros()` | 60 | N inferred as 3 |
| K1.7 | Const param in struct | [x] | `struct Matrix<const R: usize, const C: usize>` | 60 | Struct parameterized by const |
| K1.8 | Const param in enum | [x] | `enum SmallVec<T, const N: usize>` | 40 | Enum with inline storage |
| K1.9 | Const param in impl | [x] | `impl<const N: usize> Matrix<N, N> { fn identity() }` | 50 | Impl blocks with const params |
| K1.10 | 10 const generic tests | [x] | Parse, monomorphize, infer, struct, enum, impl | 150 | All 10 pass |

**Sprint K1 Gate:** `cargo test --lib` passes with all new const generic tests green.

### Sprint K2: Const Fn Enhancement (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K2.1 | `const fn` declaration | [x] | `const fn fib(n: usize) -> usize` syntax parsed | 40 | Parser accepts `const fn` |
| K2.2 | Const fn type checking | [x] | Verify const fn body is const-evaluable; non-const ops -> error | 80 | Non-const operations rejected |
| K2.3 | Const fn recursion | [x] | Support recursive const fns with bounded depth | 60 | `const fn factorial(n)` works |
| K2.4 | Const fn with generics | [x] | `const fn size_of<T>() -> usize` | 60 | Generic const fn works |
| K2.5 | Const fn with structs | [x] | Construct structs at compile time | 60 | `const ORIGIN: Point = Point { x: 0.0, y: 0.0 }` |
| K2.6 | Const fn with arrays | [x] | Create and index arrays at compile time | 60 | `const PRIMES: [i32; 5] = compute_primes()` |
| K2.7 | Const fn with match | [x] | Pattern matching in const context | 60 | `const fn abs(x: i32) -> i32 { match ... }` |
| K2.8 | Const fn with loops | [x] | For/while loops in const context (bounded) | 60 | Loop unrolled at compile time |
| K2.9 | Const fn panic | [x] | `const_panic!("message")` at compile time | 30 | Compile-time panic -> error message |
| K2.10 | 10 const fn tests | [x] | Recursion, generics, structs, arrays, match, loops | 150 | All 10 pass |

**Sprint K2 Gate:** All const fn tests pass, clippy clean.

### Sprint K3: Const Trait Bounds (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K3.1 | `const trait` definition | [x] | `const trait ConstAdd { const fn add(self, other: Self) -> Self }` | 60 | Trait parsed |
| K3.2 | `const impl` | [x] | `const impl ConstAdd for i32 { const fn add... }` | 60 | Impl parsed and checked |
| K3.3 | Const trait bounds | [x] | `fn sum<T: const ConstAdd>(arr: [T; N])` | 60 | Bound enforced at call site |
| K3.4 | Const trait method dispatch | [x] | Call const trait method at compile time | 80 | Static dispatch in const context |
| K3.5 | Built-in const traits | [x] | `const ConstEq`, `const ConstOrd`, `const ConstDefault` | 60 | Built-in types implement const traits |
| K3.6 | Derive const traits | [x] | `#[derive(ConstEq, ConstDefault)]` for user types | 60 | Derived implementations work |
| K3.7 | Const trait objects | [x] | `&dyn const ConstAdd` for compile-time polymorphism | 80 | Const vtable dispatch |
| K3.8 | Const where clauses | [x] | `where T: const Add + const Mul` compound bounds | 40 | Multiple const bounds |
| K3.9 | Const associated types | [x] | `const type Output;` in const traits | 50 | Associated type resolved at compile time |
| K3.10 | 10 const trait tests | [x] | Definition, impl, bounds, dispatch, derive | 150 | All 10 pass |

**Sprint K3 Gate:** Const trait system end-to-end, all tests pass.

### Sprint K4: Compile-Time Allocation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K4.1 | Const array allocation | [x] | `const ARR: [i32; 1000] = fill(0)` -> .rodata | 60 | Large const array in binary |
| K4.2 | Const string allocation | [x] | `const GREETING: str = format("Hello {}", NAME)` | 60 | String in .rodata |
| K4.3 | Const struct allocation | [x] | `const CONFIG: Config = Config { ... }` | 40 | Struct in .rodata |
| K4.4 | Const HashMap | [x] | `const MAP: HashMap<str, i32> = build_map()` precomputed | 100 | Map stored as static |
| K4.5 | Const slice | [x] | `const SLICE: &[i32] = &[1, 2, 3]` | 40 | Slice points to .rodata |
| K4.6 | Static promotion | [x] | Promote const expressions to static storage | 60 | Temporary const -> static lifetime |
| K4.7 | Const allocator | [x] | Arena allocator for compile-time computations | 80 | No runtime allocation for const data |
| K4.8 | Const size verification | [x] | Verify const allocation fits in target memory; >1MB -> warning | 40 | Warning on large const data |
| K4.9 | Cross-compilation const | [x] | Const evaluation respects target pointer size | 40 | Const correct for ARM64 target |
| K4.10 | 10 allocation tests | [x] | Array, string, struct, map, slice, promotion | 150 | All 10 pass |

**Sprint K4 Gate:** Compile-time allocation works with Cranelift + LLVM backends.

### Sprint K5: Compile-Time Reflection (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K5.1 | `type_name::<T>()` | [x] | Return type name as const string | 40 | `type_name::<i32>()` -> `"i32"` |
| K5.2 | `size_of::<T>()` | [x] | Return byte size at compile time | 40 | `size_of::<i64>()` -> `8` |
| K5.3 | `align_of::<T>()` | [x] | Return alignment at compile time | 30 | `align_of::<f64>()` -> `8` |
| K5.4 | `field_count::<T>()` | [x] | Return struct field count | 40 | `field_count::<Point>()` -> `2` |
| K5.5 | `field_names::<T>()` | [x] | Return field names as const array | 60 | `["x", "y"]` |
| K5.6 | `field_types::<T>()` | [x] | Return field type names | 60 | `["f64", "f64"]` |
| K5.7 | `variant_count::<T>()` | [x] | Return enum variant count | 40 | `variant_count::<Option<i32>>()` -> `2` |
| K5.8 | `variant_names::<T>()` | [x] | Return variant names | 40 | `["Some", "None"]` |
| K5.9 | `has_trait::<T, Trait>()` | [x] | Check if type implements trait at compile time | 60 | `has_trait::<i32, Display>()` -> `true` |
| K5.10 | 10 reflection tests | [x] | Type name, size, align, fields, variants, traits | 150 | All 10 pass |

**Sprint K5 Gate:** All reflection intrinsics work in const context.

### Sprint K6: Const Evaluation in Macros (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K6.1 | `const_eval!()` macro | [x] | Evaluate expression at compile time in macro | 60 | `const_eval!(1 + 2)` -> `3` |
| K6.2 | `static_assert!()` | [x] | Compile-time assertion | 40 | `static_assert!(size_of::<T>() <= 64)` |
| K6.3 | `include_str!()` | [x] | Include file contents as const string | 40 | File embedded at compile time |
| K6.4 | `include_bytes!()` | [x] | Include file as const byte array | 40 | Binary data embedded |
| K6.5 | `env!()` | [x] | Read environment variable at compile time | 30 | `env!("VERSION")` -> build-time value |
| K6.6 | `concat!()` | [x] | Concatenate const strings | 30 | `concat!("v", env!("VERSION"))` |
| K6.7 | `cfg!()` enhancement | [x] | `cfg!(target_os = "linux")` as const bool | 30 | Conditional compilation |
| K6.8 | `option_env!()` | [x] | Optional env var as `Option<str>` | 30 | Missing var -> `None` |
| K6.9 | `compile_error!()` | [x] | User-defined compile error | 20 | Custom error at compile time |
| K6.10 | 10 macro const tests | [x] | eval, assert, include, env, cfg, compile_error | 150 | All 10 pass |

**Sprint K6 Gate:** All const macros work, integrated with V12 macro system.

### Sprint K7: Const in Standard Library (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K7.1 | Const math functions | [x] | `const fn abs`, `min`, `max`, `clamp` | 60 | Math at compile time |
| K7.2 | Const string operations | [x] | `const fn str_len`, `str_eq` | 40 | String ops at compile time |
| K7.3 | Const array operations | [x] | `const fn array_len`, `array_get` | 40 | Array ops at compile time |
| K7.4 | Const Option methods | [x] | `const fn unwrap_or`, `is_some`, `map` | 40 | Option at compile time |
| K7.5 | Const Result methods | [x] | `const fn unwrap_or`, `is_ok`, `map` | 40 | Result at compile time |
| K7.6 | Const hash functions | [x] | `const fn hash_str`, `hash_bytes` | 60 | Hashing at compile time |
| K7.7 | Const formatting | [x] | `const fn format_int`, `format_float` | 60 | Number -> string at compile time |
| K7.8 | Const bit manipulation | [x] | `const fn count_ones`, `leading_zeros` | 40 | Bit ops at compile time |
| K7.9 | Const conversion | [x] | `const fn i32_to_i64`, `f64_to_f32` | 30 | Type conversion at compile time |
| K7.10 | 10 const stdlib tests | [x] | Math, string, array, option, hash, format, bits | 150 | All 10 pass |

**Sprint K7 Gate:** Const stdlib functions usable in `const` and `comptime` contexts.

### Sprint K8: Const Generics in Type System (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K8.1 | `[T; N]` fixed array type | [x] | First-class fixed-size array with const generic | 80 | `[i32; 3]` distinct from `[i32; 4]` |
| K8.2 | Matrix type | [x] | `struct Matrix<T, const R: usize, const C: usize>` | 60 | `Matrix<f64, 3, 3>` is a type |
| K8.3 | Const generic functions | [x] | `fn dot<const N: usize>(a: [f64; N], b: [f64; N]) -> f64` | 60 | Dimension-checked dot product |
| K8.4 | Const generic methods | [x] | `impl<const N: usize> [T; N] { fn len() -> usize { N } }` | 60 | Methods on const generic types |
| K8.5 | Const generic trait impl | [x] | `impl<const N: usize> Display for [T; N]` | 60 | Display for all array sizes |
| K8.6 | Const arithmetic in types | [x] | `fn concat<...>(x: [T; A], y: [T; B]) -> [T; A + B]` | 80 | Return type computed from consts |
| K8.7 | Dependent type verification | [x] | Verify `A + B` doesn't overflow usize | 40 | Overflow -> compile error |
| K8.8 | Const generic enum | [x] | `enum SmallVec<T, const N: usize> { Inline([T; N]), Heap(Vec<T>) }` | 60 | Enum with inline buffer |
| K8.9 | Default const values | [x] | `struct Buffer<T, const N: usize = 1024>` | 40 | Default const parameter |
| K8.10 | 10 type system tests | [x] | Array, matrix, functions, methods, arithmetic | 150 | All 10 pass |

**Sprint K8 Gate:** Const generics fully integrated into type system and codegen.

### Sprint K9: Pipeline Integration (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K9.1 | Analyzer integration | [x] | Const evaluation runs during semantic analysis | 60 | Const errors at check time |
| K9.2 | Cranelift integration | [x] | Const values emitted as immediates in IR | 40 | `const X = 42` -> `iconst.i64 42` |
| K9.3 | LLVM integration | [x] | Const values as LLVM constants | 40 | `const X` -> LLVM `i64 42` |
| K9.4 | VM integration | [x] | Const values precomputed in bytecode constant pool | 40 | VM loads const from pool |
| K9.5 | LSP integration | [x] | Const values shown in hover | 30 | Hover on `const X` shows value |
| K9.6 | LSP completion | [x] | Suggest const fns in const context | 30 | Completion filters non-const fns |
| K9.7 | Error messages | [x] | "cannot call non-const fn in const context" | 40 | Clear error messages |
| K9.8 | REPL support | [x] | `comptime { ... }` evaluates in REPL | 20 | REPL shows const results |
| K9.9 | Documentation | [x] | `book/const_evaluation.md` guide | 200 | Const generics + const fn guide |
| K9.10 | 10 integration tests | [x] | Analyzer, Cranelift, LLVM, VM, LSP, REPL | 150 | All 10 pass |

**Sprint K9 Gate:** Const evaluation works across all backends (interpreter, VM, Cranelift, LLVM).

### Sprint K10: Benchmarks & Validation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| K10.1 | Const eval time benchmark | [x] | Measure compile-time cost of const evaluation | 30 | < 1% compilation overhead |
| K10.2 | Const vs runtime comparison | [x] | Compare const-evaluated vs runtime-computed results | 30 | Identical results |
| K10.3 | Large const data | [x] | 1MB lookup table generated at compile time | 40 | Table correct, in .rodata |
| K10.4 | Recursive const depth | [x] | Test 256-level recursive const fn | 20 | No stack overflow |
| K10.5 | Const generics coverage | [x] | All 15 numeric types as const parameters | 30 | `u8` through `u128` work |
| K10.6 | Example: const physics | [x] | `examples/const_physics.fj` — compile-time unit checking | 100 | Units checked at compile time |
| K10.7 | Example: const LUT | [x] | `examples/const_lut.fj` — lookup table generation | 80 | LUT embedded in binary |
| K10.8 | Example: const matrix | [x] | `examples/const_matrix.fj` — dimension-checked ops | 80 | Matrix operations type-safe |
| K10.9 | Update CLAUDE.md | [x] | Document const features in language reference | 30 | CLAUDE.md reflects const features |
| K10.10 | Update GAP_ANALYSIS_V2 | [x] | Mark const fn as 100% production | 20 | Audit reflects real status |

**Sprint K10 Gate:** All benchmarks documented, 3 examples pass, docs updated.

---

## Option C: Incremental Compilation

**Goal:** Persistent disk cache, fine-grained deps, parallel build, sub-second rebuilds.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~7,200
**Dependency:** Option A complete (CI green).
**Existing:** 2,869 LOC, 40 tests (in-memory dependency graph, SHA256 change detection).

### Sprint I1: Persistent Disk Cache (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I1.1 | Cache directory layout | [x] | `target/incremental/{hash}/` with metadata.json + artifacts | 60 | Directory created on first build |
| I1.2 | Artifact serialization | [x] | Serialize analyzed AST + type info to bincode | 100 | Round-trip AST -> bytes -> AST matches |
| I1.3 | Artifact deserialization | [x] | Load cached artifacts on subsequent build | 80 | Cached build uses serialized data |
| I1.4 | Content hash persistence | [x] | Store SHA256 hashes in `target/incremental/hashes.json` | 50 | Hashes survive process restart |
| I1.5 | Cache invalidation | [x] | Invalidate when Cargo.toml, fj.toml, or compiler version changes | 60 | Config change triggers full rebuild |
| I1.6 | Cache size management | [x] | LRU eviction when cache exceeds `--cache-limit` (default 1GB) | 80 | Old artifacts evicted |
| I1.7 | Cache corruption detection | [x] | Validate checksum on load, discard corrupt entries | 60 | Corrupted cache -> clean rebuild |
| I1.8 | Atomic cache writes | [x] | Write to temp file, then atomic rename | 40 | Interrupted build doesn't corrupt |
| I1.9 | `fj clean --incremental` | [x] | CLI command to purge incremental cache | 20 | Cache directory removed |
| I1.10 | 10 persistence tests | [x] | Write/read/invalidate/corrupt/evict/clean | 150 | All 10 pass |

**Sprint I1 Gate:** Cache round-trips correctly, survives restart, handles corruption.

### Sprint I2: Fine-Grained Dependency Tracking (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I2.1 | Function-level tracking | [x] | Track which functions changed, not just files | 120 | Change one fn -> only dependents recompile |
| I2.2 | Type-level tracking | [x] | Track struct/enum definition changes | 100 | Change struct field -> dependents recompile |
| I2.3 | Import graph construction | [x] | Build use/mod import edges between modules | 80 | Import graph matches actual deps |
| I2.4 | Signature vs body change | [x] | Signature change -> recompile callers; body-only -> skip | 80 | Body-only change is fastest path |
| I2.5 | Trait impl tracking | [x] | Track which trait impls changed | 80 | New trait impl -> monomorphization invalidated |
| I2.6 | Constant propagation tracking | [x] | Track const value changes | 50 | `const X = 5` -> `6` triggers recompile |
| I2.7 | Macro expansion tracking | [x] | Track macro input -> output mapping | 80 | Macro change -> expansion sites recompile |
| I2.8 | Cross-module type inference | [x] | Track inferred types depending on other modules | 60 | Type change in A -> B reanalyzed |
| I2.9 | Dep graph visualization | [x] | `fj build --dep-graph` outputs DOT format | 50 | `dot -Tsvg` renders graph |
| I2.10 | 10 dependency tests | [x] | Function, type, signature, trait, const, macro | 150 | All 10 pass |

**Sprint I2 Gate:** Fine-grained tracking detects minimal recompilation set.

### Sprint I3: IR-Level Caching (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I3.1 | Cranelift IR serialization | [x] | Serialize Cranelift `Function` to binary | 120 | IR round-trips correctly |
| I3.2 | LLVM bitcode caching | [x] | Cache `.bc` files per module for LLVM backend | 80 | LLVM uses cached bitcode |
| I3.3 | Bytecode VM caching | [x] | Cache compiled bytecode per function | 60 | VM uses cached bytecode |
| I3.4 | Object file caching | [x] | Cache `.o` files for unchanged modules | 60 | Linker uses cached objects |
| I3.5 | Cache key computation | [x] | Hash of (source + compiler version + flags + deps) | 80 | Different flags -> different key |
| I3.6 | Incremental linking | [x] | Only re-link changed object files | 100 | Incremental link < 100ms |
| I3.7 | Debug info caching | [x] | Cache DWARF debug info per module | 60 | Debug symbols correct |
| I3.8 | Parallel cache reads | [x] | Load cached artifacts in parallel threads | 60 | Multi-core cache loading |
| I3.9 | Cache hit metrics | [x] | `--verbose` shows `Cache: 45/50 hit (90%)` | 40 | Hit/miss ratio displayed |
| I3.10 | 10 IR cache tests | [x] | Cranelift, LLVM, bytecode, object, parallel | 150 | All 10 pass |

**Sprint I3 Gate:** All three backends (Cranelift, LLVM, VM) use cached IR.

### Sprint I4: Parallel Compilation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I4.1 | Module-level parallelism | [x] | Compile independent modules on separate threads | 120 | 4 modules -> 4 threads |
| I4.2 | Thread pool config | [x] | `--jobs=N` or `FJ_JOBS` env var (default = CPU count) | 40 | `fj build --jobs=8` works |
| I4.3 | Topological scheduling | [x] | Compile in dependency order, parallelize within levels | 80 | Level 0 first, then level 1 |
| I4.4 | Work stealing scheduler | [x] | Idle threads steal work from busy threads | 100 | All cores utilized |
| I4.5 | Thread-safe diagnostics | [x] | Collect errors from all threads without data races | 60 | All errors reported |
| I4.6 | Parallel analysis | [x] | Type checker on independent modules concurrently | 80 | Analysis parallelized |
| I4.7 | Parallel codegen | [x] | Cranelift/LLVM on independent functions | 80 | Codegen parallelized |
| I4.8 | Progress reporting | [x] | `[1/50] Compiling module_a...` progress bar | 40 | Real-time progress |
| I4.9 | Deadlock prevention | [x] | Detect and break circular compilation deps | 60 | No deadlocks |
| I4.10 | 10 parallel tests | [x] | 2/4/8 threads, scheduling, progress, deadlock | 150 | All 10 pass |

**Sprint I4 Gate:** Parallel builds work with rayon/crossbeam, no data races.

### Sprint I5: Pipeline Integration (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I5.1 | Wire into `fj build` | [x] | Default incremental for `fj build` | 40 | Incremental by default |
| I5.2 | `--no-incremental` flag | [x] | Disable for clean builds | 10 | Full rebuild on flag |
| I5.3 | Wire into `fj check` | [x] | Incremental type checking | 40 | Faster on second run |
| I5.4 | Wire into `fj test` | [x] | Only recompile changed test targets | 40 | Faster when 1 file changed |
| I5.5 | Wire into `fj run` | [x] | Incremental build before run | 20 | Only changed modules rebuilt |
| I5.6 | Wire into LSP | [x] | LSP uses incremental analysis | 80 | < 200ms after edit |
| I5.7 | Wire into `fj watch` | [x] | File watcher triggers incremental rebuild | 40 | Save -> rebuild < 1s |
| I5.8 | Workspace incremental | [x] | Incremental across workspace members | 60 | Only affected members rebuild |
| I5.9 | `fj build --timings` | [x] | Time per phase (parse/analyze/codegen/link) | 60 | Timing breakdown displayed |
| I5.10 | 10 pipeline tests | [x] | build, check, test, run, watch, LSP, workspace | 150 | All 10 pass |

**Sprint I5 Gate:** All CLI commands use incremental compilation by default.

### Sprint I6: Rebuild Performance Benchmarks (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I6.1 | Cold build benchmark | [x] | Full build from scratch on 10K LOC project | 30 | Baseline recorded |
| I6.2 | Warm build (no change) | [x] | Build with no changes (all cache hits) | 20 | < 100ms for 10K LOC |
| I6.3 | Single-file change | [x] | Change one file in 10K LOC project | 20 | < 500ms rebuild |
| I6.4 | Signature change | [x] | Change function signature (cascading) | 20 | < 2s rebuild |
| I6.5 | Type change | [x] | Change struct field (moderate cascade) | 20 | < 1s rebuild |
| I6.6 | New file added | [x] | Add new file to project | 20 | < 1s rebuild |
| I6.7 | File deleted | [x] | Remove file from project | 20 | < 500ms rebuild |
| I6.8 | 10 files changed | [x] | Batch change of 10 files | 20 | < 3s rebuild |
| I6.9 | Parallel speedup | [x] | Compare 1 vs 4 vs 8 threads on 50K LOC | 30 | 4x speedup with 8 threads |
| I6.10 | Document benchmarks | [x] | `docs/INCREMENTAL_BENCHMARKS.md` | 100 | Table with all measurements |

**Sprint I6 Gate:** All benchmarks documented with reproducible methodology.

### Sprint I7: Error Recovery & Edge Cases (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I7.1 | Parse error recovery | [x] | Partially parsed file still caches others | 60 | Error in A doesn't invalidate B |
| I7.2 | Type error recovery | [x] | Type errors don't prevent caching valid modules | 60 | Valid modules cached |
| I7.3 | Interrupted build recovery | [x] | Resume after Ctrl+C without corruption | 60 | Build completes on retry |
| I7.4 | Clock skew handling | [x] | Content hash (not mtime) is source of truth | 40 | No false invalidation |
| I7.5 | Symlink handling | [x] | Follow symlinks, cache by real path | 40 | Symlinked files tracked |
| I7.6 | Case sensitivity | [x] | Handle case-insensitive FS (macOS/Windows) | 40 | No duplicate entries |
| I7.7 | Unicode paths | [x] | Handle Unicode file paths in cache keys | 30 | `src/module.fj` cached correctly |
| I7.8 | Large file handling | [x] | Efficient hashing for files > 1MB | 30 | 10MB file hashed < 10ms |
| I7.9 | Circular dep detection | [x] | Detect and report circular module deps | 60 | Clear error message |
| I7.10 | 10 edge case tests | [x] | Parse error, interrupt, clock, symlink, circular | 150 | All 10 pass |

**Sprint I7 Gate:** Incremental compilation is robust against all edge cases.

### Sprint I8: LSP Integration (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I8.1 | Incremental analysis engine | [x] | LSP uses incremental for on-type checking | 120 | Errors < 200ms after keystroke |
| I8.2 | Per-function reanalysis | [x] | Typing in fn body only reanalyzes that fn | 100 | < 50ms per function |
| I8.3 | Background indexing | [x] | Index workspace in background on startup | 80 | Full index < 5s |
| I8.4 | Incremental symbol index | [x] | Update symbol table on file change | 80 | Go-to-definition works after edit |
| I8.5 | Incremental diagnostics | [x] | Only recompute for changed files + deps | 60 | < 100ms update |
| I8.6 | Incremental completion | [x] | Completion uses cached type info | 60 | < 100ms completion |
| I8.7 | Memory-efficient caching | [x] | Share AST/type data between LSP and compiler | 60 | < 200MB for 50K LOC |
| I8.8 | Cache warming on open | [x] | Pre-analyze opened files and their imports | 40 | First edit is fast |
| I8.9 | Stale cache indicator | [x] | StatusBar shows when analysis is stale vs fresh | 30 | User knows state |
| I8.10 | 10 LSP incremental tests | [x] | Typing, completion, diagnostics, symbols, memory | 150 | All 10 pass |

**Sprint I8 Gate:** LSP has sub-200ms response time for all operations.

### Sprint I9: Workspace & Multi-Target (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I9.1 | Workspace-level cache | [x] | Shared cache at workspace root | 60 | `target/incremental/` shared |
| I9.2 | Cross-member dep tracking | [x] | Change in `core` -> rebuilds `cli` | 80 | Cross-member invalidation |
| I9.3 | Per-target caching | [x] | Separate cache for x86_64 vs aarch64 | 60 | Targets don't conflict |
| I9.4 | Per-profile caching | [x] | Separate cache for debug vs release | 60 | Profiles isolated |
| I9.5 | Shared dep caching | [x] | Common deps compiled once across members | 80 | `fj-core` compiled once |
| I9.6 | Workspace build order | [x] | Build in optimal parallel order | 60 | Maximum parallelism |
| I9.7 | Workspace `--timings` | [x] | Per-member build times | 40 | Timing displayed |
| I9.8 | Remote cache (optional) | [x] | `--cache-dir=s3://bucket/` for CI | 100 | CI reuses dev cache |
| I9.9 | Cache statistics | [x] | `fj cache stats` — size, hit rate, age | 60 | Diagnostics displayed |
| I9.10 | 10 workspace tests | [x] | Cross-member, multi-target, shared, remote | 150 | All 10 pass |

**Sprint I9 Gate:** Workspace incremental works across members and targets.

### Sprint I10: Optimization & Validation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| I10.1 | Correctness validation | [x] | Compare incremental vs clean build output | 100 | Byte-identical binaries |
| I10.2 | Deterministic builds | [x] | Same input -> same cache key always | 60 | No randomness |
| I10.3 | Memory profiling | [x] | Measure memory during incremental build | 40 | < 500MB for 50K LOC |
| I10.4 | Compile-time regression | [x] | CI check: incremental overhead < 5% | 40 | Clean build not slowed |
| I10.5 | Self-hosting test | [x] | Fajar stdlib rebuilds incrementally | 60 | Stdlib incremental works |
| I10.6 | Stress test | [x] | 1000 edit-rebuild cycles without corruption | 60 | 0 failures in 1000 cycles |
| I10.7 | Documentation | [x] | `book/incremental_compilation.md` | 200 | Architecture + usage guide |
| I10.8 | Update CLAUDE.md | [x] | Document incremental in quick commands | 30 | CLAUDE.md updated |
| I10.9 | Update GAP_ANALYSIS_V2 | [x] | Mark incremental as 100% production | 20 | Audit updated |
| I10.10 | Example: large project | [x] | `examples/workspace_demo/` with 10 modules | 200 | Incremental < 500ms |

**Sprint I10 Gate:** Incremental compilation is production-validated, documented, stress-tested.

---

# ============================================================
# PHASE 2: ECOSYSTEM
# ============================================================

---

## Option B: WASI P2 + Component Model

**Goal:** Full WASI Preview 2 — WIT, components, HTTP, sockets, resources, deployment.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~8,500
**Dependency:** Phase 1 complete. Option H (const generics) improves W8 (resources).
**Existing:** 3,323 LOC, 12 tests (P1 100%, P2 40% framework).
**Status:** Sprint W1 COMPLETE (2026-04-01)

### Sprint W1: WIT Parser & Type System (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W1.1 | WIT lexer | [x] | Tokenize `.wit` files: package, world, interface, resource, use | 200 | Tokenizes `wasi:cli/command` |
| W1.2 | WIT parser | [x] | Parse WIT into `WitDocument { interfaces, worlds, types }` | 300 | Parses WASI CLI world |
| W1.3 | WIT type system | [x] | Map WIT types to Fajar: u32->u32, string->str, list<T>->Array<T> | 150 | 15 primitives mapped |
| W1.4 | WIT record types | [x] | `record point { x: f64, y: f64 }` -> Fajar struct | 80 | Fields accessible |
| W1.5 | WIT variant types | [x] | `variant error { timeout, refused(string) }` -> Fajar enum | 100 | Matching works |
| W1.6 | WIT flags types | [x] | `flags permissions { read, write, exec }` -> bitflags | 60 | Flag ops work |
| W1.7 | WIT resource types | [x] | `resource file { open: static func(...) }` -> opaque handle | 120 | Resource methods work |
| W1.8 | WIT tuple/option/result | [x] | Map WIT generics to Fajar Option<T>, Result<T,E> | 80 | `option<string>` -> `Option<str>` |
| W1.9 | WIT `use` imports | [x] | `use wasi:filesystem/types.{descriptor}` resolves | 100 | Cross-interface refs work |
| W1.10 | 10 WIT parser tests | [x] | Parse WASI CLI, HTTP, filesystem, sockets worlds | 150 | All 10 pass |

**Sprint W1 Gate:** WIT files parse correctly for all standard WASI interfaces. ✅ 55 tests, 0 failures.

### Sprint W2: Component Model Binary Format (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W2.1 | Component section emitter | [x] | Emit `component` section (custom section 0x0d) | 100 | `wasm-tools validate` accepts |
| W2.2 | Component type encoding | [x] | Encode WIT types in canonical ABI format | 150 | Types round-trip |
| W2.3 | Import section | [x] | Emit `(import "wasi:filesystem/types")` | 80 | wasmtime resolves imports |
| W2.4 | Export section | [x] | Emit `(export "run")` for command world | 60 | wasmtime calls export |
| W2.5 | Canonical ABI lifting | [x] | Lower Fajar values to linear memory (strings->ptr+len) | 200 | String passed correctly |
| W2.6 | Canonical ABI lowering | [x] | Lift host values back to Fajar values | 200 | Host return readable |
| W2.7 | Memory allocation protocol | [x] | `cabi_realloc` export for host-allocated memory | 80 | Host allocates in guest |
| W2.8 | Post-return cleanup | [x] | `cabi_post_*` exports for freeing returned values | 60 | No memory leaks |
| W2.9 | Component validation | [x] | Validate against WIT spec before output | 100 | Invalid -> error |
| W2.10 | 10 binary format tests | [x] | Validate with `wasm-tools component validate` | 150 | All 10 pass |

**Sprint W2 Gate:** Valid component binaries produced, pass wasm-tools validation. ✅ 23 tests, 0 failures.

### Sprint W3: WASI P2 Filesystem (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W3.1 | `wasi:filesystem/types` | [x] | Descriptor, DirectoryEntry, Filestat types | 80 | Types compile |
| W3.2 | `open-at` | [x] | Open file relative to directory descriptor | 60 | `open_at(dir, "file.txt", READ)` |
| W3.3 | `read-via-stream` | [x] | Streaming file read via `wasi:io/streams` | 100 | Read 1MB in 4KB chunks |
| W3.4 | `write-via-stream` | [x] | Streaming file write | 100 | Write and verify |
| W3.5 | `stat` / `stat-at` | [x] | Get file metadata | 60 | Size matches written bytes |
| W3.6 | `readdir` | [x] | Read directory with cookie pagination | 80 | List contents |
| W3.7 | `path-create-directory` | [x] | Create nested directories | 40 | `mkdir -p` works |
| W3.8 | `unlink-file` / `remove-dir` | [x] | Delete files and directories | 40 | File removed |
| W3.9 | `path-rename` | [x] | Atomic rename | 50 | Content preserved |
| W3.10 | 10 filesystem tests | [x] | Read/write/stat/readdir/rename with wasmtime | 150 | All 10 pass |

**Sprint W3 Gate:** Full filesystem operations work. ✅ 17 tests, 0 failures.

### Sprint W4: WASI P2 Streams & I/O (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W4.1 | Input stream | [x] | Blocking and pollable read | 100 | Read stdin line by line |
| W4.2 | Output stream | [x] | Blocking and pollable write | 100 | Write to stdout with flush |
| W4.3 | Poll | [x] | `poll-one`, `poll-many` from pollable | 120 | First-ready returns |
| W4.4 | Stream splice | [x] | Zero-copy pipe input->output | 80 | Pipe stdin to file |
| W4.5 | Async streams | [x] | `subscribe()` -> pollable for non-blocking I/O | 100 | Non-blocking read |
| W4.6 | Error handling | [x] | `stream-error` with last-operation-failed | 60 | EOF/permission errors |
| W4.7 | Monotonic clock | [x] | `now()`, `resolution()`, `subscribe-duration()` | 60 | Timestamps correct |
| W4.8 | Wall clock | [x] | `now()` -> `datetime { seconds, nanoseconds }` | 40 | Within 1s of host |
| W4.9 | Random | [x] | `get-random-bytes(len)`, `get-random-u64()` | 40 | Non-zero bytes |
| W4.10 | 10 stream tests | [x] | Read/write, poll, clocks, random | 150 | All 10 pass |

**Sprint W4 Gate:** Streaming I/O, clocks, and random work end-to-end. ✅ 16 tests, 0 failures.

### Sprint W5: WASI P2 HTTP Client (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W5.1 | HTTP types | [x] | Request, Response, Headers, Method, StatusCode | 100 | Types compile |
| W5.2 | Outgoing handler | [x] | `handle(request)` -> future response | 120 | GET returns 200 |
| W5.3 | Request construction | [x] | Builder: GET/POST/PUT/DELETE | 80 | All methods work |
| W5.4 | Response reading | [x] | Status, headers, body stream | 80 | Parse JSON body |
| W5.5 | Request body streaming | [x] | Write body via output-stream for POST | 80 | POST with JSON works |
| W5.6 | Response body streaming | [x] | Read body via input-stream (chunked) | 80 | Stream large response |
| W5.7 | Header manipulation | [x] | Add, get, delete, iterate headers | 50 | Content-Type correct |
| W5.8 | Error handling | [x] | Network error, timeout, DNS failure -> Result | 60 | Timeout -> Err |
| W5.9 | HTTPS support | [x] | TLS via host | 30 | `https://` URLs work |
| W5.10 | 10 HTTP client tests | [x] | GET, POST, headers, streaming, errors | 150 | All 10 pass |

**Sprint W5 Gate:** HTTP client works for all standard methods with error handling. ✅ 10 tests, 0 failures.

### Sprint W6: WASI P2 HTTP Server (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W6.1 | Incoming handler export | [x] | `handle(request, response-outparam)` entry point | 100 | Component exports handler |
| W6.2 | Request routing | [x] | Path + method matching | 80 | `/api/users` routed |
| W6.3 | Response construction | [x] | `response-outparam.set(status, headers, body)` | 80 | 200 + JSON body |
| W6.4 | Middleware pipeline | [x] | Pre/post processing (logging, auth, CORS) | 100 | Chain executes in order |
| W6.5 | JSON serialization | [x] | Serialize Fajar structs to JSON response | 60 | Valid JSON output |
| W6.6 | Error responses | [x] | 400/404/500 with proper status codes | 40 | Correct codes |
| W6.7 | Request body parsing | [x] | Parse JSON/form body | 80 | POST body deserialized |
| W6.8 | Static file serving | [x] | Serve from preopened directory | 60 | HTML with MIME type |
| W6.9 | wasmtime serve | [x] | Deploy with `wasmtime serve component.wasm` | 30 | HTTP server runs |
| W6.10 | 10 HTTP server tests | [x] | Route, middleware, JSON, error, static | 150 | All 10 pass |

**Sprint W6 Gate:** Full HTTP server component with routing, middleware, static files. ✅ 10 tests, 0 failures.

### Sprint W7: WASI P2 Sockets (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W7.1 | TCP types | [ ] | TcpSocket, IpAddress, Network resource | 80 | Types compile |
| W7.2 | TCP connect | [ ] | `start-connect` -> `finish-connect` | 100 | Connect to remote |
| W7.3 | TCP listen & accept | [ ] | `start-listen`, `accept` | 100 | Accept connections |
| W7.4 | TCP streams | [ ] | Get input/output streams from socket | 60 | Echo server works |
| W7.5 | UDP | [ ] | Datagram send/receive | 100 | UDP echo works |
| W7.6 | DNS lookup | [ ] | Hostname -> IP addresses | 80 | `resolve("example.com")` |
| W7.7 | Socket options | [ ] | `SO_REUSEADDR`, `TCP_NODELAY`, timeouts | 60 | Options affect behavior |
| W7.8 | Non-blocking I/O | [ ] | Pollable sockets for async | 80 | Poll multiple sockets |
| W7.9 | Socket errors | [ ] | Connection refused, timeout, reset -> Result | 40 | Errors propagated |
| W7.10 | 10 socket tests | [ ] | TCP, UDP, DNS, poll | 150 | All 10 pass |

**Sprint W7 Gate:** TCP + UDP + DNS fully functional on wasmtime.

### Sprint W8: Resource Lifecycle & Ownership (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W8.1 | Resource handle table | [ ] | Guest-side handle->index table | 100 | Handles allocated/freed |
| W8.2 | Resource drop protocol | [ ] | `resource.drop(handle)` -> host cleanup | 60 | Resources freed |
| W8.3 | Resource borrow semantics | [ ] | Borrowed handles can't outlive owner | 80 | Borrow checker enforces |
| W8.4 | Own vs borrow in WIT | [ ] | `own<file>` vs `borrow<file>` in signatures | 80 | Ownership correct |
| W8.5 | Resource constructor | [ ] | `[constructor]file(path)` -> `resource.new()` | 60 | Constructor creates handle |
| W8.6 | Resource methods | [ ] | `[method]file.read(len)` dispatch | 60 | Method calls work |
| W8.7 | Resource static methods | [ ] | `[static]file.open(path)` | 50 | Factory methods work |
| W8.8 | Nested resources | [ ] | Resource containing other resources | 80 | Correct drop order |
| W8.9 | Resource in collections | [ ] | `list<own<file>>` in returns | 60 | Handles in arrays |
| W8.10 | 10 resource tests | [ ] | Create, borrow, drop, nested, collection | 150 | All 10 pass, no leaks |

**Sprint W8 Gate:** Resource lifecycle fully managed, no leaks.

### Sprint W9: Component Composition & Linking (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W9.1 | Component instantiation | [ ] | Instantiate with provided imports | 120 | Component runs |
| W9.2 | Component linking | [ ] | Link A's export to B's import | 150 | Two components communicate |
| W9.3 | Virtualized filesystem | [ ] | Override `wasi:filesystem` with custom impl | 100 | In-memory FS works |
| W9.4 | `fj build --target wasm32-wasi-p2` | [ ] | CLI flag for P2 component output | 40 | `.wasm` produced |
| W9.5 | Component adapter | [ ] | Wrap P1 module in P2 component | 80 | Legacy code runs as P2 |
| W9.6 | Multi-component package | [ ] | Single project -> multiple components | 60 | Workspace builds multiple |
| W9.7 | Import satisfaction check | [ ] | Verify all imports satisfied | 60 | Missing import -> error |
| W9.8 | WIT dep resolution | [ ] | Resolve `use wasi:*` from spec packages | 80 | Standard interfaces found |
| W9.9 | Component binary size | [ ] | Strip debug, optimize sections | 40 | Hello-world < 100KB |
| W9.10 | 10 composition tests | [ ] | Link, virtualize, adapt, multi-component | 150 | All 10 pass |

**Sprint W9 Gate:** Components compose and link, deployable on multiple runtimes.

### Sprint W10: Validation & Deployment (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| W10.1 | wasmtime compat | [ ] | All components run on wasmtime 18+ | 50 | `wasmtime run` works |
| W10.2 | WAMR compat | [ ] | Basic components on WAMR | 50 | Alternative runtime works |
| W10.3 | Spin/Fermyon deploy | [ ] | HTTP component on Fermyon Cloud | 30 | `spin up` serves handler |
| W10.4 | wasi-virt testing | [ ] | Virtual FS + network for hermetic tests | 60 | Tests without host access |
| W10.5 | Size benchmarks | [ ] | Measure hello/http/filesystem sizes | 30 | Sizes documented |
| W10.6 | Startup benchmarks | [ ] | Measure instantiation time | 30 | < 10ms for CLI |
| W10.7 | Conformance tests | [ ] | Run WASI P2 test suite | 100 | 90%+ conformance |
| W10.8 | Documentation | [ ] | `book/wasi_p2_guide.md` | 200 | Full guide |
| W10.9 | Example: HTTP server | [ ] | `examples/wasi_http_server.fj` on Spin | 100 | Working on cloud |
| W10.10 | Update GAP_ANALYSIS_V2 | [ ] | Mark WASI P2 100% production | 20 | Audit updated |

**Sprint W10 Gate:** WASI P2 deployed on wasmtime + Spin, conformance tested.

---

## Option E: FFI v2 Full Integration

**Goal:** Full C++/Python/Rust interop — templates, async, trait objects, bindgen.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~8,000
**Dependency:** Phase 1 complete. Option C (incremental) speeds up iteration.
**Existing:** 3,149 LOC, 69 tests (libclang C++, PyO3 Python, basic Rust bridge).

### Sprint E1: C++ Template Support (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E1.1 | Template class detection | [ ] | Detect `template<class T>` via libclang | 80 | `std::vector<T>` detected |
| E1.2 | Template instantiation | [ ] | Generate bindings for `vector<int>` | 120 | `vector_i32` usable |
| E1.3 | Nested templates | [ ] | `map<string, vector<int>>` | 80 | Nested types correct |
| E1.4 | Template methods | [ ] | `.push_back()`, `.at()` etc. | 80 | Method calls work |
| E1.5 | SFINAE/concepts | [ ] | Handle conditional template methods | 60 | Only valid methods exposed |
| E1.6 | Template aliases | [ ] | `using Vec = vector<int>` | 40 | Aliases map correctly |
| E1.7 | Variadic templates | [ ] | `template<typename... Args>` basic | 60 | `tuple<int, float, string>` |
| E1.8 | Partial specialization | [ ] | `vector<bool>` uses bool specialization | 60 | Specializations handled |
| E1.9 | Default template args | [ ] | `template<class T = int>` | 40 | Defaults applied |
| E1.10 | 10 template tests | [ ] | Vector, map, nested, methods, variadic | 150 | All 10 pass with g++ |

**Sprint E1 Gate:** C++ templates compile and link, methods callable from Fajar.

### Sprint E2: C++ Smart Pointer & RAII Bridge (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E2.1 | `unique_ptr<T>` | [ ] | Map to Fajar ownership (move-only) | 80 | Owned Widget works |
| E2.2 | `shared_ptr<T>` | [ ] | Map to Fajar `Rc<T>` | 80 | Ref count tracks |
| E2.3 | `weak_ptr<T>` | [ ] | Map to Fajar `Weak<T>` | 60 | Weak ref works |
| E2.4 | RAII bridge | [ ] | C++ destructor called on Fajar drop | 80 | No resource leaks |
| E2.5 | Move semantics | [ ] | `std::move()` mapped to Fajar moves | 60 | Object moved, not copied |
| E2.6 | Copy semantics | [ ] | Copy constructor for copyable types | 50 | Deep copy works |
| E2.7 | Custom deleters | [ ] | `unique_ptr<T, Deleter>` | 40 | Custom cleanup runs |
| E2.8 | Ref-qualified methods | [ ] | `&` vs `&&` overload selection | 40 | Correct overload |
| E2.9 | Exception safety | [ ] | C++ exception -> Fajar `Err(msg)` | 80 | Exceptions caught |
| E2.10 | 10 smart pointer tests | [ ] | unique, shared, weak, RAII, move, exception | 150 | All 10 pass |

**Sprint E2 Gate:** C++ smart pointers and RAII fully bridged.

### Sprint E3: C++ STL Container Bridge (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E3.1 | `std::string` <-> `str` | [ ] | Zero-copy where possible | 80 | Round-trip preserves |
| E3.2 | `vector<T>` <-> `Array<T>` | [ ] | Automatic element conversion | 80 | `[1,2,3]` round-trips |
| E3.3 | `map<K,V>` <-> `HashMap` | [ ] | Key-value conversion | 80 | Entries preserved |
| E3.4 | `set<T>` <-> `HashSet<T>` | [ ] | Unique elements | 60 | Set ops work |
| E3.5 | `optional<T>` <-> `Option<T>` | [ ] | None/Some mapping | 40 | `nullopt` -> `None` |
| E3.6 | `variant<T...>` <-> enum | [ ] | Tagged union conversion | 80 | Variant -> enum |
| E3.7 | `tuple<T...>` <-> tuple | [ ] | Positional access | 60 | Elements accessible |
| E3.8 | `array<T,N>` <-> `[T; N]` | [ ] | Fixed-size conversion | 40 | Size preserved |
| E3.9 | `span<T>` <-> slice | [ ] | No-copy view | 60 | View works |
| E3.10 | 10 STL container tests | [ ] | String, vector, map, optional, variant | 150 | All 10 pass |

**Sprint E3 Gate:** All major STL containers bridge seamlessly.

### Sprint E4: Python Async Interop (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E4.1 | Python coroutine calling | [ ] | Call `async def` from Fajar | 120 | `await py_async_fn()` |
| E4.2 | asyncio event loop bridge | [ ] | Python asyncio + Fajar tokio coexist | 100 | Both loops run |
| E4.3 | Fajar async -> Python | [ ] | Expose Fajar async fn as Python awaitable | 100 | Python `await fj_fn()` |
| E4.4 | Async generator bridge | [ ] | Python `async for` consumes Fajar gen | 80 | Async iteration |
| E4.5 | GIL management | [ ] | Release GIL during Fajar computation | 60 | Python not blocked |
| E4.6 | Exception -> error | [ ] | `asyncio.TimeoutError` -> `Err(Timeout)` | 50 | Propagated |
| E4.7 | Cancellation | [ ] | Cancel Python coroutine from Fajar | 50 | CancelledError raised |
| E4.8 | Timeout support | [ ] | Set timeout for Python async calls | 40 | Timeout triggers cancel |
| E4.9 | Connection pooling | [ ] | Share HTTP/DB connections | 60 | Single pool |
| E4.10 | 10 async Python tests | [ ] | Coroutine, event loop, gen, GIL, cancel | 150 | All 10 pass |

**Sprint E4 Gate:** Python async fully interoperable with Fajar async.

### Sprint E5: Python NumPy/PyTorch Bridge (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E5.1 | Zero-copy NumPy -> Tensor | [ ] | Share memory, no copy for compatible dtypes | 100 | Memory shared |
| E5.2 | Zero-copy Tensor -> NumPy | [ ] | Export Fajar Tensor as numpy view | 80 | Python reads data |
| E5.3 | PyTorch tensor bridge | [ ] | `torch.Tensor` <-> Fajar Tensor | 100 | GPU tensors stay on GPU |
| E5.4 | dtype mapping | [ ] | All numpy dtypes to Fajar types | 60 | 12 dtypes supported |
| E5.5 | Shape/stride handling | [ ] | Non-contiguous views preserved | 60 | Transposed views work |
| E5.6 | PyTorch model loading | [ ] | Load `.pt` model via Python bridge | 80 | Pre-trained runs |
| E5.7 | Mixed training | [ ] | Train in Fajar, eval in PyTorch (or vice versa) | 100 | Weights shared |
| E5.8 | ONNX export | [ ] | `fj_model.export_onnx()` via Python | 60 | Valid ONNX produced |
| E5.9 | Batch processing | [ ] | Auto numpy/tensor conversion for batches | 60 | Pipeline works |
| E5.10 | 10 NumPy/PyTorch tests | [ ] | Zero-copy, dtype, model, mixed, ONNX | 150 | All 10 pass |

**Sprint E5 Gate:** NumPy + PyTorch zero-copy interop, model loading works.

### Sprint E6: Rust Trait Object Marshalling (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E6.1 | Rust trait -> Fajar trait | [ ] | Import Rust trait definition | 80 | `trait Display` imported |
| E6.2 | Fajar impl for Rust struct | [ ] | Fajar implements trait Rust calls | 100 | Cross-boundary dispatch |
| E6.3 | `dyn Trait` across FFI | [ ] | Pass trait objects between languages | 100 | vtable works |
| E6.4 | Generic function bridge | [ ] | Call Rust generic fn with Fajar types | 80 | Monomorphization at FFI |
| E6.5 | Lifetime handling | [ ] | Map Rust lifetimes to Fajar borrows | 80 | No dangling refs |
| E6.6 | Error type bridge | [ ] | `Result<T, E>` seamless conversion | 50 | Errors readable |
| E6.7 | Iterator bridge | [ ] | Rust iterators in Fajar for-in | 60 | `for x in rust_iter()` |
| E6.8 | Closure bridge | [ ] | Pass Fajar closures to Rust HOFs | 80 | `sort_by(|a, b| ...)` |
| E6.9 | Async bridge | [ ] | Rust futures -> Fajar async/await | 80 | `await rust_fn()` |
| E6.10 | 10 Rust bridge tests | [ ] | Traits, dyn, generics, lifetimes, closures | 150 | All 10 pass |

**Sprint E6 Gate:** Full bidirectional Rust interop with trait objects.

### Sprint E7: Automatic Binding Generator (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E7.1 | `fj bindgen` CLI | [ ] | Generate bindings from C/C++ headers | 60 | `fj bindgen opencv.hpp` |
| E7.2 | C header parsing | [ ] | Parse C headers (simple, no libclang) | 120 | `stdio.h` -> bindings |
| E7.3 | C++ header parsing | [ ] | Use libclang for C++ | 80 | Classes, namespaces |
| E7.4 | Python stub generation | [ ] | Generate from `.pyi` files | 80 | Type-safe bindings |
| E7.5 | Rust crate binding | [ ] | Parse Rust `pub` items | 80 | Public API imported |
| E7.6 | Binding customization | [ ] | `bindgen.toml` for overrides, skip patterns | 60 | Customizable |
| E7.7 | Doc preservation | [ ] | Copy doc comments to bindings | 40 | Hover shows docs |
| E7.8 | Incremental regeneration | [ ] | Only regenerate changed headers | 40 | Cached bindings |
| E7.9 | Safety annotations | [ ] | Mark unsafe, generate safe wrappers | 60 | Clear boundaries |
| E7.10 | 10 bindgen tests | [ ] | C, C++, Python, Rust, custom, incremental | 150 | All 10 pass |

**Sprint E7 Gate:** `fj bindgen` generates correct bindings for all target languages.

### Sprint E8: Build System Integration (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E8.1 | `[ffi]` in fj.toml | [ ] | Configure FFI targets in project config | 40 | Config parsed |
| E8.2 | pkg-config integration | [ ] | Auto-detect system C/C++ libs | 60 | OpenCV found |
| E8.3 | Python venv integration | [ ] | Use `fj.toml` specified venv | 40 | `.venv` activated |
| E8.4 | CMake integration | [ ] | Build C++ deps with CMake | 80 | cmake runs in `fj build` |
| E8.5 | Cargo integration | [ ] | Build Rust deps with Cargo | 60 | cargo runs for Rust deps |
| E8.6 | Linker flag management | [ ] | Auto-add `-lopencv_core`, `-lpython3.12` | 60 | Correct libs linked |
| E8.7 | Cross-compilation | [ ] | FFI works with cross targets | 80 | ARM64 FFI compiles |
| E8.8 | Hermetic builds | [ ] | Vendor all FFI deps | 60 | Build without internet |
| E8.9 | CI integration | [ ] | GitHub Actions with FFI deps | 50 | CI builds all FFI |
| E8.10 | 10 build system tests | [ ] | fj.toml, pkg-config, venv, cmake, cross | 150 | All 10 pass |

**Sprint E8 Gate:** FFI builds integrate seamlessly with `fj build`.

### Sprint E9: Safety & Performance (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E9.1 | Boundary validation | [ ] | Validate types at FFI boundary | 80 | Invalid -> error |
| E9.2 | Memory leak detection | [ ] | Detect leaked FFI objects via Drop | 60 | Leak report on exit |
| E9.3 | Thread safety | [ ] | GIL for Python, locks for C++ | 60 | No data races |
| E9.4 | Overhead measurement | [ ] | Benchmark FFI call overhead | 40 | < 100ns per call |
| E9.5 | Batch optimization | [ ] | Amortize overhead for batch calls | 60 | 1000 calls batched |
| E9.6 | Zero-copy verification | [ ] | Verify no hidden copies | 40 | Addresses match |
| E9.7 | Alignment handling | [ ] | Handle alignment differences | 50 | No faults |
| E9.8 | Endianness handling | [ ] | Byte order for cross-platform | 40 | Conversion correct |
| E9.9 | Sanitizer integration | [ ] | ASAN/MSAN/TSAN for FFI | 60 | No sanitizer errors |
| E9.10 | 10 safety tests | [ ] | Validation, leaks, threads, alignment | 150 | All 10 pass |

**Sprint E9 Gate:** FFI is safe, leak-free, and benchmarked.

### Sprint E10: Documentation & Examples (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| E10.1 | C++ FFI tutorial | [ ] | Step-by-step OpenCV from Fajar | 200 | Builds and runs |
| E10.2 | Python FFI tutorial | [ ] | NumPy + scikit-learn from Fajar | 200 | Builds and runs |
| E10.3 | Rust FFI tutorial | [ ] | Using Rust crate from Fajar | 150 | Builds and runs |
| E10.4 | Example: OpenCV | [ ] | `examples/opencv_ffi.fj` face detection | 100 | Face detection works |
| E10.5 | Example: NumPy | [ ] | `examples/numpy_ffi.fj` data analysis | 100 | Pipeline works |
| E10.6 | Example: PyTorch | [ ] | `examples/pytorch_ffi.fj` inference | 100 | Model runs |
| E10.7 | Example: Rust interop | [ ] | `examples/rust_ffi.fj` with serde_json | 80 | JSON parsing works |
| E10.8 | API reference | [ ] | `book/ffi_v2_reference.md` | 200 | Complete API |
| E10.9 | Migration guide | [ ] | "From C FFI to FFI v2" | 100 | Users can migrate |
| E10.10 | Update GAP_ANALYSIS_V2 | [ ] | Mark FFI v2 100% production | 20 | Audit updated |

**Sprint E10 Gate:** All tutorials build, 4 examples work, docs complete.

---

# ============================================================
# PHASE 3: DIFFERENTIATION
# ============================================================

---

## Option F: SMT Formal Verification

**Goal:** Z3-backed safety proofs, symbolic execution, @kernel/@device guarantees.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~7,500
**Dependency:** Phase 1 complete. Option C (incremental) improves proof caching.
**Existing:** 2,422 LOC, 67 tests (Z3 integration, spec language, tensor shape verify).

### Sprint V1: Symbolic Execution Engine (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V1.1 | Symbolic value repr | [ ] | `SymValue { concrete, symbolic, constraints }` | 100 | Symbolic int |
| V1.2 | Symbolic expression tree | [ ] | Build from Fajar AST | 120 | `x + 1 > 0` -> constraint |
| V1.3 | Path condition tracking | [ ] | Collect at branches | 80 | If/else -> 2 paths |
| V1.4 | Symbolic memory model | [ ] | Symbolic arrays for heap/stack | 100 | `arr[sym_idx]` |
| V1.5 | Loop handling | [ ] | Bounded unrolling (10 iterations) | 80 | Loop explored |
| V1.6 | Function summaries | [ ] | Cache input->output constraints | 80 | Avoid re-analysis |
| V1.7 | Path explosion mitigation | [ ] | Merge similar constraints | 60 | Blowup controlled |
| V1.8 | Concolic execution | [ ] | Concrete + symbolic guided exploration | 100 | Guided search |
| V1.9 | Counterexample generation | [ ] | Concrete input violating property | 60 | `x=5, y=-3` OOB |
| V1.10 | 10 symbolic tests | [ ] | Expressions, paths, memory, loops, counterexamples | 150 | All 10 pass |

**Sprint V1 Gate:** Symbolic execution finds real bugs in test programs.

### Sprint V2: Property Specification Language (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V2.1 | `@requires` precondition | [ ] | `@requires(n > 0)` on fn entry | 60 | Checked at call sites |
| V2.2 | `@ensures` postcondition | [ ] | `@ensures(result >= 0)` on fn exit | 60 | Verified |
| V2.3 | `@invariant` loop invariant | [ ] | `@invariant(i >= 0 && i < len)` | 80 | Holds each iteration |
| V2.4 | `@assert` inline | [ ] | `@assert(ptr != null)` | 40 | Verified statically |
| V2.5 | Quantified properties | [ ] | `@forall(i in 0..len, arr[i] >= 0)` | 80 | Universal checked |
| V2.6 | Temporal properties | [ ] | `@eventually(lock_released)` | 60 | Liveness verified |
| V2.7 | Type state properties | [ ] | `@typestate(File: Closed -> Open -> Closed)` | 80 | State machine enforced |
| V2.8 | Data flow properties | [ ] | `@no_leak(secret_key)` | 60 | Info flow checked |
| V2.9 | Custom property macros | [ ] | `@property("sorted", arr)` extensible | 60 | Custom checkers |
| V2.10 | 10 property tests | [ ] | Requires, ensures, invariant, quantified, temporal | 150 | All 10 pass |

**Sprint V2 Gate:** All annotation types parsed, verified by Z3.

### Sprint V3: @kernel Safety Proofs (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V3.1 | No-heap proof | [ ] | @kernel never allocates heap | 80 | Heap in @kernel -> error |
| V3.2 | No-tensor proof | [ ] | @kernel never creates tensors | 60 | Tensor in @kernel -> error |
| V3.3 | Stack bound proof | [ ] | Stack usage < configurable limit | 100 | Overflow impossible |
| V3.4 | Interrupt safety | [ ] | IRQ handlers don't hold locks | 80 | Lock in IRQ -> warning |
| V3.5 | MMIO safety | [ ] | Reads/writes to valid regions only | 80 | Invalid addr -> error |
| V3.6 | DMA buffer safety | [ ] | Properly aligned and sized | 60 | Misaligned -> error |
| V3.7 | Concurrency safety | [ ] | No data races in @kernel | 100 | Shared mutable -> error |
| V3.8 | Panic-freedom | [ ] | @kernel functions cannot panic | 80 | Panic path -> error |
| V3.9 | Termination proof | [ ] | @kernel always terminates | 60 | Infinite loop -> warning |
| V3.10 | 10 kernel safety tests | [ ] | No-heap, stack, IRQ, MMIO, panic-free | 150 | All 10 pass |

**Sprint V3 Gate:** @kernel context provably safe for all checked properties.

### Sprint V4: @device Safety Proofs (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V4.1 | No-raw-pointer proof | [ ] | @device never uses raw pointers | 60 | Raw ptr -> error |
| V4.2 | Tensor shape proof | [ ] | matmul dimensions compatible at compile time | 100 | `matmul(3x4, 5x6)` -> error |
| V4.3 | Tensor dtype proof | [ ] | Operation dtypes compatible | 60 | `add(f32, i64)` -> error |
| V4.4 | Memory bound proof | [ ] | Tensor ops within allocation | 80 | OOB slice -> error |
| V4.5 | Gradient tracking proof | [ ] | backward() only on tracked tensors | 60 | Non-tracked -> error |
| V4.6 | Numerical stability | [ ] | No div/0, log(0), sqrt(neg) | 100 | Unstable -> warning |
| V4.7 | Shape inference proof | [ ] | Inferred shapes match runtime | 80 | Mismatch -> error |
| V4.8 | Broadcast compat | [ ] | Broadcast rules satisfied | 60 | Incompatible -> error |
| V4.9 | Memory layout proof | [ ] | Row-major vs col-major consistent | 50 | Layout mismatch -> error |
| V4.10 | 10 device safety tests | [ ] | No-pointer, shapes, dtype, gradient | 150 | All 10 pass |

**Sprint V4 Gate:** @device context provably safe for tensor operations.

### Sprint V5: Proof Caching & Incrementality (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V5.1 | Proof result cache | [ ] | Cache per function | 80 | Unchanged -> cached |
| V5.2 | Cache invalidation | [ ] | Invalidate on fn/dep change | 60 | Modified -> re-verified |
| V5.3 | Incremental verification | [ ] | Only verify changed fns + deps | 80 | Minimal re-verification |
| V5.4 | Parallel verification | [ ] | Z3 on multiple fns concurrently | 60 | 4 fns in parallel |
| V5.5 | Timeout management | [ ] | Per-function 10s timeout with fallback | 40 | Complex -> warning |
| V5.6 | Proof persistence | [ ] | Store in `target/verify/` for CI | 60 | CI reuses proofs |
| V5.7 | Proof visualization | [ ] | `fj verify --report` -> HTML | 80 | Visual report |
| V5.8 | Counterexample display | [ ] | Show concrete failing input | 60 | "x=-1 violates x>=0" |
| V5.9 | Proof statistics | [ ] | Time, cache hit rate, coverage | 40 | Stats with --verbose |
| V5.10 | 10 caching tests | [ ] | Cache, invalidate, incremental, parallel | 150 | All 10 pass |

**Sprint V5 Gate:** Proofs cached, incremental, and parallelized.

### Sprint V6: Automated Property Inference (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V6.1 | Null safety inference | [ ] | Infer "result never null" | 80 | Nullable -> warning |
| V6.2 | Bounds inference | [ ] | Infer "index in bounds" from loop | 80 | `for i in 0..len` safe |
| V6.3 | Overflow inference | [ ] | Infer "no overflow" from ranges | 80 | `u8 + u8` -> warning |
| V6.4 | Division safety | [ ] | Infer "denominator != 0" | 60 | Safe when proven |
| V6.5 | Resource cleanup | [ ] | Infer "all resources dropped" | 80 | Leaked handle -> warning |
| V6.6 | Unreachable code | [ ] | Dead code after return/panic | 40 | Code after return |
| V6.7 | Type narrowing | [ ] | Narrowed types after if checks | 60 | `if x != null` -> non-null |
| V6.8 | Pattern exhaustiveness | [ ] | Match covers all cases | 60 | Missing variant -> error |
| V6.9 | Purity inference | [ ] | "no side effects" -> optimize | 60 | Pure fns marked |
| V6.10 | 10 inference tests | [ ] | Null, bounds, overflow, division, purity | 150 | All 10 pass |

**Sprint V6 Gate:** Automated inference catches common bugs without annotations.

### Sprint V7: Pipeline Integration (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V7.1 | `fj verify` CLI | [ ] | Run verification on project | 30 | Checks all annotated fns |
| V7.2 | `fj check --verify` | [ ] | Integrate with type checking | 20 | Includes proofs |
| V7.3 | LSP verification hints | [ ] | Inlay hints for verified fns | 60 | Green checkmark |
| V7.4 | CI verification step | [ ] | `fj verify --ci` exit code | 20 | CI fails on unverified |
| V7.5 | Verification config | [ ] | `[verify]` in fj.toml (off/warn/error) | 40 | Config controls level |
| V7.6 | Suppression comments | [ ] | `// @suppress(no-overflow)` | 30 | Known-safe not flagged |
| V7.7 | REPL verification | [ ] | `fj repl --verify` live checks | 40 | Warnings in REPL |
| V7.8 | IDE code actions | [ ] | "Add @requires" from LSP | 60 | One-click annotation |
| V7.9 | Verification diff | [ ] | `fj verify --diff` only new issues | 40 | Only new flagged |
| V7.10 | 10 integration tests | [ ] | CLI, LSP, CI, config, suppression | 150 | All 10 pass |

**Sprint V7 Gate:** Verification integrated into all tools (CLI, LSP, CI).

### Sprint V8: Advanced Theories (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V8.1 | Bitvector theory | [ ] | Precise u8/u16/u32/u64 overflow | 80 | Bit-exact detection |
| V8.2 | Array theory | [ ] | SMT array for buffer ops | 80 | Buffer bounds verified |
| V8.3 | Floating-point theory | [ ] | IEEE 754 reasoning | 80 | NaN/Inf detection |
| V8.4 | String theory | [ ] | SMT string reasoning | 60 | Buffer overflow detected |
| V8.5 | Nonlinear arithmetic | [ ] | Polynomial constraints | 60 | `x*x >= 0` proven |
| V8.6 | Separation logic | [ ] | Heap reasoning for pointers | 100 | Aliasing detected |
| V8.7 | Concurrent theory | [ ] | Lock ordering, atomics | 100 | Deadlock detected |
| V8.8 | Theory combination | [ ] | Nelson-Oppen multiple theories | 80 | Mixed constraints solved |
| V8.9 | Custom theory plugins | [ ] | User-defined theories | 60 | Custom theory loaded |
| V8.10 | 10 theory tests | [ ] | Bitvec, array, float, concurrent, combined | 150 | All 10 pass |

**Sprint V8 Gate:** Multiple SMT theories combined for comprehensive reasoning.

### Sprint V9: Safety Certification Support (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V9.1 | MISRA-C compliance | [ ] | Subset of MISRA rules for @kernel | 100 | Violations reported |
| V9.2 | CERT-C compliance | [ ] | Memory safety rules | 80 | Violations reported |
| V9.3 | DO-178C evidence | [ ] | Aerospace verification evidence | 100 | Evidence document |
| V9.4 | ISO 26262 evidence | [ ] | Automotive ASIL-D | 80 | ASIL report |
| V9.5 | IEC 62304 evidence | [ ] | Medical device software | 60 | Medical report |
| V9.6 | Traceability matrix | [ ] | Requirements -> verified properties | 60 | Requirements covered |
| V9.7 | Verification coverage | [ ] | MCDC-style coverage | 80 | Percentage reported |
| V9.8 | Audit trail | [ ] | Log all decisions with timestamps | 40 | Auditable history |
| V9.9 | Certificate generation | [ ] | Machine-readable verification cert | 60 | Deployment gate |
| V9.10 | 10 certification tests | [ ] | MISRA, CERT, DO-178C, ISO 26262 | 150 | All 10 pass |

**Sprint V9 Gate:** Safety certification artifacts generated for 3 standards.

### Sprint V10: Benchmarks & Documentation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| V10.1 | Verification time | [ ] | Per-function time benchmark | 30 | < 10s average |
| V10.2 | Scalability | [ ] | 1K/10K/50K LOC projects | 30 | Sub-linear scaling |
| V10.3 | False positive rate | [ ] | Measure on known-safe code | 40 | < 5% false positives |
| V10.4 | Bug detection rate | [ ] | Test against known-buggy code | 40 | > 90% detected |
| V10.5 | Cache speedup | [ ] | Incremental verification speedup | 20 | > 10x with cache |
| V10.6 | Documentation | [ ] | `book/formal_verification.md` | 200 | Full guide |
| V10.7 | Example: verified kernel | [ ] | `examples/verified_kernel.fj` | 150 | All @kernel verified |
| V10.8 | Example: verified ML | [ ] | `examples/verified_ml.fj` shape proofs | 100 | Shapes verified |
| V10.9 | Update CLAUDE.md | [ ] | Document verify CLI + annotations | 30 | Updated |
| V10.10 | Update GAP_ANALYSIS_V2 | [ ] | Mark SMT 100% production | 20 | Audit updated |

**Sprint V10 Gate:** < 5% false positives, > 90% bug detection, fully documented.

---

## Option D: Distributed Runtime

**Goal:** Real Raft consensus, dynamic discovery, distributed ML training, cluster deploy.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~8,500
**Dependency:** Phase 2 complete. Option E (Python/NumPy bridge) improves D5 (distributed ML).
**Existing:** 4,235 LOC, 78 tests (RPC, TCP, cluster scheduling framework).

### Sprint D1: Raft Consensus Protocol (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D1.1 | Raft state machine | [ ] | Leader/Follower/Candidate with term tracking | 150 | Transitions correct |
| D1.2 | Leader election | [ ] | RequestVote RPC, randomized timeout | 200 | Elected within 2 timeouts |
| D1.3 | Log replication | [ ] | AppendEntries RPC, consistency check | 200 | Replicated to majority |
| D1.4 | Commit & apply | [ ] | Apply committed entries | 80 | State reflects commits |
| D1.5 | Persistence | [ ] | WAL for term, votedFor, log | 100 | Recovers after restart |
| D1.6 | Snapshot | [ ] | Compact log with state snapshot | 100 | Log size reduced 90%+ |
| D1.7 | Membership change | [ ] | Joint consensus for add/remove | 120 | No downtime |
| D1.8 | Pre-vote extension | [ ] | Prevent disruption from partitioned nodes | 60 | No false elections |
| D1.9 | Lease-based reads | [ ] | Fast linearizable reads | 80 | < 1ms read latency |
| D1.10 | 10 Raft tests | [ ] | Election, replication, partition, recovery | 200 | All 10 pass |

**Sprint D1 Gate:** Raft consensus correct per paper, tested with simulated partitions.

### Sprint D2: Dynamic Service Discovery (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D2.1 | mDNS discovery | [ ] | Multicast DNS for LAN | 100 | Nodes find each other |
| D2.2 | Seed node bootstrap | [ ] | `--seed=host1:9000,host2:9000` for WAN | 60 | Cluster forms |
| D2.3 | DNS-based discovery | [ ] | SRV records | 80 | Nodes via DNS |
| D2.4 | Gossip protocol | [ ] | SWIM-based failure detection | 150 | Converges in 5 rounds |
| D2.5 | Node metadata | [ ] | Advertise CPU, memory, GPU | 50 | Scheduler sees caps |
| D2.6 | Health checking | [ ] | Periodic probes | 60 | Unhealthy removed |
| D2.7 | Auto-scaling events | [ ] | Events on join/leave | 40 | K8s integration |
| D2.8 | Service registry | [ ] | Name -> address mapping | 60 | `rpc_connect("svc")` |
| D2.9 | Discovery config | [ ] | `[cluster.discovery]` in fj.toml | 40 | Config controls method |
| D2.10 | 10 discovery tests | [ ] | mDNS, seed, DNS, gossip, health | 150 | All 10 pass |

**Sprint D2 Gate:** Nodes discover each other via multiple methods.

### Sprint D3: Distributed Task Scheduler (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D3.1 | Task definition | [ ] | `@distributed fn process(data: Tensor)` | 60 | Annotation parsed |
| D3.2 | Task placement | [ ] | Schedule on best resource match | 80 | GPU task -> GPU node |
| D3.3 | Data locality | [ ] | Prefer nodes with input data | 60 | Reduces network |
| D3.4 | Load balancing | [ ] | Round-robin, least-loaded, weighted | 80 | Even distribution |
| D3.5 | Task queue | [ ] | Priority queue with fairness | 60 | High-priority first |
| D3.6 | Task cancellation | [ ] | Cancel with cleanup | 40 | Resources freed |
| D3.7 | Task retry | [ ] | Exponential backoff | 50 | Transient recovered |
| D3.8 | Task dependencies | [ ] | DAG-based scheduling | 80 | Order respected |
| D3.9 | Resource reservations | [ ] | Reserve CPU/memory/GPU | 60 | No over-commit |
| D3.10 | 10 scheduler tests | [ ] | Placement, locality, balance, cancel, DAG | 150 | All 10 pass |

**Sprint D3 Gate:** Tasks scheduled optimally across cluster.

### Sprint D4: Distributed Data Plane (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D4.1 | Data partitioning | [ ] | Shard tensor by rows/columns | 100 | 1000x100 -> 4 shards |
| D4.2 | Data transfer | [ ] | Zero-copy TCP for large tensors | 120 | 100MB < 1s LAN |
| D4.3 | Serialization | [ ] | Efficient binary (header + raw) | 80 | Shape + dtype preserved |
| D4.4 | Compression | [ ] | Optional LZ4 | 60 | 2x for sparse |
| D4.5 | Scatter | [ ] | Distribute to N workers | 60 | All workers receive |
| D4.6 | Gather | [ ] | Collect from N workers | 60 | Correct order |
| D4.7 | Broadcast | [ ] | Same data to all | 40 | Identical data |
| D4.8 | AllReduce | [ ] | Real ring-allreduce for gradients | 150 | Summed across 4 nodes |
| D4.9 | Pipeline parallelism | [ ] | Stream through stages | 100 | Throughput > single |
| D4.10 | 10 data plane tests | [ ] | Shard, transfer, scatter, gather, allreduce | 150 | All 10 pass |

**Sprint D4 Gate:** Ring-allreduce works, data transfers at near wire speed.

### Sprint D5: Distributed ML Training (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D5.1 | Data-parallel training | [ ] | Same model, different batches on N workers | 150 | Accuracy matches single |
| D5.2 | Gradient sync | [ ] | AllReduce after each batch | 80 | Averaged correctly |
| D5.3 | Model-parallel training | [ ] | Split model across nodes | 120 | GPT-style split |
| D5.4 | Parameter server | [ ] | Centralized with async updates | 100 | Push/pull params |
| D5.5 | LR scaling | [ ] | `base_lr * num_workers` | 30 | Linear scaling |
| D5.6 | Checkpoint saving | [ ] | Distributed checkpoint | 80 | Saved from all workers |
| D5.7 | Checkpoint loading | [ ] | Resume from checkpoint | 60 | Training resumes |
| D5.8 | Mixed precision | [ ] | FP16 communication, FP32 compute | 60 | 2x comm speedup |
| D5.9 | Elastic training | [ ] | Add/remove workers live | 100 | Training continues |
| D5.10 | 10 distributed ML tests | [ ] | Data-parallel MNIST, gradient, checkpoint | 150 | All 10 pass |

**Sprint D5 Gate:** Distributed MNIST training matches single-node accuracy.

### Sprint D6: RPC Framework Completion (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D6.1 | Bidirectional streaming | [ ] | Client + server stream simultaneously | 100 | Chat works |
| D6.2 | RPC timeout | [ ] | Per-call deadline propagation | 50 | Timeout -> Err |
| D6.3 | RPC compression | [ ] | gzip/lz4 for large payloads | 60 | 10x text reduction |
| D6.4 | RPC authentication | [ ] | TLS mutual auth + bearer | 100 | Unauth rejected |
| D6.5 | RPC interceptors | [ ] | Pre/post hooks for logging, auth | 80 | Chain executes |
| D6.6 | RPC load balancing | [ ] | Client-side across replicas | 60 | Distributed |
| D6.7 | RPC reflection | [ ] | List methods and types | 40 | `list_methods()` |
| D6.8 | RPC health service | [ ] | Standard health endpoint | 30 | `check_health()` |
| D6.9 | RPC metrics | [ ] | Count, latency, error rate | 60 | Prometheus-compatible |
| D6.10 | 10 RPC tests | [ ] | Streaming, timeout, auth, interceptors | 150 | All 10 pass |

**Sprint D6 Gate:** Production-grade RPC with auth, compression, metrics.

### Sprint D7: Fault Tolerance (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D7.1 | Leader failover | [ ] | New leader within 5s | 60 | Client reconnects |
| D7.2 | Worker failover | [ ] | Tasks reassigned | 80 | Training continues |
| D7.3 | Network partition | [ ] | Raft majority prevents split-brain | 60 | Minority read-only |
| D7.4 | Graceful shutdown | [ ] | SIGTERM -> drain -> deregister | 60 | No lost work |
| D7.5 | Data replication | [ ] | Replicate to N nodes | 80 | Survives 1 failure |
| D7.6 | Circuit breaker | [ ] | Stop sending to failing nodes | 60 | Cascade prevented |
| D7.7 | Backpressure | [ ] | Slow consumer -> producer backs off | 50 | No OOM |
| D7.8 | Idempotent ops | [ ] | Retry-safe execution | 40 | No side effects |
| D7.9 | Split-brain recovery | [ ] | Reconciliation after heal | 80 | Cluster reconverges |
| D7.10 | 10 fault tests | [ ] | Failover, partition, shutdown, circuit | 150 | All 10 pass |

**Sprint D7 Gate:** Cluster survives node failures, partitions, and graceful shutdown.

### Sprint D8: CLI & Deployment (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D8.1 | `fj run --cluster` | [ ] | Run distributed job | 40 | Dispatches to cluster |
| D8.2 | `fj cluster status` | [ ] | Show nodes, health, load | 60 | Table displayed |
| D8.3 | `fj cluster join` | [ ] | Join existing cluster | 30 | Joins and receives work |
| D8.4 | `fj cluster leave` | [ ] | Gracefully leave | 20 | Deregisters |
| D8.5 | `[cluster]` config | [ ] | fj.toml cluster section | 40 | Config controls behavior |
| D8.6 | Docker deployment | [ ] | Dockerfile for cluster node | 50 | `docker run fj-node` |
| D8.7 | Kubernetes deploy | [ ] | Helm chart + StatefulSet | 100 | `helm install` works |
| D8.8 | Monitoring dashboard | [ ] | Grafana template | 100 | Shows nodes, tasks |
| D8.9 | Log aggregation | [ ] | Structured JSON logging | 40 | Parseable by fluentd |
| D8.10 | 10 deployment tests | [ ] | CLI, Docker, K8s, monitoring | 150 | All 10 pass |

**Sprint D8 Gate:** Cluster deployable via CLI, Docker, and Kubernetes.

### Sprint D9: Security & Multi-Tenancy (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D9.1 | TLS everywhere | [ ] | All communication over mTLS | 80 | Plaintext rejected |
| D9.2 | Certificate rotation | [ ] | Auto renewal before expiry | 60 | No downtime |
| D9.3 | RBAC | [ ] | admin, scheduler, worker, reader | 80 | Worker can't admin |
| D9.4 | Resource quotas | [ ] | Per-user CPU/memory/GPU limits | 60 | Over-quota queued |
| D9.5 | Audit logging | [ ] | All ops with user attribution | 40 | Who, what, when |
| D9.6 | Secrets management | [ ] | Encrypted storage | 60 | Encrypted at rest |
| D9.7 | Network policies | [ ] | Limit to cluster ports | 40 | External blocked |
| D9.8 | Sandboxed execution | [ ] | Isolated environment | 80 | No host FS access |
| D9.9 | Data encryption | [ ] | Encrypt tensor in transit | 50 | Wireshark encrypted |
| D9.10 | 10 security tests | [ ] | TLS, RBAC, quotas, sandbox | 150 | All 10 pass |

**Sprint D9 Gate:** Cluster secure with mTLS, RBAC, encryption, sandboxing.

### Sprint D10: Benchmarks & Documentation (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| D10.1 | Single-node baseline | [ ] | MNIST training time | 30 | Baseline recorded |
| D10.2 | 2-node speedup | [ ] | MNIST on 2 nodes | 20 | > 1.8x |
| D10.3 | 4-node speedup | [ ] | MNIST on 4 nodes | 20 | > 3.2x |
| D10.4 | AllReduce latency | [ ] | 10MB gradient on LAN | 30 | < 100ms |
| D10.5 | Failure recovery time | [ ] | Node failure -> reassignment | 20 | < 10s |
| D10.6 | Election benchmark | [ ] | Election convergence | 20 | < 3s |
| D10.7 | Scalability test | [ ] | 16 simulated nodes | 40 | Linear scaling |
| D10.8 | Documentation | [ ] | `book/distributed_runtime.md` | 200 | Architecture guide |
| D10.9 | Example: distributed MNIST | [ ] | `examples/distributed_mnist.fj` | 150 | End-to-end works |
| D10.10 | Update GAP_ANALYSIS_V2 | [ ] | Mark distributed 100% production | 20 | Audit updated |

**Sprint D10 Gate:** Linear scaling verified, fully documented.

---

## Option G: Self-Hosting Compiler

**Goal:** Stage 2 bootstrap — compiler compiles itself in Fajar Lang.
**Sprints:** 10 | **Tasks:** 100 | **LOC:** ~7,000
**Dependency:** Phase 1 complete. Option H (const eval) improves S4 (bytecode codegen).
**Existing:** 3,076 LOC in stdlib/ (lexer 513, parser 784, analyzer 432).

### Sprint S1: Tree-Based AST (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S1.1 | AST node hierarchy | [ ] | `enum Expr { Int(i64), BinOp(...), ... }` in Fajar | 150 | 25+ variants |
| S1.2 | Statement nodes | [ ] | `enum Stmt { Let, Fn, Struct, Enum, While, ... }` | 100 | All statement types |
| S1.3 | Type expression nodes | [ ] | `enum TypeExpr { Name, Generic, Array, Ref, ... }` | 80 | Type AST |
| S1.4 | Pattern nodes | [ ] | `enum Pattern { Ident, Tuple, Struct, Enum, _ }` | 60 | Pattern AST |
| S1.5 | Program node | [ ] | `struct Program { items: Array<Item> }` root | 30 | Top-level structure |
| S1.6 | Span tracking | [ ] | Every node carries `span: Span` | 40 | Spans correct |
| S1.7 | AST pretty printer | [ ] | `fn print_ast(node: Expr) -> str` | 80 | Round-trips |
| S1.8 | AST visitor pattern | [ ] | `fn walk(node, visitor)` | 60 | Traverses tree |
| S1.9 | AST serialization | [ ] | AST to JSON for debugging | 80 | Round-trip works |
| S1.10 | 10 AST tests | [ ] | Node creation, printer, visitor, serialization | 150 | All 10 pass |

**Sprint S1 Gate:** Tree-based AST replaces flat arrays in self-hosted parser.

### Sprint S2: Parser Upgrade (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S2.1 | Pratt expression parser | [ ] | Port 19-level Pratt to Fajar | 200 | Precedence correct |
| S2.2 | Statement parser | [ ] | let/fn/struct/enum/impl/trait/while/for/match | 200 | All types parsed |
| S2.3 | Type expression parser | [ ] | `Array<i32>`, `&T`, `fn(i32) -> bool` | 100 | Complex types |
| S2.4 | Pattern parser | [ ] | `Some(x)`, `(a, b)`, `Point { x, y }` | 80 | All patterns |
| S2.5 | Error recovery | [ ] | Sync on `;`/`}`, continue | 80 | Multiple errors |
| S2.6 | Precedence table | [ ] | All 19 levels in Fajar data structure | 40 | Matches spec |
| S2.7 | Parenthesized exprs | [ ] | `(a + b) * c` | 20 | Override works |
| S2.8 | If/match as exprs | [ ] | `let x = if cond { a } else { b }` | 40 | Expression position |
| S2.9 | Lambda expressions | [ ] | `|x, y| x + y` parsed | 40 | Closure AST correct |
| S2.10 | 10 parser tests | [ ] | Compare to Rust parser for 10 programs | 150 | All match |

**Sprint S2 Gate:** Self-hosted parser produces identical AST to Rust parser.

### Sprint S3: Semantic Analyzer Upgrade (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S3.1 | Symbol table | [ ] | Scope stack with HashMap per scope | 100 | Nested scopes work |
| S3.2 | Type inference | [ ] | Infer from initializers and returns | 120 | `let x = 42` -> i64 |
| S3.3 | Type checking | [ ] | Binary ops, calls, fields | 100 | Mismatch detected |
| S3.4 | Scope resolution | [ ] | Block, function, module scoping | 80 | Shadowing works |
| S3.5 | Use-after-move | [ ] | Track moved values | 60 | Error on use |
| S3.6 | Mutability checking | [ ] | `let mut` required | 40 | Error without mut |
| S3.7 | Exhaustiveness | [ ] | Match covers all variants | 80 | Missing -> error |
| S3.8 | Generic checking | [ ] | Monomorphization-aware | 80 | Generic fns checked |
| S3.9 | Trait bound checking | [ ] | `where T: Display` enforced | 60 | Missing impl -> error |
| S3.10 | 10 analyzer tests | [ ] | Types, scopes, moves, generics, traits | 150 | All 10 pass |

**Sprint S3 Gate:** Self-hosted analyzer catches same errors as Rust analyzer.

### Sprint S4: Bytecode Codegen in Fajar (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S4.1 | Opcode definition | [ ] | 45 opcodes matching VM spec | 60 | All defined |
| S4.2 | Expression compilation | [ ] | BinOp, UnaryOp, Call, Index | 150 | Arithmetic works |
| S4.3 | Control flow | [ ] | if/while/for/loop/break/continue | 120 | Flow correct |
| S4.4 | Function compilation | [ ] | fn defs with locals and returns | 100 | Calls work |
| S4.5 | Variable compilation | [ ] | let/mut/assign with slot allocation | 60 | Store/load works |
| S4.6 | String/array | [ ] | Literals and operations | 80 | Composites work |
| S4.7 | Struct compilation | [ ] | Init, field access, methods | 80 | Struct ops work |
| S4.8 | Match compilation | [ ] | Jump table / if-else chain | 80 | Patterns work |
| S4.9 | Closure compilation | [ ] | Capture and invocation | 60 | Free vars captured |
| S4.10 | 10 codegen tests | [ ] | Arithmetic, control, functions, structs, closures | 150 | Correct bytecode |

**Sprint S4 Gate:** Self-hosted compiler generates valid bytecode for VM.

### Sprint S5: Standard Library in Fajar (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S5.1 | String operations | [ ] | len, contains, split, trim, replace | 100 | 10 methods |
| S5.2 | Array operations | [ ] | push, pop, map, filter, sort | 100 | 10 methods |
| S5.3 | HashMap in Fajar | [ ] | insert, get, contains, remove, keys | 120 | Chaining |
| S5.4 | File I/O wrappers | [ ] | read_file, write_file via builtins | 40 | Round-trip |
| S5.5 | Math functions | [ ] | abs, min, max, pow, sqrt | 60 | Accuracy verified |
| S5.6 | Error types | [ ] | Result<T,E>, Option<T> with methods | 60 | `?` works |
| S5.7 | Iterator protocol | [ ] | .iter(), .map(), .filter(), .collect() | 100 | Chain works |
| S5.8 | Formatting | [ ] | `format("Hello {}", name)` | 60 | Strings work |
| S5.9 | Debug printing | [ ] | `dbg(value)` | 30 | Helpful output |
| S5.10 | 10 stdlib tests | [ ] | String, array, map, file, math, iterator | 150 | All 10 pass |

**Sprint S5 Gate:** Self-hosted compiler has sufficient stdlib for bootstrap.

### Sprint S6: Stage 1 Bootstrap (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S6.1 | Compile lexer.fj | [ ] | Self-hosted compiles lexer to bytecode | 40 | Bytecode produced |
| S6.2 | Compile parser.fj | [ ] | Self-hosted compiles parser | 40 | Bytecode produced |
| S6.3 | Compile analyzer.fj | [ ] | Self-hosted compiles analyzer | 40 | Bytecode produced |
| S6.4 | Compile codegen.fj | [ ] | Self-hosted compiles codegen | 40 | Bytecode produced |
| S6.5 | Compile compiler.fj | [ ] | Self-hosted compiles ITSELF (Stage 1!) | 40 | Stage 1 achieved |
| S6.6 | Differential testing | [ ] | Stage 0 (Rust) vs Stage 1 (Fajar) output | 100 | Identical bytecode |
| S6.7 | Performance comparison | [ ] | Stage 0 vs Stage 1 speed | 30 | Within 5x |
| S6.8 | Error message comparison | [ ] | Same errors reported | 40 | Parity |
| S6.9 | 100-program test | [ ] | Compile 100 programs with Stage 1 | 60 | All correct |
| S6.10 | Document Stage 1 | [ ] | `book/self_hosting.md` | 100 | Process documented |

**Sprint S6 Gate:** Stage 1 compiler produces identical output to Rust compiler.

### Sprint S7: Stage 2 Bootstrap (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S7.1 | Fix stack overflow | [ ] | Increase limit or trampolining | 80 | Deep recursion handled |
| S7.2 | Fix closure capture | [ ] | All free vars captured | 60 | Closures work |
| S7.3 | Fix generic instantiation | [ ] | Self-hosted handles own generics | 80 | Generics compile |
| S7.4 | Fix pattern matching | [ ] | All compiler's own patterns | 60 | Complex patterns |
| S7.5 | Stage 2 compilation | [ ] | Stage 1 compiles itself -> Stage 2 | 40 | Stage 2 produced |
| S7.6 | Stage 2 validation | [ ] | Stage 2 == Stage 1 output | 60 | Fixed point |
| S7.7 | Triple bootstrap | [ ] | Stage 2 -> Stage 3, verify 2 == 3 | 30 | Fixed point reached |
| S7.8 | CI bootstrap test | [ ] | GitHub Actions Stage 1 -> 2 | 40 | Bootstrap in CI |
| S7.9 | Reproducible bootstrap | [ ] | Identical across platforms | 40 | Linux == macOS == Windows |
| S7.10 | Bootstrap documentation | [ ] | Full procedure documented | 100 | Step-by-step |

**Sprint S7 Gate:** Stage 2 achieved, fixed point verified, reproducible.

### Sprint S8: Performance Optimization (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S8.1 | String interning | [ ] | Intern identifiers/keywords | 80 | 50% fewer allocs |
| S8.2 | Arena allocation | [ ] | AST nodes from arena | 100 | 3x faster |
| S8.3 | Inline caching | [ ] | Cache method lookups | 60 | 2x faster calls |
| S8.4 | Tail call optimization | [ ] | Detect/optimize tail recursion | 60 | No stack overflow |
| S8.5 | Constant folding | [ ] | `2 + 3` -> `5` at compile | 60 | Folded |
| S8.6 | Dead code elimination | [ ] | Remove unreachable | 60 | Smaller bytecode |
| S8.7 | Register allocation | [ ] | Optimize local->register | 80 | Fewer stack accesses |
| S8.8 | Peephole optimization | [ ] | Common bytecode patterns | 60 | 10% smaller |
| S8.9 | 10K LOC benchmark | [ ] | Compile 10K LOC project | 30 | < 5s |
| S8.10 | 10 optimization tests | [ ] | Interning, arena, TCO, fold, peephole | 150 | All 10 pass |

**Sprint S8 Gate:** Self-hosted compiler within 3x of Rust compiler speed.

### Sprint S9: Error Messages & UX (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S9.1 | Source snippets | [ ] | Show `^^^` underline | 80 | Error shows snippet |
| S9.2 | Error codes | [ ] | PE001, SE004, etc. | 40 | Codes match Rust |
| S9.3 | Suggestions | [ ] | "Did you mean `println`?" | 60 | Edit distance |
| S9.4 | Multi-error display | [ ] | Show all errors | 40 | Multiple displayed |
| S9.5 | Color output | [ ] | ANSI colors | 40 | Colored terminal |
| S9.6 | Error recovery | [ ] | Continue after errors | 60 | Max errors reported |
| S9.7 | Warning system | [ ] | Unused vars, dead code | 40 | Don't block compilation |
| S9.8 | Help messages | [ ] | `--explain PE001` | 60 | Explanation |
| S9.9 | JSON error output | [ ] | `--error-format=json` for IDE | 40 | JSON array |
| S9.10 | 10 error UX tests | [ ] | Snippets, suggestions, multi, colors, JSON | 150 | All 10 pass |

**Sprint S9 Gate:** Error messages on par with Rust compiler quality.

### Sprint S10: Validation & Completion (10 tasks)

| # | Task | Status | Detail | LOC | Verify |
|---|------|--------|--------|-----|--------|
| S10.1 | Compile all examples | [ ] | 178 examples with self-hosted | 60 | 178/178 compile |
| S10.2 | Compile stdlib | [ ] | All stdlib/*.fj | 40 | All compile |
| S10.3 | Self-test suite | [ ] | Compiler tests on self-hosted | 60 | Tests pass |
| S10.4 | Feature parity audit | [ ] | Compare vs Rust compiler | 100 | 90%+ parity |
| S10.5 | Binary size comparison | [ ] | Self-hosted vs Rust | 20 | Documented |
| S10.6 | Memory comparison | [ ] | Memory during compilation | 20 | Documented |
| S10.7 | Speed comparison | [ ] | Side-by-side on 10K LOC | 20 | Documented |
| S10.8 | Example: self-hosted build | [ ] | `fj build --self-hosted examples/hello.fj` | 40 | End-to-end works |
| S10.9 | Update CLAUDE.md | [ ] | Document self-hosting in architecture | 30 | Updated |
| S10.10 | Update GAP_ANALYSIS_V2 | [ ] | Mark self-hosting 100% production | 20 | Audit updated |

**Sprint S10 Gate:** Self-hosting verified with 178 examples, Stage 2 in CI.

---

# ============================================================
# PROGRESS SUMMARY
# ============================================================

## Phase 1: Foundation

| Option | Sprint | Tasks Done | Total | Status |
|--------|--------|-----------|-------|--------|
| **A: CI Green** | A1 | 10 | 10 | COMPLETE |
| **H: Const Fn** | K1-K10 | 100 | 100 | COMPLETE |
| **C: Incremental** | I1-I10 | 100 | 100 | COMPLETE |

## Phase 2: Ecosystem

| Option | Sprint | Tasks Done | Total | Status |
|--------|--------|-----------|-------|--------|
| **B: WASI P2** | W1-W10 | 0 | 100 | PENDING |
| **E: FFI v2** | E1-E10 | 0 | 100 | PENDING |

## Phase 3: Differentiation

| Option | Sprint | Tasks Done | Total | Status |
|--------|--------|-----------|-------|--------|
| **F: SMT Verify** | V1-V10 | 0 | 100 | PENDING |
| **D: Distributed** | D1-D10 | 0 | 100 | PENDING |
| **G: Self-Hosting** | S1-S10 | 0 | 100 | PENDING |

### Grand Total

```
Phase 1: 210 / 210 tasks complete  (100.0%) ← PHASE 1 COMPLETE!
Phase 2:   0 / 200 tasks complete  ( 0.0%)
Phase 3:   0 / 300 tasks complete  ( 0.0%)
─────────────────────────────────────────
TOTAL:   210 / 710 tasks complete  (29.6%)
```

---

*V13_TASKS.md Version 1.0 | 710 tasks, 71 sprints, 3 phases | Created 2026-03-31*
*Author: Fajar (PrimeCore.id) + Claude Opus 4.6*
