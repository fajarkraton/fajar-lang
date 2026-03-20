//! Network stack simulation for FajarOS.
//!
//! Provides simulated Ethernet/IP/TCP/UDP for interpreter testing.
//! Uses in-memory packet queues — no real network I/O.

use std::collections::HashMap;

/// Network errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NetError {
    /// Socket not found.
    #[error("invalid socket: {0}")]
    InvalidSocket(u64),
    /// Port already in use.
    #[error("port already in use: {0}")]
    PortInUse(u16),
    /// Connection refused.
    #[error("connection refused: {0}:{1}")]
    ConnectionRefused(u32, u16),
    /// Not connected.
    #[error("socket not connected")]
    NotConnected,
    /// No data available.
    #[error("no data available")]
    WouldBlock,
}

/// IP address (IPv4).
pub type Ipv4Addr = u32;

/// Socket type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP stream socket.
    Tcp,
    /// UDP datagram socket.
    Udp,
}

/// Socket state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    /// Created but not bound.
    Closed,
    /// Bound to a port.
    Bound,
    /// Listening for connections (TCP).
    Listening,
    /// Connected to peer (TCP).
    Connected,
}

/// Simulated network socket.
#[derive(Debug)]
pub struct Socket {
    /// Socket ID.
    pub id: u64,
    /// Socket type.
    pub kind: SocketType,
    /// Current state.
    pub state: SocketState,
    /// Local IP address.
    pub local_addr: Ipv4Addr,
    /// Local port.
    pub local_port: u16,
    /// Remote IP address (TCP connected).
    pub remote_addr: Ipv4Addr,
    /// Remote port (TCP connected).
    pub remote_port: u16,
    /// Receive buffer.
    pub recv_buf: Vec<Vec<u8>>,
    /// Send buffer.
    pub send_buf: Vec<Vec<u8>>,
}

/// Ethernet frame.
#[derive(Debug, Clone)]
pub struct EthernetFrame {
    /// Destination MAC.
    pub dst_mac: [u8; 6],
    /// Source MAC.
    pub src_mac: [u8; 6],
    /// EtherType (0x0800 = IPv4, 0x0806 = ARP).
    pub ethertype: u16,
    /// Payload.
    pub payload: Vec<u8>,
}

impl EthernetFrame {
    /// Parses an Ethernet frame from bytes.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }
        let mut dst_mac = [0u8; 6];
        let mut src_mac = [0u8; 6];
        dst_mac.copy_from_slice(&data[0..6]);
        src_mac.copy_from_slice(&data[6..12]);
        let ethertype = u16::from_be_bytes([data[12], data[13]]);
        let payload = data[14..].to_vec();
        Some(Self {
            dst_mac,
            src_mac,
            ethertype,
            payload,
        })
    }

    /// Serializes frame to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(14 + self.payload.len());
        out.extend_from_slice(&self.dst_mac);
        out.extend_from_slice(&self.src_mac);
        out.extend_from_slice(&self.ethertype.to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }
}

/// ARP table entry.
#[derive(Debug, Clone)]
pub struct ArpEntry {
    /// IP address.
    pub ip: Ipv4Addr,
    /// MAC address.
    pub mac: [u8; 6],
}

/// Simulated network stack.
#[derive(Debug)]
pub struct NetworkStack {
    /// Our MAC address.
    pub mac: [u8; 6],
    /// Our IP address.
    pub ip: Ipv4Addr,
    /// Subnet mask.
    pub netmask: Ipv4Addr,
    /// Gateway.
    pub gateway: Ipv4Addr,
    /// ARP table.
    pub arp_table: HashMap<Ipv4Addr, [u8; 6]>,
    /// Open sockets.
    sockets: HashMap<u64, Socket>,
    /// Next socket ID.
    next_socket_id: u64,
    /// Bound ports (port -> socket_id).
    bound_ports: HashMap<u16, u64>,
}

impl NetworkStack {
    /// Creates a new network stack with default settings.
    pub fn new() -> Self {
        Self {
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56], // QEMU default
            ip: Self::ip(10, 0, 2, 15),                // QEMU user net
            netmask: Self::ip(255, 255, 255, 0),
            gateway: Self::ip(10, 0, 2, 2),
            arp_table: HashMap::new(),
            sockets: HashMap::new(),
            next_socket_id: 1,
            bound_ports: HashMap::new(),
        }
    }

    /// Constructs an IPv4 address from octets.
    pub fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
    }

    /// Creates a new socket.
    pub fn socket(&mut self, kind: SocketType) -> u64 {
        let id = self.next_socket_id;
        self.next_socket_id += 1;
        self.sockets.insert(
            id,
            Socket {
                id,
                kind,
                state: SocketState::Closed,
                local_addr: self.ip,
                local_port: 0,
                remote_addr: 0,
                remote_port: 0,
                recv_buf: Vec::new(),
                send_buf: Vec::new(),
            },
        );
        id
    }

    /// Binds a socket to a port.
    pub fn bind(&mut self, socket_id: u64, port: u16) -> Result<(), NetError> {
        if self.bound_ports.contains_key(&port) {
            return Err(NetError::PortInUse(port));
        }
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        sock.local_port = port;
        sock.state = SocketState::Bound;
        self.bound_ports.insert(port, socket_id);
        Ok(())
    }

    /// Puts a TCP socket into listening state.
    pub fn listen(&mut self, socket_id: u64) -> Result<(), NetError> {
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        sock.state = SocketState::Listening;
        Ok(())
    }

    /// Connects a TCP socket to a remote address.
    pub fn connect(
        &mut self,
        socket_id: u64,
        remote_addr: Ipv4Addr,
        remote_port: u16,
    ) -> Result<(), NetError> {
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        sock.remote_addr = remote_addr;
        sock.remote_port = remote_port;
        sock.state = SocketState::Connected;
        Ok(())
    }

    /// Sends data on a connected socket.
    pub fn send(&mut self, socket_id: u64, data: &[u8]) -> Result<usize, NetError> {
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        if sock.state != SocketState::Connected {
            return Err(NetError::NotConnected);
        }
        let len = data.len();
        sock.send_buf.push(data.to_vec());
        Ok(len)
    }

    /// Receives data from a socket's receive buffer.
    pub fn recv(&mut self, socket_id: u64) -> Result<Vec<u8>, NetError> {
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        if sock.recv_buf.is_empty() {
            return Err(NetError::WouldBlock);
        }
        Ok(sock.recv_buf.remove(0))
    }

    /// Injects data into a socket's receive buffer (for testing).
    pub fn inject_recv(&mut self, socket_id: u64, data: Vec<u8>) -> Result<(), NetError> {
        let sock = self
            .sockets
            .get_mut(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        sock.recv_buf.push(data);
        Ok(())
    }

    /// Closes a socket.
    pub fn close(&mut self, socket_id: u64) -> Result<(), NetError> {
        let sock = self
            .sockets
            .remove(&socket_id)
            .ok_or(NetError::InvalidSocket(socket_id))?;
        if sock.local_port > 0 {
            self.bound_ports.remove(&sock.local_port);
        }
        Ok(())
    }

    /// Handles an incoming ICMP echo request (ping).
    pub fn handle_icmp_echo(&self, src_ip: Ipv4Addr) -> Vec<u8> {
        // Build a minimal ICMP echo reply
        let mut reply = Vec::with_capacity(28);
        // IP header (20 bytes, simplified)
        reply.extend_from_slice(&[0x45, 0x00, 0x00, 0x1C]); // ver+ihl, tos, len
        reply.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // id, flags+offset
        reply.extend_from_slice(&[0x40, 0x01, 0x00, 0x00]); // ttl=64, proto=ICMP
        reply.extend_from_slice(&self.ip.to_be_bytes()); // src ip
        reply.extend_from_slice(&src_ip.to_be_bytes()); // dst ip
        // ICMP echo reply (8 bytes)
        reply.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // type=0 (reply), checksum
        reply.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]); // id, seq
        reply
    }

    /// Adds an ARP entry.
    pub fn arp_add(&mut self, ip: Ipv4Addr, mac: [u8; 6]) {
        self.arp_table.insert(ip, mac);
    }

    /// Looks up a MAC for an IP.
    pub fn arp_lookup(&self, ip: Ipv4Addr) -> Option<[u8; 6]> {
        self.arp_table.get(&ip).copied()
    }
}

impl Default for NetworkStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s27_1_ethernet_frame_parse() {
        let mut data = vec![0u8; 20];
        data[0..6].copy_from_slice(&[0xFF; 6]); // dst broadcast
        data[6..12].copy_from_slice(&[0x52, 0x54, 0x00, 0x12, 0x34, 0x56]); // src
        data[12..14].copy_from_slice(&[0x08, 0x00]); // IPv4
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6]); // payload

        let frame = EthernetFrame::parse(&data).unwrap();
        assert_eq!(frame.ethertype, 0x0800);
        assert_eq!(frame.payload.len(), 12);
    }

    #[test]
    fn s27_2_arp_table() {
        let mut net = NetworkStack::new();
        let ip = NetworkStack::ip(192, 168, 1, 1);
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        net.arp_add(ip, mac);
        assert_eq!(net.arp_lookup(ip), Some(mac));
        assert_eq!(net.arp_lookup(NetworkStack::ip(1, 2, 3, 4)), None);
    }

    #[test]
    fn s27_3_socket_create_and_bind() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Tcp);
        net.bind(sid, 8080).unwrap();
        assert!(matches!(
            net.bind(sid, 8080),
            Err(NetError::PortInUse(8080))
        ));
    }

    #[test]
    fn s27_4_icmp_echo_reply() {
        let net = NetworkStack::new();
        let reply = net.handle_icmp_echo(NetworkStack::ip(10, 0, 2, 2));
        assert_eq!(reply.len(), 28);
        assert_eq!(reply[9], 0x01); // ICMP protocol
        assert_eq!(reply[20], 0x00); // Echo reply type
    }

    #[test]
    fn s27_5_udp_socket() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Udp);
        net.bind(sid, 5353).unwrap();
        net.connect(sid, NetworkStack::ip(10, 0, 2, 2), 53).unwrap();
        net.send(sid, b"query").unwrap();
        net.inject_recv(sid, b"response".to_vec()).unwrap();
        let data = net.recv(sid).unwrap();
        assert_eq!(data, b"response");
    }

    #[test]
    fn s27_6_tcp_connect_send_recv() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Tcp);
        net.connect(sid, NetworkStack::ip(93, 184, 216, 34), 80)
            .unwrap();

        let sent = net.send(sid, b"GET / HTTP/1.0\r\n\r\n").unwrap();
        assert_eq!(sent, 18);

        net.inject_recv(sid, b"HTTP/1.0 200 OK\r\n\r\n".to_vec())
            .unwrap();
        let data = net.recv(sid).unwrap();
        assert!(data.starts_with(b"HTTP/1.0 200"));
    }

    #[test]
    fn s27_7_send_not_connected() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Tcp);
        assert!(matches!(
            net.send(sid, b"data"),
            Err(NetError::NotConnected)
        ));
    }

    #[test]
    fn s27_8_recv_would_block() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Tcp);
        net.connect(sid, NetworkStack::ip(1, 1, 1, 1), 53).unwrap();
        assert!(matches!(net.recv(sid), Err(NetError::WouldBlock)));
    }

    #[test]
    fn s27_9_close_socket() {
        let mut net = NetworkStack::new();
        let sid = net.socket(SocketType::Tcp);
        net.bind(sid, 9090).unwrap();
        net.close(sid).unwrap();
        // Port should be freed
        let sid2 = net.socket(SocketType::Tcp);
        assert!(net.bind(sid2, 9090).is_ok());
    }

    #[test]
    fn s27_10_ethernet_frame_roundtrip() {
        let frame = EthernetFrame {
            dst_mac: [0xFF; 6],
            src_mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            ethertype: 0x0800,
            payload: vec![1, 2, 3, 4],
        };
        let bytes = frame.to_bytes();
        let parsed = EthernetFrame::parse(&bytes).unwrap();
        assert_eq!(parsed.ethertype, 0x0800);
        assert_eq!(parsed.payload, vec![1, 2, 3, 4]);
    }
}
