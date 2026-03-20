//! Reproducible builds — deterministic compilation, source hash embedding,
//! version embedding, cross-platform reproducibility, build spec,
//! binary diff, build cache, verification.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S28.1: Deterministic Compilation
// ═══════════════════════════════════════════════════════════════════════

/// Sources of non-determinism to eliminate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonDeterminismSource {
    /// HashMap iteration order.
    HashMapIteration,
    /// Timestamp embedding.
    Timestamp,
    /// Address space layout randomization.
    Aslr,
    /// Thread scheduling.
    ThreadScheduling,
    /// Random number generation.
    RandomSeed,
}

impl fmt::Display for NonDeterminismSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NonDeterminismSource::HashMapIteration => write!(f, "HashMap iteration order"),
            NonDeterminismSource::Timestamp => write!(f, "timestamp embedding"),
            NonDeterminismSource::Aslr => write!(f, "ASLR"),
            NonDeterminismSource::ThreadScheduling => write!(f, "thread scheduling"),
            NonDeterminismSource::RandomSeed => write!(f, "random seed"),
        }
    }
}

/// Mitigation for a non-determinism source.
#[derive(Debug, Clone)]
pub struct DeterminismFix {
    /// Source being fixed.
    pub source: NonDeterminismSource,
    /// Mitigation strategy.
    pub strategy: String,
    /// Whether the fix is applied.
    pub applied: bool,
}

/// Standard mitigations for reproducible builds.
pub fn standard_mitigations() -> Vec<DeterminismFix> {
    vec![
        DeterminismFix {
            source: NonDeterminismSource::HashMapIteration,
            strategy: "Use BTreeMap or sort keys before iteration".into(),
            applied: true,
        },
        DeterminismFix {
            source: NonDeterminismSource::Timestamp,
            strategy: "Use SOURCE_DATE_EPOCH env var or fixed epoch".into(),
            applied: true,
        },
        DeterminismFix {
            source: NonDeterminismSource::Aslr,
            strategy: "Use relative addresses in debug info".into(),
            applied: true,
        },
        DeterminismFix {
            source: NonDeterminismSource::ThreadScheduling,
            strategy: "Single-threaded compilation or deterministic work stealing".into(),
            applied: true,
        },
        DeterminismFix {
            source: NonDeterminismSource::RandomSeed,
            strategy: "Use fixed seed derived from source hash".into(),
            applied: true,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// S28.2: Source Hash Embedding
// ═══════════════════════════════════════════════════════════════════════

/// Source provenance information embedded in the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProvenance {
    /// SHA-256 hash of all source files.
    pub source_hash: String,
    /// Number of source files.
    pub file_count: usize,
    /// Total source bytes.
    pub total_bytes: usize,
}

impl fmt::Display for SourceProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source: {} ({} files, {} bytes)",
            &self.source_hash[..16],
            self.file_count,
            self.total_bytes
        )
    }
}

/// Computes a simple hash for source provenance (FNV-1a).
pub fn compute_source_hash(files: &[(&str, &[u8])]) -> SourceProvenance {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut total_bytes = 0;

    // Sort files by name for determinism
    let mut sorted_files: Vec<_> = files.to_vec();
    sorted_files.sort_by_key(|(name, _)| *name);

    for (name, content) in &sorted_files {
        for &byte in name.as_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &byte in *content {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        total_bytes += content.len();
    }

    SourceProvenance {
        source_hash: format!("{hash:016x}"),
        file_count: files.len(),
        total_bytes,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.3: Compiler Version Embedding
// ═══════════════════════════════════════════════════════════════════════

/// Compiler version metadata embedded in binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerVersion {
    /// Semantic version (e.g., "2.0.0").
    pub version: String,
    /// Git commit hash.
    pub git_hash: String,
    /// Build date (SOURCE_DATE_EPOCH).
    pub build_date: String,
    /// Target triple.
    pub target: String,
}

impl fmt::Display for CompilerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fj {} ({}) for {} built {}",
            self.version,
            &self.git_hash[..8.min(self.git_hash.len())],
            self.target,
            self.build_date
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.4: Cross-Platform Reproducibility
// ═══════════════════════════════════════════════════════════════════════

/// Supported targets for cross-platform reproducibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReproTarget {
    LinuxX86_64,
    LinuxArm64,
    MacOsArm64,
}

impl fmt::Display for ReproTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReproTarget::LinuxX86_64 => write!(f, "x86_64-unknown-linux-gnu"),
            ReproTarget::LinuxArm64 => write!(f, "aarch64-unknown-linux-gnu"),
            ReproTarget::MacOsArm64 => write!(f, "aarch64-apple-darwin"),
        }
    }
}

/// Cross-platform build result.
#[derive(Debug, Clone)]
pub struct CrossPlatformBuild {
    /// Target.
    pub target: ReproTarget,
    /// Binary hash.
    pub hash: String,
    /// Binary size.
    pub size: usize,
}

/// Verifies cross-platform reproducibility.
pub fn verify_cross_platform(builds: &[CrossPlatformBuild]) -> CrossPlatformResult {
    if builds.len() < 2 {
        return CrossPlatformResult {
            reproducible: true,
            mismatches: Vec::new(),
        };
    }

    let mut mismatches = Vec::new();
    // Same-target builds should produce identical binaries
    let mut by_target: HashMap<String, Vec<&CrossPlatformBuild>> = HashMap::new();
    for build in builds {
        by_target
            .entry(build.target.to_string())
            .or_default()
            .push(build);
    }

    for (target, target_builds) in &by_target {
        if target_builds.len() >= 2 {
            let first_hash = &target_builds[0].hash;
            for build in &target_builds[1..] {
                if build.hash != *first_hash {
                    mismatches.push(target.clone());
                    break;
                }
            }
        }
    }

    CrossPlatformResult {
        reproducible: mismatches.is_empty(),
        mismatches,
    }
}

/// Result of cross-platform reproducibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossPlatformResult {
    /// Whether all same-target builds are reproducible.
    pub reproducible: bool,
    /// Targets that failed reproducibility.
    pub mismatches: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// S28.5: Reproducible Builds Spec
// ═══════════════════════════════════════════════════════════════════════

/// Inputs that affect compilation output.
#[derive(Debug, Clone)]
pub struct BuildInputs {
    /// Source hash.
    pub source_hash: String,
    /// Compiler version hash.
    pub compiler_hash: String,
    /// Compiler flags.
    pub flags: Vec<String>,
    /// Target triple.
    pub target: String,
}

impl BuildInputs {
    /// Computes a composite hash of all inputs.
    pub fn composite_hash(&self) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in self.source_hash.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for byte in self.compiler_hash.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for flag in &self.flags {
            for byte in flag.bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        for byte in self.target.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.6: Binary Diff Tool
// ═══════════════════════════════════════════════════════════════════════

/// A section in a binary for diff purposes.
#[derive(Debug, Clone)]
pub struct BinarySection {
    /// Section name.
    pub name: String,
    /// Offset in file.
    pub offset: usize,
    /// Size in bytes.
    pub size: usize,
    /// Hash of section content.
    pub hash: String,
}

/// Diff result between two binaries.
#[derive(Debug, Clone)]
pub struct BinaryDiff {
    /// Sections that match.
    pub matching: Vec<String>,
    /// Sections that differ.
    pub differing: Vec<String>,
    /// Sections only in first binary.
    pub only_in_a: Vec<String>,
    /// Sections only in second binary.
    pub only_in_b: Vec<String>,
}

impl BinaryDiff {
    /// Whether the binaries are identical.
    pub fn identical(&self) -> bool {
        self.differing.is_empty() && self.only_in_a.is_empty() && self.only_in_b.is_empty()
    }
}

/// Diffs two sets of binary sections.
pub fn diff_sections(a: &[BinarySection], b: &[BinarySection]) -> BinaryDiff {
    let a_map: HashMap<&str, &BinarySection> = a.iter().map(|s| (s.name.as_str(), s)).collect();
    let b_map: HashMap<&str, &BinarySection> = b.iter().map(|s| (s.name.as_str(), s)).collect();

    let mut matching = Vec::new();
    let mut differing = Vec::new();
    let mut only_in_a = Vec::new();
    let mut only_in_b = Vec::new();

    for (name, sec_a) in &a_map {
        if let Some(sec_b) = b_map.get(name) {
            if sec_a.hash == sec_b.hash {
                matching.push(name.to_string());
            } else {
                differing.push(name.to_string());
            }
        } else {
            only_in_a.push(name.to_string());
        }
    }

    for name in b_map.keys() {
        if !a_map.contains_key(name) {
            only_in_b.push(name.to_string());
        }
    }

    BinaryDiff {
        matching,
        differing,
        only_in_a,
        only_in_b,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.7: Build Cache
// ═══════════════════════════════════════════════════════════════════════

/// Content-addressable build cache.
#[derive(Debug, Clone, Default)]
pub struct BuildCache {
    /// Cache entries: composite_hash -> binary_hash.
    entries: HashMap<String, CacheEntry>,
}

/// A build cache entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Composite input hash.
    pub input_hash: String,
    /// Output binary hash.
    pub output_hash: String,
    /// Output binary path.
    pub binary_path: String,
}

impl BuildCache {
    /// Creates a new build cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a cached build.
    pub fn lookup(&self, input_hash: &str) -> Option<&CacheEntry> {
        self.entries.get(input_hash)
    }

    /// Stores a build result.
    pub fn store(&mut self, input_hash: &str, output_hash: &str, binary_path: &str) {
        self.entries.insert(
            input_hash.into(),
            CacheEntry {
                input_hash: input_hash.into(),
                output_hash: output_hash.into(),
                binary_path: binary_path.into(),
            },
        );
    }

    /// Returns the number of cached entries.
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Checks if a build is cached.
    pub fn is_cached(&self, input_hash: &str) -> bool {
        self.entries.contains_key(input_hash)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S28.8 / S28.9: Verification
// ═══════════════════════════════════════════════════════════════════════

/// Reproducibility verification result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationResult {
    /// Number of builds performed.
    pub builds_performed: usize,
    /// Whether all builds produced the same hash.
    pub all_match: bool,
    /// Distinct hashes observed.
    pub distinct_hashes: usize,
}

impl fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.all_match {
            write!(
                f,
                "REPRODUCIBLE: {} builds, all identical",
                self.builds_performed
            )
        } else {
            write!(
                f,
                "NOT REPRODUCIBLE: {} builds, {} distinct outputs",
                self.builds_performed, self.distinct_hashes
            )
        }
    }
}

/// Verifies reproducibility from a list of build hashes.
pub fn verify_reproducibility(hashes: &[&str]) -> VerificationResult {
    let mut unique: Vec<&str> = hashes.to_vec();
    unique.sort();
    unique.dedup();

    VerificationResult {
        builds_performed: hashes.len(),
        all_match: unique.len() <= 1,
        distinct_hashes: unique.len(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S28.1 — Deterministic Compilation
    #[test]
    fn s28_1_standard_mitigations() {
        let fixes = standard_mitigations();
        assert_eq!(fixes.len(), 5);
        assert!(fixes.iter().all(|f| f.applied));
    }

    #[test]
    fn s28_1_non_determinism_display() {
        assert!(
            NonDeterminismSource::HashMapIteration
                .to_string()
                .contains("HashMap")
        );
        assert!(
            NonDeterminismSource::Timestamp
                .to_string()
                .contains("timestamp")
        );
    }

    // S28.2 — Source Hash Embedding
    #[test]
    fn s28_2_source_hash_deterministic() {
        let files = vec![("main.fj", b"fn main() {}" as &[u8])];
        let h1 = compute_source_hash(&files);
        let h2 = compute_source_hash(&files);
        assert_eq!(h1.source_hash, h2.source_hash);
    }

    #[test]
    fn s28_2_source_hash_differs() {
        let files_a = vec![("main.fj", b"fn main() {}" as &[u8])];
        let files_b = vec![("main.fj", b"fn main() { 42 }" as &[u8])];
        assert_ne!(
            compute_source_hash(&files_a).source_hash,
            compute_source_hash(&files_b).source_hash
        );
    }

    #[test]
    fn s28_2_provenance_display() {
        let prov = compute_source_hash(&[("test.fj", b"hello")]);
        assert!(prov.to_string().contains("1 files"));
    }

    // S28.3 — Compiler Version Embedding
    #[test]
    fn s28_3_version_display() {
        let ver = CompilerVersion {
            version: "2.0.0".into(),
            git_hash: "abcdef1234567890".into(),
            build_date: "2026-03-12".into(),
            target: "x86_64-unknown-linux-gnu".into(),
        };
        let display = ver.to_string();
        assert!(display.contains("2.0.0"));
        assert!(display.contains("abcdef12"));
    }

    // S28.4 — Cross-Platform Reproducibility
    #[test]
    fn s28_4_same_target_reproducible() {
        let builds = vec![
            CrossPlatformBuild {
                target: ReproTarget::LinuxX86_64,
                hash: "abc123".into(),
                size: 5000,
            },
            CrossPlatformBuild {
                target: ReproTarget::LinuxX86_64,
                hash: "abc123".into(),
                size: 5000,
            },
        ];
        let result = verify_cross_platform(&builds);
        assert!(result.reproducible);
    }

    #[test]
    fn s28_4_same_target_not_reproducible() {
        let builds = vec![
            CrossPlatformBuild {
                target: ReproTarget::LinuxX86_64,
                hash: "abc123".into(),
                size: 5000,
            },
            CrossPlatformBuild {
                target: ReproTarget::LinuxX86_64,
                hash: "def456".into(),
                size: 5100,
            },
        ];
        let result = verify_cross_platform(&builds);
        assert!(!result.reproducible);
    }

    #[test]
    fn s28_4_target_display() {
        assert!(ReproTarget::LinuxX86_64.to_string().contains("x86_64"));
        assert!(ReproTarget::MacOsArm64.to_string().contains("darwin"));
    }

    // S28.5 — Build Inputs
    #[test]
    fn s28_5_composite_hash() {
        let inputs = BuildInputs {
            source_hash: "abc".into(),
            compiler_hash: "def".into(),
            flags: vec!["-O2".into()],
            target: "x86_64".into(),
        };
        let hash = inputs.composite_hash();
        assert!(!hash.is_empty());
    }

    #[test]
    fn s28_5_different_flags_different_hash() {
        let a = BuildInputs {
            source_hash: "abc".into(),
            compiler_hash: "def".into(),
            flags: vec!["-O2".into()],
            target: "x86_64".into(),
        };
        let b = BuildInputs {
            source_hash: "abc".into(),
            compiler_hash: "def".into(),
            flags: vec!["-O0".into()],
            target: "x86_64".into(),
        };
        assert_ne!(a.composite_hash(), b.composite_hash());
    }

    // S28.6 — Binary Diff Tool
    #[test]
    fn s28_6_diff_identical() {
        let sections = vec![BinarySection {
            name: ".text".into(),
            offset: 0,
            size: 100,
            hash: "abc".into(),
        }];
        let diff = diff_sections(&sections, &sections);
        assert!(diff.identical());
        assert_eq!(diff.matching.len(), 1);
    }

    #[test]
    fn s28_6_diff_different() {
        let a = vec![BinarySection {
            name: ".text".into(),
            offset: 0,
            size: 100,
            hash: "abc".into(),
        }];
        let b = vec![BinarySection {
            name: ".text".into(),
            offset: 0,
            size: 100,
            hash: "def".into(),
        }];
        let diff = diff_sections(&a, &b);
        assert!(!diff.identical());
        assert_eq!(diff.differing.len(), 1);
    }

    // S28.7 — Build Cache
    #[test]
    fn s28_7_cache_store_lookup() {
        let mut cache = BuildCache::new();
        cache.store("input1", "output1", "/path/binary");
        assert!(cache.is_cached("input1"));
        assert!(!cache.is_cached("input2"));
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn s28_7_cache_miss() {
        let cache = BuildCache::new();
        assert!(cache.lookup("nonexistent").is_none());
    }

    // S28.8 — Verification Script
    #[test]
    fn s28_8_all_reproducible() {
        let result = verify_reproducibility(&["abc123", "abc123", "abc123"]);
        assert!(result.all_match);
        assert_eq!(result.distinct_hashes, 1);
    }

    #[test]
    fn s28_8_not_reproducible() {
        let result = verify_reproducibility(&["abc123", "def456"]);
        assert!(!result.all_match);
        assert_eq!(result.distinct_hashes, 2);
    }

    #[test]
    fn s28_8_verification_display() {
        let result = verify_reproducibility(&["abc", "abc"]);
        assert!(result.to_string().contains("REPRODUCIBLE"));
    }

    // S28.9 — Third-Party Verification
    #[test]
    fn s28_9_verification_not_reproducible_display() {
        let result = verify_reproducibility(&["a", "b", "c"]);
        assert!(result.to_string().contains("NOT REPRODUCIBLE"));
        assert!(result.to_string().contains("3 distinct"));
    }

    // S28.10 — Additional
    #[test]
    fn s28_10_source_hash_file_order_independent() {
        let a = vec![("a.fj", b"hello" as &[u8]), ("b.fj", b"world")];
        let b = vec![("b.fj", b"world" as &[u8]), ("a.fj", b"hello")];
        assert_eq!(
            compute_source_hash(&a).source_hash,
            compute_source_hash(&b).source_hash
        );
    }

    #[test]
    fn s28_10_cache_overwrite() {
        let mut cache = BuildCache::new();
        cache.store("input1", "output1", "/path/v1");
        cache.store("input1", "output2", "/path/v2");
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.lookup("input1").unwrap().output_hash, "output2");
    }
}
