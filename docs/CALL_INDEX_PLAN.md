# Plan: f()[i] / obj.m()[i] indexing

**Source: Plan agent dispatch 2026-05-07.** Read-only agent; saved here for review.

## Executive Summary (200 words)

Fajar Lang's self-hosted compiler at v35.0.0 cannot parse `f()[i]` or `obj.m()[i]` because both branches in `stdlib/parser_ast.fj` (`BEGIN_CALL` at L431-457, `BEGIN_METHOD_CALL` at L529-614) `pr_ok` immediately on the closing `)` without consulting whether `[` follows. Codegen at `stdlib/codegen_driver.fj` L139-154 keys `BEGIN_INDEX` dispatch on a flat IDENT name (`ast[pos+1]`), so even if the parser produced `BEGIN_INDEX`, the type-lookup path can't follow a call-result subject.

This plan splits the work into B0 audit, P1 parser surgery, P2 codegen subject generalization, P3 method-ret-type registry, and P4 prevention layer. The biggest decision (D1 — AST shape) is between reusing `BEGIN_INDEX <complex-subject> <index> END_INDEX` vs. introducing `BEGIN_INDEX_EXPR`. Reuse is cheaper and consistent with how `BEGIN_METHOD_CALL` already handles arbitrary subjects (since Phase 16.5), but requires the codegen dispatch to peek the first child's tag to decide between the existing IDENT/FIELD-chain fast path and the new general path.

---

## §0 — Pre-Flight Audit (B0, mandatory per CLAUDE.md §6.8 R1)

**Goal:** prove the gap empirically before touching code. Output `docs/SELFHOST_FJ_PHASE_18_B0_FINDINGS.md`.

### B0.1 — Build + sanity baseline

```bash
cd "/home/primecore/Documents/Fajar Lang"
cargo build --release --bin fj
./target/release/fj --version
md5sum target/release/fj
```

### B0.2 — Confirm parser failure on `f()[i]` (`[i64]` ret)

```bash
cat > /tmp/b0_call_index_i64.fj <<'EOF'
fn make_arr() -> [i64] { let mut a: [i64] = []; a = a.push(10); a = a.push(20); a = a.push(30); return a }
fn main() -> i64 { return make_arr()[1] }
EOF
./target/release/fj run /tmp/b0_call_index_i64.fj
```

Predicted: parser error `ERR_PRIMARY` or "expected operator" at `[`.

### B0.3 — Confirm method-chain failure

```bash
grep -n 'fn map_method\|method == "' "/home/primecore/Documents/Fajar Lang/stdlib/codegen_driver.fj" | head -20
```

Current methods: `substring` → `str`, `push` → `_FjArr*`, `len` → `i64`, `join` → `str`. **None return `[str]`.** D3 method-chain story affects FUTURE registrations only.

### B0.4 — Trace codegen dispatch on synthesized AST

If parser were patched, what would codegen produce? Walk codegen_driver.fj L144 by hand:
- `arr_name = ast[pos+1]` would be literal `"BEGIN_CALL"`
- `lookup_var_type_in_table` returns `""`
- Defaults to `_fj_arr_get_i64`
- emitted C: `_fj_arr_get_i64(BEGIN_CALL, ...)` — compile error. Confirms codegen MUST be touched.

---

## §1 — Design space + decisions

### D1 — AST shape

| Option | AST shape | Pros | Cons |
|---|---|---|---|
| **D1.A** Reuse BEGIN_INDEX, generalize subject | `BEGIN_INDEX <subject-AST> <index-AST> END_INDEX`. Subject can be IDENT, BEGIN_CALL block, or BEGIN_METHOD_CALL block. | Same surface; consistent with Phase 16.5 generalization of BEGIN_METHOD_CALL. No new dispatch token. `skip_one_node` already knows BEGIN_INDEX. | Codegen dispatch becomes case-split on `ast[pos+1]` tag. Existing fast path "next slot is IDENT name" must change. Sites at L197-201, L361-365 also update. |
| **D1.B** New BEGIN_INDEX_EXPR | Existing IDENT/FIELD shape stays. Calls/methods emit `BEGIN_INDEX_EXPR <subject> <index> END_INDEX_EXPR`. | Discriminator-by-tag — no first-child peek. Old dispatch paths untouched. | Doubles AST surface. `skip_one_node` learns new pair. Future "everything is indexable expr" forces collapse anyway. |

**Recommendation:** D1.A. Aligns with Phase 16.5 trajectory. Same pattern as BEGIN_METHOD_CALL (codegen_driver L159).

**Decision gate:** `docs/decisions/2026-MM-DD_d1_index_ast_shape.md`. Pre-commit hook blocks P1+ commits without `Decision: D1.A` footer.

### D2 — Codegen ret-type lookup for complex subjects

| Option | Mechanism | Pros | Cons |
|---|---|---|---|
| **D2.A** Walk subject AST, peek first child | When subject is BEGIN_CALL: `let callee = ast[pos+2]; let ret_type = lookup_fn_ret_type(cg, callee)`. When BEGIN_METHOD_CALL: `let method = find_method_name(...); let ret_type = method_ret_type(method)` (D3). | Minimal change. Reuses existing helpers. | Only handles named shapes. Future shapes (BEGIN_IF_EXPR selecting between arrays) need more branches. |
| **D2.B** `parse_expr_emit_with_type` returning `(code, fj_type)` | Replace `ExprResult { code, pos }` with `TypedExprResult { code, pos, fj_type }`. Each branch records result type. | Universal, future-proof. Unlocks `len(f())`, `to_int(g().h())`, etc. | Touches ~12 branches in `parse_expr_emit`. Risk regressing 80 P-tests + 17 phase17 + Stage 2 byte-equality. |

**Recommendation:** D2.A immediate; mark D2.B as "right" structural answer for Phase 19+ roadmap.

### D3 — Method return-type registry

| Option | Where | Pros | Cons |
|---|---|---|---|
| **D3.A** Tuple-as-array `map_method(method) -> [str]` | Single source. | No new state. | Tuple-as-array awkward. Every L178 caller updates. |
| **D3.B** Parallel `map_method_ret_type(method) -> str` | Pure fn. | Localized. Easy. | Two functions to keep in sync. |
| **D3.C** Per-instance `cg.method_ret_types: [str]` | New CodegenState field. | Consistent with `fn_ret_types`, `struct_fields`. Allows runtime registration. | Most invasive — every state-clone (15+) gets one more field. |

**Constraint from B0.D0:** today, NO method maps to `[str]` ret-type. So D3 is wiring; first user-visible string-array case won't appear until a `[str]`-returning method exists. Immediate user-visible win is `f()[i]` (the call form).

**Recommendation:** D3.B.

### D4 — Test coverage scope (P81+)

5 new tests in `tests/selfhost_stage1_full.rs`:

| ID | Shape | Subject ret-type | Index | Why |
|---|---|---|---|---|
| **P81** | `make_arr()[i]` | `[i64]` | INT literal | Baseline |
| **P82** | `make_arr()[i]` | `[str]` | INT literal | Verifies `_fj_arr_get_str` dispatch on call ret-type |
| **P83** | `make_arr()[g()]` | `[i64]` | another call | Nested calls |
| **P84** | `obj.m()[i]` | (no `[str]` method exists yet) | — | SKIP cfg or wait for D3 |
| **P85** | `f()[i] + f()[j]` | `[i64]` | INT | Multiple invocations |

---

## §2 — Phased tasks

### Phase P1 — Parser surgery (`stdlib/parser_ast.fj`)

| Step | File:lines | Change | Verification | Surprise |
|---|---|---|---|---|
| **P1.1** | L431-457 (BEGIN_CALL) | After `pr_ok(a, cur+1)` candidate (L453), peek `skip_ws(src, cur+1)`. If next is `[`, emit BEGIN_INDEX wrapper around just-built call AST, parse index, expect `]`, push END_INDEX. | `cargo build --release && ./target/release/fj dump-ast /tmp/b0_call_index_i64.fj` shows `BEGIN_INDEX BEGIN_CALL make_arr END_CALL INT 1 END_INDEX` | +30% |
| **P1.2** | L529-614 (BEGIN_METHOD_CALL chain) | After chain final `)` (L611's END_METHOD_CALL push), check `[`. If present, wrap method-call AST in BEGIN_INDEX. | `dump-ast` verify wrapping. | +50% (subtle: `a.b().c()[i].d()` — defer follow-on chain) |
| **P1.3** | L431-457 — recursive case `f()[0][1]` | Decision: defer. Document scope-boundary. | `[ -z "$(./target/release/fj dump-ast /tmp/double_index.fj 2>&1)" ]` graceful error | n/a |

**DG-P1.2:** does `f()[i].method()` parse? Recommend: defer to keep P1 small.

### Phase P2 — Codegen subject generalization (`stdlib/codegen_driver.fj`)

| Step | File:lines | Change | Verification | Surprise |
|---|---|---|---|---|
| **P2.1** | L139-154 (BEGIN_INDEX dispatch) | Replace `let arr_name = ast[pos+1]` with case-split on `ast[pos+1]` tag: IDENT → existing path; BEGIN_CALL → recurse + lookup_fn_ret_type(callee); BEGIN_METHOD_CALL → recurse + D3 lookup. | `./target/release/fj run /tmp/b0_call_index_i64.fj` exits 20 | +30% |
| **P2.2** | L197-201 (push-arg dispatch reading ast[first_arg_pos+1] as IDENT) | Gate IDENT-name lookup behind `subject IS IDENT` check. | grep verifies guard. P81 PASS. | +20% |
| **P2.3** | L361-365 (`to_int` BEGIN_INDEX dispatch) | Same surgery as P2.2. | P82 PASS. | +20% |
| **P2.4** | L139-154 cursor audit | Verify `r.pos + 1` after subj-recurse + index-parse still correct. | P83 (`f()[g()]`) PASS. | +30% (off-by-one prototypical bug) |

### Phase P3 — Method ret-type registry (D3.B)

| Step | File:lines | Change | Verification | Surprise |
|---|---|---|---|---|
| **P3.1** | codegen_driver.fj add `fn map_method_ret_type(method: str) -> str` next to `map_method` (L493). Returns `"str"` for substring/concat/join; `"i64"` for len; `""` for push. | New fn. | `grep -n "fn map_method_ret_type" stdlib/codegen_driver.fj` 1 hit. All 80+17 PASS. | +20% |
| **P3.2** | L139 (BEGIN_INDEX) — when subject BEGIN_METHOD_CALL, call map_method_ret_type to drive get_str/get_i64. | Wire-up. | `obj.m()[i]` test (synthetic). | +30% |

### Phase P4 — Self-compile + native binary regression

| Step | Change | Verification | Surprise |
|---|---|---|---|
| **P4.1** | Re-run phase17 self-compile. Compiler source must still compile byte-identical. | `cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1 phase17_stage2_native_triple_test`. PASS, md5 unchanged from `1d6c52a...` (or NEW md5 captured + committed). | +50% |
| **P4.2** | Re-run all 80 stage1_full tests. | 80/80 PASS. | +20% |
| **P4.3** | Smoke existing arr[i] IDENT cases (P47, P48). | Same `cargo test`. | +10% |

---

## §3 — Prevention layer

| Mechanism | Where | What it prevents |
|---|---|---|
| **P81-P85 regression tests** in `tests/selfhost_stage1_full.rs` | New `#[test] fn full_p81_...` through `full_p85_...`. | Future regression of `f()[i]` / `obj.m()[i]` parsing or dispatch. |
| **Decision-doc pre-commit hook** `scripts/check_decision_doc.sh` | Reads commit footer for `Decision: D1.A` / `D2.A` / `D3.B`; verifies docs exist. | D1/D2/D3 prose drift from impl (§6.8 R6). |
| **Phase-18 findings doc** `docs/SELFHOST_FJ_PHASE_18_FINDINGS.md` | Mirrors phase 17 structure. | Audit-trail (§6.6 R5). |
| **CI test target** | Add `cargo test --release --test selfhost_stage1_full` to pre-push hook. Ensure phase17 in `cargo test --tests` matrix. | Stage 2 byte-equality drift. |
| **CLAUDE.md note** | Flip §17.10 "❌ arr[i] for [str]-typed arr in user-extended codegen" → "✅ closed Phase 18". | Doc integrity (§6.6 R3). |

---

## §4 — Risk register

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **R1** Stage 2 byte-equality breaks | HIGH | severe (rolls back v35.0.0 fixed-point) | Run that single test after every codegen.fj/parser_ast.fj change. Capture old + new md5. |
| **R2** Existing P47/P48/P54-P57 regress | MEDIUM | moderate | Conservative parser peek — only fire when both `)` consumed AND `[` follows post `skip_ws`. |
| **R3** codegen self-compile breaks | MEDIUM | severe | Run `selfhost_phase17_self_compile` after every P1.*/P2.*/P3.* commit. |
| **R4** `f()[i] + 1` parses wrong (Pratt) | MEDIUM | moderate | After P1.1, BEGIN_INDEX-wrapped result is itself an atom. Verify P85 (`return make_arr()[0] + make_arr()[1]`). |
| **R5** Off-by-one in `r.pos + 1` (codegen_driver L154) | HIGH | moderate | Audit explicitly per P2.4. Manifests as "undefined symbol END_INDEX_…". |
| **R6** AST size grows; Phase 17.5 O(n) regress | LOW | minor | +2 tokens per BEGIN_INDEX wrap. Negligible at 3206 LOC scale. |
| **R7** D3.B method-ret-type table goes stale | MEDIUM (post-ship) | minor | Unit test: walk both maps, assert every `map_method` entry has `map_method_ret_type` entry. |
| **R8** User writes `f()` returning `[i32]` not `[i64]` | LOW | minor | Map_type_ctx already collapses both to `_FjArr*`; same C helper. |

---

## §5 — Total budget estimate

| Phase | Optimistic | Likely | Pessimistic |
|---|---|---|---|
| B0 audit + findings doc | 0.5h | 0.75h | 1h |
| D1/D2/D3/D4 decisions + commit | 0.25h | 0.5h | 1h |
| P1.1 — call branch | 0.5h | 1h | 1.5h |
| P1.2 — method branch | 0.5h | 1h | 2h |
| P2.1 — BEGIN_INDEX dispatch case-split | 0.5h | 1h | 1.5h |
| P2.2 / P2.3 — guard sites | 0.25h | 0.5h | 1h |
| P2.4 — cursor audit | 0.25h | 0.5h | 1h |
| P3.1/P3.2 — method ret-type registry | 0.25h | 0.5h | 1h |
| P4 — full regression sweep | 0.5h | 0.75h | 1.5h |
| P81-P85 test authoring | 0.5h | 0.75h | 1h |
| Phase 18 findings doc + CLAUDE.md update | 0.25h | 0.5h | 1h |
| **TOTAL** | **3.75h** | **6.75h** | **13.5h** |

User's earlier ~2-4h claim was optimistic. Per §6.8 R5 (+25% min, +30% high-uncertainty), budget = **6.75h × 1.30 = 8.75h ceiling**.

Key drivers of upper-bound risk: R1 (Stage 2 byte-equality) and R5 (cursor off-by-one).

---

### Critical files

- `/home/primecore/Documents/Fajar Lang/stdlib/parser_ast.fj`
- `/home/primecore/Documents/Fajar Lang/stdlib/codegen_driver.fj`
- `/home/primecore/Documents/Fajar Lang/stdlib/codegen.fj`
- `/home/primecore/Documents/Fajar Lang/tests/selfhost_stage1_full.rs`
- `/home/primecore/Documents/Fajar Lang/tests/selfhost_phase17_self_compile.rs`
