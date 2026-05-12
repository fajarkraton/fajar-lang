//! Formal Verification v2 — specification language, SMT solver, tensor shape proofs.
//!
//! Enables safety-critical certification (DO-178C, ISO 26262) for embedded ML.
//!
//! ## Production surface (post-2026-05-12 SMT-freeze)
//!
//! - `symbolic` — Symbolic execution engine (V1, production CLI)
//! - `spec` — Verification-condition spec language (production CLI)
//! - `tensor_verify` — Tensor shape verification (production analyzer)
//!
//! ## Test-only surface (pending Path C decision)
//!
//! - `certification` — DO-178C/ISO-26262 evidence generation (tests only)
//! - `pipeline` — Verification pipeline glue (tests only)
//! - `smt` — SMT solver bridge (tests only)
//!
//! ## 2026-05-12 — Path A + Path B SMT-freeze (Compass §5.1)
//!
//! Under Compass §5.1 "SMT verification (DO-178C) → Bekukan. Butuh tim
//! untuk certification serius.", the following Sprint V1-V5 modules were
//! removed as dead research surface (zero consumers anywhere in src/,
//! tests/, examples/, benches/, stdlib/):
//!
//! Path A (commit `a2649182`):
//! - `device_proofs` (V4, 1,121 LOC, 24 self-tests) — "All simulated, no real Z3"
//!
//! Path B (this commit):
//! - `kernel_proofs` (V3, 987 LOC, 24 self-tests) — `@kernel` safety proofs
//! - `properties` (V2, 1,194 LOC, 25 self-tests) — property language
//! - `proof_cache` (V5, 1,239 LOC, 22 self-tests) — proof caching
//! - `inference` (1,401 LOC, 36 self-tests)
//! - `theories` (1,170 LOC, 30 self-tests) — SMT theories
//! - `benchmarks` (1,125 LOC, 31 self-tests) — verify benchmarks
//!
//! See `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` (systemic finding §5) +
//! `docs/decisions/2026-05-12-{device-proofs,verify-path-b}-deletion.md`.
//!
//! Path C (deferred) would remove the 3 test-only modules above plus
//! their test consumers in `eval/mod.rs`, `tests/feature_flag_tests.rs`,
//! `tests/nova_v2_tests.rs`.

pub mod certification;
pub mod pipeline;
pub mod smt;
pub mod spec;
pub mod symbolic;
pub mod tensor_verify;
