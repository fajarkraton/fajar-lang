//! Board Support Package (BSP) framework for Fajar Lang.
//!
//! Provides traits and structures for targeting specific hardware boards
//! (STM32, ESP32, RP2040, etc.) with proper memory maps, peripherals,
//! linker scripts, and startup code.
//!
//! # Architecture
//!
//! ```text
//! Board trait
//!     ├── name() → "STM32F407VG"
//!     ├── arch() → BspArch::Thumbv7em
//!     ├── memory_regions() → [Flash, SRAM1, SRAM2, CCM]
//!     ├── peripherals() → [GPIOA, USART1, SPI1, ...]
//!     └── vector_table_size() → 98
//!         │
//!         ▼
//!     LinkerScript / StartupCode / VectorTable
//! ```

pub mod dragonwing;
pub mod esp32;
pub mod hal;
pub mod jetson_thor;
pub mod rp2040;
pub mod stm32f407;
pub mod stm32h5;
pub mod ventuno_q;

use std::fmt;

/// MCU architecture for BSP targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BspArch {
    /// ARM Cortex-M4F (STM32F4, STM32L4, etc.).
    Thumbv7em,
    /// ARM Cortex-M0/M0+ (STM32F0, STM32L0, RP2040).
    Thumbv6m,
    /// Xtensa LX6/LX7 (ESP32, ESP32-S2/S3).
    Xtensa,
    /// RISC-V 32-bit (ESP32-C3, GD32VF103).
    Riscv32,
    /// ARM Cortex-M33 (STM32H5, STM32L5, STM32U5).
    ArmCortexM33,
    /// ARM64 Linux (Qualcomm Dragonwing, Raspberry Pi 4/5, etc.).
    Aarch64Linux,
}

impl fmt::Display for BspArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BspArch::Thumbv7em => write!(f, "thumbv7em-none-eabihf"),
            BspArch::Thumbv6m => write!(f, "thumbv6m-none-eabi"),
            BspArch::Xtensa => write!(f, "xtensa-esp32-none-elf"),
            BspArch::Riscv32 => write!(f, "riscv32imc-unknown-none-elf"),
            BspArch::ArmCortexM33 => write!(f, "thumbv8m.main-none-eabihf"),
            BspArch::Aarch64Linux => write!(f, "aarch64-unknown-linux-gnu"),
        }
    }
}

/// Memory region attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAttr {
    /// Read + Execute (Flash/ROM).
    Rx,
    /// Read + Write (RAM).
    Rw,
    /// Read + Write + Execute (RAM, if executable).
    Rwx,
    /// Read only (const data, OTP).
    Ro,
}

impl fmt::Display for MemoryAttr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryAttr::Rx => write!(f, "rx"),
            MemoryAttr::Rw => write!(f, "rw"),
            MemoryAttr::Rwx => write!(f, "rwx"),
            MemoryAttr::Ro => write!(f, "r"),
        }
    }
}

/// A memory region in the MCU address map.
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Region name (e.g., "FLASH", "SRAM1").
    pub name: String,
    /// Start address.
    pub origin: u32,
    /// Size in bytes.
    pub length: u32,
    /// Access attributes.
    pub attr: MemoryAttr,
}

impl MemoryRegion {
    /// Creates a new memory region.
    pub fn new(name: &str, origin: u32, length: u32, attr: MemoryAttr) -> Self {
        Self {
            name: name.to_string(),
            origin,
            length,
            attr,
        }
    }

    /// Returns the end address (exclusive).
    pub fn end_address(&self) -> u32 {
        self.origin + self.length
    }

    /// Formats as a linker script MEMORY entry.
    pub fn to_linker_entry(&self) -> String {
        format!(
            "  {} ({}) : ORIGIN = {:#010X}, LENGTH = {}K",
            self.name,
            self.attr,
            self.origin,
            self.length / 1024
        )
    }
}

/// A peripheral register.
#[derive(Debug, Clone)]
pub struct Register {
    /// Register name (e.g., "MODER", "ODR").
    pub name: String,
    /// Offset from peripheral base address.
    pub offset: u32,
    /// Size in bytes (typically 4).
    pub size: u8,
}

/// A hardware peripheral (GPIO port, UART, SPI, etc.).
#[derive(Debug, Clone)]
pub struct Peripheral {
    /// Peripheral name (e.g., "GPIOA", "USART1").
    pub name: String,
    /// Base address in the memory map.
    pub base_address: u32,
    /// List of registers.
    pub registers: Vec<Register>,
}

impl Peripheral {
    /// Creates a new peripheral.
    pub fn new(name: &str, base_address: u32) -> Self {
        Self {
            name: name.to_string(),
            base_address,
            registers: Vec::new(),
        }
    }

    /// Adds a register to this peripheral.
    pub fn add_register(&mut self, name: &str, offset: u32, size: u8) {
        self.registers.push(Register {
            name: name.to_string(),
            offset,
            size,
        });
    }

    /// Returns the address of a named register.
    pub fn register_address(&self, name: &str) -> Option<u32> {
        self.registers
            .iter()
            .find(|r| r.name == name)
            .map(|r| self.base_address + r.offset)
    }
}

/// Board Support Package trait.
///
/// Implement this for each target board to provide the memory map,
/// peripherals, and configuration needed for compilation.
pub trait Board {
    /// Board name (e.g., "STM32F407VG Discovery").
    fn name(&self) -> &str;

    /// Target architecture.
    fn arch(&self) -> BspArch;

    /// Memory regions (Flash, SRAM, CCM, etc.).
    fn memory_regions(&self) -> Vec<MemoryRegion>;

    /// Peripheral definitions.
    fn peripherals(&self) -> Vec<Peripheral>;

    /// Number of entries in the vector table.
    fn vector_table_size(&self) -> usize;

    /// CPU clock frequency in Hz.
    fn cpu_frequency(&self) -> u32;

    /// Generates a linker script for this board.
    fn generate_linker_script(&self) -> String {
        let mut script = String::new();
        script.push_str("/* Auto-generated linker script for ");
        script.push_str(self.name());
        script.push_str(" */\n\n");

        // MEMORY section
        script.push_str("MEMORY\n{\n");
        for region in &self.memory_regions() {
            script.push_str(&region.to_linker_entry());
            script.push('\n');
        }
        script.push_str("}\n\n");

        // Entry point
        script.push_str("ENTRY(Reset_Handler)\n\n");

        // SECTIONS
        script.push_str("SECTIONS\n{\n");
        script.push_str("  .isr_vector :\n  {\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    KEEP(*(.isr_vector))\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("  } > FLASH\n\n");

        script.push_str("  .text :\n  {\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    *(.text .text.*)\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    _etext = .;\n");
        script.push_str("  } > FLASH\n\n");

        script.push_str("  .rodata :\n  {\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    *(.rodata .rodata.*)\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("  } > FLASH\n\n");

        // Find the first RW region for data/bss
        let ram_region = self
            .memory_regions()
            .iter()
            .find(|r| r.attr == MemoryAttr::Rw || r.attr == MemoryAttr::Rwx)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "RAM".to_string());

        script.push_str("  _sidata = LOADADDR(.data);\n\n");
        script.push_str("  .data :\n  {\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    _sdata = .;\n");
        script.push_str("    *(.data .data.*)\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    _edata = .;\n");
        script.push_str(&format!("  }} > {ram_region} AT> FLASH\n\n"));

        script.push_str("  .bss :\n  {\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    _sbss = .;\n");
        script.push_str("    *(.bss .bss.* COMMON)\n");
        script.push_str("    . = ALIGN(4);\n");
        script.push_str("    _ebss = .;\n");
        script.push_str(&format!("  }} > {ram_region}\n\n"));

        script.push_str("  _estack = ORIGIN(");
        script.push_str(&ram_region);
        script.push_str(") + LENGTH(");
        script.push_str(&ram_region);
        script.push_str(");\n");

        script.push_str("}\n");
        script
    }

    /// Generates startup assembly code (Reset_Handler).
    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated startup code for ");
        code.push_str(self.name());
        code.push_str(" */\n\n");

        code.push_str(".syntax unified\n");
        code.push_str(".cpu cortex-m4\n");
        code.push_str(".fpu fpv4-sp-d16\n");
        code.push_str(".thumb\n\n");

        code.push_str(".global Reset_Handler\n");
        code.push_str(".type Reset_Handler, %function\n");
        code.push_str("Reset_Handler:\n");

        // Copy .data from Flash to RAM
        code.push_str("  ldr r0, =_sdata\n");
        code.push_str("  ldr r1, =_edata\n");
        code.push_str("  ldr r2, =_sidata\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b CopyDataLoop\n\n");
        code.push_str("CopyDataInit:\n");
        code.push_str("  ldr r4, [r2, r3]\n");
        code.push_str("  str r4, [r0, r3]\n");
        code.push_str("  adds r3, r3, #4\n\n");
        code.push_str("CopyDataLoop:\n");
        code.push_str("  adds r4, r0, r3\n");
        code.push_str("  cmp r4, r1\n");
        code.push_str("  bcc CopyDataInit\n\n");

        // Zero .bss
        code.push_str("  ldr r2, =_sbss\n");
        code.push_str("  ldr r4, =_ebss\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b ZeroBssLoop\n\n");
        code.push_str("ZeroBssInit:\n");
        code.push_str("  str r3, [r2]\n");
        code.push_str("  adds r2, r2, #4\n\n");
        code.push_str("ZeroBssLoop:\n");
        code.push_str("  cmp r2, r4\n");
        code.push_str("  bcc ZeroBssInit\n\n");

        // Enable FPU (Cortex-M4F)
        if self.arch() == BspArch::Thumbv7em {
            code.push_str("  /* Enable FPU */\n");
            code.push_str("  ldr r0, =0xE000ED88\n");
            code.push_str("  ldr r1, [r0]\n");
            code.push_str("  orr r1, r1, #(0xF << 20)\n");
            code.push_str("  str r1, [r0]\n");
            code.push_str("  dsb\n");
            code.push_str("  isb\n\n");
        }

        // Call main
        code.push_str("  bl main\n");
        code.push_str("  b .\n\n");

        // Vector table
        code.push_str(".section .isr_vector, \"a\", %progbits\n");
        code.push_str(".type g_pfnVectors, %object\n");
        code.push_str("g_pfnVectors:\n");
        code.push_str("  .word _estack\n");
        code.push_str("  .word Reset_Handler\n");

        let handler_names = [
            "NMI_Handler",
            "HardFault_Handler",
            "MemManage_Handler",
            "BusFault_Handler",
            "UsageFault_Handler",
        ];
        for name in &handler_names {
            code.push_str(&format!("  .word {name}\n"));
        }
        // Reserved entries + remaining handlers
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word SVC_Handler\n");
        code.push_str("  .word DebugMon_Handler\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word PendSV_Handler\n");
        code.push_str("  .word SysTick_Handler\n");

        code
    }
}

/// Looks up a board by name string.
pub fn board_by_name(name: &str) -> Option<Box<dyn Board>> {
    match name.to_lowercase().as_str() {
        "stm32f407" | "stm32f407vg" | "stm32f4discovery" => {
            Some(Box::new(stm32f407::Stm32f407::new()))
        }
        "esp32" => Some(Box::new(esp32::Esp32::new())),
        "rp2040" | "pico" | "raspberrypi-pico" => Some(Box::new(rp2040::Rp2040::new())),
        "stm32h5" | "stm32h5f5" => Some(Box::new(stm32h5::Stm32H5::new())),
        "dragonwing" | "dragonwing-iq8" => Some(Box::new(dragonwing::DragonwingIQ8::new())),
        "ventuno-q" | "ventuno_q" | "arduino-ventuno-q" => {
            Some(Box::new(ventuno_q::VentunoQ::new()))
        }
        "jetson-thor" | "jetson_thor" | "thor" => Some(Box::new(jetson_thor::JetsonThor::new(
            jetson_thor::ThorVariant::T5000,
        ))),
        "jetson-thor-t4000" | "thor-t4000" => Some(Box::new(jetson_thor::JetsonThor::new(
            jetson_thor::ThorVariant::T4000,
        ))),
        _ => None,
    }
}

/// Returns a list of all supported board names.
pub fn supported_boards() -> Vec<&'static str> {
    vec![
        "stm32f407",
        "esp32",
        "rp2040",
        "stm32h5f5",
        "dragonwing-iq8",
        "ventuno-q",
        "jetson-thor",
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bsp_arch_display() {
        assert_eq!(BspArch::Thumbv7em.to_string(), "thumbv7em-none-eabihf");
        assert_eq!(BspArch::Thumbv6m.to_string(), "thumbv6m-none-eabi");
        assert_eq!(BspArch::Xtensa.to_string(), "xtensa-esp32-none-elf");
        assert_eq!(BspArch::Riscv32.to_string(), "riscv32imc-unknown-none-elf");
        assert_eq!(
            BspArch::ArmCortexM33.to_string(),
            "thumbv8m.main-none-eabihf"
        );
        assert_eq!(
            BspArch::Aarch64Linux.to_string(),
            "aarch64-unknown-linux-gnu"
        );
    }

    #[test]
    fn memory_region_end_address() {
        let r = MemoryRegion::new("FLASH", 0x0800_0000, 1024 * 1024, MemoryAttr::Rx);
        assert_eq!(r.end_address(), 0x0810_0000);
    }

    #[test]
    fn memory_region_linker_entry() {
        let r = MemoryRegion::new("SRAM1", 0x2000_0000, 128 * 1024, MemoryAttr::Rw);
        let entry = r.to_linker_entry();
        assert!(entry.contains("SRAM1"));
        assert!(entry.contains("0x20000000"));
        assert!(entry.contains("128K"));
    }

    #[test]
    fn peripheral_register_address() {
        let mut p = Peripheral::new("GPIOA", 0x4002_0000);
        p.add_register("MODER", 0x00, 4);
        p.add_register("ODR", 0x14, 4);

        assert_eq!(p.register_address("MODER"), Some(0x4002_0000));
        assert_eq!(p.register_address("ODR"), Some(0x4002_0014));
        assert_eq!(p.register_address("NONEXISTENT"), None);
    }

    #[test]
    fn board_by_name_lookup() {
        assert!(board_by_name("stm32f407").is_some());
        assert!(board_by_name("STM32F407VG").is_some());
        assert!(board_by_name("stm32f4discovery").is_some());
        assert!(board_by_name("esp32").is_some());
        assert!(board_by_name("rp2040").is_some());
        assert!(board_by_name("pico").is_some());
        assert!(board_by_name("stm32h5").is_some());
        assert!(board_by_name("stm32h5f5").is_some());
        assert!(board_by_name("dragonwing").is_some());
        assert!(board_by_name("dragonwing-iq8").is_some());
        assert!(board_by_name("ventuno-q").is_some());
        assert!(board_by_name("ventuno_q").is_some());
        assert!(board_by_name("arduino-ventuno-q").is_some());
        assert!(board_by_name("jetson-thor").is_some());
        assert!(board_by_name("jetson_thor").is_some());
        assert!(board_by_name("thor").is_some());
        assert!(board_by_name("jetson-thor-t4000").is_some());
        assert!(board_by_name("unknown_board").is_none());
    }

    #[test]
    fn supported_boards_list() {
        let boards = supported_boards();
        assert_eq!(boards.len(), 7);
        assert!(boards.contains(&"stm32f407"));
        assert!(boards.contains(&"esp32"));
        assert!(boards.contains(&"rp2040"));
        assert!(boards.contains(&"stm32h5f5"));
        assert!(boards.contains(&"dragonwing-iq8"));
        assert!(boards.contains(&"ventuno-q"));
        assert!(boards.contains(&"jetson-thor"));
    }

    #[test]
    fn board_linker_script_generation() {
        let board = stm32f407::Stm32f407::new();
        let script = board.generate_linker_script();
        assert!(script.contains("MEMORY"));
        assert!(script.contains("FLASH"));
        assert!(script.contains("SRAM1"));
        assert!(script.contains(".isr_vector"));
        assert!(script.contains(".text"));
        assert!(script.contains(".bss"));
        assert!(script.contains("ENTRY(Reset_Handler)"));
    }

    #[test]
    fn board_startup_code_generation() {
        let board = stm32f407::Stm32f407::new();
        let code = board.generate_startup_code();
        assert!(code.contains("Reset_Handler"));
        assert!(code.contains("CopyDataLoop"));
        assert!(code.contains("ZeroBssLoop"));
        assert!(code.contains("bl main"));
        assert!(code.contains("Enable FPU")); // Cortex-M4F
        assert!(code.contains("g_pfnVectors"));
    }

    #[test]
    fn memory_attr_display() {
        assert_eq!(MemoryAttr::Rx.to_string(), "rx");
        assert_eq!(MemoryAttr::Rw.to_string(), "rw");
        assert_eq!(MemoryAttr::Rwx.to_string(), "rwx");
        assert_eq!(MemoryAttr::Ro.to_string(), "r");
    }

    #[test]
    fn board_trait_stm32f407_basic() {
        let board = stm32f407::Stm32f407::new();
        assert_eq!(board.name(), "STM32F407VG Discovery");
        assert_eq!(board.arch(), BspArch::Thumbv7em);
        assert!(board.memory_regions().len() >= 3);
        assert!(board.vector_table_size() > 0);
        assert_eq!(board.cpu_frequency(), 168_000_000);
    }

    #[test]
    fn board_peripherals_not_empty() {
        let board = stm32f407::Stm32f407::new();
        let periphs = board.peripherals();
        assert!(!periphs.is_empty());
        // Should have at least GPIO and USART
        assert!(periphs.iter().any(|p| p.name.starts_with("GPIO")));
        assert!(periphs.iter().any(|p| p.name.starts_with("USART")));
    }
}
