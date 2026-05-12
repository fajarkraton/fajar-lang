# Decision: Delete 6 zero-consumer verify/ modules (Path B SMT-freeze targeted)

**Date:** 2026-05-12 (EOS-36, immediate follow-up to Path A `a2649182`)
**Decider:** Fajar Putranto — "lanjutkan sesuai dengan rekomendasi"
  (B0 §7 recommended Path B as the sweet-spot continuation of Path A)
**Status:** APPROVED — SHIP IMMEDIATELY
**Audit:** `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` §5 (systemic
  finding, commit `bf65136b`) — re-verified live post-EOS-35.

## Context

Path A (commit `a2649182`) deleted `verify::device_proofs` (1,121 LOC, 24
self-tests) — the most clear-cut dead module. The systemic B0 §5 finding
documented 6 additional `verify/` sibling modules with identical zero-
consumer status, all part of the Sprint V1-V5 SMT-verification family
that Compass §5.1 explicitly froze.

Path B closes the rest of the zero-consumer surface (deferring the
test-only modules — certification, pipeline, smt — to a possible Path C).

## Re-verification (post-Path A, 2026-05-12 EOS-36)

```
$ for m in benchmarks inference kernel_proofs proof_cache properties theories; do
    grep -rln "use crate::verify::${m}\|verify::${m}::" \
        src/ tests/ examples/ benches/ stdlib/ | grep -v "src/verify/${m}.rs" | wc -l
  done
benchmarks: 0    inference: 0    kernel_proofs: 0
proof_cache: 0   properties: 0   theories: 0
```

All 6 modules **still zero-external-consumer** post-Path A. No drift.

Intra-verify/ cross-module deps: NONE. Each module's `use super::*;`
matches are inside its own `#[cfg(test)] mod tests` block (standard Rust
pattern pulling the module's own pub items into the test module).
Verified by reading each match site:

```
src/verify/proof_cache.rs:865:    use super::*;      ← inside #[cfg(test)]
src/verify/inference.rs:922:    use super::*;        ← inside #[cfg(test)]
src/verify/theories.rs:825:    use super::*;        ← inside #[cfg(test)]
src/verify/benchmarks.rs:720:    use super::*;     ← inside #[cfg(test)]
src/verify/properties.rs:850:    use super::*;      ← inside #[cfg(test)]
src/verify/kernel_proofs.rs:662:    use super::*;   ← inside #[cfg(test)]
```

Modules are fully independent. Deletion safe in any order.

## The Compass §5.1 verdict (same as Path A)

`docs/1/STRATEGIC_COMPASS.md` §5.1:
> | SMT verification (DO-178C) | Diklaim ada | **Bekukan**. Butuh tim untuk certification serius. |

Decision Framework:
> | Tambah formal proof / SMT verification? | ⏸️ Bekukan kecuali untuk niche safety-critical certification |

All 6 modules are Sprint V1-V5 SMT-verification family. The Compass verdict
applies uniformly. Same rationale as the Path A deletion of device_proofs.

## Decision

Delete all 6 zero-consumer verify/ modules in a single atomic commit + update
`verify/mod.rs` to drop their `pub mod` declarations and adjust the
doc-comment header.

## Scope (concrete, with self-test counts)

| File | LOC | Self-tests | Sprint |
|---|---|---|---|
| `src/verify/benchmarks.rs` | 1,125 | 31 | V? (perf benchmarks) |
| `src/verify/inference.rs` | 1,401 | 36 | V? (inference) |
| `src/verify/kernel_proofs.rs` | 987 | 24 | V3 (`@kernel` proofs) |
| `src/verify/proof_cache.rs` | 1,239 | 22 | V5 (proof caching) |
| `src/verify/properties.rs` | 1,194 | 25 | V2 (property language) |
| `src/verify/theories.rs` | 1,170 | 30 | (SMT theories) |
| **Total** | **7,116** | **168** | |

Plus: `verify/mod.rs` edit to:
- Remove 6 `pub mod` declarations
- Update doc-comment header to drop V2/V3/V5 bullets (V1 symbolic survives;
  V4 device_proofs already removed in Path A)
- Add Path B closure note referencing this decision

## Stage 2 byte-equality risk

NONE. All 6 modules are Rust-only. Verified via:
```
grep -r "ProofCache\|NatRange\|VerificationCondition\|SmtTheory\|\
KernelProof\|InferenceResult\|BenchmarkResult" stdlib/
→ empty
```

## Engineering gates (run pre-commit)

```bash
cargo build --lib                                    # expect clean
cargo test --lib 2>&1 | tail -3                      # expect 7,310 passed (7,478 - 168)
cargo clippy --lib -- -D warnings                    # expect 0 warnings
cargo fmt -- --check                                 # expect clean
cargo test --test context_safety_tests               # expect 149 passed
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                                     # expect 4 passed
```

Pre-push hook on push runs full self-host gate.

## Deferred (Path C)

3 test-only modules (certification, pipeline, smt) retain test consumers in:
- `src/interpreter/eval/mod.rs` (n1_*/n4_*/n10_* sprint blocks)
- `tests/feature_flag_tests.rs`
- `tests/nova_v2_tests.rs`

Path C requires careful scoped deletion of those test blocks too. Out
of scope for this commit. Decision pending Fajar's later call. If chosen:
- Path C adds ~+3,766 LOC reclaim → total Path A+B+C = ~-12,003 LOC across
  10 modules.
- Effort estimate: ~2-3h additional beyond Path B.

## Re-entry conditions

Reverse this decision if and only if:
1. Compass §5.1 SMT-verification verdict explicitly overturned by Fajar
   in a follow-up decision file, AND
2. A certification team commitment exists (per Compass rationale), AND
3. Plan Hygiene §6.8 B0/plan/phased ship before code restoration.

Files recoverable from git:
```
git log --diff-filter=D --follow -- src/verify/benchmarks.rs
git log --diff-filter=D --follow -- src/verify/inference.rs
git log --diff-filter=D --follow -- src/verify/kernel_proofs.rs
git log --diff-filter=D --follow -- src/verify/proof_cache.rs
git log --diff-filter=D --follow -- src/verify/properties.rs
git log --diff-filter=D --follow -- src/verify/theories.rs
```

## Sign-off

- Systemic B0: `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` §5 (`bf65136b`)
- Path A predecessor: `a2649182` (device_proofs.rs deletion)
- Companion decisions same session: `2026-05-12-{tensor-shapes,arrays-patterns,device-proofs}-deletion.md`
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 + Decision Framework
