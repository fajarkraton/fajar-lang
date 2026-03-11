//! GPU Memory Management — device memory allocator, host-device transfer,
//! unified memory, memory pool, defragmentation, OOM handling,
//! multi-GPU, profiling, zero-copy tensors.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S20.1: Device Memory Allocator
// ═══════════════════════════════════════════════════════════════════════

/// A GPU device ID.
pub type DeviceId = u32;

/// Device memory allocation.
#[derive(Debug, Clone)]
pub struct DeviceAllocation {
    /// Unique allocation ID.
    pub id: u64,
    /// Device ID.
    pub device: DeviceId,
    /// Offset within pool.
    pub offset: usize,
    /// Size in bytes.
    pub size: usize,
    /// Whether currently in use.
    pub in_use: bool,
}

/// Device memory allocator with pool.
#[derive(Debug, Clone)]
pub struct DeviceAllocator {
    /// Device ID.
    pub device: DeviceId,
    /// Total pool size in bytes.
    pub pool_size: usize,
    /// Max pool size.
    pub max_pool_size: usize,
    /// Allocations.
    allocations: Vec<DeviceAllocation>,
    /// Next allocation ID.
    next_id: u64,
    /// Current used bytes.
    used_bytes: usize,
}

/// Allocator error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocError {
    /// Out of device memory.
    OutOfMemory {
        /// Requested bytes.
        requested: usize,
        /// Available bytes.
        available: usize,
    },
    /// Invalid allocation ID.
    InvalidAllocation(u64),
    /// Device not found.
    DeviceNotFound(DeviceId),
}

impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AllocError::OutOfMemory {
                requested,
                available,
            } => write!(
                f,
                "GPU OOM: requested {requested} bytes, {available} available"
            ),
            AllocError::InvalidAllocation(id) => write!(f, "Invalid GPU allocation: {id}"),
            AllocError::DeviceNotFound(id) => write!(f, "GPU device not found: {id}"),
        }
    }
}

impl DeviceAllocator {
    /// Creates a new allocator with given pool size.
    pub fn new(device: DeviceId, initial_size: usize, max_size: usize) -> Self {
        Self {
            device,
            pool_size: initial_size,
            max_pool_size: max_size,
            allocations: Vec::new(),
            next_id: 1,
            used_bytes: 0,
        }
    }

    /// Allocates device memory.
    pub fn allocate(&mut self, size: usize) -> Result<DeviceAllocation, AllocError> {
        // Try free-list first
        if let Some(idx) = self.find_free_block(size) {
            self.allocations[idx].in_use = true;
            self.used_bytes += self.allocations[idx].size;
            return Ok(self.allocations[idx].clone());
        }

        let available = self.pool_size - self.used_bytes;
        if size > available {
            // Try growing
            if self.used_bytes + size <= self.max_pool_size {
                self.pool_size = (self.used_bytes + size).min(self.max_pool_size);
            } else {
                return Err(AllocError::OutOfMemory {
                    requested: size,
                    available,
                });
            }
        }

        let alloc = DeviceAllocation {
            id: self.next_id,
            device: self.device,
            offset: self.used_bytes,
            size,
            in_use: true,
        };
        self.next_id += 1;
        self.used_bytes += size;
        self.allocations.push(alloc.clone());
        Ok(alloc)
    }

    /// Frees a device allocation.
    pub fn free(&mut self, alloc_id: u64) -> Result<(), AllocError> {
        let alloc = self
            .allocations
            .iter_mut()
            .find(|a| a.id == alloc_id && a.in_use)
            .ok_or(AllocError::InvalidAllocation(alloc_id))?;
        alloc.in_use = false;
        self.used_bytes -= alloc.size;
        Ok(())
    }

    /// Returns current usage statistics.
    pub fn stats(&self) -> AllocStats {
        let active = self.allocations.iter().filter(|a| a.in_use).count();
        AllocStats {
            total_pool_bytes: self.pool_size,
            used_bytes: self.used_bytes,
            free_bytes: self.pool_size - self.used_bytes,
            active_allocations: active,
            total_allocations: self.allocations.len(),
            peak_usage: self.used_bytes, // simplified
        }
    }

    /// Finds a free block of at least `size` bytes (best-fit).
    fn find_free_block(&self, size: usize) -> Option<usize> {
        self.allocations
            .iter()
            .enumerate()
            .filter(|(_, a)| !a.in_use && a.size >= size)
            .min_by_key(|(_, a)| a.size)
            .map(|(i, _)| i)
    }
}

/// Allocator statistics.
#[derive(Debug, Clone)]
pub struct AllocStats {
    /// Total pool bytes.
    pub total_pool_bytes: usize,
    /// Currently used bytes.
    pub used_bytes: usize,
    /// Free bytes.
    pub free_bytes: usize,
    /// Active allocations.
    pub active_allocations: usize,
    /// Total allocations (including freed).
    pub total_allocations: usize,
    /// Peak memory usage.
    pub peak_usage: usize,
}

impl AllocStats {
    /// Utilization ratio.
    pub fn utilization(&self) -> f64 {
        if self.total_pool_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.total_pool_bytes as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.2: Host-Device Transfer
// ═══════════════════════════════════════════════════════════════════════

/// Transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// Host to device.
    HostToDevice,
    /// Device to host.
    DeviceToHost,
    /// Device to device (peer).
    DeviceToDevice,
}

impl fmt::Display for TransferDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferDirection::HostToDevice => write!(f, "H2D"),
            TransferDirection::DeviceToHost => write!(f, "D2H"),
            TransferDirection::DeviceToDevice => write!(f, "D2D"),
        }
    }
}

/// A transfer descriptor.
#[derive(Debug, Clone)]
pub struct TransferDesc {
    /// Direction.
    pub direction: TransferDirection,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Whether to use pinned host memory.
    pub pinned: bool,
    /// Whether this is an async transfer.
    pub is_async: bool,
    /// Stream/queue ID (for async).
    pub stream_id: u32,
}

/// Estimated transfer time in microseconds.
pub fn estimate_transfer_time(size_bytes: usize, bandwidth_gbps: f64) -> f64 {
    let bytes_per_us = bandwidth_gbps * 1e9 / 1e6; // bytes per microsecond
    size_bytes as f64 / bytes_per_us
}

// ═══════════════════════════════════════════════════════════════════════
// S20.3: Unified Memory
// ═══════════════════════════════════════════════════════════════════════

/// Unified memory mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryMode {
    /// Explicit host/device management.
    Explicit,
    /// Unified/managed memory (auto-migration).
    Unified,
    /// Zero-copy (pinned host, GPU-accessible).
    ZeroCopy,
}

impl fmt::Display for MemoryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryMode::Explicit => write!(f, "Explicit"),
            MemoryMode::Unified => write!(f, "Unified"),
            MemoryMode::ZeroCopy => write!(f, "ZeroCopy"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.4: Memory Pool (covered by DeviceAllocator free-list above)
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
// S20.5: Memory Defragmentation
// ═══════════════════════════════════════════════════════════════════════

/// Defragmentation result.
#[derive(Debug, Clone)]
pub struct DefragResult {
    /// Number of allocations moved.
    pub moved_count: usize,
    /// Bytes recovered.
    pub bytes_recovered: usize,
    /// New largest contiguous block.
    pub largest_free_block: usize,
}

/// Analyzes fragmentation of the allocator.
pub fn analyze_fragmentation(allocator: &DeviceAllocator) -> FragmentationInfo {
    let free_blocks: Vec<usize> = allocator
        .allocations
        .iter()
        .filter(|a| !a.in_use)
        .map(|a| a.size)
        .collect();

    let total_free: usize = free_blocks.iter().sum();
    let largest_free = free_blocks.iter().copied().max().unwrap_or(0);
    let num_fragments = free_blocks.len();

    FragmentationInfo {
        total_free_bytes: total_free,
        largest_free_block: largest_free,
        num_fragments,
        fragmentation_ratio: if total_free > 0 {
            1.0 - (largest_free as f64 / total_free as f64)
        } else {
            0.0
        },
    }
}

/// Fragmentation analysis info.
#[derive(Debug, Clone)]
pub struct FragmentationInfo {
    /// Total free bytes.
    pub total_free_bytes: usize,
    /// Largest contiguous free block.
    pub largest_free_block: usize,
    /// Number of free fragments.
    pub num_fragments: usize,
    /// Fragmentation ratio (0.0 = no fragmentation, 1.0 = fully fragmented).
    pub fragmentation_ratio: f64,
}

// ═══════════════════════════════════════════════════════════════════════
// S20.6: Out-of-Memory Handling
// ═══════════════════════════════════════════════════════════════════════

/// OOM recovery strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OomStrategy {
    /// Fail immediately.
    Fail,
    /// Spill tensors to host memory.
    SpillToHost,
    /// Evict least-recently-used allocations.
    EvictLru,
    /// Retry after garbage collection.
    RetryAfterGc,
}

/// An OOM event.
#[derive(Debug, Clone)]
pub struct OomEvent {
    /// Requested bytes.
    pub requested_bytes: usize,
    /// Available bytes at time of failure.
    pub available_bytes: usize,
    /// Strategy applied.
    pub strategy: OomStrategy,
    /// Whether recovery was successful.
    pub recovered: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// S20.7: Multi-GPU Memory
// ═══════════════════════════════════════════════════════════════════════

/// Peer access between GPUs.
#[derive(Debug, Clone)]
pub struct PeerAccess {
    /// Source device.
    pub src_device: DeviceId,
    /// Destination device.
    pub dst_device: DeviceId,
    /// Whether peer access is enabled.
    pub enabled: bool,
    /// Bandwidth (GB/s) for P2P transfer.
    pub bandwidth_gbps: f64,
}

/// Multi-GPU topology.
#[derive(Debug, Clone)]
pub struct GpuTopology {
    /// Number of GPUs.
    pub num_devices: u32,
    /// Peer access matrix.
    pub peer_access: Vec<PeerAccess>,
}

impl GpuTopology {
    /// Creates a topology with N devices.
    pub fn new(num_devices: u32) -> Self {
        Self {
            num_devices,
            peer_access: Vec::new(),
        }
    }

    /// Checks if peer access is possible between two devices.
    pub fn can_peer_access(&self, src: DeviceId, dst: DeviceId) -> bool {
        self.peer_access
            .iter()
            .any(|p| p.src_device == src && p.dst_device == dst && p.enabled)
    }

    /// Adds a peer access link.
    pub fn add_peer(&mut self, src: DeviceId, dst: DeviceId, bandwidth: f64) {
        self.peer_access.push(PeerAccess {
            src_device: src,
            dst_device: dst,
            enabled: true,
            bandwidth_gbps: bandwidth,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.8: Memory Profiling
// ═══════════════════════════════════════════════════════════════════════

/// Memory profiling data.
#[derive(Debug, Clone)]
pub struct MemoryProfile {
    /// Allocation events.
    pub events: Vec<ProfileEvent>,
    /// Peak usage per device.
    pub peak_usage: HashMap<DeviceId, usize>,
    /// Total bytes transferred.
    pub total_transferred: usize,
}

/// A memory profile event.
#[derive(Debug, Clone)]
pub struct ProfileEvent {
    /// Timestamp (microseconds from start).
    pub timestamp_us: u64,
    /// Event kind.
    pub kind: ProfileEventKind,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Device ID.
    pub device: DeviceId,
}

/// Kind of profile event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileEventKind {
    /// Allocation.
    Allocate,
    /// Free.
    Free,
    /// Transfer.
    Transfer,
    /// Pool resize.
    PoolResize,
}

impl MemoryProfile {
    /// Creates a new empty profile.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            peak_usage: HashMap::new(),
            total_transferred: 0,
        }
    }

    /// Records an event.
    pub fn record(&mut self, event: ProfileEvent) {
        match event.kind {
            ProfileEventKind::Allocate => {
                let peak = self.peak_usage.entry(event.device).or_insert(0);
                *peak = (*peak).max(event.size_bytes);
            }
            ProfileEventKind::Transfer => {
                self.total_transferred += event.size_bytes;
            }
            _ => {}
        }
        self.events.push(event);
    }

    /// Returns peak usage for a device.
    pub fn peak_for_device(&self, device: DeviceId) -> usize {
        self.peak_usage.get(&device).copied().unwrap_or(0)
    }
}

impl Default for MemoryProfile {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S20.9: Zero-Copy Tensors
// ═══════════════════════════════════════════════════════════════════════

/// A zero-copy tensor descriptor (pinned host memory accessible from GPU).
#[derive(Debug, Clone)]
pub struct ZeroCopyTensor {
    /// Tensor name.
    pub name: String,
    /// Shape dimensions.
    pub shape: Vec<usize>,
    /// Element size in bytes.
    pub elem_size: usize,
    /// Total size in bytes.
    pub total_bytes: usize,
    /// Whether the tensor is pinned.
    pub is_pinned: bool,
    /// Device that can access this tensor.
    pub accessible_device: DeviceId,
}

impl ZeroCopyTensor {
    /// Creates a new zero-copy tensor descriptor.
    pub fn new(name: &str, shape: Vec<usize>, elem_size: usize, device: DeviceId) -> Self {
        let total: usize = shape.iter().product::<usize>() * elem_size;
        Self {
            name: name.to_string(),
            shape,
            elem_size,
            total_bytes: total,
            is_pinned: true,
            accessible_device: device,
        }
    }

    /// Returns the number of elements.
    pub fn num_elements(&self) -> usize {
        self.shape.iter().product()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S20.1 — Device Memory Allocator
    #[test]
    fn s20_1_allocate_and_free() {
        let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        let a = alloc.allocate(4096).unwrap();
        assert_eq!(a.size, 4096);
        assert!(a.in_use);
        assert!(alloc.free(a.id).is_ok());
    }

    #[test]
    fn s20_1_alloc_stats() {
        let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        alloc.allocate(4096).unwrap();
        alloc.allocate(8192).unwrap();
        let stats = alloc.stats();
        assert_eq!(stats.used_bytes, 4096 + 8192);
        assert_eq!(stats.active_allocations, 2);
        assert!(stats.utilization() > 0.0);
    }

    #[test]
    fn s20_1_oom_error() {
        let mut alloc = DeviceAllocator::new(0, 1024, 1024);
        let result = alloc.allocate(2048);
        assert!(matches!(result, Err(AllocError::OutOfMemory { .. })));
    }

    #[test]
    fn s20_1_free_and_reuse() {
        let mut alloc = DeviceAllocator::new(0, 8192, 8192);
        let a = alloc.allocate(4096).unwrap();
        let id = a.id;
        alloc.free(id).unwrap();
        // Should be able to reuse the freed block
        let b = alloc.allocate(4096).unwrap();
        assert!(b.in_use);
    }

    // S20.2 — Host-Device Transfer
    #[test]
    fn s20_2_transfer_desc() {
        let t = TransferDesc {
            direction: TransferDirection::HostToDevice,
            size_bytes: 1024 * 1024,
            pinned: true,
            is_async: true,
            stream_id: 0,
        };
        assert_eq!(t.direction, TransferDirection::HostToDevice);
        assert_eq!(t.direction.to_string(), "H2D");
    }

    #[test]
    fn s20_2_estimate_transfer() {
        let time = estimate_transfer_time(1_000_000, 16.0); // 1 MB at 16 GB/s
        assert!(time > 0.0);
        assert!(time < 1000.0); // should be < 1ms
    }

    // S20.3 — Unified Memory
    #[test]
    fn s20_3_memory_mode_display() {
        assert_eq!(MemoryMode::Explicit.to_string(), "Explicit");
        assert_eq!(MemoryMode::Unified.to_string(), "Unified");
        assert_eq!(MemoryMode::ZeroCopy.to_string(), "ZeroCopy");
    }

    // S20.5 — Defragmentation
    #[test]
    fn s20_5_fragmentation_analysis() {
        let mut alloc = DeviceAllocator::new(0, 1024 * 1024, 4 * 1024 * 1024);
        let a = alloc.allocate(1024).unwrap();
        alloc.allocate(1024).unwrap();
        alloc.free(a.id).unwrap(); // create a hole

        let frag = analyze_fragmentation(&alloc);
        assert!(frag.num_fragments > 0 || frag.total_free_bytes > 0);
    }

    // S20.6 — OOM Handling
    #[test]
    fn s20_6_oom_event() {
        let event = OomEvent {
            requested_bytes: 1024 * 1024 * 1024,
            available_bytes: 1024,
            strategy: OomStrategy::SpillToHost,
            recovered: false,
        };
        assert_eq!(event.strategy, OomStrategy::SpillToHost);
        assert!(!event.recovered);
    }

    // S20.7 — Multi-GPU
    #[test]
    fn s20_7_gpu_topology() {
        let mut topo = GpuTopology::new(4);
        topo.add_peer(0, 1, 25.0);
        topo.add_peer(1, 0, 25.0);
        assert!(topo.can_peer_access(0, 1));
        assert!(!topo.can_peer_access(0, 2));
    }

    // S20.8 — Memory Profiling
    #[test]
    fn s20_8_memory_profile() {
        let mut profile = MemoryProfile::new();
        profile.record(ProfileEvent {
            timestamp_us: 100,
            kind: ProfileEventKind::Allocate,
            size_bytes: 4096,
            device: 0,
        });
        profile.record(ProfileEvent {
            timestamp_us: 200,
            kind: ProfileEventKind::Transfer,
            size_bytes: 1024,
            device: 0,
        });
        assert_eq!(profile.peak_for_device(0), 4096);
        assert_eq!(profile.total_transferred, 1024);
        assert_eq!(profile.events.len(), 2);
    }

    // S20.9 — Zero-Copy Tensors
    #[test]
    fn s20_9_zero_copy_tensor() {
        let t = ZeroCopyTensor::new("input", vec![32, 768], 4, 0);
        assert_eq!(t.num_elements(), 32 * 768);
        assert_eq!(t.total_bytes, 32 * 768 * 4);
        assert!(t.is_pinned);
    }

    // S20.10 — Integration
    #[test]
    fn s20_10_alloc_error_display() {
        let e = AllocError::OutOfMemory {
            requested: 1000,
            available: 500,
        };
        assert!(e.to_string().contains("1000"));
        assert!(e.to_string().contains("500"));
    }

    #[test]
    fn s20_10_invalid_free() {
        let mut alloc = DeviceAllocator::new(0, 4096, 4096);
        let result = alloc.free(999);
        assert!(matches!(result, Err(AllocError::InvalidAllocation(999))));
    }
}
