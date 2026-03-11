//! Arduino Core compatibility layer for Fajar Lang.
//!
//! Provides Arduino-style API functions (digital I/O, analog, serial, Wire)
//! on top of Zephyr RTOS. This allows developers familiar with Arduino
//! to use familiar APIs while running on the STM32H5F5 (Arduino VENTUNO Q).
//!
//! # API Groups
//!
//! - **GPIO**: `pin_mode`, `digital_write`, `digital_read`, `analog_read`
//! - **Timing**: `delay`, `delay_microseconds`, `millis`, `micros`
//! - **Serial**: `serial_begin`, `serial_print`, `serial_println`, etc.
//! - **Wire (I2C)**: `wire_begin`, `wire_begin_transmission`, `wire_write`, etc.
//!
//! # Pin Modes
//!
//! - `INPUT` (0) — Digital input, no pull-up/pull-down
//! - `OUTPUT` (1) — Digital output
//! - `INPUT_PULLUP` (2) — Digital input with internal pull-up resistor

use std::collections::{HashMap, VecDeque};

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Pin mode: digital input (no pull-up/pull-down).
pub const INPUT: u8 = 0;

/// Pin mode: digital output.
pub const OUTPUT: u8 = 1;

/// Pin mode: digital input with internal pull-up resistor.
pub const INPUT_PULLUP: u8 = 2;

/// Logic level: LOW (0V).
pub const LOW: bool = false;

/// Logic level: HIGH (3.3V on STM32H5).
pub const HIGH: bool = true;

// ═══════════════════════════════════════════════════════════════════════
// Arduino compatibility layer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated pin state.
#[derive(Debug, Clone)]
struct PinState {
    /// Pin mode (INPUT, OUTPUT, INPUT_PULLUP).
    mode: u8,
    /// Digital value (true = HIGH, false = LOW).
    digital_value: bool,
    /// Analog value (0..4095 for 12-bit ADC on STM32H5).
    analog_value: u16,
}

impl Default for PinState {
    fn default() -> Self {
        Self {
            mode: INPUT,
            digital_value: false,
            analog_value: 0,
        }
    }
}

/// Simulated serial port state.
#[derive(Debug, Clone, Default)]
struct SerialState {
    /// Baud rate.
    baud_rate: u32,
    /// Whether the serial port is initialized.
    initialized: bool,
    /// Output buffer (what has been printed).
    output_buffer: String,
    /// Input buffer (bytes available to read).
    input_buffer: VecDeque<u8>,
}

/// Simulated I2C (Wire) state.
#[derive(Debug, Clone, Default)]
struct WireState {
    /// Whether Wire has been initialized.
    initialized: bool,
    /// Current target address.
    target_addr: u8,
    /// Whether a transmission is in progress.
    transmitting: bool,
    /// Transmit buffer.
    tx_buffer: Vec<u8>,
    /// Receive buffer.
    rx_buffer: VecDeque<u8>,
}

/// Arduino Core compatibility layer.
///
/// Provides Arduino-style API functions over a simulated (or real Zephyr)
/// backend. Supports GPIO, timing, Serial, and Wire (I2C) operations.
#[derive(Debug, Clone)]
pub struct ArduinoCompat {
    /// Pin states indexed by pin number.
    pins: HashMap<u8, PinState>,
    /// Serial port state.
    serial: SerialState,
    /// Wire (I2C) state.
    wire: WireState,
    /// Elapsed time in milliseconds.
    millis_counter: u32,
    /// Elapsed time in microseconds.
    micros_counter: u32,
}

impl ArduinoCompat {
    /// Creates a new Arduino compatibility layer.
    pub fn new() -> Self {
        Self {
            pins: HashMap::new(),
            serial: SerialState::default(),
            wire: WireState::default(),
            millis_counter: 0,
            micros_counter: 0,
        }
    }

    // ─── GPIO ────────────────────────────────────────────────────────

    /// Configures a pin's mode (INPUT, OUTPUT, or INPUT_PULLUP).
    ///
    /// # Arguments
    /// * `pin` - Pin number
    /// * `mode` - Pin mode constant
    pub fn pin_mode(&mut self, pin: u8, mode: u8) {
        let state = self.pins.entry(pin).or_default();
        state.mode = mode;
        // INPUT_PULLUP defaults to HIGH
        if mode == INPUT_PULLUP {
            state.digital_value = true;
        }
    }

    /// Writes a digital value to an output pin.
    ///
    /// # Arguments
    /// * `pin` - Pin number (must be configured as OUTPUT)
    /// * `value` - true for HIGH, false for LOW
    pub fn digital_write(&mut self, pin: u8, value: bool) {
        let state = self.pins.entry(pin).or_default();
        state.digital_value = value;
    }

    /// Reads the digital value from a pin.
    ///
    /// # Arguments
    /// * `pin` - Pin number
    pub fn digital_read(&self, pin: u8) -> bool {
        self.pins
            .get(&pin)
            .map(|s| s.digital_value)
            .unwrap_or(false)
    }

    /// Reads an analog value from a pin (0..4095 for 12-bit ADC).
    ///
    /// # Arguments
    /// * `pin` - Analog pin number
    pub fn analog_read(&self, pin: u8) -> u16 {
        self.pins.get(&pin).map(|s| s.analog_value).unwrap_or(0)
    }

    /// Sets a simulated analog value for testing.
    ///
    /// # Arguments
    /// * `pin` - Pin number
    /// * `value` - Analog value (0..4095)
    pub fn set_analog_value(&mut self, pin: u8, value: u16) {
        let state = self.pins.entry(pin).or_default();
        state.analog_value = value.min(4095);
    }

    // ─── Timing ──────────────────────────────────────────────────────

    /// Delays for the given number of milliseconds.
    ///
    /// In simulation, advances the internal counters.
    pub fn delay(&mut self, ms: u32) {
        self.millis_counter = self.millis_counter.wrapping_add(ms);
        self.micros_counter = self.micros_counter.wrapping_add(ms.wrapping_mul(1000));
    }

    /// Delays for the given number of microseconds.
    ///
    /// In simulation, advances the internal counter.
    pub fn delay_microseconds(&mut self, us: u32) {
        self.micros_counter = self.micros_counter.wrapping_add(us);
        // Also advance millis if enough microseconds accumulated
        self.millis_counter = self.millis_counter.wrapping_add(us / 1000);
    }

    /// Returns the number of milliseconds since program start.
    pub fn millis(&self) -> u32 {
        self.millis_counter
    }

    /// Returns the number of microseconds since program start.
    pub fn micros(&self) -> u32 {
        self.micros_counter
    }

    // ─── Serial ──────────────────────────────────────────────────────

    /// Initializes the serial port at the given baud rate.
    ///
    /// # Arguments
    /// * `baud` - Baud rate (e.g., 9600, 115200)
    pub fn serial_begin(&mut self, baud: u32) {
        self.serial.baud_rate = baud;
        self.serial.initialized = true;
        self.serial.output_buffer.clear();
        self.serial.input_buffer.clear();
    }

    /// Prints a string to the serial port (no newline).
    ///
    /// # Arguments
    /// * `msg` - Message to print
    pub fn serial_print(&mut self, msg: &str) {
        if self.serial.initialized {
            self.serial.output_buffer.push_str(msg);
        }
    }

    /// Prints a string to the serial port with a newline.
    ///
    /// # Arguments
    /// * `msg` - Message to print
    pub fn serial_println(&mut self, msg: &str) {
        if self.serial.initialized {
            self.serial.output_buffer.push_str(msg);
            self.serial.output_buffer.push('\n');
        }
    }

    /// Returns the number of bytes available to read from the serial port.
    pub fn serial_available(&self) -> usize {
        self.serial.input_buffer.len()
    }

    /// Reads a byte from the serial port.
    ///
    /// Returns `None` if no data is available.
    pub fn serial_read(&mut self) -> Option<u8> {
        self.serial.input_buffer.pop_front()
    }

    /// Writes a single byte to the serial port.
    ///
    /// # Arguments
    /// * `byte` - Byte to write
    pub fn serial_write(&mut self, byte: u8) {
        if self.serial.initialized {
            self.serial.output_buffer.push(byte as char);
        }
    }

    /// Returns the accumulated serial output buffer (for testing).
    pub fn serial_output(&self) -> &str {
        &self.serial.output_buffer
    }

    /// Injects bytes into the serial input buffer (for testing).
    pub fn serial_inject(&mut self, data: &[u8]) {
        for &b in data {
            self.serial.input_buffer.push_back(b);
        }
    }

    /// Returns whether the serial port is initialized.
    pub fn serial_is_initialized(&self) -> bool {
        self.serial.initialized
    }

    // ─── Wire (I2C) ─────────────────────────────────────────────────

    /// Initializes the I2C bus (Wire.begin equivalent).
    pub fn wire_begin(&mut self) {
        self.wire.initialized = true;
        self.wire.tx_buffer.clear();
        self.wire.rx_buffer.clear();
    }

    /// Begins a transmission to the given I2C address.
    ///
    /// # Arguments
    /// * `addr` - 7-bit I2C address
    pub fn wire_begin_transmission(&mut self, addr: u8) {
        if self.wire.initialized {
            self.wire.target_addr = addr;
            self.wire.transmitting = true;
            self.wire.tx_buffer.clear();
        }
    }

    /// Writes a byte to the I2C transmit buffer.
    ///
    /// # Arguments
    /// * `data` - Byte to write
    pub fn wire_write(&mut self, data: u8) {
        if self.wire.transmitting {
            self.wire.tx_buffer.push(data);
        }
    }

    /// Ends the I2C transmission.
    ///
    /// Returns a status code:
    /// - 0 = success
    /// - 1 = data too long (buffer overflow)
    /// - 2 = NACK on address
    /// - 3 = NACK on data
    /// - 4 = other error
    ///
    /// In simulation, always returns 0 (success).
    pub fn wire_end_transmission(&mut self) -> u8 {
        if !self.wire.transmitting {
            return 4; // not transmitting
        }
        self.wire.transmitting = false;
        0 // success in simulation
    }

    /// Requests bytes from an I2C device.
    ///
    /// # Arguments
    /// * `addr` - 7-bit I2C address
    /// * `quantity` - Number of bytes to request
    ///
    /// Returns the number of bytes available to read.
    pub fn wire_request_from(&mut self, addr: u8, quantity: usize) -> usize {
        if !self.wire.initialized {
            return 0;
        }
        self.wire.target_addr = addr;
        // In simulation, fill rx_buffer with zeros
        self.wire.rx_buffer.clear();
        for _ in 0..quantity {
            self.wire.rx_buffer.push_back(0);
        }
        quantity
    }

    /// Reads a byte from the I2C receive buffer.
    ///
    /// Returns `None` if no data is available.
    pub fn wire_read(&mut self) -> Option<u8> {
        self.wire.rx_buffer.pop_front()
    }

    /// Injects bytes into the I2C receive buffer (for testing).
    pub fn wire_inject_rx(&mut self, data: &[u8]) {
        self.wire.rx_buffer.clear();
        for &b in data {
            self.wire.rx_buffer.push_back(b);
        }
    }

    /// Returns the I2C transmit buffer contents (for testing).
    pub fn wire_tx_buffer(&self) -> &[u8] {
        &self.wire.tx_buffer
    }

    /// Returns whether Wire is initialized.
    pub fn wire_is_initialized(&self) -> bool {
        self.wire.initialized
    }

    /// Returns the current I2C target address.
    pub fn wire_target_addr(&self) -> u8 {
        self.wire.target_addr
    }
}

impl Default for ArduinoCompat {
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

    // ─── GPIO tests ──────────────────────────────────────────────────

    #[test]
    fn pin_mode_sets_mode() {
        let mut compat = ArduinoCompat::new();
        compat.pin_mode(13, OUTPUT);
        assert_eq!(compat.pins.get(&13).map(|p| p.mode), Some(OUTPUT));
    }

    #[test]
    fn digital_write_read_roundtrip() {
        let mut compat = ArduinoCompat::new();
        compat.pin_mode(13, OUTPUT);
        compat.digital_write(13, HIGH);
        assert_eq!(compat.digital_read(13), HIGH);
        compat.digital_write(13, LOW);
        assert_eq!(compat.digital_read(13), LOW);
    }

    #[test]
    fn input_pullup_defaults_to_high() {
        let mut compat = ArduinoCompat::new();
        compat.pin_mode(2, INPUT_PULLUP);
        assert_eq!(compat.digital_read(2), HIGH);
    }

    #[test]
    fn analog_read_returns_set_value() {
        let mut compat = ArduinoCompat::new();
        compat.set_analog_value(0, 2048);
        assert_eq!(compat.analog_read(0), 2048);
    }

    #[test]
    fn analog_value_capped_at_4095() {
        let mut compat = ArduinoCompat::new();
        compat.set_analog_value(0, 5000);
        assert_eq!(compat.analog_read(0), 4095);
    }

    // ─── Timing tests ────────────────────────────────────────────────

    #[test]
    fn delay_advances_millis() {
        let mut compat = ArduinoCompat::new();
        assert_eq!(compat.millis(), 0);
        compat.delay(100);
        assert_eq!(compat.millis(), 100);
        compat.delay(50);
        assert_eq!(compat.millis(), 150);
    }

    #[test]
    fn delay_microseconds_advances_micros() {
        let mut compat = ArduinoCompat::new();
        compat.delay_microseconds(500);
        assert_eq!(compat.micros(), 500);
    }

    // ─── Serial tests ────────────────────────────────────────────────

    #[test]
    fn serial_begin_initializes() {
        let mut compat = ArduinoCompat::new();
        assert!(!compat.serial_is_initialized());
        compat.serial_begin(115200);
        assert!(compat.serial_is_initialized());
    }

    #[test]
    fn serial_print_appends_to_buffer() {
        let mut compat = ArduinoCompat::new();
        compat.serial_begin(9600);
        compat.serial_print("Hello");
        compat.serial_print(" World");
        assert_eq!(compat.serial_output(), "Hello World");
    }

    #[test]
    fn serial_println_adds_newline() {
        let mut compat = ArduinoCompat::new();
        compat.serial_begin(9600);
        compat.serial_println("Line 1");
        compat.serial_println("Line 2");
        assert_eq!(compat.serial_output(), "Line 1\nLine 2\n");
    }

    #[test]
    fn serial_read_from_injected_buffer() {
        let mut compat = ArduinoCompat::new();
        compat.serial_begin(9600);
        compat.serial_inject(&[0x41, 0x42, 0x43]); // 'A', 'B', 'C'
        assert_eq!(compat.serial_available(), 3);
        assert_eq!(compat.serial_read(), Some(0x41));
        assert_eq!(compat.serial_read(), Some(0x42));
        assert_eq!(compat.serial_read(), Some(0x43));
        assert_eq!(compat.serial_read(), None);
    }

    #[test]
    fn serial_write_appends_byte() {
        let mut compat = ArduinoCompat::new();
        compat.serial_begin(9600);
        compat.serial_write(b'X');
        assert_eq!(compat.serial_output(), "X");
    }

    // ─── Wire (I2C) tests ────────────────────────────────────────────

    #[test]
    fn wire_begin_initializes() {
        let mut compat = ArduinoCompat::new();
        assert!(!compat.wire_is_initialized());
        compat.wire_begin();
        assert!(compat.wire_is_initialized());
    }

    #[test]
    fn wire_transmission_cycle() {
        let mut compat = ArduinoCompat::new();
        compat.wire_begin();
        compat.wire_begin_transmission(0x50);
        assert_eq!(compat.wire_target_addr(), 0x50);
        compat.wire_write(0x10);
        compat.wire_write(0x20);
        let status = compat.wire_end_transmission();
        assert_eq!(status, 0); // success
        assert_eq!(compat.wire_tx_buffer(), &[0x10, 0x20]);
    }

    #[test]
    fn wire_request_from_and_read() {
        let mut compat = ArduinoCompat::new();
        compat.wire_begin();
        let count = compat.wire_request_from(0x68, 3);
        assert_eq!(count, 3);
        // Default fill is zeros
        assert_eq!(compat.wire_read(), Some(0));
        assert_eq!(compat.wire_read(), Some(0));
        assert_eq!(compat.wire_read(), Some(0));
        assert_eq!(compat.wire_read(), None);
    }

    #[test]
    fn wire_inject_rx_provides_test_data() {
        let mut compat = ArduinoCompat::new();
        compat.wire_begin();
        compat.wire_inject_rx(&[0xAA, 0xBB]);
        assert_eq!(compat.wire_read(), Some(0xAA));
        assert_eq!(compat.wire_read(), Some(0xBB));
        assert_eq!(compat.wire_read(), None);
    }
}
