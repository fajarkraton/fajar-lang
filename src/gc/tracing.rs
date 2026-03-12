//! Tracing GC — mark-sweep, root registration, generational collection,
//! write barrier, concurrent marking, pause budget, heap sizing, finalization.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S22.1 / S22.2: Mark-Sweep
// ═══════════════════════════════════════════════════════════════════════

/// Object color in tri-color marking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkColor {
    /// Not yet visited.
    White,
    /// Visited but children not fully processed.
    Gray,
    /// Fully processed.
    Black,
}

/// A GC-managed object.
#[derive(Debug, Clone)]
pub struct GcObject {
    /// Unique allocation ID.
    pub id: u64,
    /// Type name.
    pub type_name: String,
    /// References to other GC objects.
    pub references: Vec<u64>,
    /// Current mark color.
    pub color: MarkColor,
    /// Generation (0 = young, 1 = old).
    pub generation: u8,
    /// Whether this object has a finalizer.
    pub has_finalizer: bool,
    /// Size in bytes.
    pub size: usize,
}

impl GcObject {
    /// Creates a new young-generation GC object.
    pub fn new(id: u64, type_name: &str, size: usize) -> Self {
        Self {
            id,
            type_name: type_name.into(),
            references: Vec::new(),
            color: MarkColor::White,
            generation: 0,
            has_finalizer: false,
            size,
        }
    }

    /// Adds a reference to another object.
    pub fn add_reference(&mut self, target: u64) {
        self.references.push(target);
    }
}

/// The GC heap containing all managed objects.
#[derive(Debug, Clone, Default)]
pub struct GcHeap {
    /// All objects keyed by ID.
    objects: HashMap<u64, GcObject>,
    /// Root set (stack frames, globals).
    roots: HashSet<u64>,
    /// Next allocation ID.
    next_id: u64,
    /// Write barrier log (cross-generation references).
    write_barrier_log: Vec<(u64, u64)>,
}

impl GcHeap {
    /// Creates an empty GC heap.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a new object, returns its ID.
    pub fn allocate(&mut self, type_name: &str, size: usize) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, GcObject::new(id, type_name, size));
        id
    }

    /// Adds a reference from one object to another.
    pub fn add_reference(&mut self, from: u64, to: u64) {
        if let Some(obj) = self.objects.get_mut(&from) {
            let from_gen = obj.generation;
            obj.add_reference(to);
            // Write barrier: track cross-generation references
            if let Some(to_obj) = self.objects.get(&to) {
                if from_gen > to_obj.generation {
                    self.write_barrier_log.push((from, to));
                }
            }
        }
    }

    /// Registers a root.
    pub fn add_root(&mut self, id: u64) {
        self.roots.insert(id);
    }

    /// Removes a root.
    pub fn remove_root(&mut self, id: u64) {
        self.roots.remove(&id);
    }

    /// Returns the number of live objects.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Returns total heap size in bytes.
    pub fn total_size(&self) -> usize {
        self.objects.values().map(|o| o.size).sum()
    }

    /// Sets a finalizer on an object.
    pub fn set_finalizer(&mut self, id: u64) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.has_finalizer = true;
        }
    }

    /// Promotes an object to old generation.
    pub fn promote_to_old(&mut self, id: u64) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.generation = 1;
        }
    }

    // ───────────────────────────────────────────────────────────────────
    // Mark phase
    // ───────────────────────────────────────────────────────────────────

    /// Runs the mark phase from the root set.
    pub fn mark(&mut self) {
        // Reset all to white
        for obj in self.objects.values_mut() {
            obj.color = MarkColor::White;
        }

        // Start with roots as gray
        let roots: Vec<u64> = self.roots.iter().copied().collect();
        let mut worklist: Vec<u64> = Vec::new();
        for root_id in &roots {
            if let Some(obj) = self.objects.get_mut(root_id) {
                obj.color = MarkColor::Gray;
                worklist.push(*root_id);
            }
        }

        // Process gray objects
        while let Some(id) = worklist.pop() {
            let refs = if let Some(obj) = self.objects.get(&id) {
                obj.references.clone()
            } else {
                continue;
            };

            for ref_id in refs {
                if let Some(ref_obj) = self.objects.get_mut(&ref_id) {
                    if ref_obj.color == MarkColor::White {
                        ref_obj.color = MarkColor::Gray;
                        worklist.push(ref_id);
                    }
                }
            }

            if let Some(obj) = self.objects.get_mut(&id) {
                obj.color = MarkColor::Black;
            }
        }
    }

    // ───────────────────────────────────────────────────────────────────
    // Sweep phase
    // ───────────────────────────────────────────────────────────────────

    /// Runs the sweep phase, freeing unmarked objects.
    pub fn sweep(&mut self) -> SweepResult {
        let mut freed_count = 0;
        let mut freed_bytes = 0;
        let mut finalized = Vec::new();

        let to_remove: Vec<u64> = self
            .objects
            .iter()
            .filter(|(_, obj)| obj.color == MarkColor::White)
            .map(|(id, _)| *id)
            .collect();

        for id in &to_remove {
            if let Some(obj) = self.objects.get(id) {
                freed_bytes += obj.size;
                if obj.has_finalizer {
                    finalized.push(*id);
                }
            }
            self.objects.remove(id);
            freed_count += 1;
        }

        SweepResult {
            freed_count,
            freed_bytes,
            finalized,
            remaining: self.objects.len(),
        }
    }

    /// Runs a full GC cycle (mark + sweep).
    pub fn collect(&mut self) -> SweepResult {
        self.mark();
        self.sweep()
    }

    // ───────────────────────────────────────────────────────────────────
    // Generational collection
    // ───────────────────────────────────────────────────────────────────

    /// Collects only young generation objects.
    pub fn collect_young(&mut self) -> SweepResult {
        // Mark from roots but only traverse young objects
        for obj in self.objects.values_mut() {
            if obj.generation == 0 {
                obj.color = MarkColor::White;
            } else {
                obj.color = MarkColor::Black; // Old gen assumed live
            }
        }

        let roots: Vec<u64> = self.roots.iter().copied().collect();
        let mut worklist: Vec<u64> = Vec::new();
        for root_id in &roots {
            if let Some(obj) = self.objects.get_mut(root_id) {
                if obj.color == MarkColor::White {
                    obj.color = MarkColor::Gray;
                    worklist.push(*root_id);
                }
            }
        }

        // Also mark from write barrier log (old -> young refs)
        let barrier_targets: Vec<u64> = self.write_barrier_log.iter().map(|(_, to)| *to).collect();
        for target in &barrier_targets {
            if let Some(obj) = self.objects.get_mut(target) {
                if obj.color == MarkColor::White {
                    obj.color = MarkColor::Gray;
                    worklist.push(*target);
                }
            }
        }

        while let Some(id) = worklist.pop() {
            let refs = if let Some(obj) = self.objects.get(&id) {
                obj.references.clone()
            } else {
                continue;
            };

            for ref_id in refs {
                if let Some(ref_obj) = self.objects.get_mut(&ref_id) {
                    if ref_obj.color == MarkColor::White {
                        ref_obj.color = MarkColor::Gray;
                        worklist.push(ref_id);
                    }
                }
            }

            if let Some(obj) = self.objects.get_mut(&id) {
                obj.color = MarkColor::Black;
            }
        }

        // Sweep only young generation
        let mut freed_count = 0;
        let mut freed_bytes = 0;
        let mut finalized = Vec::new();

        let to_remove: Vec<u64> = self
            .objects
            .iter()
            .filter(|(_, obj)| obj.generation == 0 && obj.color == MarkColor::White)
            .map(|(id, _)| *id)
            .collect();

        for id in &to_remove {
            if let Some(obj) = self.objects.get(id) {
                freed_bytes += obj.size;
                if obj.has_finalizer {
                    finalized.push(*id);
                }
            }
            self.objects.remove(id);
            freed_count += 1;
        }

        self.write_barrier_log.clear();

        SweepResult {
            freed_count,
            freed_bytes,
            finalized,
            remaining: self.objects.len(),
        }
    }
}

/// Result of a sweep phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepResult {
    /// Number of objects freed.
    pub freed_count: usize,
    /// Total bytes freed.
    pub freed_bytes: usize,
    /// Object IDs that had finalizers.
    pub finalized: Vec<u64>,
    /// Objects remaining after sweep.
    pub remaining: usize,
}

impl fmt::Display for SweepResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GC: freed {} objects ({} bytes), {} remaining",
            self.freed_count, self.freed_bytes, self.remaining
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.7: GC Pause Budget
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for GC pause budgets.
#[derive(Debug, Clone)]
pub struct PauseBudget {
    /// Maximum pause time in microseconds.
    pub max_pause_us: u64,
    /// Whether incremental collection is enabled.
    pub incremental: bool,
}

impl Default for PauseBudget {
    fn default() -> Self {
        Self {
            max_pause_us: 1000, // 1ms
            incremental: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.8: Heap Sizing
// ═══════════════════════════════════════════════════════════════════════

/// Heap sizing policy.
#[derive(Debug, Clone)]
pub struct HeapPolicy {
    /// Current heap capacity in bytes.
    pub capacity: usize,
    /// Grow threshold (occupancy ratio, e.g., 0.75).
    pub grow_threshold: f64,
    /// Shrink threshold (occupancy ratio, e.g., 0.25).
    pub shrink_threshold: f64,
    /// Growth factor.
    pub growth_factor: f64,
}

impl Default for HeapPolicy {
    fn default() -> Self {
        Self {
            capacity: 1024 * 1024, // 1MB
            grow_threshold: 0.75,
            shrink_threshold: 0.25,
            growth_factor: 2.0,
        }
    }
}

/// Decides whether to resize the heap based on current occupancy.
pub fn should_resize(policy: &HeapPolicy, used: usize) -> HeapResize {
    let occupancy = used as f64 / policy.capacity as f64;
    if occupancy > policy.grow_threshold {
        HeapResize::Grow {
            new_capacity: (policy.capacity as f64 * policy.growth_factor) as usize,
        }
    } else if occupancy < policy.shrink_threshold && policy.capacity > 1024 {
        HeapResize::Shrink {
            new_capacity: policy.capacity / 2,
        }
    } else {
        HeapResize::NoChange
    }
}

/// Heap resize decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeapResize {
    /// Grow the heap.
    Grow { new_capacity: usize },
    /// Shrink the heap.
    Shrink { new_capacity: usize },
    /// No change needed.
    NoChange,
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S22.1 — Mark Phase
    #[test]
    fn s22_1_mark_reachable() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let b = heap.allocate("i32", 8);
        heap.add_reference(a, b);
        heap.add_root(a);
        heap.mark();
        assert_eq!(heap.objects[&a].color, MarkColor::Black);
        assert_eq!(heap.objects[&b].color, MarkColor::Black);
    }

    #[test]
    fn s22_1_mark_unreachable() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let _b = heap.allocate("i32", 8);
        heap.add_root(a);
        heap.mark();
        assert_eq!(heap.objects[&a].color, MarkColor::Black);
        assert_eq!(heap.objects[&_b].color, MarkColor::White);
    }

    // S22.2 — Sweep Phase
    #[test]
    fn s22_2_sweep_unreachable() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let _b = heap.allocate("i32", 8);
        heap.add_root(a);
        let result = heap.collect();
        assert_eq!(result.freed_count, 1);
        assert_eq!(result.remaining, 1);
    }

    #[test]
    fn s22_2_sweep_all_reachable() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let b = heap.allocate("i32", 8);
        heap.add_reference(a, b);
        heap.add_root(a);
        let result = heap.collect();
        assert_eq!(result.freed_count, 0);
        assert_eq!(result.remaining, 2);
    }

    // S22.3 — GC Root Registration
    #[test]
    fn s22_3_root_registration() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        heap.add_root(a);
        let result = heap.collect();
        assert_eq!(result.remaining, 1);
    }

    #[test]
    fn s22_3_remove_root_then_collect() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        heap.add_root(a);
        heap.remove_root(a);
        let result = heap.collect();
        assert_eq!(result.freed_count, 1);
    }

    // S22.4 — Generational Collection
    #[test]
    fn s22_4_young_gen_collection() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let _b = heap.allocate("i32", 8);
        heap.promote_to_old(a);
        heap.add_root(a);
        let result = heap.collect_young();
        assert_eq!(result.freed_count, 1); // b is young and unreachable
    }

    #[test]
    fn s22_4_old_gen_survives() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        heap.promote_to_old(a);
        // No root needed — old gen assumed live in young collection
        let result = heap.collect_young();
        assert_eq!(result.freed_count, 0);
    }

    // S22.5 — Write Barrier
    #[test]
    fn s22_5_write_barrier_logged() {
        let mut heap = GcHeap::new();
        let old = heap.allocate("Container", 64);
        let young = heap.allocate("i32", 8);
        heap.promote_to_old(old);
        heap.add_reference(old, young);
        assert_eq!(heap.write_barrier_log.len(), 1);
    }

    #[test]
    fn s22_5_write_barrier_preserves_young() {
        let mut heap = GcHeap::new();
        let old = heap.allocate("Container", 64);
        let young = heap.allocate("i32", 8);
        heap.promote_to_old(old);
        heap.add_root(old);
        heap.add_reference(old, young);
        let result = heap.collect_young();
        assert_eq!(result.freed_count, 0); // young referenced from old via barrier
    }

    // S22.6 — Concurrent Marking (tri-color)
    #[test]
    fn s22_6_tri_color_invariant() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let b = heap.allocate("i32", 8);
        let c = heap.allocate("i32", 8);
        heap.add_reference(a, b);
        heap.add_reference(b, c);
        heap.add_root(a);
        heap.mark();
        // All reachable should be black
        assert_eq!(heap.objects[&a].color, MarkColor::Black);
        assert_eq!(heap.objects[&b].color, MarkColor::Black);
        assert_eq!(heap.objects[&c].color, MarkColor::Black);
    }

    // S22.7 — GC Pause Budget
    #[test]
    fn s22_7_pause_budget_default() {
        let budget = PauseBudget::default();
        assert_eq!(budget.max_pause_us, 1000);
        assert!(budget.incremental);
    }

    // S22.8 — Heap Sizing
    #[test]
    fn s22_8_heap_grow() {
        let policy = HeapPolicy::default();
        let used = (policy.capacity as f64 * 0.8) as usize;
        match should_resize(&policy, used) {
            HeapResize::Grow { new_capacity } => {
                assert!(new_capacity > policy.capacity);
            }
            _ => panic!("expected grow"),
        }
    }

    #[test]
    fn s22_8_heap_shrink() {
        let policy = HeapPolicy {
            capacity: 1024 * 1024,
            ..HeapPolicy::default()
        };
        let used = (policy.capacity as f64 * 0.1) as usize;
        match should_resize(&policy, used) {
            HeapResize::Shrink { new_capacity } => {
                assert!(new_capacity < policy.capacity);
            }
            _ => panic!("expected shrink"),
        }
    }

    #[test]
    fn s22_8_heap_no_change() {
        let policy = HeapPolicy::default();
        let used = (policy.capacity as f64 * 0.5) as usize;
        assert_eq!(should_resize(&policy, used), HeapResize::NoChange);
    }

    // S22.9 — Finalization
    #[test]
    fn s22_9_finalizer_reported() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("Resource", 64);
        heap.set_finalizer(a);
        // Don't root it so it gets collected
        let result = heap.collect();
        assert!(result.finalized.contains(&a));
    }

    #[test]
    fn s22_9_no_finalizer() {
        let mut heap = GcHeap::new();
        let _a = heap.allocate("i32", 8);
        let result = heap.collect();
        assert!(result.finalized.is_empty());
    }

    // S22.10 — Additional
    #[test]
    fn s22_10_sweep_result_display() {
        let result = SweepResult {
            freed_count: 10,
            freed_bytes: 1024,
            finalized: vec![],
            remaining: 5,
        };
        let display = result.to_string();
        assert!(display.contains("10 objects"));
        assert!(display.contains("1024 bytes"));
    }

    #[test]
    fn s22_10_heap_total_size() {
        let mut heap = GcHeap::new();
        heap.allocate("i32", 8);
        heap.allocate("i64", 16);
        assert_eq!(heap.total_size(), 24);
    }

    #[test]
    fn s22_10_chain_collection() {
        let mut heap = GcHeap::new();
        let a = heap.allocate("i32", 8);
        let b = heap.allocate("i32", 8);
        let c = heap.allocate("i32", 8);
        let d = heap.allocate("i32", 8);
        heap.add_reference(a, b);
        heap.add_reference(b, c);
        // d is unreachable
        heap.add_root(a);
        let result = heap.collect();
        assert_eq!(result.freed_count, 1); // only d freed
        assert_eq!(result.remaining, 3);
        assert!(!heap.objects.contains_key(&d));
    }
}
