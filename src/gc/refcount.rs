//! Reference counting — Rc<T>, Weak<T>, cycle detection,
//! interior mutability, thread safety, GC statistics.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════════
// S21.1 / S21.2: Rc<T> Type & Clone Semantics
// ═══════════════════════════════════════════════════════════════════════

/// A reference-counted pointer type.
#[derive(Debug, Clone)]
pub struct RcType {
    /// Inner type name.
    pub inner_type: String,
    /// Unique allocation ID.
    pub alloc_id: u64,
    /// Strong reference count.
    pub strong_count: u32,
    /// Weak reference count.
    pub weak_count: u32,
}

impl RcType {
    /// Creates a new Rc with strong_count = 1.
    pub fn new(inner_type: &str, alloc_id: u64) -> Self {
        Self {
            inner_type: inner_type.into(),
            alloc_id,
            strong_count: 1,
            weak_count: 0,
        }
    }

    /// Clones the Rc (increments strong count).
    pub fn clone_rc(&mut self) -> Self {
        self.strong_count += 1;
        Self {
            inner_type: self.inner_type.clone(),
            alloc_id: self.alloc_id,
            strong_count: self.strong_count,
            weak_count: self.weak_count,
        }
    }

    /// Returns the current strong count.
    pub fn strong_count(&self) -> u32 {
        self.strong_count
    }

    /// Returns the current weak count.
    pub fn weak_count(&self) -> u32 {
        self.weak_count
    }
}

impl fmt::Display for RcType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rc<{}>(id={}, strong={}, weak={})",
            self.inner_type, self.alloc_id, self.strong_count, self.weak_count
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.3: Rc Drop
// ═══════════════════════════════════════════════════════════════════════

/// Result of dropping an Rc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropResult {
    /// Count decremented, still alive.
    Decremented { remaining: u32 },
    /// Last strong reference dropped, value deallocated.
    Deallocated,
    /// Weak references remain, inner value dropped but allocation kept.
    WeakOnly { weak_remaining: u32 },
}

/// Simulates dropping an Rc, returning the result.
pub fn drop_rc(rc: &mut RcType) -> DropResult {
    if rc.strong_count > 1 {
        rc.strong_count -= 1;
        DropResult::Decremented {
            remaining: rc.strong_count,
        }
    } else if rc.weak_count > 0 {
        rc.strong_count = 0;
        DropResult::WeakOnly {
            weak_remaining: rc.weak_count,
        }
    } else {
        rc.strong_count = 0;
        DropResult::Deallocated
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.4: Weak<T> Type
// ═══════════════════════════════════════════════════════════════════════

/// A weak reference that does not prevent deallocation.
#[derive(Debug, Clone)]
pub struct WeakRef {
    /// Allocation ID this weak ref points to.
    pub alloc_id: u64,
    /// Inner type name.
    pub inner_type: String,
    /// Whether the strong value is still alive.
    pub alive: bool,
}

impl WeakRef {
    /// Creates a weak reference from an Rc.
    pub fn downgrade(rc: &mut RcType) -> Self {
        rc.weak_count += 1;
        WeakRef {
            alloc_id: rc.alloc_id,
            inner_type: rc.inner_type.clone(),
            alive: rc.strong_count > 0,
        }
    }

    /// Attempts to upgrade to a strong reference.
    pub fn upgrade(&self) -> UpgradeResult {
        if self.alive {
            UpgradeResult::Some(self.alloc_id)
        } else {
            UpgradeResult::None
        }
    }
}

/// Result of upgrading a Weak reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeResult {
    /// Upgrade succeeded, returns allocation ID.
    Some(u64),
    /// Inner value already dropped.
    None,
}

// ═══════════════════════════════════════════════════════════════════════
// S21.5: Cycle Detection
// ═══════════════════════════════════════════════════════════════════════

/// An edge in the reference graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefEdge {
    /// Source allocation ID.
    pub from: u64,
    /// Target allocation ID.
    pub to: u64,
}

/// A cycle collector for Rc graphs.
#[derive(Debug, Clone, Default)]
pub struct CycleCollector {
    /// Reference edges in the graph.
    edges: Vec<RefEdge>,
    /// Known alive allocations.
    alive: HashSet<u64>,
}

impl CycleCollector {
    /// Creates a new cycle collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a reference edge.
    pub fn add_edge(&mut self, from: u64, to: u64) {
        self.edges.push(RefEdge { from, to });
        self.alive.insert(from);
        self.alive.insert(to);
    }

    /// Registers an alive allocation.
    pub fn mark_alive(&mut self, id: u64) {
        self.alive.insert(id);
    }

    /// Detects cycles in the reference graph using DFS.
    pub fn detect_cycles(&self) -> Vec<Vec<u64>> {
        let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();
        for edge in &self.edges {
            adjacency.entry(edge.from).or_default().push(edge.to);
        }

        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();

        for &node in &self.alive {
            if !visited.contains(&node) {
                let mut path = Vec::new();
                self.dfs_cycles(
                    node,
                    &adjacency,
                    &mut visited,
                    &mut on_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }
        cycles
    }

    fn dfs_cycles(
        &self,
        node: u64,
        adj: &HashMap<u64, Vec<u64>>,
        visited: &mut HashSet<u64>,
        on_stack: &mut HashSet<u64>,
        path: &mut Vec<u64>,
        cycles: &mut Vec<Vec<u64>>,
    ) {
        visited.insert(node);
        on_stack.insert(node);
        path.push(node);

        if let Some(neighbors) = adj.get(&node) {
            for &next in neighbors {
                if !visited.contains(&next) {
                    self.dfs_cycles(next, adj, visited, on_stack, path, cycles);
                } else if on_stack.contains(&next) {
                    // Found a cycle — extract it
                    if let Some(pos) = path.iter().position(|&n| n == next) {
                        cycles.push(path[pos..].to_vec());
                    }
                }
            }
        }

        path.pop();
        on_stack.remove(&node);
    }

    /// Breaks detected cycles by converting strong refs to weak refs.
    pub fn break_cycles(&self) -> Vec<RefEdge> {
        let cycles = self.detect_cycles();
        let mut to_weaken = Vec::new();
        for cycle in &cycles {
            if cycle.len() >= 2 {
                // Break the last edge in the cycle
                to_weaken.push(RefEdge {
                    from: cycle[cycle.len() - 1],
                    to: cycle[0],
                });
            }
        }
        to_weaken
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.6: Rc in Type System
// ═══════════════════════════════════════════════════════════════════════

/// Rc type representation in the type system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcTypeInfo {
    /// Inner type.
    pub inner: String,
    /// Whether auto-deref is applied for method calls.
    pub auto_deref: bool,
}

impl fmt::Display for RcTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rc<{}>", self.inner)
    }
}

/// Resolves auto-deref for Rc<T> method calls.
pub fn resolve_rc_deref(rc_type: &RcTypeInfo, method: &str) -> DerefResolution {
    // Rc own methods
    let rc_methods = ["strong_count", "weak_count", "clone", "downgrade"];
    if rc_methods.contains(&method) {
        DerefResolution::RcMethod(method.into())
    } else {
        DerefResolution::InnerMethod {
            inner_type: rc_type.inner.clone(),
            method: method.into(),
        }
    }
}

/// How a method call on Rc<T> is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerefResolution {
    /// Method on Rc itself.
    RcMethod(String),
    /// Auto-deref to inner T's method.
    InnerMethod { inner_type: String, method: String },
}

// ═══════════════════════════════════════════════════════════════════════
// S21.7: Interior Mutability
// ═══════════════════════════════════════════════════════════════════════

/// RefCell-like borrow state for interior mutability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowState {
    /// Not borrowed.
    Unborrowed,
    /// Immutably borrowed N times.
    SharedBorrow(u32),
    /// Mutably borrowed.
    MutableBorrow,
}

/// A RefCell type for interior mutability under GC mode.
#[derive(Debug, Clone)]
pub struct RefCellType {
    /// Inner type name.
    pub inner_type: String,
    /// Current borrow state.
    pub state: BorrowState,
}

impl RefCellType {
    /// Creates a new RefCell.
    pub fn new(inner_type: &str) -> Self {
        Self {
            inner_type: inner_type.into(),
            state: BorrowState::Unborrowed,
        }
    }

    /// Attempts to borrow immutably.
    pub fn try_borrow(&mut self) -> Result<(), String> {
        match self.state {
            BorrowState::Unborrowed => {
                self.state = BorrowState::SharedBorrow(1);
                Ok(())
            }
            BorrowState::SharedBorrow(n) => {
                self.state = BorrowState::SharedBorrow(n + 1);
                Ok(())
            }
            BorrowState::MutableBorrow => Err("already mutably borrowed".into()),
        }
    }

    /// Attempts to borrow mutably.
    pub fn try_borrow_mut(&mut self) -> Result<(), String> {
        match self.state {
            BorrowState::Unborrowed => {
                self.state = BorrowState::MutableBorrow;
                Ok(())
            }
            _ => Err("already borrowed".into()),
        }
    }

    /// Releases a borrow.
    pub fn release(&mut self) {
        match self.state {
            BorrowState::SharedBorrow(n) if n > 1 => {
                self.state = BorrowState::SharedBorrow(n - 1);
            }
            _ => {
                self.state = BorrowState::Unborrowed;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.8: Rc Thread Safety
// ═══════════════════════════════════════════════════════════════════════

/// Thread safety classification for reference-counted types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSafety {
    /// Single-threaded only (Rc).
    SingleThread,
    /// Thread-safe with atomic refcount (Arc).
    Atomic,
}

/// Checks whether an Rc type can be sent across threads.
pub fn check_send(safety: ThreadSafety) -> Result<(), String> {
    match safety {
        ThreadSafety::SingleThread => {
            Err("Rc<T> is !Send — use Arc<T> for multi-threaded access".into())
        }
        ThreadSafety::Atomic => Ok(()),
    }
}

/// Arc type info for thread-safe reference counting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArcTypeInfo {
    /// Inner type.
    pub inner: String,
}

impl fmt::Display for ArcTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Arc<{}>", self.inner)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S21.9: GC Statistics
// ═══════════════════════════════════════════════════════════════════════

/// GC statistics tracker.
#[derive(Debug, Default)]
pub struct GcStats {
    /// Total Rc allocations ever made.
    pub total_allocations: AtomicU64,
    /// Current live Rc count.
    pub live_count: AtomicU64,
    /// Total cycle collections performed.
    pub cycle_collections: AtomicU64,
    /// Total cycles broken.
    pub cycles_broken: AtomicU64,
}

impl GcStats {
    /// Creates a new stats tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an allocation.
    pub fn record_alloc(&self) {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.live_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a deallocation.
    pub fn record_dealloc(&self) {
        self.live_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Records a cycle collection.
    pub fn record_cycle_collection(&self, cycles_found: u64) {
        self.cycle_collections.fetch_add(1, Ordering::Relaxed);
        self.cycles_broken
            .fetch_add(cycles_found, Ordering::Relaxed);
    }

    /// Returns a snapshot of the stats.
    pub fn snapshot(&self) -> GcStatsSnapshot {
        GcStatsSnapshot {
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            live_count: self.live_count.load(Ordering::Relaxed),
            cycle_collections: self.cycle_collections.load(Ordering::Relaxed),
            cycles_broken: self.cycles_broken.load(Ordering::Relaxed),
        }
    }
}

/// A snapshot of GC statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcStatsSnapshot {
    /// Total Rc allocations.
    pub total_allocations: u64,
    /// Current live count.
    pub live_count: u64,
    /// Cycle collections performed.
    pub cycle_collections: u64,
    /// Cycles broken.
    pub cycles_broken: u64,
}

impl fmt::Display for GcStatsSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GC Stats: {} allocated, {} live, {} collections, {} cycles broken",
            self.total_allocations, self.live_count, self.cycle_collections, self.cycles_broken
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S21.1 — Rc<T> Type
    #[test]
    fn s21_1_rc_creation() {
        let rc = RcType::new("i32", 1);
        assert_eq!(rc.strong_count(), 1);
        assert_eq!(rc.weak_count(), 0);
        assert_eq!(rc.inner_type, "i32");
    }

    #[test]
    fn s21_1_rc_display() {
        let rc = RcType::new("String", 42);
        let display = rc.to_string();
        assert!(display.contains("Rc<String>"));
        assert!(display.contains("strong=1"));
    }

    // S21.2 — Rc Clone Semantics
    #[test]
    fn s21_2_clone_increments_count() {
        let mut rc = RcType::new("i32", 1);
        let cloned = rc.clone_rc();
        assert_eq!(rc.strong_count(), 2);
        assert_eq!(cloned.strong_count(), 2);
        assert_eq!(cloned.alloc_id, 1);
    }

    // S21.3 — Rc Drop
    #[test]
    fn s21_3_drop_decrements() {
        let mut rc = RcType::new("i32", 1);
        let _cloned = rc.clone_rc();
        let result = drop_rc(&mut rc);
        assert_eq!(result, DropResult::Decremented { remaining: 1 });
    }

    #[test]
    fn s21_3_drop_deallocates() {
        let mut rc = RcType::new("i32", 1);
        let result = drop_rc(&mut rc);
        assert_eq!(result, DropResult::Deallocated);
    }

    #[test]
    fn s21_3_drop_weak_only() {
        let mut rc = RcType::new("i32", 1);
        let _weak = WeakRef::downgrade(&mut rc);
        let result = drop_rc(&mut rc);
        assert_eq!(result, DropResult::WeakOnly { weak_remaining: 1 });
    }

    // S21.4 — Weak<T> Type
    #[test]
    fn s21_4_weak_downgrade() {
        let mut rc = RcType::new("i32", 1);
        let weak = WeakRef::downgrade(&mut rc);
        assert_eq!(rc.weak_count(), 1);
        assert!(weak.alive);
    }

    #[test]
    fn s21_4_weak_upgrade_alive() {
        let mut rc = RcType::new("i32", 1);
        let weak = WeakRef::downgrade(&mut rc);
        assert_eq!(weak.upgrade(), UpgradeResult::Some(1));
    }

    #[test]
    fn s21_4_weak_upgrade_expired() {
        let weak = WeakRef {
            alloc_id: 1,
            inner_type: "i32".into(),
            alive: false,
        };
        assert_eq!(weak.upgrade(), UpgradeResult::None);
    }

    // S21.5 — Cycle Detection
    #[test]
    fn s21_5_no_cycles() {
        let mut collector = CycleCollector::new();
        collector.add_edge(1, 2);
        collector.add_edge(2, 3);
        let cycles = collector.detect_cycles();
        assert!(cycles.is_empty());
    }

    #[test]
    fn s21_5_simple_cycle() {
        let mut collector = CycleCollector::new();
        collector.add_edge(1, 2);
        collector.add_edge(2, 1);
        let cycles = collector.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn s21_5_break_cycle() {
        let mut collector = CycleCollector::new();
        collector.add_edge(1, 2);
        collector.add_edge(2, 1);
        let to_weaken = collector.break_cycles();
        assert!(!to_weaken.is_empty());
    }

    // S21.6 — Rc in Type System
    #[test]
    fn s21_6_rc_type_display() {
        let info = RcTypeInfo {
            inner: "Vec<i32>".into(),
            auto_deref: true,
        };
        assert_eq!(info.to_string(), "Rc<Vec<i32>>");
    }

    #[test]
    fn s21_6_auto_deref_inner() {
        let info = RcTypeInfo {
            inner: "String".into(),
            auto_deref: true,
        };
        let resolution = resolve_rc_deref(&info, "len");
        assert_eq!(
            resolution,
            DerefResolution::InnerMethod {
                inner_type: "String".into(),
                method: "len".into()
            }
        );
    }

    #[test]
    fn s21_6_rc_own_method() {
        let info = RcTypeInfo {
            inner: "String".into(),
            auto_deref: true,
        };
        let resolution = resolve_rc_deref(&info, "strong_count");
        assert_eq!(resolution, DerefResolution::RcMethod("strong_count".into()));
    }

    // S21.7 — Interior Mutability
    #[test]
    fn s21_7_refcell_borrow() {
        let mut cell = RefCellType::new("i32");
        assert!(cell.try_borrow().is_ok());
        assert!(cell.try_borrow().is_ok());
        assert_eq!(cell.state, BorrowState::SharedBorrow(2));
    }

    #[test]
    fn s21_7_refcell_borrow_mut_conflict() {
        let mut cell = RefCellType::new("i32");
        assert!(cell.try_borrow().is_ok());
        assert!(cell.try_borrow_mut().is_err());
    }

    #[test]
    fn s21_7_refcell_release() {
        let mut cell = RefCellType::new("i32");
        cell.try_borrow().unwrap();
        cell.try_borrow().unwrap();
        cell.release();
        assert_eq!(cell.state, BorrowState::SharedBorrow(1));
        cell.release();
        assert_eq!(cell.state, BorrowState::Unborrowed);
    }

    // S21.8 — Rc Thread Safety
    #[test]
    fn s21_8_rc_not_send() {
        assert!(check_send(ThreadSafety::SingleThread).is_err());
    }

    #[test]
    fn s21_8_arc_is_send() {
        assert!(check_send(ThreadSafety::Atomic).is_ok());
    }

    #[test]
    fn s21_8_arc_display() {
        let arc = ArcTypeInfo {
            inner: "Mutex<i32>".into(),
        };
        assert_eq!(arc.to_string(), "Arc<Mutex<i32>>");
    }

    // S21.9 — GC Statistics
    #[test]
    fn s21_9_stats_tracking() {
        let stats = GcStats::new();
        stats.record_alloc();
        stats.record_alloc();
        stats.record_dealloc();
        let snap = stats.snapshot();
        assert_eq!(snap.total_allocations, 2);
        assert_eq!(snap.live_count, 1);
    }

    #[test]
    fn s21_9_stats_cycle_collection() {
        let stats = GcStats::new();
        stats.record_cycle_collection(3);
        let snap = stats.snapshot();
        assert_eq!(snap.cycle_collections, 1);
        assert_eq!(snap.cycles_broken, 3);
    }

    #[test]
    fn s21_9_stats_display() {
        let snap = GcStatsSnapshot {
            total_allocations: 100,
            live_count: 42,
            cycle_collections: 5,
            cycles_broken: 2,
        };
        let display = snap.to_string();
        assert!(display.contains("100 allocated"));
        assert!(display.contains("42 live"));
    }

    // S21.10 — Additional
    #[test]
    fn s21_10_multiple_clones() {
        let mut rc = RcType::new("Point", 1);
        let _c1 = rc.clone_rc();
        let _c2 = rc.clone_rc();
        assert_eq!(rc.strong_count(), 3);
    }

    #[test]
    fn s21_10_refcell_mut_borrow() {
        let mut cell = RefCellType::new("i32");
        assert!(cell.try_borrow_mut().is_ok());
        assert_eq!(cell.state, BorrowState::MutableBorrow);
        assert!(cell.try_borrow().is_err());
    }
}
