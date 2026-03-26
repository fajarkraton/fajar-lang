//! Distributed Computing — RPC framework, distributed tensors,
//! cluster scheduling, and fault tolerance.
//!
//! Provides built-in primitives for multi-node ML training and
//! microservices without external frameworks.

pub mod cluster;
pub mod fault_tolerance;
pub mod rpc;
pub mod tensors;
pub mod transport;
