//! FajarQuant — re-export shim.
//!
//! The FajarQuant algorithms (adaptive PCA rotation, fused quantized attention,
//! hierarchical multi-resolution, KIVI baseline) were extracted to the standalone
//! `fajarquant` crate during V26 Phase A4 (2026-04-11). This module remains as
//! a re-export shim so existing call sites in `src/interpreter/eval/builtins.rs`
//! continue to work without modification.
//!
//! For new code, prefer importing directly from `::fajarquant::*`.

pub use ::fajarquant::adaptive;
pub use ::fajarquant::fused_attention;
pub use ::fajarquant::hierarchical;
pub use ::fajarquant::kivi;
