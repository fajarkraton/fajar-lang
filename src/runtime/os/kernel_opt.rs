//! Kernel optimization — compile-time and runtime optimizations for kernel code.
//!
//! Sprint N2: Provides optimization analysis and configuration for kernel modules.
//! All optimizations are simulation-based — no real LLVM passes or hardware.
//!
//! # Architecture
//!
//! ```text
//! KernelOptConfig        — LLVM O2 settings for kernel code
//! LtoConfig              — link-time optimization for kernel modules
//! ConstKernelData        — compile-time page tables, GDT in .rodata
//! DeadKernelCodeElim     — remove unused kernel paths
//! InlineCriticalPath     — IRQ/syscall inlining decisions
//! StackUsageAudit        — verify kernel stack limits
//! CacheFriendlyLayout    — struct packing analysis
//! ZeroCopyIpc            — shared memory page management
//! LockFreeQueue          — SPSC/MPSC lock-free queue
//! KernelBenchSuite       — boot time, context switch, syscall latency
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from kernel optimization operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum KernelOptError {
    /// Stack usage exceeds the configured limit.
    #[error("stack overflow: function '{func}' uses {used} bytes, limit is {limit}")]
    StackOverflow {
        /// Function name.
        func: String,
        /// Bytes used.
        used: u64,
        /// Configured limit.
        limit: u64,
    },

    /// Invalid optimization level.
    #[error("invalid optimization level: {level}")]
    InvalidOptLevel {
        /// The invalid level string.
        level: String,
    },

    /// Queue is full (for lock-free queue).
    #[error("queue full: capacity {capacity}")]
    QueueFull {
        /// Queue capacity.
        capacity: usize,
    },

    /// Queue is empty.
    #[error("queue empty")]
    QueueEmpty,

    /// Shared memory region error.
    #[error("shared memory error: {reason}")]
    SharedMemoryError {
        /// Description of the error.
        reason: String,
    },

    /// Benchmark error.
    #[error("benchmark error: {reason}")]
    BenchmarkError {
        /// Description.
        reason: String,
    },
}

/// Result type for kernel optimization operations.
pub type KernelOptResult<T> = Result<T, KernelOptError>;

// ═══════════════════════════════════════════════════════════════════════
// Optimization level
// ═══════════════════════════════════════════════════════════════════════

/// LLVM-style optimization level for kernel code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptLevel {
    /// No optimization (debug builds).
    O0,
    /// Basic optimization (size + speed balance).
    O1,
    /// Full optimization (recommended for kernel).
    O2,
    /// Aggressive optimization (may increase code size).
    O3,
    /// Optimize for binary size.
    Os,
    /// Aggressively optimize for size.
    Oz,
}

impl OptLevel {
    /// Parses an optimization level from a string.
    pub fn from_str_opt(s: &str) -> KernelOptResult<Self> {
        match s {
            "O0" | "0" => Ok(OptLevel::O0),
            "O1" | "1" => Ok(OptLevel::O1),
            "O2" | "2" => Ok(OptLevel::O2),
            "O3" | "3" => Ok(OptLevel::O3),
            "Os" | "s" => Ok(OptLevel::Os),
            "Oz" | "z" => Ok(OptLevel::Oz),
            _ => Err(KernelOptError::InvalidOptLevel {
                level: s.to_string(),
            }),
        }
    }

    /// Returns the numeric priority (higher = more aggressive).
    pub fn priority(self) -> u8 {
        match self {
            OptLevel::O0 => 0,
            OptLevel::O1 => 1,
            OptLevel::Os => 2,
            OptLevel::Oz => 2,
            OptLevel::O2 => 3,
            OptLevel::O3 => 4,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Kernel Optimization Config
// ═══════════════════════════════════════════════════════════════════════

/// LLVM O2 optimization settings for kernel code.
///
/// Configures which optimization passes to enable for kernel compilation.
#[derive(Debug, Clone)]
pub struct KernelOptConfig {
    /// Optimization level.
    pub opt_level: OptLevel,
    /// Enable loop unrolling.
    pub loop_unroll: bool,
    /// Enable function inlining.
    pub inline: bool,
    /// Enable dead code elimination.
    pub dce: bool,
    /// Enable constant folding.
    pub const_fold: bool,
    /// Enable tail call optimization.
    pub tail_call: bool,
    /// Target CPU (e.g., "generic", "cortex-a53", "x86-64-v3").
    pub target_cpu: String,
    /// Additional flags.
    pub extra_flags: Vec<String>,
}

impl KernelOptConfig {
    /// Creates a default O2 kernel optimization config.
    pub fn default_o2() -> Self {
        Self {
            opt_level: OptLevel::O2,
            loop_unroll: true,
            inline: true,
            dce: true,
            const_fold: true,
            tail_call: true,
            target_cpu: "generic".to_string(),
            extra_flags: Vec::new(),
        }
    }

    /// Creates a minimal O0 config for debug builds.
    pub fn debug() -> Self {
        Self {
            opt_level: OptLevel::O0,
            loop_unroll: false,
            inline: false,
            dce: false,
            const_fold: false,
            tail_call: false,
            target_cpu: "generic".to_string(),
            extra_flags: Vec::new(),
        }
    }

    /// Returns a list of enabled pass names for diagnostic output.
    pub fn enabled_passes(&self) -> Vec<&'static str> {
        let mut passes = Vec::new();
        if self.const_fold {
            passes.push("constant-folding");
        }
        if self.dce {
            passes.push("dead-code-elimination");
        }
        if self.inline {
            passes.push("function-inlining");
        }
        if self.loop_unroll {
            passes.push("loop-unrolling");
        }
        if self.tail_call {
            passes.push("tail-call-optimization");
        }
        passes
    }
}

impl Default for KernelOptConfig {
    fn default() -> Self {
        Self::default_o2()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// LTO Config
// ═══════════════════════════════════════════════════════════════════════

/// LTO mode for kernel module linking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LtoMode {
    /// No LTO.
    None,
    /// Thin LTO (parallel, faster build).
    Thin,
    /// Full LTO (maximum optimization, slower build).
    Full,
}

/// Link-time optimization configuration for kernel modules.
#[derive(Debug, Clone)]
pub struct LtoConfig {
    /// LTO mode.
    pub mode: LtoMode,
    /// Modules to include in LTO.
    pub modules: Vec<String>,
    /// Cross-module inlining budget (max inlined instructions).
    pub inline_budget: u32,
    /// Enable interprocedural constant propagation.
    pub ip_const_prop: bool,
}

impl LtoConfig {
    /// Creates a new LTO config with the given mode.
    pub fn new(mode: LtoMode) -> Self {
        Self {
            mode,
            modules: Vec::new(),
            inline_budget: 1000,
            ip_const_prop: true,
        }
    }

    /// Adds a module to the LTO set.
    pub fn add_module(&mut self, name: &str) {
        self.modules.push(name.to_string());
    }

    /// Returns the number of modules in the LTO set.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Returns true if LTO is enabled.
    pub fn is_enabled(&self) -> bool {
        self.mode != LtoMode::None
    }
}

impl Default for LtoConfig {
    fn default() -> Self {
        Self::new(LtoMode::Thin)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Compile-time Kernel Data
// ═══════════════════════════════════════════════════════════════════════

/// A compile-time constant data entry placed in .rodata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEntry {
    /// Entry name (e.g., "boot_page_table", "gdt").
    pub name: String,
    /// Section name (e.g., ".rodata", ".data.rel.ro").
    pub section: String,
    /// Raw bytes.
    pub data: Vec<u8>,
    /// Alignment in bytes.
    pub align: u32,
}

/// Compile-time page tables, GDT entries, and other constant kernel data.
///
/// Generates data that can be placed directly in .rodata sections,
/// avoiding runtime initialization overhead.
#[derive(Debug, Clone)]
pub struct ConstKernelData {
    /// Named constant entries.
    entries: Vec<ConstEntry>,
}

impl ConstKernelData {
    /// Creates a new empty constant data collection.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds a constant data entry.
    pub fn add_entry(&mut self, name: &str, section: &str, data: Vec<u8>, align: u32) {
        self.entries.push(ConstEntry {
            name: name.to_string(),
            section: section.to_string(),
            data,
            align,
        });
    }

    /// Generates a compile-time identity page table (vaddr == paddr) for the
    /// given number of pages. Each entry is 8 bytes (u64).
    pub fn generate_identity_page_table(&mut self, num_pages: u64) {
        let page_size: u64 = 4096;
        let mut data = Vec::with_capacity(num_pages as usize * 8);
        for i in 0..num_pages {
            let entry = (i * page_size) | 0x03; // Present + Writable
            data.extend_from_slice(&entry.to_le_bytes());
        }
        self.add_entry("boot_page_table", ".rodata", data, 4096);
    }

    /// Generates a compile-time GDT with null, code, and data segments.
    pub fn generate_gdt(&mut self) {
        let mut data = Vec::with_capacity(24); // 3 entries * 8 bytes
        // Null descriptor
        data.extend_from_slice(&0u64.to_le_bytes());
        // Code segment: base=0, limit=0xFFFFF, 64-bit, executable, present
        let code_desc: u64 = 0x00AF_9B00_0000_FFFF;
        data.extend_from_slice(&code_desc.to_le_bytes());
        // Data segment: base=0, limit=0xFFFFF, read/write, present
        let data_desc: u64 = 0x00CF_9300_0000_FFFF;
        data.extend_from_slice(&data_desc.to_le_bytes());
        self.add_entry("gdt", ".rodata", data, 16);
    }

    /// Returns the number of constant entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the total size of all constant data in bytes.
    pub fn total_size(&self) -> usize {
        self.entries.iter().map(|e| e.data.len()).sum()
    }

    /// Returns entries as a slice for inspection.
    pub fn entries(&self) -> &[ConstEntry] {
        &self.entries
    }
}

impl Default for ConstKernelData {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Dead Kernel Code Elimination
// ═══════════════════════════════════════════════════════════════════════

/// Result of dead code elimination analysis.
#[derive(Debug, Clone)]
pub struct ElimReport {
    /// Functions removed.
    pub removed_functions: Vec<String>,
    /// Bytes saved estimate.
    pub bytes_saved: u64,
    /// Total functions analyzed.
    pub total_analyzed: u64,
}

/// Analyzes and removes unused kernel code paths.
///
/// Builds a call graph from entry points (e.g., syscall handlers, IRQ handlers)
/// and marks unreachable functions for removal.
#[derive(Debug, Clone)]
pub struct DeadKernelCodeElim {
    /// Entry points (always reachable).
    entry_points: Vec<String>,
    /// Call graph: caller -> set of callees.
    call_graph: HashMap<String, Vec<String>>,
    /// All known functions.
    all_functions: Vec<String>,
}

impl DeadKernelCodeElim {
    /// Creates a new dead code elimination analyzer.
    pub fn new() -> Self {
        Self {
            entry_points: Vec::new(),
            call_graph: HashMap::new(),
            all_functions: Vec::new(),
        }
    }

    /// Registers an entry point (always considered reachable).
    pub fn add_entry_point(&mut self, name: &str) {
        self.entry_points.push(name.to_string());
    }

    /// Registers a function.
    pub fn add_function(&mut self, name: &str) {
        self.all_functions.push(name.to_string());
    }

    /// Records a call edge: `caller` calls `callee`.
    pub fn add_call(&mut self, caller: &str, callee: &str) {
        self.call_graph
            .entry(caller.to_string())
            .or_default()
            .push(callee.to_string());
    }

    /// Runs reachability analysis and returns the elimination report.
    pub fn analyze(&self) -> ElimReport {
        let mut reachable = std::collections::HashSet::new();
        let mut work = self.entry_points.clone();

        while let Some(func) = work.pop() {
            if reachable.contains(&func) {
                continue;
            }
            reachable.insert(func.clone());
            if let Some(callees) = self.call_graph.get(&func) {
                for callee in callees {
                    if !reachable.contains(callee) {
                        work.push(callee.clone());
                    }
                }
            }
        }

        let removed: Vec<String> = self
            .all_functions
            .iter()
            .filter(|f| !reachable.contains(*f))
            .cloned()
            .collect();

        // Estimate 100 bytes per removed function (simplistic).
        let bytes_saved = removed.len() as u64 * 100;

        ElimReport {
            removed_functions: removed,
            bytes_saved,
            total_analyzed: self.all_functions.len() as u64,
        }
    }
}

impl Default for DeadKernelCodeElim {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Inline Critical Path
// ═══════════════════════════════════════════════════════════════════════

/// Inlining decision for a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDecision {
    /// Always inline (IRQ handlers, hot syscalls).
    AlwaysInline,
    /// Inline if the call site is hot.
    InlineHint,
    /// Never inline (large functions, cold paths).
    NoInline,
}

/// Analyzes and decides inlining for critical kernel paths.
#[derive(Debug, Clone)]
pub struct InlineCriticalPath {
    /// Function -> (instruction count, call count, decision).
    functions: HashMap<String, (u64, u64, InlineDecision)>,
    /// Threshold: functions with fewer instructions than this get AlwaysInline.
    small_fn_threshold: u64,
    /// Threshold: functions called more than this get InlineHint.
    hot_call_threshold: u64,
}

impl InlineCriticalPath {
    /// Creates a new inlining analyzer with default thresholds.
    pub fn new(small_fn_threshold: u64, hot_call_threshold: u64) -> Self {
        Self {
            functions: HashMap::new(),
            small_fn_threshold,
            hot_call_threshold,
        }
    }

    /// Registers a function with its instruction count and call count.
    pub fn register(&mut self, name: &str, insn_count: u64, call_count: u64) {
        let decision = if insn_count <= self.small_fn_threshold {
            InlineDecision::AlwaysInline
        } else if call_count >= self.hot_call_threshold {
            InlineDecision::InlineHint
        } else {
            InlineDecision::NoInline
        };
        self.functions
            .insert(name.to_string(), (insn_count, call_count, decision));
    }

    /// Returns the inlining decision for a function.
    pub fn decision(&self, name: &str) -> Option<InlineDecision> {
        self.functions.get(name).map(|&(_, _, d)| d)
    }

    /// Returns all functions marked AlwaysInline.
    pub fn always_inline_functions(&self) -> Vec<String> {
        self.functions
            .iter()
            .filter(|&(_, &(_, _, d))| d == InlineDecision::AlwaysInline)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Returns the total number of registered functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Stack Usage Audit
// ═══════════════════════════════════════════════════════════════════════

/// Audits kernel stack usage to ensure all functions stay within limits.
#[derive(Debug, Clone)]
pub struct StackUsageAudit {
    /// Function -> estimated stack bytes.
    stack_usage: HashMap<String, u64>,
    /// Maximum allowed stack bytes (default 8KB).
    limit: u64,
}

impl StackUsageAudit {
    /// Creates a new stack audit with the given limit in bytes.
    pub fn new(limit: u64) -> Self {
        Self {
            stack_usage: HashMap::new(),
            limit,
        }
    }

    /// Records the estimated stack usage for a function.
    pub fn record(&mut self, func: &str, bytes: u64) {
        self.stack_usage.insert(func.to_string(), bytes);
    }

    /// Checks all functions against the limit.
    pub fn check(&self) -> Vec<KernelOptError> {
        let mut errors = Vec::new();
        for (func, &used) in &self.stack_usage {
            if used > self.limit {
                errors.push(KernelOptError::StackOverflow {
                    func: func.clone(),
                    used,
                    limit: self.limit,
                });
            }
        }
        errors
    }

    /// Returns the maximum stack usage across all functions.
    pub fn max_usage(&self) -> u64 {
        self.stack_usage.values().copied().max().unwrap_or(0)
    }

    /// Returns the number of tracked functions.
    pub fn function_count(&self) -> usize {
        self.stack_usage.len()
    }

    /// Returns the configured stack limit.
    pub fn limit(&self) -> u64 {
        self.limit
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Cache-Friendly Layout
// ═══════════════════════════════════════════════════════════════════════

/// A field in a struct layout analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    /// Field name.
    pub name: String,
    /// Field size in bytes.
    pub size: u64,
    /// Access frequency (higher = hotter).
    pub access_count: u64,
}

/// Analysis result for a struct layout.
#[derive(Debug, Clone)]
pub struct LayoutAnalysis {
    /// Struct name.
    pub struct_name: String,
    /// Recommended hot fields (should be first in cache line).
    pub hot_fields: Vec<String>,
    /// Recommended cold fields (can be at the end).
    pub cold_fields: Vec<String>,
    /// Estimated padding bytes that could be saved.
    pub padding_saved: u64,
}

/// Analyzes struct layouts for cache friendliness.
///
/// Separates hot (frequently accessed) and cold (rarely accessed) fields,
/// and recommends reordering for better cache line utilization.
#[derive(Debug, Clone)]
pub struct CacheFriendlyLayout {
    /// Struct definitions: name -> fields.
    structs: HashMap<String, Vec<FieldInfo>>,
    /// Access frequency threshold for "hot" classification.
    hot_threshold: u64,
}

impl CacheFriendlyLayout {
    /// Creates a new layout analyzer with the given hot threshold.
    pub fn new(hot_threshold: u64) -> Self {
        Self {
            structs: HashMap::new(),
            hot_threshold,
        }
    }

    /// Registers a struct with its fields.
    pub fn add_struct(&mut self, name: &str, fields: Vec<FieldInfo>) {
        self.structs.insert(name.to_string(), fields);
    }

    /// Analyzes a struct and returns layout recommendations.
    pub fn analyze(&self, struct_name: &str) -> Option<LayoutAnalysis> {
        let fields = self.structs.get(struct_name)?;

        let mut hot = Vec::new();
        let mut cold = Vec::new();

        for field in fields {
            if field.access_count >= self.hot_threshold {
                hot.push(field.name.clone());
            } else {
                cold.push(field.name.clone());
            }
        }

        // Estimate padding savings from reordering: sort by size descending.
        let mut sorted_sizes: Vec<u64> = fields.iter().map(|f| f.size).collect();
        sorted_sizes.sort_unstable_by(|a, b| b.cmp(a));
        let original_size: u64 = fields.iter().map(|f| f.size).sum();
        // Simple heuristic: we save ~alignment gaps by sorting.
        let padding_saved = if original_size > 0 && fields.len() > 2 {
            // Rough estimate: 1 byte saved per 4 fields on average.
            (fields.len() as u64 / 4).max(1)
        } else {
            0
        };

        Some(LayoutAnalysis {
            struct_name: struct_name.to_string(),
            hot_fields: hot,
            cold_fields: cold,
            padding_saved,
        })
    }

    /// Returns the number of tracked structs.
    pub fn struct_count(&self) -> usize {
        self.structs.len()
    }
}

impl Default for CacheFriendlyLayout {
    fn default() -> Self {
        Self::new(10)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Zero-Copy IPC
// ═══════════════════════════════════════════════════════════════════════

/// A shared memory region for zero-copy IPC.
#[derive(Debug, Clone)]
pub struct SharedRegion {
    /// Region ID.
    pub id: u64,
    /// Base virtual address.
    pub base_vaddr: u64,
    /// Size in bytes.
    pub size: u64,
    /// Owner process ID.
    pub owner_pid: u64,
    /// Set of process IDs that have access.
    pub accessors: std::collections::HashSet<u64>,
}

/// Manages shared memory pages for large IPC messages.
///
/// Instead of copying data, maps the same physical pages into
/// multiple address spaces.
#[derive(Debug, Clone)]
pub struct ZeroCopyIpc {
    /// Active shared regions by ID.
    regions: HashMap<u64, SharedRegion>,
    /// Next region ID.
    next_id: u64,
}

impl ZeroCopyIpc {
    /// Creates a new zero-copy IPC manager.
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
            next_id: 1,
        }
    }

    /// Creates a shared region owned by the given process.
    pub fn create_region(&mut self, base_vaddr: u64, size: u64, owner_pid: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut accessors = std::collections::HashSet::new();
        accessors.insert(owner_pid);
        self.regions.insert(
            id,
            SharedRegion {
                id,
                base_vaddr,
                size,
                owner_pid,
                accessors,
            },
        );
        id
    }

    /// Grants access to a shared region for another process.
    pub fn grant_access(&mut self, region_id: u64, pid: u64) -> KernelOptResult<()> {
        let region = self
            .regions
            .get_mut(&region_id)
            .ok_or(KernelOptError::SharedMemoryError {
                reason: format!("region {} not found", region_id),
            })?;
        region.accessors.insert(pid);
        Ok(())
    }

    /// Revokes access from a shared region.
    pub fn revoke_access(&mut self, region_id: u64, pid: u64) -> KernelOptResult<()> {
        let region = self
            .regions
            .get_mut(&region_id)
            .ok_or(KernelOptError::SharedMemoryError {
                reason: format!("region {} not found", region_id),
            })?;
        if pid == region.owner_pid {
            return Err(KernelOptError::SharedMemoryError {
                reason: "cannot revoke owner access".to_string(),
            });
        }
        region.accessors.remove(&pid);
        Ok(())
    }

    /// Destroys a shared region (only owner can do this).
    pub fn destroy_region(&mut self, region_id: u64, pid: u64) -> KernelOptResult<()> {
        let region = self
            .regions
            .get(&region_id)
            .ok_or(KernelOptError::SharedMemoryError {
                reason: format!("region {} not found", region_id),
            })?;
        if region.owner_pid != pid {
            return Err(KernelOptError::SharedMemoryError {
                reason: "only owner can destroy region".to_string(),
            });
        }
        self.regions.remove(&region_id);
        Ok(())
    }

    /// Returns the number of active shared regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Checks if a process has access to a region.
    pub fn has_access(&self, region_id: u64, pid: u64) -> bool {
        self.regions
            .get(&region_id)
            .map(|r| r.accessors.contains(&pid))
            .unwrap_or(false)
    }
}

impl Default for ZeroCopyIpc {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Lock-Free Queue
// ═══════════════════════════════════════════════════════════════════════

/// Simulated lock-free SPSC (Single Producer, Single Consumer) queue.
///
/// Uses a ring buffer with separate read/write positions.
/// In a real implementation this would use atomics; here we simulate
/// the semantics for correctness testing.
#[derive(Debug, Clone)]
pub struct LockFreeQueue<T: Clone> {
    /// Ring buffer storage.
    buffer: Vec<Option<T>>,
    /// Write position (producer).
    write_pos: usize,
    /// Read position (consumer).
    read_pos: usize,
    /// Capacity of the queue.
    capacity: usize,
}

impl<T: Clone> LockFreeQueue<T> {
    /// Creates a new lock-free queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![None; capacity],
            write_pos: 0,
            read_pos: 0,
            capacity,
        }
    }

    /// Pushes an item into the queue (producer side).
    pub fn push(&mut self, item: T) -> KernelOptResult<()> {
        let next = (self.write_pos + 1) % self.capacity;
        if next == self.read_pos {
            return Err(KernelOptError::QueueFull {
                capacity: self.capacity,
            });
        }
        self.buffer[self.write_pos] = Some(item);
        self.write_pos = next;
        Ok(())
    }

    /// Pops an item from the queue (consumer side).
    pub fn pop(&mut self) -> KernelOptResult<T> {
        if self.read_pos == self.write_pos {
            return Err(KernelOptError::QueueEmpty);
        }
        let item = self.buffer[self.read_pos]
            .take()
            .ok_or(KernelOptError::QueueEmpty)?;
        self.read_pos = (self.read_pos + 1) % self.capacity;
        Ok(item)
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            self.capacity - self.read_pos + self.write_pos
        }
    }

    /// Returns the capacity of the queue.
    pub fn capacity(&self) -> usize {
        // Usable capacity is capacity - 1 (one slot wasted for full detection).
        self.capacity - 1
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Kernel Benchmark Suite
// ═══════════════════════════════════════════════════════════════════════

/// A single benchmark measurement.
#[derive(Debug, Clone)]
pub struct BenchMeasurement {
    /// Benchmark name.
    pub name: String,
    /// Measured value (nanoseconds or cycles).
    pub value_ns: u64,
    /// Unit description.
    pub unit: String,
}

/// Benchmark suite for kernel performance metrics.
///
/// Simulates measurements for boot time, context switch,
/// syscall latency, and IPC throughput.
#[derive(Debug, Clone)]
pub struct KernelBenchSuite {
    /// Recorded measurements.
    measurements: Vec<BenchMeasurement>,
}

impl KernelBenchSuite {
    /// Creates a new empty benchmark suite.
    pub fn new() -> Self {
        Self {
            measurements: Vec::new(),
        }
    }

    /// Records a benchmark measurement.
    pub fn record(&mut self, name: &str, value_ns: u64, unit: &str) {
        self.measurements.push(BenchMeasurement {
            name: name.to_string(),
            value_ns,
            unit: unit.to_string(),
        });
    }

    /// Simulates a boot time measurement.
    pub fn measure_boot_time(&mut self, kernel_size_kb: u64) {
        // Simulated: ~1ms per 100KB of kernel image.
        let boot_ns = (kernel_size_kb / 100 + 1) * 1_000_000;
        self.record("boot_time", boot_ns, "ns");
    }

    /// Simulates a context switch measurement.
    pub fn measure_context_switch(&mut self, num_registers: u64) {
        // Simulated: ~50ns per register saved/restored.
        let switch_ns = num_registers * 50;
        self.record("context_switch", switch_ns, "ns");
    }

    /// Simulates a syscall latency measurement.
    pub fn measure_syscall_latency(&mut self, num_args: u64) {
        // Simulated: ~200ns base + 20ns per argument.
        let latency_ns = 200 + num_args * 20;
        self.record("syscall_latency", latency_ns, "ns");
    }

    /// Returns all measurements.
    pub fn measurements(&self) -> &[BenchMeasurement] {
        &self.measurements
    }

    /// Returns the measurement count.
    pub fn count(&self) -> usize {
        self.measurements.len()
    }

    /// Generates a summary report.
    pub fn summary(&self) -> String {
        let mut s = String::from("=== Kernel Benchmarks ===\n");
        for m in &self.measurements {
            s.push_str(&format!("  {}: {} {}\n", m.name, m.value_ns, m.unit));
        }
        s
    }
}

impl Default for KernelBenchSuite {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── OptLevel ──

    #[test]
    fn opt_level_parse() {
        assert_eq!(OptLevel::from_str_opt("O2").unwrap(), OptLevel::O2);
        assert_eq!(OptLevel::from_str_opt("3").unwrap(), OptLevel::O3);
        assert!(OptLevel::from_str_opt("O9").is_err());
    }

    // ── KernelOptConfig ──

    #[test]
    fn kernel_opt_config_default_o2() {
        let cfg = KernelOptConfig::default_o2();
        assert_eq!(cfg.opt_level, OptLevel::O2);
        assert!(cfg.loop_unroll);
        assert!(cfg.inline);
        let passes = cfg.enabled_passes();
        assert!(passes.contains(&"function-inlining"));
        assert!(passes.contains(&"dead-code-elimination"));
    }

    #[test]
    fn kernel_opt_config_debug() {
        let cfg = KernelOptConfig::debug();
        assert_eq!(cfg.opt_level, OptLevel::O0);
        assert!(cfg.enabled_passes().is_empty());
    }

    // ── LtoConfig ──

    #[test]
    fn lto_config_thin() {
        let mut lto = LtoConfig::new(LtoMode::Thin);
        lto.add_module("kernel_core");
        lto.add_module("scheduler");
        assert_eq!(lto.module_count(), 2);
        assert!(lto.is_enabled());
    }

    #[test]
    fn lto_config_none_disabled() {
        let lto = LtoConfig::new(LtoMode::None);
        assert!(!lto.is_enabled());
    }

    // ── ConstKernelData ──

    #[test]
    fn const_data_identity_page_table() {
        let mut data = ConstKernelData::new();
        data.generate_identity_page_table(4);
        assert_eq!(data.entry_count(), 1);
        assert_eq!(data.total_size(), 32); // 4 pages * 8 bytes each
        let entry = &data.entries()[0];
        assert_eq!(entry.name, "boot_page_table");
        assert_eq!(entry.align, 4096);
    }

    #[test]
    fn const_data_gdt() {
        let mut data = ConstKernelData::new();
        data.generate_gdt();
        assert_eq!(data.entry_count(), 1);
        assert_eq!(data.total_size(), 24); // 3 GDT entries * 8 bytes
    }

    // ── DeadKernelCodeElim ──

    #[test]
    fn dead_code_elim_removes_unreachable() {
        let mut dce = DeadKernelCodeElim::new();
        dce.add_entry_point("main");
        dce.add_function("main");
        dce.add_function("used_fn");
        dce.add_function("dead_fn");
        dce.add_call("main", "used_fn");

        let report = dce.analyze();
        assert_eq!(report.removed_functions, vec!["dead_fn"]);
        assert_eq!(report.bytes_saved, 100);
        assert_eq!(report.total_analyzed, 3);
    }

    #[test]
    fn dead_code_elim_transitive_reachability() {
        let mut dce = DeadKernelCodeElim::new();
        dce.add_entry_point("entry");
        dce.add_function("entry");
        dce.add_function("helper_a");
        dce.add_function("helper_b");
        dce.add_function("orphan");
        dce.add_call("entry", "helper_a");
        dce.add_call("helper_a", "helper_b");

        let report = dce.analyze();
        assert_eq!(report.removed_functions.len(), 1);
        assert!(report.removed_functions.contains(&"orphan".to_string()));
    }

    // ── InlineCriticalPath ──

    #[test]
    fn inline_small_function_always() {
        let mut analyzer = InlineCriticalPath::new(10, 100);
        analyzer.register("tiny_irq", 5, 1);
        assert_eq!(
            analyzer.decision("tiny_irq"),
            Some(InlineDecision::AlwaysInline)
        );
    }

    #[test]
    fn inline_hot_function_hint() {
        let mut analyzer = InlineCriticalPath::new(10, 100);
        analyzer.register("hot_syscall", 50, 200);
        assert_eq!(
            analyzer.decision("hot_syscall"),
            Some(InlineDecision::InlineHint)
        );
    }

    #[test]
    fn inline_cold_function_noinline() {
        let mut analyzer = InlineCriticalPath::new(10, 100);
        analyzer.register("cold_init", 500, 1);
        assert_eq!(
            analyzer.decision("cold_init"),
            Some(InlineDecision::NoInline)
        );
    }

    // ── StackUsageAudit ──

    #[test]
    fn stack_audit_within_limit() {
        let mut audit = StackUsageAudit::new(8192);
        audit.record("irq_handler", 512);
        audit.record("syscall_dispatch", 2048);
        assert!(audit.check().is_empty());
        assert_eq!(audit.max_usage(), 2048);
    }

    #[test]
    fn stack_audit_exceeds_limit() {
        let mut audit = StackUsageAudit::new(8192);
        audit.record("recursive_fn", 16384);
        let errors = audit.check();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            KernelOptError::StackOverflow { func, used: 16384, limit: 8192 }
            if func == "recursive_fn"
        ));
    }

    // ── CacheFriendlyLayout ──

    #[test]
    fn cache_layout_hot_cold_separation() {
        let mut layout = CacheFriendlyLayout::new(10);
        layout.add_struct(
            "TaskStruct",
            vec![
                FieldInfo {
                    name: "pid".into(),
                    size: 4,
                    access_count: 100,
                },
                FieldInfo {
                    name: "state".into(),
                    size: 4,
                    access_count: 50,
                },
                FieldInfo {
                    name: "debug_info".into(),
                    size: 64,
                    access_count: 1,
                },
            ],
        );
        let analysis = layout.analyze("TaskStruct").unwrap();
        assert!(analysis.hot_fields.contains(&"pid".to_string()));
        assert!(analysis.hot_fields.contains(&"state".to_string()));
        assert!(analysis.cold_fields.contains(&"debug_info".to_string()));
    }

    // ── ZeroCopyIpc ──

    #[test]
    fn zero_copy_create_and_share() {
        let mut ipc = ZeroCopyIpc::new();
        let id = ipc.create_region(0x1000, 4096, 1);
        assert!(ipc.has_access(id, 1));
        assert!(!ipc.has_access(id, 2));
        assert!(ipc.grant_access(id, 2).is_ok());
        assert!(ipc.has_access(id, 2));
    }

    #[test]
    fn zero_copy_owner_cannot_revoke_self() {
        let mut ipc = ZeroCopyIpc::new();
        let id = ipc.create_region(0x2000, 8192, 1);
        assert!(ipc.revoke_access(id, 1).is_err());
    }

    #[test]
    fn zero_copy_destroy_only_by_owner() {
        let mut ipc = ZeroCopyIpc::new();
        let id = ipc.create_region(0x3000, 4096, 1);
        assert!(ipc.destroy_region(id, 2).is_err());
        assert!(ipc.destroy_region(id, 1).is_ok());
        assert_eq!(ipc.region_count(), 0);
    }

    // ── LockFreeQueue ──

    #[test]
    fn queue_push_pop() {
        let mut q: LockFreeQueue<u32> = LockFreeQueue::new(4);
        assert!(q.push(1).is_ok());
        assert!(q.push(2).is_ok());
        assert!(q.push(3).is_ok());
        assert!(q.push(4).is_err()); // Full (capacity - 1 usable)
        assert_eq!(q.pop().unwrap(), 1);
        assert_eq!(q.pop().unwrap(), 2);
        assert_eq!(q.pop().unwrap(), 3);
        assert!(q.pop().is_err()); // Empty
    }

    #[test]
    fn queue_len_and_empty() {
        let mut q: LockFreeQueue<i32> = LockFreeQueue::new(8);
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.push(42).is_ok());
        assert!(!q.is_empty());
        assert_eq!(q.len(), 1);
    }

    // ── KernelBenchSuite ──

    #[test]
    fn bench_suite_measurements() {
        let mut bench = KernelBenchSuite::new();
        bench.measure_boot_time(500);
        bench.measure_context_switch(16);
        bench.measure_syscall_latency(4);
        assert_eq!(bench.count(), 3);
        let summary = bench.summary();
        assert!(summary.contains("boot_time"));
        assert!(summary.contains("context_switch"));
        assert!(summary.contains("syscall_latency"));
    }
}
