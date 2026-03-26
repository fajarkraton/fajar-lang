//! Async I/O and networking runtime — Phase 6 of Fajar Lang v0.9.
//!
//! Provides simulated async I/O backends (io_uring, epoll), TCP/UDP networking,
//! HTTP client/server, WebSocket, gRPC, and resilience patterns (rate limiting,
//! circuit breaker, retry). All implementations are simulation-only — no real
//! system calls or network sockets are opened.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from async I/O and networking operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IoError {
    /// An I/O operation was cancelled before completion.
    #[error("I/O operation {op_id} cancelled")]
    Cancelled {
        /// The operation ID that was cancelled.
        op_id: u64,
    },

    /// The file descriptor is invalid or not open.
    #[error("bad file descriptor: {fd}")]
    BadFd {
        /// The invalid file descriptor.
        fd: i64,
    },

    /// The submission queue is full.
    #[error("submission queue full (capacity: {capacity})")]
    SubmissionQueueFull {
        /// Maximum queue capacity.
        capacity: usize,
    },

    /// Connection was refused by the remote host.
    #[error("connection refused: {addr}:{port}")]
    ConnectionRefused {
        /// Remote address.
        addr: String,
        /// Remote port.
        port: u16,
    },

    /// DNS resolution failed.
    #[error("DNS resolution failed for: {hostname}")]
    DnsResolutionFailed {
        /// The hostname that failed to resolve.
        hostname: String,
    },

    /// Connection pool exhausted.
    #[error("connection pool exhausted (max: {max})")]
    PoolExhausted {
        /// Maximum connections allowed.
        max: usize,
    },

    /// HTTP parse error.
    #[error("HTTP parse error: {detail}")]
    HttpParseError {
        /// Description of the parse failure.
        detail: String,
    },

    /// Route not found for the given method and path.
    #[error("no route for {method} {path}")]
    RouteNotFound {
        /// HTTP method.
        method: String,
        /// Request path.
        path: String,
    },

    /// WebSocket protocol error.
    #[error("WebSocket error: {detail}")]
    WebSocketError {
        /// Description of the WebSocket failure.
        detail: String,
    },

    /// gRPC status error.
    #[error("gRPC error: {status:?} — {message}")]
    GrpcError {
        /// gRPC status code.
        status: GrpcStatus,
        /// Error message.
        message: String,
    },

    /// Circuit breaker is open — requests are rejected.
    #[error("circuit breaker open: {failures} failures (threshold: {threshold})")]
    CircuitOpen {
        /// Number of consecutive failures.
        failures: u32,
        /// Threshold that triggered the open state.
        threshold: u32,
    },

    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    RateLimitExceeded,

    /// Generic I/O error with a message.
    #[error("I/O error: {message}")]
    Other {
        /// Error description.
        message: String,
    },
}

/// Convenience alias for results in this module.
pub type Result<T> = std::result::Result<T, IoError>;

// ═══════════════════════════════════════════════════════════════════════
// Sprint 21 — Async I/O Backend
// ═══════════════════════════════════════════════════════════════════════

/// An I/O operation to be submitted to an async backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoOp {
    /// Asynchronous file/socket read.
    Read {
        /// File descriptor to read from.
        fd: i64,
        /// Number of bytes to read.
        buf_size: usize,
        /// Byte offset within the file.
        offset: u64,
    },
    /// Asynchronous file/socket write.
    Write {
        /// File descriptor to write to.
        fd: i64,
        /// Data bytes to write.
        data: Vec<u8>,
        /// Byte offset within the file.
        offset: u64,
    },
    /// Accept an incoming connection on a listening socket.
    Accept {
        /// Listening socket file descriptor.
        fd: i64,
    },
    /// Connect to a remote address.
    Connect {
        /// Remote IP address or hostname.
        addr: String,
        /// Remote port.
        port: u16,
    },
    /// Close a file descriptor.
    Close {
        /// File descriptor to close.
        fd: i64,
    },
}

/// Result of a completed I/O operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoResult {
    /// The operation ID assigned at submission time.
    pub op_id: u64,
    /// Outcome: `Ok(bytes_transferred)` or `Err(IoError)`.
    pub result: std::result::Result<usize, IoError>,
    /// Simulated elapsed time in microseconds.
    pub elapsed_us: u64,
}

/// Trait for async I/O backends (io_uring, epoll, etc.).
pub trait IoBackend {
    /// Submit an I/O operation and return its unique operation ID.
    fn submit(&mut self, op: IoOp) -> Result<u64>;

    /// Poll for completed operations, returning all available results.
    fn poll_completion(&mut self) -> Vec<IoResult>;

    /// Cancel a previously submitted operation by its ID.
    fn cancel(&mut self, op_id: u64) -> Result<()>;
}

// ───────────────────────────────────────────────────────────────────────
// io_uring simulation
// ───────────────────────────────────────────────────────────────────────

/// Submission queue entry for the simulated io_uring.
#[derive(Debug, Clone)]
pub struct Sqe {
    /// Unique operation ID.
    pub op_id: u64,
    /// The I/O operation.
    pub op: IoOp,
}

/// Completion queue entry for the simulated io_uring.
#[derive(Debug, Clone)]
pub struct Cqe {
    /// The operation ID that completed.
    pub op_id: u64,
    /// Result value (bytes transferred, or negative on error).
    pub res: i64,
}

/// Simulated submission queue with bounded capacity.
#[derive(Debug)]
pub struct SubmissionQueue {
    /// Entries waiting to be processed.
    entries: Vec<Sqe>,
    /// Maximum number of entries.
    capacity: usize,
}

impl SubmissionQueue {
    /// Create a new submission queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push an entry. Returns error if queue is full.
    pub fn push(&mut self, sqe: Sqe) -> Result<()> {
        if self.entries.len() >= self.capacity {
            return Err(IoError::SubmissionQueueFull {
                capacity: self.capacity,
            });
        }
        self.entries.push(sqe);
        Ok(())
    }

    /// Drain all pending entries for processing.
    pub fn drain_all(&mut self) -> Vec<Sqe> {
        std::mem::take(&mut self.entries)
    }

    /// Number of entries currently queued.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Simulated completion queue (unbounded).
#[derive(Debug)]
pub struct CompletionQueue {
    /// Completed entries ready for consumption.
    entries: Vec<Cqe>,
}

impl CompletionQueue {
    /// Create a new empty completion queue.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a completion entry.
    pub fn push(&mut self, cqe: Cqe) {
        self.entries.push(cqe);
    }

    /// Drain all completed entries.
    pub fn drain_all(&mut self) -> Vec<Cqe> {
        std::mem::take(&mut self.entries)
    }

    /// Number of entries available.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CompletionQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Simulated io_uring backend.
///
/// Models the Linux io_uring interface with a submission queue and
/// completion queue. All I/O is simulated — no real kernel calls.
#[derive(Debug)]
pub struct IoUringBackend {
    /// Submission queue.
    sq: SubmissionQueue,
    /// Completion queue.
    cq: CompletionQueue,
    /// Next operation ID to assign.
    next_op_id: u64,
    /// Set of cancelled operation IDs.
    cancelled: Vec<u64>,
    /// Simulated open file descriptors.
    open_fds: Vec<i64>,
}

impl IoUringBackend {
    /// Create a new io_uring backend with the given SQ depth.
    pub fn new(sq_depth: usize) -> Self {
        Self {
            sq: SubmissionQueue::new(sq_depth),
            cq: CompletionQueue::new(),
            next_op_id: 1,
            cancelled: Vec::new(),
            open_fds: vec![0, 1, 2], // stdin, stdout, stderr
        }
    }

    /// Register a file descriptor as open (for simulation).
    pub fn register_fd(&mut self, fd: i64) {
        if !self.open_fds.contains(&fd) {
            self.open_fds.push(fd);
        }
    }

    /// Process all pending submissions into completions.
    fn process_submissions(&mut self) {
        let pending = self.sq.drain_all();
        for sqe in pending {
            if self.cancelled.contains(&sqe.op_id) {
                continue;
            }
            let cqe = self.simulate_op(&sqe);
            self.cq.push(cqe);
        }
        self.cancelled.clear();
    }

    /// Simulate a single I/O operation, producing a CQE.
    fn simulate_op(&mut self, sqe: &Sqe) -> Cqe {
        let res = match &sqe.op {
            IoOp::Read { fd, buf_size, .. } => {
                if self.open_fds.contains(fd) {
                    *buf_size as i64
                } else {
                    -1
                }
            }
            IoOp::Write { fd, data, .. } => {
                if self.open_fds.contains(fd) {
                    data.len() as i64
                } else {
                    -1
                }
            }
            IoOp::Accept { fd } => {
                if self.open_fds.contains(fd) {
                    let new_fd = 100 + sqe.op_id as i64;
                    self.open_fds.push(new_fd);
                    new_fd
                } else {
                    -1
                }
            }
            IoOp::Connect { .. } => {
                let new_fd = 200 + sqe.op_id as i64;
                self.open_fds.push(new_fd);
                new_fd
            }
            IoOp::Close { fd } => {
                self.open_fds.retain(|f| f != fd);
                0
            }
        };
        Cqe {
            op_id: sqe.op_id,
            res,
        }
    }
}

impl IoBackend for IoUringBackend {
    fn submit(&mut self, op: IoOp) -> Result<u64> {
        let op_id = self.next_op_id;
        self.next_op_id += 1;
        self.sq.push(Sqe { op_id, op })?;
        Ok(op_id)
    }

    fn poll_completion(&mut self) -> Vec<IoResult> {
        self.process_submissions();
        self.cq
            .drain_all()
            .into_iter()
            .map(|cqe| {
                let result = if cqe.res >= 0 {
                    Ok(cqe.res as usize)
                } else {
                    Err(IoError::BadFd { fd: -1 })
                };
                IoResult {
                    op_id: cqe.op_id,
                    result,
                    elapsed_us: 50,
                }
            })
            .collect()
    }

    fn cancel(&mut self, op_id: u64) -> Result<()> {
        self.cancelled.push(op_id);
        Ok(())
    }
}

// ───────────────────────────────────────────────────────────────────────
// Epoll simulation
// ───────────────────────────────────────────────────────────────────────

/// Event reported by the epoll backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpollEvent {
    /// File descriptor this event pertains to.
    pub fd: i64,
    /// Whether the fd is readable.
    pub readable: bool,
    /// Whether the fd is writable.
    pub writable: bool,
    /// Whether the fd is in an error state.
    pub error: bool,
}

/// Interest flags for epoll registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest {
    /// Watch for readability.
    pub readable: bool,
    /// Watch for writability.
    pub writable: bool,
}

/// Simulated epoll backend.
///
/// Tracks file descriptors and their interest sets, producing events
/// on poll. All events are simulated.
#[derive(Debug)]
pub struct EpollBackend {
    /// Registered file descriptors and their interest sets.
    interests: HashMap<i64, Interest>,
    /// Simulated event queue (populated by `inject_event`).
    pending_events: Vec<EpollEvent>,
}

impl EpollBackend {
    /// Create a new epoll instance.
    pub fn new() -> Self {
        Self {
            interests: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    /// Register a file descriptor with interest flags.
    pub fn register(&mut self, fd: i64, interest: Interest) {
        self.interests.insert(fd, interest);
    }

    /// Remove a file descriptor from the interest set.
    pub fn deregister(&mut self, fd: i64) {
        self.interests.remove(&fd);
    }

    /// Inject a simulated event for testing purposes.
    pub fn inject_event(&mut self, event: EpollEvent) {
        self.pending_events.push(event);
    }

    /// Poll for events, returning those that match registered interests.
    pub fn poll_events(&mut self) -> Vec<EpollEvent> {
        let drained = std::mem::take(&mut self.pending_events);
        drained
            .into_iter()
            .filter(|ev| self.interests.contains_key(&ev.fd))
            .collect()
    }
}

impl Default for EpollBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────────────────────────────────────────────────
// Buffered I/O
// ───────────────────────────────────────────────────────────────────────

/// Buffered reader with configurable read-ahead size.
///
/// Wraps a simulated source and provides buffered access.
#[derive(Debug)]
pub struct BufReader {
    /// Internal buffer holding pre-fetched data.
    buffer: Vec<u8>,
    /// Current read position within the buffer.
    pos: usize,
    /// Capacity of the internal buffer.
    capacity: usize,
    /// Underlying data source (simulated).
    source: Vec<u8>,
    /// Position within the source.
    source_pos: usize,
}

impl BufReader {
    /// Create a new buffered reader over `source` with given capacity.
    pub fn new(source: Vec<u8>, capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            pos: 0,
            capacity,
            source,
            source_pos: 0,
        }
    }

    /// Read up to `count` bytes from the buffered source.
    pub fn read(&mut self, count: usize) -> Vec<u8> {
        self.fill_if_needed();
        let available = self.buffer.len() - self.pos;
        let to_read = count.min(available);
        let data = self.buffer[self.pos..self.pos + to_read].to_vec();
        self.pos += to_read;
        data
    }

    /// Fill the internal buffer from the source when exhausted.
    fn fill_if_needed(&mut self) {
        if self.pos >= self.buffer.len() {
            let end = (self.source_pos + self.capacity).min(self.source.len());
            self.buffer = self.source[self.source_pos..end].to_vec();
            self.source_pos = end;
            self.pos = 0;
        }
    }

    /// Number of bytes remaining in the source + buffer.
    pub fn remaining(&self) -> usize {
        let buffered = self.buffer.len().saturating_sub(self.pos);
        let unread = self.source.len().saturating_sub(self.source_pos);
        buffered + unread
    }
}

/// Buffered writer with write-behind flushing.
///
/// Accumulates writes and flushes when the buffer exceeds capacity.
#[derive(Debug)]
pub struct BufWriter {
    /// Internal write buffer.
    buffer: Vec<u8>,
    /// Buffer capacity — flush triggers when exceeded.
    capacity: usize,
    /// Flushed output (simulated destination).
    flushed: Vec<u8>,
}

impl BufWriter {
    /// Create a new buffered writer with given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            capacity,
            flushed: Vec::new(),
        }
    }

    /// Write data to the buffer. Auto-flushes when capacity exceeded.
    pub fn write(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        if self.buffer.len() >= self.capacity {
            self.flush();
        }
    }

    /// Flush all buffered data to the destination.
    pub fn flush(&mut self) {
        self.flushed.append(&mut self.buffer);
    }

    /// Return a reference to all flushed data (for testing).
    pub fn flushed_data(&self) -> &[u8] {
        &self.flushed
    }

    /// Number of bytes currently buffered (unflushed).
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 22 — TCP/UDP Networking
// ═══════════════════════════════════════════════════════════════════════

/// A parsed socket address (ip + port).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    /// IP address string (e.g. "127.0.0.1").
    pub ip: String,
    /// Port number.
    pub port: u16,
}

impl SocketAddr {
    /// Create a new socket address.
    pub fn new(ip: impl Into<String>, port: u16) -> Self {
        Self {
            ip: ip.into(),
            port,
        }
    }

    /// Parse a "host:port" string into a `SocketAddr`.
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(IoError::HttpParseError {
                detail: format!("invalid socket address: {s}"),
            });
        }
        let port: u16 = parts[0].parse().map_err(|_| IoError::HttpParseError {
            detail: format!("invalid port in: {s}"),
        })?;
        let ip = parts[1].to_string();
        Ok(Self { ip, port })
    }

    /// Format as "ip:port".
    pub fn to_string_repr(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// Socket configuration options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SocketOptions {
    /// Allow address reuse (SO_REUSEADDR).
    pub reuse_addr: bool,
    /// Disable Nagle's algorithm (TCP_NODELAY).
    pub nodelay: bool,
    /// Enable TCP keep-alive.
    pub keepalive: bool,
    /// Timeout in milliseconds (0 = no timeout).
    pub timeout_ms: u64,
}

/// Simulated TCP listener bound to an address.
#[derive(Debug)]
pub struct TcpListener {
    /// Simulated file descriptor.
    pub fd: i64,
    /// Address the listener is bound to.
    pub bound_addr: SocketAddr,
    /// Maximum pending connections.
    pub backlog: usize,
    /// Simulated pending connection queue.
    pending: Vec<SocketAddr>,
    /// Socket options.
    options: SocketOptions,
}

impl TcpListener {
    /// Bind a TCP listener to the given address.
    pub fn bind(addr: SocketAddr, backlog: usize) -> Self {
        Self {
            fd: 10,
            bound_addr: addr,
            backlog,
            pending: Vec::new(),
            options: SocketOptions::default(),
        }
    }

    /// Set socket options.
    pub fn set_options(&mut self, opts: SocketOptions) {
        self.options = opts;
    }

    /// Inject a simulated incoming connection for testing.
    pub fn inject_connection(&mut self, peer: SocketAddr) {
        if self.pending.len() < self.backlog {
            self.pending.push(peer);
        }
    }

    /// Accept the next pending connection, returning a `TcpStream`.
    pub fn accept(&mut self) -> Result<TcpStream> {
        if let Some(peer) = self.pending.pop() {
            let stream = TcpStream {
                fd: 20 + self.pending.len() as i64,
                local_addr: self.bound_addr.clone(),
                peer_addr: peer,
                connected: true,
                rx_buffer: Vec::new(),
                tx_buffer: Vec::new(),
            };
            Ok(stream)
        } else {
            Err(IoError::Other {
                message: "no pending connections".into(),
            })
        }
    }
}

/// Simulated TCP stream (bidirectional connection).
#[derive(Debug)]
pub struct TcpStream {
    /// Simulated file descriptor.
    pub fd: i64,
    /// Local socket address.
    pub local_addr: SocketAddr,
    /// Remote socket address.
    pub peer_addr: SocketAddr,
    /// Whether the connection is established.
    pub connected: bool,
    /// Receive buffer (simulated incoming data).
    rx_buffer: Vec<u8>,
    /// Transmit buffer (data written by the application).
    tx_buffer: Vec<u8>,
}

impl TcpStream {
    /// Connect to a remote address (simulation).
    pub fn connect(addr: SocketAddr) -> Self {
        Self {
            fd: 30,
            local_addr: SocketAddr::new("127.0.0.1", 0),
            peer_addr: addr,
            connected: true,
            rx_buffer: Vec::new(),
            tx_buffer: Vec::new(),
        }
    }

    /// Inject data into the receive buffer (for testing).
    pub fn inject_rx_data(&mut self, data: &[u8]) {
        self.rx_buffer.extend_from_slice(data);
    }

    /// Read up to `count` bytes from the stream.
    pub fn read(&mut self, count: usize) -> Vec<u8> {
        let to_read = count.min(self.rx_buffer.len());
        self.rx_buffer.drain(..to_read).collect()
    }

    /// Write data to the stream.
    pub fn write(&mut self, data: &[u8]) -> usize {
        self.tx_buffer.extend_from_slice(data);
        data.len()
    }

    /// Return all data written to the stream (for testing).
    pub fn transmitted_data(&self) -> &[u8] {
        &self.tx_buffer
    }

    /// Close the stream.
    pub fn close(&mut self) {
        self.connected = false;
    }
}

/// Simulated UDP socket.
#[derive(Debug)]
pub struct UdpSocket {
    /// Simulated file descriptor.
    pub fd: i64,
    /// Address the socket is bound to.
    pub bound_addr: SocketAddr,
    /// Received datagrams: (source addr, data).
    rx_queue: Vec<(SocketAddr, Vec<u8>)>,
    /// Sent datagrams: (dest addr, data).
    tx_queue: Vec<(SocketAddr, Vec<u8>)>,
}

impl UdpSocket {
    /// Bind a UDP socket to the given address.
    pub fn bind(addr: SocketAddr) -> Self {
        Self {
            fd: 40,
            bound_addr: addr,
            rx_queue: Vec::new(),
            tx_queue: Vec::new(),
        }
    }

    /// Inject a received datagram (for testing).
    pub fn inject_datagram(&mut self, src: SocketAddr, data: Vec<u8>) {
        self.rx_queue.push((src, data));
    }

    /// Send a datagram to the given address.
    pub fn send_to(&mut self, dest: SocketAddr, data: &[u8]) -> usize {
        let len = data.len();
        self.tx_queue.push((dest, data.to_vec()));
        len
    }

    /// Receive the next datagram, returning (source addr, data).
    pub fn recv_from(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        if self.rx_queue.is_empty() {
            None
        } else {
            Some(self.rx_queue.remove(0))
        }
    }

    /// Return all sent datagrams (for testing).
    pub fn sent_datagrams(&self) -> &[(SocketAddr, Vec<u8>)] {
        &self.tx_queue
    }
}

/// Simulate DNS resolution (returns hardcoded addresses for known names).
pub fn dns_resolve(hostname: &str) -> Result<Vec<String>> {
    match hostname {
        "localhost" => Ok(vec!["127.0.0.1".to_string()]),
        "example.com" => Ok(vec!["93.184.216.34".to_string()]),
        "fajar-lang.dev" => Ok(vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()]),
        _ => Err(IoError::DnsResolutionFailed {
            hostname: hostname.to_string(),
        }),
    }
}

/// Connection pool for reusing TCP connections.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Maximum number of connections.
    pub max_connections: usize,
    /// Idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
    /// Active connections keyed by address string.
    active: HashMap<String, Vec<TcpStream>>,
    /// Total number of connections currently in the pool.
    total: usize,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(max_connections: usize, idle_timeout_ms: u64) -> Self {
        Self {
            max_connections,
            idle_timeout_ms,
            active: HashMap::new(),
            total: 0,
        }
    }

    /// Get a connection to the given address, creating one if needed.
    pub fn get(&mut self, addr: &SocketAddr) -> Result<TcpStream> {
        let key = addr.to_string_repr();
        if let Some(pool) = self.active.get_mut(&key) {
            if let Some(stream) = pool.pop() {
                return Ok(stream);
            }
        }
        if self.total >= self.max_connections {
            return Err(IoError::PoolExhausted {
                max: self.max_connections,
            });
        }
        self.total += 1;
        Ok(TcpStream::connect(addr.clone()))
    }

    /// Return a connection to the pool for reuse.
    pub fn release(&mut self, stream: TcpStream) {
        let key = stream.peer_addr.to_string_repr();
        self.active.entry(key).or_default().push(stream);
    }

    /// Number of active connections across all addresses.
    pub fn active_count(&self) -> usize {
        self.total
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 23 — HTTP Client/Server
// ═══════════════════════════════════════════════════════════════════════

/// HTTP methods.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    /// GET request.
    GET,
    /// POST request.
    POST,
    /// PUT request.
    PUT,
    /// DELETE request.
    DELETE,
    /// PATCH request.
    PATCH,
    /// HEAD request.
    HEAD,
    /// OPTIONS request.
    OPTIONS,
}

impl HttpMethod {
    /// Parse an HTTP method from a string.
    pub fn from_str_checked(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::GET),
            "POST" => Ok(Self::POST),
            "PUT" => Ok(Self::PUT),
            "DELETE" => Ok(Self::DELETE),
            "PATCH" => Ok(Self::PATCH),
            "HEAD" => Ok(Self::HEAD),
            "OPTIONS" => Ok(Self::OPTIONS),
            _ => Err(IoError::HttpParseError {
                detail: format!("unknown method: {s}"),
            }),
        }
    }

    /// Convert to the canonical string form.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
            Self::PATCH => "PATCH",
            Self::HEAD => "HEAD",
            Self::OPTIONS => "OPTIONS",
        }
    }
}

/// HTTP request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Request method.
    pub method: HttpMethod,
    /// Request path (e.g. "/api/data").
    pub path: String,
    /// HTTP headers.
    pub headers: HashMap<String, String>,
    /// Optional request body.
    pub body: Option<String>,
    /// HTTP version string (e.g. "HTTP/1.1").
    pub version: String,
}

/// HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code (e.g. 200, 404).
    pub status_code: u16,
    /// Status text (e.g. "OK", "Not Found").
    pub status_text: String,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Response body.
    pub body: String,
}

impl HttpResponse {
    /// Create a simple 200 OK response.
    pub fn ok(body: impl Into<String>) -> Self {
        Self {
            status_code: 200,
            status_text: "OK".into(),
            headers: HashMap::new(),
            body: body.into(),
        }
    }

    /// Create a 404 Not Found response.
    pub fn not_found() -> Self {
        Self {
            status_code: 404,
            status_text: "Not Found".into(),
            headers: HashMap::new(),
            body: "Not Found".into(),
        }
    }
}

/// Parse a raw HTTP request string into an `HttpRequest`.
///
/// Expected format:
/// ```text
/// METHOD /path HTTP/1.1\r\n
/// Header: Value\r\n
/// \r\n
/// optional body
/// ```
pub fn parse_request(raw: &str) -> Result<HttpRequest> {
    let parts: Vec<&str> = raw.splitn(2, "\r\n\r\n").collect();
    let head = parts.first().ok_or_else(|| IoError::HttpParseError {
        detail: "empty request".into(),
    })?;
    let body = parts.get(1).and_then(|b| {
        if b.is_empty() {
            None
        } else {
            Some((*b).to_string())
        }
    });

    let mut lines = head.lines();
    let request_line = lines.next().ok_or_else(|| IoError::HttpParseError {
        detail: "missing request line".into(),
    })?;

    let tokens: Vec<&str> = request_line.split_whitespace().collect();
    if tokens.len() < 3 {
        return Err(IoError::HttpParseError {
            detail: format!("malformed request line: {request_line}"),
        });
    }

    let method = HttpMethod::from_str_checked(tokens[0])?;
    let path = tokens[1].to_string();
    let version = tokens[2].to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
        version,
    })
}

/// Build a raw HTTP response string from an `HttpResponse`.
pub fn build_response(resp: &HttpResponse) -> String {
    let mut out = format!("HTTP/1.1 {} {}\r\n", resp.status_code, resp.status_text);
    for (k, v) in &resp.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!("Content-Length: {}\r\n", resp.body.len()));
    out.push_str("\r\n");
    out.push_str(&resp.body);
    out
}

/// Content types for HTTP bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    /// application/json
    Json,
    /// application/x-www-form-urlencoded
    FormUrlencoded,
    /// multipart/form-data
    Multipart,
    /// text/plain
    PlainText,
    /// text/html
    Html,
}

impl ContentType {
    /// Return the MIME type string.
    pub fn mime(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::FormUrlencoded => "application/x-www-form-urlencoded",
            Self::Multipart => "multipart/form-data",
            Self::PlainText => "text/plain",
            Self::Html => "text/html",
        }
    }
}

/// Simulated HTTP client.
///
/// Returns mock responses for all requests. Useful for testing
/// HTTP-based protocols without a real network.
#[derive(Debug)]
pub struct HttpClient {
    /// Default headers sent with every request.
    pub default_headers: HashMap<String, String>,
}

impl HttpClient {
    /// Create a new HTTP client.
    pub fn new() -> Self {
        Self {
            default_headers: HashMap::new(),
        }
    }

    /// Simulated GET request — returns a mock 200 response.
    pub fn get(&self, url: &str) -> HttpResponse {
        HttpResponse {
            status_code: 200,
            status_text: "OK".into(),
            headers: self.default_headers.clone(),
            body: format!("GET response for {url}"),
        }
    }

    /// Simulated POST request — returns a mock 201 response.
    pub fn post(&self, url: &str, body: &str) -> HttpResponse {
        HttpResponse {
            status_code: 201,
            status_text: "Created".into(),
            headers: self.default_headers.clone(),
            body: format!("POST to {url}: {body}"),
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// A registered HTTP route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// HTTP method this route matches.
    pub method: HttpMethod,
    /// Path pattern (exact match).
    pub path: String,
    /// Name of the handler function (resolved at Fajar Lang level).
    pub handler_name: String,
}

/// Simulated HTTP server with route dispatch.
#[derive(Debug)]
pub struct HttpServer {
    /// Listen address.
    pub listen_addr: SocketAddr,
    /// Registered routes.
    routes: Vec<Route>,
    /// CORS configuration.
    cors: Option<CorsConfig>,
}

impl HttpServer {
    /// Create a new HTTP server listening on the given address.
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            listen_addr: addr,
            routes: Vec::new(),
            cors: None,
        }
    }

    /// Register a route.
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    /// Set the CORS configuration.
    pub fn set_cors(&mut self, config: CorsConfig) {
        self.cors = Some(config);
    }

    /// Dispatch a request against registered routes.
    ///
    /// Returns the matching handler name or an error.
    pub fn dispatch(&self, req: &HttpRequest) -> Result<String> {
        for route in &self.routes {
            if route.method == req.method && route.path == req.path {
                return Ok(route.handler_name.clone());
            }
        }
        Err(IoError::RouteNotFound {
            method: req.method.as_str().into(),
            path: req.path.clone(),
        })
    }

    /// Number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Get the CORS config if set.
    pub fn cors_config(&self) -> Option<&CorsConfig> {
        self.cors.as_ref()
    }
}

/// Trait for HTTP middleware that can intercept requests/responses.
pub trait HttpMiddleware {
    /// Process (potentially modify) a request and response in-flight.
    fn process(&self, req: &mut HttpRequest, resp: &mut HttpResponse);
}

/// Cross-Origin Resource Sharing configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsConfig {
    /// Allowed origin domains.
    pub allow_origins: Vec<String>,
    /// Allowed HTTP methods.
    pub allow_methods: Vec<String>,
    /// Allowed headers.
    pub allow_headers: Vec<String>,
    /// Preflight cache max age in seconds.
    pub max_age: u32,
}

impl CorsConfig {
    /// Create a permissive CORS config allowing all origins.
    pub fn permissive() -> Self {
        Self {
            allow_origins: vec!["*".into()],
            allow_methods: vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into()],
            allow_headers: vec!["Content-Type".into(), "Authorization".into()],
            max_age: 3600,
        }
    }

    /// Check if a given origin is allowed.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allow_origins.contains(&"*".to_string())
            || self.allow_origins.iter().any(|o| o == origin)
    }
}

/// A CORS middleware that applies CORS headers to responses.
pub struct CorsMiddleware {
    /// The CORS config to apply.
    pub config: CorsConfig,
}

impl HttpMiddleware for CorsMiddleware {
    fn process(&self, _req: &mut HttpRequest, resp: &mut HttpResponse) {
        resp.headers.insert(
            "Access-Control-Allow-Origin".into(),
            self.config.allow_origins.join(", "),
        );
        resp.headers.insert(
            "Access-Control-Allow-Methods".into(),
            self.config.allow_methods.join(", "),
        );
        resp.headers.insert(
            "Access-Control-Max-Age".into(),
            self.config.max_age.to_string(),
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 24 — Protocol Integration
// ═══════════════════════════════════════════════════════════════════════

// ───────────────────────────────────────────────────────────────────────
// WebSocket
// ───────────────────────────────────────────────────────────────────────

/// WebSocket frame opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOpcode {
    /// UTF-8 text frame.
    Text,
    /// Binary frame.
    Binary,
    /// Ping control frame.
    Ping,
    /// Pong control frame.
    Pong,
    /// Connection close frame.
    Close,
}

impl WsOpcode {
    /// Convert to the wire byte value per RFC 6455.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Text => 0x1,
            Self::Binary => 0x2,
            Self::Ping => 0x9,
            Self::Pong => 0xA,
            Self::Close => 0x8,
        }
    }

    /// Parse from a wire byte.
    pub fn from_byte(b: u8) -> Result<Self> {
        match b {
            0x1 => Ok(Self::Text),
            0x2 => Ok(Self::Binary),
            0x9 => Ok(Self::Ping),
            0xA => Ok(Self::Pong),
            0x8 => Ok(Self::Close),
            _ => Err(IoError::WebSocketError {
                detail: format!("unknown opcode: 0x{b:02X}"),
            }),
        }
    }
}

/// A WebSocket frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsFrame {
    /// Frame opcode.
    pub opcode: WsOpcode,
    /// Payload data.
    pub payload: Vec<u8>,
    /// Whether the frame is masked (client -> server).
    pub masked: bool,
}

/// Encode a WebSocket frame into wire bytes (simplified, <=125 bytes).
///
/// Format: `[FIN+opcode]` `[mask_bit+len]` `[mask_key if masked]` `[payload]`
pub fn encode_ws_frame(frame: &WsFrame) -> Vec<u8> {
    let mut out = Vec::new();
    // FIN bit (0x80) | opcode
    out.push(0x80 | frame.opcode.to_byte());

    let len = frame.payload.len() as u8;
    if frame.masked {
        out.push(0x80 | len); // mask bit set
        // Use a fixed mask key for simulation
        let mask_key: [u8; 4] = [0x37, 0xFA, 0x21, 0x3D];
        out.extend_from_slice(&mask_key);
        for (i, b) in frame.payload.iter().enumerate() {
            out.push(b ^ mask_key[i % 4]);
        }
    } else {
        out.push(len);
        out.extend_from_slice(&frame.payload);
    }
    out
}

/// Decode a WebSocket frame from wire bytes (simplified, <=125 bytes).
pub fn decode_ws_frame(data: &[u8]) -> Result<WsFrame> {
    if data.len() < 2 {
        return Err(IoError::WebSocketError {
            detail: "frame too short".into(),
        });
    }
    let opcode = WsOpcode::from_byte(data[0] & 0x0F)?;
    let masked = (data[1] & 0x80) != 0;
    let len = (data[1] & 0x7F) as usize;

    let payload_start = if masked { 6 } else { 2 };
    if data.len() < payload_start + len {
        return Err(IoError::WebSocketError {
            detail: "frame truncated".into(),
        });
    }

    let payload = if masked {
        let mask_key = &data[2..6];
        data[6..6 + len]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ mask_key[i % 4])
            .collect()
    } else {
        data[2..2 + len].to_vec()
    };

    Ok(WsFrame {
        opcode,
        payload,
        masked,
    })
}

/// Build a WebSocket upgrade HTTP request.
pub fn ws_upgrade_request(path: &str, key: &str) -> HttpRequest {
    let mut headers = HashMap::new();
    headers.insert("Upgrade".into(), "websocket".into());
    headers.insert("Connection".into(), "Upgrade".into());
    headers.insert("Sec-WebSocket-Key".into(), key.into());
    headers.insert("Sec-WebSocket-Version".into(), "13".into());
    HttpRequest {
        method: HttpMethod::GET,
        path: path.into(),
        headers,
        body: None,
        version: "HTTP/1.1".into(),
    }
}

// ───────────────────────────────────────────────────────────────────────
// gRPC simulation
// ───────────────────────────────────────────────────────────────────────

/// gRPC status codes (subset of the standard codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcStatus {
    /// Success.
    Ok,
    /// The operation was cancelled.
    Cancelled,
    /// Unknown error.
    Unknown,
    /// Client specified an invalid argument.
    InvalidArgument,
    /// Resource not found.
    NotFound,
    /// Permission denied.
    PermissionDenied,
    /// Internal server error.
    Internal,
    /// Service unavailable.
    Unavailable,
}

impl GrpcStatus {
    /// Numeric code for the status.
    pub fn code(self) -> u32 {
        match self {
            Self::Ok => 0,
            Self::Cancelled => 1,
            Self::Unknown => 2,
            Self::InvalidArgument => 3,
            Self::NotFound => 5,
            Self::PermissionDenied => 7,
            Self::Internal => 13,
            Self::Unavailable => 14,
        }
    }
}

/// A simulated gRPC message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcMessage {
    /// Service name (e.g. "fj.inference.InferenceService").
    pub service: String,
    /// Method name (e.g. "Predict").
    pub method: String,
    /// Serialized payload bytes.
    pub payload_bytes: Vec<u8>,
}

/// Create a gRPC unary call and return a simulated response.
pub fn grpc_unary_call(msg: &GrpcMessage) -> std::result::Result<Vec<u8>, IoError> {
    if msg.service.is_empty() || msg.method.is_empty() {
        return Err(IoError::GrpcError {
            status: GrpcStatus::InvalidArgument,
            message: "service and method must be non-empty".into(),
        });
    }
    // Simulated response: echo payload with a prefix
    let mut response = b"grpc-ok:".to_vec();
    response.extend_from_slice(&msg.payload_bytes);
    Ok(response)
}

// ───────────────────────────────────────────────────────────────────────
// HTTP/2 framing simulation
// ───────────────────────────────────────────────────────────────────────

/// Simulated HTTP/2 frame types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum H2Frame {
    /// DATA frame carrying request/response body.
    Data {
        /// Stream ID.
        stream_id: u32,
        /// Payload.
        payload: Vec<u8>,
        /// End of stream.
        end_stream: bool,
    },
    /// HEADERS frame.
    Headers {
        /// Stream ID.
        stream_id: u32,
        /// Encoded headers (simplified as key-value pairs).
        headers: Vec<(String, String)>,
        /// End of stream.
        end_stream: bool,
    },
    /// PRIORITY frame.
    Priority {
        /// Stream ID.
        stream_id: u32,
        /// Dependency stream.
        dependency: u32,
        /// Weight (1-256).
        weight: u8,
    },
    /// SETTINGS frame.
    Settings {
        /// Settings as key-value pairs.
        values: Vec<(u16, u32)>,
    },
    /// PING frame.
    Ping {
        /// 8-byte opaque data.
        data: [u8; 8],
    },
    /// GOAWAY frame.
    GoAway {
        /// Last processed stream ID.
        last_stream_id: u32,
        /// Error code.
        error_code: u32,
    },
}

impl H2Frame {
    /// Return the frame type byte per HTTP/2 spec.
    pub fn frame_type(&self) -> u8 {
        match self {
            Self::Data { .. } => 0x0,
            Self::Headers { .. } => 0x1,
            Self::Priority { .. } => 0x2,
            Self::Settings { .. } => 0x4,
            Self::Ping { .. } => 0x6,
            Self::GoAway { .. } => 0x7,
        }
    }
}

// ───────────────────────────────────────────────────────────────────────
// Protocol stack
// ───────────────────────────────────────────────────────────────────────

/// Protocol schemes supported by the unified stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    /// Plain TCP connection.
    Tcp,
    /// UDP datagrams.
    Udp,
    /// WebSocket over TCP.
    WebSocket,
    /// gRPC (over HTTP/2).
    Grpc,
}

/// Unified protocol stack that selects transport by scheme.
#[derive(Debug)]
pub struct ProtocolStack {
    /// Registered protocols and their bound addresses.
    bindings: HashMap<String, Protocol>,
}

impl ProtocolStack {
    /// Create a new, empty protocol stack.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Register a protocol binding (e.g. "tcp://0.0.0.0:8080" -> Tcp).
    pub fn bind(&mut self, scheme: &str, proto: Protocol) {
        self.bindings.insert(scheme.to_string(), proto);
    }

    /// Look up the protocol for a given scheme/URL.
    pub fn resolve(&self, scheme: &str) -> Option<&Protocol> {
        self.bindings.get(scheme)
    }

    /// Select the appropriate protocol from a URL prefix.
    pub fn select_from_url(url: &str) -> Protocol {
        if url.starts_with("ws://") || url.starts_with("wss://") {
            Protocol::WebSocket
        } else if url.starts_with("grpc://") {
            Protocol::Grpc
        } else if url.starts_with("udp://") {
            Protocol::Udp
        } else {
            Protocol::Tcp
        }
    }
}

impl Default for ProtocolStack {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────────────────────────────────────────────────
// Rate limiter (Token Bucket)
// ───────────────────────────────────────────────────────────────────────

/// Token bucket rate limiter.
///
/// Allows a burst of `capacity` requests, then refills at `refill_rate`
/// tokens per second.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Maximum number of tokens.
    pub capacity: u64,
    /// Current available tokens.
    pub tokens: u64,
    /// Tokens added per second.
    pub refill_rate: u64,
    /// Last refill timestamp (simulated as a counter).
    pub last_refill: u64,
}

impl TokenBucket {
    /// Create a full token bucket.
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            last_refill: 0,
        }
    }

    /// Refill tokens based on elapsed time since last refill.
    pub fn refill(&mut self, now: u64) {
        if now > self.last_refill {
            let elapsed = now - self.last_refill;
            let new_tokens = elapsed.saturating_mul(self.refill_rate);
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }
}

/// Check if a request is allowed and consume one token.
///
/// Returns `true` if a token was consumed, `false` if rate-limited.
pub fn allow_request(bucket: &mut TokenBucket) -> bool {
    if bucket.tokens > 0 {
        bucket.tokens -= 1;
        true
    } else {
        false
    }
}

// ───────────────────────────────────────────────────────────────────────
// Circuit breaker
// ───────────────────────────────────────────────────────────────────────

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests are allowed.
    Closed,
    /// Failures exceeded threshold — requests are rejected.
    Open,
    /// Tentative — one probe request is allowed.
    HalfOpen,
}

/// Circuit breaker for fault tolerance.
///
/// Transitions: Closed -> Open (on failures >= threshold),
/// Open -> HalfOpen (after reset_timeout), HalfOpen -> Closed (on success)
/// or HalfOpen -> Open (on failure).
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state.
    pub state: CircuitState,
    /// Consecutive failure count.
    pub failure_count: u32,
    /// Failure count at which the breaker opens.
    pub threshold: u32,
    /// Time (simulated counter) after which Open -> HalfOpen.
    pub reset_timeout: u64,
    /// Timestamp when the breaker entered Open state.
    pub opened_at: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker in the Closed state.
    pub fn new(threshold: u32, reset_timeout: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            threshold,
            reset_timeout,
            opened_at: 0,
        }
    }

    /// Check if a request should be allowed at the given timestamp.
    pub fn allow(&mut self, now: u64) -> Result<()> {
        match self.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                if now >= self.opened_at + self.reset_timeout {
                    self.state = CircuitState::HalfOpen;
                    Ok(())
                } else {
                    Err(IoError::CircuitOpen {
                        failures: self.failure_count,
                        threshold: self.threshold,
                    })
                }
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    /// Record a successful operation.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
    }

    /// Record a failed operation at the given timestamp.
    pub fn record_failure(&mut self, now: u64) {
        self.failure_count += 1;
        if self.failure_count >= self.threshold {
            self.state = CircuitState::Open;
            self.opened_at = now;
        }
    }
}

// ───────────────────────────────────────────────────────────────────────
// Retry policy
// ───────────────────────────────────────────────────────────────────────

/// Retry policy with exponential backoff and jitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
    /// Base delay in milliseconds.
    pub base_delay_ms: u64,
    /// Multiplicative backoff factor (applied per attempt).
    pub backoff_factor: u64,
    /// Maximum random jitter in milliseconds added to each delay.
    pub jitter_ms: u64,
}

impl RetryPolicy {
    /// Create a standard retry policy.
    pub fn new(max_retries: u32, base_delay_ms: u64, backoff_factor: u64, jitter_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            backoff_factor,
            jitter_ms,
        }
    }
}

/// Compute the retry delay for the given attempt number.
///
/// Uses exponential backoff: `base_delay * backoff_factor^attempt + jitter`.
/// Jitter is deterministic here (uses attempt as seed) for reproducibility.
pub fn compute_delay(attempt: u32, policy: &RetryPolicy) -> u64 {
    let exp = policy
        .backoff_factor
        .checked_pow(attempt)
        .unwrap_or(u64::MAX);
    let base = policy.base_delay_ms.saturating_mul(exp);
    // Deterministic "jitter" seeded by attempt for reproducibility
    let jitter = if policy.jitter_ms > 0 {
        (attempt as u64 * 7 + 3) % (policy.jitter_ms + 1)
    } else {
        0
    };
    base.saturating_add(jitter)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────────────────────────
    // Sprint 21 — Async I/O Backend (s21_1 .. s21_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s21_1_io_backend_submit_and_poll() {
        let mut ring = IoUringBackend::new(64);
        ring.register_fd(3);
        let op = IoOp::Read {
            fd: 3,
            buf_size: 1024,
            offset: 0,
        };
        let id = ring.submit(op).unwrap();
        assert_eq!(id, 1);
        let results = ring.poll_completion();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].op_id, 1);
        assert_eq!(results[0].result, Ok(1024));
    }

    #[test]
    fn s21_2_io_uring_submission_queue_full() {
        let mut ring = IoUringBackend::new(1);
        ring.register_fd(3);
        let op1 = IoOp::Read {
            fd: 3,
            buf_size: 64,
            offset: 0,
        };
        let op2 = IoOp::Read {
            fd: 3,
            buf_size: 64,
            offset: 0,
        };
        ring.submit(op1).unwrap();
        let err = ring.submit(op2).unwrap_err();
        assert!(matches!(err, IoError::SubmissionQueueFull { capacity: 1 }));
    }

    #[test]
    fn s21_3_io_ops_write_simulation() {
        let mut ring = IoUringBackend::new(64);
        ring.register_fd(5);
        let op = IoOp::Write {
            fd: 5,
            data: vec![1, 2, 3, 4],
            offset: 0,
        };
        ring.submit(op).unwrap();
        let results = ring.poll_completion();
        assert_eq!(results[0].result, Ok(4));
    }

    #[test]
    fn s21_4_io_accept_returns_new_fd() {
        let mut ring = IoUringBackend::new(64);
        ring.register_fd(10);
        let op = IoOp::Accept { fd: 10 };
        let id = ring.submit(op).unwrap();
        let results = ring.poll_completion();
        // Accept returns a new fd = 100 + op_id
        assert_eq!(results[0].result, Ok((100 + id) as usize));
    }

    #[test]
    fn s21_5_io_cancel_prevents_completion() {
        let mut ring = IoUringBackend::new(64);
        ring.register_fd(3);
        let op = IoOp::Read {
            fd: 3,
            buf_size: 64,
            offset: 0,
        };
        let id = ring.submit(op).unwrap();
        ring.cancel(id).unwrap();
        let results = ring.poll_completion();
        assert!(results.is_empty());
    }

    #[test]
    fn s21_6_io_bad_fd_returns_error() {
        let mut ring = IoUringBackend::new(64);
        // fd 99 is not registered
        let op = IoOp::Read {
            fd: 99,
            buf_size: 64,
            offset: 0,
        };
        ring.submit(op).unwrap();
        let results = ring.poll_completion();
        assert!(results[0].result.is_err());
    }

    #[test]
    fn s21_7_epoll_register_and_poll() {
        let mut epoll = EpollBackend::new();
        epoll.register(
            3,
            Interest {
                readable: true,
                writable: false,
            },
        );
        epoll.inject_event(EpollEvent {
            fd: 3,
            readable: true,
            writable: false,
            error: false,
        });
        let events = epoll.poll_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].readable);
    }

    #[test]
    fn s21_8_epoll_filters_unregistered_fds() {
        let mut epoll = EpollBackend::new();
        epoll.register(
            3,
            Interest {
                readable: true,
                writable: false,
            },
        );
        epoll.inject_event(EpollEvent {
            fd: 99,
            readable: true,
            writable: false,
            error: false,
        });
        let events = epoll.poll_events();
        assert!(events.is_empty());
    }

    #[test]
    fn s21_9_buf_reader_reads_in_chunks() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut reader = BufReader::new(data, 4);
        let chunk1 = reader.read(4);
        assert_eq!(chunk1, vec![1, 2, 3, 4]);
        let chunk2 = reader.read(4);
        assert_eq!(chunk2, vec![5, 6, 7, 8]);
        assert_eq!(reader.remaining(), 2);
    }

    #[test]
    fn s21_10_buf_writer_auto_flushes() {
        let mut writer = BufWriter::new(4);
        writer.write(&[1, 2, 3]);
        assert_eq!(writer.buffered_len(), 3);
        assert!(writer.flushed_data().is_empty());
        // Writing more exceeds capacity, triggers auto-flush
        writer.write(&[4, 5]);
        assert_eq!(writer.flushed_data(), &[1, 2, 3, 4, 5]);
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 22 — TCP/UDP Networking (s22_1 .. s22_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s22_1_tcp_listener_bind_and_accept() {
        let addr = SocketAddr::new("127.0.0.1", 8080);
        let mut listener = TcpListener::bind(addr, 128);
        listener.inject_connection(SocketAddr::new("10.0.0.1", 5000));
        let stream = listener.accept().unwrap();
        assert!(stream.connected);
        assert_eq!(stream.peer_addr.ip, "10.0.0.1");
    }

    #[test]
    fn s22_2_tcp_stream_read_write() {
        let addr = SocketAddr::new("10.0.0.1", 80);
        let mut stream = TcpStream::connect(addr);
        stream.write(b"hello");
        assert_eq!(stream.transmitted_data(), b"hello");
        stream.inject_rx_data(b"world");
        let data = stream.read(5);
        assert_eq!(data, b"world");
    }

    #[test]
    fn s22_3_tcp_stream_connect_sets_fields() {
        let addr = SocketAddr::new("93.184.216.34", 443);
        let stream = TcpStream::connect(addr);
        assert!(stream.connected);
        assert_eq!(stream.peer_addr.port, 443);
        assert_eq!(stream.local_addr.ip, "127.0.0.1");
    }

    #[test]
    fn s22_4_udp_send_recv() {
        let mut sock = UdpSocket::bind(SocketAddr::new("0.0.0.0", 9000));
        let dest = SocketAddr::new("10.0.0.1", 9001);
        sock.send_to(dest, b"ping");
        assert_eq!(sock.sent_datagrams().len(), 1);

        let src = SocketAddr::new("10.0.0.1", 9001);
        sock.inject_datagram(src, b"pong".to_vec());
        let (from, data) = sock.recv_from().unwrap();
        assert_eq!(from.port, 9001);
        assert_eq!(data, b"pong");
    }

    #[test]
    fn s22_5_socket_addr_parse() {
        let addr = SocketAddr::parse("192.168.1.1:3000").unwrap();
        assert_eq!(addr.ip, "192.168.1.1");
        assert_eq!(addr.port, 3000);
        assert_eq!(addr.to_string_repr(), "192.168.1.1:3000");
    }

    #[test]
    fn s22_6_socket_addr_parse_invalid() {
        let err = SocketAddr::parse("no-port").unwrap_err();
        assert!(matches!(err, IoError::HttpParseError { .. }));
    }

    #[test]
    fn s22_7_dns_resolve_known() {
        let addrs = dns_resolve("localhost").unwrap();
        assert_eq!(addrs, vec!["127.0.0.1"]);
        let addrs2 = dns_resolve("fajar-lang.dev").unwrap();
        assert_eq!(addrs2.len(), 2);
    }

    #[test]
    fn s22_8_dns_resolve_unknown() {
        let err = dns_resolve("nonexistent.invalid").unwrap_err();
        assert!(matches!(err, IoError::DnsResolutionFailed { .. }));
    }

    #[test]
    fn s22_9_socket_options_default() {
        let opts = SocketOptions::default();
        assert!(!opts.reuse_addr);
        assert!(!opts.nodelay);
        assert!(!opts.keepalive);
        assert_eq!(opts.timeout_ms, 0);
    }

    #[test]
    fn s22_10_connection_pool_get_release() {
        let mut pool = ConnectionPool::new(2, 30_000);
        let addr = SocketAddr::new("10.0.0.1", 80);
        let stream = pool.get(&addr).unwrap();
        assert_eq!(pool.active_count(), 1);
        pool.release(stream);
        // Getting again should reuse the pooled connection
        let _s2 = pool.get(&addr).unwrap();
        // Still count=1 because we reused
        assert_eq!(pool.active_count(), 1);
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 23 — HTTP Client/Server (s23_1 .. s23_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s23_1_http_method_parse() {
        assert_eq!(
            HttpMethod::from_str_checked("get").unwrap(),
            HttpMethod::GET
        );
        assert_eq!(
            HttpMethod::from_str_checked("POST").unwrap(),
            HttpMethod::POST
        );
        assert!(HttpMethod::from_str_checked("INVALID").is_err());
    }

    #[test]
    fn s23_2_http_request_parse() {
        let raw = "GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, HttpMethod::GET);
        assert_eq!(req.path, "/index.html");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.headers.get("Host").unwrap(), "example.com");
        assert!(req.body.is_none());
    }

    #[test]
    fn s23_3_http_request_parse_with_body() {
        let raw = "POST /api HTTP/1.1\r\nContent-Type: text/plain\r\n\r\nhello body";
        let req = parse_request(raw).unwrap();
        assert_eq!(req.method, HttpMethod::POST);
        assert_eq!(req.body, Some("hello body".into()));
    }

    #[test]
    fn s23_4_http_response_build() {
        let resp = HttpResponse::ok("Hello, Fajar!");
        let raw = build_response(&resp);
        assert!(raw.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(raw.contains("Content-Length: 13\r\n"));
        assert!(raw.ends_with("Hello, Fajar!"));
    }

    #[test]
    fn s23_5_http_client_get() {
        let client = HttpClient::new();
        let resp = client.get("http://example.com/api");
        assert_eq!(resp.status_code, 200);
        assert!(resp.body.contains("example.com/api"));
    }

    #[test]
    fn s23_6_http_client_post() {
        let client = HttpClient::new();
        let resp = client.post("http://example.com/data", "{\"key\":1}");
        assert_eq!(resp.status_code, 201);
        assert!(resp.body.contains("{\"key\":1}"));
    }

    #[test]
    fn s23_7_http_server_route_dispatch() {
        let addr = SocketAddr::new("0.0.0.0", 8080);
        let mut server = HttpServer::new(addr);
        server.add_route(Route {
            method: HttpMethod::GET,
            path: "/health".into(),
            handler_name: "health_check".into(),
        });
        let req = HttpRequest {
            method: HttpMethod::GET,
            path: "/health".into(),
            headers: HashMap::new(),
            body: None,
            version: "HTTP/1.1".into(),
        };
        let handler = server.dispatch(&req).unwrap();
        assert_eq!(handler, "health_check");
    }

    #[test]
    fn s23_8_http_server_route_not_found() {
        let addr = SocketAddr::new("0.0.0.0", 8080);
        let server = HttpServer::new(addr);
        let req = HttpRequest {
            method: HttpMethod::GET,
            path: "/missing".into(),
            headers: HashMap::new(),
            body: None,
            version: "HTTP/1.1".into(),
        };
        let err = server.dispatch(&req).unwrap_err();
        assert!(matches!(err, IoError::RouteNotFound { .. }));
    }

    #[test]
    fn s23_9_content_type_mime() {
        assert_eq!(ContentType::Json.mime(), "application/json");
        assert_eq!(ContentType::Html.mime(), "text/html");
        assert_eq!(
            ContentType::FormUrlencoded.mime(),
            "application/x-www-form-urlencoded"
        );
    }

    #[test]
    fn s23_10_cors_middleware() {
        let config = CorsConfig::permissive();
        assert!(config.is_origin_allowed("http://any.com"));
        let mw = CorsMiddleware {
            config: config.clone(),
        };
        let mut req = HttpRequest {
            method: HttpMethod::GET,
            path: "/api".into(),
            headers: HashMap::new(),
            body: None,
            version: "HTTP/1.1".into(),
        };
        let mut resp = HttpResponse::ok("ok");
        mw.process(&mut req, &mut resp);
        assert!(resp.headers.contains_key("Access-Control-Allow-Origin"));
        assert!(resp.headers.contains_key("Access-Control-Allow-Methods"));
    }

    // ───────────────────────────────────────────────────────────────
    // Sprint 24 — Protocol Integration (s24_1 .. s24_10)
    // ───────────────────────────────────────────────────────────────

    #[test]
    fn s24_1_ws_frame_encode_decode_unmasked() {
        let frame = WsFrame {
            opcode: WsOpcode::Text,
            payload: b"hello".to_vec(),
            masked: false,
        };
        let encoded = encode_ws_frame(&frame);
        let decoded = decode_ws_frame(&encoded).unwrap();
        assert_eq!(decoded.opcode, WsOpcode::Text);
        assert_eq!(decoded.payload, b"hello");
        assert!(!decoded.masked);
    }

    #[test]
    fn s24_2_ws_frame_encode_decode_masked() {
        let frame = WsFrame {
            opcode: WsOpcode::Binary,
            payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
            masked: true,
        };
        let encoded = encode_ws_frame(&frame);
        let decoded = decode_ws_frame(&encoded).unwrap();
        assert_eq!(decoded.opcode, WsOpcode::Binary);
        assert_eq!(decoded.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(decoded.masked);
    }

    #[test]
    fn s24_3_ws_upgrade_request() {
        let req = ws_upgrade_request("/ws", "dGVzdA==");
        assert_eq!(req.method, HttpMethod::GET);
        assert_eq!(req.path, "/ws");
        assert_eq!(req.headers.get("Upgrade").unwrap(), "websocket");
        assert_eq!(req.headers.get("Sec-WebSocket-Key").unwrap(), "dGVzdA==");
    }

    #[test]
    fn s24_4_grpc_unary_call() {
        let msg = GrpcMessage {
            service: "fj.inference".into(),
            method: "Predict".into(),
            payload_bytes: vec![1, 2, 3],
        };
        let resp = grpc_unary_call(&msg).unwrap();
        assert!(resp.starts_with(b"grpc-ok:"));
        assert_eq!(&resp[8..], &[1, 2, 3]);
    }

    #[test]
    fn s24_5_grpc_empty_service_error() {
        let msg = GrpcMessage {
            service: "".into(),
            method: "Call".into(),
            payload_bytes: vec![],
        };
        let err = grpc_unary_call(&msg).unwrap_err();
        assert!(matches!(
            err,
            IoError::GrpcError {
                status: GrpcStatus::InvalidArgument,
                ..
            }
        ));
    }

    #[test]
    fn s24_6_h2_frame_types() {
        let data = H2Frame::Data {
            stream_id: 1,
            payload: vec![],
            end_stream: true,
        };
        assert_eq!(data.frame_type(), 0x0);
        let headers = H2Frame::Headers {
            stream_id: 1,
            headers: vec![],
            end_stream: false,
        };
        assert_eq!(headers.frame_type(), 0x1);
        let ping = H2Frame::Ping { data: [0; 8] };
        assert_eq!(ping.frame_type(), 0x6);
    }

    #[test]
    fn s24_7_protocol_stack_url_selection() {
        assert_eq!(
            ProtocolStack::select_from_url("ws://localhost/chat"),
            Protocol::WebSocket
        );
        assert_eq!(
            ProtocolStack::select_from_url("grpc://service:50051"),
            Protocol::Grpc
        );
        assert_eq!(
            ProtocolStack::select_from_url("udp://0.0.0.0:9000"),
            Protocol::Udp
        );
        assert_eq!(
            ProtocolStack::select_from_url("http://example.com"),
            Protocol::Tcp
        );
    }

    #[test]
    fn s24_8_token_bucket_rate_limiter() {
        let mut bucket = TokenBucket::new(3, 1);
        assert!(allow_request(&mut bucket));
        assert!(allow_request(&mut bucket));
        assert!(allow_request(&mut bucket));
        assert!(!allow_request(&mut bucket)); // exhausted
        bucket.refill(2); // 2 seconds later: +2 tokens
        assert!(allow_request(&mut bucket));
        assert!(allow_request(&mut bucket));
        assert!(!allow_request(&mut bucket));
    }

    #[test]
    fn s24_9_circuit_breaker_transitions() {
        let mut cb = CircuitBreaker::new(3, 100);
        assert_eq!(cb.state, CircuitState::Closed);

        // Record failures until threshold
        cb.record_failure(1);
        cb.record_failure(2);
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure(3);
        assert_eq!(cb.state, CircuitState::Open);

        // Should reject while open
        assert!(cb.allow(50).is_err());

        // After reset_timeout, transitions to HalfOpen
        cb.allow(103).unwrap();
        assert_eq!(cb.state, CircuitState::HalfOpen);

        // Success resets to Closed
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.failure_count, 0);
    }

    #[test]
    fn s24_10_retry_policy_compute_delay() {
        let policy = RetryPolicy::new(5, 100, 2, 10);
        // attempt 0: 100 * 2^0 + jitter(0) = 100 + 3 = 103
        let d0 = compute_delay(0, &policy);
        assert_eq!(d0, 103);
        // attempt 1: 100 * 2^1 + jitter(1) = 200 + 10%1=0 => 200 + 0
        let d1 = compute_delay(1, &policy);
        assert_eq!(d1, 200 + (1 * 7 + 3) % 11);
        // attempt 2: 100 * 2^2 + jitter(2) = 400 + (2*7+3)%11 = 400 + 6
        let d2 = compute_delay(2, &policy);
        assert_eq!(d2, 406);
        // Each attempt should be larger than the last (exponential)
        assert!(d2 > d1);
        assert!(d1 > d0);
    }
}
