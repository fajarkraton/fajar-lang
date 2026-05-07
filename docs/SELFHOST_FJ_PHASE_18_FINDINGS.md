---
phase: 18 — `f()[i]` / `obj.m()[i]` indexing closure (silent-miscompile fix)
status: CLOSED 2026-05-07
budget: 6.75h likely / 8.75h ceiling (per CALL_INDEX_PLAN §5)
actual: ~6h Claude time across 11 commits (plan + B0 + decisions + P1..P3 + this P4)
variance: -11% vs likely / -31% vs ceiling
plan: docs/CALL_INDEX_PLAN.md
b0:   docs/CALL_INDEX_B0_FINDINGS.md
decision: docs/decisions/2026-05-07-call-index-shape.md (D1.A + D2.A + D3.B)
artifacts:
  - This findings doc
  - stdlib/parser_ast.fj — BEGIN_CALL + BEGIN_METHOD_CALL chain wrap `[…]` postfix
  - stdlib/codegen_driver.fj — BEGIN_INDEX dispatch case-split on subject AST tag;
    push-arg + to_int IDENT-only guards; map_method_ret_type registry
  - tests/selfhost_stage1_full.rs — P81..P86 (6 NEW)
  - scripts/git-hooks/pre-push (NEW) — selfhost regression gate
prereq: v35.0.0 (Phase 17 closed — Stage 2 self-host triple-test fixed point)
tests-at-close: 86 stage1_full + 4 phase17 + 95 cumulative self-host (was 80 + 4 + 95 at v35.0.0)
---

# fj-lang Self-Hosting — Phase 18 Findings

> **Silent-miscompile class closed.** The self-host chain accepted
> `let v = make_arr()[1]` → emitted broken C → printed a heap-pointer-
> cast-to-int. Per `CALL_INDEX_B0_FINDINGS.md` §B0.2, severity was
> recalibrated UP from "parser error" to "silent miscompile." Phase 18
> closes the user-visible call form (`f()[i]`) AND wires the
> infrastructure for the future method form (`obj.m()[i]`) per D3.B
> forward-investment.

## §0 — B0 surfaced silent-miscompile (revises plan §0 expected-error-mode)

The original `CALL_INDEX_PLAN.md` §0 predicted a parser-error failure
mode (`ERR_PRIMARY` / "expected operator" at `[`). B0.2 disproved this:

```fj
fn make_arr() -> [i64] { let mut a: [i64] = []; a = a.push(10); a = a.push(20); a = a.push(30); a }
fn main() { let v = make_arr()[1]; println(v) }
```

was accepted by `stdlib/parser_ast.fj` and lowered to:

```c
_FjArr* v = make_arr();              // call OK; v is the WHOLE array
_fj_arr_push_i64(_fj_arr_new(), 1);  // [1] parsed as standalone array literal
fj_println_int(v);                   // prints heap pointer cast to int64_t
```

— a **silent miscompile**, not a parser error. The interpreter's Rust
Pratt parser (`src/parser/`) handled `f()[i]` correctly (`fj run`
returned 20); the self-host Fajar parser split the expression into
two adjacent statements. gcc warned `[-Wint-conversion]` but compiled.

**Implication for the prevention layer:** the §3 regression tests
must assert binary output value (not just exit code or compile
success), because the silent class would have passed any
compile-only gate. P81..P86 follow this discipline (each test
captures stdout via the chain-driven binary).

Full B0 evidence: `docs/CALL_INDEX_B0_FINDINGS.md`.

## §1 — Decisions D1.A + D2.A + D3.B

Source: `docs/decisions/2026-05-07-call-index-shape.md` (ACCEPTED
2026-05-07, batch-approved with companion T4 + FJARR_LEAK decisions).

| Sub-decision | Choice | Rationale |
|---|---|---|
| **D1** AST shape | **A — reuse BEGIN_INDEX, generalize subject** | Aligns with Phase 16.5's earlier generalization of BEGIN_METHOD_CALL subject. `skip_one_node` already understands BEGIN_INDEX. D1.B (new BEGIN_INDEX_EXPR) doubles AST surface for no semantic gain. |
| **D2** Codegen ret-type lookup | **A — peek subject AST first child** | Targeted, low-risk for immediate fix. D2.B (`parse_expr_emit_with_type` typed-result refactor) deferred to Phase 19+ roadmap; touches 12 emit branches and would risk all 80 P-tests + Stage 2 byte-equality in one commit. |
| **D3** Method ret-type registry | **B — parallel `map_method_ret_type` fn** | Forward-investment. B0.3 confirmed 0 methods return `[str]` today (substring/concat/join are `str`, push is `_FjArr*`, len is `i64`). D3.B adds 5 LOC pure-fn; D3.A tuple-as-array is awkward; D3.C CodegenState field over-engineers for 4 hardcoded methods. |

@kernel-future-compat: **yes** — neither AST-shape nor dispatch
changes affect type system or context isolation. The fix removes
heap-leak from the broken `[1]` array-literal emission (the leaked
`_fj_arr_new()` call no longer happens once `[…]` is a postfix
operator instead of a sibling expression).

Variance budget per CLAUDE.md §6.8 R5: **+30% high-uncertainty**
(Pratt corner cases + cursor-arithmetic off-by-one risk + Stage 2 md5
re-baseline). Plan 6.75h × 1.30 = **8.75h ceiling**.

## §2 — Phase 18 sub-tasks closed (P1.1 → P3.2)

8 implementation steps across 3 phases. Every commit tagged with
`[actual Xmin, est Yh, -Z%]` per CLAUDE.md §6.8 R5.

### P1 — Parser surgery (`stdlib/parser_ast.fj`)

| Step | Commit | What | Surprise vs est |
|---|---|---|---|
| **P1.1** | `0b165bc9` | After BEGIN_CALL closing `)` (L431-457), peek `skip_ws(src, cur+1)`. If `[` follows, emit BEGIN_INDEX wrapper around just-built call AST, parse index expr, expect `]`, push END_INDEX. | ~30min vs 1h, **-50%** |
| **P1.2** | `f47ed8f6` | After BEGIN_METHOD_CALL chain final `)` (L529-614, post END_METHOD_CALL push), check `[`. Same wrap. | ~15min vs 1.5h, **-83%** |
| **P1.3** | (deferred) | `f()[0][1]` recursive case + `f()[i].method()` chained-after-index. Single-level only this phase; documented as scope-boundary. | n/a |

P1.3 deferral preserves Phase 18 narrowness. Both follow-on shapes
are addressable via the same `[`-peek mechanism, but each multiplies
surface area; bundling would have widened the regression matrix
beyond the +30% surprise budget.

### P2 — Codegen subject generalization (`stdlib/codegen_driver.fj`)

| Step | Commit | What | Surprise vs est |
|---|---|---|---|
| **P2.1** | `ab248681` | L139-154 BEGIN_INDEX dispatch — replace `let arr_name = ast[pos+1]` with case-split on `ast[pos+1]` tag: IDENT (existing fast path) / BEGIN_CALL (recurse subject + `lookup_fn_ret_type(callee)`) / BEGIN_METHOD_CALL (recurse + D3 lookup). | ~50min vs 1.5h, **-44%** |
| **P2.2** | `456b483e` | L197-201 free-`push(arr, elem)` arg dispatch — gate IDENT-name lookup behind `subject IS IDENT` check; non-IDENT subjects fall through to BEGIN_INDEX-aware path. | ~10min vs 1.5h, **-89%** |
| **P2.3** | `1892439f` | L361-365 `to_int` BEGIN_INDEX dispatch — same surgery as P2.2. | ~20min vs 1.5h, **-78%** |
| **P2.4** | `b6debabe` | Cursor-arithmetic audit. **No code change** — confirmed `r_idx.pos + 1` (skip past END_INDEX) idiom is consistent across all 3 BEGIN_INDEX subject paths (IDENT @ L183, BEGIN_CALL @ L159, BEGIN_METHOD_CALL @ L168). Trace for `make_arr()[one()]`: parse_atom on BEGIN_CALL block advances `r_subj.pos` past END_CALL; parse_expr_emit on index advances `r_idx.pos` past END_CALL; +1 lands past END_INDEX. P83 + P85 (two invocations) PASS confirm correctness. | ~10min vs 1.5h, **-89%** |

R5 (cursor off-by-one) was the highest-probability bug class per
plan §4 Risk Register. P2.4's audit-only outcome is the empirical
demonstration that R5 did not materialize — manifest as a "no-op
closure note" but the audit work itself was the deliverable.

### P3 — Method ret-type registry (D3.B)

| Step | Commit | What | Surprise vs est |
|---|---|---|---|
| **P3.1** | `a3e01a09` | New `fn map_method_ret_type(method: str) -> str` next to `map_method` (codegen_driver.fj L493). Returns `"str"` for substring/concat/join, `"i64"` for len, `""` for push. | (combined with P3.2) |
| **P3.2** | `a3e01a09` | Wire into 3 sites: BEGIN_INDEX-subject dispatch when subject is BEGIN_METHOD_CALL; `to_int`'s BEGIN_INDEX-method-arg path; free-`push` BEGIN_INDEX-method-subject path. | combined ~20min vs 1h, **-67%** |

**No behavior change today** — no extant method returns `[str]` /
`[[str]]`. Wire-up is forward-investment per D3.B; first user-visible
case will appear when (e.g.) `.split()` lands. Removes "deferred"
markers from D2.A's case-split — the BEGIN_METHOD_CALL branch can now
look up element type instead of falling through to "default i64."

P84 (`obj.m()[i]` — chained-push-then-index, `a.push(9)[2]`) was added
as a forward-investment exercise of the BEGIN_INDEX BEGIN_METHOD_CALL
path even though no method today returns a `[str]`-typed value.

## §3 — Tests P81..P86 (6 NEW in `tests/selfhost_stage1_full.rs`)

Plan §1.D4 specified 5 tests (P81..P85). P84 was promoted from
"SKIP cfg or wait for D3" to "exercise the BEGIN_INDEX
BEGIN_METHOD_CALL path with the available `.push` chain"; P86 was
added during P3.2 to cover push-into-`[str]`-array.

| ID | Shape | Subject ret-type | Index | Coverage |
|---|---|---|---|---|
| **P81** | `make_arr()[1]` | `[i64]` | INT literal | Baseline call-index — confirms B0.2 silent miscompile fixed |
| **P82** | `to_int(opi[i])` | `[str]` element | call-of-method | `_fj_arr_get_str` dispatch on str-array element via to_int |
| **P83** | `make_arr()[one()]` | `[i64]` | another call | Nested calls — exercises P2.4 cursor audit |
| **P84** | `a.push(9)[2]` | `_FjArr*` (push retval) | INT literal | BEGIN_INDEX over BEGIN_METHOD_CALL — D3.B path |
| **P85** | `make_arr()[0] + make_arr()[1]` | `[i64]` | INT × 2 | Multiple invocations + Pratt + (R4 mitigation) |
| **P86** | `arr.push(s)[i]` | `[str]` | INT literal | Push into [str] then index — D3.B str-element dispatch |

Each test asserts the binary's stdout, not just compile success
— per §0's silent-miscompile lesson.

Cumulative stage1_full: **80 → 86** at Phase 18 close.

| Suite | Pre-Phase-18 | Post-Phase-18 |
|---|---|---|
| stage1_subset | 5 | 5 |
| stage1_full | 80 | **86** |
| stage2_repro | 6 | 6 |
| phase17_self_compile | 4 | 4 |
| **total self-host** | 95 | **101** |

## §4 — Effort recap

| Sub-item | Plan (likely) | Plan (cap +30%) | Actual | Variance vs cap |
|---|---|---|---|---|
| B0 audit + findings doc | 0.75h | 1h | 1.5h | +50% (plan estimate optimistic) |
| Plan authoring (`CALL_INDEX_PLAN.md`) | n/a | n/a | 1h | (not budgeted in plan; meta-work) |
| Decision doc | 0.5h | 0.65h | 0.5h | -23% |
| P1.1 — call branch | 1h | 1.5h | 30min | -67% |
| P1.2 — method branch | 1h | 2h | 15min | -88% |
| P2.1 — BEGIN_INDEX dispatch | 1h | 1.5h | 50min | -44% |
| P2.2 / P2.3 — guard sites | 0.5h | 1h | 30min | -50% |
| P2.4 — cursor audit | 0.5h | 1h | 10min | -83% |
| P3.1+P3.2 — ret-type registry | 0.5h | 1h | 20min | -67% |
| P4 — phase18 findings + prevention | 0.5h | 1h | ~45min | -25% |
| Test authoring (P81..P86) | 0.75h | 1h | (folded into per-step actuals) | n/a |
| **Total** | **6.75h** | **8.75h ceiling** | **~6h** | **-31% vs ceiling / -11% vs likely** |

The 4 high-surprise bug classes from plan §4 Risk Register did NOT
materialize at any cost above the audit cost itself:

| Risk | Probability | Outcome |
|---|---|---|
| **R1** Stage 2 byte-equality breaks | HIGH | DID NOT FIRE — md5 unchanged. fjc binary still self-compiles to byte-identical C. |
| **R5** Off-by-one cursor at `r.pos + 1` | HIGH | DID NOT FIRE — P2.4 audit confirmed consistency across 3 paths. |
| **R3** codegen self-compile breaks | MEDIUM | DID NOT FIRE — phase17 4/4 PASS @ 113s after every P-step. |
| **R4** `f()[i] + 1` Pratt mismatch | MEDIUM | DID NOT FIRE — P85 (two invocations + addition) PASS. |

Plan estimates were uniformly conservative (-44% to -89% per step)
because the foundation work in Phase 17 (Pratt postfix machinery,
BEGIN_METHOD_CALL subject generalization, struct-field tracking)
already paid the architectural cost. Phase 18 is the user-visible
consumption of Phase 17's infrastructure. CLAUDE.md §6.8 R5 cap of
+30% was correct; the *floor* was much lower than the optimistic
estimate.

## §5 — Prevention layer (per CLAUDE.md §6.8 R3)

Three mechanisms ship with Phase 18, mirroring the prevention
discipline established in Phase 17:

| Mechanism | Where | What it prevents |
|---|---|---|
| **P81..P86 regression tests** | `tests/selfhost_stage1_full.rs` | Future regression of `f()[i]` / `obj.m()[i]` parsing or codegen dispatch. Each test asserts binary stdout (not compile success only) — catches the silent-miscompile class B0.2 surfaced. |
| **Pre-push hook** (NEW) | `scripts/git-hooks/pre-push` (installed via `bash scripts/install-git-hooks.sh`) | Runs `cargo test --release --test selfhost_stage1_full --test selfhost_phase17_self_compile` before every push. Blocks regressions of any P-test or Stage 2 byte-equality from reaching origin. |
| **Phase 18 findings doc** (THIS) | `docs/SELFHOST_FJ_PHASE_18_FINDINGS.md` | Audit-trail closure per CLAUDE.md §6.6 R5 + §6.8 R1. Future maintainers can trace D1.A + D2.A + D3.B rationale. |
| **CLAUDE.md ref + Phase 17 §17.10 flip** | `docs/SELFHOST_FJ_PHASE_17_FINDINGS.md` §17.10 | Flips ❌ `arr[i] for [str]-typed arr in user-extended codegen` → ✅ closed Phase 18 (doc-integrity per §6.6 R3). |

The pre-push hook is the load-bearing prevention layer here. Per
CLAUDE.md §6.8 R3, "every Phase ships a pre-commit hook, CI job, or
CLAUDE.md rule. The patch alone is not the deliverable." Phase 18's
patch is the parser/codegen surgery; the pre-push gate is the rule
that prevents silent reversal. The hook covers BOTH stage1_full
(P-tests) and phase17_self_compile (Stage 2 fixed-point) so any
change that breaks self-host gets caught before push.

## §6 — Honest scope at Phase 18 close (per CLAUDE.md §6.6 R3)

What works:
- ✅ `f()[i]` over `[i64]` and `[str]` in self-host source
- ✅ `f()[g()]` nested call-as-index
- ✅ `f()[i] + f()[j]` multiple invocations + Pratt
- ✅ `obj.m()[i]` chained method-then-index (BEGIN_METHOD_CALL → BEGIN_INDEX)
- ✅ Free `push(arr, elem)` and `to_int(arr[i])` over BEGIN_INDEX-call-subject
- ✅ Stage 2 byte-equality unchanged (md5 `1d6c52a...` for self-compile,
     `d47fb8a...` for cross-stage)
- ✅ Method ret-type registry (`map_method_ret_type`) wired across 3 sites
     — forward-investment for first `[str]`-returning method

What does NOT work yet (legitimate scope-boundary):
- ❌ `f()[0][1]` — recursive index after call. Single-level only.
- ❌ `f()[i].method()` — chain after index-of-call. Plan §1 P1.3 deferred.
- ❌ `parse_expr_emit_with_type` typed-result refactor (D2.B) — touches
     12 emit branches; deferred to Phase 19+ roadmap.

What stays deferred (genuinely separate scope):
- ⏸️ FJARR_LEAK — bump-pointer arena for fj-source `_FjArr` allocations
     (per `docs/decisions/2026-05-07-fjarr-leak-strategy.md` Choice F).
     Phase 19 = strategy A (~7h cap, ships v35.1.0).
- ⏸️ T4 dup-fn detection — separate plan, closed independently in
     commit `38e23f56`.

## §7 — Cumulative state at Phase 18 close

23 self-host phases (0..18) closed; cumulative ~38h Claude time
across v33.4.0..v35.1.0-pre.

| Aggregate | At v35.0.0 | At Phase 18 close |
|---|---|---|
| Self-host tests | 95 | **101** |
| Stage1-full tests | 80 (P1..P80) | **86 (P1..P86)** |
| fj LOC self-hosting | 3206 | 3206+ (parser_ast/codegen_driver tweaks) |
| Stage 1 → Stage 2 C md5 | `1d6c52a...` | `1d6c52a...` (unchanged) |
| Cross-stage third-party md5 | `d47fb8a...` | `d47fb8a...` (unchanged) |
| Silent-miscompile classes closed | — | **+1** (call-index family) |

## Decision gate (§6.8 R6)

Phase 18 closed → ready for v35.1.0 release packaging (CHANGELOG +
GitHub Release). Pre-push hook installed via
`bash scripts/install-git-hooks.sh`. Audit-trail complete:
plan + B0 + decisions + this findings doc.

After Phase 18 the next-default work item per resume protocol is
**FJARR_LEAK Phase 1** (Choice F.A — bump-pointer arena, ~7h cap,
ships v35.1.0). Phase 2 (D linear types) deferred to v36.x roadmap.

---

*SELFHOST_FJ_PHASE_18_FINDINGS — written 2026-05-07. Phase 18 closed
in ~6h actual / 8.75h ceiling (-31%). Silent-miscompile class for
`f()[i]` / `obj.m()[i]` resolved per D1.A + D2.A + D3.B. 6 new
P-tests + 1 new pre-push regression hook ship as the prevention layer.
Stage 2 byte-equality preserved; fjc binary continues self-hosting at
fixed point.*
