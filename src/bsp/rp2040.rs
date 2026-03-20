//! RP2040 (Raspberry Pi Pico) board support package.
//!
//! Memory map:
//! - Flash: 2MB @ 0x1000_0000 (via XIP, execute-in-place)
//! - SRAM: 264KB @ 0x2000_0000 (6 banks, striped)
//! - ROM: 16KB @ 0x0000_0000 (bootrom)
//!
//! CPU: Dual ARM Cortex-M0+ @ 133MHz

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};

/// RP2040 (Raspberry Pi Pico) board.
pub struct Rp2040 {
    _private: (),
}

impl Rp2040 {
    /// Creates a new RP2040 board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns SIO GPIO peripheral (single-cycle I/O).
    fn sio_peripheral() -> Peripheral {
        let mut p = Peripheral::new("SIO", 0xD000_0000);
        p.add_register("GPIO_IN", 0x04, 4);
        p.add_register("GPIO_OUT", 0x10, 4);
        p.add_register("GPIO_OUT_SET", 0x14, 4);
        p.add_register("GPIO_OUT_CLR", 0x18, 4);
        p.add_register("GPIO_OUT_XOR", 0x1C, 4);
        p.add_register("GPIO_OE", 0x20, 4);
        p.add_register("GPIO_OE_SET", 0x24, 4);
        p.add_register("GPIO_OE_CLR", 0x28, 4);
        p
    }

    /// Returns IO Bank0 peripheral (GPIO function select).
    fn io_bank0_peripheral() -> Peripheral {
        let mut p = Peripheral::new("IO_BANK0", 0x4001_4000);
        // Each GPIO has CTRL and STATUS registers (8 bytes per GPIO)
        p.add_register("GPIO0_STATUS", 0x00, 4);
        p.add_register("GPIO0_CTRL", 0x04, 4);
        p.add_register("GPIO1_STATUS", 0x08, 4);
        p.add_register("GPIO1_CTRL", 0x0C, 4);
        p.add_register("GPIO25_STATUS", 0xC8, 4); // Pico LED
        p.add_register("GPIO25_CTRL", 0xCC, 4);
        p
    }

    /// Returns UART peripheral.
    fn uart_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("DR", 0x00, 4);
        p.add_register("RSR", 0x04, 4);
        p.add_register("FR", 0x18, 4);
        p.add_register("IBRD", 0x24, 4);
        p.add_register("FBRD", 0x28, 4);
        p.add_register("LCR_H", 0x2C, 4);
        p.add_register("CR", 0x30, 4);
        p.add_register("IMSC", 0x38, 4);
        p
    }
}

impl Default for Rp2040 {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for Rp2040 {
    fn name(&self) -> &str {
        "Raspberry Pi Pico (RP2040)"
    }

    fn arch(&self) -> BspArch {
        BspArch::Thumbv6m
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("FLASH", 0x1000_0000, 2 * 1024 * 1024, MemoryAttr::Rx),
            MemoryRegion::new("SRAM", 0x2000_0000, 264 * 1024, MemoryAttr::Rwx),
            MemoryRegion::new("ROM", 0x0000_0000, 16 * 1024, MemoryAttr::Ro),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        vec![
            Self::sio_peripheral(),
            Self::io_bank0_peripheral(),
            Self::uart_peripheral("UART0", 0x4003_4000),
            Self::uart_peripheral("UART1", 0x4003_8000),
            // SPI
            {
                let mut p = Peripheral::new("SPI0", 0x4003_C000);
                p.add_register("CR0", 0x00, 4);
                p.add_register("CR1", 0x04, 4);
                p.add_register("DR", 0x08, 4);
                p.add_register("SR", 0x0C, 4);
                p.add_register("CPSR", 0x10, 4);
                p
            },
            // I2C
            {
                let mut p = Peripheral::new("I2C0", 0x4004_4000);
                p.add_register("CON", 0x00, 4);
                p.add_register("TAR", 0x04, 4);
                p.add_register("DATA_CMD", 0x10, 4);
                p.add_register("SS_SCL_HCNT", 0x14, 4);
                p.add_register("SS_SCL_LCNT", 0x18, 4);
                p.add_register("STATUS", 0x70, 4);
                p
            },
            // PIO (Programmable I/O) — unique to RP2040
            {
                let mut p = Peripheral::new("PIO0", 0x5020_0000);
                p.add_register("CTRL", 0x00, 4);
                p.add_register("FSTAT", 0x04, 4);
                p.add_register("TXF0", 0x10, 4);
                p.add_register("RXF0", 0x20, 4);
                p.add_register("INSTR_MEM0", 0x48, 4);
                p
            },
            // Watchdog
            {
                let mut p = Peripheral::new("WATCHDOG", 0x4005_8000);
                p.add_register("CTRL", 0x00, 4);
                p.add_register("LOAD", 0x04, 4);
                p.add_register("TICK", 0x2C, 4);
                p
            },
            // RESETS (peripheral reset controller)
            {
                let mut p = Peripheral::new("RESETS", 0x4000_C000);
                p.add_register("RESET", 0x00, 4);
                p.add_register("WDSEL", 0x04, 4);
                p.add_register("RESET_DONE", 0x08, 4);
                p
            },
        ]
    }

    fn vector_table_size(&self) -> usize {
        // 16 Cortex-M0+ system exceptions + 26 RP2040 IRQs = 48
        // (Cortex-M0+ supports up to 32 external interrupts)
        48
    }

    fn cpu_frequency(&self) -> u32 {
        133_000_000 // 133 MHz
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated startup code for ");
        code.push_str(self.name());
        code.push_str(" */\n\n");

        code.push_str(".syntax unified\n");
        code.push_str(".cpu cortex-m0plus\n");
        code.push_str(".thumb\n\n");

        // Second-stage bootloader (256 bytes for QSPI flash config)
        code.push_str("/* Stage 2 bootloader must be first 256 bytes */\n");
        code.push_str(".section .boot2, \"ax\"\n");
        code.push_str(".align 2\n");
        code.push_str("boot2_entry:\n");
        code.push_str("  /* Minimal boot2: configure XIP for W25Q16 flash */\n");
        code.push_str("  ldr r0, =0x18000000  /* XIP_SSI base */\n");
        code.push_str("  movs r1, #0\n");
        code.push_str("  str r1, [r0, #8]     /* Disable SSI */\n");
        code.push_str("  movs r1, #0x03       /* Standard read */\n");
        code.push_str("  str r1, [r0, #0x60]  /* SPI_CTRLR0 */\n");
        code.push_str("  movs r1, #1\n");
        code.push_str("  str r1, [r0, #8]     /* Enable SSI */\n");
        code.push_str("  /* Pad to 252 bytes + 4-byte CRC32 */\n");
        code.push_str("  .align 8\n");
        code.push_str("  .space 252 - (. - boot2_entry)\n");
        code.push_str("  .word 0x00000000     /* CRC32 placeholder */\n\n");

        // Vector table
        code.push_str(".section .isr_vector, \"a\", %progbits\n");
        code.push_str(".type g_pfnVectors, %object\n");
        code.push_str("g_pfnVectors:\n");
        code.push_str("  .word _estack\n");
        code.push_str("  .word Reset_Handler\n");
        code.push_str("  .word NMI_Handler\n");
        code.push_str("  .word HardFault_Handler\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word SVC_Handler\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word PendSV_Handler\n");
        code.push_str("  .word SysTick_Handler\n\n");

        // Reset handler
        code.push_str(".global Reset_Handler\n");
        code.push_str(".type Reset_Handler, %function\n");
        code.push_str("Reset_Handler:\n");

        // Copy .data
        code.push_str("  ldr r0, =_sdata\n");
        code.push_str("  ldr r1, =_edata\n");
        code.push_str("  ldr r2, =_sidata\n");
        code.push_str("  b CopyDataLoop\n\n");
        code.push_str("CopyDataInit:\n");
        code.push_str("  ldm r2!, {r3}\n");
        code.push_str("  stm r0!, {r3}\n\n");
        code.push_str("CopyDataLoop:\n");
        code.push_str("  cmp r0, r1\n");
        code.push_str("  bcc CopyDataInit\n\n");

        // Zero .bss
        code.push_str("  ldr r0, =_sbss\n");
        code.push_str("  ldr r1, =_ebss\n");
        code.push_str("  movs r2, #0\n");
        code.push_str("  b ZeroBssLoop\n\n");
        code.push_str("ZeroBssInit:\n");
        code.push_str("  stm r0!, {r2}\n\n");
        code.push_str("ZeroBssLoop:\n");
        code.push_str("  cmp r0, r1\n");
        code.push_str("  bcc ZeroBssInit\n\n");

        // Call main
        code.push_str("  bl main\n");
        code.push_str("  b .\n");

        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// UF2 Output Format
// ═══════════════════════════════════════════════════════════════════════

/// UF2 block header for drag-and-drop flashing.
///
/// Each UF2 block is 512 bytes and contains up to 256 bytes of data.
/// The RP2040 bootloader accepts UF2 files via USB mass storage.
#[derive(Debug, Clone)]
pub struct Uf2Block {
    /// First magic number (0x0A324655).
    pub magic_start0: u32,
    /// Second magic number (0x9E5D5157).
    pub magic_start1: u32,
    /// Flags (0x00002000 for RP2040 family ID).
    pub flags: u32,
    /// Target flash address.
    pub target_addr: u32,
    /// Payload size (max 256).
    pub payload_size: u32,
    /// Block number.
    pub block_no: u32,
    /// Total number of blocks.
    pub num_blocks: u32,
    /// Board family ID (0xE48BFF56 for RP2040).
    pub family_id: u32,
    /// Payload data (256 bytes, zero-padded).
    pub data: [u8; 256],
    /// Final magic number (0x0AB16F30).
    pub magic_end: u32,
}

impl Uf2Block {
    /// RP2040 family ID.
    pub const RP2040_FAMILY_ID: u32 = 0xE48B_FF56;
    /// UF2 magic start 0.
    pub const MAGIC_START0: u32 = 0x0A32_4655;
    /// UF2 magic start 1.
    pub const MAGIC_START1: u32 = 0x9E5D_5157;
    /// UF2 magic end.
    pub const MAGIC_END: u32 = 0x0AB1_6F30;

    /// Creates a new UF2 block.
    pub fn new(target_addr: u32, data: &[u8], block_no: u32, num_blocks: u32) -> Self {
        let mut block_data = [0u8; 256];
        let len = data.len().min(256);
        block_data[..len].copy_from_slice(&data[..len]);

        Self {
            magic_start0: Self::MAGIC_START0,
            magic_start1: Self::MAGIC_START1,
            flags: 0x0000_2000, // familyID present
            target_addr,
            payload_size: len as u32,
            block_no,
            num_blocks,
            family_id: Self::RP2040_FAMILY_ID,
            data: block_data,
            magic_end: Self::MAGIC_END,
        }
    }

    /// Serializes this block to a 512-byte array.
    pub fn to_bytes(&self) -> [u8; 512] {
        let mut bytes = [0u8; 512];
        bytes[0..4].copy_from_slice(&self.magic_start0.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.magic_start1.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.flags.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.target_addr.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.payload_size.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.block_no.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.num_blocks.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.family_id.to_le_bytes());
        bytes[32..288].copy_from_slice(&self.data);
        bytes[508..512].copy_from_slice(&self.magic_end.to_le_bytes());
        bytes
    }
}

/// Converts a binary firmware blob to UF2 format for RP2040.
///
/// The firmware is split into 256-byte chunks, each wrapped in a UF2 block.
pub fn binary_to_uf2(firmware: &[u8], base_address: u32) -> Vec<u8> {
    let num_blocks = firmware.len().div_ceil(256);
    let mut uf2 = Vec::with_capacity(num_blocks * 512);

    for (i, chunk) in firmware.chunks(256).enumerate() {
        let addr = base_address + (i as u32) * 256;
        let block = Uf2Block::new(addr, chunk, i as u32, num_blocks as u32);
        uf2.extend_from_slice(&block.to_bytes());
    }

    uf2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rp2040_board_identity() {
        let board = Rp2040::new();
        assert_eq!(board.name(), "Raspberry Pi Pico (RP2040)");
        assert_eq!(board.arch(), BspArch::Thumbv6m);
        assert_eq!(board.cpu_frequency(), 133_000_000);
    }

    #[test]
    fn rp2040_memory_regions() {
        let board = Rp2040::new();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 3);

        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].origin, 0x1000_0000);
        assert_eq!(regions[0].length, 2 * 1024 * 1024);

        assert_eq!(regions[1].name, "SRAM");
        assert_eq!(regions[1].origin, 0x2000_0000);
        assert_eq!(regions[1].length, 264 * 1024);
    }

    #[test]
    fn rp2040_sio_peripheral() {
        let board = Rp2040::new();
        let periphs = board.peripherals();
        let sio = periphs.iter().find(|p| p.name == "SIO").unwrap();

        assert_eq!(sio.base_address, 0xD000_0000);
        assert_eq!(sio.register_address("GPIO_OUT"), Some(0xD000_0010));
        assert_eq!(sio.register_address("GPIO_OUT_SET"), Some(0xD000_0014));
        assert_eq!(sio.register_address("GPIO_OE"), Some(0xD000_0020));
    }

    #[test]
    fn rp2040_uart_peripherals() {
        let board = Rp2040::new();
        let periphs = board.peripherals();
        let uart0 = periphs.iter().find(|p| p.name == "UART0").unwrap();

        assert_eq!(uart0.base_address, 0x4003_4000);
        assert_eq!(uart0.register_address("DR"), Some(0x4003_4000));
        assert_eq!(uart0.register_address("CR"), Some(0x4003_4030));
    }

    #[test]
    fn rp2040_pio_peripheral() {
        let board = Rp2040::new();
        let periphs = board.peripherals();
        let pio = periphs.iter().find(|p| p.name == "PIO0").unwrap();

        assert_eq!(pio.base_address, 0x5020_0000);
        assert_eq!(pio.register_address("CTRL"), Some(0x5020_0000));
    }

    #[test]
    fn rp2040_startup_code() {
        let board = Rp2040::new();
        let code = board.generate_startup_code();
        assert!(code.contains(".cpu cortex-m0plus"));
        assert!(code.contains("Reset_Handler"));
        assert!(code.contains("boot2_entry")); // stage 2 bootloader
        assert!(code.contains("bl main"));
    }

    #[test]
    fn rp2040_linker_script() {
        let board = Rp2040::new();
        let script = board.generate_linker_script();
        assert!(script.contains("FLASH"));
        assert!(script.contains("SRAM"));
        assert!(script.contains("MEMORY"));
    }

    #[test]
    fn uf2_block_creation() {
        let data = [0xABu8; 100];
        let block = Uf2Block::new(0x1000_0000, &data, 0, 1);
        assert_eq!(block.magic_start0, Uf2Block::MAGIC_START0);
        assert_eq!(block.target_addr, 0x1000_0000);
        assert_eq!(block.payload_size, 100);
        assert_eq!(block.family_id, Uf2Block::RP2040_FAMILY_ID);
    }

    #[test]
    fn uf2_block_serialization() {
        let data = [0x42u8; 10];
        let block = Uf2Block::new(0x1000_0100, &data, 1, 5);
        let bytes = block.to_bytes();

        assert_eq!(bytes.len(), 512);
        // Check magic numbers
        assert_eq!(
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            Uf2Block::MAGIC_START0
        );
        assert_eq!(
            u32::from_le_bytes([bytes[508], bytes[509], bytes[510], bytes[511]]),
            Uf2Block::MAGIC_END
        );
        // Check payload at offset 32
        assert_eq!(bytes[32], 0x42);
        assert_eq!(bytes[41], 0x42);
        assert_eq!(bytes[42], 0x00); // zero-padded
    }

    #[test]
    fn binary_to_uf2_conversion() {
        let firmware = vec![0xFFu8; 600]; // 3 blocks needed (256+256+88)
        let uf2 = binary_to_uf2(&firmware, 0x1000_0000);

        assert_eq!(uf2.len(), 3 * 512); // 3 UF2 blocks
        // First block: addr = 0x1000_0000
        assert_eq!(
            u32::from_le_bytes([uf2[12], uf2[13], uf2[14], uf2[15]]),
            0x1000_0000
        );
        // Second block: addr = 0x1000_0100
        assert_eq!(
            u32::from_le_bytes([uf2[512 + 12], uf2[512 + 13], uf2[512 + 14], uf2[512 + 15]]),
            0x1000_0100
        );
    }

    #[test]
    fn rp2040_default_trait() {
        let board = Rp2040::default();
        assert_eq!(board.name(), "Raspberry Pi Pico (RP2040)");
    }
}
