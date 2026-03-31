//! Formal Verification v2 — specification language, SMT solver, tensor shape proofs.
//!
//! Enables safety-critical certification (DO-178C, ISO 26262) for embedded ML.
//!
//! ## Sprint V1-V5: SMT Formal Verification (Option F)
//!
//! - `symbolic` — Symbolic execution engine (V1)
//! - `properties` — Property specification language (V2)
//! - `kernel_proofs` — @kernel safety proofs (V3)
//! - `device_proofs` — @device safety proofs (V4)
//! - `proof_cache` — Proof caching & incrementality (V5)

pub mod benchmarks;
pub mod certification;
pub mod device_proofs;
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
