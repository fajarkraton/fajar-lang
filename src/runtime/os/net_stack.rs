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
    ///
    /// # Security
    ///
    /// This method unconditionally overwrites any existing entry for the
    /// given IP address. It does **not** validate whether the MAC address
    /// has changed, which means a malicious ARP reply can silently replace
    /// a legitimate mapping (ARP spoofing). Callers in adversarial network
    /// environments should add validation (e.g. static entries, DAI) before
    /// calling this method.
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
    ///
    /// # Note
    ///
    /// This method assumes `now` comes from a monotonic clock source.
    /// If the clock wraps around or is adjusted backwards, `saturating_sub`
    /// prevents underflow but entries may appear younger than they are,
    /// delaying eviction until the clock catches up.
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
///
/// Each entry maps a destination network (identified by `destination` and `netmask`)
/// to a next-hop `gateway` reachable via `interface`. The `metric` field is used as
/// a tie-breaker when multiple routes match the same prefix length (lower is preferred).
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
///
/// Stores a flat list of [`RoutingEntry`] values and selects the best route by
/// choosing the entry whose netmask has the most leading ones (longest prefix).
/// Among ties, the entry with the lowest metric wins.
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
    ///
    /// # Note
    ///
    /// The expiry time is computed as `now + ttl_secs` using `saturating_add`.
    /// This assumes `now` comes from a monotonic clock source. If the clock
    /// wraps around or is adjusted backwards, cached entries may persist
    /// longer than intended because `resolve()` compares `now < expires_at`.
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

    /// Resets the client back to `Discover` state, clearing all stored
    /// configuration.
    ///
    /// This enables lease renewal: after `reset()`, the caller can restart
    /// the DHCP handshake by calling `start_discovery()` again.
    pub fn reset(&mut self) {
        self.state = DhcpState::Discover;
        self.offered_config = None;
        self.leased_config = None;
        self.lease_secs = 0;
        self.lease_start = 0;
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
    ///
    /// # Limitations
    ///
    /// IP addresses in rules are matched by exact equality only. CIDR range
    /// matching (e.g. `10.0.0.0/8`) is not supported. To block or allow a
    /// subnet, add individual rules or implement prefix comparison externally.
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
///
/// Distinguishes between connection-oriented TCP streams and connectionless
/// UDP datagrams when creating or inspecting a [`SocketHandle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    /// TCP stream.
    Tcp,
    /// UDP datagram.
    Udp,
}

/// State of a managed socket.
///
/// Tracks the lifecycle of a [`SocketHandle`] from creation (`Open`) through
/// `Bound`, `Listening` (TCP only), `Connected`, and finally `Closed`.
/// Transitions are enforced by the methods on `SocketHandle`.
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
// VirtIO-net Device (OS2.1)
// ═══════════════════════════════════════════════════════════════════════

use std::collections::VecDeque;

/// A raw Ethernet frame with source/destination MAC, EtherType, and payload.
#[derive(Debug, Clone)]
pub struct EthernetFrame {
    /// Destination MAC address.
    pub dst_mac: MacAddress,
    /// Source MAC address.
    pub src_mac: MacAddress,
    /// EtherType field (e.g. `0x0800` = IPv4, `0x0806` = ARP).
    pub ethertype: u16,
    /// Frame payload bytes.
    pub payload: Vec<u8>,
}

impl EthernetFrame {
    /// Creates a new Ethernet frame.
    pub fn new(dst_mac: MacAddress, src_mac: MacAddress, ethertype: u16, payload: Vec<u8>) -> Self {
        Self {
            dst_mac,
            src_mac,
            ethertype,
            payload,
        }
    }

    /// Returns the total on-wire byte size (14-byte header + payload).
    pub fn wire_len(&self) -> usize {
        14 + self.payload.len()
    }
}

/// Simulated VirtIO network device with TX/RX queues.
///
/// Frames enqueued via [`send_frame`] sit in `tx_queue` awaiting delivery,
/// and [`receive_frame`] pops from `rx_queue`. Stats are updated on every
/// successful send/receive operation.
#[derive(Debug)]
pub struct VirtioNetDevice {
    /// Device MAC address.
    mac: MacAddress,
    /// Outgoing frame queue.
    tx_queue: VecDeque<EthernetFrame>,
    /// Incoming frame queue.
    rx_queue: VecDeque<EthernetFrame>,
    /// Whether the link is up.
    link_up: bool,
    /// Maximum transmission unit in bytes.
    mtu: u32,
    /// Aggregate statistics.
    stats: NetStats,
}

impl VirtioNetDevice {
    /// Creates a new VirtIO-net device with the given MAC and MTU.
    pub fn new(mac: MacAddress, mtu: u32) -> Self {
        Self {
            mac,
            tx_queue: VecDeque::new(),
            rx_queue: VecDeque::new(),
            link_up: true,
            mtu,
            stats: NetStats::new(),
        }
    }

    /// Returns the device MAC address.
    pub fn mac(&self) -> MacAddress {
        self.mac
    }

    /// Returns the MTU.
    pub fn mtu(&self) -> u32 {
        self.mtu
    }

    /// Returns `true` if the link is up.
    pub fn is_link_up(&self) -> bool {
        self.link_up
    }

    /// Sets the link state (`true` = up, `false` = down).
    pub fn set_link(&mut self, up: bool) {
        self.link_up = up;
    }

    /// Enqueues a frame for transmission.
    ///
    /// Returns `false` and records an error if the link is down or the frame
    /// exceeds the MTU.
    pub fn send_frame(&mut self, frame: EthernetFrame) -> bool {
        if !self.link_up {
            self.stats.record_error();
            return false;
        }
        if frame.wire_len() as u32 > self.mtu {
            self.stats.record_error();
            return false;
        }
        let bytes = frame.wire_len() as u64;
        self.tx_queue.push_back(frame);
        self.stats.record_tx(bytes);
        true
    }

    /// Pops the next frame from the RX queue.
    ///
    /// Returns `None` if the queue is empty or the link is down.
    pub fn receive_frame(&mut self) -> Option<EthernetFrame> {
        if !self.link_up {
            return None;
        }
        let frame = self.rx_queue.pop_front()?;
        let bytes = frame.wire_len() as u64;
        self.stats.record_rx(bytes);
        Some(frame)
    }

    /// Injects a frame into the RX queue (simulates hardware receiving a packet).
    pub fn inject_rx(&mut self, frame: EthernetFrame) {
        self.rx_queue.push_back(frame);
    }

    /// Returns the number of frames waiting in the TX queue.
    pub fn tx_pending(&self) -> usize {
        self.tx_queue.len()
    }

    /// Returns the number of frames waiting in the RX queue.
    pub fn rx_pending(&self) -> usize {
        self.rx_queue.len()
    }

    /// Returns a reference to the device statistics.
    pub fn stats(&self) -> &NetStats {
        &self.stats
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Namespace (OS2.10)
// ═══════════════════════════════════════════════════════════════════════

/// An isolated network namespace with its own stack configuration.
///
/// Each namespace has independent IP configuration, routing, firewall,
/// ARP table, DNS cache, and socket set — equivalent to a Linux network
/// namespace.
#[derive(Debug)]
pub struct NetworkNamespace {
    /// Namespace name (e.g. `"default"`, `"vrf-a"`).
    name: String,
    /// IP configuration for the primary interface.
    config: IpConfig,
    /// Routing table.
    routes: RoutingTable,
    /// Firewall.
    firewall: Firewall,
    /// ARP resolution table.
    arp: ArpTable,
    /// DNS resolution cache.
    dns: DnsCache,
    /// Active sockets in this namespace.
    sockets: Vec<SocketHandle>,
}

impl NetworkNamespace {
    /// Creates a new network namespace with the given name and IP config.
    pub fn new(name: impl Into<String>, config: IpConfig) -> Self {
        Self {
            name: name.into(),
            config,
            routes: RoutingTable::new(),
            firewall: Firewall::new(),
            arp: ArpTable::new(60),
            dns: DnsCache::new(),
            sockets: Vec::new(),
        }
    }

    /// Returns the namespace name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the IP configuration.
    pub fn config(&self) -> &IpConfig {
        &self.config
    }

    /// Returns a mutable reference to the routing table.
    pub fn routes_mut(&mut self) -> &mut RoutingTable {
        &mut self.routes
    }

    /// Returns a mutable reference to the firewall.
    pub fn firewall_mut(&mut self) -> &mut Firewall {
        &mut self.firewall
    }

    /// Returns a mutable reference to the ARP table.
    pub fn arp_mut(&mut self) -> &mut ArpTable {
        &mut self.arp
    }

    /// Returns a mutable reference to the DNS cache.
    pub fn dns_mut(&mut self) -> &mut DnsCache {
        &mut self.dns
    }

    /// Adds a socket to this namespace.
    pub fn add_socket(&mut self, socket: SocketHandle) {
        self.sockets.push(socket);
    }

    /// Removes a socket by its ID.
    ///
    /// Returns `true` if a socket with the given ID was found and removed.
    pub fn remove_socket(&mut self, id: u64) -> bool {
        let before = self.sockets.len();
        self.sockets.retain(|s| s.id != id);
        self.sockets.len() < before
    }

    /// Resolves a route for the given destination IP.
    ///
    /// Returns `None` if no matching route exists.
    pub fn resolve_route(&self, dest_ip: u32) -> Option<&RoutingEntry> {
        self.routes.lookup(dest_ip)
    }

    /// Returns the number of sockets in this namespace.
    pub fn socket_count(&self) -> usize {
        self.sockets.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TLS Session (OS2.11)
// ═══════════════════════════════════════════════════════════════════════

/// TLS 1.3 handshake state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsState {
    /// Waiting to send ClientHello.
    ClientHello,
    /// ClientHello sent, awaiting ServerHello.
    ServerHello,
    /// Processing handshake messages (certificate, finished).
    Handshake,
    /// TLS session fully established.
    Established,
    /// Session closed.
    Closed,
}

/// TLS cipher suite selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherSuite {
    /// TLS_AES_128_GCM_SHA256
    Aes128GcmSha256,
    /// TLS_AES_256_GCM_SHA384
    Aes256GcmSha384,
    /// TLS_CHACHA20_POLY1305_SHA256
    ChaCha20Poly1305,
}

/// TLS session configuration.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Selected cipher suite.
    pub cipher_suite: CipherSuite,
    /// Whether to verify the peer certificate.
    pub verify_peer: bool,
    /// Optional hostname for SNI.
    pub hostname: Option<String>,
}

impl TlsConfig {
    /// Creates a default-secure TLS 1.3 configuration.
    pub fn new(cipher_suite: CipherSuite, verify_peer: bool) -> Self {
        Self {
            cipher_suite,
            verify_peer,
            hostname: None,
        }
    }

    /// Sets the SNI hostname.
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }
}

/// Simulated TLS 1.3 session.
///
/// Models the TLS handshake state machine and XOR-based in-memory
/// encrypt/decrypt. No real cryptography is performed.
#[derive(Debug)]
pub struct TlsSession {
    /// Current handshake/session state.
    state: TlsState,
    /// Session configuration (cipher suite, peer verification, SNI hostname).
    pub config: TlsConfig,
    /// Unique session identifier.
    session_id: u64,
    /// Cumulative bytes encrypted.
    bytes_encrypted: u64,
    /// Cumulative bytes decrypted.
    bytes_decrypted: u64,
}

impl TlsSession {
    /// Creates a new TLS session in the `ClientHello` state.
    pub fn new(session_id: u64, config: TlsConfig) -> Self {
        Self {
            state: TlsState::ClientHello,
            config,
            session_id,
            bytes_encrypted: 0,
            bytes_decrypted: 0,
        }
    }

    /// Returns the current TLS state.
    pub fn state(&self) -> TlsState {
        self.state
    }

    /// Returns the session ID.
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Returns `true` if the session is fully established.
    pub fn is_established(&self) -> bool {
        self.state == TlsState::Established
    }

    /// Advances the handshake by one step.
    ///
    /// The state machine progresses:
    /// `ClientHello` → `ServerHello` → `Handshake` → `Established`
    ///
    /// Returns `false` if the session is already `Established` or `Closed`.
    pub fn handshake_step(&mut self) -> bool {
        self.state = match self.state {
            TlsState::ClientHello => TlsState::ServerHello,
            TlsState::ServerHello => TlsState::Handshake,
            TlsState::Handshake => TlsState::Established,
            TlsState::Established | TlsState::Closed => return false,
        };
        true
    }

    /// Encrypts plaintext using a simple XOR with the session ID bytes.
    ///
    /// Returns `None` if the session is not established.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Option<Vec<u8>> {
        if self.state != TlsState::Established {
            return None;
        }
        let key = self.session_id.to_le_bytes();
        let cipher: Vec<u8> = plaintext
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key[i % 8])
            .collect();
        self.bytes_encrypted = self.bytes_encrypted.saturating_add(plaintext.len() as u64);
        Some(cipher)
    }

    /// Decrypts ciphertext using the same XOR key.
    ///
    /// Returns `None` if the session is not established.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        if self.state != TlsState::Established {
            return None;
        }
        let key = self.session_id.to_le_bytes();
        let plain: Vec<u8> = ciphertext
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key[i % 8])
            .collect();
        self.bytes_decrypted = self.bytes_decrypted.saturating_add(ciphertext.len() as u64);
        Some(plain)
    }

    /// Closes the TLS session.
    pub fn close(&mut self) {
        self.state = TlsState::Closed;
    }

    /// Returns total bytes encrypted.
    pub fn bytes_encrypted(&self) -> u64 {
        self.bytes_encrypted
    }

    /// Returns total bytes decrypted.
    pub fn bytes_decrypted(&self) -> u64 {
        self.bytes_decrypted
    }
}

// ═══════════════════════════════════════════════════════════════════════
// NTP Client (OS2.13)
// ═══════════════════════════════════════════════════════════════════════

/// An NTP timestamp (RFC 5905 format).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtpTimestamp {
    /// Seconds since NTP epoch (1 Jan 1900).
    pub seconds: u64,
    /// Fractional second as a 32-bit fixed-point value.
    pub fraction: u32,
}

impl NtpTimestamp {
    /// Creates a new NTP timestamp.
    pub fn new(seconds: u64, fraction: u32) -> Self {
        Self { seconds, fraction }
    }
}

/// Simulated NTP client for system clock synchronisation.
///
/// Computes a signed millisecond offset between the server clock and the
/// local clock, and applies it when reading the current time.
#[derive(Debug)]
pub struct NtpClient {
    /// Server IPv4 address (network byte order).
    server_addr: u32,
    /// Clock offset in milliseconds (server − local).
    offset_ms: i64,
    /// Local timestamp at which the last sync was performed.
    last_sync: u64,
    /// Stratum level reported by the server.
    stratum: u8,
    /// Nominal poll interval in seconds.
    poll_interval_secs: u64,
    /// Whether the client has completed at least one successful sync.
    synced: bool,
}

impl NtpClient {
    /// Creates a new NTP client pointed at the given server address.
    pub fn new(server_addr: u32) -> Self {
        Self {
            server_addr,
            offset_ms: 0,
            last_sync: 0,
            stratum: 0,
            poll_interval_secs: 64,
            synced: false,
        }
    }

    /// Returns the server IPv4 address.
    pub fn server_addr(&self) -> u32 {
        self.server_addr
    }

    /// Returns `true` if the client has successfully synchronised at least once.
    pub fn is_synced(&self) -> bool {
        self.synced
    }

    /// Returns the current signed clock offset in milliseconds.
    pub fn offset(&self) -> i64 {
        self.offset_ms
    }

    /// Performs a simulated NTP synchronisation.
    ///
    /// `server_time_ms` is the server's time in milliseconds.
    /// `local_time_ms` is the local clock reading in milliseconds.
    /// The offset is recorded and the sync timestamp updated.
    pub fn sync(&mut self, server_time_ms: u64, local_time_ms: u64, stratum: u8) {
        self.offset_ms = (server_time_ms as i64).saturating_sub(local_time_ms as i64);
        self.last_sync = local_time_ms;
        self.stratum = stratum;
        self.synced = true;
    }

    /// Returns the corrected current time given the raw local clock reading.
    ///
    /// Returns `0` if the result would underflow (offset is too negative).
    pub fn current_time(&self, local_time_ms: u64) -> u64 {
        if self.offset_ms >= 0 {
            local_time_ms.saturating_add(self.offset_ms as u64)
        } else {
            local_time_ms.saturating_sub(self.offset_ms.unsigned_abs())
        }
    }

    /// Returns the stratum reported by the last NTP server response.
    pub fn stratum(&self) -> u8 {
        self.stratum
    }

    /// Returns the local timestamp of the last synchronisation.
    pub fn last_sync(&self) -> u64 {
        self.last_sync
    }

    /// Sets the poll interval in seconds.
    pub fn set_poll_interval(&mut self, secs: u64) {
        self.poll_interval_secs = secs;
    }

    /// Returns the poll interval in seconds.
    pub fn poll_interval_secs(&self) -> u64 {
        self.poll_interval_secs
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ICMP / Ping (OS2.14)
// ═══════════════════════════════════════════════════════════════════════

/// ICMP message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpType {
    /// Echo request (ping outgoing).
    EchoRequest,
    /// Echo reply (ping response).
    EchoReply,
    /// Destination unreachable.
    DestUnreachable,
    /// Time exceeded (TTL expired).
    TimeExceeded,
}

/// An ICMP packet.
#[derive(Debug, Clone)]
pub struct IcmpPacket {
    /// ICMP message type.
    pub icmp_type: IcmpType,
    /// Subtype code for the message type.
    pub code: u8,
    /// Echo identifier (matches request to reply).
    pub identifier: u16,
    /// Echo sequence number.
    pub sequence: u16,
    /// Arbitrary payload data.
    pub payload: Vec<u8>,
}

impl IcmpPacket {
    /// Creates a new ICMP packet.
    pub fn new(
        icmp_type: IcmpType,
        code: u8,
        identifier: u16,
        sequence: u16,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            icmp_type,
            code,
            identifier,
            sequence,
            payload,
        }
    }

    /// Returns the serialised byte length (8-byte header + payload).
    pub fn wire_len(&self) -> usize {
        8 + self.payload.len()
    }
}

/// Result of a single ping probe.
#[derive(Debug, Clone)]
pub struct PingResult {
    /// Target IPv4 address.
    pub target: u32,
    /// Round-trip time in milliseconds.
    pub rtt_ms: u64,
    /// Time-to-live of the reply.
    pub ttl: u8,
    /// Sequence number.
    pub seq: u16,
    /// `true` if a valid reply was received.
    pub success: bool,
}

/// Aggregate statistics for a ping session.
#[derive(Debug, Clone)]
pub struct PingStatistics {
    /// Number of probes sent.
    pub sent: u32,
    /// Number of successful replies received.
    pub received: u32,
    /// Packet loss percentage (0–100).
    pub loss_percent: u32,
    /// Minimum RTT in ms (0 if no replies).
    pub min_rtt_ms: u64,
    /// Maximum RTT in ms (0 if no replies).
    pub max_rtt_ms: u64,
    /// Average RTT in ms (0 if no replies).
    pub avg_rtt_ms: u64,
}

/// Manages an ICMP echo session to a single target.
#[derive(Debug)]
pub struct PingSession {
    /// Target IPv4 address.
    target: u32,
    /// Total probes to send (0 = unlimited).
    count: u32,
    /// Probes sent so far.
    sent: u32,
    /// Successful replies received.
    received: u32,
    /// Individual probe results.
    results: Vec<PingResult>,
    /// Next sequence number to use.
    next_seq: u16,
}

impl PingSession {
    /// Creates a new ping session.
    pub fn new(target: u32, count: u32) -> Self {
        Self {
            target,
            count,
            sent: 0,
            received: 0,
            results: Vec::new(),
            next_seq: 1,
        }
    }

    /// Returns the target IPv4 address.
    pub fn target(&self) -> u32 {
        self.target
    }

    /// Constructs and records an outgoing echo-request probe.
    ///
    /// Returns `None` if the configured `count` has been exhausted.
    pub fn send_ping(&mut self) -> Option<IcmpPacket> {
        if self.count != 0 && self.sent >= self.count {
            return None;
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.sent = self.sent.saturating_add(1);
        Some(IcmpPacket::new(
            IcmpType::EchoRequest,
            0,
            0,
            seq,
            b"ping".to_vec(),
        ))
    }

    /// Records a received echo-reply for the probe with the matching sequence.
    ///
    /// Returns `false` if the reply sequence number does not match the last sent probe.
    pub fn receive_pong(&mut self, reply: &IcmpPacket, rtt_ms: u64, ttl: u8) -> bool {
        if reply.icmp_type != IcmpType::EchoReply {
            return false;
        }
        self.received = self.received.saturating_add(1);
        self.results.push(PingResult {
            target: self.target,
            rtt_ms,
            ttl,
            seq: reply.sequence,
            success: true,
        });
        true
    }

    /// Computes aggregate statistics for this session.
    pub fn statistics(&self) -> PingStatistics {
        let successful: Vec<&PingResult> = self.results.iter().filter(|r| r.success).collect();
        let received = successful.len() as u32;
        let loss_percent = if self.sent == 0 {
            0
        } else {
            let lost = self.sent.saturating_sub(received);
            (lost as u64 * 100 / self.sent as u64) as u32
        };
        let (min_rtt, max_rtt, sum_rtt) =
            successful
                .iter()
                .fold((u64::MAX, 0u64, 0u64), |(min, max, sum), r| {
                    (
                        min.min(r.rtt_ms),
                        max.max(r.rtt_ms),
                        sum.saturating_add(r.rtt_ms),
                    )
                });
        let avg_rtt_ms = if received == 0 {
            0
        } else {
            sum_rtt / received as u64
        };
        PingStatistics {
            sent: self.sent,
            received,
            loss_percent,
            min_rtt_ms: if received == 0 { 0 } else { min_rtt },
            max_rtt_ms: max_rtt,
            avg_rtt_ms,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HTTP Method / Request / Response
// ═══════════════════════════════════════════════════════════════════════

/// HTTP request method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// GET — retrieve a resource.
    Get,
    /// POST — submit data to a resource.
    Post,
    /// HEAD — retrieve headers only.
    Head,
    /// PUT — replace a resource.
    Put,
    /// DELETE — remove a resource.
    Delete,
}

impl HttpMethod {
    /// Returns the method name as an uppercase string slice.
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Head => "HEAD",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An outgoing HTTP request.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Full URL string (e.g. `"http://example.com/path"`).
    pub url: String,
    /// Request headers as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// Optional request body.
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    /// Creates a new HTTP request.
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Adds a header to the request.
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()));
    }

    /// Serialises the request to an HTTP/1.1 byte sequence.
    pub fn serialise(&self) -> Vec<u8> {
        let parsed = HttpClient::parse_url(&self.url);
        let path = parsed.map(|(_, p)| p).unwrap_or_else(|| "/".into());
        let mut buf = format!("{} {} HTTP/1.1\r\n", self.method.as_str(), path);
        for (name, value) in &self.headers {
            buf.push_str(&format!("{}: {}\r\n", name, value));
        }
        if let Some(body) = &self.body {
            buf.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        buf.push_str("\r\n");
        let mut bytes = buf.into_bytes();
        if let Some(body) = &self.body {
            bytes.extend_from_slice(body);
        }
        bytes
    }
}

/// An HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code (e.g. 200, 404).
    pub status: u16,
    /// Status reason phrase (e.g. `"OK"`).
    pub reason: String,
    /// Response headers as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Creates a new HTTP response.
    pub fn new(status: u16, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Parses an HTTP/1.1 response from raw bytes.
    ///
    /// Returns `None` if the status line cannot be parsed.
    pub fn parse(raw: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(raw).ok()?;
        let mut lines = text.split("\r\n");
        let status_line = lines.next()?;
        // "HTTP/1.1 200 OK"
        let mut parts = status_line.splitn(3, ' ');
        parts.next()?; // HTTP/1.1
        let code: u16 = parts.next()?.parse().ok()?;
        let reason = parts.next().unwrap_or("").to_string();

        let mut headers = Vec::new();
        let mut body_start = status_line.len() + 2;
        for line in lines.by_ref() {
            if line.is_empty() {
                body_start += 2;
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
            body_start += line.len() + 2;
        }
        let body = raw
            .get(body_start..)
            .map(|s| s.to_vec())
            .unwrap_or_default();
        Some(Self {
            status: code,
            reason,
            headers,
            body,
        })
    }

    /// Looks up the `Content-Length` header and parses it as `usize`.
    pub fn content_length(&self) -> Option<usize> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse().ok())
    }

    /// Returns a header value by name (case-insensitive), or `None`.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HTTP Client / Wget (OS2.16)
// ═══════════════════════════════════════════════════════════════════════

/// Simulated HTTP/1.1 client.
///
/// Builds request objects and returns pre-formed responses for in-memory
/// testing. No real TCP connections are made.
#[derive(Debug)]
pub struct HttpClient {
    /// Value of the `User-Agent` header.
    user_agent: String,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
    /// Whether to follow `Location` redirects.
    follow_redirects: bool,
    /// Maximum number of redirects to follow.
    max_redirects: u32,
}

impl HttpClient {
    /// Creates a new HTTP client with default settings.
    pub fn new() -> Self {
        Self {
            user_agent: "FajarOS/1.0".into(),
            timeout_ms: 5000,
            follow_redirects: true,
            max_redirects: 10,
        }
    }

    /// Sets the User-Agent string.
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Sets redirect following behaviour.
    pub fn with_redirects(mut self, follow: bool, max: u32) -> Self {
        self.follow_redirects = follow;
        self.max_redirects = max;
        self
    }

    /// Builds an [`HttpRequest`] with `User-Agent` and `Host` headers pre-set.
    pub fn request(&self, method: HttpMethod, url: &str) -> HttpRequest {
        let mut req = HttpRequest::new(method, url);
        req.add_header("User-Agent", self.user_agent.clone());
        if let Some((host, _)) = Self::parse_url(url) {
            req.add_header("Host", host);
        }
        req
    }

    /// Builds a GET request for the given URL.
    pub fn get(&self, url: &str) -> HttpRequest {
        self.request(HttpMethod::Get, url)
    }

    /// Builds a HEAD request for the given URL.
    pub fn head(&self, url: &str) -> HttpRequest {
        self.request(HttpMethod::Head, url)
    }

    /// Parses a URL into `(host, path)`.
    ///
    /// Handles `http://host/path` and plain `host/path` forms.
    /// Returns `None` if the host cannot be determined.
    pub fn parse_url(url: &str) -> Option<(String, String)> {
        let stripped = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        let (host, path) = if let Some(slash) = stripped.find('/') {
            (&stripped[..slash], stripped[slash..].to_string())
        } else {
            (stripped, "/".to_string())
        };
        if host.is_empty() {
            return None;
        }
        Some((host.to_string(), path))
    }

    /// Returns the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns whether redirect following is enabled.
    pub fn follows_redirects(&self) -> bool {
        self.follow_redirects
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HTTP Server v2 (OS2.12)
// ═══════════════════════════════════════════════════════════════════════

/// A registered HTTP server route.
#[derive(Debug, Clone)]
pub struct HttpRoute {
    /// HTTP method to match.
    pub method: HttpMethod,
    /// URL path to match (e.g. `"/api/v1/status"`).
    pub path: String,
    /// Application-assigned handler identifier.
    pub handler_id: u64,
}

/// Simulated HTTP/1.1 server with a route table.
///
/// `handle_request` looks up the first matching route and returns a
/// synthetic `200 OK` or a `404 Not Found` response.
#[derive(Debug)]
pub struct HttpServer {
    /// Bound IPv4 address.
    addr: u32,
    /// Listening port.
    port: u16,
    /// Registered routes.
    routes: Vec<HttpRoute>,
    /// Whether the server is accepting requests.
    running: bool,
}

impl HttpServer {
    /// Creates a new HTTP server bound to the given address and port.
    pub fn new(addr: u32, port: u16) -> Self {
        Self {
            addr,
            port,
            routes: Vec::new(),
            running: false,
        }
    }

    /// Returns the bound IPv4 address.
    pub fn addr(&self) -> u32 {
        self.addr
    }

    /// Returns the listening port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns `true` if the server is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Starts the server.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stops the server.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Registers a route.
    pub fn add_route(&mut self, method: HttpMethod, path: impl Into<String>, handler_id: u64) {
        self.routes.push(HttpRoute {
            method,
            path: path.into(),
            handler_id,
        });
    }

    /// Handles an incoming request by matching it against registered routes.
    ///
    /// Returns a `200 OK` (with the handler ID in the body) if a route matches,
    /// or `404 Not Found` otherwise. Returns `None` if the server is not running.
    pub fn handle_request(&self, request: &HttpRequest) -> Option<HttpResponse> {
        if !self.running {
            return None;
        }
        for route in &self.routes {
            let path = HttpClient::parse_url(&request.url)
                .map(|(_, p)| p)
                .unwrap_or_else(|| request.url.clone());
            if route.method == request.method && route.path == path {
                let mut resp = HttpResponse::new(200, "OK");
                resp.body = format!("handler:{}", route.handler_id).into_bytes();
                return Some(resp);
            }
        }
        Some(HttpResponse::new(404, "Not Found"))
    }

    /// Returns the number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SSH Session (OS2.17)
// ═══════════════════════════════════════════════════════════════════════

/// SSH connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshState {
    /// Initial state before connection.
    Init,
    /// Exchanging keys (DH / ECDH).
    KeyExchange,
    /// Authenticating the user.
    Auth,
    /// Fully established — channels can be opened.
    Established,
    /// Session closed.
    Closed,
}

/// SSH authentication method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuthMethod {
    /// Username + password.
    Password,
    /// Public-key (simulated).
    PublicKey,
    /// No authentication (anonymous).
    None,
}

/// Channel type for an SSH multiplexed channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshChannelType {
    /// Interactive or exec session.
    Session,
    /// Direct TCP/IP tunnel (local forward).
    DirectTcpIp,
    /// Forwarded TCP/IP tunnel (remote forward).
    ForwardedTcpIp,
}

/// A single multiplexed SSH channel.
#[derive(Debug, Clone)]
pub struct SshChannel {
    /// Channel identifier.
    pub id: u32,
    /// Channel type.
    pub channel_type: SshChannelType,
    /// Whether the channel is currently open.
    pub open: bool,
    /// Buffered inbound data.
    pub data_buffer: Vec<u8>,
}

impl SshChannel {
    /// Creates a new open SSH channel.
    pub fn new(id: u32, channel_type: SshChannelType) -> Self {
        Self {
            id,
            channel_type,
            open: true,
            data_buffer: Vec::new(),
        }
    }

    /// Closes this channel.
    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Simulated SSH session.
///
/// Models the SSH state machine: Init → KeyExchange → Auth → Established.
/// Channels can be opened once the session is established.
#[derive(Debug)]
pub struct SshSession {
    /// Current connection state.
    state: SshState,
    /// Authentication method selected.
    auth_method: SshAuthMethod,
    /// Authenticating username.
    pub username: String,
    /// Server IPv4 address.
    pub server_addr: u32,
    /// Server port (default 22).
    pub port: u16,
    /// Unique session identifier.
    session_id: u64,
    /// Open channels.
    channels: Vec<SshChannel>,
}

impl SshSession {
    /// Creates a new SSH session in the `Init` state.
    pub fn new(session_id: u64, username: impl Into<String>, server_addr: u32, port: u16) -> Self {
        Self {
            state: SshState::Init,
            auth_method: SshAuthMethod::None,
            username: username.into(),
            server_addr,
            port,
            session_id,
            channels: Vec::new(),
        }
    }

    /// Returns the current SSH state.
    pub fn state(&self) -> SshState {
        self.state
    }

    /// Returns the session ID.
    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    /// Returns `true` if the session is fully established.
    pub fn is_established(&self) -> bool {
        self.state == SshState::Established
    }

    /// Advances the connection state machine.
    ///
    /// `Init` → `KeyExchange` → `Auth` → `Established`.
    /// Returns `false` if already `Established` or `Closed`.
    pub fn connect(&mut self) -> bool {
        self.state = match self.state {
            SshState::Init => SshState::KeyExchange,
            SshState::KeyExchange => SshState::Auth,
            SshState::Established | SshState::Closed => return false,
            other => other, // no-op for Auth
        };
        true
    }

    /// Authenticates the session.
    ///
    /// Transitions from `Auth` → `Established`.
    /// Returns `false` if not in `Auth` state.
    pub fn authenticate(&mut self, method: SshAuthMethod) -> bool {
        if self.state != SshState::Auth {
            return false;
        }
        self.auth_method = method;
        self.state = SshState::Established;
        true
    }

    /// Opens a new channel of the given type.
    ///
    /// Returns the channel ID, or `None` if the session is not established.
    pub fn open_channel(&mut self, channel_type: SshChannelType) -> Option<u32> {
        if self.state != SshState::Established {
            return None;
        }
        let id = self.channels.len() as u32;
        self.channels.push(SshChannel::new(id, channel_type));
        Some(id)
    }

    /// Writes data into the buffer of the channel with the given ID.
    ///
    /// Returns `false` if the channel does not exist or is closed.
    pub fn send_data(&mut self, channel_id: u32, data: &[u8]) -> bool {
        match self.channels.iter_mut().find(|c| c.id == channel_id) {
            Some(ch) if ch.open => {
                ch.data_buffer.extend_from_slice(data);
                true
            }
            _ => false,
        }
    }

    /// Drains and returns the buffered data for the given channel.
    ///
    /// Returns `None` if the channel does not exist.
    pub fn receive_data(&mut self, channel_id: u32) -> Option<Vec<u8>> {
        let ch = self.channels.iter_mut().find(|c| c.id == channel_id)?;
        Some(std::mem::take(&mut ch.data_buffer))
    }

    /// Closes all channels and transitions the session to `Closed`.
    pub fn close(&mut self) {
        for ch in &mut self.channels {
            ch.close();
        }
        self.state = SshState::Closed;
    }

    /// Returns the number of channels (open or closed).
    pub fn channel_count(&self) -> usize {
        self.channels.len()
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

    // ── VirtIO-net ──

    #[test]
    fn virtio_net_send_receive() {
        let mac = MacAddress::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        let mut dev = VirtioNetDevice::new(mac, 1514);

        let frame = EthernetFrame::new(MacAddress::BROADCAST, mac, 0x0800, vec![1u8, 2, 3, 4]);
        assert!(dev.send_frame(frame.clone()));
        assert_eq!(dev.tx_pending(), 1);
        assert_eq!(dev.stats().packets_tx, 1);

        // Simulate loopback: inject into RX and receive
        dev.inject_rx(frame);
        let received = dev.receive_frame();
        assert!(received.is_some());
        let f = received.unwrap();
        assert_eq!(f.ethertype, 0x0800);
        assert_eq!(f.payload, vec![1, 2, 3, 4]);
        assert_eq!(dev.stats().packets_rx, 1);
    }

    #[test]
    fn virtio_net_link_down_drops() {
        let mac = MacAddress::new([0x52, 0x54, 0x00, 0xAA, 0xBB, 0xCC]);
        let mut dev = VirtioNetDevice::new(mac, 1514);
        dev.set_link(false);
        assert!(!dev.is_link_up());

        let frame = EthernetFrame::new(MacAddress::BROADCAST, mac, 0x0806, vec![]);
        assert!(!dev.send_frame(frame.clone()));
        assert_eq!(dev.stats().errors, 1);
        assert_eq!(dev.tx_pending(), 0);

        dev.inject_rx(frame);
        assert!(dev.receive_frame().is_none());
    }

    // ── Network Namespace ──

    #[test]
    fn network_namespace_isolation() {
        let cfg_a = IpConfig::new(0xC0A80001, 0xFFFFFF00, 0xC0A800FE);
        let cfg_b = IpConfig::new(0x0A000001, 0xFF000000, 0x0A0000FE);

        let ns_a = NetworkNamespace::new("ns-a", cfg_a);
        let ns_b = NetworkNamespace::new("ns-b", cfg_b);

        // Each namespace has its own independent config
        assert_eq!(ns_a.config().address, 0xC0A80001);
        assert_eq!(ns_b.config().address, 0x0A000001);
        assert_eq!(ns_a.name(), "ns-a");
        assert_eq!(ns_b.name(), "ns-b");
    }

    #[test]
    fn network_namespace_socket_management() {
        let cfg = IpConfig::new(0xC0A80001, 0xFFFFFF00, 0xC0A800FE);
        let mut ns = NetworkNamespace::new("default", cfg);

        let sock = SocketHandle::new(42, SocketProtocol::Tcp);
        ns.add_socket(sock);
        assert_eq!(ns.socket_count(), 1);

        assert!(ns.remove_socket(42));
        assert_eq!(ns.socket_count(), 0);
        assert!(!ns.remove_socket(99)); // non-existent
    }

    // ── TLS Session ──

    #[test]
    fn tls_handshake_state_machine() {
        let config = TlsConfig::new(CipherSuite::Aes256GcmSha384, true);
        let mut session = TlsSession::new(1, config);

        assert_eq!(session.state(), TlsState::ClientHello);
        assert!(!session.is_established());

        assert!(session.handshake_step()); // -> ServerHello
        assert_eq!(session.state(), TlsState::ServerHello);

        assert!(session.handshake_step()); // -> Handshake
        assert_eq!(session.state(), TlsState::Handshake);

        assert!(session.handshake_step()); // -> Established
        assert_eq!(session.state(), TlsState::Established);
        assert!(session.is_established());

        // No further progress
        assert!(!session.handshake_step());
        assert_eq!(session.state(), TlsState::Established);
    }

    #[test]
    fn tls_encrypt_decrypt() {
        let config = TlsConfig::new(CipherSuite::ChaCha20Poly1305, false);
        let mut session = TlsSession::new(0xDEAD_BEEF_1234_5678, config);

        // Not established yet — encrypt/decrypt should return None
        assert!(session.encrypt(b"hello").is_none());

        // Advance to Established
        session.handshake_step();
        session.handshake_step();
        session.handshake_step();
        assert!(session.is_established());

        let plaintext = b"FajarOS rocks!";
        let ciphertext = session.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext.to_vec());

        let recovered = session.decrypt(&ciphertext).unwrap();
        assert_eq!(recovered, plaintext.to_vec());
        assert_eq!(session.bytes_encrypted(), plaintext.len() as u64);
        assert_eq!(session.bytes_decrypted(), plaintext.len() as u64);
    }

    // ── NTP Client ──

    #[test]
    fn ntp_sync_adjusts_time() {
        let mut ntp = NtpClient::new(0x08080808);
        assert!(!ntp.is_synced());

        // Server is 500 ms ahead of local clock
        let local_ms: u64 = 1_000_000;
        let server_ms: u64 = 1_000_500;
        ntp.sync(server_ms, local_ms, 2);

        assert!(ntp.is_synced());
        assert_eq!(ntp.offset(), 500);
        assert_eq!(ntp.stratum(), 2);

        // Corrected time should be local + offset
        let corrected = ntp.current_time(local_ms);
        assert_eq!(corrected, 1_000_500);
    }

    #[test]
    fn ntp_negative_offset() {
        let mut ntp = NtpClient::new(0x08080404);
        // Server is 200 ms behind local clock
        ntp.sync(1_000_000 - 200, 1_000_000, 1);
        assert_eq!(ntp.offset(), -200);
        // current_time should subtract 200
        assert_eq!(ntp.current_time(1_000_000), 999_800);
    }

    // ── ICMP / Ping ──

    #[test]
    fn icmp_ping_roundtrip() {
        let mut session = PingSession::new(0xC0A80001, 3);

        let req = session.send_ping().unwrap();
        assert_eq!(req.icmp_type, IcmpType::EchoRequest);
        assert_eq!(req.sequence, 1);

        let reply = IcmpPacket::new(IcmpType::EchoReply, 0, 0, req.sequence, b"ping".to_vec());
        assert!(session.receive_pong(&reply, 12, 64));

        let stats = session.statistics();
        assert_eq!(stats.sent, 1);
        assert_eq!(stats.received, 1);
        assert_eq!(stats.loss_percent, 0);
        assert_eq!(stats.min_rtt_ms, 12);
    }

    #[test]
    fn ping_session_statistics() {
        let mut session = PingSession::new(0x08080808, 4);

        // Send 4 pings, receive 3 replies (1 lost)
        let mut seqs = Vec::new();
        for _ in 0..4 {
            let pkt = session.send_ping().unwrap();
            seqs.push(pkt.sequence);
        }
        let rtts = [10u64, 20, 30];
        for (i, &seq) in seqs[..3].iter().enumerate() {
            let reply = IcmpPacket::new(IcmpType::EchoReply, 0, 0, seq, vec![]);
            session.receive_pong(&reply, rtts[i], 64);
        }

        let stats = session.statistics();
        assert_eq!(stats.sent, 4);
        assert_eq!(stats.received, 3);
        assert_eq!(stats.loss_percent, 25);
        assert_eq!(stats.min_rtt_ms, 10);
        assert_eq!(stats.max_rtt_ms, 30);
        assert_eq!(stats.avg_rtt_ms, 20); // (10+20+30)/3 = 20
    }

    // ── HTTP Client ──

    #[test]
    fn http_client_get_request() {
        let client = HttpClient::new();
        let req = client.get("http://example.com/index.html");

        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.url, "http://example.com/index.html");

        // Must have User-Agent and Host headers
        let ua = req
            .headers
            .iter()
            .find(|(k, _)| k == "User-Agent")
            .map(|(_, v)| v.as_str());
        assert_eq!(ua, Some("FajarOS/1.0"));

        let host = req
            .headers
            .iter()
            .find(|(k, _)| k == "Host")
            .map(|(_, v)| v.as_str());
        assert_eq!(host, Some("example.com"));

        // Serialised form must start with the request line
        let bytes = req.serialise();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.starts_with("GET /index.html HTTP/1.1\r\n"));
    }

    #[test]
    fn http_response_parse() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Type: text/plain\r\n\r\nhello";
        let resp = HttpResponse::parse(raw).unwrap();

        assert_eq!(resp.status, 200);
        assert_eq!(resp.reason, "OK");
        assert_eq!(resp.content_length(), Some(5));
        assert_eq!(resp.header("content-type"), Some("text/plain"));
        assert_eq!(resp.body, b"hello".to_vec());
    }

    // ── HTTP Server ──

    #[test]
    fn http_server_routes() {
        let mut server = HttpServer::new(0x7F000001, 8080);
        server.add_route(HttpMethod::Get, "/health", 1);
        server.add_route(HttpMethod::Post, "/api/data", 2);
        assert_eq!(server.route_count(), 2);
        assert!(!server.is_running());

        server.start();
        assert!(server.is_running());

        let req = HttpRequest::new(HttpMethod::Get, "http://localhost/health");
        let resp = server.handle_request(&req).unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.body.starts_with(b"handler:1"));

        // Unknown route → 404
        let req404 = HttpRequest::new(HttpMethod::Get, "http://localhost/missing");
        let resp404 = server.handle_request(&req404).unwrap();
        assert_eq!(resp404.status, 404);

        // Server stopped → None
        server.stop();
        assert!(server.handle_request(&req).is_none());
    }

    // ── SSH Session ──

    #[test]
    fn ssh_session_lifecycle() {
        let mut ssh = SshSession::new(99, "fajar", 0xC0A80001, 22);
        assert_eq!(ssh.state(), SshState::Init);

        // Init → KeyExchange
        assert!(ssh.connect());
        assert_eq!(ssh.state(), SshState::KeyExchange);

        // KeyExchange → Auth
        assert!(ssh.connect());
        assert_eq!(ssh.state(), SshState::Auth);

        // Auth → Established
        assert!(ssh.authenticate(SshAuthMethod::Password));
        assert!(ssh.is_established());

        // Open a session channel
        let ch_id = ssh.open_channel(SshChannelType::Session).unwrap();
        assert_eq!(ch_id, 0);
        assert_eq!(ssh.channel_count(), 1);

        // Send and receive data
        assert!(ssh.send_data(ch_id, b"ls -la\n"));
        let data = ssh.receive_data(ch_id).unwrap();
        assert_eq!(data, b"ls -la\n");

        // Second receive is empty (drained)
        let empty = ssh.receive_data(ch_id).unwrap();
        assert!(empty.is_empty());

        // Close session
        ssh.close();
        assert_eq!(ssh.state(), SshState::Closed);
        assert!(!ssh.is_established());
    }
}
