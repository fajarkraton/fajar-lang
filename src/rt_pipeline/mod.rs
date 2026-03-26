//! Real-Time ML Pipeline — sensor → inference → actuator with latency guarantees.
//!
//! Core differentiator: compiler-enforced @kernel/@device/@safe context isolation
//! ensures sensor drivers, ML inference, and actuator control cannot interfere.

pub mod sensor;
pub mod inference;
pub mod actuator;
pub mod pipeline;
