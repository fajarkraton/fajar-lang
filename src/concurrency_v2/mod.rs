//! Structured Concurrency — async scopes, cancellation, actors, STM.
//!
//! Provides structured concurrency primitives that guarantee no leaked tasks,
//! no orphan goroutines, and well-defined lifetimes for all concurrent work.

pub mod actors;
pub mod cancellation;
pub mod scopes;
pub mod stm;
