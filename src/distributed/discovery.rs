//! Service Discovery — mDNS, seed node bootstrap, DNS-based discovery,
//! gossip protocol (SWIM-based), node metadata, health checking,
//! auto-scaling events, service registry, configuration.
//!
//! Sprint D2: Service Discovery (10 tasks)
//! All simulated — no real networking.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D2.1: mDNS Discovery
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a discoverable node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiscoveryNodeId(pub u64);

impl fmt::Display for DiscoveryNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DNode({})", self.0)
    }
}

/// An mDNS service record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdnsRecord {
    /// Service name (e.g., "_fj-ml._tcp.local.").
    pub service_name: String,
    /// Hostname.
    pub hostname: String,
    /// Port.
    pub port: u16,
    /// TTL in seconds.
    pub ttl_secs: u32,
    /// TXT record key-value pairs (metadata).
    pub txt: HashMap<String, String>,
}

/// Simulated mDNS discovery engine.
#[derive(Debug, Default)]
pub struct MdnsDiscovery {
    /// Known service records.
    records: Vec<MdnsRecord>,
}

impl MdnsDiscovery {
    /// Creates a new mDNS discovery engine.
    pub fn new() -> Self {
        MdnsDiscovery::default()
    }

    /// Announces a service on the local network (simulated).
    pub fn announce(&mut self, record: MdnsRecord) {
        // Avoid duplicates by hostname+port.
        self.records
            .retain(|r| r.hostname != record.hostname || r.port != record.port);
        self.records.push(record);
    }

    /// Queries for services matching a service name.
    pub fn query(&self, service_name: &str) -> Vec<&MdnsRecord> {
        self.records
            .iter()
            .filter(|r| r.service_name == service_name)
            .collect()
    }

    /// Removes expired records based on TTL. `now_secs` is the current time.
    pub fn expire(&mut self, announced_at_secs: &HashMap<String, u32>, now_secs: u32) {
        self.records.retain(|r| {
            let announced = announced_at_secs.get(&r.hostname).copied().unwrap_or(0);
            now_secs.saturating_sub(announced) < r.ttl_secs
        });
    }

    /// Returns all known records.
    pub fn all_records(&self) -> &[MdnsRecord] {
        &self.records
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.2: Seed Node Bootstrap
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for seed-based cluster bootstrap.
#[derive(Debug, Clone)]
pub struct SeedConfig {
    /// List of seed node addresses (host:port).
    pub seeds: Vec<String>,
    /// Maximum join attempts per seed.
    pub max_attempts: u32,
    /// Timeout per attempt in milliseconds.
    pub timeout_ms: u64,
}

/// Result of attempting to join a cluster via seed nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinResult {
    /// Successfully joined via the given seed address.
    Joined { via_seed: String },
    /// All seeds were unreachable.
    AllSeedsUnreachable,
    /// Forming a new cluster (this node is the first seed).
    FormedNewCluster,
}

impl fmt::Display for JoinResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JoinResult::Joined { via_seed } => write!(f, "Joined via {via_seed}"),
            JoinResult::AllSeedsUnreachable => write!(f, "All seeds unreachable"),
            JoinResult::FormedNewCluster => write!(f, "Formed new cluster"),
        }
    }
}

/// Simulates seed node bootstrap. `reachable` contains addresses that are alive.
pub fn seed_bootstrap(config: &SeedConfig, reachable: &[String]) -> JoinResult {
    for seed in &config.seeds {
        if reachable.contains(seed) {
            return JoinResult::Joined {
                via_seed: seed.clone(),
            };
        }
    }
    // If no seeds are reachable but we are listed as a seed, form a new cluster.
    if config.seeds.is_empty() {
        return JoinResult::FormedNewCluster;
    }
    JoinResult::AllSeedsUnreachable
}

// ═══════════════════════════════════════════════════════════════════════
// D2.3: DNS-Based Discovery
// ═══════════════════════════════════════════════════════════════════════

/// A DNS SRV record for service discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsSrvRecord {
    /// Target hostname.
    pub target: String,
    /// Port.
    pub port: u16,
    /// Priority (lower = preferred).
    pub priority: u16,
    /// Weight (for load balancing within same priority).
    pub weight: u16,
}

/// Simulated DNS resolver for service discovery.
#[derive(Debug, Default)]
pub struct DnsResolver {
    /// SRV records keyed by query domain.
    records: HashMap<String, Vec<DnsSrvRecord>>,
}

impl DnsResolver {
    /// Creates a new DNS resolver.
    pub fn new() -> Self {
        DnsResolver::default()
    }

    /// Adds a SRV record.
    pub fn add_srv(&mut self, domain: &str, record: DnsSrvRecord) {
        self.records
            .entry(domain.to_string())
            .or_default()
            .push(record);
    }

    /// Resolves a domain to SRV records, sorted by priority then weight.
    pub fn resolve_srv(&self, domain: &str) -> Vec<&DnsSrvRecord> {
        let mut results: Vec<&DnsSrvRecord> = self
            .records
            .get(domain)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        results.sort_by(|a, b| a.priority.cmp(&b.priority).then(b.weight.cmp(&a.weight)));
        results
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.4: Gossip Protocol (SWIM-based)
// ═══════════════════════════════════════════════════════════════════════

/// SWIM membership state for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwimState {
    /// Node is alive.
    Alive,
    /// Node is suspected (missed direct ping, waiting for indirect ack).
    Suspect,
    /// Node is confirmed dead.
    Dead,
}

impl fmt::Display for SwimState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SwimState::Alive => write!(f, "Alive"),
            SwimState::Suspect => write!(f, "Suspect"),
            SwimState::Dead => write!(f, "Dead"),
        }
    }
}

/// SWIM gossip member entry.
#[derive(Debug, Clone)]
pub struct SwimMember {
    /// Node ID.
    pub id: DiscoveryNodeId,
    /// Address.
    pub address: String,
    /// Current SWIM state.
    pub state: SwimState,
    /// Incarnation number (monotonically increasing per node).
    pub incarnation: u64,
    /// Last known state change timestamp (ms).
    pub last_state_change_ms: u64,
}

/// SWIM gossip protocol engine.
#[derive(Debug)]
pub struct GossipProtocol {
    /// This node's ID.
    pub self_id: DiscoveryNodeId,
    /// Membership list.
    pub members: HashMap<DiscoveryNodeId, SwimMember>,
    /// Protocol period in milliseconds.
    pub period_ms: u64,
    /// Number of indirect ping targets (k in SWIM).
    pub indirect_ping_count: usize,
    /// Suspicion timeout (multiples of protocol period).
    pub suspicion_mult: u32,
}

impl GossipProtocol {
    /// Creates a new gossip protocol instance.
    pub fn new(self_id: DiscoveryNodeId, period_ms: u64) -> Self {
        GossipProtocol {
            self_id,
            members: HashMap::new(),
            period_ms,
            indirect_ping_count: 3,
            suspicion_mult: 5,
        }
    }

    /// Adds a member to the membership list.
    pub fn add_member(&mut self, id: DiscoveryNodeId, address: &str, now_ms: u64) {
        self.members.insert(
            id,
            SwimMember {
                id,
                address: address.to_string(),
                state: SwimState::Alive,
                incarnation: 0,
                last_state_change_ms: now_ms,
            },
        );
    }

    /// Processes a direct ping response — marks node alive.
    pub fn ping_ack(&mut self, id: DiscoveryNodeId, incarnation: u64, now_ms: u64) {
        if let Some(member) = self.members.get_mut(&id) {
            if incarnation >= member.incarnation {
                member.state = SwimState::Alive;
                member.incarnation = incarnation;
                member.last_state_change_ms = now_ms;
            }
        }
    }

    /// Marks a node as suspected (no response to direct or indirect ping).
    pub fn suspect(&mut self, id: DiscoveryNodeId, now_ms: u64) {
        if let Some(member) = self.members.get_mut(&id) {
            if member.state == SwimState::Alive {
                member.state = SwimState::Suspect;
                member.last_state_change_ms = now_ms;
            }
        }
    }

    /// Marks a node as dead after suspicion timeout.
    pub fn declare_dead(&mut self, id: DiscoveryNodeId, now_ms: u64) {
        if let Some(member) = self.members.get_mut(&id) {
            member.state = SwimState::Dead;
            member.last_state_change_ms = now_ms;
        }
    }

    /// Checks for suspicion timeouts and transitions suspects to dead.
    pub fn check_timeouts(&mut self, now_ms: u64) -> Vec<DiscoveryNodeId> {
        let timeout_ms = self.period_ms * self.suspicion_mult as u64;
        let mut newly_dead = Vec::new();

        let suspects: Vec<(DiscoveryNodeId, u64)> = self
            .members
            .values()
            .filter(|m| m.state == SwimState::Suspect)
            .map(|m| (m.id, m.last_state_change_ms))
            .collect();

        for (id, changed) in suspects {
            if now_ms.saturating_sub(changed) >= timeout_ms {
                self.declare_dead(id, now_ms);
                newly_dead.push(id);
            }
        }
        newly_dead
    }

    /// Returns all alive members.
    pub fn alive_members(&self) -> Vec<&SwimMember> {
        self.members
            .values()
            .filter(|m| m.state == SwimState::Alive)
            .collect()
    }

    /// Returns the total membership count (all states).
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.5: Node Metadata
// ═══════════════════════════════════════════════════════════════════════

/// Metadata attached to a node for discovery purposes.
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    /// Node ID.
    pub id: DiscoveryNodeId,
    /// Key-value tags (e.g., "role"="worker", "gpu"="4").
    pub tags: HashMap<String, String>,
    /// Capabilities advertised by the node.
    pub capabilities: Vec<String>,
    /// Software version.
    pub version: String,
}

impl NodeMetadata {
    /// Creates new metadata with a version.
    pub fn new(id: DiscoveryNodeId, version: &str) -> Self {
        NodeMetadata {
            id,
            tags: HashMap::new(),
            capabilities: Vec::new(),
            version: version.to_string(),
        }
    }

    /// Adds a tag.
    pub fn tag(&mut self, key: &str, value: &str) {
        self.tags.insert(key.to_string(), value.to_string());
    }

    /// Adds a capability.
    pub fn add_capability(&mut self, cap: &str) {
        if !self.capabilities.contains(&cap.to_string()) {
            self.capabilities.push(cap.to_string());
        }
    }

    /// Checks if a node has a specific tag value.
    pub fn has_tag(&self, key: &str, value: &str) -> bool {
        self.tags.get(key).is_some_and(|v| v == value)
    }

    /// Checks if a node has a specific capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.6: Health Checking
// ═══════════════════════════════════════════════════════════════════════

/// Health check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Node is healthy.
    Healthy,
    /// Node is degraded but operational.
    Degraded,
    /// Node is unhealthy.
    Unhealthy,
    /// Health check timed out.
    Timeout,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "Healthy"),
            HealthStatus::Degraded => write!(f, "Degraded"),
            HealthStatus::Unhealthy => write!(f, "Unhealthy"),
            HealthStatus::Timeout => write!(f, "Timeout"),
        }
    }
}

/// A health check entry for a node.
#[derive(Debug, Clone)]
pub struct HealthCheckEntry {
    /// Node ID.
    pub node_id: DiscoveryNodeId,
    /// Last check status.
    pub status: HealthStatus,
    /// Consecutive failure count.
    pub consecutive_failures: u32,
    /// Last check timestamp (ms).
    pub last_check_ms: u64,
    /// Failure threshold before marking unhealthy.
    pub failure_threshold: u32,
}

/// Health checker that tracks node health across the cluster.
#[derive(Debug, Default)]
pub struct HealthChecker {
    /// Health entries per node.
    entries: HashMap<DiscoveryNodeId, HealthCheckEntry>,
    /// Default failure threshold.
    pub default_threshold: u32,
}

impl HealthChecker {
    /// Creates a new health checker.
    pub fn new(default_threshold: u32) -> Self {
        HealthChecker {
            entries: HashMap::new(),
            default_threshold,
        }
    }

    /// Registers a node for health checking.
    pub fn register(&mut self, node_id: DiscoveryNodeId) {
        self.entries.insert(
            node_id,
            HealthCheckEntry {
                node_id,
                status: HealthStatus::Healthy,
                consecutive_failures: 0,
                last_check_ms: 0,
                failure_threshold: self.default_threshold,
            },
        );
    }

    /// Records a successful health check.
    pub fn record_success(&mut self, node_id: DiscoveryNodeId, now_ms: u64) {
        if let Some(entry) = self.entries.get_mut(&node_id) {
            entry.status = HealthStatus::Healthy;
            entry.consecutive_failures = 0;
            entry.last_check_ms = now_ms;
        }
    }

    /// Records a failed health check.
    pub fn record_failure(&mut self, node_id: DiscoveryNodeId, now_ms: u64) {
        if let Some(entry) = self.entries.get_mut(&node_id) {
            entry.consecutive_failures += 1;
            entry.last_check_ms = now_ms;
            if entry.consecutive_failures >= entry.failure_threshold {
                entry.status = HealthStatus::Unhealthy;
            } else {
                entry.status = HealthStatus::Degraded;
            }
        }
    }

    /// Returns the current status of a node.
    pub fn status(&self, node_id: DiscoveryNodeId) -> Option<HealthStatus> {
        self.entries.get(&node_id).map(|e| e.status)
    }

    /// Returns all unhealthy nodes.
    pub fn unhealthy_nodes(&self) -> Vec<DiscoveryNodeId> {
        self.entries
            .values()
            .filter(|e| e.status == HealthStatus::Unhealthy)
            .map(|e| e.node_id)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.7: Auto-Scaling Events
// ═══════════════════════════════════════════════════════════════════════

/// An auto-scaling event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalingEvent {
    /// Scale up: add N nodes.
    ScaleUp { count: usize, reason: String },
    /// Scale down: remove N nodes.
    ScaleDown { count: usize, reason: String },
    /// Replace a failed node.
    Replace { failed_node: DiscoveryNodeId },
}

impl fmt::Display for ScalingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScalingEvent::ScaleUp { count, reason } => {
                write!(f, "ScaleUp({count}, {reason})")
            }
            ScalingEvent::ScaleDown { count, reason } => {
                write!(f, "ScaleDown({count}, {reason})")
            }
            ScalingEvent::Replace { failed_node } => {
                write!(f, "Replace({failed_node})")
            }
        }
    }
}

/// Auto-scaling policy configuration.
#[derive(Debug, Clone)]
pub struct ScalingPolicy {
    /// Minimum number of nodes.
    pub min_nodes: usize,
    /// Maximum number of nodes.
    pub max_nodes: usize,
    /// CPU utilization threshold to trigger scale-up (0.0-1.0).
    pub scale_up_threshold: f64,
    /// CPU utilization threshold to trigger scale-down (0.0-1.0).
    pub scale_down_threshold: f64,
    /// Cooldown period between scaling events (ms).
    pub cooldown_ms: u64,
    /// Last scaling event timestamp (ms).
    pub last_event_ms: u64,
}

impl ScalingPolicy {
    /// Evaluates whether a scaling event should occur.
    pub fn evaluate(
        &self,
        current_nodes: usize,
        avg_cpu_utilization: f64,
        now_ms: u64,
    ) -> Option<ScalingEvent> {
        // Respect cooldown.
        if now_ms.saturating_sub(self.last_event_ms) < self.cooldown_ms {
            return None;
        }

        if avg_cpu_utilization > self.scale_up_threshold && current_nodes < self.max_nodes {
            Some(ScalingEvent::ScaleUp {
                count: 1,
                reason: format!(
                    "CPU {:.0}% > {:.0}%",
                    avg_cpu_utilization * 100.0,
                    self.scale_up_threshold * 100.0
                ),
            })
        } else if avg_cpu_utilization < self.scale_down_threshold && current_nodes > self.min_nodes
        {
            Some(ScalingEvent::ScaleDown {
                count: 1,
                reason: format!(
                    "CPU {:.0}% < {:.0}%",
                    avg_cpu_utilization * 100.0,
                    self.scale_down_threshold * 100.0
                ),
            })
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.8: Service Registry
// ═══════════════════════════════════════════════════════════════════════

/// A registered service instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInstance {
    /// Service name.
    pub service_name: String,
    /// Instance ID.
    pub instance_id: String,
    /// Address (host:port).
    pub address: String,
    /// Health status.
    pub healthy: bool,
    /// Service metadata tags.
    pub tags: HashMap<String, String>,
}

/// Service registry for discovery.
#[derive(Debug, Default)]
pub struct DiscoveryRegistry {
    /// Registered service instances.
    instances: Vec<ServiceInstance>,
}

impl DiscoveryRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        DiscoveryRegistry::default()
    }

    /// Registers a service instance.
    pub fn register(&mut self, instance: ServiceInstance) {
        // Upsert by instance_id.
        self.instances
            .retain(|i| i.instance_id != instance.instance_id);
        self.instances.push(instance);
    }

    /// Deregisters a service instance by ID.
    pub fn deregister(&mut self, instance_id: &str) {
        self.instances.retain(|i| i.instance_id != instance_id);
    }

    /// Resolves a service name to healthy instances.
    pub fn resolve(&self, service_name: &str) -> Vec<&ServiceInstance> {
        self.instances
            .iter()
            .filter(|i| i.service_name == service_name && i.healthy)
            .collect()
    }

    /// Returns all instances (healthy or not) for a service.
    pub fn all_instances(&self, service_name: &str) -> Vec<&ServiceInstance> {
        self.instances
            .iter()
            .filter(|i| i.service_name == service_name)
            .collect()
    }

    /// Marks an instance as unhealthy.
    pub fn mark_unhealthy(&mut self, instance_id: &str) {
        if let Some(inst) = self
            .instances
            .iter_mut()
            .find(|i| i.instance_id == instance_id)
        {
            inst.healthy = false;
        }
    }

    /// Total registered instance count.
    pub fn total_count(&self) -> usize {
        self.instances.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D2.9: Discovery Configuration
// ═══════════════════════════════════════════════════════════════════════

/// Discovery method selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryMode {
    /// mDNS multicast on local network.
    Mdns,
    /// Seed node list.
    Seed,
    /// DNS SRV records.
    Dns,
    /// Gossip protocol.
    Gossip,
    /// Static node list.
    Static,
}

impl fmt::Display for DiscoveryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryMode::Mdns => write!(f, "mDNS"),
            DiscoveryMode::Seed => write!(f, "Seed"),
            DiscoveryMode::Dns => write!(f, "DNS"),
            DiscoveryMode::Gossip => write!(f, "Gossip"),
            DiscoveryMode::Static => write!(f, "Static"),
        }
    }
}

/// Full discovery configuration.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Primary discovery mode.
    pub mode: DiscoveryMode,
    /// Fallback mode (optional).
    pub fallback: Option<DiscoveryMode>,
    /// Refresh interval in milliseconds.
    pub refresh_interval_ms: u64,
    /// Whether to enable gossip protocol alongside primary mode.
    pub gossip_enabled: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            mode: DiscoveryMode::Gossip,
            fallback: Some(DiscoveryMode::Seed),
            refresh_interval_ms: 5000,
            gossip_enabled: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // D2.1 — mDNS Discovery
    #[test]
    fn d2_1_mdns_announce_and_query() {
        let mut mdns = MdnsDiscovery::new();
        mdns.announce(MdnsRecord {
            service_name: "_fj-ml._tcp.local.".into(),
            hostname: "worker-1".into(),
            port: 8080,
            ttl_secs: 120,
            txt: HashMap::new(),
        });
        mdns.announce(MdnsRecord {
            service_name: "_fj-ml._tcp.local.".into(),
            hostname: "worker-2".into(),
            port: 8080,
            ttl_secs: 120,
            txt: HashMap::new(),
        });
        let results = mdns.query("_fj-ml._tcp.local.");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn d2_1_mdns_dedup_by_hostname_port() {
        let mut mdns = MdnsDiscovery::new();
        let record = MdnsRecord {
            service_name: "_fj._tcp.local.".into(),
            hostname: "node-1".into(),
            port: 9090,
            ttl_secs: 60,
            txt: HashMap::new(),
        };
        mdns.announce(record.clone());
        mdns.announce(record);
        assert_eq!(mdns.all_records().len(), 1);
    }

    #[test]
    fn d2_1_mdns_expire() {
        let mut mdns = MdnsDiscovery::new();
        mdns.announce(MdnsRecord {
            service_name: "_fj._tcp.local.".into(),
            hostname: "short-lived".into(),
            port: 80,
            ttl_secs: 10,
            txt: HashMap::new(),
        });
        let mut times = HashMap::new();
        times.insert("short-lived".to_string(), 100);
        mdns.expire(&times, 200); // 100 seconds elapsed > 10 TTL
        assert!(mdns.all_records().is_empty());
    }

    // D2.2 — Seed Node Bootstrap
    #[test]
    fn d2_2_seed_join_success() {
        let config = SeedConfig {
            seeds: vec!["10.0.0.1:8080".into(), "10.0.0.2:8080".into()],
            max_attempts: 3,
            timeout_ms: 5000,
        };
        let reachable = vec!["10.0.0.2:8080".to_string()];
        let result = seed_bootstrap(&config, &reachable);
        assert_eq!(
            result,
            JoinResult::Joined {
                via_seed: "10.0.0.2:8080".into()
            }
        );
    }

    #[test]
    fn d2_2_seed_all_unreachable() {
        let config = SeedConfig {
            seeds: vec!["10.0.0.1:8080".into()],
            max_attempts: 3,
            timeout_ms: 5000,
        };
        let result = seed_bootstrap(&config, &[]);
        assert_eq!(result, JoinResult::AllSeedsUnreachable);
    }

    #[test]
    fn d2_2_join_result_display() {
        assert!(JoinResult::FormedNewCluster.to_string().contains("Formed"));
    }

    // D2.3 — DNS-Based Discovery
    #[test]
    fn d2_3_dns_srv_resolve() {
        let mut dns = DnsResolver::new();
        dns.add_srv(
            "_fj._tcp.cluster.local",
            DnsSrvRecord {
                target: "node-1.cluster.local".into(),
                port: 8080,
                priority: 10,
                weight: 50,
            },
        );
        dns.add_srv(
            "_fj._tcp.cluster.local",
            DnsSrvRecord {
                target: "node-2.cluster.local".into(),
                port: 8080,
                priority: 5,
                weight: 100,
            },
        );
        let results = dns.resolve_srv("_fj._tcp.cluster.local");
        assert_eq!(results.len(), 2);
        // Lower priority comes first.
        assert_eq!(results[0].target, "node-2.cluster.local");
    }

    #[test]
    fn d2_3_dns_resolve_missing() {
        let dns = DnsResolver::new();
        assert!(dns.resolve_srv("nonexistent").is_empty());
    }

    // D2.4 — Gossip Protocol
    #[test]
    fn d2_4_gossip_add_and_ping() {
        let mut gossip = GossipProtocol::new(DiscoveryNodeId(1), 1000);
        gossip.add_member(DiscoveryNodeId(2), "10.0.0.2:8080", 0);
        gossip.add_member(DiscoveryNodeId(3), "10.0.0.3:8080", 0);

        assert_eq!(gossip.member_count(), 2);
        assert_eq!(gossip.alive_members().len(), 2);

        gossip.suspect(DiscoveryNodeId(2), 5000);
        assert_eq!(gossip.alive_members().len(), 1);
    }

    #[test]
    fn d2_4_gossip_suspect_to_dead() {
        let mut gossip = GossipProtocol::new(DiscoveryNodeId(1), 1000);
        gossip.add_member(DiscoveryNodeId(2), "10.0.0.2:8080", 0);
        gossip.suspect(DiscoveryNodeId(2), 1000);

        // Not yet timed out (suspicion_mult * period = 5000ms).
        let dead = gossip.check_timeouts(4000);
        assert!(dead.is_empty());

        // Now timed out.
        let dead = gossip.check_timeouts(7000);
        assert_eq!(dead, vec![DiscoveryNodeId(2)]);
    }

    #[test]
    fn d2_4_gossip_ping_ack_revives() {
        let mut gossip = GossipProtocol::new(DiscoveryNodeId(1), 1000);
        gossip.add_member(DiscoveryNodeId(2), "10.0.0.2:8080", 0);
        gossip.suspect(DiscoveryNodeId(2), 1000);
        gossip.ping_ack(DiscoveryNodeId(2), 1, 2000);
        assert_eq!(gossip.alive_members().len(), 1);
    }

    // D2.5 — Node Metadata
    #[test]
    fn d2_5_node_metadata() {
        let mut meta = NodeMetadata::new(DiscoveryNodeId(1), "10.0.0");
        meta.tag("role", "worker");
        meta.tag("gpu", "4");
        meta.add_capability("cuda");
        meta.add_capability("tensor");

        assert!(meta.has_tag("role", "worker"));
        assert!(!meta.has_tag("role", "master"));
        assert!(meta.has_capability("cuda"));
        assert!(!meta.has_capability("tpu"));
    }

    #[test]
    fn d2_5_metadata_dedup_capabilities() {
        let mut meta = NodeMetadata::new(DiscoveryNodeId(1), "1.0");
        meta.add_capability("gpu");
        meta.add_capability("gpu");
        assert_eq!(meta.capabilities.len(), 1);
    }

    // D2.6 — Health Checking
    #[test]
    fn d2_6_health_check_success() {
        let mut hc = HealthChecker::new(3);
        hc.register(DiscoveryNodeId(1));
        hc.record_success(DiscoveryNodeId(1), 1000);
        assert_eq!(hc.status(DiscoveryNodeId(1)), Some(HealthStatus::Healthy));
    }

    #[test]
    fn d2_6_health_check_degraded_then_unhealthy() {
        let mut hc = HealthChecker::new(3);
        hc.register(DiscoveryNodeId(1));
        hc.record_failure(DiscoveryNodeId(1), 1000);
        assert_eq!(hc.status(DiscoveryNodeId(1)), Some(HealthStatus::Degraded));
        hc.record_failure(DiscoveryNodeId(1), 2000);
        hc.record_failure(DiscoveryNodeId(1), 3000);
        assert_eq!(hc.status(DiscoveryNodeId(1)), Some(HealthStatus::Unhealthy));
        assert_eq!(hc.unhealthy_nodes(), vec![DiscoveryNodeId(1)]);
    }

    #[test]
    fn d2_6_health_recovery() {
        let mut hc = HealthChecker::new(2);
        hc.register(DiscoveryNodeId(1));
        hc.record_failure(DiscoveryNodeId(1), 1000);
        hc.record_failure(DiscoveryNodeId(1), 2000);
        assert_eq!(hc.status(DiscoveryNodeId(1)), Some(HealthStatus::Unhealthy));
        hc.record_success(DiscoveryNodeId(1), 3000);
        assert_eq!(hc.status(DiscoveryNodeId(1)), Some(HealthStatus::Healthy));
    }

    // D2.7 — Auto-Scaling Events
    #[test]
    fn d2_7_scale_up_on_high_cpu() {
        let policy = ScalingPolicy {
            min_nodes: 1,
            max_nodes: 10,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.2,
            cooldown_ms: 5000,
            last_event_ms: 0,
        };
        let event = policy.evaluate(3, 0.9, 10000);
        assert!(matches!(event, Some(ScalingEvent::ScaleUp { .. })));
    }

    #[test]
    fn d2_7_scale_down_on_low_cpu() {
        let policy = ScalingPolicy {
            min_nodes: 1,
            max_nodes: 10,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.2,
            cooldown_ms: 5000,
            last_event_ms: 0,
        };
        let event = policy.evaluate(5, 0.1, 10000);
        assert!(matches!(event, Some(ScalingEvent::ScaleDown { .. })));
    }

    #[test]
    fn d2_7_cooldown_prevents_scaling() {
        let policy = ScalingPolicy {
            min_nodes: 1,
            max_nodes: 10,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.2,
            cooldown_ms: 5000,
            last_event_ms: 8000,
        };
        // 10000 - 8000 = 2000 < 5000 cooldown.
        let event = policy.evaluate(3, 0.95, 10000);
        assert!(event.is_none());
    }

    #[test]
    fn d2_7_scaling_event_display() {
        let event = ScalingEvent::Replace {
            failed_node: DiscoveryNodeId(5),
        };
        assert!(event.to_string().contains("Replace"));
    }

    // D2.8 — Service Registry
    #[test]
    fn d2_8_registry_register_and_resolve() {
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "ml-worker".into(),
            instance_id: "w1".into(),
            address: "10.0.0.1:8080".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.register(ServiceInstance {
            service_name: "ml-worker".into(),
            instance_id: "w2".into(),
            address: "10.0.0.2:8080".into(),
            healthy: false,
            tags: HashMap::new(),
        });
        let healthy = reg.resolve("ml-worker");
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].instance_id, "w1");

        let all = reg.all_instances("ml-worker");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn d2_8_registry_deregister() {
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            address: "addr".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.deregister("i1");
        assert_eq!(reg.total_count(), 0);
    }

    #[test]
    fn d2_8_registry_mark_unhealthy() {
        let mut reg = DiscoveryRegistry::new();
        reg.register(ServiceInstance {
            service_name: "svc".into(),
            instance_id: "i1".into(),
            address: "addr".into(),
            healthy: true,
            tags: HashMap::new(),
        });
        reg.mark_unhealthy("i1");
        assert!(reg.resolve("svc").is_empty());
    }

    // D2.9 — Discovery Configuration
    #[test]
    fn d2_9_discovery_config_default() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.mode, DiscoveryMode::Gossip);
        assert!(config.gossip_enabled);
        assert_eq!(config.fallback, Some(DiscoveryMode::Seed));
    }

    #[test]
    fn d2_9_discovery_mode_display() {
        assert_eq!(DiscoveryMode::Mdns.to_string(), "mDNS");
        assert_eq!(DiscoveryMode::Dns.to_string(), "DNS");
        assert_eq!(DiscoveryMode::Gossip.to_string(), "Gossip");
        assert_eq!(DiscoveryMode::Static.to_string(), "Static");
    }

    // D2.10 — Integration
    #[test]
    fn d2_10_discovery_node_id_display() {
        assert_eq!(DiscoveryNodeId(99).to_string(), "DNode(99)");
    }

    #[test]
    fn d2_10_swim_state_display() {
        assert_eq!(SwimState::Alive.to_string(), "Alive");
        assert_eq!(SwimState::Suspect.to_string(), "Suspect");
        assert_eq!(SwimState::Dead.to_string(), "Dead");
    }

    #[test]
    fn d2_10_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "Degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "Unhealthy");
        assert_eq!(HealthStatus::Timeout.to_string(), "Timeout");
    }
}
