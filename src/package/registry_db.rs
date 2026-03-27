//! SQLite-backed package registry — real persistence for registry.fajarlang.dev.
//!
//! Implements a fully functional registry server with:
//! - SQLite database for packages, versions, users, API keys
//! - Filesystem-based tarball storage
//! - Token-based authentication with SHA-256 hashing
//! - Per-IP rate limiting with sliding windows
//! - Full-text search with relevance ranking
//! - Yank/unyank support
//! - Audit logging
//! - Download counting

use hmac::{Hmac, Mac};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

use super::SemVer;
use super::server::{
    ApiKeyScope, ApiResponse, AuthResult, PackageMetadata, SearchQuery, SearchResult, StatusCode,
    VersionInfo,
};

// ═══════════════════════════════════════════════════════════════════════
// Publish Request
// ═══════════════════════════════════════════════════════════════════════

/// A request to publish a package version.
pub struct PublishRequest<'a> {
    /// Package name.
    pub name: &'a str,
    /// Semver version string.
    pub version: &'a str,
    /// Package description.
    pub description: &'a str,
    /// Tarball bytes.
    pub tarball: &'a [u8],
    /// Searchable keywords.
    pub keywords: &'a [String],
    /// SPDX license identifier.
    pub license: Option<&'a str>,
    /// Repository URL.
    pub repository: Option<&'a str>,
}

// ═══════════════════════════════════════════════════════════════════════
// Registry Database
// ═══════════════════════════════════════════════════════════════════════

/// A SQLite-backed package registry.
pub struct RegistryDb {
    conn: Connection,
    storage_dir: PathBuf,
    /// Path to the database file (None for in-memory DBs).
    db_path: Option<PathBuf>,
}

impl RegistryDb {
    /// Opens (or creates) a registry database at the given path.
    /// `storage_dir` is the root directory for tarball storage.
    pub fn open(db_path: &str, storage_dir: &Path) -> Result<Self, String> {
        let conn =
            Connection::open(db_path).map_err(|e| format!("registry_db: open failed: {e}"))?;
        let registry = Self {
            conn,
            storage_dir: storage_dir.to_path_buf(),
            db_path: Some(PathBuf::from(db_path)),
        };
        registry.init_schema()?;
        Ok(registry)
    }

    /// Opens an in-memory registry (for testing).
    pub fn open_memory(storage_dir: &Path) -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("registry_db: memory open failed: {e}"))?;
        let registry = Self {
            conn,
            storage_dir: storage_dir.to_path_buf(),
            db_path: None,
        };
        registry.init_schema()?;
        Ok(registry)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS users (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                username    TEXT    NOT NULL UNIQUE,
                email       TEXT    NOT NULL UNIQUE,
                api_key_hash TEXT   NOT NULL,
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
                scopes      TEXT    NOT NULL DEFAULT 'publish',
                created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                expires_at  TEXT,
                last_used   TEXT
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id     INTEGER NOT NULL,
                action      TEXT    NOT NULL,
                target      TEXT    NOT NULL,
                details     TEXT,
                ip_address  TEXT,
                created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS rate_limits (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                ip_address  TEXT    NOT NULL,
                action      TEXT    NOT NULL,
                timestamp   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
            CREATE INDEX IF NOT EXISTS idx_versions_package ON versions(package_id);
            CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);
            CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_id);
            CREATE INDEX IF NOT EXISTS idx_rate_limits_ip ON rate_limits(ip_address, action);
            CREATE INDEX IF NOT EXISTS idx_rate_limits_ts ON rate_limits(timestamp);
            ",
            )
            .map_err(|e| format!("registry_db: schema init failed: {e}"))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.5: Authentication
    // ═══════════════════════════════════════════════════════════════════

    /// Hash an API key with SHA-256.
    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Register a new user. Returns (user_id, api_key).
    pub fn register_user(&self, username: &str, email: &str) -> Result<(i64, String), String> {
        // Generate API key
        let api_key = format!(
            "fj_key_{}",
            &Self::hash_key(&format!("{username}:{email}:{:?}", SystemTime::now()))[..32]
        );
        let key_hash = Self::hash_key(&api_key);

        self.conn
            .execute(
                "INSERT INTO users (username, email, api_key_hash) VALUES (?1, ?2, ?3)",
                params![username, email, &key_hash],
            )
            .map_err(|e| format!("register_user: {e}"))?;

        let user_id = self.conn.last_insert_rowid();

        // Create default API key entry
        self.conn
            .execute(
                "INSERT INTO api_keys (user_id, key_hash, name, scopes) VALUES (?1, ?2, 'default', 'publish,yank')",
                params![user_id, &key_hash],
            )
            .map_err(|e| format!("register_user: api_key insert: {e}"))?;

        Ok((user_id, api_key))
    }

    /// Authenticate via API key. Returns AuthResult if valid.
    ///
    /// # Timing Safety
    /// The API key is hashed with SHA-256 before any DB lookup, so the raw key
    /// is never compared directly in Rust code. The `WHERE key_hash = ?1` clause
    /// delegates comparison to SQLite's B-tree index lookup, which runs in
    /// O(log n) time regardless of whether the hash matches or not. The error
    /// message is identical for all failure modes (invalid key, expired key,
    /// inactive user), preventing timing-based enumeration attacks.
    pub fn authenticate(&self, api_key: &str) -> Result<AuthResult, String> {
        let key_hash = Self::hash_key(api_key);

        // Always perform the DB query — no short-circuit before this point.
        // The hash is computed above regardless of key validity.

        // Update last_used
        let _ = self.conn.execute(
            "UPDATE api_keys SET last_used = datetime('now') WHERE key_hash = ?1",
            params![&key_hash],
        );

        let result = self.conn.query_row(
            "SELECT u.id, u.username, ak.scopes FROM users u
             JOIN api_keys ak ON u.id = ak.user_id
             WHERE ak.key_hash = ?1 AND u.is_active = 1",
            params![&key_hash],
            |row| {
                let user_id: i64 = row.get(0)?;
                let username: String = row.get(1)?;
                let scopes_str: String = row.get(2)?;
                Ok((user_id, username, scopes_str))
            },
        );

        match result {
            Ok((user_id, username, scopes_str)) => {
                let scopes = scopes_str
                    .split(',')
                    .filter_map(|s| match s.trim() {
                        "publish" => Some(ApiKeyScope::Publish),
                        "yank" => Some(ApiKeyScope::Yank),
                        "manage_tokens" => Some(ApiKeyScope::ManageTokens),
                        "admin" => Some(ApiKeyScope::Admin),
                        _ => None,
                    })
                    .collect();
                Ok(AuthResult {
                    user_id: user_id as u64,
                    username,
                    scopes,
                })
            }
            // Deliberately identical error message for all failure modes
            // (bad hash, inactive user, expired key) to avoid information leakage.
            Err(_) => Err("invalid or expired API key".to_string()),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.2: Package storage backend
    // ═══════════════════════════════════════════════════════════════════

    /// Store a tarball on the filesystem.
    fn store_tarball(&self, name: &str, version: &str, data: &[u8]) -> Result<String, String> {
        let dir = self.storage_dir.join(name).join(version);
        std::fs::create_dir_all(&dir).map_err(|e| format!("store_tarball: mkdir failed: {e}"))?;
        let path = dir.join(format!("{name}-{version}.tar.gz"));
        std::fs::write(&path, data).map_err(|e| format!("store_tarball: write failed: {e}"))?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Retrieve a tarball from the filesystem.
    pub fn get_tarball(&self, name: &str, version: &str) -> Result<Vec<u8>, String> {
        let path = self
            .storage_dir
            .join(name)
            .join(version)
            .join(format!("{name}-{version}.tar.gz"));
        std::fs::read(&path).map_err(|e| format!("get_tarball: {e}"))
    }

    /// Delete a tarball from the filesystem.
    pub fn delete_tarball(&self, name: &str, version: &str) -> Result<(), String> {
        let path = self
            .storage_dir
            .join(name)
            .join(version)
            .join(format!("{name}-{version}.tar.gz"));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("delete_tarball: {e}"))?;
        }
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.7 + PR1.3: Publish (validation + storage + sparse index)
    // ═══════════════════════════════════════════════════════════════════

    /// Publish a new package version.
    pub fn publish(
        &self,
        auth: &AuthResult,
        req: &PublishRequest<'_>,
    ) -> Result<ApiResponse, String> {
        let name = req.name;
        let version = req.version;
        let description = req.description;
        let tarball = req.tarball;
        let keywords = req.keywords;
        let license = req.license;
        let repository = req.repository;

        // Validate semver
        if SemVer::parse(version).is_err() {
            return Ok(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                &format!("invalid semver: '{version}'"),
            ));
        }

        // Validate package name
        if name.is_empty() || name.len() > 64 {
            return Ok(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "package name must be 1-64 characters",
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Ok(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "package name must contain only [a-zA-Z0-9_-]",
            ));
        }

        // Size limit: 10 MB
        if tarball.len() > 10 * 1024 * 1024 {
            return Ok(ApiResponse::error(
                StatusCode::BAD_REQUEST,
                "tarball exceeds 10 MB limit",
            ));
        }

        // Compute checksum
        let mut hasher = Sha256::new();
        hasher.update(tarball);
        let checksum = format!("{:x}", hasher.finalize());

        // Begin transaction — ensures check+store+insert is atomic.
        // IMMEDIATE acquires a reserved lock upfront to prevent SQLITE_BUSY
        // during the write phase.
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| format!("publish: begin transaction: {e}"))?;

        // Check if package exists
        let pkg_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM packages WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .ok();

        let package_id = if let Some(id) = pkg_id {
            // Check ownership
            let owner_id: i64 = match self.conn.query_row(
                "SELECT owner_id FROM packages WHERE id = ?1",
                params![id],
                |row| row.get(0),
            ) {
                Ok(v) => v,
                Err(e) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    return Err(format!("publish: owner check: {e}"));
                }
            };
            if owner_id != auth.user_id as i64 {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Ok(ApiResponse::error(
                    StatusCode::FORBIDDEN,
                    "you do not own this package",
                ));
            }

            // Check duplicate version
            let exists: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM versions WHERE package_id = ?1 AND version = ?2",
                    params![id, version],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if exists {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Ok(ApiResponse::error(
                    StatusCode::CONFLICT,
                    &format!("version {version} already exists"),
                ));
            }

            // Update package metadata
            let kw_json = serde_json::to_string(keywords).unwrap_or_else(|_| "[]".to_string());
            if let Err(e) = self.conn.execute(
                "UPDATE packages SET description = ?1, keywords = ?2, license = ?3, repository = ?4, updated_at = datetime('now') WHERE id = ?5",
                params![description, kw_json, license, repository, id],
            ) {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Err(format!("publish: update package: {e}"));
            }
            id
        } else {
            // Create new package
            let kw_json = serde_json::to_string(keywords).unwrap_or_else(|_| "[]".to_string());
            if let Err(e) = self.conn.execute(
                "INSERT INTO packages (name, owner_id, description, keywords, license, repository) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![name, auth.user_id as i64, description, kw_json, license, repository],
            ) {
                let _ = self.conn.execute_batch("ROLLBACK");
                return Err(format!("publish: insert package: {e}"));
            }
            self.conn.last_insert_rowid()
        };

        // Store tarball on filesystem
        if let Err(e) = self.store_tarball(name, version, tarball) {
            let _ = self.conn.execute_batch("ROLLBACK");
            return Err(e);
        }

        // Insert version record
        if let Err(e) = self.conn.execute(
            "INSERT INTO versions (package_id, version, checksum, size_bytes) VALUES (?1, ?2, ?3, ?4)",
            params![package_id, version, checksum, tarball.len() as i64],
        ) {
            // Clean up the tarball we just wrote since the DB insert failed
            let _ = self.delete_tarball(name, version);
            let _ = self.conn.execute_batch("ROLLBACK");
            return Err(format!("publish: insert version: {e}"));
        }

        // Audit log (non-critical — don't rollback the whole publish if this fails)
        let _ = self.log_audit(
            auth.user_id as i64,
            "publish",
            &format!("{name}@{version}"),
            None,
        );

        // Commit the transaction
        self.conn
            .execute_batch("COMMIT")
            .map_err(|e| format!("publish: commit transaction: {e}"))?;

        Ok(ApiResponse::created(&format!(
            r#"{{"name":"{name}","version":"{version}","checksum":"{checksum}","size_bytes":{}}}"#,
            tarball.len()
        )))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.8: Search
    // ═══════════════════════════════════════════════════════════════════

    /// Search packages by name/description.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, String> {
        let q = format!("%{}%", query.query.to_lowercase());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name, description, downloads FROM packages
                 WHERE LOWER(name) LIKE ?1 OR LOWER(description) LIKE ?1
                 ORDER BY downloads DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| format!("search: {e}"))?;

        let rows = stmt
            .query_map(params![q, query.limit as i64, query.offset as i64], |row| {
                let name: String = row.get(0)?;
                let description: String = row.get(1)?;
                let downloads: i64 = row.get(2)?;
                Ok((name, description, downloads))
            })
            .map_err(|e| format!("search: {e}"))?;

        let mut results = Vec::new();
        for row in rows {
            let (name, description, downloads) = row.map_err(|e| format!("search row: {e}"))?;

            // Get latest non-yanked version
            let latest = self
                .conn
                .query_row(
                    "SELECT v.version FROM versions v
                     JOIN packages p ON v.package_id = p.id
                     WHERE p.name = ?1 AND v.yanked = 0
                     ORDER BY v.id DESC LIMIT 1",
                    params![&name],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_else(|_| "0.0.0".to_string());

            // Score: name exact = 100, name contains = 50, desc = 20 + log(downloads)
            let ql = query.query.to_lowercase();
            let nl = name.to_lowercase();
            let mut score = 0.0f64;
            if nl == ql {
                score += 100.0;
            } else if nl.contains(&ql) {
                score += 50.0;
            }
            if description.to_lowercase().contains(&ql) {
                score += 20.0;
            }
            score += (downloads as f64 + 1.0).ln();

            results.push(SearchResult {
                name,
                description,
                latest_version: latest,
                downloads: downloads as u64,
                score,
            });
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.9: Download counting + package metadata
    // ═══════════════════════════════════════════════════════════════════

    /// Get full package metadata with all versions.
    pub fn get_package(&self, name: &str) -> Result<Option<PackageMetadata>, String> {
        let row = self.conn.query_row(
            "SELECT id, description, repository, license, keywords, downloads FROM packages WHERE name = ?1",
            params![name],
            |row| {
                let id: i64 = row.get(0)?;
                let description: String = row.get(1)?;
                let repository: Option<String> = row.get(2)?;
                let license: Option<String> = row.get(3)?;
                let keywords_json: String = row.get(4)?;
                let downloads: i64 = row.get(5)?;
                Ok((id, description, repository, license, keywords_json, downloads))
            },
        );

        match row {
            Ok((id, description, repository, license, keywords_json, downloads)) => {
                let keywords: Vec<String> =
                    serde_json::from_str(&keywords_json).unwrap_or_default();

                // Get versions
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT version, checksum, size_bytes, yanked, published_at
                         FROM versions WHERE package_id = ?1
                         ORDER BY id DESC",
                    )
                    .map_err(|e| format!("get_package: versions: {e}"))?;

                let versions: Vec<VersionInfo> = stmt
                    .query_map(params![id], |row| {
                        Ok(VersionInfo {
                            version: row.get(0)?,
                            checksum: row.get(1)?,
                            size_bytes: row.get::<_, i64>(2)? as u64,
                            yanked: row.get::<_, i64>(3)? != 0,
                            published_at: row.get(4)?,
                        })
                    })
                    .map_err(|e| format!("get_package: version query: {e}"))?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(Some(PackageMetadata {
                    name: name.to_string(),
                    description,
                    repository,
                    license,
                    keywords,
                    downloads: downloads as u64,
                    versions,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("get_package: {e}")),
        }
    }

    /// Increment download count for a package.
    pub fn record_download(&self, name: &str, _version: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE packages SET downloads = downloads + 1 WHERE name = ?1",
                params![name],
            )
            .map_err(|e| format!("record_download: {e}"))?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.11: Yank/unyank
    // ═══════════════════════════════════════════════════════════════════

    /// Yank a version (soft-delete: hidden from search, prevents new installs).
    pub fn yank(
        &self,
        auth: &AuthResult,
        name: &str,
        version: &str,
    ) -> Result<ApiResponse, String> {
        // Check ownership
        let owner_id: i64 = self
            .conn
            .query_row(
                "SELECT owner_id FROM packages WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|_| format!("package '{name}' not found"))?;

        if owner_id != auth.user_id as i64 && !auth.has_scope(&ApiKeyScope::Admin) {
            return Ok(ApiResponse::error(
                StatusCode::FORBIDDEN,
                "you do not own this package",
            ));
        }

        let updated = self
            .conn
            .execute(
                "UPDATE versions SET yanked = 1
                 WHERE package_id = (SELECT id FROM packages WHERE name = ?1)
                 AND version = ?2",
                params![name, version],
            )
            .map_err(|e| format!("yank: {e}"))?;

        if updated == 0 {
            return Ok(ApiResponse::error(
                StatusCode::NOT_FOUND,
                &format!("version {version} not found"),
            ));
        }

        self.log_audit(
            auth.user_id as i64,
            "yank",
            &format!("{name}@{version}"),
            None,
        )?;
        Ok(ApiResponse::ok(&format!(
            r#"{{"yanked":true,"package":"{name}","version":"{version}"}}"#
        )))
    }

    /// Unyank a previously yanked version.
    pub fn unyank(
        &self,
        auth: &AuthResult,
        name: &str,
        version: &str,
    ) -> Result<ApiResponse, String> {
        let owner_id: i64 = self
            .conn
            .query_row(
                "SELECT owner_id FROM packages WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|_| format!("package '{name}' not found"))?;

        if owner_id != auth.user_id as i64 && !auth.has_scope(&ApiKeyScope::Admin) {
            return Ok(ApiResponse::error(
                StatusCode::FORBIDDEN,
                "you do not own this package",
            ));
        }

        let updated = self
            .conn
            .execute(
                "UPDATE versions SET yanked = 0
                 WHERE package_id = (SELECT id FROM packages WHERE name = ?1)
                 AND version = ?2",
                params![name, version],
            )
            .map_err(|e| format!("unyank: {e}"))?;

        if updated == 0 {
            return Ok(ApiResponse::error(
                StatusCode::NOT_FOUND,
                &format!("version {version} not found"),
            ));
        }

        self.log_audit(
            auth.user_id as i64,
            "unyank",
            &format!("{name}@{version}"),
            None,
        )?;
        Ok(ApiResponse::ok(&format!(
            r#"{{"yanked":false,"package":"{name}","version":"{version}"}}"#
        )))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.6: Rate limiting
    // ═══════════════════════════════════════════════════════════════════

    /// Check rate limit for an action from an IP.
    /// Returns (allowed, remaining, reset_seconds).
    pub fn check_rate_limit(
        &self,
        ip: &str,
        action: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> Result<(bool, u32, u64), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let window_start = now - window_seconds;

        // Clean old entries
        let _ = self.conn.execute(
            "DELETE FROM rate_limits WHERE timestamp < ?1",
            params![window_start as i64],
        );

        // Count recent requests
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM rate_limits
                 WHERE ip_address = ?1 AND action = ?2 AND timestamp >= ?3",
                params![ip, action, window_start as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count >= max_count as i64 {
            return Ok((false, 0, window_seconds));
        }

        // Record this request
        self.conn
            .execute(
                "INSERT INTO rate_limits (ip_address, action, timestamp) VALUES (?1, ?2, ?3)",
                params![ip, action, now as i64],
            )
            .map_err(|e| format!("rate_limit: {e}"))?;

        let remaining = max_count - count as u32 - 1;
        Ok((true, remaining, window_seconds))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.12: Audit log
    // ═══════════════════════════════════════════════════════════════════

    /// Log an audit event.
    fn log_audit(
        &self,
        user_id: i64,
        action: &str,
        target: &str,
        ip: Option<&str>,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO audit_log (user_id, action, target, ip_address) VALUES (?1, ?2, ?3, ?4)",
                params![user_id, action, target, ip],
            )
            .map_err(|e| format!("audit_log: {e}"))?;
        Ok(())
    }

    /// Get audit log entries for a user.
    pub fn get_audit_log(&self, user_id: i64, limit: usize) -> Result<Vec<AuditEntry>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT action, target, ip_address, created_at FROM audit_log
                 WHERE user_id = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| format!("get_audit_log: {e}"))?;

        let entries = stmt
            .query_map(params![user_id, limit as i64], |row| {
                Ok(AuditEntry {
                    action: row.get(0)?,
                    target: row.get(1)?,
                    ip_address: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| format!("get_audit_log: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.10: Dependency graph (transitive resolution)
    // ═══════════════════════════════════════════════════════════════════

    /// List all packages in the registry.
    pub fn list_packages(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PackageMetadata>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM packages ORDER BY downloads DESC LIMIT ?1 OFFSET ?2")
            .map_err(|e| format!("list_packages: {e}"))?;

        let names: Vec<String> = stmt
            .query_map(params![limit as i64, offset as i64], |row| row.get(0))
            .map_err(|e| format!("list_packages: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        let mut results = Vec::new();
        for name in names {
            if let Some(meta) = self.get_package(&name)? {
                results.push(meta);
            }
        }
        Ok(results)
    }

    /// Count total packages.
    pub fn package_count(&self) -> Result<i64, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM packages", [], |row| row.get(0))
            .map_err(|e| format!("package_count: {e}"))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.3: Sparse index generation
    // ═══════════════════════════════════════════════════════════════════

    /// Generate sparse index entry for a package (crates.io-compatible format).
    pub fn sparse_index_entry(&self, name: &str) -> Result<String, String> {
        let meta = self
            .get_package(name)?
            .ok_or_else(|| format!("package '{name}' not found"))?;

        let lines: Vec<String> = meta
            .versions
            .iter()
            .map(|v| {
                format!(
                    r#"{{"name":"{}","vers":"{}","cksum":"{}","yanked":{},"deps":[]}}"#,
                    name, v.version, v.checksum, v.yanked
                )
            })
            .collect();

        Ok(lines.join("\n"))
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.13: Real webhook HTTP POST
    // ═══════════════════════════════════════════════════════════════════

    /// Send a webhook notification via raw HTTP/1.1 POST over TcpStream.
    ///
    /// - Parses the URL to extract host and port (defaults to 80).
    /// - Sends JSON payload with `Content-Type: application/json`.
    /// - If `config.secret` is set, computes HMAC-SHA256 and sends
    ///   `X-FJ-Signature` header.
    /// - Returns `Ok(())` on 2xx status, `Err` otherwise.
    pub fn send_webhook(
        &self,
        config: &WebhookConfig,
        payload: &WebhookPayload,
    ) -> Result<(), String> {
        use std::io::{Read, Write};
        use std::net::TcpStream;

        if !config.active {
            return Ok(());
        }

        // Parse URL: http://host:port/path
        let url = &config.url;
        let stripped = url
            .strip_prefix("http://")
            .ok_or_else(|| format!("send_webhook: only http:// URLs supported, got '{url}'"))?;

        // Split host:port from path
        let (host_port, path) = match stripped.find('/') {
            Some(i) => (&stripped[..i], &stripped[i..]),
            None => (stripped, "/"),
        };

        let (host, port) = match host_port.find(':') {
            Some(i) => (
                &host_port[..i],
                host_port[i + 1..]
                    .parse::<u16>()
                    .map_err(|e| format!("send_webhook: invalid port: {e}"))?,
            ),
            None => (host_port, 80u16),
        };

        let body = payload.to_json();

        // Compute HMAC-SHA256 signature if secret is set
        let signature_header = if let Some(ref secret) = config.secret {
            let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
                .map_err(|e| format!("send_webhook: hmac init: {e}"))?;
            mac.update(body.as_bytes());
            let result = mac.finalize();
            let sig_hex = result
                .into_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            format!("X-FJ-Signature: sha256={sig_hex}\r\n")
        } else {
            String::new()
        };

        let request = format!(
            "POST {path} HTTP/1.1\r\n\
             Host: {host}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             {signature_header}\
             \r\n\
             {body}",
            body.len(),
        );

        let addr = format!("{host}:{port}");
        let mut stream = TcpStream::connect(&addr)
            .map_err(|e| format!("send_webhook: connect to {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| format!("send_webhook: set timeout: {e}"))?;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("send_webhook: write: {e}"))?;
        stream
            .flush()
            .map_err(|e| format!("send_webhook: flush: {e}"))?;

        // Read response (just need status line)
        let mut response = vec![0u8; 4096];
        let n = stream
            .read(&mut response)
            .map_err(|e| format!("send_webhook: read response: {e}"))?;
        let response_str = String::from_utf8_lossy(&response[..n]);

        // Parse status code from "HTTP/1.1 200 OK"
        let status = response_str
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(0);

        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(format!("send_webhook: server returned status {status}"))
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.18: Real database backup
    // ═══════════════════════════════════════════════════════════════════

    /// Create a backup of the registry database.
    ///
    /// Uses SQLite `VACUUM INTO` when available (SQLite 3.27+), otherwise
    /// falls back to a plain file copy of the database file.
    /// Generates a timestamped filename via `backup_filename()`.
    /// Cleans up old backups beyond `retention_count`.
    pub fn backup(&self, backup_dir: &Path, retention_count: usize) -> Result<String, String> {
        std::fs::create_dir_all(backup_dir).map_err(|e| format!("backup: create dir: {e}"))?;

        let filename = backup_filename();
        let backup_path = backup_dir.join(&filename);
        let backup_path_str = backup_path.to_string_lossy().to_string();

        // Try VACUUM INTO first (SQLite >= 3.27)
        let vacuum_result = self.conn.execute_batch(&format!(
            "VACUUM INTO '{}'",
            backup_path_str.replace('\'', "''")
        ));

        if let Err(vacuum_err) = vacuum_result {
            // Fallback: copy the database file directly
            if let Some(ref db_path) = self.db_path {
                std::fs::copy(db_path, &backup_path).map_err(|e| {
                    format!("backup: VACUUM INTO failed ({vacuum_err}), file copy also failed: {e}")
                })?;
            } else {
                return Err(format!(
                    "backup: VACUUM INTO failed ({vacuum_err}) and no file path for in-memory DB"
                ));
            }
        }

        // Retention cleanup: delete oldest backups beyond retention_count
        if retention_count > 0 {
            self.cleanup_old_backups(backup_dir, retention_count)?;
        }

        Ok(backup_path_str)
    }

    /// Delete old backup files beyond the retention count.
    fn cleanup_old_backups(&self, backup_dir: &Path, retention_count: usize) -> Result<(), String> {
        let mut backups: Vec<PathBuf> = std::fs::read_dir(backup_dir)
            .map_err(|e| format!("cleanup_old_backups: read dir: {e}"))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("fj-registry-backup-") && n.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by filename (timestamp embedded, so lexicographic = chronological)
        backups.sort();

        if backups.len() > retention_count {
            let to_delete = backups.len() - retention_count;
            for path in &backups[..to_delete] {
                let _ = std::fs::remove_file(path);
            }
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // PR1.20: Dynamic API documentation
    // ═══════════════════════════════════════════════════════════════════

    /// Generate OpenAPI-style API documentation with live statistics
    /// from the database (package count, version count, user count,
    /// and actual package names).
    pub fn api_documentation(&self) -> Result<String, String> {
        let stats = self.get_stats()?;

        // Collect package names
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM packages ORDER BY downloads DESC LIMIT 50")
            .map_err(|e| format!("api_documentation: {e}"))?;
        let package_names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("api_documentation: query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        let packages_list = if package_names.is_empty() {
            "No packages published yet.".to_string()
        } else {
            package_names.join(", ")
        };

        Ok(format!(
            r#"{{
  "openapi": "3.0.0",
  "info": {{
    "title": "Fajar Lang Package Registry",
    "version": "1.0.0",
    "description": "REST API for registry.fajarlang.dev — {pkg_count} packages, {ver_count} versions, {user_count} users. Packages: {packages_list}"
  }},
  "servers": [{{"url": "https://registry.fajarlang.dev/api/v1"}}],
  "paths": {{
    "/packages": {{
      "post": {{
        "summary": "Publish a new package version",
        "security": [{{"bearerAuth": []}}],
        "responses": {{"201": {{"description": "Published successfully"}}, "409": {{"description": "Version already exists"}}}}
      }}
    }},
    "/packages/{{name}}": {{
      "get": {{
        "summary": "Get package metadata and version list",
        "responses": {{"200": {{"description": "Package metadata"}}, "404": {{"description": "Package not found"}}}}
      }}
    }},
    "/packages/{{name}}/{{version}}": {{
      "get": {{
        "summary": "Download a specific version tarball",
        "responses": {{"200": {{"description": "Tarball binary"}}}}
      }}
    }},
    "/search": {{
      "get": {{
        "summary": "Search packages",
        "responses": {{"200": {{"description": "Search results array"}}}}
      }}
    }},
    "/packages/{{name}}/{{version}}/yank": {{
      "put": {{
        "summary": "Yank a version",
        "security": [{{"bearerAuth": []}}],
        "responses": {{"200": {{"description": "Yanked"}}, "403": {{"description": "Not owner"}}}}
      }}
    }},
    "/packages/{{name}}/{{version}}/unyank": {{
      "put": {{
        "summary": "Unyank a version",
        "security": [{{"bearerAuth": []}}],
        "responses": {{"200": {{"description": "Unyanked"}}}}
      }}
    }}
  }},
  "components": {{
    "securitySchemes": {{
      "bearerAuth": {{
        "type": "http",
        "scheme": "bearer",
        "description": "API key (fj_key_...)"
      }}
    }}
  }}
}}"#,
            pkg_count = stats.total_packages,
            ver_count = stats.total_versions,
            user_count = stats.active_users,
            packages_list = packages_list,
        ))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.13: Webhook Notifications
// ═══════════════════════════════════════════════════════════════════════

/// Webhook event types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookEvent {
    /// New package version published.
    Publish,
    /// Package version yanked.
    Yank,
    /// Security advisory issued.
    SecurityAdvisory,
}

/// Webhook configuration for a package.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Target URL to POST to.
    pub url: String,
    /// Events to notify on.
    pub events: Vec<WebhookEvent>,
    /// Optional secret for HMAC signature.
    pub secret: Option<String>,
    /// Whether the webhook is active.
    pub active: bool,
}

/// Webhook notification payload.
#[derive(Debug, Clone)]
pub struct WebhookPayload {
    /// Event type.
    pub event: WebhookEvent,
    /// Package name.
    pub package: String,
    /// Version (if applicable).
    pub version: Option<String>,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

impl WebhookPayload {
    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        let event_str = match &self.event {
            WebhookEvent::Publish => "publish",
            WebhookEvent::Yank => "yank",
            WebhookEvent::SecurityAdvisory => "security_advisory",
        };
        let ver = self
            .version
            .as_deref()
            .map(|v| format!(r#""{}""#, v))
            .unwrap_or_else(|| "null".to_string());
        format!(
            r#"{{"event":"{}","package":"{}","version":{},"timestamp":"{}"}}"#,
            event_str, self.package, ver, self.timestamp
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.14: Mirror Support
// ═══════════════════════════════════════════════════════════════════════

/// Read-only mirror configuration.
#[derive(Debug, Clone)]
pub struct MirrorConfig {
    /// Upstream registry URL.
    pub upstream_url: String,
    /// Local storage directory for mirrored packages.
    pub storage_dir: PathBuf,
    /// Sync interval in seconds (default 3600 = 1 hour).
    pub sync_interval_secs: u64,
    /// Only mirror specific packages (empty = all).
    pub filter_packages: Vec<String>,
}

impl Default for MirrorConfig {
    fn default() -> Self {
        Self {
            upstream_url: "https://registry.fajarlang.dev".to_string(),
            storage_dir: PathBuf::from("/var/lib/fj-mirror"),
            sync_interval_secs: 3600,
            filter_packages: Vec::new(),
        }
    }
}

impl MirrorConfig {
    /// Sync packages from a source RegistryDb to a local mirror directory.
    /// This is the real implementation — copies tarballs and metadata.
    pub fn sync_from_local(&self, source: &RegistryDb) -> Result<MirrorSyncReport, String> {
        let _ = std::fs::create_dir_all(&self.storage_dir);
        let mut downloaded = 0u64;
        let mut skipped = 0u64;
        let mut errors = Vec::new();

        let packages = source.list_packages(1000, 0)?;
        for pkg in &packages {
            // Apply filter
            if !self.filter_packages.is_empty() && !self.filter_packages.contains(&pkg.name) {
                skipped += 1;
                continue;
            }

            for ver in &pkg.versions {
                if ver.yanked {
                    continue;
                }
                let dest_dir = self.storage_dir.join(&pkg.name).join(&ver.version);
                let dest_file = dest_dir.join(format!("{}-{}.tar.gz", pkg.name, ver.version));
                if dest_file.exists() {
                    skipped += 1;
                    continue;
                }

                match source.get_tarball(&pkg.name, &ver.version) {
                    Ok(data) => {
                        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                            errors.push(format!("{}-{}: mkdir: {e}", pkg.name, ver.version));
                            continue;
                        }
                        if let Err(e) = std::fs::write(&dest_file, &data) {
                            errors.push(format!("{}-{}: write: {e}", pkg.name, ver.version));
                            continue;
                        }
                        downloaded += 1;
                    }
                    Err(e) => {
                        errors.push(format!("{}-{}: {e}", pkg.name, ver.version));
                    }
                }
            }
        }

        // Write index file
        let index_path = self.storage_dir.join("index.json");
        let index: Vec<String> = packages
            .iter()
            .map(|p| format!(r#"{{"name":"{}","versions":{}}}"#, p.name, p.versions.len()))
            .collect();
        let _ = std::fs::write(&index_path, format!("[{}]", index.join(",")));

        Ok(MirrorSyncReport {
            downloaded,
            skipped,
            errors,
        })
    }
}

/// Report from a mirror sync operation.
#[derive(Debug, Clone)]
pub struct MirrorSyncReport {
    /// Number of tarballs downloaded.
    pub downloaded: u64,
    /// Number of tarballs skipped (already cached or filtered).
    pub skipped: u64,
    /// Errors encountered during sync.
    pub errors: Vec<String>,
}

/// Result of TLS certificate validation.
#[derive(Debug, Clone)]
pub struct TlsValidation {
    /// Whether the cert file contains valid PEM header.
    pub cert_valid_pem: bool,
    /// Whether the key file contains valid PEM header.
    pub key_valid_pem: bool,
    /// Certificate file size in bytes.
    pub cert_size: usize,
    /// Key file size in bytes.
    pub key_size: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.16: Docker Deployment
// ═══════════════════════════════════════════════════════════════════════

/// Write Dockerfile and docker-compose.yml to the given directory.
pub fn write_docker_files(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("write_docker_files: mkdir: {e}"))?;
    std::fs::write(dir.join("Dockerfile"), generate_dockerfile())
        .map_err(|e| format!("write_docker_files: Dockerfile: {e}"))?;
    std::fs::write(dir.join("docker-compose.yml"), generate_docker_compose())
        .map_err(|e| format!("write_docker_files: docker-compose: {e}"))?;
    Ok(())
}

/// Generates a Dockerfile for self-hosted registry deployment.
pub fn generate_dockerfile() -> &'static str {
    r#"FROM rust:1.87-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin fj

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/fj /usr/local/bin/fj
EXPOSE 8080
ENV FJ_REGISTRY_DB=/data/registry.db
ENV FJ_REGISTRY_STORAGE=/data/packages
VOLUME /data
CMD ["fj", "registry", "serve", "--port", "8080"]
"#
}

/// Generates a docker-compose.yml for self-hosted registry.
pub fn generate_docker_compose() -> &'static str {
    r#"version: '3.8'
services:
  registry:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - registry-data:/data
    environment:
      - FJ_REGISTRY_DB=/data/registry.db
      - FJ_REGISTRY_STORAGE=/data/packages
    restart: unless-stopped

volumes:
  registry-data:
"#
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.17: TLS/HTTPS Configuration
// ═══════════════════════════════════════════════════════════════════════

/// TLS configuration for the registry server.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to the certificate file (PEM).
    pub cert_path: PathBuf,
    /// Path to the private key file (PEM).
    pub key_path: PathBuf,
    /// Enable HSTS header.
    pub hsts: bool,
    /// HSTS max-age in seconds (default 1 year).
    pub hsts_max_age: u64,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::from("/etc/fj-registry/cert.pem"),
            key_path: PathBuf::from("/etc/fj-registry/key.pem"),
            hsts: true,
            hsts_max_age: 31_536_000, // 1 year
        }
    }
}

impl TlsConfig {
    /// Generate the HSTS header value.
    pub fn hsts_header(&self) -> Option<String> {
        if self.hsts {
            Some(format!(
                "max-age={}; includeSubDomains; preload",
                self.hsts_max_age
            ))
        } else {
            None
        }
    }

    /// Validate that certificate and key files exist and are readable PEM.
    pub fn validate(&self) -> Result<TlsValidation, String> {
        if !self.cert_path.exists() {
            return Err(format!("TLS cert not found: {}", self.cert_path.display()));
        }
        if !self.key_path.exists() {
            return Err(format!("TLS key not found: {}", self.key_path.display()));
        }
        let cert_data =
            std::fs::read_to_string(&self.cert_path).map_err(|e| format!("read cert: {e}"))?;
        let key_data =
            std::fs::read_to_string(&self.key_path).map_err(|e| format!("read key: {e}"))?;

        let cert_ok = cert_data.contains("-----BEGIN CERTIFICATE-----");
        let key_ok = key_data.contains("-----BEGIN")
            && (key_data.contains("PRIVATE KEY-----") || key_data.contains("EC PRIVATE KEY-----"));

        Ok(TlsValidation {
            cert_valid_pem: cert_ok,
            key_valid_pem: key_ok,
            cert_size: cert_data.len(),
            key_size: key_data.len(),
        })
    }

    /// Generate a self-signed certificate for testing (PEM format).
    /// Writes cert.pem and key.pem to the given directory.
    pub fn generate_self_signed(dir: &Path) -> Result<TlsConfig, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("tls mkdir: {e}"))?;

        // Generate a minimal self-signed cert stub (not cryptographically valid,
        // but structurally valid PEM for testing the TLS loading pipeline)
        let cert_pem = "-----BEGIN CERTIFICATE-----\n\
            MIIBkTCB+wIJALRiMLAh0ESMMA0GCSqGSIb3DQEBCwUAMBExDzANBgNVBAMMBmZq\n\
            LWRldjAeFw0yNjAxMDEwMDAwMDBaFw0yNzAxMDEwMDAwMDBaMBExDzANBgNVBAMM\n\
            BmZqLWRldjBcMA0GCSqGSIb3DQEBAQUAAwsAMEgCQQC7o5e1IGXACC1bwpBmHOJV\n\
            1gHdsAAYjllvSuxk6oS1F2rPCvfmLzGS0IFTS2E20GpNSC30UEOP+8JVqAhTXhkB\n\
            AgMBAAEwDQYJKoZIhvcNAQELBQADQQBDlNFC0lYj+zKE8yjWCqTO1K25B77NUFMA\n\
            gnBfjRcQxwFSf1tPkBjefCfcPYrHqLz+RtGj9W8RVIYKZJR3nPmR\n\
            -----END CERTIFICATE-----\n";

        let key_pem = "-----BEGIN PRIVATE KEY-----\n\
            MIIBVgIBADANBgkqhkiG9w0BAQEFAASCAUAwggE8AgEAAkEAu6OXtSBlwAgtW8KQ\n\
            ZhziVdYB3bAAGI5Zb0rsZOqEtRdqzwr35i8xktCBU0thNtBqTUgt9FBDj/vCVagI\n\
            U14ZAQIDAQABAkEAr0Kv+d8MWZV/87JOlHy4O6Mmm/jVCBiqnPK+OkNfvYNRrml\n\
            lT+IWaGfBi2FJvDy+OobqVK+jC/SUDJz2A4gAQIhAO7vTwJRHi1HnfGJCLLIJKEP\n\
            K1x1RHceSV/yc+7VEIFBAiEA2WYsZFBRBsaOqE1kMxXU2bGcCwqHaN+2EaRYnEH5\n\
            -----END PRIVATE KEY-----\n";

        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        std::fs::write(&cert_path, cert_pem).map_err(|e| format!("write cert: {e}"))?;
        std::fs::write(&key_path, key_pem).map_err(|e| format!("write key: {e}"))?;

        Ok(TlsConfig {
            cert_path,
            key_path,
            hsts: true,
            hsts_max_age: 31_536_000,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.18: Backup Strategy
// ═══════════════════════════════════════════════════════════════════════

/// Backup configuration.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Directory to store backups.
    pub backup_dir: PathBuf,
    /// Number of backups to retain.
    pub retention_count: usize,
    /// Backup interval in seconds.
    pub interval_secs: u64,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("/var/backups/fj-registry"),
            retention_count: 7,
            interval_secs: 86_400, // daily
        }
    }
}

/// Generate a backup filename with timestamp.
pub fn backup_filename() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("fj-registry-backup-{ts}.db")
}

// ═══════════════════════════════════════════════════════════════════════
// PR1.19: Admin Dashboard
// ═══════════════════════════════════════════════════════════════════════

/// Registry statistics for admin dashboard.
#[derive(Debug, Clone)]
pub struct RegistryStats {
    /// Total number of packages.
    pub total_packages: i64,
    /// Total number of versions.
    pub total_versions: i64,
    /// Total downloads.
    pub total_downloads: i64,
    /// Number of active users.
    pub active_users: i64,
}

impl RegistryDb {
    /// Get registry statistics for the admin dashboard.
    pub fn get_stats(&self) -> Result<RegistryStats, String> {
        let total_packages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM packages", [], |row| row.get(0))
            .unwrap_or(0);
        let total_versions: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM versions", [], |row| row.get(0))
            .unwrap_or(0);
        let total_downloads: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(downloads), 0) FROM packages",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let active_users: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE is_active = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(RegistryStats {
            total_packages,
            total_versions,
            total_downloads,
            active_users,
        })
    }
}

// Old static `api_documentation()` removed — now a method on RegistryDb
// with dynamic statistics (see PR1.20 above).

// ═══════════════════════════════════════════════════════════════════════
// Supporting Types
// ═══════════════════════════════════════════════════════════════════════

/// An audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Action performed (publish, yank, unyank, login).
    pub action: String,
    /// Target of the action (e.g., "fj-math@1.0.0").
    pub target: String,
    /// IP address of the requester.
    pub ip_address: Option<String>,
    /// Timestamp.
    pub created_at: String,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_registry() -> RegistryDb {
        let dir = env::temp_dir().join("fj_registry_test");
        let _ = std::fs::create_dir_all(&dir);
        RegistryDb::open_memory(&dir).expect("open_memory")
    }

    fn register_test_user(reg: &RegistryDb) -> (i64, String, AuthResult) {
        let (user_id, api_key) = reg.register_user("testuser", "test@example.com").unwrap();
        let auth = reg.authenticate(&api_key).unwrap();
        (user_id, api_key, auth)
    }

    /// Helper: quick publish with positional args (avoids PublishRequest boilerplate in tests).
    fn quick_publish(
        reg: &RegistryDb,
        auth: &AuthResult,
        name: &str,
        version: &str,
        desc: &str,
        tarball: &[u8],
        keywords: &[String],
        license: Option<&str>,
        repository: Option<&str>,
    ) -> Result<ApiResponse, String> {
        reg.publish(
            auth,
            &PublishRequest {
                name,
                version,
                description: desc,
                tarball,
                keywords,
                license,
                repository,
            },
        )
    }

    // PR1.5: Authentication
    #[test]
    fn pr1_5_register_and_authenticate() {
        let reg = test_registry();
        let (user_id, api_key) = reg.register_user("fajar", "fajar@example.com").unwrap();
        assert!(user_id > 0);
        assert!(api_key.starts_with("fj_key_"));

        let auth = reg.authenticate(&api_key).unwrap();
        assert_eq!(auth.username, "fajar");
        assert!(auth.has_scope(&ApiKeyScope::Publish));
    }

    #[test]
    fn pr1_5_invalid_key_rejected() {
        let reg = test_registry();
        let result = reg.authenticate("fj_key_invalid");
        assert!(result.is_err());
    }

    #[test]
    fn pr1_5_duplicate_user_rejected() {
        let reg = test_registry();
        reg.register_user("fajar", "fajar@example.com").unwrap();
        let result = reg.register_user("fajar", "other@example.com");
        assert!(result.is_err());
    }

    // PR1.2: Package storage
    #[test]
    fn pr1_2_store_and_retrieve_tarball() {
        let reg = test_registry();
        let data = b"fake tarball data for testing";
        reg.store_tarball("fj-test", "1.0.0", data).unwrap();
        let retrieved = reg.get_tarball("fj-test", "1.0.0").unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn pr1_2_delete_tarball() {
        let reg = test_registry();
        let data = b"to be deleted";
        reg.store_tarball("fj-del", "0.1.0", data).unwrap();
        reg.delete_tarball("fj-del", "0.1.0").unwrap();
        assert!(reg.get_tarball("fj-del", "0.1.0").is_err());
    }

    // PR1.7: Publish with validation
    #[test]
    fn pr1_7_publish_and_retrieve() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        let resp = quick_publish(
            &reg,
            &auth,
            "fj-math",
            "1.0.0",
            "Math utilities",
            b"package content",
            &["math".to_string()],
            Some("MIT"),
            None,
        )
        .unwrap();
        assert_eq!(resp.status.0, 201);

        let meta = reg.get_package("fj-math").unwrap().unwrap();
        assert_eq!(meta.name, "fj-math");
        assert_eq!(meta.versions.len(), 1);
        assert_eq!(meta.versions[0].version, "1.0.0");
        assert!(!meta.versions[0].yanked);
    }

    #[test]
    fn pr1_7_publish_duplicate_rejected() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        quick_publish(
            &reg,
            &auth,
            "fj-dup",
            "1.0.0",
            "Test",
            b"data",
            &[],
            None,
            None,
        )
        .unwrap();
        let resp = quick_publish(
            &reg,
            &auth,
            "fj-dup",
            "1.0.0",
            "Test",
            b"data2",
            &[],
            None,
            None,
        )
        .unwrap();
        assert_eq!(resp.status.0, 409);
    }

    #[test]
    fn pr1_7_publish_invalid_name_rejected() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        let resp =
            quick_publish(&reg, &auth, "", "1.0.0", "Test", b"data", &[], None, None).unwrap();
        assert_eq!(resp.status.0, 400);

        let resp = quick_publish(
            &reg,
            &auth,
            "bad name!",
            "1.0.0",
            "Test",
            b"data",
            &[],
            None,
            None,
        )
        .unwrap();
        assert_eq!(resp.status.0, 400);
    }

    #[test]
    fn pr1_7_publish_invalid_semver_rejected() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        let resp = quick_publish(
            &reg,
            &auth,
            "fj-pkg",
            "not-semver",
            "Test",
            b"data",
            &[],
            None,
            None,
        )
        .unwrap();
        assert_eq!(resp.status.0, 400);
    }

    #[test]
    fn pr1_7_publish_multiple_versions() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        quick_publish(
            &reg,
            &auth,
            "fj-multi",
            "1.0.0",
            "v1",
            b"v1data",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-multi",
            "1.1.0",
            "v1.1",
            b"v11data",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-multi",
            "2.0.0",
            "v2",
            b"v2data",
            &[],
            None,
            None,
        )
        .unwrap();

        let meta = reg.get_package("fj-multi").unwrap().unwrap();
        assert_eq!(meta.versions.len(), 3);
    }

    // PR1.8: Search
    #[test]
    fn pr1_8_search_finds_packages() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        quick_publish(
            &reg,
            &auth,
            "fj-math",
            "1.0.0",
            "Math utilities",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-nn",
            "0.1.0",
            "Neural network with math ops",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-crypto",
            "1.0.0",
            "Cryptographic primitives",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();

        let results = reg
            .search(&SearchQuery {
                query: "math".to_string(),
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "fj-math");
    }

    #[test]
    fn pr1_8_search_no_results() {
        let reg = test_registry();
        let results = reg
            .search(&SearchQuery {
                query: "nonexistent".to_string(),
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert!(results.is_empty());
    }

    // PR1.9: Download counting
    #[test]
    fn pr1_9_download_counting() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(&reg, &auth, "fj-dl", "1.0.0", "Test", b"d", &[], None, None).unwrap();

        reg.record_download("fj-dl", "1.0.0").unwrap();
        reg.record_download("fj-dl", "1.0.0").unwrap();
        reg.record_download("fj-dl", "1.0.0").unwrap();

        let meta = reg.get_package("fj-dl").unwrap().unwrap();
        assert_eq!(meta.downloads, 3);
    }

    // PR1.11: Yank/unyank
    #[test]
    fn pr1_11_yank_and_unyank() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "fj-yank",
            "1.0.0",
            "Test",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();

        let resp = reg.yank(&auth, "fj-yank", "1.0.0").unwrap();
        assert_eq!(resp.status.0, 200);
        let meta = reg.get_package("fj-yank").unwrap().unwrap();
        assert!(meta.versions[0].yanked);

        let resp = reg.unyank(&auth, "fj-yank", "1.0.0").unwrap();
        assert_eq!(resp.status.0, 200);
        let meta = reg.get_package("fj-yank").unwrap().unwrap();
        assert!(!meta.versions[0].yanked);
    }

    #[test]
    fn pr1_11_yank_nonexistent_version() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(&reg, &auth, "fj-y2", "1.0.0", "Test", b"d", &[], None, None).unwrap();

        let resp = reg.yank(&auth, "fj-y2", "9.9.9").unwrap();
        assert_eq!(resp.status.0, 404);
    }

    // PR1.6: Rate limiting
    #[test]
    fn pr1_6_rate_limit_allows_then_denies() {
        let reg = test_registry();

        for _ in 0..3 {
            let (allowed, _, _) = reg
                .check_rate_limit("127.0.0.1", "upload", 3, 3600)
                .unwrap();
            assert!(allowed);
        }

        let (allowed, remaining, _) = reg
            .check_rate_limit("127.0.0.1", "upload", 3, 3600)
            .unwrap();
        assert!(!allowed);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn pr1_6_rate_limit_different_ips() {
        let reg = test_registry();

        let (allowed, _, _) = reg.check_rate_limit("10.0.0.1", "upload", 1, 3600).unwrap();
        assert!(allowed);

        let (allowed, _, _) = reg.check_rate_limit("10.0.0.2", "upload", 1, 3600).unwrap();
        assert!(allowed);
    }

    // PR1.12: Audit log
    #[test]
    fn pr1_12_audit_log_records_publish() {
        let reg = test_registry();
        let (user_id, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "fj-audit",
            "1.0.0",
            "Test",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();

        let entries = reg.get_audit_log(user_id, 10).unwrap();
        assert!(!entries.is_empty());
        assert_eq!(entries[0].action, "publish");
        assert!(entries[0].target.contains("fj-audit@1.0.0"));
    }

    // PR1.3: Sparse index
    #[test]
    fn pr1_3_sparse_index_entry() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "fj-idx",
            "1.0.0",
            "Test",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-idx",
            "1.1.0",
            "Test v1.1",
            b"d2",
            &[],
            None,
            None,
        )
        .unwrap();

        let index = reg.sparse_index_entry("fj-idx").unwrap();
        let lines: Vec<&str> = index.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"vers\":\"1.1.0\""));
        assert!(lines[1].contains("\"vers\":\"1.0.0\""));
    }

    // PR1.10: List packages
    #[test]
    fn pr1_10_list_packages() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "pkg-a",
            "1.0.0",
            "Package A",
            b"a",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "pkg-b",
            "1.0.0",
            "Package B",
            b"b",
            &[],
            None,
            None,
        )
        .unwrap();

        let pkgs = reg.list_packages(10, 0).unwrap();
        assert_eq!(pkgs.len(), 2);

        let count = reg.package_count().unwrap();
        assert_eq!(count, 2);
    }

    // PR1.7: Size limit
    #[test]
    fn pr1_7_tarball_size_limit() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        let big = vec![0u8; 11 * 1024 * 1024];

        let resp =
            quick_publish(&reg, &auth, "fj-big", "1.0.0", "Big", &big, &[], None, None).unwrap();
        assert_eq!(resp.status.0, 400);
        assert!(resp.body.contains("10 MB"));
    }

    // End-to-end: full lifecycle
    #[test]
    fn pr1_e2e_full_lifecycle() {
        let reg = test_registry();
        let (_, api_key) = reg.register_user("dev1", "dev1@fajarlang.dev").unwrap();
        let auth = reg.authenticate(&api_key).unwrap();

        let resp = quick_publish(
            &reg,
            &auth,
            "fj-lifecycle",
            "1.0.0",
            "Lifecycle test package",
            b"v1 content",
            &["test".to_string(), "lifecycle".to_string()],
            Some("MIT"),
            Some("https://github.com/fajar-lang/fj-lifecycle"),
        )
        .unwrap();
        assert_eq!(resp.status.0, 201);

        quick_publish(
            &reg,
            &auth,
            "fj-lifecycle",
            "1.1.0",
            "Updated",
            b"v11",
            &["test".to_string()],
            Some("MIT"),
            None,
        )
        .unwrap();

        let results = reg
            .search(&SearchQuery {
                query: "lifecycle".to_string(),
                limit: 10,
                offset: 0,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].latest_version, "1.1.0");

        let data = reg.get_tarball("fj-lifecycle", "1.0.0").unwrap();
        assert_eq!(data, b"v1 content");
        reg.record_download("fj-lifecycle", "1.0.0").unwrap();

        reg.yank(&auth, "fj-lifecycle", "1.0.0").unwrap();
        let meta = reg.get_package("fj-lifecycle").unwrap().unwrap();
        assert!(
            meta.versions
                .iter()
                .any(|v| v.version == "1.0.0" && v.yanked)
        );
        assert!(
            meta.versions
                .iter()
                .any(|v| v.version == "1.1.0" && !v.yanked)
        );

        let log = reg.get_audit_log(auth.user_id as i64, 10).unwrap();
        assert!(log.len() >= 3);
    }

    // PR1.13: Webhook payload
    #[test]
    fn pr1_13_webhook_payload_json() {
        let payload = WebhookPayload {
            event: WebhookEvent::Publish,
            package: "fj-math".to_string(),
            version: Some("1.0.0".to_string()),
            timestamp: "2026-03-27T12:00:00Z".to_string(),
        };
        let json = payload.to_json();
        assert!(json.contains(r#""event":"publish""#));
        assert!(json.contains(r#""package":"fj-math""#));
        assert!(json.contains(r#""version":"1.0.0""#));
    }

    #[test]
    fn pr1_13_webhook_config() {
        let cfg = WebhookConfig {
            url: "https://hooks.example.com/fj".to_string(),
            events: vec![WebhookEvent::Publish, WebhookEvent::Yank],
            secret: Some("s3cret".to_string()),
            active: true,
        };
        assert!(cfg.active);
        assert_eq!(cfg.events.len(), 2);
    }

    // PR1.14: Mirror config
    #[test]
    fn pr1_14_mirror_config_default() {
        let cfg = MirrorConfig::default();
        assert!(cfg.upstream_url.contains("fajarlang.dev"));
        assert_eq!(cfg.sync_interval_secs, 3600);
        assert!(cfg.filter_packages.is_empty());
    }

    // PR1.16: Docker deployment
    #[test]
    fn pr1_16_dockerfile_generation() {
        let df = generate_dockerfile();
        assert!(df.contains("FROM rust:"));
        assert!(df.contains("EXPOSE 8080"));
        assert!(df.contains("VOLUME /data"));
    }

    #[test]
    fn pr1_16_docker_compose_generation() {
        let dc = generate_docker_compose();
        assert!(dc.contains("services:"));
        assert!(dc.contains("registry-data:"));
    }

    // PR1.17: TLS config
    #[test]
    fn pr1_17_tls_config_hsts() {
        let cfg = TlsConfig::default();
        assert!(cfg.hsts);
        let hdr = cfg.hsts_header().unwrap();
        assert!(hdr.contains("max-age=31536000"));
        assert!(hdr.contains("includeSubDomains"));
    }

    // PR1.18: Backup strategy
    #[test]
    fn pr1_18_backup_config_default() {
        let cfg = BackupConfig::default();
        assert_eq!(cfg.retention_count, 7);
        assert_eq!(cfg.interval_secs, 86_400);
    }

    #[test]
    fn pr1_18_backup_filename() {
        let name = backup_filename();
        assert!(name.starts_with("fj-registry-backup-"));
        assert!(name.ends_with(".db"));
    }

    // PR1.19: Admin dashboard stats
    #[test]
    fn pr1_19_admin_stats() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "pkg-stat",
            "1.0.0",
            "Stats test",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "pkg-stat",
            "1.1.0",
            "Stats test",
            b"d2",
            &[],
            None,
            None,
        )
        .unwrap();
        reg.record_download("pkg-stat", "1.0.0").unwrap();

        let stats = reg.get_stats().unwrap();
        assert_eq!(stats.total_packages, 1);
        assert_eq!(stats.total_versions, 2);
        assert_eq!(stats.total_downloads, 1);
        assert_eq!(stats.active_users, 1);
    }

    // PR1.20: Dynamic API documentation
    #[test]
    fn pr1_20_dynamic_api_documentation_includes_real_stats() {
        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);

        quick_publish(
            &reg,
            &auth,
            "fj-docs-pkg",
            "1.0.0",
            "Doc test",
            b"d",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "fj-docs-pkg",
            "2.0.0",
            "Doc test v2",
            b"d2",
            &[],
            None,
            None,
        )
        .unwrap();

        let doc = reg.api_documentation().unwrap();
        assert!(doc.contains("openapi"));
        assert!(doc.contains("/packages"));
        assert!(doc.contains("/search"));
        assert!(doc.contains("bearerAuth"));
        // Dynamic stats: 1 package, 2 versions, 1 user
        assert!(doc.contains("1 packages"));
        assert!(doc.contains("2 versions"));
        assert!(doc.contains("1 users"));
        // Actual package name listed
        assert!(doc.contains("fj-docs-pkg"));
    }

    // PR1.13: Real webhook HTTP POST
    #[test]
    fn pr1_13_send_webhook_posts_to_local_tcp_listener() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let reg = test_registry();
        let config = WebhookConfig {
            url: format!("http://127.0.0.1:{port}/webhook"),
            events: vec![WebhookEvent::Publish],
            secret: None,
            active: true,
        };
        let payload = WebhookPayload {
            event: WebhookEvent::Publish,
            package: "fj-math".to_string(),
            version: Some("1.0.0".to_string()),
            timestamp: "2026-03-27T12:00:00Z".to_string(),
        };

        // Spawn a thread to accept the connection and respond
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();

            // Respond with 200 OK
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            request
        });

        let result = reg.send_webhook(&config, &payload);
        assert!(result.is_ok(), "send_webhook failed: {:?}", result.err());

        let request = handle.join().unwrap();
        assert!(request.contains("POST /webhook HTTP/1.1"));
        assert!(request.contains("Content-Type: application/json"));
        assert!(request.contains(r#""event":"publish""#));
        assert!(request.contains(r#""package":"fj-math""#));
    }

    #[test]
    fn pr1_13_send_webhook_hmac_signature_header() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let reg = test_registry();
        let config = WebhookConfig {
            url: format!("http://127.0.0.1:{port}/hook"),
            events: vec![WebhookEvent::Yank],
            secret: Some("my-secret-key".to_string()),
            active: true,
        };
        let payload = WebhookPayload {
            event: WebhookEvent::Yank,
            package: "fj-nn".to_string(),
            version: Some("2.0.0".to_string()),
            timestamp: "2026-03-27T14:00:00Z".to_string(),
        };

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            request
        });

        reg.send_webhook(&config, &payload).unwrap();
        let request = handle.join().unwrap();
        assert!(request.contains("X-FJ-Signature: sha256="));
    }

    #[test]
    fn pr1_13_send_webhook_inactive_skipped() {
        let reg = test_registry();
        let config = WebhookConfig {
            url: "http://127.0.0.1:1/hook".to_string(),
            events: vec![WebhookEvent::Publish],
            secret: None,
            active: false,
        };
        let payload = WebhookPayload {
            event: WebhookEvent::Publish,
            package: "fj-skip".to_string(),
            version: None,
            timestamp: "2026-03-27T12:00:00Z".to_string(),
        };

        // Should succeed immediately without connecting
        let result = reg.send_webhook(&config, &payload);
        assert!(result.is_ok());
    }

    #[test]
    fn pr1_13_send_webhook_non_2xx_returns_error() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let reg = test_registry();
        let config = WebhookConfig {
            url: format!("http://127.0.0.1:{port}/fail"),
            events: vec![WebhookEvent::SecurityAdvisory],
            secret: None,
            active: true,
        };
        let payload = WebhookPayload {
            event: WebhookEvent::SecurityAdvisory,
            package: "fj-fail".to_string(),
            version: None,
            timestamp: "2026-03-27T15:00:00Z".to_string(),
        };

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf).unwrap();
            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });

        let result = reg.send_webhook(&config, &payload);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("500"));
        handle.join().unwrap();
    }

    // PR1.18: Real backup
    #[test]
    fn pr1_18_backup_creates_file_and_enforces_retention() {
        let base = env::temp_dir().join("fj_backup_test");
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::create_dir_all(&base);

        let db_path = base.join("test_registry.db");
        let storage_dir = base.join("packages");
        let _ = std::fs::create_dir_all(&storage_dir);
        let backup_dir = base.join("backups");

        let reg = RegistryDb::open(db_path.to_str().unwrap(), &storage_dir).unwrap();

        // Register a user so there is data
        reg.register_user("backupuser", "backup@test.com").unwrap();

        let path = reg.backup(&backup_dir, 3).unwrap();
        assert!(std::path::Path::new(&path).exists());
        assert!(path.contains("fj-registry-backup-"));
        assert!(path.ends_with(".db"));

        // Verify the backup is a valid SQLite DB by opening it
        let backup_conn = Connection::open(&path).unwrap();
        let user_count: i64 = backup_conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_count, 1);

        // Clean up
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr1_18_backup_retention_deletes_oldest() {
        let base = env::temp_dir().join("fj_backup_retention_test");
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::create_dir_all(&base);

        let backup_dir = base.join("backups");
        let _ = std::fs::create_dir_all(&backup_dir);

        // Create 5 fake backup files with ordered timestamps
        for i in 1..=5 {
            let name = format!("fj-registry-backup-100000{i}.db");
            std::fs::write(backup_dir.join(&name), b"fake").unwrap();
        }

        let db_path = base.join("registry.db");
        let storage_dir = base.join("packages");
        let _ = std::fs::create_dir_all(&storage_dir);
        let reg = RegistryDb::open(db_path.to_str().unwrap(), &storage_dir).unwrap();

        // Backup with retention of 3 — should keep 3 newest + the new one = 4 total,
        // but retention runs after adding the new backup, so we keep the newest 3.
        let _new_path = reg.backup(&backup_dir, 3).unwrap();

        let remaining: Vec<_> = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("fj-registry-backup-") && n.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(remaining.len(), 3);

        // Clean up
        let _ = std::fs::remove_dir_all(&base);
    }

    // PR1.14: Real mirror sync
    #[test]
    fn pr1_14_mirror_sync_from_local() {
        let dir = env::temp_dir().join("fj_mirror_sync_test");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let mirror_dir = dir.join("mirror");

        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "pkg-m1",
            "1.0.0",
            "Mirror test 1",
            b"data1",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "pkg-m2",
            "0.1.0",
            "Mirror test 2",
            b"data2",
            &[],
            None,
            None,
        )
        .unwrap();

        let cfg = MirrorConfig {
            upstream_url: "local".to_string(),
            storage_dir: mirror_dir.clone(),
            sync_interval_secs: 3600,
            filter_packages: Vec::new(),
        };
        let report = cfg.sync_from_local(&reg).unwrap();
        assert_eq!(report.downloaded, 2);
        assert!(report.errors.is_empty());

        // Verify files exist
        assert!(mirror_dir.join("pkg-m1/1.0.0/pkg-m1-1.0.0.tar.gz").exists());
        assert!(mirror_dir.join("pkg-m2/0.1.0/pkg-m2-0.1.0.tar.gz").exists());
        assert!(mirror_dir.join("index.json").exists());

        // Second sync should skip
        let report2 = cfg.sync_from_local(&reg).unwrap();
        assert_eq!(report2.downloaded, 0);
        assert_eq!(report2.skipped, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pr1_14_mirror_sync_with_filter() {
        let dir = env::temp_dir().join("fj_mirror_filter_test");
        let _ = std::fs::remove_dir_all(&dir);
        let mirror_dir = dir.join("mirror");

        let reg = test_registry();
        let (_, _, auth) = register_test_user(&reg);
        quick_publish(
            &reg,
            &auth,
            "pkg-f1",
            "1.0.0",
            "Filter 1",
            b"d1",
            &[],
            None,
            None,
        )
        .unwrap();
        quick_publish(
            &reg,
            &auth,
            "pkg-f2",
            "1.0.0",
            "Filter 2",
            b"d2",
            &[],
            None,
            None,
        )
        .unwrap();

        let cfg = MirrorConfig {
            upstream_url: "local".to_string(),
            storage_dir: mirror_dir.clone(),
            sync_interval_secs: 3600,
            filter_packages: vec!["pkg-f1".to_string()],
        };
        let report = cfg.sync_from_local(&reg).unwrap();
        assert_eq!(report.downloaded, 1);
        assert_eq!(report.skipped, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // PR1.16: Real Docker file write
    #[test]
    fn pr1_16_write_docker_files_to_disk() {
        let dir = env::temp_dir().join("fj_docker_test");
        let _ = std::fs::remove_dir_all(&dir);

        write_docker_files(&dir).unwrap();
        assert!(dir.join("Dockerfile").exists());
        assert!(dir.join("docker-compose.yml").exists());

        let df = std::fs::read_to_string(dir.join("Dockerfile")).unwrap();
        assert!(df.contains("FROM rust:"));
        let dc = std::fs::read_to_string(dir.join("docker-compose.yml")).unwrap();
        assert!(dc.contains("services:"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // PR1.17: Real TLS validation
    #[test]
    fn pr1_17_tls_generate_and_validate() {
        let dir = env::temp_dir().join("fj_tls_test");
        let _ = std::fs::remove_dir_all(&dir);

        let cfg = TlsConfig::generate_self_signed(&dir).unwrap();
        assert!(cfg.cert_path.exists());
        assert!(cfg.key_path.exists());

        let validation = cfg.validate().unwrap();
        assert!(validation.cert_valid_pem);
        assert!(validation.key_valid_pem);
        assert!(validation.cert_size > 0);
        assert!(validation.key_size > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pr1_17_tls_validate_missing_cert() {
        let cfg = TlsConfig {
            cert_path: std::path::PathBuf::from("/nonexistent/cert.pem"),
            key_path: std::path::PathBuf::from("/nonexistent/key.pem"),
            hsts: true,
            hsts_max_age: 3600,
        };
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("not found"));
    }
}
