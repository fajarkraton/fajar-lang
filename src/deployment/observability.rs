//! Observability — structured logging, log levels, metrics registry,
//! Prometheus endpoint, distributed tracing, trace propagation,
//! custom metrics, alerting rules, dashboard generation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════════
// S30.1: Structured Logging
// ═══════════════════════════════════════════════════════════════════════

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Most verbose — internal debug traces
    Trace = 0,
    /// Debug information
    Debug = 1,
    /// Normal informational messages
    Info = 2,
    /// Potential issues
    Warn = 3,
    /// Errors that need attention
    Error = 4,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Parse log level from string.
pub fn parse_log_level(s: &str) -> Option<LogLevel> {
    match s.to_lowercase().as_str() {
        "trace" => Some(LogLevel::Trace),
        "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warn" | "warning" => Some(LogLevel::Warn),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}

/// Structured log entry with key-value fields.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Structured fields
    pub fields: Vec<(String, String)>,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Module/source
    pub module: String,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: &str) -> Self {
        Self {
            level,
            message: message.to_string(),
            fields: Vec::new(),
            timestamp_ns: 0,
            module: String::new(),
        }
    }

    /// Add a field.
    pub fn field(mut self, key: &str, value: &str) -> Self {
        self.fields.push((key.to_string(), value.to_string()));
        self
    }

    /// Format as JSON.
    pub fn to_json(&self) -> String {
        let mut parts = vec![
            format!("\"level\":\"{}\"", self.level),
            format!("\"message\":\"{}\"", self.message),
        ];
        if self.timestamp_ns > 0 {
            parts.push(format!("\"timestamp\":{}", self.timestamp_ns));
        }
        if !self.module.is_empty() {
            parts.push(format!("\"module\":\"{}\"", self.module));
        }
        for (key, val) in &self.fields {
            parts.push(format!("\"{key}\":\"{val}\""));
        }
        format!("{{{}}}", parts.join(","))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.2: Log Levels (runtime configurable)
// ═══════════════════════════════════════════════════════════════════════

/// Logger with configurable minimum level.
#[derive(Debug)]
pub struct Logger {
    /// Minimum level to output
    pub min_level: LogLevel,
    /// Entries captured (for testing)
    pub entries: Vec<LogEntry>,
}

impl Logger {
    /// Create with a minimum level.
    pub fn new(min_level: LogLevel) -> Self {
        Self {
            min_level,
            entries: Vec::new(),
        }
    }

    /// Log an entry if its level meets the minimum.
    pub fn log(&mut self, entry: LogEntry) -> bool {
        if entry.level >= self.min_level {
            self.entries.push(entry);
            true
        } else {
            false
        }
    }

    /// Set minimum level at runtime.
    pub fn set_level(&mut self, level: LogLevel) {
        self.min_level = level;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.3: Metrics Registry
// ═══════════════════════════════════════════════════════════════════════

/// Metric type.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricType {
    /// Monotonically increasing counter
    Counter,
    /// Gauge that can go up and down
    Gauge,
    /// Histogram with bucket boundaries
    Histogram(Vec<f64>),
}

/// A metric value.
#[derive(Debug)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Help text
    pub help: String,
    /// Counter/Gauge value (atomic for thread safety)
    value: AtomicU64,
    /// Histogram observations
    observations: Vec<f64>,
}

impl Metric {
    /// Create a new metric.
    pub fn new(name: &str, metric_type: MetricType, help: &str) -> Self {
        Self {
            name: name.to_string(),
            metric_type,
            help: help.to_string(),
            value: AtomicU64::new(0),
            observations: Vec::new(),
        }
    }

    /// Increment a counter.
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Add to a counter.
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Set a gauge value (using f64 bits as u64).
    pub fn set_gauge(&self, val: f64) {
        self.value.store(val.to_bits(), Ordering::Relaxed);
    }

    /// Get the current value.
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get gauge as f64.
    pub fn get_gauge(&self) -> f64 {
        f64::from_bits(self.value.load(Ordering::Relaxed))
    }

    /// Observe a histogram value.
    pub fn observe(&mut self, val: f64) {
        self.observations.push(val);
    }
}

/// Metrics registry holding all metrics.
#[derive(Debug)]
pub struct MetricsRegistry {
    /// Registered metrics by name
    pub metrics: Vec<Metric>,
}

impl MetricsRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    /// Register a new metric.
    pub fn register(&mut self, name: &str, metric_type: MetricType, help: &str) -> usize {
        let idx = self.metrics.len();
        self.metrics.push(Metric::new(name, metric_type, help));
        idx
    }

    /// Get a metric by index.
    pub fn get(&self, idx: usize) -> Option<&Metric> {
        self.metrics.get(idx)
    }

    /// Get a mutable metric by index.
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Metric> {
        self.metrics.get_mut(idx)
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.4: Prometheus Endpoint
// ═══════════════════════════════════════════════════════════════════════

/// Format metrics in Prometheus text exposition format.
pub fn format_prometheus(registry: &MetricsRegistry) -> String {
    let mut output = String::new();
    for metric in &registry.metrics {
        let type_str = match &metric.metric_type {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram(_) => "histogram",
        };
        output.push_str(&format!("# HELP {} {}\n", metric.name, metric.help));
        output.push_str(&format!("# TYPE {} {}\n", metric.name, type_str));

        match &metric.metric_type {
            MetricType::Counter => {
                output.push_str(&format!("{} {}\n", metric.name, metric.get()));
            }
            MetricType::Gauge => {
                output.push_str(&format!("{} {}\n", metric.name, metric.get_gauge()));
            }
            MetricType::Histogram(buckets) => {
                for bucket in buckets {
                    let bucket_count = metric
                        .observations
                        .iter()
                        .filter(|&&v| v <= *bucket)
                        .count() as u64;
                    output.push_str(&format!(
                        "{}_bucket{{le=\"{}\"}} {}\n",
                        metric.name, bucket, bucket_count
                    ));
                }
                let count = metric.observations.len() as u64;
                let sum: f64 = metric.observations.iter().sum();
                output.push_str(&format!(
                    "{}_bucket{{le=\"+Inf\"}} {}\n",
                    metric.name, count
                ));
                output.push_str(&format!("{}_sum {}\n", metric.name, sum));
                output.push_str(&format!("{}_count {}\n", metric.name, count));
            }
        }
    }
    output
}

// ═══════════════════════════════════════════════════════════════════════
// S30.5: Distributed Tracing
// ═══════════════════════════════════════════════════════════════════════

/// Trace identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TraceId(pub String);

/// Span identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpanId(pub String);

/// A tracing span.
#[derive(Debug, Clone)]
pub struct Span {
    /// Trace ID (shared across all spans in a trace)
    pub trace_id: TraceId,
    /// Unique span ID
    pub span_id: SpanId,
    /// Parent span ID (None for root spans)
    pub parent_span_id: Option<SpanId>,
    /// Operation name
    pub operation: String,
    /// Start time (nanoseconds)
    pub start_ns: u64,
    /// End time (nanoseconds, 0 if not finished)
    pub end_ns: u64,
    /// Span attributes
    pub attributes: HashMap<String, String>,
    /// Status
    pub status: SpanStatus,
}

/// Span status.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span had an error
    Error(String),
    /// Span is still in progress
    InProgress,
}

impl Span {
    /// Create a new root span.
    pub fn new_root(trace_id: &str, span_id: &str, operation: &str) -> Self {
        Self {
            trace_id: TraceId(trace_id.to_string()),
            span_id: SpanId(span_id.to_string()),
            parent_span_id: None,
            operation: operation.to_string(),
            start_ns: 0,
            end_ns: 0,
            attributes: HashMap::new(),
            status: SpanStatus::InProgress,
        }
    }

    /// Create a child span.
    pub fn new_child(parent: &Span, span_id: &str, operation: &str) -> Self {
        Self {
            trace_id: parent.trace_id.clone(),
            span_id: SpanId(span_id.to_string()),
            parent_span_id: Some(parent.span_id.clone()),
            operation: operation.to_string(),
            start_ns: 0,
            end_ns: 0,
            attributes: HashMap::new(),
            status: SpanStatus::InProgress,
        }
    }

    /// Duration in nanoseconds.
    pub fn duration_ns(&self) -> u64 {
        self.end_ns.saturating_sub(self.start_ns)
    }

    /// Finish the span.
    pub fn finish(&mut self, end_ns: u64) {
        self.end_ns = end_ns;
        if self.status == SpanStatus::InProgress {
            self.status = SpanStatus::Ok;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.6: Trace Context Propagation
// ═══════════════════════════════════════════════════════════════════════

/// Trace context for propagation (W3C Trace Context format).
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// Trace ID
    pub trace_id: String,
    /// Parent span ID
    pub span_id: String,
    /// Trace flags (sampled, etc.)
    pub trace_flags: u8,
}

impl TraceContext {
    /// Encode as W3C traceparent header value.
    pub fn encode(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id, self.span_id, self.trace_flags
        )
    }

    /// Decode from W3C traceparent header value.
    pub fn decode(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }
        let trace_flags = u8::from_str_radix(parts[3], 16).ok()?;
        Some(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            trace_flags,
        })
    }

    /// Whether this trace is sampled.
    pub fn is_sampled(&self) -> bool {
        self.trace_flags & 0x01 != 0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.7: Custom Metrics (@metric annotation)
// ═══════════════════════════════════════════════════════════════════════

/// Metric annotation on a function.
#[derive(Debug, Clone)]
pub struct MetricAnnotation {
    /// Metric type (counter, gauge, histogram)
    pub metric_type: MetricType,
    /// Metric name
    pub name: String,
    /// Labels extracted from function args
    pub labels: Vec<String>,
}

/// Parse a metric annotation string.
pub fn parse_metric_annotation(s: &str) -> Option<MetricAnnotation> {
    // Format: "counter, requests_total" or "histogram, request_duration"
    let parts: Vec<&str> = s.splitn(2, ',').map(|p| p.trim()).collect();
    if parts.len() != 2 {
        return None;
    }
    let metric_type = match parts[0] {
        "counter" => MetricType::Counter,
        "gauge" => MetricType::Gauge,
        "histogram" => MetricType::Histogram(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        _ => return None,
    };
    Some(MetricAnnotation {
        metric_type,
        name: parts[1].to_string(),
        labels: Vec::new(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S30.8: Alerting Rules
// ═══════════════════════════════════════════════════════════════════════

/// Alert condition.
#[derive(Debug, Clone)]
pub enum AlertCondition {
    /// Metric exceeds threshold
    GreaterThan(f64),
    /// Metric falls below threshold
    LessThan(f64),
    /// Metric equals value
    Equals(f64),
    /// Metric absent for duration
    Absent,
}

/// Alert rule definition.
#[derive(Debug, Clone)]
pub struct AlertRule {
    /// Alert name
    pub name: String,
    /// Metric to watch
    pub metric_name: String,
    /// Alert condition
    pub condition: AlertCondition,
    /// Duration the condition must hold (seconds)
    pub for_secs: u64,
    /// Severity
    pub severity: AlertSeverity,
    /// Description template
    pub description: String,
}

/// Alert severity.
#[derive(Debug, Clone, PartialEq)]
pub enum AlertSeverity {
    /// Informational
    Info,
    /// Warning — needs attention soon
    Warning,
    /// Critical — needs immediate attention
    Critical,
}

/// Evaluate an alert rule against a metric value.
pub fn evaluate_alert(rule: &AlertRule, value: Option<f64>) -> bool {
    match (&rule.condition, value) {
        (AlertCondition::GreaterThan(threshold), Some(v)) => v > *threshold,
        (AlertCondition::LessThan(threshold), Some(v)) => v < *threshold,
        (AlertCondition::Equals(expected), Some(v)) => (v - expected).abs() < f64::EPSILON,
        (AlertCondition::Absent, None) => true,
        (AlertCondition::Absent, Some(_)) => false,
        (_, None) => false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S30.9: Dashboard Generation
// ═══════════════════════════════════════════════════════════════════════

/// Grafana panel type.
#[derive(Debug, Clone)]
pub enum PanelType {
    /// Line graph
    Graph,
    /// Single stat
    Stat,
    /// Table
    Table,
    /// Heatmap
    Heatmap,
}

/// Dashboard panel.
#[derive(Debug, Clone)]
pub struct DashboardPanel {
    /// Panel title
    pub title: String,
    /// Panel type
    pub panel_type: PanelType,
    /// PromQL queries
    pub queries: Vec<String>,
}

/// Generate a basic Grafana dashboard JSON structure.
pub fn generate_dashboard(title: &str, panels: &[DashboardPanel]) -> String {
    let mut panel_json = Vec::new();
    for (i, panel) in panels.iter().enumerate() {
        let ptype = match &panel.panel_type {
            PanelType::Graph => "graph",
            PanelType::Stat => "stat",
            PanelType::Table => "table",
            PanelType::Heatmap => "heatmap",
        };
        let queries_str: Vec<String> = panel
            .queries
            .iter()
            .map(|q| format!("{{\"expr\":\"{q}\"}}", q = q))
            .collect();
        panel_json.push(format!(
            "{{\"id\":{id},\"title\":\"{title}\",\"type\":\"{ptype}\",\"targets\":[{targets}]}}",
            id = i + 1,
            title = panel.title,
            ptype = ptype,
            targets = queries_str.join(","),
        ));
    }
    format!(
        "{{\"title\":\"{title}\",\"panels\":[{panels}]}}",
        title = title,
        panels = panel_json.join(","),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S30.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s30_1_structured_log_json() {
        let entry = LogEntry::new(LogLevel::Info, "request processed")
            .field("method", "GET")
            .field("status", "200");
        let json = entry.to_json();
        assert!(json.contains("\"level\":\"INFO\""));
        assert!(json.contains("\"method\":\"GET\""));
        assert!(json.contains("\"status\":\"200\""));
    }

    #[test]
    fn s30_2_log_level_filtering() {
        let mut logger = Logger::new(LogLevel::Warn);
        assert!(!logger.log(LogEntry::new(LogLevel::Debug, "debug msg")));
        assert!(logger.log(LogEntry::new(LogLevel::Error, "error msg")));
        assert_eq!(logger.entries.len(), 1);
    }

    #[test]
    fn s30_2_parse_log_level() {
        assert_eq!(parse_log_level("info"), Some(LogLevel::Info));
        assert_eq!(parse_log_level("WARNING"), Some(LogLevel::Warn));
        assert_eq!(parse_log_level("unknown"), None);
    }

    #[test]
    fn s30_3_counter_metric() {
        let metric = Metric::new("requests_total", MetricType::Counter, "Total requests");
        metric.inc();
        metric.inc();
        metric.add(3);
        assert_eq!(metric.get(), 5);
    }

    #[test]
    fn s30_3_gauge_metric() {
        let metric = Metric::new("temperature", MetricType::Gauge, "Current temp");
        metric.set_gauge(23.5);
        assert!((metric.get_gauge() - 23.5).abs() < f64::EPSILON);
    }

    #[test]
    fn s30_3_histogram_metric() {
        let mut metric = Metric::new(
            "latency",
            MetricType::Histogram(vec![0.01, 0.05, 0.1, 0.5, 1.0]),
            "Request latency",
        );
        metric.observe(0.02);
        metric.observe(0.08);
        metric.observe(0.5);
        assert_eq!(metric.observations.len(), 3);
    }

    #[test]
    fn s30_4_prometheus_format() {
        let mut registry = MetricsRegistry::new();
        let idx = registry.register(
            "http_requests_total",
            MetricType::Counter,
            "Total HTTP requests",
        );
        registry.get(idx).unwrap().add(42);
        let output = format_prometheus(&registry);
        assert!(output.contains("# HELP http_requests_total"));
        assert!(output.contains("# TYPE http_requests_total counter"));
        assert!(output.contains("http_requests_total 42"));
    }

    #[test]
    fn s30_5_span_creation() {
        let root = Span::new_root("trace-1", "span-1", "handle_request");
        let child = Span::new_child(&root, "span-2", "query_db");
        assert_eq!(child.trace_id, root.trace_id);
        assert_eq!(child.parent_span_id, Some(SpanId("span-1".into())));
    }

    #[test]
    fn s30_5_span_duration() {
        let mut span = Span::new_root("t1", "s1", "op");
        span.start_ns = 1000;
        span.finish(5000);
        assert_eq!(span.duration_ns(), 4000);
        assert_eq!(span.status, SpanStatus::Ok);
    }

    #[test]
    fn s30_6_trace_context_roundtrip() {
        let ctx = TraceContext {
            trace_id: "abcdef1234567890".into(),
            span_id: "fedcba0987654321".into(),
            trace_flags: 0x01,
        };
        let encoded = ctx.encode();
        assert_eq!(encoded, "00-abcdef1234567890-fedcba0987654321-01");
        let decoded = TraceContext::decode(&encoded).unwrap();
        assert_eq!(decoded.trace_id, ctx.trace_id);
        assert!(decoded.is_sampled());
    }

    #[test]
    fn s30_7_parse_metric_annotation() {
        let ann = parse_metric_annotation("counter, requests_total").unwrap();
        assert_eq!(ann.metric_type, MetricType::Counter);
        assert_eq!(ann.name, "requests_total");

        assert!(parse_metric_annotation("invalid").is_none());
    }

    #[test]
    fn s30_8_alert_evaluation() {
        let rule = AlertRule {
            name: "high_error_rate".into(),
            metric_name: "error_rate".into(),
            condition: AlertCondition::GreaterThan(0.05),
            for_secs: 300,
            severity: AlertSeverity::Critical,
            description: "Error rate too high".into(),
        };
        assert!(evaluate_alert(&rule, Some(0.1)));
        assert!(!evaluate_alert(&rule, Some(0.01)));
    }

    #[test]
    fn s30_8_alert_absent() {
        let rule = AlertRule {
            name: "metric_missing".into(),
            metric_name: "heartbeat".into(),
            condition: AlertCondition::Absent,
            for_secs: 60,
            severity: AlertSeverity::Warning,
            description: "No heartbeat".into(),
        };
        assert!(evaluate_alert(&rule, None));
        assert!(!evaluate_alert(&rule, Some(1.0)));
    }

    #[test]
    fn s30_9_generate_dashboard() {
        let panels = vec![DashboardPanel {
            title: "Request Rate".into(),
            panel_type: PanelType::Graph,
            queries: vec!["rate(http_requests_total[5m])".into()],
        }];
        let json = generate_dashboard("My Service", &panels);
        assert!(json.contains("\"title\":\"My Service\""));
        assert!(json.contains("\"title\":\"Request Rate\""));
        assert!(json.contains("rate(http_requests_total[5m])"));
    }

    #[test]
    fn s30_2_logger_set_level() {
        let mut logger = Logger::new(LogLevel::Error);
        assert!(!logger.log(LogEntry::new(LogLevel::Warn, "w")));
        logger.set_level(LogLevel::Trace);
        assert!(logger.log(LogEntry::new(LogLevel::Trace, "t")));
    }

    #[test]
    fn s30_6_trace_context_not_sampled() {
        let ctx = TraceContext::decode("00-aaa-bbb-00").unwrap();
        assert!(!ctx.is_sampled());
    }
}
