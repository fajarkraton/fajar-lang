# Decision — Self-Host Parser Annotation Grammar Shape (v35.7.x Option B)

> **Date:** 2026-05-12
> **Owner:** Fajar (user delegation via "lanjutkan sesuai dengan rekomendasi")
> **Status:** ✅ Decided — D1.A skip-only this session, D1.B/D1.C deferred
> **B0 source:** `docs/V35_7_PARSER_ANNOTATION_GRAMMAR_B0_FINDINGS.md`
> **Plan Hygiene §6.8 R6:** This file is the committed decision; downstream
> work is gated on this shape.

## Decision

**Adopt D1.A (skip-only) for v35.7.0.** Defer D1.B (preserve in AST) and
D1.C (side-channel) to v36.x or later if a consumer for annotation data
ever emerges.

### Scope of D1.A

1. Add `skip_at_annotations(src, pos, n) -> i64` helper above `parse_to_ast`
   in `stdlib/parser_ast.fj`. Consumes `@<word>` plus optional paren-balanced
   `(<args>)`, looped to handle stacked annotations.
2. Call `skip_at_annotations` at TWO sites in `parse_to_ast`:
   - Before the optional `pub` skip → accepts `@safe pub fn …`
   - After the optional `pub` skip → accepts `pub @safe fn …`
   This matches the Rust parser's tolerance for either annotation order.
3. Add 5 regression tests in `tests/selfhost_stage1_full.rs` (`full_p87`
   through `full_p91`) exercising the new logic end-to-end through the
   self-host parse_to_ast → emit_program chain.
4. Do NOT modify the AST emitted by parse_fn_ast or sibling parsers.
   Codegen consumers (`stdlib/codegen_driver.fj`) see exactly the same
   AST shape as before.
5. Do NOT annotate any stdlib chain module in this commit. That's
   Phase 2, deferred to v35.8 / v36 as a separate audit.

### Why D1.A over the alternatives

| Option | Effort | Why rejected/deferred |
|---|---|---|
| **D1.B** Parse annotation into AST + propagate through codegen | ~2-3h | Premature: no consumer for annotation data today. fj-source analyzer.fj doesn't do context-isolation. Higher Phase17 risk. |
| **D1.C** Skip-and-record into side-channel map | ~1-2h | Adds state to a stateless dispatcher → ergonomic regression. Premature for the same reason as D1.B. |
| **D1.A** Skip-only | ~30-45min | **Chosen.** Smallest scope, closes the architectural blocker, codegen unchanged, minimal Phase17 risk. Phase 2 (annotate stdlib chain modules) unblocked for free in a separate later commit. |

### What D1.A does NOT do (intentional non-scope)

- Does NOT propagate annotation information into the AST.
- Does NOT add a side-channel for annotation data.
- Does NOT annotate any stdlib chain module (parser_ast/codegen/codegen_driver/
  analyzer/lexer) with `@`-annotations. Phase 2 deferred.
- Does NOT tag as v35.7.0 GitHub Release. Same pattern as B-δ: this is an
  internal architectural closure, not a user-visible feature. Next user-
  visible improvement bundles will include it.

### Re-entry conditions for D1.B (preserve in AST)

Open the AST-preservation refactor if any of these occur:

1. The fj-source analyzer.fj begins doing context-isolation enforcement
   (currently the Rust analyzer is sole authority).
2. A stdlib-level lint or static-analysis tool needs annotation visibility.
3. Phase 2 (annotate stdlib chain modules) ships and a downstream pass
   needs to dispatch on the annotation kind (e.g., `@kernel` should emit
   different C than `@safe` — though there's no such requirement today).

Until then, D1.A's prevention layer (5 stage1_full regression tests
locking in the skip semantics for `@safe`, `@inline`, `pub @safe`,
`@device("net")`, stacked `@inline @cold`) is the operative guard.

### Verification commands (executed @ ship)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Build release first (selfhost tests invoke target/release/fj)
cargo build --release

# D1.A regression tests
cargo test --release --test selfhost_stage1_full full_p8
# expect: 11 passed (full_p80..p89 incl. p86 baseline + p87..p89 new)
cargo test --release --test selfhost_stage1_full full_p9
# expect: 3 passed (full_p9 baseline + p90..p91 new)

# Full stage1_full suite
cargo test --release --test selfhost_stage1_full
# expect: 91 passed (was 86, +5 from D1.A)

# Stage 2 byte-equality (the critical gate)
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed — phase17_stage2_native_triple_test confirms Stage 1
# (Rust interpreter) and Stage 2 (Stage-1-native-binary) produce identical
# emit_C for the new parser_ast.fj source, including the new
# skip_at_annotations helper.

# Quality gates
cargo clippy --lib -- -D warnings   # 0 warnings
cargo fmt -- --check                # clean
```

### Stage 2 byte-equality risk: VERIFIED LOW

D1.A touches `stdlib/parser_ast.fj` (which IS part of the self-host
chain — both Stage 1 and Stage 2 process the modified source). The new
`skip_at_annotations` helper uses only chain-stable builtins (`while`,
`if`, `str_byte_at`, `read_word`, `skip_ws`, integer compare/add). The
phase17 4/4 PASS @ ship empirically confirms Stage 1 ≡ Stage 2 semantics
for the new logic.

### Phase 2 timing

Phase 2 (annotate stdlib chain modules with `@safe`, `@unsafe`, etc.) is
NOT bundled with D1.A per the "first step only" rule. Open a new session
to:
1. Audit which stdlib modules should get which annotations
   (`@safe` is the default since v35.6.0, so explicit `@safe` is mostly
   documentation; `@unsafe` makes sense for codegen modules that touch
   raw bytes; etc.).
2. Verify each annotation preserves phase17 byte-equality (the C emitted
   should not depend on annotations since codegen doesn't see them).
3. Ship as separate v35.7.1 (or v35.8.0) commit chain.

## References

- B0 findings: `docs/V35_7_PARSER_ANNOTATION_GRAMMAR_B0_FINDINGS.md` (commit `3fdee876`)
- Resume protocol: `memory/project_resume_lanjut_protocol.md` §2.B
- Predecessor §4.4 closure: `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §A.4 (the detour that surfaced this blocker)
- Companion B-δ decision (closed Option A): `docs/decisions/2026-05-12-cranelift-builtin-list-shape.md`
