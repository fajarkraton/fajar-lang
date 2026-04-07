//! FajarQuant — Adaptive vector quantization for embedded ML inference.
//!
//! Innovations over TurboQuant (Zandieh et al., 2025):
//! 1. **Adaptive rotation** — PCA-based per-head rotation instead of random
//! 2. **Fused quantized attention** — compute on quantized KV, skip dequantize
//! 3. **Hierarchical multi-resolution** — more bits for recent tokens

pub mod adaptive;
pub mod fused_attention;
pub mod hierarchical;
pub mod kivi;
