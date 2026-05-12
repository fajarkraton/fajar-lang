//! Formal Verification v2 — specification language, SMT solver, tensor shape proofs.
//!
//! Enables safety-critical certification (DO-178C, ISO 26262) for embedded ML.
//!
//! ## Sprint V1-V5: SMT Formal Verification (Option F)
//!
//! - `symbolic` — Symbolic execution engine (V1)
//! - `properties` — Property specification language (V2)
//! - `kernel_proofs` — @kernel safety proofs (V3)
//! - `proof_cache` — Proof caching & incrementality (V5)
//!
//! ## 2026-05-12 — Path A SMT-freeze (Compass §5.1)
//!
//! `device_proofs` (V4, `@device` safety proofs) removed under Compass §5.1
//! "SMT verification (DO-178C) → Bekukan. Butuh tim untuk certification
//! serius." The module was 1,121 LOC of simulated proofs (header
//! self-admitted "All simulated, no real Z3") with zero consumers anywhere
//! in src/, tests/, examples/, benches/, stdlib/. See
//! `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md` +
//! `docs/decisions/2026-05-12-device-proofs-deletion.md`. The 6 sibling
//! zero-consumer modules (kernel_proofs/proof_cache/properties/inference/
//! theories/benchmarks) remain pending Fajar's later Path B decision.

pub mod benchmarks;
pub mod certification;
pub mod inference;
pub mod kernel_proofs;
pub mod pipeline;
pub mod proof_cache;
pub mod properties;
pub mod smt;
pub mod spec;
pub mod symbolic;
pub mod tensor_verify;
pub mod theories;
