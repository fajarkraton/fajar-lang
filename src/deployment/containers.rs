//! Container Support — Dockerfile generation, minimal base images,
//! static linking, 12-factor config, Docker Compose, health checks,
//! resource limits, Kubernetes manifests, Helm charts.

// ═══════════════════════════════════════════════════════════════════════
// S29.1: Dockerfile Generation
// ═══════════════════════════════════════════════════════════════════════

/// Base image strategy for container builds.
#[derive(Debug, Clone, PartialEq)]
pub enum BaseImage {
    /// FROM scratch — smallest possible, no OS
    Scratch,
    /// Distroless — Google's minimal runtime images
    Distroless,
    /// Alpine — small Linux distribution (~5MB)
    Alpine,
    /// Debian slim — standard but trimmed
    DebianSlim,
    /// Custom base image
    Custom(String),
}

impl std::fmt::Display for BaseImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaseImage::Scratch => write!(f, "scratch"),
            BaseImage::Distroless => write!(f, "gcr.io/distroless/static-debian12:nonroot"),
            BaseImage::Alpine => write!(f, "alpine:3.19"),
            BaseImage::DebianSlim => write!(f, "debian:bookworm-slim"),
            BaseImage::Custom(img) => write!(f, "{img}"),
        }
    }
}

/// Configuration for Dockerfile generation.
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Application binary name
    pub binary_name: String,
    /// Base image for runtime stage
    pub base_image: BaseImage,
    /// Whether to use multi-stage build
    pub multi_stage: bool,
    /// Whether to use static linking (musl)
    pub static_link: bool,
    /// Exposed ports
    pub ports: Vec<u16>,
    /// Environment variables
    pub env_vars: Vec<(String, String)>,
    /// Health check endpoint
    pub health_check: Option<String>,
    /// Extra COPY instructions
    pub extra_copies: Vec<(String, String)>,
    /// Labels
    pub labels: Vec<(String, String)>,
}

impl DockerConfig {
    /// Create a new Docker configuration with defaults.
    pub fn new(binary_name: &str) -> Self {
        Self {
            binary_name: binary_name.to_string(),
            base_image: BaseImage::Distroless,
            multi_stage: true,
            static_link: true,
            ports: vec![8080],
            env_vars: Vec::new(),
            health_check: None,
            extra_copies: Vec::new(),
            labels: Vec::new(),
        }
    }
}

/// Generate a Dockerfile from configuration.
pub fn generate_dockerfile(config: &DockerConfig) -> String {
    let mut lines = Vec::new();

    if config.multi_stage {
        lines.push("# Build stage".to_string());
        lines.push("FROM rust:1.77-slim AS builder".to_string());
        lines.push("WORKDIR /build".to_string());
        lines.push("COPY . .".to_string());

        if config.static_link {
            lines.push("RUN apt-get update && apt-get install -y musl-tools".to_string());
            lines.push("RUN rustup target add x86_64-unknown-linux-musl".to_string());
            lines.push(
                "RUN cargo build --release --target x86_64-unknown-linux-musl && \\".to_string(),
            );
            lines.push(format!(
                "    cp target/x86_64-unknown-linux-musl/release/{} /build/app",
                config.binary_name
            ));
        } else {
            lines.push("RUN cargo build --release".to_string());
            lines.push(format!(
                "RUN cp target/release/{} /build/app",
                config.binary_name
            ));
        }

        lines.push(String::new());
        lines.push("# Runtime stage".to_string());
        lines.push(format!("FROM {}", config.base_image));
    } else {
        lines.push(format!("FROM {}", config.base_image));
    }

    // Labels
    for (key, value) in &config.labels {
        lines.push(format!("LABEL {key}=\"{value}\""));
    }

    lines.push("WORKDIR /app".to_string());

    if config.multi_stage {
        lines.push("COPY --from=builder /build/app /app/app".to_string());
    }

    for (src, dst) in &config.extra_copies {
        lines.push(format!("COPY {src} {dst}"));
    }

    for (key, value) in &config.env_vars {
        lines.push(format!("ENV {key}={value}"));
    }

    for port in &config.ports {
        lines.push(format!("EXPOSE {port}"));
    }

    if let Some(health) = &config.health_check {
        lines.push(format!(
            "HEALTHCHECK --interval=30s --timeout=3s --retries=3 CMD wget -qO- {health} || exit 1"
        ));
    }

    lines.push("ENTRYPOINT [\"/app/app\"]".to_string());

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S29.2: Minimal Base Image
// ═══════════════════════════════════════════════════════════════════════

/// Estimated image size for a base image.
pub fn estimated_image_size(base: &BaseImage) -> u64 {
    match base {
        BaseImage::Scratch => 0,
        BaseImage::Distroless => 2_500_000,
        BaseImage::Alpine => 5_500_000,
        BaseImage::DebianSlim => 80_000_000,
        BaseImage::Custom(_) => 100_000_000,
    }
}

/// Recommend a base image for a given binary size and requirements.
pub fn recommend_base_image(needs_shell: bool, needs_libc: bool) -> BaseImage {
    if !needs_shell && !needs_libc {
        BaseImage::Scratch
    } else if !needs_shell {
        BaseImage::Distroless
    } else {
        BaseImage::Alpine
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.3: Static Linking
// ═══════════════════════════════════════════════════════════════════════

/// Linker configuration for static builds.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkMode {
    /// Dynamic linking (default)
    Dynamic,
    /// Static linking with musl libc
    StaticMusl,
    /// Static linking with glibc (partial)
    StaticGlibc,
}

/// Build target triple for static linking.
pub fn static_target_triple(arch: &str, link_mode: &LinkMode) -> String {
    match link_mode {
        LinkMode::Dynamic => format!("{arch}-unknown-linux-gnu"),
        LinkMode::StaticMusl => format!("{arch}-unknown-linux-musl"),
        LinkMode::StaticGlibc => format!("{arch}-unknown-linux-gnu"),
    }
}

/// Rustflags needed for static linking.
pub fn static_rustflags(link_mode: &LinkMode) -> Vec<String> {
    match link_mode {
        LinkMode::Dynamic => vec![],
        LinkMode::StaticMusl => vec!["-C".into(), "target-feature=+crt-static".into()],
        LinkMode::StaticGlibc => {
            vec![
                "-C".into(),
                "target-feature=+crt-static".into(),
                "-C".into(),
                "link-arg=-static".into(),
            ]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.4: Container Config (12-Factor)
// ═══════════════════════════════════════════════════════════════════════

/// 12-factor app configuration source.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    /// Environment variable
    EnvVar(String),
    /// Configuration file (TOML/JSON/YAML)
    File(String),
    /// Command-line argument
    CliArg(String),
    /// Default value
    Default(String),
}

/// Configuration entry with precedence.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    /// Configuration key
    pub key: String,
    /// Description
    pub description: String,
    /// Sources in priority order (first match wins)
    pub sources: Vec<ConfigSource>,
    /// Whether this config is required
    pub required: bool,
}

/// Resolve a configuration value from sources.
pub fn resolve_config(entry: &ConfigEntry, env: &[(String, String)]) -> Option<String> {
    for source in &entry.sources {
        match source {
            ConfigSource::EnvVar(name) => {
                if let Some((_, val)) = env.iter().find(|(k, _)| k == name) {
                    return Some(val.clone());
                }
            }
            ConfigSource::Default(val) => return Some(val.clone()),
            ConfigSource::File(_) | ConfigSource::CliArg(_) => {
                // File/CLI resolution would need I/O — return None to fall through
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// S29.5: Docker Compose
// ═══════════════════════════════════════════════════════════════════════

/// Service definition for Docker Compose.
#[derive(Debug, Clone)]
pub struct ComposeService {
    /// Service name
    pub name: String,
    /// Image or build context
    pub image: Option<String>,
    /// Build context path
    pub build: Option<String>,
    /// Port mappings (host:container)
    pub ports: Vec<(u16, u16)>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Dependencies on other services
    pub depends_on: Vec<String>,
    /// Volumes
    pub volumes: Vec<(String, String)>,
    /// Restart policy
    pub restart: String,
}

/// Docker Compose project.
#[derive(Debug, Clone)]
pub struct ComposeProject {
    /// Project services
    pub services: Vec<ComposeService>,
    /// Named volumes
    pub volumes: Vec<String>,
    /// Named networks
    pub networks: Vec<String>,
}

/// Generate docker-compose.yml content.
pub fn generate_compose(project: &ComposeProject) -> String {
    let mut lines = vec!["version: \"3.8\"".to_string(), String::new()];

    lines.push("services:".to_string());
    for svc in &project.services {
        lines.push(format!("  {}:", svc.name));

        if let Some(image) = &svc.image {
            lines.push(format!("    image: {image}"));
        }
        if let Some(build) = &svc.build {
            lines.push(format!("    build: {build}"));
        }

        if !svc.ports.is_empty() {
            lines.push("    ports:".to_string());
            for (host, container) in &svc.ports {
                lines.push(format!("      - \"{host}:{container}\""));
            }
        }

        if !svc.env.is_empty() {
            lines.push("    environment:".to_string());
            for (key, val) in &svc.env {
                lines.push(format!("      - {key}={val}"));
            }
        }

        if !svc.depends_on.is_empty() {
            lines.push("    depends_on:".to_string());
            for dep in &svc.depends_on {
                lines.push(format!("      - {dep}"));
            }
        }

        if !svc.volumes.is_empty() {
            lines.push("    volumes:".to_string());
            for (src, dst) in &svc.volumes {
                lines.push(format!("      - {src}:{dst}"));
            }
        }

        lines.push(format!("    restart: {}", svc.restart));
    }

    if !project.volumes.is_empty() {
        lines.push(String::new());
        lines.push("volumes:".to_string());
        for vol in &project.volumes {
            lines.push(format!("  {vol}:"));
        }
    }

    if !project.networks.is_empty() {
        lines.push(String::new());
        lines.push("networks:".to_string());
        for net in &project.networks {
            lines.push(format!("  {net}:"));
        }
    }

    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S29.6: Health Check
// ═══════════════════════════════════════════════════════════════════════

/// Health check status.
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded but functional
    Degraded(String),
    /// Service is unhealthy
    Unhealthy(String),
}

/// Health check component.
#[derive(Debug, Clone)]
pub struct HealthComponent {
    /// Component name
    pub name: String,
    /// Component status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub latency_ms: u64,
}

/// Aggregate health check result.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Overall status
    pub status: HealthStatus,
    /// Individual component statuses
    pub components: Vec<HealthComponent>,
    /// Server uptime in seconds
    pub uptime_secs: u64,
}

impl HealthReport {
    /// Create a health report from component checks.
    pub fn from_components(components: Vec<HealthComponent>, uptime_secs: u64) -> Self {
        let status = if components
            .iter()
            .any(|c| matches!(c.status, HealthStatus::Unhealthy(_)))
        {
            HealthStatus::Unhealthy("one or more components unhealthy".into())
        } else if components
            .iter()
            .any(|c| matches!(c.status, HealthStatus::Degraded(_)))
        {
            HealthStatus::Degraded("one or more components degraded".into())
        } else {
            HealthStatus::Healthy
        };
        Self {
            status,
            components,
            uptime_secs,
        }
    }

    /// Whether the service is considered ready for traffic.
    pub fn is_ready(&self) -> bool {
        !matches!(self.status, HealthStatus::Unhealthy(_))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.7: Resource Limits
// ═══════════════════════════════════════════════════════════════════════

/// Detected resource limits from cgroup.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Memory limit in bytes (0 = unlimited)
    pub memory_bytes: u64,
    /// CPU quota (millicores, e.g., 2000 = 2 cores)
    pub cpu_millicores: u32,
    /// Whether running inside a container
    pub containerized: bool,
}

impl ResourceLimits {
    /// Detect resource limits (simulated for now).
    pub fn detect() -> Self {
        Self {
            memory_bytes: 0,
            cpu_millicores: 0,
            containerized: false,
        }
    }

    /// Recommended thread pool size based on CPU limits.
    pub fn recommended_threads(&self) -> usize {
        if self.cpu_millicores > 0 {
            (self.cpu_millicores as usize / 1000).max(1)
        } else {
            4
        }
    }

    /// Recommended buffer size based on memory limits.
    pub fn recommended_buffer_bytes(&self) -> u64 {
        if self.memory_bytes > 0 {
            // Use ~10% of available memory for buffers
            self.memory_bytes / 10
        } else {
            64 * 1024 * 1024 // 64MB default
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S29.8: Kubernetes Manifest
// ═══════════════════════════════════════════════════════════════════════

/// Kubernetes deployment configuration.
#[derive(Debug, Clone)]
pub struct K8sDeployment {
    /// Application name
    pub name: String,
    /// Container image
    pub image: String,
    /// Number of replicas
    pub replicas: u32,
    /// Container port
    pub port: u16,
    /// CPU request (millicores)
    pub cpu_request: u32,
    /// Memory request (Mi)
    pub mem_request_mi: u32,
    /// CPU limit (millicores)
    pub cpu_limit: u32,
    /// Memory limit (Mi)
    pub mem_limit_mi: u32,
    /// Health check path
    pub health_path: String,
    /// Namespace
    pub namespace: String,
}

impl K8sDeployment {
    /// Create with defaults.
    pub fn new(name: &str, image: &str) -> Self {
        Self {
            name: name.to_string(),
            image: image.to_string(),
            replicas: 2,
            port: 8080,
            cpu_request: 100,
            mem_request_mi: 128,
            cpu_limit: 500,
            mem_limit_mi: 256,
            health_path: "/health".to_string(),
            namespace: "default".to_string(),
        }
    }
}

/// Generate Kubernetes Deployment + Service YAML.
pub fn generate_k8s_manifest(deploy: &K8sDeployment) -> String {
    format!(
        r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: {name}
  namespace: {ns}
spec:
  replicas: {replicas}
  selector:
    matchLabels:
      app: {name}
  template:
    metadata:
      labels:
        app: {name}
    spec:
      containers:
        - name: {name}
          image: {image}
          ports:
            - containerPort: {port}
          resources:
            requests:
              cpu: "{cpu_req}m"
              memory: "{mem_req}Mi"
            limits:
              cpu: "{cpu_lim}m"
              memory: "{mem_lim}Mi"
          livenessProbe:
            httpGet:
              path: {health}
              port: {port}
            initialDelaySeconds: 10
            periodSeconds: 30
          readinessProbe:
            httpGet:
              path: {health}
              port: {port}
            initialDelaySeconds: 5
            periodSeconds: 10
---
apiVersion: v1
kind: Service
metadata:
  name: {name}
  namespace: {ns}
spec:
  selector:
    app: {name}
  ports:
    - port: 80
      targetPort: {port}
  type: ClusterIP"#,
        name = deploy.name,
        ns = deploy.namespace,
        replicas = deploy.replicas,
        image = deploy.image,
        port = deploy.port,
        cpu_req = deploy.cpu_request,
        mem_req = deploy.mem_request_mi,
        cpu_lim = deploy.cpu_limit,
        mem_lim = deploy.mem_limit_mi,
        health = deploy.health_path,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S29.9: Helm Chart
// ═══════════════════════════════════════════════════════════════════════

/// Helm chart metadata.
#[derive(Debug, Clone)]
pub struct HelmChart {
    /// Chart name
    pub name: String,
    /// Chart version
    pub version: String,
    /// App version
    pub app_version: String,
    /// Description
    pub description: String,
    /// Default values
    pub values: Vec<(String, String)>,
}

/// Generate Helm Chart.yaml content.
pub fn generate_chart_yaml(chart: &HelmChart) -> String {
    format!(
        r#"apiVersion: v2
name: {}
description: {}
type: application
version: {}
appVersion: {}"#,
        chart.name, chart.description, chart.version, chart.app_version,
    )
}

/// Generate Helm values.yaml content.
pub fn generate_values_yaml(chart: &HelmChart) -> String {
    let mut lines = Vec::new();
    for (key, value) in &chart.values {
        lines.push(format!("{key}: {value}"));
    }
    lines.join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// S29.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s29_1_generate_dockerfile_multistage() {
        let config = DockerConfig::new("myapp");
        let dockerfile = generate_dockerfile(&config);
        assert!(dockerfile.contains("FROM rust:1.77-slim AS builder"));
        assert!(dockerfile.contains("FROM gcr.io/distroless/static-debian12:nonroot"));
        assert!(dockerfile.contains("COPY --from=builder"));
        assert!(dockerfile.contains("ENTRYPOINT [\"/app/app\"]"));
    }

    #[test]
    fn s29_1_dockerfile_with_ports_and_env() {
        let mut config = DockerConfig::new("api");
        config.ports = vec![8080, 9090];
        config.env_vars = vec![("ENV".into(), "production".into())];
        let dockerfile = generate_dockerfile(&config);
        assert!(dockerfile.contains("EXPOSE 8080"));
        assert!(dockerfile.contains("EXPOSE 9090"));
        assert!(dockerfile.contains("ENV ENV=production"));
    }

    #[test]
    fn s29_2_minimal_base_images() {
        assert_eq!(estimated_image_size(&BaseImage::Scratch), 0);
        assert!(
            estimated_image_size(&BaseImage::Alpine) > estimated_image_size(&BaseImage::Distroless)
        );
        assert!(
            estimated_image_size(&BaseImage::DebianSlim) > estimated_image_size(&BaseImage::Alpine)
        );
    }

    #[test]
    fn s29_2_recommend_scratch() {
        let img = recommend_base_image(false, false);
        assert_eq!(img, BaseImage::Scratch);
    }

    #[test]
    fn s29_3_static_target_musl() {
        let triple = static_target_triple("x86_64", &LinkMode::StaticMusl);
        assert_eq!(triple, "x86_64-unknown-linux-musl");
    }

    #[test]
    fn s29_3_static_rustflags() {
        let flags = static_rustflags(&LinkMode::StaticMusl);
        assert!(flags.contains(&"target-feature=+crt-static".to_string()));
        assert!(static_rustflags(&LinkMode::Dynamic).is_empty());
    }

    #[test]
    fn s29_4_resolve_env_config() {
        let entry = ConfigEntry {
            key: "PORT".into(),
            description: "Server port".into(),
            sources: vec![
                ConfigSource::EnvVar("PORT".into()),
                ConfigSource::Default("8080".into()),
            ],
            required: true,
        };
        let env = vec![("PORT".into(), "3000".into())];
        assert_eq!(resolve_config(&entry, &env), Some("3000".into()));
    }

    #[test]
    fn s29_4_resolve_default_config() {
        let entry = ConfigEntry {
            key: "PORT".into(),
            description: "Server port".into(),
            sources: vec![
                ConfigSource::EnvVar("PORT".into()),
                ConfigSource::Default("8080".into()),
            ],
            required: false,
        };
        let env: Vec<(String, String)> = vec![];
        assert_eq!(resolve_config(&entry, &env), Some("8080".into()));
    }

    #[test]
    fn s29_5_generate_compose() {
        let project = ComposeProject {
            services: vec![ComposeService {
                name: "web".into(),
                image: None,
                build: Some(".".into()),
                ports: vec![(8080, 8080)],
                env: vec![("ENV".into(), "prod".into())],
                depends_on: vec!["db".into()],
                volumes: vec![],
                restart: "unless-stopped".into(),
            }],
            volumes: vec!["pgdata".into()],
            networks: vec![],
        };
        let yaml = generate_compose(&project);
        assert!(yaml.contains("web:"));
        assert!(yaml.contains("build: ."));
        assert!(yaml.contains("\"8080:8080\""));
        assert!(yaml.contains("depends_on:"));
        assert!(yaml.contains("pgdata:"));
    }

    #[test]
    fn s29_6_health_report_healthy() {
        let components = vec![HealthComponent {
            name: "db".into(),
            status: HealthStatus::Healthy,
            latency_ms: 5,
        }];
        let report = HealthReport::from_components(components, 3600);
        assert_eq!(report.status, HealthStatus::Healthy);
        assert!(report.is_ready());
    }

    #[test]
    fn s29_6_health_report_unhealthy() {
        let components = vec![HealthComponent {
            name: "db".into(),
            status: HealthStatus::Unhealthy("connection refused".into()),
            latency_ms: 0,
        }];
        let report = HealthReport::from_components(components, 100);
        assert!(!report.is_ready());
    }

    #[test]
    fn s29_7_resource_limits_threads() {
        let limits = ResourceLimits {
            memory_bytes: 512 * 1024 * 1024,
            cpu_millicores: 2000,
            containerized: true,
        };
        assert_eq!(limits.recommended_threads(), 2);
    }

    #[test]
    fn s29_7_resource_limits_buffer() {
        let limits = ResourceLimits {
            memory_bytes: 1024 * 1024 * 1024, // 1GB
            cpu_millicores: 4000,
            containerized: true,
        };
        assert_eq!(limits.recommended_buffer_bytes(), 1024 * 1024 * 1024 / 10);
    }

    #[test]
    fn s29_8_k8s_manifest() {
        let deploy = K8sDeployment::new("myapp", "myapp:latest");
        let yaml = generate_k8s_manifest(&deploy);
        assert!(yaml.contains("kind: Deployment"));
        assert!(yaml.contains("kind: Service"));
        assert!(yaml.contains("image: myapp:latest"));
        assert!(yaml.contains("replicas: 2"));
        assert!(yaml.contains("containerPort: 8080"));
        assert!(yaml.contains("/health"));
    }

    #[test]
    fn s29_9_helm_chart_yaml() {
        let chart = HelmChart {
            name: "myapp".into(),
            version: "0.1.0".into(),
            app_version: "1.0.0".into(),
            description: "My Fajar App".into(),
            values: vec![("replicaCount".into(), "2".into())],
        };
        let yaml = generate_chart_yaml(&chart);
        assert!(yaml.contains("name: myapp"));
        assert!(yaml.contains("version: 0.1.0"));
        let vals = generate_values_yaml(&chart);
        assert!(vals.contains("replicaCount: 2"));
    }

    #[test]
    fn s29_6_healthcheck_in_dockerfile() {
        let mut config = DockerConfig::new("api");
        config.health_check = Some("http://localhost:8080/health".into());
        let dockerfile = generate_dockerfile(&config);
        assert!(dockerfile.contains("HEALTHCHECK"));
        assert!(dockerfile.contains("http://localhost:8080/health"));
    }

    #[test]
    fn s29_2_base_image_display() {
        assert_eq!(BaseImage::Scratch.to_string(), "scratch");
        assert!(BaseImage::Distroless.to_string().contains("distroless"));
        assert_eq!(BaseImage::Custom("myimg:v1".into()).to_string(), "myimg:v1");
    }
}
