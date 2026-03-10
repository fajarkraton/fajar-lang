//! Bus driver traits — I2C, SPI interfaces.
//!
//! Provides abstract bus driver traits for embedded peripheral
//! communication. Includes mock implementations for testing.

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Bus errors
// ═══════════════════════════════════════════════════════════════════════

/// Bus communication errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum BusError {
    /// NAK received (device not responding).
    #[error("bus NAK: device at address {addr:#x} not responding")]
    Nak { addr: u8 },
    /// Transfer buffer size mismatch.
    #[error("bus transfer size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },
    /// Bus busy / arbitration lost.
    #[error("bus busy")]
    Busy,
    /// Generic bus error.
    #[error("bus error: {0}")]
    Other(String),
}

// ═══════════════════════════════════════════════════════════════════════
// I2C bus trait
// ═══════════════════════════════════════════════════════════════════════

/// I2C bus driver interface.
///
/// Standard two-wire serial interface for communicating with sensors,
/// EEPROMs, and other peripherals.
pub trait I2cBus {
    /// Read bytes from the given address and register.
    fn read(&mut self, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), BusError>;

    /// Write bytes to the given address and register.
    fn write(&mut self, addr: u8, reg: u8, data: &[u8]) -> Result<(), BusError>;

    /// Read a single byte.
    fn read_byte(&mut self, addr: u8, reg: u8) -> Result<u8, BusError> {
        let mut buf = [0u8; 1];
        self.read(addr, reg, &mut buf)?;
        Ok(buf[0])
    }

    /// Write a single byte.
    fn write_byte(&mut self, addr: u8, reg: u8, value: u8) -> Result<(), BusError> {
        self.write(addr, reg, &[value])
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SPI bus trait
// ═══════════════════════════════════════════════════════════════════════

/// SPI bus driver interface.
///
/// Full-duplex serial interface. Transfer sends TX and receives RX
/// simultaneously.
pub trait SpiBus {
    /// Full-duplex transfer: sends `tx` and receives into `rx`.
    fn transfer(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(), BusError>;

    /// Write-only (ignores received data).
    fn write(&mut self, data: &[u8]) -> Result<(), BusError> {
        let mut rx = vec![0u8; data.len()];
        self.transfer(data, &mut rx)
    }

    /// Read-only (sends zeros).
    fn read(&mut self, buf: &mut [u8]) -> Result<(), BusError> {
        let tx = vec![0u8; buf.len()];
        self.transfer(&tx, buf)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Mock I2C
// ═══════════════════════════════════════════════════════════════════════

/// Mock I2C bus for testing.
///
/// Stores register values per device address. Read/write operations
/// access the internal register map.
#[derive(Debug, Default)]
pub struct MockI2c {
    /// Device registers: (addr, reg) → value.
    registers: std::collections::HashMap<(u8, u8), u8>,
    /// Transaction log: (addr, reg, is_write, data).
    log: Vec<(u8, u8, bool, Vec<u8>)>,
}

impl MockI2c {
    /// Create a new mock I2C bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-set a register value (for testing read operations).
    pub fn set_register(&mut self, addr: u8, reg: u8, value: u8) {
        self.registers.insert((addr, reg), value);
    }

    /// Get the transaction log.
    pub fn log(&self) -> &[(u8, u8, bool, Vec<u8>)] {
        &self.log
    }

    /// Get a register value (for verifying writes).
    pub fn get_register(&self, addr: u8, reg: u8) -> Option<u8> {
        self.registers.get(&(addr, reg)).copied()
    }
}

impl I2cBus for MockI2c {
    fn read(&mut self, addr: u8, reg: u8, buf: &mut [u8]) -> Result<(), BusError> {
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = self
                .registers
                .get(&(addr, reg.wrapping_add(i as u8)))
                .copied()
                .unwrap_or(0xFF);
        }
        self.log.push((addr, reg, false, buf.to_vec()));
        Ok(())
    }

    fn write(&mut self, addr: u8, reg: u8, data: &[u8]) -> Result<(), BusError> {
        for (i, &byte) in data.iter().enumerate() {
            self.registers
                .insert((addr, reg.wrapping_add(i as u8)), byte);
        }
        self.log.push((addr, reg, true, data.to_vec()));
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Mock SPI
// ═══════════════════════════════════════════════════════════════════════

/// Mock SPI bus for testing.
///
/// Returns pre-configured response data during transfers.
#[derive(Debug, Default)]
pub struct MockSpi {
    /// Response data to return on next transfer.
    response: Vec<u8>,
    /// Transaction log: (tx_data).
    log: Vec<Vec<u8>>,
}

impl MockSpi {
    /// Create a new mock SPI bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the response data for the next transfer.
    pub fn set_response(&mut self, data: &[u8]) {
        self.response = data.to_vec();
    }

    /// Get the transaction log.
    pub fn log(&self) -> &[Vec<u8>] {
        &self.log
    }
}

impl SpiBus for MockSpi {
    fn transfer(&mut self, tx: &[u8], rx: &mut [u8]) -> Result<(), BusError> {
        if tx.len() != rx.len() {
            return Err(BusError::SizeMismatch {
                expected: tx.len(),
                actual: rx.len(),
            });
        }
        for (i, byte) in rx.iter_mut().enumerate() {
            *byte = self.response.get(i).copied().unwrap_or(0);
        }
        self.log.push(tx.to_vec());
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// DMA buffer
// ═══════════════════════════════════════════════════════════════════════

/// Physically contiguous DMA buffer.
///
/// Represents a buffer suitable for DMA transfers — aligned and
/// at a known physical address.
#[derive(Debug)]
pub struct DmaBuffer {
    /// Buffer data.
    data: Vec<u8>,
    /// Simulated physical address.
    phys_addr: u64,
    /// Alignment requirement.
    alignment: usize,
}

impl DmaBuffer {
    /// Allocate a DMA buffer of the given size at a simulated physical address.
    pub fn new(size: usize, phys_addr: u64, alignment: usize) -> Self {
        Self {
            data: vec![0u8; size],
            phys_addr,
            alignment,
        }
    }

    /// Get the buffer size.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the physical address.
    pub fn phys_addr(&self) -> u64 {
        self.phys_addr
    }

    /// Get the alignment.
    pub fn alignment(&self) -> usize {
        self.alignment
    }

    /// Get a slice of the buffer data.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Fill the buffer with a value.
    pub fn fill(&mut self, value: u8) {
        self.data.fill(value);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i2c_mock_write_and_read() {
        let mut i2c = MockI2c::new();
        i2c.write_byte(0x48, 0x00, 0x42).unwrap();
        assert_eq!(i2c.get_register(0x48, 0x00), Some(0x42));

        let value = i2c.read_byte(0x48, 0x00).unwrap();
        assert_eq!(value, 0x42);
    }

    #[test]
    fn i2c_mock_preset_register() {
        let mut i2c = MockI2c::new();
        i2c.set_register(0x68, 0x75, 0x71); // MPU-6050 WHO_AM_I

        let who_am_i = i2c.read_byte(0x68, 0x75).unwrap();
        assert_eq!(who_am_i, 0x71);
    }

    #[test]
    fn i2c_mock_multi_byte() {
        let mut i2c = MockI2c::new();
        i2c.write(0x50, 0x00, &[0xAA, 0xBB, 0xCC]).unwrap();

        let mut buf = [0u8; 3];
        i2c.read(0x50, 0x00, &mut buf).unwrap();
        assert_eq!(buf, [0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn spi_mock_transfer() {
        let mut spi = MockSpi::new();
        spi.set_response(&[0xDE, 0xAD]);

        let tx = [0x01, 0x02];
        let mut rx = [0u8; 2];
        spi.transfer(&tx, &mut rx).unwrap();

        assert_eq!(rx, [0xDE, 0xAD]);
        assert_eq!(spi.log(), &[vec![0x01, 0x02]]);
    }

    #[test]
    fn spi_mock_write() {
        let mut spi = MockSpi::new();
        SpiBus::write(&mut spi, &[0xFF, 0x00]).unwrap();
        assert_eq!(spi.log().len(), 1);
    }

    #[test]
    fn dma_buffer_alloc() {
        let mut dma = DmaBuffer::new(4096, 0x0010_0000, 4096);
        assert_eq!(dma.size(), 4096);
        assert_eq!(dma.phys_addr(), 0x0010_0000);
        assert_eq!(dma.alignment(), 4096);

        dma.fill(0xAA);
        assert!(dma.as_slice().iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn spi_size_mismatch() {
        let mut spi = MockSpi::new();
        let tx = [0x01, 0x02];
        let mut rx = [0u8; 3]; // Wrong size
        let err = spi.transfer(&tx, &mut rx).unwrap_err();
        assert_eq!(
            err,
            BusError::SizeMismatch {
                expected: 2,
                actual: 3
            }
        );
    }
}
