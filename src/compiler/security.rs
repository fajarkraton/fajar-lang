//! # Security Hardening — Phase 5 (v0.9, Sprints 17–20)
//!
//! Provides compile-time and runtime security hardening for Fajar Lang binaries:
//!
//! - **Sprint 17** — Stack protection: canaries, shadow stack, stack clash probes, guards
//! - **Sprint 18** — Control flow integrity: forward/backward-edge CFI, vtable guards
//! - **Sprint 19** — Memory safety runtime: ASan, MSan, leak detection, quarantine
//! - **Sprint 20** — Secure compilation: PIC, RELRO, NX, fortify, audit, hardening score
//!
//! All implementations are simulation-level: they model the security mechanisms
//! in-process without requiring OS kernel cooperation or hardware support.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// SecurityError
// ═══════════════════════════════════════════════════════════════════════

/// Errors raised by security hardening checks.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SecurityError {
    /// Stack canary value was corrupted (buffer overflow detected).
    #[error("canary violation in `{fn_name}`: expected {expected:#x}, got {actual:#x}")]
    CanaryViolation {
        /// Function where the violation occurred.
        fn_name: String,
        /// Expected canary value.
        expected: u64,
        /// Actual canary value found on the stack.
        actual: u64,
    },

    /// Stack allocation exceeded the configured guard size.
    #[error("stack overflow: depth {depth} exceeds limit {limit}")]
    StackOverflow {
        /// Current stack depth in bytes.
        depth: usize,
        /// Configured maximum stack size.
        limit: usize,
    },

    /// Large stack allocation without proper guard page probing.
    #[error("stack clash: allocation of {alloc_size} bytes requires {probe_count} probes")]
    StackClash {
        /// Requested allocation size in bytes.
        alloc_size: usize,
        /// Number of guard page probes required.
        probe_count: usize,
    },

    /// Indirect call target does not match expected type hash.
    #[error("CFI violation: target hash {target_hash:#x} != expected {expected_hash:#x}")]
    CfiViolation {
        /// Hash of the actual call target.
        target_hash: u64,
        /// Hash of the expected function type.
        expected_hash: u64,
    },

    /// Address sanitizer detected an invalid memory access.
    #[error("ASan: {kind} at address {addr:#x} (size {size})")]
    AsanViolation {
        /// Kind of violation (e.g., "use-after-free", "buffer-overflow").
        kind: String,
        /// Memory address involved.
        addr: u64,
        /// Access size in bytes.
        size: usize,
    },

    /// Generic undefined behavior detected.
    #[error("undefined behavior: {description}")]
    UndefinedBehavior {
        /// Description of the undefined behavior.
        description: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// SecurityConfig
// ═══════════════════════════════════════════════════════════════════════

/// Master configuration for all security hardening features.
///
/// Each field toggles a specific protection mechanism. Use
/// [`SecurityConfig::all()`] to enable everything, or
/// [`SecurityConfig::release_defaults()`] for production-recommended settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityConfig {
    /// Insert stack canaries in function prologues/epilogues.
    pub enable_canaries: bool,
    /// Maintain a shadow stack for return address integrity.
    pub enable_shadow_stack: bool,
    /// Probe guard pages for large stack allocations.
    pub enable_stack_clash: bool,
    /// Enable address sanitizer instrumentation.
    pub enable_asan: bool,
    /// Enable control flow integrity checks.
    pub enable_cfi: bool,
    /// Generate position-independent code.
    pub enable_pic: bool,
    /// Mark relocation sections as read-only after init.
    pub enable_relro: bool,
    /// Mark stack pages as non-executable.
    pub enable_nx: bool,
}

impl SecurityConfig {
    /// Returns a configuration with all features disabled.
    pub fn none() -> Self {
        Self {
            enable_canaries: false,
            enable_shadow_stack: false,
            enable_stack_clash: false,
            enable_asan: false,
            enable_cfi: false,
            enable_pic: false,
            enable_relro: false,
            enable_nx: false,
        }
    }

    /// Returns a configuration with every feature enabled (`-fharden`).
    pub fn all() -> Self {
        Self {
            enable_canaries: true,
            enable_shadow_stack: true,
            enable_stack_clash: true,
            enable_asan: true,
            enable_cfi: true,
            enable_pic: true,
            enable_relro: true,
            enable_nx: true,
        }
    }

    /// Sane defaults for release builds: canaries, CFI, PIC, RELRO, NX.
    ///
    /// ASan and shadow stack are disabled by default in release mode because
    /// they impose measurable runtime overhead.
    pub fn release_defaults() -> Self {
        Self {
            enable_canaries: true,
            enable_shadow_stack: false,
            enable_stack_clash: true,
            enable_asan: false,
            enable_cfi: true,
            enable_pic: true,
            enable_relro: true,
            enable_nx: true,
        }
    }

    /// Returns the number of features currently enabled.
    pub fn enabled_count(&self) -> usize {
        [
            self.enable_canaries,
            self.enable_shadow_stack,
            self.enable_stack_clash,
            self.enable_asan,
            self.enable_cfi,
            self.enable_pic,
            self.enable_relro,
            self.enable_nx,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::none()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 17 — Stack Protection
// ═══════════════════════════════════════════════════════════════════════

/// Generates per-function stack canary values using a deterministic seed.
///
/// In a real compiler these would come from `/dev/urandom` or similar; here
/// we use FNV-1a over the function name combined with a configurable seed.
#[derive(Debug, Clone)]
pub struct CanaryGenerator {
    /// Base seed mixed with function names to produce canaries.
    seed: u64,
}

impl CanaryGenerator {
    /// Creates a new generator with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Produces a deterministic canary value for `fn_name`.
    pub fn generate_canary(&self, fn_name: &str) -> u64 {
        let mut hash = self.seed ^ 0xcbf29ce484222325;
        for byte in fn_name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x00000100000001B3);
        }
        // Ensure the canary contains a null byte (classic defence)
        hash & 0xffffff00ffffff00
    }
}

/// Verifies that a stack canary has not been corrupted.
///
/// Returns `Ok(())` if `actual == expected`, or a [`SecurityError::CanaryViolation`].
pub fn check_canary(fn_name: &str, expected: u64, actual: u64) -> Result<(), SecurityError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SecurityError::CanaryViolation {
            fn_name: fn_name.to_string(),
            expected,
            actual,
        })
    }
}

// ─── Stack Clash Protection ──────────────────────────────────────────

/// A single probe point inserted to touch a guard page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbePoint {
    /// Byte offset from the stack pointer where the probe occurs.
    pub offset: usize,
}

/// Stack clash protection: inserts probe points for large allocations.
///
/// When a function's frame exceeds `page_size` bytes the compiler must
/// touch each guard page sequentially to avoid jumping over it.
#[derive(Debug, Clone)]
pub struct StackClashProtector {
    /// Guard page size (typically 4096 bytes).
    page_size: usize,
}

impl StackClashProtector {
    /// Creates a protector with the given page size.
    pub fn new(page_size: usize) -> Self {
        Self { page_size }
    }

    /// Returns the list of probe offsets required for `alloc_size`.
    ///
    /// One probe is emitted per guard page that the allocation spans.
    pub fn probe_stack(&self, alloc_size: usize) -> Vec<ProbePoint> {
        if self.page_size == 0 || alloc_size <= self.page_size {
            return Vec::new();
        }
        let count = alloc_size / self.page_size;
        (1..=count)
            .map(|i| ProbePoint {
                offset: i * self.page_size,
            })
            .collect()
    }
}

// ─── Shadow Stack ────────────────────────────────────────────────────

/// Shadow stack for return address integrity (backward-edge CFI).
///
/// Maintains a separate stack of return addresses that is compared against
/// the real stack on function return to detect ROP-style overwrites.
#[derive(Debug, Clone)]
pub struct ShadowStack {
    /// The stack of return addresses (most recent last).
    entries: Vec<u64>,
}

impl ShadowStack {
    /// Creates an empty shadow stack.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Pushes a return address onto the shadow stack.
    pub fn push_return(&mut self, addr: u64) {
        self.entries.push(addr);
    }

    /// Pops the most recent return address.
    pub fn pop_return(&mut self) -> Option<u64> {
        self.entries.pop()
    }

    /// Verifies that `addr` matches the top of the shadow stack.
    ///
    /// Returns `true` if the shadow stack is empty (nothing to check)
    /// or the top entry matches.
    pub fn verify_return(&self, addr: u64) -> bool {
        match self.entries.last() {
            Some(&top) => top == addr,
            None => true,
        }
    }

    /// Returns the current depth of the shadow stack.
    pub fn depth(&self) -> usize {
        self.entries.len()
    }
}

impl Default for ShadowStack {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Stack Depth Analysis ────────────────────────────────────────────

/// Analyzes the maximum stack depth for a function given a call graph.
///
/// `call_graph` maps each function name to the list of functions it calls.
/// `frame_sizes` maps each function name to its stack frame size in bytes.
///
/// Returns the worst-case stack depth starting from `fn_name`, or `0` if
/// the function is unknown.
pub fn analyze_stack_depth(
    fn_name: &str,
    call_graph: &HashMap<String, Vec<String>>,
    frame_sizes: &HashMap<String, usize>,
) -> usize {
    let mut visited = Vec::new();
    analyze_depth_recursive(fn_name, call_graph, frame_sizes, &mut visited)
}

/// Recursive helper that tracks visited functions to avoid cycles.
fn analyze_depth_recursive(
    fn_name: &str,
    call_graph: &HashMap<String, Vec<String>>,
    frame_sizes: &HashMap<String, usize>,
    visited: &mut Vec<String>,
) -> usize {
    if visited.contains(&fn_name.to_string()) {
        return 0; // cycle — stop recursion
    }
    let own_size = frame_sizes.get(fn_name).copied().unwrap_or(0);
    visited.push(fn_name.to_string());
    let max_callee = call_graph
        .get(fn_name)
        .map(|callees| {
            callees
                .iter()
                .map(|c| analyze_depth_recursive(c, call_graph, frame_sizes, visited))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    visited.pop();
    own_size + max_callee
}

// ─── Stack Guard ─────────────────────────────────────────────────────

/// Configurable stack overflow guard.
///
/// Tracks current usage against a configured maximum and raises
/// [`SecurityError::StackOverflow`] when exceeded.
#[derive(Debug, Clone)]
pub struct StackGuard {
    /// Maximum allowed stack usage in bytes.
    limit: usize,
    /// Current stack usage in bytes.
    current: usize,
}

impl StackGuard {
    /// Creates a guard with the given byte limit.
    pub fn new(limit: usize) -> Self {
        Self { limit, current: 0 }
    }

    /// Records `size` additional bytes of stack usage.
    pub fn push(&mut self, size: usize) -> Result<(), SecurityError> {
        let new = self.current.saturating_add(size);
        if new > self.limit {
            return Err(SecurityError::StackOverflow {
                depth: new,
                limit: self.limit,
            });
        }
        self.current = new;
        Ok(())
    }

    /// Releases `size` bytes of stack usage.
    pub fn pop(&mut self, size: usize) {
        self.current = self.current.saturating_sub(size);
    }

    /// Returns the current stack usage.
    pub fn current_usage(&self) -> usize {
        self.current
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 18 — Control Flow Integrity
// ═══════════════════════════════════════════════════════════════════════

/// Metadata attached to each function for CFI validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiMetadata {
    /// Deterministic hash of the function's type signature.
    pub type_hash: u64,
    /// The function's source name.
    pub fn_name: String,
    /// Parameter type names, in order.
    pub param_types: Vec<String>,
    /// Return type name.
    pub return_type: String,
}

/// Computes a deterministic type hash from a function signature string.
///
/// The signature is built as `(param1,param2,...)->ret` and hashed with
/// FNV-1a to produce a stable u64.
pub fn compute_type_hash(param_types: &[String], return_type: &str) -> u64 {
    let sig = format!("({})->{}", param_types.join(","), return_type);
    fnv1a(&sig)
}

/// FNV-1a hash used throughout the security module.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    hash
}

/// Validates an indirect call by comparing target and expected type hashes.
pub fn validate_indirect_call(target_hash: u64, expected_hash: u64) -> Result<(), SecurityError> {
    if target_hash == expected_hash {
        Ok(())
    } else {
        Err(SecurityError::CfiViolation {
            target_hash,
            expected_hash,
        })
    }
}

// ─── CFI Registry ────────────────────────────────────────────────────

/// Registry of all valid indirect-call targets in a compilation unit.
///
/// Every address-taken function is registered here; indirect calls are
/// validated against this set at call time.
#[derive(Debug, Clone)]
pub struct CfiRegistry {
    /// Maps function address (simulated) to its CFI metadata.
    entries: HashMap<u64, CfiMetadata>,
}

impl CfiRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Registers a function as a valid indirect-call target.
    pub fn register(&mut self, addr: u64, meta: CfiMetadata) {
        self.entries.insert(addr, meta);
    }

    /// Validates that `target_addr` is a registered call target with
    /// the expected type hash.
    pub fn validate(&self, target_addr: u64, expected_hash: u64) -> Result<(), SecurityError> {
        match self.entries.get(&target_addr) {
            Some(meta) => validate_indirect_call(meta.type_hash, expected_hash),
            None => Err(SecurityError::CfiViolation {
                target_hash: 0,
                expected_hash,
            }),
        }
    }

    /// Returns the number of registered call targets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no targets are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CfiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates a raw function pointer against a set of known entry points.
pub fn validate_fn_ptr(ptr: u64, known_entries: &[u64]) -> Result<(), SecurityError> {
    if known_entries.contains(&ptr) {
        Ok(())
    } else {
        Err(SecurityError::CfiViolation {
            target_hash: ptr,
            expected_hash: 0,
        })
    }
}

// ─── VTable Guard ────────────────────────────────────────────────────

/// Hash-protected virtual dispatch table.
///
/// Stores a hash over vtable contents and a read-only flag to detect
/// corruption at dispatch time.
#[derive(Debug, Clone)]
pub struct VTableGuard {
    /// Hash of the original vtable contents.
    hash: u64,
    /// The vtable entries (simulated as addresses).
    entries: Vec<u64>,
    /// Whether the vtable has been sealed (read-only).
    read_only: bool,
}

impl VTableGuard {
    /// Creates a new vtable guard over the given entries.
    pub fn new(entries: Vec<u64>) -> Self {
        let hash = Self::compute_hash(&entries);
        Self {
            hash,
            entries,
            read_only: false,
        }
    }

    /// Seals the vtable, preventing further modification.
    pub fn seal(&mut self) {
        self.read_only = true;
    }

    /// Verifies that the vtable has not been tampered with.
    pub fn verify(&self) -> Result<(), SecurityError> {
        let current = Self::compute_hash(&self.entries);
        if current == self.hash {
            Ok(())
        } else {
            Err(SecurityError::CfiViolation {
                target_hash: current,
                expected_hash: self.hash,
            })
        }
    }

    /// Returns the entry at `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<u64> {
        self.entries.get(index).copied()
    }

    /// Computes FNV-1a over a slice of u64 entries.
    fn compute_hash(entries: &[u64]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &entry in entries {
            for byte in entry.to_le_bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x00000100000001B3);
            }
        }
        hash
    }
}

/// Verifies a vtable pointer against its expected hash.
pub fn verify_vtable(vtable: &VTableGuard, expected_hash: u64) -> Result<(), SecurityError> {
    let current = VTableGuard::compute_hash(&vtable.entries);
    if current == expected_hash {
        Ok(())
    } else {
        Err(SecurityError::CfiViolation {
            target_hash: current,
            expected_hash,
        })
    }
}

// ─── Jump Table Guard ────────────────────────────────────────────────

/// Bounds-checked jump table for switch/match dispatch.
#[derive(Debug, Clone)]
pub struct JumpTableGuard {
    /// Number of valid entries in the jump table.
    size: usize,
}

impl JumpTableGuard {
    /// Creates a guard for a jump table with `size` entries.
    pub fn new(size: usize) -> Self {
        Self { size }
    }

    /// Validates that `index` is within the jump table bounds.
    pub fn check_index(&self, index: usize) -> Result<(), SecurityError> {
        if index < self.size {
            Ok(())
        } else {
            Err(SecurityError::UndefinedBehavior {
                description: format!(
                    "jump table index {index} out of bounds (size {size})",
                    size = self.size
                ),
            })
        }
    }
}

// ─── Function Diversifier (ROP Mitigation) ───────────────────────────

/// Simulates function prologue diversification to hinder ROP gadget chains.
///
/// Inserts a deterministic number of NOP-equivalent instructions at the
/// start of each function based on a seed, making gadget offsets
/// unpredictable across builds.
#[derive(Debug, Clone)]
pub struct FunctionDiversifier {
    /// Seed for the diversification PRNG.
    seed: u64,
}

impl FunctionDiversifier {
    /// Creates a diversifier with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Returns the number of NOP-equivalent instructions to prepend.
    ///
    /// Range: 0..=15 NOPs, deterministic per function name.
    pub fn nop_count(&self, fn_name: &str) -> usize {
        let hash = fnv1a(fn_name) ^ self.seed;
        (hash % 16) as usize
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 19 — Memory Safety Runtime
// ═══════════════════════════════════════════════════════════════════════

/// State of a memory region tracked by shadow memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemState {
    /// Address has never been allocated.
    Unallocated,
    /// Address belongs to a live allocation of `size` bytes.
    Allocated(usize),
    /// Address was freed — access is use-after-free.
    Freed,
    /// Address is a red-zone (padding around allocations).
    Poisoned,
    /// Address was allocated but never written — reads are undefined.
    Uninitialized,
}

/// Shadow memory: per-address allocation state tracking.
#[derive(Debug, Clone)]
pub struct ShadowMemory {
    /// Maps simulated addresses to their state.
    state: HashMap<u64, MemState>,
}

impl ShadowMemory {
    /// Creates an empty shadow memory.
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }

    /// Sets the state of a single address.
    pub fn set(&mut self, addr: u64, state: MemState) {
        self.state.insert(addr, state);
    }

    /// Returns the state of an address, defaulting to `Unallocated`.
    pub fn get(&self, addr: u64) -> MemState {
        self.state
            .get(&addr)
            .copied()
            .unwrap_or(MemState::Unallocated)
    }

    /// Returns the number of tracked addresses.
    pub fn tracked_count(&self) -> usize {
        self.state.len()
    }
}

impl Default for ShadowMemory {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Red Zone ────────────────────────────────────────────────────────

/// Describes the red-zone padding around an allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedZone {
    /// Address of the start of the left red zone.
    pub left_start: u64,
    /// Size of each red zone in bytes.
    pub zone_size: usize,
    /// Address of the start of the right red zone.
    pub right_start: u64,
}

// ─── Address Sanitizer ───────────────────────────────────────────────

/// Simulated address sanitizer.
///
/// Tracks all allocations with red zones and detects use-after-free,
/// buffer overflow, and double-free violations.
#[derive(Debug, Clone)]
pub struct AddressSanitizer {
    /// Shadow memory backing the sanitizer.
    shadow: ShadowMemory,
    /// Red zone size in bytes (default 16).
    redzone_size: usize,
    /// Next simulated allocation address.
    next_addr: u64,
    /// Quarantine for recently-freed addresses.
    quarantine: Quarantine,
}

impl AddressSanitizer {
    /// Creates a new sanitizer with the given red zone size.
    pub fn new(redzone_size: usize) -> Self {
        Self {
            shadow: ShadowMemory::new(),
            redzone_size,
            next_addr: 0x1000,
            quarantine: Quarantine::new(64),
        }
    }

    /// Allocates `size` bytes with red zones on both sides.
    ///
    /// Returns the usable address and the red-zone descriptor.
    pub fn asan_alloc(&mut self, size: usize) -> (u64, RedZone) {
        let left_start = self.next_addr;
        let usable_start = left_start + self.redzone_size as u64;
        let right_start = usable_start + size as u64;

        // Poison left red zone
        for i in 0..self.redzone_size {
            self.shadow.set(left_start + i as u64, MemState::Poisoned);
        }
        // Mark usable region as uninitialized
        for i in 0..size {
            self.shadow
                .set(usable_start + i as u64, MemState::Uninitialized);
        }
        // Poison right red zone
        for i in 0..self.redzone_size {
            self.shadow.set(right_start + i as u64, MemState::Poisoned);
        }

        self.next_addr = right_start + self.redzone_size as u64;

        let rz = RedZone {
            left_start,
            zone_size: self.redzone_size,
            right_start,
        };
        (usable_start, rz)
    }

    /// Frees a previously allocated address, moving it to quarantine.
    pub fn asan_free(&mut self, addr: u64) -> Result<(), SecurityError> {
        match self.shadow.get(addr) {
            MemState::Allocated(_) | MemState::Uninitialized => {
                self.mark_freed(addr);
                self.quarantine.push(addr);
                Ok(())
            }
            MemState::Freed => Err(SecurityError::AsanViolation {
                kind: "double-free".to_string(),
                addr,
                size: 0,
            }),
            _ => Err(SecurityError::AsanViolation {
                kind: "invalid-free".to_string(),
                addr,
                size: 0,
            }),
        }
    }

    /// Checks whether accessing `size` bytes at `addr` is valid.
    pub fn asan_check_access(&self, addr: u64, size: usize) -> Result<(), SecurityError> {
        for i in 0..size {
            let a = addr + i as u64;
            match self.shadow.get(a) {
                MemState::Allocated(_) => {}
                MemState::Uninitialized => {} // reads caught by MSan
                MemState::Freed => {
                    return Err(SecurityError::AsanViolation {
                        kind: "use-after-free".to_string(),
                        addr: a,
                        size,
                    });
                }
                MemState::Poisoned => {
                    return Err(SecurityError::AsanViolation {
                        kind: "buffer-overflow".to_string(),
                        addr: a,
                        size,
                    });
                }
                MemState::Unallocated => {
                    return Err(SecurityError::AsanViolation {
                        kind: "wild-access".to_string(),
                        addr: a,
                        size,
                    });
                }
            }
        }
        Ok(())
    }

    /// Marks a single address as freed in shadow memory.
    fn mark_freed(&mut self, addr: u64) {
        self.shadow.set(addr, MemState::Freed);
    }

    /// Marks an address as fully written (transitions Uninitialized -> Allocated).
    pub fn mark_written(&mut self, addr: u64, size: usize) {
        for i in 0..size {
            self.shadow.set(addr + i as u64, MemState::Allocated(size));
        }
    }
}

// ─── Quarantine ──────────────────────────────────────────────────────

/// Delays reuse of freed memory to increase use-after-free detection.
///
/// Freed addresses sit in the quarantine buffer until it reaches
/// capacity, at which point the oldest entries are evicted.
#[derive(Debug, Clone)]
pub struct Quarantine {
    /// Ring buffer of quarantined addresses.
    entries: Vec<u64>,
    /// Maximum number of addresses to hold.
    capacity: usize,
}

impl Quarantine {
    /// Creates a quarantine with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Adds an address to quarantine, evicting the oldest if full.
    pub fn push(&mut self, addr: u64) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(addr);
    }

    /// Returns `true` if `addr` is currently quarantined.
    pub fn contains(&self, addr: u64) -> bool {
        self.entries.contains(&addr)
    }

    /// Returns the number of addresses in quarantine.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the quarantine is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─── Memory Sanitizer ────────────────────────────────────────────────

/// Simulated memory sanitizer: detects reads of uninitialized memory.
#[derive(Debug, Clone)]
pub struct MemorySanitizer {
    /// Shadow memory tracking initialization state.
    shadow: ShadowMemory,
}

impl MemorySanitizer {
    /// Creates a new MSan instance.
    pub fn new() -> Self {
        Self {
            shadow: ShadowMemory::new(),
        }
    }

    /// Records that `size` bytes at `addr` have been allocated (uninitialized).
    pub fn msan_alloc(&mut self, addr: u64, size: usize) {
        for i in 0..size {
            self.shadow.set(addr + i as u64, MemState::Uninitialized);
        }
    }

    /// Records that `size` bytes at `addr` have been written (initialized).
    pub fn msan_write(&mut self, addr: u64, size: usize) {
        for i in 0..size {
            self.shadow.set(addr + i as u64, MemState::Allocated(size));
        }
    }

    /// Checks that `size` bytes at `addr` are initialized before reading.
    pub fn msan_check_read(&self, addr: u64, size: usize) -> Result<(), SecurityError> {
        for i in 0..size {
            let a = addr + i as u64;
            if self.shadow.get(a) == MemState::Uninitialized {
                return Err(SecurityError::AsanViolation {
                    kind: "use-of-uninitialized".to_string(),
                    addr: a,
                    size,
                });
            }
        }
        Ok(())
    }
}

impl Default for MemorySanitizer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Leak Detector ───────────────────────────────────────────────────

/// Reports heap allocations that were never freed.
#[derive(Debug, Clone)]
pub struct LeakDetector {
    /// Set of addresses that are currently allocated.
    live: HashMap<u64, usize>,
}

impl LeakDetector {
    /// Creates a new leak detector.
    pub fn new() -> Self {
        Self {
            live: HashMap::new(),
        }
    }

    /// Records that `size` bytes were allocated at `addr`.
    pub fn alloc(&mut self, addr: u64, size: usize) {
        self.live.insert(addr, size);
    }

    /// Records that the allocation at `addr` was freed.
    pub fn free(&mut self, addr: u64) -> Result<(), SecurityError> {
        if self.live.remove(&addr).is_some() {
            Ok(())
        } else {
            Err(SecurityError::AsanViolation {
                kind: "double-free".to_string(),
                addr,
                size: 0,
            })
        }
    }

    /// Returns the list of addresses that have not been freed.
    pub fn report_leaks(&self) -> Vec<(u64, usize)> {
        let mut leaks: Vec<_> = self.live.iter().map(|(&a, &s)| (a, s)).collect();
        leaks.sort_by_key(|&(a, _)| a);
        leaks
    }

    /// Returns the number of live (unfreed) allocations.
    pub fn live_count(&self) -> usize {
        self.live.len()
    }
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 20 — Secure Compilation
// ═══════════════════════════════════════════════════════════════════════

// ─── PIC (Position-Independent Code) ─────────────────────────────────

/// A GOT/PLT simulation entry for position-independent code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PicEntry {
    /// The function's source name.
    pub fn_name: String,
    /// Simulated GOT (Global Offset Table) slot index.
    pub got_index: usize,
    /// Simulated PLT (Procedure Linkage Table) slot index.
    pub plt_index: usize,
}

/// Generates PIC metadata for functions in a compilation unit.
#[derive(Debug, Clone)]
pub struct PicGenerator {
    /// Next available GOT slot.
    next_got: usize,
    /// Next available PLT slot.
    next_plt: usize,
}

impl PicGenerator {
    /// Creates a new PIC generator.
    pub fn new() -> Self {
        Self {
            next_got: 0,
            next_plt: 0,
        }
    }

    /// Generates a PIC entry for `fn_name`.
    pub fn generate_pic_metadata(&mut self, fn_name: &str) -> PicEntry {
        let entry = PicEntry {
            fn_name: fn_name.to_string(),
            got_index: self.next_got,
            plt_index: self.next_plt,
        };
        self.next_got += 1;
        self.next_plt += 1;
        entry
    }
}

impl Default for PicGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ─── RELRO Section ───────────────────────────────────────────────────

/// Simulated RELRO (RELocation Read-Only) section.
///
/// After the dynamic linker has resolved relocations the section is
/// marked read-only to prevent GOT overwrites.
#[derive(Debug, Clone)]
pub struct RelroSection {
    /// Names of sections marked as RELRO.
    sections: Vec<String>,
    /// Whether the sections have been sealed.
    sealed: bool,
}

impl RelroSection {
    /// Creates a new RELRO section set.
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            sealed: false,
        }
    }

    /// Adds a section name to the RELRO set.
    pub fn add_section(&mut self, name: &str) {
        if !self.sealed {
            self.sections.push(name.to_string());
        }
    }

    /// Seals all sections as read-only.
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Returns `true` if the sections have been sealed.
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Returns the list of RELRO section names.
    pub fn sections(&self) -> &[String] {
        &self.sections
    }
}

impl Default for RelroSection {
    fn default() -> Self {
        Self::new()
    }
}

// ─── NX Stack ────────────────────────────────────────────────────────

/// Configuration for non-executable stack pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NxStackConfig {
    /// Whether NX (no-execute) is enabled for the stack.
    pub enabled: bool,
    /// Stack start address (simulated).
    pub stack_start: u64,
    /// Stack size in bytes.
    pub stack_size: usize,
}

impl NxStackConfig {
    /// Creates an NX stack config with default parameters.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stack_start: 0x7fff_0000,
            stack_size: 8 * 1024 * 1024, // 8 MB
        }
    }

    /// Checks whether an address falls within the stack region.
    pub fn is_stack_address(&self, addr: u64) -> bool {
        addr >= self.stack_start && addr < self.stack_start + self.stack_size as u64
    }

    /// Returns an error if execution is attempted on a stack address.
    pub fn check_execute(&self, addr: u64) -> Result<(), SecurityError> {
        if self.enabled && self.is_stack_address(addr) {
            Err(SecurityError::UndefinedBehavior {
                description: format!(
                    "attempted execution at stack address {addr:#x} (NX violation)"
                ),
            })
        } else {
            Ok(())
        }
    }
}

// ─── Fortify Source ──────────────────────────────────────────────────

/// A warning generated by the fortify source checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortifyWarning {
    /// The unsafe operation that was detected.
    pub operation: String,
    /// Description of the risk.
    pub risk: String,
    /// Recommended safe alternative.
    pub recommendation: String,
}

/// Checks buffer operations for potential overflows.
///
/// Simulates GCC/Clang `-D_FORTIFY_SOURCE` by detecting when a copy
/// operation would exceed the destination buffer.
#[derive(Debug, Clone)]
pub struct FortifyChecker;

impl FortifyChecker {
    /// Creates a new checker.
    pub fn new() -> Self {
        Self
    }

    /// Checks a buffer operation and returns a warning if unsafe.
    ///
    /// - `op` — the operation name (e.g., "memcpy", "strcpy")
    /// - `copy_size` — bytes being copied
    /// - `buf_size` — destination buffer capacity
    pub fn check_buffer_op(
        &self,
        op: &str,
        copy_size: usize,
        buf_size: usize,
    ) -> Option<FortifyWarning> {
        if copy_size > buf_size {
            Some(FortifyWarning {
                operation: op.to_string(),
                risk: format!("{op} of {copy_size} bytes into {buf_size}-byte buffer"),
                recommendation: format!(
                    "use bounded variant or increase buffer to >= {copy_size} bytes"
                ),
            })
        } else {
            None
        }
    }
}

impl Default for FortifyChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Harden Config ───────────────────────────────────────────────────

/// Convenience alias: enables all security features via [`SecurityConfig::all()`].
///
/// Equivalent to passing `-fharden` on the command line.
#[derive(Debug, Clone)]
pub struct HardenConfig;

impl HardenConfig {
    /// Returns a [`SecurityConfig`] with every protection enabled.
    pub fn all() -> SecurityConfig {
        SecurityConfig::all()
    }
}

// ─── Security Audit ──────────────────────────────────────────────────

/// Severity level for an audit finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Minor concern — informational.
    Low,
    /// Potential vulnerability.
    Medium,
    /// Likely exploitable vulnerability.
    High,
    /// Actively dangerous, must fix before release.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Low => write!(f, "LOW"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::High => write!(f, "HIGH"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single finding from a security audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding {
    /// Severity of the finding.
    pub severity: Severity,
    /// Source location or identifier.
    pub location: String,
    /// What was found.
    pub description: String,
    /// Suggested remediation.
    pub recommendation: String,
}

/// Scans a [`SecurityConfig`] and produces a report of findings.
#[derive(Debug, Clone)]
pub struct SecurityAudit;

impl SecurityAudit {
    /// Creates a new audit runner.
    pub fn new() -> Self {
        Self
    }

    /// Audits the given configuration and returns any findings.
    pub fn audit(&self, config: &SecurityConfig) -> Vec<AuditFinding> {
        let mut findings = Vec::new();
        if !config.enable_canaries {
            findings.push(AuditFinding {
                severity: Severity::High,
                location: "SecurityConfig".to_string(),
                description: "stack canaries are disabled".to_string(),
                recommendation: "enable canaries to detect stack buffer overflows".to_string(),
            });
        }
        if !config.enable_cfi {
            findings.push(AuditFinding {
                severity: Severity::High,
                location: "SecurityConfig".to_string(),
                description: "CFI is disabled".to_string(),
                recommendation: "enable CFI to prevent control-flow hijacking".to_string(),
            });
        }
        if !config.enable_nx {
            findings.push(AuditFinding {
                severity: Severity::Critical,
                location: "SecurityConfig".to_string(),
                description: "NX stack is disabled".to_string(),
                recommendation: "enable NX to prevent stack code execution".to_string(),
            });
        }
        if !config.enable_relro {
            findings.push(AuditFinding {
                severity: Severity::Medium,
                location: "SecurityConfig".to_string(),
                description: "RELRO is disabled".to_string(),
                recommendation: "enable RELRO to protect GOT from overwrites".to_string(),
            });
        }
        if !config.enable_pic {
            findings.push(AuditFinding {
                severity: Severity::Medium,
                location: "SecurityConfig".to_string(),
                description: "PIC is disabled".to_string(),
                recommendation: "enable PIC for ASLR compatibility".to_string(),
            });
        }
        if !config.enable_stack_clash {
            findings.push(AuditFinding {
                severity: Severity::Medium,
                location: "SecurityConfig".to_string(),
                description: "stack clash protection is disabled".to_string(),
                recommendation: "enable stack clash probing for large frames".to_string(),
            });
        }
        findings
    }
}

impl Default for SecurityAudit {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Hardening Score ─────────────────────────────────────────────────

/// Computes a hardening score (0–100) based on which features are enabled.
///
/// Each of the 8 features contributes a weighted amount to the total:
///
/// | Feature       | Weight |
/// |---------------|--------|
/// | canaries      | 15     |
/// | shadow_stack  | 10     |
/// | stack_clash   | 10     |
/// | asan          | 10     |
/// | cfi           | 20     |
/// | pic           | 10     |
/// | relro         | 10     |
/// | nx            | 15     |
pub fn compute_hardening_score(config: &SecurityConfig) -> u8 {
    let mut score: u32 = 0;
    if config.enable_canaries {
        score += 15;
    }
    if config.enable_shadow_stack {
        score += 10;
    }
    if config.enable_stack_clash {
        score += 10;
    }
    if config.enable_asan {
        score += 10;
    }
    if config.enable_cfi {
        score += 20;
    }
    if config.enable_pic {
        score += 10;
    }
    if config.enable_relro {
        score += 10;
    }
    if config.enable_nx {
        score += 15;
    }
    score.min(100) as u8
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — 40 tests (10 per sprint: s17_1..s17_10, s18_1..s18_10, etc.)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 17: Stack Protection (s17_1 – s17_10) ─────────────────

    #[test]
    fn s17_1_security_config_none_disables_all() {
        let cfg = SecurityConfig::none();
        assert!(!cfg.enable_canaries);
        assert!(!cfg.enable_shadow_stack);
        assert!(!cfg.enable_stack_clash);
        assert!(!cfg.enable_asan);
        assert!(!cfg.enable_cfi);
        assert!(!cfg.enable_pic);
        assert!(!cfg.enable_relro);
        assert!(!cfg.enable_nx);
        assert_eq!(cfg.enabled_count(), 0);
    }

    #[test]
    fn s17_2_security_config_all_enables_all() {
        let cfg = SecurityConfig::all();
        assert!(cfg.enable_canaries);
        assert!(cfg.enable_shadow_stack);
        assert!(cfg.enable_stack_clash);
        assert!(cfg.enable_asan);
        assert!(cfg.enable_cfi);
        assert!(cfg.enable_pic);
        assert!(cfg.enable_relro);
        assert!(cfg.enable_nx);
        assert_eq!(cfg.enabled_count(), 8);
    }

    #[test]
    fn s17_3_canary_generator_deterministic() {
        let gen = CanaryGenerator::new(42);
        let c1 = gen.generate_canary("main");
        let c2 = gen.generate_canary("main");
        assert_eq!(c1, c2, "same fn_name must produce same canary");
        let c3 = gen.generate_canary("other");
        assert_ne!(c1, c3, "different fn_name should produce different canary");
    }

    #[test]
    fn s17_4_check_canary_ok_and_violation() {
        assert!(check_canary("f", 0xDEAD, 0xDEAD).is_ok());
        let err = check_canary("f", 0xDEAD, 0xBEEF).unwrap_err();
        match err {
            SecurityError::CanaryViolation {
                expected, actual, ..
            } => {
                assert_eq!(expected, 0xDEAD);
                assert_eq!(actual, 0xBEEF);
            }
            _ => panic!("expected CanaryViolation"),
        }
    }

    #[test]
    fn s17_5_stack_clash_probe_points() {
        let p = StackClashProtector::new(4096);
        assert!(
            p.probe_stack(4096).is_empty(),
            "exactly one page: no probes"
        );
        let probes = p.probe_stack(12288);
        assert_eq!(probes.len(), 3);
        assert_eq!(probes[0].offset, 4096);
        assert_eq!(probes[1].offset, 8192);
        assert_eq!(probes[2].offset, 12288);
    }

    #[test]
    fn s17_6_shadow_stack_push_pop_verify() {
        let mut ss = ShadowStack::new();
        ss.push_return(0x1000);
        ss.push_return(0x2000);
        assert!(ss.verify_return(0x2000));
        assert!(!ss.verify_return(0x9999));
        assert_eq!(ss.pop_return(), Some(0x2000));
        assert_eq!(ss.pop_return(), Some(0x1000));
        assert_eq!(ss.pop_return(), None);
    }

    #[test]
    fn s17_7_shadow_stack_empty_verify_returns_true() {
        let ss = ShadowStack::new();
        assert!(ss.verify_return(0xABCD));
    }

    #[test]
    fn s17_8_analyze_stack_depth_linear_call_chain() {
        let mut graph = HashMap::new();
        graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.insert("b".to_string(), vec!["c".to_string()]);
        graph.insert("c".to_string(), vec![]);
        let mut sizes = HashMap::new();
        sizes.insert("a".to_string(), 100);
        sizes.insert("b".to_string(), 200);
        sizes.insert("c".to_string(), 50);
        assert_eq!(analyze_stack_depth("a", &graph, &sizes), 350);
    }

    #[test]
    fn s17_9_analyze_stack_depth_handles_cycles() {
        let mut graph = HashMap::new();
        graph.insert("x".to_string(), vec!["y".to_string()]);
        graph.insert("y".to_string(), vec!["x".to_string()]);
        let mut sizes = HashMap::new();
        sizes.insert("x".to_string(), 64);
        sizes.insert("y".to_string(), 32);
        // Should not infinitely recurse; cycle breaks at revisit
        let depth = analyze_stack_depth("x", &graph, &sizes);
        assert!(depth > 0);
    }

    #[test]
    fn s17_10_stack_guard_overflow_detection() {
        let mut guard = StackGuard::new(1024);
        assert!(guard.push(512).is_ok());
        assert_eq!(guard.current_usage(), 512);
        assert!(guard.push(256).is_ok());
        let err = guard.push(512).unwrap_err();
        match err {
            SecurityError::StackOverflow { depth, limit } => {
                assert_eq!(limit, 1024);
                assert!(depth > 1024);
            }
            _ => panic!("expected StackOverflow"),
        }
        guard.pop(256);
        assert_eq!(guard.current_usage(), 512);
    }

    // ── Sprint 18: Control Flow Integrity (s18_1 – s18_10) ───────────

    #[test]
    fn s18_1_cfi_metadata_construction() {
        let meta = CfiMetadata {
            type_hash: 0x1234,
            fn_name: "add".to_string(),
            param_types: vec!["i32".to_string(), "i32".to_string()],
            return_type: "i32".to_string(),
        };
        assert_eq!(meta.fn_name, "add");
        assert_eq!(meta.param_types.len(), 2);
    }

    #[test]
    fn s18_2_compute_type_hash_deterministic() {
        let params = vec!["i32".to_string(), "f64".to_string()];
        let h1 = compute_type_hash(&params, "bool");
        let h2 = compute_type_hash(&params, "bool");
        assert_eq!(h1, h2);
        let h3 = compute_type_hash(&params, "i32");
        assert_ne!(h1, h3, "different return type should differ");
    }

    #[test]
    fn s18_3_validate_indirect_call_ok_and_fail() {
        assert!(validate_indirect_call(100, 100).is_ok());
        let err = validate_indirect_call(100, 200).unwrap_err();
        matches!(err, SecurityError::CfiViolation { .. });
    }

    #[test]
    fn s18_4_cfi_registry_register_and_validate() {
        let mut reg = CfiRegistry::new();
        assert!(reg.is_empty());
        let hash = compute_type_hash(&["i32".to_string()], "i32");
        let meta = CfiMetadata {
            type_hash: hash,
            fn_name: "inc".to_string(),
            param_types: vec!["i32".to_string()],
            return_type: "i32".to_string(),
        };
        reg.register(0x4000, meta);
        assert_eq!(reg.len(), 1);
        assert!(reg.validate(0x4000, hash).is_ok());
        assert!(reg.validate(0x4000, hash.wrapping_add(1)).is_err());
        assert!(reg.validate(0x9999, hash).is_err());
    }

    #[test]
    fn s18_5_validate_fn_ptr_known_and_unknown() {
        let known = vec![0x1000, 0x2000, 0x3000];
        assert!(validate_fn_ptr(0x2000, &known).is_ok());
        assert!(validate_fn_ptr(0x5000, &known).is_err());
    }

    #[test]
    fn s18_6_vtable_guard_verify_intact() {
        let vt = VTableGuard::new(vec![0xA, 0xB, 0xC]);
        assert!(vt.verify().is_ok());
        assert_eq!(vt.get(1), Some(0xB));
        assert_eq!(vt.get(5), None);
    }

    #[test]
    fn s18_7_vtable_guard_detect_corruption() {
        let mut vt = VTableGuard::new(vec![0xA, 0xB]);
        // Corrupt an entry
        vt.entries[0] = 0xFF;
        assert!(vt.verify().is_err());
    }

    #[test]
    fn s18_8_verify_vtable_against_expected_hash() {
        let entries = vec![0x10, 0x20];
        let vt = VTableGuard::new(entries.clone());
        let expected = VTableGuard::compute_hash(&entries);
        assert!(verify_vtable(&vt, expected).is_ok());
        assert!(verify_vtable(&vt, expected.wrapping_add(1)).is_err());
    }

    #[test]
    fn s18_9_jump_table_bounds_check() {
        let jt = JumpTableGuard::new(4);
        assert!(jt.check_index(0).is_ok());
        assert!(jt.check_index(3).is_ok());
        assert!(jt.check_index(4).is_err());
        assert!(jt.check_index(100).is_err());
    }

    #[test]
    fn s18_10_function_diversifier_nop_count() {
        let div = FunctionDiversifier::new(0);
        let n1 = div.nop_count("main");
        assert!(n1 <= 15, "nop_count must be 0..=15");
        let n2 = div.nop_count("main");
        assert_eq!(n1, n2, "must be deterministic");
        // Different function names may produce different counts
        let _n3 = div.nop_count("helper");
    }

    // ── Sprint 19: Memory Safety Runtime (s19_1 – s19_10) ────────────

    #[test]
    fn s19_1_shadow_memory_default_unallocated() {
        let sm = ShadowMemory::new();
        assert_eq!(sm.get(0x1000), MemState::Unallocated);
    }

    #[test]
    fn s19_2_shadow_memory_set_and_get() {
        let mut sm = ShadowMemory::new();
        sm.set(0x100, MemState::Allocated(8));
        sm.set(0x200, MemState::Freed);
        assert_eq!(sm.get(0x100), MemState::Allocated(8));
        assert_eq!(sm.get(0x200), MemState::Freed);
        assert_eq!(sm.tracked_count(), 2);
    }

    #[test]
    fn s19_3_asan_alloc_creates_red_zones() {
        let mut asan = AddressSanitizer::new(8);
        let (addr, rz) = asan.asan_alloc(16);
        assert_eq!(rz.zone_size, 8);
        // Left red zone should be poisoned
        assert_eq!(asan.shadow.get(rz.left_start), MemState::Poisoned);
        // Usable region should be uninitialized
        assert_eq!(asan.shadow.get(addr), MemState::Uninitialized);
        // Right red zone should be poisoned
        assert_eq!(asan.shadow.get(rz.right_start), MemState::Poisoned);
    }

    #[test]
    fn s19_4_asan_check_access_detects_overflow() {
        let mut asan = AddressSanitizer::new(8);
        let (addr, rz) = asan.asan_alloc(16);
        asan.mark_written(addr, 16);
        // Valid access
        assert!(asan.asan_check_access(addr, 16).is_ok());
        // Overflow into right red zone
        let err = asan.asan_check_access(rz.right_start, 1).unwrap_err();
        match err {
            SecurityError::AsanViolation { kind, .. } => {
                assert_eq!(kind, "buffer-overflow");
            }
            _ => panic!("expected AsanViolation"),
        }
    }

    #[test]
    fn s19_5_asan_free_and_use_after_free() {
        let mut asan = AddressSanitizer::new(8);
        let (addr, _) = asan.asan_alloc(4);
        assert!(asan.asan_free(addr).is_ok());
        let err = asan.asan_check_access(addr, 1).unwrap_err();
        match err {
            SecurityError::AsanViolation { kind, .. } => {
                assert_eq!(kind, "use-after-free");
            }
            _ => panic!("expected use-after-free"),
        }
    }

    #[test]
    fn s19_6_asan_double_free_detected() {
        let mut asan = AddressSanitizer::new(8);
        let (addr, _) = asan.asan_alloc(4);
        assert!(asan.asan_free(addr).is_ok());
        let err = asan.asan_free(addr).unwrap_err();
        match err {
            SecurityError::AsanViolation { kind, .. } => {
                assert_eq!(kind, "double-free");
            }
            _ => panic!("expected double-free"),
        }
    }

    #[test]
    fn s19_7_quarantine_capacity_and_eviction() {
        let mut q = Quarantine::new(3);
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!(q.len(), 3);
        assert!(q.contains(1));
        q.push(4); // evicts 1
        assert!(!q.contains(1));
        assert!(q.contains(4));
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn s19_8_msan_detects_uninitialized_read() {
        let mut msan = MemorySanitizer::new();
        msan.msan_alloc(0x500, 8);
        let err = msan.msan_check_read(0x500, 4).unwrap_err();
        match err {
            SecurityError::AsanViolation { kind, .. } => {
                assert_eq!(kind, "use-of-uninitialized");
            }
            _ => panic!("expected use-of-uninitialized"),
        }
        msan.msan_write(0x500, 4);
        assert!(msan.msan_check_read(0x500, 4).is_ok());
    }

    #[test]
    fn s19_9_leak_detector_reports_unfreed() {
        let mut ld = LeakDetector::new();
        ld.alloc(0x100, 32);
        ld.alloc(0x200, 64);
        assert_eq!(ld.live_count(), 2);
        assert!(ld.free(0x100).is_ok());
        let leaks = ld.report_leaks();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0], (0x200, 64));
    }

    #[test]
    fn s19_10_leak_detector_double_free() {
        let mut ld = LeakDetector::new();
        ld.alloc(0x300, 16);
        assert!(ld.free(0x300).is_ok());
        let err = ld.free(0x300).unwrap_err();
        match err {
            SecurityError::AsanViolation { kind, .. } => {
                assert_eq!(kind, "double-free");
            }
            _ => panic!("expected double-free"),
        }
    }

    // ── Sprint 20: Secure Compilation (s20_1 – s20_10) ───────────────

    #[test]
    fn s20_1_pic_generator_sequential_slots() {
        let mut pic = PicGenerator::new();
        let e1 = pic.generate_pic_metadata("foo");
        let e2 = pic.generate_pic_metadata("bar");
        assert_eq!(e1.got_index, 0);
        assert_eq!(e1.plt_index, 0);
        assert_eq!(e2.got_index, 1);
        assert_eq!(e2.plt_index, 1);
        assert_eq!(e1.fn_name, "foo");
    }

    #[test]
    fn s20_2_relro_section_add_and_seal() {
        let mut relro = RelroSection::new();
        relro.add_section(".got");
        relro.add_section(".got.plt");
        assert!(!relro.is_sealed());
        relro.seal();
        assert!(relro.is_sealed());
        relro.add_section(".ignored"); // should be ignored
        assert_eq!(relro.sections().len(), 2);
    }

    #[test]
    fn s20_3_nx_stack_detects_execution() {
        let nx = NxStackConfig::new(true);
        // Address inside the stack
        let stack_addr = nx.stack_start + 100;
        assert!(nx.is_stack_address(stack_addr));
        assert!(nx.check_execute(stack_addr).is_err());
        // Address outside the stack
        assert!(nx.check_execute(0x0040_0000).is_ok());
    }

    #[test]
    fn s20_4_nx_stack_disabled_allows_all() {
        let nx = NxStackConfig::new(false);
        let stack_addr = nx.stack_start + 100;
        assert!(nx.check_execute(stack_addr).is_ok());
    }

    #[test]
    fn s20_5_fortify_checker_detects_overflow() {
        let fc = FortifyChecker::new();
        let warn = fc.check_buffer_op("memcpy", 64, 32);
        assert!(warn.is_some());
        let w = warn.unwrap();
        assert_eq!(w.operation, "memcpy");
        assert!(w.recommendation.contains("64"));
    }

    #[test]
    fn s20_6_fortify_checker_allows_safe_op() {
        let fc = FortifyChecker::new();
        assert!(fc.check_buffer_op("memcpy", 16, 32).is_none());
        assert!(fc.check_buffer_op("strcpy", 0, 0).is_none());
    }

    #[test]
    fn s20_7_harden_config_all_matches_security_config_all() {
        let harden = HardenConfig::all();
        let all = SecurityConfig::all();
        assert_eq!(harden, all);
    }

    #[test]
    fn s20_8_security_audit_finds_disabled_features() {
        let cfg = SecurityConfig::none();
        let audit = SecurityAudit::new();
        let findings = audit.audit(&cfg);
        // Should report canaries, CFI, NX, RELRO, PIC, stack_clash
        assert!(findings.len() >= 5);
        let critical_count = findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        assert!(critical_count >= 1, "NX disabled should be critical");
    }

    #[test]
    fn s20_9_security_audit_clean_for_all() {
        let cfg = SecurityConfig::all();
        let audit = SecurityAudit::new();
        let findings = audit.audit(&cfg);
        assert!(
            findings.is_empty(),
            "all-enabled config should have no findings"
        );
    }

    #[test]
    fn s20_10_compute_hardening_score() {
        assert_eq!(compute_hardening_score(&SecurityConfig::none()), 0);
        assert_eq!(compute_hardening_score(&SecurityConfig::all()), 100);
        let release = SecurityConfig::release_defaults();
        let score = compute_hardening_score(&release);
        assert!(
            score > 50,
            "release defaults should score > 50, got {score}"
        );
        assert!(score < 100, "release defaults omit some features");
    }
}
