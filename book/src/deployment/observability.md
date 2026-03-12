# Observability

Fajar Lang provides built-in logging, metrics, and tracing for production monitoring.

## Structured Logging

```fajar
use deployment::logging

let logger = Logger::new(LogLevel::Info)

logger.info("Server started", fields: {"port": 8080, "version": "3.0.0"})
logger.warn("High memory usage", fields: {"usage_mb": 450, "limit_mb": 512})
logger.error("Request failed", fields: {"path": "/api/data", "status": 500})
```

Output (JSON):
```json
{"timestamp":"2026-03-12T10:30:00Z","level":"info","message":"Server started","port":8080,"version":"3.0.0"}
```

## Metrics (Prometheus)

```fajar
use deployment::metrics

let registry = MetricsRegistry::new()

// Counter — monotonically increasing
let requests = registry.counter("http_requests_total")
requests.increment()

// Gauge — current value
let connections = registry.gauge("active_connections")
connections.set(42)

// Histogram — distribution
let latency = registry.histogram("request_duration_seconds")
latency.observe(0.045)
```

Expose at `/metrics` endpoint in Prometheus text format:
```
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total 1234

# HELP active_connections Current connections
# TYPE active_connections gauge
active_connections 42
```

## Distributed Tracing

W3C Trace Context compatible:

```fajar
use deployment::tracing

let ctx = TraceContext::new()
let span = ctx.start_span("handle_request")

// Propagate trace via W3C traceparent header
let traceparent = ctx.traceparent()
// "00-<trace_id>-<span_id>-01"

span.end(SpanStatus::Ok)
```

## Alerting

```fajar
let alert = AlertRule {
    name: "high_error_rate",
    condition: AlertCondition::GreaterThan {
        metric: "error_rate",
        threshold: 0.05,
    },
    duration_seconds: 300,  // Must exceed for 5 minutes
    severity: "critical",
}
```

## Grafana Dashboards

```bash
fj deploy --dashboard
```

Generates a Grafana dashboard JSON with panels for request rate, error rate, latency percentiles, and resource usage.
