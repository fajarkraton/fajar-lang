//! Unified Hardware Abstraction Layer (HAL) traits.
//!
//! These traits provide a common interface across all BSP boards
//! for GPIO, UART, SPI, and I2C peripherals.

/// GPIO pin trait for digital I/O.
pub trait GpioPin {
    /// Configures the pin as output.
    fn set_output(&mut self);

    /// Configures the pin as input.
    fn set_input(&mut self);

    /// Writes a digital value (high = true, low = false).
    fn write(&mut self, high: bool);

    /// Reads the digital value.
    fn read(&self) -> bool;

    /// Toggles the pin state.
    fn toggle(&mut self);

    /// Returns the pin number.
    fn pin_number(&self) -> u8;
}

/// UART / serial port trait.
pub trait Uart {
    /// Error type.
    type Error: core::fmt::Debug;

    /// Initializes the UART with given baud rate.
    fn init(&mut self, baud_rate: u32) -> Result<(), Self::Error>;

    /// Writes a single byte (blocking).
    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;

    /// Reads a single byte (blocking).
    fn read_byte(&mut self) -> Result<u8, Self::Error>;

    /// Writes a byte slice (blocking).
    fn write_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        for &b in data {
            self.write_byte(b)?;
        }
        Ok(())
    }

    /// Returns the configured baud rate.
    fn baud_rate(&self) -> u32;
}

/// SPI bus trait (master mode).
pub trait Spi {
    /// Error type.
    type Error: core::fmt::Debug;

    /// Initializes the SPI bus.
    fn init(&mut self, clock_hz: u32) -> Result<(), Self::Error>;

    /// Transfers a byte (full duplex: sends tx, returns rx).
    fn transfer(&mut self, tx: u8) -> Result<u8, Self::Error>;

    /// Writes a byte slice (ignores received data).
    fn write_bytes(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        for &b in data {
            self.transfer(b)?;
        }
        Ok(())
    }

    /// Reads into buffer (sends dummy bytes).
    fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<(), Self::Error> {
        for b in buffer.iter_mut() {
            *b = self.transfer(0xFF)?;
        }
        Ok(())
    }
}

/// I2C bus trait (master mode).
pub trait I2c {
    /// Error type.
    type Error: core::fmt::Debug;

    /// Initializes the I2C bus.
    fn init(&mut self, clock_hz: u32) -> Result<(), Self::Error>;

    /// Writes data to a slave address.
    fn write(&mut self, addr: u8, data: &[u8]) -> Result<(), Self::Error>;

    /// Reads data from a slave address.
    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Self::Error>;

    /// Writes then reads (combined transaction).
    fn write_read(
        &mut self,
        addr: u8,
        write_data: &[u8],
        read_buffer: &mut [u8],
    ) -> Result<(), Self::Error>;
}

/// Delay trait for timing.
pub trait Delay {
    /// Delays for the given number of milliseconds.
    fn delay_ms(&mut self, ms: u32);

    /// Delays for the given number of microseconds.
    fn delay_us(&mut self, us: u32);
}

// ═══════════════════════════════════════════════════════════════════════
// Binary Format Conversion
// ═══════════════════════════════════════════════════════════════════════

/// Converts ELF section data to raw binary (strip headers).
///
/// This is a simplified conversion that takes raw data and base address.
/// Full ELF parsing would require the `object` crate.
pub fn to_bin(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

/// Converts binary data to Intel HEX format.
pub fn to_intel_hex(data: &[u8], base_address: u32) -> String {
    let mut hex = String::new();

    // Extended linear address record if needed
    if base_address > 0xFFFF {
        let upper = ((base_address >> 16) & 0xFFFF) as u16;
        let checksum = {
            // Record: byte_count=2, addr=0x0000, type=4
            let byte_count: u32 = 2;
            let record_type: u32 = 4;
            let sum: u32 = byte_count + record_type + (upper >> 8) as u32 + (upper & 0xFF) as u32;
            ((!sum).wrapping_add(1) & 0xFF) as u8
        };
        hex.push_str(&format!(":02000004{:04X}{:02X}\n", upper, checksum));
    }

    // Data records (16 bytes per line)
    for (i, chunk) in data.chunks(16).enumerate() {
        let addr = (base_address as usize + i * 16) & 0xFFFF;
        let len = chunk.len();
        let mut sum: u32 = len as u32 + ((addr >> 8) & 0xFF) as u32 + (addr & 0xFF) as u32;

        hex.push_str(&format!(":{:02X}{:04X}00", len, addr));
        for &b in chunk {
            hex.push_str(&format!("{:02X}", b));
            sum += b as u32;
        }
        let checksum = ((!sum).wrapping_add(1) & 0xFF) as u8;
        hex.push_str(&format!("{:02X}\n", checksum));
    }

    // End-of-file record
    hex.push_str(":00000001FF\n");
    hex
}

// ═══════════════════════════════════════════════════════════════════════
// Memory Budget Analyzer
// ═══════════════════════════════════════════════════════════════════════

/// Memory usage report for a single region.
#[derive(Debug, Clone)]
pub struct MemoryUsage {
    /// Region name.
    pub name: String,
    /// Total available bytes.
    pub total: u32,
    /// Used bytes.
    pub used: u32,
}

impl MemoryUsage {
    /// Creates a new memory usage report.
    pub fn new(name: &str, total: u32, used: u32) -> Self {
        Self {
            name: name.to_string(),
            total,
            used,
        }
    }

    /// Returns the percentage used.
    pub fn percent_used(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f64 / self.total as f64) * 100.0
    }

    /// Returns remaining bytes.
    pub fn remaining(&self) -> u32 {
        self.total.saturating_sub(self.used)
    }

    /// Returns true if usage exceeds the region capacity.
    pub fn is_over_budget(&self) -> bool {
        self.used > self.total
    }

    /// Formats as a human-readable report line.
    pub fn report_line(&self) -> String {
        let bar_width = 20;
        let filled = if self.total > 0 {
            ((self.used as f64 / self.total as f64) * bar_width as f64) as usize
        } else {
            0
        };
        let empty = bar_width - filled.min(bar_width);
        let bar: String = "#".repeat(filled.min(bar_width)) + &".".repeat(empty);

        format!(
            "  {:<8} [{bar}] {:>6} / {:>6} ({:>5.1}%)",
            self.name,
            format_bytes(self.used),
            format_bytes(self.total),
            self.percent_used()
        )
    }
}

/// Formats bytes as human-readable string (B, KB, MB).
fn format_bytes(bytes: u32) -> String {
    if bytes >= 1024 * 1024 {
        format!("{}MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{}B", bytes)
    }
}

/// Generates a memory budget report for a board.
pub fn memory_budget_report(board_name: &str, usages: &[MemoryUsage]) -> String {
    let mut report = String::new();
    report.push_str(&format!("Memory Budget: {board_name}\n"));
    report.push_str(&"─".repeat(60));
    report.push('\n');

    let mut any_over = false;
    for usage in usages {
        report.push_str(&usage.report_line());
        report.push('\n');
        if usage.is_over_budget() {
            any_over = true;
        }
    }

    report.push_str(&"─".repeat(60));
    report.push('\n');

    if any_over {
        report.push_str("WARNING: Memory budget exceeded!\n");
    } else {
        report.push_str("OK: All regions within budget.\n");
    }

    report
}

// ═══════════════════════════════════════════════════════════════════════
// Flash Command Descriptions
// ═══════════════════════════════════════════════════════════════════════

/// Returns the flash command for a given board.
pub fn flash_command(board: &str, firmware_path: &str, options: &FlashOptions) -> Option<String> {
    match board.to_lowercase().as_str() {
        "stm32f407" | "stm32f407vg" => {
            let probe = options.probe.as_deref().unwrap_or("stlink");
            Some(format!(
                "probe-rs run --chip STM32F407VGTx --probe {probe} {firmware_path}"
            ))
        }
        "rp2040" | "pico" => {
            if options.uf2 {
                Some(format!("cp {firmware_path} /media/$USER/RPI-RP2/"))
            } else {
                Some(format!("picotool load -f {firmware_path}"))
            }
        }
        "esp32" => {
            let port = options.port.as_deref().unwrap_or("/dev/ttyUSB0");
            Some(format!(
                "esptool.py --port {port} --baud 460800 write_flash 0x10000 {firmware_path}"
            ))
        }
        _ => None,
    }
}

/// Flash command options.
#[derive(Debug, Clone, Default)]
pub struct FlashOptions {
    /// Probe type (stlink, jlink, cmsis-dap).
    pub probe: Option<String>,
    /// Serial port for ESP32.
    pub port: Option<String>,
    /// Use UF2 format (RP2040).
    pub uf2: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// QEMU Command Generation
// ═══════════════════════════════════════════════════════════════════════

/// Returns a QEMU command for testing firmware.
pub fn qemu_command(board: &str, firmware_path: &str, semihosting: bool) -> Option<String> {
    let semi_flag = if semihosting {
        " -semihosting-config enable=on,target=native"
    } else {
        ""
    };

    match board.to_lowercase().as_str() {
        "stm32f407" | "stm32f407vg" => Some(format!(
            "qemu-system-arm -machine lm3s6965evb -nographic -kernel {firmware_path}{semi_flag}"
        )),
        "rp2040" | "pico" => Some(format!(
            "qemu-system-arm -machine raspi2b -nographic -kernel {firmware_path}{semi_flag}"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intel_hex_basic() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let hex = to_intel_hex(&data, 0x0000);
        assert!(hex.contains(":04000000"));
        assert!(hex.contains(":00000001FF")); // EOF
    }

    #[test]
    fn intel_hex_extended_address() {
        let data = [0xAB; 4];
        let hex = to_intel_hex(&data, 0x0800_0000);
        assert!(hex.contains(":02000004")); // Extended linear address
        assert!(hex.contains(":00000001FF")); // EOF
    }

    #[test]
    fn memory_usage_percent() {
        let usage = MemoryUsage::new("FLASH", 1024 * 1024, 256 * 1024);
        assert!((usage.percent_used() - 25.0).abs() < 0.1);
        assert_eq!(usage.remaining(), 768 * 1024);
        assert!(!usage.is_over_budget());
    }

    #[test]
    fn memory_usage_over_budget() {
        let usage = MemoryUsage::new("SRAM", 1024, 2048);
        assert!(usage.is_over_budget());
    }

    #[test]
    fn memory_budget_report_format() {
        let usages = vec![
            MemoryUsage::new("FLASH", 1024 * 1024, 50 * 1024),
            MemoryUsage::new("SRAM", 128 * 1024, 32 * 1024),
        ];
        let report = memory_budget_report("STM32F407", &usages);
        assert!(report.contains("Memory Budget"));
        assert!(report.contains("FLASH"));
        assert!(report.contains("SRAM"));
        assert!(report.contains("OK"));
    }

    #[test]
    fn flash_command_stm32() {
        let cmd = flash_command("stm32f407", "firmware.elf", &FlashOptions::default());
        assert!(cmd.is_some());
        assert!(cmd.unwrap().contains("probe-rs"));
    }

    #[test]
    fn flash_command_rp2040_uf2() {
        let opts = FlashOptions {
            uf2: true,
            ..Default::default()
        };
        let cmd = flash_command("rp2040", "firmware.uf2", &opts);
        assert!(cmd.is_some());
        assert!(cmd.unwrap().contains("RPI-RP2"));
    }

    #[test]
    fn flash_command_esp32() {
        let cmd = flash_command("esp32", "firmware.bin", &FlashOptions::default());
        assert!(cmd.is_some());
        assert!(cmd.unwrap().contains("esptool"));
    }

    #[test]
    fn qemu_command_stm32() {
        let cmd = qemu_command("stm32f407", "test.elf", true);
        assert!(cmd.is_some());
        let cmd_str = cmd.unwrap();
        assert!(cmd_str.contains("qemu-system-arm"));
        assert!(cmd_str.contains("semihosting"));
    }

    #[test]
    fn qemu_command_unknown() {
        assert!(qemu_command("unknown_board", "test.elf", false).is_none());
    }

    #[test]
    fn format_bytes_display() {
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1024), "1KB");
        assert_eq!(format_bytes(128 * 1024), "128KB");
        assert_eq!(format_bytes(1024 * 1024), "1MB");
    }

    #[test]
    fn memory_usage_report_line() {
        let usage = MemoryUsage::new("FLASH", 1024 * 1024, 512 * 1024);
        let line = usage.report_line();
        assert!(line.contains("FLASH"));
        assert!(line.contains("50.0%"));
    }
}
