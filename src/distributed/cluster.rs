//! Cluster Scheduling — node discovery, resource advertisement,
//! task placement, fault detection, work stealing, barriers, quotas.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S11.1: Node Discovery
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a cluster node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClusterNodeId(pub u64);

impl fmt::Display for ClusterNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClusterNode({})", self.0)
    }
}

/// Discovery method for finding peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Multicast discovery.
    Multicast { group: String, port: u16 },
    /// Seed node list.
    SeedNodes(Vec<String>),
    /// Static configuration.
    Static,
}

impl fmt::Display for DiscoveryMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryMethod::Multicast { group, port } => {
                write!(f, "Multicast({group}:{port})")
            }
            DiscoveryMethod::SeedNodes(seeds) => write!(f, "Seeds({})", seeds.len()),
            DiscoveryMethod::Static => write!(f, "Static"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.2: Resource Advertisement
// ═══════════════════════════════════════════════════════════════════════

/// Resources available on a node.
#[derive(Debug, Clone)]
pub struct NodeResources {
    /// Number of CPU cores.
    pub cpu_cores: u32,
    /// Number of GPUs.
    pub gpu_count: u32,
    /// Total memory in MB.
    pub memory_mb: u64,
    /// Network bandwidth in Mbps.
    pub network_mbps: u32,
}

/// A node in the cluster with its resources.
#[derive(Debug, Clone)]
pub struct ClusterNode {
    /// Node identifier.
    pub id: ClusterNodeId,
    /// Node address (host:port).
    pub address: String,
    /// Advertised resources.
    pub resources: NodeResources,
    /// Current status.
    pub status: NodeStatus,
    /// Rack/datacenter location.
    pub location: Option<String>,
}

/// Status of a cluster node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeStatus {
    /// Node is healthy and accepting work.
    Healthy,
    /// Node is suspected of failure (missed heartbeats).
    Suspected,
    /// Node is confirmed dead.
    Dead,
    /// Node is draining (not accepting new work).
    Draining,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Healthy => write!(f, "Healthy"),
            NodeStatus::Suspected => write!(f, "Suspected"),
            NodeStatus::Dead => write!(f, "Dead"),
            NodeStatus::Draining => write!(f, "Draining"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.3: Task Placement
// ═══════════════════════════════════════════════════════════════════════

/// Resource requirements for a task.
#[derive(Debug, Clone)]
pub struct TaskRequirements {
    /// Minimum CPU cores needed.
    pub cpu_cores: u32,
    /// Minimum GPU count needed.
    pub gpu_count: u32,
    /// Minimum memory in MB.
    pub memory_mb: u64,
}

/// Places a task on the best available node.
pub fn place_task(requirements: &TaskRequirements, nodes: &[ClusterNode]) -> Option<ClusterNodeId> {
    nodes
        .iter()
        .filter(|n| n.status == NodeStatus::Healthy)
        .filter(|n| {
            n.resources.cpu_cores >= requirements.cpu_cores
                && n.resources.gpu_count >= requirements.gpu_count
                && n.resources.memory_mb >= requirements.memory_mb
        })
        // Prefer node with most available resources
        .max_by_key(|n| n.resources.memory_mb + n.resources.cpu_cores as u64 * 1024)
        .map(|n| n.id)
}

// ═══════════════════════════════════════════════════════════════════════
// S11.4: Fault Detection
// ═══════════════════════════════════════════════════════════════════════

/// Heartbeat-based failure detector.
#[derive(Debug)]
pub struct FailureDetector {
    /// Heartbeat timeout.
    pub timeout: Duration,
    /// Last heartbeat times (node ID → milliseconds since start).
    pub last_heartbeat: HashMap<ClusterNodeId, u64>,
}

impl FailureDetector {
    /// Creates a new failure detector.
    pub fn new(timeout: Duration) -> Self {
        FailureDetector {
            timeout,
            last_heartbeat: HashMap::new(),
        }
    }

    /// Records a heartbeat from a node.
    pub fn heartbeat(&mut self, node: ClusterNodeId, now_ms: u64) {
        self.last_heartbeat.insert(node, now_ms);
    }

    /// Checks which nodes are suspected of failure.
    pub fn check(&self, now_ms: u64) -> Vec<ClusterNodeId> {
        let timeout_ms = self.timeout.as_millis() as u64;
        self.last_heartbeat
            .iter()
            .filter(|(_, &last)| now_ms - last > timeout_ms)
            .map(|(&id, _)| id)
            .collect()
    }

    /// Returns whether a specific node is healthy.
    pub fn is_healthy(&self, node: ClusterNodeId, now_ms: u64) -> bool {
        let timeout_ms = self.timeout.as_millis() as u64;
        self.last_heartbeat
            .get(&node)
            .is_some_and(|&last| now_ms - last <= timeout_ms)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.5: Task Migration
// ═══════════════════════════════════════════════════════════════════════

/// A checkpoint for task migration.
#[derive(Debug, Clone)]
pub struct TaskCheckpoint {
    /// Task identifier.
    pub task_id: u64,
    /// Serialized task state.
    pub state: Vec<u8>,
    /// Progress (0.0 - 1.0).
    pub progress: f64,
    /// Source node.
    pub source_node: ClusterNodeId,
}

/// Result of a task migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationResult {
    /// Migration successful.
    Success {
        from: ClusterNodeId,
        to: ClusterNodeId,
    },
    /// No suitable target node found.
    NoTarget,
    /// Migration failed.
    Failed(String),
}

impl fmt::Display for MigrationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationResult::Success { from, to } => {
                write!(f, "Migrated from {} to {}", from, to)
            }
            MigrationResult::NoTarget => write!(f, "No suitable target node"),
            MigrationResult::Failed(e) => write!(f, "Migration failed: {e}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.6: Work Stealing
// ═══════════════════════════════════════════════════════════════════════

/// Work stealing queue for a node.
#[derive(Debug, Clone)]
pub struct WorkQueue {
    /// Node that owns this queue.
    pub node: ClusterNodeId,
    /// Tasks waiting to be executed.
    pub tasks: Vec<u64>,
}

impl WorkQueue {
    /// Creates a new work queue.
    pub fn new(node: ClusterNodeId) -> Self {
        WorkQueue {
            node,
            tasks: Vec::new(),
        }
    }

    /// Adds a task to the queue.
    pub fn push(&mut self, task_id: u64) {
        self.tasks.push(task_id);
    }

    /// Takes a task from the front (owner).
    pub fn pop(&mut self) -> Option<u64> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks.remove(0))
        }
    }

    /// Steals a task from the back (thief).
    pub fn steal(&mut self) -> Option<u64> {
        self.tasks.pop()
    }

    /// Returns the queue length.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

/// Identifies which node to steal from (highest queue length).
pub fn find_steal_target(queues: &[WorkQueue], thief: ClusterNodeId) -> Option<ClusterNodeId> {
    queues
        .iter()
        .filter(|q| q.node != thief && !q.is_empty())
        .max_by_key(|q| q.len())
        .map(|q| q.node)
}

// ═══════════════════════════════════════════════════════════════════════
// S11.7: Barrier Synchronization
// ═══════════════════════════════════════════════════════════════════════

/// A barrier for synchronizing all cluster nodes.
#[derive(Debug)]
pub struct ClusterBarrier {
    /// Expected number of participants.
    pub expected: usize,
    /// Nodes that have arrived.
    pub arrived: Vec<ClusterNodeId>,
}

impl ClusterBarrier {
    /// Creates a new barrier for N participants.
    pub fn new(expected: usize) -> Self {
        ClusterBarrier {
            expected,
            arrived: Vec::new(),
        }
    }

    /// A node arrives at the barrier.
    pub fn arrive(&mut self, node: ClusterNodeId) {
        if !self.arrived.contains(&node) {
            self.arrived.push(node);
        }
    }

    /// Checks if all nodes have arrived.
    pub fn is_complete(&self) -> bool {
        self.arrived.len() >= self.expected
    }

    /// Resets the barrier for the next phase.
    pub fn reset(&mut self) {
        self.arrived.clear();
    }

    /// Returns the number of nodes still pending.
    pub fn pending(&self) -> usize {
        self.expected.saturating_sub(self.arrived.len())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.8: Cluster Topology
// ═══════════════════════════════════════════════════════════════════════

/// Topology-aware scheduling information.
#[derive(Debug, Clone)]
pub struct TopologyInfo {
    /// Rack ID for each node.
    pub rack: HashMap<ClusterNodeId, String>,
    /// Datacenter for each node.
    pub datacenter: HashMap<ClusterNodeId, String>,
}

impl TopologyInfo {
    /// Creates a new empty topology.
    pub fn new() -> Self {
        TopologyInfo {
            rack: HashMap::new(),
            datacenter: HashMap::new(),
        }
    }

    /// Sets the location for a node.
    pub fn set_location(&mut self, node: ClusterNodeId, rack: &str, dc: &str) {
        self.rack.insert(node, rack.to_string());
        self.datacenter.insert(node, dc.to_string());
    }

    /// Checks if two nodes are in the same rack.
    pub fn same_rack(&self, a: ClusterNodeId, b: ClusterNodeId) -> bool {
        self.rack.get(&a) == self.rack.get(&b) && self.rack.contains_key(&a)
    }

    /// Checks if two nodes are in the same datacenter.
    pub fn same_datacenter(&self, a: ClusterNodeId, b: ClusterNodeId) -> bool {
        self.datacenter.get(&a) == self.datacenter.get(&b) && self.datacenter.contains_key(&a)
    }
}

impl Default for TopologyInfo {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S11.9: Resource Quotas
// ═══════════════════════════════════════════════════════════════════════

/// Resource quota for a user or project.
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    /// Owner (user or project name).
    pub owner: String,
    /// Maximum CPU cores allowed.
    pub max_cpu: u32,
    /// Maximum GPUs allowed.
    pub max_gpu: u32,
    /// Maximum memory in MB.
    pub max_memory_mb: u64,
    /// Currently used CPU cores.
    pub used_cpu: u32,
    /// Currently used GPUs.
    pub used_gpu: u32,
    /// Currently used memory in MB.
    pub used_memory_mb: u64,
}

impl ResourceQuota {
    /// Creates a new quota.
    pub fn new(owner: &str, max_cpu: u32, max_gpu: u32, max_memory_mb: u64) -> Self {
        ResourceQuota {
            owner: owner.to_string(),
            max_cpu,
            max_gpu,
            max_memory_mb,
            used_cpu: 0,
            used_gpu: 0,
            used_memory_mb: 0,
        }
    }

    /// Checks if a resource request fits within the quota.
    pub fn can_allocate(&self, req: &TaskRequirements) -> bool {
        self.used_cpu + req.cpu_cores <= self.max_cpu
            && self.used_gpu + req.gpu_count <= self.max_gpu
            && self.used_memory_mb + req.memory_mb <= self.max_memory_mb
    }

    /// Allocates resources against the quota.
    pub fn allocate(&mut self, req: &TaskRequirements) -> bool {
        if !self.can_allocate(req) {
            return false;
        }
        self.used_cpu += req.cpu_cores;
        self.used_gpu += req.gpu_count;
        self.used_memory_mb += req.memory_mb;
        true
    }

    /// Releases allocated resources.
    pub fn release(&mut self, req: &TaskRequirements) {
        self.used_cpu = self.used_cpu.saturating_sub(req.cpu_cores);
        self.used_gpu = self.used_gpu.saturating_sub(req.gpu_count);
        self.used_memory_mb = self.used_memory_mb.saturating_sub(req.memory_mb);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: u64, cpu: u32, gpu: u32, mem: u64) -> ClusterNode {
        ClusterNode {
            id: ClusterNodeId(id),
            address: format!("10.0.0.{id}:8080"),
            resources: NodeResources {
                cpu_cores: cpu,
                gpu_count: gpu,
                memory_mb: mem,
                network_mbps: 1000,
            },
            status: NodeStatus::Healthy,
            location: None,
        }
    }

    // S11.1 — Node Discovery
    #[test]
    fn s11_1_discovery_methods() {
        let m = DiscoveryMethod::Multicast {
            group: "239.0.0.1".into(),
            port: 5000,
        };
        assert!(m.to_string().contains("Multicast"));

        let s = DiscoveryMethod::SeedNodes(vec!["node1".into(), "node2".into()]);
        assert!(s.to_string().contains("Seeds(2)"));
    }

    // S11.2 — Resource Advertisement
    #[test]
    fn s11_2_node_resources() {
        let node = make_node(1, 16, 2, 65536);
        assert_eq!(node.resources.cpu_cores, 16);
        assert_eq!(node.resources.gpu_count, 2);
    }

    // S11.3 — Task Placement
    #[test]
    fn s11_3_place_task() {
        let nodes = vec![make_node(1, 4, 0, 8192), make_node(2, 16, 4, 65536)];
        let req = TaskRequirements {
            cpu_cores: 8,
            gpu_count: 2,
            memory_mb: 16384,
        };
        let placed = place_task(&req, &nodes);
        assert_eq!(placed, Some(ClusterNodeId(2)));
    }

    #[test]
    fn s11_3_no_suitable_node() {
        let nodes = vec![make_node(1, 2, 0, 4096)];
        let req = TaskRequirements {
            cpu_cores: 32,
            gpu_count: 8,
            memory_mb: 131072,
        };
        assert_eq!(place_task(&req, &nodes), None);
    }

    // S11.4 — Fault Detection
    #[test]
    fn s11_4_heartbeat_detection() {
        let mut fd = FailureDetector::new(Duration::from_secs(10));
        fd.heartbeat(ClusterNodeId(1), 1000);
        fd.heartbeat(ClusterNodeId(2), 1000);

        assert!(fd.is_healthy(ClusterNodeId(1), 5000));
        assert!(!fd.is_healthy(ClusterNodeId(1), 15000)); // 15s > 10s timeout

        let suspected = fd.check(15000);
        assert_eq!(suspected.len(), 2);
    }

    // S11.5 — Task Migration
    #[test]
    fn s11_5_migration_result() {
        let result = MigrationResult::Success {
            from: ClusterNodeId(1),
            to: ClusterNodeId(2),
        };
        assert!(result.to_string().contains("Migrated"));
    }

    #[test]
    fn s11_5_checkpoint() {
        let cp = TaskCheckpoint {
            task_id: 42,
            state: vec![1, 2, 3],
            progress: 0.75,
            source_node: ClusterNodeId(1),
        };
        assert_eq!(cp.progress, 0.75);
    }

    // S11.6 — Work Stealing
    #[test]
    fn s11_6_work_stealing() {
        let mut q1 = WorkQueue::new(ClusterNodeId(1));
        let mut q2 = WorkQueue::new(ClusterNodeId(2));
        q1.push(100);
        q1.push(200);
        q1.push(300);
        q2.push(400);

        let target = find_steal_target(&[q1.clone(), q2], ClusterNodeId(2));
        assert_eq!(target, Some(ClusterNodeId(1)));

        let stolen = q1.steal(); // Steals from back
        assert_eq!(stolen, Some(300));
        assert_eq!(q1.len(), 2);
    }

    #[test]
    fn s11_6_pop_vs_steal() {
        let mut q = WorkQueue::new(ClusterNodeId(1));
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.pop(), Some(1)); // Front
        assert_eq!(q.steal(), Some(3)); // Back
        assert_eq!(q.len(), 1);
    }

    // S11.7 — Barrier Synchronization
    #[test]
    fn s11_7_barrier() {
        let mut barrier = ClusterBarrier::new(3);
        assert!(!barrier.is_complete());
        assert_eq!(barrier.pending(), 3);

        barrier.arrive(ClusterNodeId(1));
        barrier.arrive(ClusterNodeId(2));
        assert_eq!(barrier.pending(), 1);

        barrier.arrive(ClusterNodeId(3));
        assert!(barrier.is_complete());

        barrier.reset();
        assert!(!barrier.is_complete());
    }

    #[test]
    fn s11_7_barrier_duplicate_arrive() {
        let mut barrier = ClusterBarrier::new(2);
        barrier.arrive(ClusterNodeId(1));
        barrier.arrive(ClusterNodeId(1)); // duplicate
        assert_eq!(barrier.arrived.len(), 1);
    }

    // S11.8 — Cluster Topology
    #[test]
    fn s11_8_topology() {
        let mut topo = TopologyInfo::new();
        topo.set_location(ClusterNodeId(1), "rack-a", "dc-us-east");
        topo.set_location(ClusterNodeId(2), "rack-a", "dc-us-east");
        topo.set_location(ClusterNodeId(3), "rack-b", "dc-us-east");

        assert!(topo.same_rack(ClusterNodeId(1), ClusterNodeId(2)));
        assert!(!topo.same_rack(ClusterNodeId(1), ClusterNodeId(3)));
        assert!(topo.same_datacenter(ClusterNodeId(1), ClusterNodeId(3)));
    }

    // S11.9 — Resource Quotas
    #[test]
    fn s11_9_quota_allocation() {
        let mut quota = ResourceQuota::new("team-ml", 32, 4, 131072);
        let req = TaskRequirements {
            cpu_cores: 8,
            gpu_count: 2,
            memory_mb: 32768,
        };
        assert!(quota.allocate(&req));
        assert_eq!(quota.used_cpu, 8);
        assert_eq!(quota.used_gpu, 2);

        // Second allocation still fits
        assert!(quota.allocate(&req));

        // Third would exceed GPU quota
        assert!(!quota.can_allocate(&req));
    }

    #[test]
    fn s11_9_quota_release() {
        let mut quota = ResourceQuota::new("user", 16, 2, 65536);
        let req = TaskRequirements {
            cpu_cores: 8,
            gpu_count: 1,
            memory_mb: 16384,
        };
        quota.allocate(&req);
        quota.release(&req);
        assert_eq!(quota.used_cpu, 0);
        assert_eq!(quota.used_gpu, 0);
    }

    // S11.10 — Integration
    #[test]
    fn s11_10_node_status_display() {
        assert_eq!(NodeStatus::Healthy.to_string(), "Healthy");
        assert_eq!(NodeStatus::Dead.to_string(), "Dead");
        assert_eq!(NodeStatus::Draining.to_string(), "Draining");
    }

    #[test]
    fn s11_10_cluster_node_id_display() {
        assert_eq!(ClusterNodeId(42).to_string(), "ClusterNode(42)");
    }
}
