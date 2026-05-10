---
phase: FJARR_LEAK Phase 2 D-FULL — full-strict default-on
plan: docs/D_FULL_OPTION_B_PHASE_PLAN.md (Phase 1 B0 + Phases 2-5 implementation)
status: SHIPPED 2026-05-10 (v35.5.0)
prereq: v35.4.1 + Phase 1 (arena) + D-LITE (opt-in `--strict-ownership`)
purpose: closure findings for full-strict default-on; closes Compass §4.4 "@safe sebagai default"
---

# FJARR_LEAK Phase 2 D-FULL — Closure Findings (v35.5.0)

> Default ownership mode is now **full-strict**: all non-primitive types
> (str, [T], struct, enum, tuple-with-move-fields, tensor, quantized,
> etc.) are affine. Reuse after consume requires `.clone()`. The
> `--strict-ownership` CLI flag is preserved as a no-op for compatibility.

## §0 — TL;DR

| Aspect | Before (v35.4.1) | After (v35.5.0) |
|---|---|---|
| Default mode `is_copy_type(Type::Str)` | true (Copy) | **false (Move)** |
| Default mode `is_copy_type(Type::Array(_))` | true (Copy) | **false (Move)** |
| Default mode `is_copy_type(Type::Struct/Enum/Tuple/Tensor/Quantized)` | true (Copy) | **false (Move)** |
| `--strict-ownership` CLI flag | gates Move semantics | accepted but no-op (default already strict) |
| Compass §4.4 "@safe default" | partial (D-LITE opt-in) | **fully satisfied** |
| `_FjArr` realloc-leak class | closed by Phase 1 arena | still closed |

**Empirical: all engineering gates GREEN.**

| Gate | Result @ v35.5.0 |
|---|---|
| `cargo test --lib` | **7,633 PASS** / 0 fail |
| `cargo test --release --tests` | **80 suites PASS** (~1900+ integration tests) / 0 fail |
| `cargo test --release --test selfhost_stage1_full` | **86/86 PASS** @ ~13.8s |
| `cargo test --release --test selfhost_phase17_self_compile` | **4/4 PASS** @ ~106s (Stage 2 byte-equality preserved) |
| `cargo clippy --lib -- -D warnings` | clean |
| `cargo fmt -- --check` | clean |

## §1 — Cumulative work shipped

### Phase 1 — B0 audit (docs/D_FULL_OPTION_B_PHASE_PLAN.md)
- 12 stdlib consume sites identified across `parser_ast.fj` + `codegen_driver.fj`
- 8 unique variable patterns (`vars`, `ast`, `op`, `arr_name`, `method`, `raw_name`, `declared_type`, `r`)
- Pre-Phase-2 lib test count: 5 documented; 4 break under D-FULL flip
- E2 design decision: **E2.C (skip E2 / methods don't consume receivers)** — internally
  consistent, no codegen changes, matches existing arena-based runtime semantics

### Phase 2 — E2 method-receiver consume
- **Skipped per E2.C decision** (chose internal consistency + cheapest path)

### Phase 3 — Cascade `.clone()` insertions in stdlib

**stdlib/parser_ast.fj:**
- `parse_expr_prec` line 731: `op_prec(opi[0])` (re-read instead of consuming `op`)

**stdlib/codegen_driver.fj** (~30 insertions across):
- BEGIN_INDEX IDENT path: `let mut subj = ast[pos + 1]` (re-fetch, don't consume `arr_name`)
- BEGIN_METHOD_CALL: postpone `map_method(method)` to last use; pre-init `helper = ""`
- BEGIN_CALL: `let mut name = ast[pos + 1]` (re-fetch, don't consume `raw_name`)
- BEGIN_LET (×2 sites: `inline_let_emit` + `emit_stmt`): `let mut fj_type` initialized
  via independent re-fetch from ast (don't consume `declared_type`); pre-snapshot
  `dt_empty = declared_type == ""` for late comparison; `r2 = concat!(r, "")` /
  `r3 = concat!(r, "")` to preserve `r` across multi-branch use
- All `parse_expr_emit / parse_atom / atom_is_str / lookup_var_type_in_table /
  inline_let_emit / find_method_name / skip_one_node / stmt_end / emit_stmt /
  emit_fn_forward_decl / emit_fn / struct_end / fn_end / enum_end / const_end /
  emit_const / emit_struct / emit_enum / emit_if_implicit_return` calls now pass
  `ast.clone()` and/or `vars.clone()` (replace_all-applied)
- `emit_program` BEGIN_STRUCT path: re-fetch `struct_name` from `ast[sp + 1]`
  in the field loop (preserves `struct_name` across iterations)

**stdlib/analyzer.fj:**
- All 6 `extract_ident(source, starts, ends, ...)` calls now pass `.clone()` on
  each str/[T] arg

### Phase 4 — Pre-Phase-2 lib test updates

**src/analyzer/type_check/mod.rs:**
1. `move_type_use_after_move_detected` (line 3391) — flipped from `is_ok()` to
   assert SE024 fires
2. `fn_call_moves_move_type_arg` (line 3419) — flipped to assert SE024
3. `match_enum_destructure_moves_subject` (line 3578) — flipped to assert SE024
4. `move_while_immutably_borrowed_me003` (line 3712) — flipped to assert ME003
5. `strict_default_mode_no_move_errors` (line 5090) — flipped to assert ME001
   (default mode IS strict now)

**src/analyzer/borrow_lite.rs:**
6. `all_types_are_copy_in_interpreter_mode` → renamed `full_strict_default_non_copy_types`
   with inverted assertions (Move types correctly identified)
7. `copy_types_are_correct` — str/struct/array assertions flipped to `!is_copy_type(...)`

**Integration tests updated:**
- `tests/analyzer_branch_merge_terminator.rs::default_mode_pre_phase2_arrays_still_copy`
  → renamed `default_mode_d_full_arrays_are_affine` with inverted expectation
- `tests/safety_tests.rs::safety_move_string_use_after_move` /
  `safety_move_array_use_after_move` — flipped to expect ME001/SE024
- `tests/safety_tests.rs::strict_me001_array_use_after_move` — expects SE024 (not
  ME001) since `[T]` use-after-move dispatches to SE024 per check_ident split
- `tests/eval_tests.rs::s44_string_copy_semantics_no_move_error` — `.clone()` added
- `tests/eval_tests.rs::h2_combined_redirect` — `.clone()` added
- `tests/eval_tests.rs::w7_1_interpreter_array_push` — chain-grow re-assignment
- `tests/eval_tests.rs::v15_b1_7_nested_handle_multi_step` — `.clone()` added
- `tests/nova_v2_tests.rs::v14_n14_1_kernel_complex_struct` — `.clone()` added
- `tests/fajarquant_v2_device.rs::device_fn_full_v2_pipeline` — `.clone()` added
- `tests/stack_kv_cache.rs::overflow_detection` — `.clone()` added
- `tests/tensor_axis_ops.rs::v3_profiler_pipeline` — `.clone()` added
- `tests/validation_tests.rs::v14_w3_5_backward_pass` — `.clone()` added
- `tests/stdlib_v3_crypto_signing_integration.rs` — 8 crypto round-trip tests
  updated with `.clone()` on `key`/`iv`/`nonce`/`msg`/`sig`/`tag` reuse
- `tests/selfhost_analyzer_dup_detection.rs` — driver template updated to
  `.clone()` `src` and `state`

**Q6A example files updated** (19 files): each fn-arg consume followed by later
use of the same str now has `.clone()` at the consume site. Bulk-applied via
`/tmp/auto_clone_fix3.py` for tractable patterns; manual fixes for f-string and
loop-body cases. Touched files:
`q6a_uart_echo`, `q6a_uart_bridge`, `q6a_uart_gps`, `q6a_data_pipeline`,
`q6a_http_infer`, `q6a_mqtt_sensor`, `q6a_ota_update`, `q6a_power_monitor`,
`q6a_rest_api`, `q6a_tls_server`, `q6a_video_detect`, `q6a_batch_scheduler`,
`q6a_fleet_manager`, `q6a_hw_info`, `q6a_model_ab_test`, `q6a_model_hotreload`,
`q6a_plant_monitor`, `q6a_sensor_logger`, `q6a_thermal_monitor`,
`self_lexer_test`, `selfhost_lexer_test`.

### Phase 5 — Default flip + runtime O(1) clone

**src/analyzer/borrow_lite.rs:**
- `is_copy_type` permanently delegates to `is_copy_type_strict`. The `[PHASE_3_PROBE]`
  comment is removed. Comment block updated to document the new contract.

**src/interpreter/eval/methods.rs — universal O(1) `.clone()` recognition:**
- `(Value::Array(a), "clone")` — Arc-share clone (refcount bump only). All array
  mutations (`push`/`pop`/`insert`/`remove`) already use `Arc::make_mut` for COW,
  so sharing the inner Arc is safe and avoids O(n²) cascade in chain bootstrap.
- `(Value::Str(s), "clone")` — Rc-share clone (refcount bump).
- `(Value::Tensor(t), "clone")` — Arc-share clone.
- Catch-all `(v @ Struct/Enum/Tuple/Map/Quantized, "clone")` — Rust-side
  `Value::clone` (Rc/Arc-based; effectively O(1) for refcount-backed inner state).

**stdlib/codegen.fj — refcount + COW for `_FjArr` C runtime:**
- `_FjArr` struct gains `int rc` field.
- `_fj_arr_new` and grow's COW-allocated copies initialize `rc = 1`.
- `_fj_arr_clone` is now refcount-bump only: `a->rc++; return a;` — O(1).
- `_fj_arr_grow` checks `rc > 1` first; if shared, deep-copies before mutating
  (COW) so the mutation doesn't leak to other holders. Otherwise grow in place.

This C-runtime refactor is **necessary**: without COW, `.clone()` cascades in
the chain bootstrap caused OOM (process killed at >26 GB resident memory).
With COW, phase17 self-compile completes in ~106s (vs ~13.5min without COW
optimization, baseline ~54s pre-Phase-2-D-FULL).

## §2 — Engineering metrics

| Metric | Value |
|---|---|
| Files changed | ~35 (stdlib + tests + examples + 1 src wire-flip) |
| LOC delta | +~150 (new content) -~50 (test contract flips) net ~+100 |
| Lib tests | 7,629 → **7,633** (+4: borrow_lite + Quantized clone) |
| Integration suites | 79 → 80 (selfhost_phase17 still 4/4) |
| Stage 2 byte-equality | preserved end-to-end through D-FULL ship |
| Phase 17 self-compile time | ~54s baseline → ~106s (2× slowdown from clone overhead) |
| Pre-Phase-2 ME001 contract tests | 5 → migrated to SE024/ME001/ME003 expectation |
| GitHub Release | v35.5.0 (TBD) |

## §3 — Why E2.C (skip E2)

The over-fire findings doc raised E2 (method-receiver consume tracking) as a
potential gap: `arr.method(...)` doesn't currently mark `arr` moved. E2 would
require defining the method's `self`-kind (consume, `&self`, `&mut self`) per
method.

**E2.C decision rationale:**
1. The existing C-runtime allocator returns fresh arrays from `push` / `pop` /
   etc. (functional-style). Receivers are effectively `&self` borrows.
2. With COW (added in Phase 5 above), shared receivers mutated through any path
   safely deep-copy on first write.
3. Adding E2.A (consume) or E2.B (`&mut self`) would require changing fn
   signatures + codegen + chain dispatch — substantial work for unclear gain.
4. The free-fn fn-arg consume path catches actual ownership transfer in
   `func(arg)` form. Method-call args also go through `check_method_call` which
   currently checks via `check_expr` (read), not consume — same as receiver.
   Internally consistent.

If a user wants stronger semantics later (e.g., `into_iter` style consumers),
individual methods can opt in via an `@consume fn` attribute. **Out of scope
for v35.5.0.**

## §4 — Closure ship sequence

```bash
cd "/home/primecore/Documents/Fajar Lang"
# Verify all gates green
cargo test --lib && cargo test --release --tests \
  && cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1 \
  && cargo clippy --lib -- -D warnings && cargo fmt -- --check
# Commit + tag + push + GitHub Release
git add -A && git commit -m "feat(fjarr-leak): v35.5.0 Phase 2 D-FULL — full-strict default-on"
git tag -a v35.5.0 -m "FJARR_LEAK Phase 2 D-FULL closure"
git push origin main && git push origin v35.5.0
gh release create v35.5.0 --notes "..."
```

## §5 — Self-check (per CLAUDE.md §6.8)

```
[x] Pre-flight audit (B0/C0/D0) exists for this phase?           (R1: D_FULL_OPTION_B_PHASE_PLAN)
[x] Every action has a runnable verification command?             (R2: §0 + §4)
[x] Prevention mechanism added?                                   (R3: pre-push hook gates phase17 + stage1_full)
[x] Agent-produced numbers cross-checked with Bash?               (R4: all numbers in §0)
[x] Effort variance tagged?                                       (R5: ~6-12h estimate vs actual ~6h)
[x] Decisions are committed files?                                (R6: this doc + plan doc)
[x] No public-artifact drift?                                     (R7: CLAUDE.md + CHANGELOG synced this commit)
[x] Multi-repo state check?                                       (R8: only fajar-lang touched)
```

---

*FJARR_LEAK Phase 2 D-FULL — written 2026-05-10. Closes the Compass §4.4
default-on safety initiative for FJARR_LEAK Phase 2. ~6 hours actual end-to-end
across audit + cascade fixes + lib/integration test contract updates + COW
runtime refactor + closure docs. All engineering gates GREEN; 7,633 lib + 86
stage1_full + 4/4 phase17 + 80 integration suites all pass. Stage 2
byte-equality preserved through D-FULL ship.*
