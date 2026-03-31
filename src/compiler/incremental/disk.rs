//! Persistent disk cache for incremental compilation.
//!
//! Extends the in-memory `ArtifactCache` with disk-backed storage.
//! Artifacts are persisted to `target/incremental/{hash}/` with metadata
//! and checksums for corruption detection.
//!
//! # Directory Layout
//!
//! ```text
//! target/incremental/
//!   metadata.json          ← compiler version, config hash
//!   hashes.json            ← file path → content SHA256
//!   artifacts/
//!     {key_hash}.bin       ← serialized artifact data
//!     {key_hash}.meta      ← artifact metadata (type, size, timestamp)
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::cache::{ArtifactType, CachedArtifact};
use super::IncrementalError;

/// Default cache size limit: 1 GB.
const DEFAULT_CACHE_LIMIT: usize = 1_073_741_824;

/// Magic bytes for artifact files (for corruption detection).
const ARTIFACT_MAGIC: &[u8; 4] = b"FJIC"; // Fajar Incremental Cache

// ═══════════════════════════════════════════════════════════════════════
// I1.1: Cache Directory Layout
// ═══════════════════════════════════════════════════════════════════════

/// Persistent disk cache configuration.
#[derive(Debug, Clone)]
pub struct DiskCacheConfig {
    /// Root directory for incremental cache (default: `target/incremental`).
    pub cache_dir: PathBuf,
    /// Maximum cache size in bytes.
    pub size_limit: usize,
    /// Compiler version (for invalidation).
    pub compiler_version: String,
    /// Target architecture.
    pub target: String,
    /// Optimization level.
    pub opt_level: String,
}

impl DiskCacheConfig {
    /// Creates a default config for the given project root.
    pub fn new(project_root: &str) -> Self {
        Self {
            cache_dir: PathBuf::from(project_root).join("target").join("incremental"),
            size_limit: DEFAULT_CACHE_LIMIT,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            target: "x86_64".to_string(),
            opt_level: "O0".to_string(),
        }
    }

    /// Artifacts subdirectory.
    pub fn artifacts_dir(&self) -> PathBuf {
        self.cache_dir.join("artifacts")
    }

    /// Path to metadata.json.
    pub fn metadata_path(&self) -> PathBuf {
        self.cache_dir.join("metadata.json")
    }

    /// Path to hashes.json.
    pub fn hashes_path(&self) -> PathBuf {
        self.cache_dir.join("hashes.json")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I1.2 / I1.3: Artifact Serialization & Deserialization
// ═══════════════════════════════════════════════════════════════════════

/// Serialized artifact on disk — magic + checksum + data.
#[derive(Debug, Clone)]
pub struct SerializedArtifact {
    /// Artifact type.
    pub artifact_type: ArtifactType,
    /// Raw data bytes.
    pub data: Vec<u8>,
    /// Timestamp.
    pub compiled_at: u64,
    /// Checksum of data (FNV-1a).
    pub checksum: u64,
}

impl SerializedArtifact {
    /// Serialize an artifact to bytes for disk storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Magic
        buf.extend_from_slice(ARTIFACT_MAGIC);
        // Type (1 byte)
        buf.push(match self.artifact_type {
            ArtifactType::Ast => 0,
            ArtifactType::TypedAst => 1,
            ArtifactType::Ir => 2,
            ArtifactType::Object => 3,
        });
        // Timestamp (8 bytes LE)
        buf.extend_from_slice(&self.compiled_at.to_le_bytes());
        // Checksum (8 bytes LE)
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        // Data length (8 bytes LE)
        buf.extend_from_slice(&(self.data.len() as u64).to_le_bytes());
        // Data
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Deserialize from bytes. Returns error on corruption.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, IncrementalError> {
        if bytes.len() < 29 {
            // 4 magic + 1 type + 8 timestamp + 8 checksum + 8 length = 29 header
            return Err(IncrementalError::CacheCorruption {
                message: "artifact too small".into(),
            });
        }

        // Validate magic
        if &bytes[0..4] != ARTIFACT_MAGIC {
            return Err(IncrementalError::CacheCorruption {
                message: "invalid magic bytes".into(),
            });
        }

        // Parse type
        let artifact_type = match bytes[4] {
            0 => ArtifactType::Ast,
            1 => ArtifactType::TypedAst,
            2 => ArtifactType::Ir,
            3 => ArtifactType::Object,
            _ => {
                return Err(IncrementalError::CacheCorruption {
                    message: format!("unknown artifact type: {}", bytes[4]),
                })
            }
        };

        // Parse timestamp
        let timestamp = u64::from_le_bytes(bytes[5..13].try_into().map_err(|_| {
            IncrementalError::CacheCorruption {
                message: "bad timestamp".into(),
            }
        })?);

        // Parse checksum
        let checksum = u64::from_le_bytes(bytes[13..21].try_into().map_err(|_| {
            IncrementalError::CacheCorruption {
                message: "bad checksum".into(),
            }
        })?);

        // Parse data length
        let data_len = u64::from_le_bytes(bytes[21..29].try_into().map_err(|_| {
            IncrementalError::CacheCorruption {
                message: "bad data length".into(),
            }
        })?) as usize;

        if bytes.len() < 29 + data_len {
            return Err(IncrementalError::CacheCorruption {
                message: format!(
                    "truncated data: expected {} bytes, got {}",
                    data_len,
                    bytes.len() - 29
                ),
            });
        }

        let data = bytes[29..29 + data_len].to_vec();

        // Validate checksum
        let actual_checksum = fnv1a_hash(&data);
        if actual_checksum != checksum {
            return Err(IncrementalError::CacheCorruption {
                message: format!(
                    "checksum mismatch: expected {:016x}, got {:016x}",
                    checksum, actual_checksum
                ),
            });
        }

        Ok(Self {
            artifact_type,
            data,
            compiled_at: timestamp,
            checksum,
        })
    }

    /// Create from a CachedArtifact.
    pub fn from_artifact(artifact: &CachedArtifact) -> Self {
        let checksum = fnv1a_hash(&artifact.data);
        Self {
            artifact_type: artifact.artifact_type.clone(),
            data: artifact.data.clone(),
            compiled_at: artifact.compiled_at,
            checksum,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I1.4: Content Hash Persistence
// ═══════════════════════════════════════════════════════════════════════

/// Persistent content hash store — maps file paths to SHA256 hashes.
#[derive(Debug, Clone, Default)]
pub struct HashStore {
    /// File path → content hash.
    pub hashes: HashMap<String, String>,
}

impl HashStore {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        let mut entries: Vec<(&str, &str)> = self
            .hashes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        entries.sort_by_key(|(k, _)| *k);
        let mut json = String::from("{\n");
        for (i, (k, v)) in entries.iter().enumerate() {
            if i > 0 {
                json.push_str(",\n");
            }
            json.push_str(&format!("  \"{k}\": \"{v}\""));
        }
        json.push_str("\n}");
        json
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, IncrementalError> {
        let mut hashes = HashMap::new();
        for line in json.lines() {
            let line = line.trim().trim_end_matches(',');
            if line.starts_with('"') && line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim().trim_matches('"');
                    let val = parts[1].trim().trim_matches('"');
                    if !key.is_empty() && !val.is_empty() {
                        hashes.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }
        Ok(Self { hashes })
    }

    /// Check if a file has changed since last cached.
    pub fn has_changed(&self, path: &str, current_hash: &str) -> bool {
        match self.hashes.get(path) {
            Some(cached) => cached != current_hash,
            None => true, // New file
        }
    }

    /// Update the hash for a file.
    pub fn update(&mut self, path: &str, hash: &str) {
        self.hashes.insert(path.to_string(), hash.to_string());
    }

    /// Remove entry for a deleted file.
    pub fn remove(&mut self, path: &str) {
        self.hashes.remove(path);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I1.5: Cache Invalidation
// ═══════════════════════════════════════════════════════════════════════

/// Cache metadata — stored at `metadata.json`.
#[derive(Debug, Clone)]
pub struct CacheMetadata {
    /// Compiler version.
    pub compiler_version: String,
    /// Config hash (fj.toml + Cargo.toml content).
    pub config_hash: String,
    /// Target architecture.
    pub target: String,
    /// Optimization level.
    pub opt_level: String,
}

impl CacheMetadata {
    /// Check if the current config matches the cached metadata.
    pub fn is_valid(&self, config: &DiskCacheConfig) -> bool {
        self.compiler_version == config.compiler_version
            && self.target == config.target
            && self.opt_level == config.opt_level
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"compiler_version\": \"{}\",\n  \"config_hash\": \"{}\",\n  \"target\": \"{}\",\n  \"opt_level\": \"{}\"\n}}",
            self.compiler_version, self.config_hash, self.target, self.opt_level
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I1.6: LRU Cache Eviction
// ═══════════════════════════════════════════════════════════════════════

/// Manages LRU eviction for the disk cache.
#[derive(Debug, Clone, Default)]
pub struct CacheEvictor {
    /// Entries sorted by last access time: (key_hash, size, timestamp).
    entries: Vec<(String, usize, u64)>,
    /// Size limit in bytes.
    size_limit: usize,
}

impl CacheEvictor {
    /// Creates a new evictor with the given size limit.
    pub fn new(size_limit: usize) -> Self {
        Self {
            entries: Vec::new(),
            size_limit,
        }
    }

    /// Records an entry.
    pub fn record(&mut self, key_hash: &str, size: usize, timestamp: u64) {
        // Update if exists, otherwise add
        if let Some(entry) = self.entries.iter_mut().find(|(k, _, _)| k == key_hash) {
            entry.1 = size;
            entry.2 = timestamp;
        } else {
            self.entries
                .push((key_hash.to_string(), size, timestamp));
        }
    }

    /// Evict oldest entries until total size is under the limit.
    /// Returns the key hashes of evicted entries.
    pub fn evict(&mut self) -> Vec<String> {
        let total: usize = self.entries.iter().map(|(_, s, _)| s).sum();
        if total <= self.size_limit {
            return Vec::new();
        }

        // Sort by timestamp (oldest first)
        self.entries.sort_by_key(|(_, _, ts)| *ts);

        let mut evicted = Vec::new();
        let mut current_total = total;
        while current_total > self.size_limit && !self.entries.is_empty() {
            let (key, size, _) = self.entries.remove(0);
            current_total -= size;
            evicted.push(key);
        }
        evicted
    }

    /// Total cache size in bytes.
    pub fn total_size(&self) -> usize {
        self.entries.iter().map(|(_, s, _)| s).sum()
    }

    /// Number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I1.7 / I1.8: Corruption Detection & Atomic Writes
// ═══════════════════════════════════════════════════════════════════════

/// Write data to a file atomically (write to temp, then rename).
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), IncrementalError> {
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, data).map_err(|e| IncrementalError::IoError {
        message: format!("write to {}: {}", temp_path.display(), e),
    })?;
    std::fs::rename(&temp_path, path).map_err(|e| IncrementalError::IoError {
        message: format!("rename {} -> {}: {}", temp_path.display(), path.display(), e),
    })?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// I1.9: fj clean --incremental
// ═══════════════════════════════════════════════════════════════════════

/// Remove the entire incremental cache directory.
pub fn clean_cache(cache_dir: &Path) -> Result<usize, IncrementalError> {
    if !cache_dir.exists() {
        return Ok(0);
    }
    let size = dir_size(cache_dir);
    std::fs::remove_dir_all(cache_dir).map_err(|e| IncrementalError::IoError {
        message: format!("remove {}: {}", cache_dir.display(), e),
    })?;
    Ok(size)
}

/// Calculate total size of a directory.
fn dir_size(path: &Path) -> usize {
    if path.is_file() {
        return path.metadata().map(|m| m.len() as usize).unwrap_or(0);
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| dir_size(&e.path()))
                .sum()
        })
        .unwrap_or(0)
}

/// FNV-1a hash for data checksums.
fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — I1.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::incremental::cache::CacheKey;

    // ── I1.1: Cache directory layout ──

    #[test]
    fn i1_1_config_paths() {
        let cfg = DiskCacheConfig::new("/project");
        assert_eq!(cfg.cache_dir, PathBuf::from("/project/target/incremental"));
        assert_eq!(cfg.artifacts_dir(), PathBuf::from("/project/target/incremental/artifacts"));
        assert!(cfg.metadata_path().to_str().unwrap().ends_with("metadata.json"));
        assert!(cfg.hashes_path().to_str().unwrap().ends_with("hashes.json"));
    }

    // ── I1.2 / I1.3: Serialization round-trip ──

    #[test]
    fn i1_2_serialize_deserialize_artifact() {
        let artifact = CachedArtifact::new(
            CacheKey::new("abc123".into(), "10.0.0".into(), "x86_64".into(), "O2".into()),
            ArtifactType::TypedAst,
            vec![1, 2, 3, 4, 5],
            1000,
        );
        let serialized = SerializedArtifact::from_artifact(&artifact);
        let bytes = serialized.to_bytes();
        let deserialized = SerializedArtifact::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.artifact_type, ArtifactType::TypedAst);
        assert_eq!(deserialized.data, vec![1, 2, 3, 4, 5]);
        assert_eq!(deserialized.compiled_at, 1000);
    }

    #[test]
    fn i1_3_corruption_detected() {
        let artifact = CachedArtifact::new(
            CacheKey::new("abc".into(), "1.0".into(), "x86_64".into(), "O0".into()),
            ArtifactType::Ast,
            vec![10, 20, 30],
            500,
        );
        let serialized = SerializedArtifact::from_artifact(&artifact);
        let mut bytes = serialized.to_bytes();
        // Corrupt a data byte
        if let Some(last) = bytes.last_mut() {
            *last ^= 0xFF;
        }
        let result = SerializedArtifact::from_bytes(&bytes);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("checksum") || err.contains("corrupt") || err.contains("truncated"));
    }

    #[test]
    fn i1_3_bad_magic_detected() {
        let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let result = SerializedArtifact::from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("magic"));
    }

    // ── I1.4: Content hash persistence ──

    #[test]
    fn i1_4_hash_store_round_trip() {
        let mut store = HashStore::default();
        store.update("src/main.fj", "aabbcc");
        store.update("src/lib.fj", "112233");

        let json = store.to_json();
        let restored = HashStore::from_json(&json).unwrap();
        assert_eq!(restored.hashes.get("src/main.fj"), Some(&"aabbcc".to_string()));
        assert_eq!(restored.hashes.get("src/lib.fj"), Some(&"112233".to_string()));
    }

    #[test]
    fn i1_4_hash_change_detection() {
        let mut store = HashStore::default();
        store.update("a.fj", "hash1");

        assert!(!store.has_changed("a.fj", "hash1")); // same hash
        assert!(store.has_changed("a.fj", "hash2"));  // different hash
        assert!(store.has_changed("b.fj", "hash1"));  // new file
    }

    // ── I1.5: Cache invalidation ──

    #[test]
    fn i1_5_metadata_validation() {
        let config = DiskCacheConfig::new("/project");
        let meta = CacheMetadata {
            compiler_version: config.compiler_version.clone(),
            config_hash: "abc".into(),
            target: config.target.clone(),
            opt_level: config.opt_level.clone(),
        };
        assert!(meta.is_valid(&config));

        let stale_meta = CacheMetadata {
            compiler_version: "9.0.0".into(), // old version
            config_hash: "abc".into(),
            target: "x86_64".into(),
            opt_level: "O0".into(),
        };
        assert!(!stale_meta.is_valid(&config));
    }

    // ── I1.6: LRU eviction ──

    #[test]
    fn i1_6_lru_eviction() {
        let mut evictor = CacheEvictor::new(100); // 100 byte limit

        evictor.record("old", 40, 1);
        evictor.record("mid", 40, 2);
        evictor.record("new", 40, 3);

        // Total = 120 > 100, should evict oldest
        let evicted = evictor.evict();
        assert_eq!(evicted, vec!["old"]);
        assert_eq!(evictor.total_size(), 80);
    }

    #[test]
    fn i1_6_no_eviction_under_limit() {
        let mut evictor = CacheEvictor::new(1000);
        evictor.record("a", 100, 1);
        evictor.record("b", 100, 2);
        let evicted = evictor.evict();
        assert!(evicted.is_empty());
    }

    // ── I1.7: Corruption detection (covered by I1.3 tests) ──

    #[test]
    fn i1_7_truncated_file() {
        let bytes = vec![b'F', b'J', b'I', b'C', 0]; // Too short
        let result = SerializedArtifact::from_bytes(&bytes);
        assert!(result.is_err());
    }

    // ── I1.8: Atomic writes ──

    #[test]
    fn i1_8_atomic_write() {
        let dir = std::env::temp_dir();
        let path = dir.join("fj_test_atomic.bin");
        atomic_write(&path, b"hello incremental").unwrap();
        let contents = std::fs::read(&path).unwrap();
        assert_eq!(contents, b"hello incremental");
        // Temp file should not exist
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_file(&path).ok();
    }

    // ── I1.9: fj clean --incremental ──

    #[test]
    fn i1_9_clean_cache() {
        let dir = std::env::temp_dir().join("fj_test_clean_cache");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.bin"), b"data").unwrap();

        let freed = clean_cache(&dir).unwrap();
        assert!(freed > 0);
        assert!(!dir.exists());
    }

    #[test]
    fn i1_9_clean_nonexistent() {
        let dir = PathBuf::from("/tmp/fj_nonexistent_cache_12345");
        let freed = clean_cache(&dir).unwrap();
        assert_eq!(freed, 0);
    }

    // ── I1.10: Integration ──

    #[test]
    fn i1_10_full_cache_workflow() {
        // Create artifact
        let key = CacheKey::new("hash1".into(), "10.0.0".into(), "x86_64".into(), "O0".into());
        let artifact = CachedArtifact::new(key, ArtifactType::TypedAst, vec![42; 100], 1000);

        // Serialize
        let serialized = SerializedArtifact::from_artifact(&artifact);
        let bytes = serialized.to_bytes();

        // Write atomically
        let dir = std::env::temp_dir();
        let path = dir.join("fj_test_workflow.bin");
        atomic_write(&path, &bytes).unwrap();

        // Read back and deserialize
        let read_bytes = std::fs::read(&path).unwrap();
        let restored = SerializedArtifact::from_bytes(&read_bytes).unwrap();
        assert_eq!(restored.data.len(), 100);
        assert_eq!(restored.artifact_type, ArtifactType::TypedAst);

        // Hash store
        let mut hashes = HashStore::default();
        hashes.update("main.fj", "abc123");
        let json = hashes.to_json();
        let hash_path = dir.join("fj_test_hashes.json");
        atomic_write(&hash_path, json.as_bytes()).unwrap();

        let read_json = std::fs::read_to_string(&hash_path).unwrap();
        let restored_hashes = HashStore::from_json(&read_json).unwrap();
        assert!(!restored_hashes.has_changed("main.fj", "abc123"));

        // Cleanup
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(&hash_path).ok();
    }
}
