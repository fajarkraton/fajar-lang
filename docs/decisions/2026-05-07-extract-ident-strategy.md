---
decision: T4 dup-fn detection — extract_ident strategy
date: 2026-05-07
status: ACCEPTED 2026-05-07 (batch-approved with companion D-files)
prereq: docs/T4_DUP_FN_PLAN.md, docs/T4_DUP_FN_B0_FINDINGS.md
plan: docs/T4_DUP_FN_PLAN.md §1
---

# T4 Decision — extract_ident strategy

## Choice

**Choice: A** (fj-side `tokenize_with_spans` in stdlib/lexer.fj)

> **Status: ACCEPTED 2026-05-07.** Batch-approved with the two companion
> decision files in this directory.

## Rationale (≥3 sentences)

Option A keeps the bootstrap-purity story aligned with v35.0.0's
fixed-point self-host milestone. After Phase 17 closure, the fj-source
compiler reaches fixed point; adding a Rust builtin (`__fj_token_text`,
Option B) would re-introduce a runtime dependency on
`src/interpreter/eval/builtins.rs` that a fully-bootstrapped fj binary
would still need. By contrast, Option A's `tokenize_with_spans(source) ->
SpanResult` puts span tracking in `stdlib/lexer.fj` where it composes
with the existing self-host chain.

The B0 audit (`docs/T4_DUP_FN_B0_FINDINGS.md` §B0.9) confirmed the
fj-source analyzer already commits to **parallel arrays** as its data
model (35 hits for `var_names`/`var_types`/`fn_names`/`fn_param_counts`
patterns). A `SpanResult` struct holding parallel `[i64]` arrays
(`tokens`, `starts`, `ends`) matches existing convention; a tuple-of-three
shape would be stylistically alien.

Future T-class limitations (real `line:col` in `format_error`, span-aware
SE004 type-mismatch arrows, dup-struct/dup-const/dup-enum, use-after-move
arrows) all become easy follow-ons once the lexer carries spans natively
— Option B would force a per-feature builtin each time, growing the FFI
surface.

The cost difference is small: Option A 4-6.5h vs Option B 2.5-3.5h. The
extra ~3h in Option A buys long-term bootstrap cleanliness that
compounds across multiple future features.

## @kernel-future-compat

**Compatible: yes** (no impact)

This decision is purely about how the analyzer extracts identifier
text. Neither option changes the type system or runtime model in ways
that affect `@kernel`/`@device`/`@safe` context isolation. Both options
add zero new heap allocations in `@kernel` paths.

## Migration path

1. **Pre-flight committed:** `docs/T4_DUP_FN_B0_FINDINGS.md` (already
   committed in commit `f5448b03`).

2. **A1**: Add `pub fn tokenize_with_spans(source: str) -> SpanResult`
   to `stdlib/lexer.fj`. SpanResult = `struct { tokens: [i64], starts:
   [i64], ends: [i64] }`. Keep existing `tokenize` unchanged.

3. **A2**: Change `extract_ident` signature in `stdlib/analyzer.fj` to
   `(source, starts, ends, idx) -> str` and use `source.substring(...)`.

4. **A3**: Change `analyze_tokens` to `analyze_tokens_with_spans(spans:
   SpanResult, source) -> AnalyzerState`. Keep `analyze_tokens` as a
   thin shim calling `tokenize_with_spans` internally + forwarding.
   `compiler.fj` consumer (1 site at L41) unchanged.

5. **A4**: New regression test `tests/selfhost_analyzer_dup_detection.rs`
   asserting 6 dup-name detection cases (T4 + dup-struct + dup-enum +
   dup-const + 2 not-fire cases).

6. **A5**: Pass-1 dup-fn guard around L280/287/293/300 in analyzer.fj
   to actually emit `ERR_DUPLICATE_DEF` when `scope_contains(state.fn_names, name)`.

7. **A6**: Re-run full self-host suite. Verify Stage 2 byte-equality
   intact.

8. **Closure**: append closure section to
   `docs/SELFHOST_FJ_PHASE_3_FINDINGS.md` flipping T4 from ❌ FAIL to
   ✅ PASS.

All 95 existing self-host tests must remain green throughout.

## Surprise budget

**+25%** baseline (per CLAUDE.md §6.8 R5 default).

Not bumped to +30%: B0 audit confirmed all infrastructure intact
(scope_contains, define_fn, analyze_tokens, parallel-array convention),
no greenfield risk. The work is mechanical: rewire 6 call sites,
threading two new array fields.

Variance tag in commit: `feat(selfhost-t4): close T4 dup-fn detection
[actual Xh, est 6.5h, +Y%]`.

## Rejected candidates

- **Option B (Rust builtin `__fj_token_text`)**: lower effort (~2.5h
  shorter) but grows FFI surface; sets precedent for "ask the runtime
  for source-level info" that erodes bootstrap story. Plan §1
  identified ≥4 future features that would each be tempted to add
  another builtin under this path. Rejected for long-term cleanliness.

## Reverse-cost

**Low.** Both options share the same external API (`extract_ident`
signature + downstream callers). If Option A turns out to have a
performance issue (unlikely — `substring` is O(n), called per-ident at
analysis time, not in hot path), we can ship Option B as a faster
backend in a future commit without changing anything outside lexer.fj +
analyzer.fj.

If user prefers B, change `Choice: A` to `Choice: B` above and the
implementation just diverges to plan §2.B branch — call sites in
analyzer.fj stay the same shape, only the body of `extract_ident`
changes.

---

*ACCEPTED 2026-05-07. Implementation proceeds per migration path §3
above. Variance tracking per §6.8 R5: tag commit with `[actual Xh,
est 6.5h, +Y%]`.*
