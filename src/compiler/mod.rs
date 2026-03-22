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
