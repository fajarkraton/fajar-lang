//! Sprint W7: WebSocket Chat Server — server, client, rooms, message protocol,
//! handshake, frame encode/decode, connection management, chat history.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W7.1: MessageProtocol — Typed Chat Messages
// ═══════════════════════════════════════════════════════════════════════

/// Unique identifier for a client connection.
pub type ClientId = u64;

/// Unique identifier for a chat room.
pub type RoomId = String;

/// WebSocket message types following the chat protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatMessage {
    /// Plain text message from a user.
    Text {
        /// Sender client ID.
        from: ClientId,
        /// Sender display name.
        username: String,
        /// Message content.
        content: String,
        /// Timestamp (unix epoch seconds).
        timestamp: u64,
    },
    /// System notification (join, leave, etc.).
    System {
        /// Notification content.
        content: String,
        /// Timestamp.
        timestamp: u64,
    },
    /// User joined the room.
    UserJoin {
        /// Client ID that joined.
        client_id: ClientId,
        /// Display name.
        username: String,
        /// Timestamp.
        timestamp: u64,
    },
    /// User left the room.
    UserLeave {
        /// Client ID that left.
        client_id: ClientId,
        /// Display name.
        username: String,
        /// Timestamp.
        timestamp: u64,
    },
}

impl fmt::Display for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatMessage::Text {
                username, content, ..
            } => write!(f, "<{username}> {content}"),
            ChatMessage::System { content, .. } => write!(f, "*** {content} ***"),
            ChatMessage::UserJoin { username, .. } => write!(f, ">>> {username} joined"),
            ChatMessage::UserLeave { username, .. } => write!(f, "<<< {username} left"),
        }
    }
}

impl ChatMessage {
    /// Serialize message to a simple protocol string.
    ///
    /// Format: `TYPE|field1|field2|...`
    pub fn encode(&self) -> String {
        match self {
            ChatMessage::Text {
                from,
                username,
                content,
                timestamp,
            } => format!("TEXT|{from}|{username}|{content}|{timestamp}"),
            ChatMessage::System { content, timestamp } => {
                format!("SYSTEM|{content}|{timestamp}")
            }
            ChatMessage::UserJoin {
                client_id,
                username,
                timestamp,
            } => format!("JOIN|{client_id}|{username}|{timestamp}"),
            ChatMessage::UserLeave {
                client_id,
                username,
                timestamp,
            } => format!("LEAVE|{client_id}|{username}|{timestamp}"),
        }
    }

    /// Deserialize a protocol string back into a message.
    pub fn decode(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.splitn(5, '|').collect();
        if parts.is_empty() {
            return Err("empty message".into());
        }
        match parts[0] {
            "TEXT" if parts.len() >= 5 => Ok(ChatMessage::Text {
                from: parts[1].parse().map_err(|e| format!("bad client_id: {e}"))?,
                username: parts[2].into(),
                content: parts[3].into(),
                timestamp: parts[4].parse().map_err(|e| format!("bad timestamp: {e}"))?,
            }),
            "SYSTEM" if parts.len() >= 3 => Ok(ChatMessage::System {
                content: parts[1].into(),
                timestamp: parts[2].parse().map_err(|e| format!("bad timestamp: {e}"))?,
            }),
            "JOIN" if parts.len() >= 4 => Ok(ChatMessage::UserJoin {
                client_id: parts[1].parse().map_err(|e| format!("bad client_id: {e}"))?,
                username: parts[2].into(),
                timestamp: parts[3].parse().map_err(|e| format!("bad timestamp: {e}"))?,
            }),
            "LEAVE" if parts.len() >= 4 => Ok(ChatMessage::UserLeave {
                client_id: parts[1].parse().map_err(|e| format!("bad client_id: {e}"))?,
                username: parts[2].into(),
                timestamp: parts[3].parse().map_err(|e| format!("bad timestamp: {e}"))?,
            }),
            other => Err(format!("unknown message type: {other}")),
        }
    }

    /// Returns the timestamp of this message.
    pub fn timestamp(&self) -> u64 {
        match self {
            ChatMessage::Text { timestamp, .. }
            | ChatMessage::System { timestamp, .. }
            | ChatMessage::UserJoin { timestamp, .. }
            | ChatMessage::UserLeave { timestamp, .. } => *timestamp,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.2: WsFrame — WebSocket Frame Encode/Decode
// ═══════════════════════════════════════════════════════════════════════

/// WebSocket opcodes (RFC 6455).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOpcode {
    /// Continuation frame.
    Continuation,
    /// Text frame.
    Text,
    /// Binary frame.
    Binary,
    /// Connection close.
    Close,
    /// Ping.
    Ping,
    /// Pong.
    Pong,
}

impl WsOpcode {
    /// Convert opcode to its numeric value.
    pub fn to_u8(self) -> u8 {
        match self {
            WsOpcode::Continuation => 0x0,
            WsOpcode::Text => 0x1,
            WsOpcode::Binary => 0x2,
            WsOpcode::Close => 0x8,
            WsOpcode::Ping => 0x9,
            WsOpcode::Pong => 0xA,
        }
    }

    /// Parse opcode from numeric value.
    pub fn from_u8(val: u8) -> Result<Self, String> {
        match val {
            0x0 => Ok(WsOpcode::Continuation),
            0x1 => Ok(WsOpcode::Text),
            0x2 => Ok(WsOpcode::Binary),
            0x8 => Ok(WsOpcode::Close),
            0x9 => Ok(WsOpcode::Ping),
            0xA => Ok(WsOpcode::Pong),
            other => Err(format!("unknown opcode: 0x{other:02X}")),
        }
    }
}

/// A WebSocket frame (simplified per RFC 6455).
#[derive(Debug, Clone, PartialEq)]
pub struct WsFrame {
    /// Whether this is the final fragment.
    pub fin: bool,
    /// Frame opcode.
    pub opcode: WsOpcode,
    /// Whether the payload is masked (client -> server).
    pub masked: bool,
    /// 4-byte masking key (if masked).
    pub mask_key: [u8; 4],
    /// Payload data.
    pub payload: Vec<u8>,
}

impl WsFrame {
    /// Create a text frame with the given payload.
    pub fn text(payload: &str) -> Self {
        Self {
            fin: true,
            opcode: WsOpcode::Text,
            masked: false,
            mask_key: [0; 4],
            payload: payload.as_bytes().to_vec(),
        }
    }

    /// Create a close frame.
    pub fn close() -> Self {
        Self {
            fin: true,
            opcode: WsOpcode::Close,
            masked: false,
            mask_key: [0; 4],
            payload: Vec::new(),
        }
    }

    /// Create a ping frame.
    pub fn ping(data: &[u8]) -> Self {
        Self {
            fin: true,
            opcode: WsOpcode::Ping,
            masked: false,
            mask_key: [0; 4],
            payload: data.to_vec(),
        }
    }

    /// Create a pong frame responding to a ping.
    pub fn pong(ping: &WsFrame) -> Self {
        Self {
            fin: true,
            opcode: WsOpcode::Pong,
            masked: false,
            mask_key: [0; 4],
            payload: ping.payload.clone(),
        }
    }

    /// Encode the frame into bytes (simplified — no extended length encoding).
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut b0: u8 = self.opcode.to_u8();
        if self.fin {
            b0 |= 0x80;
        }
        bytes.push(b0);

        let len = self.payload.len();
        let mut b1: u8 = 0;
        if self.masked {
            b1 |= 0x80;
        }
        if len < 126 {
            b1 |= len as u8;
            bytes.push(b1);
        } else if len < 65536 {
            b1 |= 126;
            bytes.push(b1);
            bytes.push((len >> 8) as u8);
            bytes.push((len & 0xFF) as u8);
        } else {
            b1 |= 127;
            bytes.push(b1);
            for i in (0..8).rev() {
                bytes.push(((len >> (i * 8)) & 0xFF) as u8);
            }
        }

        if self.masked {
            bytes.extend_from_slice(&self.mask_key);
            for (i, byte) in self.payload.iter().enumerate() {
                bytes.push(byte ^ self.mask_key[i % 4]);
            }
        } else {
            bytes.extend_from_slice(&self.payload);
        }
        bytes
    }

    /// Decode a frame from bytes (simplified).
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 2 {
            return Err("frame too short".into());
        }
        let fin = (data[0] & 0x80) != 0;
        let opcode = WsOpcode::from_u8(data[0] & 0x0F)?;
        let masked = (data[1] & 0x80) != 0;
        let len_byte = data[1] & 0x7F;

        let (payload_len, header_end) = if len_byte < 126 {
            (len_byte as usize, 2)
        } else if len_byte == 126 {
            if data.len() < 4 {
                return Err("frame too short for 16-bit length".into());
            }
            let len = ((data[2] as usize) << 8) | (data[3] as usize);
            (len, 4)
        } else {
            if data.len() < 10 {
                return Err("frame too short for 64-bit length".into());
            }
            let mut len: usize = 0;
            for i in 0..8 {
                len = (len << 8) | (data[2 + i] as usize);
            }
            (len, 10)
        };

        let mut mask_key = [0u8; 4];
        let payload_start = if masked {
            if data.len() < header_end + 4 {
                return Err("frame too short for mask key".into());
            }
            mask_key.copy_from_slice(&data[header_end..header_end + 4]);
            header_end + 4
        } else {
            header_end
        };

        if data.len() < payload_start + payload_len {
            return Err("frame payload truncated".into());
        }

        let mut payload = data[payload_start..payload_start + payload_len].to_vec();
        if masked {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask_key[i % 4];
            }
        }

        Ok(WsFrame {
            fin,
            opcode,
            masked,
            mask_key,
            payload,
        })
    }

    /// Returns the payload as a UTF-8 string (for text frames).
    pub fn payload_text(&self) -> Result<String, String> {
        String::from_utf8(self.payload.clone()).map_err(|e| format!("invalid UTF-8: {e}"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.3: WsHandshake — HTTP Upgrade Handshake
// ═══════════════════════════════════════════════════════════════════════

/// WebSocket handshake request.
#[derive(Debug, Clone)]
pub struct WsHandshakeRequest {
    /// Request path (e.g., "/chat").
    pub path: String,
    /// Host header.
    pub host: String,
    /// Sec-WebSocket-Key (base64-encoded 16-byte nonce).
    pub key: String,
    /// Requested sub-protocols.
    pub protocols: Vec<String>,
}

/// WebSocket handshake response.
#[derive(Debug, Clone)]
pub struct WsHandshakeResponse {
    /// HTTP status code (101 for success).
    pub status: u16,
    /// Sec-WebSocket-Accept value.
    pub accept_key: String,
    /// Selected sub-protocol (if any).
    pub protocol: Option<String>,
}

/// Handles the WebSocket HTTP upgrade handshake.
pub struct WsHandshake;

impl WsHandshake {
    /// The magic GUID used in WebSocket handshake (RFC 6455 section 4.2.2).
    const WS_GUID: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    /// Generate the Sec-WebSocket-Accept value from the client key.
    ///
    /// In production this would be SHA-1 + base64. We use a simplified
    /// deterministic transform for demo purposes.
    pub fn compute_accept_key(client_key: &str) -> String {
        // Simplified: concatenate key + GUID, then produce a deterministic hash string
        let combined = format!("{client_key}{}", Self::WS_GUID);
        let hash: u64 = combined.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        format!("accept-{hash:016x}")
    }

    /// Build an HTTP upgrade request string.
    pub fn build_request(req: &WsHandshakeRequest) -> String {
        let mut out = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: {}\r\n",
            req.path, req.host, req.key
        );
        if !req.protocols.is_empty() {
            out.push_str(&format!(
                "Sec-WebSocket-Protocol: {}\r\n",
                req.protocols.join(", ")
            ));
        }
        out.push_str("\r\n");
        out
    }

    /// Process a handshake request and produce a response.
    pub fn accept(req: &WsHandshakeRequest) -> WsHandshakeResponse {
        let accept_key = Self::compute_accept_key(&req.key);
        let protocol = req.protocols.first().cloned();
        WsHandshakeResponse {
            status: 101,
            accept_key,
            protocol,
        }
    }

    /// Build an HTTP upgrade response string.
    pub fn build_response(resp: &WsHandshakeResponse) -> String {
        let mut out = format!(
            "HTTP/1.1 {} Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n",
            resp.status, resp.accept_key
        );
        if let Some(ref proto) = resp.protocol {
            out.push_str(&format!("Sec-WebSocket-Protocol: {proto}\r\n"));
        }
        out.push_str("\r\n");
        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.4: WsClient — Client Connection
// ═══════════════════════════════════════════════════════════════════════

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not yet connected.
    Disconnected,
    /// Handshake in progress.
    Connecting,
    /// Fully connected.
    Connected,
    /// Connection closing.
    Closing,
    /// Connection closed.
    Closed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "DISCONNECTED"),
            ConnectionState::Connecting => write!(f, "CONNECTING"),
            ConnectionState::Connected => write!(f, "CONNECTED"),
            ConnectionState::Closing => write!(f, "CLOSING"),
            ConnectionState::Closed => write!(f, "CLOSED"),
        }
    }
}

/// A WebSocket client connection.
#[derive(Debug, Clone)]
pub struct WsClient {
    /// Client identifier.
    pub id: ClientId,
    /// Display name.
    pub username: String,
    /// Connection state.
    pub state: ConnectionState,
    /// Outbound message queue.
    pub outbox: Vec<WsFrame>,
    /// Number of messages sent.
    pub sent_count: u64,
    /// Number of messages received.
    pub recv_count: u64,
}

impl WsClient {
    /// Create a new client.
    pub fn new(id: ClientId, username: &str) -> Self {
        Self {
            id,
            username: username.into(),
            state: ConnectionState::Disconnected,
            outbox: Vec::new(),
            sent_count: 0,
            recv_count: 0,
        }
    }

    /// Simulate connecting.
    pub fn connect(&mut self) {
        self.state = ConnectionState::Connecting;
        self.state = ConnectionState::Connected;
    }

    /// Send a text message.
    pub fn send_text(&mut self, text: &str) -> Result<(), String> {
        if self.state != ConnectionState::Connected {
            return Err(format!("cannot send in state {}", self.state));
        }
        self.outbox.push(WsFrame::text(text));
        self.sent_count += 1;
        Ok(())
    }

    /// Receive a frame (simulate).
    pub fn receive(&mut self, _frame: &WsFrame) {
        self.recv_count += 1;
    }

    /// Disconnect the client.
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Closing;
        self.outbox.push(WsFrame::close());
        self.state = ConnectionState::Closed;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.5: ConnectionManager — Track Connected Clients
// ═══════════════════════════════════════════════════════════════════════

/// Manages all client connections.
#[derive(Debug, Default)]
pub struct ConnectionManager {
    /// Active clients indexed by ID.
    clients: HashMap<ClientId, WsClient>,
    /// Next available client ID.
    next_id: ClientId,
}

impl ConnectionManager {
    /// Create a new connection manager.
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a new client, returning the assigned ID.
    pub fn register(&mut self, username: &str) -> ClientId {
        let id = self.next_id;
        self.next_id += 1;
        let mut client = WsClient::new(id, username);
        client.connect();
        self.clients.insert(id, client);
        id
    }

    /// Disconnect and remove a client.
    pub fn disconnect(&mut self, id: ClientId) -> Option<WsClient> {
        if let Some(mut client) = self.clients.remove(&id) {
            client.disconnect();
            Some(client)
        } else {
            None
        }
    }

    /// Get a reference to a client.
    pub fn get(&self, id: ClientId) -> Option<&WsClient> {
        self.clients.get(&id)
    }

    /// Get a mutable reference to a client.
    pub fn get_mut(&mut self, id: ClientId) -> Option<&mut WsClient> {
        self.clients.get_mut(&id)
    }

    /// Number of connected clients.
    pub fn connected_count(&self) -> usize {
        self.clients
            .values()
            .filter(|c| c.state == ConnectionState::Connected)
            .count()
    }

    /// All connected client IDs.
    pub fn connected_ids(&self) -> Vec<ClientId> {
        self.clients
            .iter()
            .filter(|(_, c)| c.state == ConnectionState::Connected)
            .map(|(id, _)| *id)
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.6: ChatHistory — Message History with Pagination
// ═══════════════════════════════════════════════════════════════════════

/// Stores message history with pagination support.
#[derive(Debug, Default)]
pub struct ChatHistory {
    /// All messages in chronological order.
    messages: Vec<ChatMessage>,
    /// Maximum stored messages (0 = unlimited).
    max_size: usize,
}

impl ChatHistory {
    /// Create a new history with unlimited storage.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_size: 0,
        }
    }

    /// Create a history with a maximum size (oldest messages dropped).
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_size,
        }
    }

    /// Add a message to the history.
    pub fn push(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
        if self.max_size > 0 && self.messages.len() > self.max_size {
            self.messages.remove(0);
        }
    }

    /// Total number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get a page of messages (0-indexed, newest last).
    pub fn page(&self, page_num: usize, page_size: usize) -> &[ChatMessage] {
        if page_size == 0 {
            return &[];
        }
        let start = page_num * page_size;
        if start >= self.messages.len() {
            return &[];
        }
        let end = (start + page_size).min(self.messages.len());
        &self.messages[start..end]
    }

    /// Number of pages given a page size.
    pub fn page_count(&self, page_size: usize) -> usize {
        if page_size == 0 || self.messages.is_empty() {
            return 0;
        }
        (self.messages.len() + page_size - 1) / page_size
    }

    /// Get the most recent N messages.
    pub fn recent(&self, n: usize) -> &[ChatMessage] {
        let len = self.messages.len();
        if n >= len {
            &self.messages
        } else {
            &self.messages[len - n..]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.7: ChatRoom — Room Management
// ═══════════════════════════════════════════════════════════════════════

/// A chat room with members and message history.
#[derive(Debug)]
pub struct ChatRoom {
    /// Room identifier.
    pub id: RoomId,
    /// Room display name.
    pub name: String,
    /// Member client IDs.
    members: Vec<ClientId>,
    /// Message history.
    pub history: ChatHistory,
    /// Maximum members (0 = unlimited).
    pub max_members: usize,
}

impl ChatRoom {
    /// Create a new room.
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            members: Vec::new(),
            history: ChatHistory::new(),
            max_members: 0,
        }
    }

    /// Create a room with member limit.
    pub fn with_max_members(mut self, max: usize) -> Self {
        self.max_members = max;
        self
    }

    /// Join a client to this room.
    pub fn join(&mut self, client_id: ClientId, username: &str, timestamp: u64) -> Result<(), String> {
        if self.members.contains(&client_id) {
            return Err("already in room".into());
        }
        if self.max_members > 0 && self.members.len() >= self.max_members {
            return Err("room is full".into());
        }
        self.members.push(client_id);
        self.history.push(ChatMessage::UserJoin {
            client_id,
            username: username.into(),
            timestamp,
        });
        Ok(())
    }

    /// Remove a client from this room.
    pub fn leave(&mut self, client_id: ClientId, username: &str, timestamp: u64) -> Result<(), String> {
        let pos = self
            .members
            .iter()
            .position(|&id| id == client_id)
            .ok_or_else(|| "not in room".to_string())?;
        self.members.remove(pos);
        self.history.push(ChatMessage::UserLeave {
            client_id,
            username: username.into(),
            timestamp,
        });
        Ok(())
    }

    /// Broadcast a text message to the room.
    pub fn broadcast(
        &mut self,
        from: ClientId,
        username: &str,
        content: &str,
        timestamp: u64,
    ) -> Result<Vec<ClientId>, String> {
        if !self.members.contains(&from) {
            return Err("sender not in room".into());
        }
        self.history.push(ChatMessage::Text {
            from,
            username: username.into(),
            content: content.into(),
            timestamp,
        });
        // Return list of recipients (all members except sender)
        Ok(self.members.iter().filter(|&&id| id != from).copied().collect())
    }

    /// Current member count.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if a client is a member.
    pub fn has_member(&self, client_id: ClientId) -> bool {
        self.members.contains(&client_id)
    }

    /// List all member IDs.
    pub fn members(&self) -> &[ClientId] {
        &self.members
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W7.8: WsServer — WebSocket Server with Room Management
// ═══════════════════════════════════════════════════════════════════════

/// WebSocket chat server managing connections and rooms.
#[derive(Debug, Default)]
pub struct WsServer {
    /// Connection manager.
    pub connections: ConnectionManager,
    /// Chat rooms indexed by room ID.
    rooms: HashMap<RoomId, ChatRoom>,
    /// Map client ID -> username.
    usernames: HashMap<ClientId, String>,
}

impl WsServer {
    /// Create a new server.
    pub fn new() -> Self {
        Self {
            connections: ConnectionManager::new(),
            rooms: HashMap::new(),
            usernames: HashMap::new(),
        }
    }

    /// Connect a new client, returning their ID.
    pub fn connect_client(&mut self, username: &str) -> ClientId {
        let id = self.connections.register(username);
        self.usernames.insert(id, username.into());
        id
    }

    /// Disconnect a client from the server and all rooms.
    pub fn disconnect_client(&mut self, client_id: ClientId) {
        let username = self
            .usernames
            .remove(&client_id)
            .unwrap_or_else(|| "unknown".into());
        // Remove from all rooms
        for room in self.rooms.values_mut() {
            if room.has_member(client_id) {
                let _ = room.leave(client_id, &username, 0);
            }
        }
        self.connections.disconnect(client_id);
    }

    /// Create a new chat room.
    pub fn create_room(&mut self, id: &str, name: &str) -> Result<(), String> {
        if self.rooms.contains_key(id) {
            return Err(format!("room '{id}' already exists"));
        }
        self.rooms.insert(id.into(), ChatRoom::new(id, name));
        Ok(())
    }

    /// Join a client to a room.
    pub fn join_room(&mut self, client_id: ClientId, room_id: &str) -> Result<(), String> {
        let username = self
            .usernames
            .get(&client_id)
            .cloned()
            .ok_or_else(|| "unknown client".to_string())?;
        let room = self
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| format!("room '{room_id}' not found"))?;
        room.join(client_id, &username, 0)
    }

    /// Leave a room.
    pub fn leave_room(&mut self, client_id: ClientId, room_id: &str) -> Result<(), String> {
        let username = self
            .usernames
            .get(&client_id)
            .cloned()
            .unwrap_or_else(|| "unknown".into());
        let room = self
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| format!("room '{room_id}' not found"))?;
        room.leave(client_id, &username, 0)
    }

    /// Send a message to a room.
    pub fn send_message(
        &mut self,
        client_id: ClientId,
        room_id: &str,
        content: &str,
        timestamp: u64,
    ) -> Result<Vec<ClientId>, String> {
        let username = self
            .usernames
            .get(&client_id)
            .cloned()
            .ok_or_else(|| "unknown client".to_string())?;
        let room = self
            .rooms
            .get_mut(room_id)
            .ok_or_else(|| format!("room '{room_id}' not found"))?;
        room.broadcast(client_id, &username, content, timestamp)
    }

    /// Get a reference to a room.
    pub fn room(&self, id: &str) -> Option<&ChatRoom> {
        self.rooms.get(id)
    }

    /// List all room IDs.
    pub fn room_ids(&self) -> Vec<&str> {
        self.rooms.keys().map(|s| s.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W7.1: MessageProtocol
    #[test]
    fn w7_1_chat_message_text_display() {
        let msg = ChatMessage::Text {
            from: 1,
            username: "Fajar".into(),
            content: "Hello!".into(),
            timestamp: 1000,
        };
        assert_eq!(format!("{msg}"), "<Fajar> Hello!");
    }

    #[test]
    fn w7_1_chat_message_encode_decode() {
        let msg = ChatMessage::Text {
            from: 42,
            username: "Alice".into(),
            content: "Hi there".into(),
            timestamp: 12345,
        };
        let encoded = msg.encode();
        let decoded = ChatMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn w7_1_chat_message_system() {
        let msg = ChatMessage::System {
            content: "Server restarting".into(),
            timestamp: 999,
        };
        assert_eq!(format!("{msg}"), "*** Server restarting ***");
        let encoded = msg.encode();
        let decoded = ChatMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn w7_1_chat_message_join_leave() {
        let join = ChatMessage::UserJoin {
            client_id: 5,
            username: "Bob".into(),
            timestamp: 100,
        };
        assert!(format!("{join}").contains("Bob joined"));
        let leave = ChatMessage::UserLeave {
            client_id: 5,
            username: "Bob".into(),
            timestamp: 200,
        };
        assert!(format!("{leave}").contains("Bob left"));
    }

    // W7.2: WsFrame
    #[test]
    fn w7_2_frame_text_encode_decode() {
        let frame = WsFrame::text("Hello WebSocket");
        let bytes = frame.encode();
        let decoded = WsFrame::decode(&bytes).unwrap();
        assert!(decoded.fin);
        assert_eq!(decoded.opcode, WsOpcode::Text);
        assert_eq!(decoded.payload_text().unwrap(), "Hello WebSocket");
    }

    #[test]
    fn w7_2_frame_close() {
        let frame = WsFrame::close();
        let bytes = frame.encode();
        let decoded = WsFrame::decode(&bytes).unwrap();
        assert_eq!(decoded.opcode, WsOpcode::Close);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn w7_2_frame_ping_pong() {
        let ping = WsFrame::ping(b"heartbeat");
        let pong = WsFrame::pong(&ping);
        assert_eq!(pong.opcode, WsOpcode::Pong);
        assert_eq!(pong.payload, b"heartbeat");
    }

    #[test]
    fn w7_2_frame_masked() {
        let mut frame = WsFrame::text("secret");
        frame.masked = true;
        frame.mask_key = [0xAA, 0xBB, 0xCC, 0xDD];
        let bytes = frame.encode();
        let decoded = WsFrame::decode(&bytes).unwrap();
        assert_eq!(decoded.payload_text().unwrap(), "secret");
    }

    #[test]
    fn w7_2_opcode_roundtrip() {
        for op in [
            WsOpcode::Continuation,
            WsOpcode::Text,
            WsOpcode::Binary,
            WsOpcode::Close,
            WsOpcode::Ping,
            WsOpcode::Pong,
        ] {
            assert_eq!(WsOpcode::from_u8(op.to_u8()).unwrap(), op);
        }
    }

    // W7.3: WsHandshake
    #[test]
    fn w7_3_handshake_request() {
        let req = WsHandshakeRequest {
            path: "/chat".into(),
            host: "localhost:8080".into(),
            key: "dGhlIHNhbXBsZSBub25jZQ==".into(),
            protocols: vec!["chat".into()],
        };
        let http = WsHandshake::build_request(&req);
        assert!(http.contains("GET /chat HTTP/1.1"));
        assert!(http.contains("Upgrade: websocket"));
        assert!(http.contains("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ=="));
    }

    #[test]
    fn w7_3_handshake_accept() {
        let req = WsHandshakeRequest {
            path: "/chat".into(),
            host: "localhost:8080".into(),
            key: "test-key".into(),
            protocols: vec!["chat".into()],
        };
        let resp = WsHandshake::accept(&req);
        assert_eq!(resp.status, 101);
        assert!(!resp.accept_key.is_empty());
        assert_eq!(resp.protocol, Some("chat".into()));
    }

    #[test]
    fn w7_3_handshake_response_format() {
        let resp = WsHandshakeResponse {
            status: 101,
            accept_key: "accept-1234".into(),
            protocol: Some("chat".into()),
        };
        let http = WsHandshake::build_response(&resp);
        assert!(http.contains("101 Switching Protocols"));
        assert!(http.contains("Sec-WebSocket-Accept: accept-1234"));
        assert!(http.contains("Sec-WebSocket-Protocol: chat"));
    }

    // W7.4: WsClient
    #[test]
    fn w7_4_client_lifecycle() {
        let mut client = WsClient::new(1, "Fajar");
        assert_eq!(client.state, ConnectionState::Disconnected);
        client.connect();
        assert_eq!(client.state, ConnectionState::Connected);
        client.send_text("hello").unwrap();
        assert_eq!(client.sent_count, 1);
        client.disconnect();
        assert_eq!(client.state, ConnectionState::Closed);
    }

    #[test]
    fn w7_4_client_send_requires_connected() {
        let mut client = WsClient::new(1, "Alice");
        assert!(client.send_text("fail").is_err());
    }

    // W7.5: ConnectionManager
    #[test]
    fn w7_5_connection_manager() {
        let mut mgr = ConnectionManager::new();
        let id1 = mgr.register("Alice");
        let id2 = mgr.register("Bob");
        assert_eq!(mgr.connected_count(), 2);
        assert!(mgr.get(id1).is_some());
        mgr.disconnect(id1);
        assert_eq!(mgr.connected_count(), 1);
        assert!(mgr.get(id1).is_none());
        assert!(mgr.connected_ids().contains(&id2));
    }

    // W7.6: ChatHistory
    #[test]
    fn w7_6_history_push_and_page() {
        let mut hist = ChatHistory::new();
        for i in 0..25 {
            hist.push(ChatMessage::System {
                content: format!("msg {i}"),
                timestamp: i as u64,
            });
        }
        assert_eq!(hist.len(), 25);
        assert_eq!(hist.page_count(10), 3);
        assert_eq!(hist.page(0, 10).len(), 10);
        assert_eq!(hist.page(2, 10).len(), 5);
        assert_eq!(hist.page(5, 10).len(), 0);
    }

    #[test]
    fn w7_6_history_max_size() {
        let mut hist = ChatHistory::with_max_size(5);
        for i in 0..10 {
            hist.push(ChatMessage::System {
                content: format!("msg {i}"),
                timestamp: i as u64,
            });
        }
        assert_eq!(hist.len(), 5);
        assert_eq!(hist.recent(2).len(), 2);
    }

    // W7.7: ChatRoom
    #[test]
    fn w7_7_room_join_leave() {
        let mut room = ChatRoom::new("general", "General");
        room.join(1, "Alice", 100).unwrap();
        room.join(2, "Bob", 101).unwrap();
        assert_eq!(room.member_count(), 2);
        assert!(room.has_member(1));
        room.leave(1, "Alice", 200).unwrap();
        assert_eq!(room.member_count(), 1);
        assert!(!room.has_member(1));
    }

    #[test]
    fn w7_7_room_full() {
        let mut room = ChatRoom::new("vip", "VIP").with_max_members(2);
        room.join(1, "A", 0).unwrap();
        room.join(2, "B", 0).unwrap();
        assert!(room.join(3, "C", 0).is_err());
    }

    #[test]
    fn w7_7_room_broadcast() {
        let mut room = ChatRoom::new("chat", "Chat");
        room.join(1, "Alice", 0).unwrap();
        room.join(2, "Bob", 0).unwrap();
        room.join(3, "Charlie", 0).unwrap();
        let recipients = room.broadcast(1, "Alice", "Hello!", 100).unwrap();
        assert_eq!(recipients.len(), 2);
        assert!(!recipients.contains(&1));
        assert!(recipients.contains(&2));
        assert!(recipients.contains(&3));
    }

    // W7.8: WsServer
    #[test]
    fn w7_8_server_full_flow() {
        let mut server = WsServer::new();
        server.create_room("general", "General").unwrap();

        let alice = server.connect_client("Alice");
        let bob = server.connect_client("Bob");

        server.join_room(alice, "general").unwrap();
        server.join_room(bob, "general").unwrap();

        let recipients = server
            .send_message(alice, "general", "Hello!", 1000)
            .unwrap();
        assert_eq!(recipients.len(), 1);
        assert!(recipients.contains(&bob));

        let room = server.room("general").unwrap();
        assert_eq!(room.member_count(), 2);
        assert_eq!(room.history.len(), 3); // 2 joins + 1 text

        server.disconnect_client(alice);
        let room = server.room("general").unwrap();
        assert_eq!(room.member_count(), 1);
    }

    #[test]
    fn w7_8_server_duplicate_room() {
        let mut server = WsServer::new();
        server.create_room("chat", "Chat").unwrap();
        assert!(server.create_room("chat", "Chat2").is_err());
    }

    #[test]
    fn w7_8_server_unknown_room() {
        let mut server = WsServer::new();
        let id = server.connect_client("X");
        assert!(server.join_room(id, "nonexistent").is_err());
    }
}
