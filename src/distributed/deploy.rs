//! CLI & Deployment — Sprint D8: `fj run --cluster`, cluster management
//! commands, fj.toml cluster config, Dockerfile generation, Kubernetes
//! Helm chart, Grafana dashboard, and structured JSON logging.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D8.1: `fj run --cluster` CLI
// ═══════════════════════════════════════════════════════════════════════

/// Parsed arguments for `fj run --cluster`.
#[derive(Debug, Clone)]
pub struct ClusterRunArgs {
    /// Source file to compile and distribute.
    pub source_file: String,
    /// Number of workers to spawn (or auto-detect).
    pub workers: Option<u32>,
    /// Bind address for the scheduler.
    pub bind_address: String,
    /// Cluster name.
    pub cluster_name: String,
    /// Whether to enable GPU workers.
    pub enable_gpu: bool,
    /// Log level.
    pub log_level: LogLevel,
}

impl Default for ClusterRunArgs {
    fn default() -> Self {
        ClusterRunArgs {
            source_file: String::new(),
            workers: None,
            bind_address: "0.0.0.0:9000".to_string(),
            cluster_name: "fj-cluster".to_string(),
            enable_gpu: false,
            log_level: LogLevel::Info,
        }
    }
}

impl ClusterRunArgs {
    /// Parses cluster run arguments from a slice of strings.
    pub fn parse(args: &[&str]) -> Result<Self, String> {
        let mut result = ClusterRunArgs::default();

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "--workers" | "-w" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--workers requires a value".to_string());
                    }
                    result.workers = Some(
                        args[i]
                            .parse()
                            .map_err(|_| format!("invalid worker count: {}", args[i]))?,
                    );
                }
                "--bind" | "-b" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--bind requires a value".to_string());
                    }
                    result.bind_address = args[i].to_string();
                }
                "--name" | "-n" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--name requires a value".to_string());
                    }
                    result.cluster_name = args[i].to_string();
                }
                "--gpu" => {
                    result.enable_gpu = true;
                }
                "--log-level" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("--log-level requires a value".to_string());
                    }
                    result.log_level = LogLevel::parse_level(args[i])?;
                }
                s if !s.starts_with('-') && result.source_file.is_empty() => {
                    result.source_file = s.to_string();
                }
                other => {
                    return Err(format!("unknown flag: {other}"));
                }
            }
            i += 1;
        }

        if result.source_file.is_empty() {
            return Err("source file required".to_string());
        }

        Ok(result)
    }

    /// Generates the command-line string representation.
    pub fn to_command_string(&self) -> String {
        let mut parts = vec![format!("fj run --cluster {}", self.source_file)];
        if let Some(w) = self.workers {
            parts.push(format!("--workers {w}"));
        }
        parts.push(format!("--bind {}", self.bind_address));
        parts.push(format!("--name {}", self.cluster_name));
        if self.enable_gpu {
            parts.push("--gpu".to_string());
        }
        parts.join(" ")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D8.2: `fj cluster status/join/leave` CLI
// ═══════════════════════════════════════════════════════════════════════

/// A cluster management command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterCommand {
    /// Show cluster status.
    Status,
    /// Join an existing cluster.
    Join { address: String },
    /// Leave the cluster gracefully.
    Leave,
    /// List all nodes.
    Nodes,
    /// Scale workers up or down.
    Scale { workers: u32 },
}

impl fmt::Display for ClusterCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClusterCommand::Status => write!(f, "cluster status"),
            ClusterCommand::Join { address } => write!(f, "cluster join {address}"),
            ClusterCommand::Leave => write!(f, "cluster leave"),
            ClusterCommand::Nodes => write!(f, "cluster nodes"),
            ClusterCommand::Scale { workers } => write!(f, "cluster scale {workers}"),
        }
    }
}

/// Parses a cluster management command from args.
pub fn parse_cluster_command(args: &[&str]) -> Result<ClusterCommand, String> {
    if args.is_empty() {
        return Err("cluster subcommand required (status, join, leave, nodes, scale)".to_string());
    }

    match args[0] {
        "status" => Ok(ClusterCommand::Status),
        "join" => {
            if args.len() < 2 {
                return Err("cluster join requires an address".to_string());
            }
            Ok(ClusterCommand::Join {
                address: args[1].to_string(),
            })
        }
        "leave" => Ok(ClusterCommand::Leave),
        "nodes" => Ok(ClusterCommand::Nodes),
        "scale" => {
            if args.len() < 2 {
                return Err("cluster scale requires a worker count".to_string());
            }
            let workers: u32 = args[1]
                .parse()
                .map_err(|_| format!("invalid worker count: {}", args[1]))?;
            Ok(ClusterCommand::Scale { workers })
        }
        other => Err(format!("unknown cluster command: {other}")),
    }
}

/// Result of executing a cluster command.
#[derive(Debug, Clone)]
pub struct ClusterCommandResult {
    /// Whether the command succeeded.
    pub success: bool,
    /// Human-readable message.
    pub message: String,
    /// Optional structured data.
    pub data: HashMap<String, String>,
}

impl ClusterCommandResult {
    /// Creates a success result.
    pub fn ok(message: &str) -> Self {
        ClusterCommandResult {
            success: true,
            message: message.to_string(),
            data: HashMap::new(),
        }
    }

    /// Creates an error result.
    pub fn err(message: &str) -> Self {
        ClusterCommandResult {
            success: false,
            message: message.to_string(),
            data: HashMap::new(),
        }
    }

    /// Adds a data field.
    pub fn with_data(mut self, key: &str, value: &str) -> Self {
        self.data.insert(key.to_string(), value.to_string());
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D8.3: `[cluster]` Config in fj.toml
// ═══════════════════════════════════════════════════════════════════════

/// Cluster configuration parsed from `[cluster]` in fj.toml.
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Cluster name.
    pub name: String,
    /// Scheduler address.
    pub scheduler_address: String,
    /// Discovery method.
    pub discovery: DiscoveryConfig,
    /// Number of workers.
    pub workers: u32,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_secs: u32,
    /// Heartbeat timeout in seconds.
    pub heartbeat_timeout_secs: u32,
    /// Replication factor.
    pub replication_factor: u32,
    /// Whether TLS is enabled.
    pub tls_enabled: bool,
    /// TLS certificate path (if enabled).
    pub tls_cert_path: Option<String>,
    /// TLS key path (if enabled).
    pub tls_key_path: Option<String>,
}

/// Discovery configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryConfig {
    /// Static seed nodes.
    Static { seeds: Vec<String> },
    /// Multicast discovery.
    Multicast { group: String, port: u16 },
    /// DNS-based discovery.
    Dns { hostname: String },
}

impl fmt::Display for DiscoveryConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscoveryConfig::Static { seeds } => write!(f, "Static({} seeds)", seeds.len()),
            DiscoveryConfig::Multicast { group, port } => {
                write!(f, "Multicast({group}:{port})")
            }
            DiscoveryConfig::Dns { hostname } => write!(f, "DNS({hostname})"),
        }
    }
}

impl Default for ClusterConfig {
    fn default() -> Self {
        ClusterConfig {
            name: "fj-cluster".to_string(),
            scheduler_address: "0.0.0.0:9000".to_string(),
            discovery: DiscoveryConfig::Static {
                seeds: vec!["127.0.0.1:9000".to_string()],
            },
            workers: 4,
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            replication_factor: 3,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

impl ClusterConfig {
    /// Generates a TOML string representation of this config.
    pub fn to_toml(&self) -> String {
        let mut lines = Vec::new();
        lines.push("[cluster]".to_string());
        lines.push(format!("name = \"{}\"", self.name));
        lines.push(format!(
            "scheduler_address = \"{}\"",
            self.scheduler_address
        ));
        lines.push(format!("workers = {}", self.workers));
        lines.push(format!(
            "heartbeat_interval_secs = {}",
            self.heartbeat_interval_secs
        ));
        lines.push(format!(
            "heartbeat_timeout_secs = {}",
            self.heartbeat_timeout_secs
        ));
        lines.push(format!("replication_factor = {}", self.replication_factor));
        lines.push(format!("tls_enabled = {}", self.tls_enabled));

        match &self.discovery {
            DiscoveryConfig::Static { seeds } => {
                let seeds_str: Vec<String> = seeds.iter().map(|s| format!("\"{s}\"")).collect();
                lines.push("discovery = \"static\"".to_string());
                lines.push(format!("seeds = [{}]", seeds_str.join(", ")));
            }
            DiscoveryConfig::Multicast { group, port } => {
                lines.push("discovery = \"multicast\"".to_string());
                lines.push(format!("multicast_group = \"{group}\""));
                lines.push(format!("multicast_port = {port}"));
            }
            DiscoveryConfig::Dns { hostname } => {
                lines.push("discovery = \"dns\"".to_string());
                lines.push(format!("dns_hostname = \"{hostname}\""));
            }
        }

        lines.join("\n")
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.name.is_empty() {
            errors.push("cluster name cannot be empty".to_string());
        }
        if self.workers == 0 {
            errors.push("workers must be > 0".to_string());
        }
        if self.heartbeat_timeout_secs <= self.heartbeat_interval_secs {
            errors.push("heartbeat_timeout must be > heartbeat_interval".to_string());
        }
        if self.tls_enabled && self.tls_cert_path.is_none() {
            errors.push("tls_cert_path required when TLS is enabled".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D8.4: Dockerfile Generation
// ═══════════════════════════════════════════════════════════════════════

/// Dockerfile generation configuration.
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Base image.
    pub base_image: String,
    /// Rust toolchain version.
    pub rust_version: String,
    /// Whether to include GPU support.
    pub gpu: bool,
    /// Additional packages to install.
    pub extra_packages: Vec<String>,
    /// Exposed ports.
    pub ports: Vec<u16>,
    /// Entry point command.
    pub entrypoint: String,
}

impl Default for DockerConfig {
    fn default() -> Self {
        DockerConfig {
            base_image: "rust:1.87-slim".to_string(),
            rust_version: "1.87".to_string(),
            gpu: false,
            extra_packages: Vec::new(),
            ports: vec![9000],
            entrypoint: "/usr/local/bin/fj".to_string(),
        }
    }
}

/// Generates a Dockerfile for a Fajar Lang cluster node.
pub fn generate_dockerfile(config: &DockerConfig) -> String {
    let mut lines = Vec::new();

    let base = if config.gpu {
        "nvidia/cuda:12.2.0-runtime-ubuntu22.04".to_string()
    } else {
        config.base_image.clone()
    };

    lines.push("# Fajar Lang Distributed Runtime".to_string());
    lines.push("# Auto-generated by `fj cluster dockerfile`".to_string());
    lines.push(format!("FROM {base} AS builder"));
    lines.push(String::new());

    if config.gpu {
        lines.push("# Install Rust toolchain".to_string());
        lines.push(format!(
            "RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain {}", config.rust_version
        ));
        lines.push("ENV PATH=\"/root/.cargo/bin:${PATH}\"".to_string());
    }

    if !config.extra_packages.is_empty() {
        lines.push("RUN apt-get update && apt-get install -y \\".to_string());
        for pkg in &config.extra_packages {
            lines.push(format!("    {pkg} \\"));
        }
        lines.push("    && rm -rf /var/lib/apt/lists/*".to_string());
    }

    lines.push(String::new());
    lines.push("WORKDIR /app".to_string());
    lines.push("COPY . .".to_string());
    lines.push("RUN cargo build --release".to_string());
    lines.push(String::new());

    // Multi-stage for smaller image
    lines.push("FROM debian:bookworm-slim AS runtime".to_string());
    lines.push(format!(
        "COPY --from=builder /app/target/release/fj {}",
        config.entrypoint
    ));
    lines.push(String::new());

    for port in &config.ports {
        lines.push(format!("EXPOSE {port}"));
    }
    lines.push(String::new());

    lines.push(format!("ENTRYPOINT [\"{}\"]", config.entrypoint));
    lines.push("CMD [\"run\", \"--cluster\", \"main.fj\"]".to_string());

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// D8.5: Kubernetes Helm Chart
// ═══════════════════════════════════════════════════════════════════════

/// Helm chart configuration for Kubernetes deployment.
#[derive(Debug, Clone)]
pub struct HelmChartConfig {
    /// Chart name.
    pub chart_name: String,
    /// Chart version.
    pub chart_version: String,
    /// Application version.
    pub app_version: String,
    /// Docker image.
    pub image: String,
    /// Number of scheduler replicas.
    pub scheduler_replicas: u32,
    /// Number of worker replicas.
    pub worker_replicas: u32,
    /// CPU request per worker.
    pub worker_cpu: String,
    /// Memory request per worker.
    pub worker_memory: String,
    /// GPU request per worker (0 for no GPU).
    pub worker_gpu: u32,
    /// Service type (ClusterIP, NodePort, LoadBalancer).
    pub service_type: String,
}

impl Default for HelmChartConfig {
    fn default() -> Self {
        HelmChartConfig {
            chart_name: "fj-cluster".to_string(),
            chart_version: "0.1.0".to_string(),
            app_version: "10.0.0".to_string(),
            image: "fajarlang/fj:latest".to_string(),
            scheduler_replicas: 1,
            worker_replicas: 4,
            worker_cpu: "1000m".to_string(),
            worker_memory: "2Gi".to_string(),
            worker_gpu: 0,
            service_type: "ClusterIP".to_string(),
        }
    }
}

/// Generates the Chart.yaml content.
pub fn generate_chart_yaml(config: &HelmChartConfig) -> String {
    let mut lines = Vec::new();
    lines.push("apiVersion: v2".to_string());
    lines.push(format!("name: {}", config.chart_name));
    lines.push("description: Fajar Lang Distributed Runtime Helm Chart".to_string());
    lines.push("type: application".to_string());
    lines.push(format!("version: {}", config.chart_version));
    lines.push(format!("appVersion: \"{}\"", config.app_version));
    lines.join("\n")
}

/// Generates the values.yaml content.
pub fn generate_values_yaml(config: &HelmChartConfig) -> String {
    let mut lines = Vec::new();
    lines.push("# Fajar Lang Cluster Values".to_string());
    lines.push(format!("image: {}", config.image));
    lines.push(String::new());
    lines.push("scheduler:".to_string());
    lines.push(format!("  replicas: {}", config.scheduler_replicas));
    lines.push("  port: 9000".to_string());
    lines.push(String::new());
    lines.push("worker:".to_string());
    lines.push(format!("  replicas: {}", config.worker_replicas));
    lines.push("  resources:".to_string());
    lines.push("    requests:".to_string());
    lines.push(format!("      cpu: {}", config.worker_cpu));
    lines.push(format!("      memory: {}", config.worker_memory));
    if config.worker_gpu > 0 {
        lines.push(format!("      nvidia.com/gpu: {}", config.worker_gpu));
    }
    lines.push(String::new());
    lines.push("service:".to_string());
    lines.push(format!("  type: {}", config.service_type));
    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// D8.6: Grafana Dashboard Template
// ═══════════════════════════════════════════════════════════════════════

/// A panel in a Grafana dashboard.
#[derive(Debug, Clone)]
pub struct GrafanaPanel {
    /// Panel title.
    pub title: String,
    /// Panel type (graph, stat, table, gauge).
    pub panel_type: String,
    /// PromQL query.
    pub query: String,
}

/// Generates a Grafana dashboard JSON template.
pub fn generate_grafana_dashboard(cluster_name: &str) -> GrafanaDashboard {
    let panels = vec![
        GrafanaPanel {
            title: "Active Workers".to_string(),
            panel_type: "stat".to_string(),
            query: format!("fj_cluster_active_workers{{cluster=\"{cluster_name}\"}}"),
        },
        GrafanaPanel {
            title: "Task Throughput".to_string(),
            panel_type: "graph".to_string(),
            query: format!(
                "rate(fj_cluster_tasks_completed_total{{cluster=\"{cluster_name}\"}}[5m])"
            ),
        },
        GrafanaPanel {
            title: "RPC Latency (p99)".to_string(),
            panel_type: "graph".to_string(),
            query: format!(
                "histogram_quantile(0.99, fj_rpc_duration_seconds_bucket{{cluster=\"{cluster_name}\"}})"
            ),
        },
        GrafanaPanel {
            title: "Error Rate".to_string(),
            panel_type: "gauge".to_string(),
            query: format!("rate(fj_cluster_errors_total{{cluster=\"{cluster_name}\"}}[5m])"),
        },
        GrafanaPanel {
            title: "Memory Usage".to_string(),
            panel_type: "graph".to_string(),
            query: format!("fj_worker_memory_bytes{{cluster=\"{cluster_name}\"}}"),
        },
        GrafanaPanel {
            title: "GPU Utilization".to_string(),
            panel_type: "graph".to_string(),
            query: format!("fj_worker_gpu_utilization{{cluster=\"{cluster_name}\"}}"),
        },
    ];

    GrafanaDashboard {
        title: format!("Fajar Lang Cluster: {cluster_name}"),
        panels,
    }
}

/// A Grafana dashboard definition.
#[derive(Debug, Clone)]
pub struct GrafanaDashboard {
    /// Dashboard title.
    pub title: String,
    /// Dashboard panels.
    pub panels: Vec<GrafanaPanel>,
}

impl GrafanaDashboard {
    /// Returns the number of panels.
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    /// Generates a simplified JSON representation.
    pub fn to_json(&self) -> String {
        let mut panels_json = Vec::new();
        for (i, panel) in self.panels.iter().enumerate() {
            panels_json.push(format!(
                "    {{\"id\": {}, \"title\": \"{}\", \"type\": \"{}\", \"query\": \"{}\"}}",
                i + 1,
                panel.title,
                panel.panel_type,
                panel.query
            ));
        }
        format!(
            "{{\n  \"title\": \"{}\",\n  \"panels\": [\n{}\n  ]\n}}",
            self.title,
            panels_json.join(",\n")
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D8.7-D8.8: Structured JSON Logging
// ═══════════════════════════════════════════════════════════════════════

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace-level detail.
    Trace,
    /// Debug information.
    Debug,
    /// General information.
    Info,
    /// Warning.
    Warn,
    /// Error.
    Error,
}

impl LogLevel {
    /// Parses a log level from a string.
    pub fn parse_level(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            other => Err(format!("unknown log level: {other}")),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A structured log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Log level.
    pub level: LogLevel,
    /// Message.
    pub message: String,
    /// Timestamp (ISO 8601 string).
    pub timestamp: String,
    /// Component/module that produced this log.
    pub component: String,
    /// Additional key-value fields.
    pub fields: HashMap<String, String>,
}

impl LogEntry {
    /// Creates a new log entry.
    pub fn new(level: LogLevel, component: &str, message: &str, timestamp: &str) -> Self {
        LogEntry {
            level,
            message: message.to_string(),
            timestamp: timestamp.to_string(),
            component: component.to_string(),
            fields: HashMap::new(),
        }
    }

    /// Adds a field to the log entry.
    pub fn with_field(mut self, key: &str, value: &str) -> Self {
        self.fields.insert(key.to_string(), value.to_string());
        self
    }

    /// Serializes the log entry to JSON.
    pub fn to_json(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("\"level\":\"{}\"", self.level));
        parts.push(format!("\"ts\":\"{}\"", self.timestamp));
        parts.push(format!("\"component\":\"{}\"", self.component));
        parts.push(format!("\"msg\":\"{}\"", self.message));

        for (k, v) in &self.fields {
            parts.push(format!("\"{k}\":\"{v}\""));
        }

        format!("{{{}}}", parts.join(","))
    }
}

/// A structured logger that buffers entries.
#[derive(Debug, Default)]
pub struct StructuredLogger {
    /// Minimum log level.
    pub min_level: Option<LogLevel>,
    /// Buffered log entries.
    pub entries: Vec<LogEntry>,
}

impl StructuredLogger {
    /// Creates a new logger with the given minimum level.
    pub fn new(min_level: LogLevel) -> Self {
        StructuredLogger {
            min_level: Some(min_level),
            entries: Vec::new(),
        }
    }

    /// Logs an entry if it meets the minimum level.
    pub fn log(&mut self, entry: LogEntry) {
        if let Some(min) = self.min_level {
            if entry.level >= min {
                self.entries.push(entry);
            }
        } else {
            self.entries.push(entry);
        }
    }

    /// Returns all entries as JSON lines.
    pub fn to_json_lines(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.to_json())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns the number of logged entries.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D8.9-D8.10: Deployment Orchestrator
// ═══════════════════════════════════════════════════════════════════════

/// A deployment plan combining all D8 artifacts.
#[derive(Debug)]
pub struct DeploymentPlan {
    /// Cluster configuration.
    pub cluster_config: ClusterConfig,
    /// Docker configuration.
    pub docker_config: DockerConfig,
    /// Helm chart configuration.
    pub helm_config: HelmChartConfig,
    /// Generated artifacts (name -> content).
    pub artifacts: HashMap<String, String>,
}

impl DeploymentPlan {
    /// Creates a new deployment plan from cluster config.
    pub fn new(cluster_config: ClusterConfig) -> Self {
        DeploymentPlan {
            cluster_config,
            docker_config: DockerConfig::default(),
            helm_config: HelmChartConfig::default(),
            artifacts: HashMap::new(),
        }
    }

    /// Generates all deployment artifacts.
    pub fn generate(&mut self) {
        // Dockerfile
        self.artifacts.insert(
            "Dockerfile".to_string(),
            generate_dockerfile(&self.docker_config),
        );

        // Helm Chart.yaml
        self.artifacts.insert(
            "chart/Chart.yaml".to_string(),
            generate_chart_yaml(&self.helm_config),
        );

        // Helm values.yaml
        self.artifacts.insert(
            "chart/values.yaml".to_string(),
            generate_values_yaml(&self.helm_config),
        );

        // fj.toml cluster section
        self.artifacts
            .insert("fj.toml".to_string(), self.cluster_config.to_toml());

        // Grafana dashboard
        let dashboard = generate_grafana_dashboard(&self.cluster_config.name);
        self.artifacts
            .insert("grafana/dashboard.json".to_string(), dashboard.to_json());
    }

    /// Returns the number of generated artifacts.
    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    /// Returns the artifact names.
    pub fn artifact_names(&self) -> Vec<&str> {
        self.artifacts.keys().map(|s| s.as_str()).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // D8.1 — `fj run --cluster`
    #[test]
    fn d8_1_parse_cluster_run_basic() {
        let args = ClusterRunArgs::parse(&["main.fj", "--workers", "4"]).unwrap();
        assert_eq!(args.source_file, "main.fj");
        assert_eq!(args.workers, Some(4));
    }

    #[test]
    fn d8_1_parse_cluster_run_all_flags() {
        let args = ClusterRunArgs::parse(&[
            "train.fj",
            "--workers",
            "8",
            "--bind",
            "10.0.0.1:9000",
            "--name",
            "ml-cluster",
            "--gpu",
        ])
        .unwrap();
        assert_eq!(args.source_file, "train.fj");
        assert_eq!(args.workers, Some(8));
        assert_eq!(args.bind_address, "10.0.0.1:9000");
        assert!(args.enable_gpu);
    }

    #[test]
    fn d8_1_parse_missing_source() {
        assert!(ClusterRunArgs::parse(&["--workers", "4"]).is_err());
    }

    // D8.2 — Cluster Commands
    #[test]
    fn d8_2_parse_cluster_status() {
        let cmd = parse_cluster_command(&["status"]).unwrap();
        assert_eq!(cmd, ClusterCommand::Status);
    }

    #[test]
    fn d8_2_parse_cluster_join() {
        let cmd = parse_cluster_command(&["join", "10.0.0.1:9000"]).unwrap();
        assert_eq!(
            cmd,
            ClusterCommand::Join {
                address: "10.0.0.1:9000".to_string()
            }
        );
    }

    #[test]
    fn d8_2_parse_cluster_scale() {
        let cmd = parse_cluster_command(&["scale", "8"]).unwrap();
        assert_eq!(cmd, ClusterCommand::Scale { workers: 8 });
    }

    #[test]
    fn d8_2_command_result() {
        let result = ClusterCommandResult::ok("Cluster healthy")
            .with_data("nodes", "5")
            .with_data("tasks", "12");
        assert!(result.success);
        assert_eq!(result.data.get("nodes").unwrap(), "5");
    }

    // D8.3 — Cluster Config
    #[test]
    fn d8_3_config_to_toml() {
        let config = ClusterConfig::default();
        let toml = config.to_toml();
        assert!(toml.contains("[cluster]"));
        assert!(toml.contains("name = \"fj-cluster\""));
        assert!(toml.contains("workers = 4"));
    }

    #[test]
    fn d8_3_config_validation() {
        let mut config = ClusterConfig::default();
        assert!(config.validate().is_ok());

        config.workers = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn d8_3_config_tls_validation() {
        let mut config = ClusterConfig::default();
        config.tls_enabled = true;
        let errors = config.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("tls_cert_path")));
    }

    // D8.4 — Dockerfile
    #[test]
    fn d8_4_generate_dockerfile() {
        let config = DockerConfig::default();
        let dockerfile = generate_dockerfile(&config);
        assert!(dockerfile.contains("FROM rust:1.87-slim AS builder"));
        assert!(dockerfile.contains("cargo build --release"));
        assert!(dockerfile.contains("EXPOSE 9000"));
    }

    #[test]
    fn d8_4_dockerfile_gpu() {
        let config = DockerConfig {
            gpu: true,
            ..DockerConfig::default()
        };
        let dockerfile = generate_dockerfile(&config);
        assert!(dockerfile.contains("nvidia/cuda"));
    }

    // D8.5 — Helm Chart
    #[test]
    fn d8_5_chart_yaml() {
        let config = HelmChartConfig::default();
        let yaml = generate_chart_yaml(&config);
        assert!(yaml.contains("apiVersion: v2"));
        assert!(yaml.contains("name: fj-cluster"));
    }

    #[test]
    fn d8_5_values_yaml() {
        let config = HelmChartConfig {
            worker_gpu: 1,
            ..HelmChartConfig::default()
        };
        let yaml = generate_values_yaml(&config);
        assert!(yaml.contains("replicas: 4"));
        assert!(yaml.contains("nvidia.com/gpu: 1"));
    }

    // D8.6 — Grafana Dashboard
    #[test]
    fn d8_6_grafana_dashboard() {
        let dashboard = generate_grafana_dashboard("ml-train");
        assert_eq!(dashboard.panel_count(), 6);
        let json = dashboard.to_json();
        assert!(json.contains("Active Workers"));
        assert!(json.contains("ml-train"));
    }

    // D8.7-D8.8 — Structured Logging
    #[test]
    fn d8_7_log_entry_json() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "scheduler",
            "Task assigned",
            "2026-03-31T00:00:00Z",
        )
        .with_field("task_id", "42")
        .with_field("worker", "node-1");
        let json = entry.to_json();
        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"task_id\":\"42\""));
    }

    #[test]
    fn d8_8_structured_logger() {
        let mut logger = StructuredLogger::new(LogLevel::Info);
        logger.log(LogEntry::new(
            LogLevel::Debug,
            "test",
            "debug msg",
            "2026-01-01T00:00:00Z",
        ));
        logger.log(LogEntry::new(
            LogLevel::Info,
            "test",
            "info msg",
            "2026-01-01T00:00:00Z",
        ));
        logger.log(LogEntry::new(
            LogLevel::Error,
            "test",
            "error msg",
            "2026-01-01T00:00:00Z",
        ));
        assert_eq!(logger.count(), 2); // Debug filtered out
    }

    #[test]
    fn d8_8_log_level_parsing() {
        assert_eq!(LogLevel::parse_level("info").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::parse_level("WARNING").unwrap(), LogLevel::Warn);
        assert!(LogLevel::parse_level("verbose").is_err());
    }

    // D8.9-D8.10 — Deployment Plan
    #[test]
    fn d8_9_deployment_plan_generation() {
        let mut plan = DeploymentPlan::new(ClusterConfig::default());
        plan.generate();
        assert_eq!(plan.artifact_count(), 5);
        let names = plan.artifact_names();
        assert!(names.contains(&"Dockerfile"));
        assert!(names.contains(&"chart/Chart.yaml"));
        assert!(names.contains(&"grafana/dashboard.json"));
    }

    #[test]
    fn d8_10_deployment_plan_custom_config() {
        let config = ClusterConfig {
            name: "mnist-cluster".to_string(),
            workers: 8,
            ..ClusterConfig::default()
        };
        let mut plan = DeploymentPlan::new(config);
        plan.helm_config.worker_replicas = 8;
        plan.generate();

        let values = plan.artifacts.get("chart/values.yaml").unwrap();
        assert!(values.contains("replicas: 8"));

        let dashboard = plan.artifacts.get("grafana/dashboard.json").unwrap();
        assert!(dashboard.contains("mnist-cluster"));
    }
}
