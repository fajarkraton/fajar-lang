//! ESP32 board support package.
//!
//! Memory map:
//! - DRAM: 520KB @ 0x3FFB_0000 (internal SRAM)
//! - IRAM: 128KB @ 0x4007_0000 (instruction RAM)
//! - Flash: 4MB @ 0x3F40_0000 (memory-mapped via cache)
//! - RTC SRAM: 8KB @ 0x5000_0000
//!
//! CPU: Xtensa LX6 dual-core @ 240MHz

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};

/// ESP32 board (generic module).
pub struct Esp32 {
    _private: (),
}

impl Esp32 {
    /// Creates a new ESP32 board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns GPIO peripheral.
    fn gpio_peripheral() -> Peripheral {
        let mut p = Peripheral::new("GPIO", 0x3FF4_4000);
        p.add_register("OUT", 0x04, 4);
        p.add_register("OUT_W1TS", 0x08, 4);
        p.add_register("OUT_W1TC", 0x0C, 4);
        p.add_register("OUT1", 0x10, 4);
        p.add_register("ENABLE", 0x20, 4);
        p.add_register("ENABLE_W1TS", 0x24, 4);
        p.add_register("ENABLE_W1TC", 0x28, 4);
        p.add_register("IN", 0x3C, 4);
        p.add_register("IN1", 0x40, 4);
        p.add_register("STATUS", 0x44, 4);
        // Pin function selection (IO_MUX)
        p.add_register("FUNC_OUT_SEL_CFG0", 0x530, 4);
        p
    }

    /// Returns UART peripheral.
    fn uart_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("FIFO", 0x00, 4);
        p.add_register("INT_RAW", 0x04, 4);
        p.add_register("INT_ST", 0x08, 4);
        p.add_register("INT_ENA", 0x0C, 4);
        p.add_register("INT_CLR", 0x10, 4);
        p.add_register("CLKDIV", 0x14, 4);
        p.add_register("CONF0", 0x20, 4);
        p.add_register("CONF1", 0x24, 4);
        p.add_register("STATUS", 0x1C, 4);
        p
    }
}

impl Default for Esp32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for Esp32 {
    fn name(&self) -> &str {
        "ESP32"
    }

    fn arch(&self) -> BspArch {
        BspArch::Xtensa
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("DRAM", 0x3FFB_0000, 520 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("IRAM", 0x4007_0000, 128 * 1024, MemoryAttr::Rwx),
            MemoryRegion::new("FLASH", 0x3F40_0000, 4 * 1024 * 1024, MemoryAttr::Rx),
            MemoryRegion::new("RTC_SRAM", 0x5000_0000, 8 * 1024, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        vec![
            Self::gpio_peripheral(),
            Self::uart_peripheral("UART0", 0x3FF4_0000),
            Self::uart_peripheral("UART1", 0x3FF5_0000),
            Self::uart_peripheral("UART2", 0x3FF6_E000),
            // SPI
            {
                let mut p = Peripheral::new("SPI2", 0x3FF6_4000);
                p.add_register("CMD", 0x00, 4);
                p.add_register("ADDR", 0x04, 4);
                p.add_register("CTRL", 0x08, 4);
                p.add_register("CLOCK", 0x18, 4);
                p.add_register("USER", 0x1C, 4);
                p.add_register("W0", 0x80, 4);
                p
            },
            // I2C
            {
                let mut p = Peripheral::new("I2C0", 0x3FF5_3000);
                p.add_register("SCL_LOW_PERIOD", 0x00, 4);
                p.add_register("CTR", 0x04, 4);
                p.add_register("SR", 0x08, 4);
                p.add_register("TO", 0x0C, 4);
                p.add_register("DATA", 0x1C, 4);
                p
            },
            // Timer Group 0
            {
                let mut p = Peripheral::new("TIMG0", 0x3FF5_F000);
                p.add_register("T0CONFIG", 0x00, 4);
                p.add_register("T0LO", 0x04, 4);
                p.add_register("T0HI", 0x08, 4);
                p.add_register("T0ALARMLO", 0x10, 4);
                p.add_register("T0ALARMHI", 0x14, 4);
                p
            },
            // RTC
            {
                let mut p = Peripheral::new("RTC_CNTL", 0x3FF4_8000);
                p.add_register("OPTIONS0", 0x00, 4);
                p.add_register("SLP_TIMER0", 0x04, 4);
                p.add_register("CLK_CONF", 0x70, 4);
                p
            },
        ]
    }

    fn vector_table_size(&self) -> usize {
        // ESP32 uses Xtensa exception/interrupt vectors, not ARM-style
        // 32 interrupt sources
        32
    }

    fn cpu_frequency(&self) -> u32 {
        240_000_000 // 240 MHz
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated startup code for ");
        code.push_str(self.name());
        code.push_str(" */\n\n");
        code.push_str("/* ESP32 uses second-stage bootloader from esp-idf */\n");
        code.push_str("/* Startup is handled by ROM bootloader → app_main */\n\n");
        code.push_str(".global app_main\n");
        code.push_str(".type app_main, @function\n");
        code.push_str("app_main:\n");
        code.push_str("  call4 main\n");
        code.push_str("  retw.n\n");
        code
    }
}

/// ESP32 partition table entry.
#[derive(Debug, Clone)]
pub struct PartitionEntry {
    /// Partition name (max 16 bytes).
    pub name: String,
    /// Partition type (0=app, 1=data).
    pub ptype: u8,
    /// Subtype.
    pub subtype: u8,
    /// Offset in flash.
    pub offset: u32,
    /// Size in bytes.
    pub size: u32,
}

impl PartitionEntry {
    /// Creates a new partition entry.
    pub fn new(name: &str, ptype: u8, subtype: u8, offset: u32, size: u32) -> Self {
        Self {
            name: name.to_string(),
            ptype,
            subtype,
            offset,
            size,
        }
    }
}

/// Default ESP32 partition table.
pub fn default_partition_table() -> Vec<PartitionEntry> {
    vec![
        // nvs (non-volatile storage)
        PartitionEntry::new("nvs", 1, 0x02, 0x9000, 0x6000),
        // phy_init
        PartitionEntry::new("phy_init", 1, 0x01, 0xF000, 0x1000),
        // factory app
        PartitionEntry::new("factory", 0, 0x00, 0x10000, 0x100000),
    ]
}

/// Generates a CSV partition table for esptool.
pub fn partition_table_csv(entries: &[PartitionEntry]) -> String {
    let mut csv = String::new();
    csv.push_str("# ESP-IDF Partition Table\n");
    csv.push_str("# Name,   Type, SubType, Offset, Size\n");
    for entry in entries {
        csv.push_str(&format!(
            "{},       {},    {:#04x},    {:#x},  {:#x}\n",
            entry.name, entry.ptype, entry.subtype, entry.offset, entry.size
        ));
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esp32_board_identity() {
        let board = Esp32::new();
        assert_eq!(board.name(), "ESP32");
        assert_eq!(board.arch(), BspArch::Xtensa);
        assert_eq!(board.cpu_frequency(), 240_000_000);
    }

    #[test]
    fn esp32_memory_regions() {
        let board = Esp32::new();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 4);

        assert_eq!(regions[0].name, "DRAM");
        assert_eq!(regions[0].length, 520 * 1024);

        assert_eq!(regions[2].name, "FLASH");
        assert_eq!(regions[2].length, 4 * 1024 * 1024);
    }

    #[test]
    fn esp32_gpio_peripheral() {
        let board = Esp32::new();
        let periphs = board.peripherals();
        let gpio = periphs.iter().find(|p| p.name == "GPIO").unwrap();
        assert_eq!(gpio.base_address, 0x3FF4_4000);
        assert_eq!(gpio.register_address("OUT"), Some(0x3FF4_4004));
        assert_eq!(gpio.register_address("IN"), Some(0x3FF4_403C));
    }

    #[test]
    fn esp32_uart_peripherals() {
        let board = Esp32::new();
        let periphs = board.peripherals();
        let uart0 = periphs.iter().find(|p| p.name == "UART0").unwrap();
        assert_eq!(uart0.base_address, 0x3FF4_0000);
        assert_eq!(uart0.register_address("FIFO"), Some(0x3FF4_0000));
    }

    #[test]
    fn esp32_startup_code() {
        let board = Esp32::new();
        let code = board.generate_startup_code();
        assert!(code.contains("app_main"));
        assert!(code.contains("call4 main"));
    }

    #[test]
    fn esp32_partition_table() {
        let table = default_partition_table();
        assert_eq!(table.len(), 3);
        assert_eq!(table[0].name, "nvs");
        assert_eq!(table[2].name, "factory");
    }

    #[test]
    fn esp32_partition_csv() {
        let table = default_partition_table();
        let csv = partition_table_csv(&table);
        assert!(csv.contains("nvs"));
        assert!(csv.contains("factory"));
        assert!(csv.contains("# ESP-IDF"));
    }

    #[test]
    fn esp32_linker_script() {
        let board = Esp32::new();
        let script = board.generate_linker_script();
        assert!(script.contains("DRAM"));
        assert!(script.contains("FLASH"));
        assert!(script.contains("MEMORY"));
    }

    #[test]
    fn esp32_default_trait() {
        let board = Esp32::default();
        assert_eq!(board.name(), "ESP32");
    }
}
