---
plan: fj-lang Self-Hosting Production Plan
target: Stage-1-Subset milestone first (~1-2 weeks); Stage-1-Full and Stage-2-Triple-Test follow
release: v33.4.0 (minor — adds self-host capability)
status: Phase 0 audit closed; Phase 1 subset-lexer in progress
---

# fj-lang Self-Hosting — Master Plan

## Goal

Land a genuinely self-hosted fj-lang compiler. **First milestone: Stage-1-Subset** —
a fj-source compiler (compiled by Stage 0 Rust binary) that can compile a 40-feature
subset of fj-lang programs to runnable binaries. This proves language maturity and
unlocks future "fj-lang compiles itself" claims.

## Out-of-scope

- **Sister Rust compiler stays.** `src/{lexer,parser,analyzer,codegen}/` remains
  the production fj-lang compiler. Self-host is parallel proof point, not a Rust replacement.
- **Full self-host (Stage-1-Full, Stage-2-Triple-Test)** deferred until after
  Subset milestone closes. Roadmap-only scope.
- **Existing `src/selfhost/*.rs` Rust simulation** — preparatory infrastructure;
  becomes reference for fj port. Not deleted.

## Phase 0 — Audit ✅ CLOSED

See `docs/SELFHOST_FJ_PHASE_0_FINDINGS.md`. Honest audit revealed existing
"self-host" is Rust simulation theatre. Path A (Stage-1-Subset) recommended.

## Phase 1 — Subset Lexer ✅ FIRST PROOF SHIPPED

| Task | Verification |
|---|---|
| 1.A Verify existing `stdlib/lexer.fj` (513 LOC, 4 fns) tokenizes subset-fj | `fj check stdlib/lexer.fj` → no errors ✅ |
| 1.B Bit-equivalent vs Rust lexer on canonical input | `fn add(a: i64, b: i64) -> i64 { a + b }` produces **19 tokens** matching Rust `fj dump-tokens` exactly ✅ |
| 1.C Token tag sequence test | All 19 tags match: `[15,133,110,133,118,36,119,133,118,36,111,121,36,112,133,70,133,113,0]` ✅ |
| 1.D Phase 1 findings doc | this section |

**Phase 1 effort**: ~30min Claude time. Existing stdlib/lexer.fj already has
the full tokenize fn including multi-char ops, string lits, char lits, f-strings,
ident/keyword separation. Sprint S44 baseline was already substantial.

## Phase 2 — Subset Parser

Existing `stdlib/parser.fj` (784 LOC, 26 fns) — partial. Need full Pratt expr
+ all stmt forms in Stage-1-Subset (40 features per `bootstrap_v2::SubsetDefinition`).

| Task | Verification |
|---|---|
| 2.A Audit `stdlib/parser.fj` coverage vs Rust parser | findings list missing parsers |
| 2.B Port missing expr forms (Pratt levels 1-19 except generics/closures/match) | bit-equivalent vs `fj dump-ast` on canonical inputs |
| 2.C Port missing stmt forms (let, fn, return, while, for, if/else, struct, enum, impl, use) | ditto |
| 2.D Port type expressions (i8..u128, f32..f64, &T, &mut T, [T; N], (T,U), Box<T>) | ditto |
| 2.E Phase 2 findings doc | committed |

**Estimated**: ~1-3K LOC fj. ~3-7 sessions per FAJAROS+FAJARQUANT velocity.

## Phase 3 — Subset Analyzer

Existing `stdlib/analyzer.fj` (432 LOC, 11 fns) — partial.

| Task | Verification |
|---|---|
| 3.A Symbol table + scope resolution | each test program matches Rust analyzer's symbol output |
| 3.B Type inference (subset — no generics) | each test program matches Rust analyzer's typed-AST |
| 3.C Context analysis (`@kernel`/`@device`/`@safe`/`@unsafe`) | per-fn context tags match Rust |
| 3.D Error code emission (subset of 78 codes — at least LE001-LE008, PE001-PE010, SE001-SE016) | error messages format-equivalent to Rust |
| 3.E Phase 3 findings doc | committed |

**Estimated**: ~2-5K LOC fj. ~5-10 sessions.

## Phase 4 — Subset Codegen via Cranelift FFI

**Critical-path decision point.** Recommended: expose Cranelift IR builder to fj-source
as builtins, fj-source emits IR, Rust internals lower to machine code.

| Task | Verification |
|---|---|
| 4.A Add fj-lang builtins: `cl_create_fn`, `cl_iadd`, `cl_isub`, `cl_imul`, `cl_load`, `cl_store`, `cl_call`, `cl_branch`, `cl_jump`, `cl_return`, `cl_emit_obj` | unit tests in `tests/cranelift_ffi_builtins.rs` |
| 4.B Port subset codegen logic from `src/codegen/cranelift/` to `stdlib/codegen.fj` | fj-source emits IR; Rust internals link object file |
| 4.C E2E test: small subset-fj program compiles end-to-end via fj-source codegen | output binary runs identically to Stage 0's output |
| 4.D Phase 4 findings doc | committed |

**Estimated**: ~3-7K LOC fj + ~2-5K LOC Rust FFI shim. ~7-15 sessions.

**Risk**: Cranelift FFI surface is large. If it bloats, fall back to Option 2
(emit LLVM IR text + shell-out to llc).

## Phase 5 — Stage-1-Subset Bootstrap

| Task | Verification |
|---|---|
| 5.A Build Stage 1: `fj build stdlib/{lexer,parser,analyzer,codegen}.fj -o fj-stage1` | binary produced, runs |
| 5.B Run Stage 1 on subset-fj test programs | each test outputs identical binary to Stage 0's compilation |
| 5.C Update `fj bootstrap` to actually invoke Stage 1 (not just initialize Rust struct) | `fj bootstrap --real` produces actual Stage 0 vs Stage 1 binary diff report |
| 5.D Phase 5 findings doc | committed |

## Phase 6 — Subset Test Suite

| Task | Verification |
|---|---|
| 6.A Curate ≥20 subset-fj test programs covering all 40 features | tests live in `tests/selfhost_stage1_subset/*.fj` |
| 6.B Each test: Stage 0 binary == Stage 1 binary (bit-equivalent or behavior-equivalent) | `make test-selfhost-stage1-subset` passes |
| 6.C CI integration | new GitHub Actions job: `selfhost_subset` |

## Phase 7 — Release v33.4.0

| Task | Verification |
|---|---|
| 7.A Bump Cargo.toml version 33.3.0 → 33.4.0 | committed |
| 7.B CHANGELOG.md entry | committed |
| 7.C README/CLAUDE.md sync: "Stage-1-Subset self-hosted" claim | grep verifies |
| 7.D Tag v33.4.0; GitHub Release with 5 platform binaries | `gh release view v33.4.0` |

## Effort Estimate

| Phase | Optimistic | Realistic per FAJAROS+FAJARQUANT pattern |
|---|---|---|
| 0 audit ✅ | done | done |
| 1 subset lexer ✅ | 30min | 30min (already shipped) |
| 2 subset parser | 1-2d | 0.5-1d |
| 3 subset analyzer | 2-3d | 1-2d |
| 4 subset codegen + Cranelift FFI | 3-7d | 1.5-3d |
| 5 bootstrap chain | 1d | 0.5d |
| 6 test suite | 1d | 0.5d |
| 7 release | 0.5d | 0.5d |
| **Total Stage-1-Subset** | **~9-15 days** | **~5-8 days** |

## Risk Register

| ID | Risk | Mitigation |
|---|---|---|
| R1 | fj-lang feature gaps surface | Pattern: -99% variance — each gap closes in 5-30min |
| R2 | Cranelift FFI shim large surface | Phase 4 fallback: LLVM IR text emission |
| R3 | Stage1 ≢ Stage0 (subtle semantic diff) | Subset is small; behavior-equivalent gate (not byte-equivalent) acceptable |
| R4 | Generics/traits in subset (excluded) leak via dependency | SubsetDefinition explicitly excludes; subset programs hand-curated |
| R5 | String manipulation slow in fj-lang interpreter | Phase 4 codegen runs through Rust internals; fj-source not on hot path |

## Decision Points (Resolved by Default Recommendations)

1. ✅ **Path**: Subset-first
2. ✅ **Codegen**: Cranelift FFI (with LLVM-IR fallback)
3. ✅ **Versioning**: v33.4.0 minor
4. ⏳ **Timing**: Phase 1 started — proceed when Anda confirm
5. ✅ **Sister Rust compiler**: stays indefinitely

## What This Plan Does NOT Claim (Honesty per CLAUDE.md §6.6)

- ❌ "Full self-host" — out of scope for v33.4.0; that's a future release (v34.0?)
- ❌ "Stage 2 triple test" — also future
- ❌ "Rust compiler deprecation" — never; Rust compiler is and stays the production reference

What it DOES claim once landed:
- ✅ "Stage-1-Subset self-hosted: fj-source compiler compiles subset-fj programs"
- ✅ Defensible per CLAUDE.md §6.6 R1 ("[x] means END-TO-END working")

---

*SELFHOST_FJ_PRODUCTION_PLAN — created 2026-05-05. Phase 0 audit closed;
Phase 1 first proof shipped (stdlib/lexer.fj produces bit-equivalent
token sequence vs Rust lexer for canonical input). Ready to continue
Phase 2-7 per founder approval.*
