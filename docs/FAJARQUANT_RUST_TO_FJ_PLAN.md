---
plan: FajarQuant Rust algorithm crate → Fajar Lang stdlib port
target: fajar-lang `src/runtime/ml/fajarquant/*.fj` (Option C)
release: fj-lang v33.3.0 (minor — adds stdlib module)
budget: 10.5-17 days realistic (Phase 0 finding compressed from 12-24)
status: Phase 0 CLOSED 2026-05-05; Phase 1 ready to start
---

# FajarQuant Rust → Fajar Lang Port — Master Plan

## Goal

Port the pure-Rust portion of FajarQuant algorithm crate (~2,649 LOC
across 7 modules) to Fajar Lang. After completion, fj-lang will have
`use fajarquant` as a stdlib module, the FajarQuant algorithm code
becomes 100% Fajar Lang, and the Rust crate continues to ship for
`crates.io` distribution as a sister artifact.

**OUT-OF-SCOPE**:
- Python training scripts (`python/phase_d`, `python/phase_e`) — stay as
  PyTorch/HuggingFace. Different lifecycle phase from inference.
- Vendored microsoft/BitNet TL2 C++ kernel (`cpu_kernels/tl2.rs` +
  `tl2_encoder.rs`, 1,502 LOC). Already PERMANENT-DEFERRED per
  fajarquant `docs/FJQ_PHASE_F_F11_*` chain.

**WHY** (per user discussion 2026-05-05):
- Strengthens "100% Fajar Lang" messaging consistency with FajarOS
- Stress-tests fj-lang Tensor type system at scale
- Enables stdlib `use fajarquant` import (better DX)
- Aligns with vision §1: "kernel + NN share codebase, type system, compiler"
- ZERO functional improvement; this is alignment work, not feature work

## Phase 0 — Pre-flight audit ✅ CLOSED 2026-05-05

See `docs/FAJARQUANT_FJ_PORT_PHASE_0_FINDINGS.md`.

Key findings:
- ✅ Linalg "gap" was illusory — `adaptive.rs` uses power iteration, no LAPACK
- ✅ fj-lang stdlib has 62 tensor builtins covering all FajarQuant ops
- ✅ Iterators/closures working in fj-lang (multiple production examples)
- 🟡 Single minor gap: `tensor_init_with(shape, closure)` helper missing
- 🟡 Residual risk: fj-lang LLVM codegen bugs may surface (~1-2 per major migration based on FAJAROS_100PCT pattern)

Compresses Phase 1 from 1-3d to ~0.5d. Total plan revised DOWN -15-30%.

## Phase 1 — fj-lang stdlib gap (~0.5 day, +25%)

| Task | Verification |
|---|---|
| 1.A Implement `tensor_init_with(shape: [i64], f: closure) -> Tensor` — equivalent to ndarray `Array1::from_shape_fn(d, |i| ...)`. Pure-fj helper using `tensor_zeros` + loop + `tensor_set`. | `examples/tensor_init_with_demo.fj` runs, output matches expected sequence |
| 1.B Verify existing 62 tensor builtins via FajarQuant-shaped test program — sanity that `tensor_dot`, `tensor_matmul`, `tensor_transpose`, `tensor_var_axis` accept the dim sizes FajarQuant uses (head_dim 64-128, seq_len 1024+). | new `tests/fajarquant_smoke.rs` exercises each on representative inputs |
| 1.C Phase 1 findings doc | committed |

**Prevention layer**: each tensor op exercised becomes a fj-lang regression test.

## Phase 2 — Port simple modules (~1-2 days, +25%)

Total ~660 LOC, no external linalg, mostly closed-form math.

| Task | Source | Target | Bit-equivalent gate |
|---|---|---|---|
| 2.A `lib.rs` (84 LOC) — docs + module exports | `src/lib.rs` | `src/runtime/ml/fajarquant/mod.fj` | re-export check via `fj check` |
| 2.B `hierarchical.rs` (401 LOC) — exponential bit decay schedule, no tensor ops | `src/hierarchical.rs` | new `hierarchical.fj` | matches Rust on 50 token-position inputs within 1e-9 |
| 2.C `cpu_kernels/scalar_baseline.rs` (263 LOC) — scalar ref impl | `src/cpu_kernels/scalar_baseline.rs` | new `scalar_baseline.fj` | matches Rust on 100 random vector inputs within 1e-6 |
| 2.D Phase 2 findings doc | committed |

**Prevention layer**: each port commit includes cross-validation test
(fj-lang output vs Rust output on identical seed). Tolerance bands:
- f64 closed-form: 1e-9
- f32: 1e-5
- Stochastic (LCG seeds): exact match

## Phase 3 — Port mid-complexity modules (~3-5 days, +30%)

Total ~1,355 LOC. Tensor stat ops + grouping + codebook math.

| Task | Source | LOC | Bit-equivalent gate |
|---|---|---|---|
| 3.A `fused_attention.rs` — quantized KV cache + codebook attention dot product | 320 | matches Rust on 100 random KV+Q vector pairs within 1e-5 |
| 3.B `turboquant.rs` — Lloyd-Max codebook + grouping quant + beta distribution sampling | 535 | matches Rust on standard 768-d test set; codebooks bit-identical (deterministic LCG seed) |
| 3.C `kivi.rs` — KIVI baseline (per-head per-channel quant) | 500 | matches Rust on KIVI canonical inputs |
| 3.D Phase 3 findings doc | committed |

**Risks**:
- Floating-point order-sensitive accumulation (Lloyd-Max iterations).
  Mitigation: validate with looser tolerance for accumulated quantities;
  bit-exact for non-accumulating outputs (codebook centroids).
- LCG seeded random: must produce identical sequences. Mitigation: port
  `lcg_next_f64` first as standalone test before module body.

## Phase 4 — Port complex module (~1.5-2.5 days, +35%)

| Task | Source | LOC | Bit-equivalent gate |
|---|---|---|---|
| 4.A `adaptive.rs` — PCA rotation via power iteration, calibration buffer, per-(layer,head) rotation map | 518 | PCA eigenvectors match Rust within 1e-3 (relax due to power iteration sign+order ambiguity); FINAL adaptive-quant output PPL within 0.1% of Rust on Gemma 4 E2B |
| 4.B Phase 4 findings doc | committed |

**Risk**: PCA solutions are non-unique (sign ambiguity, eigenvalue order
in degenerate cases). Validate by FINAL OUTPUT (per-token quant error)
not by intermediate eigenvectors.

## Phase 5 — End-to-end validation (~1-2 days, +25%)

| Task | Verification |
|---|---|
| 5.A Reproduce Gemma 4 E2B 50-prompt benchmark with fj-lang FajarQuant | `make test-fajarquant-fj-gemma-50` produces PPL within 1% of Rust baseline (80.14 / 75.65 / 157.01 at 2/3/4-bit) |
| 5.B Cross-validation script | `python scripts/verify_fj_vs_rust_outputs.py` runs both, asserts MAE < 1e-4 across 1000 random inputs |
| 5.C Performance — fj-lang version within 2× of Rust runtime (acceptable) | `make bench-fajarquant-fj` outputs comparison table |
| 5.D Phase 5 findings doc | committed |

**Decision gate**: 5.A PASS = port acceptable. FAIL = revert to Rust crate; document gap as known fj-lang issue + filing for later.

## Phase 6 — Integration with fajar-lang (~1 day, +25%)

| Task | Verification |
|---|---|
| 6.A `src/runtime/ml/fajarquant/mod.rs` shim now re-exports fj-lang FajarQuant builtins instead of Rust crate APIs | `cargo build --release --features llvm,native` clean; existing `fajarquant_*` integration tests pass |
| 6.B Update 16 existing integration tests | `cargo test --features llvm,native fajarquant_` 16/16 PASS |
| 6.C Cargo dep on Rust `fajarquant` crate flagged optional via feature `fajarquant_rust_compat` (default OFF) | `cargo features` reflects |
| 6.D Phase 6 findings doc | committed |

## Phase 7 — Documentation + release (~0.5 day, +25%)

| Task | Verification |
|---|---|
| 7.A README/CLAUDE.md updated: "FajarQuant algorithm: 100% Fajar Lang as of YYYY-MM-DD" | grep verifies |
| 7.B Sister Rust crate `fajarquant 0.5.0` republished with note "auto-generated bridge for crates.io interop; canonical source is .fj" | `cargo publish --dry-run` |
| 7.C fajar-lang v33.3.0 tagged + GitHub Release with binaries | `gh release view v33.3.0` shows new tag + 5 platform binaries |
| 7.D fajarquant repo CHANGELOG entry | committed |

## Effort Total

| Phase | Optimistic | Realistic +25-35% |
|---|---|---|
| 0 ✅ | DONE 30min | (Phase 0 doc shipped) |
| 1 | 0.5d | 0.6d |
| 2 | 1-2d | 1.3-2.5d |
| 3 | 3-5d | 3.9-6.5d |
| 4 | 1.5-2.5d | 2-3.4d |
| 5 | 1-2d | 1.3-2.5d |
| 6 | 1d | 1.3d |
| 7 | 0.5d | 0.6d |
| **Total** | **8.5-13.5d** | **10.5-17d** |

Per session ~2-4 jam Claude work, calendar **~3-5 minggu** part-time
sustained, atau lebih lama if interleaved.

## Risk Register

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | fj-lang LLVM codegen bug surfaces during port | MED | 0.5-1d/gap | Use Cranelift JIT during dev; surface bugs incrementally |
| R2 | Floating-point order drift breaks bit-equivalent gate | MED | 0.5d retest | Tolerance bands per phase; validate by final output not intermediate |
| R3 | Power iteration eigenvector sign ambiguity | LOW | 0.2d | Test by FINAL quant error, not eigenvector matrix |
| R4 | LCG seed reproducibility fails | LOW | 0.3d | Port lcg_next_f64 standalone first; verify byte-exact sequence |
| R5 | `tensor_init_with` helper has perf regression | LOW | 0.2d | Phase 1.A bench on representative shape sizes |

## Anti-Recommendations (locked in from Phase 0)

- ❌ JANGAN port `cpu_kernels/tl2.rs` (1067 LOC FFI to vendored C++) — no upside
- ❌ JANGAN port `cpu_kernels/tl2_encoder.rs` (435 LOC vendor-related encoder)
- ❌ JANGAN port Rust unit tests verbatim — write fj-lang tests asserting same INPUTS → same OUTPUTS as Rust
- ❌ JANGAN deprecate Rust crate — keep for `crates.io` ecosystem; auto-gen bridge from .fj source long-term
- ❌ JANGAN port Python training (out of scope; PyTorch is the right tool)

## Decision points (resolved post-Phase-0)

1. ✅ **Scope**: 7 pure-Rust modules (2,649 LOC); skip TL2 (1,502 LOC).
2. ✅ **Target location**: **Option C** — `fajar-lang/src/runtime/ml/fajarquant/*.fj`. Reason: stdlib integration → `use fajarquant` from any `.fj` program.
3. ✅ **Linalg mitigation**: NONE NEEDED — power iteration in source.
4. ✅ **Versioning**: fj-lang **v33.3.0** (minor; adds stdlib module).
5. ⏳ **Timing**: founder approval to proceed; can interleave with paper, FajarQuant Phase E5, etc.

## Reference

- Phase 0 findings: `docs/FAJARQUANT_FJ_PORT_PHASE_0_FINDINGS.md`
- FAJAROS_100PCT_FJ_PLAN (sister plan, completed): `docs/FAJAROS_100PCT_FJ_PLAN.md` + per-phase findings
- Lessons learned (compiler gap pattern): `docs/COMPILER_GAPS_LESSONS_LEARNED.md`
- Source: fajarquant repo `~/Documents/fajarquant` v0.4.0
- 16 integration tests anchor: `tests/fajarquant_*.rs` in fajar-lang

---

*FAJARQUANT_RUST_TO_FJ_PLAN — created 2026-05-05 after Phase 0 audit
demolished initial linalg-gap concern. Plan calibrated DOWN from
12-24 days estimate to 10.5-17 days realistic. Ready to start Phase 1
on founder approval.*
