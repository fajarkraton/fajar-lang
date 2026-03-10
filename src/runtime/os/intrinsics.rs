//! Platform-specific intrinsics for bare metal programming.
//!
//! Provides simulated versions of CPU instructions that would normally
//! be emitted as inline assembly. These enable testing kernel code
//! without actual hardware.
//!
//! Architecture coverage:
//! - x86_64: cli, sti, hlt, inb, outb, invlpg, rdmsr, wrmsr
//! - AArch64: wfi, dsb, isb, mrs, msr, sev, wfe
//! - RISC-V: wfi, csrr, csrw, fence, ecall, ebreak

// ═══════════════════════════════════════════════════════════════════════
// x86_64 intrinsics
// ═══════════════════════════════════════════════════════════════════════

/// Simulated x86_64 CPU state for intrinsics.
#[derive(Debug)]
pub struct X86State {
    /// Interrupt flag (IF in RFLAGS).
    pub interrupts_enabled: bool,
    /// Whether HLT was called (waiting for interrupt).
    pub halted: bool,
    /// Port I/O values.
    ports: std::collections::HashMap<u16, u8>,
    /// Model-specific registers (MSR).
    msrs: std::collections::HashMap<u32, u64>,
}

impl X86State {
    /// Create a new x86_64 CPU state.
    pub fn new() -> Self {
        Self {
            interrupts_enabled: true,
            halted: false,
            ports: std::collections::HashMap::new(),
            msrs: std::collections::HashMap::new(),
        }
    }

    /// `cli` — Clear interrupt flag (disable interrupts).
    pub fn cli(&mut self) {
        self.interrupts_enabled = false;
    }

    /// `sti` — Set interrupt flag (enable interrupts).
    pub fn sti(&mut self) {
        self.interrupts_enabled = true;
    }

    /// `hlt` — Halt CPU until next interrupt.
    pub fn hlt(&mut self) {
        self.halted = true;
    }

    /// `inb` — Read byte from I/O port.
    pub fn inb(&self, port: u16) -> u8 {
        self.ports.get(&port).copied().unwrap_or(0xFF)
    }

    /// `outb` — Write byte to I/O port.
    pub fn outb(&mut self, port: u16, value: u8) {
        self.ports.insert(port, value);
    }

    /// `rdmsr` — Read model-specific register.
    pub fn rdmsr(&self, msr: u32) -> u64 {
        self.msrs.get(&msr).copied().unwrap_or(0)
    }

    /// `wrmsr` — Write model-specific register.
    pub fn wrmsr(&mut self, msr: u32, value: u64) {
        self.msrs.insert(msr, value);
    }

    /// Simulate an interrupt arriving (clears halted state).
    pub fn interrupt(&mut self) {
        self.halted = false;
    }
}

impl Default for X86State {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AArch64 intrinsics
// ═══════════════════════════════════════════════════════════════════════

/// Simulated AArch64 CPU state for intrinsics.
#[derive(Debug)]
pub struct Aarch64State {
    /// Whether WFI/WFE is active (waiting).
    pub waiting: bool,
    /// System registers (index → value).
    system_regs: std::collections::HashMap<u32, u64>,
    /// Event flag (set by SEV).
    pub event_flag: bool,
    /// Barrier count (DSB/ISB calls).
    pub barrier_count: u64,
}

impl Aarch64State {
    /// Create a new AArch64 CPU state.
    pub fn new() -> Self {
        Self {
            waiting: false,
            system_regs: std::collections::HashMap::new(),
            event_flag: false,
            barrier_count: 0,
        }
    }

    /// `wfi` — Wait for interrupt.
    pub fn wfi(&mut self) {
        self.waiting = true;
    }

    /// `wfe` — Wait for event.
    pub fn wfe(&mut self) {
        if self.event_flag {
            self.event_flag = false;
        } else {
            self.waiting = true;
        }
    }

    /// `sev` — Send event (wakes WFE on all cores).
    pub fn sev(&mut self) {
        self.event_flag = true;
    }

    /// `dsb` — Data synchronization barrier.
    pub fn dsb(&mut self) {
        self.barrier_count += 1;
    }

    /// `isb` — Instruction synchronization barrier.
    pub fn isb(&mut self) {
        self.barrier_count += 1;
    }

    /// `mrs` — Move system register to general register.
    pub fn mrs(&self, reg: u32) -> u64 {
        self.system_regs.get(&reg).copied().unwrap_or(0)
    }

    /// `msr` — Move general register to system register.
    pub fn msr(&mut self, reg: u32, value: u64) {
        self.system_regs.insert(reg, value);
    }

    /// Simulate an interrupt arriving.
    pub fn interrupt(&mut self) {
        self.waiting = false;
    }
}

impl Default for Aarch64State {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RISC-V intrinsics
// ═══════════════════════════════════════════════════════════════════════

/// Simulated RISC-V CPU state for intrinsics.
#[derive(Debug)]
pub struct RiscvState {
    /// Whether WFI is active (waiting).
    pub waiting: bool,
    /// Control and Status Registers.
    csrs: std::collections::HashMap<u16, u64>,
    /// Fence count.
    pub fence_count: u64,
}

/// Standard RISC-V CSR addresses.
pub mod csr {
    /// Machine status register.
    pub const MSTATUS: u16 = 0x300;
    /// Machine interrupt enable.
    pub const MIE: u16 = 0x304;
    /// Machine trap vector.
    pub const MTVEC: u16 = 0x305;
    /// Machine scratch register.
    pub const MSCRATCH: u16 = 0x340;
    /// Machine exception PC.
    pub const MEPC: u16 = 0x341;
    /// Machine cause register.
    pub const MCAUSE: u16 = 0x342;
    /// Machine trap value.
    pub const MTVAL: u16 = 0x343;
    /// Machine interrupt pending.
    pub const MIP: u16 = 0x344;
    /// Cycle counter (read-only).
    pub const CYCLE: u16 = 0xC00;
}

impl RiscvState {
    /// Create a new RISC-V CPU state.
    pub fn new() -> Self {
        Self {
            waiting: false,
            csrs: std::collections::HashMap::new(),
            fence_count: 0,
        }
    }

    /// `wfi` — Wait for interrupt.
    pub fn wfi(&mut self) {
        self.waiting = true;
    }

    /// `csrr` — Read CSR.
    pub fn csrr(&self, csr_addr: u16) -> u64 {
        self.csrs.get(&csr_addr).copied().unwrap_or(0)
    }

    /// `csrw` — Write CSR.
    pub fn csrw(&mut self, csr_addr: u16, value: u64) {
        self.csrs.insert(csr_addr, value);
    }

    /// `csrs` — Set bits in CSR.
    pub fn csrs(&mut self, csr_addr: u16, bits: u64) {
        let current = self.csrr(csr_addr);
        self.csrw(csr_addr, current | bits);
    }

    /// `csrc` — Clear bits in CSR.
    pub fn csrc(&mut self, csr_addr: u16, bits: u64) {
        let current = self.csrr(csr_addr);
        self.csrw(csr_addr, current & !bits);
    }

    /// `fence` — Memory fence (ordering barrier).
    pub fn fence(&mut self) {
        self.fence_count += 1;
    }

    /// `ecall` — Environment call (trap to higher privilege).
    pub fn ecall(&mut self) {
        // In simulation, just record the trap cause
        self.csrw(csr::MCAUSE, 11); // Environment call from M-mode
    }

    /// `ebreak` — Breakpoint.
    pub fn ebreak(&mut self) {
        self.csrw(csr::MCAUSE, 3); // Breakpoint
    }

    /// Simulate an interrupt arriving.
    pub fn interrupt(&mut self) {
        self.waiting = false;
    }
}

impl Default for RiscvState {
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

    // x86_64 tests
    #[test]
    fn x86_cli_sti() {
        let mut cpu = X86State::new();
        assert!(cpu.interrupts_enabled);

        cpu.cli();
        assert!(!cpu.interrupts_enabled);

        cpu.sti();
        assert!(cpu.interrupts_enabled);
    }

    #[test]
    fn x86_hlt_and_interrupt() {
        let mut cpu = X86State::new();
        cpu.hlt();
        assert!(cpu.halted);

        cpu.interrupt();
        assert!(!cpu.halted);
    }

    #[test]
    fn x86_port_io() {
        let mut cpu = X86State::new();
        cpu.outb(0x3F8, b'A');
        assert_eq!(cpu.inb(0x3F8), b'A');
        assert_eq!(cpu.inb(0x99), 0xFF); // unset port
    }

    #[test]
    fn x86_msr() {
        let mut cpu = X86State::new();
        cpu.wrmsr(0x1B, 0xFEE0_0000); // APIC base
        assert_eq!(cpu.rdmsr(0x1B), 0xFEE0_0000);
    }

    // AArch64 tests
    #[test]
    fn aarch64_wfi() {
        let mut cpu = Aarch64State::new();
        cpu.wfi();
        assert!(cpu.waiting);
        cpu.interrupt();
        assert!(!cpu.waiting);
    }

    #[test]
    fn aarch64_dsb_isb() {
        let mut cpu = Aarch64State::new();
        cpu.dsb();
        cpu.isb();
        assert_eq!(cpu.barrier_count, 2);
    }

    #[test]
    fn aarch64_mrs_msr() {
        let mut cpu = Aarch64State::new();
        cpu.msr(0xC000, 0x12345678); // SCTLR_EL1 (example)
        assert_eq!(cpu.mrs(0xC000), 0x12345678);
    }

    #[test]
    fn aarch64_sev_wfe() {
        let mut cpu = Aarch64State::new();
        cpu.sev();
        assert!(cpu.event_flag);
        cpu.wfe(); // Should consume event, not sleep
        assert!(!cpu.event_flag);
        assert!(!cpu.waiting);
    }

    // RISC-V tests
    #[test]
    fn riscv_wfi() {
        let mut cpu = RiscvState::new();
        cpu.wfi();
        assert!(cpu.waiting);
        cpu.interrupt();
        assert!(!cpu.waiting);
    }

    #[test]
    fn riscv_csr() {
        let mut cpu = RiscvState::new();
        cpu.csrw(csr::MTVEC, 0x8000_0000);
        assert_eq!(cpu.csrr(csr::MTVEC), 0x8000_0000);

        // Set bits
        cpu.csrs(csr::MSTATUS, 0x08); // MIE bit
        assert_eq!(cpu.csrr(csr::MSTATUS), 0x08);

        // Clear bits
        cpu.csrc(csr::MSTATUS, 0x08);
        assert_eq!(cpu.csrr(csr::MSTATUS), 0x00);
    }

    #[test]
    fn riscv_fence() {
        let mut cpu = RiscvState::new();
        cpu.fence();
        cpu.fence();
        assert_eq!(cpu.fence_count, 2);
    }

    #[test]
    fn riscv_ecall_ebreak() {
        let mut cpu = RiscvState::new();
        cpu.ecall();
        assert_eq!(cpu.csrr(csr::MCAUSE), 11);

        cpu.ebreak();
        assert_eq!(cpu.csrr(csr::MCAUSE), 3);
    }
}
