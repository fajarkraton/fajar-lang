//! x86_64 Interrupt Descriptor Table (IDT).
//!
//! Provides a simulated 256-entry IDT for OS kernel development.
//! Supports interrupt gates, trap gates, exception handlers, and
//! the `#[interrupt]` attribute for automatic register save/restore.
//!
//! This module models the hardware IDT structure without requiring
//! real x86_64 hardware — enabling testing and validation on any platform.

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Total number of IDT entries (x86_64 standard).
pub const IDT_ENTRIES: usize = 256;

/// Divide-by-zero exception (vector 0).
pub const EXCEPTION_DIVIDE_BY_ZERO: u8 = 0;
/// Debug exception (vector 1).
pub const EXCEPTION_DEBUG: u8 = 1;
/// Non-maskable interrupt (vector 2).
pub const EXCEPTION_NMI: u8 = 2;
/// Breakpoint exception (vector 3).
pub const EXCEPTION_BREAKPOINT: u8 = 3;
/// Overflow exception (vector 4).
pub const EXCEPTION_OVERFLOW: u8 = 4;
/// Bound range exceeded (vector 5).
pub const EXCEPTION_BOUND_RANGE: u8 = 5;
/// Invalid opcode (vector 6).
pub const EXCEPTION_INVALID_OPCODE: u8 = 6;
/// Device not available (vector 7).
pub const EXCEPTION_DEVICE_NOT_AVAILABLE: u8 = 7;
/// Double fault (vector 8).
pub const EXCEPTION_DOUBLE_FAULT: u8 = 8;
/// Invalid TSS (vector 10).
pub const EXCEPTION_INVALID_TSS: u8 = 10;
/// Segment not present (vector 11).
pub const EXCEPTION_SEGMENT_NOT_PRESENT: u8 = 11;
/// Stack-segment fault (vector 12).
pub const EXCEPTION_STACK_SEGMENT_FAULT: u8 = 12;
/// General protection fault (vector 13).
pub const EXCEPTION_GENERAL_PROTECTION: u8 = 13;
/// Page fault (vector 14).
pub const EXCEPTION_PAGE_FAULT: u8 = 14;

// ═══════════════════════════════════════════════════════════════════════
// Gate type
// ═══════════════════════════════════════════════════════════════════════

/// Type of IDT gate descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateType {
    /// Interrupt gate — clears IF (disables further interrupts).
    Interrupt,
    /// Trap gate — does NOT clear IF (allows nested interrupts).
    Trap,
}

impl std::fmt::Display for GateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateType::Interrupt => write!(f, "interrupt"),
            GateType::Trap => write!(f, "trap"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Interrupt stack frame
// ═══════════════════════════════════════════════════════════════════════

/// CPU state pushed by hardware on interrupt entry (x86_64).
///
/// When an interrupt fires, the CPU pushes these values onto the
/// handler's stack before transferring control. The `iretq`
/// instruction pops them to resume execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptStackFrame {
    /// Instruction pointer at interrupt (return address).
    pub rip: u64,
    /// Code segment selector.
    pub cs: u64,
    /// CPU flags register.
    pub rflags: u64,
    /// Stack pointer at interrupt.
    pub rsp: u64,
    /// Stack segment selector.
    pub ss: u64,
}

impl InterruptStackFrame {
    /// Create a new stack frame with the given values.
    pub fn new(rip: u64, cs: u64, rflags: u64, rsp: u64, ss: u64) -> Self {
        Self {
            rip,
            cs,
            rflags,
            rsp,
            ss,
        }
    }

    /// Create a stack frame for kernel-mode code (CS=0x08, SS=0x10).
    pub fn kernel(rip: u64, rsp: u64, rflags: u64) -> Self {
        Self {
            rip,
            cs: 0x08, // Kernel code segment
            rflags,
            rsp,
            ss: 0x10, // Kernel data segment
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IDT entry
// ═══════════════════════════════════════════════════════════════════════

/// A single IDT gate descriptor.
///
/// Models the x86_64 16-byte IDT entry containing the handler address,
/// segment selector, gate type, and privilege level.
#[derive(Debug, Clone)]
pub struct IdtEntry {
    /// Handler function name (resolved at link time in real hardware).
    pub handler_name: String,
    /// Handler address (simulated — offset in code).
    pub handler_addr: u64,
    /// Code segment selector (typically 0x08 for kernel).
    pub segment_selector: u16,
    /// Gate type (interrupt or trap).
    pub gate_type: GateType,
    /// Descriptor privilege level (0 = kernel, 3 = user).
    pub dpl: u8,
    /// Whether this entry is present (valid).
    pub present: bool,
    /// Interrupt Stack Table index (0 = no IST, 1-7 = IST entry).
    pub ist_index: u8,
}

impl IdtEntry {
    /// Create a new IDT entry with default kernel settings.
    fn new(handler_name: String, handler_addr: u64, gate_type: GateType) -> Self {
        Self {
            handler_name,
            handler_addr,
            segment_selector: 0x08, // Kernel code segment
            gate_type,
            dpl: 0, // Ring 0
            present: true,
            ist_index: 0, // No IST
        }
    }

    /// Set the IST index (1-7 for dedicated stacks, e.g., double fault).
    pub fn with_ist(mut self, ist: u8) -> Self {
        self.ist_index = ist.min(7);
        self
    }

    /// Encode as raw x86_64 IDT descriptor bytes (16 bytes).
    ///
    /// Layout:
    /// ```text
    /// Bytes 0-1:  offset[15:0]
    /// Bytes 2-3:  segment selector
    /// Byte  4:    IST[2:0] (bits 0-2), reserved (bits 3-7)
    /// Byte  5:    type_attr (gate type + DPL + present)
    /// Bytes 6-7:  offset[31:16]
    /// Bytes 8-11: offset[63:32]
    /// Bytes 12-15: reserved (must be zero)
    /// ```
    pub fn encode(&self) -> [u8; 16] {
        let addr = self.handler_addr;
        let offset_lo = (addr & 0xFFFF) as u16;
        let offset_mid = ((addr >> 16) & 0xFFFF) as u16;
        let offset_hi = ((addr >> 32) & 0xFFFF_FFFF) as u32;

        let gate_bits: u8 = match self.gate_type {
            GateType::Interrupt => 0x0E, // 64-bit interrupt gate
            GateType::Trap => 0x0F,      // 64-bit trap gate
        };
        let type_attr = gate_bits | ((self.dpl & 0x03) << 5) | if self.present { 0x80 } else { 0 };

        let mut bytes = [0u8; 16];
        bytes[0..2].copy_from_slice(&offset_lo.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.segment_selector.to_le_bytes());
        bytes[4] = self.ist_index & 0x07;
        bytes[5] = type_attr;
        bytes[6..8].copy_from_slice(&offset_mid.to_le_bytes());
        bytes[8..12].copy_from_slice(&offset_hi.to_le_bytes());
        // bytes[12..16] = reserved, already zero
        bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════
// IDT errors
// ═══════════════════════════════════════════════════════════════════════

/// Errors from IDT operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum IdtError {
    /// Vector number already has a handler registered.
    #[error("IDT vector {vector} already has a handler: {existing}")]
    AlreadyRegistered { vector: u8, existing: String },

    /// No handler registered for the given vector.
    #[error("no handler for IDT vector {vector}")]
    NoHandler { vector: u8 },

    /// IDT has not been loaded (no `lidt` performed).
    #[error("IDT not loaded — call idt.load() first")]
    NotLoaded,

    /// Invalid IST index (must be 0-7).
    #[error("invalid IST index {ist}: must be 0-7")]
    InvalidIst { ist: u8 },

    /// Exception handler error with context.
    #[error("exception at vector {vector} ({name}): {message}")]
    ExceptionFault {
        vector: u8,
        name: String,
        message: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Interrupt Descriptor Table
// ═══════════════════════════════════════════════════════════════════════

/// x86_64 Interrupt Descriptor Table with 256 entries.
///
/// Provides handler registration, dispatch simulation, and raw encoding
/// for the `lidt` instruction. Exception vectors 0-31 are reserved for
/// CPU exceptions; vectors 32-255 are available for hardware IRQs and
/// software interrupts.
#[derive(Debug)]
pub struct InterruptDescriptorTable {
    /// Sparse entry storage — only populated vectors have entries.
    entries: HashMap<u8, IdtEntry>,
    /// Whether the IDT has been "loaded" (simulated `lidt`).
    loaded: bool,
    /// Dispatch log for testing (vector numbers dispatched).
    dispatch_log: Vec<u8>,
    /// Active handler stack for nested interrupt tracking.
    active_stack: Vec<u8>,
}

impl InterruptDescriptorTable {
    /// Create a new empty IDT.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            loaded: false,
            dispatch_log: Vec::new(),
            active_stack: Vec::new(),
        }
    }

    /// Register a handler for the given vector number.
    ///
    /// Uses interrupt gate by default (clears IF on entry).
    pub fn set_handler(
        &mut self,
        vector: u8,
        handler_name: &str,
        handler_addr: u64,
    ) -> Result<(), IdtError> {
        if let Some(existing) = self.entries.get(&vector) {
            return Err(IdtError::AlreadyRegistered {
                vector,
                existing: existing.handler_name.clone(),
            });
        }
        self.entries.insert(
            vector,
            IdtEntry::new(handler_name.to_string(), handler_addr, GateType::Interrupt),
        );
        Ok(())
    }

    /// Register a trap gate handler (does NOT clear IF).
    pub fn set_trap_handler(
        &mut self,
        vector: u8,
        handler_name: &str,
        handler_addr: u64,
    ) -> Result<(), IdtError> {
        if let Some(existing) = self.entries.get(&vector) {
            return Err(IdtError::AlreadyRegistered {
                vector,
                existing: existing.handler_name.clone(),
            });
        }
        self.entries.insert(
            vector,
            IdtEntry::new(handler_name.to_string(), handler_addr, GateType::Trap),
        );
        Ok(())
    }

    /// Register a handler with a dedicated IST stack (for double fault, etc.).
    pub fn set_handler_with_ist(
        &mut self,
        vector: u8,
        handler_name: &str,
        handler_addr: u64,
        ist_index: u8,
    ) -> Result<(), IdtError> {
        if ist_index > 7 {
            return Err(IdtError::InvalidIst { ist: ist_index });
        }
        if let Some(existing) = self.entries.get(&vector) {
            return Err(IdtError::AlreadyRegistered {
                vector,
                existing: existing.handler_name.clone(),
            });
        }
        let entry = IdtEntry::new(handler_name.to_string(), handler_addr, GateType::Interrupt)
            .with_ist(ist_index);
        self.entries.insert(vector, entry);
        Ok(())
    }

    /// Remove a handler from the given vector.
    pub fn remove_handler(&mut self, vector: u8) -> Result<(), IdtError> {
        self.entries
            .remove(&vector)
            .map(|_| ())
            .ok_or(IdtError::NoHandler { vector })
    }

    /// Check if a vector has a handler registered.
    pub fn has_handler(&self, vector: u8) -> bool {
        self.entries.contains_key(&vector)
    }

    /// Get handler info for a vector.
    pub fn get_handler(&self, vector: u8) -> Option<&IdtEntry> {
        self.entries.get(&vector)
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.entries.len()
    }

    /// Simulate loading the IDT (`lidt` instruction).
    ///
    /// In real hardware this writes the IDTR register with the IDT
    /// base address and limit. Here it marks the table as active.
    pub fn load(&mut self) {
        self.loaded = true;
    }

    /// Whether the IDT has been loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Simulate dispatching an interrupt to its handler.
    ///
    /// Returns the handler name if found, or an error.
    pub fn dispatch(&mut self, vector: u8) -> Result<&str, IdtError> {
        if !self.loaded {
            return Err(IdtError::NotLoaded);
        }
        let entry = self
            .entries
            .get(&vector)
            .ok_or(IdtError::NoHandler { vector })?;
        self.dispatch_log.push(vector);
        self.active_stack.push(vector);
        Ok(&entry.handler_name)
    }

    /// Signal that the current handler has completed (simulated `iretq`).
    pub fn handler_return(&mut self) {
        self.active_stack.pop();
    }

    /// Get the currently active handler vector (if any).
    pub fn active_vector(&self) -> Option<u8> {
        self.active_stack.last().copied()
    }

    /// Whether a handler is currently executing.
    pub fn is_handling(&self) -> bool {
        !self.active_stack.is_empty()
    }

    /// Nesting depth of active handlers.
    pub fn nesting_depth(&self) -> usize {
        self.active_stack.len()
    }

    /// Get the dispatch log (for testing).
    pub fn dispatch_log(&self) -> &[u8] {
        &self.dispatch_log
    }

    /// Clear the dispatch log.
    pub fn clear_log(&mut self) {
        self.dispatch_log.clear();
    }

    /// Encode the full IDT as a byte array (256 * 16 = 4096 bytes).
    ///
    /// Empty entries are zero-filled (present=false).
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; IDT_ENTRIES * 16];
        for (&vector, entry) in &self.entries {
            let offset = (vector as usize) * 16;
            let encoded = entry.encode();
            bytes[offset..offset + 16].copy_from_slice(&encoded);
        }
        bytes
    }

    /// Get the IDTR value: (base_address, limit).
    ///
    /// In simulation, base is 0 (the IDT encode() starts at offset 0).
    /// Limit is `(256 * 16) - 1 = 4095`.
    pub fn idtr(&self) -> (u64, u16) {
        (0, (IDT_ENTRIES * 16 - 1) as u16)
    }

    /// Register standard x86_64 exception handlers with default names.
    ///
    /// Sets up vectors 0 (divide-by-zero), 8 (double fault),
    /// and 14 (page fault) with trap gates. Double fault uses IST 1.
    pub fn register_default_exceptions(&mut self) -> Result<(), IdtError> {
        // Divide-by-zero: trap gate (allows debugging)
        self.set_trap_handler(EXCEPTION_DIVIDE_BY_ZERO, "divide_by_zero_handler", 0)?;
        // Double fault: interrupt gate + IST 1 (dedicated stack)
        self.set_handler_with_ist(EXCEPTION_DOUBLE_FAULT, "double_fault_handler", 0, 1)?;
        // Page fault: interrupt gate
        self.set_handler(EXCEPTION_PAGE_FAULT, "page_fault_handler", 0)?;
        Ok(())
    }
}

impl Default for InterruptDescriptorTable {
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
    fn idt_new_is_empty_and_not_loaded() {
        let idt = InterruptDescriptorTable::new();
        assert_eq!(idt.handler_count(), 0);
        assert!(!idt.is_loaded());
        assert!(!idt.is_handling());
    }

    #[test]
    fn idt_set_handler_and_lookup() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer_handler", 0x1000).unwrap();

        assert!(idt.has_handler(0x20));
        assert!(!idt.has_handler(0x21));
        assert_eq!(idt.handler_count(), 1);

        let entry = idt.get_handler(0x20).unwrap();
        assert_eq!(entry.handler_name, "timer_handler");
        assert_eq!(entry.handler_addr, 0x1000);
        assert_eq!(entry.gate_type, GateType::Interrupt);
        assert_eq!(entry.dpl, 0);
        assert!(entry.present);
    }

    #[test]
    fn idt_duplicate_handler_rejected() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer", 0x1000).unwrap();

        let err = idt.set_handler(0x20, "timer2", 0x2000).unwrap_err();
        assert_eq!(
            err,
            IdtError::AlreadyRegistered {
                vector: 0x20,
                existing: "timer".into(),
            }
        );
    }

    #[test]
    fn idt_remove_handler() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer", 0x1000).unwrap();
        assert!(idt.has_handler(0x20));

        idt.remove_handler(0x20).unwrap();
        assert!(!idt.has_handler(0x20));

        // Removing again fails
        let err = idt.remove_handler(0x20).unwrap_err();
        assert_eq!(err, IdtError::NoHandler { vector: 0x20 });
    }

    #[test]
    fn idt_dispatch_requires_load() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer", 0x1000).unwrap();

        let err = idt.dispatch(0x20).unwrap_err();
        assert_eq!(err, IdtError::NotLoaded);

        idt.load();
        let name = idt.dispatch(0x20).unwrap();
        assert_eq!(name, "timer");
    }

    #[test]
    fn idt_dispatch_no_handler() {
        let mut idt = InterruptDescriptorTable::new();
        idt.load();

        let err = idt.dispatch(0xFF).unwrap_err();
        assert_eq!(err, IdtError::NoHandler { vector: 0xFF });
    }

    #[test]
    fn idt_dispatch_log_and_nesting() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer", 0x1000).unwrap();
        idt.set_handler(0x21, "keyboard", 0x2000).unwrap();
        idt.load();

        // Dispatch timer
        idt.dispatch(0x20).unwrap();
        assert_eq!(idt.active_vector(), Some(0x20));
        assert_eq!(idt.nesting_depth(), 1);

        // Nested keyboard interrupt
        idt.dispatch(0x21).unwrap();
        assert_eq!(idt.active_vector(), Some(0x21));
        assert_eq!(idt.nesting_depth(), 2);

        // Keyboard returns
        idt.handler_return();
        assert_eq!(idt.active_vector(), Some(0x20));
        assert_eq!(idt.nesting_depth(), 1);

        // Timer returns
        idt.handler_return();
        assert!(!idt.is_handling());

        assert_eq!(idt.dispatch_log(), &[0x20, 0x21]);
    }

    #[test]
    fn idt_trap_gate() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_trap_handler(EXCEPTION_BREAKPOINT, "breakpoint", 0x3000)
            .unwrap();

        let entry = idt.get_handler(EXCEPTION_BREAKPOINT).unwrap();
        assert_eq!(entry.gate_type, GateType::Trap);
    }

    #[test]
    fn idt_handler_with_ist() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler_with_ist(EXCEPTION_DOUBLE_FAULT, "double_fault", 0x4000, 1)
            .unwrap();

        let entry = idt.get_handler(EXCEPTION_DOUBLE_FAULT).unwrap();
        assert_eq!(entry.ist_index, 1);
        assert_eq!(entry.gate_type, GateType::Interrupt);
    }

    #[test]
    fn idt_invalid_ist_rejected() {
        let mut idt = InterruptDescriptorTable::new();
        let err = idt.set_handler_with_ist(0x08, "df", 0, 8).unwrap_err();
        assert_eq!(err, IdtError::InvalidIst { ist: 8 });
    }

    #[test]
    fn idt_entry_encode_roundtrip() {
        let entry = IdtEntry::new("test".into(), 0xDEAD_BEEF_CAFE_1234, GateType::Interrupt);
        let bytes = entry.encode();

        // offset[15:0] at bytes 0-1
        let lo = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(lo, 0x1234);

        // segment selector at bytes 2-3
        let sel = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(sel, 0x08);

        // IST at byte 4
        assert_eq!(bytes[4], 0);

        // type_attr at byte 5: present(0x80) | DPL=0 | interrupt(0x0E)
        assert_eq!(bytes[5], 0x8E);

        // offset[31:16] at bytes 6-7
        let mid = u16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(mid, 0xCAFE);

        // offset[63:32] at bytes 8-11
        let hi = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        assert_eq!(hi, 0xDEAD_BEEF);
    }

    #[test]
    fn idt_full_encode_is_4096_bytes() {
        let idt = InterruptDescriptorTable::new();
        let bytes = idt.encode();
        assert_eq!(bytes.len(), 4096);
        // Empty IDT: all zeros
        assert!(bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn idt_encode_with_handler() {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(0x20, "timer", 0x1000).unwrap();

        let bytes = idt.encode();
        // Vector 0x20 = entry at offset 0x20 * 16 = 512
        let entry_bytes = &bytes[512..528];
        // Should be non-zero (has a handler)
        assert!(entry_bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn idt_idtr_value() {
        let idt = InterruptDescriptorTable::new();
        let (base, limit) = idt.idtr();
        assert_eq!(base, 0);
        assert_eq!(limit, 4095); // 256 * 16 - 1
    }

    #[test]
    fn idt_register_default_exceptions() {
        let mut idt = InterruptDescriptorTable::new();
        idt.register_default_exceptions().unwrap();

        assert_eq!(idt.handler_count(), 3);
        assert!(idt.has_handler(EXCEPTION_DIVIDE_BY_ZERO));
        assert!(idt.has_handler(EXCEPTION_DOUBLE_FAULT));
        assert!(idt.has_handler(EXCEPTION_PAGE_FAULT));

        // Divide-by-zero is a trap gate
        let div = idt.get_handler(EXCEPTION_DIVIDE_BY_ZERO).unwrap();
        assert_eq!(div.gate_type, GateType::Trap);

        // Double fault uses IST 1
        let df = idt.get_handler(EXCEPTION_DOUBLE_FAULT).unwrap();
        assert_eq!(df.ist_index, 1);
    }

    #[test]
    fn interrupt_stack_frame_kernel() {
        let frame = InterruptStackFrame::kernel(0x1000, 0x7000, 0x202);
        assert_eq!(frame.rip, 0x1000);
        assert_eq!(frame.cs, 0x08);
        assert_eq!(frame.rflags, 0x202);
        assert_eq!(frame.rsp, 0x7000);
        assert_eq!(frame.ss, 0x10);
    }

    #[test]
    fn exception_constants_correct() {
        assert_eq!(EXCEPTION_DIVIDE_BY_ZERO, 0);
        assert_eq!(EXCEPTION_DOUBLE_FAULT, 8);
        assert_eq!(EXCEPTION_PAGE_FAULT, 14);
        assert_eq!(EXCEPTION_GENERAL_PROTECTION, 13);
    }
}
