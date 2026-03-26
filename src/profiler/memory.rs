//! Memory Profiler — allocation tracking, leak detection, fragmentation.
//!
//! D1.3: 10 tasks covering allocation timeline, leak detection, peak analysis,
//! tensor memory tracking, fragmentation, and Valgrind-style reports.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// D1.3.1-D1.3.3: Allocation Tracking
// ═══════════════════════════════════════════════════════════════════════

/// A memory allocation record.
#[derive(Debug, Clone)]
pub struct AllocRecord {
    /// Unique allocation ID.
    pub id: u64,
    /// Pointer address.
    pub addr: u64,
    /// Size in bytes.
    pub size: usize,
    /// Allocation timestamp (ns).
    pub alloc_ns: u64,
    /// Free timestamp (0 = still alive).
    pub free_ns: u64,
    /// Allocation site (file:line).
    pub site: String,
    /// Allocation type.
    pub kind: AllocKind,
}

/// Allocation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocKind {
    Heap,
    Tensor,
    String,
    Array,
    Struct,
    Closure,
}

impl AllocRecord {
    /// Returns true if this allocation is still alive (not freed).
    pub fn is_alive(&self) -> bool { self.free_ns == 0 }

    /// Returns lifetime in nanoseconds (0 if still alive).
    pub fn lifetime_ns(&self) -> u64 {
        if self.free_ns == 0 { return 0; }
        self.free_ns.saturating_sub(self.alloc_ns)
    }
}

/// Memory profiler state.
#[derive(Debug, Clone, Default)]
pub struct MemoryProfile {
    /// All allocation records.
    pub records: Vec<AllocRecord>,
    /// Current heap size.
    pub current_bytes: usize,
    /// Peak heap size.
    pub peak_bytes: usize,
    /// Total allocated.
    pub total_allocated: u64,
    /// Total freed.
    pub total_freed: u64,
    /// Allocation count.
    pub alloc_count: u64,
    /// Free count.
    pub free_count: u64,
}

impl MemoryProfile {
    /// Records an allocation.
    pub fn record_alloc(&mut self, addr: u64, size: usize, site: &str, kind: AllocKind, now_ns: u64) {
        self.records.push(AllocRecord {
            id: self.alloc_count, addr, size, alloc_ns: now_ns, free_ns: 0,
            site: site.to_string(), kind,
        });
        self.current_bytes += size;
        self.total_allocated += size as u64;
        self.alloc_count += 1;
        if self.current_bytes > self.peak_bytes { self.peak_bytes = self.current_bytes; }
    }

    /// Records a free.
    pub fn record_free(&mut self, addr: u64, now_ns: u64) {
        if let Some(rec) = self.records.iter_mut().rev().find(|r| r.addr == addr && r.free_ns == 0) {
            rec.free_ns = now_ns;
            self.current_bytes = self.current_bytes.saturating_sub(rec.size);
            self.total_freed += rec.size as u64;
            self.free_count += 1;
        }
    }

    /// Returns all leaked allocations (allocated but never freed).
    pub fn leaks(&self) -> Vec<&AllocRecord> {
        self.records.iter().filter(|r| r.is_alive()).collect()
    }

    /// Returns leak summary by allocation site.
    pub fn leak_summary(&self) -> Vec<LeakSummary> {
        let mut by_site: HashMap<&str, (u64, usize)> = HashMap::new();
        for rec in self.leaks() {
            let entry = by_site.entry(&rec.site).or_default();
            entry.0 += 1;
            entry.1 += rec.size;
        }
        let mut result: Vec<_> = by_site.into_iter().map(|(site, (count, bytes))| {
            LeakSummary { site: site.to_string(), count, total_bytes: bytes }
        }).collect();
        result.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
        result
    }

    /// Returns peak memory contributing allocations.
    pub fn peak_contributors(&self, top_n: usize) -> Vec<&AllocRecord> {
        let mut alive_at_peak: Vec<&AllocRecord> = self.records.iter()
            .filter(|r| r.alloc_ns <= self.peak_timestamp_ns() && (r.free_ns == 0 || r.free_ns >= self.peak_timestamp_ns()))
            .collect();
        alive_at_peak.sort_by(|a, b| b.size.cmp(&a.size));
        alive_at_peak.truncate(top_n);
        alive_at_peak
    }

    fn peak_timestamp_ns(&self) -> u64 {
        // Find the timestamp when heap was at peak
        let mut current: usize = 0;
        let mut peak_ts: u64 = 0;
        let mut peak: usize = 0;
        // Simplified: use alloc events sorted by time
        for rec in &self.records {
            current += rec.size;
            if current > peak { peak = current; peak_ts = rec.alloc_ns; }
        }
        peak_ts
    }

    /// Tensor-specific memory stats.
    pub fn tensor_stats(&self) -> TensorMemStats {
        let tensor_allocs: Vec<_> = self.records.iter().filter(|r| r.kind == AllocKind::Tensor).collect();
        let alive: Vec<_> = tensor_allocs.iter().filter(|r| r.is_alive()).collect();
        TensorMemStats {
            total_tensors: tensor_allocs.len() as u64,
            live_tensors: alive.len() as u64,
            total_bytes: tensor_allocs.iter().map(|r| r.size as u64).sum(),
            live_bytes: alive.iter().map(|r| r.size as u64).sum(),
            peak_bytes: self.peak_bytes as u64, // approximate
        }
    }
}

/// Leak summary for a single allocation site.
#[derive(Debug, Clone)]
pub struct LeakSummary {
    /// Allocation site (file:line).
    pub site: String,
    /// Number of leaked allocations.
    pub count: u64,
    /// Total leaked bytes.
    pub total_bytes: usize,
}

/// Tensor memory statistics.
#[derive(Debug, Clone)]
pub struct TensorMemStats {
    pub total_tensors: u64,
    pub live_tensors: u64,
    pub total_bytes: u64,
    pub live_bytes: u64,
    pub peak_bytes: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// D1.3.6: Fragmentation Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Fragmentation metrics.
#[derive(Debug, Clone)]
pub struct FragmentationStats {
    /// Number of free blocks.
    pub free_blocks: u64,
    /// Total free bytes.
    pub free_bytes: u64,
    /// Largest free block.
    pub largest_free: u64,
    /// Fragmentation ratio (1.0 - largest_free / free_bytes).
    pub fragmentation: f64,
}

impl FragmentationStats {
    /// Computes fragmentation from a list of free block sizes.
    pub fn from_free_blocks(blocks: &[u64]) -> Self {
        let free_blocks = blocks.len() as u64;
        let free_bytes: u64 = blocks.iter().sum();
        let largest_free = blocks.iter().copied().max().unwrap_or(0);
        let fragmentation = if free_bytes > 0 {
            1.0 - (largest_free as f64 / free_bytes as f64)
        } else {
            0.0
        };
        Self { free_blocks, free_bytes, largest_free, fragmentation }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// D1.3.10: Valgrind-Style Report
// ═══════════════════════════════════════════════════════════════════════

/// Leak classification (Valgrind-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeakKind {
    /// Pointer exists and is reachable.
    StillReachable,
    /// No pointer found — definitely lost.
    DefinitelyLost,
    /// Pointer to interior — possibly lost.
    PossiblyLost,
    /// Reachable via other leaked block.
    IndirectlyLost,
}

impl fmt::Display for LeakKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StillReachable => write!(f, "still reachable"),
            Self::DefinitelyLost => write!(f, "definitely lost"),
            Self::PossiblyLost => write!(f, "possibly lost"),
            Self::IndirectlyLost => write!(f, "indirectly lost"),
        }
    }
}

/// Generates a Valgrind-style memory report.
pub fn valgrind_report(profile: &MemoryProfile) -> String {
    let leaks = profile.leaks();
    let total_leaked: usize = leaks.iter().map(|r| r.size).sum();
    let summary = profile.leak_summary();

    let mut report = String::new();
    report.push_str("==== HEAP SUMMARY ====\n");
    report.push_str(&format!("    in use at exit: {} bytes in {} blocks\n", profile.current_bytes, leaks.len()));
    report.push_str(&format!("  total heap usage: {} allocs, {} frees, {} bytes allocated\n",
        profile.alloc_count, profile.free_count, profile.total_allocated));
    report.push_str(&format!("         peak heap: {} bytes\n\n", profile.peak_bytes));

    if leaks.is_empty() {
        report.push_str("All heap blocks were freed — no leaks are possible\n");
    } else {
        report.push_str("==== LEAK SUMMARY ====\n");
        report.push_str(&format!("   definitely lost: {} bytes in {} blocks\n", total_leaked, leaks.len()));
        report.push_str("\n==== LEAK DETAILS ====\n");
        for s in &summary {
            report.push_str(&format!("  {} bytes in {} blocks at {}\n", s.total_bytes, s.count, s.site));
        }
    }
    report
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_3_alloc_tracking() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 256, "main.fj:5", AllocKind::Heap, 100);
        prof.record_alloc(0x2000, 1024, "main.fj:10", AllocKind::Tensor, 200);
        assert_eq!(prof.current_bytes, 1280);
        assert_eq!(prof.alloc_count, 2);

        prof.record_free(0x1000, 300);
        assert_eq!(prof.current_bytes, 1024);
        assert_eq!(prof.free_count, 1);
    }

    #[test]
    fn d1_3_peak_memory() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 1000, "a.fj:1", AllocKind::Heap, 100);
        prof.record_alloc(0x2000, 2000, "a.fj:2", AllocKind::Heap, 200);
        assert_eq!(prof.peak_bytes, 3000);
        prof.record_free(0x1000, 300);
        assert_eq!(prof.peak_bytes, 3000); // peak unchanged
        assert_eq!(prof.current_bytes, 2000);
    }

    #[test]
    fn d1_3_leak_detection() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 100, "leak.fj:1", AllocKind::Heap, 100);
        prof.record_alloc(0x2000, 200, "leak.fj:5", AllocKind::Heap, 200);
        prof.record_free(0x1000, 300);
        // 0x2000 is leaked
        let leaks = prof.leaks();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].addr, 0x2000);
    }

    #[test]
    fn d1_3_leak_summary() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 100, "a.fj:1", AllocKind::Heap, 100);
        prof.record_alloc(0x2000, 200, "a.fj:1", AllocKind::Heap, 200);
        prof.record_alloc(0x3000, 50, "b.fj:5", AllocKind::Heap, 300);
        let summary = prof.leak_summary();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].site, "a.fj:1"); // largest first
        assert_eq!(summary[0].total_bytes, 300);
    }

    #[test]
    fn d1_3_tensor_stats() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 4096, "nn.fj:1", AllocKind::Tensor, 100);
        prof.record_alloc(0x2000, 8192, "nn.fj:2", AllocKind::Tensor, 200);
        prof.record_alloc(0x3000, 256, "main.fj:1", AllocKind::Heap, 300);
        prof.record_free(0x1000, 400);
        let stats = prof.tensor_stats();
        assert_eq!(stats.total_tensors, 2);
        assert_eq!(stats.live_tensors, 1);
        assert_eq!(stats.live_bytes, 8192);
    }

    #[test]
    fn d1_3_fragmentation() {
        let blocks = vec![100, 50, 200, 30, 80];
        let stats = FragmentationStats::from_free_blocks(&blocks);
        assert_eq!(stats.free_blocks, 5);
        assert_eq!(stats.free_bytes, 460);
        assert_eq!(stats.largest_free, 200);
        assert!((stats.fragmentation - (1.0 - 200.0 / 460.0)).abs() < 0.001);
    }

    #[test]
    fn d1_3_valgrind_report_clean() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 100, "a.fj:1", AllocKind::Heap, 100);
        prof.record_free(0x1000, 200);
        let report = valgrind_report(&prof);
        assert!(report.contains("no leaks"));
    }

    #[test]
    fn d1_3_valgrind_report_leak() {
        let mut prof = MemoryProfile::default();
        prof.record_alloc(0x1000, 100, "leak.fj:1", AllocKind::Heap, 100);
        let report = valgrind_report(&prof);
        assert!(report.contains("definitely lost"));
        assert!(report.contains("100 bytes"));
    }

    #[test]
    fn d1_3_leak_kind_display() {
        assert_eq!(format!("{}", LeakKind::DefinitelyLost), "definitely lost");
        assert_eq!(format!("{}", LeakKind::StillReachable), "still reachable");
    }
}
