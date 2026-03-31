//! Distributed Computing — RPC framework, distributed tensors,
//! cluster scheduling, and fault tolerance.
//!
//! Provides built-in primitives for multi-node ML training and
//! microservices without external frameworks.

pub mod cluster;
pub mod data_plane;
pub mod deploy;
pub mod discovery;
pub mod dist_bench;
pub mod fault_tolerance;
pub mod fault_tolerance_v2;
pub mod ml_training;
pub mod raft;
pub mod rpc;
pub mod rpc_v2;
pub mod scheduler;
pub mod security;
pub mod tensors;
pub mod transport;
