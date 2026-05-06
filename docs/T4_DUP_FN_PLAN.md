# Plan: T4 — Close duplicate-fn-def known-limitation

**Source: Plan agent dispatch 2026-05-07.** Read-only agent; saved here for review.

## Executive summary (≈200 words)

T4 (duplicate-fn detection in `stdlib/analyzer.fj`) fails because `extract_ident` returns `f"var_{idx}"` instead of real source text — the placeholder collides with itself only by index, never by name, so `scope_contains` reports two `fn f` declarations as unique. The detection logic itself (line 329's `scope_contains` against `state.var_names`, and the Pass-1 `define_fn` walk) is correct and ships unchanged once `extract_ident` returns real text.

Two strategies are available and the choice is the user's: **Option A** extends `lexer.fj` with a span-aware `tokenize_with_spans(source) -> { tokens: [i64], starts: [i64], ends: [i64] }` (parallel-arrays — matching analyzer.fj's existing convention; tuples are rare in stdlib) and `extract_ident` slices `source.substring(start, end)`. **Option B** adds a Rust builtin `__fj_token_text(source, token_idx) -> str` registered in `src/interpreter/eval/builtins.rs`. A pre-flight audit (B0) confirms 6 call sites, no committed T4 test (only the doc smoke test), and `extract_ident` still placeholder. The plan delivers Option A and Option B branches with runnable verifications, locks T4 closed via a new `tests/selfhost_analyzer_dup_detection.rs` harness, and adds `selfhost_analyzer_extract_ident_not_placeholder` as a regression guard. Recommend **Option A** for bootstrap purity; user decides via committed `docs/decisions/2026-05-07-extract-ident-strategy.md`.

---

## §0 — Pre-flight audit (B0)

| # | Check | Command | Pass criterion |
|---|---|---|---|
| B0.1 | `extract_ident` still placeholder | `grep -n 'f"var_{idx}"' "/home/primecore/Documents/Fajar Lang/stdlib/analyzer.fj"` | One match at line ~376 |
| B0.2 | Call site count + locations | `grep -n 'extract_ident(' "/home/primecore/Documents/Fajar Lang/stdlib/analyzer.fj"` | 6 sites + 1 def at L279, 286, 293, 300, 327, 342, 373 |
| B0.3 | Surrounding infra intact | `grep -n 'fn scope_contains\|fn define_fn\|fn analyze_tokens' "/home/primecore/Documents/Fajar Lang/stdlib/analyzer.fj"` | All three defined |
| B0.4 | T4 has NO committed test today | `grep -rn "duplicate.*fn\|T4\|fn f().*fn f" "/home/primecore/Documents/Fajar Lang/tests/" \| grep -i selfhost` | No hits — confirms gap |
| B0.5 | Phase 3 doc surfaces it | `grep -n "T4\|extract_ident\|var_{idx}" "/home/primecore/Documents/Fajar Lang/docs/SELFHOST_FJ_PHASE_3_FINDINGS.md"` | Hits at §3.3, §3.4, §3.8 |
| B0.6 | Live demo of T4 today (RED) | Ad-hoc `.fj` driver: `tokenize("fn f(){} fn f(){}")` then `analyze_tokens(t, src)`; assert `error_count >= 1` | Today: `error_count = 0` (RED). Goal post-fix: `>= 1` (GREEN) |
| B0.7 | compiler.fj consumer of analyze_tokens | `grep -n 'analyze_tokens' "/home/primecore/Documents/Fajar Lang/stdlib/compiler.fj"` | Hit at L41 — confirms API ripples |
| B0.8 | `Value::Tuple` exists in interpreter (Option A consideration) | `grep -n 'Tuple(Vec<Value>)' "/home/primecore/Documents/Fajar Lang/src/interpreter/value.rs"` | Hit at ~L203 |
| B0.9 | Parallel-array convention used elsewhere in analyzer.fj | `grep -c 'var_names\|var_types\|fn_names\|fn_param_counts' "/home/primecore/Documents/Fajar Lang/stdlib/analyzer.fj"` | ≥30 hits — confirms pattern fits Option A |
| B0.10 | No existing fj-token-text builtin | `grep -rn 'fj_token_text\|fj_ident_at\|tokens_with_text' "/home/primecore/Documents/Fajar Lang/src/" "/home/primecore/Documents/Fajar Lang/stdlib/"` | Zero — confirms greenfield |
| B0.11 | Baseline test suite green pre-change | `cargo test --lib selfhost_ && cargo test --tests selfhost_` | All `selfhost_*` PASS |

**Output:** `docs/SELFHOST_FJ_PHASE_3_T4_B0_FINDINGS.md` committed before §1 starts.

---

## §1 — Design space + decision gate (Option A vs B)

Per §6.8 R6: committed `docs/decisions/2026-05-07-extract-ident-strategy.md`.

### 1.1 Tradeoffs table

| Axis | Option A — fj-side spans | Option B — Rust builtin |
|---|---|---|
| Touchpoints in `lexer.fj` | YES — new `tokenize_with_spans(source) -> SpanResult` | NO — lexer untouched |
| Touchpoints in `analyzer.fj` | rewrite `extract_ident(source, starts, ends, idx)` body to `source.substring(starts[idx], ends[idx])` + thread `starts/ends` through `analyze_tokens` (or store on `AnalyzerState`) | rewrite `extract_ident` body to `__fj_token_text(source, idx)` — call-site sigs unchanged |
| Touchpoints in `src/interpreter/eval/builtins.rs` | NONE | NEW arm for `"__fj_token_text"` — ~30 LOC including arity + type checks |
| Bootstrap purity | HIGH — fully native fj binary needs no Rust hook for ident extraction | LOW — fj binary depends on `__fj_token_text` builtin in runtime |
| FFI surface growth | 0 | +1 builtin; sets precedent |
| Migration cost | 6 call-site sigs + `analyze_tokens` sig + `compiler.fj` (1 site) + state struct grows by 2 fields | 0 call-site sigs |
| Performance | One pass over source rebuilt during `tokenize_with_spans`; `extract_ident` is O(end−start) substring | `__fj_token_text` re-walks tokens server-side OR receives precomputed spans |
| Stage 2 reproducibility risk | Low — pure fj change | Low, but builds on a runtime contract |
| Test coverage | Same |
| Maintainability | Future analyzer features (`line:col`, span-aware diagnostics, dup-struct/const/enum) all benefit | Each new "give me source-level info" need adds another builtin |
| Effort estimate (raw) | ~3–5h | ~1.5–3h |
| With +25% surprise (R5) | ~4–6.25h | ~2–3.75h |

### 1.2 Recommendation

**Option A.** Reasoning:

1. Phase 17 closed Stage 2 self-host fixed-point. Bootstrap story cleaner if span tracking lives in `lexer.fj` — once fjc compiles itself byte-identical, the lexer carries everything analyzer needs.
2. `analyzer.fj` already commits to **parallel arrays** (B0.9 ≥30 hits). Adding `token_starts/ends: [i64]` is idiomatic. `Value::Tuple` shape is grammatically possible but stylistically alien.
3. Future T-class limitations same `starts/ends` arrays unlock for free: `line:col` in `format_error`, span-aware SE004 arrows, dup-struct/const/enum, use-after-move arrows. Option B forces per-feature builtin each time.
4. Option B's only advantage is "fewer LOC moved." Actual delta small either way (A: ~50 LOC across 2 fj files; B: ~30 Rust LOC + 1 fj call-site change). Bootstrap cleanliness wins.

**THIS DECISION REQUIRES THE USER'S APPROVAL** before implementation.

---

## §2 — Phased tasks (per option)

### §2.A — Option A branch

| # | Task | Verification | Surprise |
|---|---|---|---|
| **A1** | In `stdlib/lexer.fj`, add `pub fn tokenize_with_spans(source: str) -> SpanResult`. `SpanResult { tokens: [i64], starts: [i64], ends: [i64] }`. Same logic as `tokenize` but record byte-offset start/end per token. **Keep `tokenize` unchanged.** | `cargo run -- check stdlib/lexer.fj` clean. Smoke: `tokenize_with_spans("fn f(){}")` returns `tokens.len() == ends.len() == starts.len()` AND `source.substring(starts[1], ends[1]) == "f"`. | +30% |
| **A2** | In `stdlib/analyzer.fj`, change `extract_ident` sig to `fn extract_ident(source: str, starts: [i64], ends: [i64], idx: i64) -> str { source.substring(starts[idx], ends[idx]) }`. | `cargo run -- check stdlib/analyzer.fj` clean. Unit driver recovers known ident. | +25% |
| **A3** | Change `pub fn analyze_tokens(tokens: [i64], source: str) -> AnalyzerState` to `pub fn analyze_tokens_with_spans(spans: SpanResult, source: str) -> AnalyzerState`. Update 6 `extract_ident` call sites. **Keep old `analyze_tokens` as thin shim** that calls `tokenize_with_spans` internally — preserves `compiler.fj` API. | `cargo run -- check stdlib/analyzer.fj` + `cargo run -- check stdlib/compiler.fj` clean. | +25% |
| **A4** | T4 driver: `fn f(){} fn f(){}` reports `error_count >= 1` and `format_error` prints `SE006: duplicate definition 'f'`. Capture in new `tests/selfhost_analyzer_dup_detection.rs`. | `cargo test --tests selfhost_analyzer_dup_detection` — RED on main, GREEN after A1–A3. | +25% |
| **A5** | Add `scope_contains(state.fn_names, name)` guard in Pass-1 around L280/287/293/300 → emit `ERR_DUPLICATE_DEF`. Pass 1 currently silently re-`define_fn`s on second `fn f`. | T4 driver shows error AT EXPECTED LOCATION (Pass 1, not 2). New T4b: `let x = 1; fn x(){}` produces 0 dup errors. | +25% |
| **A6** | Re-run full self-host suite. | `cargo test --lib selfhost_ && cargo test --tests selfhost_`. `selfhost_phase17_self_compile` (Stage 2 fixed point) PASS. | +30% (Stage 2 byte-identity may shift if lexer changes alter token order — should NOT, but canary). |
| **A7** | `cargo clippy --lib -- -D warnings && cargo fmt -- --check` | Clean. | +0% |

**Total Option A:** 4h × 1.25–1.30 = 5.0–5.2h.

### §2.B — Option B branch

| # | Task | Verification | Surprise |
|---|---|---|---|
| **B1** | In `src/interpreter/eval/builtins.rs`, add `match` arm for `"__fj_token_text"`. Sig: `(source: Value::Str, idx: Value::Int) -> Value::Str`. Re-walk lexer or cache by `(source_ptr, idx)`. | New unit test in `tests/builtin_token_text.rs`: `__fj_token_text("fn f(){}", 1)` returns `"f"`. | +30% |
| **B2** | In `stdlib/analyzer.fj`, replace `extract_ident` body: `__fj_token_text(source, idx)`. Sigs unchanged. | `cargo run -- check stdlib/analyzer.fj` clean. T4 driver: `error_count >= 1`. | +25% |
| **B3** | Same as A5 (Pass-1 dup-fn guard). | Same. | +25% |
| **B4** | `tests/selfhost_analyzer_dup_detection.rs` — same as A4. | RED before B1–B2; GREEN after. | +25% |
| **B5** | Re-run full self-host suite + Stage 2. | `selfhost_phase17_self_compile` PASS. | +30% |
| **B6** | clippy + fmt. | Clean. | +0% |
| **B7** | Document new builtin in `docs/STDLIB_SPEC.md`. | Visual review + doc-coverage script. | +0% |

**Total Option B:** 2.5h × 1.25–1.30 = 3.1–3.3h.

---

## §3 — Prevention layer

### 3.1 Direct T4 lock — committed test

`tests/selfhost_analyzer_dup_detection.rs`:

| Test | Source | Expected |
|---|---|---|
| `dup_fn_simple_two_decls` | `fn f(){} fn f(){}` | `error_count >= 1` AND `format_error` contains `SE006: duplicate definition 'f'` |
| `dup_fn_with_pub_modifier` | `pub fn f(){} fn f(){}` | Same |
| `dup_fn_disjoint_does_not_fire` | `fn f(){} fn g(){}` | 0 errors |
| `dup_struct` | `struct A{} struct A{}` | ≥1 error |
| `dup_enum` | `enum E{} enum E{}` | ≥1 error |
| `dup_const` | `const X: i64 = 1 const X: i64 = 2` | ≥1 error |
| `let_shadow_fn_does_not_fire` | `fn x(){} let x = 1` | 0 errors (separate namespaces) |
| `let_shadow_let_in_same_scope_fires` | `let x = 1; let x = 2` | ≥1 error |

### 3.2 Indirect lock — extract_ident-not-placeholder grep test

`tests/selfhost_analyzer_extract_ident_not_placeholder.rs`:

```rust
#[test]
fn analyzer_extract_ident_returns_real_text_not_placeholder() {
    let source = std::fs::read_to_string("stdlib/analyzer.fj").unwrap();
    assert!(
        !source.contains(r#"f"var_{idx}""#),
        "extract_ident reverted to placeholder — T4 dup-fn detection broken; \
         see docs/SELFHOST_FJ_PHASE_3_T4_*"
    );
}
```

### 3.3 Phase 3 findings update

Append to `docs/SELFHOST_FJ_PHASE_3_FINDINGS.md` OR new `docs/SELFHOST_FJ_PHASE_3_T4_CLOSURE_FINDINGS.md`:
- §3.3 row: `T4 ✅ PASS`
- §3.5 coverage table: ident extraction ✅
- Risk register: R6 closed
- Variance tag in commit (R5)

---

## §4 — Risk register

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| RT1 | Stage 2 byte-identity (`md5 d47fb8a...`) shifts | MEDIUM | HIGH (re-opens Phase 17) | Stage 2 hashes COMPILED OUTPUT not source; analyzer source change should be transparent. Verify in A6/B5. If hash drifts, re-cap canonical hash + update fixture in same commit. |
| RT2 | (A) `tokenize_with_spans` desyncs from `tokenize` | LOW | HIGH | New test `selfhost_lexer_tokenize_vs_tokenize_with_spans_agree.rs`: assert `tokenize(s) == tokenize_with_spans(s).tokens` for `examples/*.fj` corpus |
| RT3 | (A) Returning struct-of-arrays from pub fn — fj-source codegen path not exercised | MEDIUM | MEDIUM | `AnalyzerState` already does this. If fails in fjc native (Phase 17 chain), fall back to flat parallel arrays separately OR move spans onto `AnalyzerState`. |
| RT4 | (B) Stage 2 reproducibility breaks because builtin name appears as string literal | LOW | HIGH | Builtin invocation is `Value::BuiltinFn` — codegen doesn't emit literal. Verify in B5. |
| RT5 | (B) Builtin precedent erodes bootstrap story | MEDIUM | MEDIUM (long-term) | Document policy in decision file. |
| RT6 | `compiler.fj` callers break (Option A renames) | LOW | MEDIUM | A3 keeps `analyze_tokens` shim — no breakage. |
| RT7 | Pass-1 dup-fn (A5/B3) false positives — pass-1 walks `pub fn name(` independently | LOW | MEDIUM | Unit driver `pub fn f(){} fn g(){}` → 0 errors. |
| RT8 | fj `substring` on UTF-8 byte boundaries with non-ASCII | LOW | LOW | Subset programs ASCII; defer for Stage-1-Full. |
| RT9 | Wiring decision-doc pre-commit hook (R6) | MEDIUM | LOW | Use existing `docs/decisions/` convention if present, else flag 30-min meta-task. |

---

## §5 — Total budget per option

| Option | Raw | +25% | +30% (high-uncertainty) | Cap |
|---|---|---|---|---|
| A — fj-side spans | 4h | 5h | 5.2h | **5.5h** (round up for findings + decision-doc + commit-tagging) |
| B — Rust builtin | 2.5h | 3.1h | 3.3h | **3.5h** |
| Either + B0 audit + decision doc | +1h | — | — | +1h |

**Recommended budget envelope:**
- Option A path total: **6.5h cap** (5.5h impl + 1h B0/decision)
- Option B path total: **4.5h cap** (3.5h impl + 1h B0/decision)

Variance tag: `feat(selfhost-t4): close T4 dup-fn detection [actual Xh, est Yh, +Z%]`.

---

## §7 — Decisions surfaced (NOT auto-resolved)

User calls that block work:

1. **Option A vs B.** Plan recommends A.
2. (A only) `SpanResult` shape: struct vs three top-level return arrays. Plan picks struct.
3. (A only) Should `analyze_tokens` shim stay forever? Plan keeps shim indefinitely.
4. (B only) Builtin name: `__fj_token_text` (matching `__fj_print` convention) vs `token_text`. Plan picks `__fj_token_text`.
5. Pass-1 dup-fn guard (A5/B3) emit at SECOND `define_fn` call OR walk pre-fn-name list once before any `define_fn`? Plan picks former (1-line guard).
6. Sibling tests (dup-struct/const/enum) — ship with T4 closure or follow-on phase? Plan ships with — cheap once `extract_ident` works.
7. Stage 2 hash drift response: re-pin in same commit OR block? Plan: re-pin with WHY comment.

---

### Critical files
- `/home/primecore/Documents/Fajar Lang/stdlib/analyzer.fj`
- `/home/primecore/Documents/Fajar Lang/stdlib/lexer.fj`
- `/home/primecore/Documents/Fajar Lang/stdlib/compiler.fj`
- `/home/primecore/Documents/Fajar Lang/src/interpreter/eval/builtins.rs` (Option B only)
- new: `tests/selfhost_analyzer_dup_detection.rs`
- new: `tests/selfhost_analyzer_extract_ident_not_placeholder.rs`
