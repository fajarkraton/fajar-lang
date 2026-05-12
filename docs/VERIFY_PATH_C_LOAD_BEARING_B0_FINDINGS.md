# verify/ Path C SMT-freeze (test-only surface) — B0 Findings

> **Phase:** Compass §5.1 SMT-verification full freeze closure. Path C
> follows Path A (`a2649182` — device_proofs.rs) + Path B (`6ab84dc9` —
> 6 zero-consumer modules).
> **Audit date:** 2026-05-12 EOS-37 (on `lanjutkan` first-step).
> **Plan Hygiene §6.8 R1:** Audit only. Decision pending Fajar.
> **Predecessor B0:** `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` §5
> (systemic finding) — Path C scope was deferred there.

## §1. Scope

Verify the 3 remaining `verify/` modules (`certification`, `pipeline`, `smt`)
are still test-only post-Path B + locate every test consumer to scope
the deletion precisely. The systemic B0 already established the verdict
("test-only" = no production consumer); this B0 turns that into a
mechanically-executable deletion plan.

## §2. Module inventory (HEAD `6ab84dc9`)

| Module | LOC | Self-tests | Sprint | Production status |
|---|---|---|---|---|
| `certification` | 1,316 | 28 | (DO-178C/ISO-26262 evidence) | Test-only |
| `pipeline` | 1,196 | 32 | (verification pipeline glue) | Test-only |
| `smt` | 1,254 | 30 | (SMT solver bridge) | Test-only |
| **Total** | **3,766** | **90** | | |

### 2.1 Independence verified
- No production consumers in `src/main.rs` (verified: zero matches).
- No cross-deps with the live trio (`spec`/`symbolic`/`tensor_verify`):
  grep `use crate::verify::{certification,pipeline,smt}` in the live
  trio → empty.
- No intra-deps among the 3 themselves (each module's `use super::*;`
  matches are inside its own `#[cfg(test)]` block — standard Rust
  pattern, not cross-module deps).

## §3. Test consumer trace (re-verified live)

### 3.1 `src/interpreter/eval/mod.rs` (12 tests, ~150-180 LOC)

The N1/N4/N10 sprint blocks have tests that import from `verify::smt`
or `verify::certification`. Precise consumer-test map:

| Test | Line | Import | Action |
|---|---|---|---|
| `n1_1_smt_prove_non_negative` | 7264 | `smt::prove_non_negative` | DELETE |
| `n1_2_smt_prove_array_bounds` | 7271 | `smt::prove_array_bounds` | DELETE |
| `n1_3_smt_prove_no_overflow` | 7278 | `smt::prove_no_i32_overflow` | DELETE |
| `n1_4_smt_prove_matmul_shapes` | 7285 | `smt::prove_matmul_shapes` | DELETE |
| `n1_5_smt_check_satisfiable` | 7292 | `smt::check_satisfiable` | DELETE |
| `n1_6_smt_prove_with_timeout` | 7303 | `smt::prove_with_timeout` | DELETE |
| **`n1_7_symbolic_engine_creation`** | 7310 | `symbolic::SymbolicEngine` | **KEEP** (LIVE module) |
| **`n1_8_symbolic_engine_property_check`** | 7319 | `symbolic::SymbolicEngine` | **KEEP** (LIVE module) |
| `n1_9_proof_cache` | 7329 | `smt::{ProofCache, SmtResult}` | DELETE |
| `n1_10_misra_compliance` | 7340 | `certification::check_misra_compliance` | DELETE |
| `n4_9_smt_kernel_safety` | 7690 | `smt::*` | DELETE |
| `n4_10_do178c_evidence` | 7698 | `certification::{DalLevel, generate_do178c_evidence}` | DELETE |
| `n10_1_iso26262_evidence` | 8242 | `certification::{AsilLevel, generate_iso26262_evidence}` | DELETE |
| `n10_3_proof_cache_hit_rate` | 8281 | `smt::{ProofCache, SmtResult}` | DELETE |

**Path C deletion target: 12 tests.** Non-contiguous — n1_7+n1_8 sit
between n1_6 and n1_9, so the n1 deletion must be split into two blocks
(n1_1..n1_6 + n1_9..n1_10). Similarly n4_9+n4_10 sit inside the wider
n4_1..n4_10 sprint block (n4_1..n4_8 are non-verify tests covering
workload/GPU/ffi — KEEP). And n10_1+n10_3 sit within n10_1..n10_10
sprint (others are spirv/registry/ffi/wit/etc — KEEP).

### 3.2 `tests/feature_flag_tests.rs` (1 `#[cfg(feature = "smt")]` mod, 3 tests)

Lines 134-163 (approx):
```rust
// ═══════════════════════════════════════════════════════════
// SMT (feature = "smt")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "smt")]
mod feature_smt {
    use fajar_lang::verify::smt::*;

    #[test] fn solver_config_defaults() { ... }
    #[test] fn smt_logic_display() { ... }
    #[test] fn solver_backend_display() { ... }
}
```

**Path C deletion target: 1 cfg-gated mod containing 3 tests.**

### 3.3 `tests/nova_v2_tests.rs` (3 individual tests)

Pipeline usage sites:

| Line | Test fn | Test fn start | Action |
|---|---|---|---|
| 140 | `pipeline::VerificationPipeline::new()` | `v14_n1_10_verify_module_exists_and_has_api` (L132) | DELETE TEST |
| 578 | `pipeline::VerificationPipeline::new()` | `v14_n6_1_verify_pipeline_api` (L577) | DELETE TEST |
| 1506 | `pipeline::VerificationPipeline::new()` | `v14_n13_1_verify_module_api` (L1505) | DELETE TEST |

3 individual `#[test]` fns. The remaining 135 tests in this file are
unrelated to pipeline and stay untouched.

### 3.4 Cargo.toml feature flag impact

`Cargo.toml`:
```toml
smt = ["dep:z3"]
```

The `smt` feature flag becomes useless once `verify::smt` is deleted. It
should be removed from the `[features]` table. Z3 dep also dropped from
`[dependencies]` (if it was only used by `verify::smt`).

```
$ grep -rln 'cfg(feature\s*=\s*"smt"' src/ tests/
src/verify/smt.rs
tests/feature_flag_tests.rs
```

Only 2 sites gated by `feature = "smt"`. Both deleted in Path C
(the file itself + the feature_flag_tests block). So removing the
Cargo.toml entry is safe and the right cleanup.

## §4. Aggregate impact (predicted)

| Surface | LOC delta | Test delta |
|---|---|---|
| 3 module files (certification + pipeline + smt) | -3,766 | -90 lib (self-tests) |
| `src/verify/mod.rs` header rewrite | small (~30 lines) | 0 |
| eval/mod.rs 12 tests removed (~12-15 LOC each + import) | ~-150 to -180 | -12 lib |
| tests/feature_flag_tests.rs SMT section | ~-30 | -3 integ (cfg-gated) |
| tests/nova_v2_tests.rs 3 tests | ~-30 to -60 | -3 integ |
| Cargo.toml `smt` feature removal | -1 (1 line) | 0 |
| **Total** | **~-3,977 to -4,037 LOC** | **-102 (90 lib + 12 lib + 3 integ + 3 integ; cfg-gated may not always run)** |

**Predicted lib count post-Path C:** 7,310 − 90 − 12 = **7,208**.

**Predicted integ count change:** -6 (3 from feature_flag, 3 from nova_v2).

## §5. Stage 2 byte-equality risk

NONE. All 3 modules are Rust-only. Verified live:
```
grep -r "VerificationPipeline\|ProofCache\|SmtResult\|SmtLogic\|\
SolverConfig\|SolverBackend\|check_misra_compliance\|generate_do178c_evidence\|\
generate_iso26262_evidence\|DalLevel\|AsilLevel" stdlib/
→ empty
```
Phase17 self-host gate unaffected (4/4 stays green).

## §6. CLI / user-facing surface impact

ZERO. None of the 3 modules has CLI exposure (`grep "certification\|pipeline\|\
verify::smt" src/main.rs` → empty for these specific paths).
`spec`/`symbolic`/`tensor_verify` (production-live) remain untouched.

## §7. Three execution paths

### Path C-narrow — files only
- Delete 3 files + mod.rs header rewrite.
- Test consumers in eval/mod.rs/feature_flag/nova_v2 → cause cargo test failures.
- **Wrong path** — leaves broken tests. Not viable.

### Path C-targeted — files + 12 eval/mod.rs tests + cfg-gated feature_smt mod + 3 nova_v2 tests + Cargo.toml smt feature
- Complete + atomic.
- **Effort:** ~1.5-2.5h (test boundary care, careful Edit calls).
- **Risk:** LOW-MED — multiple files, boundary detection in eval/mod.rs needs care.
- **Recommended.**

### Path C-deferred — files only now, tests later
- Same as C-narrow but with broken tests temporarily.
- **Wrong** — never ship broken main.

## §8. Recommendation: Path C-targeted (single atomic commit)

**Rationale:**
1. Mirrors Path A + Path B pattern: single atomic commit with all related changes.
2. Test boundaries are well-defined; non-contiguous deletions in eval/mod.rs handled via 5 Edit calls (n1_1..n1_6 + n1_9..n1_10 + n4_9..n4_10 + n10_1 + n10_3).
3. Feature flag removal in Cargo.toml is a 1-line edit + dropping z3 from `[dependencies]` if only used here.
4. Closes Compass §5.1 SMT-freeze at FULL scope (Path A + B + C cumulative reclaim ~-12,000 LOC across 10 modules).

## §9. Re-entry conditions

Same as Path A/B: Compass §5.1 SMT-verification verdict overturned by
Fajar in a follow-up decision file, + certification team commitment, +
Plan Hygiene §6.8 B0/plan/phased ship before code restoration.

Files recoverable via:
```
git log --diff-filter=D --follow -- src/verify/certification.rs
git log --diff-filter=D --follow -- src/verify/pipeline.rs
git log --diff-filter=D --follow -- src/verify/smt.rs
```

## §10. Verification commands (pre-flight + post-ship)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Pre-flight (this audit)
for m in certification pipeline smt; do
  grep -rln "use crate::verify::${m}\|verify::${m}::" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "src/verify/${m}.rs"
done

# Post-ship
cargo test --lib 2>&1 | tail -3           # expect ~7,208 passed
cargo clippy --lib -- -D warnings         # expect 0 warnings
cargo fmt -- --check                      # expect clean
cargo test --test context_safety_tests    # expect 149 passed
cargo test --test feature_flag_tests      # expect existing - 3 (or unchanged if smt cfg never ran)
cargo test --test nova_v2_tests           # expect existing - 3
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                          # expect 4 passed
```

## §11. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §10)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — applied at ship time via updated mod.rs header)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all consumer locations live-verified)
[ ] Effort variance tagged in commit message               (Rule 5 — at ship time)
[ ] Decisions are committed files                          (Rule 6 — pending Fajar's choice)
[x] Public-artifact drift swept                            (Rule 7 — done EOS-29 this session)
[x] Multi-repo state checked                               (Rule 8 — done EOS-29..36 this session)
```

## §12. Source artifacts

- This file: `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md`
- Systemic B0: `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` §5 (`bf65136b`)
- Path A: commit `a2649182` + `docs/decisions/2026-05-12-device-proofs-deletion.md`
- Path B: commit `6ab84dc9` + `docs/decisions/2026-05-12-verify-path-b-deletion.md`
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 + Decision Framework

---

*B0 written 2026-05-12 EOS-37. ~30min actual. Path C scope precisely
mapped: 3 module files + 12 eval/mod.rs tests + 3-test cfg-gated
feature_smt block in feature_flag_tests.rs + 3 nova_v2_tests.rs tests +
Cargo.toml smt feature removal. Predicted reclaim: ~-3,977 to -4,037 LOC,
-102 tests. Recommendation: Path C-targeted (single atomic commit).
Decision pending Fajar.*
