//! TurboQuant — re-export shim.
//!
//! The TurboQuant baseline algorithm was extracted to the standalone
//! `fajarquant` crate during V26 Phase A4 (2026-04-11). This file remains
//! as a re-export shim so existing call sites in `src/interpreter/eval/builtins.rs`
//! continue to work without modification.
//!
//! For new code, prefer importing directly from `::fajarquant::turboquant::*`.

pub use ::fajarquant::turboquant::*;
