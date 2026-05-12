//! Formal Verification — production surface only (post-2026-05-12 SMT-freeze).
//!
//! Enables compile-time tensor-shape verification (via `tensor_verify`) and
//! symbolic execution of verification conditions (via `symbolic` + `spec`).
//!
//! ## Production modules
//!
//! - `spec` — Verification-condition spec language (consumed by main.rs CLI step 3)
//! - `symbolic` — Symbolic execution engine (consumed by main.rs CLI + analyzer)
//! - `tensor_verify` — Tensor shape verification (consumed by analyzer/type_check)
//!
//! ## 2026-05-12 — Path A + B + C SMT-freeze complete (Compass §5.1)
//!
//! Under Compass §5.1 "SMT verification (DO-178C) → Bekukan. Butuh tim
//! untuk certification serius.", the entire SMT-verification research
//! surface has been removed across three phases:
//!
//! - **Path A** (commit `a2649182`): `device_proofs` (V4) — 1,121 LOC, 24 self-tests.
//! - **Path B** (commit `6ab84dc9`): 6 zero-consumer modules — `kernel_proofs`
//!   (V3), `proof_cache` (V5), `properties` (V2), `inference`, `theories`,
//!   `benchmarks` — 7,116 LOC, 168 self-tests.
//! - **Path C** (this commit): 3 test-only modules — `certification`, `pipeline`,
//!   `smt` — 3,766 LOC + 90 self-tests + 18 test-consumer cleanups across
//!   eval/mod.rs (12), feature_flag_tests.rs (3 cfg-gated), nova_v2_tests.rs (3).
//!   Plus `smt = ["dep:z3"]` Cargo.toml feature flag removed.
//!
//! **Cumulative SMT-freeze reclaim: ~12,003 LOC across 10 modules + ~280 tests.**
//!
//! See B0 audits: `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` (systemic),
//! `docs/VERIFY_PATH_C_LOAD_BEARING_B0_FINDINGS.md` (Path C).
//! Decisions: `docs/decisions/2026-05-12-{device-proofs,verify-path-b,verify-path-c}-deletion.md`.
//!
//! Re-entry condition: Compass §5.1 SMT-verification verdict overturned by
//! Fajar in a follow-up decision file + certification team commitment +
//! Plan Hygiene §6.8 B0/plan/phased ship. Files recoverable via
//! `git log --diff-filter=D -- src/verify/<name>.rs`.

pub mod spec;
pub mod symbolic;
pub mod tensor_verify;
