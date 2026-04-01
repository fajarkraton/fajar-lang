# V16 "Horizon" — Implementation Tasks

> **Master Tracking Document** — Gap closure + new capabilities.
> **Rule:** Work per-phase, per-sprint. Complete ALL tasks in a sprint before moving to the next.
> **Marking:** `[x]` = done (verified by `fj run`), `[f]` = framework/cargo test only, `[ ]` = pending
> **Previous:** V15 "Delivery" — 46/120 [x], 74 [f]. Effect system fixed, ML runtime enhanced.

---

## Completed Features (v12.2.0)

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| Q1 | Array concat `+` | [x] | Analyzer + interpreter: `[1,2] + [3,4]` = `[1,2,3,4]` | ✅ |
| Q2 | Binary file I/O | [x] | `read_binary(path)` / `write_binary(path, bytes)` | ✅ |
| Q3 | `@gpu` annotation | [x] | Lexer → Parser → Analyzer → LSP full pipeline | ✅ |
| G1.1 | `@gpu` context rules | [x] | Blocks file I/O, raw ptrs, heap. Allows math + tensors | ✅ |
| L1.1 | `arr.pop()` | [x] | Remove and return last element | ✅ |
| L1.2 | `arr.insert(idx, val)` | [x] | Insert element at index | ✅ |
| L1.3 | `arr.remove(idx)` | [x] | Remove element at index | ✅ |
| L1.4 | `arr.index_of(val)` | [x] | Find first occurrence, -1 if not found | ✅ |
| L1.5 | `str.to_upper()` / `to_lower()` | [x] | Shorthand aliases for to_uppercase/to_lowercase | ✅ |
| L1.6 | `len()` returns `i64` | [x] | Fixed usize/i64 friction — #1 pain point | ✅ |
| L2.1 | `if let` expression | [x] | Desugars to match, works as expression | ✅ |
| L2.2 | `while let` expression | [x] | Desugars to loop+match, auto-break on mismatch | ✅ |
| I3.2 | JSON pretty-printer | [x] | `json_fmt.fj` — char-level formatting with indent | ✅ |
| I3.3 | CSV to JSON converter | [x] | `csv2json.fj` — headers → object keys | ✅ |
| I3.5 | Expression calculator | [x] | `calc.fj` — two-pass eval with precedence | ✅ |
| R1.1 | MNIST IDX binary loader | [x] | `mnist_real.fj` — parses IDX format headers | ✅ |

---

## Execution Order & Dependencies

```
OPTION 1 — GPU CODEGEN PIPELINE (wires @gpu to real SPIR-V/PTX)
  Sprint G1: @gpu context rules ........... 10 tasks  (depends on Q3)
  Sprint G2: SPIR-V emission .............. 10 tasks  (depends on G1)
  Sprint G3: PTX emission + CUDA .......... 10 tasks  (depends on G1)

OPTION 2 — LANGUAGE COMPLETENESS
  Sprint L1: Array/string methods ......... 10 tasks  (NO dependency)
  Sprint L2: Pattern matching enhancements . 10 tasks  (NO dependency)
  Sprint L3: Error handling ergonomics ..... 10 tasks  (NO dependency)

OPTION 3 — REAL-WORLD DEPLOYMENT
  Sprint R1: MNIST with real data ......... 10 tasks  (depends on Q2 binary I/O)
  Sprint R2: WebAssembly deployment ....... 10 tasks  (NO dependency)
  Sprint R3: Package ecosystem ............ 10 tasks  (NO dependency)

OPTION 4 — DEVELOPER EXPERIENCE
  Sprint X1: REPL improvements ............ 10 tasks  (NO dependency)
  Sprint X2: Debugger ..................... 10 tasks  (NO dependency)
  Sprint X3: Documentation ................ 10 tasks  (NO dependency)

TOTAL: 12 sprints, 120 tasks
```

---

# ============================================================
# OPTION 1: GPU CODEGEN PIPELINE
# ============================================================

## Sprint G1: @gpu Context Rules

**Goal:** Enforce @gpu context restrictions and provide helpful error messages.
**Tasks:** 10 | **Dependency:** Q3 complete.
**Status:** PENDING

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| G1.1 | @gpu blocks raw pointers | [ ] | `@gpu fn f() { let p: *mut i32 = ... }` → error | `fj check`: error on raw ptr in @gpu |
| G1.2 | @gpu blocks syscalls | [ ] | `@gpu fn f() { syscall_dispatch(0) }` → error | `fj check`: error on syscall in @gpu |
| G1.3 | @gpu allows tensor ops | [ ] | `@gpu fn f() { let t = zeros(3,3) }` → OK | `fj run`: tensor ops work in @gpu |
| G1.4 | @gpu allows math builtins | [ ] | `@gpu fn f() { sin(3.14) }` → OK | `fj run`: math works in @gpu |
| G1.5 | @gpu blocks file I/O | [ ] | `@gpu fn f() { read_file("x") }` → error | `fj check`: error on file I/O in @gpu |
| G1.6 | @gpu blocks heap strings | [ ] | `@gpu fn f() { let s = "hello" }` → error (GPU has no heap) | `fj check`: error |
| G1.7 | @gpu allows shared memory | [ ] | Shared memory annotation for GPU kernels | `fj check`: shared memory OK |
| G1.8 | @gpu thread indexing | [ ] | Built-in `thread_idx()`, `block_idx()`, `block_dim()` | `fj run`: returns mock values |
| G1.9 | @gpu <-> @device bridge | [ ] | @device can call @gpu functions, not vice versa | `fj check`: correct context rules |
| G1.10 | @gpu test suite | [ ] | 10 .fj test programs for @gpu context | All 10 pass |

---

## Sprint G2: SPIR-V Emission

**Goal:** Wire @gpu functions to actual SPIR-V binary output.
**Tasks:** 10 | **Dependency:** G1 complete.
**Status:** PENDING

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| G2.1 | SPIR-V module structure | [ ] | Generate valid SPIR-V binary header from @gpu fn | Valid .spv file |
| G2.2 | Basic compute shader | [ ] | @gpu fn that does element-wise add → SPIR-V | spirv-val passes |
| G2.3 | Type mapping | [ ] | f32/f64/i32/i64 → SPIR-V OpTypeFloat/OpTypeInt | Correct types in SPIR-V |
| G2.4 | Buffer bindings | [ ] | Tensor args → SPIR-V storage buffers | Bindings in descriptor set |
| G2.5 | Thread dispatch | [ ] | thread_idx()/block_idx() → SPIR-V GlobalInvocationId | Correct built-in access |
| G2.6 | Control flow | [ ] | if/while in @gpu → SPIR-V OpBranch/OpLoopMerge | Valid structured CF |
| G2.7 | Math builtins | [ ] | sin/cos/sqrt → SPIR-V GLSL.std.450 extended instructions | Correct ext ops |
| G2.8 | fj build --target spirv | [ ] | CLI flag to emit .spv files | .spv file created |
| G2.9 | Vulkan compute dispatch | [ ] | Load .spv, create pipeline, dispatch compute | Result matches CPU |
| G2.10 | SPIR-V test suite | [ ] | 5 compute shaders verified with spirv-val | All valid |

---

# ============================================================
# OPTION 2: LANGUAGE COMPLETENESS
# ============================================================

## Sprint L1: Array & String Methods

**Goal:** Complete array/string method dispatch for common operations.
**Tasks:** 10 | **Dependency:** None.
**Status:** PENDING

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| L1.1 | arr.push(val) | [ ] | Mutably append element to array | `fj run`: push works |
| L1.2 | arr.pop() | [ ] | Remove and return last element | `fj run`: pop works |
| L1.3 | arr.reverse() | [ ] | Reverse array in place | `fj run`: reversed |
| L1.4 | arr.sort() | [ ] | Sort array in place (integers) | `fj run`: sorted |
| L1.5 | arr.map(fn) | [ ] | Apply function to each element, return new array | `fj run`: mapped |
| L1.6 | arr.filter(fn) | [ ] | Filter elements by predicate | `fj run`: filtered |
| L1.7 | str.to_upper() | [ ] | Convert string to uppercase | `fj run`: uppercased |
| L1.8 | str.to_lower() | [ ] | Convert string to lowercase | `fj run`: lowercased |
| L1.9 | str.chars() | [ ] | Return array of characters | `fj run`: char array |
| L1.10 | str.repeat(n) | [ ] | Repeat string n times | `fj run`: repeated |

---

## Sprint L2: Pattern Matching Enhancements

**Goal:** Add guard clauses, nested patterns, and or-patterns to match.
**Tasks:** 10 | **Dependency:** None.
**Status:** PENDING

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| L2.1 | Match guard `if` clause | [ ] | `match x { n if n > 0 => ... }` | `fj run`: guard filters |
| L2.2 | Nested struct patterns | [ ] | `match p { Point { x: 0, y } => ... }` | `fj run`: nested match |
| L2.3 | Or-patterns | [ ] | `match x { 1 \| 2 \| 3 => "small" }` | `fj run`: or-pattern |
| L2.4 | Range patterns | [ ] | `match x { 0..10 => "digit" }` | `fj run`: range match |
| L2.5 | Tuple patterns | [ ] | `match t { (0, y) => y }` | `fj run`: tuple match |
| L2.6 | Array patterns | [ ] | `match arr { [first, ..rest] => first }` | `fj run`: array destructure |
| L2.7 | Binding in patterns | [ ] | `match x { n @ 1..10 => use(n) }` | `fj run`: binding works |
| L2.8 | Exhaustiveness check | [ ] | Missing match arm produces warning | `fj check`: warning shown |
| L2.9 | if-let expression | [ ] | `if let Some(x) = opt { use(x) }` | `fj run`: if-let works |
| L2.10 | Pattern match tests | [ ] | 10 .fj test programs | All 10 pass |

---

# ============================================================
# OPTION 3: REAL-WORLD DEPLOYMENT
# ============================================================

## Sprint R1: MNIST with Real Data

**Goal:** Load real MNIST IDX files, train to 90%+ accuracy.
**Tasks:** 10 | **Dependency:** Q2 (binary I/O).
**Status:** PENDING

| # | Task | Status | Detail | Verify |
|---|------|--------|--------|--------|
| R1.1 | IDX header parser | [ ] | Parse magic number, dimensions from IDX binary format | `fj run`: reads header correctly |
| R1.2 | Image data loader | [ ] | Read 28x28 pixel images, normalize to [0,1] | `fj run`: 60K images loaded |
| R1.3 | Label loader | [ ] | Read label bytes, create one-hot targets | `fj run`: labels match |
| R1.4 | Batch iterator | [ ] | Split data into batches of 32 | `fj run`: correct batch sizes |
| R1.5 | Training loop | [ ] | Full epoch: batches → forward → loss → backward → SGD | `fj run`: loss decreases |
| R1.6 | Test evaluation | [ ] | Run trained model on test set | `fj run`: accuracy printed |
| R1.7 | Learning rate schedule | [ ] | Decrease LR after each epoch | `fj run`: LR changes |
| R1.8 | Achieve 90%+ | [ ] | Tune hyperparameters for 90%+ test accuracy | `fj run`: accuracy ≥ 90% |
| R1.9 | Training visualization | [ ] | Print loss curve as ASCII art | `fj run`: graph shown |
| R1.10 | MNIST tutorial | [ ] | Step-by-step tutorial with code | Document complete |

---

## Deferred to V17+

| Feature | Reason | Effort |
|---------|--------|--------|
| Dependent type user syntax (Pi/Sigma) | Major type theory work, limited practical value now | ~2,000 LOC |
| Live package registry server | Infrastructure + hosting required | ~3,000 LOC |
| Self-hosting compiler Stage 3 | Requires codegen completeness | ~5,000 LOC |

---

*V16 Tasks — Version 1.0 | Quick wins complete, 120 tasks planned | 2026-04-01*
