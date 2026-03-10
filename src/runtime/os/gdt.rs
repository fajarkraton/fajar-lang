//! x86_64 Global Descriptor Table (GDT).
//!
//! Provides segment descriptors for 64-bit long mode. In long mode
//! most segmentation is disabled, but the GDT is still required for:
//! - Code segment (CS) with L=1 for 64-bit execution
//! - Data segment (SS/DS) for stack and data access
//! - TSS descriptor for interrupt stack switching

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum number of GDT entries.
pub const GDT_MAX_ENTRIES: usize = 8;

/// Null segment selector.
pub const SEL_NULL: u16 = 0x00;
/// Kernel code segment selector.
pub const SEL_KERNEL_CODE: u16 = 0x08;
/// Kernel data segment selector.
pub const SEL_KERNEL_DATA: u16 = 0x10;
/// User code segment selector.
pub const SEL_USER_CODE: u16 = 0x18;
/// User data segment selector.
pub const SEL_USER_DATA: u16 = 0x20;

// ═══════════════════════════════════════════════════════════════════════
// Segment descriptor
// ═══════════════════════════════════════════════════════════════════════

/// A 64-bit GDT segment descriptor.
///
/// Standard 8-byte format used for code/data segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    /// Null descriptor (required as first GDT entry).
    pub const NULL: Self = Self(0);

    /// Create a 64-bit kernel code segment.
    ///
    /// Bits: Base=0, Limit=0, L=1, D=0, Present, Ring 0, Code/Execute/Read.
    pub fn kernel_code() -> Self {
        // Access byte: Present(1) | DPL=0 | S=1 | Code(1) | Conforming(0) | Readable(1) | Accessed(0)
        // = 0b1001_1010 = 0x9A
        // Flags nibble at bits [55:52]: G=0, D=0, L=1, AVL=0 = 0b0010 = 0x2
        let access: u64 = 0x9A;
        let flags: u64 = 0x2; // L=1 (64-bit), D=0
        Self((access << 40) | (flags << 52))
    }

    /// Create a 64-bit kernel data segment.
    ///
    /// Bits: Base=0, Limit=0, Present, Ring 0, Data/Read/Write.
    pub fn kernel_data() -> Self {
        // Access byte: Present(1) | DPL=0 | S=1 | Data(0) | Direction(0) | Writable(1) | Accessed(0)
        // = 0b1001_0010 = 0x92
        let access: u64 = 0x92;
        Self(access << 40)
    }

    /// Create a 64-bit user code segment (Ring 3).
    pub fn user_code() -> Self {
        // Access: Present(1) | DPL=3 | S=1 | Code(1) | Readable(1) = 0xFA
        let access: u64 = 0xFA;
        let flags: u64 = 0x2; // L=1
        Self((access << 40) | (flags << 52))
    }

    /// Create a 64-bit user data segment (Ring 3).
    pub fn user_data() -> Self {
        // Access: Present(1) | DPL=3 | S=1 | Data(0) | Writable(1) = 0xF2
        let access: u64 = 0xF2;
        Self(access << 40)
    }

    /// Get raw 64-bit value.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Encode as 8 little-endian bytes.
    pub fn encode(self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    /// Get the access byte.
    pub fn access_byte(self) -> u8 {
        ((self.0 >> 40) & 0xFF) as u8
    }

    /// Check if present bit is set.
    pub fn is_present(self) -> bool {
        self.access_byte() & 0x80 != 0
    }

    /// Get DPL (descriptor privilege level).
    pub fn dpl(self) -> u8 {
        (self.access_byte() >> 5) & 0x03
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GDT errors
// ═══════════════════════════════════════════════════════════════════════

/// GDT errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GdtError {
    /// GDT is full.
    #[error("GDT full: max {GDT_MAX_ENTRIES} entries")]
    Full,
    /// GDT has not been loaded.
    #[error("GDT not loaded — call gdt.load() first")]
    NotLoaded,
}

// ═══════════════════════════════════════════════════════════════════════
// Global Descriptor Table
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 Global Descriptor Table.
///
/// Stores segment descriptors and provides encoding for `lgdt`.
#[derive(Debug)]
pub struct GlobalDescriptorTable {
    /// Descriptor entries.
    entries: Vec<SegmentDescriptor>,
    /// Whether the GDT has been loaded (simulated `lgdt`).
    loaded: bool,
}

impl GlobalDescriptorTable {
    /// Create a new GDT with just the null descriptor.
    pub fn new() -> Self {
        Self {
            entries: vec![SegmentDescriptor::NULL],
            loaded: false,
        }
    }

    /// Add a segment descriptor, returning its selector offset.
    pub fn add_entry(&mut self, desc: SegmentDescriptor) -> Result<u16, GdtError> {
        if self.entries.len() >= GDT_MAX_ENTRIES {
            return Err(GdtError::Full);
        }
        let selector = (self.entries.len() * 8) as u16;
        self.entries.push(desc);
        Ok(selector)
    }

    /// Number of entries (including null).
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Simulate loading the GDT (`lgdt` instruction).
    pub fn load(&mut self) {
        self.loaded = true;
    }

    /// Whether the GDT has been loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Encode the full GDT as bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.entries.len() * 8);
        for entry in &self.entries {
            bytes.extend_from_slice(&entry.encode());
        }
        bytes
    }

    /// Get the GDTR value: (base_address, limit).
    ///
    /// Limit = total bytes - 1.
    pub fn gdtr(&self) -> (u64, u16) {
        let limit = (self.entries.len() * 8 - 1) as u16;
        (0, limit)
    }

    /// Create a standard 64-bit GDT with kernel + user segments.
    ///
    /// Layout: [null, kernel_code, kernel_data, user_code, user_data]
    pub fn standard_64bit() -> Self {
        let mut gdt = Self::new();
        gdt.add_entry(SegmentDescriptor::kernel_code()).ok();
        gdt.add_entry(SegmentDescriptor::kernel_data()).ok();
        gdt.add_entry(SegmentDescriptor::user_code()).ok();
        gdt.add_entry(SegmentDescriptor::user_data()).ok();
        gdt
    }
}

impl Default for GlobalDescriptorTable {
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

    #[test]
    fn gdt_new_has_null_entry() {
        let gdt = GlobalDescriptorTable::new();
        assert_eq!(gdt.entry_count(), 1);
        assert!(!gdt.is_loaded());
    }

    #[test]
    fn gdt_add_entries() {
        let mut gdt = GlobalDescriptorTable::new();
        let sel1 = gdt.add_entry(SegmentDescriptor::kernel_code()).unwrap();
        let sel2 = gdt.add_entry(SegmentDescriptor::kernel_data()).unwrap();
        assert_eq!(sel1, 0x08);
        assert_eq!(sel2, 0x10);
        assert_eq!(gdt.entry_count(), 3);
    }

    #[test]
    fn gdt_standard_layout() {
        let gdt = GlobalDescriptorTable::standard_64bit();
        assert_eq!(gdt.entry_count(), 5);

        let bytes = gdt.encode();
        // Null entry should be all zeros
        assert!(bytes[0..8].iter().all(|&b| b == 0));
        // Kernel code entry should be non-zero
        assert!(bytes[8..16].iter().any(|&b| b != 0));
    }

    #[test]
    fn gdt_selectors() {
        assert_eq!(SEL_NULL, 0x00);
        assert_eq!(SEL_KERNEL_CODE, 0x08);
        assert_eq!(SEL_KERNEL_DATA, 0x10);
        assert_eq!(SEL_USER_CODE, 0x18);
        assert_eq!(SEL_USER_DATA, 0x20);
    }

    #[test]
    fn segment_descriptor_kernel_code() {
        let desc = SegmentDescriptor::kernel_code();
        assert!(desc.is_present());
        assert_eq!(desc.dpl(), 0);
        // L bit should be set (64-bit mode)
        let flags_nibble = ((desc.raw() >> 52) & 0x0F) as u8;
        assert!(flags_nibble & 0x02 != 0); // L=1
    }

    #[test]
    fn segment_descriptor_user_code_ring3() {
        let desc = SegmentDescriptor::user_code();
        assert!(desc.is_present());
        assert_eq!(desc.dpl(), 3);
    }

    #[test]
    fn gdt_gdtr_value() {
        let gdt = GlobalDescriptorTable::standard_64bit();
        let (base, limit) = gdt.gdtr();
        assert_eq!(base, 0);
        assert_eq!(limit, 5 * 8 - 1); // 39
    }

    #[test]
    fn gdt_load() {
        let mut gdt = GlobalDescriptorTable::new();
        assert!(!gdt.is_loaded());
        gdt.load();
        assert!(gdt.is_loaded());
    }
}
