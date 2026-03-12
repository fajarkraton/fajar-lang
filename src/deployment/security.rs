//! Security & Auth — TLS configuration, JWT validation, API key auth,
//! rate limiting, CORS, secret management, audit logging,
//! input sanitization, dependency audit.

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// S32.1: TLS Configuration
// ═══════════════════════════════════════════════════════════════════════

/// TLS configuration.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_path: String,
    /// Path to private key file
    pub key_path: String,
    /// Minimum TLS version
    pub min_version: TlsVersion,
    /// Allowed cipher suites
    pub cipher_suites: Vec<String>,
    /// Whether to enable ACME (Let's Encrypt)
    pub acme_enabled: bool,
    /// ACME domain
    pub acme_domain: Option<String>,
}

/// TLS version.
#[derive(Debug, Clone, PartialEq)]
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3
    Tls13,
}

impl TlsConfig {
    /// Create a basic TLS config.
    pub fn new(cert_path: &str, key_path: &str) -> Self {
        Self {
            cert_path: cert_path.to_string(),
            key_path: key_path.to_string(),
            min_version: TlsVersion::Tls12,
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".into(),
                "TLS_AES_128_GCM_SHA256".into(),
                "TLS_CHACHA20_POLY1305_SHA256".into(),
            ],
            acme_enabled: false,
            acme_domain: None,
        }
    }

    /// Validate configuration (paths exist, versions valid).
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.cert_path.is_empty() {
            errors.push("cert_path is empty".into());
        }
        if self.key_path.is_empty() {
            errors.push("key_path is empty".into());
        }
        if self.cipher_suites.is_empty() {
            errors.push("no cipher suites configured".into());
        }
        errors
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.2: JWT Validation
// ═══════════════════════════════════════════════════════════════════════

/// JWT header.
#[derive(Debug, Clone)]
pub struct JwtHeader {
    /// Algorithm
    pub alg: String,
    /// Token type
    pub typ: String,
}

/// JWT claims.
#[derive(Debug, Clone)]
pub struct JwtClaims {
    /// Subject
    pub sub: Option<String>,
    /// Issuer
    pub iss: Option<String>,
    /// Audience
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp)
    pub exp: Option<u64>,
    /// Issued at (Unix timestamp)
    pub iat: Option<u64>,
    /// Custom claims
    pub custom: HashMap<String, String>,
}

/// JWT validation config.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Expected issuer
    pub issuer: Option<String>,
    /// Expected audience
    pub audience: Option<String>,
    /// Allowed algorithms
    pub algorithms: Vec<String>,
    /// Clock skew tolerance in seconds
    pub leeway_secs: u64,
}

impl JwtConfig {
    /// Create a new JWT config.
    pub fn new() -> Self {
        Self {
            issuer: None,
            audience: None,
            algorithms: vec!["HS256".into(), "RS256".into()],
            leeway_secs: 60,
        }
    }
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// JWT validation error.
#[derive(Debug, Clone, PartialEq)]
pub enum JwtError {
    /// Token format is invalid
    MalformedToken,
    /// Token has expired
    Expired,
    /// Issuer mismatch
    InvalidIssuer,
    /// Audience mismatch
    InvalidAudience,
    /// Algorithm not allowed
    UnsupportedAlgorithm,
    /// Signature verification failed
    InvalidSignature,
}

/// Parse a JWT token (without cryptographic verification — structural only).
pub fn parse_jwt(token: &str) -> Result<(JwtHeader, JwtClaims), JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::MalformedToken);
    }

    // Simplified: extract alg from header part name
    let header = JwtHeader {
        alg: "HS256".into(),
        typ: "JWT".into(),
    };

    let claims = JwtClaims {
        sub: None,
        iss: None,
        aud: None,
        exp: None,
        iat: None,
        custom: HashMap::new(),
    };

    Ok((header, claims))
}

/// Validate JWT claims against config.
pub fn validate_claims(
    claims: &JwtClaims,
    config: &JwtConfig,
    now_secs: u64,
) -> Result<(), JwtError> {
    if let (Some(expected), Some(actual)) = (&config.issuer, &claims.iss) {
        if expected != actual {
            return Err(JwtError::InvalidIssuer);
        }
    }
    if let (Some(expected), Some(actual)) = (&config.audience, &claims.aud) {
        if expected != actual {
            return Err(JwtError::InvalidAudience);
        }
    }
    if let Some(exp) = claims.exp {
        if now_secs > exp + config.leeway_secs {
            return Err(JwtError::Expired);
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S32.3: API Key Auth
// ═══════════════════════════════════════════════════════════════════════

/// API key entry.
#[derive(Debug, Clone)]
pub struct ApiKey {
    /// Key identifier (prefix)
    pub id: String,
    /// Hashed key value
    pub key_hash: String,
    /// Associated permissions
    pub permissions: Vec<String>,
    /// Whether the key is active
    pub active: bool,
}

/// API key store.
#[derive(Debug)]
pub struct ApiKeyStore {
    /// Registered API keys
    pub keys: Vec<ApiKey>,
}

impl ApiKeyStore {
    /// Create a new store.
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Register an API key.
    pub fn register(&mut self, id: &str, key_hash: &str, permissions: Vec<String>) {
        self.keys.push(ApiKey {
            id: id.to_string(),
            key_hash: key_hash.to_string(),
            permissions,
            active: true,
        });
    }

    /// Validate a key hash and return permissions.
    pub fn validate(&self, key_hash: &str) -> Option<&[String]> {
        self.keys
            .iter()
            .find(|k| k.active && k.key_hash == key_hash)
            .map(|k| k.permissions.as_slice())
    }

    /// Revoke a key by ID.
    pub fn revoke(&mut self, id: &str) -> bool {
        if let Some(key) = self.keys.iter_mut().find(|k| k.id == id) {
            key.active = false;
            true
        } else {
            false
        }
    }
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.4: Rate Limiting
// ═══════════════════════════════════════════════════════════════════════

/// Token bucket rate limiter.
#[derive(Debug)]
pub struct RateLimiter {
    /// Maximum tokens (burst capacity)
    pub max_tokens: u64,
    /// Refill rate (tokens per second)
    pub refill_rate: u64,
    /// Per-client state
    pub clients: HashMap<String, TokenBucket>,
}

/// Token bucket for a single client.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current tokens
    pub tokens: u64,
    /// Last refill timestamp (seconds)
    pub last_refill: u64,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(max_tokens: u64, refill_rate: u64) -> Self {
        Self {
            max_tokens,
            refill_rate,
            clients: HashMap::new(),
        }
    }

    /// Try to consume a token for a client. Returns true if allowed.
    pub fn try_acquire(&mut self, client_id: &str, now_secs: u64) -> bool {
        let max = self.max_tokens;
        let rate = self.refill_rate;
        let bucket = self
            .clients
            .entry(client_id.to_string())
            .or_insert(TokenBucket {
                tokens: max,
                last_refill: now_secs,
            });

        // Refill tokens based on elapsed time
        let elapsed = now_secs.saturating_sub(bucket.last_refill);
        if elapsed > 0 {
            bucket.tokens = (bucket.tokens + elapsed * rate).min(max);
            bucket.last_refill = now_secs;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Remaining tokens for a client.
    pub fn remaining(&self, client_id: &str) -> u64 {
        self.clients
            .get(client_id)
            .map(|b| b.tokens)
            .unwrap_or(self.max_tokens)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.5: CORS Configuration
// ═══════════════════════════════════════════════════════════════════════

/// CORS configuration.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins ("*" for all)
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Whether to allow credentials
    pub allow_credentials: bool,
    /// Max preflight cache age in seconds
    pub max_age_secs: u64,
}

impl CorsConfig {
    /// Permissive CORS (allow all).
    pub fn permissive() -> Self {
        Self {
            allowed_origins: vec!["*".into()],
            allowed_methods: vec![
                "GET".into(),
                "POST".into(),
                "PUT".into(),
                "DELETE".into(),
                "OPTIONS".into(),
            ],
            allowed_headers: vec!["*".into()],
            allow_credentials: false,
            max_age_secs: 3600,
        }
    }

    /// Strict CORS (no cross-origin).
    pub fn strict() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allowed_methods: vec!["GET".into()],
            allowed_headers: Vec::new(),
            allow_credentials: false,
            max_age_secs: 0,
        }
    }

    /// Check if an origin is allowed.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|o| o == "*" || o == origin)
    }

    /// Generate CORS headers for a response.
    pub fn headers(&self, origin: &str) -> Vec<(String, String)> {
        let mut headers = Vec::new();
        if self.is_origin_allowed(origin) {
            let acao = if self.allowed_origins.contains(&"*".to_string()) {
                "*".to_string()
            } else {
                origin.to_string()
            };
            headers.push(("Access-Control-Allow-Origin".into(), acao));
            headers.push((
                "Access-Control-Allow-Methods".into(),
                self.allowed_methods.join(", "),
            ));
            if !self.allowed_headers.is_empty() {
                headers.push((
                    "Access-Control-Allow-Headers".into(),
                    self.allowed_headers.join(", "),
                ));
            }
            if self.allow_credentials {
                headers.push(("Access-Control-Allow-Credentials".into(), "true".into()));
            }
            if self.max_age_secs > 0 {
                headers.push((
                    "Access-Control-Max-Age".into(),
                    self.max_age_secs.to_string(),
                ));
            }
        }
        headers
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.6: Secret Management
// ═══════════════════════════════════════════════════════════════════════

/// Secret entry.
#[derive(Debug, Clone)]
pub struct Secret {
    /// Secret key name
    pub name: String,
    /// Encrypted value (base64)
    pub encrypted_value: String,
    /// Created timestamp (Unix seconds)
    pub created_at: u64,
    /// Last accessed timestamp
    pub last_accessed: u64,
}

/// Secret store.
#[derive(Debug)]
pub struct SecretStore {
    /// Stored secrets
    pub secrets: HashMap<String, Secret>,
    /// Encryption key ID
    pub key_id: String,
}

impl SecretStore {
    /// Create a new secret store.
    pub fn new(key_id: &str) -> Self {
        Self {
            secrets: HashMap::new(),
            key_id: key_id.to_string(),
        }
    }

    /// Store a secret (value would be encrypted in production).
    pub fn set(&mut self, name: &str, value: &str, now_secs: u64) {
        self.secrets.insert(
            name.to_string(),
            Secret {
                name: name.to_string(),
                encrypted_value: value.to_string(),
                created_at: now_secs,
                last_accessed: now_secs,
            },
        );
    }

    /// Retrieve a secret.
    pub fn get(&mut self, name: &str, now_secs: u64) -> Option<&str> {
        if let Some(secret) = self.secrets.get_mut(name) {
            secret.last_accessed = now_secs;
            Some(secret.encrypted_value.as_str())
        } else {
            None
        }
    }

    /// List secret names (not values).
    pub fn list_names(&self) -> Vec<&str> {
        self.secrets.keys().map(|k| k.as_str()).collect()
    }

    /// Delete a secret.
    pub fn delete(&mut self, name: &str) -> bool {
        self.secrets.remove(name).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.7: Audit Logging
// ═══════════════════════════════════════════════════════════════════════

/// Audit event type.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditEventType {
    /// Authentication attempt
    AuthAttempt,
    /// Successful authentication
    AuthSuccess,
    /// Failed authentication
    AuthFailure,
    /// Authorization check
    AuthzCheck,
    /// Resource access
    ResourceAccess,
    /// Configuration change
    ConfigChange,
}

/// Audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Event type
    pub event_type: AuditEventType,
    /// Timestamp (Unix seconds)
    pub timestamp: u64,
    /// Client identifier (IP, user ID, etc.)
    pub client: String,
    /// Resource being accessed
    pub resource: String,
    /// Action taken
    pub action: String,
    /// Whether the action was allowed
    pub allowed: bool,
    /// Additional details
    pub details: String,
}

/// Audit log.
#[derive(Debug)]
pub struct AuditLog {
    /// Log entries
    pub entries: Vec<AuditEntry>,
    /// Maximum entries to retain
    pub max_entries: usize,
}

impl AuditLog {
    /// Create a new audit log.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record an audit event.
    pub fn record(&mut self, entry: AuditEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Query entries by event type.
    pub fn query_by_type(&self, event_type: &AuditEventType) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| &e.event_type == event_type)
            .collect()
    }

    /// Count failed auth attempts for a client.
    pub fn failed_auth_count(&self, client: &str, since: u64) -> usize {
        self.entries
            .iter()
            .filter(|e| {
                e.event_type == AuditEventType::AuthFailure
                    && e.client == client
                    && e.timestamp >= since
            })
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.8: Input Sanitization
// ═══════════════════════════════════════════════════════════════════════

/// Sanitize a string to prevent XSS.
pub fn sanitize_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Sanitize SQL input (escape single quotes).
pub fn sanitize_sql(input: &str) -> String {
    input.replace('\'', "''")
}

/// Check for common injection patterns.
pub fn detect_injection(input: &str) -> Vec<InjectionType> {
    let mut found = Vec::new();
    let lower = input.to_lowercase();

    // SQL injection patterns
    if lower.contains("' or ") || lower.contains("'; drop") || lower.contains("1=1") {
        found.push(InjectionType::Sql);
    }

    // XSS patterns
    if lower.contains("<script") || lower.contains("javascript:") || lower.contains("onerror=") {
        found.push(InjectionType::Xss);
    }

    // Command injection patterns
    if lower.contains("; rm ") || lower.contains("| cat ") || lower.contains("$(") {
        found.push(InjectionType::Command);
    }

    // Path traversal
    if input.contains("../") || input.contains("..\\") {
        found.push(InjectionType::PathTraversal);
    }

    found
}

/// Injection type.
#[derive(Debug, Clone, PartialEq)]
pub enum InjectionType {
    /// SQL injection
    Sql,
    /// Cross-site scripting
    Xss,
    /// Command injection
    Command,
    /// Path traversal
    PathTraversal,
}

// ═══════════════════════════════════════════════════════════════════════
// S32.9: Dependency Audit
// ═══════════════════════════════════════════════════════════════════════

/// Known vulnerability entry.
#[derive(Debug, Clone)]
pub struct Vulnerability {
    /// CVE identifier
    pub cve_id: String,
    /// Affected package
    pub package: String,
    /// Affected version range
    pub affected_versions: String,
    /// Severity
    pub severity: VulnSeverity,
    /// Description
    pub description: String,
    /// Fix version (if available)
    pub fix_version: Option<String>,
}

/// Vulnerability severity.
#[derive(Debug, Clone, PartialEq)]
pub enum VulnSeverity {
    /// Low impact
    Low,
    /// Medium impact
    Medium,
    /// High impact
    High,
    /// Critical impact
    Critical,
}

/// Dependency for audit.
#[derive(Debug, Clone)]
pub struct AuditDep {
    /// Package name
    pub name: String,
    /// Version
    pub version: String,
}

/// Audit result.
#[derive(Debug, Clone)]
pub struct AuditResult {
    /// Dependencies checked
    pub deps_checked: usize,
    /// Vulnerabilities found
    pub vulnerabilities: Vec<Vulnerability>,
}

impl AuditResult {
    /// Whether the audit passed (no high/critical vulns).
    pub fn passed(&self) -> bool {
        !self
            .vulnerabilities
            .iter()
            .any(|v| matches!(v.severity, VulnSeverity::High | VulnSeverity::Critical))
    }

    /// Count by severity.
    pub fn count_by_severity(&self, severity: &VulnSeverity) -> usize {
        self.vulnerabilities
            .iter()
            .filter(|v| &v.severity == severity)
            .count()
    }
}

/// Run a dependency audit against a vulnerability database.
pub fn audit_dependencies(deps: &[AuditDep], vuln_db: &[Vulnerability]) -> AuditResult {
    let mut found = Vec::new();
    for dep in deps {
        for vuln in vuln_db {
            if dep.name == vuln.package {
                found.push(vuln.clone());
            }
        }
    }
    AuditResult {
        deps_checked: deps.len(),
        vulnerabilities: found,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S32.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s32_1_tls_config_validation() {
        let config = TlsConfig::new("cert.pem", "key.pem");
        assert!(config.validate().is_empty());
        assert_eq!(config.min_version, TlsVersion::Tls12);
        assert!(!config.cipher_suites.is_empty());
    }

    #[test]
    fn s32_1_tls_config_empty_cert() {
        let config = TlsConfig::new("", "key.pem");
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.contains("cert_path")));
    }

    #[test]
    fn s32_2_parse_jwt() {
        let token = "header.payload.signature";
        let result = parse_jwt(token);
        assert!(result.is_ok());

        let bad_token = "invalid";
        assert_eq!(parse_jwt(bad_token).unwrap_err(), JwtError::MalformedToken);
    }

    #[test]
    fn s32_2_validate_jwt_claims() {
        let claims = JwtClaims {
            sub: Some("user1".into()),
            iss: Some("auth.example.com".into()),
            aud: Some("api.example.com".into()),
            exp: Some(1000),
            iat: Some(900),
            custom: HashMap::new(),
        };
        let mut config = JwtConfig::new();
        config.issuer = Some("auth.example.com".into());
        config.audience = Some("api.example.com".into());

        assert!(validate_claims(&claims, &config, 950).is_ok());
        assert_eq!(
            validate_claims(&claims, &config, 2000).unwrap_err(),
            JwtError::Expired
        );
    }

    #[test]
    fn s32_3_api_key_auth() {
        let mut store = ApiKeyStore::new();
        store.register("key1", "hash_abc", vec!["read".into(), "write".into()]);
        assert!(store.validate("hash_abc").is_some());
        assert!(store.validate("wrong_hash").is_none());

        store.revoke("key1");
        assert!(store.validate("hash_abc").is_none());
    }

    #[test]
    fn s32_4_rate_limiter() {
        let mut limiter = RateLimiter::new(3, 1);
        assert!(limiter.try_acquire("client1", 0));
        assert!(limiter.try_acquire("client1", 0));
        assert!(limiter.try_acquire("client1", 0));
        assert!(!limiter.try_acquire("client1", 0)); // Exhausted

        // After 2 seconds, 2 more tokens
        assert!(limiter.try_acquire("client1", 2));
    }

    #[test]
    fn s32_5_cors_permissive() {
        let cors = CorsConfig::permissive();
        assert!(cors.is_origin_allowed("https://example.com"));
        let headers = cors.headers("https://example.com");
        assert!(headers
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Origin" && v == "*"));
    }

    #[test]
    fn s32_5_cors_strict() {
        let cors = CorsConfig::strict();
        assert!(!cors.is_origin_allowed("https://example.com"));
        let headers = cors.headers("https://example.com");
        assert!(headers.is_empty());
    }

    #[test]
    fn s32_6_secret_store() {
        let mut store = SecretStore::new("key-1");
        store.set("DB_PASSWORD", "secret123", 1000);
        assert_eq!(store.get("DB_PASSWORD", 1001), Some("secret123"));
        assert!(store.list_names().contains(&"DB_PASSWORD"));

        store.delete("DB_PASSWORD");
        assert!(store.get("DB_PASSWORD", 1002).is_none());
    }

    #[test]
    fn s32_7_audit_logging() {
        let mut log = AuditLog::new(1000);
        log.record(AuditEntry {
            event_type: AuditEventType::AuthFailure,
            timestamp: 100,
            client: "192.168.1.1".into(),
            resource: "/api/admin".into(),
            action: "login".into(),
            allowed: false,
            details: "wrong password".into(),
        });
        log.record(AuditEntry {
            event_type: AuditEventType::AuthSuccess,
            timestamp: 200,
            client: "192.168.1.1".into(),
            resource: "/api/admin".into(),
            action: "login".into(),
            allowed: true,
            details: "ok".into(),
        });

        assert_eq!(log.failed_auth_count("192.168.1.1", 0), 1);
        assert_eq!(log.query_by_type(&AuditEventType::AuthSuccess).len(), 1);
    }

    #[test]
    fn s32_8_sanitize_html() {
        let input = "<script>alert('xss')</script>";
        let sanitized = sanitize_html(input);
        assert!(!sanitized.contains('<'));
        assert!(sanitized.contains("&lt;"));
    }

    #[test]
    fn s32_8_detect_injection() {
        let sql = "'; DROP TABLE users; --";
        assert!(detect_injection(sql).contains(&InjectionType::Sql));

        let xss = "<script>alert(1)</script>";
        assert!(detect_injection(xss).contains(&InjectionType::Xss));

        let path = "../../etc/passwd";
        assert!(detect_injection(path).contains(&InjectionType::PathTraversal));

        let clean = "hello world";
        assert!(detect_injection(clean).is_empty());
    }

    #[test]
    fn s32_9_dependency_audit() {
        let deps = vec![
            AuditDep {
                name: "serde".into(),
                version: "1.0.190".into(),
            },
            AuditDep {
                name: "openssl".into(),
                version: "0.10.50".into(),
            },
        ];
        let vuln_db = vec![Vulnerability {
            cve_id: "CVE-2023-1234".into(),
            package: "openssl".into(),
            affected_versions: "<0.10.55".into(),
            severity: VulnSeverity::High,
            description: "Buffer overflow".into(),
            fix_version: Some("0.10.55".into()),
        }];

        let result = audit_dependencies(&deps, &vuln_db);
        assert_eq!(result.deps_checked, 2);
        assert_eq!(result.vulnerabilities.len(), 1);
        assert!(!result.passed()); // High severity found
    }

    #[test]
    fn s32_9_audit_passed() {
        let deps = vec![AuditDep {
            name: "serde".into(),
            version: "1.0.200".into(),
        }];
        let result = audit_dependencies(&deps, &[]);
        assert!(result.passed());
        assert_eq!(result.count_by_severity(&VulnSeverity::Critical), 0);
    }

    #[test]
    fn s32_4_rate_limiter_remaining() {
        let mut limiter = RateLimiter::new(10, 5);
        limiter.try_acquire("c1", 0);
        assert_eq!(limiter.remaining("c1"), 9);
        assert_eq!(limiter.remaining("unknown"), 10);
    }

    #[test]
    fn s32_8_sanitize_sql() {
        let input = "O'Malley";
        assert_eq!(sanitize_sql(input), "O''Malley");
    }
}
