//! Networking — TCP/UDP, HTTP client/server, WebSocket, DNS, TLS.
//!
//! Phase S1 + V8 GC1: Real networking implementations via std::net.
//! TCP client/server, UDP, HTTP client/server, DNS resolution.

use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};

// ═══════════════════════════════════════════════════════════════════════
// S1.1-S1.2: TCP Client/Server + UDP
// ═══════════════════════════════════════════════════════════════════════

/// TCP socket address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    /// IP address (v4 or v6).
    pub ip: IpAddr,
    /// Port number.
    pub port: u16,
}

impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.ip {
            IpAddr::V4(a, b, c, d) => write!(f, "{a}.{b}.{c}.{d}:{}", self.port),
            IpAddr::V6(segments) => {
                write!(f, "[")?;
                for (i, s) in segments.iter().enumerate() {
                    if i > 0 {
                        write!(f, ":")?;
                    }
                    write!(f, "{s:04x}")?;
                }
                write!(f, "]:{}", self.port)
            }
        }
    }
}

/// IP address (v4 or v6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IpAddr {
    V4(u8, u8, u8, u8),
    V6([u16; 8]),
}

/// TCP connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    TimeWait,
}

/// TCP connection configuration.
#[derive(Debug, Clone)]
pub struct TcpConfig {
    /// Read timeout in milliseconds (0 = no timeout).
    pub read_timeout_ms: u64,
    /// Write timeout in milliseconds.
    pub write_timeout_ms: u64,
    /// TCP_NODELAY (disable Nagle's algorithm).
    pub no_delay: bool,
    /// Keep-alive interval in seconds (0 = disabled).
    pub keep_alive_secs: u32,
    /// Receive buffer size.
    pub recv_buf_size: usize,
    /// Send buffer size.
    pub send_buf_size: usize,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            read_timeout_ms: 30_000,
            write_timeout_ms: 30_000,
            no_delay: true,
            keep_alive_secs: 60,
            recv_buf_size: 65536,
            send_buf_size: 65536,
        }
    }
}

/// UDP datagram configuration.
#[derive(Debug, Clone)]
pub struct UdpConfig {
    /// Receive buffer size.
    pub recv_buf_size: usize,
    /// Broadcast enabled.
    pub broadcast: bool,
    /// Multicast TTL.
    pub multicast_ttl: u8,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            recv_buf_size: 65536,
            broadcast: false,
            multicast_ttl: 1,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.3-S1.5: HTTP Client
// ═══════════════════════════════════════════════════════════════════════

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
            Self::Patch => write!(f, "PATCH"),
            Self::Head => write!(f, "HEAD"),
            Self::Options => write!(f, "OPTIONS"),
        }
    }
}

/// HTTP request builder.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Request URL.
    pub url: String,
    /// Headers.
    pub headers: HashMap<String, String>,
    /// Request body (for POST/PUT/PATCH).
    pub body: Option<Vec<u8>>,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Follow redirects.
    pub follow_redirects: bool,
    /// Maximum redirects.
    pub max_redirects: u32,
}

impl HttpRequest {
    /// Creates a GET request.
    pub fn get(url: &str) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.to_string(),
            headers: default_headers(),
            body: None,
            timeout_ms: 30_000,
            follow_redirects: true,
            max_redirects: 10,
        }
    }

    /// Creates a POST request with JSON body.
    pub fn post_json(url: &str, json: &str) -> Self {
        let mut headers = default_headers();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            method: HttpMethod::Post,
            url: url.to_string(),
            headers,
            body: Some(json.as_bytes().to_vec()),
            timeout_ms: 30_000,
            follow_redirects: true,
            max_redirects: 10,
        }
    }

    /// Adds a header.
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Sets timeout.
    pub fn timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }
}

fn default_headers() -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert("User-Agent".to_string(), "FajarLang/5.5.0".to_string());
    h.insert("Accept".to_string(), "*/*".to_string());
    h
}

/// HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Status text.
    pub status_text: String,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Response body.
    pub body: Vec<u8>,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: f64,
}

impl HttpResponse {
    /// Returns body as UTF-8 string.
    pub fn text(&self) -> Result<String, String> {
        String::from_utf8(self.body.clone()).map_err(|e| format!("invalid UTF-8: {e}"))
    }

    /// Returns true if status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Returns content-type header.
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("content-type").map(|s| s.as_str())
    }

    /// Returns content-length.
    pub fn content_length(&self) -> Option<usize> {
        self.headers
            .get("content-length")
            .and_then(|s| s.parse().ok())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.6-S1.8: HTTP Server with Routing
// ═══════════════════════════════════════════════════════════════════════

/// HTTP route handler.
#[derive(Debug, Clone)]
pub struct Route {
    /// HTTP method.
    pub method: HttpMethod,
    /// URL pattern (e.g., "/api/users/:id").
    pub pattern: String,
    /// Handler function name.
    pub handler: String,
}

/// HTTP server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Listen address.
    pub addr: SocketAddr,
    /// Maximum concurrent connections.
    pub max_connections: usize,
    /// Request body size limit.
    pub body_limit: usize,
    /// Keep-alive timeout.
    pub keep_alive_ms: u64,
    /// Routes.
    pub routes: Vec<Route>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr {
                ip: IpAddr::V4(0, 0, 0, 0),
                port: 8080,
            },
            max_connections: 1024,
            body_limit: 10 * 1024 * 1024, // 10MB
            keep_alive_ms: 60_000,
            routes: Vec::new(),
        }
    }
}

impl ServerConfig {
    /// Adds a route.
    pub fn route(mut self, method: HttpMethod, pattern: &str, handler: &str) -> Self {
        self.routes.push(Route {
            method,
            pattern: pattern.to_string(),
            handler: handler.to_string(),
        });
        self
    }
}

/// Matches a URL against a route pattern.
pub fn match_route(pattern: &str, url: &str) -> Option<HashMap<String, String>> {
    let pat_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let url_parts: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();

    if pat_parts.len() != url_parts.len() {
        return None;
    }

    let mut params = HashMap::new();
    for (p, u) in pat_parts.iter().zip(url_parts.iter()) {
        if let Some(stripped) = p.strip_prefix(':') {
            params.insert(stripped.to_string(), u.to_string());
        } else if p != u {
            return None;
        }
    }
    Some(params)
}

// ═══════════════════════════════════════════════════════════════════════
// S1.9-S1.10: WebSocket
// ═══════════════════════════════════════════════════════════════════════

/// WebSocket frame opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsOpcode {
    Text,
    Binary,
    Close,
    Ping,
    Pong,
    Continuation,
}

/// WebSocket message.
#[derive(Debug, Clone)]
pub struct WsMessage {
    /// Opcode.
    pub opcode: WsOpcode,
    /// Payload.
    pub payload: Vec<u8>,
}

impl WsMessage {
    /// Creates a text message.
    pub fn text(s: &str) -> Self {
        Self {
            opcode: WsOpcode::Text,
            payload: s.as_bytes().to_vec(),
        }
    }

    /// Creates a binary message.
    pub fn binary(data: Vec<u8>) -> Self {
        Self {
            opcode: WsOpcode::Binary,
            payload: data,
        }
    }

    /// Creates a close message.
    pub fn close() -> Self {
        Self {
            opcode: WsOpcode::Close,
            payload: Vec::new(),
        }
    }

    /// Returns payload as string (for text frames).
    pub fn as_text(&self) -> Result<String, String> {
        String::from_utf8(self.payload.clone()).map_err(|e| format!("{e}"))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S1.11-S1.14: DNS + TLS + URL Parsing
// ═══════════════════════════════════════════════════════════════════════

/// DNS record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
    NS,
    SOA,
    SRV,
}

/// A parsed URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    /// Scheme (http, https, ws, wss).
    pub scheme: String,
    /// Host.
    pub host: String,
    /// Port (default: 80/443).
    pub port: u16,
    /// Path.
    pub path: String,
    /// Query string.
    pub query: Option<String>,
    /// Fragment.
    pub fragment: Option<String>,
    /// Username (for auth URLs).
    pub username: Option<String>,
    /// Password.
    pub password: Option<String>,
}

impl Url {
    /// Parses a URL string.
    pub fn parse(url: &str) -> Result<Self, String> {
        // Simple parser: scheme://[user:pass@]host[:port]/path[?query][#fragment]
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| "missing scheme".to_string())?;

        let (auth_host, path_rest) = rest.split_once('/').unwrap_or((rest, ""));

        // Parse auth
        let (username, password, host_port) = if let Some((auth, hp)) = auth_host.split_once('@') {
            let (u, p) = auth.split_once(':').unwrap_or((auth, ""));
            (Some(u.to_string()), Some(p.to_string()), hp)
        } else {
            (None, None, auth_host)
        };

        // Parse host:port
        let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
            let port = p.parse::<u16>().map_err(|_| "invalid port")?;
            (h.to_string(), port)
        } else {
            let default_port = match scheme {
                "https" | "wss" => 443,
                _ => 80,
            };
            (host_port.to_string(), default_port)
        };

        // Parse path?query#fragment
        let full_path = format!("/{path_rest}");
        let (path, fragment) = if let Some((p, f)) = full_path.split_once('#') {
            (p.to_string(), Some(f.to_string()))
        } else {
            (full_path, None)
        };

        let (path, query) = if let Some((p, q)) = path.split_once('?') {
            (p.to_string(), Some(q.to_string()))
        } else {
            (path, None)
        };

        Ok(Url {
            scheme: scheme.to_string(),
            host,
            port,
            path,
            query,
            fragment,
            username,
            password,
        })
    }

    /// Returns the full URL as string.
    pub fn to_string_url(&self) -> String {
        let mut url = format!("{}://{}", self.scheme, self.host);
        let default_port = if self.scheme == "https" || self.scheme == "wss" {
            443
        } else {
            80
        };
        if self.port != default_port {
            url.push_str(&format!(":{}", self.port));
        }
        url.push_str(&self.path);
        if let Some(ref q) = self.query {
            url.push('?');
            url.push_str(q);
        }
        if let Some(ref f) = self.fragment {
            url.push('#');
            url.push_str(f);
        }
        url
    }
}

/// Parse query string into key-value pairs.
pub fn parse_query_string(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            params.insert(k.to_string(), v.to_string());
        }
    }
    params
}

// ═══════════════════════════════════════════════════════════════════════
// S1.15-S1.18: Connection Pool + Rate Limiter
// ═══════════════════════════════════════════════════════════════════════

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum connections per host.
    pub max_per_host: usize,
    /// Total maximum connections.
    pub max_total: usize,
    /// Idle timeout in seconds.
    pub idle_timeout_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_host: 10,
            max_total: 100,
            idle_timeout_secs: 300,
        }
    }
}

/// Token bucket rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Maximum tokens (burst capacity).
    pub max_tokens: u64,
    /// Current tokens.
    pub tokens: u64,
    /// Refill rate (tokens per second).
    pub refill_rate: u64,
    /// Last refill timestamp (ms).
    pub last_refill_ms: u64,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    pub fn new(max_tokens: u64, refill_rate: u64) -> Self {
        Self {
            max_tokens,
            tokens: max_tokens,
            refill_rate,
            last_refill_ms: 0,
        }
    }

    /// Attempts to consume one token. Returns true if allowed.
    pub fn try_acquire(&mut self, now_ms: u64) -> bool {
        // Refill tokens
        let elapsed = now_ms.saturating_sub(self.last_refill_ms);
        let refill = elapsed * self.refill_rate / 1000;
        if refill > 0 {
            self.tokens = (self.tokens + refill).min(self.max_tokens);
            self.last_refill_ms = now_ms;
        }
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for fault tolerance.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state.
    pub state: CircuitState,
    /// Failure count.
    pub failure_count: u32,
    /// Failure threshold to open.
    pub threshold: u32,
    /// Time to wait before half-open (ms).
    pub reset_timeout_ms: u64,
    /// Timestamp when opened (ms).
    pub opened_at_ms: u64,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker.
    pub fn new(threshold: u32, reset_timeout_ms: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            threshold,
            reset_timeout_ms,
            opened_at_ms: 0,
        }
    }

    /// Records a success.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
    }

    /// Records a failure. Returns true if circuit just opened.
    pub fn record_failure(&mut self, now_ms: u64) -> bool {
        self.failure_count += 1;
        if self.failure_count >= self.threshold {
            self.state = CircuitState::Open;
            self.opened_at_ms = now_ms;
            return true;
        }
        false
    }

    /// Checks if the request should be allowed.
    pub fn allow_request(&mut self, now_ms: u64) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if now_ms - self.opened_at_ms >= self.reset_timeout_ms {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.11: Real TCP Client
// ═══════════════════════════════════════════════════════════════════════

/// Connect to a TCP server and exchange data.
pub fn tcp_connect(addr: &str, data: &[u8], timeout_ms: u64) -> Result<Vec<u8>, String> {
    use std::net::TcpStream;
    use std::time::Duration;

    let timeout = Duration::from_millis(timeout_ms);
    let stream = TcpStream::connect(addr).map_err(|e| format!("TCP connect failed: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("set timeout: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("set timeout: {e}"))?;

    let mut stream = stream;
    stream
        .write_all(data)
        .map_err(|e| format!("TCP write: {e}"))?;
    stream.flush().map_err(|e| format!("TCP flush: {e}"))?;

    let mut response = Vec::new();
    // Read with timeout — will return when data available or timeout
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(n) => response.extend_from_slice(&buf[..n]),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
        Err(e) => return Err(format!("TCP read: {e}")),
    }
    Ok(response)
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.12: Real TCP Server
// ═══════════════════════════════════════════════════════════════════════

/// A simple TCP server that accepts connections and calls a handler.
pub struct TcpServer {
    listener: std::net::TcpListener,
}

impl TcpServer {
    /// Bind to an address. Use "127.0.0.1:0" for OS-assigned port.
    pub fn bind(addr: &str) -> Result<Self, String> {
        let listener = std::net::TcpListener::bind(addr).map_err(|e| format!("TCP bind: {e}"))?;
        Ok(Self { listener })
    }

    /// Returns the local address (useful when bound to port 0).
    pub fn local_addr(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|a| a.to_string())
            .map_err(|e| format!("{e}"))
    }

    /// Accept one connection, read request, call handler, send response.
    pub fn accept_one<F>(&self, handler: F) -> Result<(), String>
    where
        F: FnOnce(&[u8]) -> Vec<u8>,
    {
        let (mut stream, _addr) = self
            .listener
            .accept()
            .map_err(|e| format!("TCP accept: {e}"))?;

        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        let response = handler(&buf[..n]);
        stream
            .write_all(&response)
            .map_err(|e| format!("write: {e}"))?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.13: Real UDP
// ═══════════════════════════════════════════════════════════════════════

/// Send a UDP datagram and optionally receive a response.
pub fn udp_send_recv(
    bind_addr: &str,
    target_addr: &str,
    data: &[u8],
    timeout_ms: u64,
) -> Result<Vec<u8>, String> {
    use std::net::UdpSocket;
    use std::time::Duration;

    let socket = UdpSocket::bind(bind_addr).map_err(|e| format!("UDP bind: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .map_err(|e| format!("set timeout: {e}"))?;

    socket
        .send_to(data, target_addr)
        .map_err(|e| format!("UDP send: {e}"))?;

    let mut buf = [0u8; 65535];
    match socket.recv_from(&mut buf) {
        Ok((n, _addr)) => Ok(buf[..n].to_vec()),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => Ok(Vec::new()),
        Err(e) => Err(format!("UDP recv: {e}")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.14: Real HTTP Client
// ═══════════════════════════════════════════════════════════════════════

/// Perform a real HTTP request using std::net::TcpStream.
/// Supports HTTP/1.1 GET and POST.
pub fn http_request(req: &HttpRequest) -> Result<HttpResponse, String> {
    let url = Url::parse(&req.url)?;
    let host = &url.host;
    let port = url.port;
    let addr = format!("{host}:{port}");

    let start = std::time::Instant::now();

    let mut stream =
        std::net::TcpStream::connect(&addr).map_err(|e| format!("HTTP connect to {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(req.timeout_ms)))
        .map_err(|e| format!("set timeout: {e}"))?;

    // Build HTTP/1.1 request
    let method = format!("{}", req.method);
    let path = if url.path.is_empty() { "/" } else { &url.path };
    let mut request_str = format!("{method} {path}");
    if let Some(q) = &url.query {
        request_str.push('?');
        request_str.push_str(q);
    }
    request_str.push_str(&format!(" HTTP/1.1\r\nHost: {host}\r\n"));

    for (k, v) in &req.headers {
        request_str.push_str(&format!("{k}: {v}\r\n"));
    }

    if let Some(ref body) = req.body {
        if !body.is_empty() {
            request_str.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
    }

    request_str.push_str("Connection: close\r\n\r\n");

    stream
        .write_all(request_str.as_bytes())
        .map_err(|e| format!("HTTP write: {e}"))?;
    if let Some(ref body) = req.body {
        if !body.is_empty() {
            stream
                .write_all(body)
                .map_err(|e| format!("HTTP write body: {e}"))?;
        }
    }
    stream.flush().map_err(|e| format!("HTTP flush: {e}"))?;

    // Read response
    let reader = BufReader::new(&stream);
    let mut lines = reader.lines();

    // Parse status line
    let status_line = lines
        .next()
        .ok_or("empty response")?
        .map_err(|e| format!("read status: {e}"))?;
    let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err(format!("invalid status line: {status_line}"));
    }
    let status: u16 = parts[1].parse().unwrap_or(0);
    let status_text = parts.get(2).unwrap_or(&"").to_string();

    // Parse headers
    let mut headers = HashMap::new();
    let mut content_length: usize = 0;
    for line_result in lines.by_ref() {
        let line = line_result.map_err(|e| format!("read header: {e}"))?;
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(": ") {
            let key = k.to_lowercase();
            if key == "content-length" {
                content_length = v.trim().parse().unwrap_or(0);
            }
            headers.insert(key, v.trim().to_string());
        }
    }

    // Read body
    let mut body = Vec::new();
    if content_length > 0 {
        body.resize(content_length, 0);
        stream
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {e}"))?;
    } else {
        // Read until EOF for chunked/unknown length
        let _ = stream.read_to_end(&mut body);
    }

    let elapsed = start.elapsed();

    Ok(HttpResponse {
        status,
        status_text,
        headers,
        body,
        elapsed_ms: elapsed.as_secs_f64() * 1000.0,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.15: Real HTTP Server
// ═══════════════════════════════════════════════════════════════════════

/// A minimal HTTP server that serves requests.
pub struct HttpServer {
    listener: std::net::TcpListener,
}

impl HttpServer {
    /// Bind to an address.
    pub fn bind(addr: &str) -> Result<Self, String> {
        let listener = std::net::TcpListener::bind(addr).map_err(|e| format!("HTTP bind: {e}"))?;
        Ok(Self { listener })
    }

    /// Returns the local address.
    pub fn local_addr(&self) -> Result<String, String> {
        self.listener
            .local_addr()
            .map(|a| a.to_string())
            .map_err(|e| format!("{e}"))
    }

    /// Accept one HTTP request and send a response.
    pub fn accept_one<F>(&self, handler: F) -> Result<(), String>
    where
        F: FnOnce(&str, &str, &HashMap<String, String>) -> (u16, String, Vec<u8>),
    {
        let (stream, _addr) = self.listener.accept().map_err(|e| format!("accept: {e}"))?;
        let mut reader = BufReader::new(&stream);

        // Read request line
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .map_err(|e| format!("read: {e}"))?;
        let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
        let method = parts.first().unwrap_or(&"GET");
        let path = parts.get(1).unwrap_or(&"/");

        // Read headers
        let mut headers = HashMap::new();
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|e| format!("read header: {e}"))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some((k, v)) = trimmed.split_once(": ") {
                headers.insert(k.to_lowercase(), v.to_string());
            }
        }

        let (status, status_text, body) = handler(method, path, &headers);

        // Send response
        let mut stream = stream;
        let response = format!(
            "HTTP/1.1 {status} {status_text}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        stream
            .write_all(&body)
            .map_err(|e| format!("write body: {e}"))?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V8 GC1.16: Real DNS Resolver
// ═══════════════════════════════════════════════════════════════════════

/// Resolve a hostname to IP addresses using the system resolver.
pub fn dns_resolve(hostname: &str) -> Result<Vec<String>, String> {
    use std::net::ToSocketAddrs;

    let addr_str = format!("{hostname}:0");
    let addrs = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolve '{hostname}': {e}"))?;
    Ok(addrs.map(|a| a.ip().to_string()).collect())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s1_1_socket_addr_display() {
        let addr = SocketAddr {
            ip: IpAddr::V4(127, 0, 0, 1),
            port: 8080,
        };
        assert_eq!(format!("{addr}"), "127.0.0.1:8080");
    }

    #[test]
    fn s1_2_tcp_config_default() {
        let cfg = TcpConfig::default();
        assert!(cfg.no_delay);
        assert_eq!(cfg.keep_alive_secs, 60);
    }

    #[test]
    fn s1_3_http_request_builder() {
        let req = HttpRequest::get("https://api.example.com/users")
            .header("Authorization", "Bearer token123")
            .timeout(5000);
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.timeout_ms, 5000);
        assert!(req.headers.contains_key("Authorization"));
    }

    #[test]
    fn s1_4_http_response() {
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::from([("content-type".to_string(), "application/json".to_string())]),
            body: b"hello".to_vec(),
            elapsed_ms: 42.5,
        };
        assert!(resp.is_success());
        assert_eq!(resp.text().unwrap(), "hello");
        assert_eq!(resp.content_type(), Some("application/json"));
    }

    #[test]
    fn s1_5_http_method_display() {
        assert_eq!(format!("{}", HttpMethod::Post), "POST");
        assert_eq!(format!("{}", HttpMethod::Delete), "DELETE");
    }

    #[test]
    fn s1_6_route_matching() {
        let params = match_route("/api/users/:id", "/api/users/42").unwrap();
        assert_eq!(params.get("id").unwrap(), "42");
        assert!(match_route("/api/users/:id", "/api/posts/1").is_none());
        assert!(match_route("/api/users", "/api/users").is_some());
    }

    #[test]
    fn s1_7_server_config() {
        let cfg = ServerConfig::default()
            .route(HttpMethod::Get, "/", "index")
            .route(HttpMethod::Post, "/api/data", "create_data");
        assert_eq!(cfg.routes.len(), 2);
    }

    #[test]
    fn s1_9_ws_message() {
        let msg = WsMessage::text("hello");
        assert_eq!(msg.opcode, WsOpcode::Text);
        assert_eq!(msg.as_text().unwrap(), "hello");
    }

    #[test]
    fn s1_11_url_parse() {
        let url = Url::parse("https://api.example.com:8443/v1/users?page=1#top").unwrap();
        assert_eq!(url.scheme, "https");
        assert_eq!(url.host, "api.example.com");
        assert_eq!(url.port, 8443);
        assert_eq!(url.path, "/v1/users");
        assert_eq!(url.query.as_deref(), Some("page=1"));
        assert_eq!(url.fragment.as_deref(), Some("top"));
    }

    #[test]
    fn s1_12_url_default_port() {
        let url = Url::parse("http://example.com/path").unwrap();
        assert_eq!(url.port, 80);
        let url2 = Url::parse("https://example.com/path").unwrap();
        assert_eq!(url2.port, 443);
    }

    #[test]
    fn s1_13_query_string() {
        let params = parse_query_string("name=fajar&lang=fj&version=5");
        assert_eq!(params.get("name").unwrap(), "fajar");
        assert_eq!(params.get("lang").unwrap(), "fj");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn s1_15_rate_limiter() {
        let mut rl = RateLimiter::new(3, 1);
        rl.last_refill_ms = 0;
        assert!(rl.try_acquire(0));
        assert!(rl.try_acquire(0));
        assert!(rl.try_acquire(0));
        assert!(!rl.try_acquire(0)); // exhausted
        assert!(rl.try_acquire(2000)); // refilled after 2s
    }

    #[test]
    fn s1_16_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3, 5000);
        assert!(cb.allow_request(0));
        cb.record_failure(1000);
        cb.record_failure(2000);
        assert!(cb.record_failure(3000)); // 3rd failure = threshold → opens circuit
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.allow_request(3500)); // still open
        assert!(cb.allow_request(8001)); // half-open after 5s
    }

    // ═══════════════════════════════════════════════════════════════════
    // V8 GC1.19: Real networking integration tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn gc1_tcp_echo_server() {
        // Start TCP server on random port
        let server = TcpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();

        // Spawn server in background thread
        let handle = std::thread::spawn(move || {
            server.accept_one(|data| {
                let mut response = b"ECHO:".to_vec();
                response.extend_from_slice(data);
                response
            })
        });

        // Client connects and sends data
        let response = tcp_connect(&addr, b"hello", 5000).unwrap();
        assert_eq!(response, b"ECHO:hello");

        handle.join().unwrap().unwrap();
    }

    #[test]
    fn gc1_tcp_large_payload() {
        let server = TcpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            server.accept_one(|data| format!("got {} bytes", data.len()).into_bytes())
        });

        let payload = vec![0xABu8; 1024];
        let response = tcp_connect(&addr, &payload, 5000).unwrap();
        assert_eq!(response, b"got 1024 bytes");

        handle.join().unwrap().unwrap();
    }

    #[test]
    fn gc1_udp_roundtrip() {
        // Server socket
        let server_sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let server_addr = server_sock.local_addr().unwrap().to_string();

        // Server echoes in background
        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let (n, src) = server_sock.recv_from(&mut buf).unwrap();
            server_sock.send_to(&buf[..n], src).unwrap();
        });

        // Client sends and receives
        let response = udp_send_recv("127.0.0.1:0", &server_addr, b"ping", 5000).unwrap();
        assert_eq!(response, b"ping");

        handle.join().unwrap();
    }

    #[test]
    fn gc1_http_server_client() {
        let server = HttpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            server.accept_one(|method, path, _headers| {
                let body = format!("{{\"method\":\"{method}\",\"path\":\"{path}\"}}");
                (200, "OK".to_string(), body.into_bytes())
            })
        });

        // Real HTTP request
        let req = HttpRequest::get(&format!("http://{addr}/api/test"));
        let resp = http_request(&req).unwrap();
        assert_eq!(resp.status, 200);
        let body = resp.text().unwrap();
        assert!(body.contains("\"method\":\"GET\""));
        assert!(body.contains("\"path\":\"/api/test\""));

        handle.join().unwrap().unwrap();
    }

    #[test]
    fn gc1_http_post() {
        let server = HttpServer::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            server.accept_one(|method, _path, _headers| {
                let body = format!("method={method}");
                (201, "Created".to_string(), body.into_bytes())
            })
        });

        let req =
            HttpRequest::post_json(&format!("http://{addr}/api/items"), "{\"name\":\"fajar\"}");
        let resp = http_request(&req).unwrap();
        assert_eq!(resp.status, 201);

        handle.join().unwrap().unwrap();
    }

    #[test]
    fn gc1_dns_resolve_localhost() {
        let addrs = dns_resolve("localhost").unwrap();
        assert!(!addrs.is_empty());
        // localhost should resolve to 127.0.0.1 or ::1
        assert!(
            addrs.iter().any(|a| a == "127.0.0.1" || a == "::1"),
            "localhost should resolve to 127.0.0.1 or ::1, got: {addrs:?}"
        );
    }

    #[test]
    fn gc1_dns_resolve_invalid() {
        let result = dns_resolve("this.host.definitely.does.not.exist.invalid");
        assert!(result.is_err());
    }

    #[test]
    fn gc1_tcp_connect_refused() {
        // Try to connect to a port that's definitely not listening
        let result = tcp_connect("127.0.0.1:1", b"test", 1000);
        assert!(result.is_err());
    }
}
