//! RPC Framework — service definition, code generation, serialization,
//! transport, discovery, load balancing, retry, streaming.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// S9.1: Service Definition
// ═══════════════════════════════════════════════════════════════════════

/// An RPC method definition within a service.
#[derive(Debug, Clone)]
pub struct RpcMethod {
    /// Method name.
    pub name: String,
    /// Parameter types.
    pub params: Vec<(String, String)>,
    /// Return type.
    pub return_type: String,
    /// Whether this method streams its response.
    pub streaming: StreamingMode,
}

/// Streaming mode for an RPC method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingMode {
    /// Unary: single request, single response.
    Unary,
    /// Server streaming: single request, stream of responses.
    ServerStream,
    /// Client streaming: stream of requests, single response.
    ClientStream,
    /// Bidirectional streaming.
    Bidirectional,
}

impl fmt::Display for StreamingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingMode::Unary => write!(f, "Unary"),
            StreamingMode::ServerStream => write!(f, "ServerStream"),
            StreamingMode::ClientStream => write!(f, "ClientStream"),
            StreamingMode::Bidirectional => write!(f, "Bidirectional"),
        }
    }
}

/// A service definition annotated with `@rpc`.
#[derive(Debug, Clone)]
pub struct ServiceDef {
    /// Service name.
    pub name: String,
    /// Methods in this service.
    pub methods: Vec<RpcMethod>,
    /// Service version.
    pub version: String,
}

impl ServiceDef {
    /// Creates a new service definition.
    pub fn new(name: &str, version: &str) -> Self {
        ServiceDef {
            name: name.to_string(),
            methods: Vec::new(),
            version: version.to_string(),
        }
    }

    /// Adds a unary method.
    pub fn add_method(&mut self, name: &str, params: Vec<(&str, &str)>, return_type: &str) {
        self.methods.push(RpcMethod {
            name: name.to_string(),
            params: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect(),
            return_type: return_type.to_string(),
            streaming: StreamingMode::Unary,
        });
    }

    /// Adds a streaming method.
    pub fn add_streaming_method(
        &mut self,
        name: &str,
        params: Vec<(&str, &str)>,
        return_type: &str,
        mode: StreamingMode,
    ) {
        self.methods.push(RpcMethod {
            name: name.to_string(),
            params: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect(),
            return_type: return_type.to_string(),
            streaming: mode,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.2: Code Generation
// ═══════════════════════════════════════════════════════════════════════

/// Generated client stub for an RPC service.
#[derive(Debug, Clone)]
pub struct ClientStub {
    /// Service name.
    pub service_name: String,
    /// Generated method signatures.
    pub methods: Vec<String>,
}

/// Generated server skeleton for an RPC service.
#[derive(Debug, Clone)]
pub struct ServerSkeleton {
    /// Service name.
    pub service_name: String,
    /// Handler trait methods.
    pub handlers: Vec<String>,
}

/// Generates a client stub from a service definition.
pub fn generate_client_stub(service: &ServiceDef) -> ClientStub {
    let methods = service
        .methods
        .iter()
        .map(|m| {
            let params: Vec<String> = m.params.iter().map(|(n, t)| format!("{n}: {t}")).collect();
            format!(
                "fn {}({}) -> Future<{}>",
                m.name,
                params.join(", "),
                m.return_type
            )
        })
        .collect();
    ClientStub {
        service_name: service.name.clone(),
        methods,
    }
}

/// Generates a server skeleton from a service definition.
pub fn generate_server_skeleton(service: &ServiceDef) -> ServerSkeleton {
    let handlers = service
        .methods
        .iter()
        .map(|m| {
            let params: Vec<String> = m.params.iter().map(|(n, t)| format!("{n}: {t}")).collect();
            format!(
                "fn handle_{}({}) -> {}",
                m.name,
                params.join(", "),
                m.return_type
            )
        })
        .collect();
    ServerSkeleton {
        service_name: service.name.clone(),
        handlers,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.3: Serialization
// ═══════════════════════════════════════════════════════════════════════

/// Wire format tag for serialized values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType {
    /// Variable-length integer.
    Varint,
    /// Fixed 64-bit value.
    Fixed64,
    /// Length-delimited bytes.
    LengthDelimited,
    /// Fixed 32-bit value.
    Fixed32,
}

impl fmt::Display for WireType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireType::Varint => write!(f, "Varint"),
            WireType::Fixed64 => write!(f, "Fixed64"),
            WireType::LengthDelimited => write!(f, "LengthDelimited"),
            WireType::Fixed32 => write!(f, "Fixed32"),
        }
    }
}

/// A serialized RPC message field.
#[derive(Debug, Clone)]
pub struct SerializedField {
    /// Field number.
    pub field_num: u32,
    /// Wire type.
    pub wire_type: WireType,
    /// Serialized bytes.
    pub data: Vec<u8>,
}

/// Serializes an i64 value as a varint.
pub fn serialize_varint(value: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut v = value as u64;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if v == 0 {
            break;
        }
    }
    bytes
}

/// Deserializes a varint from bytes.
pub fn deserialize_varint(bytes: &[u8]) -> Option<(i64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result as i64, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

/// Serializes a string as length-delimited bytes.
pub fn serialize_string(s: &str) -> Vec<u8> {
    let mut result = serialize_varint(s.len() as i64);
    result.extend_from_slice(s.as_bytes());
    result
}

// ═══════════════════════════════════════════════════════════════════════
// S9.4: Transport Layer
// ═══════════════════════════════════════════════════════════════════════

/// A network endpoint address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    /// Hostname or IP.
    pub host: String,
    /// Port number.
    pub port: u16,
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum connections per endpoint.
    pub max_connections: usize,
    /// Connection idle timeout.
    pub idle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        PoolConfig {
            max_connections: 10,
            idle_timeout: Duration::from_secs(60),
        }
    }
}

/// A simulated connection pool.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Pool configuration.
    pub config: PoolConfig,
    /// Active connections per endpoint.
    pub connections: HashMap<String, usize>,
}

impl ConnectionPool {
    /// Creates a new connection pool.
    pub fn new(config: PoolConfig) -> Self {
        ConnectionPool {
            config,
            connections: HashMap::new(),
        }
    }

    /// Acquires a connection to an endpoint.
    pub fn acquire(&mut self, endpoint: &Endpoint) -> Result<(), String> {
        let key = endpoint.to_string();
        let count = self.connections.entry(key.clone()).or_insert(0);
        if *count >= self.config.max_connections {
            return Err(format!(
                "connection pool exhausted for {key} (max: {})",
                self.config.max_connections
            ));
        }
        *count += 1;
        Ok(())
    }

    /// Releases a connection.
    pub fn release(&mut self, endpoint: &Endpoint) {
        let key = endpoint.to_string();
        if let Some(count) = self.connections.get_mut(&key) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }

    /// Returns the active connection count for an endpoint.
    pub fn active_count(&self, endpoint: &Endpoint) -> usize {
        self.connections
            .get(&endpoint.to_string())
            .copied()
            .unwrap_or(0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.5: Service Discovery
// ═══════════════════════════════════════════════════════════════════════

/// A service registry for name → endpoint resolution.
#[derive(Debug, Default)]
pub struct ServiceRegistry {
    /// Service name → list of endpoints.
    services: HashMap<String, Vec<Endpoint>>,
}

impl ServiceRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        ServiceRegistry::default()
    }

    /// Registers a service endpoint.
    pub fn register(&mut self, service_name: &str, endpoint: Endpoint) {
        self.services
            .entry(service_name.to_string())
            .or_default()
            .push(endpoint);
    }

    /// Resolves a service name to its endpoints.
    pub fn resolve(&self, service_name: &str) -> Option<&[Endpoint]> {
        self.services.get(service_name).map(|v| v.as_slice())
    }

    /// Deregisters a specific endpoint for a service.
    pub fn deregister(&mut self, service_name: &str, endpoint: &Endpoint) {
        if let Some(endpoints) = self.services.get_mut(service_name) {
            endpoints.retain(|e| e != endpoint);
        }
    }

    /// Returns all registered service names.
    pub fn service_names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.6: Load Balancing
// ═══════════════════════════════════════════════════════════════════════

/// Load balancing strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    /// Round-robin — cycle through endpoints in order.
    RoundRobin,
    /// Least connections — pick the endpoint with fewest active connections.
    LeastConnections,
    /// Weighted — pick based on configured weights.
    Weighted(Vec<u32>),
}

impl fmt::Display for LoadBalanceStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadBalanceStrategy::RoundRobin => write!(f, "RoundRobin"),
            LoadBalanceStrategy::LeastConnections => write!(f, "LeastConnections"),
            LoadBalanceStrategy::Weighted(_) => write!(f, "Weighted"),
        }
    }
}

/// A load balancer that selects endpoints.
#[derive(Debug)]
pub struct LoadBalancer {
    /// Strategy.
    pub strategy: LoadBalanceStrategy,
    /// Round-robin counter.
    rr_counter: usize,
}

impl LoadBalancer {
    /// Creates a new load balancer.
    pub fn new(strategy: LoadBalanceStrategy) -> Self {
        LoadBalancer {
            strategy,
            rr_counter: 0,
        }
    }

    /// Selects the next endpoint from a list.
    pub fn select<'a>(&mut self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        if endpoints.is_empty() {
            return None;
        }
        match &self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let idx = self.rr_counter % endpoints.len();
                self.rr_counter += 1;
                Some(&endpoints[idx])
            }
            LoadBalanceStrategy::LeastConnections => {
                // In simulation, just pick first (real impl would track connections)
                Some(&endpoints[0])
            }
            LoadBalanceStrategy::Weighted(weights) => {
                // Pick the endpoint with highest weight
                let max_idx = weights
                    .iter()
                    .enumerate()
                    .take(endpoints.len())
                    .max_by_key(|(_, w)| *w)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                Some(&endpoints[max_idx])
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S9.7: Retry Policy
// ═══════════════════════════════════════════════════════════════════════

/// Retry policy configuration.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial backoff duration.
    pub initial_backoff: Duration,
    /// Backoff multiplier.
    pub multiplier: f64,
    /// Maximum backoff duration.
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            multiplier: 2.0,
            max_backoff: Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Computes the backoff duration for a given attempt (0-indexed).
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let backoff_ms =
            self.initial_backoff.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let clamped = backoff_ms.min(self.max_backoff.as_millis() as f64);
        Duration::from_millis(clamped as u64)
    }

    /// Returns true if the attempt is within the retry limit.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Allowing requests.
    Closed,
    /// Blocking requests (too many failures).
    Open,
    /// Allowing a probe request to check recovery.
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::Open => write!(f, "Open"),
            CircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// A circuit breaker for RPC calls.
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Current state.
    pub state: CircuitState,
    /// Failure count.
    pub failure_count: u32,
    /// Failure threshold to open.
    pub threshold: u32,
    /// Success count in half-open.
    pub half_open_successes: u32,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with a failure threshold.
    pub fn new(threshold: u32) -> Self {
        CircuitBreaker {
            state: CircuitState::Closed,
            failure_count: 0,
            threshold,
            half_open_successes: 0,
        }
    }

    /// Records a successful call.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= 3 {
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.half_open_successes = 0;
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Records a failed call.
    pub fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.half_open_successes = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Attempts to allow a request through.
    pub fn allow_request(&self) -> bool {
        matches!(self.state, CircuitState::Closed | CircuitState::HalfOpen)
    }

    /// Transitions from Open to HalfOpen (called after timeout).
    pub fn try_half_open(&mut self) {
        if self.state == CircuitState::Open {
            self.state = CircuitState::HalfOpen;
            self.half_open_successes = 0;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S9.1 — Service Definition
    #[test]
    fn s9_1_service_definition() {
        let mut svc = ServiceDef::new("UserService", "1.0");
        svc.add_method("get_user", vec![("id", "u64")], "Result<User, Error>");
        assert_eq!(svc.name, "UserService");
        assert_eq!(svc.methods.len(), 1);
        assert_eq!(svc.methods[0].streaming, StreamingMode::Unary);
    }

    // S9.2 — Code Generation
    #[test]
    fn s9_2_client_stub_generation() {
        let mut svc = ServiceDef::new("UserService", "1.0");
        svc.add_method("get_user", vec![("id", "u64")], "Result<User, Error>");
        let stub = generate_client_stub(&svc);
        assert_eq!(stub.service_name, "UserService");
        assert!(stub.methods[0].contains("get_user"));
        assert!(stub.methods[0].contains("Future<"));
    }

    #[test]
    fn s9_2_server_skeleton_generation() {
        let mut svc = ServiceDef::new("OrderService", "2.0");
        svc.add_method("create_order", vec![("item", "String")], "OrderId");
        let skel = generate_server_skeleton(&svc);
        assert!(skel.handlers[0].contains("handle_create_order"));
    }

    // S9.3 — Serialization
    #[test]
    fn s9_3_varint_roundtrip() {
        for &val in &[0i64, 1, 127, 128, 300, 10000, 1_000_000] {
            let encoded = serialize_varint(val);
            let (decoded, _len) = deserialize_varint(&encoded).unwrap();
            assert_eq!(decoded, val);
        }
    }

    #[test]
    fn s9_3_string_serialization() {
        let encoded = serialize_string("hello");
        assert_eq!(encoded[0], 5); // length
        assert_eq!(&encoded[1..], b"hello");
    }

    #[test]
    fn s9_3_wire_type_display() {
        assert_eq!(WireType::Varint.to_string(), "Varint");
        assert_eq!(WireType::LengthDelimited.to_string(), "LengthDelimited");
    }

    // S9.4 — Transport Layer
    #[test]
    fn s9_4_connection_pool() {
        let config = PoolConfig {
            max_connections: 2,
            idle_timeout: Duration::from_secs(30),
        };
        let mut pool = ConnectionPool::new(config);
        let ep = Endpoint {
            host: "localhost".into(),
            port: 8080,
        };

        pool.acquire(&ep).unwrap();
        pool.acquire(&ep).unwrap();
        assert!(pool.acquire(&ep).is_err()); // Pool exhausted
        assert_eq!(pool.active_count(&ep), 2);

        pool.release(&ep);
        assert_eq!(pool.active_count(&ep), 1);
    }

    // S9.5 — Service Discovery
    #[test]
    fn s9_5_service_registry() {
        let mut registry = ServiceRegistry::new();
        registry.register(
            "user-svc",
            Endpoint {
                host: "10.0.0.1".into(),
                port: 8080,
            },
        );
        registry.register(
            "user-svc",
            Endpoint {
                host: "10.0.0.2".into(),
                port: 8080,
            },
        );

        let endpoints = registry.resolve("user-svc").unwrap();
        assert_eq!(endpoints.len(), 2);
        assert!(registry.resolve("unknown").is_none());
    }

    #[test]
    fn s9_5_deregister() {
        let mut registry = ServiceRegistry::new();
        let ep = Endpoint {
            host: "10.0.0.1".into(),
            port: 8080,
        };
        registry.register("svc", ep.clone());
        registry.deregister("svc", &ep);
        assert_eq!(registry.resolve("svc").unwrap().len(), 0);
    }

    // S9.6 — Load Balancing
    #[test]
    fn s9_6_round_robin() {
        let endpoints = vec![
            Endpoint {
                host: "a".into(),
                port: 80,
            },
            Endpoint {
                host: "b".into(),
                port: 80,
            },
            Endpoint {
                host: "c".into(),
                port: 80,
            },
        ];
        let mut lb = LoadBalancer::new(LoadBalanceStrategy::RoundRobin);
        assert_eq!(lb.select(&endpoints).unwrap().host, "a");
        assert_eq!(lb.select(&endpoints).unwrap().host, "b");
        assert_eq!(lb.select(&endpoints).unwrap().host, "c");
        assert_eq!(lb.select(&endpoints).unwrap().host, "a"); // wraps
    }

    #[test]
    fn s9_6_weighted() {
        let endpoints = vec![
            Endpoint {
                host: "a".into(),
                port: 80,
            },
            Endpoint {
                host: "b".into(),
                port: 80,
            },
        ];
        let mut lb = LoadBalancer::new(LoadBalanceStrategy::Weighted(vec![1, 5]));
        assert_eq!(lb.select(&endpoints).unwrap().host, "b"); // highest weight
    }

    // S9.7 — Retry Policy
    #[test]
    fn s9_7_retry_backoff() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));

        let b0 = policy.backoff_for(0);
        let b1 = policy.backoff_for(1);
        assert!(b1 > b0);
    }

    #[test]
    fn s9_7_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3);
        assert!(cb.allow_request());

        cb.record_failure();
        cb.record_failure();
        assert!(cb.allow_request()); // Still closed
        cb.record_failure();
        assert!(!cb.allow_request()); // Open

        cb.try_half_open();
        assert!(cb.allow_request()); // Half-open
        cb.record_success();
        cb.record_success();
        cb.record_success();
        assert_eq!(cb.state, CircuitState::Closed);
    }

    // S9.8-S9.9 — Streaming
    #[test]
    fn s9_8_streaming_method() {
        let mut svc = ServiceDef::new("DataService", "1.0");
        svc.add_streaming_method(
            "stream_data",
            vec![("query", "Query")],
            "Stream<DataChunk>",
            StreamingMode::ServerStream,
        );
        assert_eq!(svc.methods[0].streaming, StreamingMode::ServerStream);
    }

    #[test]
    fn s9_9_bidirectional_streaming() {
        let mut svc = ServiceDef::new("ChatService", "1.0");
        svc.add_streaming_method(
            "chat",
            vec![("stream", "Stream<Msg>")],
            "Stream<Msg>",
            StreamingMode::Bidirectional,
        );
        assert_eq!(svc.methods[0].streaming, StreamingMode::Bidirectional);
    }

    // S9.10 — Integration
    #[test]
    fn s9_10_endpoint_display() {
        let ep = Endpoint {
            host: "localhost".into(),
            port: 443,
        };
        assert_eq!(ep.to_string(), "localhost:443");
    }

    #[test]
    fn s9_10_streaming_mode_display() {
        assert_eq!(StreamingMode::Unary.to_string(), "Unary");
        assert_eq!(StreamingMode::Bidirectional.to_string(), "Bidirectional");
    }
}
