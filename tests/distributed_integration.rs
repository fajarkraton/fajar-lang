//! Phase F.6 — integration smoke tests against the extracted
//! `fajarkraton/fajar-distributed` crate.
//!
//! `cmd_run_cluster` was removed at Phase F.4 (Compass §5.1 Option α),
//! so fajar-lang has no production consumer of this crate. These
//! tests are purely smoke: they verify the git-dep wires up and that
//! the historically-tested core surface (Raft consensus types,
//! cluster membership, service discovery) still constructs and
//! satisfies the basic invariants the deleted N3 sprint tests
//! exercised.
//!
//! Mirrors `wasi_p2_integration.rs` shape (6 tests). If a future
//! consumer is added in fajar-lang, expand here with round-trip
//! tests for that path.

use fajar_distributed::cluster::{ClusterNodeId, FailureDetector};
use fajar_distributed::discovery::{DiscoveryRegistry, ServiceInstance};
use fajar_distributed::raft::{RaftNode, RaftNodeId, RaftRole};
use std::collections::HashMap;
use std::time::Duration;

/// F.6.1 — `RaftNode::new` produces correct cluster_size and quorum
/// for a 4-node cluster (1 self + 3 peers). Reproduces the deleted
/// `n3_1_raft_node_creation` lib test as an integ-level smoke.
#[test]
fn f6_1_raft_node_creation() {
    let node = RaftNode::new(
        RaftNodeId(1),
        vec![RaftNodeId(2), RaftNodeId(3), RaftNodeId(4)],
    );
    assert_eq!(node.cluster_size(), 4, "self + 3 peers");
    assert_eq!(node.quorum(), 3, "majority of 4 = 3");
}

/// F.6.2 — Fresh RaftNode reports zero log index/term. Reproduces
/// `n3_2_raft_log_index`.
#[test]
fn f6_2_raft_log_index_starts_zero() {
    let node = RaftNode::new(RaftNodeId(1), vec![RaftNodeId(2), RaftNodeId(3)]);
    assert_eq!(node.last_log_index(), 0);
    assert_eq!(node.last_log_term(), 0);
}

/// F.6.3 — `DiscoveryRegistry::register` + `total_count`. Reproduces
/// `n3_3_discovery_registry`.
#[test]
fn f6_3_discovery_registry_register() {
    let mut reg = DiscoveryRegistry::new();
    reg.register(ServiceInstance {
        service_name: "scheduler".into(),
        instance_id: "sched-1".into(),
        address: "10.0.0.1:8080".into(),
        healthy: true,
        tags: HashMap::new(),
    });
    assert_eq!(reg.total_count(), 1);
}

/// F.6.4 — `resolve` returns only healthy instances. Spans the
/// register → mark_unhealthy → resolve invariant the deleted
/// `n3_4_discovery_resolve` / `n3_5_discovery_unhealthy` tests
/// covered together.
#[test]
fn f6_4_discovery_resolve_filters_unhealthy() {
    let mut reg = DiscoveryRegistry::new();
    reg.register(ServiceInstance {
        service_name: "api".into(),
        instance_id: "api-healthy".into(),
        address: "10.0.0.1:80".into(),
        healthy: true,
        tags: HashMap::new(),
    });
    reg.register(ServiceInstance {
        service_name: "api".into(),
        instance_id: "api-unhealthy".into(),
        address: "10.0.0.2:80".into(),
        healthy: false,
        tags: HashMap::new(),
    });
    let healthy = reg.resolve("api");
    assert_eq!(healthy.len(), 1, "only healthy instances resolved");
    assert_eq!(healthy[0].instance_id, "api-healthy");
}

/// F.6.5 — `FailureDetector::new` constructs and accepts a heartbeat
/// timeout. Reproduces the cluster-membership constructor that
/// `n3_7_failure_detector` exercised (full liveness semantics covered
/// by the extracted crate's own lib tests; this is a smoke that the
/// type still imports + constructs from the integ-test boundary).
#[test]
fn f6_5_failure_detector_constructs() {
    let _detector = FailureDetector::new(Duration::from_millis(500));
    // Construction-only smoke; the deleted N3 tests exercised richer
    // behavior already covered by the upstream crate's lib tests.
}

/// F.6.6 — `ClusterNodeId` + `RaftRole` enum variants are
/// constructible (smoke for cross-module enum re-export).
#[test]
fn f6_6_id_and_role_enum_constructible() {
    let _id = ClusterNodeId(42);
    let role = RaftRole::Follower;
    assert!(matches!(role, RaftRole::Follower));
    let _candidate = RaftRole::Candidate;
    let _leader = RaftRole::Leader;
}
