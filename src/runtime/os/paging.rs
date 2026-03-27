//! x86_64 4-level page table (PML4 → PDP → PD → PT).
//!
//! Models the hierarchical virtual memory translation used by x86_64
//! processors. Supports 4 KiB pages with standard flags: Present,
//! Writable, UserAccessible, NoExecute, etc.
//!
//! Address bits (48-bit canonical virtual address):
//! ```text
//! [63:48] Sign extend of bit 47 (canonical check)
//! [47:39] PML4 index   (9 bits, 512 entries)
//! [38:30] PDP index    (9 bits, 512 entries)
//! [29:21] PD index     (9 bits, 512 entries)
//! [20:12] PT index     (9 bits, 512 entries)
//! [11:0]  Page offset  (12 bits, 4 KiB)
//! ```

use super::memory::{PhysAddr, VirtAddr};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Number of entries per page table level (512 for x86_64).
pub const ENTRIES_PER_TABLE: usize = 512;

/// Page size (4 KiB).
pub const PAGE_SIZE_4K: u64 = 4096;

/// Mask for 48-bit physical address extraction.
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

// ═══════════════════════════════════════════════════════════════════════
// Page table entry flags
// ═══════════════════════════════════════════════════════════════════════

/// Flags for a page table entry (x86_64 format).
///
/// Bit layout matches the hardware:
/// ```text
/// Bit 0:  Present
/// Bit 1:  Writable
/// Bit 2:  User-accessible
/// Bit 3:  Write-through
/// Bit 4:  Cache disable
/// Bit 5:  Accessed
/// Bit 6:  Dirty
/// Bit 63: No-execute (NX)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableFlags(u64);

impl PageTableFlags {
    /// Entry is present (mapped).
    pub const PRESENT: Self = Self(1 << 0);
    /// Page is writable.
    pub const WRITABLE: Self = Self(1 << 1);
    /// Page is accessible from user mode (ring 3).
    pub const USER_ACCESSIBLE: Self = Self(1 << 2);
    /// Write-through caching.
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    /// Cache disabled for this page.
    pub const CACHE_DISABLE: Self = Self(1 << 4);
    /// Page has been accessed (set by CPU).
    pub const ACCESSED: Self = Self(1 << 5);
    /// Page has been written to (set by CPU).
    pub const DIRTY: Self = Self(1 << 6);
    /// No-execute bit (requires NX support).
    pub const NO_EXECUTE: Self = Self(1 << 63);

    /// No flags set.
    pub const EMPTY: Self = Self(0);

    /// Kernel read-write: Present + Writable.
    pub const KERNEL_RW: Self = Self(0b11);
    /// Kernel read-only: Present.
    pub const KERNEL_RO: Self = Self(0b01);
    /// User read-write: Present + Writable + UserAccessible.
    pub const USER_RW: Self = Self(0b111);

    /// Create from raw bits.
    pub fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    pub fn bits(self) -> u64 {
        self.0
    }

    /// Check if a flag is set.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Combine flags.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if entry is present.
    pub fn is_present(self) -> bool {
        self.contains(Self::PRESENT)
    }

    /// Check if entry is writable.
    pub fn is_writable(self) -> bool {
        self.contains(Self::WRITABLE)
    }

    /// Check if entry is user-accessible.
    pub fn is_user(self) -> bool {
        self.contains(Self::USER_ACCESSIBLE)
    }

    /// Check if entry has no-execute.
    pub fn is_no_execute(self) -> bool {
        self.contains(Self::NO_EXECUTE)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Page table entry
// ═══════════════════════════════════════════════════════════════════════

/// A single page table entry (8 bytes on x86_64).
///
/// Contains a physical frame address and flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// Empty (not present) entry.
    pub const EMPTY: Self = Self(0);

    /// Create from a physical address and flags.
    pub fn new(phys_addr: PhysAddr, flags: PageTableFlags) -> Self {
        Self((phys_addr.0 & ADDR_MASK) | flags.0)
    }

    /// Get the physical address stored in this entry.
    pub fn addr(self) -> PhysAddr {
        PhysAddr(self.0 & ADDR_MASK)
    }

    /// Get the flags.
    pub fn flags(self) -> PageTableFlags {
        PageTableFlags(self.0 & !ADDR_MASK)
    }

    /// Check if the entry is present.
    pub fn is_present(self) -> bool {
        self.flags().is_present()
    }

    /// Get raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Page table level (512 entries)
// ═══════════════════════════════════════════════════════════════════════

/// A single level of the page table hierarchy (512 entries).
#[derive(Debug, Clone)]
struct PageTableLevel {
    entries: Vec<PageTableEntry>,
}

impl PageTableLevel {
    fn new() -> Self {
        Self {
            entries: vec![PageTableEntry::EMPTY; ENTRIES_PER_TABLE],
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Paging errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from page table operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PagingError {
    /// Virtual address is already mapped.
    #[error("page already mapped: virtual {virt:#x}")]
    AlreadyMapped { virt: u64 },

    /// Virtual address is not mapped.
    #[error("page not mapped: virtual {virt:#x}")]
    NotMapped { virt: u64 },

    /// Address is not page-aligned.
    #[error("address not page-aligned: {addr:#x}")]
    NotAligned { addr: u64 },

    /// Virtual address is not canonical (x86_64 48-bit).
    #[error("non-canonical virtual address: {addr:#x}")]
    NonCanonical { addr: u64 },
}

// ═══════════════════════════════════════════════════════════════════════
// TLB simulation
// ═══════════════════════════════════════════════════════════════════════

/// Simulated TLB (Translation Lookaside Buffer).
///
/// Caches recent virtual→physical translations.
#[derive(Debug)]
struct Tlb {
    /// Cached entries: virtual page → (physical page, flags).
    cache: std::collections::HashMap<u64, (u64, PageTableFlags)>,
    /// Total flushes performed (for testing).
    flush_count: u64,
}

impl Tlb {
    fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            flush_count: 0,
        }
    }

    fn lookup(&self, virt_page: u64) -> Option<(u64, PageTableFlags)> {
        self.cache.get(&virt_page).copied()
    }

    fn insert(&mut self, virt_page: u64, phys_page: u64, flags: PageTableFlags) {
        self.cache.insert(virt_page, (phys_page, flags));
    }

    fn invalidate(&mut self, virt_page: u64) {
        self.cache.remove(&virt_page);
        self.flush_count += 1;
    }

    fn flush_all(&mut self) {
        self.cache.clear();
        self.flush_count += 1;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4-level page table
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 4-level page table with TLB simulation.
///
/// Hierarchy: PML4 → PDP → PD → PT → Physical frame.
///
/// Each level has 512 entries. Tables are created on demand
/// when pages are mapped.
#[derive(Debug)]
pub struct FourLevelPageTable {
    /// PML4 (root) table.
    pml4: PageTableLevel,
    /// PDP (level 3) tables, indexed by PML4 entry.
    pdp_tables: std::collections::HashMap<usize, PageTableLevel>,
    /// PD (level 2) tables, indexed by (PML4, PDP) pair.
    pd_tables: std::collections::HashMap<(usize, usize), PageTableLevel>,
    /// PT (level 1) tables, indexed by (PML4, PDP, PD) triple.
    pt_tables: std::collections::HashMap<(usize, usize, usize), PageTableLevel>,
    /// Next available physical frame for intermediate tables.
    next_frame: u64,
    /// Simulated TLB.
    tlb: Tlb,
    /// Total mapped pages.
    mapped_count: usize,
}

impl FourLevelPageTable {
    /// Create a new empty 4-level page table.
    ///
    /// `frame_start` is the physical address where new page table
    /// frames will be allocated from.
    pub fn new(frame_start: u64) -> Self {
        Self {
            pml4: PageTableLevel::new(),
            pdp_tables: std::collections::HashMap::new(),
            pd_tables: std::collections::HashMap::new(),
            pt_tables: std::collections::HashMap::new(),
            next_frame: frame_start,
            tlb: Tlb::new(),
            mapped_count: 0,
        }
    }

    /// Map a virtual page to a physical frame with the given flags.
    ///
    /// Both addresses must be page-aligned. Creates intermediate
    /// page table levels as needed.
    pub fn map_page(
        &mut self,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: PageTableFlags,
    ) -> Result<(), PagingError> {
        if !virt.0.is_multiple_of(PAGE_SIZE_4K) {
            return Err(PagingError::NotAligned { addr: virt.0 });
        }
        if !phys.0.is_multiple_of(PAGE_SIZE_4K) {
            return Err(PagingError::NotAligned { addr: phys.0 });
        }

        let (pml4_idx, pdp_idx, pd_idx, pt_idx) = Self::split_virt(virt.0);

        // Pre-allocate frames for intermediate tables to avoid borrow issues
        let need_pdp = !self.pml4.entries[pml4_idx].is_present();
        let need_pd = need_pdp
            || self
                .pdp_tables
                .get(&pml4_idx)
                .is_none_or(|t| !t.entries[pdp_idx].is_present());
        let need_pt = need_pd
            || self
                .pd_tables
                .get(&(pml4_idx, pdp_idx))
                .is_none_or(|t| !t.entries[pd_idx].is_present());

        let frame_pdp = if need_pdp {
            Some(self.alloc_frame())
        } else {
            None
        };
        let frame_pd = if need_pd {
            Some(self.alloc_frame())
        } else {
            None
        };
        let frame_pt = if need_pt {
            Some(self.alloc_frame())
        } else {
            None
        };

        // Ensure PML4 → PDP link exists
        if let Some(frame) = frame_pdp {
            self.pml4.entries[pml4_idx] =
                PageTableEntry::new(PhysAddr(frame), PageTableFlags::KERNEL_RW);
            self.pdp_tables.insert(pml4_idx, PageTableLevel::new());
        }

        // Ensure PDP → PD link exists
        if let Some(frame) = frame_pd {
            let pdp = self.pdp_tables.get_mut(&pml4_idx).expect(
                "paging: PDP table must exist for pml4_idx after PML4→PDP link was just created",
            );
            pdp.entries[pdp_idx] = PageTableEntry::new(PhysAddr(frame), PageTableFlags::KERNEL_RW);
            self.pd_tables
                .insert((pml4_idx, pdp_idx), PageTableLevel::new());
        }

        // Ensure PD → PT link exists
        if let Some(frame) = frame_pt {
            let pd = self
                .pd_tables
                .get_mut(&(pml4_idx, pdp_idx))
                .expect(
                    "paging: PD table must exist for (pml4_idx, pdp_idx) after PDP→PD link was just created",
                );
            pd.entries[pd_idx] = PageTableEntry::new(PhysAddr(frame), PageTableFlags::KERNEL_RW);
            self.pt_tables
                .insert((pml4_idx, pdp_idx, pd_idx), PageTableLevel::new());
        }

        // Set the leaf PT entry
        let pt = self
            .pt_tables
            .get_mut(&(pml4_idx, pdp_idx, pd_idx))
            .expect(
                "paging: PT table must exist for (pml4_idx, pdp_idx, pd_idx) after PD→PT link was created or pre-existed",
            );

        if pt.entries[pt_idx].is_present() {
            return Err(PagingError::AlreadyMapped { virt: virt.0 });
        }

        let entry_flags = flags.union(PageTableFlags::PRESENT);
        pt.entries[pt_idx] = PageTableEntry::new(phys, entry_flags);
        self.mapped_count += 1;

        // Invalidate TLB for this page
        self.tlb.invalidate(virt.0 / PAGE_SIZE_4K);

        Ok(())
    }

    /// Unmap a virtual page.
    pub fn unmap_page(&mut self, virt: VirtAddr) -> Result<(), PagingError> {
        if !virt.0.is_multiple_of(PAGE_SIZE_4K) {
            return Err(PagingError::NotAligned { addr: virt.0 });
        }

        let (pml4_idx, pdp_idx, pd_idx, pt_idx) = Self::split_virt(virt.0);

        let pt = self
            .pt_tables
            .get_mut(&(pml4_idx, pdp_idx, pd_idx))
            .ok_or(PagingError::NotMapped { virt: virt.0 })?;

        if !pt.entries[pt_idx].is_present() {
            return Err(PagingError::NotMapped { virt: virt.0 });
        }

        pt.entries[pt_idx] = PageTableEntry::EMPTY;
        self.mapped_count -= 1;

        // Invalidate TLB for this page (simulated invlpg)
        self.tlb.invalidate(virt.0 / PAGE_SIZE_4K);

        Ok(())
    }

    /// Translate a virtual address to physical, walking all 4 levels.
    ///
    /// Checks TLB first, falls back to full table walk.
    pub fn translate(&mut self, virt: VirtAddr) -> Result<(PhysAddr, PageTableFlags), PagingError> {
        let virt_page = virt.0 / PAGE_SIZE_4K;
        let offset = virt.0 % PAGE_SIZE_4K;

        // Check TLB first
        if let Some((phys_page, flags)) = self.tlb.lookup(virt_page) {
            return Ok((PhysAddr(phys_page * PAGE_SIZE_4K + offset), flags));
        }

        // Full 4-level walk
        let (phys, flags) = self.walk(virt)?;

        // Cache in TLB
        self.tlb.insert(virt_page, phys.0 / PAGE_SIZE_4K, flags);

        Ok((PhysAddr(phys.0 + offset), flags))
    }

    /// Perform a full 4-level page table walk (no TLB).
    fn walk(&self, virt: VirtAddr) -> Result<(PhysAddr, PageTableFlags), PagingError> {
        let (pml4_idx, pdp_idx, pd_idx, pt_idx) = Self::split_virt(virt.0);

        // Level 4: PML4
        if !self.pml4.entries[pml4_idx].is_present() {
            return Err(PagingError::NotMapped { virt: virt.0 });
        }

        // Level 3: PDP
        let pdp = self
            .pdp_tables
            .get(&pml4_idx)
            .ok_or(PagingError::NotMapped { virt: virt.0 })?;
        if !pdp.entries[pdp_idx].is_present() {
            return Err(PagingError::NotMapped { virt: virt.0 });
        }

        // Level 2: PD
        let pd = self
            .pd_tables
            .get(&(pml4_idx, pdp_idx))
            .ok_or(PagingError::NotMapped { virt: virt.0 })?;
        if !pd.entries[pd_idx].is_present() {
            return Err(PagingError::NotMapped { virt: virt.0 });
        }

        // Level 1: PT (leaf)
        let pt = self
            .pt_tables
            .get(&(pml4_idx, pdp_idx, pd_idx))
            .ok_or(PagingError::NotMapped { virt: virt.0 })?;
        let entry = pt.entries[pt_idx];
        if !entry.is_present() {
            return Err(PagingError::NotMapped { virt: virt.0 });
        }

        Ok((entry.addr(), entry.flags()))
    }

    /// Simulate `invlpg` — flush a single TLB entry.
    pub fn invlpg(&mut self, virt: VirtAddr) {
        self.tlb.invalidate(virt.0 / PAGE_SIZE_4K);
    }

    /// Flush the entire TLB (simulates writing to CR3).
    pub fn flush_tlb(&mut self) {
        self.tlb.flush_all();
    }

    /// Get the number of TLB flushes performed.
    pub fn tlb_flush_count(&self) -> u64 {
        self.tlb.flush_count
    }

    /// Number of mapped pages.
    pub fn mapped_count(&self) -> usize {
        self.mapped_count
    }

    /// Create an identity mapping for a range of physical addresses.
    ///
    /// Maps virtual == physical for each page in `[start, start + size)`.
    pub fn identity_map(
        &mut self,
        start: PhysAddr,
        size: u64,
        flags: PageTableFlags,
    ) -> Result<(), PagingError> {
        let aligned_start = start.0 & !(PAGE_SIZE_4K - 1);
        let aligned_end = (start.0 + size + PAGE_SIZE_4K - 1) & !(PAGE_SIZE_4K - 1);

        let mut addr = aligned_start;
        while addr < aligned_end {
            self.map_page(VirtAddr(addr), PhysAddr(addr), flags)?;
            addr += PAGE_SIZE_4K;
        }
        Ok(())
    }

    /// Split a virtual address into 4-level indices.
    fn split_virt(virt: u64) -> (usize, usize, usize, usize) {
        let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
        let pdp_idx = ((virt >> 30) & 0x1FF) as usize;
        let pd_idx = ((virt >> 21) & 0x1FF) as usize;
        let pt_idx = ((virt >> 12) & 0x1FF) as usize;
        (pml4_idx, pdp_idx, pd_idx, pt_idx)
    }

    /// Allocate a new physical frame for intermediate page tables.
    fn alloc_frame(&mut self) -> u64 {
        let frame = self.next_frame;
        self.next_frame += PAGE_SIZE_4K;
        frame
    }
}

impl Default for FourLevelPageTable {
    fn default() -> Self {
        // Start allocating frames at 1 MiB
        Self::new(0x0010_0000)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_and_translate_single_page() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        let (phys, flags) = pt.translate(VirtAddr(0x0020_0000)).unwrap();
        assert_eq!(phys.0, 0x0040_0000);
        assert!(flags.is_present());
        assert!(flags.is_writable());
    }

    #[test]
    fn translate_with_page_offset() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        // Access with offset 0x100 within the page
        let (phys, _) = pt.translate(VirtAddr(0x0020_0100)).unwrap();
        assert_eq!(phys.0, 0x0040_0100);
    }

    #[test]
    fn translate_unmapped_fails() {
        let mut pt = FourLevelPageTable::default();
        let err = pt.translate(VirtAddr(0x1000)).unwrap_err();
        assert_eq!(err, PagingError::NotMapped { virt: 0x1000 });
    }

    #[test]
    fn unmap_page_succeeds() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();
        assert_eq!(pt.mapped_count(), 1);

        pt.unmap_page(VirtAddr(0x0020_0000)).unwrap();
        assert_eq!(pt.mapped_count(), 0);

        // Translation should now fail
        let err = pt.translate(VirtAddr(0x0020_0000)).unwrap_err();
        assert_eq!(err, PagingError::NotMapped { virt: 0x0020_0000 });
    }

    #[test]
    fn unmap_unmapped_fails() {
        let mut pt = FourLevelPageTable::default();
        let err = pt.unmap_page(VirtAddr(0x1000)).unwrap_err();
        assert_eq!(err, PagingError::NotMapped { virt: 0x1000 });
    }

    #[test]
    fn double_map_rejected() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        let err = pt
            .map_page(
                VirtAddr(0x0020_0000),
                PhysAddr(0x0050_0000),
                PageTableFlags::KERNEL_RW,
            )
            .unwrap_err();
        assert_eq!(err, PagingError::AlreadyMapped { virt: 0x0020_0000 });
    }

    #[test]
    fn page_table_flags() {
        let flags = PageTableFlags::PRESENT
            .union(PageTableFlags::WRITABLE)
            .union(PageTableFlags::NO_EXECUTE);
        assert!(flags.is_present());
        assert!(flags.is_writable());
        assert!(flags.is_no_execute());
        assert!(!flags.is_user());
    }

    #[test]
    fn user_accessible_page() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0080_0000),
            PhysAddr(0x0100_0000),
            PageTableFlags::USER_RW,
        )
        .unwrap();

        let (_, flags) = pt.translate(VirtAddr(0x0080_0000)).unwrap();
        assert!(flags.is_present());
        assert!(flags.is_writable());
        assert!(flags.is_user());
    }

    #[test]
    fn identity_mapping() {
        let mut pt = FourLevelPageTable::default();
        // Identity map 4 pages starting at 2 MiB
        pt.identity_map(
            PhysAddr(0x0020_0000),
            4 * PAGE_SIZE_4K,
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        assert_eq!(pt.mapped_count(), 4);

        // Each virtual address should equal physical
        for i in 0..4 {
            let addr = 0x0020_0000 + i * PAGE_SIZE_4K;
            let (phys, _) = pt.translate(VirtAddr(addr)).unwrap();
            assert_eq!(phys.0, addr);
        }
    }

    #[test]
    fn tlb_caching() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        // First translate populates TLB
        let (phys1, _) = pt.translate(VirtAddr(0x0020_0000)).unwrap();
        // Second translate hits TLB
        let (phys2, _) = pt.translate(VirtAddr(0x0020_0000)).unwrap();
        assert_eq!(phys1.0, phys2.0);
    }

    #[test]
    fn invlpg_invalidates_tlb() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        // Populate TLB
        pt.translate(VirtAddr(0x0020_0000)).unwrap();

        let before = pt.tlb_flush_count();
        pt.invlpg(VirtAddr(0x0020_0000));
        assert!(pt.tlb_flush_count() > before);
    }

    #[test]
    fn flush_tlb_clears_all() {
        let mut pt = FourLevelPageTable::default();
        pt.map_page(
            VirtAddr(0x0020_0000),
            PhysAddr(0x0040_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();
        pt.map_page(
            VirtAddr(0x0021_0000),
            PhysAddr(0x0041_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();

        // Populate TLB
        pt.translate(VirtAddr(0x0020_0000)).unwrap();
        pt.translate(VirtAddr(0x0021_0000)).unwrap();

        let before = pt.tlb_flush_count();
        pt.flush_tlb();
        assert!(pt.tlb_flush_count() > before);
    }

    #[test]
    fn not_aligned_rejected() {
        let mut pt = FourLevelPageTable::default();
        let err = pt
            .map_page(
                VirtAddr(0x0020_0001),
                PhysAddr(0x0040_0000),
                PageTableFlags::KERNEL_RW,
            )
            .unwrap_err();
        assert_eq!(err, PagingError::NotAligned { addr: 0x0020_0001 });
    }

    #[test]
    fn page_table_entry_encode() {
        let entry = PageTableEntry::new(
            PhysAddr(0x0040_0000),
            PageTableFlags::PRESENT.union(PageTableFlags::WRITABLE),
        );
        assert!(entry.is_present());
        assert_eq!(entry.addr().0, 0x0040_0000);
        assert!(entry.flags().is_writable());
    }

    #[test]
    fn split_virt_correctness() {
        // Address: PML4=1, PDP=2, PD=3, PT=4, offset=0
        let virt = (1u64 << 39) | (2u64 << 30) | (3u64 << 21) | (4u64 << 12);
        let (pml4, pdp, pd, pt) = FourLevelPageTable::split_virt(virt);
        assert_eq!(pml4, 1);
        assert_eq!(pdp, 2);
        assert_eq!(pd, 3);
        assert_eq!(pt, 4);
    }

    #[test]
    fn multiple_pages_across_tables() {
        let mut pt = FourLevelPageTable::default();

        // Map pages in different PD entries (different bits 29:21)
        let addr1 = 0x0020_0000; // PD idx changes
        let addr2 = 0x0040_0000;
        let addr3 = 0x0060_0000;

        pt.map_page(
            VirtAddr(addr1),
            PhysAddr(0x100_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();
        pt.map_page(
            VirtAddr(addr2),
            PhysAddr(0x200_0000),
            PageTableFlags::KERNEL_RW,
        )
        .unwrap();
        pt.map_page(
            VirtAddr(addr3),
            PhysAddr(0x300_0000),
            PageTableFlags::KERNEL_RO,
        )
        .unwrap();

        assert_eq!(pt.mapped_count(), 3);

        let (p1, _) = pt.translate(VirtAddr(addr1)).unwrap();
        let (p2, _) = pt.translate(VirtAddr(addr2)).unwrap();
        let (p3, f3) = pt.translate(VirtAddr(addr3)).unwrap();

        assert_eq!(p1.0, 0x100_0000);
        assert_eq!(p2.0, 0x200_0000);
        assert_eq!(p3.0, 0x300_0000);
        assert!(f3.is_present());
        assert!(!f3.is_writable());
    }
}
