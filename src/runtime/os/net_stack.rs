//! Advanced network stack for FajarOS Nova v2.0.
//!
//! Extends the basic TCP/UDP in `network.rs` with ARP, routing, DNS cache,
//! DHCP client, firewall, TCP congestion control, and socket management.
//! All structures are simulated in-memory — no real network I/O.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// MAC Address
// ═══════════════════════════════════════════════════════════════════════

/// A 6-byte Ethernet MAC address.
///
/// Supports formatting as colon-separated hex (`AA:BB:CC:DD:EE:FF`),
/// parsing from that format, and broadcast detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// The broadcast MAC address (`FF:FF:FF:FF:FF:FF`).
    pub const BROADCAST: Self = Self([0xFF; 6]);

    /// Creates a new MAC address from six bytes.
    pub fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Returns `true` if this is the broadcast address.
    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF; 6]
    }

    /// Returns `true` if the multicast bit (LSB of first byte) is set.
    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    /// Parses a MAC address from colon-separated hex (`"AA:BB:CC:DD:EE:FF"`).
    ///
    /// Returns `None` if the format is invalid.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return None;
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// Returns the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ARP Table
// ═══════════════════════════════════════════════════════════════════════

/// State of an ARP table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpState {
    /// ARP request sent, waiting for reply.
    Incomplete,
    /// Recently confirmed, considered valid.
    Reachable,
    /// Has not been re-confirmed within the timeout window.
    Stale,
}

/// A single ARP table entry mapping an IPv4 address to a MAC address.
#[derive(Debug, Clone)]
pub struct ArpEntry {
    /// IPv4 address (network byte order).
    pub ip: u32,
    /// Resolved MAC address.
    pub mac: MacAddress,
    /// Timestamp (seconds since boot) when this entry was last confirmed.
    pub timestamp: u64,
    /// Current state of the entry.
    pub state: ArpState,
}

/// ARP resolution table.
///
/// Maps IPv4 addresses to MAC addresses with TTL-based garbage collection.
#[derive(Debug)]
pub struct ArpTable {
    /// Entries keyed by IPv4 address.
    entries: HashMap<u32, ArpEntry>,
    /// Timeout in seconds before an entry transitions to `Stale`.
    pub timeout_secs: u64,
}

impl ArpTable {
    /// Creates an empty ARP table with the given timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            timeout_secs,
        }
    }

    /// Looks up the MAC address for an IPv4 address.
    pub fn lookup(&self, ip: u32) -> Option<&ArpEntry> {
        self.entries.get(&ip)
    }

    /// Inserts or updates an ARP entry as `Reachable`.
    pub fn insert(&mut self, ip: u32, mac: MacAddress, now: u64) {
        self.entries.insert(
            ip,
            ArpEntry {
                ip,
                mac,
                timestamp: now,
                state: ArpState::Reachable,
            },
        );
    }

    /// Inserts an incomplete entry (ARP request sent, no reply yet).
    pub fn insert_incomplete(&mut self, ip: u32, now: u64) {
        self.entries.insert(
            ip,
            ArpEntry {
                ip,
                mac: MacAddress::new([0; 6]),
                timestamp: now,
                state: ArpState::Incomplete,
            },
        );
    }

    /// Garbage-collects stale and expired entries.
    ///
    /// Entries older than `timeout_secs` are marked `Stale`.
    /// Entries older than `2 * timeout_secs` are removed entirely.
    pub fn gc(&mut self, now: u64) {
        let timeout = self.timeout_secs;
        self.entries.retain(|_, entry| {
            let age = now.saturating_sub(entry.timestamp);
            if age > timeout * 2 {
                return false; // remove
            }
            if age > timeout && entry.state == ArpState::Reachable {
                entry.state = ArpState::Stale;
            }
            true
        });
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IP Configuration
// ═══════════════════════════════════════════════════════════════════════

/// IP configuration for a network interface.
#[derive(Debug, Clone)]
pub struct IpConfig {
    /// IPv4 address.
    pub address: u32,
    /// Subnet mask.
    pub netmask: u32,
    /// Default gateway.
    pub gateway: u32,
    /// DNS server addresses.
    pub dns_servers: Vec<u32>,
}

impl IpConfig {
    /// Creates a new IP configuration.
    pub fn new(address: u32, netmask: u32, gateway: u32) -> Self {
        Self {
            address,
            netmask,
            gateway,
            dns_servers: Vec::new(),
        }
    }

    /// Returns the network address (address AND netmask).
    pub fn network(&self) -> u32 {
        self.address & self.netmask
    }

    /// Returns `true` if the given IP is on the same subnet.
    pub fn is_local(&self, ip: u32) -> bool {
        (ip & self.netmask) == self.network()
    }

    /// Formats an IPv4 u32 as a dotted-quad string.
    pub fn format_ip(ip: u32) -> String {
        format!(
            "{}.{}.{}.{}",
            (ip >> 24) & 0xFF,
            (ip >> 16) & 0xFF,
            (ip >> 8) & 0xFF,
            ip & 0xFF,
        )
    }

    /// Parses a dotted-quad string into an IPv4 u32.
    ///
    /// Returns `None` if the format is invalid.
    pub fn parse_ip(s: &str) -> Option<u32> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut ip: u32 = 0;
        for (i, part) in parts.iter().enumerate() {
            let octet: u32 = part.parse().ok()?;
            if octet > 255 {
                return None;
            }
            ip |= octet << (24 - i * 8);
        }
        Some(ip)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Routing Table
// ═══════════════════════════════════════════════════════════════════════

/// A single entry in the routing table.
#[derive(Debug, Clone)]
pub struct RoutingEntry {
    /// Destination network address.
    pub destination: u32,
    /// Subnet mask for the destination network.
    pub netmask: u32,
    /// Gateway IP (0 for directly connected).
    pub gateway: u32,
    /// Interface name (e.g. `"eth0"`).
    pub interface: String,
    /// Metric (lower is preferred).
    pub metric: u32,
}

/// IP routing table with longest-prefix-match lookup.
#[derive(Debug)]
pub struct RoutingTable {
    /// Routing entries.
    entries: Vec<RoutingEntry>,
}

impl RoutingTable {
    /// Creates an empty routing table.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds a routing entry.
    pub fn add(&mut self, entry: RoutingEntry) {
        self.entries.push(entry);
    }

    /// Removes all routes for the given interface.
    pub fn remove_interface(&mut self, interface: &str) {
        self.entries.retain(|e| e.interface != interface);
    }

    /// Looks up the best route for a destination IP using longest prefix match.
    ///
    /// Among routes with equal prefix length, the one with the lowest metric wins.
    pub fn lookup(&self, dest_ip: u32) -> Option<&RoutingEntry> {
        let mut best: Option<&RoutingEntry> = None;
        let mut best_prefix_len: u32 = 0;

        for entry in &self.entries {
            if (dest_ip & entry.netmask) == (entry.destination & entry.netmask) {
                let prefix_len = entry.netmask.count_ones();
                if best.is_none()
                    || prefix_len > best_prefix_len
                    || (prefix_len == best_prefix_len
                        && entry.metric < best.map(|b| b.metric).unwrap_or(u32::MAX))
                {
                    best = Some(entry);
                    best_prefix_len = prefix_len;
                }
            }
        }
        best
    }

    /// Returns the number of routes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the table has no routes.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TCP Congestion Control
// ═══════════════════════════════════════════════════════════════════════

/// TCP congestion control state (RFC 5681).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpCongestion {
    /// Exponential growth phase.
    SlowStart,
    /// Linear growth phase.
    CongestionAvoidance,
    /// Recovering from packet loss.
    FastRecovery,
}

/// TCP congestion control algorithm (Reno-like).
///
/// Tracks the congestion window (`cwnd`), slow-start threshold (`ssthresh`),
/// and smoothed RTT. Transitions between `SlowStart`, `CongestionAvoidance`,
/// and `FastRecovery` in response to ACKs, losses, and timeouts.
#[derive(Debug, Clone)]
pub struct TcpCongestionControl {
    /// Current congestion window (in segments).
    pub cwnd: u32,
    /// Slow-start threshold (in segments).
    pub ssthresh: u32,
    /// Smoothed round-trip time in milliseconds.
    pub rtt_ms: u32,
    /// Current congestion state.
    pub state: TcpCongestion,
    /// Maximum segment size.
    pub mss: u32,
    /// Duplicate ACK count (for fast retransmit).
    dup_ack_count: u32,
}

impl TcpCongestionControl {
    /// Initial congestion window size.
    const INITIAL_CWND: u32 = 10;
    /// Initial slow-start threshold.
    const INITIAL_SSTHRESH: u32 = 65535;

    /// Creates a new congestion controller with the given MSS.
    pub fn new(mss: u32) -> Self {
        Self {
            cwnd: Self::INITIAL_CWND,
            ssthresh: Self::INITIAL_SSTHRESH,
            rtt_ms: 0,
            state: TcpCongestion::SlowStart,
            mss,
            dup_ack_count: 0,
        }
    }

    /// Called when a new ACK is received (not a duplicate).
    ///
    /// In `SlowStart`, doubles `cwnd`. In `CongestionAvoidance`, increases
    /// `cwnd` by roughly 1 MSS per RTT. In `FastRecovery`, transitions
    /// back to `CongestionAvoidance`.
    pub fn on_ack(&mut self) {
        self.dup_ack_count = 0;
        match self.state {
            TcpCongestion::SlowStart => {
                self.cwnd = self.cwnd.saturating_add(self.mss);
                if self.cwnd >= self.ssthresh {
                    self.state = TcpCongestion::CongestionAvoidance;
                }
            }
            TcpCongestion::CongestionAvoidance => {
                // Approximately +1 MSS per RTT: cwnd += MSS * MSS / cwnd
                let increment = (self.mss as u64 * self.mss as u64) / self.cwnd.max(1) as u64;
                self.cwnd = self.cwnd.saturating_add(increment.max(1) as u32);
            }
            TcpCongestion::FastRecovery => {
                self.cwnd = self.ssthresh;
                self.state = TcpCongestion::CongestionAvoidance;
            }
        }
    }

    /// Called when a duplicate ACK is received (indicates possible loss).
    ///
    /// After 3 duplicate ACKs, enters `FastRecovery` and halves `ssthresh`.
    pub fn on_loss(&mut self) {
        self.dup_ack_count = self.dup_ack_count.saturating_add(1);
        if self.dup_ack_count >= 3 && self.state != TcpCongestion::FastRecovery {
            self.ssthresh = (self.cwnd / 2).max(2 * self.mss);
            self.cwnd = self.ssthresh.saturating_add(3 * self.mss);
            self.state = TcpCongestion::FastRecovery;
        }
    }

    /// Called when an RTO (retransmission timeout) fires.
    ///
    /// Resets to `SlowStart` with `cwnd = 1 MSS` and `ssthresh = cwnd/2`.
    pub fn on_timeout(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(2 * self.mss);
        self.cwnd = self.mss;
        self.dup_ack_count = 0;
        self.state = TcpCongestion::SlowStart;
    }

    /// Updates the smoothed RTT with a new sample using exponential moving average.
    pub fn update_rtt(&mut self, sample_ms: u32) {
        if self.rtt_ms == 0 {
            self.rtt_ms = sample_ms;
        } else {
            // SRTT = 7/8 * SRTT + 1/8 * sample
            self.rtt_ms = (self.rtt_ms * 7 + sample_ms) / 8;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DNS Cache
// ═══════════════════════════════════════════════════════════════════════

/// Cached DNS entry: resolved IP and expiry timestamp.
#[derive(Debug, Clone)]
struct DnsCacheEntry {
    /// Resolved IPv4 address.
    ip: u32,
    /// Absolute expiry time (seconds since boot).
    expires_at: u64,
}

/// A simple DNS resolution cache with TTL-based expiry.
///
/// Stores hostname-to-IP mappings and evicts expired entries on `gc()`.
#[derive(Debug)]
pub struct DnsCache {
    /// Entries keyed by hostname.
    entries: HashMap<String, DnsCacheEntry>,
}

impl DnsCache {
    /// Creates an empty DNS cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Resolves a hostname from cache, returning `None` if not cached or expired.
    pub fn resolve(&self, hostname: &str, now: u64) -> Option<u32> {
        self.entries.get(hostname).and_then(|entry| {
            if now < entry.expires_at {
                Some(entry.ip)
            } else {
                None
            }
        })
    }

    /// Inserts a DNS entry with the given TTL (in seconds).
    pub fn insert(&mut self, hostname: &str, ip: u32, ttl_secs: u64, now: u64) {
        self.entries.insert(
            hostname.to_string(),
            DnsCacheEntry {
                ip,
                expires_at: now.saturating_add(ttl_secs),
            },
        );
    }

    /// Removes all expired entries.
    pub fn gc(&mut self, now: u64) {
        self.entries.retain(|_, entry| now < entry.expires_at);
    }

    /// Returns the number of cached entries (including expired ones not yet GC'd).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DHCP Client
// ═══════════════════════════════════════════════════════════════════════

/// DHCP client state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpState {
    /// Initial state — ready to send DHCPDISCOVER.
    Discover,
    /// DHCPDISCOVER sent, waiting for DHCPOFFER.
    Offer,
    /// DHCPREQUEST sent, waiting for DHCPACK.
    Request,
    /// Lease acquired.
    Ack,
}

/// Simulated DHCP client for automatic IP configuration.
///
/// Implements the DHCP state machine: `Discover` → `Offer` → `Request` → `Ack`.
#[derive(Debug)]
pub struct DhcpClient {
    /// Current state.
    pub state: DhcpState,
    /// Transaction ID.
    pub xid: u32,
    /// Offered IP configuration (populated after `Offer`).
    offered_config: Option<IpConfig>,
    /// Final leased configuration (populated after `Ack`).
    leased_config: Option<IpConfig>,
    /// Lease duration in seconds.
    pub lease_secs: u64,
    /// Timestamp when the lease was acquired.
    pub lease_start: u64,
}

impl DhcpClient {
    /// Creates a new DHCP client with the given transaction ID.
    pub fn new(xid: u32) -> Self {
        Self {
            state: DhcpState::Discover,
            xid,
            offered_config: None,
            leased_config: None,
            lease_secs: 0,
            lease_start: 0,
        }
    }

    /// Initiates DHCP discovery. Transitions from `Discover` to `Offer` (waiting).
    ///
    /// Returns `true` if the transition was valid.
    pub fn start_discovery(&mut self) -> bool {
        if self.state != DhcpState::Discover {
            return false;
        }
        self.state = DhcpState::Offer;
        true
    }

    /// Handles a DHCPOFFER. Stores the offered config and transitions to `Request`.
    ///
    /// Returns `true` if the transition was valid.
    pub fn handle_offer(&mut self, config: IpConfig) -> bool {
        if self.state != DhcpState::Offer {
            return false;
        }
        self.offered_config = Some(config);
        self.state = DhcpState::Request;
        true
    }

    /// Handles a DHCPACK. Finalizes the lease and transitions to `Ack`.
    ///
    /// Returns the final `IpConfig`, or `None` if the state was invalid.
    pub fn handle_ack(&mut self, lease_secs: u64, now: u64) -> Option<IpConfig> {
        if self.state != DhcpState::Request {
            return None;
        }
        let config = self.offered_config.take()?;
        self.leased_config = Some(config.clone());
        self.lease_secs = lease_secs;
        self.lease_start = now;
        self.state = DhcpState::Ack;
        Some(config)
    }

    /// Returns the currently leased configuration, if any.
    pub fn leased_config(&self) -> Option<&IpConfig> {
        self.leased_config.as_ref()
    }

    /// Returns `true` if the lease has expired.
    pub fn is_lease_expired(&self, now: u64) -> bool {
        if self.state != DhcpState::Ack {
            return true;
        }
        now.saturating_sub(self.lease_start) >= self.lease_secs
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Firewall
// ═══════════════════════════════════════════════════════════════════════

/// Firewall rule action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallAction {
    /// Allow the packet.
    Allow,
    /// Silently drop the packet.
    Drop,
    /// Drop and send an error response.
    Reject,
}

/// Traffic direction for a firewall rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallDirection {
    /// Incoming traffic.
    In,
    /// Outgoing traffic.
    Out,
}

/// Protocol for a firewall rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallProtocol {
    /// Match TCP packets.
    Tcp,
    /// Match UDP packets.
    Udp,
    /// Match any protocol.
    Any,
}

/// A single firewall rule.
///
/// Rules are matched in order; the first matching rule determines the action.
/// Fields set to `None` or `0` are treated as wildcards.
#[derive(Debug, Clone)]
pub struct FirewallRule {
    /// Action to take when matched.
    pub action: FirewallAction,
    /// Traffic direction.
    pub direction: FirewallDirection,
    /// Protocol to match.
    pub protocol: FirewallProtocol,
    /// Source IP (0 = any).
    pub src_ip: u32,
    /// Destination IP (0 = any).
    pub dst_ip: u32,
    /// Port to match (0 = any).
    pub port: u16,
}

/// A packet-filtering firewall with ordered rules.
///
/// Rules are evaluated in insertion order. The first matching rule wins.
/// If no rule matches, the default action is `Allow`.
#[derive(Debug)]
pub struct Firewall {
    /// Ordered list of rules.
    rules: Vec<FirewallRule>,
    /// Default action when no rule matches.
    pub default_action: FirewallAction,
}

impl Firewall {
    /// Creates a new firewall with default-allow policy.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            default_action: FirewallAction::Allow,
        }
    }

    /// Creates a new firewall with default-drop policy.
    pub fn new_deny_all() -> Self {
        Self {
            rules: Vec::new(),
            default_action: FirewallAction::Drop,
        }
    }

    /// Appends a rule.
    pub fn add_rule(&mut self, rule: FirewallRule) {
        self.rules.push(rule);
    }

    /// Removes all rules.
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Checks a packet against the firewall rules.
    ///
    /// Returns `true` if the packet should be allowed, `false` if dropped/rejected.
    pub fn check_packet(
        &self,
        direction: FirewallDirection,
        protocol: FirewallProtocol,
        src_ip: u32,
        dst_ip: u32,
        port: u16,
    ) -> bool {
        for rule in &self.rules {
            if !Self::direction_matches(rule.direction, direction) {
                continue;
            }
            if !Self::protocol_matches(rule.protocol, protocol) {
                continue;
            }
            if rule.src_ip != 0 && rule.src_ip != src_ip {
                continue;
            }
            if rule.dst_ip != 0 && rule.dst_ip != dst_ip {
                continue;
            }
            if rule.port != 0 && rule.port != port {
                continue;
            }
            return rule.action == FirewallAction::Allow;
        }
        self.default_action == FirewallAction::Allow
    }

    /// Returns the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Checks if directions match.
    fn direction_matches(rule_dir: FirewallDirection, actual: FirewallDirection) -> bool {
        rule_dir == actual
    }

    /// Checks if protocols match (with wildcard support).
    fn protocol_matches(rule_proto: FirewallProtocol, actual: FirewallProtocol) -> bool {
        rule_proto == FirewallProtocol::Any || rule_proto == actual
    }
}

impl Default for Firewall {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Socket Handle
// ═══════════════════════════════════════════════════════════════════════

/// Transport protocol for a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    /// TCP stream.
    Tcp,
    /// UDP datagram.
    Udp,
}

/// State of a managed socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleState {
    /// Socket created but not connected.
    Open,
    /// Socket bound to local address.
    Bound,
    /// TCP socket is listening.
    Listening,
    /// Socket is connected.
    Connected,
    /// Socket is closed.
    Closed,
}

/// A managed socket handle with metadata.
#[derive(Debug, Clone)]
pub struct SocketHandle {
    /// Unique socket identifier.
    pub id: u64,
    /// Transport protocol.
    pub protocol: SocketProtocol,
    /// Local address (IP:port) packed as `(ip, port)`.
    pub local_addr: (u32, u16),
    /// Remote address (IP:port) packed as `(ip, port)`.
    pub remote_addr: (u32, u16),
    /// Current state.
    pub state: HandleState,
}

impl SocketHandle {
    /// Creates a new socket handle.
    pub fn new(id: u64, protocol: SocketProtocol) -> Self {
        Self {
            id,
            protocol,
            local_addr: (0, 0),
            remote_addr: (0, 0),
            state: HandleState::Open,
        }
    }

    /// Binds the socket to a local address.
    pub fn bind(&mut self, ip: u32, port: u16) {
        self.local_addr = (ip, port);
        self.state = HandleState::Bound;
    }

    /// Marks the socket as connected to a remote address.
    pub fn connect(&mut self, ip: u32, port: u16) {
        self.remote_addr = (ip, port);
        self.state = HandleState::Connected;
    }

    /// Marks the socket as listening.
    pub fn listen(&mut self) {
        self.state = HandleState::Listening;
    }

    /// Closes the socket.
    pub fn close(&mut self) {
        self.state = HandleState::Closed;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Statistics
// ═══════════════════════════════════════════════════════════════════════

/// Aggregate network statistics counters.
#[derive(Debug, Clone, Default)]
pub struct NetStats {
    /// Total packets received.
    pub packets_rx: u64,
    /// Total packets transmitted.
    pub packets_tx: u64,
    /// Total bytes received.
    pub bytes_rx: u64,
    /// Total bytes transmitted.
    pub bytes_tx: u64,
    /// Total error count (checksum failures, drops, etc.).
    pub errors: u64,
    /// Number of currently active connections.
    pub connections_active: u64,
}

impl NetStats {
    /// Creates zeroed statistics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a received packet.
    pub fn record_rx(&mut self, bytes: u64) {
        self.packets_rx = self.packets_rx.saturating_add(1);
        self.bytes_rx = self.bytes_rx.saturating_add(bytes);
    }

    /// Records a transmitted packet.
    pub fn record_tx(&mut self, bytes: u64) {
        self.packets_tx = self.packets_tx.saturating_add(1);
        self.bytes_tx = self.bytes_tx.saturating_add(bytes);
    }

    /// Records an error.
    pub fn record_error(&mut self) {
        self.errors = self.errors.saturating_add(1);
    }

    /// Resets all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl fmt::Display for NetStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rx: {} pkts/{} bytes, tx: {} pkts/{} bytes, err: {}, active: {}",
            self.packets_rx,
            self.bytes_rx,
            self.packets_tx,
            self.bytes_tx,
            self.errors,
            self.connections_active,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── MacAddress ──

    #[test]
    fn mac_display_format() {
        let mac = MacAddress::new([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(mac.to_string(), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn mac_parse_valid() {
        let mac = MacAddress::parse("01:23:45:67:89:AB");
        assert!(mac.is_some());
        assert_eq!(mac.unwrap().0, [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB]);
    }

    #[test]
    fn mac_parse_invalid() {
        assert!(MacAddress::parse("not-a-mac").is_none());
        assert!(MacAddress::parse("01:02:03").is_none());
    }

    #[test]
    fn mac_broadcast_detection() {
        assert!(MacAddress::BROADCAST.is_broadcast());
        assert!(!MacAddress::new([0x01, 0, 0, 0, 0, 0]).is_broadcast());
    }

    // ── ARP ──

    #[test]
    fn arp_insert_and_lookup() {
        let mut table = ArpTable::new(60);
        let mac = MacAddress::new([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
        table.insert(0xC0A80001, mac, 100);
        let entry = table.lookup(0xC0A80001);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().mac, mac);
        assert_eq!(entry.unwrap().state, ArpState::Reachable);
    }

    #[test]
    fn arp_gc_expires_stale() {
        let mut table = ArpTable::new(10);
        let mac = MacAddress::new([0x01; 6]);
        table.insert(0x0A000001, mac, 0);
        // At t=11, should be stale
        table.gc(11);
        assert_eq!(table.lookup(0x0A000001).unwrap().state, ArpState::Stale);
        // At t=21, should be removed
        table.gc(21);
        assert!(table.lookup(0x0A000001).is_none());
    }

    // ── IpConfig ──

    #[test]
    fn ip_config_is_local() {
        let cfg = IpConfig::new(0xC0A80164, 0xFFFFFF00, 0xC0A80101); // 192.168.1.100/24
        assert!(cfg.is_local(0xC0A801FE)); // 192.168.1.254
        assert!(!cfg.is_local(0xC0A80201)); // 192.168.2.1
    }

    #[test]
    fn ip_format_and_parse() {
        let ip = IpConfig::parse_ip("10.0.0.1");
        assert_eq!(ip, Some(0x0A000001));
        assert_eq!(IpConfig::format_ip(0x0A000001), "10.0.0.1");
    }

    // ── Routing ──

    #[test]
    fn routing_longest_prefix_match() {
        let mut rt = RoutingTable::new();
        rt.add(RoutingEntry {
            destination: 0,
            netmask: 0,
            gateway: 0xC0A80101,
            interface: "eth0".into(),
            metric: 100,
        });
        rt.add(RoutingEntry {
            destination: 0xC0A80100,
            netmask: 0xFFFFFF00,
            gateway: 0,
            interface: "eth0".into(),
            metric: 0,
        });
        // 192.168.1.50 should match /24 route (longer prefix)
        let route = rt.lookup(0xC0A80132);
        assert!(route.is_some());
        assert_eq!(route.unwrap().netmask, 0xFFFFFF00);
        // 10.0.0.1 should match default route
        let route = rt.lookup(0x0A000001);
        assert!(route.is_some());
        assert_eq!(route.unwrap().netmask, 0);
    }

    // ── TCP Congestion ──

    #[test]
    fn congestion_slow_start_to_avoidance() {
        let mut cc = TcpCongestionControl::new(1);
        assert_eq!(cc.state, TcpCongestion::SlowStart);
        cc.ssthresh = 12;
        // ACK until cwnd >= ssthresh
        for _ in 0..20 {
            cc.on_ack();
        }
        assert_eq!(cc.state, TcpCongestion::CongestionAvoidance);
    }

    #[test]
    fn congestion_timeout_resets() {
        let mut cc = TcpCongestionControl::new(1);
        cc.cwnd = 20;
        cc.on_timeout();
        assert_eq!(cc.cwnd, 1); // reset to 1 MSS
        assert_eq!(cc.state, TcpCongestion::SlowStart);
    }

    #[test]
    fn congestion_fast_recovery_on_triple_dup_ack() {
        let mut cc = TcpCongestionControl::new(1);
        cc.cwnd = 20;
        cc.on_loss();
        cc.on_loss();
        cc.on_loss();
        assert_eq!(cc.state, TcpCongestion::FastRecovery);
    }

    // ── DNS Cache ──

    #[test]
    fn dns_cache_resolve_and_expiry() {
        let mut cache = DnsCache::new();
        cache.insert("example.com", 0x08080808, 60, 100);
        assert_eq!(cache.resolve("example.com", 150), Some(0x08080808));
        assert_eq!(cache.resolve("example.com", 161), None); // expired
    }

    #[test]
    fn dns_cache_gc() {
        let mut cache = DnsCache::new();
        cache.insert("a.com", 1, 10, 0);
        cache.insert("b.com", 2, 100, 0);
        cache.gc(50);
        assert_eq!(cache.len(), 1); // a.com removed
    }

    // ── DHCP ──

    #[test]
    fn dhcp_full_state_machine() {
        let mut client = DhcpClient::new(0x1234);
        assert_eq!(client.state, DhcpState::Discover);
        assert!(client.start_discovery());
        assert_eq!(client.state, DhcpState::Offer);
        let config = IpConfig::new(0xC0A80164, 0xFFFFFF00, 0xC0A80101);
        assert!(client.handle_offer(config));
        assert_eq!(client.state, DhcpState::Request);
        let result = client.handle_ack(3600, 1000);
        assert!(result.is_some());
        assert_eq!(client.state, DhcpState::Ack);
        assert!(!client.is_lease_expired(2000));
        assert!(client.is_lease_expired(5000));
    }

    // ── Firewall ──

    #[test]
    fn firewall_allow_rule() {
        let mut fw = Firewall::new_deny_all();
        fw.add_rule(FirewallRule {
            action: FirewallAction::Allow,
            direction: FirewallDirection::In,
            protocol: FirewallProtocol::Tcp,
            src_ip: 0,
            dst_ip: 0,
            port: 80,
        });
        assert!(fw.check_packet(FirewallDirection::In, FirewallProtocol::Tcp, 1, 2, 80));
        assert!(!fw.check_packet(FirewallDirection::In, FirewallProtocol::Tcp, 1, 2, 443));
    }

    #[test]
    fn firewall_drop_rule() {
        let mut fw = Firewall::new();
        fw.add_rule(FirewallRule {
            action: FirewallAction::Drop,
            direction: FirewallDirection::Out,
            protocol: FirewallProtocol::Any,
            src_ip: 0,
            dst_ip: 0x0A000001,
            port: 0,
        });
        assert!(!fw.check_packet(
            FirewallDirection::Out,
            FirewallProtocol::Tcp,
            0xC0A80101,
            0x0A000001,
            22,
        ));
        // Different destination should be allowed (default allow)
        assert!(fw.check_packet(
            FirewallDirection::Out,
            FirewallProtocol::Tcp,
            0xC0A80101,
            0x08080808,
            22,
        ));
    }

    // ── Socket Handle ──

    #[test]
    fn socket_handle_lifecycle() {
        let mut sock = SocketHandle::new(1, SocketProtocol::Tcp);
        assert_eq!(sock.state, HandleState::Open);
        sock.bind(0, 8080);
        assert_eq!(sock.state, HandleState::Bound);
        sock.connect(0xC0A80101, 80);
        assert_eq!(sock.state, HandleState::Connected);
        sock.close();
        assert_eq!(sock.state, HandleState::Closed);
    }

    // ── NetStats ──

    #[test]
    fn net_stats_record() {
        let mut stats = NetStats::new();
        stats.record_rx(1500);
        stats.record_rx(800);
        stats.record_tx(200);
        stats.record_error();
        assert_eq!(stats.packets_rx, 2);
        assert_eq!(stats.bytes_rx, 2300);
        assert_eq!(stats.packets_tx, 1);
        assert_eq!(stats.bytes_tx, 200);
        assert_eq!(stats.errors, 1);
    }
}
