//! Package registry server — REST API for registry.fajarlang.dev.
//!
//! Implements the backend for a Cloudflare Workers-based registry with
//! D1 database, R2 blob storage, and rate limiting. This module defines
//! the schema, routes, and logic that would be deployed as a Worker.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D1 Database Schema
// ═══════════════════════════════════════════════════════════════════════

/// SQL schema for the D1 database backing the registry.
pub const D1_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    username    TEXT    NOT NULL UNIQUE,
    email       TEXT    NOT NULL UNIQUE,
    api_key     TEXT    NOT NULL UNIQUE,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    is_active   INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS packages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    owner_id    INTEGER NOT NULL REFERENCES users(id),
    description TEXT    NOT NULL DEFAULT '',
    repository  TEXT,
    license     TEXT,
    keywords    TEXT    NOT NULL DEFAULT '[]',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    downloads   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS versions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id  INTEGER NOT NULL REFERENCES packages(id),
    version     TEXT    NOT NULL,
    checksum    TEXT    NOT NULL,
    r2_key      TEXT    NOT NULL,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    yanked      INTEGER NOT NULL DEFAULT 0,
    published_at TEXT   NOT NULL DEFAULT (datetime('now')),
    UNIQUE(package_id, version)
);

CREATE TABLE IF NOT EXISTS api_keys (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    key_hash    TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL DEFAULT 'default',
    scopes      TEXT    NOT NULL DEFAULT '["publish"]',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT,
    last_used   TEXT
);

CREATE TABLE IF NOT EXISTS downloads (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    version_id  INTEGER NOT NULL REFERENCES versions(id),
    ip_hash     TEXT    NOT NULL,
    user_agent  TEXT,
    downloaded_at TEXT  NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
CREATE INDEX IF NOT EXISTS idx_versions_package ON versions(package_id);
CREATE INDEX IF NOT EXISTS idx_downloads_version ON downloads(version_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);
"#;

// ═══════════════════════════════════════════════════════════════════════
// API Types
// ═══════════════════════════════════════════════════════════════════════

/// HTTP method for registry API routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    /// GET request.
    Get,
    /// POST request.
    Post,
    /// PUT request.
    Put,
    /// DELETE request.
    Delete,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
        }
    }
}

/// An API route definition.
#[derive(Debug, Clone)]
pub struct Route {
    /// HTTP method.
    pub method: HttpMethod,
    /// URL path pattern (e.g., "/api/v1/packages/{name}").
    pub path: String,
    /// Description of what this route does.
    pub description: String,
    /// Whether authentication is required.
    pub requires_auth: bool,
}

/// Registry API route definitions.
pub fn api_routes() -> Vec<Route> {
    vec![
        Route {
            method: HttpMethod::Post,
            path: "/api/v1/packages".to_string(),
            description: "Upload a new package version (.tar.gz)".to_string(),
            requires_auth: true,
        },
        Route {
            method: HttpMethod::Get,
            path: "/api/v1/packages/{name}".to_string(),
            description: "Get package metadata and version list".to_string(),
            requires_auth: false,
        },
        Route {
            method: HttpMethod::Get,
            path: "/api/v1/packages/{name}/{version}".to_string(),
            description: "Download a specific package version".to_string(),
            requires_auth: false,
        },
        Route {
            method: HttpMethod::Get,
            path: "/api/v1/search".to_string(),
            description: "Search packages by name and description".to_string(),
            requires_auth: false,
        },
        Route {
            method: HttpMethod::Put,
            path: "/api/v1/packages/{name}/{version}/yank".to_string(),
            description: "Yank a published version (hide from search)".to_string(),
            requires_auth: true,
        },
        Route {
            method: HttpMethod::Put,
            path: "/api/v1/packages/{name}/{version}/unyank".to_string(),
            description: "Unyank a previously yanked version".to_string(),
            requires_auth: true,
        },
        Route {
            method: HttpMethod::Post,
            path: "/api/v1/auth/login".to_string(),
            description: "Authenticate and receive API key".to_string(),
            requires_auth: false,
        },
        Route {
            method: HttpMethod::Get,
            path: "/api/v1/auth/tokens".to_string(),
            description: "List active API tokens".to_string(),
            requires_auth: true,
        },
    ]
}

/// HTTP status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCode(pub u16);

impl StatusCode {
    /// 200 OK.
    pub const OK: Self = Self(200);
    /// 201 Created.
    pub const CREATED: Self = Self(201);
    /// 400 Bad Request.
    pub const BAD_REQUEST: Self = Self(400);
    /// 401 Unauthorized.
    pub const UNAUTHORIZED: Self = Self(401);
    /// 403 Forbidden.
    pub const FORBIDDEN: Self = Self(403);
    /// 404 Not Found.
    pub const NOT_FOUND: Self = Self(404);
    /// 409 Conflict (duplicate version).
    pub const CONFLICT: Self = Self(409);
    /// 429 Too Many Requests.
    pub const TOO_MANY_REQUESTS: Self = Self(429);
    /// 500 Internal Server Error.
    pub const INTERNAL_ERROR: Self = Self(500);
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.0 {
            200 => "OK",
            201 => "Created",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            409 => "Conflict",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            _ => "Unknown",
        };
        write!(f, "{} {}", self.0, label)
    }
}

/// A registry API response.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    /// HTTP status code.
    pub status: StatusCode,
    /// JSON response body.
    pub body: String,
    /// Response headers.
    pub headers: HashMap<String, String>,
}

impl ApiResponse {
    /// Creates a success response with JSON body.
    pub fn ok(body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            status: StatusCode::OK,
            body: body.to_string(),
            headers,
        }
    }

    /// Creates a 201 Created response.
    pub fn created(body: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            status: StatusCode::CREATED,
            body: body.to_string(),
            headers,
        }
    }

    /// Creates an error response.
    pub fn error(status: StatusCode, message: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        Self {
            status,
            body: format!(r#"{{"error":"{}"}}"#, message),
            headers,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Package Metadata
// ═══════════════════════════════════════════════════════════════════════

/// Package metadata returned by the API.
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    /// Package name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Repository URL.
    pub repository: Option<String>,
    /// SPDX license identifier.
    pub license: Option<String>,
    /// Searchable keywords.
    pub keywords: Vec<String>,
    /// Total download count.
    pub downloads: u64,
    /// Available versions (newest first).
    pub versions: Vec<VersionInfo>,
}

/// Version metadata.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Semver version string.
    pub version: String,
    /// SHA-256 checksum of the tarball.
    pub checksum: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Whether this version is yanked.
    pub yanked: bool,
    /// ISO 8601 publication timestamp.
    pub published_at: String,
}

impl PackageMetadata {
    /// Serializes package metadata to JSON.
    pub fn to_json(&self) -> String {
        let versions_json: Vec<String> = self
            .versions
            .iter()
            .map(|v| {
                format!(
                    r#"{{"version":"{}","checksum":"{}","size_bytes":{},"yanked":{},"published_at":"{}"}}"#,
                    v.version, v.checksum, v.size_bytes, v.yanked, v.published_at
                )
            })
            .collect();
        let keywords_json: Vec<String> =
            self.keywords.iter().map(|k| format!(r#""{k}""#)).collect();
        format!(
            r#"{{"name":"{}","description":"{}","repository":{},"license":{},"keywords":[{}],"downloads":{},"versions":[{}]}}"#,
            self.name,
            self.description,
            self.repository
                .as_ref()
                .map(|r| format!(r#""{r}""#))
                .unwrap_or_else(|| "null".to_string()),
            self.license
                .as_ref()
                .map(|l| format!(r#""{l}""#))
                .unwrap_or_else(|| "null".to_string()),
            keywords_json.join(","),
            self.downloads,
            versions_json.join(","),
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Search
// ═══════════════════════════════════════════════════════════════════════

/// A search result entry.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Package name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Latest non-yanked version.
    pub latest_version: String,
    /// Total download count.
    pub downloads: u64,
    /// Relevance score (higher = more relevant).
    pub score: f64,
}

/// Search query parameters.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Search term.
    pub query: String,
    /// Maximum number of results (default 20).
    pub limit: usize,
    /// Offset for pagination.
    pub offset: usize,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 20,
            offset: 0,
        }
    }
}

/// Performs full-text search over package entries.
pub fn search_packages(entries: &[PackageMetadata], query: &SearchQuery) -> Vec<SearchResult> {
    let q = query.query.to_lowercase();
    let mut results: Vec<SearchResult> = entries
        .iter()
        .filter_map(|pkg| {
            let name_lower = pkg.name.to_lowercase();
            let desc_lower = pkg.description.to_lowercase();

            // Score: exact name match = 100, name contains = 50, description contains = 20
            let mut score = 0.0;
            if name_lower == q {
                score += 100.0;
            } else if name_lower.contains(&q) {
                score += 50.0;
            }
            if desc_lower.contains(&q) {
                score += 20.0;
            }
            // Boost by download count (log scale)
            score += (pkg.downloads as f64 + 1.0).ln();

            if score > 0.0 {
                let latest = pkg
                    .versions
                    .iter()
                    .find(|v| !v.yanked)
                    .map(|v| v.version.clone())
                    .unwrap_or_else(|| "0.0.0".to_string());
                Some(SearchResult {
                    name: pkg.name.clone(),
                    description: pkg.description.clone(),
                    latest_version: latest,
                    downloads: pkg.downloads,
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// R2 Storage
// ═══════════════════════════════════════════════════════════════════════

/// R2 bucket configuration for storing package tarballs.
#[derive(Debug, Clone)]
pub struct R2Config {
    /// Bucket name.
    pub bucket: String,
    /// Key prefix for package tarballs.
    pub prefix: String,
}

impl Default for R2Config {
    fn default() -> Self {
        Self {
            bucket: "fj-packages".to_string(),
            prefix: "packages/".to_string(),
        }
    }
}

impl R2Config {
    /// Generates the R2 key for a package version tarball.
    pub fn tarball_key(&self, name: &str, version: &str) -> String {
        format!("{}{}/{}/{}.tar.gz", self.prefix, name, version, name)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Rate Limiting
// ═══════════════════════════════════════════════════════════════════════

/// Rate limiting configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum uploads per hour per IP.
    pub upload_per_hour: u32,
    /// Maximum downloads per hour per IP.
    pub download_per_hour: u32,
    /// Maximum search queries per minute per IP.
    pub search_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            upload_per_hour: 10,
            download_per_hour: 1000,
            search_per_minute: 60,
        }
    }
}

/// Rate limit check result.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Requests remaining in window.
    pub remaining: u32,
    /// Seconds until window resets.
    pub reset_seconds: u64,
}

impl RateLimitResult {
    /// Creates an allowed result.
    pub fn allow(remaining: u32, reset_seconds: u64) -> Self {
        Self {
            allowed: true,
            remaining,
            reset_seconds,
        }
    }

    /// Creates a denied (rate limited) result.
    pub fn deny(reset_seconds: u64) -> Self {
        Self {
            allowed: false,
            remaining: 0,
            reset_seconds,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Authentication
// ═══════════════════════════════════════════════════════════════════════

/// API key scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyScope {
    /// Can publish packages.
    Publish,
    /// Can yank/unyank versions.
    Yank,
    /// Can manage tokens.
    ManageTokens,
    /// Full access.
    Admin,
}

impl fmt::Display for ApiKeyScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publish => write!(f, "publish"),
            Self::Yank => write!(f, "yank"),
            Self::ManageTokens => write!(f, "manage_tokens"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

/// Validates an API key and returns the associated user and scopes.
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// User ID.
    pub user_id: u64,
    /// Username.
    pub username: String,
    /// Granted scopes.
    pub scopes: Vec<ApiKeyScope>,
}

impl AuthResult {
    /// Checks if the user has the required scope.
    pub fn has_scope(&self, scope: &ApiKeyScope) -> bool {
        self.scopes.contains(&ApiKeyScope::Admin) || self.scopes.contains(scope)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Version Validation
// ═══════════════════════════════════════════════════════════════════════

/// Validates a package upload request.
#[derive(Debug, Clone)]
pub struct UploadValidation {
    /// Whether the upload is valid.
    pub valid: bool,
    /// Validation error messages (empty if valid).
    pub errors: Vec<String>,
}

/// Validates package metadata for upload.
pub fn validate_upload(
    name: &str,
    version: &str,
    existing_versions: &[String],
) -> UploadValidation {
    let mut errors = Vec::new();

    // Name validation
    if name.is_empty() {
        errors.push("package name cannot be empty".to_string());
    } else if name.len() > 64 {
        errors.push("package name too long (max 64 chars)".to_string());
    } else if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        errors.push("package name must contain only [a-zA-Z0-9_-]".to_string());
    }

    // Version validation
    if super::SemVer::parse(version).is_err() {
        errors.push(format!("invalid semver: '{version}'"));
    }

    // Duplicate check
    if existing_versions.contains(&version.to_string()) {
        errors.push(format!("version {version} already exists"));
    }

    UploadValidation {
        valid: errors.is_empty(),
        errors,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sparse Index
// ═══════════════════════════════════════════════════════════════════════

/// Generates a sparse index entry for a package (like crates.io index).
pub fn sparse_index_path(name: &str) -> String {
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// Generates a sparse index JSON line for a version.
pub fn sparse_index_entry(
    name: &str,
    version: &str,
    checksum: &str,
    yanked: bool,
    deps: &[(String, String)],
) -> String {
    let deps_json: Vec<String> = deps
        .iter()
        .map(|(n, v)| format!(r#"{{"name":"{n}","req":"{v}"}}"#))
        .collect();
    format!(
        r#"{{"name":"{name}","vers":"{version}","cksum":"{checksum}","yanked":{yanked},"deps":[{deps}]}}"#,
        deps = deps_json.join(",")
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Download Counter
// ═══════════════════════════════════════════════════════════════════════

/// In-memory download counter (periodically flushed to D1).
#[derive(Debug, Clone)]
pub struct DownloadCounter {
    counts: HashMap<String, u64>,
}

impl Default for DownloadCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadCounter {
    /// Creates a new empty counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increments the download count for a package version.
    pub fn increment(&mut self, name: &str, version: &str) {
        let key = format!("{name}@{version}");
        *self.counts.entry(key).or_insert(0) += 1;
    }

    /// Returns the accumulated count for a package version.
    pub fn count(&self, name: &str, version: &str) -> u64 {
        let key = format!("{name}@{version}");
        *self.counts.get(&key).unwrap_or(&0)
    }

    /// Drains all accumulated counts, returning them for batch D1 update.
    pub fn drain(&mut self) -> Vec<(String, u64)> {
        self.counts.drain().collect()
    }

    /// Returns total accumulated downloads across all versions.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S25.1: API route definitions
    #[test]
    fn s25_1_api_routes_cover_all_endpoints() {
        let routes = api_routes();
        assert!(routes.len() >= 8);
        assert!(
            routes
                .iter()
                .any(|r| r.path.contains("packages") && r.method == HttpMethod::Post)
        );
        assert!(routes.iter().any(|r| r.path.contains("search")));
        assert!(routes.iter().any(|r| r.path.contains("yank")));
        assert!(routes.iter().any(|r| r.path.contains("login")));
    }

    // S25.2: D1 schema
    #[test]
    fn s25_2_d1_schema_has_all_tables() {
        assert!(D1_SCHEMA.contains("CREATE TABLE IF NOT EXISTS users"));
        assert!(D1_SCHEMA.contains("CREATE TABLE IF NOT EXISTS packages"));
        assert!(D1_SCHEMA.contains("CREATE TABLE IF NOT EXISTS versions"));
        assert!(D1_SCHEMA.contains("CREATE TABLE IF NOT EXISTS api_keys"));
        assert!(D1_SCHEMA.contains("CREATE TABLE IF NOT EXISTS downloads"));
        assert!(D1_SCHEMA.contains("CREATE INDEX"));
    }

    // S25.3: Package upload endpoint types
    #[test]
    fn s25_3_api_response_created() {
        let resp = ApiResponse::created(r#"{"name":"fj-math","version":"1.0.0"}"#);
        assert_eq!(resp.status.0, 201);
        assert!(resp.body.contains("fj-math"));
    }

    // S25.4: Package download endpoint
    #[test]
    fn s25_4_r2_tarball_key() {
        let cfg = R2Config::default();
        assert_eq!(
            cfg.tarball_key("fj-math", "1.0.0"),
            "packages/fj-math/1.0.0/fj-math.tar.gz"
        );
    }

    // S25.5: Search
    #[test]
    fn s25_5_search_packages_ranking() {
        let entries = vec![
            PackageMetadata {
                name: "fj-math".to_string(),
                description: "Math utilities for Fajar Lang".to_string(),
                repository: None,
                license: Some("MIT".to_string()),
                keywords: vec!["math".to_string()],
                downloads: 1000,
                versions: vec![VersionInfo {
                    version: "1.0.0".to_string(),
                    checksum: "abc123".to_string(),
                    size_bytes: 5000,
                    yanked: false,
                    published_at: "2026-01-01T00:00:00Z".to_string(),
                }],
            },
            PackageMetadata {
                name: "fj-nn".to_string(),
                description: "Neural network library with math ops".to_string(),
                repository: None,
                license: Some("MIT".to_string()),
                keywords: vec!["ml".to_string()],
                downloads: 500,
                versions: vec![VersionInfo {
                    version: "0.2.0".to_string(),
                    checksum: "def456".to_string(),
                    size_bytes: 8000,
                    yanked: false,
                    published_at: "2026-02-01T00:00:00Z".to_string(),
                }],
            },
        ];
        let query = SearchQuery {
            query: "math".to_string(),
            limit: 10,
            offset: 0,
        };
        let results = search_packages(&entries, &query);
        assert_eq!(results.len(), 2);
        // fj-math should rank higher (exact name contains + description match)
        assert_eq!(results[0].name, "fj-math");
    }

    // S25.6: Authentication
    #[test]
    fn s25_6_auth_result_scope_check() {
        let auth = AuthResult {
            user_id: 1,
            username: "testuser".to_string(),
            scopes: vec![ApiKeyScope::Publish],
        };
        assert!(auth.has_scope(&ApiKeyScope::Publish));
        assert!(!auth.has_scope(&ApiKeyScope::Yank));

        let admin = AuthResult {
            user_id: 2,
            username: "admin".to_string(),
            scopes: vec![ApiKeyScope::Admin],
        };
        assert!(admin.has_scope(&ApiKeyScope::Publish));
        assert!(admin.has_scope(&ApiKeyScope::Yank));
    }

    // S25.7: Version validation
    #[test]
    fn s25_7_validate_upload_rejects_invalid() {
        let result = validate_upload("", "1.0.0", &[]);
        assert!(!result.valid);
        assert!(result.errors[0].contains("empty"));

        let result = validate_upload("fj-math", "not-semver", &[]);
        assert!(!result.valid);

        let result = validate_upload("fj-math", "1.0.0", &["1.0.0".to_string()]);
        assert!(!result.valid);
        assert!(result.errors[0].contains("already exists"));
    }

    #[test]
    fn s25_7_validate_upload_accepts_valid() {
        let result = validate_upload("fj-math", "1.0.0", &[]);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    // S25.8: Rate limiting
    #[test]
    fn s25_8_rate_limit_config() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.upload_per_hour, 10);
        assert_eq!(cfg.download_per_hour, 1000);
        assert_eq!(cfg.search_per_minute, 60);
    }

    #[test]
    fn s25_8_rate_limit_result() {
        let allow = RateLimitResult::allow(9, 3600);
        assert!(allow.allowed);
        assert_eq!(allow.remaining, 9);

        let deny = RateLimitResult::deny(120);
        assert!(!deny.allowed);
        assert_eq!(deny.remaining, 0);
    }

    // S25.9: R2 storage
    #[test]
    fn s25_9_r2_config_default() {
        let cfg = R2Config::default();
        assert_eq!(cfg.bucket, "fj-packages");
        assert!(cfg.prefix.starts_with("packages"));
    }

    // S25.10: Download counter
    #[test]
    fn s25_10_download_counter() {
        let mut counter = DownloadCounter::new();
        counter.increment("fj-math", "1.0.0");
        counter.increment("fj-math", "1.0.0");
        counter.increment("fj-nn", "0.2.0");
        assert_eq!(counter.count("fj-math", "1.0.0"), 2);
        assert_eq!(counter.count("fj-nn", "0.2.0"), 1);
        assert_eq!(counter.total(), 3);

        let drained = counter.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(counter.total(), 0);
    }

    // Sparse index tests
    #[test]
    fn s25_10_sparse_index_path_formats() {
        assert_eq!(sparse_index_path("a"), "1/a");
        assert_eq!(sparse_index_path("ab"), "2/ab");
        assert_eq!(sparse_index_path("abc"), "3/a/abc");
        assert_eq!(sparse_index_path("fj-math"), "fj/-m/fj-math");
    }

    #[test]
    fn s25_10_sparse_index_entry_format() {
        let entry = sparse_index_entry("fj-math", "1.0.0", "abc123", false, &[]);
        assert!(entry.contains(r#""name":"fj-math""#));
        assert!(entry.contains(r#""vers":"1.0.0""#));
        assert!(entry.contains(r#""yanked":false"#));
    }

    // Package metadata serialization
    #[test]
    fn s25_10_package_metadata_json() {
        let meta = PackageMetadata {
            name: "fj-test".to_string(),
            description: "Test package".to_string(),
            repository: Some("https://github.com/fajar-lang/fj-test".to_string()),
            license: Some("MIT".to_string()),
            keywords: vec!["test".to_string()],
            downloads: 42,
            versions: vec![],
        };
        let json = meta.to_json();
        assert!(json.contains(r#""name":"fj-test""#));
        assert!(json.contains(r#""downloads":42"#));
        assert!(json.contains(r#""license":"MIT""#));
    }

    #[test]
    fn s25_10_status_code_display() {
        assert_eq!(format!("{}", StatusCode::OK), "200 OK");
        assert_eq!(format!("{}", StatusCode::NOT_FOUND), "404 Not Found");
        assert_eq!(
            format!("{}", StatusCode::TOO_MANY_REQUESTS),
            "429 Too Many Requests"
        );
    }

    #[test]
    fn s25_10_api_response_error() {
        let resp = ApiResponse::error(StatusCode::UNAUTHORIZED, "invalid token");
        assert_eq!(resp.status.0, 401);
        assert!(resp.body.contains("invalid token"));
    }
}
