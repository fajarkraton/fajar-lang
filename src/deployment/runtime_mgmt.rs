//! Runtime Management — graceful shutdown, hot reload, feature flags,
//! connection draining, process supervision, rolling update,
//! memory limits, thread pool tuning, runtime info endpoint.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// S31.1: Graceful Shutdown
// ═══════════════════════════════════════════════════════════════════════

/// Shutdown phase.
#[derive(Debug, Clone, PartialEq)]
pub enum ShutdownPhase {
    /// Running normally
    Running,
    /// Signal received, draining connections
    Draining,
    /// Flushing buffers and saving state
    Flushing,
    /// Shutdown complete
    Stopped,
}

/// Shutdown controller.
#[derive(Debug)]
pub struct ShutdownController {
    /// Current phase
    pub phase: ShutdownPhase,
    /// Registered shutdown hooks (name → completed)
    pub hooks: Vec<(String, bool)>,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Whether a signal has been received
    pub signal_received: bool,
}

impl ShutdownController {
    /// Create a new shutdown controller.
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            phase: ShutdownPhase::Running,
            hooks: Vec::new(),
            timeout_secs,
            signal_received: false,
        }
    }

    /// Register a shutdown hook.
    pub fn register_hook(&mut self, name: &str) {
        self.hooks.push((name.to_string(), false));
    }

    /// Begin shutdown sequence.
    pub fn begin_shutdown(&mut self) {
        self.signal_received = true;
        self.phase = ShutdownPhase::Draining;
    }

    /// Mark a hook as completed.
    pub fn complete_hook(&mut self, name: &str) -> bool {
        if let Some(hook) = self.hooks.iter_mut().find(|(n, _)| n == name) {
            hook.1 = true;
            true
        } else {
            false
        }
    }

    /// Advance to next phase if all hooks in current phase are done.
    pub fn advance(&mut self) -> ShutdownPhase {
        let all_done = self.hooks.iter().all(|(_, done)| *done);
        match &self.phase {
            ShutdownPhase::Draining if all_done => {
                self.phase = ShutdownPhase::Flushing;
            }
            ShutdownPhase::Flushing if all_done => {
                self.phase = ShutdownPhase::Stopped;
            }
            _ => {}
        }
        self.phase.clone()
    }

    /// Check if shutdown is complete.
    pub fn is_stopped(&self) -> bool {
        self.phase == ShutdownPhase::Stopped
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.2: Hot Reload
// ═══════════════════════════════════════════════════════════════════════

/// Configuration that can be hot-reloaded.
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Configuration values
    pub values: HashMap<String, String>,
    /// Reload generation (incremented on each reload)
    pub generation: u64,
    /// Registered change watchers
    pub watchers: Vec<String>,
}

impl HotReloadConfig {
    /// Create a new config.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            generation: 0,
            watchers: Vec::new(),
        }
    }

    /// Set a value.
    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    /// Get a value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Reload with new values, increment generation.
    pub fn reload(&mut self, new_values: HashMap<String, String>) -> Vec<String> {
        let mut changed = Vec::new();
        for (key, new_val) in &new_values {
            let old_val = self.values.get(key);
            if old_val.map(|v| v.as_str()) != Some(new_val.as_str()) {
                changed.push(key.clone());
            }
        }
        self.values = new_values;
        self.generation += 1;
        changed
    }

    /// Register a watcher for config changes.
    pub fn watch(&mut self, key: &str) {
        self.watchers.push(key.to_string());
    }
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.3: Feature Flags Runtime
// ═══════════════════════════════════════════════════════════════════════

/// Feature flag state.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagState {
    /// Feature enabled
    Enabled,
    /// Feature disabled
    Disabled,
    /// Percentage rollout (0-100)
    Rollout(u8),
}

/// Runtime feature flag.
#[derive(Debug, Clone)]
pub struct FeatureFlag {
    /// Flag name
    pub name: String,
    /// Current state
    pub state: FlagState,
    /// Description
    pub description: String,
}

/// Feature flag registry.
#[derive(Debug)]
pub struct FlagRegistry {
    /// All flags
    pub flags: Vec<FeatureFlag>,
}

impl FlagRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self { flags: Vec::new() }
    }

    /// Register a flag.
    pub fn register(&mut self, name: &str, state: FlagState, description: &str) {
        self.flags.push(FeatureFlag {
            name: name.to_string(),
            state,
            description: description.to_string(),
        });
    }

    /// Check if a flag is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags
            .iter()
            .find(|f| f.name == name)
            .map(|f| matches!(f.state, FlagState::Enabled))
            .unwrap_or(false)
    }

    /// Toggle a flag.
    pub fn toggle(&mut self, name: &str) -> bool {
        if let Some(flag) = self.flags.iter_mut().find(|f| f.name == name) {
            flag.state = match flag.state {
                FlagState::Enabled => FlagState::Disabled,
                FlagState::Disabled | FlagState::Rollout(_) => FlagState::Enabled,
            };
            true
        } else {
            false
        }
    }

    /// Set rollout percentage.
    pub fn set_rollout(&mut self, name: &str, percent: u8) -> bool {
        if let Some(flag) = self.flags.iter_mut().find(|f| f.name == name) {
            flag.state = FlagState::Rollout(percent.min(100));
            true
        } else {
            false
        }
    }

    /// Check if rollout includes a given user (hash-based).
    pub fn check_rollout(&self, name: &str, user_id: u64) -> bool {
        self.flags
            .iter()
            .find(|f| f.name == name)
            .map(|f| match f.state {
                FlagState::Enabled => true,
                FlagState::Disabled => false,
                FlagState::Rollout(pct) => (user_id % 100) < pct as u64,
            })
            .unwrap_or(false)
    }
}

impl Default for FlagRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.4: Connection Draining
// ═══════════════════════════════════════════════════════════════════════

/// Active connection.
#[derive(Debug, Clone)]
pub struct Connection {
    /// Connection ID
    pub id: u64,
    /// Client address
    pub addr: String,
    /// Whether this connection is in-flight (processing a request)
    pub in_flight: bool,
    /// Whether new requests are accepted on this connection
    pub accepting: bool,
}

/// Connection drainer.
#[derive(Debug)]
pub struct ConnectionDrainer {
    /// Active connections
    pub connections: Vec<Connection>,
    /// Whether draining has started
    pub draining: bool,
}

impl ConnectionDrainer {
    /// Create a new drainer.
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            draining: false,
        }
    }

    /// Add a connection.
    pub fn add(&mut self, id: u64, addr: &str) {
        self.connections.push(Connection {
            id,
            addr: addr.to_string(),
            in_flight: false,
            accepting: true,
        });
    }

    /// Start draining — stop accepting new requests.
    pub fn start_drain(&mut self) {
        self.draining = true;
        for conn in &mut self.connections {
            conn.accepting = false;
        }
    }

    /// Mark a connection as finished.
    pub fn finish_connection(&mut self, id: u64) {
        self.connections.retain(|c| c.id != id);
    }

    /// Count remaining active connections.
    pub fn active_count(&self) -> usize {
        self.connections.len()
    }

    /// Whether draining is complete (all connections closed).
    pub fn is_drained(&self) -> bool {
        self.draining && self.connections.is_empty()
    }
}

impl Default for ConnectionDrainer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.5: Process Supervision
// ═══════════════════════════════════════════════════════════════════════

/// Process restart policy.
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    /// Maximum restarts in the window
    pub max_restarts: u32,
    /// Window duration in seconds
    pub window_secs: u64,
    /// Base backoff in milliseconds
    pub base_backoff_ms: u64,
    /// Maximum backoff in milliseconds
    pub max_backoff_ms: u64,
}

impl RestartPolicy {
    /// Default: 5 restarts in 60 seconds.
    pub fn default_policy() -> Self {
        Self {
            max_restarts: 5,
            window_secs: 60,
            base_backoff_ms: 100,
            max_backoff_ms: 30_000,
        }
    }
}

/// Process supervisor state.
#[derive(Debug)]
pub struct Supervisor {
    /// Restart policy
    pub policy: RestartPolicy,
    /// Restart timestamps (seconds)
    pub restart_times: Vec<u64>,
    /// Current backoff level
    pub backoff_level: u32,
}

impl Supervisor {
    /// Create a new supervisor.
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            restart_times: Vec::new(),
            backoff_level: 0,
        }
    }

    /// Check if we should restart (within policy limits).
    pub fn should_restart(&self, now_secs: u64) -> bool {
        let window_start = now_secs.saturating_sub(self.policy.window_secs);
        let recent_restarts = self
            .restart_times
            .iter()
            .filter(|&&t| t >= window_start)
            .count() as u32;
        recent_restarts < self.policy.max_restarts
    }

    /// Record a restart.
    pub fn record_restart(&mut self, now_secs: u64) {
        self.restart_times.push(now_secs);
        self.backoff_level += 1;
    }

    /// Current backoff in milliseconds (exponential).
    pub fn current_backoff_ms(&self) -> u64 {
        let backoff = self.policy.base_backoff_ms * 2u64.pow(self.backoff_level);
        backoff.min(self.policy.max_backoff_ms)
    }

    /// Reset backoff after successful run.
    pub fn reset_backoff(&mut self) {
        self.backoff_level = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.6: Rolling Update
// ═══════════════════════════════════════════════════════════════════════

/// Rolling update state.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    /// No update in progress
    Idle,
    /// Preparing new version
    Preparing,
    /// Handing off listen socket
    Handoff,
    /// New process running, old draining
    Draining,
    /// Update complete
    Complete,
}

/// Rolling update controller.
#[derive(Debug)]
pub struct RollingUpdate {
    /// Current state
    pub state: UpdateState,
    /// Old process PID
    pub old_pid: Option<u32>,
    /// New process PID
    pub new_pid: Option<u32>,
    /// New version
    pub new_version: String,
}

impl RollingUpdate {
    /// Create a new rolling update.
    pub fn new(new_version: &str) -> Self {
        Self {
            state: UpdateState::Idle,
            old_pid: None,
            new_pid: None,
            new_version: new_version.to_string(),
        }
    }

    /// Begin the update.
    pub fn begin(&mut self, old_pid: u32) {
        self.old_pid = Some(old_pid);
        self.state = UpdateState::Preparing;
    }

    /// Complete handoff — new process is listening.
    pub fn handoff(&mut self, new_pid: u32) {
        self.new_pid = Some(new_pid);
        self.state = UpdateState::Handoff;
    }

    /// Begin draining old process.
    pub fn start_drain(&mut self) {
        self.state = UpdateState::Draining;
    }

    /// Mark update complete.
    pub fn complete(&mut self) {
        self.state = UpdateState::Complete;
    }

    /// Whether the update is finished.
    pub fn is_complete(&self) -> bool {
        self.state == UpdateState::Complete
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.7: Memory Limits
// ═══════════════════════════════════════════════════════════════════════

/// Memory limit enforcement.
#[derive(Debug)]
pub struct MemoryLimiter {
    /// Maximum heap bytes
    pub max_bytes: u64,
    /// Current usage
    pub current_bytes: u64,
    /// Number of times OOM was triggered
    pub oom_count: u64,
    /// Strategy when OOM
    pub oom_strategy: OomAction,
}

/// Action to take on OOM.
#[derive(Debug, Clone, PartialEq)]
pub enum OomAction {
    /// Reject new work
    RejectNew,
    /// Drop oldest items
    DropOldest,
    /// Log warning only
    WarnOnly,
}

impl MemoryLimiter {
    /// Create a new limiter.
    pub fn new(max_bytes: u64, strategy: OomAction) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            oom_count: 0,
            oom_strategy: strategy,
        }
    }

    /// Try to allocate. Returns false if OOM.
    pub fn try_alloc(&mut self, bytes: u64) -> bool {
        if self.current_bytes + bytes > self.max_bytes {
            self.oom_count += 1;
            false
        } else {
            self.current_bytes += bytes;
            true
        }
    }

    /// Free bytes.
    pub fn free(&mut self, bytes: u64) {
        self.current_bytes = self.current_bytes.saturating_sub(bytes);
    }

    /// Usage percentage.
    pub fn usage_percent(&self) -> f64 {
        if self.max_bytes == 0 {
            return 0.0;
        }
        (self.current_bytes as f64 / self.max_bytes as f64) * 100.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S31.8: Thread Pool Tuning
// ═══════════════════════════════════════════════════════════════════════

/// Thread pool configuration.
#[derive(Debug, Clone)]
pub struct ThreadPoolConfig {
    /// Minimum threads
    pub min_threads: usize,
    /// Maximum threads
    pub max_threads: usize,
    /// Current thread count
    pub current_threads: usize,
    /// Idle timeout before scaling down (seconds)
    pub idle_timeout_secs: u64,
}

impl ThreadPoolConfig {
    /// Create an adaptive thread pool config from CPU count.
    pub fn adaptive(cpu_count: usize) -> Self {
        Self {
            min_threads: 1,
            max_threads: cpu_count * 2,
            current_threads: cpu_count,
            idle_timeout_secs: 60,
        }
    }

    /// Suggest scaling based on queue depth.
    pub fn suggest_scale(&self, queue_depth: usize) -> ScaleAction {
        if queue_depth > self.current_threads * 4 && self.current_threads < self.max_threads {
            ScaleAction::ScaleUp(1)
        } else if queue_depth == 0 && self.current_threads > self.min_threads {
            ScaleAction::ScaleDown(1)
        } else {
            ScaleAction::NoChange
        }
    }
}

/// Thread pool scaling action.
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleAction {
    /// Add threads
    ScaleUp(usize),
    /// Remove threads
    ScaleDown(usize),
    /// No change needed
    NoChange,
}

// ═══════════════════════════════════════════════════════════════════════
// S31.9: Runtime Info Endpoint
// ═══════════════════════════════════════════════════════════════════════

/// Runtime information.
#[derive(Debug, Clone)]
pub struct RuntimeInfo {
    /// Application version
    pub version: String,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Active connections
    pub active_connections: u64,
    /// Total requests served
    pub total_requests: u64,
    /// Current thread count
    pub thread_count: usize,
    /// Heap usage in bytes
    pub heap_bytes: u64,
    /// Configuration hash (for detecting config changes)
    pub config_hash: String,
}

/// Format runtime info as JSON.
pub fn format_runtime_info(info: &RuntimeInfo) -> String {
    format!(
        concat!(
            "{{\"version\":\"{}\",\"uptime_secs\":{},",
            "\"active_connections\":{},\"total_requests\":{},",
            "\"thread_count\":{},\"heap_bytes\":{},",
            "\"config_hash\":\"{}\"}}"
        ),
        info.version,
        info.uptime_secs,
        info.active_connections,
        info.total_requests,
        info.thread_count,
        info.heap_bytes,
        info.config_hash,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S31.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s31_1_shutdown_sequence() {
        let mut ctrl = ShutdownController::new(30);
        ctrl.register_hook("drain_connections");
        ctrl.register_hook("flush_buffers");
        assert_eq!(ctrl.phase, ShutdownPhase::Running);

        ctrl.begin_shutdown();
        assert_eq!(ctrl.phase, ShutdownPhase::Draining);

        ctrl.complete_hook("drain_connections");
        ctrl.complete_hook("flush_buffers");
        ctrl.advance();
        assert_eq!(ctrl.phase, ShutdownPhase::Flushing);

        ctrl.advance();
        assert_eq!(ctrl.phase, ShutdownPhase::Stopped);
        assert!(ctrl.is_stopped());
    }

    #[test]
    fn s31_2_hot_reload_config() {
        let mut config = HotReloadConfig::new();
        config.set("port", "8080");
        assert_eq!(config.get("port"), Some("8080"));

        let mut new_values = HashMap::new();
        new_values.insert("port".into(), "9090".into());
        new_values.insert("host".into(), "0.0.0.0".into());

        let changed = config.reload(new_values);
        assert!(changed.contains(&"port".to_string()));
        assert_eq!(config.generation, 1);
        assert_eq!(config.get("port"), Some("9090"));
    }

    #[test]
    fn s31_3_feature_flags() {
        let mut registry = FlagRegistry::new();
        registry.register("new_ui", FlagState::Disabled, "New UI");
        assert!(!registry.is_enabled("new_ui"));

        registry.toggle("new_ui");
        assert!(registry.is_enabled("new_ui"));
    }

    #[test]
    fn s31_3_feature_flag_rollout() {
        let mut registry = FlagRegistry::new();
        registry.register("experiment", FlagState::Disabled, "Experiment");
        registry.set_rollout("experiment", 50);

        // User IDs 0-49 should be included, 50-99 should not
        assert!(registry.check_rollout("experiment", 10));
        assert!(!registry.check_rollout("experiment", 75));
    }

    #[test]
    fn s31_4_connection_draining() {
        let mut drainer = ConnectionDrainer::new();
        drainer.add(1, "192.168.1.1:5000");
        drainer.add(2, "192.168.1.2:5001");
        assert_eq!(drainer.active_count(), 2);

        drainer.start_drain();
        assert!(drainer.draining);
        assert!(!drainer.connections[0].accepting);

        drainer.finish_connection(1);
        drainer.finish_connection(2);
        assert!(drainer.is_drained());
    }

    #[test]
    fn s31_5_supervisor_restart_policy() {
        let policy = RestartPolicy::default_policy();
        let mut supervisor = Supervisor::new(policy);

        // Should allow restarts
        assert!(supervisor.should_restart(100));
        supervisor.record_restart(100);
        supervisor.record_restart(101);
        assert!(supervisor.should_restart(102));

        // After 5 restarts in 60s, should deny
        supervisor.record_restart(102);
        supervisor.record_restart(103);
        supervisor.record_restart(104);
        assert!(!supervisor.should_restart(105));
    }

    #[test]
    fn s31_5_supervisor_backoff() {
        let policy = RestartPolicy::default_policy();
        let mut supervisor = Supervisor::new(policy);

        assert_eq!(supervisor.current_backoff_ms(), 100); // 100 * 2^0
        supervisor.record_restart(1);
        assert_eq!(supervisor.current_backoff_ms(), 200); // 100 * 2^1
        supervisor.record_restart(2);
        assert_eq!(supervisor.current_backoff_ms(), 400); // 100 * 2^2

        supervisor.reset_backoff();
        assert_eq!(supervisor.current_backoff_ms(), 100);
    }

    #[test]
    fn s31_6_rolling_update() {
        let mut update = RollingUpdate::new("2.0.0");
        assert_eq!(update.state, UpdateState::Idle);

        update.begin(1000);
        assert_eq!(update.state, UpdateState::Preparing);

        update.handoff(1001);
        assert_eq!(update.new_pid, Some(1001));

        update.start_drain();
        assert_eq!(update.state, UpdateState::Draining);

        update.complete();
        assert!(update.is_complete());
    }

    #[test]
    fn s31_7_memory_limiter() {
        let mut limiter = MemoryLimiter::new(1024, OomAction::RejectNew);
        assert!(limiter.try_alloc(512));
        assert!(limiter.try_alloc(256));
        assert!(!limiter.try_alloc(512)); // Exceeds limit
        assert_eq!(limiter.oom_count, 1);
        assert!((limiter.usage_percent() - 75.0).abs() < 0.1);

        limiter.free(256);
        assert!(limiter.try_alloc(512)); // Now fits
    }

    #[test]
    fn s31_8_thread_pool_tuning() {
        let config = ThreadPoolConfig::adaptive(4);
        assert_eq!(config.current_threads, 4);
        assert_eq!(config.max_threads, 8);

        // High queue depth → scale up
        assert_eq!(config.suggest_scale(20), ScaleAction::ScaleUp(1));
        // Empty queue → scale down
        assert_eq!(config.suggest_scale(0), ScaleAction::ScaleDown(1));
        // Normal → no change
        assert_eq!(config.suggest_scale(5), ScaleAction::NoChange);
    }

    #[test]
    fn s31_9_runtime_info() {
        let info = RuntimeInfo {
            version: "1.0.0".into(),
            uptime_secs: 3600,
            active_connections: 42,
            total_requests: 10_000,
            thread_count: 8,
            heap_bytes: 50_000_000,
            config_hash: "abc123".into(),
        };
        let json = format_runtime_info(&info);
        assert!(json.contains("\"version\":\"1.0.0\""));
        assert!(json.contains("\"uptime_secs\":3600"));
        assert!(json.contains("\"active_connections\":42"));
    }

    #[test]
    fn s31_1_shutdown_incomplete_hooks() {
        let mut ctrl = ShutdownController::new(30);
        ctrl.register_hook("hook_a");
        ctrl.begin_shutdown();
        // Don't complete hook_a
        ctrl.advance();
        // Should stay in Draining because hook not done
        assert_eq!(ctrl.phase, ShutdownPhase::Draining);
    }

    #[test]
    fn s31_3_unknown_flag() {
        let registry = FlagRegistry::new();
        assert!(!registry.is_enabled("nonexistent"));
        assert!(!registry.check_rollout("nonexistent", 1));
    }
}
