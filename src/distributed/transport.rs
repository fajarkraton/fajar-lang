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
        self.actors
            .lock()
            .unwrap()
            .insert(name.to_string(), sender);
    }

    /// Add a known peer.
    pub fn add_peer(&self, peer_id: u64, addr: &str) {
        self.peers
            .lock()
            .unwrap()
            .insert(peer_id, addr.to_string());
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
        let received = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        )
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
}
