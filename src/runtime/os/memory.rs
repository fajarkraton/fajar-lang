//! Memory manager — heap simulation, virtual/physical addresses, page tables.
//!
//! Provides a simulated memory subsystem for OS-level programming in Fajar Lang.
//! All memory operations are bounds-checked and safe by default.
//!
//! # Architecture
//!
//! ```text
//! MemoryManager
//! ├── backing: Vec<u8>           — simulated physical memory
//! ├── regions: Vec<MemoryRegion> — allocated region tracking
//! └── page_table: PageTable      — virtual → physical mapping
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Address types
// ═══════════════════════════════════════════════════════════════════════

/// A virtual memory address (distinct from PhysAddr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VirtAddr(pub u64);

/// A physical memory address (distinct from VirtAddr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhysAddr(pub u64);

impl std::fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08x}", self.0)
    }
}

impl std::fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:08x}", self.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Page flags
// ═══════════════════════════════════════════════════════════════════════

/// Memory protection flags for pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u8);

impl PageFlags {
    /// Page is readable.
    pub const READ: PageFlags = PageFlags(0b0001);
    /// Page is writable.
    pub const WRITE: PageFlags = PageFlags(0b0010);
    /// Page is executable.
    pub const EXEC: PageFlags = PageFlags(0b0100);
    /// Page is accessible from user mode.
    pub const USER: PageFlags = PageFlags(0b1000);
    /// Read + Write.
    pub const RW: PageFlags = PageFlags(0b0011);
    /// Read + Execute.
    pub const RX: PageFlags = PageFlags(0b0101);
    /// Read + Write + Execute.
    pub const RWX: PageFlags = PageFlags(0b0111);

    /// Returns true if this flag set contains the given flag.
    pub fn contains(self, flag: PageFlags) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Combines two flag sets.
    pub fn union(self, other: PageFlags) -> PageFlags {
        PageFlags(self.0 | other.0)
    }

    /// Creates flags from raw bits.
    pub fn from_bits(bits: u8) -> PageFlags {
        PageFlags(bits)
    }

    /// Returns the raw bits.
    pub fn bits(self) -> u8 {
        self.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Memory errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from memory operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MemoryError {
    /// ME006: Allocation failed (out of memory or invalid size).
    #[error("ME006: allocation failed: {reason}")]
    AllocFailed { reason: String },

    /// ME007: Invalid free (address not from alloc).
    #[error("ME007: invalid free at {addr}")]
    InvalidFree { addr: VirtAddr },

    /// ME002: Double free.
    #[error("ME002: double free at {addr}")]
    DoubleFree { addr: VirtAddr },

    /// Out of bounds memory access.
    #[error("out of bounds access at {addr} (size {size}, memory size {mem_size})")]
    OutOfBounds {
        addr: VirtAddr,
        size: usize,
        mem_size: usize,
    },

    /// Write to read-only page.
    #[error("protection violation: write to read-only page at {addr}")]
    ProtectionViolation { addr: VirtAddr },

    /// Page not mapped.
    #[error("page fault: no mapping for {addr}")]
    PageFault { addr: VirtAddr },

    /// Page already mapped.
    #[error("page already mapped at {addr}")]
    AlreadyMapped { addr: VirtAddr },
}

// ═══════════════════════════════════════════════════════════════════════
// Memory region
// ═══════════════════════════════════════════════════════════════════════

/// Tracks an allocated region of memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRegion {
    /// Starting address.
    pub start: u64,
    /// Size in bytes.
    pub size: usize,
    /// Whether this region is currently allocated.
    pub allocated: bool,
}

impl MemoryRegion {
    /// Returns the end address (exclusive) of this region.
    pub fn end(&self) -> u64 {
        self.start + self.size as u64
    }

    /// Returns true if this region overlaps with another.
    pub fn overlaps(&self, other: &MemoryRegion) -> bool {
        self.start < other.end() && other.start < self.end()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Page table
// ═══════════════════════════════════════════════════════════════════════

/// Page size (4 KiB).
pub const PAGE_SIZE: u64 = 4096;

/// Virtual-to-physical page table.
#[derive(Debug, Clone)]
pub struct PageTable {
    /// Mapping from virtual page number to (physical page number, flags).
    entries: HashMap<u64, (u64, PageFlags)>,
}

impl PageTable {
    /// Creates an empty page table.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Maps a virtual address to a physical address with the given flags.
    ///
    /// Both addresses are page-aligned (rounded down to PAGE_SIZE boundary).
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MemoryError> {
        let vpn = va.0 / PAGE_SIZE;
        if self.entries.contains_key(&vpn) {
            return Err(MemoryError::AlreadyMapped { addr: va });
        }
        let ppn = pa.0 / PAGE_SIZE;
        self.entries.insert(vpn, (ppn, flags));
        Ok(())
    }

    /// Unmaps a virtual address.
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<(), MemoryError> {
        let vpn = va.0 / PAGE_SIZE;
        if self.entries.remove(&vpn).is_none() {
            return Err(MemoryError::PageFault { addr: va });
        }
        Ok(())
    }

    /// Translates a virtual address to a physical address and flags.
    pub fn translate(&self, va: VirtAddr) -> Result<(PhysAddr, PageFlags), MemoryError> {
        let vpn = va.0 / PAGE_SIZE;
        let offset = va.0 % PAGE_SIZE;
        match self.entries.get(&vpn) {
            Some(&(ppn, flags)) => Ok((PhysAddr(ppn * PAGE_SIZE + offset), flags)),
            None => Err(MemoryError::PageFault { addr: va }),
        }
    }

    /// Returns the number of mapped pages.
    pub fn page_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Memory manager
// ═══════════════════════════════════════════════════════════════════════

/// Default memory size: 1 MiB.
pub const DEFAULT_MEMORY_SIZE: usize = 1024 * 1024;

/// Simulated memory manager for OS-level programming.
///
/// Provides heap allocation, deallocation, read/write operations,
/// and virtual memory management via a page table.
#[derive(Debug)]
pub struct MemoryManager {
    /// Backing store (simulated physical memory).
    backing: Vec<u8>,
    /// Tracked allocated regions.
    regions: Vec<MemoryRegion>,
    /// Virtual-to-physical page table.
    pub page_table: PageTable,
    /// Next allocation address (simple bump allocator).
    next_addr: u64,
}

impl MemoryManager {
    /// Creates a new memory manager with the given size in bytes.
    pub fn new(size: usize) -> Self {
        Self {
            backing: vec![0u8; size],
            regions: Vec::new(),
            page_table: PageTable::new(),
            next_addr: 0,
        }
    }

    /// Creates a memory manager with the default 1 MiB size.
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_MEMORY_SIZE)
    }

    /// Returns the total memory size in bytes.
    pub fn size(&self) -> usize {
        self.backing.len()
    }

    /// Allocates a region of `size` bytes with the given alignment.
    ///
    /// Returns the starting virtual address of the allocated region.
    /// Alignment must be a power of two.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<VirtAddr, MemoryError> {
        if size == 0 {
            return Err(MemoryError::AllocFailed {
                reason: "cannot allocate 0 bytes".into(),
            });
        }
        if align == 0 || !align.is_power_of_two() {
            return Err(MemoryError::AllocFailed {
                reason: format!("alignment must be a power of two, got {align}"),
            });
        }

        // Align the next address
        let aligned_addr = align_up(self.next_addr, align as u64);

        // Check if it fits
        let end = aligned_addr + size as u64;
        if end > self.backing.len() as u64 {
            return Err(MemoryError::AllocFailed {
                reason: format!(
                    "out of memory: need {} bytes at {}, but only {} total",
                    size,
                    aligned_addr,
                    self.backing.len()
                ),
            });
        }

        // Record the region
        self.regions.push(MemoryRegion {
            start: aligned_addr,
            size,
            allocated: true,
        });

        self.next_addr = end;

        Ok(VirtAddr(aligned_addr))
    }

    /// Frees a previously allocated region at the given address.
    pub fn free(&mut self, addr: VirtAddr) -> Result<(), MemoryError> {
        for region in &mut self.regions {
            if region.start == addr.0 {
                if !region.allocated {
                    return Err(MemoryError::DoubleFree { addr });
                }
                region.allocated = false;
                return Ok(());
            }
        }
        Err(MemoryError::InvalidFree { addr })
    }

    /// Reads a single byte from the given address.
    pub fn read_u8(&self, addr: VirtAddr) -> Result<u8, MemoryError> {
        let offset = addr.0 as usize;
        if offset >= self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 1,
                mem_size: self.backing.len(),
            });
        }
        Ok(self.backing[offset])
    }

    /// Writes a single byte to the given address.
    pub fn write_u8(&mut self, addr: VirtAddr, value: u8) -> Result<(), MemoryError> {
        let offset = addr.0 as usize;
        if offset >= self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 1,
                mem_size: self.backing.len(),
            });
        }
        self.backing[offset] = value;
        Ok(())
    }

    /// Reads a 32-bit unsigned integer (little-endian) from the given address.
    pub fn read_u32(&self, addr: VirtAddr) -> Result<u32, MemoryError> {
        let offset = addr.0 as usize;
        if offset + 4 > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 4,
                mem_size: self.backing.len(),
            });
        }
        let bytes: [u8; 4] = self.backing[offset..offset + 4].try_into().unwrap();
        Ok(u32::from_le_bytes(bytes))
    }

    /// Writes a 32-bit unsigned integer (little-endian) to the given address.
    pub fn write_u32(&mut self, addr: VirtAddr, value: u32) -> Result<(), MemoryError> {
        let offset = addr.0 as usize;
        if offset + 4 > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 4,
                mem_size: self.backing.len(),
            });
        }
        let bytes = value.to_le_bytes();
        self.backing[offset..offset + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Reads a 64-bit unsigned integer (little-endian) from the given address.
    pub fn read_u64(&self, addr: VirtAddr) -> Result<u64, MemoryError> {
        let offset = addr.0 as usize;
        if offset + 8 > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 8,
                mem_size: self.backing.len(),
            });
        }
        let bytes: [u8; 8] = self.backing[offset..offset + 8].try_into().unwrap();
        Ok(u64::from_le_bytes(bytes))
    }

    /// Writes a 64-bit unsigned integer (little-endian) to the given address.
    pub fn write_u64(&mut self, addr: VirtAddr, value: u64) -> Result<(), MemoryError> {
        let offset = addr.0 as usize;
        if offset + 8 > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: 8,
                mem_size: self.backing.len(),
            });
        }
        let bytes = value.to_le_bytes();
        self.backing[offset..offset + 8].copy_from_slice(&bytes);
        Ok(())
    }

    /// Reads a slice of bytes from the given address.
    pub fn read_bytes(&self, addr: VirtAddr, len: usize) -> Result<Vec<u8>, MemoryError> {
        let offset = addr.0 as usize;
        if offset + len > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: len,
                mem_size: self.backing.len(),
            });
        }
        Ok(self.backing[offset..offset + len].to_vec())
    }

    /// Writes a slice of bytes to the given address.
    pub fn write_bytes(&mut self, addr: VirtAddr, data: &[u8]) -> Result<(), MemoryError> {
        let offset = addr.0 as usize;
        if offset + data.len() > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: data.len(),
                mem_size: self.backing.len(),
            });
        }
        self.backing[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Returns a list of all currently allocated regions.
    pub fn allocated_regions(&self) -> Vec<&MemoryRegion> {
        self.regions.iter().filter(|r| r.allocated).collect()
    }

    /// Copies `count` bytes from `src` to `dst` in memory.
    pub fn memory_copy(
        &mut self,
        src: VirtAddr,
        dst: VirtAddr,
        count: usize,
    ) -> Result<(), MemoryError> {
        let src_off = src.0 as usize;
        let dst_off = dst.0 as usize;
        if src_off + count > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr: src,
                size: count,
                mem_size: self.backing.len(),
            });
        }
        if dst_off + count > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr: dst,
                size: count,
                mem_size: self.backing.len(),
            });
        }
        // Handle overlapping copies
        let data: Vec<u8> = self.backing[src_off..src_off + count].to_vec();
        self.backing[dst_off..dst_off + count].copy_from_slice(&data);
        Ok(())
    }

    /// Sets `count` bytes starting at `addr` to `value`.
    pub fn memory_set(
        &mut self,
        addr: VirtAddr,
        value: u8,
        count: usize,
    ) -> Result<(), MemoryError> {
        let offset = addr.0 as usize;
        if offset + count > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr,
                size: count,
                mem_size: self.backing.len(),
            });
        }
        self.backing[offset..offset + count].fill(value);
        Ok(())
    }

    /// Compares `count` bytes at `a` and `b`, returns 0 if equal, -1 if a < b, 1 if a > b.
    pub fn memory_compare(
        &self,
        a: VirtAddr,
        b: VirtAddr,
        count: usize,
    ) -> Result<i64, MemoryError> {
        let off_a = a.0 as usize;
        let off_b = b.0 as usize;
        if off_a + count > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr: a,
                size: count,
                mem_size: self.backing.len(),
            });
        }
        if off_b + count > self.backing.len() {
            return Err(MemoryError::OutOfBounds {
                addr: b,
                size: count,
                mem_size: self.backing.len(),
            });
        }
        let slice_a = &self.backing[off_a..off_a + count];
        let slice_b = &self.backing[off_b..off_b + count];
        Ok(match slice_a.cmp(slice_b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        })
    }
}

/// Aligns `addr` up to the nearest multiple of `align`.
fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── VirtAddr / PhysAddr ──

    #[test]
    fn virtaddr_display() {
        assert_eq!(format!("{}", VirtAddr(0x1000)), "0x00001000");
    }

    #[test]
    fn physaddr_display() {
        assert_eq!(format!("{}", PhysAddr(0xDEAD)), "0x0000dead");
    }

    #[test]
    fn virtaddr_ne_physaddr_by_design() {
        // VirtAddr and PhysAddr are distinct types — cannot be compared directly.
        // This is a compile-time guarantee, verified here for documentation.
        let _va = VirtAddr(100);
        let _pa = PhysAddr(100);
        // _va == _pa would be a compile error — that's the point.
    }

    // ── PageFlags ──

    #[test]
    fn page_flags_contains() {
        let rw = PageFlags::RW;
        assert!(rw.contains(PageFlags::READ));
        assert!(rw.contains(PageFlags::WRITE));
        assert!(!rw.contains(PageFlags::EXEC));
    }

    #[test]
    fn page_flags_union() {
        let combined = PageFlags::READ.union(PageFlags::EXEC);
        assert!(combined.contains(PageFlags::READ));
        assert!(combined.contains(PageFlags::EXEC));
        assert!(!combined.contains(PageFlags::WRITE));
    }

    // ── MemoryRegion ──

    #[test]
    fn region_end() {
        let r = MemoryRegion {
            start: 100,
            size: 50,
            allocated: true,
        };
        assert_eq!(r.end(), 150);
    }

    #[test]
    fn region_overlaps() {
        let a = MemoryRegion {
            start: 100,
            size: 50,
            allocated: true,
        };
        let b = MemoryRegion {
            start: 140,
            size: 20,
            allocated: true,
        };
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn region_no_overlap() {
        let a = MemoryRegion {
            start: 100,
            size: 50,
            allocated: true,
        };
        let b = MemoryRegion {
            start: 200,
            size: 50,
            allocated: true,
        };
        assert!(!a.overlaps(&b));
    }

    // ── align_up ──

    #[test]
    fn align_up_already_aligned() {
        assert_eq!(align_up(16, 8), 16);
    }

    #[test]
    fn align_up_needs_padding() {
        assert_eq!(align_up(17, 8), 24);
    }

    #[test]
    fn align_up_from_zero() {
        assert_eq!(align_up(0, 4096), 0);
    }

    // ── MemoryManager: allocation ──

    #[test]
    fn alloc_basic() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 1).unwrap();
        assert_eq!(addr, VirtAddr(0));
        assert_eq!(mm.allocated_regions().len(), 1);
    }

    #[test]
    fn alloc_multiple() {
        let mut mm = MemoryManager::new(4096);
        let a1 = mm.alloc(32, 1).unwrap();
        let a2 = mm.alloc(32, 1).unwrap();
        assert_ne!(a1, a2);
        assert_eq!(mm.allocated_regions().len(), 2);
    }

    #[test]
    fn alloc_with_alignment() {
        let mut mm = MemoryManager::new(4096);
        let _ = mm.alloc(1, 1).unwrap(); // addr 0, next_addr = 1
        let addr = mm.alloc(64, 16).unwrap(); // aligned to 16
        assert_eq!(addr.0 % 16, 0);
    }

    #[test]
    fn alloc_zero_size_fails() {
        let mut mm = MemoryManager::new(4096);
        assert!(matches!(
            mm.alloc(0, 1),
            Err(MemoryError::AllocFailed { .. })
        ));
    }

    #[test]
    fn alloc_bad_alignment_fails() {
        let mut mm = MemoryManager::new(4096);
        assert!(matches!(
            mm.alloc(64, 3),
            Err(MemoryError::AllocFailed { .. })
        ));
    }

    #[test]
    fn alloc_out_of_memory() {
        let mut mm = MemoryManager::new(64);
        assert!(matches!(
            mm.alloc(128, 1),
            Err(MemoryError::AllocFailed { .. })
        ));
    }

    // ── MemoryManager: free ──

    #[test]
    fn free_basic() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 1).unwrap();
        mm.free(addr).unwrap();
        assert_eq!(mm.allocated_regions().len(), 0);
    }

    #[test]
    fn free_double_free() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 1).unwrap();
        mm.free(addr).unwrap();
        assert!(matches!(mm.free(addr), Err(MemoryError::DoubleFree { .. })));
    }

    #[test]
    fn free_invalid_address() {
        let mut mm = MemoryManager::new(4096);
        assert!(matches!(
            mm.free(VirtAddr(999)),
            Err(MemoryError::InvalidFree { .. })
        ));
    }

    // ── MemoryManager: read/write ──

    #[test]
    fn write_read_u8() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 1).unwrap();
        mm.write_u8(addr, 0xAB).unwrap();
        assert_eq!(mm.read_u8(addr).unwrap(), 0xAB);
    }

    #[test]
    fn write_read_u32() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 4).unwrap();
        mm.write_u32(addr, 0xDEADBEEF).unwrap();
        assert_eq!(mm.read_u32(addr).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn write_read_u64() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 8).unwrap();
        mm.write_u64(addr, 0x123456789ABCDEF0).unwrap();
        assert_eq!(mm.read_u64(addr).unwrap(), 0x123456789ABCDEF0);
    }

    #[test]
    fn write_read_bytes() {
        let mut mm = MemoryManager::new(4096);
        let addr = mm.alloc(64, 1).unwrap();
        let data = b"Hello, OS!";
        mm.write_bytes(addr, data).unwrap();
        let read = mm.read_bytes(addr, data.len()).unwrap();
        assert_eq!(read, data);
    }

    #[test]
    fn read_out_of_bounds() {
        let mm = MemoryManager::new(16);
        assert!(matches!(
            mm.read_u8(VirtAddr(16)),
            Err(MemoryError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn write_out_of_bounds() {
        let mut mm = MemoryManager::new(16);
        assert!(matches!(
            mm.write_u32(VirtAddr(14), 0),
            Err(MemoryError::OutOfBounds { .. })
        ));
    }

    // ── PageTable ──

    #[test]
    fn page_table_map_and_translate() {
        let mut pt = PageTable::new();
        pt.map_page(VirtAddr(0x1000), PhysAddr(0x2000), PageFlags::RW)
            .unwrap();
        let (pa, flags) = pt.translate(VirtAddr(0x1000)).unwrap();
        assert_eq!(pa, PhysAddr(0x2000));
        assert!(flags.contains(PageFlags::READ));
        assert!(flags.contains(PageFlags::WRITE));
    }

    #[test]
    fn page_table_translate_with_offset() {
        let mut pt = PageTable::new();
        pt.map_page(VirtAddr(0x1000), PhysAddr(0x5000), PageFlags::READ)
            .unwrap();
        // Access at offset 0x100 within the page
        let (pa, _) = pt.translate(VirtAddr(0x1100)).unwrap();
        assert_eq!(pa, PhysAddr(0x5100));
    }

    #[test]
    fn page_table_unmap() {
        let mut pt = PageTable::new();
        pt.map_page(VirtAddr(0x1000), PhysAddr(0x2000), PageFlags::READ)
            .unwrap();
        pt.unmap_page(VirtAddr(0x1000)).unwrap();
        assert!(matches!(
            pt.translate(VirtAddr(0x1000)),
            Err(MemoryError::PageFault { .. })
        ));
    }

    #[test]
    fn page_table_double_map_fails() {
        let mut pt = PageTable::new();
        pt.map_page(VirtAddr(0x1000), PhysAddr(0x2000), PageFlags::READ)
            .unwrap();
        assert!(matches!(
            pt.map_page(VirtAddr(0x1000), PhysAddr(0x3000), PageFlags::READ),
            Err(MemoryError::AlreadyMapped { .. })
        ));
    }

    #[test]
    fn page_table_unmap_nonexistent_fails() {
        let mut pt = PageTable::new();
        assert!(matches!(
            pt.unmap_page(VirtAddr(0x9000)),
            Err(MemoryError::PageFault { .. })
        ));
    }

    #[test]
    fn page_table_page_count() {
        let mut pt = PageTable::new();
        assert_eq!(pt.page_count(), 0);
        pt.map_page(VirtAddr(0x1000), PhysAddr(0x2000), PageFlags::READ)
            .unwrap();
        pt.map_page(VirtAddr(0x2000), PhysAddr(0x3000), PageFlags::RW)
            .unwrap();
        assert_eq!(pt.page_count(), 2);
    }

    #[test]
    fn page_table_translate_unmapped_fails() {
        let pt = PageTable::new();
        assert!(matches!(
            pt.translate(VirtAddr(0x1000)),
            Err(MemoryError::PageFault { .. })
        ));
    }

    // ── Default size ──

    #[test]
    fn default_size_is_1mib() {
        let mm = MemoryManager::with_default_size();
        assert_eq!(mm.size(), 1024 * 1024);
    }

    // ── Alloc + Write + Free cycle ──

    #[test]
    fn alloc_write_free_cycle() {
        let mut mm = MemoryManager::new(4096);

        // Allocate
        let addr = mm.alloc(128, 8).unwrap();

        // Write some data
        mm.write_u32(addr, 42).unwrap();
        mm.write_u32(VirtAddr(addr.0 + 4), 100).unwrap();

        // Verify
        assert_eq!(mm.read_u32(addr).unwrap(), 42);
        assert_eq!(mm.read_u32(VirtAddr(addr.0 + 4)).unwrap(), 100);

        // Free
        mm.free(addr).unwrap();
        assert_eq!(mm.allocated_regions().len(), 0);
    }
}
