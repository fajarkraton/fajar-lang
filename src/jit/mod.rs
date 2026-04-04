//! Tiered JIT — execution counters, baseline JIT, optimizing JIT,
//! on-stack replacement, profile-guided tier promotion.

pub mod baseline;
pub mod counters;
pub mod optimizing;
pub mod osr;
pub mod runtime;
