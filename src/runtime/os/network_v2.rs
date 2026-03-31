//! Network Stack v2 for FajarOS Nova v2.0 — Sprint N6.
//!
//! Extends the base network stack with WiFi simulation, IPv6, recursive DNS,
//! DHCP client, packet-filter firewall, TCP optimizations, network metrics,
//! BSD socket API, and ARP resolution cache. All structures are simulated
//! in-memory — no real network I/O.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Network V2 Errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors produced by the v2 network stack.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NetV2Error {
    /// WiFi access point not found.
    #[error("WiFi AP not found: {0}")]
    ApNotFound(String),
    /// WiFi authentication failed.
    #[error("WiFi auth failed: {0}")]
    AuthFailed(String),
    /// Socket not found.
    #[error("invalid socket: {0}")]
    InvalidSocket(u64),
    /// Port already in use.
    #[error("port already in use: {0}")]
    PortInUse(u16),
    /// Socket not connected.
    #[error("socket not connected")]
    NotConnected,
    /// No data available.
    #[error("no data available")]
    WouldBlock,
    /// DNS resolution failure.
    #[error("DNS resolution failed: {0}")]
    DnsFailure(String),
    /// Firewall packet rejected.
    #[error("packet rejected by firewall rule")]
    FirewallRejected,
    /// Invalid IPv6 address.
    #[error("invalid IPv6 address: {0}")]
    InvalidIpv6(String),
    /// DHCP lease expired.
    #[error("DHCP lease expired")]
    DhcpLeaseExpired,
    /// ARP resolution timed out.
    #[error("ARP resolution timeout for {0}")]
    ArpTimeout(String),
}

// ═══════════════════════════════════════════════════════════════════════
// WiFi Driver
// ═══════════════════════════════════════════════════════════════════════

/// WiFi security type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSecurity {
    /// Open (no password).
    Open,
    /// WPA2-PSK.
    Wpa2,
    /// WPA3-SAE.
    Wpa3,
}

impl fmt::Display for WifiSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::Wpa2 => write!(f, "WPA2"),
            Self::Wpa3 => write!(f, "WPA3"),
        }
    }
}

/// WiFi connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiState {
    /// Not connected.
    Disconnected,
    /// Scanning for access points.
    Scanning,
    /// Associating with an AP.
    Associating,
    /// Connected.
    Connected,
}

/// A visible WiFi access point.
#[derive(Debug, Clone)]
pub struct AccessPoint {
    /// Network SSID.
    pub ssid: String,
    /// BSSID (MAC address as string).
    pub bssid: String,
    /// Channel number.
    pub channel: u8,
    /// Signal strength in dBm (e.g. -50 is strong, -90 is weak).
    pub signal_dbm: i8,
    /// Security type.
    pub security: WifiSecurity,
    /// Password (empty for open networks).
    password: String,
}

/// Simulated WiFi driver.
///
/// Supports scanning for simulated access points, connecting with WPA2
/// authentication, disconnecting, and reporting signal strength.
#[derive(Debug)]
pub struct WifiDriver {
    /// Current connection state.
    pub state: WifiState,
    /// Currently connected SSID (if any).
    pub connected_ssid: Option<String>,
    /// Signal strength of current connection.
    pub signal_dbm: i8,
    /// Available access points (simulated).
    access_points: Vec<AccessPoint>,
    /// MAC address of the WiFi interface.
    pub mac: [u8; 6],
}

impl WifiDriver {
    /// Creates a new WiFi driver with pre-populated simulated APs.
    pub fn new() -> Self {
        let mut driver = Self {
            state: WifiState::Disconnected,
            connected_ssid: None,
            signal_dbm: -100,
            access_points: Vec::new(),
            mac: [0x52, 0x54, 0x00, 0xAB, 0xCD, 0xEF],
        };
        // Pre-populate simulated access points
        driver.access_points.push(AccessPoint {
            ssid: "FajarOS-Lab".to_string(),
            bssid: "AA:BB:CC:DD:EE:01".to_string(),
            channel: 6,
            signal_dbm: -45,
            security: WifiSecurity::Wpa2,
            password: "fajarlan9!".to_string(),
        });
        driver.access_points.push(AccessPoint {
            ssid: "Guest-Open".to_string(),
            bssid: "AA:BB:CC:DD:EE:02".to_string(),
            channel: 11,
            signal_dbm: -70,
            security: WifiSecurity::Open,
            password: String::new(),
        });
        driver.access_points.push(AccessPoint {
            ssid: "IoT-Secure".to_string(),
            bssid: "AA:BB:CC:DD:EE:03".to_string(),
            channel: 1,
            signal_dbm: -55,
            security: WifiSecurity::Wpa3,
            password: "iot-secret-2026".to_string(),
        });
        driver
    }

    /// Adds a simulated access point.
    pub fn add_ap(
        &mut self,
        ssid: &str,
        bssid: &str,
        channel: u8,
        signal_dbm: i8,
        security: WifiSecurity,
        password: &str,
    ) {
        self.access_points.push(AccessPoint {
            ssid: ssid.to_string(),
            bssid: bssid.to_string(),
            channel,
            signal_dbm,
            security,
            password: password.to_string(),
        });
    }

    /// Scans for available networks. Returns a list of visible APs.
    pub fn scan(&mut self) -> Vec<AccessPoint> {
        self.state = WifiState::Scanning;
        let results = self.access_points.clone();
        if self.connected_ssid.is_some() {
            self.state = WifiState::Connected;
        } else {
            self.state = WifiState::Disconnected;
        }
        results
    }

    /// Connects to a WiFi network by SSID with the given password.
    pub fn connect(&mut self, ssid: &str, password: &str) -> Result<(), NetV2Error> {
        let ap = self
            .access_points
            .iter()
            .find(|ap| ap.ssid == ssid)
            .ok_or_else(|| NetV2Error::ApNotFound(ssid.to_string()))?
            .clone();

        self.state = WifiState::Associating;

        // Verify password
        match ap.security {
            WifiSecurity::Open => { /* no password needed */ }
            WifiSecurity::Wpa2 | WifiSecurity::Wpa3 => {
                if password != ap.password {
                    self.state = WifiState::Disconnected;
                    return Err(NetV2Error::AuthFailed(ssid.to_string()));
                }
            }
        }

        self.connected_ssid = Some(ssid.to_string());
        self.signal_dbm = ap.signal_dbm;
        self.state = WifiState::Connected;
        Ok(())
    }

    /// Disconnects from the current network.
    pub fn disconnect(&mut self) {
        self.connected_ssid = None;
        self.signal_dbm = -100;
        self.state = WifiState::Disconnected;
    }

    /// Returns the current signal strength in dBm.
    pub fn signal_strength(&self) -> i8 {
        self.signal_dbm
    }

    /// Returns `true` if connected.
    pub fn is_connected(&self) -> bool {
        self.state == WifiState::Connected
    }
}

impl Default for WifiDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IPv6 Stack
// ═══════════════════════════════════════════════════════════════════════

/// An IPv6 address (128 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv6Addr(pub [u8; 16]);

impl Ipv6Addr {
    /// Creates an IPv6 address from bytes.
    pub fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The loopback address (::1).
    pub const LOOPBACK: Self = Self([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    /// The unspecified address (::).
    pub const UNSPECIFIED: Self = Self([0; 16]);

    /// Link-local prefix (fe80::/10).
    pub fn is_link_local(&self) -> bool {
        self.0[0] == 0xfe && (self.0[1] & 0xC0) == 0x80
    }

    /// Returns `true` if this is a multicast address (ff00::/8).
    pub fn is_multicast(&self) -> bool {
        self.0[0] == 0xff
    }

    /// Returns `true` if this is the loopback address (::1).
    pub fn is_loopback(&self) -> bool {
        *self == Self::LOOPBACK
    }

    /// Parses a simplified IPv6 string (colon-hex, no :: shorthand).
    pub fn parse(s: &str) -> Result<Self, NetV2Error> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 8 {
            return Err(NetV2Error::InvalidIpv6(s.to_string()));
        }
        let mut bytes = [0u8; 16];
        for (i, part) in parts.iter().enumerate() {
            let val =
                u16::from_str_radix(part, 16).map_err(|_| NetV2Error::InvalidIpv6(s.to_string()))?;
            bytes[i * 2] = (val >> 8) as u8;
            bytes[i * 2 + 1] = val as u8;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let groups: Vec<String> = (0..8)
            .map(|i| {
                let val = u16::from_be_bytes([self.0[i * 2], self.0[i * 2 + 1]]);
                format!("{:04x}", val)
            })
            .collect();
        write!(f, "{}", groups.join(":"))
    }
}

/// IPv6 neighbor cache entry (like ARP for IPv6).
#[derive(Debug, Clone)]
pub struct NeighborEntry {
    /// IPv6 address.
    pub addr: Ipv6Addr,
    /// Link-layer (MAC) address.
    pub mac: [u8; 6],
    /// Timestamp of last confirmation.
    pub timestamp: u64,
    /// Is this entry still reachable?
    pub reachable: bool,
}

/// IPv6 routing entry.
#[derive(Debug, Clone)]
pub struct Ipv6Route {
    /// Destination prefix.
    pub prefix: Ipv6Addr,
    /// Prefix length (0-128).
    pub prefix_len: u8,
    /// Next-hop address.
    pub next_hop: Ipv6Addr,
    /// Interface name.
    pub interface: String,
    /// Metric (lower is preferred).
    pub metric: u32,
}

/// Simulated IPv6 stack.
///
/// Provides IPv6 address configuration, neighbor discovery (NDP), and
/// prefix-based routing. All structures are in-memory.
#[derive(Debug)]
pub struct Ipv6Stack {
    /// Configured addresses.
    pub addresses: Vec<Ipv6Addr>,
    /// Neighbor cache (IPv6 -> MAC).
    neighbors: HashMap<[u8; 16], NeighborEntry>,
    /// Routing table.
    routes: Vec<Ipv6Route>,
    /// Interface name.
    pub interface: String,
}

impl Ipv6Stack {
    /// Creates a new IPv6 stack on the given interface.
    pub fn new(interface: &str) -> Self {
        Self {
            addresses: Vec::new(),
            neighbors: HashMap::new(),
            routes: Vec::new(),
            interface: interface.to_string(),
        }
    }

    /// Adds an IPv6 address to the interface.
    pub fn add_address(&mut self, addr: Ipv6Addr) {
        if !self.addresses.contains(&addr) {
            self.addresses.push(addr);
        }
    }

    /// Removes an IPv6 address from the interface.
    pub fn remove_address(&mut self, addr: &Ipv6Addr) -> bool {
        let len = self.addresses.len();
        self.addresses.retain(|a| a != addr);
        self.addresses.len() < len
    }

    /// Adds a neighbor entry (NDP cache).
    pub fn add_neighbor(&mut self, addr: Ipv6Addr, mac: [u8; 6], now: u64) {
        self.neighbors.insert(
            addr.0,
            NeighborEntry {
                addr,
                mac,
                timestamp: now,
                reachable: true,
            },
        );
    }

    /// Looks up a neighbor's MAC address.
    pub fn lookup_neighbor(&self, addr: &Ipv6Addr) -> Option<&NeighborEntry> {
        self.neighbors.get(&addr.0)
    }

    /// Adds an IPv6 route.
    pub fn add_route(&mut self, route: Ipv6Route) {
        self.routes.push(route);
    }

    /// Performs longest-prefix-match routing lookup.
    pub fn route_lookup(&self, dest: &Ipv6Addr) -> Option<&Ipv6Route> {
        let mut best: Option<&Ipv6Route> = None;
        let mut best_len: u8 = 0;

        for route in &self.routes {
            if Self::prefix_match(dest, &route.prefix, route.prefix_len) {
                if best.is_none()
                    || route.prefix_len > best_len
                    || (route.prefix_len == best_len
                        && route.metric < best.map(|r| r.metric).unwrap_or(u32::MAX))
                {
                    best = Some(route);
                    best_len = route.prefix_len;
                }
            }
        }
        best
    }

    /// Checks whether `addr` matches `prefix` for the first `len` bits.
    fn prefix_match(addr: &Ipv6Addr, prefix: &Ipv6Addr, len: u8) -> bool {
        let full_bytes = (len / 8) as usize;
        let remaining_bits = len % 8;

        if addr.0[..full_bytes] != prefix.0[..full_bytes] {
            return false;
        }
        if remaining_bits > 0 && full_bytes < 16 {
            let mask = 0xFFu8 << (8 - remaining_bits);
            if (addr.0[full_bytes] & mask) != (prefix.0[full_bytes] & mask) {
                return false;
            }
        }
        true
    }

    /// Returns the number of configured addresses.
    pub fn address_count(&self) -> usize {
        self.addresses.len()
    }

    /// Returns the number of neighbor entries.
    pub fn neighbor_count(&self) -> usize {
        self.neighbors.len()
    }

    /// Returns the number of routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DNS Resolver (Recursive)
// ═══════════════════════════════════════════════════════════════════════

/// DNS record type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordType {
    /// A record (IPv4).
    A,
    /// AAAA record (IPv6).
    Aaaa,
    /// CNAME record (alias).
    Cname,
    /// MX record (mail exchange).
    Mx,
    /// NS record (name server).
    Ns,
}

/// A single DNS record.
#[derive(Debug, Clone)]
pub struct DnsRecord {
    /// Domain name.
    pub name: String,
    /// Record type.
    pub record_type: DnsRecordType,
    /// Record value (IP address string, hostname, etc.).
    pub value: String,
    /// TTL in seconds.
    pub ttl: u64,
}

/// Cached DNS resolution result.
#[derive(Debug, Clone)]
struct DnsCacheEntryV2 {
    /// Resolved records.
    records: Vec<DnsRecord>,
    /// Expiry timestamp.
    expires_at: u64,
}

/// Recursive DNS resolver with caching.
///
/// Maintains an authoritative zone (simulated) and a TTL-based cache.
/// Supports A, AAAA, CNAME, MX, and NS record types.
#[derive(Debug)]
pub struct DnsResolver {
    /// Authoritative zone records (simulated upstream).
    zone: Vec<DnsRecord>,
    /// Resolution cache.
    cache: HashMap<(String, u8), DnsCacheEntryV2>,
    /// DNS server address.
    pub server: String,
    /// Maximum recursion depth for CNAME chains.
    pub max_recursion: u8,
}

impl DnsResolver {
    /// Creates a new DNS resolver with common pre-populated records.
    pub fn new(server: &str) -> Self {
        let mut resolver = Self {
            zone: Vec::new(),
            cache: HashMap::new(),
            server: server.to_string(),
            max_recursion: 10,
        };
        // Pre-populate common records
        resolver.add_zone_record(DnsRecord {
            name: "fajaros.local".to_string(),
            record_type: DnsRecordType::A,
            value: "10.0.2.15".to_string(),
            ttl: 3600,
        });
        resolver.add_zone_record(DnsRecord {
            name: "gateway.local".to_string(),
            record_type: DnsRecordType::A,
            value: "10.0.2.2".to_string(),
            ttl: 3600,
        });
        resolver
    }

    /// Adds a record to the authoritative zone.
    pub fn add_zone_record(&mut self, record: DnsRecord) {
        self.zone.push(record);
    }

    /// Resolves a hostname to records of the given type.
    ///
    /// Checks the cache first, then falls back to the zone (simulated
    /// recursive resolution). CNAME chains are followed up to
    /// `max_recursion` depth.
    pub fn resolve(
        &mut self,
        hostname: &str,
        record_type: DnsRecordType,
        now: u64,
    ) -> Result<Vec<DnsRecord>, NetV2Error> {
        self.resolve_recursive(hostname, record_type, now, 0)
    }

    /// Internal recursive resolver.
    fn resolve_recursive(
        &mut self,
        hostname: &str,
        record_type: DnsRecordType,
        now: u64,
        depth: u8,
    ) -> Result<Vec<DnsRecord>, NetV2Error> {
        if depth > self.max_recursion {
            return Err(NetV2Error::DnsFailure(format!(
                "CNAME recursion limit for {}",
                hostname
            )));
        }

        // Check cache
        let cache_key = (hostname.to_string(), record_type as u8);
        if let Some(entry) = self.cache.get(&cache_key) {
            if now < entry.expires_at {
                return Ok(entry.records.clone());
            }
        }

        // Search zone for direct matches
        let mut results: Vec<DnsRecord> = self
            .zone
            .iter()
            .filter(|r| r.name == hostname && r.record_type == record_type)
            .cloned()
            .collect();

        // If no direct matches, check for CNAME
        if results.is_empty() {
            let cnames: Vec<DnsRecord> = self
                .zone
                .iter()
                .filter(|r| r.name == hostname && r.record_type == DnsRecordType::Cname)
                .cloned()
                .collect();

            for cname in &cnames {
                let sub = self.resolve_recursive(&cname.value, record_type, now, depth + 1)?;
                results.extend(sub);
            }
        }

        if results.is_empty() {
            return Err(NetV2Error::DnsFailure(hostname.to_string()));
        }

        // Cache results
        let min_ttl = results.iter().map(|r| r.ttl).min().unwrap_or(60);
        self.cache.insert(
            cache_key,
            DnsCacheEntryV2 {
                records: results.clone(),
                expires_at: now.saturating_add(min_ttl),
            },
        );

        Ok(results)
    }

    /// Clears expired cache entries.
    pub fn gc(&mut self, now: u64) {
        self.cache.retain(|_, entry| now < entry.expires_at);
    }

    /// Returns the number of cached entries.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clears the entire cache.
    pub fn flush_cache(&mut self) {
        self.cache.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DHCP Client v2
// ═══════════════════════════════════════════════════════════════════════

/// DHCPv2 client state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhcpV2State {
    /// Initial state — ready to send DISCOVER.
    Init,
    /// DISCOVER sent, waiting for OFFER.
    Selecting,
    /// REQUEST sent, waiting for ACK.
    Requesting,
    /// Lease acquired, operational.
    Bound,
    /// Lease halfway expired, sending REQUEST to renew.
    Renewing,
    /// Lease almost expired, broadcasting REQUEST.
    Rebinding,
}

/// DHCP lease information.
#[derive(Debug, Clone)]
pub struct DhcpLease {
    /// Assigned IP address.
    pub address: u32,
    /// Subnet mask.
    pub netmask: u32,
    /// Default gateway.
    pub gateway: u32,
    /// DNS servers.
    pub dns_servers: Vec<u32>,
    /// Lease duration in seconds.
    pub lease_secs: u64,
    /// Lease start timestamp.
    pub lease_start: u64,
    /// DHCP server address.
    pub server: u32,
}

impl DhcpLease {
    /// Returns `true` if the lease has expired.
    pub fn is_expired(&self, now: u64) -> bool {
        now.saturating_sub(self.lease_start) >= self.lease_secs
    }

    /// Returns remaining lease time in seconds.
    pub fn remaining(&self, now: u64) -> u64 {
        let elapsed = now.saturating_sub(self.lease_start);
        self.lease_secs.saturating_sub(elapsed)
    }
}

/// Simulated DHCP client with full discover/offer/request/ack cycle.
#[derive(Debug)]
pub struct DhcpClient {
    /// Current state.
    pub state: DhcpV2State,
    /// Transaction ID.
    pub xid: u32,
    /// Client MAC address.
    pub mac: [u8; 6],
    /// Current lease (if bound).
    lease: Option<DhcpLease>,
}

impl DhcpClient {
    /// Creates a new DHCP client.
    pub fn new(xid: u32, mac: [u8; 6]) -> Self {
        Self {
            state: DhcpV2State::Init,
            xid,
            mac,
            lease: None,
        }
    }

    /// Sends DISCOVER and transitions to Selecting.
    pub fn discover(&mut self) -> Vec<u8> {
        self.state = DhcpV2State::Selecting;
        // Build a minimal simulated DHCP DISCOVER packet
        let mut pkt = Vec::with_capacity(32);
        pkt.push(1); // op: BOOTREQUEST
        pkt.push(1); // htype: Ethernet
        pkt.push(6); // hlen: 6
        pkt.push(0); // hops
        pkt.extend_from_slice(&self.xid.to_be_bytes());
        pkt.extend_from_slice(&[0; 8]); // secs, flags, ciaddr, yiaddr (simplified)
        pkt.extend_from_slice(&self.mac);
        pkt.extend_from_slice(&[0; 10]); // padding
        pkt
    }

    /// Processes an OFFER and transitions to Requesting.
    pub fn receive_offer(
        &mut self,
        address: u32,
        netmask: u32,
        gateway: u32,
        dns: Vec<u32>,
        server: u32,
    ) {
        if self.state == DhcpV2State::Selecting {
            self.lease = Some(DhcpLease {
                address,
                netmask,
                gateway,
                dns_servers: dns,
                lease_secs: 0,
                lease_start: 0,
                server,
            });
            self.state = DhcpV2State::Requesting;
        }
    }

    /// Sends REQUEST and transitions to Bound upon ACK.
    pub fn receive_ack(&mut self, lease_secs: u64, now: u64) {
        if self.state == DhcpV2State::Requesting || self.state == DhcpV2State::Renewing {
            if let Some(ref mut lease) = self.lease {
                lease.lease_secs = lease_secs;
                lease.lease_start = now;
            }
            self.state = DhcpV2State::Bound;
        }
    }

    /// Checks if renewal is needed (at T1 = 50% of lease).
    pub fn check_renewal(&mut self, now: u64) {
        if self.state == DhcpV2State::Bound {
            if let Some(ref lease) = self.lease {
                let elapsed = now.saturating_sub(lease.lease_start);
                if elapsed >= lease.lease_secs {
                    self.state = DhcpV2State::Init;
                    self.lease = None;
                } else if elapsed >= (lease.lease_secs * 7) / 8 {
                    self.state = DhcpV2State::Rebinding;
                } else if elapsed >= lease.lease_secs / 2 {
                    self.state = DhcpV2State::Renewing;
                }
            }
        }
    }

    /// Returns the current lease, if any.
    pub fn lease(&self) -> Option<&DhcpLease> {
        self.lease.as_ref()
    }

    /// Returns `true` if the client has a valid lease.
    pub fn is_bound(&self) -> bool {
        self.state == DhcpV2State::Bound
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
    /// Deny (drop) the packet.
    Deny,
}

/// IP protocol for firewall matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallProto {
    /// Match any protocol.
    Any,
    /// TCP only.
    Tcp,
    /// UDP only.
    Udp,
    /// ICMP only.
    Icmp,
}

/// Direction for firewall rule matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallDirection {
    /// Inbound traffic.
    Inbound,
    /// Outbound traffic.
    Outbound,
    /// Both directions.
    Both,
}

/// A single firewall rule.
#[derive(Debug, Clone)]
pub struct FirewallRule {
    /// Rule priority (lower number = higher priority).
    pub priority: u32,
    /// Action to take.
    pub action: FirewallAction,
    /// Direction.
    pub direction: FirewallDirection,
    /// Protocol to match.
    pub protocol: FirewallProto,
    /// Source address (0 = any).
    pub src_addr: u32,
    /// Destination address (0 = any).
    pub dst_addr: u32,
    /// Source port (0 = any).
    pub src_port: u16,
    /// Destination port (0 = any).
    pub dst_port: u16,
    /// Human-readable description.
    pub description: String,
}

/// Simulated packet context for firewall evaluation.
#[derive(Debug, Clone)]
pub struct PacketContext {
    /// Source IP.
    pub src_addr: u32,
    /// Destination IP.
    pub dst_addr: u32,
    /// Source port.
    pub src_port: u16,
    /// Destination port.
    pub dst_port: u16,
    /// Protocol.
    pub protocol: FirewallProto,
    /// Direction.
    pub direction: FirewallDirection,
}

/// Packet-filter firewall.
///
/// Evaluates rules in priority order. First matching rule determines the
/// action. If no rule matches, the default policy applies.
#[derive(Debug)]
pub struct Firewall {
    /// Ordered rules.
    rules: Vec<FirewallRule>,
    /// Default policy (when no rule matches).
    pub default_policy: FirewallAction,
    /// Packet counter (allowed).
    pub allowed_count: u64,
    /// Packet counter (denied).
    pub denied_count: u64,
}

impl Firewall {
    /// Creates a new firewall with the given default policy.
    pub fn new(default_policy: FirewallAction) -> Self {
        Self {
            rules: Vec::new(),
            default_policy,
            allowed_count: 0,
            denied_count: 0,
        }
    }

    /// Adds a firewall rule. Rules are kept sorted by priority.
    pub fn add_rule(&mut self, rule: FirewallRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.priority);
    }

    /// Removes all rules with the given description.
    pub fn remove_rule(&mut self, description: &str) -> usize {
        let before = self.rules.len();
        self.rules.retain(|r| r.description != description);
        before - self.rules.len()
    }

    /// Evaluates a packet against the firewall rules.
    pub fn evaluate(&mut self, pkt: &PacketContext) -> FirewallAction {
        for rule in &self.rules {
            if self.rule_matches(rule, pkt) {
                match rule.action {
                    FirewallAction::Allow => {
                        self.allowed_count += 1;
                        return FirewallAction::Allow;
                    }
                    FirewallAction::Deny => {
                        self.denied_count += 1;
                        return FirewallAction::Deny;
                    }
                }
            }
        }
        // Default policy
        match self.default_policy {
            FirewallAction::Allow => {
                self.allowed_count += 1;
                FirewallAction::Allow
            }
            FirewallAction::Deny => {
                self.denied_count += 1;
                FirewallAction::Deny
            }
        }
    }

    /// Checks if a rule matches a packet.
    fn rule_matches(&self, rule: &FirewallRule, pkt: &PacketContext) -> bool {
        // Direction check
        match rule.direction {
            FirewallDirection::Inbound => {
                if pkt.direction != FirewallDirection::Inbound {
                    return false;
                }
            }
            FirewallDirection::Outbound => {
                if pkt.direction != FirewallDirection::Outbound {
                    return false;
                }
            }
            FirewallDirection::Both => { /* matches all */ }
        }
        // Protocol check
        if rule.protocol != FirewallProto::Any && rule.protocol != pkt.protocol {
            return false;
        }
        // Address checks (0 = wildcard)
        if rule.src_addr != 0 && rule.src_addr != pkt.src_addr {
            return false;
        }
        if rule.dst_addr != 0 && rule.dst_addr != pkt.dst_addr {
            return false;
        }
        // Port checks (0 = wildcard)
        if rule.src_port != 0 && rule.src_port != pkt.src_port {
            return false;
        }
        if rule.dst_port != 0 && rule.dst_port != pkt.dst_port {
            return false;
        }
        true
    }

    /// Returns the number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Resets packet counters.
    pub fn reset_counters(&mut self) {
        self.allowed_count = 0;
        self.denied_count = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TCP Optimizations
// ═══════════════════════════════════════════════════════════════════════

/// TCP window scaling configuration.
#[derive(Debug, Clone)]
pub struct TcpWindowScale {
    /// Window scale factor (shift count, 0-14).
    pub scale: u8,
    /// Base window size.
    pub base_window: u32,
}

impl TcpWindowScale {
    /// Creates a new window scale.
    pub fn new(scale: u8) -> Self {
        Self {
            scale: scale.min(14),
            base_window: 65535,
        }
    }

    /// Returns the effective window size.
    pub fn effective_window(&self) -> u64 {
        (self.base_window as u64) << self.scale
    }
}

/// Selective ACK (SACK) block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SackBlock {
    /// Left edge (first byte received).
    pub left: u32,
    /// Right edge (byte after last received).
    pub right: u32,
}

/// TCP optimizations: window scaling, SACK, congestion control.
#[derive(Debug)]
pub struct TcpOptimizations {
    /// Window scaling.
    pub window_scale: TcpWindowScale,
    /// SACK blocks (up to 4).
    pub sack_blocks: Vec<SackBlock>,
    /// Congestion window (segments).
    pub cwnd: u32,
    /// Slow-start threshold (segments).
    pub ssthresh: u32,
    /// Current RTT estimate (ms).
    pub rtt_ms: u32,
    /// Retransmission timeout (ms).
    pub rto_ms: u32,
    /// Duplicate ACK counter.
    dup_ack_count: u32,
    /// Is Nagle algorithm enabled?
    pub nagle_enabled: bool,
}

impl TcpOptimizations {
    /// Creates TCP optimizations with the given window scale.
    pub fn new(window_scale: u8) -> Self {
        Self {
            window_scale: TcpWindowScale::new(window_scale),
            sack_blocks: Vec::new(),
            cwnd: 10,
            ssthresh: 65535,
            rtt_ms: 0,
            rto_ms: 1000,
            dup_ack_count: 0,
            nagle_enabled: true,
        }
    }

    /// Adds a SACK block (max 4).
    pub fn add_sack(&mut self, left: u32, right: u32) {
        if self.sack_blocks.len() < 4 {
            self.sack_blocks.push(SackBlock { left, right });
        }
    }

    /// Called on ACK received — adjusts congestion window.
    pub fn on_ack(&mut self) {
        self.dup_ack_count = 0;
        if self.cwnd < self.ssthresh {
            // Slow start: exponential growth
            self.cwnd = self.cwnd.saturating_add(1);
        } else {
            // Congestion avoidance: linear growth
            let inc = (1_u64 * 1_u64) / self.cwnd.max(1) as u64;
            self.cwnd = self.cwnd.saturating_add(inc.max(1) as u32);
        }
    }

    /// Called on duplicate ACK — may trigger fast retransmit.
    pub fn on_dup_ack(&mut self) {
        self.dup_ack_count = self.dup_ack_count.saturating_add(1);
        if self.dup_ack_count >= 3 {
            // Fast retransmit + fast recovery
            self.ssthresh = (self.cwnd / 2).max(2);
            self.cwnd = self.ssthresh.saturating_add(3);
        }
    }

    /// Called on timeout — resets to slow start.
    pub fn on_timeout(&mut self) {
        self.ssthresh = (self.cwnd / 2).max(2);
        self.cwnd = 1;
        self.dup_ack_count = 0;
    }

    /// Updates RTT estimate with a new sample (exponential moving average).
    pub fn update_rtt(&mut self, sample_ms: u32) {
        if self.rtt_ms == 0 {
            self.rtt_ms = sample_ms;
            self.rto_ms = sample_ms.saturating_mul(3);
        } else {
            self.rtt_ms = (self.rtt_ms * 7 + sample_ms) / 8;
            self.rto_ms = self.rtt_ms.saturating_mul(2).max(200);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Network Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Network performance metrics.
#[derive(Debug)]
pub struct NetworkMetrics {
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total packets sent.
    pub packets_sent: u64,
    /// Total packets received.
    pub packets_received: u64,
    /// Packets lost (retransmissions, drops).
    pub packets_lost: u64,
    /// Active connections count.
    pub active_connections: u32,
    /// Latency samples (ms).
    latency_samples: Vec<u32>,
    /// Maximum sample count.
    max_samples: usize,
}

impl NetworkMetrics {
    /// Creates a new metrics tracker.
    pub fn new() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            packets_lost: 0,
            active_connections: 0,
            latency_samples: Vec::new(),
            max_samples: 1000,
        }
    }

    /// Records bytes sent.
    pub fn record_send(&mut self, bytes: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(bytes);
        self.packets_sent = self.packets_sent.saturating_add(1);
    }

    /// Records bytes received.
    pub fn record_recv(&mut self, bytes: u64) {
        self.bytes_received = self.bytes_received.saturating_add(bytes);
        self.packets_received = self.packets_received.saturating_add(1);
    }

    /// Records a packet loss.
    pub fn record_loss(&mut self) {
        self.packets_lost = self.packets_lost.saturating_add(1);
    }

    /// Records a latency sample.
    pub fn record_latency(&mut self, ms: u32) {
        if self.latency_samples.len() >= self.max_samples {
            self.latency_samples.remove(0);
        }
        self.latency_samples.push(ms);
    }

    /// Returns the average latency in ms.
    pub fn avg_latency(&self) -> f64 {
        if self.latency_samples.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.latency_samples.iter().map(|&s| s as u64).sum();
        sum as f64 / self.latency_samples.len() as f64
    }

    /// Returns the packet loss ratio (0.0 to 1.0).
    pub fn loss_ratio(&self) -> f64 {
        let total = self.packets_sent.saturating_add(self.packets_received);
        if total == 0 {
            return 0.0;
        }
        self.packets_lost as f64 / total as f64
    }

    /// Returns estimated bandwidth (bytes/sec) given the observation window in seconds.
    pub fn bandwidth_bps(&self, window_secs: f64) -> f64 {
        if window_secs <= 0.0 {
            return 0.0;
        }
        (self.bytes_sent.saturating_add(self.bytes_received)) as f64 / window_secs
    }

    /// Registers a new connection.
    pub fn connection_opened(&mut self) {
        self.active_connections = self.active_connections.saturating_add(1);
    }

    /// Unregisters a connection.
    pub fn connection_closed(&mut self) {
        self.active_connections = self.active_connections.saturating_sub(1);
    }

    /// Resets all counters.
    pub fn reset(&mut self) {
        self.bytes_sent = 0;
        self.bytes_received = 0;
        self.packets_sent = 0;
        self.packets_received = 0;
        self.packets_lost = 0;
        self.active_connections = 0;
        self.latency_samples.clear();
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Socket API (BSD-style)
// ═══════════════════════════════════════════════════════════════════════

/// Socket type for the BSD API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsdSocketType {
    /// SOCK_STREAM (TCP).
    Stream,
    /// SOCK_DGRAM (UDP).
    Datagram,
}

/// Socket state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BsdSocketState {
    /// Created.
    Created,
    /// Bound to an address.
    Bound,
    /// Listening (TCP server).
    Listening,
    /// Connected (TCP client or accepted).
    Connected,
    /// Closed.
    Closed,
}

/// A BSD-style network socket.
#[derive(Debug)]
pub struct BsdSocket {
    /// File descriptor.
    pub fd: u64,
    /// Socket type.
    pub kind: BsdSocketType,
    /// Current state.
    pub state: BsdSocketState,
    /// Local address.
    pub local_addr: u32,
    /// Local port.
    pub local_port: u16,
    /// Remote address.
    pub remote_addr: u32,
    /// Remote port.
    pub remote_port: u16,
    /// Receive buffer.
    recv_buf: Vec<Vec<u8>>,
    /// Send buffer.
    send_buf: Vec<Vec<u8>>,
    /// Non-blocking mode.
    pub non_blocking: bool,
    /// Backlog of pending connections (for listening sockets).
    pending_connections: Vec<(u32, u16)>,
}

/// BSD socket API — socket, bind, listen, accept, connect, send, recv.
#[derive(Debug)]
pub struct SocketApi {
    /// Open sockets keyed by fd.
    sockets: HashMap<u64, BsdSocket>,
    /// Next file descriptor.
    next_fd: u64,
    /// Bound ports.
    bound_ports: HashMap<u16, u64>,
    /// Network metrics.
    pub metrics: NetworkMetrics,
}

impl SocketApi {
    /// Creates a new socket API instance.
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            next_fd: 3, // 0,1,2 reserved for stdin/stdout/stderr
            bound_ports: HashMap::new(),
            metrics: NetworkMetrics::new(),
        }
    }

    /// Creates a new socket. Returns the file descriptor.
    pub fn socket(&mut self, kind: BsdSocketType) -> u64 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.sockets.insert(
            fd,
            BsdSocket {
                fd,
                kind,
                state: BsdSocketState::Created,
                local_addr: 0,
                local_port: 0,
                remote_addr: 0,
                remote_port: 0,
                recv_buf: Vec::new(),
                send_buf: Vec::new(),
                non_blocking: false,
                pending_connections: Vec::new(),
            },
        );
        fd
    }

    /// Binds a socket to an address and port.
    pub fn bind(&mut self, fd: u64, addr: u32, port: u16) -> Result<(), NetV2Error> {
        if self.bound_ports.contains_key(&port) {
            return Err(NetV2Error::PortInUse(port));
        }
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        sock.local_addr = addr;
        sock.local_port = port;
        sock.state = BsdSocketState::Bound;
        self.bound_ports.insert(port, fd);
        Ok(())
    }

    /// Sets a socket to listen mode.
    pub fn listen(&mut self, fd: u64, _backlog: u32) -> Result<(), NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        sock.state = BsdSocketState::Listening;
        Ok(())
    }

    /// Accepts a pending connection (returns new socket fd).
    pub fn accept(&mut self, fd: u64) -> Result<u64, NetV2Error> {
        let (remote_addr, remote_port) = {
            let sock = self
                .sockets
                .get_mut(&fd)
                .ok_or(NetV2Error::InvalidSocket(fd))?;
            if sock.pending_connections.is_empty() {
                return Err(NetV2Error::WouldBlock);
            }
            sock.pending_connections.remove(0)
        };

        // Create new connected socket
        let new_fd = self.next_fd;
        self.next_fd += 1;
        let local_addr = self
            .sockets
            .get(&fd)
            .map(|s| s.local_addr)
            .unwrap_or(0);
        let local_port = self
            .sockets
            .get(&fd)
            .map(|s| s.local_port)
            .unwrap_or(0);
        self.sockets.insert(
            new_fd,
            BsdSocket {
                fd: new_fd,
                kind: BsdSocketType::Stream,
                state: BsdSocketState::Connected,
                local_addr,
                local_port,
                remote_addr,
                remote_port,
                recv_buf: Vec::new(),
                send_buf: Vec::new(),
                non_blocking: false,
                pending_connections: Vec::new(),
            },
        );
        self.metrics.connection_opened();
        Ok(new_fd)
    }

    /// Connects a socket to a remote address.
    pub fn connect(
        &mut self,
        fd: u64,
        remote_addr: u32,
        remote_port: u16,
    ) -> Result<(), NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        sock.remote_addr = remote_addr;
        sock.remote_port = remote_port;
        sock.state = BsdSocketState::Connected;
        self.metrics.connection_opened();
        Ok(())
    }

    /// Sends data on a connected socket.
    pub fn send(&mut self, fd: u64, data: &[u8]) -> Result<usize, NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        if sock.state != BsdSocketState::Connected {
            return Err(NetV2Error::NotConnected);
        }
        let len = data.len();
        sock.send_buf.push(data.to_vec());
        self.metrics.record_send(len as u64);
        Ok(len)
    }

    /// Receives data from a socket.
    pub fn recv(&mut self, fd: u64) -> Result<Vec<u8>, NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        if sock.recv_buf.is_empty() {
            return Err(NetV2Error::WouldBlock);
        }
        let data = sock.recv_buf.remove(0);
        self.metrics.record_recv(data.len() as u64);
        Ok(data)
    }

    /// Injects data into a socket's receive buffer (for testing).
    pub fn inject_recv(&mut self, fd: u64, data: Vec<u8>) -> Result<(), NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        sock.recv_buf.push(data);
        Ok(())
    }

    /// Injects a pending connection into a listening socket (for testing).
    pub fn inject_connection(
        &mut self,
        fd: u64,
        remote_addr: u32,
        remote_port: u16,
    ) -> Result<(), NetV2Error> {
        let sock = self
            .sockets
            .get_mut(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        sock.pending_connections.push((remote_addr, remote_port));
        Ok(())
    }

    /// Closes a socket.
    pub fn close(&mut self, fd: u64) -> Result<(), NetV2Error> {
        let sock = self
            .sockets
            .remove(&fd)
            .ok_or(NetV2Error::InvalidSocket(fd))?;
        if sock.local_port > 0 {
            self.bound_ports.remove(&sock.local_port);
        }
        if sock.state == BsdSocketState::Connected {
            self.metrics.connection_closed();
        }
        Ok(())
    }

    /// Returns the number of open sockets.
    pub fn socket_count(&self) -> usize {
        self.sockets.len()
    }
}

impl Default for SocketApi {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ARP Table v2
// ═══════════════════════════════════════════════════════════════════════

/// ARP entry state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpV2State {
    /// Request sent, waiting for reply.
    Incomplete,
    /// Confirmed and reachable.
    Reachable,
    /// Entry has not been refreshed within timeout.
    Stale,
    /// Permanent (static) entry.
    Permanent,
}

/// ARP table entry.
#[derive(Debug, Clone)]
pub struct ArpV2Entry {
    /// IPv4 address.
    pub ip: u32,
    /// MAC address.
    pub mac: [u8; 6],
    /// Last confirmation timestamp.
    pub timestamp: u64,
    /// Entry state.
    pub state: ArpV2State,
}

/// ARP resolution cache with timeout and garbage collection.
#[derive(Debug)]
pub struct ArpTable {
    /// Entries keyed by IP.
    entries: HashMap<u32, ArpV2Entry>,
    /// Timeout in seconds before marking entries stale.
    pub timeout_secs: u64,
}

impl ArpTable {
    /// Creates a new ARP table with the given timeout.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            timeout_secs,
        }
    }

    /// Looks up the MAC for an IP.
    pub fn lookup(&self, ip: u32) -> Option<&ArpV2Entry> {
        self.entries.get(&ip)
    }

    /// Inserts or updates an entry as Reachable.
    pub fn insert(&mut self, ip: u32, mac: [u8; 6], now: u64) {
        self.entries.insert(
            ip,
            ArpV2Entry {
                ip,
                mac,
                timestamp: now,
                state: ArpV2State::Reachable,
            },
        );
    }

    /// Inserts a permanent (static) entry.
    pub fn insert_permanent(&mut self, ip: u32, mac: [u8; 6]) {
        self.entries.insert(
            ip,
            ArpV2Entry {
                ip,
                mac,
                timestamp: 0,
                state: ArpV2State::Permanent,
            },
        );
    }

    /// Inserts an incomplete entry (request sent).
    pub fn insert_incomplete(&mut self, ip: u32, now: u64) {
        self.entries.insert(
            ip,
            ArpV2Entry {
                ip,
                mac: [0; 6],
                timestamp: now,
                state: ArpV2State::Incomplete,
            },
        );
    }

    /// Garbage-collects stale and expired entries.
    ///
    /// Entries older than `timeout_secs` become Stale.
    /// Entries older than `2 * timeout_secs` are removed.
    /// Permanent entries are never removed.
    pub fn gc(&mut self, now: u64) {
        let timeout = self.timeout_secs;
        self.entries.retain(|_, entry| {
            if entry.state == ArpV2State::Permanent {
                return true;
            }
            let age = now.saturating_sub(entry.timestamp);
            if age > timeout * 2 {
                return false;
            }
            if age > timeout && entry.state == ArpV2State::Reachable {
                entry.state = ArpV2State::Stale;
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

    /// Removes an entry by IP.
    pub fn remove(&mut self, ip: u32) -> bool {
        self.entries.remove(&ip).is_some()
    }
}

impl Default for ArpTable {
    fn default() -> Self {
        Self::new(300) // 5-minute default timeout
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- WiFi Driver tests ---

    #[test]
    fn wifi_scan_returns_access_points() {
        let mut wifi = WifiDriver::new();
        let aps = wifi.scan();
        assert!(aps.len() >= 3);
        assert!(aps.iter().any(|ap| ap.ssid == "FajarOS-Lab"));
    }

    #[test]
    fn wifi_connect_wpa2_success() {
        let mut wifi = WifiDriver::new();
        assert!(wifi.connect("FajarOS-Lab", "fajarlan9!").is_ok());
        assert!(wifi.is_connected());
        assert_eq!(wifi.connected_ssid.as_deref(), Some("FajarOS-Lab"));
    }

    #[test]
    fn wifi_connect_wrong_password() {
        let mut wifi = WifiDriver::new();
        let result = wifi.connect("FajarOS-Lab", "wrong");
        assert!(result.is_err());
        assert!(!wifi.is_connected());
    }

    #[test]
    fn wifi_connect_unknown_ssid() {
        let mut wifi = WifiDriver::new();
        let result = wifi.connect("NonExistent", "pass");
        assert!(matches!(result, Err(NetV2Error::ApNotFound(_))));
    }

    #[test]
    fn wifi_disconnect() {
        let mut wifi = WifiDriver::new();
        wifi.connect("FajarOS-Lab", "fajarlan9!").ok();
        wifi.disconnect();
        assert!(!wifi.is_connected());
        assert_eq!(wifi.signal_dbm, -100);
    }

    #[test]
    fn wifi_signal_strength() {
        let mut wifi = WifiDriver::new();
        wifi.connect("FajarOS-Lab", "fajarlan9!").ok();
        assert!(wifi.signal_strength() > -100);
        assert!(wifi.signal_strength() < 0);
    }

    // --- IPv6 Stack tests ---

    #[test]
    fn ipv6_add_and_remove_address() {
        let mut stack = Ipv6Stack::new("eth0");
        let addr = Ipv6Addr::LOOPBACK;
        stack.add_address(addr);
        assert_eq!(stack.address_count(), 1);
        assert!(stack.remove_address(&addr));
        assert_eq!(stack.address_count(), 0);
    }

    #[test]
    fn ipv6_neighbor_discovery() {
        let mut stack = Ipv6Stack::new("eth0");
        let addr = Ipv6Addr::LOOPBACK;
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        stack.add_neighbor(addr, mac, 100);
        let entry = stack.lookup_neighbor(&addr);
        assert!(entry.is_some());
        assert_eq!(entry.map(|e| e.mac), Some(mac));
    }

    #[test]
    fn ipv6_routing_lookup() {
        let mut stack = Ipv6Stack::new("eth0");
        let prefix = Ipv6Addr::new([0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let next_hop =
            Ipv6Addr::new([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        stack.add_route(Ipv6Route {
            prefix,
            prefix_len: 32,
            next_hop,
            interface: "eth0".to_string(),
            metric: 100,
        });
        let dest =
            Ipv6Addr::new([0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let route = stack.route_lookup(&dest);
        assert!(route.is_some());
        assert_eq!(route.map(|r| r.prefix_len), Some(32));
    }

    #[test]
    fn ipv6_parse_and_display() {
        let addr = Ipv6Addr::parse("2001:0db8:0000:0000:0000:0000:0000:0001").unwrap();
        assert_eq!(addr.to_string(), "2001:0db8:0000:0000:0000:0000:0000:0001");
        assert!(!addr.is_link_local());
        assert!(!addr.is_multicast());
    }

    #[test]
    fn ipv6_link_local_detection() {
        let addr = Ipv6Addr::new([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert!(addr.is_link_local());
    }

    // --- DNS Resolver tests ---

    #[test]
    fn dns_resolve_known_host() {
        let mut dns = DnsResolver::new("10.0.2.3");
        let result = dns.resolve("fajaros.local", DnsRecordType::A, 0);
        assert!(result.is_ok());
        let records = result.unwrap();
        assert!(!records.is_empty());
        assert_eq!(records[0].value, "10.0.2.15");
    }

    #[test]
    fn dns_resolve_cname_chain() {
        let mut dns = DnsResolver::new("10.0.2.3");
        dns.add_zone_record(DnsRecord {
            name: "www.fajaros.local".to_string(),
            record_type: DnsRecordType::Cname,
            value: "fajaros.local".to_string(),
            ttl: 300,
        });
        let result = dns.resolve("www.fajaros.local", DnsRecordType::A, 0);
        assert!(result.is_ok());
        let records = result.unwrap();
        assert_eq!(records[0].value, "10.0.2.15");
    }

    #[test]
    fn dns_cache_hit() {
        let mut dns = DnsResolver::new("10.0.2.3");
        // First resolve populates cache
        dns.resolve("fajaros.local", DnsRecordType::A, 0).ok();
        assert!(dns.cache_size() > 0);
        // Second resolve should hit cache
        let result = dns.resolve("fajaros.local", DnsRecordType::A, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn dns_cache_expiry() {
        let mut dns = DnsResolver::new("10.0.2.3");
        dns.resolve("fajaros.local", DnsRecordType::A, 0).ok();
        dns.gc(100_000); // far future
        assert_eq!(dns.cache_size(), 0);
    }

    #[test]
    fn dns_unknown_host() {
        let mut dns = DnsResolver::new("10.0.2.3");
        let result = dns.resolve("no-such-host.invalid", DnsRecordType::A, 0);
        assert!(result.is_err());
    }

    // --- DHCP Client tests ---

    #[test]
    fn dhcp_discover_offer_request_ack() {
        let mut dhcp = DhcpClient::new(0x12345678, [0x52, 0x54, 0x00, 0xAB, 0xCD, 0xEF]);
        assert_eq!(dhcp.state, DhcpV2State::Init);

        let pkt = dhcp.discover();
        assert!(!pkt.is_empty());
        assert_eq!(dhcp.state, DhcpV2State::Selecting);

        // Simulate offer
        let ip = (10 << 24) | (0 << 16) | (2 << 8) | 15;
        let mask = (255 << 24) | (255 << 16) | (255 << 8) | 0;
        let gw = (10 << 24) | (0 << 16) | (2 << 8) | 2;
        let dns_server = (8 << 24) | (8 << 16) | (8 << 8) | 8;
        dhcp.receive_offer(ip, mask, gw, vec![dns_server], gw);
        assert_eq!(dhcp.state, DhcpV2State::Requesting);

        // Simulate ACK
        dhcp.receive_ack(3600, 100);
        assert_eq!(dhcp.state, DhcpV2State::Bound);
        assert!(dhcp.is_bound());

        let lease = dhcp.lease().unwrap();
        assert_eq!(lease.address, ip);
        assert_eq!(lease.lease_secs, 3600);
    }

    #[test]
    fn dhcp_lease_renewal() {
        let mut dhcp = DhcpClient::new(1, [0; 6]);
        dhcp.discover();
        dhcp.receive_offer(1, 2, 3, vec![], 3);
        dhcp.receive_ack(1000, 0);
        assert_eq!(dhcp.state, DhcpV2State::Bound);

        // At T1 (50% of 1000 = 500)
        dhcp.check_renewal(500);
        assert_eq!(dhcp.state, DhcpV2State::Renewing);
    }

    #[test]
    fn dhcp_lease_rebinding() {
        let mut dhcp = DhcpClient::new(1, [0; 6]);
        dhcp.discover();
        dhcp.receive_offer(1, 2, 3, vec![], 3);
        dhcp.receive_ack(1000, 0);

        // At T2 (87.5% of 1000 = 875)
        dhcp.check_renewal(875);
        assert_eq!(dhcp.state, DhcpV2State::Rebinding);
    }

    #[test]
    fn dhcp_lease_expiry() {
        let mut dhcp = DhcpClient::new(1, [0; 6]);
        dhcp.discover();
        dhcp.receive_offer(1, 2, 3, vec![], 3);
        dhcp.receive_ack(1000, 0);

        dhcp.check_renewal(1001);
        assert_eq!(dhcp.state, DhcpV2State::Init);
        assert!(dhcp.lease().is_none());
    }

    // --- Firewall tests ---

    #[test]
    fn firewall_allow_by_rule() {
        let mut fw = Firewall::new(FirewallAction::Deny);
        fw.add_rule(FirewallRule {
            priority: 10,
            action: FirewallAction::Allow,
            direction: FirewallDirection::Inbound,
            protocol: FirewallProto::Tcp,
            src_addr: 0,
            dst_addr: 0,
            src_port: 0,
            dst_port: 80,
            description: "Allow HTTP".to_string(),
        });

        let pkt = PacketContext {
            src_addr: 1,
            dst_addr: 2,
            src_port: 50000,
            dst_port: 80,
            protocol: FirewallProto::Tcp,
            direction: FirewallDirection::Inbound,
        };
        assert_eq!(fw.evaluate(&pkt), FirewallAction::Allow);
    }

    #[test]
    fn firewall_deny_by_default() {
        let mut fw = Firewall::new(FirewallAction::Deny);
        let pkt = PacketContext {
            src_addr: 1,
            dst_addr: 2,
            src_port: 50000,
            dst_port: 443,
            protocol: FirewallProto::Tcp,
            direction: FirewallDirection::Inbound,
        };
        assert_eq!(fw.evaluate(&pkt), FirewallAction::Deny);
    }

    #[test]
    fn firewall_priority_ordering() {
        let mut fw = Firewall::new(FirewallAction::Allow);
        fw.add_rule(FirewallRule {
            priority: 20,
            action: FirewallAction::Allow,
            direction: FirewallDirection::Both,
            protocol: FirewallProto::Tcp,
            src_addr: 0,
            dst_addr: 0,
            src_port: 0,
            dst_port: 22,
            description: "Allow SSH".to_string(),
        });
        fw.add_rule(FirewallRule {
            priority: 10,
            action: FirewallAction::Deny,
            direction: FirewallDirection::Both,
            protocol: FirewallProto::Any,
            src_addr: 0,
            dst_addr: 0,
            src_port: 0,
            dst_port: 22,
            description: "Block port 22".to_string(),
        });

        let pkt = PacketContext {
            src_addr: 1,
            dst_addr: 2,
            src_port: 50000,
            dst_port: 22,
            protocol: FirewallProto::Tcp,
            direction: FirewallDirection::Inbound,
        };
        // Priority 10 rule (Deny) should match first
        assert_eq!(fw.evaluate(&pkt), FirewallAction::Deny);
    }

    #[test]
    fn firewall_remove_rule() {
        let mut fw = Firewall::new(FirewallAction::Allow);
        fw.add_rule(FirewallRule {
            priority: 10,
            action: FirewallAction::Deny,
            direction: FirewallDirection::Both,
            protocol: FirewallProto::Any,
            src_addr: 0,
            dst_addr: 0,
            src_port: 0,
            dst_port: 0,
            description: "Block all".to_string(),
        });
        assert_eq!(fw.rule_count(), 1);
        fw.remove_rule("Block all");
        assert_eq!(fw.rule_count(), 0);
    }

    // --- TCP Optimizations tests ---

    #[test]
    fn tcp_window_scaling() {
        let ws = TcpWindowScale::new(7);
        assert_eq!(ws.effective_window(), 65535 * 128); // 2^7 = 128
    }

    #[test]
    fn tcp_congestion_slow_start() {
        let mut tcp = TcpOptimizations::new(7);
        let initial = tcp.cwnd;
        tcp.on_ack();
        assert!(tcp.cwnd > initial);
    }

    #[test]
    fn tcp_fast_retransmit() {
        let mut tcp = TcpOptimizations::new(7);
        tcp.cwnd = 20;
        tcp.on_dup_ack();
        tcp.on_dup_ack();
        tcp.on_dup_ack(); // 3rd dup ACK triggers fast retransmit
        assert!(tcp.ssthresh < 20);
    }

    #[test]
    fn tcp_timeout_resets() {
        let mut tcp = TcpOptimizations::new(7);
        tcp.cwnd = 100;
        tcp.on_timeout();
        assert_eq!(tcp.cwnd, 1);
        assert!(tcp.ssthresh < 100);
    }

    #[test]
    fn tcp_rtt_update() {
        let mut tcp = TcpOptimizations::new(7);
        tcp.update_rtt(100);
        assert_eq!(tcp.rtt_ms, 100);
        tcp.update_rtt(200);
        // SRTT = 7/8 * 100 + 1/8 * 200 = 87.5 + 25 = 112
        assert!(tcp.rtt_ms > 100 && tcp.rtt_ms < 200);
    }

    // --- Network Metrics tests ---

    #[test]
    fn metrics_tracking() {
        let mut m = NetworkMetrics::new();
        m.record_send(1000);
        m.record_recv(500);
        m.record_loss();
        assert_eq!(m.bytes_sent, 1000);
        assert_eq!(m.bytes_received, 500);
        assert_eq!(m.packets_sent, 1);
        assert_eq!(m.packets_received, 1);
        assert_eq!(m.packets_lost, 1);
    }

    #[test]
    fn metrics_latency() {
        let mut m = NetworkMetrics::new();
        m.record_latency(10);
        m.record_latency(20);
        m.record_latency(30);
        assert!((m.avg_latency() - 20.0).abs() < 0.01);
    }

    #[test]
    fn metrics_loss_ratio() {
        let mut m = NetworkMetrics::new();
        m.record_send(100);
        m.record_send(100);
        m.record_recv(100);
        m.record_loss();
        // 1 lost out of 3 total (2 sent + 1 recv)
        assert!(m.loss_ratio() > 0.0 && m.loss_ratio() < 1.0);
    }

    #[test]
    fn metrics_bandwidth() {
        let mut m = NetworkMetrics::new();
        m.record_send(1_000_000);
        m.record_recv(500_000);
        let bps = m.bandwidth_bps(1.0);
        assert!((bps - 1_500_000.0).abs() < 0.01);
    }

    // --- Socket API tests ---

    #[test]
    fn socket_create_bind_listen_accept() {
        let mut api = SocketApi::new();
        let fd = api.socket(BsdSocketType::Stream);
        api.bind(fd, 0, 8080).unwrap();
        api.listen(fd, 5).unwrap();

        // Inject a connection
        api.inject_connection(fd, 1, 50000).unwrap();
        let client_fd = api.accept(fd).unwrap();
        assert!(client_fd > fd);
        assert_eq!(api.socket_count(), 2);
    }

    #[test]
    fn socket_connect_send_recv() {
        let mut api = SocketApi::new();
        let fd = api.socket(BsdSocketType::Stream);
        api.connect(fd, 1, 80).unwrap();
        api.send(fd, b"GET / HTTP/1.1\r\n").unwrap();
        api.inject_recv(fd, b"HTTP/1.1 200 OK\r\n".to_vec())
            .unwrap();
        let response = api.recv(fd).unwrap();
        assert_eq!(response, b"HTTP/1.1 200 OK\r\n");
    }

    #[test]
    fn socket_close_frees_port() {
        let mut api = SocketApi::new();
        let fd = api.socket(BsdSocketType::Stream);
        api.bind(fd, 0, 9090).unwrap();
        api.close(fd).unwrap();

        // Port should be available again
        let fd2 = api.socket(BsdSocketType::Stream);
        assert!(api.bind(fd2, 0, 9090).is_ok());
    }

    #[test]
    fn socket_port_in_use() {
        let mut api = SocketApi::new();
        let fd1 = api.socket(BsdSocketType::Stream);
        api.bind(fd1, 0, 8080).unwrap();
        let fd2 = api.socket(BsdSocketType::Stream);
        assert!(matches!(
            api.bind(fd2, 0, 8080),
            Err(NetV2Error::PortInUse(8080))
        ));
    }

    // --- ARP Table tests ---

    #[test]
    fn arp_insert_and_lookup() {
        let mut arp = ArpTable::new(300);
        arp.insert(0x0A000201, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01], 100);
        let entry = arp.lookup(0x0A000201).unwrap();
        assert_eq!(entry.state, ArpV2State::Reachable);
        assert_eq!(entry.mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x01]);
    }

    #[test]
    fn arp_permanent_entry_survives_gc() {
        let mut arp = ArpTable::new(10);
        arp.insert_permanent(0x0A000201, [1, 2, 3, 4, 5, 6]);
        arp.gc(100_000);
        assert_eq!(arp.len(), 1);
    }

    #[test]
    fn arp_gc_removes_expired() {
        let mut arp = ArpTable::new(10);
        arp.insert(0x0A000201, [1, 2, 3, 4, 5, 6], 0);
        arp.gc(25); // 25 > 2 * 10 = 20 -> removed
        assert_eq!(arp.len(), 0);
    }

    #[test]
    fn arp_gc_marks_stale() {
        let mut arp = ArpTable::new(10);
        arp.insert(0x0A000201, [1, 2, 3, 4, 5, 6], 0);
        arp.gc(15); // 15 > 10 but < 20 -> stale, not removed
        assert_eq!(arp.len(), 1);
        let entry = arp.lookup(0x0A000201).unwrap();
        assert_eq!(entry.state, ArpV2State::Stale);
    }
}
