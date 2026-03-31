//! Workspace and multi-target incremental compilation.
//!
//! Handles caching across workspace members, different targets (x86_64 vs aarch64),
//! different profiles (debug vs release), shared dependencies, optimal build order,
//! per-member timings, remote cache support, and cache statistics.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use super::compute_content_hash;

// ═══════════════════════════════════════════════════════════════════════
// I9.1: Workspace-Level Cache
// ═══════════════════════════════════════════════════════════════════════

/// A workspace with multiple members sharing an incremental cache.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Workspace root directory.
    pub root: String,
    /// Members: name → member config.
    pub members: HashMap<String, WorkspaceMemberConfig>,
    /// Shared cache at workspace root.
    pub cache_root: String,
}

/// Configuration for a workspace member.
#[derive(Debug, Clone)]
pub struct WorkspaceMemberConfig {
    /// Member name.
    pub name: String,
    /// Member root directory (relative to workspace).
    pub path: String,
    /// Dependencies on other workspace members.
    pub member_deps: Vec<String>,
    /// External dependencies.
    pub external_deps: Vec<String>,
}

impl Workspace {
    /// Create a new workspace.
    pub fn new(root: &str) -> Self {
        Self {
            root: root.to_string(),
            members: HashMap::new(),
            cache_root: format!("{root}/target/incremental"),
        }
    }

    /// Add a workspace member.
    pub fn add_member(&mut self, config: WorkspaceMemberConfig) {
        self.members.insert(config.name.clone(), config);
    }

    /// Get all member names.
    pub fn member_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.members.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get member count.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I9.2: Cross-Member Dependency Tracking
// ═══════════════════════════════════════════════════════════════════════

/// Determines which workspace members need rebuilding when one changes.
pub fn cross_member_invalidation(
    workspace: &Workspace,
    changed_members: &[String],
) -> HashSet<String> {
    let mut to_rebuild: HashSet<String> = changed_members.iter().cloned().collect();

    // Transitive propagation
    let mut changed = true;
    while changed {
        changed = false;
        for (name, config) in &workspace.members {
            if !to_rebuild.contains(name) {
                for dep in &config.member_deps {
                    if to_rebuild.contains(dep) {
                        to_rebuild.insert(name.clone());
                        changed = true;
                        break;
                    }
                }
            }
        }
    }

    to_rebuild
}

// ═══════════════════════════════════════════════════════════════════════
// I9.3 / I9.4: Per-Target and Per-Profile Caching
// ═══════════════════════════════════════════════════════════════════════

/// A cache partition key — combines target and profile for isolation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CachePartition {
    /// Target triple (e.g., "x86_64", "aarch64", "wasm32").
    pub target: String,
    /// Build profile (e.g., "debug", "release").
    pub profile: String,
    /// Feature flags (sorted for determinism).
    pub features: Vec<String>,
}

impl CachePartition {
    /// Create a new partition.
    pub fn new(target: &str, profile: &str, features: &[String]) -> Self {
        let mut features = features.to_vec();
        features.sort();
        Self {
            target: target.to_string(),
            profile: profile.to_string(),
            features,
        }
    }

    /// Cache subdirectory name.
    pub fn dir_name(&self) -> String {
        let feat_hash = if self.features.is_empty() {
            "default".to_string()
        } else {
            compute_content_hash(&self.features.join(","))
        };
        let suffix = if feat_hash.len() >= 8 { &feat_hash[..8] } else { &feat_hash };
        format!("{}-{}-{}", self.target, self.profile, suffix)
    }
}

/// Multi-partition cache — separate caches per target/profile.
#[derive(Debug, Clone, Default)]
pub struct MultiPartitionCache {
    /// Partition → set of cached module hashes.
    pub partitions: HashMap<CachePartition, HashSet<String>>,
}

impl MultiPartitionCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a cached module in a partition.
    pub fn record(&mut self, partition: &CachePartition, module_hash: &str) {
        self.partitions
            .entry(partition.clone())
            .or_default()
            .insert(module_hash.to_string());
    }

    /// Check if a module is cached in a partition.
    pub fn is_cached(&self, partition: &CachePartition, module_hash: &str) -> bool {
        self.partitions
            .get(partition)
            .map(|set| set.contains(module_hash))
            .unwrap_or(false)
    }

    /// Number of partitions.
    pub fn partition_count(&self) -> usize {
        self.partitions.len()
    }

    /// Total cached entries across all partitions.
    pub fn total_entries(&self) -> usize {
        self.partitions.values().map(|s| s.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I9.5: Shared Dependency Caching
// ═══════════════════════════════════════════════════════════════════════

/// Identifies shared dependencies that can be compiled once for all members.
pub fn find_shared_deps(workspace: &Workspace) -> Vec<String> {
    let mut dep_count: HashMap<String, usize> = HashMap::new();

    for config in workspace.members.values() {
        for dep in &config.external_deps {
            *dep_count.entry(dep.clone()).or_insert(0) += 1;
        }
    }

    // Shared = used by more than one member
    let mut shared: Vec<String> = dep_count
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(dep, _)| dep)
        .collect();
    shared.sort();
    shared
}

// ═══════════════════════════════════════════════════════════════════════
// I9.6: Workspace Build Order
// ═══════════════════════════════════════════════════════════════════════

/// Compute optimal build order for workspace members.
///
/// Returns levels — members at the same level can be built in parallel.
pub fn workspace_build_order(workspace: &Workspace) -> Vec<Vec<String>> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for (name, config) in &workspace.members {
        in_degree.entry(name.clone()).or_insert(0);
        for dep in &config.member_deps {
            if workspace.members.contains_key(dep) {
                *in_degree.entry(name.clone()).or_insert(0) += 1;
                dependents.entry(dep.clone()).or_default().push(name.clone());
            }
        }
    }

    let mut levels = Vec::new();
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(name, _)| name.clone())
        .collect();
    queue.sort();

    while !queue.is_empty() {
        let current_level = queue.clone();
        queue.clear();

        for name in &current_level {
            if let Some(deps) = dependents.get(name) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }
        queue.sort();

        levels.push(current_level);
    }

    levels
}

// ═══════════════════════════════════════════════════════════════════════
// I9.7: Workspace-Level Timings
// ═══════════════════════════════════════════════════════════════════════

/// Per-member build timing.
#[derive(Debug, Clone)]
pub struct MemberTiming {
    pub name: String,
    pub duration: Duration,
    pub modules_compiled: usize,
    pub cached: bool,
}

/// Format workspace timings as a table.
pub fn format_workspace_timings(timings: &[MemberTiming]) -> String {
    let mut out = String::from("Member          Time       Modules  Status\n");
    out.push_str("──────────────  ─────────  ───────  ──────\n");
    for t in timings {
        let status = if t.cached { "cached" } else { "built" };
        let dur = if t.duration.as_millis() < 1000 {
            format!("{}ms", t.duration.as_millis())
        } else {
            format!("{:.2}s", t.duration.as_secs_f64())
        };
        out.push_str(&format!(
            "{:<14}  {:>9}  {:>7}  {}\n",
            t.name, dur, t.modules_compiled, status
        ));
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════
// I9.8: Remote Cache (Optional)
// ═══════════════════════════════════════════════════════════════════════

/// Remote cache configuration.
#[derive(Debug, Clone)]
pub struct RemoteCacheConfig {
    /// Remote URL (e.g., "s3://bucket/cache/", "file:///shared/cache/").
    pub url: String,
    /// Whether remote cache is enabled.
    pub enabled: bool,
    /// Whether to push artifacts to remote.
    pub push: bool,
    /// Whether to pull artifacts from remote.
    pub pull: bool,
}

impl RemoteCacheConfig {
    /// Parse from `--cache-dir=URL` flag.
    pub fn from_url(url: &str) -> Self {
        Self {
            url: url.to_string(),
            enabled: true,
            push: true,
            pull: true,
        }
    }

    /// Disabled remote cache.
    pub fn disabled() -> Self {
        Self {
            url: String::new(),
            enabled: false,
            push: false,
            pull: false,
        }
    }

    /// Whether this is an S3-compatible remote.
    pub fn is_s3(&self) -> bool {
        self.url.starts_with("s3://")
    }

    /// Whether this is a local shared directory.
    pub fn is_local(&self) -> bool {
        self.url.starts_with("file://") || self.url.starts_with("/")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I9.9: Cache Statistics
// ═══════════════════════════════════════════════════════════════════════

/// Comprehensive cache statistics for `fj cache stats`.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceCacheStats {
    /// Total cache size in bytes.
    pub total_bytes: usize,
    /// Number of cached artifacts.
    pub total_entries: usize,
    /// Number of partitions (target/profile combos).
    pub partition_count: usize,
    /// Cache hit rate (percentage).
    pub hit_rate_pct: f64,
    /// Age of oldest entry (seconds).
    pub oldest_age_secs: u64,
    /// Age of newest entry (seconds).
    pub newest_age_secs: u64,
    /// Per-member stats.
    pub member_stats: HashMap<String, MemberCacheStats>,
}

/// Per-member cache statistics.
#[derive(Debug, Clone, Default)]
pub struct MemberCacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub hit_count: usize,
    pub miss_count: usize,
}

impl WorkspaceCacheStats {
    /// Format for `fj cache stats` output.
    pub fn format_display(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Cache size:    {} bytes ({:.1} MB)\n",
            self.total_bytes, self.total_bytes as f64 / 1_048_576.0));
        out.push_str(&format!("Entries:       {}\n", self.total_entries));
        out.push_str(&format!("Partitions:    {}\n", self.partition_count));
        out.push_str(&format!("Hit rate:      {:.1}%\n", self.hit_rate_pct));

        if !self.member_stats.is_empty() {
            out.push_str("\nPer-member:\n");
            let mut members: Vec<_> = self.member_stats.iter().collect();
            members.sort_by_key(|(name, _)| (*name).clone());
            for (name, stats) in members {
                out.push_str(&format!(
                    "  {}: {} entries, {:.1} MB\n",
                    name, stats.entries, stats.bytes as f64 / 1_048_576.0
                ));
            }
        }

        out
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — I9.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_workspace() -> Workspace {
        let mut ws = Workspace::new("/project");
        ws.add_member(WorkspaceMemberConfig {
            name: "core".into(), path: "core/".into(),
            member_deps: vec![], external_deps: vec!["serde".into(), "log".into()],
        });
        ws.add_member(WorkspaceMemberConfig {
            name: "lib".into(), path: "lib/".into(),
            member_deps: vec!["core".into()], external_deps: vec!["serde".into()],
        });
        ws.add_member(WorkspaceMemberConfig {
            name: "cli".into(), path: "cli/".into(),
            member_deps: vec!["lib".into(), "core".into()], external_deps: vec!["clap".into(), "log".into()],
        });
        ws.add_member(WorkspaceMemberConfig {
            name: "tests".into(), path: "tests/".into(),
            member_deps: vec!["lib".into()], external_deps: vec![],
        });
        ws
    }

    // ── I9.1: Workspace-level cache ──

    #[test]
    fn i9_1_workspace_setup() {
        let ws = sample_workspace();
        assert_eq!(ws.member_count(), 4);
        assert_eq!(ws.cache_root, "/project/target/incremental");
        assert_eq!(ws.member_names(), vec!["cli", "core", "lib", "tests"]);
    }

    // ── I9.2: Cross-member dependency tracking ──

    #[test]
    fn i9_2_cross_member_invalidation() {
        let ws = sample_workspace();
        let rebuild = cross_member_invalidation(&ws, &["core".into()]);
        assert!(rebuild.contains("core"));
        assert!(rebuild.contains("lib"));  // depends on core
        assert!(rebuild.contains("cli"));  // depends on lib + core
        assert!(rebuild.contains("tests")); // depends on lib
    }

    #[test]
    fn i9_2_leaf_change_no_cascade() {
        let ws = sample_workspace();
        let rebuild = cross_member_invalidation(&ws, &["cli".into()]);
        assert!(rebuild.contains("cli"));
        assert!(!rebuild.contains("core")); // core doesn't depend on cli
        assert!(!rebuild.contains("lib"));
    }

    // ── I9.3: Per-target caching ──

    #[test]
    fn i9_3_different_targets() {
        let p1 = CachePartition::new("x86_64", "debug", &[]);
        let p2 = CachePartition::new("aarch64", "debug", &[]);
        assert_ne!(p1.dir_name(), p2.dir_name());
    }

    // ── I9.4: Per-profile caching ──

    #[test]
    fn i9_4_different_profiles() {
        let p1 = CachePartition::new("x86_64", "debug", &[]);
        let p2 = CachePartition::new("x86_64", "release", &[]);
        assert_ne!(p1.dir_name(), p2.dir_name());

        let mut cache = MultiPartitionCache::new();
        cache.record(&p1, "mod_hash_1");
        assert!(cache.is_cached(&p1, "mod_hash_1"));
        assert!(!cache.is_cached(&p2, "mod_hash_1")); // different profile
    }

    #[test]
    fn i9_4_feature_flags_affect_partition() {
        let p1 = CachePartition::new("x86_64", "debug", &["native".into()]);
        let p2 = CachePartition::new("x86_64", "debug", &["llvm".into()]);
        assert_ne!(p1.dir_name(), p2.dir_name());
    }

    // ── I9.5: Shared dependency caching ──

    #[test]
    fn i9_5_find_shared_deps() {
        let ws = sample_workspace();
        let shared = find_shared_deps(&ws);
        assert!(shared.contains(&"serde".to_string())); // used by core + lib
        assert!(shared.contains(&"log".to_string()));   // used by core + cli
        assert!(!shared.contains(&"clap".to_string()));  // only cli
    }

    // ── I9.6: Workspace build order ──

    #[test]
    fn i9_6_build_order() {
        let ws = sample_workspace();
        let levels = workspace_build_order(&ws);

        // Level 0: core (no deps)
        assert_eq!(levels[0], vec!["core"]);
        // Level 1: lib (depends on core)
        assert_eq!(levels[1], vec!["lib"]);
        // Level 2: cli + tests (depend on lib, can be parallel)
        assert!(levels[2].contains(&"cli".to_string()));
        assert!(levels[2].contains(&"tests".to_string()));
    }

    // ── I9.7: Workspace timings ──

    #[test]
    fn i9_7_format_timings() {
        let timings = vec![
            MemberTiming { name: "core".into(), duration: Duration::from_millis(500), modules_compiled: 10, cached: false },
            MemberTiming { name: "lib".into(), duration: Duration::from_millis(200), modules_compiled: 5, cached: false },
            MemberTiming { name: "cli".into(), duration: Duration::from_millis(50), modules_compiled: 0, cached: true },
        ];
        let table = format_workspace_timings(&timings);
        assert!(table.contains("core"));
        assert!(table.contains("500ms"));
        assert!(table.contains("cached"));
    }

    // ── I9.8: Remote cache ──

    #[test]
    fn i9_8_remote_cache_config() {
        let s3 = RemoteCacheConfig::from_url("s3://my-bucket/cache/");
        assert!(s3.enabled);
        assert!(s3.is_s3());
        assert!(!s3.is_local());

        let local = RemoteCacheConfig::from_url("/shared/cache/");
        assert!(local.is_local());

        let disabled = RemoteCacheConfig::disabled();
        assert!(!disabled.enabled);
    }

    // ── I9.9: Cache statistics ──

    #[test]
    fn i9_9_cache_stats_display() {
        let mut stats = WorkspaceCacheStats {
            total_bytes: 52_428_800, // 50 MB
            total_entries: 200,
            partition_count: 3,
            hit_rate_pct: 87.5,
            oldest_age_secs: 3600,
            newest_age_secs: 60,
            member_stats: HashMap::new(),
        };
        stats.member_stats.insert("core".into(), MemberCacheStats {
            entries: 50, bytes: 10_000_000, hit_count: 40, miss_count: 10,
        });

        let display = stats.format_display();
        assert!(display.contains("50.0 MB"));
        assert!(display.contains("200"));
        assert!(display.contains("87.5%"));
        assert!(display.contains("core"));
    }

    // ── I9.10: Integration ──

    #[test]
    fn i9_10_full_workspace_pipeline() {
        let ws = sample_workspace();

        // Build order
        let levels = workspace_build_order(&ws);
        assert_eq!(levels.len(), 3);

        // Shared deps
        let shared = find_shared_deps(&ws);
        assert!(!shared.is_empty());

        // Multi-partition cache
        let mut cache = MultiPartitionCache::new();
        let debug = CachePartition::new("x86_64", "debug", &[]);
        let release = CachePartition::new("x86_64", "release", &[]);

        for member in ws.member_names() {
            let hash = compute_content_hash(&member);
            cache.record(&debug, &hash);
        }
        assert_eq!(cache.partition_count(), 1);
        assert_eq!(cache.total_entries(), 4);

        // Invalidation: core changes → all rebuild
        let rebuild = cross_member_invalidation(&ws, &["core".into()]);
        assert_eq!(rebuild.len(), 4); // all 4 members

        // tests changes → only tests
        let rebuild = cross_member_invalidation(&ws, &["tests".into()]);
        assert_eq!(rebuild.len(), 1);

        // Release partition is separate
        cache.record(&release, "release_core");
        assert_eq!(cache.partition_count(), 2);
        assert!(!cache.is_cached(&debug, "release_core"));
        assert!(cache.is_cached(&release, "release_core"));
    }
}
