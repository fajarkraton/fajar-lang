//! Online playground — compile and run Fajar Lang in the browser via Wasm.
//!
//! Provides the Wasm-bindgen entry points for the browser playground,
//! memory sandboxing, execution timeouts, and print capture.

pub mod examples;
pub mod sandbox;
pub mod share;
pub mod ui;
