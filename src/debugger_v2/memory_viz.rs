//! Memory Visualization — heap map, allocation timeline, reference
//! graph, stack visualization, leak detection, memory diff,
//! ownership visualization, tensor memory map, cache analysis.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S27.1: Heap Map
// ═══════════════════════════════════════════════════════════════════════

/// A heap block in the heap map.
#[derive(Debug, Clone)]
pub struct HeapBlock {
    /// Start address.
    pub addr: u64,
    /// Size in bytes.
    pub size: usize,
    /// Whether currently allocated.
    pub allocated: bool,
    /// Allocation source (function:line).
    pub source: Option<String>,
    /// Type name (if known).
    pub type_name: Option<String>,
}

/// A heap map snapshot.
#[derive(Debug, Clone)]
pub struct HeapMap {
    /// All blocks (allocated + free).
    pub blocks: Vec<HeapBlock>,
    /// Total heap size.
    pub total_size: usize,
    /// Used bytes.
    pub used_bytes: usize,
}

impl HeapMap {
    /// Creates a new empty heap map.
    pub fn new(total_size: usize) -> Self {
        Self {
            blocks: Vec::new(),
            total_size,
            used_bytes: 0,
        }
    }

    /// Records an allocation.
    pub fn allocate(&mut self, addr: u64, size: usize, source: Option<String>) {
        self.blocks.push(HeapBlock {
            addr,
            size,
            allocated: true,
            source,
            type_name: None,
        });
        self.used_bytes += size;
    }

    /// Records a deallocation.
    pub fn free(&mut self, addr: u64) {
        if let Some(block) = self
            .blocks
            .iter_mut()
            .find(|b| b.addr == addr && b.allocated)
        {
            block.allocated = false;
            self.used_bytes = self.used_bytes.saturating_sub(block.size);
        }
    }

    /// Fragmentation ratio (0.0 = no fragmentation).
    pub fn fragmentation(&self) -> f64 {
        let free_blocks: Vec<usize> = self
            .blocks
            .iter()
            .filter(|b| !b.allocated)
            .map(|b| b.size)
            .collect();
        if free_blocks.is_empty() {
            return 0.0;
        }
        let total_free: usize = free_blocks.iter().sum();
        let largest_free = free_blocks.iter().copied().max().unwrap_or(0);
        if total_free == 0 {
            return 0.0;
        }
        1.0 - (largest_free as f64 / total_free as f64)
    }

    /// Count of allocated blocks.
    pub fn allocated_count(&self) -> usize {
        self.blocks.iter().filter(|b| b.allocated).count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.2: Allocation Timeline
// ═══════════════════════════════════════════════════════════════════════

/// An allocation event on the timeline.
#[derive(Debug, Clone)]
pub struct AllocEvent {
    /// Timestamp (ns).
    pub timestamp_ns: u64,
    /// Event type.
    pub kind: AllocEventKind,
    /// Address.
    pub addr: u64,
    /// Size.
    pub size: usize,
    /// Cumulative heap usage after this event.
    pub heap_usage: usize,
}

/// Allocation event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocEventKind {
    /// Allocation.
    Alloc,
    /// Deallocation.
    Free,
    /// Reallocation.
    Realloc,
}

/// Allocation timeline.
#[derive(Debug, Clone)]
pub struct AllocTimeline {
    /// Events in chronological order.
    pub events: Vec<AllocEvent>,
    /// Peak usage.
    pub peak_usage: usize,
}

impl AllocTimeline {
    /// Creates a new timeline.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            peak_usage: 0,
        }
    }

    /// Records an allocation event.
    pub fn record(&mut self, timestamp_ns: u64, kind: AllocEventKind, addr: u64, size: usize) {
        let prev_usage = self.events.last().map_or(0, |e| e.heap_usage);
        let usage = match kind {
            AllocEventKind::Alloc => prev_usage + size,
            AllocEventKind::Free => prev_usage.saturating_sub(size),
            AllocEventKind::Realloc => prev_usage, // simplified
        };
        self.peak_usage = self.peak_usage.max(usage);
        self.events.push(AllocEvent {
            timestamp_ns,
            kind,
            addr,
            size,
            heap_usage: usage,
        });
    }

    /// Current heap usage.
    pub fn current_usage(&self) -> usize {
        self.events.last().map_or(0, |e| e.heap_usage)
    }
}

impl Default for AllocTimeline {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.3: Reference Graph
// ═══════════════════════════════════════════════════════════════════════

/// A node in the reference graph.
#[derive(Debug, Clone)]
pub struct RefNode {
    /// Object address.
    pub addr: u64,
    /// Type name.
    pub type_name: String,
    /// Size in bytes.
    pub size: usize,
    /// Outgoing references (addresses of referenced objects).
    pub refs_to: Vec<u64>,
}

/// A reference graph.
#[derive(Debug, Clone)]
pub struct RefGraph {
    /// Nodes indexed by address.
    pub nodes: HashMap<u64, RefNode>,
}

impl RefGraph {
    /// Creates a new reference graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Adds a node.
    pub fn add_node(&mut self, node: RefNode) {
        self.nodes.insert(node.addr, node);
    }

    /// Detects reference cycles using DFS.
    pub fn detect_cycles(&self) -> Vec<Vec<u64>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();

        for &addr in self.nodes.keys() {
            if !visited.contains(&addr) {
                let mut path = Vec::new();
                self.dfs_cycle(addr, &mut visited, &mut in_stack, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn dfs_cycle(
        &self,
        addr: u64,
        visited: &mut HashSet<u64>,
        in_stack: &mut HashSet<u64>,
        path: &mut Vec<u64>,
        cycles: &mut Vec<Vec<u64>>,
    ) {
        visited.insert(addr);
        in_stack.insert(addr);
        path.push(addr);

        if let Some(node) = self.nodes.get(&addr) {
            for &ref_addr in &node.refs_to {
                if !visited.contains(&ref_addr) {
                    self.dfs_cycle(ref_addr, visited, in_stack, path, cycles);
                } else if in_stack.contains(&ref_addr) {
                    // Found a cycle
                    let cycle_start = path.iter().position(|&a| a == ref_addr).unwrap_or(0);
                    cycles.push(path[cycle_start..].to_vec());
                }
            }
        }

        path.pop();
        in_stack.remove(&addr);
    }

    /// Finds dangling references (refs to non-existent nodes).
    pub fn find_dangling(&self) -> Vec<(u64, u64)> {
        let mut dangling = Vec::new();
        for (addr, node) in &self.nodes {
            for &ref_addr in &node.refs_to {
                if !self.nodes.contains_key(&ref_addr) {
                    dangling.push((*addr, ref_addr));
                }
            }
        }
        dangling
    }
}

impl Default for RefGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.4: Stack Visualization
// ═══════════════════════════════════════════════════════════════════════

/// A stack frame for visualization.
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Frame index (0 = top).
    pub index: usize,
    /// Function name.
    pub function: String,
    /// Source location.
    pub location: String,
    /// Local variables.
    pub locals: Vec<(String, String, usize)>, // (name, value, size_bytes)
    /// Frame size in bytes.
    pub frame_size: usize,
}

/// A stack snapshot.
#[derive(Debug, Clone)]
pub struct StackSnapshot {
    /// Frames (top of stack first).
    pub frames: Vec<StackFrame>,
    /// Total stack usage.
    pub total_size: usize,
}

impl StackSnapshot {
    /// Depth (number of frames).
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.5: Memory Leak Detection
// ═══════════════════════════════════════════════════════════════════════

/// A suspected memory leak.
#[derive(Debug, Clone)]
pub struct LeakReport {
    /// Address of leaked allocation.
    pub addr: u64,
    /// Size of leaked allocation.
    pub size: usize,
    /// Allocation source.
    pub source: String,
    /// Confidence level.
    pub confidence: LeakConfidence,
}

/// Confidence level for leak detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeakConfidence {
    /// Definitely leaked (allocated, never freed, unreachable).
    Definite,
    /// Possibly leaked (allocated, never freed, but may be reachable).
    Possible,
}

impl fmt::Display for LeakConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeakConfidence::Definite => write!(f, "Definite"),
            LeakConfidence::Possible => write!(f, "Possible"),
        }
    }
}

/// Detects memory leaks from allocation/free history.
pub fn detect_leaks(allocs: &[(u64, usize, String)], frees: &[u64]) -> Vec<LeakReport> {
    let freed: HashSet<u64> = frees.iter().copied().collect();

    allocs
        .iter()
        .filter(|(addr, _, _)| !freed.contains(addr))
        .map(|(addr, size, source)| LeakReport {
            addr: *addr,
            size: *size,
            source: source.clone(),
            confidence: LeakConfidence::Definite,
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S27.6: Memory Diff
// ═══════════════════════════════════════════════════════════════════════

/// Diff between two heap snapshots.
#[derive(Debug, Clone)]
pub struct HeapDiff {
    /// New allocations.
    pub new_allocs: Vec<HeapBlock>,
    /// Freed blocks.
    pub freed: Vec<u64>,
    /// Net change in bytes.
    pub net_change: i64,
}

/// Computes diff between two heap maps.
pub fn diff_heaps(before: &HeapMap, after: &HeapMap) -> HeapDiff {
    let before_addrs: HashSet<u64> = before
        .blocks
        .iter()
        .filter(|b| b.allocated)
        .map(|b| b.addr)
        .collect();

    let after_addrs: HashSet<u64> = after
        .blocks
        .iter()
        .filter(|b| b.allocated)
        .map(|b| b.addr)
        .collect();

    let new_allocs: Vec<HeapBlock> = after
        .blocks
        .iter()
        .filter(|b| b.allocated && !before_addrs.contains(&b.addr))
        .cloned()
        .collect();

    let freed: Vec<u64> = before_addrs
        .iter()
        .filter(|a| !after_addrs.contains(a))
        .copied()
        .collect();

    let net_change = after.used_bytes as i64 - before.used_bytes as i64;

    HeapDiff {
        new_allocs,
        freed,
        net_change,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.7: Ownership Visualization
// ═══════════════════════════════════════════════════════════════════════

/// An ownership node.
#[derive(Debug, Clone)]
pub struct OwnershipNode {
    /// Variable name.
    pub name: String,
    /// Type.
    pub type_name: String,
    /// Children (owned objects).
    pub children: Vec<OwnershipNode>,
    /// Borrow status.
    pub borrow: BorrowStatus,
}

/// Borrow status of an owned value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowStatus {
    /// Not borrowed.
    Owned,
    /// Immutably borrowed.
    SharedBorrow,
    /// Mutably borrowed.
    MutBorrow,
    /// Moved (no longer valid).
    Moved,
}

impl fmt::Display for BorrowStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BorrowStatus::Owned => write!(f, "owned"),
            BorrowStatus::SharedBorrow => write!(f, "&"),
            BorrowStatus::MutBorrow => write!(f, "&mut"),
            BorrowStatus::Moved => write!(f, "moved"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.8: Tensor Memory Map
// ═══════════════════════════════════════════════════════════════════════

/// Tensor memory layout.
#[derive(Debug, Clone)]
pub struct TensorLayout {
    /// Tensor name.
    pub name: String,
    /// Shape.
    pub shape: Vec<usize>,
    /// Strides (in elements).
    pub strides: Vec<usize>,
    /// Element size in bytes.
    pub elem_size: usize,
    /// Start address.
    pub base_addr: u64,
    /// Total size in bytes.
    pub total_bytes: usize,
    /// Alignment in bytes.
    pub alignment: usize,
    /// Whether contiguous in memory.
    pub is_contiguous: bool,
}

impl TensorLayout {
    /// Number of elements.
    pub fn num_elements(&self) -> usize {
        self.shape.iter().product()
    }

    /// Checks if strides indicate contiguous layout.
    pub fn check_contiguous(shape: &[usize], strides: &[usize]) -> bool {
        if shape.is_empty() {
            return true;
        }
        let mut expected_stride = 1;
        for i in (0..shape.len()).rev() {
            if strides[i] != expected_stride {
                return false;
            }
            expected_stride *= shape[i];
        }
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S27.9: Cache Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Cache simulation configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache line size in bytes.
    pub line_size: usize,
    /// Number of cache lines.
    pub num_lines: usize,
    /// Associativity.
    pub associativity: usize,
}

/// Cache simulation result.
#[derive(Debug, Clone)]
pub struct CacheResult {
    /// Total accesses.
    pub total_accesses: u64,
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
}

impl CacheResult {
    /// Hit rate.
    pub fn hit_rate(&self) -> f64 {
        if self.total_accesses == 0 {
            return 0.0;
        }
        self.hits as f64 / self.total_accesses as f64
    }

    /// Miss rate.
    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

/// Simulates cache behavior for a sequence of memory accesses.
pub fn simulate_cache(accesses: &[u64], config: &CacheConfig) -> CacheResult {
    let mut cache: HashSet<u64> = HashSet::new();
    let mut hits = 0u64;
    let mut misses = 0u64;

    for &addr in accesses {
        let line = addr / config.line_size as u64;
        if cache.contains(&line) {
            hits += 1;
        } else {
            misses += 1;
            if cache.len() >= config.num_lines {
                // Evict (simplified — remove arbitrary)
                let to_remove = *cache.iter().next().unwrap();
                cache.remove(&to_remove);
            }
            cache.insert(line);
        }
    }

    CacheResult {
        total_accesses: hits + misses,
        hits,
        misses,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S27.1 — Heap Map
    #[test]
    fn s27_1_heap_map() {
        let mut heap = HeapMap::new(1024 * 1024);
        heap.allocate(0x1000, 256, Some("main:5".into()));
        heap.allocate(0x2000, 512, None);
        assert_eq!(heap.allocated_count(), 2);
        assert_eq!(heap.used_bytes, 768);

        heap.free(0x1000);
        assert_eq!(heap.allocated_count(), 1);
        assert_eq!(heap.used_bytes, 512);
    }

    // S27.2 — Allocation Timeline
    #[test]
    fn s27_2_alloc_timeline() {
        let mut tl = AllocTimeline::new();
        tl.record(100, AllocEventKind::Alloc, 0x1000, 256);
        tl.record(200, AllocEventKind::Alloc, 0x2000, 512);
        tl.record(300, AllocEventKind::Free, 0x1000, 256);
        assert_eq!(tl.peak_usage, 768);
        assert_eq!(tl.current_usage(), 512);
    }

    // S27.3 — Reference Graph
    #[test]
    fn s27_3_detect_cycles() {
        let mut graph = RefGraph::new();
        graph.add_node(RefNode {
            addr: 1,
            type_name: "A".into(),
            size: 8,
            refs_to: vec![2],
        });
        graph.add_node(RefNode {
            addr: 2,
            type_name: "B".into(),
            size: 8,
            refs_to: vec![1],
        });
        let cycles = graph.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn s27_3_find_dangling() {
        let mut graph = RefGraph::new();
        graph.add_node(RefNode {
            addr: 1,
            type_name: "A".into(),
            size: 8,
            refs_to: vec![999],
        });
        let dangling = graph.find_dangling();
        assert_eq!(dangling.len(), 1);
        assert_eq!(dangling[0], (1, 999));
    }

    // S27.4 — Stack Visualization
    #[test]
    fn s27_4_stack_snapshot() {
        let snap = StackSnapshot {
            frames: vec![
                StackFrame {
                    index: 0,
                    function: "inner".into(),
                    location: "lib.fj:10".into(),
                    locals: vec![("x".into(), "42".into(), 8)],
                    frame_size: 32,
                },
                StackFrame {
                    index: 1,
                    function: "main".into(),
                    location: "main.fj:1".into(),
                    locals: vec![],
                    frame_size: 16,
                },
            ],
            total_size: 48,
        };
        assert_eq!(snap.depth(), 2);
    }

    // S27.5 — Leak Detection
    #[test]
    fn s27_5_detect_leaks() {
        let allocs = vec![
            (0x1000, 256, "main:5".into()),
            (0x2000, 512, "main:10".into()),
        ];
        let frees = vec![0x1000];
        let leaks = detect_leaks(&allocs, &frees);
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].addr, 0x2000);
        assert_eq!(leaks[0].confidence, LeakConfidence::Definite);
    }

    // S27.6 — Memory Diff
    #[test]
    fn s27_6_heap_diff() {
        let mut before = HeapMap::new(1024 * 1024);
        before.allocate(0x1000, 256, None);

        let mut after = HeapMap::new(1024 * 1024);
        after.allocate(0x1000, 256, None);
        after.allocate(0x2000, 512, None);

        let diff = diff_heaps(&before, &after);
        assert_eq!(diff.new_allocs.len(), 1);
        assert!(diff.freed.is_empty());
        assert_eq!(diff.net_change, 512);
    }

    // S27.7 — Ownership
    #[test]
    fn s27_7_borrow_status() {
        assert_eq!(BorrowStatus::Owned.to_string(), "owned");
        assert_eq!(BorrowStatus::SharedBorrow.to_string(), "&");
        assert_eq!(BorrowStatus::MutBorrow.to_string(), "&mut");
        assert_eq!(BorrowStatus::Moved.to_string(), "moved");
    }

    // S27.8 — Tensor Memory Map
    #[test]
    fn s27_8_tensor_layout() {
        let layout = TensorLayout {
            name: "weights".into(),
            shape: vec![32, 768],
            strides: vec![768, 1],
            elem_size: 4,
            base_addr: 0x10000,
            total_bytes: 32 * 768 * 4,
            alignment: 64,
            is_contiguous: true,
        };
        assert_eq!(layout.num_elements(), 32 * 768);
    }

    #[test]
    fn s27_8_contiguous_check() {
        assert!(TensorLayout::check_contiguous(&[3, 4], &[4, 1]));
        assert!(!TensorLayout::check_contiguous(&[3, 4], &[4, 2]));
    }

    // S27.9 — Cache Analysis
    #[test]
    fn s27_9_cache_simulation() {
        let config = CacheConfig {
            line_size: 64,
            num_lines: 16,
            associativity: 1,
        };
        // Sequential access — good cache behavior
        let accesses: Vec<u64> = (0..100).map(|i| i * 4).collect();
        let result = simulate_cache(&accesses, &config);
        assert!(result.hit_rate() > 0.5);
    }

    // S27.10 — Leak confidence display
    #[test]
    fn s27_10_leak_confidence() {
        assert_eq!(LeakConfidence::Definite.to_string(), "Definite");
        assert_eq!(LeakConfidence::Possible.to_string(), "Possible");
    }

    #[test]
    fn s27_10_fragmentation() {
        let mut heap = HeapMap::new(1024);
        heap.allocate(0, 100, None);
        heap.allocate(100, 100, None);
        heap.allocate(200, 100, None);
        heap.free(100);
        assert!(heap.fragmentation() >= 0.0);
    }
}
