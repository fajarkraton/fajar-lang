# Decision: Delete certification + pipeline + smt — Path C SMT-freeze (full closure)

**Date:** 2026-05-12 (EOS-37 execution, immediate follow-up to Path B `6ab84dc9`)
**Decider:** Fajar Putranto — "Lanjutkan"
**Status:** APPROVED — SHIP IMMEDIATELY
**B0 audit:** `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md` (commit `8fada921`)
**Path:** C-targeted (single atomic commit per B0 §7 recommendation)

## Context

Path A (`a2649182`) deleted `device_proofs` (1 module, 1,121 LOC, 24 self-tests).
Path B (`6ab84dc9`) deleted 6 zero-consumer modules (7,116 LOC, 168 self-tests).
Path C closes the remaining 3 test-only modules (certification + pipeline +
smt) at full scope: files + test consumers + Cargo.toml smt feature.

Per Compass §5.1: "SMT verification (DO-178C) → **Bekukan**. Butuh tim
untuk certification serius."

## Decision

Delete all 3 test-only modules atomically + their test consumers + Cargo.toml smt feature.

## Scope (concrete)

### Module deletions
| File | LOC | Self-tests |
|---|---|---|
| `src/verify/certification.rs` | 1,316 | 28 |
| `src/verify/pipeline.rs` | 1,196 | 32 |
| `src/verify/smt.rs` | 1,254 | 30 |
| **Total** | **3,766** | **90** |

### Test consumer deletions
| Consumer | Tests removed | Why |
|---|---|---|
| `src/interpreter/eval/mod.rs` n1_1..n1_6 | 6 | All use `verify::smt::*` |
| `src/interpreter/eval/mod.rs` n1_9, n1_10 | 2 | n1_9 uses smt::{ProofCache,SmtResult}; n1_10 uses certification |
| `src/interpreter/eval/mod.rs` n4_9, n4_10 | 2 | smt + certification |
| `src/interpreter/eval/mod.rs` n10_1, n10_3 | 2 | certification + smt |
| `tests/feature_flag_tests.rs` `mod feature_smt` | 3 (cfg-gated) | Wildcard import of `verify::smt::*` |
| `tests/nova_v2_tests.rs` v14_n1_10/v14_n6_1/v14_n13_1 | 3 | All use `verify::pipeline::VerificationPipeline` |
| **Total** | **18 tests** | |

### KEPT (production-live):
- `n1_7_symbolic_engine_creation`, `n1_8_symbolic_engine_property_check` — use `verify::symbolic` (LIVE)
- Other 130+ tests in nova_v2_tests.rs (unrelated)
- 19 other tests in feature_flag_tests.rs (unrelated)
- All n4_1..n4_8, n10_2, n10_4..n10_10, and other sprint tests (unrelated)

### Cargo.toml
Remove `smt = ["dep:z3"]` from `[features]`. The `z3` dep itself is
`[dependencies.z3]` optional (per `dep:z3` syntax) — it will be cleaned
from Cargo.lock on next build.

### `src/verify/mod.rs`
Rewrite header to document Path A+B+C complete closure. Drop 3 more `pub mod`
declarations, leaving only `spec` + `symbolic` + `tensor_verify`.

## Aggregate impact
- **LOC delta:** ~-3,977 to -4,037 (per B0 §4 estimate)
- **Test delta:** -90 (file self-tests) − 12 (eval/mod.rs) − 3 (feature_flag, cfg-gated) − 3 (nova_v2) = **-108** total
- **Predicted lib count:** 7,310 → **7,208**
- **Stage 2 byte-equality:** NONE (verified per B0 §5)

## Engineering gates

```bash
cargo build --lib                                  # expect clean
cargo test --lib 2>&1 | tail -3                    # expect ~7,208 passed
cargo clippy --lib -- -D warnings                  # expect 0 warnings
cargo fmt -- --check                               # expect clean
cargo test --test context_safety_tests             # expect 149 passed
cargo test --test feature_flag_tests               # cfg-gated smt section deleted
cargo test --test nova_v2_tests                    # 3 fewer tests
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                                   # expect 4 passed (byte-equality)
```

## Re-entry conditions

Same as Path A/B. Reverse only if Compass §5.1 SMT-verification verdict
explicitly overturned + certification team commitment + Plan Hygiene §6.8
B0/plan/phased ship. Files recoverable via:
```
git log --diff-filter=D --follow -- src/verify/certification.rs
git log --diff-filter=D --follow -- src/verify/pipeline.rs
git log --diff-filter=D --follow -- src/verify/smt.rs
```

## Cumulative Compass §5.1 SMT-freeze (Path A + B + C)
- Modules deleted: 10 (device_proofs + 6 zero-consumer + 3 test-only)
- LOC reclaim: ~12,003
- Tests removed: ~280
- Production trio preserved: spec + symbolic + tensor_verify (2,623 LOC)

## Sign-off
- B0 audits: `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md`, predecessor systemic finding `bf65136b` §5
- Path A: `a2649182` + `2026-05-12-device-proofs-deletion.md`
- Path B: `6ab84dc9` + `2026-05-12-verify-path-b-deletion.md`
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 + Decision Framework
