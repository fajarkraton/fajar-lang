//! Sprint E9: FFI Safety & Performance.
//!
//! Boundary validation, leak detection, thread safety, overhead measurement,
//! batch optimization, zero-copy verification, alignment, endianness, and
//! sanitizer integration for FFI v2.
//!
//! All checks are simulated — no real C ABI or foreign process interaction.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Common types
// ═══════════════════════════════════════════════════════════════════════

/// Errors raised by FFI safety checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiSafetyError {
    /// A value at the FFI boundary has an invalid type.
    BoundaryTypeMismatch {
        /// Position in the argument list.
        position: usize,
        /// Expected Fajar type.
        expected: String,
        /// Type that was actually provided.
        got: String,
    },
    /// A pointer passed across the FFI boundary is null.
    NullPointer {
        /// Name of the parameter.
        param: String,
    },
    /// An FFI allocation was never freed.
    MemoryLeak {
        /// Simulated pointer address.
        ptr: u64,
        /// Size in bytes.
        size: usize,
        /// What type the allocation held.
        type_name: String,
    },
    /// A GIL-sensitive call was made from the wrong thread.
    GilViolation {
        /// Thread that attempted the call.
        thread: String,
        /// Thread that holds the GIL.
        holder: String,
    },
    /// Two locks were acquired in an unsafe order.
    LockOrderViolation {
        /// First lock held.
        held: String,
        /// Second lock requested (lower order).
        requested: String,
    },
    /// A pointer does not satisfy the required alignment.
    AlignmentViolation {
        /// Simulated pointer address.
        address: u64,
        /// Required alignment in bytes.
        required: usize,
        /// Actual alignment of the address.
        actual: usize,
    },
    /// Zero-copy transfer changed the underlying address.
    ZeroCopyAddressMismatch {
        /// Address before transfer.
        before: u64,
        /// Address after transfer.
        after: u64,
    },
    /// Generic safety error.
    Other(String),
}

impl fmt::Display for FfiSafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundaryTypeMismatch {
                position,
                expected,
                got,
            } => write!(
                f,
                "boundary type mismatch at position {position}: expected {expected}, got {got}"
            ),
            Self::NullPointer { param } => write!(f, "null pointer for parameter '{param}'"),
            Self::MemoryLeak {
                ptr,
                size,
                type_name,
            } => write!(f, "memory leak: 0x{ptr:x} ({size} bytes, type {type_name})"),
            Self::GilViolation { thread, holder } => {
                write!(
                    f,
                    "GIL violation: thread '{thread}' called while '{holder}' holds GIL"
                )
            }
            Self::LockOrderViolation { held, requested } => {
                write!(
                    f,
                    "lock order violation: holding '{held}', requesting '{requested}'"
                )
            }
            Self::AlignmentViolation {
                address,
                required,
                actual,
            } => write!(
                f,
                "alignment violation at 0x{address:x}: required {required}, actual {actual}"
            ),
            Self::ZeroCopyAddressMismatch { before, after } => {
                write!(f, "zero-copy address mismatch: 0x{before:x} != 0x{after:x}")
            }
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.1: Boundary validation
// ═══════════════════════════════════════════════════════════════════════

/// The kind of an FFI boundary value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryType {
    /// Signed 64-bit integer.
    Int,
    /// 64-bit float.
    Float,
    /// UTF-8 string.
    Str,
    /// Boolean.
    Bool,
    /// Opaque pointer/handle.
    Pointer,
    /// Fixed-size array of a given element type.
    Array(Box<BoundaryType>),
    /// Struct with named fields.
    Struct(String),
}

impl fmt::Display for BoundaryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "i64"),
            Self::Float => write!(f, "f64"),
            Self::Str => write!(f, "str"),
            Self::Bool => write!(f, "bool"),
            Self::Pointer => write!(f, "ptr"),
            Self::Array(inner) => write!(f, "[{inner}]"),
            Self::Struct(name) => write!(f, "struct {name}"),
        }
    }
}

/// A boundary value that can be type-checked.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryValue {
    /// Signed integer.
    Int(i64),
    /// Float.
    Float(f64),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
    /// Opaque pointer (0 means null).
    Pointer(u64),
    /// Array of boundary values.
    Array(Vec<BoundaryValue>),
    /// Named struct.
    Struct {
        /// Struct type name.
        name: String,
        /// Field values.
        fields: HashMap<String, BoundaryValue>,
    },
}

impl BoundaryValue {
    /// Returns the runtime type of this value.
    pub fn runtime_type(&self) -> BoundaryType {
        match self {
            Self::Int(_) => BoundaryType::Int,
            Self::Float(_) => BoundaryType::Float,
            Self::Str(_) => BoundaryType::Str,
            Self::Bool(_) => BoundaryType::Bool,
            Self::Pointer(_) => BoundaryType::Pointer,
            Self::Array(elems) => {
                let inner = elems
                    .first()
                    .map(|e| e.runtime_type())
                    .unwrap_or(BoundaryType::Int);
                BoundaryType::Array(Box::new(inner))
            }
            Self::Struct { name, .. } => BoundaryType::Struct(name.clone()),
        }
    }
}

/// Validates types and null-safety at the FFI boundary.
///
/// Each call signature declares a list of expected types; the validator
/// confirms that actual values match before the call crosses the boundary.
#[derive(Debug, Clone)]
pub struct BoundaryValidator {
    /// Whether null pointers are allowed.
    pub allow_null: bool,
    /// Registered function signatures: name -> expected param types.
    signatures: HashMap<String, Vec<BoundaryType>>,
}

impl BoundaryValidator {
    /// Creates a new validator.
    pub fn new() -> Self {
        Self {
            allow_null: false,
            signatures: HashMap::new(),
        }
    }

    /// Registers an FFI function signature.
    pub fn register(&mut self, name: impl Into<String>, param_types: Vec<BoundaryType>) {
        self.signatures.insert(name.into(), param_types);
    }

    /// Validates a call's arguments against its registered signature.
    pub fn validate_call(
        &self,
        name: &str,
        args: &[BoundaryValue],
    ) -> Result<(), Vec<FfiSafetyError>> {
        let sig = match self.signatures.get(name) {
            Some(s) => s,
            None => {
                return Err(vec![FfiSafetyError::Other(format!(
                    "unknown FFI function: {name}"
                ))]);
            }
        };

        let mut errors = Vec::new();

        if args.len() != sig.len() {
            errors.push(FfiSafetyError::Other(format!(
                "arity mismatch for '{name}': expected {} args, got {}",
                sig.len(),
                args.len()
            )));
            return Err(errors);
        }

        for (i, (arg, expected)) in args.iter().zip(sig.iter()).enumerate() {
            let got = arg.runtime_type();
            if !Self::types_compatible(&got, expected) {
                errors.push(FfiSafetyError::BoundaryTypeMismatch {
                    position: i,
                    expected: format!("{expected}"),
                    got: format!("{got}"),
                });
            }
            // Null-pointer check
            if !self.allow_null {
                if let BoundaryValue::Pointer(0) = arg {
                    errors.push(FfiSafetyError::NullPointer {
                        param: format!("arg{i}"),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Checks if two boundary types are compatible.
    fn types_compatible(got: &BoundaryType, expected: &BoundaryType) -> bool {
        match (got, expected) {
            (BoundaryType::Array(a), BoundaryType::Array(b)) => Self::types_compatible(a, b),
            (a, b) => a == b,
        }
    }

    /// Returns how many signatures are registered.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

impl Default for BoundaryValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.2: Memory leak detection
// ═══════════════════════════════════════════════════════════════════════

/// Tracks a single FFI allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiAllocation {
    /// Simulated pointer address.
    pub ptr: u64,
    /// Size in bytes.
    pub size: usize,
    /// Name of the allocated type.
    pub type_name: String,
    /// Simulated callsite description.
    pub allocated_at: String,
}

/// A report of leaked allocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeakReport {
    /// All allocations that were never freed.
    pub leaked: Vec<FfiAllocation>,
    /// Total bytes leaked.
    pub total_bytes: usize,
}

impl LeakReport {
    /// Returns `true` when there are no leaks.
    pub fn is_clean(&self) -> bool {
        self.leaked.is_empty()
    }
}

impl fmt::Display for LeakReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_clean() {
            return write!(f, "No leaks detected.");
        }
        writeln!(
            f,
            "LEAK REPORT: {} allocation(s), {} bytes total",
            self.leaked.len(),
            self.total_bytes
        )?;
        for alloc in &self.leaked {
            writeln!(
                f,
                "  - 0x{:x}: {} bytes ({}) allocated at {}",
                alloc.ptr, alloc.size, alloc.type_name, alloc.allocated_at
            )?;
        }
        Ok(())
    }
}

/// Tracks FFI allocations and detects leaks on report.
///
/// Call `alloc()` when crossing the boundary and `free()` when the foreign
/// object is released. `report()` returns any remaining (leaked) allocations.
#[derive(Debug, Clone)]
pub struct LeakDetector {
    /// Active (not-yet-freed) allocations keyed by pointer.
    active: HashMap<u64, FfiAllocation>,
    /// Counter for generating simulated addresses.
    next_addr: u64,
    /// Total allocations made (for statistics).
    total_allocs: usize,
    /// Total frees performed.
    total_frees: usize,
}

impl LeakDetector {
    /// Creates a new leak detector.
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            next_addr: 0x1000,
            total_allocs: 0,
            total_frees: 0,
        }
    }

    /// Records an allocation. Returns a simulated pointer address.
    pub fn alloc(
        &mut self,
        size: usize,
        type_name: impl Into<String>,
        callsite: impl Into<String>,
    ) -> u64 {
        let ptr = self.next_addr;
        // Align to 16-byte boundary for realism.
        self.next_addr += (size as u64).div_ceil(16) * 16;
        self.active.insert(
            ptr,
            FfiAllocation {
                ptr,
                size,
                type_name: type_name.into(),
                allocated_at: callsite.into(),
            },
        );
        self.total_allocs += 1;
        ptr
    }

    /// Records a free. Returns `true` if the pointer was tracked; `false` otherwise.
    pub fn free(&mut self, ptr: u64) -> bool {
        if self.active.remove(&ptr).is_some() {
            self.total_frees += 1;
            true
        } else {
            false
        }
    }

    /// Returns the number of currently active (unfreed) allocations.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Generates a leak report from all unfreed allocations.
    pub fn report(&self) -> LeakReport {
        let leaked: Vec<FfiAllocation> = self.active.values().cloned().collect();
        let total_bytes: usize = leaked.iter().map(|a| a.size).sum();
        LeakReport {
            leaked,
            total_bytes,
        }
    }

    /// Returns `(total_allocs, total_frees)`.
    pub fn stats(&self) -> (usize, usize) {
        (self.total_allocs, self.total_frees)
    }
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.3: Thread safety
// ═══════════════════════════════════════════════════════════════════════

/// Detects GIL violations and lock ordering issues for FFI calls that
/// interact with Python or other GIL-based runtimes.
#[derive(Debug, Clone)]
pub struct ThreadSafetyChecker {
    /// Which thread currently holds the GIL (None = nobody).
    gil_holder: Option<String>,
    /// Lock ordering: name -> numeric order. Lower numbers must be acquired first.
    lock_order: HashMap<String, u32>,
    /// Per-thread list of currently held locks (name, order).
    held_locks: HashMap<String, Vec<(String, u32)>>,
}

impl ThreadSafetyChecker {
    /// Creates a new checker with no GIL holder and no lock order constraints.
    pub fn new() -> Self {
        Self {
            gil_holder: None,
            lock_order: HashMap::new(),
            held_locks: HashMap::new(),
        }
    }

    /// Defines a lock ordering constraint. Locks with lower order must be
    /// acquired before locks with higher order.
    pub fn define_lock_order(&mut self, lock_name: impl Into<String>, order: u32) {
        self.lock_order.insert(lock_name.into(), order);
    }

    /// Simulates acquiring the GIL from a given thread.
    pub fn acquire_gil(&mut self, thread: impl Into<String>) -> Result<(), FfiSafetyError> {
        let thread = thread.into();
        if let Some(holder) = &self.gil_holder {
            if *holder != thread {
                return Err(FfiSafetyError::GilViolation {
                    thread,
                    holder: holder.clone(),
                });
            }
        }
        self.gil_holder = Some(thread);
        Ok(())
    }

    /// Releases the GIL.
    pub fn release_gil(&mut self) {
        self.gil_holder = None;
    }

    /// Returns which thread currently holds the GIL, if any.
    pub fn gil_holder(&self) -> Option<&str> {
        self.gil_holder.as_deref()
    }

    /// Simulates a thread acquiring a lock, checking ordering constraints.
    pub fn acquire_lock(
        &mut self,
        thread: impl Into<String>,
        lock_name: impl Into<String>,
    ) -> Result<(), FfiSafetyError> {
        let thread = thread.into();
        let lock_name = lock_name.into();
        let new_order = self.lock_order.get(&lock_name).copied().unwrap_or(0);

        let held = self.held_locks.entry(thread).or_default();

        // Check: every held lock must have order < new_order (or equal for reentrant).
        for (held_name, held_order) in held.iter() {
            if new_order < *held_order {
                return Err(FfiSafetyError::LockOrderViolation {
                    held: held_name.clone(),
                    requested: lock_name,
                });
            }
        }

        held.push((lock_name, new_order));
        Ok(())
    }

    /// Releases a lock held by a thread.
    pub fn release_lock(&mut self, thread: &str, lock_name: &str) {
        if let Some(held) = self.held_locks.get_mut(thread) {
            if let Some(pos) = held.iter().position(|(n, _)| n == lock_name) {
                held.remove(pos);
            }
        }
    }

    /// Returns the number of locks held by a thread.
    pub fn locks_held_by(&self, thread: &str) -> usize {
        self.held_locks.get(thread).map_or(0, |v| v.len())
    }
}

impl Default for ThreadSafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.4: Overhead measurement
// ═══════════════════════════════════════════════════════════════════════

/// Simulated call timing statistics in nanoseconds.
#[derive(Debug, Clone, PartialEq)]
pub struct CallTimings {
    /// Minimum call overhead in nanoseconds.
    pub min_ns: u64,
    /// Maximum call overhead in nanoseconds.
    pub max_ns: u64,
    /// Arithmetic mean overhead in nanoseconds.
    pub avg_ns: u64,
    /// 99th percentile overhead in nanoseconds.
    pub p99_ns: u64,
    /// Number of calls measured.
    pub call_count: usize,
}

impl fmt::Display for CallTimings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "calls={} min={}ns avg={}ns p99={}ns max={}ns",
            self.call_count, self.min_ns, self.avg_ns, self.p99_ns, self.max_ns
        )
    }
}

/// Measures FFI call overhead using simulated nanosecond timestamps.
///
/// Production would use `std::time::Instant`; here we accept pre-recorded
/// durations so that tests are deterministic.
#[derive(Debug, Clone)]
pub struct FfiOverhead {
    /// Name of the measured function.
    pub function_name: String,
    /// Recorded durations (nanoseconds).
    samples: Vec<u64>,
}

impl FfiOverhead {
    /// Creates a new overhead tracker for a function.
    pub fn new(function_name: impl Into<String>) -> Self {
        Self {
            function_name: function_name.into(),
            samples: Vec::new(),
        }
    }

    /// Records a single call's overhead in nanoseconds.
    pub fn record(&mut self, duration_ns: u64) {
        self.samples.push(duration_ns);
    }

    /// Records multiple call overheads at once.
    pub fn record_batch(&mut self, durations: &[u64]) {
        self.samples.extend_from_slice(durations);
    }

    /// Returns the number of recorded samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Computes timing statistics from all recorded samples.
    ///
    /// Returns `None` if no samples have been recorded.
    pub fn compute(&self) -> Option<CallTimings> {
        if self.samples.is_empty() {
            return None;
        }

        let mut sorted = self.samples.clone();
        sorted.sort_unstable();

        let min_ns = sorted[0];
        let max_ns = sorted[sorted.len() - 1];
        let sum: u64 = sorted.iter().sum();
        let avg_ns = sum / sorted.len() as u64;
        let p99_index = ((sorted.len() as f64) * 0.99).ceil() as usize;
        let p99_ns = sorted[p99_index.min(sorted.len()) - 1];

        Some(CallTimings {
            min_ns,
            max_ns,
            avg_ns,
            p99_ns,
            call_count: sorted.len(),
        })
    }

    /// Resets all recorded samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.5: Batch optimization
// ═══════════════════════════════════════════════════════════════════════

/// A queued call waiting to be dispatched in a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedCall {
    /// Name of the FFI function.
    pub function_name: String,
    /// Arguments for this call.
    pub args: Vec<BoundaryValue>,
}

/// Result of executing a batch of FFI calls.
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Number of calls in the batch.
    pub call_count: usize,
    /// Simulated total overhead in nanoseconds.
    pub total_overhead_ns: u64,
    /// Simulated per-call overhead (amortized).
    pub per_call_overhead_ns: u64,
    /// Whether the batch was fused into a single boundary crossing.
    pub fused: bool,
}

/// Amortizes FFI call overhead by batching repeated calls to the same
/// foreign function into a single boundary crossing.
#[derive(Debug, Clone)]
pub struct BatchCallOptimizer {
    /// Queued calls waiting to be dispatched.
    queue: Vec<QueuedCall>,
    /// Simulated single-call overhead in nanoseconds.
    pub single_call_overhead_ns: u64,
    /// Simulated batch-fixed overhead in nanoseconds (paid once per batch).
    pub batch_fixed_overhead_ns: u64,
    /// Simulated per-item overhead within a batch (much lower than single-call).
    pub batch_per_item_ns: u64,
}

impl BatchCallOptimizer {
    /// Creates a batch optimizer with default overhead parameters.
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            single_call_overhead_ns: 500,
            batch_fixed_overhead_ns: 600,
            batch_per_item_ns: 50,
        }
    }

    /// Enqueues a call for batching.
    pub fn enqueue(&mut self, function_name: impl Into<String>, args: Vec<BoundaryValue>) {
        self.queue.push(QueuedCall {
            function_name: function_name.into(),
            args,
        });
    }

    /// Returns the number of queued calls.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Flushes the queue and returns a batch result with overhead statistics.
    ///
    /// If only one call is queued, there is no batching advantage.
    pub fn flush(&mut self) -> BatchResult {
        let count = self.queue.len();
        self.queue.clear();

        if count <= 1 {
            return BatchResult {
                call_count: count,
                total_overhead_ns: self.single_call_overhead_ns * count as u64,
                per_call_overhead_ns: self.single_call_overhead_ns,
                fused: false,
            };
        }

        let total = self.batch_fixed_overhead_ns + self.batch_per_item_ns * count as u64;
        let per_call = total / count as u64;

        BatchResult {
            call_count: count,
            total_overhead_ns: total,
            per_call_overhead_ns: per_call,
            fused: true,
        }
    }

    /// Computes the theoretical speedup of batching `n` calls vs individual calls.
    pub fn speedup_factor(&self, n: usize) -> f64 {
        if n == 0 {
            return 1.0;
        }
        let individual = self.single_call_overhead_ns * n as u64;
        let batched = self.batch_fixed_overhead_ns + self.batch_per_item_ns * n as u64;
        individual as f64 / batched as f64
    }
}

impl Default for BatchCallOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.6: Zero-copy verification
// ═══════════════════════════════════════════════════════════════════════

/// A simulated buffer with an address, used to verify zero-copy semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiBuffer {
    /// Simulated memory address of the data.
    pub address: u64,
    /// Size in bytes.
    pub size: usize,
    /// Owner tag (which side owns the buffer).
    pub owner: String,
}

/// Verifies that data was not copied during an FFI transfer by comparing
/// addresses before and after the boundary crossing.
#[derive(Debug, Clone)]
pub struct ZeroCopyVerifier {
    /// Log of verified transfers.
    transfers: Vec<ZeroCopyTransfer>,
}

/// A single zero-copy transfer record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroCopyTransfer {
    /// Label for the transfer.
    pub label: String,
    /// Address before crossing.
    pub address_before: u64,
    /// Address after crossing.
    pub address_after: u64,
    /// Whether the transfer was truly zero-copy.
    pub is_zero_copy: bool,
}

impl ZeroCopyVerifier {
    /// Creates a new verifier.
    pub fn new() -> Self {
        Self {
            transfers: Vec::new(),
        }
    }

    /// Verifies that a buffer's address is the same before and after crossing.
    pub fn verify(
        &mut self,
        label: impl Into<String>,
        before: &FfiBuffer,
        after: &FfiBuffer,
    ) -> Result<(), FfiSafetyError> {
        let label = label.into();
        let is_zero_copy = before.address == after.address;

        self.transfers.push(ZeroCopyTransfer {
            label: label.clone(),
            address_before: before.address,
            address_after: after.address,
            is_zero_copy,
        });

        if is_zero_copy {
            Ok(())
        } else {
            Err(FfiSafetyError::ZeroCopyAddressMismatch {
                before: before.address,
                after: after.address,
            })
        }
    }

    /// Returns all transfer records.
    pub fn transfers(&self) -> &[ZeroCopyTransfer] {
        &self.transfers
    }

    /// Returns how many transfers were truly zero-copy.
    pub fn zero_copy_count(&self) -> usize {
        self.transfers.iter().filter(|t| t.is_zero_copy).count()
    }

    /// Returns how many transfers involved a copy.
    pub fn copy_count(&self) -> usize {
        self.transfers.iter().filter(|t| !t.is_zero_copy).count()
    }
}

impl Default for ZeroCopyVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.7: Alignment handling
// ═══════════════════════════════════════════════════════════════════════

/// An alignment requirement for a type at the FFI boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignmentRequirement {
    /// Name of the type.
    pub type_name: String,
    /// Required alignment in bytes (must be a power of 2).
    pub required_alignment: usize,
    /// Actual alignment of the provided address.
    pub actual_alignment: usize,
}

/// Verifies that pointers/buffers satisfy alignment requirements.
#[derive(Debug, Clone)]
pub struct AlignmentChecker {
    /// Registered type -> required alignment.
    requirements: HashMap<String, usize>,
}

impl AlignmentChecker {
    /// Creates a new alignment checker with common defaults.
    pub fn new() -> Self {
        let mut requirements = HashMap::new();
        requirements.insert("i8".to_string(), 1);
        requirements.insert("i16".to_string(), 2);
        requirements.insert("i32".to_string(), 4);
        requirements.insert("i64".to_string(), 8);
        requirements.insert("f32".to_string(), 4);
        requirements.insert("f64".to_string(), 8);
        requirements.insert("ptr".to_string(), 8);
        requirements.insert("simd128".to_string(), 16);
        requirements.insert("simd256".to_string(), 32);
        Self { requirements }
    }

    /// Registers or overrides an alignment requirement.
    pub fn set_requirement(&mut self, type_name: impl Into<String>, alignment: usize) {
        self.requirements.insert(type_name.into(), alignment);
    }

    /// Returns the required alignment for a type, or `None` if unregistered.
    pub fn required_alignment(&self, type_name: &str) -> Option<usize> {
        self.requirements.get(type_name).copied()
    }

    /// Computes the actual alignment of an address (largest power of 2 dividing it).
    pub fn actual_alignment(address: u64) -> usize {
        if address == 0 {
            // Address 0 is trivially aligned to everything.
            return usize::MAX;
        }
        // Lowest set bit gives the alignment.
        1 << address.trailing_zeros()
    }

    /// Checks whether `address` satisfies the alignment for `type_name`.
    pub fn check(
        &self,
        type_name: &str,
        address: u64,
    ) -> Result<AlignmentRequirement, FfiSafetyError> {
        let required = self.requirements.get(type_name).copied().unwrap_or(1);
        let actual = Self::actual_alignment(address);

        let req = AlignmentRequirement {
            type_name: type_name.to_string(),
            required_alignment: required,
            actual_alignment: actual,
        };

        if actual >= required {
            Ok(req)
        } else {
            Err(FfiSafetyError::AlignmentViolation {
                address,
                required,
                actual,
            })
        }
    }

    /// Returns the number of registered alignment requirements.
    pub fn requirement_count(&self) -> usize {
        self.requirements.len()
    }
}

impl Default for AlignmentChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.8: Endianness handling
// ═══════════════════════════════════════════════════════════════════════

/// Byte order for cross-platform FFI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    /// Least significant byte first (x86, ARM default).
    Little,
    /// Most significant byte first (network byte order).
    Big,
}

impl fmt::Display for Endianness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => write!(f, "little-endian"),
            Self::Big => write!(f, "big-endian"),
        }
    }
}

/// Converts byte representations between endianness for FFI data exchange.
#[derive(Debug, Clone)]
pub struct EndiannessConverter {
    /// The native endianness of this platform.
    pub native: Endianness,
}

impl EndiannessConverter {
    /// Creates a converter. Detects native endianness at construction.
    pub fn new() -> Self {
        // Detect at runtime.
        let native = if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        };
        Self { native }
    }

    /// Creates a converter with an explicit native endianness (for testing).
    pub fn with_native(native: Endianness) -> Self {
        Self { native }
    }

    /// Converts a `u16` to bytes in the target endianness.
    pub fn u16_to_bytes(&self, value: u16, target: Endianness) -> [u8; 2] {
        match target {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }

    /// Reads a `u16` from bytes in a given source endianness.
    pub fn u16_from_bytes(&self, bytes: [u8; 2], source: Endianness) -> u16 {
        match source {
            Endianness::Little => u16::from_le_bytes(bytes),
            Endianness::Big => u16::from_be_bytes(bytes),
        }
    }

    /// Converts a `u32` to bytes in the target endianness.
    pub fn u32_to_bytes(&self, value: u32, target: Endianness) -> [u8; 4] {
        match target {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }

    /// Reads a `u32` from bytes in a given source endianness.
    pub fn u32_from_bytes(&self, bytes: [u8; 4], source: Endianness) -> u32 {
        match source {
            Endianness::Little => u32::from_le_bytes(bytes),
            Endianness::Big => u32::from_be_bytes(bytes),
        }
    }

    /// Converts a `u64` to bytes in the target endianness.
    pub fn u64_to_bytes(&self, value: u64, target: Endianness) -> [u8; 8] {
        match target {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }

    /// Reads a `u64` from bytes in a given source endianness.
    pub fn u64_from_bytes(&self, bytes: [u8; 8], source: Endianness) -> u64 {
        match source {
            Endianness::Little => u64::from_le_bytes(bytes),
            Endianness::Big => u64::from_be_bytes(bytes),
        }
    }

    /// Converts a `f32` to bytes in the target endianness.
    pub fn f32_to_bytes(&self, value: f32, target: Endianness) -> [u8; 4] {
        let bits = value.to_bits();
        self.u32_to_bytes(bits, target)
    }

    /// Reads a `f32` from bytes in a given source endianness.
    pub fn f32_from_bytes(&self, bytes: [u8; 4], source: Endianness) -> f32 {
        f32::from_bits(self.u32_from_bytes(bytes, source))
    }

    /// Swaps a buffer of 2-byte values from `source` endianness to `target`.
    pub fn swap_u16_buffer(&self, buf: &mut [u8], source: Endianness, target: Endianness) {
        if source == target || buf.len() < 2 {
            return;
        }
        for chunk in buf.chunks_exact_mut(2) {
            let val = self.u16_from_bytes([chunk[0], chunk[1]], source);
            let swapped = self.u16_to_bytes(val, target);
            chunk[0] = swapped[0];
            chunk[1] = swapped[1];
        }
    }

    /// Returns `true` if the native endianness matches the target.
    pub fn is_native(&self, target: Endianness) -> bool {
        self.native == target
    }
}

impl Default for EndiannessConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.9: Sanitizer integration
// ═══════════════════════════════════════════════════════════════════════

/// Which sanitizer to enable for FFI boundary checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sanitizer {
    /// AddressSanitizer — out-of-bounds, use-after-free, double-free.
    Asan,
    /// MemorySanitizer — reads of uninitialized memory.
    Msan,
    /// ThreadSanitizer — data races.
    Tsan,
    /// UndefinedBehaviorSanitizer — integer overflow, null deref, etc.
    Ubsan,
}

impl fmt::Display for Sanitizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asan => write!(f, "AddressSanitizer"),
            Self::Msan => write!(f, "MemorySanitizer"),
            Self::Tsan => write!(f, "ThreadSanitizer"),
            Self::Ubsan => write!(f, "UBSanitizer"),
        }
    }
}

/// A simulated sanitizer finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizerFinding {
    /// Which sanitizer produced the finding.
    pub sanitizer: Sanitizer,
    /// Severity (e.g. "error", "warning").
    pub severity: String,
    /// Human-readable description.
    pub message: String,
    /// Simulated source location.
    pub location: String,
}

/// Configuration and simulated output for sanitizer integration.
///
/// In production this would set compiler flags (`-fsanitize=address`, etc.)
/// and parse sanitizer output. Here we maintain a config and record findings.
#[derive(Debug, Clone)]
pub struct SanitizerConfig {
    /// Enabled sanitizers.
    pub enabled: Vec<Sanitizer>,
    /// Compiler flags that would be emitted.
    pub compiler_flags: Vec<String>,
    /// Recorded findings from simulated runs.
    findings: Vec<SanitizerFinding>,
}

impl SanitizerConfig {
    /// Creates a config with no sanitizers enabled.
    pub fn new() -> Self {
        Self {
            enabled: Vec::new(),
            compiler_flags: Vec::new(),
            findings: Vec::new(),
        }
    }

    /// Enables a sanitizer and records the appropriate compiler flag.
    pub fn enable(&mut self, sanitizer: Sanitizer) {
        if !self.enabled.contains(&sanitizer) {
            self.enabled.push(sanitizer);
            let flag = match sanitizer {
                Sanitizer::Asan => "-fsanitize=address",
                Sanitizer::Msan => "-fsanitize=memory",
                Sanitizer::Tsan => "-fsanitize=thread",
                Sanitizer::Ubsan => "-fsanitize=undefined",
            };
            self.compiler_flags.push(flag.to_string());
        }
    }

    /// Returns `true` if the given sanitizer is enabled.
    pub fn is_enabled(&self, sanitizer: Sanitizer) -> bool {
        self.enabled.contains(&sanitizer)
    }

    /// Records a simulated finding.
    pub fn record_finding(&mut self, finding: SanitizerFinding) {
        self.findings.push(finding);
    }

    /// Returns all recorded findings.
    pub fn findings(&self) -> &[SanitizerFinding] {
        &self.findings
    }

    /// Returns findings filtered by sanitizer.
    pub fn findings_for(&self, sanitizer: Sanitizer) -> Vec<&SanitizerFinding> {
        self.findings
            .iter()
            .filter(|f| f.sanitizer == sanitizer)
            .collect()
    }

    /// Returns the total number of findings.
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }

    /// Clears all findings.
    pub fn clear_findings(&mut self) {
        self.findings.clear();
    }

    /// Generates a summary of enabled sanitizers and finding counts.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for san in &self.enabled {
            let count = self.findings.iter().filter(|f| f.sanitizer == *san).count();
            parts.push(format!("{san}: {count} finding(s)"));
        }
        if parts.is_empty() {
            "No sanitizers enabled.".to_string()
        } else {
            parts.join(", ")
        }
    }
}

impl Default for SanitizerConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E9.10: Tests (15+ required)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── E9.1: Boundary validation ──

    #[test]
    fn e9_1_boundary_validator_valid_call() {
        let mut v = BoundaryValidator::new();
        v.register("add", vec![BoundaryType::Int, BoundaryType::Int]);

        let result = v.validate_call("add", &[BoundaryValue::Int(1), BoundaryValue::Int(2)]);
        assert!(result.is_ok());
    }

    #[test]
    fn e9_1_boundary_validator_type_mismatch() {
        let mut v = BoundaryValidator::new();
        v.register("greet", vec![BoundaryType::Str]);

        let result = v.validate_call("greet", &[BoundaryValue::Int(42)]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            FfiSafetyError::BoundaryTypeMismatch { position: 0, .. }
        ));
    }

    #[test]
    fn e9_1_boundary_validator_null_pointer_rejected() {
        let mut v = BoundaryValidator::new();
        v.register("deref", vec![BoundaryType::Pointer]);

        let result = v.validate_call("deref", &[BoundaryValue::Pointer(0)]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, FfiSafetyError::NullPointer { .. }))
        );
    }

    #[test]
    fn e9_1_boundary_validator_null_allowed() {
        let mut v = BoundaryValidator::new();
        v.allow_null = true;
        v.register("maybe_deref", vec![BoundaryType::Pointer]);

        let result = v.validate_call("maybe_deref", &[BoundaryValue::Pointer(0)]);
        assert!(result.is_ok());
    }

    #[test]
    fn e9_1_boundary_validator_arity_mismatch() {
        let mut v = BoundaryValidator::new();
        v.register("pair", vec![BoundaryType::Int, BoundaryType::Int]);

        let result = v.validate_call("pair", &[BoundaryValue::Int(1)]);
        assert!(result.is_err());
    }

    #[test]
    fn e9_1_boundary_validator_unknown_function() {
        let v = BoundaryValidator::new();
        let result = v.validate_call("nonexistent", &[]);
        assert!(result.is_err());
    }

    // ── E9.2: Leak detection ──

    #[test]
    fn e9_2_leak_detector_clean() {
        let mut ld = LeakDetector::new();
        let p = ld.alloc(64, "Buffer", "test.fj:10");
        assert_eq!(ld.active_count(), 1);
        assert!(ld.free(p));
        assert_eq!(ld.active_count(), 0);
        let report = ld.report();
        assert!(report.is_clean());
        assert_eq!(report.total_bytes, 0);
    }

    #[test]
    fn e9_2_leak_detector_reports_leaks() {
        let mut ld = LeakDetector::new();
        ld.alloc(128, "Image", "render.fj:5");
        ld.alloc(256, "Model", "ml.fj:20");
        let p3 = ld.alloc(32, "Temp", "util.fj:1");
        ld.free(p3);

        let report = ld.report();
        assert!(!report.is_clean());
        assert_eq!(report.leaked.len(), 2);
        assert_eq!(report.total_bytes, 128 + 256);
        assert_eq!(ld.stats(), (3, 1));
    }

    #[test]
    fn e9_2_leak_detector_double_free_returns_false() {
        let mut ld = LeakDetector::new();
        let p = ld.alloc(16, "Small", "test.fj:1");
        assert!(ld.free(p));
        assert!(!ld.free(p)); // double free
    }

    // ── E9.3: Thread safety ──

    #[test]
    fn e9_3_gil_acquire_release() {
        let mut checker = ThreadSafetyChecker::new();
        assert!(checker.acquire_gil("thread-1").is_ok());
        assert_eq!(checker.gil_holder(), Some("thread-1"));

        // Same thread re-acquires OK
        assert!(checker.acquire_gil("thread-1").is_ok());

        // Different thread fails
        let err = checker.acquire_gil("thread-2").unwrap_err();
        assert!(matches!(err, FfiSafetyError::GilViolation { .. }));

        checker.release_gil();
        assert!(checker.gil_holder().is_none());
        assert!(checker.acquire_gil("thread-2").is_ok());
    }

    #[test]
    fn e9_3_lock_ordering_violation() {
        let mut checker = ThreadSafetyChecker::new();
        checker.define_lock_order("mutex_a", 1);
        checker.define_lock_order("mutex_b", 2);

        // Correct order: a then b
        assert!(checker.acquire_lock("t1", "mutex_a").is_ok());
        assert!(checker.acquire_lock("t1", "mutex_b").is_ok());
        assert_eq!(checker.locks_held_by("t1"), 2);

        // Reverse order on another thread: b then a -> violation
        assert!(checker.acquire_lock("t2", "mutex_b").is_ok());
        let err = checker.acquire_lock("t2", "mutex_a").unwrap_err();
        assert!(matches!(err, FfiSafetyError::LockOrderViolation { .. }));
    }

    #[test]
    fn e9_3_lock_release() {
        let mut checker = ThreadSafetyChecker::new();
        checker.define_lock_order("m", 1);
        checker.acquire_lock("t1", "m").unwrap();
        assert_eq!(checker.locks_held_by("t1"), 1);
        checker.release_lock("t1", "m");
        assert_eq!(checker.locks_held_by("t1"), 0);
    }

    // ── E9.4: Overhead measurement ──

    #[test]
    fn e9_4_overhead_compute() {
        let mut overhead = FfiOverhead::new("ffi_call");
        overhead.record_batch(&[100, 200, 150, 300, 250]);

        let timings = overhead.compute().unwrap();
        assert_eq!(timings.min_ns, 100);
        assert_eq!(timings.max_ns, 300);
        assert_eq!(timings.avg_ns, 200); // (100+150+200+250+300)/5 = 200
        assert_eq!(timings.call_count, 5);
        // p99 for 5 samples: ceil(5*0.99)=5 -> index 4 -> 300
        assert_eq!(timings.p99_ns, 300);
    }

    #[test]
    fn e9_4_overhead_empty() {
        let overhead = FfiOverhead::new("empty");
        assert!(overhead.compute().is_none());
    }

    #[test]
    fn e9_4_overhead_display() {
        let timings = CallTimings {
            min_ns: 10,
            max_ns: 90,
            avg_ns: 50,
            p99_ns: 85,
            call_count: 100,
        };
        let s = format!("{timings}");
        assert!(s.contains("min=10ns"));
        assert!(s.contains("p99=85ns"));
    }

    // ── E9.5: Batch optimization ──

    #[test]
    fn e9_5_batch_optimizer_flush() {
        let mut opt = BatchCallOptimizer::new();
        for i in 0..10 {
            opt.enqueue("process", vec![BoundaryValue::Int(i)]);
        }
        assert_eq!(opt.pending_count(), 10);

        let result = opt.flush();
        assert_eq!(result.call_count, 10);
        assert!(result.fused);
        // Batched overhead should be less than 10 individual calls.
        let individual = opt.single_call_overhead_ns * 10;
        assert!(result.total_overhead_ns < individual);
        assert_eq!(opt.pending_count(), 0);
    }

    #[test]
    fn e9_5_batch_single_call_not_fused() {
        let mut opt = BatchCallOptimizer::new();
        opt.enqueue("single", vec![]);
        let result = opt.flush();
        assert_eq!(result.call_count, 1);
        assert!(!result.fused);
    }

    #[test]
    fn e9_5_batch_speedup_factor() {
        let opt = BatchCallOptimizer::new();
        let speedup = opt.speedup_factor(100);
        // 100 * 500 = 50000 vs 600 + 100*50 = 5600 -> ~8.9x
        assert!(speedup > 5.0);
    }

    // ── E9.6: Zero-copy verification ──

    #[test]
    fn e9_6_zero_copy_same_address() {
        let mut zc = ZeroCopyVerifier::new();
        let before = FfiBuffer {
            address: 0xDEAD,
            size: 1024,
            owner: "fajar".into(),
        };
        let after = FfiBuffer {
            address: 0xDEAD,
            size: 1024,
            owner: "c++".into(),
        };
        assert!(zc.verify("tensor_transfer", &before, &after).is_ok());
        assert_eq!(zc.zero_copy_count(), 1);
        assert_eq!(zc.copy_count(), 0);
    }

    #[test]
    fn e9_6_zero_copy_address_mismatch() {
        let mut zc = ZeroCopyVerifier::new();
        let before = FfiBuffer {
            address: 0x1000,
            size: 64,
            owner: "fajar".into(),
        };
        let after = FfiBuffer {
            address: 0x2000,
            size: 64,
            owner: "python".into(),
        };
        let err = zc.verify("bad_transfer", &before, &after).unwrap_err();
        assert!(matches!(
            err,
            FfiSafetyError::ZeroCopyAddressMismatch { .. }
        ));
        assert_eq!(zc.copy_count(), 1);
    }

    // ── E9.7: Alignment ──

    #[test]
    fn e9_7_alignment_checker_valid() {
        let checker = AlignmentChecker::new();
        // 0x1000 is aligned to 4096 bytes, satisfies i64 (8).
        let result = checker.check("i64", 0x1000).unwrap();
        assert_eq!(result.required_alignment, 8);
        assert!(result.actual_alignment >= 8);
    }

    #[test]
    fn e9_7_alignment_checker_violation() {
        let checker = AlignmentChecker::new();
        // 0x1003 has alignment 1 (odd address), i32 requires 4.
        let err = checker.check("i32", 0x1003).unwrap_err();
        assert!(matches!(
            err,
            FfiSafetyError::AlignmentViolation {
                required: 4,
                actual: 1,
                ..
            }
        ));
    }

    #[test]
    fn e9_7_alignment_actual_computation() {
        assert_eq!(AlignmentChecker::actual_alignment(0x0), usize::MAX);
        assert_eq!(AlignmentChecker::actual_alignment(0x8), 8);
        assert_eq!(AlignmentChecker::actual_alignment(0x10), 16);
        assert_eq!(AlignmentChecker::actual_alignment(0x3), 1);
        assert_eq!(AlignmentChecker::actual_alignment(0x6), 2);
    }

    #[test]
    fn e9_7_alignment_simd() {
        let checker = AlignmentChecker::new();
        // simd256 requires 32-byte alignment.
        let ok = checker.check("simd256", 0x20);
        assert!(ok.is_ok());
        let fail = checker.check("simd256", 0x18);
        assert!(fail.is_err());
    }

    // ── E9.8: Endianness ──

    #[test]
    fn e9_8_endianness_u32_roundtrip() {
        let conv = EndiannessConverter::with_native(Endianness::Little);
        let value: u32 = 0xDEAD_BEEF;

        let be_bytes = conv.u32_to_bytes(value, Endianness::Big);
        assert_eq!(be_bytes, [0xDE, 0xAD, 0xBE, 0xEF]);

        let le_bytes = conv.u32_to_bytes(value, Endianness::Little);
        assert_eq!(le_bytes, [0xEF, 0xBE, 0xAD, 0xDE]);

        assert_eq!(conv.u32_from_bytes(be_bytes, Endianness::Big), value);
        assert_eq!(conv.u32_from_bytes(le_bytes, Endianness::Little), value);
    }

    #[test]
    fn e9_8_endianness_u16_swap_buffer() {
        let conv = EndiannessConverter::with_native(Endianness::Little);
        let mut buf = vec![0x01, 0x00, 0x02, 0x00]; // LE: [1, 2]
        conv.swap_u16_buffer(&mut buf, Endianness::Little, Endianness::Big);
        assert_eq!(buf, vec![0x00, 0x01, 0x00, 0x02]); // BE: [1, 2]
    }

    #[test]
    fn e9_8_endianness_f32_roundtrip() {
        let conv = EndiannessConverter::new();
        let val: f32 = 3.14;
        let bytes = conv.f32_to_bytes(val, Endianness::Big);
        let restored = conv.f32_from_bytes(bytes, Endianness::Big);
        assert!((restored - val).abs() < 1e-6);
    }

    #[test]
    fn e9_8_endianness_is_native() {
        let conv = EndiannessConverter::with_native(Endianness::Little);
        assert!(conv.is_native(Endianness::Little));
        assert!(!conv.is_native(Endianness::Big));
    }

    // ── E9.9: Sanitizer integration ──

    #[test]
    fn e9_9_sanitizer_config_enable() {
        let mut config = SanitizerConfig::new();
        config.enable(Sanitizer::Asan);
        config.enable(Sanitizer::Tsan);
        assert!(config.is_enabled(Sanitizer::Asan));
        assert!(config.is_enabled(Sanitizer::Tsan));
        assert!(!config.is_enabled(Sanitizer::Msan));
        assert_eq!(config.compiler_flags.len(), 2);
        assert!(
            config
                .compiler_flags
                .contains(&"-fsanitize=address".to_string())
        );
        assert!(
            config
                .compiler_flags
                .contains(&"-fsanitize=thread".to_string())
        );
    }

    #[test]
    fn e9_9_sanitizer_findings() {
        let mut config = SanitizerConfig::new();
        config.enable(Sanitizer::Asan);
        config.record_finding(SanitizerFinding {
            sanitizer: Sanitizer::Asan,
            severity: "error".to_string(),
            message: "heap-buffer-overflow".to_string(),
            location: "ffi_bridge.c:42".to_string(),
        });
        config.record_finding(SanitizerFinding {
            sanitizer: Sanitizer::Asan,
            severity: "warning".to_string(),
            message: "use-after-free".to_string(),
            location: "ffi_bridge.c:58".to_string(),
        });
        assert_eq!(config.finding_count(), 2);
        assert_eq!(config.findings_for(Sanitizer::Asan).len(), 2);
        assert_eq!(config.findings_for(Sanitizer::Tsan).len(), 0);
    }

    #[test]
    fn e9_9_sanitizer_summary() {
        let mut config = SanitizerConfig::new();
        config.enable(Sanitizer::Msan);
        assert!(config.summary().contains("MemorySanitizer: 0 finding(s)"));
    }

    #[test]
    fn e9_9_sanitizer_no_duplicate_enable() {
        let mut config = SanitizerConfig::new();
        config.enable(Sanitizer::Ubsan);
        config.enable(Sanitizer::Ubsan);
        assert_eq!(config.enabled.len(), 1);
        assert_eq!(config.compiler_flags.len(), 1);
    }

    // ── Error display ──

    #[test]
    fn e9_10_error_display_coverage() {
        let e1 = FfiSafetyError::BoundaryTypeMismatch {
            position: 0,
            expected: "i64".into(),
            got: "str".into(),
        };
        assert!(format!("{e1}").contains("expected i64"));

        let e2 = FfiSafetyError::NullPointer {
            param: "buf".into(),
        };
        assert!(format!("{e2}").contains("null pointer"));

        let e3 = FfiSafetyError::MemoryLeak {
            ptr: 0x1000,
            size: 64,
            type_name: "Buffer".into(),
        };
        assert!(format!("{e3}").contains("memory leak"));

        let e4 = FfiSafetyError::Other("custom".into());
        assert_eq!(format!("{e4}"), "custom");
    }

    #[test]
    fn e9_10_leak_report_display() {
        let report = LeakReport {
            leaked: vec![FfiAllocation {
                ptr: 0xBEEF,
                size: 256,
                type_name: "Tensor".into(),
                allocated_at: "model.fj:10".into(),
            }],
            total_bytes: 256,
        };
        let s = format!("{report}");
        assert!(s.contains("LEAK REPORT"));
        assert!(s.contains("256 bytes"));
        assert!(s.contains("Tensor"));
    }

    #[test]
    fn e9_10_boundary_value_runtime_type() {
        assert_eq!(BoundaryValue::Int(1).runtime_type(), BoundaryType::Int);
        assert_eq!(
            BoundaryValue::Float(1.0).runtime_type(),
            BoundaryType::Float
        );
        assert_eq!(
            BoundaryValue::Str("hi".into()).runtime_type(),
            BoundaryType::Str
        );
        assert_eq!(BoundaryValue::Bool(true).runtime_type(), BoundaryType::Bool);
        assert_eq!(
            BoundaryValue::Pointer(0x10).runtime_type(),
            BoundaryType::Pointer
        );
        let arr = BoundaryValue::Array(vec![BoundaryValue::Int(1)]);
        assert_eq!(
            arr.runtime_type(),
            BoundaryType::Array(Box::new(BoundaryType::Int))
        );
    }
}
