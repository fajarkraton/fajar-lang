# Code Quality Improvement Plan

> **Scope:** Production code quality fixes — no hardware required.
> **Started:** 2026-03-17
> **Verified by:** `cargo test --features native && cargo clippy -- -D warnings && cargo fmt -- --check`

---

## Overview

| Phase | Focus | Tasks | Status |
|-------|-------|-------|--------|
| **1** | Fix `.unwrap()` in production code | 3 files, 16 calls | **COMPLETE** |
| **2** | Cleanup commented-out code | LLVM module + others | **SKIPPED** (all clean) |
| **3** | `#[allow(dead_code)]` audit | 11 files, 23→2 annotations | **COMPLETE** |
| **4** | Large file modularization | compile/mod.rs split done | **4.1 COMPLETE** |

---

## Phase 1: Fix `.unwrap()` in Production Code — COMPLETE

> **Rule:** "NEVER use `.unwrap()` in `src/` — only allowed in `tests/` and `benches/`"
> **Verified:** 5,910 tests pass (--features native), clippy 0 warnings, fmt clean

### 1A — `src/interpreter/eval.rs` (8 calls → fixed)

| # | Line | Code | Fix | Status |
|---|------|------|-----|--------|
| 1.1 | 1913 | `params.into_iter().next().unwrap()` | `.ok_or_else(EvalError::Runtime(TypeError))` | [x] |
| 1.2 | 2323 | `args.next().unwrap()` (map_insert arg0) | `.ok_or_else(EvalError::Runtime(ArityMismatch))` | [x] |
| 1.3 | 2324 | `args.next().unwrap()` (map_insert arg1) | Same | [x] |
| 1.4 | 2325 | `args.next().unwrap()` (map_insert arg2) | Same | [x] |
| 1.5 | 2367 | `args.next().unwrap()` (map_remove arg0) | Same | [x] |
| 1.6 | 2368 | `args.next().unwrap()` (map_remove arg1) | Same | [x] |
| 1.7 | 6444 | `args_iter.next().unwrap()` (map insert method arg0) | Same | [x] |
| 1.8 | 6445 | `args_iter.next().unwrap()` (map insert method arg1) | Same | [x] |

### 1B — `src/runtime/ml/ops.rs` (7 calls → fixed)

| # | Line | Code | Fix | Status |
|---|------|------|-----|--------|
| 1.9 | 601 | `into_shape_with_order().unwrap()` (matmul grad g) | `match` + zero-gradient fallback | [x] |
| 1.10 | 605 | `into_shape_with_order().unwrap()` (matmul grad b) | Same | [x] |
| 1.11 | 609 | `into_shape_with_order().unwrap()` (matmul grad a) | Same | [x] |
| 1.12 | 744 | `from_shape_vec().unwrap()` (scalar grad) | `.unwrap_or_else` + `from_elem` fallback | [x] |
| 1.13 | 771 | `from_shape_vec().unwrap()` (broadcast reshape) | `.unwrap_or_else` + clone fallback | [x] |
| 1.14 | 916 | `from_shape_vec().unwrap()` (cross-entropy grad) | `.unwrap_or_else` + zeros fallback | [x] |
| 1.15 | 951 | `from_shape_vec().unwrap()` (BCE grad) | Same | [x] |

### 1C — `src/parser/mod.rs` (2 calls → fixed)

| # | Line | Code | Fix | Status |
|---|------|------|-----|--------|
| 1.16 | 1564 | `infix_binding_power(&kind).unwrap()` (pipe) | `.ok_or_else(ParseError::UnexpectedToken)` | [x] |
| 1.17 | 1581 | `infix_binding_power(&kind).unwrap()` (range) | Same | [x] |

### Notes

- **Parser `panic!()` calls**: ALL 80+ panics are inside `#[cfg(test)] mod tests` (starts line 3059) — NOT production violations. No fix needed.
- **type_check.rs `panic!()`**: Line 6744 is inside `#[cfg(test)] mod tests` (starts line 4561) — NOT a violation.
- **Autograd closures**: `GradFn = Box<dyn Fn(&ArrayD) -> Vec<ArrayD>>` cannot return `Result`, so used `match`/`unwrap_or_else` with safe fallbacks (zero gradients) instead of panicking.

---

## Phase 2: Cleanup Commented-Out Code — SKIPPED (No Action Needed)

| # | File | Finding | Status |
|---|------|---------|--------|
| 2.1 | `src/codegen/llvm/mod.rs` | 29 blocks are **intentional test docs** (Fajar Lang source shown inline) — NOT dead code | CLEAN |
| 2.2 | `src/analyzer/polonius/facts.rs` | 0 commented-out code — all comments are test scenario descriptions | CLEAN |
| 2.3 | `src/analyzer/polonius/solver.rs` | 0 commented-out code — all comments are datalog rule docs | CLEAN |

---

## Phase 3: `#[allow(dead_code)]` Audit — COMPLETE

> **Result:** 23 annotations → 2 remaining (justified). 21 annotations removed.
> **Verified:** 5,910 tests pass (--features native), clippy 0 warnings, fmt clean

| # | File | Action | Status |
|---|------|--------|--------|
| 3.1 | `type_check.rs` | `TraitMethodSig` unused (removed by sed), `type_satisfies_trait()` kept (used by tests) | [x] |
| 3.2 | `context.rs` | 6 fields: `generic_enum_defs`, `trait_impls`, `async_fns`, `future_handles`, `last_future_new`, `current_context` — annotations removed (not actually dead) | [x] |
| 3.3 | `npu/qnn.rs` | `execute_graph()` — annotation removed (not actually dead) | [x] |
| 3.4 | `fp4.rs` | `EXP_BIAS` → `_EXP_BIAS` (format documentation constant) | [x] |
| 3.5 | `vm/engine.rs` | `function_index` → `_function_index` (stored for debugging) | [x] |
| 3.6 | `parser/macros.rs` | No `#[allow(dead_code)]` found | [x] |
| 3.7 | `freertos.rs` | 3 sim structs: unused fields prefixed with `_` | [x] |
| 3.8 | `zephyr.rs` | 3 sim structs: unused fields prefixed with `_` | [x] |
| 3.9 | `dap_server.rs` | `source_file/source_code/server_output` → `_` prefix | [x] |
| 3.10 | `hw/gpu.rs` | `GpuDetectError` — kept `#[allow(dead_code)]` on enum (tuple variant fields can't prefix) | [x] |
| 3.11 | `fp8.rs` | `EXP_BITS` → `_EXP_BITS` x2 (format documentation constants) | [x] |
| 3.12 | `runtime_fns.rs` | `mutex_ptr` — annotation removed (not actually dead) | [x] |

---

## Phase 4: Large File Modularization

### 4.1 — `compile/mod.rs` split — COMPLETE

> **Before:** 6,312 LOC in single file | **After:** 38 LOC mod.rs + 3 new files
> **Verified:** 6,463 tests pass (--features native), clippy 0 warnings, fmt clean

| File | LOC | Content |
|------|-----|---------|
| `mod.rs` | 38 | Module declarations and re-exports |
| `call.rs` | 3,092 | `compile_call`, `compile_regular_call`, `compile_path_call`, `compile_enum_constructor`, `compile_generic_call`, `infer_semantic_type`, `compile_fn_ptr_call` |
| `method.rs` | 2,600 | `compile_method_call`, `compile_map_method` |
| `asm.rs` | 630 | `compile_inline_asm`, `validate_asm_operand_type`, `validate_asm_register_class`, `extract_ident_name` |

### 4.2 — `eval.rs` split (Future)

| # | File | LOC | Target |
|---|------|-----|--------|
| 4.2 | `src/interpreter/eval.rs` | 8,389 | Split into `eval/expr.rs`, `eval/stmt.rs`, `eval/builtins.rs` |

### 4.3 — `type_check.rs` split (Future)

| # | File | LOC | Target |
|---|------|-----|--------|
| 4.3 | `src/analyzer/type_check.rs` | 7,344 | Split into `type_rules.rs`, `inference.rs` |

---

*Plan Version: 1.1 | Updated: 2026-03-17 | Phase 1 COMPLETE*
