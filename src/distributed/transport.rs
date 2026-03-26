//! Real TCP Transport Layer for Distributed Computing.
//!
//! V8 GC3: Provides actual TCP networking for actor messages,
//! RPC calls, and distributed tensor operations.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

// ═══════════════════════════════════════════════════════════════════════
// GC3.2-GC3.3: TCP Transport + Message Serialization
// ═══════════════════════════════════════════════════════════════════════

/// A message sent over the network between nodes.
#[derive(Debug, Clone)]
pub struct NetMessage {
    /// Message type tag.
    pub msg_type: MessageType,
    /// Target actor or service name.
    pub target: String,
    /// Payload bytes (serialized via serde_json).
    pub payload: Vec<u8>,
    /// Sender node ID.
    pub sender_id: u64,
    /// Monotonic sequence number.
    pub seq: u64,
}

/// Message type for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// Actor mailbox message.
    ActorMessage,
    /// RPC request.
    RpcRequest,
    /// RPC response.
    RpcResponse,
    /// Heartbeat ping.
    Heartbeat,
    /// Heartbeat pong.
    HeartbeatAck,
    /// Cluster join request.
    Join,
    /// Cluster join accepted.
    JoinAck,
    /// Tensor shard data.
    TensorShard,
    /// Gradient for allreduce.
    Gradient,
}

impl NetMessage {
    /// Serialize to bytes: [type:1][target_len:4][target][payload_len:4][payload][sender:8][seq:8]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.msg_type as u8);

        let target_bytes = self.target.as_bytes();
        buf.extend_from_slice(&(target_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(target_bytes);

        buf.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.payload);

        buf.extend_from_slice(&self.sender_id.to_le_bytes());
        buf.extend_from_slice(&self.seq.to_le_bytes());
        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 1 + 4 {
            return Err("message too short".to_string());
        }

        let msg_type = match data[0] {
            0 => MessageType::ActorMessage,
            1 => MessageType::RpcRequest,
            2 => MessageType::RpcResponse,
            3 => MessageType::Heartbeat,
            4 => MessageType::HeartbeatAck,
            5 => MessageType::Join,
            6 => MessageType::JoinAck,
            7 => MessageType::TensorShard,
            8 => MessageType::Gradient,
            _ => return Err(format!("unknown message type: {}", data[0])),
        };

        let mut pos = 1;
        let target_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let target = String::from_utf8_lossy(&data[pos..pos + target_len]).into_owned();
        pos += target_len;

        let payload_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let payload = data[pos..pos + payload_len].to_vec();
        pos += payload_len;

        let sender_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let seq = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());

        Ok(NetMessage {
            msg_type,
            target,
            payload,
            sender_id,
            seq,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GC3.4-GC3.5: Actor Mailbox + Spawn
// ═══════════════════════════════════════════════════════════════════════

/// A real actor with a tokio mpsc mailbox.
pub struct Actor {
    /// Actor name.
    pub name: String,
    /// Incoming message channel.
    pub mailbox_tx: mpsc::Sender<NetMessage>,
    /// Incoming message receiver (owned by the actor task).
    mailbox_rx: Option<mpsc::Receiver<NetMessage>>,
}

impl Actor {
    /// Create a new actor with a bounded mailbox.
    pub fn new(name: &str, mailbox_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(mailbox_size);
        Actor {
            name: name.to_string(),
            mailbox_tx: tx,
            mailbox_rx: Some(rx),
        }
    }

    /// Send a message to this actor's mailbox.
    pub async fn send(&self, msg: NetMessage) -> Result<(), String> {
        self.mailbox_tx
            .send(msg)
            .await
            .map_err(|e| format!("mailbox send failed: {e}"))
    }

    /// Take ownership of the receiver (for spawning the actor task).
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<NetMessage>> {
        self.mailbox_rx.take()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GC3.6-GC3.10: Transport Node
// ═══════════════════════════════════════════════════════════════════════

/// A transport node that listens for connections and routes messages.
pub struct TransportNode {
    /// This node's ID.
    pub node_id: u64,
    /// Address this node is listening on.
    pub listen_addr: String,
    /// Known peer addresses.
    pub peers: Arc<Mutex<HashMap<u64, String>>>,
    /// Local actors.
    pub actors: Arc<Mutex<HashMap<String, mpsc::Sender<NetMessage>>>>,
    /// Message counter for sequencing.
    seq: Arc<Mutex<u64>>,
}

impl TransportNode {
    /// Create a new transport node.
    pub fn new(node_id: u64, listen_addr: &str) -> Self {
        TransportNode {
            node_id,
            listen_addr: listen_addr.to_string(),
            peers: Arc::new(Mutex::new(HashMap::new())),
            actors: Arc::new(Mutex::new(HashMap::new())),
            seq: Arc::new(Mutex::new(0)),
        }
    }

    /// Register a local actor.
    pub fn register_actor(&self, name: &str, sender: mpsc::Sender<NetMessage>) {
        self.actors.lock().unwrap().insert(name.to_string(), sender);
    }

    /// Add a known peer.
    pub fn add_peer(&self, peer_id: u64, addr: &str) {
        self.peers.lock().unwrap().insert(peer_id, addr.to_string());
    }

    /// Send a message to a remote node via TCP.
    pub async fn send_to_node(&self, peer_id: u64, msg: NetMessage) -> Result<(), String> {
        let addr = {
            let peers = self.peers.lock().unwrap();
            peers
                .get(&peer_id)
                .ok_or_else(|| format!("unknown peer: {peer_id}"))?
                .clone()
        };

        let mut stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("connect to {addr}: {e}"))?;

        let data = msg.to_bytes();
        let len = data.len() as u32;
        stream
            .write_all(&len.to_le_bytes())
            .await
            .map_err(|e| format!("write len: {e}"))?;
        stream
            .write_all(&data)
            .await
            .map_err(|e| format!("write data: {e}"))?;
        stream.flush().await.map_err(|e| format!("flush: {e}"))?;

        Ok(())
    }

    /// Start listening for incoming connections. Returns the actual bound address.
    pub async fn start_listener(&self) -> Result<String, String> {
        let listener = TcpListener::bind(&self.listen_addr)
            .await
            .map_err(|e| format!("bind {}: {e}", self.listen_addr))?;
        let actual_addr = listener
            .local_addr()
            .map_err(|e| format!("local_addr: {e}"))?
            .to_string();

        let actors = self.actors.clone();

        tokio::spawn(async move {
            while let Ok((mut stream, _addr)) = listener.accept().await {
                {
                    let actors = actors.clone();
                    tokio::spawn(async move {
                        let mut len_buf = [0u8; 4];
                        if stream.read_exact(&mut len_buf).await.is_err() {
                            return;
                        }
                        let len = u32::from_le_bytes(len_buf) as usize;
                        let mut data = vec![0u8; len];
                        if stream.read_exact(&mut data).await.is_err() {
                            return;
                        }
                        if let Ok(msg) = NetMessage::from_bytes(&data) {
                            let actors = actors.lock().unwrap();
                            if let Some(sender) = actors.get(&msg.target) {
                                let _ = sender.try_send(msg);
                            }
                        }
                    });
                }
            }
        });

        Ok(actual_addr)
    }

    /// Next sequence number.
    pub fn next_seq(&self) -> u64 {
        let mut seq = self.seq.lock().unwrap();
        *seq += 1;
        *seq
    }

    /// Create a heartbeat message.
    pub fn heartbeat_msg(&self) -> NetMessage {
        NetMessage {
            msg_type: MessageType::Heartbeat,
            target: "system".to_string(),
            payload: vec![],
            sender_id: self.node_id,
            seq: self.next_seq(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.1: Reconnection with Exponential Backoff
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for retry/reconnection logic.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Backoff multiplier (typically 2 for exponential).
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff_ms: 100,
            max_backoff_ms: 10_000,
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate the backoff delay for a given attempt (0-based).
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        let delay = self.initial_backoff_ms as f64 * self.multiplier.powi(attempt as i32);
        (delay as u64).min(self.max_backoff_ms)
    }
}

impl TransportNode {
    /// Send a message with automatic retry and exponential backoff.
    ///
    /// Attempts `config.max_retries` reconnections with increasing delays.
    /// Returns the number of attempts made on success, or the last error.
    pub async fn send_with_retry(
        &self,
        peer_id: u64,
        msg: NetMessage,
        config: &RetryConfig,
    ) -> Result<u32, String> {
        let mut last_err = String::new();
        for attempt in 0..=config.max_retries {
            match self.send_to_node(peer_id, msg.clone()).await {
                Ok(()) => return Ok(attempt + 1),
                Err(e) => {
                    last_err = e;
                    if attempt < config.max_retries {
                        let delay = config.backoff_ms(attempt);
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                }
            }
        }
        Err(format!(
            "failed after {} attempts: {last_err}",
            config.max_retries + 1
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.5: Connection Pool — reuse TCP streams
// ═══════════════════════════════════════════════════════════════════════

/// A pool of TCP connections keyed by peer address.
pub struct ConnectionPool {
    /// Open connections per address.
    pool: Arc<Mutex<HashMap<String, Vec<TcpStream>>>>,
    /// Maximum connections per address.
    max_per_addr: usize,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(max_per_addr: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
            max_per_addr,
        }
    }

    /// Get a connection from the pool, or create a new one.
    pub async fn get_or_connect(&self, addr: &str) -> Result<TcpStream, String> {
        // Try to get an existing connection
        {
            let mut pool = self.pool.lock().unwrap();
            if let Some(conns) = pool.get_mut(addr) {
                if let Some(stream) = conns.pop() {
                    return Ok(stream);
                }
            }
        }

        // Create a new connection
        TcpStream::connect(addr)
            .await
            .map_err(|e| format!("connect to {addr}: {e}"))
    }

    /// Return a connection to the pool for reuse.
    pub fn return_connection(&self, addr: &str, stream: TcpStream) {
        let mut pool = self.pool.lock().unwrap();
        let conns = pool.entry(addr.to_string()).or_default();
        if conns.len() < self.max_per_addr {
            conns.push(stream);
        }
        // else: drop the connection (pool is full)
    }

    /// Number of pooled connections for an address.
    pub fn pooled_count(&self, addr: &str) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.get(addr).map_or(0, |c| c.len())
    }

    /// Total pooled connections across all addresses.
    pub fn total_pooled(&self) -> usize {
        let pool = self.pool.lock().unwrap();
        pool.values().map(|c| c.len()).sum()
    }

    /// Clear all pooled connections for a given address.
    pub fn clear_addr(&self, addr: &str) {
        let mut pool = self.pool.lock().unwrap();
        pool.remove(addr);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.7: Service Discovery via UDP Multicast
// ═══════════════════════════════════════════════════════════════════════

/// A service announcement for discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAnnouncement {
    /// Node ID.
    pub node_id: u64,
    /// Service name (e.g., "worker", "scheduler").
    pub service_name: String,
    /// TCP address for RPC.
    pub rpc_addr: String,
    /// Timestamp (epoch milliseconds).
    pub timestamp_ms: u64,
}

impl ServiceAnnouncement {
    /// Serialize to bytes: [node_id:8][ts:8][name_len:4][name][addr_len:4][addr]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.node_id.to_le_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        let name = self.service_name.as_bytes();
        buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
        buf.extend_from_slice(name);
        let addr = self.rpc_addr.as_bytes();
        buf.extend_from_slice(&(addr.len() as u32).to_le_bytes());
        buf.extend_from_slice(addr);
        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 20 {
            return Err("announcement too short".to_string());
        }
        let node_id = u64::from_le_bytes(
            data[0..8]
                .try_into()
                .map_err(|_| "bad node_id".to_string())?,
        );
        let timestamp_ms = u64::from_le_bytes(
            data[8..16]
                .try_into()
                .map_err(|_| "bad timestamp".to_string())?,
        );
        let name_len = u32::from_le_bytes(
            data[16..20]
                .try_into()
                .map_err(|_| "bad name_len".to_string())?,
        ) as usize;
        if data.len() < 20 + name_len + 4 {
            return Err("announcement truncated".to_string());
        }
        let service_name = String::from_utf8(data[20..20 + name_len].to_vec())
            .map_err(|_| "bad name".to_string())?;
        let addr_start = 20 + name_len;
        let addr_len = u32::from_le_bytes(
            data[addr_start..addr_start + 4]
                .try_into()
                .map_err(|_| "bad addr_len".to_string())?,
        ) as usize;
        if data.len() < addr_start + 4 + addr_len {
            return Err("announcement addr truncated".to_string());
        }
        let rpc_addr = String::from_utf8(data[addr_start + 4..addr_start + 4 + addr_len].to_vec())
            .map_err(|_| "bad addr".to_string())?;

        Ok(Self {
            node_id,
            service_name,
            rpc_addr,
            timestamp_ms,
        })
    }
}

/// Registry of discovered services (updated by announcements).
pub struct ServiceRegistry {
    /// Known services: name → list of (node_id, addr, last_seen_ms).
    services: HashMap<String, Vec<(u64, String, u64)>>,
    /// TTL for entries in milliseconds.
    ttl_ms: u64,
}

impl ServiceRegistry {
    /// Create a new service registry.
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            services: HashMap::new(),
            ttl_ms,
        }
    }

    /// Register or update a service from an announcement.
    pub fn register(&mut self, ann: &ServiceAnnouncement) {
        let entries = self.services.entry(ann.service_name.clone()).or_default();
        if let Some(entry) = entries.iter_mut().find(|(id, _, _)| *id == ann.node_id) {
            entry.1 = ann.rpc_addr.clone();
            entry.2 = ann.timestamp_ms;
        } else {
            entries.push((ann.node_id, ann.rpc_addr.clone(), ann.timestamp_ms));
        }
    }

    /// Look up all addresses for a service (excluding expired).
    pub fn lookup(&self, service_name: &str, now_ms: u64) -> Vec<(u64, String)> {
        self.services
            .get(service_name)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|(_, _, ts)| now_ms.saturating_sub(*ts) <= self.ttl_ms)
                    .map(|(id, addr, _)| (*id, addr.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Remove expired entries.
    pub fn gc(&mut self, now_ms: u64) {
        for entries in self.services.values_mut() {
            entries.retain(|(_, _, ts)| now_ms.saturating_sub(*ts) <= self.ttl_ms);
        }
        self.services.retain(|_, entries| !entries.is_empty());
    }

    /// Number of total service entries (across all names).
    pub fn entry_count(&self) -> usize {
        self.services.values().map(|v| v.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.2: Heartbeat Timeout Detection
// ═══════════════════════════════════════════════════════════════════════

/// Tracks heartbeat timestamps per node for failure detection.
pub struct HeartbeatTracker {
    /// Last heartbeat time per node (milliseconds).
    last_seen: HashMap<u64, u64>,
    /// Timeout threshold in milliseconds.
    timeout_ms: u64,
}

impl HeartbeatTracker {
    /// Create a new tracker with given timeout.
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            last_seen: HashMap::new(),
            timeout_ms,
        }
    }

    /// Record a heartbeat from a node.
    pub fn record_heartbeat(&mut self, node_id: u64, now_ms: u64) {
        self.last_seen.insert(node_id, now_ms);
    }

    /// Check which nodes are considered dead (no heartbeat within timeout).
    pub fn dead_nodes(&self, now_ms: u64) -> Vec<u64> {
        self.last_seen
            .iter()
            .filter(|(_, last)| now_ms.saturating_sub(**last) > self.timeout_ms)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Check if a specific node is alive.
    pub fn is_alive(&self, node_id: u64, now_ms: u64) -> bool {
        self.last_seen
            .get(&node_id)
            .is_some_and(|last| now_ms.saturating_sub(*last) <= self.timeout_ms)
    }

    /// Number of tracked nodes.
    pub fn node_count(&self) -> usize {
        self.last_seen.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.4: FIFO Message Ordering
// ═══════════════════════════════════════════════════════════════════════

/// Reorders messages by sequence number to guarantee FIFO per sender.
pub struct FifoBuffer {
    /// Expected next sequence per sender.
    expected_seq: HashMap<u64, u64>,
    /// Buffered out-of-order messages per sender.
    pending: HashMap<u64, Vec<NetMessage>>,
}

impl FifoBuffer {
    /// Create a new FIFO buffer.
    pub fn new() -> Self {
        Self {
            expected_seq: HashMap::new(),
            pending: HashMap::new(),
        }
    }

    /// Process an incoming message. Returns messages ready for delivery (in order).
    pub fn receive(&mut self, msg: NetMessage) -> Vec<NetMessage> {
        let sender = msg.sender_id;
        let expected = self.expected_seq.entry(sender).or_insert(1);
        let mut ready = Vec::new();

        if msg.seq == *expected {
            ready.push(msg);
            *expected += 1;

            // Check if any buffered messages are now in order
            let pending = self.pending.entry(sender).or_default();
            while let Some(pos) = pending.iter().position(|m| m.seq == *expected) {
                ready.push(pending.remove(pos));
                *expected += 1;
            }
        } else {
            // Out of order — buffer it
            self.pending.entry(sender).or_default().push(msg);
        }

        ready
    }

    /// Number of pending (buffered) messages.
    pub fn pending_count(&self) -> usize {
        self.pending.values().map(|v| v.len()).sum()
    }
}

impl Default for FifoBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DQ5.9: Transport Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Metrics for the transport layer.
#[derive(Debug, Clone, Default)]
pub struct TransportMetrics {
    /// Total messages sent.
    pub messages_sent: u64,
    /// Total messages received.
    pub messages_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Send errors.
    pub send_errors: u64,
    /// Connection count.
    pub connections: u64,
}

impl TransportMetrics {
    /// Record a sent message.
    pub fn record_send(&mut self, bytes: u64) {
        self.messages_sent += 1;
        self.bytes_sent += bytes;
    }

    /// Record a received message.
    pub fn record_recv(&mut self, bytes: u64) {
        self.messages_received += 1;
        self.bytes_received += bytes;
    }

    /// Record a send error.
    pub fn record_error(&mut self) {
        self.send_errors += 1;
    }

    /// Format as Prometheus-compatible text.
    pub fn to_prometheus(&self, prefix: &str) -> String {
        format!(
            "{prefix}_messages_sent {}\n\
             {prefix}_messages_received {}\n\
             {prefix}_bytes_sent {}\n\
             {prefix}_bytes_received {}\n\
             {prefix}_send_errors {}\n\
             {prefix}_connections {}\n",
            self.messages_sent,
            self.messages_received,
            self.bytes_sent,
            self.bytes_received,
            self.send_errors,
            self.connections,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc3_message_serialization_roundtrip() {
        let msg = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "worker-1".to_string(),
            payload: b"hello distributed world".to_vec(),
            sender_id: 42,
            seq: 7,
        };
        let bytes = msg.to_bytes();
        let decoded = NetMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.msg_type, MessageType::ActorMessage);
        assert_eq!(decoded.target, "worker-1");
        assert_eq!(decoded.payload, b"hello distributed world");
        assert_eq!(decoded.sender_id, 42);
        assert_eq!(decoded.seq, 7);
    }

    #[test]
    fn gc3_message_all_types() {
        for (i, mt) in [
            MessageType::ActorMessage,
            MessageType::RpcRequest,
            MessageType::RpcResponse,
            MessageType::Heartbeat,
            MessageType::HeartbeatAck,
            MessageType::Join,
            MessageType::JoinAck,
            MessageType::TensorShard,
            MessageType::Gradient,
        ]
        .iter()
        .enumerate()
        {
            let msg = NetMessage {
                msg_type: *mt,
                target: format!("t{i}"),
                payload: vec![i as u8],
                sender_id: i as u64,
                seq: i as u64,
            };
            let decoded = NetMessage::from_bytes(&msg.to_bytes()).unwrap();
            assert_eq!(decoded.msg_type, *mt);
        }
    }

    #[test]
    fn gc3_actor_creation() {
        let actor = Actor::new("test-actor", 100);
        assert_eq!(actor.name, "test-actor");
        assert!(!actor.mailbox_tx.is_closed());
    }

    #[test]
    fn gc3_transport_node_creation() {
        let node = TransportNode::new(1, "127.0.0.1:0");
        assert_eq!(node.node_id, 1);
        node.add_peer(2, "127.0.0.1:9001");
        assert!(node.peers.lock().unwrap().contains_key(&2));
    }

    #[test]
    fn gc3_register_actor() {
        let node = TransportNode::new(1, "127.0.0.1:0");
        let actor = Actor::new("worker", 10);
        node.register_actor("worker", actor.mailbox_tx.clone());
        assert!(node.actors.lock().unwrap().contains_key("worker"));
    }

    #[test]
    fn gc3_sequence_numbers() {
        let node = TransportNode::new(1, "127.0.0.1:0");
        assert_eq!(node.next_seq(), 1);
        assert_eq!(node.next_seq(), 2);
        assert_eq!(node.next_seq(), 3);
    }

    #[tokio::test]
    async fn gc3_actor_mailbox_send_recv() {
        let mut actor = Actor::new("echo", 10);
        let msg = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "echo".to_string(),
            payload: b"ping".to_vec(),
            sender_id: 0,
            seq: 1,
        };
        actor.send(msg).await.unwrap();

        let mut rx = actor.take_receiver().unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, b"ping");
        assert_eq!(received.target, "echo");
    }

    #[tokio::test]
    async fn gc3_tcp_transport_two_nodes() {
        // Node 1: listener
        let node1 = TransportNode::new(1, "127.0.0.1:0");
        let mut actor = Actor::new("handler", 10);
        node1.register_actor("handler", actor.mailbox_tx.clone());

        let addr1 = node1.start_listener().await.unwrap();

        // Node 2: sender
        let node2 = TransportNode::new(2, "127.0.0.1:0");
        node2.add_peer(1, &addr1);

        // Send message from node2 to node1
        let msg = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "handler".to_string(),
            payload: b"hello from node 2".to_vec(),
            sender_id: 2,
            seq: 1,
        };
        node2.send_to_node(1, msg).await.unwrap();

        // Receive on node1's actor
        let mut rx = actor.take_receiver().unwrap();
        let received = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received.payload, b"hello from node 2");
        assert_eq!(received.sender_id, 2);
        assert_eq!(received.target, "handler");
    }

    #[tokio::test]
    async fn gc3_heartbeat() {
        let node = TransportNode::new(42, "127.0.0.1:0");
        let hb = node.heartbeat_msg();
        assert_eq!(hb.msg_type, MessageType::Heartbeat);
        assert_eq!(hb.sender_id, 42);
        assert_eq!(hb.seq, 1);
    }

    // ═══════════════════════════════════════════════════════════════════
    // DQ5: Quality improvement tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn dq5_2_heartbeat_alive() {
        let mut tracker = HeartbeatTracker::new(5000);
        tracker.record_heartbeat(1, 1000);
        tracker.record_heartbeat(2, 1000);

        assert!(tracker.is_alive(1, 3000)); // 2s < 5s timeout
        assert!(tracker.is_alive(2, 3000));
        assert_eq!(tracker.node_count(), 2);
    }

    #[test]
    fn dq5_2_heartbeat_dead() {
        let mut tracker = HeartbeatTracker::new(5000);
        tracker.record_heartbeat(1, 1000);
        tracker.record_heartbeat(2, 1000);

        // At t=7000, node 1 is dead (6s > 5s)
        assert!(!tracker.is_alive(1, 7000));
        let dead = tracker.dead_nodes(7000);
        assert!(dead.contains(&1));
        assert!(dead.contains(&2));
    }

    #[test]
    fn dq5_2_heartbeat_refresh() {
        let mut tracker = HeartbeatTracker::new(5000);
        tracker.record_heartbeat(1, 1000);
        tracker.record_heartbeat(1, 4000); // refresh

        // At t=8000: last seen at 4000, 4s < 5s → alive
        assert!(tracker.is_alive(1, 8000));
        // At t=10000: last seen at 4000, 6s > 5s → dead
        assert!(!tracker.is_alive(1, 10000));
    }

    #[test]
    fn dq5_4_fifo_in_order() {
        let mut fifo = FifoBuffer::new();
        let msg1 = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "t".into(),
            payload: b"1".to_vec(),
            sender_id: 1,
            seq: 1,
        };
        let msg2 = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "t".into(),
            payload: b"2".to_vec(),
            sender_id: 1,
            seq: 2,
        };

        let ready1 = fifo.receive(msg1);
        assert_eq!(ready1.len(), 1);
        assert_eq!(ready1[0].seq, 1);

        let ready2 = fifo.receive(msg2);
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].seq, 2);
    }

    #[test]
    fn dq5_4_fifo_out_of_order() {
        let mut fifo = FifoBuffer::new();
        // Send seq 2 before seq 1
        let msg2 = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "t".into(),
            payload: b"2".to_vec(),
            sender_id: 1,
            seq: 2,
        };
        let msg1 = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "t".into(),
            payload: b"1".to_vec(),
            sender_id: 1,
            seq: 1,
        };

        let ready_first = fifo.receive(msg2);
        assert_eq!(
            ready_first.len(),
            0,
            "seq 2 should be buffered (waiting for 1)"
        );
        assert_eq!(fifo.pending_count(), 1);

        // Now send seq 1 — both should be released in order
        let ready_second = fifo.receive(msg1);
        assert_eq!(
            ready_second.len(),
            2,
            "should release seq 1 + buffered seq 2"
        );
        assert_eq!(ready_second[0].seq, 1);
        assert_eq!(ready_second[1].seq, 2);
        assert_eq!(fifo.pending_count(), 0);
    }

    #[test]
    fn dq5_9_metrics_recording() {
        let mut metrics = TransportMetrics::default();
        metrics.record_send(100);
        metrics.record_send(200);
        metrics.record_recv(150);
        metrics.record_error();

        assert_eq!(metrics.messages_sent, 2);
        assert_eq!(metrics.bytes_sent, 300);
        assert_eq!(metrics.messages_received, 1);
        assert_eq!(metrics.bytes_received, 150);
        assert_eq!(metrics.send_errors, 1);
    }

    // DQ5.3: Backpressure — bounded channel doesn't OOM
    #[tokio::test]
    async fn dq5_3_backpressure_bounded() {
        let mut actor = Actor::new("slow", 3); // capacity 3
        let tx = actor.mailbox_tx.clone();

        // Fill the channel
        for i in 0..3 {
            let msg = NetMessage {
                msg_type: MessageType::ActorMessage,
                target: "slow".to_string(),
                payload: vec![i as u8],
                sender_id: 0,
                seq: i as u64 + 1,
            };
            tx.send(msg).await.unwrap();
        }

        // 4th send should fail (channel full) with try_send
        let msg4 = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "slow".to_string(),
            payload: vec![99],
            sender_id: 0,
            seq: 4,
        };
        let result = tx.try_send(msg4);
        assert!(result.is_err(), "bounded channel should reject when full");
    }

    // DQ5.8: Graceful shutdown — drain messages
    #[tokio::test]
    async fn dq5_8_graceful_drain() {
        let mut actor = Actor::new("drain-test", 10);

        // Send 5 messages using the actor's own sender
        for i in 0..5u64 {
            let msg = NetMessage {
                msg_type: MessageType::ActorMessage,
                target: "drain-test".to_string(),
                payload: format!("msg-{i}").into_bytes(),
                sender_id: 0,
                seq: i + 1,
            };
            actor.send(msg).await.unwrap();
        }

        // Take receiver and drain with timeout
        let mut rx = actor.take_receiver().unwrap();
        // Drop the actor (which drops the last sender clone)
        drop(actor);

        let mut drained = 0;
        // Use try_recv in a loop since channel is closed
        while let Ok(msg) = rx.try_recv() {
            let _ = msg;
            drained += 1;
        }
        assert_eq!(drained, 5, "should drain all 5 messages");
    }

    #[test]
    fn dq5_9_prometheus_format() {
        let mut metrics = TransportMetrics::default();
        metrics.record_send(100);
        metrics.record_recv(50);
        let prom = metrics.to_prometheus("fj_transport");
        assert!(prom.contains("fj_transport_messages_sent 1"));
        assert!(prom.contains("fj_transport_bytes_sent 100"));
        assert!(prom.contains("fj_transport_messages_received 1"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // DQ5.1: Reconnection / Retry tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn dq5_1_retry_config_defaults() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 5);
        assert_eq!(cfg.initial_backoff_ms, 100);
        assert_eq!(cfg.max_backoff_ms, 10_000);
    }

    #[test]
    fn dq5_1_exponential_backoff() {
        let cfg = RetryConfig {
            max_retries: 5,
            initial_backoff_ms: 100,
            max_backoff_ms: 5_000,
            multiplier: 2.0,
        };
        assert_eq!(cfg.backoff_ms(0), 100); // 100 * 2^0
        assert_eq!(cfg.backoff_ms(1), 200); // 100 * 2^1
        assert_eq!(cfg.backoff_ms(2), 400); // 100 * 2^2
        assert_eq!(cfg.backoff_ms(3), 800); // 100 * 2^3
        assert_eq!(cfg.backoff_ms(4), 1600); // 100 * 2^4
        assert_eq!(cfg.backoff_ms(5), 3200); // 100 * 2^5
        assert_eq!(cfg.backoff_ms(10), 5000); // capped at max
    }

    #[tokio::test]
    async fn dq5_1_send_with_retry_success() {
        // Set up listener node
        let node1 = TransportNode::new(1, "127.0.0.1:0");
        let mut actor = Actor::new("retry-test", 10);
        node1.register_actor("retry-test", actor.mailbox_tx.clone());
        let addr = node1.start_listener().await.unwrap();

        // Sender with retry
        let node2 = TransportNode::new(2, "127.0.0.1:0");
        node2.add_peer(1, &addr);

        let msg = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "retry-test".to_string(),
            payload: b"retry-ok".to_vec(),
            sender_id: 2,
            seq: 1,
        };

        let config = RetryConfig::default();
        let result = node2.send_with_retry(1, msg, &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // succeeded on first attempt

        // Verify message arrived
        let mut rx = actor.take_receiver().unwrap();
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received.payload, b"retry-ok");
    }

    #[tokio::test]
    async fn dq5_1_send_with_retry_fails_bad_peer() {
        let node = TransportNode::new(1, "127.0.0.1:0");
        // Peer at an address that doesn't exist
        node.add_peer(99, "127.0.0.1:1"); // port 1 likely refused

        let msg = NetMessage {
            msg_type: MessageType::ActorMessage,
            target: "x".to_string(),
            payload: vec![],
            sender_id: 1,
            seq: 1,
        };

        let config = RetryConfig {
            max_retries: 2,
            initial_backoff_ms: 10, // fast for testing
            max_backoff_ms: 50,
            multiplier: 2.0,
        };
        let result = node.send_with_retry(99, msg, &config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed after 3 attempts"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // DQ5.5: Connection Pool tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn dq5_5_pool_creation() {
        let pool = ConnectionPool::new(5);
        assert_eq!(pool.total_pooled(), 0);
        assert_eq!(pool.pooled_count("127.0.0.1:8080"), 0);
    }

    #[tokio::test]
    async fn dq5_5_pool_get_creates_connection() {
        // Start a TCP listener to accept connections
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let pool = ConnectionPool::new(5);
        let stream = pool.get_or_connect(&addr).await;
        assert!(stream.is_ok(), "should connect to listener");
    }

    #[tokio::test]
    async fn dq5_5_pool_return_and_reuse() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let pool = ConnectionPool::new(5);
        let stream = pool.get_or_connect(&addr).await.unwrap();

        // Return it to pool
        pool.return_connection(&addr, stream);
        assert_eq!(pool.pooled_count(&addr), 1);
        assert_eq!(pool.total_pooled(), 1);

        // Clear
        pool.clear_addr(&addr);
        assert_eq!(pool.pooled_count(&addr), 0);
    }

    // ═══════════════════════════════════════════════════════════════════
    // DQ5.7: Service Discovery tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn dq5_7_announcement_roundtrip() {
        let ann = ServiceAnnouncement {
            node_id: 42,
            service_name: "worker".to_string(),
            rpc_addr: "192.168.1.10:9000".to_string(),
            timestamp_ms: 1_000_000,
        };
        let bytes = ann.to_bytes();
        let decoded = ServiceAnnouncement::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, ann);
    }

    #[test]
    fn dq5_7_service_registry_register() {
        let mut reg = ServiceRegistry::new(30_000);
        let ann = ServiceAnnouncement {
            node_id: 1,
            service_name: "worker".to_string(),
            rpc_addr: "10.0.0.1:8080".to_string(),
            timestamp_ms: 1000,
        };
        reg.register(&ann);
        assert_eq!(reg.entry_count(), 1);

        let found = reg.lookup("worker", 2000);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1);
        assert_eq!(found[0].1, "10.0.0.1:8080");
    }

    #[test]
    fn dq5_7_service_registry_multiple_nodes() {
        let mut reg = ServiceRegistry::new(30_000);
        for i in 1..=3 {
            reg.register(&ServiceAnnouncement {
                node_id: i,
                service_name: "worker".to_string(),
                rpc_addr: format!("10.0.0.{i}:8080"),
                timestamp_ms: 1000,
            });
        }
        assert_eq!(reg.entry_count(), 3);
        assert_eq!(reg.lookup("worker", 2000).len(), 3);
        assert_eq!(reg.lookup("scheduler", 2000).len(), 0);
    }

    #[test]
    fn dq5_7_service_registry_expiry() {
        let mut reg = ServiceRegistry::new(5000); // 5s TTL
        reg.register(&ServiceAnnouncement {
            node_id: 1,
            service_name: "worker".to_string(),
            rpc_addr: "10.0.0.1:8080".to_string(),
            timestamp_ms: 1000,
        });

        // Still alive at t=5000
        assert_eq!(reg.lookup("worker", 5000).len(), 1);
        // Expired at t=7000 (6s > 5s TTL)
        assert_eq!(reg.lookup("worker", 7000).len(), 0);
    }

    #[test]
    fn dq5_7_service_registry_gc() {
        let mut reg = ServiceRegistry::new(5000);
        reg.register(&ServiceAnnouncement {
            node_id: 1,
            service_name: "old".to_string(),
            rpc_addr: "x".to_string(),
            timestamp_ms: 1000,
        });
        reg.register(&ServiceAnnouncement {
            node_id: 2,
            service_name: "new".to_string(),
            rpc_addr: "y".to_string(),
            timestamp_ms: 10_000,
        });
        assert_eq!(reg.entry_count(), 2);

        reg.gc(15_000); // old (t=1000) expired, new (t=10000) alive
        assert_eq!(reg.entry_count(), 1);
        assert_eq!(reg.lookup("new", 15_000).len(), 1);
    }

    #[test]
    fn dq5_7_service_registry_update() {
        let mut reg = ServiceRegistry::new(30_000);
        reg.register(&ServiceAnnouncement {
            node_id: 1,
            service_name: "worker".to_string(),
            rpc_addr: "old:8080".to_string(),
            timestamp_ms: 1000,
        });
        // Same node re-announces with new address
        reg.register(&ServiceAnnouncement {
            node_id: 1,
            service_name: "worker".to_string(),
            rpc_addr: "new:9090".to_string(),
            timestamp_ms: 2000,
        });
        assert_eq!(reg.entry_count(), 1); // updated, not duplicated
        let found = reg.lookup("worker", 3000);
        assert_eq!(found[0].1, "new:9090");
    }
}
