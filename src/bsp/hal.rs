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

/// CAN-FD bus trait.
pub trait CanFd {
    /// Error type.
    type Error: core::fmt::Debug;

    /// Initializes the CAN-FD controller with nominal and data bitrates.
    fn init(&mut self, bitrate: u32, data_bitrate: u32) -> Result<(), Self::Error>;

    /// Sends a CAN-FD frame.
    fn send(&mut self, frame: &CanFrame) -> Result<(), Self::Error>;

    /// Receives a CAN-FD frame (blocking).
    fn receive(&mut self) -> Result<CanFrame, Self::Error>;

    /// Sets an acceptance filter with ID and mask.
    fn set_filter(&mut self, id: u32, mask: u32) -> Result<(), Self::Error>;
}

/// CAN-FD frame.
#[derive(Debug, Clone)]
pub struct CanFrame {
    /// CAN identifier (11-bit standard or 29-bit extended).
    pub id: u32,
    /// True if using 29-bit extended identifier.
    pub is_extended: bool,
    /// True if this is an FD frame (supports > 8 bytes and BRS).
    pub is_fd: bool,
    /// True if bit rate switching is enabled (FD only).
    pub is_brs: bool,
    /// Data length code (0-8 for classic, 0-64 for FD).
    pub dlc: u8,
    /// Frame data payload (max 64 bytes for FD, max 8 for classic).
    pub data: Vec<u8>,
}

impl CanFrame {
    /// Creates a new standard CAN frame (classic, 11-bit ID).
    pub fn new_standard(id: u32, data: &[u8]) -> Self {
        let len = data.len().min(8);
        Self {
            id: id & 0x7FF,
            is_extended: false,
            is_fd: false,
            is_brs: false,
            dlc: len as u8,
            data: data[..len].to_vec(),
        }
    }

    /// Creates a new CAN-FD frame (up to 64 bytes, optional BRS).
    pub fn new_fd(id: u32, data: &[u8], is_brs: bool) -> Self {
        let len = data.len().min(64);
        Self {
            id: id & 0x7FF,
            is_extended: false,
            is_fd: true,
            is_brs,
            dlc: Self::data_len_to_dlc(len),
            data: data[..len].to_vec(),
        }
    }

    /// Creates a new extended-ID CAN frame (29-bit ID).
    pub fn new_extended(id: u32, data: &[u8]) -> Self {
        let len = data.len().min(8);
        Self {
            id: id & 0x1FFF_FFFF,
            is_extended: true,
            is_fd: false,
            is_brs: false,
            dlc: len as u8,
            data: data[..len].to_vec(),
        }
    }

    /// Returns the maximum data length for this frame's DLC.
    pub fn max_data_length(&self) -> usize {
        if self.is_fd {
            Self::dlc_to_data_len(self.dlc)
        } else {
            self.dlc.min(8) as usize
        }
    }

    /// Converts a data length to a DLC value (CAN-FD encoding).
    fn data_len_to_dlc(len: usize) -> u8 {
        match len {
            0..=8 => len as u8,
            9..=12 => 9,
            13..=16 => 10,
            17..=20 => 11,
            21..=24 => 12,
            25..=32 => 13,
            33..=48 => 14,
            _ => 15, // 49..=64
        }
    }

    /// Converts a DLC value to the actual data length (CAN-FD decoding).
    fn dlc_to_data_len(dlc: u8) -> usize {
        match dlc {
            0..=8 => dlc as usize,
            9 => 12,
            10 => 16,
            11 => 20,
            12 => 24,
            13 => 32,
            14 => 48,
            _ => 64,
        }
    }
}

/// CAN bit timing parameters (for both nominal and data phases).
#[derive(Debug, Clone)]
pub struct CanBitTiming {
    /// Prescaler divider.
    pub prescaler: u16,
    /// Synchronization jump width.
    pub sjw: u8,
    /// Time segment 1 (propagation + phase segment 1).
    pub tseg1: u16,
    /// Time segment 2 (phase segment 2).
    pub tseg2: u8,
}

impl CanBitTiming {
    /// Calculates the actual bitrate from these timing parameters.
    pub fn actual_bitrate(&self, clock_hz: u32) -> u32 {
        let total_tq = 1 + self.tseg1 as u32 + self.tseg2 as u32;
        if total_tq == 0 || self.prescaler == 0 {
            return 0;
        }
        clock_hz / (self.prescaler as u32 * total_tq)
    }

    /// Returns the sample point percentage.
    pub fn sample_point(&self) -> f64 {
        let total_tq = 1 + self.tseg1 as u32 + self.tseg2 as u32;
        if total_tq == 0 {
            return 0.0;
        }
        ((1 + self.tseg1 as u32) as f64 / total_tq as f64) * 100.0
    }
}

/// Calculates CAN bit timing for a given clock and target bitrate.
///
/// Returns `None` if no valid timing can be found.
pub fn calculate_can_timing(clock_hz: u32, target_bitrate: u32) -> Option<CanBitTiming> {
    for prescaler in 1..=512u16 {
        let tq_freq = clock_hz / prescaler as u32;
        if tq_freq == 0 || !tq_freq.is_multiple_of(target_bitrate) {
            continue;
        }
        let total_tq = tq_freq / target_bitrate;
        if !(3..=385).contains(&total_tq) {
            continue;
        }
        // Target ~87.5% sample point
        let tseg1 = ((total_tq * 7) / 8).saturating_sub(1);
        let tseg2 = total_tq - 1 - tseg1;
        if !(1..=256).contains(&tseg1) || !(1..=128).contains(&tseg2) {
            continue;
        }
        let sjw = tseg2.min(128) as u8;
        return Some(CanBitTiming {
            prescaler,
            sjw,
            tseg1: tseg1 as u16,
            tseg2: tseg2 as u8,
        });
    }
    None
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
        "stm32h5" | "stm32h5f5" => {
            let probe = options.probe.as_deref().unwrap_or("stlink");
            Some(format!(
                "probe-rs run --chip STM32H5F5LJTx --probe {probe} {firmware_path}"
            ))
        }
        "ventuno-q" | "ventuno_q" | "arduino-ventuno-q" => {
            let probe = options.probe.as_deref().unwrap_or("stlink");
            Some(format!(
                "probe-rs run --chip STM32H5F5LJTx --probe {probe} {firmware_path}"
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
        "stm32h5" | "stm32h5f5" | "ventuno-q" | "ventuno_q" | "arduino-ventuno-q" => Some(format!(
            "qemu-system-arm -machine mps3-an547 -nographic -kernel {firmware_path}{semi_flag}"
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

    // ── CAN-FD Tests ────────────────────────────────────────────

    #[test]
    fn can_frame_standard_creation() {
        let frame = CanFrame::new_standard(0x123, &[1, 2, 3, 4]);
        assert_eq!(frame.id, 0x123);
        assert!(!frame.is_extended);
        assert!(!frame.is_fd);
        assert!(!frame.is_brs);
        assert_eq!(frame.dlc, 4);
        assert_eq!(frame.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn can_frame_fd_creation() {
        let data = vec![0u8; 32];
        let frame = CanFrame::new_fd(0x100, &data, true);
        assert!(frame.is_fd);
        assert!(frame.is_brs);
        assert_eq!(frame.dlc, 13); // 32 bytes = DLC 13
        assert_eq!(frame.data.len(), 32);
    }

    #[test]
    fn can_frame_extended_creation() {
        let frame = CanFrame::new_extended(0x1234_5678, &[0xAB, 0xCD]);
        assert!(frame.is_extended);
        assert_eq!(frame.id, 0x1234_5678 & 0x1FFF_FFFF);
        assert_eq!(frame.dlc, 2);
    }

    #[test]
    fn can_frame_max_data_length() {
        let classic = CanFrame::new_standard(0x100, &[1, 2, 3]);
        assert_eq!(classic.max_data_length(), 3);

        let fd = CanFrame::new_fd(0x100, &vec![0u8; 48], false);
        assert_eq!(fd.max_data_length(), 48); // DLC 14 = 48 bytes
    }

    #[test]
    fn can_frame_dlc_encoding() {
        // FD DLC encoding: 9=12, 10=16, 11=20, 12=24, 13=32, 14=48, 15=64
        let frame_12 = CanFrame::new_fd(0x100, &vec![0u8; 12], false);
        assert_eq!(frame_12.dlc, 9);

        let frame_64 = CanFrame::new_fd(0x100, &vec![0u8; 64], false);
        assert_eq!(frame_64.dlc, 15);
    }

    #[test]
    fn can_bit_timing_bitrate() {
        let timing = CanBitTiming {
            prescaler: 5,
            sjw: 4,
            tseg1: 31,
            tseg2: 8,
        };
        // Total TQ = 1 + 31 + 8 = 40
        // Bitrate = 250_000_000 / (5 * 40) = 1_250_000
        assert_eq!(timing.actual_bitrate(250_000_000), 1_250_000);
    }

    #[test]
    fn can_bit_timing_sample_point() {
        let timing = CanBitTiming {
            prescaler: 5,
            sjw: 4,
            tseg1: 31,
            tseg2: 8,
        };
        // Sample point = (1+31) / 40 = 80%
        assert!((timing.sample_point() - 80.0).abs() < 0.1);
    }

    #[test]
    fn can_timing_calculation_500kbps() {
        let timing = calculate_can_timing(250_000_000, 500_000);
        assert!(timing.is_some());
        let t = timing.unwrap();
        assert_eq!(t.actual_bitrate(250_000_000), 500_000);
    }

    #[test]
    fn can_timing_calculation_1mbps() {
        let timing = calculate_can_timing(250_000_000, 1_000_000);
        assert!(timing.is_some());
        let t = timing.unwrap();
        assert_eq!(t.actual_bitrate(250_000_000), 1_000_000);
    }

    // ── STM32H5 / VENTUNO Q Flash & QEMU Tests ─────────────────

    #[test]
    fn flash_command_stm32h5() {
        let cmd = flash_command("stm32h5", "firmware.elf", &FlashOptions::default());
        assert!(cmd.is_some());
        let cmd_str = cmd.unwrap();
        assert!(cmd_str.contains("probe-rs"));
        assert!(cmd_str.contains("STM32H5F5LJTx"));
    }

    #[test]
    fn flash_command_ventuno_q() {
        let cmd = flash_command("ventuno-q", "firmware.elf", &FlashOptions::default());
        assert!(cmd.is_some());
        let cmd_str = cmd.unwrap();
        assert!(cmd_str.contains("probe-rs"));
        assert!(cmd_str.contains("STM32H5F5LJTx"));
    }

    #[test]
    fn qemu_command_stm32h5() {
        let cmd = qemu_command("stm32h5", "test.elf", false);
        assert!(cmd.is_some());
        let cmd_str = cmd.unwrap();
        assert!(cmd_str.contains("qemu-system-arm"));
        assert!(cmd_str.contains("mps3-an547"));
    }

    #[test]
    fn qemu_command_ventuno_q() {
        let cmd = qemu_command("ventuno-q", "test.elf", true);
        assert!(cmd.is_some());
        let cmd_str = cmd.unwrap();
        assert!(cmd_str.contains("mps3-an547"));
        assert!(cmd_str.contains("semihosting"));
    }
}
