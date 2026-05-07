---
phase: T4 dup-fn closure — B0 pre-flight audit
plan: docs/T4_DUP_FN_PLAN.md §0
status: B0 CLOSED 2026-05-07
artifacts: this doc
purpose: empirical baseline before §1 decision (Option A vs Option B)
prereq: docs/T4_DUP_FN_PLAN.md committed (commit 77bda3f0)
---

# T4 dup-fn — B0 Pre-Flight Audit Findings

> Per CLAUDE.md §6.8 R1, every Phase opens with a pre-flight audit
> that confirms baseline via runnable commands. This doc records
> outputs of B0.1–B0.11 from `docs/T4_DUP_FN_PLAN.md` §0. **No
> implementation work begins until §1 decision (Option A vs Option B)
> is committed.**

## Method

Each row records the runnable command from the plan and the verbatim
output (or its essential summary). Static evidence from grep/source
inspection where dynamic execution is blocked by orthogonal concerns
(see B0.6 below).

## B0.1 — `extract_ident` still placeholder ✅ CONFIRMED

**Command:** `grep -n 'f"var_{idx}"' stdlib/analyzer.fj`

**Output:**
```
376:    f"var_{idx}"
```

**Conclusion:** placeholder lives at line 376 of analyzer.fj. The
implementation has not changed since Phase 3 findings (committed
in earlier work).

## B0.2 — Call sites + def location ✅ CONFIRMED

**Command:** `grep -n 'extract_ident(' stdlib/analyzer.fj`

**Output:**
```
279:            let name = extract_ident(source, tokens, i + 1)
286:            let name = extract_ident(source, tokens, i + 2)
293:            let name = extract_ident(source, tokens, i + 1)
300:            let name = extract_ident(source, tokens, i + 1)
327:                let name = extract_ident(source, tokens, next)
342:            let name = extract_ident(source, tokens, j + 1)
373:fn extract_ident(source: str, tokens: [i64], idx: i64) -> str {
```

**Conclusion:** **6 call sites** (L279, 286, 293, 300, 327, 342) +
**1 def** (L373). Plan estimate (6 sites + 1 def) confirmed exact.

## B0.3 — Surrounding infra intact ✅ CONFIRMED

**Command:** `grep -n 'fn scope_contains\|fn define_fn\|fn analyze_tokens' stdlib/analyzer.fj`

**Output:**
```
51:fn scope_contains(names: [str], name: str) -> bool {
226:fn define_fn(state: AnalyzerState, name: str, param_count: i64) -> AnalyzerState {
270:pub fn analyze_tokens(tokens: [i64], source: str) -> AnalyzerState {
```

**Conclusion:** All three core symbols exist. Detection logic ships
unchanged — bug is purely in `extract_ident` returning unique
placeholders. Once `extract_ident` returns real text, `define_fn`
+ `scope_contains` will correctly detect duplicates.

## B0.4 — No T4 test committed ✅ CONFIRMED (matches plan expectation)

**Command:** `grep -rn "duplicate.*fn\|T4\|fn f().*fn f" tests/ | grep -i selfhost`

**Output:** *(empty)*

**Conclusion:** No committed regression test for T4. Phase 3 findings
ran T4 as "ad-hoc smoke test" via copy-paste. Confirms the gap that
§3.1 of the plan (`tests/selfhost_analyzer_dup_detection.rs`) closes.

## B0.5 — Phase 3 doc surfaces T4 ✅ CONFIRMED

**Command:** `grep -n "T4\|extract_ident\|var_{idx}" docs/SELFHOST_FJ_PHASE_3_FINDINGS.md`

**Key output excerpts:**
- L20: "name extraction returns a placeholder `var_{idx}` instead of real"
- L54: "`extract_ident` | **Placeholder**: returns `f\"var_{idx}\"` instead of real text"
- L76: "T4 | duplicate fn def `fn f(){} fn f(){}` | errors≥1 | 0 | ❌ FAIL — known limitation"
- L83: "## 3.4 — T4 honest analysis (known limitation, NOT implementation bug)"
- L174: "**NEW R6** | **Ident text placeholder**"

**Conclusion:** Phase 3 documented T4 thoroughly. R6 (placeholder)
is the named risk this plan closes.

## B0.6 — Live RED reproducer 🟡 STATIC EVIDENCE

**Plan command:** ad-hoc `.fj` driver calling `tokenize` + `analyze_tokens`.

**Attempt:** wrote `/tmp/t4_red.fj` calling those fns. `cargo run -- run`
returned `SE001: undefined variable 'tokenize'`. Same with `cargo run --
check stdlib/compiler.fj` (which itself calls `tokenize` and
`analyze_tokens`).

**Diagnosis:** `stdlib/*.fj` modules are **source-of-truth reference
implementations**, not loaded into the fj CLI runtime by default. They
are consumed by:
- Bootstrap tests (`tests/selfhost_tests.rs` — runs as Rust harness)
- Stage 2 self-host chain (`stdlib/codegen.fj` etc. compile through
  the chain)

There is no "load this module + call its functions interactively" path
today. So the dynamic T4 reproducer is blocked by an **orthogonal
concern** (stdlib module loading from CLI), not by anything related to
T4 itself.

**Static evidence is mathematically sufficient:**

1. `extract_ident(source, tokens, idx)` returns `f"var_{idx}"` — a
   string formatted from the integer `idx`.
2. Two distinct call sites for `fn f` will pass DIFFERENT `idx`
   values (the token positions of the two `fn` keywords).
3. Therefore `extract_ident` returns DIFFERENT placeholders for the
   two `fn f` declarations: e.g. `var_1` and `var_5`.
4. `scope_contains(state.fn_names, "var_1")` is checked against a
   list containing `var_5` → returns `false`.
5. `define_fn` adds `var_1` to the list, no error reported.
6. `error_count` stays at 0.

**T4 is RED by construction.** Dynamic reproduction would only
re-confirm this; static derivation is sound.

**Action for plan §2:** when Option A or B lands, the regression test
will exercise the GREEN path via the `tests/selfhost_*.rs` Rust
harness pattern (which CAN call into fj-source via the existing test
infrastructure). The B0.6 gap is closed by §3.1 of the plan.

## B0.7 — `compiler.fj` consumer of `analyze_tokens` ✅ CONFIRMED

**Command:** `grep -n 'analyze_tokens' stdlib/compiler.fj`

**Output:**
```
7://   stdlib/analyzer.fj — analyze_tokens(tokens, source) → AnalyzerState
41:    let state = analyze_tokens(tokens2, source)
```

**Conclusion:** **1 consumer** at L41. Option A's plan to keep
`analyze_tokens` as a thin shim (per A3) is well-targeted —
`compiler.fj` won't break.

## B0.8 — `Value::Tuple` exists in interpreter ✅ CONFIRMED

**Command:** `grep -n 'Tuple(Vec<Value>)' src/interpreter/value.rs`

**Output:**
```
203:    Tuple(Vec<Value>),
```

**Conclusion:** Tuple shape is available IF Option A picks
"three top-level return values" instead of struct-of-arrays. (Plan
recommends struct-of-arrays via `SpanResult` to match `AnalyzerState`
convention; this is a minor decision noted in plan §1.2.)

## B0.9 — Parallel-array convention count ✅ CONFIRMED (35 hits)

**Command:** `grep -c 'var_names\|var_types\|fn_names\|fn_param_counts' stdlib/analyzer.fj`

**Output:** `35`

**Conclusion:** Plan estimated "≥30 hits"; actual is 35. **Parallel
arrays are the dominant convention in analyzer.fj.** This biases the
Option A `SpanResult` shape toward struct-of-`[i64]`-arrays rather
than tuples-of-i64.

## B0.10 — No existing fj-token-text builtin ✅ GREENFIELD

**Command:** `grep -rn 'fj_token_text\|fj_ident_at\|tokens_with_text' src/ stdlib/`

**Output:** *(empty)*

**Conclusion:** Greenfield for both options. Option A adds
`tokenize_with_spans` to `stdlib/lexer.fj`; Option B adds
`__fj_token_text` to `src/interpreter/eval/builtins.rs`. No
collision risk.

## B0.11 — Baseline test suite green pre-change ✅ CONFIRMED

(Verified at v35.0.0 + R15 closure earlier this session — see
commits `5a3e8bfb`, `3a3dd586`, `93a94ee8`.)

- `cargo test --lib`: 7,629 PASS
- `cargo test --tests`: 10,489 PASS (72 files)
- `cargo test --doc`: 14 PASS, 1 ignored
- Stress 5/5 at `--test-threads=64`: PASS
- clippy + fmt: clean

**Conclusion:** Starting from a known-clean baseline. Any new failures
introduced by §2 work are attributable to that work.

## Summary table

| ID | Check | Status | Notes |
|---|---|---|---|
| B0.1 | extract_ident placeholder | ✅ confirmed | line 376 |
| B0.2 | Call site count | ✅ confirmed | 6 sites + 1 def |
| B0.3 | Infra intact | ✅ confirmed | scope_contains + define_fn + analyze_tokens |
| B0.4 | T4 not committed test | ✅ confirmed | matches plan B0.4 |
| B0.5 | Phase 3 doc surfaces T4 | ✅ confirmed | 8 hits |
| B0.6 | Live RED reproducer | 🟡 static-only | dynamic blocked by stdlib loading; static derivation sound |
| B0.7 | compiler.fj consumer | ✅ confirmed | 1 site at L41 |
| B0.8 | Value::Tuple exists | ✅ confirmed | line 203 |
| B0.9 | Parallel-array convention | ✅ confirmed | 35 hits (>30 plan threshold) |
| B0.10 | No existing builtin | ✅ greenfield | both options safe |
| B0.11 | Baseline suite green | ✅ confirmed | from earlier session work |

## Decision-gate inputs (for §1)

The B0 audit confirms:

1. **Bug exists** as plan-described (B0.1).
2. **Detection logic is correct** — only `extract_ident` is broken
   (B0.3).
3. **Migration cost low** for both options:
   - Option A: 6 fj call sites + 1 shim caller (B0.2 + B0.7)
   - Option B: 0 fj call sites changed; +1 builtin (B0.10)
4. **Style fits**: parallel-array shape (B0.9, 35 hits) supports
   Option A's `SpanResult` design.
5. **No greenfield collision** for either option (B0.10).
6. **Baseline test suite is green** (B0.11), so any failures during
   §2 are attributable.

## Next step

Per §6.8 R6: commit `docs/decisions/2026-05-07-extract-ident-strategy.md`
recording **Option A vs Option B**. Until that file exists, no §2
implementation work starts.

---

*T4_DUP_FN_B0_FINDINGS — 2026-05-07. B0 closed 100% (10 ✅ + 1 🟡
static). Plan §1 decision unblocked: Option A or B.*
