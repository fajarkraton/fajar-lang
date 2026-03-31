//! Distributed kernel services — multi-node kernel coordination.
//!
//! Sprint N3: Provides simulated distributed kernel primitives for
//! cluster-wide process management, shared memory, and service discovery.
//! All networking is simulated in-memory — no real sockets.
//!
//! # Architecture
//!
//! ```text
//! KernelRpcServer           — expose kernel services over simulated network
//! RemoteProcessSpawn        — fork process on remote node
//! DistributedSharedMemory   — cross-node shared pages
//! GlobalPidSpace            — cluster-wide unique PID allocation
//! NetworkFilesystem         — NFS-like remote mount
//! DistributedScheduler      — schedule across nodes by load
//! ClusterHealthMonitor      — heartbeat + failure detection
//! KernelNodeDiscovery       — mDNS-style discovery
//! SecureInterNode           — simulated TLS for kernel-to-kernel
//! ClusterIntegrationTest    — 4-node simulated cluster
//! ```

use std::collections::{HashMap, HashSet};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from distributed kernel operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DistributedKernelError {
    /// Node not found.
    #[error("node '{node_id}' not found in cluster")]
    NodeNotFound {
        /// The missing node ID.
        node_id: String,
    },

    /// Node already exists.
    #[error("node '{node_id}' already registered")]
    NodeAlreadyExists {
        /// The duplicate node ID.
        node_id: String,
    },

    /// RPC call failed.
    #[error("RPC to '{node_id}' failed: {reason}")]
    RpcFailed {
        /// Target node.
        node_id: String,
        /// Failure reason.
        reason: String,
    },

    /// Process not found.
    #[error("process {pid} not found on node '{node_id}'")]
    ProcessNotFound {
        /// Process ID.
        pid: u64,
        /// Node where process was expected.
        node_id: String,
    },

    /// PID space exhausted.
    #[error("global PID space exhausted (max {max})")]
    PidExhausted {
        /// Maximum PID value.
        max: u64,
    },

    /// Shared memory conflict.
    #[error("shared memory conflict for region '{region}': {reason}")]
    SharedMemoryConflict {
        /// Region name.
        region: String,
        /// Conflict description.
        reason: String,
    },

    /// Network filesystem error.
    #[error("NFS error: {reason}")]
    NfsError {
        /// Error description.
        reason: String,
    },

    /// Health check failure.
    #[error("node '{node_id}' failed health check: {reason}")]
    HealthCheckFailed {
        /// Failing node.
        node_id: String,
        /// Failure reason.
        reason: String,
    },

    /// TLS/security error.
    #[error("security error: {reason}")]
    SecurityError {
        /// Error description.
        reason: String,
    },
}

/// Result type for distributed kernel operations.
pub type DistKernelResult<T> = Result<T, DistributedKernelError>;

// ═══════════════════════════════════════════════════════════════════════
// Node representation
// ═══════════════════════════════════════════════════════════════════════

/// Status of a node in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    /// Node is healthy and accepting work.
    Healthy,
    /// Node is degraded (high load, but still responding).
    Degraded,
    /// Node is unreachable (missed heartbeats).
    Unreachable,
    /// Node has been removed from the cluster.
    Removed,
}

/// A node in the distributed kernel cluster.
#[derive(Debug, Clone)]
pub struct KernelNode {
    /// Unique node identifier.
    pub id: String,
    /// Simulated IP address.
    pub address: String,
    /// Node status.
    pub status: NodeStatus,
    /// Current load (0.0 - 1.0).
    pub load: f64,
    /// Number of processes running on this node.
    pub process_count: u64,
    /// Heartbeat sequence number.
    pub heartbeat_seq: u64,
    /// Whether TLS is established for this node.
    pub tls_established: bool,
}

impl KernelNode {
    /// Creates a new kernel node.
    pub fn new(id: &str, address: &str) -> Self {
        Self {
            id: id.to_string(),
            address: address.to_string(),
            status: NodeStatus::Healthy,
            load: 0.0,
            process_count: 0,
            heartbeat_seq: 0,
            tls_established: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Kernel RPC Server
// ═══════════════════════════════════════════════════════════════════════

/// An RPC request.
#[derive(Debug, Clone)]
pub struct RpcRequest {
    /// Source node ID.
    pub from: String,
    /// Method name.
    pub method: String,
    /// Serialized arguments (simulated as bytes).
    pub args: Vec<u8>,
}

/// An RPC response.
#[derive(Debug, Clone)]
pub struct RpcResponse {
    /// Whether the call succeeded.
    pub success: bool,
    /// Response data (simulated as bytes).
    pub data: Vec<u8>,
    /// Error message if not successful.
    pub error: Option<String>,
}

/// Simulated RPC server exposing kernel services to other nodes.
#[derive(Debug, Clone)]
pub struct KernelRpcServer {
    /// Node ID of this server.
    node_id: String,
    /// Registered method handlers: method -> handler description.
    methods: HashMap<String, String>,
    /// Request log (for testing).
    request_log: Vec<RpcRequest>,
}

impl KernelRpcServer {
    /// Creates a new RPC server for the given node.
    pub fn new(node_id: &str) -> Self {
        Self {
            node_id: node_id.to_string(),
            methods: HashMap::new(),
            request_log: Vec::new(),
        }
    }

    /// Registers an RPC method.
    pub fn register_method(&mut self, method: &str, description: &str) {
        self.methods
            .insert(method.to_string(), description.to_string());
    }

    /// Handles an RPC request. Returns a simulated response.
    pub fn handle_request(&mut self, request: RpcRequest) -> RpcResponse {
        self.request_log.push(request.clone());
        if self.methods.contains_key(&request.method) {
            RpcResponse {
                success: true,
                data: format!("{}:ok", self.node_id).into_bytes(),
                error: None,
            }
        } else {
            RpcResponse {
                success: false,
                data: Vec::new(),
                error: Some(format!("unknown method: {}", request.method)),
            }
        }
    }

    /// Returns the number of handled requests.
    pub fn request_count(&self) -> usize {
        self.request_log.len()
    }

    /// Returns the number of registered methods.
    pub fn method_count(&self) -> usize {
        self.methods.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Remote Process Spawn
// ═══════════════════════════════════════════════════════════════════════

/// Serialized process state for remote spawn.
#[derive(Debug, Clone)]
pub struct ProcessState {
    /// Process ID.
    pub pid: u64,
    /// Process name.
    pub name: String,
    /// Serialized memory (simulated).
    pub memory_snapshot: Vec<u8>,
    /// Register state (simulated as key-value).
    pub registers: HashMap<String, u64>,
}

/// Manages remote process spawning across nodes.
#[derive(Debug, Clone)]
pub struct RemoteProcessSpawn {
    /// Processes by (node_id, pid).
    processes: HashMap<(String, u64), ProcessState>,
    /// Migration history: (from_node, to_node, pid).
    migrations: Vec<(String, String, u64)>,
}

impl RemoteProcessSpawn {
    /// Creates a new remote process spawn manager.
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            migrations: Vec::new(),
        }
    }

    /// Spawns a process on a remote node.
    pub fn spawn_remote(&mut self, node_id: &str, state: ProcessState) -> DistKernelResult<()> {
        let key = (node_id.to_string(), state.pid);
        if self.processes.contains_key(&key) {
            return Err(DistributedKernelError::ProcessNotFound {
                pid: state.pid,
                node_id: node_id.to_string(),
            });
        }
        self.processes.insert(key, state);
        Ok(())
    }

    /// Migrates a process from one node to another.
    pub fn migrate(&mut self, from: &str, to: &str, pid: u64) -> DistKernelResult<()> {
        let from_key = (from.to_string(), pid);
        let state =
            self.processes
                .remove(&from_key)
                .ok_or(DistributedKernelError::ProcessNotFound {
                    pid,
                    node_id: from.to_string(),
                })?;
        let to_key = (to.to_string(), pid);
        self.processes.insert(to_key, state);
        self.migrations
            .push((from.to_string(), to.to_string(), pid));
        Ok(())
    }

    /// Returns the number of active processes.
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Returns the migration count.
    pub fn migration_count(&self) -> usize {
        self.migrations.len()
    }
}

impl Default for RemoteProcessSpawn {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Distributed Shared Memory
// ═══════════════════════════════════════════════════════════════════════

/// Consistency protocol for shared memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyProtocol {
    /// Strong consistency (all nodes see writes immediately).
    Strong,
    /// Eventual consistency (writes propagate asynchronously).
    Eventual,
    /// Release consistency (sync on acquire/release).
    Release,
}

/// A shared memory region accessible across nodes.
#[derive(Debug, Clone)]
pub struct DistributedRegion {
    /// Region name.
    pub name: String,
    /// Data (simulated).
    pub data: Vec<u8>,
    /// Owner node.
    pub owner: String,
    /// Nodes that have a cached copy.
    pub replicas: HashSet<String>,
    /// Consistency protocol.
    pub protocol: ConsistencyProtocol,
    /// Version number for conflict detection.
    pub version: u64,
}

/// Manages cross-node shared memory pages with consistency protocols.
#[derive(Debug, Clone)]
pub struct DistributedSharedMemory {
    /// Regions by name.
    regions: HashMap<String, DistributedRegion>,
}

impl DistributedSharedMemory {
    /// Creates a new distributed shared memory manager.
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
        }
    }

    /// Creates a new shared region.
    pub fn create_region(
        &mut self,
        name: &str,
        owner: &str,
        size: usize,
        protocol: ConsistencyProtocol,
    ) -> DistKernelResult<()> {
        if self.regions.contains_key(name) {
            return Err(DistributedKernelError::SharedMemoryConflict {
                region: name.to_string(),
                reason: "already exists".to_string(),
            });
        }
        let mut replicas = HashSet::new();
        replicas.insert(owner.to_string());
        self.regions.insert(
            name.to_string(),
            DistributedRegion {
                name: name.to_string(),
                data: vec![0; size],
                owner: owner.to_string(),
                replicas,
                protocol,
                version: 0,
            },
        );
        Ok(())
    }

    /// Writes data to a shared region, incrementing the version.
    pub fn write_region(&mut self, name: &str, offset: usize, data: &[u8]) -> DistKernelResult<()> {
        let region =
            self.regions
                .get_mut(name)
                .ok_or(DistributedKernelError::SharedMemoryConflict {
                    region: name.to_string(),
                    reason: "not found".to_string(),
                })?;
        let end = offset + data.len();
        if end > region.data.len() {
            return Err(DistributedKernelError::SharedMemoryConflict {
                region: name.to_string(),
                reason: "write out of bounds".to_string(),
            });
        }
        region.data[offset..end].copy_from_slice(data);
        region.version += 1;
        Ok(())
    }

    /// Reads data from a shared region.
    pub fn read_region(&self, name: &str, offset: usize, len: usize) -> DistKernelResult<Vec<u8>> {
        let region =
            self.regions
                .get(name)
                .ok_or(DistributedKernelError::SharedMemoryConflict {
                    region: name.to_string(),
                    reason: "not found".to_string(),
                })?;
        let end = offset + len;
        if end > region.data.len() {
            return Err(DistributedKernelError::SharedMemoryConflict {
                region: name.to_string(),
                reason: "read out of bounds".to_string(),
            });
        }
        Ok(region.data[offset..end].to_vec())
    }

    /// Adds a replica node to a region.
    pub fn add_replica(&mut self, name: &str, node_id: &str) -> DistKernelResult<()> {
        let region =
            self.regions
                .get_mut(name)
                .ok_or(DistributedKernelError::SharedMemoryConflict {
                    region: name.to_string(),
                    reason: "not found".to_string(),
                })?;
        region.replicas.insert(node_id.to_string());
        Ok(())
    }

    /// Returns the version of a region.
    pub fn region_version(&self, name: &str) -> Option<u64> {
        self.regions.get(name).map(|r| r.version)
    }

    /// Returns the number of regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

impl Default for DistributedSharedMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Global PID Space
// ═══════════════════════════════════════════════════════════════════════

/// Allocates cluster-wide unique PIDs.
#[derive(Debug, Clone)]
pub struct GlobalPidSpace {
    /// Next PID to allocate.
    next_pid: u64,
    /// Maximum PID value.
    max_pid: u64,
    /// PID -> node assignment.
    assignments: HashMap<u64, String>,
}

impl GlobalPidSpace {
    /// Creates a new global PID space with the given maximum.
    pub fn new(max_pid: u64) -> Self {
        Self {
            next_pid: 1,
            max_pid,
            assignments: HashMap::new(),
        }
    }

    /// Allocates a new PID for a process on the given node.
    pub fn allocate(&mut self, node_id: &str) -> DistKernelResult<u64> {
        if self.next_pid > self.max_pid {
            return Err(DistributedKernelError::PidExhausted { max: self.max_pid });
        }
        let pid = self.next_pid;
        self.next_pid += 1;
        self.assignments.insert(pid, node_id.to_string());
        Ok(pid)
    }

    /// Frees a PID (process exited).
    pub fn free(&mut self, pid: u64) {
        self.assignments.remove(&pid);
    }

    /// Looks up which node owns a PID.
    pub fn lookup(&self, pid: u64) -> Option<&str> {
        self.assignments.get(&pid).map(|s| s.as_str())
    }

    /// Returns the number of active PIDs.
    pub fn active_count(&self) -> usize {
        self.assignments.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Filesystem
// ═══════════════════════════════════════════════════════════════════════

/// A mount point for a remote filesystem.
#[derive(Debug, Clone)]
pub struct NfsMount {
    /// Local mount path.
    pub local_path: String,
    /// Remote node ID.
    pub remote_node: String,
    /// Remote export path.
    pub remote_path: String,
    /// Read-only mount.
    pub read_only: bool,
}

/// NFS-like network filesystem for cross-node file access.
#[derive(Debug, Clone)]
pub struct NetworkFilesystem {
    /// Active mounts.
    mounts: Vec<NfsMount>,
    /// Simulated file cache: (mount_index, path) -> data.
    cache: HashMap<(usize, String), Vec<u8>>,
}

impl NetworkFilesystem {
    /// Creates a new network filesystem manager.
    pub fn new() -> Self {
        Self {
            mounts: Vec::new(),
            cache: HashMap::new(),
        }
    }

    /// Mounts a remote filesystem.
    pub fn mount(
        &mut self,
        local_path: &str,
        remote_node: &str,
        remote_path: &str,
        read_only: bool,
    ) -> usize {
        let idx = self.mounts.len();
        self.mounts.push(NfsMount {
            local_path: local_path.to_string(),
            remote_node: remote_node.to_string(),
            remote_path: remote_path.to_string(),
            read_only,
        });
        idx
    }

    /// Unmounts by mount index.
    pub fn unmount(&mut self, index: usize) -> DistKernelResult<()> {
        if index >= self.mounts.len() {
            return Err(DistributedKernelError::NfsError {
                reason: format!("mount index {} out of range", index),
            });
        }
        self.mounts.remove(index);
        Ok(())
    }

    /// Simulates reading a file from a mount.
    pub fn read_file(&mut self, mount_index: usize, path: &str) -> DistKernelResult<Vec<u8>> {
        if mount_index >= self.mounts.len() {
            return Err(DistributedKernelError::NfsError {
                reason: format!("mount index {} out of range", mount_index),
            });
        }
        let key = (mount_index, path.to_string());
        Ok(self.cache.get(&key).cloned().unwrap_or_default())
    }

    /// Simulates writing a file to a mount.
    pub fn write_file(
        &mut self,
        mount_index: usize,
        path: &str,
        data: Vec<u8>,
    ) -> DistKernelResult<()> {
        if mount_index >= self.mounts.len() {
            return Err(DistributedKernelError::NfsError {
                reason: format!("mount index {} out of range", mount_index),
            });
        }
        if self.mounts[mount_index].read_only {
            return Err(DistributedKernelError::NfsError {
                reason: "mount is read-only".to_string(),
            });
        }
        let key = (mount_index, path.to_string());
        self.cache.insert(key, data);
        Ok(())
    }

    /// Returns the number of active mounts.
    pub fn mount_count(&self) -> usize {
        self.mounts.len()
    }
}

impl Default for NetworkFilesystem {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Distributed Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// Schedules processes across nodes based on load.
#[derive(Debug, Clone)]
pub struct DistributedScheduler {
    /// Node loads: node_id -> current load (0.0 - 1.0).
    node_loads: HashMap<String, f64>,
    /// Assignment history: pid -> node_id.
    assignments: HashMap<u64, String>,
    /// Load threshold above which a node is considered overloaded.
    overload_threshold: f64,
}

impl DistributedScheduler {
    /// Creates a new distributed scheduler.
    pub fn new(overload_threshold: f64) -> Self {
        Self {
            node_loads: HashMap::new(),
            assignments: HashMap::new(),
            overload_threshold,
        }
    }

    /// Registers a node with its current load.
    pub fn update_load(&mut self, node_id: &str, load: f64) {
        self.node_loads.insert(node_id.to_string(), load);
    }

    /// Selects the best node for a new process (lowest load).
    pub fn select_node(&self) -> DistKernelResult<String> {
        self.node_loads
            .iter()
            .filter(|&(_, &load)| load < self.overload_threshold)
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id.clone())
            .ok_or(DistributedKernelError::RpcFailed {
                node_id: "cluster".to_string(),
                reason: "all nodes overloaded".to_string(),
            })
    }

    /// Assigns a process to a node.
    pub fn assign(&mut self, pid: u64, node_id: &str) {
        self.assignments.insert(pid, node_id.to_string());
    }

    /// Returns the number of nodes in the scheduler.
    pub fn node_count(&self) -> usize {
        self.node_loads.len()
    }

    /// Returns the number of assigned processes.
    pub fn assignment_count(&self) -> usize {
        self.assignments.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Cluster Health Monitor
// ═══════════════════════════════════════════════════════════════════════

/// Monitors cluster health via heartbeats and failure detection.
#[derive(Debug, Clone)]
pub struct ClusterHealthMonitor {
    /// Last heartbeat sequence per node.
    last_heartbeat: HashMap<String, u64>,
    /// Nodes marked as failed.
    failed_nodes: HashSet<String>,
    /// Maximum missed heartbeats before marking as failed.
    max_missed: u64,
    /// Current tick (simulated time).
    current_tick: u64,
}

impl ClusterHealthMonitor {
    /// Creates a new health monitor with the given failure threshold.
    pub fn new(max_missed: u64) -> Self {
        Self {
            last_heartbeat: HashMap::new(),
            failed_nodes: HashSet::new(),
            max_missed,
            current_tick: 0,
        }
    }

    /// Records a heartbeat from a node.
    pub fn record_heartbeat(&mut self, node_id: &str) {
        self.last_heartbeat
            .insert(node_id.to_string(), self.current_tick);
        self.failed_nodes.remove(node_id);
    }

    /// Advances the tick and checks for failures.
    pub fn tick(&mut self) -> Vec<String> {
        self.current_tick += 1;
        let mut newly_failed = Vec::new();
        for (node_id, &last) in &self.last_heartbeat {
            if self.current_tick - last > self.max_missed && !self.failed_nodes.contains(node_id) {
                newly_failed.push(node_id.clone());
            }
        }
        for node_id in &newly_failed {
            self.failed_nodes.insert(node_id.clone());
        }
        newly_failed
    }

    /// Returns true if a node is considered failed.
    pub fn is_failed(&self, node_id: &str) -> bool {
        self.failed_nodes.contains(node_id)
    }

    /// Returns the number of healthy nodes.
    pub fn healthy_count(&self) -> usize {
        self.last_heartbeat.len() - self.failed_nodes.len()
    }

    /// Returns the number of failed nodes.
    pub fn failed_count(&self) -> usize {
        self.failed_nodes.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Kernel Node Discovery
// ═══════════════════════════════════════════════════════════════════════

/// mDNS-style kernel-to-kernel node discovery.
#[derive(Debug, Clone)]
pub struct KernelNodeDiscovery {
    /// Known nodes: id -> address.
    known_nodes: HashMap<String, String>,
    /// Discovery announcements log.
    announcements: Vec<(String, String)>,
}

impl KernelNodeDiscovery {
    /// Creates a new node discovery service.
    pub fn new() -> Self {
        Self {
            known_nodes: HashMap::new(),
            announcements: Vec::new(),
        }
    }

    /// Announces this node's presence.
    pub fn announce(&mut self, node_id: &str, address: &str) {
        self.known_nodes
            .insert(node_id.to_string(), address.to_string());
        self.announcements
            .push((node_id.to_string(), address.to_string()));
    }

    /// Discovers all known nodes.
    pub fn discover(&self) -> Vec<(String, String)> {
        self.known_nodes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Removes a node from the known set.
    pub fn remove(&mut self, node_id: &str) {
        self.known_nodes.remove(node_id);
    }

    /// Returns the number of known nodes.
    pub fn known_count(&self) -> usize {
        self.known_nodes.len()
    }

    /// Returns the number of announcements made.
    pub fn announcement_count(&self) -> usize {
        self.announcements.len()
    }
}

impl Default for KernelNodeDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Secure Inter-Node Communication
// ═══════════════════════════════════════════════════════════════════════

/// Simulated TLS session state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsState {
    /// No session established.
    None,
    /// Handshake in progress.
    Handshaking,
    /// Session established (encrypted communication).
    Established,
    /// Session closed.
    Closed,
}

/// Simulated TLS session between two nodes.
#[derive(Debug, Clone)]
pub struct TlsSession {
    /// Local node ID.
    pub local: String,
    /// Remote node ID.
    pub remote: String,
    /// Session state.
    pub state: TlsState,
    /// Simulated cipher suite.
    pub cipher_suite: String,
    /// Bytes encrypted (simulated counter).
    pub bytes_encrypted: u64,
}

/// Manages simulated TLS connections between kernel nodes.
#[derive(Debug, Clone)]
pub struct SecureInterNode {
    /// Active sessions by (local, remote) pair.
    sessions: HashMap<(String, String), TlsSession>,
}

impl SecureInterNode {
    /// Creates a new secure inter-node manager.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Initiates a TLS handshake with a remote node.
    pub fn initiate_handshake(&mut self, local: &str, remote: &str) -> DistKernelResult<()> {
        let key = (local.to_string(), remote.to_string());
        if self.sessions.contains_key(&key) {
            return Err(DistributedKernelError::SecurityError {
                reason: format!("session already exists: {} -> {}", local, remote),
            });
        }
        self.sessions.insert(
            key,
            TlsSession {
                local: local.to_string(),
                remote: remote.to_string(),
                state: TlsState::Handshaking,
                cipher_suite: "TLS_AES_256_GCM_SHA384".to_string(),
                bytes_encrypted: 0,
            },
        );
        Ok(())
    }

    /// Completes the TLS handshake.
    pub fn complete_handshake(&mut self, local: &str, remote: &str) -> DistKernelResult<()> {
        let key = (local.to_string(), remote.to_string());
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(DistributedKernelError::SecurityError {
                reason: format!("no session found: {} -> {}", local, remote),
            })?;
        if session.state != TlsState::Handshaking {
            return Err(DistributedKernelError::SecurityError {
                reason: "session not in handshaking state".to_string(),
            });
        }
        session.state = TlsState::Established;
        Ok(())
    }

    /// Simulates sending encrypted data.
    pub fn send_encrypted(
        &mut self,
        local: &str,
        remote: &str,
        data_len: u64,
    ) -> DistKernelResult<()> {
        let key = (local.to_string(), remote.to_string());
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(DistributedKernelError::SecurityError {
                reason: format!("no session found: {} -> {}", local, remote),
            })?;
        if session.state != TlsState::Established {
            return Err(DistributedKernelError::SecurityError {
                reason: "session not established".to_string(),
            });
        }
        session.bytes_encrypted += data_len;
        Ok(())
    }

    /// Closes a TLS session.
    pub fn close_session(&mut self, local: &str, remote: &str) -> DistKernelResult<()> {
        let key = (local.to_string(), remote.to_string());
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(DistributedKernelError::SecurityError {
                reason: format!("no session found: {} -> {}", local, remote),
            })?;
        session.state = TlsState::Closed;
        Ok(())
    }

    /// Returns the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns the number of established sessions.
    pub fn established_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.state == TlsState::Established)
            .count()
    }
}

impl Default for SecureInterNode {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── KernelRpcServer ──

    #[test]
    fn rpc_server_handles_known_method() {
        let mut server = KernelRpcServer::new("node1");
        server.register_method("spawn_process", "spawn a remote process");
        let resp = server.handle_request(RpcRequest {
            from: "node2".into(),
            method: "spawn_process".into(),
            args: vec![],
        });
        assert!(resp.success);
        assert_eq!(server.request_count(), 1);
    }

    #[test]
    fn rpc_server_rejects_unknown_method() {
        let mut server = KernelRpcServer::new("node1");
        let resp = server.handle_request(RpcRequest {
            from: "node2".into(),
            method: "nonexistent".into(),
            args: vec![],
        });
        assert!(!resp.success);
        assert!(resp.error.is_some());
    }

    // ── RemoteProcessSpawn ──

    #[test]
    fn remote_spawn_and_migrate() {
        let mut spawner = RemoteProcessSpawn::new();
        let state = ProcessState {
            pid: 100,
            name: "worker".into(),
            memory_snapshot: vec![0; 64],
            registers: HashMap::new(),
        };
        assert!(spawner.spawn_remote("node1", state).is_ok());
        assert_eq!(spawner.process_count(), 1);
        assert!(spawner.migrate("node1", "node2", 100).is_ok());
        assert_eq!(spawner.migration_count(), 1);
    }

    #[test]
    fn remote_migrate_nonexistent_fails() {
        let mut spawner = RemoteProcessSpawn::new();
        assert!(spawner.migrate("node1", "node2", 999).is_err());
    }

    // ── DistributedSharedMemory ──

    #[test]
    fn dsm_create_write_read() {
        let mut dsm = DistributedSharedMemory::new();
        assert!(
            dsm.create_region("shmem1", "node1", 1024, ConsistencyProtocol::Strong)
                .is_ok()
        );
        assert!(dsm.write_region("shmem1", 0, &[1, 2, 3, 4]).is_ok());
        let data = dsm.read_region("shmem1", 0, 4).unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);
        assert_eq!(dsm.region_version("shmem1"), Some(1));
    }

    #[test]
    fn dsm_duplicate_create_fails() {
        let mut dsm = DistributedSharedMemory::new();
        assert!(
            dsm.create_region("r1", "n1", 64, ConsistencyProtocol::Eventual)
                .is_ok()
        );
        assert!(
            dsm.create_region("r1", "n2", 64, ConsistencyProtocol::Eventual)
                .is_err()
        );
    }

    #[test]
    fn dsm_out_of_bounds_write() {
        let mut dsm = DistributedSharedMemory::new();
        assert!(
            dsm.create_region("small", "n1", 4, ConsistencyProtocol::Strong)
                .is_ok()
        );
        assert!(dsm.write_region("small", 2, &[1, 2, 3]).is_err());
    }

    // ── GlobalPidSpace ──

    #[test]
    fn pid_space_allocate_and_lookup() {
        let mut pids = GlobalPidSpace::new(1000);
        let p1 = pids.allocate("node1").unwrap();
        let p2 = pids.allocate("node2").unwrap();
        assert_ne!(p1, p2);
        assert_eq!(pids.lookup(p1), Some("node1"));
        assert_eq!(pids.lookup(p2), Some("node2"));
    }

    #[test]
    fn pid_space_exhaustion() {
        let mut pids = GlobalPidSpace::new(2);
        assert!(pids.allocate("n1").is_ok());
        assert!(pids.allocate("n2").is_ok());
        assert!(pids.allocate("n3").is_err());
    }

    // ── NetworkFilesystem ──

    #[test]
    fn nfs_mount_write_read() {
        let mut nfs = NetworkFilesystem::new();
        let idx = nfs.mount("/mnt/remote", "node2", "/export", false);
        assert!(nfs.write_file(idx, "test.txt", b"hello".to_vec()).is_ok());
        let data = nfs.read_file(idx, "test.txt").unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn nfs_read_only_write_fails() {
        let mut nfs = NetworkFilesystem::new();
        let idx = nfs.mount("/mnt/ro", "node3", "/export", true);
        assert!(nfs.write_file(idx, "test.txt", vec![1]).is_err());
    }

    // ── DistributedScheduler ──

    #[test]
    fn scheduler_selects_least_loaded() {
        let mut sched = DistributedScheduler::new(0.8);
        sched.update_load("node1", 0.7);
        sched.update_load("node2", 0.3);
        sched.update_load("node3", 0.5);
        let selected = sched.select_node().unwrap();
        assert_eq!(selected, "node2");
    }

    #[test]
    fn scheduler_all_overloaded() {
        let mut sched = DistributedScheduler::new(0.5);
        sched.update_load("node1", 0.9);
        sched.update_load("node2", 0.8);
        assert!(sched.select_node().is_err());
    }

    // ── ClusterHealthMonitor ──

    #[test]
    fn health_monitor_detects_failure() {
        let mut monitor = ClusterHealthMonitor::new(3);
        monitor.record_heartbeat("node1");
        monitor.record_heartbeat("node2");
        // Advance 4 ticks without heartbeat from node1.
        for _ in 0..4 {
            monitor.tick();
        }
        assert!(monitor.is_failed("node1"));
        assert!(monitor.is_failed("node2"));
    }

    #[test]
    fn health_monitor_recovery() {
        let mut monitor = ClusterHealthMonitor::new(2);
        monitor.record_heartbeat("node1");
        monitor.tick();
        monitor.tick();
        monitor.tick(); // node1 missed > 2 ticks.
        assert!(monitor.is_failed("node1"));
        // Node recovers.
        monitor.record_heartbeat("node1");
        assert!(!monitor.is_failed("node1"));
    }

    // ── KernelNodeDiscovery ──

    #[test]
    fn discovery_announce_and_discover() {
        let mut disc = KernelNodeDiscovery::new();
        disc.announce("node1", "10.0.0.1:9000");
        disc.announce("node2", "10.0.0.2:9000");
        let nodes = disc.discover();
        assert_eq!(nodes.len(), 2);
        assert_eq!(disc.known_count(), 2);
    }

    #[test]
    fn discovery_remove_node() {
        let mut disc = KernelNodeDiscovery::new();
        disc.announce("node1", "10.0.0.1:9000");
        disc.remove("node1");
        assert_eq!(disc.known_count(), 0);
    }

    // ── SecureInterNode ──

    #[test]
    fn tls_full_lifecycle() {
        let mut sec = SecureInterNode::new();
        assert!(sec.initiate_handshake("node1", "node2").is_ok());
        assert!(sec.complete_handshake("node1", "node2").is_ok());
        assert_eq!(sec.established_count(), 1);
        assert!(sec.send_encrypted("node1", "node2", 1024).is_ok());
        assert!(sec.close_session("node1", "node2").is_ok());
        assert_eq!(sec.established_count(), 0);
    }

    #[test]
    fn tls_send_before_established_fails() {
        let mut sec = SecureInterNode::new();
        assert!(sec.initiate_handshake("n1", "n2").is_ok());
        // Still handshaking, not established.
        assert!(sec.send_encrypted("n1", "n2", 100).is_err());
    }

    #[test]
    fn tls_duplicate_session_fails() {
        let mut sec = SecureInterNode::new();
        assert!(sec.initiate_handshake("n1", "n2").is_ok());
        assert!(sec.initiate_handshake("n1", "n2").is_err());
    }

    // ── Cluster Integration ──

    #[test]
    fn four_node_cluster_integration() {
        // Set up 4-node cluster.
        let mut discovery = KernelNodeDiscovery::new();
        let mut health = ClusterHealthMonitor::new(3);
        let mut scheduler = DistributedScheduler::new(0.8);
        let mut pids = GlobalPidSpace::new(10000);
        let mut dsm = DistributedSharedMemory::new();
        let mut sec = SecureInterNode::new();

        let nodes = ["node1", "node2", "node3", "node4"];
        for (i, node) in nodes.iter().enumerate() {
            discovery.announce(node, &format!("10.0.0.{}:9000", i + 1));
            health.record_heartbeat(node);
            scheduler.update_load(node, 0.1 + i as f64 * 0.1);
        }

        // All nodes discovered.
        assert_eq!(discovery.known_count(), 4);

        // Establish TLS between all pairs.
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                assert!(sec.initiate_handshake(nodes[i], nodes[j]).is_ok());
                assert!(sec.complete_handshake(nodes[i], nodes[j]).is_ok());
            }
        }
        assert_eq!(sec.established_count(), 6); // C(4,2) = 6 pairs

        // Schedule processes.
        let best = scheduler.select_node().unwrap();
        assert_eq!(best, "node1"); // Lowest load.
        let pid = pids.allocate(&best).unwrap();
        scheduler.assign(pid, &best);

        // Create shared memory.
        assert!(
            dsm.create_region("shared_buf", "node1", 4096, ConsistencyProtocol::Strong)
                .is_ok()
        );
        assert!(dsm.add_replica("shared_buf", "node2").is_ok());

        // Health check: all healthy.
        health.tick();
        assert_eq!(health.failed_count(), 0);
    }
}
