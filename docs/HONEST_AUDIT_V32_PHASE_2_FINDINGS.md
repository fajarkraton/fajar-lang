---
phase: HONEST_AUDIT_V32 Phase 2 — mechanical verification
status: CLOSED 2026-05-02
budget: ~1.5h actual (est 3-4h, -50%)
---

# Phase 2 Findings — Mechanical Verification

## Summary

**Phase 2 PASS with NUMERICAL DRIFT.** All §3 quality gates green
(0 failures, 0 flakes, 0 clippy warnings, 0 fmt drift, 0 unwrap, 0
doc warnings). But several numerical claims in CLAUDE.md §3 have
drifted vs hand-verified actuals. Code is fine; documentation lags.

## Quality gates (all PASS ✓)

| Gate | Cmd | Result |
|---|---|---|
| Lib tests | `cargo test --lib --release` | **7626 passed, 0 failed, 0 ignored, 0.77s** |
| Stress test 5x | `cargo test --lib --release -- --test-threads=64` | 5/5 PASS, 7626/7626 each, max 1.99s, no flakes |
| Doc tests | `cargo test --doc --release` | 14 passed, 0 failed, 1 ignored, 0.72s |
| Integ tests | `cargo test --test '*' --release` | **2498 passed, 0 failed, 0 ignored** across 55 test files |
| Clippy | `cargo clippy --lib --release -- -D warnings` | EXIT=0 (0 warnings) |
| Fmt | `cargo fmt -- --check` | EXIT=0 (0 drift) |
| Unwrap audit | `python3 scripts/audit_unwrap.py` | 0 production unwraps (header-only output) |
| Doc warnings | `cargo doc --no-deps --lib` | 0 warnings, 0 errors |
| Version sync | `bash scripts/check_version_sync.sh` | PASS (Cargo 31.0.0 ↔ CLAUDE.md V31.4 same major) |
| V27.5 regression | `cargo test --test v27_5_compiler_prep --release` | 16 passed, 0 failed |
| Release build | `cargo build --release --bin fj` | clean, 18M binary |

**Total tests verified live:** 7626 lib + 2498 integ + 14 doc = **10,138 tests, 0 failures, 0 flakes** (matches PRODUCTION_AUDIT_V1's 10,138 claim).

## Numerical drift vs CLAUDE.md §3

| Claim (CLAUDE.md) | Actual | Δ | Status |
|---|---|---|---|
| ~7,611 lib tests | **7,626** | +15 (+0.2%) | ✓ within tolerance |
| 2,553 integ tests in 52 files | **2,498 in 55 files** | -55 tests, +3 files | ✗ drift, code-true |
| 14 doc + 1 ignored | 14 + 1 ignored | 0 | ✓ EXACT |
| ≈10,179 total | 10,138 | -41 (-0.4%) | ✓ within tolerance |
| ~448,000 LOC Rust | 449,280 | +0.3% | ✓ EXACT-ish |
| 394 src/ files | 391 | -3 (-0.8%) | ✓ within tolerance |
| 42 lib.rs pub mods | 42 | 0 | ✓ EXACT |
| 238 .fj examples | **243** | +5 (+2.1%) | ✗ drift, undercount |
| Binary 14 MB release | **18 MB** | +4 MB (+29%) | ✗ **significant drift** |
| 23 CLI subcommands | **39** | +16 (+70%) | ✗ **significant drift** |

**Three drift items beyond ±5% tolerance:**

1. **Binary size 14 → 18 MB (+29%).** Likely caused by V27 LLVM 30
   enhancements + V27.5 additions + V31 features ballooning. Either
   CLAUDE.md needs update OR optimization opportunity (audit deferred).

2. **CLI subcommands 23 → 39 (+70%).** PRODUCTION_AUDIT_V1 (2026-04-30)
   already noted this drift independently. CLAUDE.md §3 line "23
   subcommands declared in src/main.rs" is wrong — actual is 39.

3. **Examples 238 → 243 (+5).** Minor. PRODUCTION_AUDIT_V1 said 243,
   matching actual.

These are CLAUDE.md-side documentation drifts, not production bugs.
**Phase 6 will sync CLAUDE.md §3 numbers** at audit closeout.

## Spot-check on CHANGELOG numerical claims

| CHANGELOG 31.0.0 claim | Actual | Status |
|---|---|---|
| "16 E2E integration tests" in `tests/v27_5_compiler_prep.rs` | 16 #[test] functions, all PASS | ✓ EXACT |
| "12 untested feature flag tests" in `tests/feature_flag_tests.rs` | **22 #[test] functions** (all `#[cfg(feature)]`-gated, all PASS under `--all-features`) | ✗ undercount (CHANGELOG says 12) |
| "0 production .unwrap()" | 0 (header-only audit output) | ✓ |
| "0 clippy warnings" | 0 (EXIT=0) | ✓ |
| "0 fmt drift" | 0 (EXIT=0) | ✓ |
| "0 doc warnings" | 0 | ✓ |
| "Modules: 54 [x] / 0 [f] / 0 [s]" | (Phase 3 will hand-verify per-module) | pending |
| "Cargo.toml: 31.0.0" | 31.0.0 | ✓ EXACT |
| "CLAUDE.md banner: 31.0+V31.C.TRACKB" | "31.4 (V32-prep F.13 closure cycle + F.11 demotion)" | ✗ outdated; banner has advanced past 31.0+V31.C.TRACKB |

## Stress test detail

```
run 1: 7626 passed, 1.99s (slow first run, OK)
run 2: 7626 passed, 0.70s
run 3: 7626 passed, 0.76s
run 4: 7626 passed, 0.73s
run 5: 7626 passed, 0.71s
```

Stable under `--test-threads=64`. No flakes. §6.7 wall-clock-assertion
hygiene rule is holding.

## Phase 2 conclusion

- **All quality gates: PASS** ✓
- **3 numerical drifts beyond ±5% tolerance** (binary, CLI, examples)
  → CLAUDE.md §3 sync needed in Phase 6
- **Module count claim (54 [x])**: hand-verification deferred to Phase 3
- **CHANGELOG 31.0.0 has minor undercount drift** (feature_flag_tests
  count, banner-version freshness)

No production code changes needed at this phase. Onward to Phase 3
(per-module callable-surface audit).

---

*Phase 2 closed 2026-05-02. Mechanical scoreboard captured.*
