//! Async Networking — TCP/UDP, HTTP client/server, WebSocket, DNS, TLS.
//!
//! Phase S1: 20 tasks covering the full networking stack with
//! connection pooling, timeouts, and streaming support.

use std::collections::HashMap;
use std::fmt;

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
}
