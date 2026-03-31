//! IR-level caching — cache compiled intermediate representations and object files
//! for all three backends (Cranelift, LLVM, Bytecode VM).
//!
//! When a module hasn't changed, we skip codegen entirely and reuse the cached
//! IR/object. This is the biggest speedup in incremental compilation.
//!
//! # Cache Layers
//!
//! ```text
//! Source → Parse → Analyze → [cache check] → Codegen → [cache store] → Link
//!                                ↓ hit                      ↓ miss
//!                          Cached IR/Object            Fresh IR/Object
//! ```

use std::collections::HashMap;

use super::cache::ArtifactType;
use super::compute_content_hash;

// ═══════════════════════════════════════════════════════════════════════
// I3.5: Cache Key Computation
// ═══════════════════════════════════════════════════════════════════════

/// A cache key for IR-level artifacts.
///
/// Composed of: source hash + compiler version + target + opt level + dependency hashes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IrCacheKey {
    /// Hash of the source module content.
    pub source_hash: String,
    /// Compiler version.
    pub compiler_version: String,
    /// Target triple (e.g., "x86_64-unknown-linux").
    pub target: String,
    /// Optimization level (e.g., "O0", "O2").
    pub opt_level: String,
    /// Hashes of all dependencies (sorted for determinism).
    pub dep_hashes: Vec<String>,
}

impl IrCacheKey {
    /// Compute a single combined hash for this key.
    pub fn combined_hash(&self) -> String {
        let mut combined = format!(
            "{}:{}:{}:{}",
            self.source_hash, self.compiler_version, self.target, self.opt_level
        );
        for dh in &self.dep_hashes {
            combined.push(':');
            combined.push_str(dh);
        }
        compute_content_hash(&combined)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I3.1-I3.4: Backend-Specific Cache Entries
// ═══════════════════════════════════════════════════════════════════════

/// Cached IR for a single module.
#[derive(Debug, Clone)]
pub struct CachedIr {
    /// Cache key that produced this entry.
    pub key: IrCacheKey,
    /// The backend this IR targets.
    pub backend: BackendKind,
    /// Serialized IR data.
    pub data: Vec<u8>,
    /// Size in bytes.
    pub size: usize,
    /// Whether debug info is included.
    pub has_debug_info: bool,
}

/// Which backend produced the cached IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    /// I3.1: Cranelift IR.
    Cranelift,
    /// I3.2: LLVM bitcode (.bc).
    Llvm,
    /// I3.3: Bytecode VM.
    Bytecode,
    /// I3.4: Native object file (.o).
    Object,
    /// I3.7: Debug info (DWARF).
    DebugInfo,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::Cranelift => write!(f, "cranelift"),
            BackendKind::Llvm => write!(f, "llvm"),
            BackendKind::Bytecode => write!(f, "bytecode"),
            BackendKind::Object => write!(f, "object"),
            BackendKind::DebugInfo => write!(f, "debug"),
        }
    }
}

impl BackendKind {
    /// File extension for cached artifacts.
    pub fn extension(&self) -> &str {
        match self {
            BackendKind::Cranelift => ".clif",
            BackendKind::Llvm => ".bc",
            BackendKind::Bytecode => ".fjbc",
            BackendKind::Object => ".o",
            BackendKind::DebugInfo => ".dwarf",
        }
    }

    /// Map to ArtifactType.
    pub fn artifact_type(&self) -> ArtifactType {
        match self {
            BackendKind::Cranelift | BackendKind::Llvm | BackendKind::Bytecode => ArtifactType::Ir,
            BackendKind::Object => ArtifactType::Object,
            BackendKind::DebugInfo => ArtifactType::Ir,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I3.9: Cache Hit Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Statistics for IR cache usage.
#[derive(Debug, Clone, Default)]
pub struct IrCacheStats {
    /// Cache hits per backend.
    pub hits: HashMap<BackendKind, usize>,
    /// Cache misses per backend.
    pub misses: HashMap<BackendKind, usize>,
    /// Total bytes loaded from cache.
    pub bytes_loaded: usize,
    /// Total bytes stored to cache.
    pub bytes_stored: usize,
    /// Time saved by cache hits (estimated, in microseconds).
    pub time_saved_us: u64,
}

impl IrCacheStats {
    /// Record a cache hit.
    pub fn record_hit(&mut self, backend: BackendKind, bytes: usize) {
        *self.hits.entry(backend).or_insert(0) += 1;
        self.bytes_loaded += bytes;
    }

    /// Record a cache miss.
    pub fn record_miss(&mut self, backend: BackendKind) {
        *self.misses.entry(backend).or_insert(0) += 1;
    }

    /// Record bytes stored.
    pub fn record_store(&mut self, bytes: usize) {
        self.bytes_stored += bytes;
    }

    /// Total hits across all backends.
    pub fn total_hits(&self) -> usize {
        self.hits.values().sum()
    }

    /// Total misses across all backends.
    pub fn total_misses(&self) -> usize {
        self.misses.values().sum()
    }

    /// Hit rate as percentage.
    pub fn hit_rate_pct(&self) -> f64 {
        let total = self.total_hits() + self.total_misses();
        if total == 0 {
            0.0
        } else {
            (self.total_hits() as f64 / total as f64) * 100.0
        }
    }

    /// Format as display string: `Cache: 45/50 hit (90.0%)`
    pub fn display_summary(&self) -> String {
        let hits = self.total_hits();
        let total = hits + self.total_misses();
        format!(
            "Cache: {}/{} hit ({:.1}%)",
            hits,
            total,
            self.hit_rate_pct()
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IR Cache Store
// ═══════════════════════════════════════════════════════════════════════

/// In-memory IR cache with per-backend storage.
#[derive(Debug, Clone, Default)]
pub struct IrCacheStore {
    /// Cached entries: combined_hash → CachedIr.
    entries: HashMap<String, CachedIr>,
    /// Statistics.
    pub stats: IrCacheStats,
}

impl IrCacheStore {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// I3.1-I3.4: Store a compiled IR artifact.
    pub fn store(&mut self, ir: CachedIr) {
        let hash = ir.key.combined_hash();
        self.stats.record_store(ir.size);
        self.entries.insert(hash, ir);
    }

    /// Look up a cached IR artifact.
    pub fn lookup(&mut self, key: &IrCacheKey) -> Option<&CachedIr> {
        let hash = key.combined_hash();
        if let Some(entry) = self.entries.get(&hash) {
            self.stats.record_hit(entry.backend, entry.size);
            Some(entry)
        } else {
            // Determine backend from key context — default to Object for miss tracking
            self.stats.record_miss(BackendKind::Object);
            None
        }
    }

    /// Look up with specific backend for miss tracking.
    pub fn lookup_for(&mut self, key: &IrCacheKey, backend: BackendKind) -> Option<&CachedIr> {
        let hash = key.combined_hash();
        if let Some(entry) = self.entries.get(&hash) {
            self.stats.record_hit(entry.backend, entry.size);
            Some(entry)
        } else {
            self.stats.record_miss(backend);
            None
        }
    }

    /// I3.6: Get all object files for incremental linking.
    ///
    /// Returns only Object-type entries, sorted by module name for determinism.
    pub fn object_files(&self) -> Vec<&CachedIr> {
        let mut objects: Vec<&CachedIr> = self
            .entries
            .values()
            .filter(|ir| ir.backend == BackendKind::Object)
            .collect();
        objects.sort_by_key(|ir| &ir.key.source_hash);
        objects
    }

    /// I3.8: Load multiple entries in parallel (simulated — returns all matching).
    pub fn parallel_load(&mut self, keys: &[IrCacheKey]) -> Vec<Option<CachedIr>> {
        keys.iter()
            .map(|key| {
                let hash = key.combined_hash();
                if let Some(entry) = self.entries.get(&hash) {
                    self.stats.record_hit(entry.backend, entry.size);
                    Some(entry.clone())
                } else {
                    self.stats.record_miss(BackendKind::Object);
                    None
                }
            })
            .collect()
    }

    /// Total number of cached entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Total cached bytes.
    pub fn total_bytes(&self) -> usize {
        self.entries.values().map(|ir| ir.size).sum()
    }

    /// Remove an entry.
    pub fn invalidate(&mut self, key: &IrCacheKey) -> bool {
        let hash = key.combined_hash();
        self.entries.remove(&hash).is_some()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I3.6: Incremental Linker
// ═══════════════════════════════════════════════════════════════════════

/// Tracks which object files need re-linking.
#[derive(Debug, Clone, Default)]
pub struct IncrementalLinker {
    /// Object files to link: module name → object data.
    pub objects: HashMap<String, Vec<u8>>,
    /// Which modules changed since last link.
    pub changed_modules: HashSet<String>,
}

impl IncrementalLinker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an object file for a module.
    pub fn add_object(&mut self, module: &str, data: Vec<u8>) {
        self.objects.insert(module.to_string(), data);
    }

    /// Mark a module as changed (needs re-link).
    pub fn mark_changed(&mut self, module: &str) {
        self.changed_modules.insert(module.to_string());
    }

    /// Check if a full re-link is needed (all modules changed).
    pub fn needs_full_link(&self) -> bool {
        self.changed_modules.len() == self.objects.len()
    }

    /// Get only the changed object files for incremental linking.
    pub fn changed_objects(&self) -> Vec<(&str, &[u8])> {
        self.changed_modules
            .iter()
            .filter_map(|m| self.objects.get(m).map(|d| (m.as_str(), d.as_slice())))
            .collect()
    }

    /// Total number of modules.
    pub fn module_count(&self) -> usize {
        self.objects.len()
    }
}

use std::collections::HashSet;

// ═══════════════════════════════════════════════════════════════════════
// Tests — I3.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(source: &str, target: &str, opt: &str) -> IrCacheKey {
        IrCacheKey {
            source_hash: compute_content_hash(source),
            compiler_version: "10.0.0".into(),
            target: target.into(),
            opt_level: opt.into(),
            dep_hashes: vec![],
        }
    }

    fn make_ir(key: IrCacheKey, backend: BackendKind, data: Vec<u8>) -> CachedIr {
        let size = data.len();
        CachedIr {
            key,
            backend,
            data,
            size,
            has_debug_info: false,
        }
    }

    // ── I3.1: Cranelift IR caching ──

    #[test]
    fn i3_1_cranelift_ir_cache() {
        let mut store = IrCacheStore::new();
        let key = make_key("fn main() { 42 }", "x86_64", "O0");
        let ir = make_ir(key.clone(), BackendKind::Cranelift, vec![0xCF; 100]);

        store.store(ir);
        let cached = store.lookup(&key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().backend, BackendKind::Cranelift);
        assert_eq!(cached.unwrap().data.len(), 100);
    }

    // ── I3.2: LLVM bitcode caching ──

    #[test]
    fn i3_2_llvm_bitcode_cache() {
        let mut store = IrCacheStore::new();
        let key = make_key("fn add(a: i64, b: i64) -> i64 { a + b }", "x86_64", "O2");
        let ir = make_ir(key.clone(), BackendKind::Llvm, vec![0xBC; 200]);

        store.store(ir);
        let cached = store.lookup_for(&key, BackendKind::Llvm);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().backend, BackendKind::Llvm);
    }

    // ── I3.3: Bytecode VM caching ──

    #[test]
    fn i3_3_bytecode_cache() {
        let mut store = IrCacheStore::new();
        let key = make_key("fn hello() { println(\"hi\") }", "vm", "O0");
        let ir = make_ir(key.clone(), BackendKind::Bytecode, vec![0x01, 0x02, 0x03]);

        store.store(ir);
        let cached = store.lookup_for(&key, BackendKind::Bytecode);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().data, vec![0x01, 0x02, 0x03]);
    }

    // ── I3.4: Object file caching ──

    #[test]
    fn i3_4_object_file_cache() {
        let mut store = IrCacheStore::new();
        let key = make_key("fn main() {}", "x86_64", "O2");
        let ir = make_ir(
            key.clone(),
            BackendKind::Object,
            vec![0x7F, 0x45, 0x4C, 0x46],
        ); // ELF magic

        store.store(ir);
        let objects = store.object_files();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].backend, BackendKind::Object);
    }

    // ── I3.5: Cache key computation ──

    #[test]
    fn i3_5_different_flags_different_key() {
        let k1 = make_key("fn f() {}", "x86_64", "O0");
        let k2 = make_key("fn f() {}", "x86_64", "O2");
        assert_ne!(k1.combined_hash(), k2.combined_hash());
    }

    #[test]
    fn i3_5_different_target_different_key() {
        let k1 = make_key("fn f() {}", "x86_64", "O0");
        let k2 = make_key("fn f() {}", "aarch64", "O0");
        assert_ne!(k1.combined_hash(), k2.combined_hash());
    }

    #[test]
    fn i3_5_same_input_same_key() {
        let k1 = make_key("fn f() {}", "x86_64", "O0");
        let k2 = make_key("fn f() {}", "x86_64", "O0");
        assert_eq!(k1.combined_hash(), k2.combined_hash());
    }

    #[test]
    fn i3_5_dep_hashes_affect_key() {
        let mut k1 = make_key("fn f() {}", "x86_64", "O0");
        k1.dep_hashes = vec!["dep_a".into()];

        let mut k2 = make_key("fn f() {}", "x86_64", "O0");
        k2.dep_hashes = vec!["dep_b".into()];

        assert_ne!(k1.combined_hash(), k2.combined_hash());
    }

    // ── I3.6: Incremental linking ──

    #[test]
    fn i3_6_incremental_linker() {
        let mut linker = IncrementalLinker::new();
        linker.add_object("main", vec![1, 2, 3]);
        linker.add_object("lib", vec![4, 5, 6]);
        linker.add_object("utils", vec![7, 8, 9]);

        linker.mark_changed("lib");

        assert_eq!(linker.module_count(), 3);
        assert!(!linker.needs_full_link());
        assert_eq!(linker.changed_objects().len(), 1);
        assert_eq!(linker.changed_objects()[0].0, "lib");
    }

    // ── I3.7: Debug info caching ──

    #[test]
    fn i3_7_debug_info_cache() {
        let mut store = IrCacheStore::new();
        let key = make_key("fn main() {}", "x86_64", "O0");
        let ir = CachedIr {
            key: key.clone(),
            backend: BackendKind::DebugInfo,
            data: vec![0xDB; 50],
            size: 50,
            has_debug_info: true,
        };

        store.store(ir);
        let cached = store.lookup_for(&key, BackendKind::DebugInfo);
        assert!(cached.is_some());
        assert!(cached.unwrap().has_debug_info);
    }

    // ── I3.8: Parallel cache reads ──

    #[test]
    fn i3_8_parallel_load() {
        let mut store = IrCacheStore::new();

        let k1 = make_key("fn a() {}", "x86_64", "O0");
        let k2 = make_key("fn b() {}", "x86_64", "O0");
        let k3 = make_key("fn c() {}", "x86_64", "O0"); // not stored

        store.store(make_ir(k1.clone(), BackendKind::Cranelift, vec![1; 10]));
        store.store(make_ir(k2.clone(), BackendKind::Cranelift, vec![2; 20]));

        let results = store.parallel_load(&[k1, k2, k3]);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_none()); // miss
    }

    // ── I3.9: Cache hit metrics ──

    #[test]
    fn i3_9_cache_stats() {
        let mut store = IrCacheStore::new();

        let k1 = make_key("fn a() {}", "x86_64", "O0");
        store.store(make_ir(k1.clone(), BackendKind::Cranelift, vec![0; 100]));

        // 1 hit
        store.lookup(&k1);
        // 1 miss
        let k_miss = make_key("fn missing() {}", "x86_64", "O0");
        store.lookup(&k_miss);

        assert_eq!(store.stats.total_hits(), 1);
        assert_eq!(store.stats.total_misses(), 1);
        assert!((store.stats.hit_rate_pct() - 50.0).abs() < 0.1);
        assert_eq!(store.stats.display_summary(), "Cache: 1/2 hit (50.0%)");
    }

    #[test]
    fn i3_9_verbose_display() {
        let mut stats = IrCacheStats::default();
        stats.record_hit(BackendKind::Cranelift, 100);
        stats.record_hit(BackendKind::Cranelift, 200);
        stats.record_miss(BackendKind::Llvm);

        assert_eq!(stats.total_hits(), 2);
        assert_eq!(stats.total_misses(), 1);
        assert_eq!(stats.bytes_loaded, 300);
        assert_eq!(stats.display_summary(), "Cache: 2/3 hit (66.7%)");
    }

    // ── I3.10: Integration ──

    #[test]
    fn i3_10_full_ir_cache_workflow() {
        let mut store = IrCacheStore::new();

        // Compile 3 modules
        let modules = ["main.fj", "lib.fj", "utils.fj"];
        for module in &modules {
            let key = make_key(&format!("// {module}"), "x86_64", "O2");
            store.store(make_ir(key, BackendKind::Object, vec![0xEF; 500]));
        }

        assert_eq!(store.entry_count(), 3);
        assert_eq!(store.total_bytes(), 1500);
        assert_eq!(store.object_files().len(), 3);

        // Simulate rebuild: main.fj changed, others cached
        let key_main = make_key("// main.fj v2", "x86_64", "O2"); // changed
        let key_lib = make_key("// lib.fj", "x86_64", "O2"); // unchanged

        assert!(store.lookup_for(&key_lib, BackendKind::Object).is_some()); // hit
        assert!(store.lookup_for(&key_main, BackendKind::Object).is_none()); // miss (changed)

        // Store new main
        store.store(make_ir(key_main, BackendKind::Object, vec![0xAB; 600]));

        assert_eq!(store.entry_count(), 4); // 3 old + 1 new
        assert_eq!(store.stats.total_hits(), 1);
        assert_eq!(store.stats.total_misses(), 1);
    }

    #[test]
    fn i3_10_backend_extensions() {
        assert_eq!(BackendKind::Cranelift.extension(), ".clif");
        assert_eq!(BackendKind::Llvm.extension(), ".bc");
        assert_eq!(BackendKind::Bytecode.extension(), ".fjbc");
        assert_eq!(BackendKind::Object.extension(), ".o");
        assert_eq!(BackendKind::DebugInfo.extension(), ".dwarf");
    }
}
