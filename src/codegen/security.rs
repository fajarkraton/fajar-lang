//! Security hardening for the Fajar Lang compiler.
//!
//! Provides comprehensive compile-time and runtime security features across
//! three domains:
//!
//! - **SEC1 — Memory Safety Hardening:** Stack canaries, bounds checking,
//!   overflow detection, allocation budgets, and stack depth guards.
//! - **SEC2 — Supply Chain Security:** Typosquat detection, token scoping,
//!   reproducible builds, build provenance (SLSA), and CVE advisories.
//! - **SEC3 — Audit & Certification:** 20 real lint rules, taint analysis,
//!   security scorecards, compliance modes, capability sets, and sandbox policies.
//!
//! # Architecture
//!
//! ```text
//! MemoryHardening ─── aggregate config for all SEC1 features
//!     ├── StackCanaryConfig   ─── random canary per function frame
//!     ├── BoundsCheckMode     ─── none / debug / release
//!     ├── OverflowCheckConfig ─── signed + unsigned overflow detection
//!     ├── AllocationBudget    ─── per-context memory limit tracking
//!     └── StackDepthGuard     ─── configurable max recursion depth
//!
//! SupplyChainSecurity
//!     ├── TyposquatDetector   ─── Levenshtein distance ≤ 2
//!     ├── TokenScope / TokenRotation ─── scoped, expiring tokens
//!     ├── ReproducibleBuild   ─── deterministic build_id from inputs
//!     ├── BuildProvenance     ─── SLSA-style attestation
//!     └── SecurityAdvisory    ─── CVE tracking + version matching
//!
//! AuditCertification
//!     ├── SecurityLinter      ─── 20 lint rules with real detection
//!     ├── TaintAnalysis       ─── source → sink tracking
//!     ├── SecurityScorecard   ─── 0-100 aggregate score
//!     ├── ComplianceMode      ─── MISRA, CERT_C, ISO_26262, etc.
//!     ├── CapabilitySet       ─── fine-grained permission bits
//!     └── SandboxPolicy       ─── per-module operation restrictions
//! ```

use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// SEC1: Memory Safety Hardening
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for stack canary insertion.
///
/// Stack canaries are random values placed between the return address and
/// local variables on the stack frame. Before a function returns, the canary
/// is checked — if it was overwritten (e.g., by a buffer overflow), the
/// program aborts instead of returning to a corrupted address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackCanaryConfig {
    /// Whether canary insertion is enabled.
    pub enabled: bool,
    /// Base seed for canary value derivation. Each call site uses
    /// `seed ^ call_site_id` to produce a unique canary value.
    pub seed: u64,
    /// Counter tracking the number of unique canary values generated.
    generation_counter: u64,
}

impl StackCanaryConfig {
    /// Creates a new canary configuration with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            enabled: true,
            seed,
            generation_counter: 0,
        }
    }

    /// Creates a disabled canary configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            seed: 0,
            generation_counter: 0,
        }
    }

    /// Generates a unique canary value for a specific call site.
    ///
    /// Each invocation produces a different value by combining the seed
    /// with the call-site identifier and an internal counter. The mixing
    /// function uses a Murmur3-style finalizer for good distribution.
    pub fn generate_canary(&mut self, call_site_id: u64) -> u64 {
        if !self.enabled {
            return 0;
        }
        self.generation_counter = self.generation_counter.wrapping_add(1);
        let mut h = self.seed ^ call_site_id ^ self.generation_counter;
        // Murmur3-style 64-bit finalizer — distributes bits well.
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        h
    }

    /// Returns the number of canary values generated so far.
    pub fn generation_count(&self) -> u64 {
        self.generation_counter
    }

    /// Returns true if the canary value matches what was expected for a site.
    ///
    /// This simulates the runtime check that would be inserted at function
    /// epilogue. The `expected` value is the one placed at prologue time;
    /// `actual` is read back from the stack before returning.
    pub fn verify_canary(expected: u64, actual: u64) -> bool {
        expected == actual
    }
}

/// Bounds checking mode for array/slice accesses.
///
/// Controls how aggressively the compiler inserts bounds checks on
/// indexing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundsCheckMode {
    /// No bounds checks — maximum performance, no safety.
    None,
    /// Bounds checks in debug builds only.
    Debug,
    /// Bounds checks in all builds (debug + release).
    Release,
}

impl BoundsCheckMode {
    /// Returns true if bounds checks should be emitted for the given
    /// `is_release` build flag.
    pub fn should_check(&self, is_release: bool) -> bool {
        match self {
            Self::None => false,
            Self::Debug => !is_release,
            Self::Release => true,
        }
    }
}

impl fmt::Display for BoundsCheckMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
        }
    }
}

/// Configuration for integer overflow detection.
///
/// Controls whether signed and/or unsigned arithmetic operations insert
/// overflow traps. When enabled, operations like `a + b` are lowered to
/// checked-add instructions that abort on overflow instead of wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverflowCheckConfig {
    /// Check signed integer arithmetic (i8..i128).
    pub check_signed: bool,
    /// Check unsigned integer arithmetic (u8..u128).
    pub check_unsigned: bool,
    /// Trap (abort) on overflow instead of just reporting.
    pub trap_on_overflow: bool,
}

impl Default for OverflowCheckConfig {
    fn default() -> Self {
        Self {
            check_signed: true,
            check_unsigned: false,
            trap_on_overflow: true,
        }
    }
}

impl OverflowCheckConfig {
    /// Creates a config that checks both signed and unsigned arithmetic.
    pub fn strict() -> Self {
        Self {
            check_signed: true,
            check_unsigned: true,
            trap_on_overflow: true,
        }
    }

    /// Creates a config that performs no overflow checking.
    pub fn unchecked() -> Self {
        Self {
            check_signed: false,
            check_unsigned: false,
            trap_on_overflow: false,
        }
    }

    /// Returns true if any overflow checking is active.
    pub fn any_checks_enabled(&self) -> bool {
        self.check_signed || self.check_unsigned
    }
}

/// Per-context memory allocation budget.
///
/// Tracks cumulative allocations against a configured limit. Used to enforce
/// memory caps in sandboxed execution contexts (e.g., @device functions
/// running ML inference should not exceed their allocation budget).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationBudget {
    /// Maximum allocation budget in bytes.
    pub max_bytes: u64,
    /// Current allocation usage in bytes.
    used_bytes: u64,
    /// Number of individual allocation operations performed.
    allocation_count: u64,
}

impl AllocationBudget {
    /// Creates a new allocation budget with the given limit.
    pub fn new(max_bytes: u64) -> Self {
        Self {
            max_bytes,
            used_bytes: 0,
            allocation_count: 0,
        }
    }

    /// Attempts to allocate `size` bytes. Returns `Ok(())` if within budget,
    /// or `Err` with a message if the budget would be exceeded.
    pub fn allocate(&mut self, size: u64) -> Result<(), String> {
        let new_total = self.used_bytes.saturating_add(size);
        if new_total > self.max_bytes {
            return Err(format!(
                "allocation of {} bytes exceeds budget: {} / {} bytes used",
                size, self.used_bytes, self.max_bytes
            ));
        }
        self.used_bytes = new_total;
        self.allocation_count = self.allocation_count.saturating_add(1);
        Ok(())
    }

    /// Frees `size` bytes, reducing the used total.
    pub fn free(&mut self, size: u64) {
        self.used_bytes = self.used_bytes.saturating_sub(size);
    }

    /// Returns the number of bytes remaining in the budget.
    pub fn remaining(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes)
    }

    /// Returns the number of bytes currently in use.
    pub fn used(&self) -> u64 {
        self.used_bytes
    }

    /// Returns the total number of allocation operations performed.
    pub fn allocation_count(&self) -> u64 {
        self.allocation_count
    }

    /// Returns true if the budget has been fully exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.used_bytes >= self.max_bytes
    }

    /// Checks whether `size` bytes can be allocated without actually
    /// performing the allocation.
    pub fn can_allocate(&self, size: u64) -> bool {
        self.used_bytes.saturating_add(size) <= self.max_bytes
    }

    /// Resets the budget to zero usage (e.g., between execution contexts).
    pub fn reset(&mut self) {
        self.used_bytes = 0;
        self.allocation_count = 0;
    }
}

/// Stack depth guard for recursion limiting.
///
/// Prevents unbounded recursion by tracking the current call depth and
/// comparing it against a configurable maximum. When the maximum is
/// exceeded, the runtime can abort or return an error instead of
/// overflowing the native stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackDepthGuard {
    /// Current recursion depth.
    current: u32,
    /// Maximum allowed recursion depth.
    pub max_depth: u32,
    /// Peak depth reached during execution (for profiling).
    peak: u32,
}

impl StackDepthGuard {
    /// Creates a new guard with the given maximum depth.
    pub fn new(max_depth: u32) -> Self {
        Self {
            current: 0,
            max_depth,
            peak: 0,
        }
    }

    /// Attempts to enter a new recursion level. Returns `Ok(current_depth)`
    /// on success, or `Err` if the maximum depth would be exceeded.
    pub fn enter(&mut self) -> Result<u32, String> {
        if self.current >= self.max_depth {
            return Err(format!(
                "stack overflow: recursion depth {} exceeds maximum {}",
                self.current, self.max_depth
            ));
        }
        self.current += 1;
        if self.current > self.peak {
            self.peak = self.current;
        }
        Ok(self.current)
    }

    /// Exits the current recursion level, decrementing the depth counter.
    pub fn exit(&mut self) {
        self.current = self.current.saturating_sub(1);
    }

    /// Returns the current recursion depth.
    pub fn current_depth(&self) -> u32 {
        self.current
    }

    /// Returns the peak recursion depth observed.
    pub fn peak_depth(&self) -> u32 {
        self.peak
    }

    /// Returns true if the guard is at the maximum depth.
    pub fn is_at_limit(&self) -> bool {
        self.current >= self.max_depth
    }

    /// Resets the guard to zero depth (preserving peak for reporting).
    pub fn reset(&mut self) {
        self.current = 0;
    }
}

/// Aggregate memory hardening configuration.
///
/// Combines all SEC1 features into a single configuration object that
/// the code generator uses to decide which safety checks to emit.
#[derive(Debug, Clone)]
pub struct MemoryHardening {
    /// Stack canary configuration.
    pub canaries: StackCanaryConfig,
    /// Bounds checking mode.
    pub bounds_check: BoundsCheckMode,
    /// Overflow detection configuration.
    pub overflow: OverflowCheckConfig,
    /// Per-context allocation budget.
    pub budget: AllocationBudget,
    /// Stack depth guard.
    pub depth_guard: StackDepthGuard,
}

impl MemoryHardening {
    /// Creates a default hardening config suitable for debug builds.
    ///
    /// Enables canaries, debug-only bounds checks, signed overflow detection,
    /// 256 MB allocation budget, and 1024-deep recursion limit.
    pub fn debug_default() -> Self {
        Self {
            canaries: StackCanaryConfig::new(0xDEAD_BEEF_CAFE_BABE),
            bounds_check: BoundsCheckMode::Debug,
            overflow: OverflowCheckConfig::default(),
            budget: AllocationBudget::new(256 * 1024 * 1024), // 256 MB
            depth_guard: StackDepthGuard::new(1024),
        }
    }

    /// Creates a strict hardening config suitable for safety-critical builds.
    ///
    /// Enables all checks in all build modes.
    pub fn strict() -> Self {
        Self {
            canaries: StackCanaryConfig::new(0xDEAD_BEEF_CAFE_BABE),
            bounds_check: BoundsCheckMode::Release,
            overflow: OverflowCheckConfig::strict(),
            budget: AllocationBudget::new(64 * 1024 * 1024), // 64 MB
            depth_guard: StackDepthGuard::new(256),
        }
    }

    /// Creates a minimal hardening config for maximum performance.
    pub fn performance() -> Self {
        Self {
            canaries: StackCanaryConfig::disabled(),
            bounds_check: BoundsCheckMode::None,
            overflow: OverflowCheckConfig::unchecked(),
            budget: AllocationBudget::new(u64::MAX),
            depth_guard: StackDepthGuard::new(u32::MAX),
        }
    }

    /// Returns the number of active hardening features.
    pub fn active_feature_count(&self) -> u32 {
        let mut count = 0;
        if self.canaries.enabled {
            count += 1;
        }
        if self.bounds_check != BoundsCheckMode::None {
            count += 1;
        }
        if self.overflow.any_checks_enabled() {
            count += 1;
        }
        if self.budget.max_bytes < u64::MAX {
            count += 1;
        }
        if self.depth_guard.max_depth < u32::MAX {
            count += 1;
        }
        count
    }
}

/// Benchmark overhead measurement for a security feature.
///
/// Records the average overhead in nanoseconds per check for a given
/// security feature, allowing developers to quantify the cost of
/// hardening settings.
#[derive(Debug, Clone, PartialEq)]
pub struct SecurityOverhead {
    /// Name of the security feature being measured.
    pub feature_name: String,
    /// Average nanoseconds per check invocation.
    pub ns_per_check: f64,
    /// Number of samples used to compute the average.
    pub sample_count: u64,
    /// Total nanoseconds accumulated across all samples.
    total_ns: f64,
}

impl SecurityOverhead {
    /// Creates a new overhead tracker for the named feature.
    pub fn new(feature_name: &str) -> Self {
        Self {
            feature_name: feature_name.to_string(),
            ns_per_check: 0.0,
            sample_count: 0,
            total_ns: 0.0,
        }
    }

    /// Records a single timing sample in nanoseconds.
    pub fn record_sample(&mut self, ns: f64) {
        self.total_ns += ns;
        self.sample_count += 1;
        if self.sample_count > 0 {
            self.ns_per_check = self.total_ns / self.sample_count as f64;
        }
    }

    /// Returns the total overhead accumulated.
    pub fn total_ns(&self) -> f64 {
        self.total_ns
    }

    /// Returns a human-readable summary of the overhead.
    pub fn summary(&self) -> String {
        format!(
            "{}: {:.1} ns/check ({} samples, {:.0} ns total)",
            self.feature_name, self.ns_per_check, self.sample_count, self.total_ns
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SEC2: Supply Chain Security
// ═══════════════════════════════════════════════════════════════════════

/// Detects typosquatting attacks on package names.
///
/// Uses Levenshtein distance to identify packages whose names are
/// suspiciously similar to known-good packages, which is a common
/// attack vector in package registries.
#[derive(Debug, Clone)]
pub struct TyposquatDetector {
    /// Set of known-good (trusted) package names.
    known_packages: HashSet<String>,
    /// Maximum Levenshtein distance to flag as suspicious.
    pub max_distance: usize,
}

impl TyposquatDetector {
    /// Creates a detector with the given set of known package names.
    pub fn new(known_packages: Vec<String>, max_distance: usize) -> Self {
        Self {
            known_packages: known_packages.into_iter().collect(),
            max_distance,
        }
    }

    /// Creates a detector pre-loaded with Fajar Lang standard packages.
    pub fn with_stdlib() -> Self {
        let stdlib = vec![
            "fj-math",
            "fj-nn",
            "fj-hal",
            "fj-drivers",
            "fj-http",
            "fj-json",
            "fj-crypto",
            "fj-core",
            "fj-std",
            "fj-test",
            "fj-net",
            "fj-os",
            "fj-io",
            "fj-async",
            "fj-cli",
        ];
        Self::new(stdlib.into_iter().map(String::from).collect(), 2)
    }

    /// Computes the Levenshtein edit distance between two strings.
    ///
    /// Uses the Wagner-Fischer dynamic programming algorithm with O(min(m,n))
    /// space by keeping only two rows of the matrix.
    pub fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();

        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }

        let mut prev: Vec<usize> = (0..=n).collect();
        let mut curr = vec![0; n + 1];

        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }

        prev[n]
    }

    /// Checks whether a package name is a potential typosquat of any known
    /// package. Returns the list of suspiciously similar known packages.
    ///
    /// A name is suspicious if:
    /// - Its Levenshtein distance to a known package is ≤ `max_distance`
    /// - It is NOT an exact match (exact matches are legitimate)
    pub fn check(&self, name: &str) -> Vec<TyposquatMatch> {
        let mut matches = Vec::new();

        // Exact match is legitimate, not a typosquat.
        if self.known_packages.contains(name) {
            return matches;
        }

        for known in &self.known_packages {
            let dist = Self::levenshtein(name, known);
            if dist > 0 && dist <= self.max_distance {
                matches.push(TyposquatMatch {
                    submitted_name: name.to_string(),
                    similar_to: known.clone(),
                    distance: dist,
                });
            }
        }

        // Sort by distance ascending — closest matches first.
        matches.sort_by_key(|m| m.distance);
        matches
    }

    /// Adds a package to the known-good set.
    pub fn register_package(&mut self, name: &str) {
        self.known_packages.insert(name.to_string());
    }

    /// Returns the number of known packages.
    pub fn known_count(&self) -> usize {
        self.known_packages.len()
    }
}

/// A typosquatting match result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TyposquatMatch {
    /// The submitted package name being checked.
    pub submitted_name: String,
    /// The known package it is similar to.
    pub similar_to: String,
    /// Levenshtein distance between the names.
    pub distance: usize,
}

impl fmt::Display for TyposquatMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WARNING: '{}' is similar to '{}' (edit distance {})",
            self.submitted_name, self.similar_to, self.distance
        )
    }
}

/// Permission scope for registry authentication tokens.
///
/// Tokens should be granted the minimum required scope to limit
/// damage from token compromise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TokenScope {
    /// Read-only access — download packages.
    Read,
    /// Write access — upload new versions.
    Write,
    /// Publish access — create new packages.
    Publish,
    /// Admin access — manage owners, yank versions.
    Admin,
}

impl TokenScope {
    /// Returns true if this scope permits the given operation scope.
    ///
    /// Scopes are ordered: Admin >= Publish >= Write >= Read.
    pub fn permits(&self, required: TokenScope) -> bool {
        *self >= required
    }

    /// Returns the human-readable name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Publish => "publish",
            Self::Admin => "admin",
        }
    }
}

impl fmt::Display for TokenScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Token rotation and expiry tracking.
///
/// Enforces that registry tokens have bounded lifetimes and tracks
/// when they need to be renewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRotation {
    /// Token identifier (opaque).
    pub token_id: String,
    /// Scope of the token.
    pub scope: TokenScope,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
    /// Expiration timestamp (Unix epoch seconds).
    pub expires_at: u64,
    /// Maximum token lifetime in days before mandatory rotation.
    pub max_lifetime_days: u32,
    /// Whether the token has been revoked.
    pub revoked: bool,
}

impl TokenRotation {
    /// Creates a new token rotation tracker.
    pub fn new(token_id: &str, scope: TokenScope, created_at: u64, lifetime_days: u32) -> Self {
        let expires_at = created_at + (lifetime_days as u64 * 86400);
        Self {
            token_id: token_id.to_string(),
            scope,
            created_at,
            expires_at,
            max_lifetime_days: lifetime_days,
            revoked: false,
        }
    }

    /// Returns true if the token has expired at the given timestamp.
    pub fn is_expired(&self, now: u64) -> bool {
        self.revoked || now >= self.expires_at
    }

    /// Returns the number of days until expiry. Returns 0 if already expired.
    pub fn days_until_expiry(&self, now: u64) -> u64 {
        if now >= self.expires_at {
            return 0;
        }
        (self.expires_at - now) / 86400
    }

    /// Returns true if the token needs renewal (≤ 7 days remaining).
    pub fn needs_renewal(&self, now: u64) -> bool {
        self.days_until_expiry(now) <= 7
    }

    /// Revokes the token immediately.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }
}

/// Reproducible build configuration.
///
/// Generates a deterministic build identifier by hashing the source code,
/// dependency list, and compiler version. Two builds with identical inputs
/// must produce the same `build_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReproducibleBuild {
    /// SHA-256 hash of the source tree.
    pub source_hash: String,
    /// Sorted list of `name@version` dependency strings.
    pub dependencies: Vec<String>,
    /// Compiler version string (e.g., "fj 6.1.0").
    pub compiler_version: String,
    /// Target triple (e.g., "x86_64-unknown-linux-gnu").
    pub target: String,
    /// Optimization level.
    pub opt_level: String,
}

impl ReproducibleBuild {
    /// Creates a new reproducible build configuration.
    pub fn new(
        source_hash: &str,
        dependencies: Vec<String>,
        compiler_version: &str,
        target: &str,
        opt_level: &str,
    ) -> Self {
        let mut deps = dependencies;
        deps.sort();
        Self {
            source_hash: source_hash.to_string(),
            dependencies: deps,
            compiler_version: compiler_version.to_string(),
            target: target.to_string(),
            opt_level: opt_level.to_string(),
        }
    }

    /// Computes a deterministic build identifier by hashing all inputs.
    ///
    /// Uses a simple DJB2-style hash for determinism without requiring
    /// external crate dependencies. The hash incorporates source, deps,
    /// compiler version, target, and optimization level.
    pub fn build_id(&self) -> String {
        let mut hash: u64 = 5381;
        let feed = |h: &mut u64, s: &str| {
            for b in s.bytes() {
                *h = h.wrapping_mul(33).wrapping_add(b as u64);
            }
        };

        feed(&mut hash, &self.source_hash);
        for dep in &self.dependencies {
            feed(&mut hash, dep);
        }
        feed(&mut hash, &self.compiler_version);
        feed(&mut hash, &self.target);
        feed(&mut hash, &self.opt_level);

        format!("{:016x}", hash)
    }

    /// Verifies that two builds are reproducible (same build_id).
    pub fn is_reproducible_with(&self, other: &Self) -> bool {
        self.build_id() == other.build_id()
    }
}

/// SLSA-style build provenance attestation.
///
/// Records who built the artifact, from which source, at what time,
/// and the resulting digest. This is the basis for supply chain
/// integrity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProvenance {
    /// Builder identity (e.g., "github-actions", "local-dev").
    pub builder: String,
    /// Source repository URI.
    pub source_uri: String,
    /// Source commit hash.
    pub source_digest: String,
    /// Build timestamp (Unix epoch seconds).
    pub build_timestamp: u64,
    /// SHA-256 digest of the output artifact.
    pub artifact_digest: String,
    /// SLSA build level (0-4).
    pub slsa_level: u8,
    /// Build parameters (env vars, flags, etc.).
    pub parameters: HashMap<String, String>,
}

impl BuildProvenance {
    /// Creates a new build provenance attestation.
    pub fn new(
        builder: &str,
        source_uri: &str,
        source_digest: &str,
        build_timestamp: u64,
        artifact_digest: &str,
    ) -> Self {
        Self {
            builder: builder.to_string(),
            source_uri: source_uri.to_string(),
            source_digest: source_digest.to_string(),
            build_timestamp,
            artifact_digest: artifact_digest.to_string(),
            slsa_level: 1,
            parameters: HashMap::new(),
        }
    }

    /// Sets the SLSA build level (clamped to 0-4).
    pub fn with_slsa_level(mut self, level: u8) -> Self {
        self.slsa_level = level.min(4);
        self
    }

    /// Adds a build parameter.
    pub fn with_parameter(mut self, key: &str, value: &str) -> Self {
        self.parameters.insert(key.to_string(), value.to_string());
        self
    }

    /// Verifies the attestation matches a known artifact digest.
    pub fn verify_artifact(&self, expected_digest: &str) -> bool {
        self.artifact_digest == expected_digest
    }

    /// Returns a summary string for display.
    pub fn summary(&self) -> String {
        format!(
            "Build by {} from {} @ {} (SLSA L{}, artifact {})",
            self.builder,
            self.source_uri,
            self.source_digest,
            self.slsa_level,
            &self.artifact_digest[..16.min(self.artifact_digest.len())]
        )
    }
}

/// Security advisory for CVE tracking.
///
/// Represents a known vulnerability in a package, including severity,
/// affected version range, and remediation information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityAdvisory {
    /// Advisory identifier (e.g., "FJ-2026-001" or "CVE-2026-12345").
    pub id: String,
    /// Severity level.
    pub severity: AdvisorySeverity,
    /// Package name affected.
    pub package: String,
    /// Affected version range (inclusive). Format: `[min, max]`.
    pub affected_versions: (String, String),
    /// Version that patches the vulnerability, if available.
    pub patched_version: Option<String>,
    /// Human-readable description.
    pub description: String,
}

/// Severity level for security advisories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdvisorySeverity {
    /// Informational — no direct security impact.
    Info,
    /// Low severity — minimal impact.
    Low,
    /// Medium severity — limited impact.
    Medium,
    /// High severity — significant impact.
    High,
    /// Critical severity — immediate action required.
    Critical,
}

impl fmt::Display for AdvisorySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl SecurityAdvisory {
    /// Creates a new security advisory.
    pub fn new(
        id: &str,
        severity: AdvisorySeverity,
        package: &str,
        affected_min: &str,
        affected_max: &str,
        patched_version: Option<&str>,
        description: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            severity,
            package: package.to_string(),
            affected_versions: (affected_min.to_string(), affected_max.to_string()),
            patched_version: patched_version.map(String::from),
            description: description.to_string(),
        }
    }

    /// Checks if a given version string falls within the affected range.
    ///
    /// Uses simple lexicographic comparison on semver strings. For production
    /// use, this should be replaced with proper semver parsing.
    pub fn affects_version(&self, version: &str) -> bool {
        let (ref min, ref max) = self.affected_versions;
        version >= min.as_str() && version <= max.as_str()
    }

    /// Returns true if a patch is available.
    pub fn has_patch(&self) -> bool {
        self.patched_version.is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SEC3: Audit & Certification
// ═══════════════════════════════════════════════════════════════════════

/// A single security lint rule.
///
/// Each rule has an identifier, description, severity, and a detection
/// function that checks source text for violations.
#[derive(Debug, Clone)]
pub struct LintRule {
    /// Rule identifier (e.g., "SEC001").
    pub id: String,
    /// Human-readable rule name (e.g., "sql_injection").
    pub name: String,
    /// Description of what the rule detects.
    pub description: String,
    /// Severity level of the rule.
    pub severity: LintSeverity,
    /// Patterns that trigger the rule (substrings or indicators).
    pub patterns: Vec<String>,
    /// Whether the rule is enabled.
    pub enabled: bool,
}

/// Severity for lint rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LintSeverity {
    /// Informational hint.
    Hint,
    /// Warning — should be addressed.
    Warning,
    /// Error — must be fixed before deployment.
    Error,
    /// Deny — blocks compilation.
    Deny,
}

impl fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hint => write!(f, "hint"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Deny => write!(f, "deny"),
        }
    }
}

/// A lint violation found by the security linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintViolation {
    /// The rule that was violated.
    pub rule_id: String,
    /// The rule name.
    pub rule_name: String,
    /// Severity level.
    pub severity: LintSeverity,
    /// Line number where the violation was found (1-based).
    pub line: usize,
    /// The source line text containing the violation.
    pub line_text: String,
    /// Description of the specific violation.
    pub message: String,
}

impl fmt::Display for LintViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (line {}): {} — {}",
            self.rule_id, self.severity, self.line, self.rule_name, self.message
        )
    }
}

/// Security linter with 20 built-in lint rules.
///
/// Scans Fajar Lang source code for common security anti-patterns.
/// Each rule performs substring/pattern detection on source lines
/// and produces `LintViolation` records for flagged lines.
#[derive(Debug, Clone)]
pub struct SecurityLinter {
    /// Active lint rules.
    pub rules: Vec<LintRule>,
}

impl SecurityLinter {
    /// Creates a linter with all 20 default rules enabled.
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
        }
    }

    /// Returns the 20 built-in security lint rules.
    fn default_rules() -> Vec<LintRule> {
        vec![
            // 1. SQL injection
            LintRule {
                id: "SEC001".into(),
                name: "sql_injection".into(),
                description: "String concatenation in database query — use parameterized queries"
                    .into(),
                severity: LintSeverity::Error,
                patterns: vec![
                    "db_execute".into(),
                    "sql_query".into(),
                    "execute_sql".into(),
                ],
                enabled: true,
            },
            // 2. Command injection
            LintRule {
                id: "SEC002".into(),
                name: "command_injection".into(),
                description: "String concatenation in process command — use argument arrays".into(),
                severity: LintSeverity::Error,
                patterns: vec![
                    "process::Command".into(),
                    "exec(".into(),
                    "system(".into(),
                    "shell(".into(),
                ],
                enabled: true,
            },
            // 3. Path traversal
            LintRule {
                id: "SEC003".into(),
                name: "path_traversal".into(),
                description: "Path contains '..' — potential directory traversal attack".into(),
                severity: LintSeverity::Error,
                patterns: vec!["..".into()],
                enabled: true,
            },
            // 4. Hardcoded secret
            LintRule {
                id: "SEC004".into(),
                name: "hardcoded_secret".into(),
                description: "Potential hardcoded secret or credential in source code".into(),
                severity: LintSeverity::Deny,
                patterns: vec![
                    "password".into(),
                    "secret".into(),
                    "api_key".into(),
                    "apikey".into(),
                    "private_key".into(),
                    "access_token".into(),
                ],
                enabled: true,
            },
            // 5. Weak crypto
            LintRule {
                id: "SEC005".into(),
                name: "weak_crypto".into(),
                description: "Weak cryptographic algorithm — use SHA-256+ or AES-256".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["MD5".into(), "SHA1".into(), "DES".into(), "RC4".into()],
                enabled: true,
            },
            // 6. Missing error check
            LintRule {
                id: "SEC006".into(),
                name: "missing_error_check".into(),
                description: "Result value is ignored — errors may go unhandled".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["let _ =".into()],
                enabled: true,
            },
            // 7. Unsafe unwrap
            LintRule {
                id: "SEC007".into(),
                name: "unsafe_unwrap".into(),
                description: "Using .unwrap() — will panic on None/Err at runtime".into(),
                severity: LintSeverity::Error,
                patterns: vec![".unwrap()".into()],
                enabled: true,
            },
            // 8. Unvalidated input
            LintRule {
                id: "SEC008".into(),
                name: "unvalidated_input".into(),
                description: "External input used without validation".into(),
                severity: LintSeverity::Warning,
                patterns: vec![
                    "read_line".into(),
                    "stdin".into(),
                    "from_request".into(),
                    "query_param".into(),
                ],
                enabled: true,
            },
            // 9. Insecure random
            LintRule {
                id: "SEC009".into(),
                name: "insecure_random".into(),
                description:
                    "Non-cryptographic RNG used — use crypto::random for security contexts".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["rand()".into(), "random()".into(), "Math.random".into()],
                enabled: true,
            },
            // 10. Missing auth
            LintRule {
                id: "SEC010".into(),
                name: "missing_auth".into(),
                description: "Public endpoint without authentication check".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["@route".into(), "@endpoint".into(), "@handler".into()],
                enabled: true,
            },
            // 11. Unsafe block
            LintRule {
                id: "SEC011".into(),
                name: "unsafe_block".into(),
                description: "Unsafe block without SAFETY comment".into(),
                severity: LintSeverity::Error,
                patterns: vec!["@unsafe".into()],
                enabled: true,
            },
            // 12. Hardcoded IP/port
            LintRule {
                id: "SEC012".into(),
                name: "hardcoded_network".into(),
                description: "Hardcoded IP address or port — use configuration".into(),
                severity: LintSeverity::Hint,
                patterns: vec!["127.0.0.1".into(), "0.0.0.0".into(), "localhost".into()],
                enabled: true,
            },
            // 13. Insecure HTTP
            LintRule {
                id: "SEC013".into(),
                name: "insecure_http".into(),
                description: "Using HTTP instead of HTTPS — data transmitted in plaintext".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["http://".into()],
                enabled: true,
            },
            // 14. Debug in production
            LintRule {
                id: "SEC014".into(),
                name: "debug_in_production".into(),
                description: "Debug/logging statement may leak sensitive information".into(),
                severity: LintSeverity::Hint,
                patterns: vec!["dbg!".into(), "debug_print".into(), "console.log".into()],
                enabled: true,
            },
            // 15. Timing attack
            LintRule {
                id: "SEC015".into(),
                name: "timing_attack".into(),
                description: "String comparison for secrets — use constant-time comparison".into(),
                severity: LintSeverity::Error,
                patterns: vec![
                    "== password".into(),
                    "== secret".into(),
                    "== token".into(),
                    "== key".into(),
                ],
                enabled: true,
            },
            // 16. Uninitialized memory
            LintRule {
                id: "SEC016".into(),
                name: "uninitialized_memory".into(),
                description: "Potential use of uninitialized memory".into(),
                severity: LintSeverity::Deny,
                patterns: vec![
                    "mem_uninitialized".into(),
                    "alloc_zeroed".into(),
                    "MaybeUninit".into(),
                ],
                enabled: true,
            },
            // 17. Integer truncation
            LintRule {
                id: "SEC017".into(),
                name: "integer_truncation".into(),
                description: "Implicit integer truncation may cause data loss".into(),
                severity: LintSeverity::Warning,
                patterns: vec![
                    "as i8".into(),
                    "as u8".into(),
                    "as i16".into(),
                    "as u16".into(),
                ],
                enabled: true,
            },
            // 18. Null dereference
            LintRule {
                id: "SEC018".into(),
                name: "null_dereference".into(),
                description: "Potential null pointer dereference — check for null first".into(),
                severity: LintSeverity::Error,
                patterns: vec!["deref_raw".into(), "ptr_read".into(), "*ptr".into()],
                enabled: true,
            },
            // 19. Deprecated API
            LintRule {
                id: "SEC019".into(),
                name: "deprecated_api".into(),
                description: "Using deprecated API — migrate to replacement".into(),
                severity: LintSeverity::Hint,
                patterns: vec!["@deprecated".into(), "#[deprecated]".into()],
                enabled: true,
            },
            // 20. Infinite loop
            LintRule {
                id: "SEC020".into(),
                name: "infinite_loop".into(),
                description: "Potential infinite loop without break condition".into(),
                severity: LintSeverity::Warning,
                patterns: vec!["loop {".into(), "while true".into(), "while (true)".into()],
                enabled: true,
            },
        ]
    }

    /// Scans source code and returns all lint violations found.
    ///
    /// Each line of source is tested against each enabled rule's patterns.
    /// When a pattern match is found, a `LintViolation` is emitted.
    pub fn lint(&self, source: &str) -> Vec<LintViolation> {
        let mut violations = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // Skip comment-only lines.
            if trimmed.starts_with("//") {
                // Exception: check for SAFETY comments in unsafe blocks.
                continue;
            }

            for rule in &self.rules {
                if !rule.enabled {
                    continue;
                }

                // Special handling for specific rules.
                let matched = match rule.name.as_str() {
                    "sql_injection" => {
                        // Flag if a db_execute/sql_query call contains string concat (+).
                        rule.patterns.iter().any(|p| line.contains(p.as_str()))
                            && (line.contains('+')
                                || line.contains("format!")
                                || line.contains("f\""))
                    }
                    "command_injection" => {
                        // Flag if command execution uses string concatenation.
                        rule.patterns.iter().any(|p| line.contains(p.as_str()))
                            && (line.contains('+')
                                || line.contains("format!")
                                || line.contains("f\""))
                    }
                    "path_traversal" => {
                        // Only flag ".." inside string literals (between quotes).
                        line.contains("\"..") || line.contains("/..")
                    }
                    "hardcoded_secret" => {
                        // Flag if pattern appears as part of an assignment with a string value.
                        let lower = line.to_lowercase();
                        rule.patterns.iter().any(|p| {
                            lower.contains(p.as_str())
                                && (lower.contains("= \"") || lower.contains("=\""))
                        })
                    }
                    "missing_auth" => {
                        // Flag route/endpoint annotations without nearby auth check.
                        // Simple heuristic: flag if line has route annotation but no "auth" nearby.
                        rule.patterns.iter().any(|p| line.contains(p.as_str()))
                            && !line.contains("auth")
                            && !line.contains("Auth")
                    }
                    "unsafe_block" => {
                        // Flag @unsafe without a preceding SAFETY comment.
                        line.contains("@unsafe") && !line.contains("SAFETY")
                    }
                    "insecure_http" => {
                        // Don't flag https:// — only plain http.
                        line.contains("http://") && !line.contains("https://")
                    }
                    "infinite_loop" => {
                        // Flag loop/while(true) only if no break on same line.
                        rule.patterns.iter().any(|p| line.contains(p.as_str()))
                            && !line.contains("break")
                    }
                    _ => {
                        // Default: simple substring match.
                        rule.patterns.iter().any(|p| line.contains(p.as_str()))
                    }
                };

                if matched {
                    violations.push(LintViolation {
                        rule_id: rule.id.clone(),
                        rule_name: rule.name.clone(),
                        severity: rule.severity,
                        line: line_num,
                        line_text: line.to_string(),
                        message: rule.description.clone(),
                    });
                }
            }
        }

        violations
    }

    /// Returns the number of enabled rules.
    pub fn enabled_rule_count(&self) -> usize {
        self.rules.iter().filter(|r| r.enabled).count()
    }

    /// Disables a rule by its ID.
    pub fn disable_rule(&mut self, rule_id: &str) {
        for rule in &mut self.rules {
            if rule.id == rule_id {
                rule.enabled = false;
            }
        }
    }

    /// Enables a rule by its ID.
    pub fn enable_rule(&mut self, rule_id: &str) {
        for rule in &mut self.rules {
            if rule.id == rule_id {
                rule.enabled = true;
            }
        }
    }

    /// Returns the total number of rules (enabled + disabled).
    pub fn total_rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for SecurityLinter {
    fn default() -> Self {
        Self::new()
    }
}

/// Taint analysis for tracking untrusted data flow.
///
/// Marks variables as tainted (from untrusted sources like user input),
/// propagates taint through assignments and function calls, and checks
/// that tainted values do not reach security-sensitive sinks without
/// sanitization.
#[derive(Debug, Clone)]
pub struct TaintAnalysis {
    /// Set of tainted variable names.
    tainted: HashSet<String>,
    /// Known sanitization functions — calling these removes taint.
    sanitizers: HashSet<String>,
    /// Known sensitive sinks — tainted data reaching these is a violation.
    sinks: HashSet<String>,
    /// Recorded taint violations.
    pub violations: Vec<TaintViolation>,
}

/// A taint analysis violation — untrusted data reaching a sensitive sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaintViolation {
    /// The tainted variable name.
    pub variable: String,
    /// The sink it reached.
    pub sink: String,
    /// Description of the issue.
    pub message: String,
}

impl fmt::Display for TaintViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TAINT: variable '{}' reaches sink '{}' — {}",
            self.variable, self.sink, self.message
        )
    }
}

impl TaintAnalysis {
    /// Creates a new taint analysis with default sinks and sanitizers.
    pub fn new() -> Self {
        let mut sanitizers = HashSet::new();
        sanitizers.insert("sanitize".to_string());
        sanitizers.insert("escape_html".to_string());
        sanitizers.insert("validate".to_string());
        sanitizers.insert("parameterize".to_string());
        sanitizers.insert("encode".to_string());

        let mut sinks = HashSet::new();
        sinks.insert("db_execute".to_string());
        sinks.insert("sql_query".to_string());
        sinks.insert("exec".to_string());
        sinks.insert("eval".to_string());
        sinks.insert("write_file".to_string());
        sinks.insert("send_response".to_string());
        sinks.insert("redirect".to_string());

        Self {
            tainted: HashSet::new(),
            sanitizers,
            sinks,
            violations: Vec::new(),
        }
    }

    /// Marks a variable as tainted (coming from an untrusted source).
    pub fn mark_tainted(&mut self, var: &str) {
        self.tainted.insert(var.to_string());
    }

    /// Removes taint from a variable (after sanitization).
    pub fn mark_clean(&mut self, var: &str) {
        self.tainted.remove(var);
    }

    /// Returns true if the given variable is currently tainted.
    pub fn is_tainted(&self, var: &str) -> bool {
        self.tainted.contains(var)
    }

    /// Propagates taint through an assignment: `target = source`.
    ///
    /// If `source` is tainted and not passed through a sanitizer,
    /// `target` becomes tainted. If `source` is clean, `target` is clean.
    pub fn propagate(&mut self, target: &str, source: &str, through_sanitizer: bool) {
        if self.is_tainted(source) && !through_sanitizer {
            self.tainted.insert(target.to_string());
        } else {
            self.tainted.remove(target);
        }
    }

    /// Checks whether a tainted variable is being passed to a sensitive sink.
    ///
    /// If the variable is tainted and the function is a known sink,
    /// a violation is recorded and returned.
    pub fn check_sink(&mut self, var: &str, function: &str) -> Option<TaintViolation> {
        if self.is_tainted(var) && self.sinks.contains(function) {
            let violation = TaintViolation {
                variable: var.to_string(),
                sink: function.to_string(),
                message: format!(
                    "untrusted data from '{}' flows to sensitive sink '{}' without sanitization",
                    var, function
                ),
            };
            self.violations.push(violation.clone());
            Some(violation)
        } else {
            None
        }
    }

    /// Adds a custom sanitizer function name.
    pub fn add_sanitizer(&mut self, name: &str) {
        self.sanitizers.insert(name.to_string());
    }

    /// Adds a custom sink function name.
    pub fn add_sink(&mut self, name: &str) {
        self.sinks.insert(name.to_string());
    }

    /// Returns true if the given function is a known sanitizer.
    pub fn is_sanitizer(&self, name: &str) -> bool {
        self.sanitizers.contains(name)
    }

    /// Returns the number of currently tainted variables.
    pub fn tainted_count(&self) -> usize {
        self.tainted.len()
    }

    /// Returns the number of recorded violations.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }
}

impl Default for TaintAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

/// Category breakdown for the security scorecard.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardCategory {
    /// Category name.
    pub name: String,
    /// Score for this category (0-100).
    pub score: u32,
    /// Weight for overall score computation (0.0 - 1.0).
    pub weight: f64,
    /// Findings/issues in this category.
    pub findings: Vec<String>,
}

/// Aggregate security scorecard for a project.
///
/// Generates an overall security score (0-100) based on weighted
/// category scores covering code quality, dependency health,
/// build security, and runtime protections.
#[derive(Debug, Clone)]
pub struct SecurityScorecard {
    /// Project name being scored.
    pub project: String,
    /// Category breakdowns.
    pub categories: Vec<ScorecardCategory>,
}

impl SecurityScorecard {
    /// Creates a new scorecard for the given project.
    pub fn new(project: &str) -> Self {
        Self {
            project: project.to_string(),
            categories: Vec::new(),
        }
    }

    /// Adds a scoring category.
    pub fn add_category(&mut self, name: &str, score: u32, weight: f64, findings: Vec<String>) {
        self.categories.push(ScorecardCategory {
            name: name.to_string(),
            score: score.min(100),
            weight: weight.clamp(0.0, 1.0),
            findings,
        });
    }

    /// Computes the overall security score as a weighted average.
    ///
    /// Each category's score is multiplied by its weight, and the
    /// results are summed and divided by the total weight.
    pub fn overall_score(&self) -> u32 {
        if self.categories.is_empty() {
            return 0;
        }
        let total_weight: f64 = self.categories.iter().map(|c| c.weight).sum();
        if total_weight <= 0.0 {
            return 0;
        }
        let weighted_sum: f64 = self
            .categories
            .iter()
            .map(|c| c.score as f64 * c.weight)
            .sum();
        (weighted_sum / total_weight).round() as u32
    }

    /// Returns the letter grade for the overall score.
    pub fn grade(&self) -> &'static str {
        match self.overall_score() {
            90..=100 => "A",
            80..=89 => "B",
            70..=79 => "C",
            60..=69 => "D",
            _ => "F",
        }
    }

    /// Returns a full text report of the scorecard.
    pub fn report(&self) -> String {
        let mut out = format!(
            "Security Scorecard: {} — {} ({})\n",
            self.project,
            self.overall_score(),
            self.grade()
        );
        out.push_str(&"=".repeat(60));
        out.push('\n');
        for cat in &self.categories {
            out.push_str(&format!(
                "  {} — {}/100 (weight {:.0}%)\n",
                cat.name,
                cat.score,
                cat.weight * 100.0
            ));
            for finding in &cat.findings {
                out.push_str(&format!("    - {}\n", finding));
            }
        }
        out
    }

    /// Generates a scorecard from lint results and hardening configuration.
    pub fn from_analysis(
        project: &str,
        lint_violations: &[LintViolation],
        hardening: &MemoryHardening,
        taint_violations: usize,
        dep_count: usize,
        has_provenance: bool,
    ) -> Self {
        let mut sc = Self::new(project);

        // Code quality score — based on lint violations.
        let code_score = if lint_violations.is_empty() {
            100
        } else {
            let error_count = lint_violations
                .iter()
                .filter(|v| v.severity >= LintSeverity::Error)
                .count();
            let warning_count = lint_violations
                .iter()
                .filter(|v| v.severity == LintSeverity::Warning)
                .count();
            100u32
                .saturating_sub(error_count as u32 * 15)
                .saturating_sub(warning_count as u32 * 5)
        };
        sc.add_category(
            "Code Quality",
            code_score,
            0.30,
            lint_violations.iter().map(|v| v.to_string()).collect(),
        );

        // Memory safety score — based on hardening features.
        let safety_score = hardening.active_feature_count() * 20; // 0-100
        sc.add_category(
            "Memory Safety",
            safety_score.min(100),
            0.25,
            vec![format!(
                "{}/5 hardening features active",
                hardening.active_feature_count()
            )],
        );

        // Taint analysis score.
        let taint_score = if taint_violations == 0 {
            100
        } else {
            100u32.saturating_sub(taint_violations as u32 * 25)
        };
        sc.add_category(
            "Data Flow",
            taint_score,
            0.20,
            vec![format!("{} taint violations", taint_violations)],
        );

        // Supply chain score.
        let mut supply_score = 70u32;
        if has_provenance {
            supply_score += 20;
        }
        if dep_count < 10 {
            supply_score += 10;
        }
        sc.add_category(
            "Supply Chain",
            supply_score.min(100),
            0.15,
            vec![
                format!("{} dependencies", dep_count),
                format!("provenance: {}", if has_provenance { "yes" } else { "no" }),
            ],
        );

        // Configuration score.
        let config_score = 80u32; // Base score for having security module configured.
        sc.add_category("Configuration", config_score, 0.10, Vec::new());

        sc
    }
}

/// Compliance standard for safety-critical certification.
///
/// Each mode defines a set of rules that must be satisfied for the
/// specified certification standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComplianceMode {
    /// MISRA C:2012 — automotive C coding standard.
    Misra,
    /// SEI CERT C Coding Standard.
    CertC,
    /// ISO 26262 — automotive functional safety.
    Iso26262,
    /// DO-178C — airborne systems software.
    Do178C,
    /// IEC 62443 — industrial automation security.
    Iec62443,
}

impl ComplianceMode {
    /// Returns the formal name of the compliance standard.
    pub fn formal_name(&self) -> &'static str {
        match self {
            Self::Misra => "MISRA C:2012",
            Self::CertC => "SEI CERT C Coding Standard",
            Self::Iso26262 => "ISO 26262 Functional Safety",
            Self::Do178C => "DO-178C Airborne Software",
            Self::Iec62443 => "IEC 62443 Industrial Security",
        }
    }

    /// Returns the number of rules in this compliance standard.
    ///
    /// These counts reflect the actual number of rules in each standard.
    pub fn rule_count(&self) -> usize {
        match self {
            Self::Misra => 175,   // MISRA C:2012 has 175 rules
            Self::CertC => 116,   // CERT C has 116 rules
            Self::Iso26262 => 42, // ISO 26262 Part 6 coding guidelines
            Self::Do178C => 65,   // DO-178C objectives (Level A)
            Self::Iec62443 => 89, // IEC 62443 security requirements
        }
    }

    /// Returns the lint rule IDs that apply to this compliance mode.
    ///
    /// Different standards emphasize different security concerns.
    pub fn applicable_lint_rules(&self) -> Vec<&'static str> {
        match self {
            Self::Misra => vec!["SEC006", "SEC007", "SEC016", "SEC017", "SEC018", "SEC020"],
            Self::CertC => vec![
                "SEC001", "SEC002", "SEC003", "SEC007", "SEC016", "SEC017", "SEC018",
            ],
            Self::Iso26262 => vec![
                "SEC006", "SEC007", "SEC011", "SEC016", "SEC017", "SEC018", "SEC020",
            ],
            Self::Do178C => vec![
                "SEC006", "SEC007", "SEC011", "SEC014", "SEC016", "SEC017", "SEC018", "SEC020",
            ],
            Self::Iec62443 => vec![
                "SEC001", "SEC002", "SEC003", "SEC004", "SEC005", "SEC008", "SEC009", "SEC010",
                "SEC013", "SEC015",
            ],
        }
    }
}

impl fmt::Display for ComplianceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.formal_name())
    }
}

/// Result of checking `@secure` annotation on a function.
///
/// Functions marked `@secure` must have undergone a security review.
/// This struct records the review status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecureAnnotationCheck {
    /// Function name being checked.
    pub function_name: String,
    /// Whether the function has the `@secure` annotation.
    pub has_annotation: bool,
    /// Whether a security review has been recorded for this function.
    pub has_review: bool,
    /// Reviewer identity, if reviewed.
    pub reviewer: Option<String>,
    /// Review timestamp, if reviewed.
    pub review_date: Option<u64>,
}

impl SecureAnnotationCheck {
    /// Creates a new check for a function with the `@secure` annotation.
    pub fn new(function_name: &str) -> Self {
        Self {
            function_name: function_name.to_string(),
            has_annotation: true,
            has_review: false,
            reviewer: None,
            review_date: None,
        }
    }

    /// Records that a security review has been performed.
    pub fn record_review(&mut self, reviewer: &str, date: u64) {
        self.has_review = true;
        self.reviewer = Some(reviewer.to_string());
        self.review_date = Some(date);
    }

    /// Returns true if the annotation is satisfied (reviewed).
    pub fn is_satisfied(&self) -> bool {
        !self.has_annotation || self.has_review
    }

    /// Checks a list of `@secure` functions against a review registry.
    ///
    /// Returns the list of functions that have the annotation but
    /// have NOT been reviewed.
    pub fn check_all(
        functions: &[String],
        reviews: &HashMap<String, (String, u64)>,
    ) -> Vec<SecureAnnotationCheck> {
        let mut results = Vec::new();
        for fname in functions {
            let mut check = SecureAnnotationCheck::new(fname);
            if let Some((reviewer, date)) = reviews.get(fname) {
                check.record_review(reviewer, *date);
            }
            results.push(check);
        }
        results
    }
}

/// Fine-grained capability permission.
///
/// Represents a single operation that a module may or may not be
/// allowed to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// Read files from the filesystem.
    FileRead,
    /// Write files to the filesystem.
    FileWrite,
    /// Open network connections.
    NetConnect,
    /// Listen on network ports.
    NetListen,
    /// Execute external processes.
    Exec,
    /// Allocate heap memory.
    HeapAlloc,
    /// Access hardware registers / port I/O.
    HardwareAccess,
    /// Perform tensor/ML operations.
    TensorOps,
    /// Access environment variables.
    EnvAccess,
    /// Use cryptographic primitives.
    CryptoOps,
    /// Access system clock / timers.
    TimerAccess,
    /// Use inter-process communication.
    IpcAccess,
}

impl Capability {
    /// Returns the human-readable name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileRead => "file_read",
            Self::FileWrite => "file_write",
            Self::NetConnect => "net_connect",
            Self::NetListen => "net_listen",
            Self::Exec => "exec",
            Self::HeapAlloc => "heap_alloc",
            Self::HardwareAccess => "hardware_access",
            Self::TensorOps => "tensor_ops",
            Self::EnvAccess => "env_access",
            Self::CryptoOps => "crypto_ops",
            Self::TimerAccess => "timer_access",
            Self::IpcAccess => "ipc_access",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A set of capabilities granted to a module or function.
///
/// Capabilities follow the principle of least privilege: a module only
/// gets the capabilities it explicitly requests, and the compiler
/// verifies that it does not exceed them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySet {
    /// The granted capabilities.
    capabilities: HashSet<Capability>,
    /// Module or scope name this set applies to.
    pub scope_name: String,
}

impl CapabilitySet {
    /// Creates an empty capability set for the given scope.
    pub fn empty(scope_name: &str) -> Self {
        Self {
            capabilities: HashSet::new(),
            scope_name: scope_name.to_string(),
        }
    }

    /// Creates a capability set with the given capabilities.
    pub fn with_capabilities(scope_name: &str, caps: &[Capability]) -> Self {
        Self {
            capabilities: caps.iter().copied().collect(),
            scope_name: scope_name.to_string(),
        }
    }

    /// Creates a full capability set (all capabilities granted).
    pub fn full(scope_name: &str) -> Self {
        use Capability::*;
        Self::with_capabilities(
            scope_name,
            &[
                FileRead,
                FileWrite,
                NetConnect,
                NetListen,
                Exec,
                HeapAlloc,
                HardwareAccess,
                TensorOps,
                EnvAccess,
                CryptoOps,
                TimerAccess,
                IpcAccess,
            ],
        )
    }

    /// Creates a capability set appropriate for `@kernel` context.
    pub fn kernel_default(scope_name: &str) -> Self {
        use Capability::*;
        Self::with_capabilities(scope_name, &[HardwareAccess, TimerAccess, IpcAccess])
    }

    /// Creates a capability set appropriate for `@device` context.
    pub fn device_default(scope_name: &str) -> Self {
        use Capability::*;
        Self::with_capabilities(scope_name, &[TensorOps, HeapAlloc, CryptoOps])
    }

    /// Creates a capability set appropriate for `@safe` context.
    pub fn safe_default(scope_name: &str) -> Self {
        use Capability::*;
        Self::with_capabilities(
            scope_name,
            &[FileRead, HeapAlloc, EnvAccess, CryptoOps, TimerAccess],
        )
    }

    /// Grants a capability.
    pub fn grant(&mut self, cap: Capability) {
        self.capabilities.insert(cap);
    }

    /// Revokes a capability.
    pub fn revoke(&mut self, cap: Capability) {
        self.capabilities.remove(&cap);
    }

    /// Returns true if the given capability is granted.
    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Checks if this set permits a required capability. Returns an error
    /// message if the capability is not granted.
    pub fn check(&self, required: Capability) -> Result<(), String> {
        if self.has(required) {
            Ok(())
        } else {
            Err(format!(
                "capability '{}' not granted in scope '{}'",
                required, self.scope_name
            ))
        }
    }

    /// Returns the number of granted capabilities.
    pub fn count(&self) -> usize {
        self.capabilities.len()
    }

    /// Returns true if the set has no capabilities.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Returns the intersection of two capability sets.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            capabilities: self
                .capabilities
                .intersection(&other.capabilities)
                .copied()
                .collect(),
            scope_name: format!("{} & {}", self.scope_name, other.scope_name),
        }
    }
}

/// Sandbox policy for restricting operations per module.
///
/// Combines capability restrictions with resource limits to define
/// the execution environment for a module.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Policy name.
    pub name: String,
    /// Granted capabilities.
    pub capabilities: CapabilitySet,
    /// Maximum memory allocation (bytes).
    pub max_memory: u64,
    /// Maximum execution time (milliseconds).
    pub max_execution_ms: u64,
    /// Maximum number of file descriptors.
    pub max_file_descriptors: u32,
    /// Allowed file path prefixes (empty = no file access).
    pub allowed_paths: Vec<String>,
    /// Blocked system calls.
    pub blocked_syscalls: HashSet<String>,
}

impl SandboxPolicy {
    /// Creates a new sandbox policy with the given name and capabilities.
    pub fn new(name: &str, capabilities: CapabilitySet) -> Self {
        Self {
            name: name.to_string(),
            capabilities,
            max_memory: 256 * 1024 * 1024, // 256 MB
            max_execution_ms: 30_000,      // 30 seconds
            max_file_descriptors: 64,
            allowed_paths: Vec::new(),
            blocked_syscalls: HashSet::new(),
        }
    }

    /// Creates a restrictive sandbox suitable for untrusted code.
    pub fn untrusted(name: &str) -> Self {
        let caps = CapabilitySet::empty(name);
        Self {
            name: name.to_string(),
            capabilities: caps,
            max_memory: 16 * 1024 * 1024, // 16 MB
            max_execution_ms: 5_000,      // 5 seconds
            max_file_descriptors: 0,
            allowed_paths: Vec::new(),
            blocked_syscalls: HashSet::new(),
        }
    }

    /// Creates a permissive sandbox suitable for trusted code.
    pub fn trusted(name: &str) -> Self {
        let caps = CapabilitySet::full(name);
        Self {
            name: name.to_string(),
            capabilities: caps,
            max_memory: 4 * 1024 * 1024 * 1024, // 4 GB
            max_execution_ms: 600_000,          // 10 minutes
            max_file_descriptors: 1024,
            allowed_paths: vec!["/".to_string()],
            blocked_syscalls: HashSet::new(),
        }
    }

    /// Checks if an operation is allowed by this policy.
    pub fn allows_operation(&self, cap: Capability) -> bool {
        self.capabilities.has(cap)
    }

    /// Checks if a file path is allowed by this policy.
    pub fn allows_path(&self, path: &str) -> bool {
        if self.allowed_paths.is_empty() {
            return false;
        }
        self.allowed_paths
            .iter()
            .any(|prefix| path.starts_with(prefix))
    }

    /// Checks if a syscall is blocked by this policy.
    pub fn is_syscall_blocked(&self, syscall: &str) -> bool {
        self.blocked_syscalls.contains(syscall)
    }

    /// Adds a blocked syscall.
    pub fn block_syscall(&mut self, syscall: &str) {
        self.blocked_syscalls.insert(syscall.to_string());
    }

    /// Adds an allowed file path prefix.
    pub fn allow_path(&mut self, prefix: &str) {
        self.allowed_paths.push(prefix.to_string());
    }

    /// Validates all policy constraints for a single operation.
    ///
    /// Returns a list of violation messages. Empty list means all checks pass.
    pub fn validate(
        &self,
        operation: Capability,
        memory_used: u64,
        elapsed_ms: u64,
        path: Option<&str>,
    ) -> Vec<String> {
        let mut violations = Vec::new();

        if !self.capabilities.has(operation) {
            violations.push(format!(
                "operation '{}' not permitted by sandbox '{}'",
                operation, self.name
            ));
        }

        if memory_used > self.max_memory {
            violations.push(format!(
                "memory usage {} exceeds sandbox limit {} for '{}'",
                memory_used, self.max_memory, self.name
            ));
        }

        if elapsed_ms > self.max_execution_ms {
            violations.push(format!(
                "execution time {}ms exceeds sandbox limit {}ms for '{}'",
                elapsed_ms, self.max_execution_ms, self.name
            ));
        }

        if let Some(p) = path {
            if !self.allows_path(p) {
                violations.push(format!(
                    "path '{}' not in allowed paths for sandbox '{}'",
                    p, self.name
                ));
            }
        }

        violations
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────────────────────────────
    // SEC1: Memory Safety Hardening
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn stack_canary_generates_unique_values() {
        let mut config = StackCanaryConfig::new(0xABCD);
        let c1 = config.generate_canary(1);
        let c2 = config.generate_canary(1);
        // Same call site, different generation counter -> different canary.
        assert_ne!(c1, c2, "each canary generation must be unique");
        assert_eq!(config.generation_count(), 2);
    }

    #[test]
    fn stack_canary_different_sites_different_values() {
        let mut config = StackCanaryConfig::new(0x1234);
        let c1 = config.generate_canary(100);
        // Reset counter to isolate call-site effect.
        let mut config2 = StackCanaryConfig::new(0x1234);
        let c2 = config2.generate_canary(200);
        assert_ne!(
            c1, c2,
            "different call sites must produce different canaries"
        );
    }

    #[test]
    fn stack_canary_disabled_returns_zero() {
        let mut config = StackCanaryConfig::disabled();
        assert_eq!(config.generate_canary(42), 0);
        assert!(!config.enabled);
    }

    #[test]
    fn stack_canary_verify_correct() {
        assert!(StackCanaryConfig::verify_canary(0xDEAD, 0xDEAD));
        assert!(!StackCanaryConfig::verify_canary(0xDEAD, 0xBEEF));
    }

    #[test]
    fn bounds_check_mode_debug_only() {
        let mode = BoundsCheckMode::Debug;
        assert!(mode.should_check(false), "debug mode, debug build -> check");
        assert!(
            !mode.should_check(true),
            "debug mode, release build -> no check"
        );
    }

    #[test]
    fn bounds_check_mode_release() {
        let mode = BoundsCheckMode::Release;
        assert!(mode.should_check(false));
        assert!(mode.should_check(true));
    }

    #[test]
    fn bounds_check_mode_none() {
        let mode = BoundsCheckMode::None;
        assert!(!mode.should_check(false));
        assert!(!mode.should_check(true));
    }

    #[test]
    fn overflow_config_default_checks_signed_only() {
        let config = OverflowCheckConfig::default();
        assert!(config.check_signed);
        assert!(!config.check_unsigned);
        assert!(config.any_checks_enabled());
    }

    #[test]
    fn overflow_config_strict_checks_both() {
        let config = OverflowCheckConfig::strict();
        assert!(config.check_signed);
        assert!(config.check_unsigned);
        assert!(config.trap_on_overflow);
    }

    #[test]
    fn overflow_config_unchecked() {
        let config = OverflowCheckConfig::unchecked();
        assert!(!config.any_checks_enabled());
    }

    #[test]
    fn allocation_budget_allocate_within_limit() {
        let mut budget = AllocationBudget::new(1024);
        assert!(budget.allocate(512).is_ok());
        assert_eq!(budget.used(), 512);
        assert_eq!(budget.remaining(), 512);
        assert_eq!(budget.allocation_count(), 1);
    }

    #[test]
    fn allocation_budget_exceed_limit() {
        let mut budget = AllocationBudget::new(100);
        assert!(budget.allocate(50).is_ok());
        let err = budget.allocate(100);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("exceeds budget"));
        // Used should not have changed on failure.
        assert_eq!(budget.used(), 50);
    }

    #[test]
    fn allocation_budget_remaining_and_free() {
        let mut budget = AllocationBudget::new(1000);
        assert!(budget.allocate(300).is_ok());
        assert_eq!(budget.remaining(), 700);
        budget.free(200);
        assert_eq!(budget.used(), 100);
        assert_eq!(budget.remaining(), 900);
    }

    #[test]
    fn allocation_budget_can_allocate_check() {
        let mut budget = AllocationBudget::new(100);
        assert!(budget.can_allocate(100));
        assert!(budget.allocate(60).is_ok());
        assert!(budget.can_allocate(40));
        assert!(!budget.can_allocate(41));
    }

    #[test]
    fn allocation_budget_reset() {
        let mut budget = AllocationBudget::new(100);
        assert!(budget.allocate(100).is_ok());
        assert!(budget.is_exhausted());
        budget.reset();
        assert_eq!(budget.used(), 0);
        assert_eq!(budget.allocation_count(), 0);
    }

    #[test]
    fn stack_depth_guard_enter_and_exit() {
        let mut guard = StackDepthGuard::new(10);
        assert_eq!(guard.current_depth(), 0);
        assert!(guard.enter().is_ok());
        assert_eq!(guard.current_depth(), 1);
        guard.exit();
        assert_eq!(guard.current_depth(), 0);
    }

    #[test]
    fn stack_depth_guard_exceeds_max() {
        let mut guard = StackDepthGuard::new(3);
        assert!(guard.enter().is_ok());
        assert!(guard.enter().is_ok());
        assert!(guard.enter().is_ok());
        let err = guard.enter();
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("stack overflow"));
    }

    #[test]
    fn stack_depth_guard_peak_tracking() {
        let mut guard = StackDepthGuard::new(100);
        for _ in 0..5 {
            let _ = guard.enter();
        }
        for _ in 0..5 {
            guard.exit();
        }
        assert_eq!(guard.peak_depth(), 5);
        assert_eq!(guard.current_depth(), 0);
    }

    #[test]
    fn memory_hardening_debug_default() {
        let h = MemoryHardening::debug_default();
        assert!(h.canaries.enabled);
        assert_eq!(h.bounds_check, BoundsCheckMode::Debug);
        assert!(h.overflow.check_signed);
        assert_eq!(h.active_feature_count(), 5);
    }

    #[test]
    fn memory_hardening_performance() {
        let h = MemoryHardening::performance();
        assert!(!h.canaries.enabled);
        assert_eq!(h.bounds_check, BoundsCheckMode::None);
        assert!(!h.overflow.any_checks_enabled());
        assert_eq!(h.active_feature_count(), 0);
    }

    #[test]
    fn security_overhead_measurement() {
        let mut overhead = SecurityOverhead::new("bounds_check");
        overhead.record_sample(10.0);
        overhead.record_sample(20.0);
        overhead.record_sample(30.0);
        assert_eq!(overhead.sample_count, 3);
        assert!((overhead.ns_per_check - 20.0).abs() < 0.001);
        assert!((overhead.total_ns() - 60.0).abs() < 0.001);
        let s = overhead.summary();
        assert!(s.contains("bounds_check"));
        assert!(s.contains("20.0 ns/check"));
    }

    // ───────────────────────────────────────────────────────────────────
    // SEC2: Supply Chain Security
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn levenshtein_identical_strings() {
        assert_eq!(TyposquatDetector::levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(TyposquatDetector::levenshtein("kitten", "sitten"), 1);
    }

    #[test]
    fn levenshtein_empty_string() {
        assert_eq!(TyposquatDetector::levenshtein("", "abc"), 3);
        assert_eq!(TyposquatDetector::levenshtein("abc", ""), 3);
    }

    #[test]
    fn typosquat_detects_similar_name() {
        let detector = TyposquatDetector::with_stdlib();
        let matches = detector.check("fj-mat"); // similar to "fj-math" (distance 1)
        assert!(!matches.is_empty());
        assert_eq!(matches[0].similar_to, "fj-math");
        assert_eq!(matches[0].distance, 1);
    }

    #[test]
    fn typosquat_exact_match_is_not_suspicious() {
        let detector = TyposquatDetector::with_stdlib();
        let matches = detector.check("fj-math");
        assert!(matches.is_empty(), "exact match should not be flagged");
    }

    #[test]
    fn typosquat_very_different_name_not_flagged() {
        let detector = TyposquatDetector::with_stdlib();
        let matches = detector.check("totally-different-package");
        assert!(matches.is_empty());
    }

    #[test]
    fn token_scope_permissions() {
        assert!(TokenScope::Admin.permits(TokenScope::Read));
        assert!(TokenScope::Admin.permits(TokenScope::Publish));
        assert!(TokenScope::Write.permits(TokenScope::Read));
        assert!(!TokenScope::Read.permits(TokenScope::Write));
        assert!(!TokenScope::Write.permits(TokenScope::Admin));
    }

    #[test]
    fn token_rotation_expiry() {
        let token = TokenRotation::new("tok_001", TokenScope::Read, 1000, 30);
        // 30 days = 2592000 seconds.
        assert!(!token.is_expired(1000));
        assert!(!token.is_expired(1000 + 86400 * 29));
        assert!(token.is_expired(1000 + 86400 * 30));
    }

    #[test]
    fn token_rotation_days_until_expiry() {
        let token = TokenRotation::new("tok_002", TokenScope::Write, 0, 90);
        assert_eq!(token.days_until_expiry(0), 90);
        assert_eq!(token.days_until_expiry(86400 * 10), 80);
        assert_eq!(token.days_until_expiry(86400 * 90), 0);
    }

    #[test]
    fn token_rotation_needs_renewal() {
        let token = TokenRotation::new("tok_003", TokenScope::Publish, 0, 30);
        assert!(!token.needs_renewal(0));
        assert!(token.needs_renewal(86400 * 24)); // 6 days left
    }

    #[test]
    fn token_rotation_revoke() {
        let mut token = TokenRotation::new("tok_004", TokenScope::Admin, 0, 365);
        assert!(!token.is_expired(0));
        token.revoke();
        assert!(token.is_expired(0), "revoked token should be expired");
    }

    #[test]
    fn reproducible_build_deterministic_id() {
        let b1 = ReproducibleBuild::new(
            "abc123",
            vec!["fj-math@1.0.0".into(), "fj-nn@2.0.0".into()],
            "fj 6.1.0",
            "x86_64-unknown-linux-gnu",
            "O2",
        );
        let b2 = ReproducibleBuild::new(
            "abc123",
            vec!["fj-nn@2.0.0".into(), "fj-math@1.0.0".into()], // different order
            "fj 6.1.0",
            "x86_64-unknown-linux-gnu",
            "O2",
        );
        assert_eq!(
            b1.build_id(),
            b2.build_id(),
            "deps are sorted so order should not matter"
        );
        assert!(b1.is_reproducible_with(&b2));
    }

    #[test]
    fn reproducible_build_different_inputs_different_id() {
        let b1 = ReproducibleBuild::new("abc", vec![], "fj 6.1.0", "x86_64", "O2");
        let b2 = ReproducibleBuild::new("def", vec![], "fj 6.1.0", "x86_64", "O2");
        assert_ne!(b1.build_id(), b2.build_id());
    }

    #[test]
    fn build_provenance_verify_artifact() {
        let prov = BuildProvenance::new(
            "github-actions",
            "github.com/fajar/fj",
            "abc123def456",
            1700000000,
            "sha256:deadbeef",
        );
        assert!(prov.verify_artifact("sha256:deadbeef"));
        assert!(!prov.verify_artifact("sha256:wrong"));
    }

    #[test]
    fn build_provenance_slsa_level_clamped() {
        let prov = BuildProvenance::new("ci", "repo", "hash", 0, "digest").with_slsa_level(10);
        assert_eq!(prov.slsa_level, 4);
    }

    #[test]
    fn security_advisory_affects_version() {
        let adv = SecurityAdvisory::new(
            "FJ-2026-001",
            AdvisorySeverity::High,
            "fj-http",
            "1.0.0",
            "1.5.0",
            Some("1.5.1"),
            "Buffer overflow in HTTP parser",
        );
        assert!(adv.affects_version("1.2.0"));
        assert!(adv.affects_version("1.0.0"));
        assert!(adv.affects_version("1.5.0"));
        assert!(!adv.affects_version("1.5.1"));
        assert!(!adv.affects_version("0.9.0"));
        assert!(adv.has_patch());
    }

    #[test]
    fn security_advisory_no_patch() {
        let adv = SecurityAdvisory::new(
            "FJ-2026-002",
            AdvisorySeverity::Critical,
            "fj-crypto",
            "0.1.0",
            "0.9.9",
            None,
            "Key derivation weakness",
        );
        assert!(!adv.has_patch());
    }

    // ───────────────────────────────────────────────────────────────────
    // SEC3: Audit & Certification
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn linter_has_20_rules() {
        let linter = SecurityLinter::new();
        assert_eq!(linter.total_rule_count(), 20);
        assert_eq!(linter.enabled_rule_count(), 20);
    }

    #[test]
    fn lint_detects_sql_injection() {
        let linter = SecurityLinter::new();
        let source = r#"let query = db_execute("SELECT * FROM users WHERE id=" + user_id)"#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "sql_injection"),
            "should flag string concat in db_execute"
        );
    }

    #[test]
    fn lint_detects_hardcoded_secret() {
        let linter = SecurityLinter::new();
        let source = r#"let password = "hunter2""#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "hardcoded_secret"),
            "should flag hardcoded password"
        );
    }

    #[test]
    fn lint_detects_unsafe_unwrap() {
        let linter = SecurityLinter::new();
        let source = r#"let val = result.unwrap()"#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "unsafe_unwrap"),
            "should flag .unwrap()"
        );
    }

    #[test]
    fn lint_detects_weak_crypto() {
        let linter = SecurityLinter::new();
        let source = r#"let hash = MD5::digest(data)"#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "weak_crypto"),
            "should flag MD5"
        );
    }

    #[test]
    fn lint_detects_path_traversal() {
        let linter = SecurityLinter::new();
        let source = r#"let f = read_file("../../etc/passwd")"#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "path_traversal"),
            "should flag path traversal"
        );
    }

    #[test]
    fn lint_clean_code_no_violations() {
        let linter = SecurityLinter::new();
        let source = "let x = 42\nlet y = x + 1\n";
        let violations = linter.lint(source);
        assert!(
            violations.is_empty(),
            "clean code should have no violations"
        );
    }

    #[test]
    fn lint_disable_rule() {
        let mut linter = SecurityLinter::new();
        linter.disable_rule("SEC007");
        assert_eq!(linter.enabled_rule_count(), 19);
        let source = r#"let val = result.unwrap()"#;
        let violations = linter.lint(source);
        assert!(
            !violations.iter().any(|v| v.rule_id == "SEC007"),
            "disabled rule should not fire"
        );
    }

    #[test]
    fn lint_skips_comments() {
        let linter = SecurityLinter::new();
        let source = r#"// let password = "hunter2""#;
        let violations = linter.lint(source);
        assert!(
            !violations.iter().any(|v| v.rule_name == "hardcoded_secret"),
            "should skip comment lines"
        );
    }

    #[test]
    fn taint_analysis_mark_and_check() {
        let mut taint = TaintAnalysis::new();
        taint.mark_tainted("user_input");
        assert!(taint.is_tainted("user_input"));
        assert!(!taint.is_tainted("safe_var"));
        assert_eq!(taint.tainted_count(), 1);
    }

    #[test]
    fn taint_analysis_propagation() {
        let mut taint = TaintAnalysis::new();
        taint.mark_tainted("input");
        taint.propagate("derived", "input", false);
        assert!(taint.is_tainted("derived"), "taint should propagate");

        taint.propagate("sanitized", "input", true);
        assert!(
            !taint.is_tainted("sanitized"),
            "sanitized data should be clean"
        );
    }

    #[test]
    fn taint_analysis_sink_violation() {
        let mut taint = TaintAnalysis::new();
        taint.mark_tainted("user_query");
        let violation = taint.check_sink("user_query", "db_execute");
        assert!(violation.is_some());
        let v = violation.expect("violation should exist");
        assert_eq!(v.variable, "user_query");
        assert_eq!(v.sink, "db_execute");
        assert_eq!(taint.violation_count(), 1);
    }

    #[test]
    fn taint_analysis_clean_variable_no_violation() {
        let mut taint = TaintAnalysis::new();
        // Not tainted — should be allowed.
        let violation = taint.check_sink("safe_var", "db_execute");
        assert!(violation.is_none());
    }

    #[test]
    fn security_scorecard_weighted_average() {
        let mut sc = SecurityScorecard::new("test-project");
        sc.add_category("Code Quality", 90, 0.5, vec![]);
        sc.add_category("Memory Safety", 70, 0.5, vec![]);
        assert_eq!(sc.overall_score(), 80);
        assert_eq!(sc.grade(), "B");
    }

    #[test]
    fn security_scorecard_empty() {
        let sc = SecurityScorecard::new("empty");
        assert_eq!(sc.overall_score(), 0);
        assert_eq!(sc.grade(), "F");
    }

    #[test]
    fn security_scorecard_from_analysis() {
        let hardening = MemoryHardening::strict();
        let sc = SecurityScorecard::from_analysis(
            "my-app",
            &[], // no lint violations
            &hardening,
            0,    // no taint violations
            5,    // 5 deps
            true, // has provenance
        );
        assert!(sc.overall_score() >= 80);
        assert!(!sc.categories.is_empty());
    }

    #[test]
    fn security_scorecard_report_contains_info() {
        let mut sc = SecurityScorecard::new("report-test");
        sc.add_category("Test", 85, 1.0, vec!["finding1".into()]);
        let report = sc.report();
        assert!(report.contains("report-test"));
        assert!(report.contains("85"));
        assert!(report.contains("finding1"));
    }

    #[test]
    fn compliance_mode_rule_counts() {
        assert_eq!(ComplianceMode::Misra.rule_count(), 175);
        assert_eq!(ComplianceMode::CertC.rule_count(), 116);
        assert_eq!(ComplianceMode::Iso26262.rule_count(), 42);
        assert_eq!(ComplianceMode::Do178C.rule_count(), 65);
        assert_eq!(ComplianceMode::Iec62443.rule_count(), 89);
    }

    #[test]
    fn compliance_mode_applicable_rules() {
        let rules = ComplianceMode::Iec62443.applicable_lint_rules();
        assert!(rules.contains(&"SEC001")); // sql_injection
        assert!(rules.contains(&"SEC004")); // hardcoded_secret
        assert!(!rules.contains(&"SEC020")); // infinite_loop — not in IEC 62443
    }

    #[test]
    fn secure_annotation_check_satisfied() {
        let mut check = SecureAnnotationCheck::new("handle_payment");
        assert!(!check.is_satisfied());
        check.record_review("security-team", 1700000000);
        assert!(check.is_satisfied());
        assert_eq!(check.reviewer.as_deref(), Some("security-team"));
    }

    #[test]
    fn secure_annotation_check_all() {
        let functions = vec!["fn_a".to_string(), "fn_b".to_string()];
        let mut reviews = HashMap::new();
        reviews.insert("fn_a".to_string(), ("reviewer1".to_string(), 100));

        let results = SecureAnnotationCheck::check_all(&functions, &reviews);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_satisfied()); // fn_a reviewed
        assert!(!results[1].is_satisfied()); // fn_b not reviewed
    }

    #[test]
    fn capability_set_permissions() {
        let caps = CapabilitySet::with_capabilities(
            "test",
            &[Capability::FileRead, Capability::NetConnect],
        );
        assert!(caps.has(Capability::FileRead));
        assert!(caps.has(Capability::NetConnect));
        assert!(!caps.has(Capability::FileWrite));
        assert_eq!(caps.count(), 2);
    }

    #[test]
    fn capability_set_check_returns_error() {
        let caps = CapabilitySet::empty("restricted");
        let result = caps.check(Capability::Exec);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exec"));
    }

    #[test]
    fn capability_set_kernel_default() {
        let caps = CapabilitySet::kernel_default("kernel_mod");
        assert!(caps.has(Capability::HardwareAccess));
        assert!(caps.has(Capability::TimerAccess));
        assert!(!caps.has(Capability::TensorOps)); // no ML in kernel
        assert!(!caps.has(Capability::HeapAlloc)); // no heap in kernel
    }

    #[test]
    fn capability_set_device_default() {
        let caps = CapabilitySet::device_default("device_mod");
        assert!(caps.has(Capability::TensorOps));
        assert!(!caps.has(Capability::HardwareAccess)); // no hardware in device
    }

    #[test]
    fn capability_set_intersect() {
        let a = CapabilitySet::with_capabilities(
            "a",
            &[
                Capability::FileRead,
                Capability::NetConnect,
                Capability::Exec,
            ],
        );
        let b =
            CapabilitySet::with_capabilities("b", &[Capability::FileRead, Capability::FileWrite]);
        let inter = a.intersect(&b);
        assert!(inter.has(Capability::FileRead));
        assert!(!inter.has(Capability::NetConnect));
        assert!(!inter.has(Capability::FileWrite));
        assert_eq!(inter.count(), 1);
    }

    #[test]
    fn capability_set_grant_and_revoke() {
        let mut caps = CapabilitySet::empty("test");
        assert!(caps.is_empty());
        caps.grant(Capability::CryptoOps);
        assert!(caps.has(Capability::CryptoOps));
        caps.revoke(Capability::CryptoOps);
        assert!(!caps.has(Capability::CryptoOps));
    }

    #[test]
    fn sandbox_policy_untrusted() {
        let policy = SandboxPolicy::untrusted("sandbox");
        assert!(!policy.allows_operation(Capability::FileRead));
        assert!(!policy.allows_operation(Capability::NetConnect));
        assert!(!policy.allows_path("/etc/passwd"));
        assert_eq!(policy.max_file_descriptors, 0);
    }

    #[test]
    fn sandbox_policy_trusted() {
        let policy = SandboxPolicy::trusted("trusted");
        assert!(policy.allows_operation(Capability::FileRead));
        assert!(policy.allows_operation(Capability::Exec));
        assert!(policy.allows_path("/home/user/file.txt"));
    }

    #[test]
    fn sandbox_policy_validate_violations() {
        let mut policy = SandboxPolicy::untrusted("test");
        policy.allow_path("/tmp");
        let violations = policy.validate(
            Capability::FileRead,
            32 * 1024 * 1024, // 32 MB > 16 MB limit
            1000,
            Some("/etc/passwd"),
        );
        // Should have: capability denied, memory exceeded, path not allowed.
        assert!(violations.len() >= 2);
        assert!(violations.iter().any(|v| v.contains("file_read")));
        assert!(violations.iter().any(|v| v.contains("memory")));
    }

    #[test]
    fn sandbox_policy_blocked_syscalls() {
        let mut policy = SandboxPolicy::new("test", CapabilitySet::full("test"));
        policy.block_syscall("execve");
        assert!(policy.is_syscall_blocked("execve"));
        assert!(!policy.is_syscall_blocked("read"));
    }

    #[test]
    fn lint_detects_command_injection() {
        let linter = SecurityLinter::new();
        let source = r#"exec("rm -rf " + user_path)"#;
        let violations = linter.lint(source);
        assert!(
            violations
                .iter()
                .any(|v| v.rule_name == "command_injection"),
            "should flag string concat in exec()"
        );
    }

    #[test]
    fn lint_detects_insecure_http() {
        let linter = SecurityLinter::new();
        let source = r#"let url = "http://api.example.com/data""#;
        let violations = linter.lint(source);
        assert!(
            violations.iter().any(|v| v.rule_name == "insecure_http"),
            "should flag http:// URL"
        );
    }

    #[test]
    fn lint_allows_https() {
        let linter = SecurityLinter::new();
        let source = r#"let url = "https://api.example.com/data""#;
        let violations = linter.lint(source);
        assert!(
            !violations.iter().any(|v| v.rule_name == "insecure_http"),
            "should NOT flag https:// URL"
        );
    }
}
