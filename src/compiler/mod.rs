//! # Compiler Infrastructure
//!
//! Provides incremental compilation, dependency tracking, artifact caching,
//! compilation benchmarking, edition management, and API stability tooling
//! for the Fajar Lang compiler pipeline.

pub mod benchmark;
pub mod comptime;
pub mod edition;
pub mod incremental;
pub mod security;
