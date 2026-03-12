# Container Deployment

Fajar Lang generates production-ready container configurations.

## Dockerfile Generation

```bash
fj deploy --dockerfile
```

Generates a multi-stage Dockerfile:

```dockerfile
# Build stage
FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime stage (minimal image)
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/myapp /
CMD ["/myapp"]
```

### Base Images

| Image | Size | Use Case |
|-------|------|----------|
| `scratch` | 0 | Static binary, no OS |
| `distroless` | ~2MB | Minimal, no shell |
| `alpine` | ~5MB | Small with package manager |
| `debian-slim` | ~25MB | Full libc, debugging tools |

## Docker Compose

```bash
fj deploy --compose
```

Generates `docker-compose.yml` with your app, dependencies, and health checks.

## Kubernetes

```bash
fj deploy --k8s
```

Generates Kubernetes manifests:
- **Deployment** — replica count, resource limits, health probes
- **Service** — load balancer, port mapping
- **ConfigMap** — environment configuration

## Helm Charts

```bash
fj deploy --helm
```

Generates a Helm chart with `Chart.yaml`, `values.yaml`, and templates.

## Static Linking

For minimal container images, compile with static linking:

```bash
fj build --target x86_64-unknown-linux-musl --release
```

Produces a fully static binary that runs in `scratch` containers.

## Health Checks

```fajar
fn health_check() -> HealthReport {
    HealthReport {
        status: Healthy,
        components: [
            HealthComponent { name: "database", status: check_db() },
            HealthComponent { name: "cache", status: check_cache() },
        ]
    }
}
```

## 12-Factor Config

Environment-based configuration following the 12-factor methodology:

```fajar
let port = config::resolve("PORT", default: "8080")
let db_url = config::resolve("DATABASE_URL", required: true)
```

Resolution order: environment variable → config file → default value.
