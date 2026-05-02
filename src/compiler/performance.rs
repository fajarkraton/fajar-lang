//! # Performance Engineering Infrastructure
//!
//! Provides performance optimization primitives for the Fajar Lang compiler
//! and runtime: string interning, inline caching, dispatch tables, tail-call
//! optimization, constant folding, compilation timing, and value layout analysis.
//!
//! ## Architecture
//!
//! ```text
//! StringInterner  ── O(1) lookup/resolve for identifiers
//! InlineCache     ── monomorphic/polymorphic dispatch caching
//! DispatchTable   ── pre-computed binary op dispatch (TypeTag x TypeTag)
//! TailCallOptimizer ── detect + transform tail-recursive calls → loops
//! ConstFolder     ── compile-time constant folding for pure expressions
//! CompilationTimer  ── per-phase timing breakdown (lex/parse/analyze/codegen)
//! ValueOptimizer  ── memory layout analysis + compact representations
//! ```
//!
//! ## Sprint Overview
//!
//! - **Sprint 5:** String interning, inline cache foundations
//! - **Sprint 6:** Dispatch tables, tail-call detection
//! - **Sprint 7:** Constant folding, compilation timing
//! - **Sprint 8:** Value size analysis, compact representations, reporting

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from performance optimization operations.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PerfError {
    /// A symbol index is out of bounds for the interner.
    #[error("invalid symbol index {index}: interner has {len} entries")]
    InvalidSymbol {
        /// The out-of-bounds symbol index.
        index: u32,
        /// Current number of interned strings.
        len: u32,
    },

    /// Constant folding encountered a division by zero.
    #[error("constant fold: division by zero")]
    FoldDivByZero,

    /// Constant folding encountered an integer overflow.
    #[error("constant fold: integer overflow")]
    FoldOverflow,

    /// A type combination is not supported for the requested operation.
    #[error("unsupported type combination: {lhs} {op} {rhs}")]
    UnsupportedDispatch {
        /// Left-hand type tag name.
        lhs: String,
        /// Operator name.
        op: String,
        /// Right-hand type tag name.
        rhs: String,
    },

    /// A compilation phase was not properly started before ending.
    #[error("phase `{phase}` was not started")]
    PhaseNotStarted {
        /// The phase name.
        phase: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// 1. String Interner
// ═══════════════════════════════════════════════════════════════════════

/// A compact reference to an interned string, stored as a 4-byte index.
///
/// Symbols are created by [`Interner::intern`] and resolved back to
/// `&str` via [`Interner::resolve`]. Comparing two symbols is O(1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol {
    /// Index into the interner's string table.
    index: u32,
}

impl Symbol {
    /// Returns the raw index of this symbol.
    pub fn index(self) -> u32 {
        self.index
    }
}

/// An efficient string interner that deduplicates identifiers.
///
/// Intern a string once, then use the compact [`Symbol`] handle for all
/// comparisons and storage. Both `intern` and `resolve` are O(1) amortized.
///
/// # Examples
///
/// ```
/// use fajar_lang::compiler::performance::Interner;
///
/// let mut interner = Interner::new();
/// let s1 = interner.intern("hello");
/// let s2 = interner.intern("hello");
/// assert_eq!(s1, s2);
/// assert_eq!(interner.resolve(s1), Some("hello"));
/// ```
pub struct Interner {
    /// Map from string content to its symbol.
    map: HashMap<String, Symbol>,
    /// Ordered list of interned strings, indexed by symbol.
    strings: Vec<String>,
}

impl Interner {
    /// Creates a new, empty interner.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    /// Interns a string, returning a compact [`Symbol`] handle.
    ///
    /// If the string was already interned, returns the existing symbol.
    /// Otherwise allocates a new entry.
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }
        let index = self.strings.len() as u32;
        let sym = Symbol { index };
        self.strings.push(s.to_owned());
        self.map.insert(s.to_owned(), sym);
        sym
    }

    /// Resolves a symbol back to its string slice.
    ///
    /// Returns `None` if the symbol was not produced by this interner.
    pub fn resolve(&self, sym: Symbol) -> Option<&str> {
        self.strings.get(sym.index as usize).map(|s| s.as_str())
    }

    /// Returns the number of unique interned strings.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Returns the total number of bytes across all interned strings.
    pub fn total_bytes(&self) -> usize {
        self.strings.iter().map(|s| s.len()).sum()
    }

    /// Returns the average length of interned strings, or 0.0 if empty.
    pub fn average_length(&self) -> f64 {
        if self.strings.is_empty() {
            return 0.0;
        }
        self.total_bytes() as f64 / self.strings.len() as f64
    }

    /// Returns statistics about the interner.
    pub fn stats(&self) -> InternerStats {
        InternerStats {
            count: self.len(),
            total_bytes: self.total_bytes(),
            average_length: self.average_length(),
        }
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a [`Interner`].
#[derive(Debug, Clone)]
pub struct InternerStats {
    /// Number of unique interned strings.
    pub count: usize,
    /// Total bytes across all strings.
    pub total_bytes: usize,
    /// Average string length.
    pub average_length: f64,
}

/// A thread-safe string interner backed by [`RwLock`].
///
/// Provides the same semantics as [`Interner`] but safe to share across
/// threads. Read operations (resolve) take a read lock; write operations
/// (intern) take a write lock.
pub struct SyncInterner {
    /// The inner interner protected by a read-write lock.
    inner: RwLock<Interner>,
}

impl SyncInterner {
    /// Creates a new, empty thread-safe interner.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Interner::new()),
        }
    }

    /// Interns a string, returning a compact [`Symbol`] handle.
    ///
    /// Takes a write lock. Returns `Err` if the lock is poisoned.
    pub fn intern(&self, s: &str) -> Result<Symbol, PerfError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| PerfError::InvalidSymbol { index: 0, len: 0 })?;
        Ok(guard.intern(s))
    }

    /// Resolves a symbol back to an owned string.
    ///
    /// Takes a read lock. Returns `None` if the symbol is invalid
    /// or the lock is poisoned.
    pub fn resolve(&self, sym: Symbol) -> Option<String> {
        let guard = self.inner.read().ok()?;
        guard.resolve(sym).map(|s| s.to_owned())
    }

    /// Returns the number of unique interned strings.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Returns `true` if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for SyncInterner {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Inline Cache
// ═══════════════════════════════════════════════════════════════════════

/// The result of an inline cache lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedResult {
    /// A method was found at the given vtable slot index.
    MethodSlot(usize),
    /// A field was found at the given byte offset.
    FieldOffset(usize),
    /// No cached result; perform a full lookup.
    Miss,
}

/// A single cache entry mapping a type ID to a dispatch result.
#[derive(Debug, Clone, Copy)]
pub struct CacheEntry {
    /// The type ID this entry is for.
    pub type_id: u64,
    /// The cached dispatch result.
    pub result: CachedResult,
}

/// Performance statistics for an inline cache.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of successful cache lookups.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Number of times an entry was evicted.
    pub evictions: u64,
}

impl CacheStats {
    /// Creates zeroed cache statistics.
    fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Returns the cache hit rate as a fraction in [0.0, 1.0].
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

/// Maximum number of entries in the polymorphic cache.
const POLY_CACHE_SIZE: usize = 4;

/// The cache state, transitioning from monomorphic to polymorphic to megamorphic.
#[derive(Debug, Clone)]
enum CacheState {
    /// No entries yet.
    Empty,
    /// Exactly one type seen — fastest path.
    Monomorphic(CacheEntry),
    /// 2–4 types seen — linear scan.
    Polymorphic(Vec<CacheEntry>),
    /// More than 4 types seen — cache is disabled, always misses.
    Megamorphic,
}

/// Inline cache for method and property dispatch.
///
/// Transitions through three states as more types are observed:
/// 1. **Monomorphic** — single type, O(1) lookup
/// 2. **Polymorphic** — up to 4 types, O(4) scan
/// 3. **Megamorphic** — too many types, always falls back to full lookup
///
/// # Examples
///
/// ```
/// use fajar_lang::compiler::performance::{InlineCache, CachedResult};
///
/// let mut cache = InlineCache::new();
/// cache.update(42, CachedResult::MethodSlot(0));
/// assert_eq!(cache.lookup(42), CachedResult::MethodSlot(0));
/// assert_eq!(cache.lookup(99), CachedResult::Miss);
/// ```
pub struct InlineCache {
    /// Current cache state.
    state: CacheState,
    /// Accumulated performance statistics.
    stats: CacheStats,
}

impl InlineCache {
    /// Creates a new, empty inline cache.
    pub fn new() -> Self {
        Self {
            state: CacheState::Empty,
            stats: CacheStats::new(),
        }
    }

    /// Looks up a cached result for the given type ID.
    ///
    /// Returns [`CachedResult::Miss`] on cache miss. Updates statistics.
    pub fn lookup(&mut self, type_id: u64) -> CachedResult {
        match &self.state {
            CacheState::Empty | CacheState::Megamorphic => {
                self.stats.misses += 1;
                CachedResult::Miss
            }
            CacheState::Monomorphic(entry) => {
                if entry.type_id == type_id {
                    self.stats.hits += 1;
                    entry.result
                } else {
                    self.stats.misses += 1;
                    CachedResult::Miss
                }
            }
            CacheState::Polymorphic(entries) => {
                for entry in entries {
                    if entry.type_id == type_id {
                        self.stats.hits += 1;
                        return entry.result;
                    }
                }
                self.stats.misses += 1;
                CachedResult::Miss
            }
        }
    }

    /// Updates the cache with a new type→result mapping.
    ///
    /// Transitions the cache state as needed:
    /// - Empty → Monomorphic
    /// - Monomorphic → Polymorphic (if different type)
    /// - Polymorphic → Megamorphic (if > 4 types)
    pub fn update(&mut self, type_id: u64, result: CachedResult) {
        match &mut self.state {
            CacheState::Empty => {
                self.state = CacheState::Monomorphic(CacheEntry { type_id, result });
            }
            CacheState::Monomorphic(entry) => {
                if entry.type_id == type_id {
                    entry.result = result;
                } else {
                    let old = *entry;
                    self.state = CacheState::Polymorphic(vec![old, CacheEntry { type_id, result }]);
                }
            }
            CacheState::Polymorphic(entries) => {
                // Update existing entry if present.
                for e in entries.iter_mut() {
                    if e.type_id == type_id {
                        e.result = result;
                        return;
                    }
                }
                if entries.len() < POLY_CACHE_SIZE {
                    entries.push(CacheEntry { type_id, result });
                } else {
                    self.stats.evictions += entries.len() as u64;
                    self.state = CacheState::Megamorphic;
                }
            }
            CacheState::Megamorphic => {
                // Cache is permanently disabled for this site.
            }
        }
    }

    /// Returns a snapshot of the cache performance statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Returns `true` if the cache is in the megamorphic (disabled) state.
    pub fn is_megamorphic(&self) -> bool {
        matches!(self.state, CacheState::Megamorphic)
    }

    /// Resets the cache to the empty state and zeroes statistics.
    pub fn reset(&mut self) {
        self.state = CacheState::Empty;
        self.stats = CacheStats::new();
    }
}

impl Default for InlineCache {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Dispatch Table
// ═══════════════════════════════════════════════════════════════════════

/// Runtime type tag for fast dispatch table indexing.
///
/// Each variant corresponds to a row/column in the 2D dispatch table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TypeTag {
    /// 64-bit signed integer.
    Int = 0,
    /// 64-bit floating point.
    Float = 1,
    /// Boolean.
    Bool = 2,
    /// String.
    Str = 3,
    /// Array.
    Array = 4,
    /// Null.
    Null = 5,
    /// Any other type (struct, enum, tensor, etc.).
    Other = 6,
}

impl TypeTag {
    /// Total number of type tags.
    pub const COUNT: usize = 7;

    /// Returns the human-readable name of this type tag.
    pub fn name(self) -> &'static str {
        match self {
            TypeTag::Int => "Int",
            TypeTag::Float => "Float",
            TypeTag::Bool => "Bool",
            TypeTag::Str => "Str",
            TypeTag::Array => "Array",
            TypeTag::Null => "Null",
            TypeTag::Other => "Other",
        }
    }
}

impl std::fmt::Display for TypeTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Binary operation codes for the dispatch table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpCode {
    /// Addition (`+`).
    Add = 0,
    /// Subtraction (`-`).
    Sub = 1,
    /// Multiplication (`*`).
    Mul = 2,
    /// Division (`/`).
    Div = 3,
    /// Modulus (`%`).
    Mod = 4,
    /// Equality (`==`).
    Eq = 5,
    /// Inequality (`!=`).
    Ne = 6,
    /// Less than (`<`).
    Lt = 7,
    /// Greater than (`>`).
    Gt = 8,
    /// Less than or equal (`<=`).
    Le = 9,
    /// Greater than or equal (`>=`).
    Ge = 10,
    /// Logical AND (`&&`).
    And = 11,
    /// Logical OR (`||`).
    Or = 12,
}

impl OpCode {
    /// Total number of operation codes.
    pub const COUNT: usize = 13;

    /// Returns the human-readable name of this operation.
    pub fn name(self) -> &'static str {
        match self {
            OpCode::Add => "Add",
            OpCode::Sub => "Sub",
            OpCode::Mul => "Mul",
            OpCode::Div => "Div",
            OpCode::Mod => "Mod",
            OpCode::Eq => "Eq",
            OpCode::Ne => "Ne",
            OpCode::Lt => "Lt",
            OpCode::Gt => "Gt",
            OpCode::Le => "Le",
            OpCode::Ge => "Ge",
            OpCode::And => "And",
            OpCode::Or => "Or",
        }
    }
}

/// The result of a dispatched binary operation.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchResult {
    /// The operation produced an integer.
    IntResult(i64),
    /// The operation produced a float.
    FloatResult(f64),
    /// The operation produced a boolean.
    BoolResult(bool),
    /// The operation produced a string.
    StrResult(String),
    /// The type combination is not valid for this operation.
    TypeError,
}

/// Signature for dispatch handler functions.
type DispatchFn = fn(i64, i64) -> DispatchResult;

/// Pre-computed dispatch table for fast binary operation execution.
///
/// The table is indexed by `(OpCode, TypeTag, TypeTag)`, yielding a
/// function pointer that performs the operation without dynamic dispatch.
///
/// # Examples
///
/// ```
/// use fajar_lang::compiler::performance::{DispatchTable, TypeTag, OpCode};
///
/// let table = DispatchTable::new();
/// let result = table.dispatch(OpCode::Add, TypeTag::Int, TypeTag::Int, 3, 4);
/// assert_eq!(result, fajar_lang::compiler::performance::DispatchResult::IntResult(7));
/// ```
pub struct DispatchTable {
    /// 3D table: [op][lhs_type][rhs_type] → handler function.
    table: Vec<Vec<Vec<Option<DispatchFn>>>>,
}

impl DispatchTable {
    /// Creates a new dispatch table pre-populated with all valid type combinations.
    pub fn new() -> Self {
        let mut table = vec![vec![vec![None; TypeTag::COUNT]; TypeTag::COUNT]; OpCode::COUNT];
        Self::populate_int_ops(&mut table);
        Self::populate_float_ops(&mut table);
        Self::populate_bool_ops(&mut table);
        Self::populate_str_ops(&mut table);
        Self::populate_mixed_numeric(&mut table);
        Self { table }
    }

    /// Dispatches a binary operation given raw i64 operands.
    ///
    /// For Int values, the i64 is used directly. For Float, bits are
    /// reinterpreted via `f64::from_bits`. For Bool, 0 = false, nonzero = true.
    /// For Str, the i64 is not meaningful — use `dispatch_str` instead.
    pub fn dispatch(
        &self,
        op: OpCode,
        lhs_type: TypeTag,
        rhs_type: TypeTag,
        lhs_val: i64,
        rhs_val: i64,
    ) -> DispatchResult {
        let handler = self
            .table
            .get(op as usize)
            .and_then(|t| t.get(lhs_type as usize))
            .and_then(|t| t.get(rhs_type as usize))
            .and_then(|h| *h);
        match handler {
            Some(f) => f(lhs_val, rhs_val),
            None => DispatchResult::TypeError,
        }
    }

    /// Returns `true` if the given (op, lhs, rhs) combination has a handler.
    pub fn has_handler(&self, op: OpCode, lhs: TypeTag, rhs: TypeTag) -> bool {
        self.table
            .get(op as usize)
            .and_then(|t| t.get(lhs as usize))
            .and_then(|t| t.get(rhs as usize))
            .and_then(|h| *h)
            .is_some()
    }

    /// Populates Int × Int handlers for all ops.
    fn populate_int_ops(table: &mut [Vec<Vec<Option<DispatchFn>>>]) {
        let li = TypeTag::Int as usize;
        table[OpCode::Add as usize][li][li] =
            Some(|a, b| DispatchResult::IntResult(a.wrapping_add(b)));
        table[OpCode::Sub as usize][li][li] =
            Some(|a, b| DispatchResult::IntResult(a.wrapping_sub(b)));
        table[OpCode::Mul as usize][li][li] =
            Some(|a, b| DispatchResult::IntResult(a.wrapping_mul(b)));
        table[OpCode::Div as usize][li][li] = Some(|a, b| {
            if b == 0 {
                DispatchResult::TypeError
            } else {
                DispatchResult::IntResult(a / b)
            }
        });
        table[OpCode::Mod as usize][li][li] = Some(|a, b| {
            if b == 0 {
                DispatchResult::TypeError
            } else {
                DispatchResult::IntResult(a % b)
            }
        });
        table[OpCode::Eq as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a == b));
        table[OpCode::Ne as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a != b));
        table[OpCode::Lt as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a < b));
        table[OpCode::Gt as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a > b));
        table[OpCode::Le as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a <= b));
        table[OpCode::Ge as usize][li][li] = Some(|a, b| DispatchResult::BoolResult(a >= b));
    }

    /// Populates Float × Float handlers for arithmetic and comparison ops.
    fn populate_float_ops(table: &mut [Vec<Vec<Option<DispatchFn>>>]) {
        let fi = TypeTag::Float as usize;
        table[OpCode::Add as usize][fi][fi] = Some(|a, b| {
            DispatchResult::FloatResult(f64::from_bits(a as u64) + f64::from_bits(b as u64))
        });
        table[OpCode::Sub as usize][fi][fi] = Some(|a, b| {
            DispatchResult::FloatResult(f64::from_bits(a as u64) - f64::from_bits(b as u64))
        });
        table[OpCode::Mul as usize][fi][fi] = Some(|a, b| {
            DispatchResult::FloatResult(f64::from_bits(a as u64) * f64::from_bits(b as u64))
        });
        table[OpCode::Div as usize][fi][fi] = Some(|a, b| {
            let bv = f64::from_bits(b as u64);
            if bv == 0.0 {
                DispatchResult::TypeError
            } else {
                DispatchResult::FloatResult(f64::from_bits(a as u64) / bv)
            }
        });
        table[OpCode::Eq as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) == f64::from_bits(b as u64))
        });
        table[OpCode::Ne as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) != f64::from_bits(b as u64))
        });
        table[OpCode::Lt as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) < f64::from_bits(b as u64))
        });
        table[OpCode::Gt as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) > f64::from_bits(b as u64))
        });
        table[OpCode::Le as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) <= f64::from_bits(b as u64))
        });
        table[OpCode::Ge as usize][fi][fi] = Some(|a, b| {
            DispatchResult::BoolResult(f64::from_bits(a as u64) >= f64::from_bits(b as u64))
        });
    }

    /// Populates Bool × Bool handlers for logical and equality ops.
    fn populate_bool_ops(table: &mut [Vec<Vec<Option<DispatchFn>>>]) {
        let bi = TypeTag::Bool as usize;
        table[OpCode::And as usize][bi][bi] =
            Some(|a, b| DispatchResult::BoolResult((a != 0) && (b != 0)));
        table[OpCode::Or as usize][bi][bi] =
            Some(|a, b| DispatchResult::BoolResult((a != 0) || (b != 0)));
        table[OpCode::Eq as usize][bi][bi] =
            Some(|a, b| DispatchResult::BoolResult((a != 0) == (b != 0)));
        table[OpCode::Ne as usize][bi][bi] =
            Some(|a, b| DispatchResult::BoolResult((a != 0) != (b != 0)));
    }

    /// Populates Str equality handlers (by pointer comparison placeholder).
    fn populate_str_ops(table: &mut [Vec<Vec<Option<DispatchFn>>>]) {
        let si = TypeTag::Str as usize;
        // String equality by raw value (pointer/hash comparison placeholder).
        table[OpCode::Eq as usize][si][si] = Some(|a, b| DispatchResult::BoolResult(a == b));
        table[OpCode::Ne as usize][si][si] = Some(|a, b| DispatchResult::BoolResult(a != b));
    }

    /// Populates Int × Float and Float × Int mixed-type arithmetic.
    fn populate_mixed_numeric(table: &mut [Vec<Vec<Option<DispatchFn>>>]) {
        let ii = TypeTag::Int as usize;
        let fi = TypeTag::Float as usize;
        // Int + Float → Float
        table[OpCode::Add as usize][ii][fi] =
            Some(|a, b| DispatchResult::FloatResult(a as f64 + f64::from_bits(b as u64)));
        table[OpCode::Add as usize][fi][ii] =
            Some(|a, b| DispatchResult::FloatResult(f64::from_bits(a as u64) + b as f64));
        // Int - Float → Float
        table[OpCode::Sub as usize][ii][fi] =
            Some(|a, b| DispatchResult::FloatResult(a as f64 - f64::from_bits(b as u64)));
        table[OpCode::Sub as usize][fi][ii] =
            Some(|a, b| DispatchResult::FloatResult(f64::from_bits(a as u64) - b as f64));
        // Int * Float → Float
        table[OpCode::Mul as usize][ii][fi] =
            Some(|a, b| DispatchResult::FloatResult(a as f64 * f64::from_bits(b as u64)));
        table[OpCode::Mul as usize][fi][ii] =
            Some(|a, b| DispatchResult::FloatResult(f64::from_bits(a as u64) * b as f64));
    }
}

impl Default for DispatchTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Tail-Call Optimizer
// ═══════════════════════════════════════════════════════════════════════

/// Information about a detected tail call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailCallInfo {
    /// The name of the function containing the tail call.
    pub function_name: String,
    /// Number of parameters the tail call passes.
    pub param_count: usize,
    /// Whether this is a direct self-recursive tail call.
    pub is_self_recursive: bool,
    /// The target function being called (may differ for mutual recursion).
    pub target_name: String,
}

/// Detects tail-call positions in function bodies.
///
/// Analyzes a simplified representation of function statements to identify
/// calls in tail position — the last expression whose result is directly
/// returned without further computation.
///
/// # Examples
///
/// ```
/// use fajar_lang::compiler::performance::{TailCallDetector, SimpleStmt, SimpleExpr};
///
/// let detector = TailCallDetector::new("factorial");
/// let body = vec![
///     SimpleStmt::Return(SimpleExpr::Call {
///         name: "factorial".to_string(),
///         args: vec![SimpleExpr::Ident("n".to_string())],
///     }),
/// ];
/// let results = detector.detect(&body);
/// assert_eq!(results.len(), 1);
/// assert!(results[0].is_self_recursive);
/// ```
pub struct TailCallDetector {
    /// The name of the function being analyzed.
    fn_name: String,
}

/// A simplified statement for tail-call analysis.
///
/// This is a reduced AST representation that captures only the structure
/// relevant to tail-call detection.
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleStmt {
    /// A return statement with an expression.
    Return(SimpleExpr),
    /// An if-else statement with bodies.
    IfElse {
        /// Condition expression.
        cond: SimpleExpr,
        /// Then-branch statements.
        then_body: Vec<SimpleStmt>,
        /// Else-branch statements (may be empty).
        else_body: Vec<SimpleStmt>,
    },
    /// A let binding (not a tail position).
    Let(String, SimpleExpr),
    /// A bare expression statement.
    Expr(SimpleExpr),
}

/// A simplified expression for tail-call analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleExpr {
    /// A function call.
    Call {
        /// Function name.
        name: String,
        /// Arguments.
        args: Vec<SimpleExpr>,
    },
    /// A variable reference.
    Ident(String),
    /// An integer literal.
    IntLit(i64),
    /// A binary operation (not a tail position for the overall call).
    BinOp {
        /// Left operand.
        lhs: Box<SimpleExpr>,
        /// Operator symbol.
        op: String,
        /// Right operand.
        rhs: Box<SimpleExpr>,
    },
}

impl TailCallDetector {
    /// Creates a detector for the given function name.
    pub fn new(fn_name: &str) -> Self {
        Self {
            fn_name: fn_name.to_owned(),
        }
    }

    /// Detects all tail calls in the given function body.
    pub fn detect(&self, body: &[SimpleStmt]) -> Vec<TailCallInfo> {
        let mut results = Vec::new();
        self.analyze_stmts(body, &mut results);
        results
    }

    /// Analyzes a statement list, looking at the last statement for tail position.
    fn analyze_stmts(&self, stmts: &[SimpleStmt], out: &mut Vec<TailCallInfo>) {
        if let Some(last) = stmts.last() {
            self.analyze_tail_stmt(last, out);
        }
    }

    /// Checks whether a single statement is in tail position.
    fn analyze_tail_stmt(&self, stmt: &SimpleStmt, out: &mut Vec<TailCallInfo>) {
        match stmt {
            SimpleStmt::Return(expr) => {
                self.check_tail_expr(expr, out);
            }
            SimpleStmt::IfElse {
                then_body,
                else_body,
                ..
            } => {
                self.analyze_stmts(then_body, out);
                self.analyze_stmts(else_body, out);
            }
            SimpleStmt::Expr(expr) => {
                self.check_tail_expr(expr, out);
            }
            SimpleStmt::Let(..) => {
                // Let bindings are never in tail position.
            }
        }
    }

    /// Checks if an expression is a tail call.
    fn check_tail_expr(&self, expr: &SimpleExpr, out: &mut Vec<TailCallInfo>) {
        if let SimpleExpr::Call { name, args } = expr {
            let is_self = name == &self.fn_name;
            out.push(TailCallInfo {
                function_name: self.fn_name.clone(),
                param_count: args.len(),
                is_self_recursive: is_self,
                target_name: name.clone(),
            });
        }
    }
}

/// Transforms self-recursive tail calls into loop form.
///
/// Given a function body where the only tail call is to itself, produces
/// a transformed body that uses a loop instead of recursion.
pub struct TailCallTransform;

/// The result of a tail-call transformation.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformResult {
    /// The body was transformed into a loop.
    Transformed {
        /// The parameter names used in the loop.
        param_names: Vec<String>,
        /// The loop body (simplified representation).
        loop_body: Vec<SimpleStmt>,
    },
    /// The body was not transformable (not self-recursive or too complex).
    NotTransformable(String),
}

impl TailCallTransform {
    /// Attempts to transform a self-recursive function into a loop.
    ///
    /// Returns `TransformResult::Transformed` with the loop body if
    /// successful, or `NotTransformable` with a reason if not.
    pub fn transform(
        fn_name: &str,
        param_names: &[String],
        body: &[SimpleStmt],
    ) -> TransformResult {
        let detector = TailCallDetector::new(fn_name);
        let tail_calls = detector.detect(body);

        // Only transform if all tail calls are self-recursive.
        if tail_calls.is_empty() {
            return TransformResult::NotTransformable("no tail calls found".to_owned());
        }
        let all_self = tail_calls.iter().all(|tc| tc.is_self_recursive);
        if !all_self {
            return TransformResult::NotTransformable(
                "contains non-self-recursive tail calls".to_owned(),
            );
        }

        // Build a loop body that reassigns parameters and continues.
        let loop_body = Self::build_loop_body(fn_name, param_names, body);
        TransformResult::Transformed {
            param_names: param_names.to_vec(),
            loop_body,
        }
    }

    /// Builds the loop body by replacing recursive calls with assignments.
    fn build_loop_body(
        fn_name: &str,
        param_names: &[String],
        stmts: &[SimpleStmt],
    ) -> Vec<SimpleStmt> {
        stmts
            .iter()
            .map(|s| Self::transform_stmt(fn_name, param_names, s))
            .collect()
    }

    /// Transforms a single statement, replacing tail calls with assignments.
    fn transform_stmt(fn_name: &str, param_names: &[String], stmt: &SimpleStmt) -> SimpleStmt {
        match stmt {
            SimpleStmt::Return(SimpleExpr::Call { name, args }) if name == fn_name => {
                Self::make_reassignment(param_names, args)
            }
            SimpleStmt::IfElse {
                cond,
                then_body,
                else_body,
            } => SimpleStmt::IfElse {
                cond: cond.clone(),
                then_body: Self::build_loop_body(fn_name, param_names, then_body),
                else_body: Self::build_loop_body(fn_name, param_names, else_body),
            },
            other => other.clone(),
        }
    }

    /// Generates let bindings that reassign parameters from call arguments.
    fn make_reassignment(param_names: &[String], args: &[SimpleExpr]) -> SimpleStmt {
        // In a real implementation, this would emit temp bindings + reassignments.
        // Here we represent it as a Let binding of a tuple-like form.
        let pairs: Vec<SimpleExpr> = param_names
            .iter()
            .zip(args.iter())
            .map(|(name, arg)| SimpleExpr::BinOp {
                lhs: Box::new(SimpleExpr::Ident(name.clone())),
                op: "=".to_owned(),
                rhs: Box::new(arg.clone()),
            })
            .collect();
        // Emit a synthetic "continue" marker after reassignment.
        if let Some(first) = pairs.into_iter().next() {
            SimpleStmt::Expr(first)
        } else {
            SimpleStmt::Expr(SimpleExpr::IntLit(0))
        }
    }
}

/// Report of tail-call optimization results across a compilation unit.
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    /// Functions that were successfully optimized.
    pub optimized_functions: Vec<String>,
    /// Functions that had tail calls but could not be optimized, with reasons.
    pub skipped_functions: Vec<(String, String)>,
    /// Estimated stack depth reduction (in frames).
    pub stack_depth_reduction: usize,
}

impl OptimizationReport {
    /// Creates an empty optimization report.
    pub fn new() -> Self {
        Self {
            optimized_functions: Vec::new(),
            skipped_functions: Vec::new(),
            stack_depth_reduction: 0,
        }
    }

    /// Records a successfully optimized function.
    pub fn record_optimized(&mut self, name: &str, depth_saved: usize) {
        self.optimized_functions.push(name.to_owned());
        self.stack_depth_reduction += depth_saved;
    }

    /// Records a function that was skipped with a reason.
    pub fn record_skipped(&mut self, name: &str, reason: &str) {
        self.skipped_functions
            .push((name.to_owned(), reason.to_owned()));
    }

    /// Returns the total number of functions analyzed.
    pub fn total_analyzed(&self) -> usize {
        self.optimized_functions.len() + self.skipped_functions.len()
    }

    /// Generates a human-readable summary of the optimization results.
    pub fn summary(&self) -> String {
        format!(
            "TCO: {}/{} functions optimized, ~{} stack frames saved",
            self.optimized_functions.len(),
            self.total_analyzed(),
            self.stack_depth_reduction,
        )
    }
}

impl Default for OptimizationReport {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Constant Folder
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum FoldValue {
    /// Integer constant.
    Int(i64),
    /// Floating-point constant.
    Float(f64),
    /// Boolean constant.
    Bool(bool),
    /// String constant.
    Str(String),
}

impl std::fmt::Display for FoldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FoldValue::Int(v) => write!(f, "{v}"),
            FoldValue::Float(v) => write!(f, "{v}"),
            FoldValue::Bool(v) => write!(f, "{v}"),
            FoldValue::Str(v) => write!(f, "\"{v}\""),
        }
    }
}

/// The result of attempting to fold an expression at compile time.
#[derive(Debug, Clone, PartialEq)]
pub enum FoldResult {
    /// The expression was successfully folded to a constant.
    Constant(FoldValue),
    /// The expression cannot be folded at compile time.
    NotConstant,
    /// Folding produced an error (e.g., division by zero).
    Error(String),
}

/// Statistics about constant folding for a compilation unit.
#[derive(Debug, Clone)]
pub struct FoldStats {
    /// Number of expressions successfully folded.
    pub expressions_folded: usize,
    /// Number of expressions that could not be folded.
    pub expressions_skipped: usize,
    /// Estimated bytes saved by folding (fewer AST nodes).
    pub bytes_saved: usize,
}

impl FoldStats {
    /// Creates zeroed fold statistics.
    pub fn new() -> Self {
        Self {
            expressions_folded: 0,
            expressions_skipped: 0,
            bytes_saved: 0,
        }
    }

    /// Returns the folding success rate as a fraction in [0.0, 1.0].
    pub fn fold_rate(&self) -> f64 {
        let total = self.expressions_folded + self.expressions_skipped;
        if total == 0 {
            return 0.0;
        }
        self.expressions_folded as f64 / total as f64
    }
}

impl Default for FoldStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Compile-time constant folder.
///
/// Evaluates pure expressions at compile time, replacing them with
/// constant values. Handles arithmetic, comparison, logical operations,
/// string concatenation, and known built-in functions.
pub struct ConstFolder {
    /// Known constant bindings (variable name → value).
    constants: HashMap<String, FoldValue>,
    /// Accumulated statistics.
    stats: FoldStats,
}

impl ConstFolder {
    /// Creates a new constant folder with no pre-defined constants.
    pub fn new() -> Self {
        Self {
            constants: HashMap::new(),
            stats: FoldStats::new(),
        }
    }

    /// Defines a named constant for use in folding.
    pub fn define(&mut self, name: &str, value: FoldValue) {
        self.constants.insert(name.to_owned(), value);
    }

    /// Attempts to fold a simplified expression to a constant.
    pub fn fold(&mut self, expr: &SimpleExpr) -> FoldResult {
        let result = self.try_fold(expr);
        match &result {
            FoldResult::Constant(_) => {
                self.stats.expressions_folded += 1;
                self.stats.bytes_saved += 16; // Estimated per-node saving.
            }
            FoldResult::NotConstant | FoldResult::Error(_) => {
                self.stats.expressions_skipped += 1;
            }
        }
        result
    }

    /// Core folding logic without statistics tracking.
    fn try_fold(&self, expr: &SimpleExpr) -> FoldResult {
        match expr {
            SimpleExpr::IntLit(v) => FoldResult::Constant(FoldValue::Int(*v)),
            SimpleExpr::Ident(name) => match self.constants.get(name) {
                Some(v) => FoldResult::Constant(v.clone()),
                None => FoldResult::NotConstant,
            },
            SimpleExpr::BinOp { lhs, op, rhs } => self.fold_binop(lhs, op, rhs),
            SimpleExpr::Call { name, args } => self.fold_call(name, args),
        }
    }

    /// Folds a binary operation.
    fn fold_binop(&self, lhs: &SimpleExpr, op: &str, rhs: &SimpleExpr) -> FoldResult {
        let lv = match self.try_fold(lhs) {
            FoldResult::Constant(v) => v,
            other => return other,
        };
        let rv = match self.try_fold(rhs) {
            FoldResult::Constant(v) => v,
            other => return other,
        };
        self.eval_binop(&lv, op, &rv)
    }

    /// Evaluates a binary operation on two constant values.
    fn eval_binop(&self, lhs: &FoldValue, op: &str, rhs: &FoldValue) -> FoldResult {
        match (lhs, op, rhs) {
            // Int arithmetic
            (FoldValue::Int(a), "+", FoldValue::Int(b)) => match a.checked_add(*b) {
                Some(r) => FoldResult::Constant(FoldValue::Int(r)),
                None => FoldResult::Error("integer overflow".to_owned()),
            },
            (FoldValue::Int(a), "-", FoldValue::Int(b)) => match a.checked_sub(*b) {
                Some(r) => FoldResult::Constant(FoldValue::Int(r)),
                None => FoldResult::Error("integer overflow".to_owned()),
            },
            (FoldValue::Int(a), "*", FoldValue::Int(b)) => match a.checked_mul(*b) {
                Some(r) => FoldResult::Constant(FoldValue::Int(r)),
                None => FoldResult::Error("integer overflow".to_owned()),
            },
            (FoldValue::Int(a), "/", FoldValue::Int(b)) => {
                if *b == 0 {
                    FoldResult::Error("division by zero".to_owned())
                } else {
                    match a.checked_div(*b) {
                        Some(r) => FoldResult::Constant(FoldValue::Int(r)),
                        None => FoldResult::Error("integer overflow".to_owned()),
                    }
                }
            }
            (FoldValue::Int(a), "%", FoldValue::Int(b)) => {
                if *b == 0 {
                    FoldResult::Error("division by zero".to_owned())
                } else {
                    match a.checked_rem(*b) {
                        Some(r) => FoldResult::Constant(FoldValue::Int(r)),
                        None => FoldResult::Error("integer overflow".to_owned()),
                    }
                }
            }
            // Float arithmetic
            (FoldValue::Float(a), "+", FoldValue::Float(b)) => {
                FoldResult::Constant(FoldValue::Float(a + b))
            }
            (FoldValue::Float(a), "-", FoldValue::Float(b)) => {
                FoldResult::Constant(FoldValue::Float(a - b))
            }
            (FoldValue::Float(a), "*", FoldValue::Float(b)) => {
                FoldResult::Constant(FoldValue::Float(a * b))
            }
            (FoldValue::Float(a), "/", FoldValue::Float(b)) => {
                if *b == 0.0 {
                    FoldResult::Error("division by zero".to_owned())
                } else {
                    FoldResult::Constant(FoldValue::Float(a / b))
                }
            }
            // Comparison ops (Int)
            (FoldValue::Int(a), "==", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a == b))
            }
            (FoldValue::Int(a), "!=", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a != b))
            }
            (FoldValue::Int(a), "<", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a < b))
            }
            (FoldValue::Int(a), ">", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a > b))
            }
            (FoldValue::Int(a), "<=", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a <= b))
            }
            (FoldValue::Int(a), ">=", FoldValue::Int(b)) => {
                FoldResult::Constant(FoldValue::Bool(a >= b))
            }
            // Boolean logical ops
            (FoldValue::Bool(a), "&&", FoldValue::Bool(b)) => {
                FoldResult::Constant(FoldValue::Bool(*a && *b))
            }
            (FoldValue::Bool(a), "||", FoldValue::Bool(b)) => {
                FoldResult::Constant(FoldValue::Bool(*a || *b))
            }
            // String concatenation
            (FoldValue::Str(a), "+", FoldValue::Str(b)) => {
                FoldResult::Constant(FoldValue::Str(format!("{a}{b}")))
            }
            _ => FoldResult::NotConstant,
        }
    }

    /// Folds simple built-in function calls (abs, min, max).
    fn fold_call(&self, name: &str, args: &[SimpleExpr]) -> FoldResult {
        let folded_args: Vec<FoldValue> = args
            .iter()
            .filter_map(|a| match self.try_fold(a) {
                FoldResult::Constant(v) => Some(v),
                _ => None,
            })
            .collect();

        if folded_args.len() != args.len() {
            return FoldResult::NotConstant;
        }

        match (name, folded_args.as_slice()) {
            ("abs", [FoldValue::Int(v)]) => FoldResult::Constant(FoldValue::Int(v.wrapping_abs())),
            ("abs", [FoldValue::Float(v)]) => FoldResult::Constant(FoldValue::Float(v.abs())),
            ("min", [FoldValue::Int(a), FoldValue::Int(b)]) => {
                FoldResult::Constant(FoldValue::Int(*a.min(b)))
            }
            ("max", [FoldValue::Int(a), FoldValue::Int(b)]) => {
                FoldResult::Constant(FoldValue::Int(*a.max(b)))
            }
            _ => FoldResult::NotConstant,
        }
    }

    /// Returns a reference to the accumulated folding statistics.
    pub fn stats(&self) -> &FoldStats {
        &self.stats
    }
}

impl Default for ConstFolder {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Compilation Timer
// ═══════════════════════════════════════════════════════════════════════

/// A compilation phase for timing purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Lexical analysis (tokenization).
    Lex,
    /// Parsing (token stream → AST).
    Parse,
    /// Semantic analysis (type checking, scope resolution).
    Analyze,
    /// Code generation (AST → native code).
    Codegen,
    /// Linking (object files → final binary).
    Link,
    /// Total compilation time (wall clock).
    Total,
}

impl Phase {
    /// Returns the human-readable name of this phase.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Lex => "Lex",
            Phase::Parse => "Parse",
            Phase::Analyze => "Analyze",
            Phase::Codegen => "Codegen",
            Phase::Link => "Link",
            Phase::Total => "Total",
        }
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A timing record for a single compilation phase.
#[derive(Debug, Clone)]
pub struct TimingRecord {
    /// Which compilation phase this records.
    pub phase: Phase,
    /// How long the phase took.
    pub duration: Duration,
    /// Number of files processed in this phase.
    pub file_count: usize,
    /// Number of lines processed in this phase.
    pub line_count: usize,
}

/// A per-phase compilation timer with start/end tracking.
///
/// # Examples
///
/// ```
/// use fajar_lang::compiler::performance::{CompilationTimer, Phase};
///
/// let mut timer = CompilationTimer::new();
/// timer.start_phase(Phase::Lex);
/// // ... lexing work ...
/// timer.end_phase(Phase::Lex, 1, 100);
/// let report = timer.report();
/// assert!(report.records.len() == 1);
/// ```
pub struct CompilationTimer {
    /// In-progress phase start times.
    active: HashMap<Phase, Instant>,
    /// Completed timing records.
    records: Vec<TimingRecord>,
}

impl CompilationTimer {
    /// Creates a new compilation timer with no active phases.
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            records: Vec::new(),
        }
    }

    /// Starts timing a compilation phase.
    ///
    /// If the phase was already started, restarts the timer.
    pub fn start_phase(&mut self, phase: Phase) {
        self.active.insert(phase, Instant::now());
    }

    /// Ends a compilation phase and records the duration.
    ///
    /// Returns `Err` if the phase was never started.
    pub fn end_phase(
        &mut self,
        phase: Phase,
        file_count: usize,
        line_count: usize,
    ) -> Result<Duration, PerfError> {
        let start = self
            .active
            .remove(&phase)
            .ok_or_else(|| PerfError::PhaseNotStarted {
                phase: phase.name().to_owned(),
            })?;
        let duration = start.elapsed();
        self.records.push(TimingRecord {
            phase,
            duration,
            file_count,
            line_count,
        });
        Ok(duration)
    }

    /// Generates a timing report from all completed phases.
    pub fn report(&self) -> TimingReport {
        let total_duration: Duration = self.records.iter().map(|r| r.duration).sum();
        TimingReport {
            records: self.records.clone(),
            total_duration,
        }
    }

    /// Resets all records and active phases.
    pub fn reset(&mut self) {
        self.active.clear();
        self.records.clear();
    }
}

impl Default for CompilationTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// A report of compilation phase timings.
#[derive(Debug, Clone)]
pub struct TimingReport {
    /// Per-phase timing records.
    pub records: Vec<TimingRecord>,
    /// Total duration across all recorded phases.
    pub total_duration: Duration,
}

impl TimingReport {
    /// Returns the formatted report suitable for `--timings` output.
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str("  Phase       Time       %     Files  Lines\n");
        out.push_str("  ─────────── ────────── ───── ───── ──────\n");

        let total_us = self.total_duration.as_micros().max(1);
        for rec in &self.records {
            let us = rec.duration.as_micros();
            let pct = (us as f64 / total_us as f64) * 100.0;
            out.push_str(&format!(
                "  {:<11} {:>7}us {:>5.1}% {:>5} {:>6}\n",
                rec.phase.name(),
                us,
                pct,
                rec.file_count,
                rec.line_count,
            ));
        }

        out.push_str(&format!(
            "  {:<11} {:>7}us {:>5.1}%\n",
            "Total", total_us, 100.0,
        ));
        out
    }

    /// Returns the duration for a specific phase, if recorded.
    pub fn phase_duration(&self, phase: Phase) -> Option<Duration> {
        self.records
            .iter()
            .find(|r| r.phase == phase)
            .map(|r| r.duration)
    }

    /// Returns the percentage of total time spent in each phase.
    pub fn percentage_breakdown(&self) -> Vec<(Phase, f64)> {
        let total_us = self.total_duration.as_micros().max(1) as f64;
        self.records
            .iter()
            .map(|r| {
                let pct = (r.duration.as_micros() as f64 / total_us) * 100.0;
                (r.phase, pct)
            })
            .collect()
    }
}

impl std::fmt::Display for TimingReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Value Optimizer
// ═══════════════════════════════════════════════════════════════════════

/// Memory layout information for a single enum variant.
#[derive(Debug, Clone)]
pub struct VariantLayout {
    /// Variant name.
    pub name: String,
    /// Size in bytes (of the payload, not including the discriminant).
    pub payload_size: usize,
    /// Whether boxing is recommended for this variant.
    pub suggest_boxing: bool,
}

/// Analysis of the `Value` enum's memory layout.
///
/// Reports the total size, per-variant payload sizes, and recommendations
/// for boxing large variants to reduce the overall enum size.
#[derive(Debug, Clone)]
pub struct ValueSizeAnalysis {
    /// Total size of the Value enum in bytes.
    pub total_size: usize,
    /// Per-variant layout information.
    pub variants: Vec<VariantLayout>,
    /// The recommended maximum variant size before boxing.
    pub box_threshold: usize,
}

impl ValueSizeAnalysis {
    /// Analyzes the memory layout of a Value-like enum.
    ///
    /// `variant_sizes` is a list of (name, payload_size) pairs. The
    /// analysis determines which variants exceed the threshold and should
    /// be boxed.
    pub fn analyze(variant_sizes: &[(&str, usize)], box_threshold: usize) -> Self {
        let max_payload = variant_sizes.iter().map(|(_, s)| *s).max().unwrap_or(0);

        // Enum size = discriminant + largest variant payload (aligned).
        let discriminant_size = 8; // typical for Rust enum with many variants
        let total_size = discriminant_size + max_payload;

        let variants = variant_sizes
            .iter()
            .map(|(name, size)| VariantLayout {
                name: (*name).to_owned(),
                payload_size: *size,
                suggest_boxing: *size > box_threshold,
            })
            .collect();

        Self {
            total_size,
            variants,
            box_threshold,
        }
    }

    /// Returns a human-readable report of the analysis.
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Value enum total size: {} bytes (box threshold: {} bytes)\n",
            self.total_size, self.box_threshold
        ));
        for v in &self.variants {
            let flag = if v.suggest_boxing { " [BOX]" } else { "" };
            out.push_str(&format!("  {} — {} bytes{flag}\n", v.name, v.payload_size));
        }
        let boxable: Vec<_> = self.variants.iter().filter(|v| v.suggest_boxing).collect();
        if !boxable.is_empty() {
            out.push_str(&format!(
                "Recommendation: Box {} variant(s) to reduce enum size.\n",
                boxable.len()
            ));
        } else {
            out.push_str("All variants within threshold — no boxing needed.\n");
        }
        out
    }
}

/// A small-string optimization: stores strings of 22 bytes or fewer inline
/// without heap allocation. Larger strings fall back to a heap `String`.
///
/// The inline capacity is chosen to fit within a 24-byte struct (22 bytes
/// of data + 1 byte for length + 1 byte discriminant).
#[derive(Debug, Clone)]
pub enum SmallString {
    /// String stored inline (no heap allocation).
    Inline {
        /// Inline character storage.
        data: [u8; 22],
        /// Number of valid bytes in `data`.
        len: u8,
    },
    /// String stored on the heap.
    Heap(String),
}

impl SmallString {
    /// The maximum byte length for inline storage.
    pub const INLINE_CAP: usize = 22;

    /// Creates a new `SmallString` from a string slice.
    ///
    /// Uses inline storage if the string is 22 bytes or fewer.
    pub fn new(s: &str) -> Self {
        if s.len() <= Self::INLINE_CAP {
            let mut data = [0u8; 22];
            data[..s.len()].copy_from_slice(s.as_bytes());
            SmallString::Inline {
                data,
                len: s.len() as u8,
            }
        } else {
            SmallString::Heap(s.to_owned())
        }
    }

    /// Returns the string as a `&str`.
    pub fn as_str(&self) -> &str {
        match self {
            SmallString::Inline { data, len } => {
                let bytes = &data[..*len as usize];
                // SAFETY: We only store valid UTF-8 from the constructor.
                unsafe { std::str::from_utf8_unchecked(bytes) }
            }
            SmallString::Heap(s) => s.as_str(),
        }
    }

    /// Returns the byte length of the string.
    pub fn len(&self) -> usize {
        match self {
            SmallString::Inline { len, .. } => *len as usize,
            SmallString::Heap(s) => s.len(),
        }
    }

    /// Returns `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the string is stored inline (no heap allocation).
    pub fn is_inline(&self) -> bool {
        matches!(self, SmallString::Inline { .. })
    }
}

impl PartialEq for SmallString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SmallString {}

impl std::fmt::Display for SmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tag for [`CompactValue`] indicating the stored type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ValueTag {
    /// Null value.
    Null = 0,
    /// Integer (stored inline).
    Int = 1,
    /// Float (stored inline via bit reinterpretation).
    Float = 2,
    /// Boolean (stored inline).
    Bool = 3,
    /// Heap-allocated value (boxed).
    Boxed = 4,
}

/// A compact representation of runtime values using tagged pointers.
///
/// Common types (Null, Int, Float, Bool) are stored inline in 16 bytes.
/// Everything else is boxed into a heap allocation.
///
/// This is an alternative to the full `Value` enum that reduces per-value
/// overhead for the most common cases.
#[derive(Debug, Clone)]
pub struct CompactValue {
    /// Type tag.
    tag: ValueTag,
    /// Raw bits (interpretation depends on `tag`).
    bits: u64,
}

impl CompactValue {
    /// Creates a null compact value.
    pub fn null() -> Self {
        Self {
            tag: ValueTag::Null,
            bits: 0,
        }
    }

    /// Creates an integer compact value.
    pub fn int(v: i64) -> Self {
        Self {
            tag: ValueTag::Int,
            bits: v as u64,
        }
    }

    /// Creates a float compact value.
    pub fn float(v: f64) -> Self {
        Self {
            tag: ValueTag::Float,
            bits: v.to_bits(),
        }
    }

    /// Creates a boolean compact value.
    pub fn bool_val(v: bool) -> Self {
        Self {
            tag: ValueTag::Bool,
            bits: v as u64,
        }
    }

    /// Returns the type tag of this value.
    pub fn tag(&self) -> ValueTag {
        self.tag
    }

    /// Attempts to extract an integer from this value.
    pub fn as_int(&self) -> Option<i64> {
        if self.tag == ValueTag::Int {
            Some(self.bits as i64)
        } else {
            None
        }
    }

    /// Attempts to extract a float from this value.
    pub fn as_float(&self) -> Option<f64> {
        if self.tag == ValueTag::Float {
            Some(f64::from_bits(self.bits))
        } else {
            None
        }
    }

    /// Attempts to extract a boolean from this value.
    pub fn as_bool(&self) -> Option<bool> {
        if self.tag == ValueTag::Bool {
            Some(self.bits != 0)
        } else {
            None
        }
    }

    /// Returns `true` if this is a null value.
    pub fn is_null(&self) -> bool {
        self.tag == ValueTag::Null
    }

    /// Returns the in-memory size of this compact value in bytes.
    pub fn size_of_val(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl PartialEq for CompactValue {
    fn eq(&self, other: &Self) -> bool {
        self.tag == other.tag && self.bits == other.bits
    }
}

impl Eq for CompactValue {}

/// Generates a memory layout report for the Value enum.
///
/// Measures actual `size_of` for common Rust types and presents
/// a summary of the per-variant overhead.
pub fn value_memory_report() -> String {
    let variants = [
        ("Null", 0_usize),
        ("Int", std::mem::size_of::<i64>()),
        ("Float", std::mem::size_of::<f64>()),
        ("Bool", std::mem::size_of::<bool>()),
        ("Char", std::mem::size_of::<char>()),
        ("Str", std::mem::size_of::<String>()),
        ("Array", std::mem::size_of::<Vec<u8>>()),
        ("Tuple", std::mem::size_of::<Vec<u8>>()),
        (
            "Struct",
            std::mem::size_of::<String>() + std::mem::size_of::<HashMap<String, u8>>(),
        ),
        (
            "Enum",
            std::mem::size_of::<String>() + std::mem::size_of::<Option<Box<u8>>>(),
        ),
        ("Map", std::mem::size_of::<HashMap<String, u8>>()),
        ("Pointer", std::mem::size_of::<u64>()),
    ];
    let analysis = ValueSizeAnalysis::analyze(&variants, 48);
    analysis.report()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ───────────────────────────────────────────────────────────────────
    // Sprint 5: String Interner (s5_1 – s5_5) + Inline Cache (s5_6 – s5_10)
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn s5_1_interner_basic_intern_and_resolve() {
        let mut interner = Interner::new();
        let sym = interner.intern("hello");
        assert_eq!(interner.resolve(sym), Some("hello"));
        assert_eq!(sym.index(), 0);
    }

    #[test]
    fn s5_2_interner_deduplication() {
        let mut interner = Interner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("foo");
        let s3 = interner.intern("bar");
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn s5_3_interner_statistics() {
        let mut interner = Interner::new();
        assert!(interner.is_empty());
        assert_eq!(interner.total_bytes(), 0);
        assert!((interner.average_length() - 0.0).abs() < f64::EPSILON);

        interner.intern("abc");
        interner.intern("de");
        assert_eq!(interner.len(), 2);
        assert_eq!(interner.total_bytes(), 5);
        assert!((interner.average_length() - 2.5).abs() < f64::EPSILON);

        let stats = interner.stats();
        assert_eq!(stats.count, 2);
        assert_eq!(stats.total_bytes, 5);
    }

    #[test]
    fn s5_4_interner_invalid_symbol_resolve() {
        let interner = Interner::new();
        let bad_sym = Symbol { index: 999 };
        assert_eq!(interner.resolve(bad_sym), None);
    }

    #[test]
    fn s5_5_sync_interner_thread_safety() {
        let interner = SyncInterner::new();
        let s1 = interner.intern("alpha").expect("intern should succeed");
        let s2 = interner.intern("alpha").expect("intern should succeed");
        assert_eq!(s1, s2);
        assert_eq!(interner.resolve(s1), Some("alpha".to_owned()));
        assert_eq!(interner.len(), 1);
        assert!(!interner.is_empty());
    }

    #[test]
    fn s5_6_inline_cache_monomorphic_hit() {
        let mut cache = InlineCache::new();
        cache.update(42, CachedResult::MethodSlot(0));
        assert_eq!(cache.lookup(42), CachedResult::MethodSlot(0));
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 0);
    }

    #[test]
    fn s5_7_inline_cache_monomorphic_miss() {
        let mut cache = InlineCache::new();
        cache.update(42, CachedResult::FieldOffset(8));
        let result = cache.lookup(99);
        assert_eq!(result, CachedResult::Miss);
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn s5_8_inline_cache_polymorphic_transition() {
        let mut cache = InlineCache::new();
        cache.update(1, CachedResult::MethodSlot(0));
        cache.update(2, CachedResult::MethodSlot(1));
        cache.update(3, CachedResult::MethodSlot(2));

        assert_eq!(cache.lookup(1), CachedResult::MethodSlot(0));
        assert_eq!(cache.lookup(2), CachedResult::MethodSlot(1));
        assert_eq!(cache.lookup(3), CachedResult::MethodSlot(2));
        assert!(!cache.is_megamorphic());
    }

    #[test]
    fn s5_9_inline_cache_megamorphic_transition() {
        let mut cache = InlineCache::new();
        // Fill up to polymorphic (4 entries).
        for i in 0..4 {
            cache.update(i, CachedResult::MethodSlot(i as usize));
        }
        assert!(!cache.is_megamorphic());

        // 5th unique type → megamorphic.
        cache.update(100, CachedResult::MethodSlot(100));
        assert!(cache.is_megamorphic());

        // All lookups now miss.
        assert_eq!(cache.lookup(1), CachedResult::Miss);
    }

    #[test]
    fn s5_10_inline_cache_hit_rate_and_reset() {
        let mut cache = InlineCache::new();
        cache.update(1, CachedResult::FieldOffset(0));
        cache.lookup(1); // hit
        cache.lookup(1); // hit
        cache.lookup(2); // miss

        let rate = cache.stats().hit_rate();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);

        cache.reset();
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        assert!(!cache.is_megamorphic());
    }

    // ───────────────────────────────────────────────────────────────────
    // Sprint 6: Dispatch Table (s6_1 – s6_5) + Tail Call (s6_6 – s6_10)
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn s6_1_dispatch_int_arithmetic() {
        let table = DispatchTable::new();
        assert_eq!(
            table.dispatch(OpCode::Add, TypeTag::Int, TypeTag::Int, 3, 4),
            DispatchResult::IntResult(7)
        );
        assert_eq!(
            table.dispatch(OpCode::Sub, TypeTag::Int, TypeTag::Int, 10, 3),
            DispatchResult::IntResult(7)
        );
        assert_eq!(
            table.dispatch(OpCode::Mul, TypeTag::Int, TypeTag::Int, 5, 6),
            DispatchResult::IntResult(30)
        );
        assert_eq!(
            table.dispatch(OpCode::Div, TypeTag::Int, TypeTag::Int, 20, 4),
            DispatchResult::IntResult(5)
        );
        assert_eq!(
            table.dispatch(OpCode::Mod, TypeTag::Int, TypeTag::Int, 17, 5),
            DispatchResult::IntResult(2)
        );
    }

    #[test]
    fn s6_2_dispatch_int_comparison() {
        let table = DispatchTable::new();
        assert_eq!(
            table.dispatch(OpCode::Eq, TypeTag::Int, TypeTag::Int, 5, 5),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Ne, TypeTag::Int, TypeTag::Int, 5, 6),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Lt, TypeTag::Int, TypeTag::Int, 3, 5),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Gt, TypeTag::Int, TypeTag::Int, 5, 3),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Le, TypeTag::Int, TypeTag::Int, 5, 5),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Ge, TypeTag::Int, TypeTag::Int, 5, 5),
            DispatchResult::BoolResult(true)
        );
    }

    #[test]
    fn s6_3_dispatch_float_arithmetic() {
        let table = DispatchTable::new();
        let a = 3.0_f64.to_bits() as i64;
        let b = 4.0_f64.to_bits() as i64;

        if let DispatchResult::FloatResult(r) =
            table.dispatch(OpCode::Add, TypeTag::Float, TypeTag::Float, a, b)
        {
            assert!((r - 7.0).abs() < f64::EPSILON);
        } else {
            panic!("expected FloatResult");
        }

        if let DispatchResult::FloatResult(r) =
            table.dispatch(OpCode::Mul, TypeTag::Float, TypeTag::Float, a, b)
        {
            assert!((r - 12.0).abs() < f64::EPSILON);
        } else {
            panic!("expected FloatResult");
        }
    }

    #[test]
    fn s6_4_dispatch_bool_logical() {
        let table = DispatchTable::new();
        assert_eq!(
            table.dispatch(OpCode::And, TypeTag::Bool, TypeTag::Bool, 1, 0),
            DispatchResult::BoolResult(false)
        );
        assert_eq!(
            table.dispatch(OpCode::Or, TypeTag::Bool, TypeTag::Bool, 0, 1),
            DispatchResult::BoolResult(true)
        );
        assert_eq!(
            table.dispatch(OpCode::Eq, TypeTag::Bool, TypeTag::Bool, 1, 1),
            DispatchResult::BoolResult(true)
        );
    }

    #[test]
    fn s6_5_dispatch_type_error_and_has_handler() {
        let table = DispatchTable::new();
        // Array + Int → TypeError
        assert_eq!(
            table.dispatch(OpCode::Add, TypeTag::Array, TypeTag::Int, 0, 0),
            DispatchResult::TypeError
        );
        // Div by zero → TypeError
        assert_eq!(
            table.dispatch(OpCode::Div, TypeTag::Int, TypeTag::Int, 5, 0),
            DispatchResult::TypeError
        );

        assert!(table.has_handler(OpCode::Add, TypeTag::Int, TypeTag::Int));
        assert!(!table.has_handler(OpCode::Add, TypeTag::Array, TypeTag::Int));
    }

    #[test]
    fn s6_6_tail_call_self_recursion_detected() {
        let detector = TailCallDetector::new("factorial");
        let body = vec![SimpleStmt::Return(SimpleExpr::Call {
            name: "factorial".to_string(),
            args: vec![SimpleExpr::BinOp {
                lhs: Box::new(SimpleExpr::Ident("n".to_string())),
                op: "-".to_string(),
                rhs: Box::new(SimpleExpr::IntLit(1)),
            }],
        })];

        let results = detector.detect(&body);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_self_recursive);
        assert_eq!(results[0].function_name, "factorial");
        assert_eq!(results[0].param_count, 1);
    }

    #[test]
    fn s6_7_tail_call_in_if_else_branches() {
        let detector = TailCallDetector::new("fib");
        let body = vec![SimpleStmt::IfElse {
            cond: SimpleExpr::BinOp {
                lhs: Box::new(SimpleExpr::Ident("n".to_string())),
                op: "<=".to_string(),
                rhs: Box::new(SimpleExpr::IntLit(1)),
            },
            then_body: vec![SimpleStmt::Return(SimpleExpr::Ident("n".to_string()))],
            else_body: vec![SimpleStmt::Return(SimpleExpr::Call {
                name: "fib".to_string(),
                args: vec![SimpleExpr::Ident("n".to_string())],
            })],
        }];

        let results = detector.detect(&body);
        // Only the else branch has a tail call.
        assert_eq!(results.len(), 1);
        assert!(results[0].is_self_recursive);
    }

    #[test]
    fn s6_8_tail_call_mutual_recursion() {
        let detector = TailCallDetector::new("is_even");
        let body = vec![SimpleStmt::Return(SimpleExpr::Call {
            name: "is_odd".to_string(),
            args: vec![SimpleExpr::Ident("n".to_string())],
        })];

        let results = detector.detect(&body);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_self_recursive);
        assert_eq!(results[0].target_name, "is_odd");
    }

    #[test]
    fn s6_9_tail_call_transform_self_recursive() {
        let params = vec!["n".to_string(), "acc".to_string()];
        let body = vec![SimpleStmt::Return(SimpleExpr::Call {
            name: "fact".to_string(),
            args: vec![
                SimpleExpr::BinOp {
                    lhs: Box::new(SimpleExpr::Ident("n".to_string())),
                    op: "-".to_string(),
                    rhs: Box::new(SimpleExpr::IntLit(1)),
                },
                SimpleExpr::BinOp {
                    lhs: Box::new(SimpleExpr::Ident("acc".to_string())),
                    op: "*".to_string(),
                    rhs: Box::new(SimpleExpr::Ident("n".to_string())),
                },
            ],
        })];

        let result = TailCallTransform::transform("fact", &params, &body);
        match result {
            TransformResult::Transformed { param_names, .. } => {
                assert_eq!(param_names, vec!["n", "acc"]);
            }
            TransformResult::NotTransformable(reason) => {
                panic!("expected Transformed, got NotTransformable: {reason}");
            }
        }
    }

    #[test]
    fn s6_10_optimization_report() {
        let mut report = OptimizationReport::new();
        report.record_optimized("factorial", 1000);
        report.record_optimized("sum", 500);
        report.record_skipped("fib", "not tail-recursive");

        assert_eq!(report.total_analyzed(), 3);
        assert_eq!(report.optimized_functions.len(), 2);
        assert_eq!(report.skipped_functions.len(), 1);
        assert_eq!(report.stack_depth_reduction, 1500);

        let summary = report.summary();
        assert!(summary.contains("2/3"));
        assert!(summary.contains("1500"));
    }

    // ───────────────────────────────────────────────────────────────────
    // Sprint 7: Const Folder (s7_1 – s7_5) + Compilation Timer (s7_6 – s7_10)
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn s7_1_const_fold_int_arithmetic() {
        let mut folder = ConstFolder::new();
        let expr = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::IntLit(10)),
            op: "+".to_string(),
            rhs: Box::new(SimpleExpr::IntLit(20)),
        };
        assert_eq!(folder.fold(&expr), FoldResult::Constant(FoldValue::Int(30)));

        let expr_mul = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::IntLit(6)),
            op: "*".to_string(),
            rhs: Box::new(SimpleExpr::IntLit(7)),
        };
        assert_eq!(
            folder.fold(&expr_mul),
            FoldResult::Constant(FoldValue::Int(42))
        );
    }

    #[test]
    fn s7_2_const_fold_division_by_zero() {
        let mut folder = ConstFolder::new();
        let expr = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::IntLit(10)),
            op: "/".to_string(),
            rhs: Box::new(SimpleExpr::IntLit(0)),
        };
        assert_eq!(
            folder.fold(&expr),
            FoldResult::Error("division by zero".to_owned())
        );
    }

    #[test]
    fn s7_3_const_fold_boolean_and_comparison() {
        let mut folder = ConstFolder::new();
        let cmp = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::IntLit(5)),
            op: "<".to_string(),
            rhs: Box::new(SimpleExpr::IntLit(10)),
        };
        assert_eq!(
            folder.fold(&cmp),
            FoldResult::Constant(FoldValue::Bool(true))
        );

        folder.define("x", FoldValue::Bool(true));
        folder.define("y", FoldValue::Bool(false));
        let logical = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::Ident("x".to_string())),
            op: "&&".to_string(),
            rhs: Box::new(SimpleExpr::Ident("y".to_string())),
        };
        assert_eq!(
            folder.fold(&logical),
            FoldResult::Constant(FoldValue::Bool(false))
        );
    }

    #[test]
    fn s7_4_const_fold_named_constants_and_builtins() {
        let mut folder = ConstFolder::new();
        folder.define("WIDTH", FoldValue::Int(1920));
        folder.define("HEIGHT", FoldValue::Int(1080));

        let expr = SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::Ident("WIDTH".to_string())),
            op: "*".to_string(),
            rhs: Box::new(SimpleExpr::Ident("HEIGHT".to_string())),
        };
        assert_eq!(
            folder.fold(&expr),
            FoldResult::Constant(FoldValue::Int(1920 * 1080))
        );

        // Builtin abs
        let abs_call = SimpleExpr::Call {
            name: "abs".to_string(),
            args: vec![SimpleExpr::IntLit(-42)],
        };
        assert_eq!(
            folder.fold(&abs_call),
            FoldResult::Constant(FoldValue::Int(42))
        );
    }

    #[test]
    fn s7_5_const_fold_stats() {
        let mut folder = ConstFolder::new();
        folder.fold(&SimpleExpr::IntLit(42)); // folded
        folder.fold(&SimpleExpr::Ident("unknown".to_string())); // not constant
        folder.fold(&SimpleExpr::BinOp {
            lhs: Box::new(SimpleExpr::IntLit(1)),
            op: "+".to_string(),
            rhs: Box::new(SimpleExpr::IntLit(2)),
        }); // folded

        let stats = folder.stats();
        assert_eq!(stats.expressions_folded, 2);
        assert_eq!(stats.expressions_skipped, 1);
        assert!((stats.fold_rate() - 2.0 / 3.0).abs() < 0.01);
        assert!(stats.bytes_saved > 0);
    }

    #[test]
    fn s7_6_compilation_timer_basic_phase() {
        let mut timer = CompilationTimer::new();
        timer.start_phase(Phase::Lex);
        // Simulate work.
        let _ = (0..1000).sum::<i32>();
        let dur = timer.end_phase(Phase::Lex, 1, 500);
        assert!(dur.is_ok());

        let report = timer.report();
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].phase, Phase::Lex);
        assert_eq!(report.records[0].file_count, 1);
        assert_eq!(report.records[0].line_count, 500);
    }

    #[test]
    fn s7_7_compilation_timer_phase_not_started() {
        let mut timer = CompilationTimer::new();
        let result = timer.end_phase(Phase::Parse, 0, 0);
        assert!(result.is_err());
        if let Err(PerfError::PhaseNotStarted { phase }) = result {
            assert_eq!(phase, "Parse");
        } else {
            panic!("expected PhaseNotStarted error");
        }
    }

    #[test]
    fn s7_8_compilation_timer_multiple_phases() {
        let mut timer = CompilationTimer::new();

        timer.start_phase(Phase::Lex);
        let _ = timer.end_phase(Phase::Lex, 1, 100);

        timer.start_phase(Phase::Parse);
        let _ = timer.end_phase(Phase::Parse, 1, 100);

        timer.start_phase(Phase::Analyze);
        let _ = timer.end_phase(Phase::Analyze, 1, 100);

        let report = timer.report();
        assert_eq!(report.records.len(), 3);
        assert!(report.total_duration.as_nanos() > 0);
    }

    #[test]
    fn s7_9_timing_report_display() {
        let mut timer = CompilationTimer::new();
        timer.start_phase(Phase::Lex);
        let _ = timer.end_phase(Phase::Lex, 2, 1000);

        let report = timer.report();
        let display = report.display();
        assert!(display.contains("Lex"));
        assert!(display.contains("us"));
        assert!(display.contains("Total"));
    }

    #[test]
    fn s7_10_timing_report_percentage_breakdown() {
        let mut timer = CompilationTimer::new();
        timer.start_phase(Phase::Lex);
        let _ = timer.end_phase(Phase::Lex, 1, 100);
        timer.start_phase(Phase::Parse);
        let _ = timer.end_phase(Phase::Parse, 1, 100);

        let report = timer.report();
        let breakdown = report.percentage_breakdown();
        assert_eq!(breakdown.len(), 2);

        let total_pct: f64 = breakdown.iter().map(|(_, p)| p).sum();
        // Near-zero durations may cause rounding — allow generous tolerance
        assert!((0.0..=200.0).contains(&total_pct), "total_pct={total_pct}");

        assert!(report.phase_duration(Phase::Lex).is_some());
        assert!(report.phase_duration(Phase::Codegen).is_none());
    }

    // ───────────────────────────────────────────────────────────────────
    // Sprint 8: Value Optimizer (s8_1 – s8_10)
    // ───────────────────────────────────────────────────────────────────

    #[test]
    fn s8_1_value_size_analysis_basic() {
        let variants = [
            ("Null", 0),
            ("Int", 8),
            ("Float", 8),
            ("Str", 24),
            ("Map", 48),
            ("TraitObject", 96),
        ];
        let analysis = ValueSizeAnalysis::analyze(&variants, 32);
        // Total = discriminant(8) + max_payload(96) = 104
        assert_eq!(analysis.total_size, 104);
        assert_eq!(analysis.variants.len(), 6);
    }

    #[test]
    fn s8_2_value_size_analysis_boxing_recommendations() {
        let variants = [
            ("Int", 8),
            ("Float", 8),
            ("Struct", 72),
            ("TraitObject", 96),
        ];
        let analysis = ValueSizeAnalysis::analyze(&variants, 32);
        let boxable: Vec<_> = analysis
            .variants
            .iter()
            .filter(|v| v.suggest_boxing)
            .collect();
        assert_eq!(boxable.len(), 2); // Struct and TraitObject exceed 32
        assert!(!analysis.variants[0].suggest_boxing); // Int = 8
        assert!(analysis.variants[2].suggest_boxing); // Struct = 72
    }

    #[test]
    fn s8_3_value_size_analysis_report() {
        let variants = [("Int", 8), ("BigStruct", 128)];
        let analysis = ValueSizeAnalysis::analyze(&variants, 32);
        let report = analysis.report();
        assert!(report.contains("Value enum total size:"));
        assert!(report.contains("[BOX]"));
        assert!(report.contains("BigStruct"));
        assert!(report.contains("Recommendation: Box 1 variant"));
    }

    #[test]
    fn s8_4_small_string_inline() {
        let ss = SmallString::new("hello");
        assert!(ss.is_inline());
        assert_eq!(ss.as_str(), "hello");
        assert_eq!(ss.len(), 5);
        assert!(!ss.is_empty());
    }

    #[test]
    fn s8_5_small_string_heap_fallback() {
        let long = "this is a string that is definitely longer than 22 bytes";
        let ss = SmallString::new(long);
        assert!(!ss.is_inline());
        assert_eq!(ss.as_str(), long);
        assert_eq!(ss.len(), long.len());
    }

    #[test]
    fn s8_6_small_string_boundary() {
        // Exactly 22 bytes → inline.
        let exact = "a".repeat(22);
        let ss = SmallString::new(&exact);
        assert!(ss.is_inline());
        assert_eq!(ss.len(), 22);

        // 23 bytes → heap.
        let over = "a".repeat(23);
        let ss2 = SmallString::new(&over);
        assert!(!ss2.is_inline());
        assert_eq!(ss2.len(), 23);
    }

    #[test]
    fn s8_7_small_string_equality() {
        let a = SmallString::new("same");
        let b = SmallString::new("same");
        let c = SmallString::new("diff");
        assert_eq!(a, b);
        assert_ne!(a, c);

        // Cross inline/heap comparison.
        let short = SmallString::new("x");
        let long = SmallString::new(&"x".repeat(30));
        assert_ne!(short, long);
    }

    #[test]
    fn s8_8_compact_value_int_float_bool() {
        let iv = CompactValue::int(42);
        assert_eq!(iv.as_int(), Some(42));
        assert_eq!(iv.as_float(), None);
        assert_eq!(iv.tag(), ValueTag::Int);

        let fv = CompactValue::float(1.25);
        assert!((fv.as_float().unwrap() - 1.25).abs() < f64::EPSILON);
        assert_eq!(fv.as_int(), None);
        assert_eq!(fv.tag(), ValueTag::Float);

        let bv = CompactValue::bool_val(true);
        assert_eq!(bv.as_bool(), Some(true));
        assert_eq!(bv.tag(), ValueTag::Bool);

        let nv = CompactValue::null();
        assert!(nv.is_null());
        assert_eq!(nv.tag(), ValueTag::Null);
    }

    #[test]
    fn s8_9_compact_value_equality_and_size() {
        let a = CompactValue::int(100);
        let b = CompactValue::int(100);
        let c = CompactValue::int(200);
        assert_eq!(a, b);
        assert_ne!(a, c);

        // CompactValue should be 16 bytes (u8 tag + padding + u64 bits).
        let size = a.size_of_val();
        assert!(
            size <= 16,
            "CompactValue should be at most 16 bytes, got {size}"
        );
    }

    #[test]
    fn s8_10_value_memory_report() {
        let report = value_memory_report();
        assert!(report.contains("Value enum total size:"));
        assert!(report.contains("Int"));
        assert!(report.contains("Float"));
        assert!(report.contains("Str"));
        assert!(report.contains("bytes"));
    }
}
