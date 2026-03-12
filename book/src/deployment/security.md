# Production Security

Security features for deploying Fajar Lang services to production.

## TLS

```fajar
use deployment::security

let tls = TlsConfig {
    cert_path: "/etc/ssl/cert.pem",
    key_path: "/etc/ssl/key.pem",
    min_version: TlsVersion::V1_2,
    cipher_suites: [
        "TLS_AES_256_GCM_SHA384",
        "TLS_CHACHA20_POLY1305_SHA256",
    ],
}
```

ACME (Let's Encrypt) support for automatic certificate provisioning.

## JWT Authentication

```fajar
let jwt_config = JwtConfig {
    secret: env("JWT_SECRET"),
    issuer: "fajar-service",
    audience: "api-users",
    expiry_seconds: 3600,
}

// Validate incoming token
match parse_jwt(token, jwt_config) {
    Ok(claims) => {
        println(f"User: {claims.subject}")
        proceed(claims)
    },
    Err(e) => reject(401, f"Unauthorized: {e}"),
}
```

## Rate Limiting

Token bucket per-client rate limiting:

```fajar
let limiter = RateLimiter::new(
    max_tokens: 100,       // Burst capacity
    refill_rate: 10.0,     // 10 requests/second
)

if limiter.allow(client_ip) {
    handle_request()
} else {
    respond(429, "Too Many Requests")
}
```

## CORS

```fajar
// Strict (production)
let cors = CorsConfig::strict(
    allowed_origins: ["https://app.example.com"],
    allowed_methods: ["GET", "POST"],
)

// Permissive (development)
let cors = CorsConfig::permissive()
```

## Input Sanitization

```fajar
use deployment::security

// Automatically sanitize user input
let safe_html = sanitize_html(user_input)  // Strips <script>, onclick, etc.
let safe_sql = sanitize_sql(user_input)    // Escapes quotes

// Detect injection attempts
if detect_injection(input) {
    audit_log.record(AuditEvent::InjectionAttempt, client_ip)
    reject(400, "Invalid input")
}
```

Detection covers: SQL injection, XSS, command injection, path traversal.

## Secret Management

```fajar
let secrets = SecretStore::new(encryption_key: env("MASTER_KEY"))

secrets.store("db_password", "s3cret")
let password = secrets.retrieve("db_password")  // Decrypted at runtime
```

Secrets are encrypted at rest.

## Audit Logging

```fajar
let audit = AuditLog::new()

audit.record(AuditEvent::Login { user: "alice", success: true })
audit.record(AuditEvent::DataAccess { user: "alice", resource: "users/123" })

// Query audit events
let failed_logins = audit.query(EventType::Login, success: false, last_hours: 24)
```

## Dependency Auditing

```bash
fj audit
```

Scans dependencies against CVE databases and reports:
- Known vulnerabilities with severity levels
- License compliance
- SBOM (Software Bill of Materials) generation
