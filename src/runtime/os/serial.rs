//! Serial port (UART 16550) driver for kernel debug output.
//!
//! Provides character-level I/O over COM1 (0x3F8).
//! In QEMU, serial output goes to the host terminal via `-serial stdio`.

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// COM1 base I/O port.
pub const COM1_BASE: u16 = 0x3F8;
/// COM1 data register (read/write).
pub const COM1_DATA: u16 = COM1_BASE;
/// COM1 interrupt enable register.
pub const COM1_IER: u16 = COM1_BASE + 1;
/// COM1 FIFO control register.
pub const COM1_FCR: u16 = COM1_BASE + 2;
/// COM1 line control register.
pub const COM1_LCR: u16 = COM1_BASE + 3;
/// COM1 modem control register.
pub const COM1_MCR: u16 = COM1_BASE + 4;
/// COM1 line status register.
pub const COM1_LSR: u16 = COM1_BASE + 5;

/// Line status: transmitter holding register empty.
pub const LSR_THRE: u8 = 0x20;
/// Line status: data ready.
pub const LSR_DATA_READY: u8 = 0x01;

// ═══════════════════════════════════════════════════════════════════════
// Serial port
// ═══════════════════════════════════════════════════════════════════════

/// Simulated serial port (UART 16550).
///
/// Captures output for testing. In a real kernel, bytes are written
/// to hardware I/O ports.
#[derive(Debug)]
pub struct SerialPort {
    /// Base I/O port address.
    base: u16,
    /// Whether the port has been initialized.
    initialized: bool,
    /// Output buffer (captures transmitted bytes).
    output: Vec<u8>,
    /// Input buffer (simulated received bytes).
    input: Vec<u8>,
}

impl SerialPort {
    /// Create a new serial port at the given base address.
    pub fn new(base: u16) -> Self {
        Self {
            base,
            initialized: false,
            output: Vec::new(),
            input: Vec::new(),
        }
    }

    /// Create COM1 serial port.
    pub fn com1() -> Self {
        Self::new(COM1_BASE)
    }

    /// Initialize the serial port.
    ///
    /// Configures: 115200 baud, 8N1, FIFO enabled.
    /// Returns the port initialization bytes for real hardware.
    pub fn init(&mut self) -> Vec<(u16, u8)> {
        self.initialized = true;
        vec![
            (self.base + 1, 0x00), // Disable interrupts
            (self.base + 3, 0x80), // Enable DLAB (set baud rate divisor)
            (self.base, 0x01),     // Divisor lo: 115200 baud
            (self.base + 1, 0x00), // Divisor hi
            (self.base + 3, 0x03), // 8 bits, no parity, one stop bit
            (self.base + 2, 0xC7), // Enable FIFO, clear, 14-byte threshold
            (self.base + 4, 0x0B), // IRQs enabled, RTS/DSR set
        ]
    }

    /// Write a single byte.
    pub fn write_byte(&mut self, byte: u8) {
        self.output.push(byte);
    }

    /// Write a string.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    /// Write a string followed by newline.
    pub fn write_line(&mut self, s: &str) {
        self.write_string(s);
        self.write_byte(b'\n');
    }

    /// Read a byte from the input buffer.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.input.is_empty() {
            None
        } else {
            Some(self.input.remove(0))
        }
    }

    /// Push bytes into the input buffer (simulated receive).
    pub fn push_input(&mut self, data: &[u8]) {
        self.input.extend_from_slice(data);
    }

    /// Whether there is data available to read.
    pub fn has_data(&self) -> bool {
        !self.input.is_empty()
    }

    /// Get all output as a string.
    pub fn output_string(&self) -> String {
        String::from_utf8_lossy(&self.output).to_string()
    }

    /// Get raw output bytes.
    pub fn output_bytes(&self) -> &[u8] {
        &self.output
    }

    /// Clear the output buffer.
    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    /// Whether the port has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the base port address.
    pub fn base_port(&self) -> u16 {
        self.base
    }
}

impl Default for SerialPort {
    fn default() -> Self {
        Self::com1()
    }
}

impl std::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_init() {
        let mut serial = SerialPort::com1();
        assert!(!serial.is_initialized());
        let cmds = serial.init();
        assert!(serial.is_initialized());
        assert_eq!(cmds.len(), 7);
        assert_eq!(cmds[0].0, COM1_IER); // First: disable interrupts
    }

    #[test]
    fn serial_write_and_read_output() {
        let mut serial = SerialPort::com1();
        serial.init();
        serial.write_string("Hello, COM1!");
        assert_eq!(serial.output_string(), "Hello, COM1!");
    }

    #[test]
    fn serial_input_simulation() {
        let mut serial = SerialPort::com1();
        serial.push_input(b"test");
        assert!(serial.has_data());
        assert_eq!(serial.read_byte(), Some(b't'));
        assert_eq!(serial.read_byte(), Some(b'e'));
        assert_eq!(serial.read_byte(), Some(b's'));
        assert_eq!(serial.read_byte(), Some(b't'));
        assert_eq!(serial.read_byte(), None);
    }

    #[test]
    fn serial_write_line() {
        let mut serial = SerialPort::com1();
        serial.write_line("boot");
        assert_eq!(serial.output_string(), "boot\n");
    }

    #[test]
    fn serial_clear_output() {
        let mut serial = SerialPort::com1();
        serial.write_string("data");
        assert!(!serial.output_bytes().is_empty());
        serial.clear_output();
        assert!(serial.output_bytes().is_empty());
    }
}
