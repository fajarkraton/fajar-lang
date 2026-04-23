//! RPC Framework V2 — Sprint D6: bidirectional streaming, deadline propagation,
//! compression, authentication, interceptors, client-side load balancing,
//! reflection, health service, and metrics collection.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// D6.1: Bidirectional Streaming
// ═══════════════════════════════════════════════════════════════════════

/// Direction of an RPC stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    /// Client sends a stream of messages, server replies once.
    ClientToServer,
    /// Server sends a stream of messages to a single client request.
    ServerToClient,
    /// Both sides stream messages concurrently.
    Bidirectional,
}

impl fmt::Display for StreamDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamDirection::ClientToServer => write!(f, "ClientToServer"),
            StreamDirection::ServerToClient => write!(f, "ServerToClient"),
            StreamDirection::Bidirectional => write!(f, "Bidirectional"),
        }
    }
}

/// A single frame in a streaming RPC exchange.
#[derive(Debug, Clone)]
pub struct StreamFrame {
    /// Monotonic sequence number within the stream.
    pub seq: u64,
    /// Payload bytes.
    pub payload: Vec<u8>,
    /// Whether this is the final frame in the stream.
    pub end_of_stream: bool,
}

/// A bidirectional stream channel holding frames from both sides.
#[derive(Debug)]
pub struct BiStream {
    /// Stream identifier.
    pub stream_id: u64,
    /// Direction of this stream.
    pub direction: StreamDirection,
    /// Frames sent by the client (outbound from client perspective).
    pub client_frames: Vec<StreamFrame>,
    /// Frames sent by the server (outbound from server perspective).
    pub server_frames: Vec<StreamFrame>,
    /// Whether the client side has closed.
    pub client_closed: bool,
    /// Whether the server side has closed.
    pub server_closed: bool,
}

impl BiStream {
    /// Creates a new bidirectional stream.
    pub fn new(stream_id: u64, direction: StreamDirection) -> Self {
        BiStream {
            stream_id,
            direction,
            client_frames: Vec::new(),
            server_frames: Vec::new(),
            client_closed: false,
            server_closed: false,
        }
    }

    /// Sends a frame from the client side.
    pub fn send_client(&mut self, payload: Vec<u8>, end_of_stream: bool) -> Result<(), String> {
        if self.client_closed {
            return Err("client side already closed".to_string());
        }
        let seq = self.client_frames.len() as u64;
        self.client_frames.push(StreamFrame {
            seq,
            payload,
            end_of_stream,
        });
        if end_of_stream {
            self.client_closed = true;
        }
        Ok(())
    }

    /// Sends a frame from the server side.
    pub fn send_server(&mut self, payload: Vec<u8>, end_of_stream: bool) -> Result<(), String> {
        if self.server_closed {
            return Err("server side already closed".to_string());
        }
        let seq = self.server_frames.len() as u64;
        self.server_frames.push(StreamFrame {
            seq,
            payload,
            end_of_stream,
        });
        if end_of_stream {
            self.server_closed = true;
        }
        Ok(())
    }

    /// Returns true if both sides have closed.
    pub fn is_complete(&self) -> bool {
        self.client_closed && self.server_closed
    }

    /// Returns the total number of frames exchanged.
    pub fn total_frames(&self) -> usize {
        self.client_frames.len() + self.server_frames.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.2: RPC Timeout & Deadline Propagation
// ═══════════════════════════════════════════════════════════════════════

/// A deadline for an RPC call that propagates across hops.
#[derive(Debug, Clone)]
pub struct RpcDeadline {
    /// Absolute deadline as milliseconds since epoch.
    pub absolute_ms: u64,
    /// Original timeout requested by the caller.
    pub original_timeout: Duration,
    /// Number of hops this deadline has traversed.
    pub hops: u32,
}

impl RpcDeadline {
    /// Creates a deadline from a timeout relative to a given "now" timestamp.
    pub fn from_timeout(now_ms: u64, timeout: Duration) -> Self {
        RpcDeadline {
            absolute_ms: now_ms + timeout.as_millis() as u64,
            original_timeout: timeout,
            hops: 0,
        }
    }

    /// Returns the remaining time at the given "now" timestamp, or None if expired.
    pub fn remaining(&self, now_ms: u64) -> Option<Duration> {
        if now_ms >= self.absolute_ms {
            None
        } else {
            Some(Duration::from_millis(self.absolute_ms - now_ms))
        }
    }

    /// Propagates the deadline to the next hop (increments hop counter).
    pub fn propagate(&self) -> Self {
        RpcDeadline {
            absolute_ms: self.absolute_ms,
            original_timeout: self.original_timeout,
            hops: self.hops + 1,
        }
    }

    /// Returns true if the deadline has expired at the given time.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.absolute_ms
    }
}

impl fmt::Display for RpcDeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Deadline(abs={}ms, hops={})",
            self.absolute_ms, self.hops
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.3: Compression (gzip / lz4)
// ═══════════════════════════════════════════════════════════════════════

/// Compression algorithm for RPC payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression.
    None,
    /// Gzip compression.
    Gzip,
    /// LZ4 fast compression.
    Lz4,
    /// Zstandard compression.
    Zstd,
}

impl fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompressionAlgorithm::None => write!(f, "None"),
            CompressionAlgorithm::Gzip => write!(f, "Gzip"),
            CompressionAlgorithm::Lz4 => write!(f, "LZ4"),
            CompressionAlgorithm::Zstd => write!(f, "Zstd"),
        }
    }
}

/// Compressed payload with metadata.
#[derive(Debug, Clone)]
pub struct CompressedPayload {
    /// Algorithm used.
    pub algorithm: CompressionAlgorithm,
    /// Original size in bytes.
    pub original_size: usize,
    /// Compressed data.
    pub data: Vec<u8>,
}

/// Simulates compressing a payload. In production this would call real
/// gzip/lz4 libraries; here we use a trivial RLE-like stub.
pub fn compress(data: &[u8], algorithm: CompressionAlgorithm) -> CompressedPayload {
    let compressed = match algorithm {
        CompressionAlgorithm::None => data.to_vec(),
        CompressionAlgorithm::Gzip | CompressionAlgorithm::Lz4 | CompressionAlgorithm::Zstd => {
            // Simulated compression: prefix with algorithm tag + original length
            let mut buf = Vec::with_capacity(data.len() + 5);
            buf.push(algorithm as u8);
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
            buf
        }
    };
    CompressedPayload {
        algorithm,
        original_size: data.len(),
        data: compressed,
    }
}

/// Simulates decompressing a payload.
pub fn decompress(payload: &CompressedPayload) -> Result<Vec<u8>, String> {
    match payload.algorithm {
        CompressionAlgorithm::None => Ok(payload.data.clone()),
        CompressionAlgorithm::Gzip | CompressionAlgorithm::Lz4 | CompressionAlgorithm::Zstd => {
            if payload.data.len() < 5 {
                return Err("compressed payload too short".to_string());
            }
            let orig_len =
                u32::from_le_bytes(payload.data[1..5].try_into().map_err(|e| format!("{e}"))?)
                    as usize;
            if payload.data.len() < 5 + orig_len {
                return Err("compressed payload truncated".to_string());
            }
            Ok(payload.data[5..5 + orig_len].to_vec())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.4: Authentication (TLS + Bearer Token)
// ═══════════════════════════════════════════════════════════════════════

/// Authentication method for RPC calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// No authentication.
    None,
    /// Bearer token (e.g., JWT).
    BearerToken(String),
    /// Mutual TLS with certificate fingerprint.
    MutualTls { cert_fingerprint: String },
    /// API key.
    ApiKey(String),
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthMethod::None => write!(f, "None"),
            AuthMethod::BearerToken(_) => write!(f, "BearerToken(****)"),
            AuthMethod::MutualTls { .. } => write!(f, "MutualTLS"),
            AuthMethod::ApiKey(_) => write!(f, "ApiKey(****)"),
        }
    }
}

/// An RPC authentication context attached to each call.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authentication method.
    pub method: AuthMethod,
    /// Authenticated identity (set after verification).
    pub identity: Option<String>,
    /// Roles assigned to this identity.
    pub roles: Vec<String>,
}

impl AuthContext {
    /// Creates an unauthenticated context.
    pub fn anonymous() -> Self {
        AuthContext {
            method: AuthMethod::None,
            identity: None,
            roles: Vec::new(),
        }
    }

    /// Creates a context with a bearer token.
    pub fn with_bearer(token: &str) -> Self {
        AuthContext {
            method: AuthMethod::BearerToken(token.to_string()),
            identity: None,
            roles: Vec::new(),
        }
    }

    /// Simulates verifying the token. In production this would check JWT signatures.
    pub fn verify(&mut self, valid_tokens: &HashMap<String, String>) -> bool {
        match &self.method {
            AuthMethod::BearerToken(token) => {
                if let Some(user) = valid_tokens.get(token) {
                    self.identity = Some(user.clone());
                    true
                } else {
                    false
                }
            }
            AuthMethod::ApiKey(key) => {
                if let Some(user) = valid_tokens.get(key) {
                    self.identity = Some(user.clone());
                    true
                } else {
                    false
                }
            }
            AuthMethod::MutualTls { cert_fingerprint } => {
                if let Some(user) = valid_tokens.get(cert_fingerprint) {
                    self.identity = Some(user.clone());
                    true
                } else {
                    false
                }
            }
            AuthMethod::None => false,
        }
    }

    /// Returns true if the context has been authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.identity.is_some()
    }

    /// Checks if the identity has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.5: Interceptors (Pre/Post Hooks)
// ═══════════════════════════════════════════════════════════════════════

/// The phase at which an interceptor runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterceptorPhase {
    /// Before the RPC handler executes.
    Pre,
    /// After the RPC handler executes.
    Post,
}

impl fmt::Display for InterceptorPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterceptorPhase::Pre => write!(f, "Pre"),
            InterceptorPhase::Post => write!(f, "Post"),
        }
    }
}

/// An interceptor entry with a name and phase.
#[derive(Debug, Clone)]
pub struct Interceptor {
    /// Interceptor name for debugging.
    pub name: String,
    /// Phase at which this interceptor runs.
    pub phase: InterceptorPhase,
    /// Whether this interceptor is enabled.
    pub enabled: bool,
}

/// An interceptor chain that runs pre/post hooks around RPC calls.
#[derive(Debug, Default)]
pub struct InterceptorChain {
    /// Registered interceptors in order.
    interceptors: Vec<Interceptor>,
    /// Execution log (for testing/debugging).
    pub execution_log: Vec<String>,
}

impl InterceptorChain {
    /// Creates an empty interceptor chain.
    pub fn new() -> Self {
        InterceptorChain::default()
    }

    /// Adds an interceptor.
    pub fn add(&mut self, name: &str, phase: InterceptorPhase) {
        self.interceptors.push(Interceptor {
            name: name.to_string(),
            phase,
            enabled: true,
        });
    }

    /// Runs all pre-interceptors and returns their names.
    pub fn run_pre(&mut self) -> Vec<String> {
        let names: Vec<String> = self
            .interceptors
            .iter()
            .filter(|i| i.enabled && i.phase == InterceptorPhase::Pre)
            .map(|i| i.name.clone())
            .collect();
        for name in &names {
            self.execution_log.push(format!("pre:{name}"));
        }
        names
    }

    /// Runs all post-interceptors and returns their names.
    pub fn run_post(&mut self) -> Vec<String> {
        let names: Vec<String> = self
            .interceptors
            .iter()
            .filter(|i| i.enabled && i.phase == InterceptorPhase::Post)
            .map(|i| i.name.clone())
            .collect();
        for name in &names {
            self.execution_log.push(format!("post:{name}"));
        }
        names
    }

    /// Disables an interceptor by name.
    pub fn disable(&mut self, name: &str) {
        for i in &mut self.interceptors {
            if i.name == name {
                i.enabled = false;
            }
        }
    }

    /// Returns the total number of registered interceptors.
    pub fn count(&self) -> usize {
        self.interceptors.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.6: Client-Side Load Balancing
// ═══════════════════════════════════════════════════════════════════════

/// Client-side load balancing strategy with health awareness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientLbStrategy {
    /// Round-robin across healthy endpoints.
    RoundRobin,
    /// Pick the endpoint with lowest observed latency.
    LeastLatency,
    /// Consistent hashing by request key.
    ConsistentHash,
    /// Random selection.
    Random,
}

impl fmt::Display for ClientLbStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientLbStrategy::RoundRobin => write!(f, "RoundRobin"),
            ClientLbStrategy::LeastLatency => write!(f, "LeastLatency"),
            ClientLbStrategy::ConsistentHash => write!(f, "ConsistentHash"),
            ClientLbStrategy::Random => write!(f, "Random"),
        }
    }
}

/// Health state of an endpoint for client-side load balancing.
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    /// Endpoint address.
    pub address: String,
    /// Whether this endpoint is considered healthy.
    pub healthy: bool,
    /// Average latency in microseconds.
    pub avg_latency_us: u64,
    /// Consecutive failures.
    pub consecutive_failures: u32,
}

/// A client-side load balancer tracking endpoint health.
#[derive(Debug)]
pub struct ClientLoadBalancer {
    /// Strategy.
    pub strategy: ClientLbStrategy,
    /// Known endpoints and their health.
    pub endpoints: Vec<EndpointHealth>,
    /// Round-robin counter.
    rr_counter: usize,
}

impl ClientLoadBalancer {
    /// Creates a new client load balancer.
    pub fn new(strategy: ClientLbStrategy) -> Self {
        ClientLoadBalancer {
            strategy,
            endpoints: Vec::new(),
            rr_counter: 0,
        }
    }

    /// Registers an endpoint.
    pub fn add_endpoint(&mut self, address: &str) {
        self.endpoints.push(EndpointHealth {
            address: address.to_string(),
            healthy: true,
            avg_latency_us: 0,
            consecutive_failures: 0,
        });
    }

    /// Marks an endpoint unhealthy after failures.
    pub fn mark_unhealthy(&mut self, address: &str) {
        if let Some(ep) = self.endpoints.iter_mut().find(|e| e.address == address) {
            ep.healthy = false;
        }
    }

    /// Marks an endpoint healthy.
    pub fn mark_healthy(&mut self, address: &str) {
        if let Some(ep) = self.endpoints.iter_mut().find(|e| e.address == address) {
            ep.healthy = true;
            ep.consecutive_failures = 0;
        }
    }

    /// Updates observed latency for an endpoint.
    pub fn update_latency(&mut self, address: &str, latency_us: u64) {
        if let Some(ep) = self.endpoints.iter_mut().find(|e| e.address == address) {
            // Exponential moving average
            if ep.avg_latency_us == 0 {
                ep.avg_latency_us = latency_us;
            } else {
                ep.avg_latency_us = (ep.avg_latency_us * 7 + latency_us) / 8;
            }
        }
    }

    /// Selects the next endpoint based on the strategy.
    pub fn select(&mut self) -> Option<String> {
        let healthy: Vec<&EndpointHealth> = self.endpoints.iter().filter(|e| e.healthy).collect();
        if healthy.is_empty() {
            return None;
        }

        match self.strategy {
            ClientLbStrategy::RoundRobin => {
                let idx = self.rr_counter % healthy.len();
                self.rr_counter += 1;
                Some(healthy[idx].address.clone())
            }
            ClientLbStrategy::LeastLatency => healthy
                .iter()
                .min_by_key(|e| e.avg_latency_us)
                .map(|e| e.address.clone()),
            ClientLbStrategy::ConsistentHash => {
                // Simplified: pick first healthy
                Some(healthy[0].address.clone())
            }
            ClientLbStrategy::Random => {
                // Deterministic for testing: pick last healthy
                healthy.last().map(|e| e.address.clone())
            }
        }
    }

    /// Returns the number of healthy endpoints.
    pub fn healthy_count(&self) -> usize {
        self.endpoints.iter().filter(|e| e.healthy).count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.7: Reflection (List Methods)
// ═══════════════════════════════════════════════════════════════════════

/// Descriptor for a method exposed via reflection.
#[derive(Debug, Clone)]
pub struct MethodDescriptor {
    /// Fully qualified method name (service.method).
    pub full_name: String,
    /// Input type name.
    pub input_type: String,
    /// Output type name.
    pub output_type: String,
    /// Whether this method supports streaming.
    pub streaming: bool,
}

/// A reflection service that lists all registered RPC methods.
#[derive(Debug, Default)]
pub struct ReflectionService {
    /// Registered service descriptors.
    services: HashMap<String, Vec<MethodDescriptor>>,
}

impl ReflectionService {
    /// Creates a new reflection service.
    pub fn new() -> Self {
        ReflectionService::default()
    }

    /// Registers a method for a service.
    pub fn register_method(
        &mut self,
        service: &str,
        method: &str,
        input_type: &str,
        output_type: &str,
        streaming: bool,
    ) {
        let descriptor = MethodDescriptor {
            full_name: format!("{service}.{method}"),
            input_type: input_type.to_string(),
            output_type: output_type.to_string(),
            streaming,
        };
        self.services
            .entry(service.to_string())
            .or_default()
            .push(descriptor);
    }

    /// Lists all services.
    pub fn list_services(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }

    /// Lists methods for a specific service.
    pub fn list_methods(&self, service: &str) -> Option<&[MethodDescriptor]> {
        self.services.get(service).map(|v| v.as_slice())
    }

    /// Looks up a method by its full name.
    pub fn find_method(&self, full_name: &str) -> Option<&MethodDescriptor> {
        for methods in self.services.values() {
            if let Some(m) = methods.iter().find(|m| m.full_name == full_name) {
                return Some(m);
            }
        }
        None
    }

    /// Returns the total number of registered methods.
    pub fn method_count(&self) -> usize {
        self.services.values().map(|v| v.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.8: Health Service
// ═══════════════════════════════════════════════════════════════════════

/// Health status of a service component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is fully operational.
    Serving,
    /// Component is not serving (degraded or down).
    NotServing,
    /// Health status is unknown.
    Unknown,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Serving => write!(f, "SERVING"),
            HealthStatus::NotServing => write!(f, "NOT_SERVING"),
            HealthStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// A health service that reports the status of registered components.
#[derive(Debug, Default)]
pub struct HealthService {
    /// Component name -> health status.
    components: HashMap<String, HealthStatus>,
}

impl HealthService {
    /// Creates a new health service.
    pub fn new() -> Self {
        HealthService::default()
    }

    /// Sets the health status of a component.
    pub fn set_status(&mut self, component: &str, status: HealthStatus) {
        self.components.insert(component.to_string(), status);
    }

    /// Checks the health of a specific component.
    pub fn check(&self, component: &str) -> HealthStatus {
        self.components
            .get(component)
            .copied()
            .unwrap_or(HealthStatus::Unknown)
    }

    /// Returns true if all registered components are serving.
    pub fn is_all_healthy(&self) -> bool {
        !self.components.is_empty()
            && self
                .components
                .values()
                .all(|s| *s == HealthStatus::Serving)
    }

    /// Returns the list of unhealthy components.
    pub fn unhealthy_components(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|(_, s)| **s != HealthStatus::Serving)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Returns the total number of registered components.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.9: Metrics (Count / Latency / Error Rate)
// ═══════════════════════════════════════════════════════════════════════

/// Metrics collected per RPC method.
#[derive(Debug, Clone)]
pub struct MethodMetrics {
    /// Method name.
    pub method: String,
    /// Total number of calls.
    pub call_count: u64,
    /// Number of successful calls.
    pub success_count: u64,
    /// Number of failed calls.
    pub error_count: u64,
    /// Total latency in microseconds (for average computation).
    pub total_latency_us: u64,
    /// Minimum observed latency in microseconds.
    pub min_latency_us: u64,
    /// Maximum observed latency in microseconds.
    pub max_latency_us: u64,
}

impl MethodMetrics {
    /// Creates new zeroed metrics for a method.
    pub fn new(method: &str) -> Self {
        MethodMetrics {
            method: method.to_string(),
            call_count: 0,
            success_count: 0,
            error_count: 0,
            total_latency_us: 0,
            min_latency_us: u64::MAX,
            max_latency_us: 0,
        }
    }

    /// Records a successful call with latency.
    pub fn record_success(&mut self, latency_us: u64) {
        self.call_count += 1;
        self.success_count += 1;
        self.total_latency_us += latency_us;
        if latency_us < self.min_latency_us {
            self.min_latency_us = latency_us;
        }
        if latency_us > self.max_latency_us {
            self.max_latency_us = latency_us;
        }
    }

    /// Records a failed call.
    pub fn record_error(&mut self) {
        self.call_count += 1;
        self.error_count += 1;
    }

    /// Returns the average latency in microseconds, or 0 if no successes.
    pub fn avg_latency_us(&self) -> u64 {
        self.total_latency_us
            .checked_div(self.success_count)
            .unwrap_or(0)
    }

    /// Returns the error rate as a fraction (0.0 - 1.0).
    pub fn error_rate(&self) -> f64 {
        if self.call_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.call_count as f64
        }
    }
}

/// A metrics collector for all RPC methods.
#[derive(Debug, Default)]
pub struct MetricsCollector {
    /// Per-method metrics.
    methods: HashMap<String, MethodMetrics>,
}

impl MetricsCollector {
    /// Creates a new empty collector.
    pub fn new() -> Self {
        MetricsCollector::default()
    }

    /// Records a successful call.
    pub fn record_success(&mut self, method: &str, latency_us: u64) {
        self.methods
            .entry(method.to_string())
            .or_insert_with(|| MethodMetrics::new(method))
            .record_success(latency_us);
    }

    /// Records a failed call.
    pub fn record_error(&mut self, method: &str) {
        self.methods
            .entry(method.to_string())
            .or_insert_with(|| MethodMetrics::new(method))
            .record_error();
    }

    /// Returns metrics for a specific method.
    pub fn get(&self, method: &str) -> Option<&MethodMetrics> {
        self.methods.get(method)
    }

    /// Returns the total call count across all methods.
    pub fn total_calls(&self) -> u64 {
        self.methods.values().map(|m| m.call_count).sum()
    }

    /// Returns the overall error rate across all methods.
    pub fn overall_error_rate(&self) -> f64 {
        let total_calls: u64 = self.methods.values().map(|m| m.call_count).sum();
        let total_errors: u64 = self.methods.values().map(|m| m.error_count).sum();
        if total_calls == 0 {
            0.0
        } else {
            total_errors as f64 / total_calls as f64
        }
    }

    /// Resets all metrics.
    pub fn reset(&mut self) {
        self.methods.clear();
    }

    /// Returns the number of tracked methods.
    pub fn tracked_methods(&self) -> usize {
        self.methods.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D6.10: Integration — RPC V2 Server
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for the RPC V2 server.
#[derive(Debug, Clone)]
pub struct RpcV2Config {
    /// Compression algorithm for payloads.
    pub compression: CompressionAlgorithm,
    /// Default call timeout.
    pub default_timeout: Duration,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
    /// Whether to enable reflection.
    pub enable_reflection: bool,
    /// Whether to enable health service.
    pub enable_health: bool,
    /// Whether to enable metrics collection.
    pub enable_metrics: bool,
}

impl Default for RpcV2Config {
    fn default() -> Self {
        RpcV2Config {
            compression: CompressionAlgorithm::None,
            default_timeout: Duration::from_secs(30),
            max_message_size: 16 * 1024 * 1024, // 16 MB
            enable_reflection: true,
            enable_health: true,
            enable_metrics: true,
        }
    }
}

/// An RPC V2 server combining all D6 features.
#[derive(Debug)]
pub struct RpcV2Server {
    /// Server configuration.
    pub config: RpcV2Config,
    /// Interceptor chain.
    pub interceptors: InterceptorChain,
    /// Health service.
    pub health: HealthService,
    /// Reflection service.
    pub reflection: ReflectionService,
    /// Metrics collector.
    pub metrics: MetricsCollector,
    /// Active streams.
    pub active_streams: HashMap<u64, BiStream>,
    /// Next stream ID.
    next_stream_id: u64,
}

impl RpcV2Server {
    /// Creates a new RPC V2 server with the given configuration.
    pub fn new(config: RpcV2Config) -> Self {
        RpcV2Server {
            config,
            interceptors: InterceptorChain::new(),
            health: HealthService::new(),
            reflection: ReflectionService::new(),
            metrics: MetricsCollector::new(),
            active_streams: HashMap::new(),
            next_stream_id: 1,
        }
    }

    /// Opens a new bidirectional stream and returns its ID.
    pub fn open_stream(&mut self, direction: StreamDirection) -> u64 {
        let id = self.next_stream_id;
        self.next_stream_id += 1;
        self.active_streams.insert(id, BiStream::new(id, direction));
        id
    }

    /// Closes and removes a stream.
    pub fn close_stream(&mut self, stream_id: u64) -> bool {
        self.active_streams.remove(&stream_id).is_some()
    }

    /// Registers a method in the reflection service.
    pub fn register_method(
        &mut self,
        service: &str,
        method: &str,
        input_type: &str,
        output_type: &str,
        streaming: bool,
    ) {
        self.reflection
            .register_method(service, method, input_type, output_type, streaming);
    }

    /// Simulates handling an RPC call with full pipeline.
    pub fn handle_call(&mut self, method: &str, _payload: &[u8], latency_us: u64, success: bool) {
        // Run pre-interceptors
        self.interceptors.run_pre();

        // Record metrics
        if success {
            self.metrics.record_success(method, latency_us);
        } else {
            self.metrics.record_error(method);
        }

        // Run post-interceptors
        self.interceptors.run_post();
    }

    /// Returns the number of active streams.
    pub fn active_stream_count(&self) -> usize {
        self.active_streams.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // D6.1 — Bidirectional Streaming
    #[test]
    fn d6_1_bistream_send_receive() {
        let mut stream = BiStream::new(1, StreamDirection::Bidirectional);
        stream.send_client(b"hello".to_vec(), false).unwrap();
        stream.send_server(b"world".to_vec(), false).unwrap();
        assert_eq!(stream.total_frames(), 2);
        assert!(!stream.is_complete());
    }

    #[test]
    fn d6_1_bistream_close() {
        let mut stream = BiStream::new(1, StreamDirection::Bidirectional);
        stream.send_client(b"data".to_vec(), true).unwrap();
        assert!(stream.client_closed);
        assert!(stream.send_client(b"more".to_vec(), false).is_err());
        stream.send_server(b"reply".to_vec(), true).unwrap();
        assert!(stream.is_complete());
    }

    #[test]
    fn d6_1_stream_direction_display() {
        assert_eq!(StreamDirection::Bidirectional.to_string(), "Bidirectional");
        assert_eq!(
            StreamDirection::ClientToServer.to_string(),
            "ClientToServer"
        );
    }

    // D6.2 — Deadline Propagation
    #[test]
    fn d6_2_deadline_remaining() {
        let deadline = RpcDeadline::from_timeout(1000, Duration::from_millis(500));
        assert_eq!(deadline.remaining(1200), Some(Duration::from_millis(300)));
        assert!(deadline.remaining(1500).is_none()); // Expired
    }

    #[test]
    fn d6_2_deadline_propagation() {
        let d1 = RpcDeadline::from_timeout(1000, Duration::from_secs(5));
        let d2 = d1.propagate();
        assert_eq!(d2.hops, 1);
        assert_eq!(d2.absolute_ms, d1.absolute_ms);
        let d3 = d2.propagate();
        assert_eq!(d3.hops, 2);
    }

    #[test]
    fn d6_2_deadline_expired() {
        let deadline = RpcDeadline::from_timeout(1000, Duration::from_millis(100));
        assert!(!deadline.is_expired(1050));
        assert!(deadline.is_expired(1100));
    }

    // D6.3 — Compression
    #[test]
    fn d6_3_compress_decompress_gzip() {
        let data = b"hello world, this is a test payload for compression";
        let compressed = compress(data, CompressionAlgorithm::Gzip);
        assert_eq!(compressed.original_size, data.len());
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn d6_3_compress_none_passthrough() {
        let data = b"raw data";
        let compressed = compress(data, CompressionAlgorithm::None);
        assert_eq!(compressed.data, data);
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn d6_3_compression_algorithm_display() {
        assert_eq!(CompressionAlgorithm::Lz4.to_string(), "LZ4");
        assert_eq!(CompressionAlgorithm::Zstd.to_string(), "Zstd");
    }

    // D6.4 — Authentication
    #[test]
    fn d6_4_bearer_auth() {
        let mut ctx = AuthContext::with_bearer("token-abc");
        let mut tokens = HashMap::new();
        tokens.insert("token-abc".to_string(), "user@example.com".to_string());

        assert!(!ctx.is_authenticated());
        assert!(ctx.verify(&tokens));
        assert!(ctx.is_authenticated());
        assert_eq!(ctx.identity.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn d6_4_invalid_token() {
        let mut ctx = AuthContext::with_bearer("bad-token");
        let tokens = HashMap::new();
        assert!(!ctx.verify(&tokens));
        assert!(!ctx.is_authenticated());
    }

    // D6.5 — Interceptors
    #[test]
    fn d6_5_interceptor_chain() {
        let mut chain = InterceptorChain::new();
        chain.add("auth", InterceptorPhase::Pre);
        chain.add("logging", InterceptorPhase::Pre);
        chain.add("metrics", InterceptorPhase::Post);

        let pre = chain.run_pre();
        assert_eq!(pre, vec!["auth", "logging"]);

        let post = chain.run_post();
        assert_eq!(post, vec!["metrics"]);

        assert_eq!(chain.execution_log.len(), 3);
    }

    #[test]
    fn d6_5_interceptor_disable() {
        let mut chain = InterceptorChain::new();
        chain.add("auth", InterceptorPhase::Pre);
        chain.add("logging", InterceptorPhase::Pre);

        chain.disable("auth");
        let pre = chain.run_pre();
        assert_eq!(pre, vec!["logging"]);
    }

    // D6.6 — Client-Side Load Balancing
    #[test]
    fn d6_6_round_robin_lb() {
        let mut lb = ClientLoadBalancer::new(ClientLbStrategy::RoundRobin);
        lb.add_endpoint("10.0.0.1:8080");
        lb.add_endpoint("10.0.0.2:8080");
        lb.add_endpoint("10.0.0.3:8080");

        assert_eq!(lb.select().unwrap(), "10.0.0.1:8080");
        assert_eq!(lb.select().unwrap(), "10.0.0.2:8080");
        assert_eq!(lb.select().unwrap(), "10.0.0.3:8080");
        assert_eq!(lb.select().unwrap(), "10.0.0.1:8080"); // Wraps
    }

    #[test]
    fn d6_6_unhealthy_endpoint_skipped() {
        let mut lb = ClientLoadBalancer::new(ClientLbStrategy::RoundRobin);
        lb.add_endpoint("10.0.0.1:8080");
        lb.add_endpoint("10.0.0.2:8080");

        lb.mark_unhealthy("10.0.0.1:8080");
        assert_eq!(lb.healthy_count(), 1);
        assert_eq!(lb.select().unwrap(), "10.0.0.2:8080");
    }

    #[test]
    fn d6_6_least_latency() {
        let mut lb = ClientLoadBalancer::new(ClientLbStrategy::LeastLatency);
        lb.add_endpoint("fast:8080");
        lb.add_endpoint("slow:8080");

        lb.update_latency("fast:8080", 100);
        lb.update_latency("slow:8080", 5000);
        assert_eq!(lb.select().unwrap(), "fast:8080");
    }

    // D6.7 — Reflection
    #[test]
    fn d6_7_reflection_list_methods() {
        let mut refl = ReflectionService::new();
        refl.register_method("Greeter", "SayHello", "HelloRequest", "HelloReply", false);
        refl.register_method("Greeter", "StreamGreet", "HelloRequest", "HelloReply", true);

        assert_eq!(refl.method_count(), 2);
        let methods = refl.list_methods("Greeter").unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].full_name, "Greeter.SayHello");
    }

    #[test]
    fn d6_7_reflection_find_method() {
        let mut refl = ReflectionService::new();
        refl.register_method("Math", "Add", "AddReq", "AddResp", false);

        let m = refl.find_method("Math.Add").unwrap();
        assert_eq!(m.input_type, "AddReq");
        assert!(refl.find_method("Math.Sub").is_none());
    }

    // D6.8 — Health Service
    #[test]
    fn d6_8_health_service() {
        let mut hs = HealthService::new();
        hs.set_status("rpc", HealthStatus::Serving);
        hs.set_status("db", HealthStatus::Serving);

        assert!(hs.is_all_healthy());
        assert_eq!(hs.check("rpc"), HealthStatus::Serving);

        hs.set_status("db", HealthStatus::NotServing);
        assert!(!hs.is_all_healthy());
        assert_eq!(hs.unhealthy_components(), vec!["db"]);
    }

    #[test]
    fn d6_8_health_unknown() {
        let hs = HealthService::new();
        assert_eq!(hs.check("nonexistent"), HealthStatus::Unknown);
    }

    #[test]
    fn d6_8_health_status_display() {
        assert_eq!(HealthStatus::Serving.to_string(), "SERVING");
        assert_eq!(HealthStatus::NotServing.to_string(), "NOT_SERVING");
    }

    // D6.9 — Metrics
    #[test]
    fn d6_9_metrics_collection() {
        let mut mc = MetricsCollector::new();
        mc.record_success("SayHello", 500);
        mc.record_success("SayHello", 1000);
        mc.record_error("SayHello");

        let m = mc.get("SayHello").unwrap();
        assert_eq!(m.call_count, 3);
        assert_eq!(m.success_count, 2);
        assert_eq!(m.error_count, 1);
        assert_eq!(m.avg_latency_us(), 750);
        assert!((m.error_rate() - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn d6_9_metrics_min_max_latency() {
        let mut mc = MetricsCollector::new();
        mc.record_success("Ping", 100);
        mc.record_success("Ping", 900);
        mc.record_success("Ping", 500);

        let m = mc.get("Ping").unwrap();
        assert_eq!(m.min_latency_us, 100);
        assert_eq!(m.max_latency_us, 900);
    }

    #[test]
    fn d6_9_overall_error_rate() {
        let mut mc = MetricsCollector::new();
        mc.record_success("A", 100);
        mc.record_error("B");
        assert!((mc.overall_error_rate() - 0.5).abs() < 0.01);
    }

    // D6.10 — RPC V2 Server Integration
    #[test]
    fn d6_10_server_open_close_stream() {
        let mut server = RpcV2Server::new(RpcV2Config::default());
        let id = server.open_stream(StreamDirection::Bidirectional);
        assert_eq!(server.active_stream_count(), 1);
        assert!(server.close_stream(id));
        assert_eq!(server.active_stream_count(), 0);
    }

    #[test]
    fn d6_10_server_handle_call() {
        let mut server = RpcV2Server::new(RpcV2Config::default());
        server.interceptors.add("auth", InterceptorPhase::Pre);
        server.interceptors.add("log", InterceptorPhase::Post);

        server.handle_call("Greeter.SayHello", b"payload", 200, true);

        assert_eq!(server.metrics.total_calls(), 1);
        assert_eq!(server.interceptors.execution_log.len(), 2);
    }

    #[test]
    fn d6_10_server_full_pipeline() {
        let mut server = RpcV2Server::new(RpcV2Config {
            compression: CompressionAlgorithm::Lz4,
            ..RpcV2Config::default()
        });

        server.register_method("ML", "Predict", "Tensor", "Tensor", false);
        server.health.set_status("ML", HealthStatus::Serving);
        server.handle_call("ML.Predict", b"input_tensor", 1500, true);

        assert!(server.health.is_all_healthy());
        assert_eq!(server.reflection.method_count(), 1);
        assert_eq!(server.metrics.total_calls(), 1);
    }
}
