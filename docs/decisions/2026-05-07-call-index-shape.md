---
decision: f()[i] / obj.m()[i] indexing — D1 + D2 + D3 strategy
date: 2026-05-07
status: ACCEPTED 2026-05-07 (batch-approved with companion D-files)
prereq: docs/CALL_INDEX_PLAN.md, docs/CALL_INDEX_B0_FINDINGS.md
plan: docs/CALL_INDEX_PLAN.md §1
---

# CALL_INDEX Decision — D1 + D2 + D3 strategy

This decision covers three sub-decisions because they are interlocked:
the AST shape (D1) determines how codegen dispatches (D2), which
determines how method ret-types are looked up (D3).

## Choice

**D1: A** (reuse BEGIN_INDEX, generalize subject)
**D2: A** (peek subject AST first child) — for immediate fix; D2.B (typed parse_expr_emit) deferred to Phase 19+
**D3: B** (parallel `map_method_ret_type` fn) — wired now, even though no [str]-returning method exists today (forward-investment)

> **Status: ACCEPTED 2026-05-07.** Batch-approved with the two companion
> decision files in this directory.

## Rationale (≥3 sentences)

**D1.A** is the cheaper path consistent with Phase 16.5's earlier
generalization of `BEGIN_METHOD_CALL` subject-can-be-any-expression.
Adding a sibling `BEGIN_INDEX_EXPR` (D1.B) would double the AST surface
area without semantic gain — the codegen dispatch case-split on
`ast[pos+1]` tag is the same complexity either way. Plan §1.D1
identified `skip_one_node` already understands BEGIN_INDEX; reusing it
costs less than teaching it about a new shape pair.

**D2.A** (walk subject's first child to look up ret-type) is targeted
and low-risk for the immediate fix. The deeper refactor `D2.B`
(`parse_expr_emit_with_type` returning `(code, fj_type)` tuple) would
unlock several future features (`len(f())`, `to_int(g().h())`,
`[arr_returning_call(), other][0]`) but touches all 12 branches of
`parse_expr_emit` — too much surface area to perturb in one commit.
Plan §1.D2 explicitly puts D2.B on the Phase 19+ roadmap; D2.A is the
incremental wedge.

**D3.B** ships method ret-type infrastructure now even though zero
methods return `[str]` today (per B0.3 inventory). This is
forward-investment: when a `[str]`-returning method lands (likely
`.split()` per kompas niche needs), `obj.m()[i]` already works. D3.B
adds 5 LOC; D3.A's tuple-of-array is awkward, D3.C's CodegenState
field is over-engineered for 4 hardcoded methods.

The B0 headline finding (`docs/CALL_INDEX_B0_FINDINGS.md` §B0.2)
revised the failure mode from "parser error" to **silent miscompile**
— the chain accepts `let v = make_arr()[1]` and emits broken C that
gcc warns on but compiles. Severity recalibrated UP. This decision
fixes the most user-visible silent-bug class in the self-host chain.

## @kernel-future-compat

**Compatible: yes** (no impact)

These decisions affect parser AST shape and codegen dispatch logic.
Neither changes the type system or context-isolation rules. The fix
adds zero new heap allocations in `@kernel` paths — the C dispatch
goes from `_fj_arr_get_i64(BEGIN_CALL, ...)` (broken) to
`_fj_arr_get_str(make_arr(), ...)` (correct), all within existing
runtime helpers.

## Migration path

1. **Pre-flight committed:** `docs/CALL_INDEX_B0_FINDINGS.md` (already
   committed in commit `f5448b03`).

2. **P1 Parser surgery (`stdlib/parser_ast.fj`):**
   - **P1.1**: extend BEGIN_CALL branch (L431-457) to detect `[` after
     closing `)` and wrap in BEGIN_INDEX.
   - **P1.2**: extend BEGIN_METHOD_CALL chain branch (L529-614)
     similarly.
   - **P1.3** (deferred): nested `f()[0][1]` — single-level only
     this phase; document as scope-boundary.

3. **P2 Codegen subject generalization (`stdlib/codegen_driver.fj`):**
   - **P2.1**: replace `let arr_name = ast[pos+1]` (L139-154) with
     case-split on `ast[pos+1]` tag — IDENT/BEGIN_CALL/BEGIN_METHOD_CALL.
   - **P2.2 / P2.3**: guard the IDENT-name lookups at L197-201 + L361-365
     behind `subject IS IDENT` check.
   - **P2.4**: cursor-arithmetic audit (`r.pos + 1` past END_INDEX)
     when subject is multi-token.

4. **P3 Method ret-type registry (D3.B):**
   - **P3.1**: add `fn map_method_ret_type(method: str) -> str` next to
     `map_method` in codegen_driver.fj. Returns `"str"`/`"i64"`/`""`.
   - **P3.2**: BEGIN_INDEX dispatch consults it when subject is
     BEGIN_METHOD_CALL.

5. **P4 Regression**: 5 NEW P-tests (P81-P85):
   - P81: `make_arr()[i]` returning `[i64]`
   - P82: `make_arr()[i]` returning `[str]`
   - P83: `f()[g()]` (nested calls)
   - P84: synthetic `[str]`-returning fn (scaffold for future method)
   - P85: `f()[i] + f()[j]` (multiple invocations + Pratt interaction)

6. **P5 Stage 2 byte-equality re-baseline**: run
   `phase17_stage2_native_triple_test`. **Expected:** md5 changes
   from `1d6c52a...` → NEW (the chain emits different C now, that's
   correct). Capture new md5 and update test fixture in same commit
   with `WHY:` comment per plan §4 R1 mitigation.

7. **Closure**: append v35.1.0 entry to CHANGELOG, update CLAUDE.md
   §17.10 flipping `arr[i] for [str]-typed arr` from ❌ to ✅.

All 95 existing self-host tests must remain green throughout (after
md5 re-baseline at P5).

## Surprise budget

**+30%** (high-uncertainty per CLAUDE.md §6.8 R5).

Bumped from default +25% because:
- Pratt parser corner cases (`f()[i] + 1`, `f()[i].method()`) historically surface late.
- Cursor-arithmetic off-by-one is the prototypical bug for this family.
- Stage 2 byte-equality re-baseline carries unknown amount of expected diff to validate.

Plan likely 6.75h × 1.30 = **8.75h ceiling**. Variance tag:
`feat(selfhost-call-index): close f()[i] silent miscompile [actual Xh,
est 6.75h, +Y%]`.

## Rejected candidates

### D1
- **D1.B (new BEGIN_INDEX_EXPR distinct from BEGIN_INDEX)**: doubles
  AST surface area for no semantic gain. Future "everything is
  indexable" refactor would force collapsing them anyway. Rejected.

### D2
- **D2.B (`parse_expr_emit_with_type` typed-result refactor)**:
  structurally cleaner long-term but touches 12 emit branches in one
  commit. Risk of regressing 80 P-tests + 17 phase17 self-compile +
  Stage 2 byte-equality is too high. Deferred to Phase 19+ roadmap;
  D2.A unblocks the immediate user-visible fix.

### D3
- **D3.A (tuple-as-array `map_method` returns `[str]` length 2)**:
  awkward syntax, every L178 caller updates. Rejected for ergonomics.
- **D3.C (per-instance `cg.method_ret_types: [str]` field)**: most
  invasive. Every of the 15+ state-clone sites in codegen.fj gets one
  more field. Justified only if methods become user-defined; today
  they're hardcoded 4 entries. Rejected as over-engineering.

## Reverse-cost

**Medium.** D1.A is the most reversible — if D1.A turns out to break
something subtle, D1.B can be retrofitted as a separate AST shape and
the dispatch can support both during a deprecation window.

D2.A is reversible: D2.B's typed-result refactor can supersede it
later without breaking D2.A's call path (D2.A becomes a special-case
shortcut inside the typed walker).

D3.B is the most reversible — `map_method_ret_type` is a pure function
swap-able with D3.A or D3.C if requirements change.

Stage 2 md5 baseline change is **NOT** reversible without effort
(once new md5 is committed, future changes that produce the OLD md5
would now fail). This is acceptable because the new md5 represents
correct semantics; the old md5 represented silent miscompile.

---

*ACCEPTED 2026-05-07. Implementation proceeds per migration path §3
above. Variance tracking per §6.8 R5: tag commit with `[actual Xh,
est 6.75h, +Y%]`.*
