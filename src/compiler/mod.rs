//! # Compiler Infrastructure
//!
//! Provides incremental compilation, dependency tracking, artifact caching,
//! compilation benchmarking, edition management, API stability tooling,
//! and release engineering for the Fajar Lang compiler pipeline.

pub mod benchmark;
pub mod edition;
pub mod incremental;
pub mod performance;
pub mod release;
pub mod security;

// Re-export real-time pipeline (sensor -> inference -> actuator).
pub use crate::rt_pipeline;

/// Returns the pipeline stage names for the RT sensor-inference-actuator flow.
pub fn rt_pipeline_stages() -> Vec<&'static str> {
    vec!["sensor", "inference", "actuator", "pipeline"]
}
