//! VirtIO block device simulation for FajarOS.
//!
//! Provides a simulated VirtIO block device for interpreter testing.
//! Uses in-memory storage — does NOT touch real hardware.

use std::collections::HashMap;

/// VirtIO device types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioDeviceType {
    /// Network device.
    Net = 1,
    /// Block device.
    Block = 2,
    /// Console device.
    Console = 3,
    /// GPU device.
    Gpu = 16,
}

/// VirtIO device status flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device reset.
    Reset = 0,
    /// Guest OS has found the device.
    Acknowledge = 1,
    /// Guest OS knows how to drive the device.
    Driver = 2,
    /// Feature negotiation complete.
    FeaturesOk = 8,
    /// Driver is ready.
    DriverOk = 4,
}

/// Errors from VirtIO operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum VirtioError {
    /// Device not found.
    #[error("VirtIO device not found: {0}")]
    DeviceNotFound(String),
    /// Sector out of range.
    #[error("sector {sector} out of range (capacity: {capacity})")]
    SectorOutOfRange {
        /// Requested sector.
        sector: u64,
        /// Device capacity in sectors.
        capacity: u64,
    },
    /// Device not initialized.
    #[error("VirtIO device not initialized")]
    NotInitialized,
    /// Invalid buffer size.
    #[error("invalid buffer size: expected {expected}, got {got}")]
    InvalidBufferSize {
        /// Expected size.
        expected: usize,
        /// Actual size.
        got: usize,
    },
}

/// Simulated VirtIO block device.
///
/// Stores sectors as 512-byte blocks in a HashMap.
#[derive(Debug)]
pub struct VirtioBlockDevice {
    /// Device ID.
    pub id: u32,
    /// Total capacity in sectors (512 bytes each).
    pub capacity: u64,
    /// Block size in bytes (always 512 for simulation).
    pub block_size: u32,
    /// Sector storage.
    sectors: HashMap<u64, Vec<u8>>,
    /// Device status.
    pub status: u8,
    /// Whether device is initialized.
    pub initialized: bool,
    /// Simple LRU cache (sector -> data).
    cache: HashMap<u64, Vec<u8>>,
    /// Cache capacity.
    cache_capacity: usize,
}

/// Sector size in bytes.
pub const SECTOR_SIZE: usize = 512;

impl VirtioBlockDevice {
    /// Creates a new VirtIO block device with given capacity (in sectors).
    pub fn new(id: u32, capacity: u64) -> Self {
        Self {
            id,
            capacity,
            block_size: SECTOR_SIZE as u32,
            sectors: HashMap::new(),
            status: 0,
            initialized: false,
            cache: HashMap::new(),
            cache_capacity: 64,
        }
    }

    /// Initializes the device (negotiate features, setup queues).
    pub fn init(&mut self) -> Result<(), VirtioError> {
        self.status = DeviceStatus::Acknowledge as u8
            | DeviceStatus::Driver as u8
            | DeviceStatus::FeaturesOk as u8
            | DeviceStatus::DriverOk as u8;
        self.initialized = true;
        Ok(())
    }

    /// Reads a sector into the provided buffer.
    pub fn read_sector(&mut self, sector: u64, buf: &mut [u8]) -> Result<(), VirtioError> {
        if !self.initialized {
            return Err(VirtioError::NotInitialized);
        }
        if sector >= self.capacity {
            return Err(VirtioError::SectorOutOfRange {
                sector,
                capacity: self.capacity,
            });
        }
        if buf.len() != SECTOR_SIZE {
            return Err(VirtioError::InvalidBufferSize {
                expected: SECTOR_SIZE,
                got: buf.len(),
            });
        }

        // Check cache first
        if let Some(cached) = self.cache.get(&sector) {
            buf.copy_from_slice(cached);
            return Ok(());
        }

        // Read from storage
        if let Some(data) = self.sectors.get(&sector) {
            buf.copy_from_slice(data);
        } else {
            // Unwritten sector returns zeros
            buf.fill(0);
        }

        // Update cache
        if self.cache.len() >= self.cache_capacity {
            // Evict oldest entry (simple strategy)
            if let Some(&key) = self.cache.keys().next() {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(sector, buf.to_vec());

        Ok(())
    }

    /// Writes a sector from the provided buffer.
    pub fn write_sector(&mut self, sector: u64, buf: &[u8]) -> Result<(), VirtioError> {
        if !self.initialized {
            return Err(VirtioError::NotInitialized);
        }
        if sector >= self.capacity {
            return Err(VirtioError::SectorOutOfRange {
                sector,
                capacity: self.capacity,
            });
        }
        if buf.len() != SECTOR_SIZE {
            return Err(VirtioError::InvalidBufferSize {
                expected: SECTOR_SIZE,
                got: buf.len(),
            });
        }

        self.sectors.insert(sector, buf.to_vec());
        // Invalidate cache entry
        self.cache.remove(&sector);

        Ok(())
    }

    /// Returns device capacity in sectors.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Returns device capacity in bytes.
    pub fn capacity_bytes(&self) -> u64 {
        self.capacity * SECTOR_SIZE as u64
    }
}

/// VirtIO device manager — holds all VirtIO devices.
#[derive(Debug)]
pub struct VirtioManager {
    /// Block devices.
    pub block_devices: Vec<VirtioBlockDevice>,
}

impl VirtioManager {
    /// Creates a new VirtIO manager with no devices.
    pub fn new() -> Self {
        Self {
            block_devices: Vec::new(),
        }
    }

    /// Adds a block device and returns its index.
    pub fn add_block_device(&mut self, capacity_sectors: u64) -> usize {
        let id = self.block_devices.len() as u32;
        self.block_devices
            .push(VirtioBlockDevice::new(id, capacity_sectors));
        id as usize
    }

    /// Gets a mutable reference to a block device.
    pub fn block_device(&mut self, index: usize) -> Option<&mut VirtioBlockDevice> {
        self.block_devices.get_mut(index)
    }
}

impl Default for VirtioManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s25_1_virtio_device_creation() {
        let dev = VirtioBlockDevice::new(0, 2048);
        assert_eq!(dev.capacity, 2048);
        assert_eq!(dev.block_size, 512);
        assert!(!dev.initialized);
    }

    #[test]
    fn s25_2_virtio_device_init() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        assert!(dev.init().is_ok());
        assert!(dev.initialized);
        assert_ne!(dev.status, 0);
    }

    #[test]
    fn s25_3_virtio_read_uninitialized() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        let mut buf = vec![0u8; SECTOR_SIZE];
        assert!(matches!(
            dev.read_sector(0, &mut buf),
            Err(VirtioError::NotInitialized)
        ));
    }

    #[test]
    fn s25_4_virtio_write_read_roundtrip() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        dev.init().unwrap();

        let mut write_buf = vec![0u8; SECTOR_SIZE];
        write_buf[0] = 0xDE;
        write_buf[1] = 0xAD;
        write_buf[511] = 0xFF;
        dev.write_sector(5, &write_buf).unwrap();

        let mut read_buf = vec![0u8; SECTOR_SIZE];
        dev.read_sector(5, &mut read_buf).unwrap();
        assert_eq!(read_buf[0], 0xDE);
        assert_eq!(read_buf[1], 0xAD);
        assert_eq!(read_buf[511], 0xFF);
    }

    #[test]
    fn s25_5_virtio_sector_out_of_range() {
        let mut dev = VirtioBlockDevice::new(0, 100);
        dev.init().unwrap();
        let mut buf = vec![0u8; SECTOR_SIZE];
        assert!(matches!(
            dev.read_sector(100, &mut buf),
            Err(VirtioError::SectorOutOfRange { .. })
        ));
    }

    #[test]
    fn s25_6_virtio_unwritten_sector_returns_zeros() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        dev.init().unwrap();
        let mut buf = vec![0xFFu8; SECTOR_SIZE];
        dev.read_sector(42, &mut buf).unwrap();
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn s25_7_virtio_cache_hit() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        dev.init().unwrap();

        let write_buf = vec![0xABu8; SECTOR_SIZE];
        dev.write_sector(10, &write_buf).unwrap();

        // First read populates cache
        let mut buf1 = vec![0u8; SECTOR_SIZE];
        dev.read_sector(10, &mut buf1).unwrap();
        assert_eq!(buf1[0], 0xAB);

        // Second read hits cache
        let mut buf2 = vec![0u8; SECTOR_SIZE];
        dev.read_sector(10, &mut buf2).unwrap();
        assert_eq!(buf2[0], 0xAB);
    }

    #[test]
    fn s25_8_virtio_manager() {
        let mut mgr = VirtioManager::new();
        let idx = mgr.add_block_device(2048);
        assert_eq!(idx, 0);

        let dev = mgr.block_device(idx).unwrap();
        dev.init().unwrap();
        assert_eq!(dev.capacity_bytes(), 2048 * 512);
    }

    #[test]
    fn s25_9_virtio_invalid_buffer_size() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        dev.init().unwrap();
        let mut small_buf = vec![0u8; 256];
        assert!(matches!(
            dev.read_sector(0, &mut small_buf),
            Err(VirtioError::InvalidBufferSize { .. })
        ));
    }

    #[test]
    fn s25_10_virtio_multiple_sectors() {
        let mut dev = VirtioBlockDevice::new(0, 1024);
        dev.init().unwrap();

        for i in 0..10u64 {
            let buf = vec![i as u8; SECTOR_SIZE];
            dev.write_sector(i, &buf).unwrap();
        }

        for i in 0..10u64 {
            let mut buf = vec![0u8; SECTOR_SIZE];
            dev.read_sector(i, &mut buf).unwrap();
            assert_eq!(buf[0], i as u8);
        }
    }
}
