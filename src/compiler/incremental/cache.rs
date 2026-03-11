//! # Artifact Cache
//!
//! Stores and retrieves compiled artifacts keyed by content hash, compiler
//! version, target architecture, and optimization level. Supports pruning
//! by age and total size, and tracks hit/miss statistics.
//!
//! ## Design
//!
//! The cache operates in-memory with a directory-backed model. Each artifact
//! is identified by a composite [`CacheKey`] that ensures invalidation when
//! any compilation parameter changes. The cache is designed to be safe for
//! parallel lookups from multiple compilation workers.

use std::collections::HashMap;

use super::IncrementalError;

// ═══════════════════════════════════════════════════════════════════════
// CacheKey
// ═══════════════════════════════════════════════════════════════════════

/// Composite key for uniquely identifying a cached artifact.
///
/// The combined hash ensures that artifacts are invalidated when the source
/// content, compiler version, target, or optimization level changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Hash of the source file content.
    pub content_hash: String,
    /// Compiler version that produced this artifact.
    pub compiler_version: String,
    /// Target architecture (e.g., "x86_64", "aarch64", "riscv64").
    pub target: String,
    /// Optimization level (e.g., "O0", "O1", "O2", "O3").
    pub opt_level: String,
}

impl CacheKey {
    /// Creates a new cache key from its components.
    pub fn new(
        content_hash: String,
        compiler_version: String,
        target: String,
        opt_level: String,
    ) -> Self {
        Self {
            content_hash,
            compiler_version,
            target,
            opt_level,
        }
    }

    /// Returns a combined hash string representing this key.
    pub fn combined_hash(&self) -> String {
        let combined = format!(
            "{}:{}:{}:{}",
            self.content_hash, self.compiler_version, self.target, self.opt_level
        );
        let hash = fnv1a_hash_str(&combined);
        format!("{hash:016x}")
    }
}

/// FNV-1a string hashing for cache key combination.
fn fnv1a_hash_str(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

// ═══════════════════════════════════════════════════════════════════════
// ArtifactType
// ═══════════════════════════════════════════════════════════════════════

/// The type of compiled artifact stored in the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactType {
    /// Parsed abstract syntax tree (serialized).
    Ast,
    /// Type-checked / analyzed AST with semantic info.
    TypedAst,
    /// Intermediate representation (e.g., Cranelift IR).
    Ir,
    /// Native object file (.o).
    Object,
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactType::Ast => write!(f, "AST"),
            ArtifactType::TypedAst => write!(f, "TypedAST"),
            ArtifactType::Ir => write!(f, "IR"),
            ArtifactType::Object => write!(f, "Object"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CachedArtifact
// ═══════════════════════════════════════════════════════════════════════

/// A compiled artifact stored in the cache.
#[derive(Debug, Clone)]
pub struct CachedArtifact {
    /// The cache key that identifies this artifact.
    pub key: CacheKey,
    /// What kind of artifact this is.
    pub artifact_type: ArtifactType,
    /// The serialized artifact data.
    pub data: Vec<u8>,
    /// Timestamp (unix epoch seconds) when this was compiled.
    pub compiled_at: u64,
    /// Size of the artifact data in bytes.
    pub size_bytes: usize,
}

impl CachedArtifact {
    /// Creates a new cached artifact.
    pub fn new(
        key: CacheKey,
        artifact_type: ArtifactType,
        data: Vec<u8>,
        compiled_at: u64,
    ) -> Self {
        let size_bytes = data.len();
        Self {
            key,
            artifact_type,
            data,
            compiled_at,
            size_bytes,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PruneResult
// ═══════════════════════════════════════════════════════════════════════

/// Result of a cache pruning operation.
#[derive(Debug, Clone)]
pub struct PruneResult {
    /// Number of cache entries removed.
    pub entries_removed: usize,
    /// Total bytes freed by pruning.
    pub bytes_freed: usize,
}

// ═══════════════════════════════════════════════════════════════════════
// CacheStats
// ═══════════════════════════════════════════════════════════════════════

/// Statistics about cache usage.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits (successful lookups).
    pub hit_count: u64,
    /// Number of cache misses (failed lookups).
    pub miss_count: u64,
    /// Total size of all cached artifacts in bytes.
    pub total_size: usize,
    /// Number of entries currently in the cache.
    pub entry_count: usize,
}

impl CacheStats {
    /// Computes the cache hit rate as a percentage (0.0 - 100.0).
    ///
    /// Returns 0.0 if no lookups have been performed.
    pub fn hit_rate_pct(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            return 0.0;
        }
        (self.hit_count as f64 / total as f64) * 100.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ArtifactCache
// ═══════════════════════════════════════════════════════════════════════

/// In-memory artifact cache with statistics tracking.
///
/// Stores compiled artifacts keyed by a composite [`CacheKey`] that
/// includes content hash, compiler version, target, and opt level.
/// Supports store, lookup, invalidation, pruning, and stats.
#[derive(Debug)]
pub struct ArtifactCache {
    /// The cache directory path (for future disk-backed mode).
    pub cache_dir: String,
    /// Map from cache key to cached artifact.
    entries: HashMap<CacheKey, CachedArtifact>,
    /// Running statistics.
    stats: CacheStats,
}

impl ArtifactCache {
    /// Creates a new artifact cache with the given directory path.
    pub fn new(cache_dir: String) -> Self {
        Self {
            cache_dir,
            entries: HashMap::new(),
            stats: CacheStats::default(),
        }
    }

    /// Stores an artifact in the cache.
    ///
    /// If an artifact with the same key already exists, it is replaced.
    /// Updates the total size statistics accordingly.
    pub fn cache_store(
        &mut self,
        key: CacheKey,
        artifact: CachedArtifact,
    ) -> Result<(), IncrementalError> {
        // Remove old entry size if replacing
        if let Some(old) = self.entries.get(&key) {
            self.stats.total_size = self.stats.total_size.saturating_sub(old.size_bytes);
        }

        self.stats.total_size += artifact.size_bytes;
        self.entries.insert(key, artifact);
        self.stats.entry_count = self.entries.len();
        Ok(())
    }

    /// Looks up an artifact by its cache key.
    ///
    /// Records a hit or miss in the statistics.
    pub fn cache_lookup(&mut self, key: &CacheKey) -> Option<CachedArtifact> {
        match self.entries.get(key) {
            Some(artifact) => {
                self.stats.hit_count += 1;
                Some(artifact.clone())
            }
            None => {
                self.stats.miss_count += 1;
                None
            }
        }
    }

    /// Removes a specific artifact from the cache.
    pub fn cache_invalidate(&mut self, key: &CacheKey) {
        if let Some(artifact) = self.entries.remove(key) {
            self.stats.total_size = self.stats.total_size.saturating_sub(artifact.size_bytes);
            self.stats.entry_count = self.entries.len();
        }
    }

    /// Prunes the cache by removing entries older than `max_age_secs`
    /// or until total size is below `max_size_bytes`.
    ///
    /// Entries are evicted oldest-first when trimming by size.
    pub fn cache_prune(
        &mut self,
        max_age_secs: u64,
        max_size_bytes: usize,
        current_time: u64,
    ) -> PruneResult {
        let mut entries_removed = 0usize;
        let mut bytes_freed = 0usize;

        // Phase 1: Remove entries older than max_age
        let expired_keys = collect_expired_keys(&self.entries, max_age_secs, current_time);
        for key in &expired_keys {
            if let Some(artifact) = self.entries.remove(key) {
                bytes_freed += artifact.size_bytes;
                entries_removed += 1;
            }
        }

        // Phase 2: Evict oldest until under size limit
        while self.stats.total_size.saturating_sub(bytes_freed) > max_size_bytes {
            if let Some(oldest_key) = find_oldest_key(&self.entries) {
                if let Some(artifact) = self.entries.remove(&oldest_key) {
                    bytes_freed += artifact.size_bytes;
                    entries_removed += 1;
                }
            } else {
                break;
            }
        }

        self.stats.total_size = self.stats.total_size.saturating_sub(bytes_freed);
        self.stats.entry_count = self.entries.len();

        PruneResult {
            entries_removed,
            bytes_freed,
        }
    }

    /// Returns current cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hit_count: self.stats.hit_count,
            miss_count: self.stats.miss_count,
            total_size: self.stats.total_size,
            entry_count: self.entries.len(),
        }
    }

    /// Clears all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.total_size = 0;
        self.stats.entry_count = 0;
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Collects keys of entries older than the maximum age.
fn collect_expired_keys(
    entries: &HashMap<CacheKey, CachedArtifact>,
    max_age_secs: u64,
    current_time: u64,
) -> Vec<CacheKey> {
    entries
        .iter()
        .filter(|(_, artifact)| current_time.saturating_sub(artifact.compiled_at) > max_age_secs)
        .map(|(key, _)| key.clone())
        .collect()
}

/// Finds the cache key of the oldest entry.
fn find_oldest_key(entries: &HashMap<CacheKey, CachedArtifact>) -> Option<CacheKey> {
    entries
        .iter()
        .min_by_key(|(_, artifact)| artifact.compiled_at)
        .map(|(key, _)| key.clone())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(hash: &str) -> CacheKey {
        CacheKey::new(
            hash.to_string(),
            "0.8.0".to_string(),
            "x86_64".to_string(),
            "O2".to_string(),
        )
    }

    fn make_artifact(key: CacheKey, data: &[u8], compiled_at: u64) -> CachedArtifact {
        CachedArtifact::new(key, ArtifactType::Object, data.to_vec(), compiled_at)
    }

    #[test]
    fn s10_1_cache_key_combined_hash_deterministic() {
        let key1 = make_key("abc123");
        let key2 = make_key("abc123");
        assert_eq!(key1.combined_hash(), key2.combined_hash());

        let key3 = make_key("def456");
        assert_ne!(key1.combined_hash(), key3.combined_hash());
    }

    #[test]
    fn s10_2_cache_store_and_lookup() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());
        let key = make_key("hash1");
        let artifact = make_artifact(key.clone(), b"compiled code", 1000);

        cache.cache_store(key.clone(), artifact).expect("store ok");
        let result = cache.cache_lookup(&key);
        assert!(result.is_some());

        let retrieved = result.expect("artifact found");
        assert_eq!(retrieved.data, b"compiled code");
        assert_eq!(retrieved.artifact_type, ArtifactType::Object);
    }

    #[test]
    fn s10_3_cache_miss_records_stats() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());
        let key = make_key("nonexistent");

        let result = cache.cache_lookup(&key);
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.miss_count, 1);
        assert_eq!(stats.hit_count, 0);
    }

    #[test]
    fn s10_4_cache_invalidate_removes_entry() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());
        let key = make_key("to_remove");
        let artifact = make_artifact(key.clone(), b"data", 1000);

        cache.cache_store(key.clone(), artifact).expect("store ok");
        assert_eq!(cache.len(), 1);

        cache.cache_invalidate(&key);
        assert_eq!(cache.len(), 0);

        let result = cache.cache_lookup(&key);
        assert!(result.is_none());
    }

    #[test]
    fn s10_5_cache_prune_by_age() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());

        // Old entry (compiled at t=100)
        let key_old = make_key("old");
        cache
            .cache_store(key_old, make_artifact(make_key("old"), b"old data", 100))
            .expect("store ok");

        // Recent entry (compiled at t=900)
        let key_new = make_key("new");
        cache
            .cache_store(
                key_new.clone(),
                make_artifact(make_key("new"), b"new data", 900),
            )
            .expect("store ok");

        // Prune entries older than 500 seconds, current time = 1000
        let result = cache.cache_prune(500, usize::MAX, 1000);
        assert_eq!(result.entries_removed, 1);
        assert_eq!(result.bytes_freed, 8); // "old data" = 8 bytes
        assert_eq!(cache.len(), 1);

        // New entry should still be there
        let found = cache.cache_lookup(&key_new);
        assert!(found.is_some());
    }

    #[test]
    fn s10_6_cache_prune_by_size() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());

        // Add several entries
        for i in 0..5 {
            let key = make_key(&format!("file{i}"));
            let data = vec![0u8; 100]; // 100 bytes each
            cache
                .cache_store(
                    key,
                    make_artifact(make_key(&format!("file{i}")), &data, i as u64 * 100),
                )
                .expect("store ok");
        }

        assert_eq!(cache.len(), 5);
        assert_eq!(cache.stats().total_size, 500);

        // Prune to max 250 bytes (should remove oldest entries)
        let result = cache.cache_prune(u64::MAX, 250, 10000);
        assert!(result.entries_removed >= 2);
        assert!(cache.stats().total_size <= 300); // within size budget
    }

    #[test]
    fn s10_7_artifact_types() {
        assert_eq!(format!("{}", ArtifactType::Ast), "AST");
        assert_eq!(format!("{}", ArtifactType::TypedAst), "TypedAST");
        assert_eq!(format!("{}", ArtifactType::Ir), "IR");
        assert_eq!(format!("{}", ArtifactType::Object), "Object");
    }

    #[test]
    fn s10_8_cache_stats_hit_rate() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());
        let key = make_key("target");
        let artifact = make_artifact(key.clone(), b"data", 1000);
        cache.cache_store(key.clone(), artifact).expect("store ok");

        // 3 hits
        for _ in 0..3 {
            cache.cache_lookup(&key);
        }
        // 1 miss
        cache.cache_lookup(&make_key("missing"));

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 3);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate_pct() - 75.0).abs() < 0.01);
    }

    #[test]
    fn s10_9_cache_replace_updates_size() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());
        let key = make_key("replace_me");

        // Store 10-byte artifact
        let art1 = make_artifact(key.clone(), &vec![0u8; 10], 100);
        cache.cache_store(key.clone(), art1).expect("store ok");
        assert_eq!(cache.stats().total_size, 10);

        // Replace with 20-byte artifact
        let art2 = make_artifact(key.clone(), &vec![0u8; 20], 200);
        cache.cache_store(key, art2).expect("store ok");
        assert_eq!(cache.stats().total_size, 20);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn s10_10_cache_clear_resets_everything() {
        let mut cache = ArtifactCache::new("/tmp/fj-cache".to_string());

        for i in 0..10 {
            let key = make_key(&format!("item{i}"));
            cache
                .cache_store(
                    key,
                    make_artifact(make_key(&format!("item{i}")), b"x", i as u64),
                )
                .expect("store ok");
        }
        assert_eq!(cache.len(), 10);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats().total_size, 0);
        assert!(cache.is_empty());
    }
}
