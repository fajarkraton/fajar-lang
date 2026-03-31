//! Hardware Abstraction Layer v2 — advanced driver simulations.
//!
//! Sprint N5: Provides simulated hardware drivers for USB 3.0, PCIe,
//! AHCI/SATA, audio, ACPI, multi-monitor, keyboard layouts, mouse,
//! and real-time clock. All simulated — no real hardware access.
//!
//! # Architecture
//!
//! ```text
//! Usb3Driver         — xHCI controller simulation
//! PcieEnumV2         — full BAR mapping, MSI-X
//! AhciDriver         — SATA read/write sectors
//! AudioDriver        — HD Audio buffer management
//! AcpiSupport        — power states S0-S5
//! MultiMonitor       — multi-head display config
//! KeyboardLayout     — international layout mapping
//! MouseDriver        — PS/2 + USB mouse simulation
//! RtcClock           — real-time clock
//! HardwareTestSuite  — test all drivers
//! ```

use std::collections::HashMap;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from HAL v2 operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HalV2Error {
    /// USB device not found.
    #[error("USB device not found: vendor={vendor:#06x} product={product:#06x}")]
    UsbDeviceNotFound {
        /// Vendor ID.
        vendor: u16,
        /// Product ID.
        product: u16,
    },

    /// USB endpoint error.
    #[error("USB endpoint {endpoint} error: {reason}")]
    UsbEndpointError {
        /// Endpoint number.
        endpoint: u8,
        /// Error reason.
        reason: String,
    },

    /// PCIe BAR mapping error.
    #[error("PCIe BAR{bar} mapping error for {bdf}: {reason}")]
    PcieBarError {
        /// Bus:Device.Function string.
        bdf: String,
        /// BAR index.
        bar: u8,
        /// Error reason.
        reason: String,
    },

    /// AHCI/SATA error.
    #[error("AHCI port {port} error: {reason}")]
    AhciError {
        /// Port number.
        port: u8,
        /// Error reason.
        reason: String,
    },

    /// Audio driver error.
    #[error("audio error: {reason}")]
    AudioError {
        /// Error reason.
        reason: String,
    },

    /// ACPI error.
    #[error("ACPI error: {reason}")]
    AcpiError {
        /// Error reason.
        reason: String,
    },

    /// Display error.
    #[error("display error: {reason}")]
    DisplayError {
        /// Error reason.
        reason: String,
    },

    /// Keyboard error.
    #[error("keyboard error: {reason}")]
    KeyboardError {
        /// Error reason.
        reason: String,
    },

    /// Mouse error.
    #[error("mouse error: {reason}")]
    MouseError {
        /// Error reason.
        reason: String,
    },

    /// RTC error.
    #[error("RTC error: {reason}")]
    RtcError {
        /// Error reason.
        reason: String,
    },
}

/// Result type for HAL v2 operations.
pub type HalV2Result<T> = Result<T, HalV2Error>;

// ═══════════════════════════════════════════════════════════════════════
// USB 3.0 Driver (xHCI)
// ═══════════════════════════════════════════════════════════════════════

/// USB device speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    /// Low speed (1.5 Mbps).
    Low,
    /// Full speed (12 Mbps).
    Full,
    /// High speed (480 Mbps, USB 2.0).
    High,
    /// Super speed (5 Gbps, USB 3.0).
    Super,
    /// Super speed+ (10 Gbps, USB 3.1).
    SuperPlus,
}

/// A USB endpoint descriptor.
#[derive(Debug, Clone)]
pub struct UsbEndpoint {
    /// Endpoint number (0-15).
    pub number: u8,
    /// Direction: true = IN (device to host), false = OUT.
    pub direction_in: bool,
    /// Maximum packet size.
    pub max_packet_size: u16,
}

/// A USB device descriptor.
#[derive(Debug, Clone)]
pub struct UsbDevice {
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Device class.
    pub class: u8,
    /// Device name.
    pub name: String,
    /// Speed.
    pub speed: UsbSpeed,
    /// Endpoints.
    pub endpoints: Vec<UsbEndpoint>,
    /// Assigned slot number.
    pub slot: u8,
}

/// Simulated xHCI (USB 3.0) controller.
///
/// Manages device enumeration, endpoint configuration, and data transfer.
#[derive(Debug, Clone)]
pub struct Usb3Driver {
    /// Connected devices by slot number.
    devices: HashMap<u8, UsbDevice>,
    /// Next slot to assign.
    next_slot: u8,
    /// Maximum slots (xHCI typically supports 256).
    max_slots: u8,
    /// Transfer log: (slot, endpoint, bytes).
    transfer_log: Vec<(u8, u8, usize)>,
}

impl Usb3Driver {
    /// Creates a new xHCI controller with the given max slot count.
    pub fn new(max_slots: u8) -> Self {
        Self {
            devices: HashMap::new(),
            next_slot: 1, // Slot 0 is reserved.
            max_slots,
            transfer_log: Vec::new(),
        }
    }

    /// Enumerates (connects) a USB device.
    pub fn enumerate_device(
        &mut self,
        vendor_id: u16,
        product_id: u16,
        name: &str,
        speed: UsbSpeed,
        endpoints: Vec<UsbEndpoint>,
    ) -> HalV2Result<u8> {
        if self.next_slot >= self.max_slots {
            return Err(HalV2Error::UsbEndpointError {
                endpoint: 0,
                reason: "no free slots".to_string(),
            });
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.devices.insert(
            slot,
            UsbDevice {
                vendor_id,
                product_id,
                class: 0,
                name: name.to_string(),
                speed,
                endpoints,
                slot,
            },
        );
        Ok(slot)
    }

    /// Configures an endpoint for a device.
    pub fn configure_endpoint(&mut self, slot: u8, endpoint: UsbEndpoint) -> HalV2Result<()> {
        let device = self
            .devices
            .get_mut(&slot)
            .ok_or(HalV2Error::UsbDeviceNotFound {
                vendor: 0,
                product: 0,
            })?;
        device.endpoints.push(endpoint);
        Ok(())
    }

    /// Simulates a data transfer on an endpoint.
    pub fn transfer(&mut self, slot: u8, endpoint: u8, data_len: usize) -> HalV2Result<()> {
        if !self.devices.contains_key(&slot) {
            return Err(HalV2Error::UsbDeviceNotFound {
                vendor: 0,
                product: 0,
            });
        }
        self.transfer_log.push((slot, endpoint, data_len));
        Ok(())
    }

    /// Returns the number of connected devices.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Returns the transfer log.
    pub fn transfer_count(&self) -> usize {
        self.transfer_log.len()
    }

    /// Disconnects a device by slot.
    pub fn disconnect(&mut self, slot: u8) -> HalV2Result<()> {
        self.devices
            .remove(&slot)
            .ok_or(HalV2Error::UsbDeviceNotFound {
                vendor: 0,
                product: 0,
            })?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PCIe Enumeration v2
// ═══════════════════════════════════════════════════════════════════════

/// A PCIe Base Address Register (BAR) mapping.
#[derive(Debug, Clone)]
pub struct PcieBar {
    /// BAR index (0-5).
    pub index: u8,
    /// Base address.
    pub base: u64,
    /// Size in bytes.
    pub size: u64,
    /// Is memory-mapped (vs I/O).
    pub is_memory: bool,
    /// Is 64-bit BAR.
    pub is_64bit: bool,
    /// Is prefetchable.
    pub prefetchable: bool,
}

/// MSI-X table entry.
#[derive(Debug, Clone)]
pub struct MsiXEntry {
    /// Vector index.
    pub vector: u16,
    /// Message address.
    pub address: u64,
    /// Message data.
    pub data: u32,
    /// Masked.
    pub masked: bool,
}

/// A PCIe device.
#[derive(Debug, Clone)]
pub struct PcieDevice {
    /// Bus number.
    pub bus: u8,
    /// Device number.
    pub device: u8,
    /// Function number.
    pub function: u8,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Device ID.
    pub device_id: u16,
    /// Device class.
    pub class: u8,
    /// Subclass.
    pub subclass: u8,
    /// BAR mappings.
    pub bars: Vec<PcieBar>,
    /// MSI-X entries.
    pub msix_entries: Vec<MsiXEntry>,
}

impl PcieDevice {
    /// Returns the BDF (Bus:Device.Function) string.
    pub fn bdf(&self) -> String {
        format!("{:02x}:{:02x}.{}", self.bus, self.device, self.function)
    }
}

/// Full PCIe enumeration with BAR mapping and MSI-X support.
#[derive(Debug, Clone)]
pub struct PcieEnumV2 {
    /// Enumerated devices.
    devices: Vec<PcieDevice>,
}

impl PcieEnumV2 {
    /// Creates a new PCIe enumerator.
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Adds a discovered device.
    pub fn add_device(&mut self, device: PcieDevice) {
        self.devices.push(device);
    }

    /// Maps a BAR for a device.
    pub fn map_bar(&mut self, bus: u8, dev: u8, func: u8, bar: PcieBar) -> HalV2Result<()> {
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.bus == bus && d.device == dev && d.function == func)
            .ok_or(HalV2Error::PcieBarError {
                bdf: format!("{:02x}:{:02x}.{}", bus, dev, func),
                bar: bar.index,
                reason: "device not found".to_string(),
            })?;
        device.bars.push(bar);
        Ok(())
    }

    /// Configures MSI-X for a device.
    pub fn configure_msix(
        &mut self,
        bus: u8,
        dev: u8,
        func: u8,
        entries: Vec<MsiXEntry>,
    ) -> HalV2Result<()> {
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.bus == bus && d.device == dev && d.function == func)
            .ok_or(HalV2Error::PcieBarError {
                bdf: format!("{:02x}:{:02x}.{}", bus, dev, func),
                bar: 0,
                reason: "device not found for MSI-X".to_string(),
            })?;
        device.msix_entries = entries;
        Ok(())
    }

    /// Returns all enumerated devices.
    pub fn devices(&self) -> &[PcieDevice] {
        &self.devices
    }

    /// Returns the device count.
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
}

impl Default for PcieEnumV2 {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// AHCI Driver (SATA)
// ═══════════════════════════════════════════════════════════════════════

/// AHCI port status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AhciPortStatus {
    /// No device connected.
    Empty,
    /// Device present, not initialized.
    Present,
    /// Device initialized and ready.
    Ready,
    /// Device in error state.
    Error,
}

/// A simulated AHCI port (SATA connection).
#[derive(Debug, Clone)]
pub struct AhciPort {
    /// Port number.
    pub port: u8,
    /// Status.
    pub status: AhciPortStatus,
    /// Device model string.
    pub model: String,
    /// Capacity in sectors (each 512 bytes).
    pub capacity_sectors: u64,
    /// Simulated storage (sector -> data).
    storage: HashMap<u64, Vec<u8>>,
}

/// Simulated AHCI (SATA) controller.
///
/// Supports read/write of 512-byte sectors.
#[derive(Debug, Clone)]
pub struct AhciDriver {
    /// Ports (up to 32).
    ports: Vec<AhciPort>,
    /// Sector size in bytes.
    pub sector_size: usize,
}

impl AhciDriver {
    /// Creates a new AHCI controller.
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            sector_size: 512,
        }
    }

    /// Adds a simulated SATA disk on a port.
    pub fn add_disk(&mut self, port: u8, model: &str, capacity_sectors: u64) {
        self.ports.push(AhciPort {
            port,
            status: AhciPortStatus::Ready,
            model: model.to_string(),
            capacity_sectors,
            storage: HashMap::new(),
        });
    }

    /// Writes a sector.
    pub fn write_sector(&mut self, port: u8, lba: u64, data: Vec<u8>) -> HalV2Result<()> {
        let p = self
            .ports
            .iter_mut()
            .find(|p| p.port == port)
            .ok_or(HalV2Error::AhciError {
                port,
                reason: "port not found".to_string(),
            })?;
        if p.status != AhciPortStatus::Ready {
            return Err(HalV2Error::AhciError {
                port,
                reason: "port not ready".to_string(),
            });
        }
        if lba >= p.capacity_sectors {
            return Err(HalV2Error::AhciError {
                port,
                reason: format!("LBA {} out of range (max {})", lba, p.capacity_sectors - 1),
            });
        }
        if data.len() != self.sector_size {
            return Err(HalV2Error::AhciError {
                port,
                reason: format!(
                    "data size {} != sector size {}",
                    data.len(),
                    self.sector_size
                ),
            });
        }
        p.storage.insert(lba, data);
        Ok(())
    }

    /// Reads a sector.
    pub fn read_sector(&self, port: u8, lba: u64) -> HalV2Result<Vec<u8>> {
        let p = self
            .ports
            .iter()
            .find(|p| p.port == port)
            .ok_or(HalV2Error::AhciError {
                port,
                reason: "port not found".to_string(),
            })?;
        if p.status != AhciPortStatus::Ready {
            return Err(HalV2Error::AhciError {
                port,
                reason: "port not ready".to_string(),
            });
        }
        Ok(p.storage
            .get(&lba)
            .cloned()
            .unwrap_or_else(|| vec![0; self.sector_size]))
    }

    /// Returns the number of active ports.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }
}

impl Default for AhciDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Audio Driver (HD Audio)
// ═══════════════════════════════════════════════════════════════════════

/// Audio sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// 16-bit signed PCM.
    Pcm16,
    /// 24-bit signed PCM.
    Pcm24,
    /// 32-bit float.
    Float32,
}

/// Audio buffer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBufferState {
    /// Buffer is empty and ready for data.
    Empty,
    /// Buffer has data ready for playback.
    Ready,
    /// Buffer is currently playing.
    Playing,
    /// Playback complete.
    Done,
}

/// A simulated audio buffer.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Buffer ID.
    pub id: u32,
    /// Audio data.
    pub data: Vec<u8>,
    /// State.
    pub state: AudioBufferState,
    /// Sample rate (Hz).
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u8,
    /// Format.
    pub format: AudioFormat,
}

/// Simulated HD Audio controller.
#[derive(Debug, Clone)]
pub struct AudioDriver {
    /// Audio buffers.
    buffers: Vec<AudioBuffer>,
    /// Next buffer ID.
    next_id: u32,
    /// Master volume (0-100).
    pub volume: u8,
    /// Muted.
    pub muted: bool,
}

impl AudioDriver {
    /// Creates a new audio driver.
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            next_id: 1,
            volume: 80,
            muted: false,
        }
    }

    /// Creates an audio buffer with the given parameters.
    pub fn create_buffer(&mut self, sample_rate: u32, channels: u8, format: AudioFormat) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.buffers.push(AudioBuffer {
            id,
            data: Vec::new(),
            state: AudioBufferState::Empty,
            sample_rate,
            channels,
            format,
        });
        id
    }

    /// Fills a buffer with audio data.
    pub fn fill_buffer(&mut self, id: u32, data: Vec<u8>) -> HalV2Result<()> {
        let buf = self
            .buffers
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(HalV2Error::AudioError {
                reason: format!("buffer {} not found", id),
            })?;
        buf.data = data;
        buf.state = AudioBufferState::Ready;
        Ok(())
    }

    /// Starts playback of a buffer.
    pub fn play(&mut self, id: u32) -> HalV2Result<()> {
        let buf = self
            .buffers
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(HalV2Error::AudioError {
                reason: format!("buffer {} not found", id),
            })?;
        if buf.state != AudioBufferState::Ready {
            return Err(HalV2Error::AudioError {
                reason: format!("buffer {} not ready", id),
            });
        }
        buf.state = AudioBufferState::Playing;
        Ok(())
    }

    /// Completes playback of a buffer (simulated).
    pub fn complete_playback(&mut self, id: u32) -> HalV2Result<()> {
        let buf = self
            .buffers
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or(HalV2Error::AudioError {
                reason: format!("buffer {} not found", id),
            })?;
        buf.state = AudioBufferState::Done;
        Ok(())
    }

    /// Returns the number of buffers.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Sets the master volume.
    pub fn set_volume(&mut self, vol: u8) {
        self.volume = vol.min(100);
    }
}

impl Default for AudioDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ACPI Support
// ═══════════════════════════════════════════════════════════════════════

/// ACPI sleep/power states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiState {
    /// S0: Working (normal operation).
    S0Working,
    /// S1: Sleep (CPU stopped, RAM refreshed).
    S1Sleep,
    /// S2: Sleep (CPU off, RAM refreshed).
    S2Sleep,
    /// S3: Suspend to RAM (STR).
    S3SuspendToRam,
    /// S4: Suspend to Disk (hibernate).
    S4Hibernate,
    /// S5: Soft Off (mechanical off equivalent).
    S5SoftOff,
}

/// Simulated ACPI power management.
#[derive(Debug, Clone)]
pub struct AcpiSupport {
    /// Current power state.
    current_state: AcpiState,
    /// Supported states.
    supported: Vec<AcpiState>,
    /// State transition log.
    transitions: Vec<(AcpiState, AcpiState)>,
}

impl AcpiSupport {
    /// Creates a new ACPI support module with all states supported.
    pub fn new() -> Self {
        Self {
            current_state: AcpiState::S0Working,
            supported: vec![
                AcpiState::S0Working,
                AcpiState::S1Sleep,
                AcpiState::S2Sleep,
                AcpiState::S3SuspendToRam,
                AcpiState::S4Hibernate,
                AcpiState::S5SoftOff,
            ],
            transitions: Vec::new(),
        }
    }

    /// Transitions to a new power state.
    pub fn transition_to(&mut self, state: AcpiState) -> HalV2Result<()> {
        if !self.supported.contains(&state) {
            return Err(HalV2Error::AcpiError {
                reason: format!("state {:?} not supported", state),
            });
        }
        let old = self.current_state;
        self.current_state = state;
        self.transitions.push((old, state));
        Ok(())
    }

    /// Returns the current power state.
    pub fn current_state(&self) -> AcpiState {
        self.current_state
    }

    /// Performs a soft shutdown (transition to S5).
    pub fn shutdown(&mut self) -> HalV2Result<()> {
        self.transition_to(AcpiState::S5SoftOff)
    }

    /// Performs a sleep (transition to S3).
    pub fn sleep(&mut self) -> HalV2Result<()> {
        self.transition_to(AcpiState::S3SuspendToRam)
    }

    /// Wakes up (transition to S0).
    pub fn wake(&mut self) -> HalV2Result<()> {
        self.transition_to(AcpiState::S0Working)
    }

    /// Returns the number of state transitions.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }
}

impl Default for AcpiSupport {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Multi-Monitor
// ═══════════════════════════════════════════════════════════════════════

/// Display resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_hz: u32,
}

/// A display output configuration.
#[derive(Debug, Clone)]
pub struct DisplayOutput {
    /// Output name (e.g., "HDMI-1", "DP-2").
    pub name: String,
    /// Current resolution.
    pub resolution: Resolution,
    /// Position offset (x, y) in the virtual desktop.
    pub position: (i32, i32),
    /// Is this the primary display.
    pub primary: bool,
    /// Is enabled.
    pub enabled: bool,
}

/// Multi-head display configuration manager.
#[derive(Debug, Clone)]
pub struct MultiMonitor {
    /// Display outputs.
    outputs: Vec<DisplayOutput>,
}

impl MultiMonitor {
    /// Creates a new multi-monitor manager.
    pub fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }

    /// Adds a display output.
    pub fn add_output(&mut self, output: DisplayOutput) {
        self.outputs.push(output);
    }

    /// Sets the resolution for an output.
    pub fn set_resolution(&mut self, name: &str, resolution: Resolution) -> HalV2Result<()> {
        let output =
            self.outputs
                .iter_mut()
                .find(|o| o.name == name)
                .ok_or(HalV2Error::DisplayError {
                    reason: format!("output '{}' not found", name),
                })?;
        output.resolution = resolution;
        Ok(())
    }

    /// Sets the position for an output.
    pub fn set_position(&mut self, name: &str, x: i32, y: i32) -> HalV2Result<()> {
        let output =
            self.outputs
                .iter_mut()
                .find(|o| o.name == name)
                .ok_or(HalV2Error::DisplayError {
                    reason: format!("output '{}' not found", name),
                })?;
        output.position = (x, y);
        Ok(())
    }

    /// Sets the primary display.
    pub fn set_primary(&mut self, name: &str) -> HalV2Result<()> {
        let found = self.outputs.iter().any(|o| o.name == name);
        if !found {
            return Err(HalV2Error::DisplayError {
                reason: format!("output '{}' not found", name),
            });
        }
        for output in &mut self.outputs {
            output.primary = output.name == name;
        }
        Ok(())
    }

    /// Returns the number of outputs.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }

    /// Returns the virtual desktop total size (bounding box).
    pub fn virtual_desktop_size(&self) -> (u32, u32) {
        let mut max_x: i32 = 0;
        let mut max_y: i32 = 0;
        for output in &self.outputs {
            if !output.enabled {
                continue;
            }
            let right = output.position.0 + output.resolution.width as i32;
            let bottom = output.position.1 + output.resolution.height as i32;
            if right > max_x {
                max_x = right;
            }
            if bottom > max_y {
                max_y = bottom;
            }
        }
        (max_x as u32, max_y as u32)
    }
}

impl Default for MultiMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Keyboard Layout
// ═══════════════════════════════════════════════════════════════════════

/// Keyboard layout identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutId {
    /// US English (QWERTY).
    Us,
    /// German (QWERTZ).
    De,
    /// Japanese (QWERTY + IME).
    Jp,
    /// Indonesian (QWERTY).
    Id,
}

/// International keyboard layout mapping.
///
/// Maps scan codes to characters based on the active layout.
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    /// Active layout.
    active: LayoutId,
    /// Layout mappings: layout -> (scancode -> character).
    layouts: HashMap<LayoutId, HashMap<u8, char>>,
}

impl KeyboardLayout {
    /// Creates a new keyboard layout manager with default mappings.
    pub fn new() -> Self {
        let mut layouts = HashMap::new();

        // US English layout (simplified - top row).
        let mut us = HashMap::new();
        for (i, ch) in "qwertyuiop".chars().enumerate() {
            us.insert(16 + i as u8, ch);
        }
        for (i, ch) in "asdfghjkl".chars().enumerate() {
            us.insert(30 + i as u8, ch);
        }
        for (i, ch) in "zxcvbnm".chars().enumerate() {
            us.insert(44 + i as u8, ch);
        }
        layouts.insert(LayoutId::Us, us);

        // German layout (QWERTZ — Z and Y swapped).
        let mut de = HashMap::new();
        for (i, ch) in "qwertzuiop".chars().enumerate() {
            de.insert(16 + i as u8, ch);
        }
        for (i, ch) in "asdfghjkl".chars().enumerate() {
            de.insert(30 + i as u8, ch);
        }
        for (i, ch) in "yxcvbnm".chars().enumerate() {
            de.insert(44 + i as u8, ch);
        }
        layouts.insert(LayoutId::De, de);

        // Japanese layout (same as US base, IME would handle conversion).
        layouts.insert(
            LayoutId::Jp,
            layouts.get(&LayoutId::Us).cloned().unwrap_or_default(),
        );

        // Indonesian layout (same as US).
        layouts.insert(
            LayoutId::Id,
            layouts.get(&LayoutId::Us).cloned().unwrap_or_default(),
        );

        Self {
            active: LayoutId::Us,
            layouts,
        }
    }

    /// Sets the active layout.
    pub fn set_layout(&mut self, layout: LayoutId) {
        self.active = layout;
    }

    /// Returns the active layout.
    pub fn active_layout(&self) -> LayoutId {
        self.active
    }

    /// Maps a scan code to a character using the active layout.
    pub fn map_scancode(&self, scancode: u8) -> Option<char> {
        self.layouts
            .get(&self.active)
            .and_then(|m| m.get(&scancode).copied())
    }

    /// Returns the number of supported layouts.
    pub fn layout_count(&self) -> usize {
        self.layouts.len()
    }
}

impl Default for KeyboardLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Mouse Driver
// ═══════════════════════════════════════════════════════════════════════

/// Mouse button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseButtons {
    /// Left button pressed.
    pub left: bool,
    /// Right button pressed.
    pub right: bool,
    /// Middle button pressed.
    pub middle: bool,
}

impl MouseButtons {
    /// No buttons pressed.
    pub fn none() -> Self {
        Self {
            left: false,
            right: false,
            middle: false,
        }
    }
}

impl Default for MouseButtons {
    fn default() -> Self {
        Self::none()
    }
}

/// Mouse interface type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseInterface {
    /// PS/2 mouse.
    Ps2,
    /// USB mouse.
    Usb,
}

/// Simulated mouse driver (PS/2 + USB).
#[derive(Debug, Clone)]
pub struct MouseDriver {
    /// Current cursor X position.
    pub x: i32,
    /// Current cursor Y position.
    pub y: i32,
    /// Button state.
    pub buttons: MouseButtons,
    /// Interface type.
    pub interface: MouseInterface,
    /// Screen bounds (width, height).
    bounds: (i32, i32),
    /// Event log: (dx, dy, buttons).
    event_log: Vec<(i32, i32, MouseButtons)>,
}

impl MouseDriver {
    /// Creates a new mouse driver.
    pub fn new(interface: MouseInterface, width: i32, height: i32) -> Self {
        Self {
            x: width / 2,
            y: height / 2,
            buttons: MouseButtons::none(),
            interface,
            bounds: (width, height),
            event_log: Vec::new(),
        }
    }

    /// Processes a mouse movement event.
    pub fn move_relative(&mut self, dx: i32, dy: i32) {
        self.x = (self.x + dx).clamp(0, self.bounds.0 - 1);
        self.y = (self.y + dy).clamp(0, self.bounds.1 - 1);
        self.event_log.push((dx, dy, self.buttons));
    }

    /// Sets button state.
    pub fn set_buttons(&mut self, buttons: MouseButtons) {
        self.buttons = buttons;
    }

    /// Returns the cursor position.
    pub fn position(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Returns the event count.
    pub fn event_count(&self) -> usize {
        self.event_log.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RTC Clock
// ═══════════════════════════════════════════════════════════════════════

/// Real-time clock time representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcTime {
    /// Year (e.g., 2026).
    pub year: u16,
    /// Month (1-12).
    pub month: u8,
    /// Day (1-31).
    pub day: u8,
    /// Hour (0-23).
    pub hour: u8,
    /// Minute (0-59).
    pub minute: u8,
    /// Second (0-59).
    pub second: u8,
}

impl RtcTime {
    /// Creates a new RTC time.
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Validates the time fields.
    pub fn is_valid(&self) -> bool {
        self.month >= 1
            && self.month <= 12
            && self.day >= 1
            && self.day <= 31
            && self.hour <= 23
            && self.minute <= 59
            && self.second <= 59
    }
}

impl std::fmt::Display for RtcTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Simulated real-time clock.
///
/// Provides wall-clock time at second precision.
#[derive(Debug, Clone)]
pub struct RtcClock {
    /// Current time.
    current: RtcTime,
    /// Total seconds elapsed since boot (simulated).
    uptime_seconds: u64,
}

impl RtcClock {
    /// Creates a new RTC clock with the given initial time.
    pub fn new(initial: RtcTime) -> HalV2Result<Self> {
        if !initial.is_valid() {
            return Err(HalV2Error::RtcError {
                reason: "invalid initial time".to_string(),
            });
        }
        Ok(Self {
            current: initial,
            uptime_seconds: 0,
        })
    }

    /// Advances the clock by one second.
    pub fn tick(&mut self) {
        self.uptime_seconds += 1;
        self.current.second += 1;
        if self.current.second >= 60 {
            self.current.second = 0;
            self.current.minute += 1;
            if self.current.minute >= 60 {
                self.current.minute = 0;
                self.current.hour += 1;
                if self.current.hour >= 24 {
                    self.current.hour = 0;
                    self.current.day += 1;
                    // Simplified: no month overflow handling for simulation.
                }
            }
        }
    }

    /// Returns the current time.
    pub fn read_time(&self) -> RtcTime {
        self.current
    }

    /// Sets the current time.
    pub fn set_time(&mut self, time: RtcTime) -> HalV2Result<()> {
        if !time.is_valid() {
            return Err(HalV2Error::RtcError {
                reason: "invalid time".to_string(),
            });
        }
        self.current = time;
        Ok(())
    }

    /// Returns the uptime in seconds.
    pub fn uptime(&self) -> u64 {
        self.uptime_seconds
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Hardware Test Suite
// ═══════════════════════════════════════════════════════════════════════

/// Result of a hardware driver test.
#[derive(Debug, Clone)]
pub struct DriverTestResult {
    /// Driver name.
    pub driver: String,
    /// Test name.
    pub test: String,
    /// Passed.
    pub passed: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Runs integration tests across all hardware drivers.
#[derive(Debug, Clone)]
pub struct HardwareTestSuite {
    /// Test results.
    results: Vec<DriverTestResult>,
}

impl HardwareTestSuite {
    /// Creates a new hardware test suite.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Runs all driver self-tests and returns the suite.
    pub fn run_all(&mut self) {
        // USB test.
        self.test_usb();
        // AHCI test.
        self.test_ahci();
        // Audio test.
        self.test_audio();
        // ACPI test.
        self.test_acpi();
        // RTC test.
        self.test_rtc();
        // Keyboard test.
        self.test_keyboard();
        // Mouse test.
        self.test_mouse();
    }

    /// Returns the test results.
    pub fn results(&self) -> &[DriverTestResult] {
        &self.results
    }

    /// Returns the number of passed tests.
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    /// Returns the total test count.
    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    fn record(&mut self, driver: &str, test: &str, result: Result<(), String>) {
        match result {
            Ok(()) => self.results.push(DriverTestResult {
                driver: driver.to_string(),
                test: test.to_string(),
                passed: true,
                error: None,
            }),
            Err(e) => self.results.push(DriverTestResult {
                driver: driver.to_string(),
                test: test.to_string(),
                passed: false,
                error: Some(e),
            }),
        }
    }

    fn test_usb(&mut self) {
        let mut usb = Usb3Driver::new(32);
        let result = usb
            .enumerate_device(0x1234, 0x5678, "test_device", UsbSpeed::Super, vec![])
            .map(|_| ())
            .map_err(|e| e.to_string());
        self.record("USB3", "enumerate", result);
    }

    fn test_ahci(&mut self) {
        let mut ahci = AhciDriver::new();
        ahci.add_disk(0, "TestDisk", 1000);
        let data = vec![0xAA; 512];
        let result = ahci.write_sector(0, 0, data).map_err(|e| e.to_string());
        self.record("AHCI", "write_sector", result);
    }

    fn test_audio(&mut self) {
        let mut audio = AudioDriver::new();
        let id = audio.create_buffer(44100, 2, AudioFormat::Pcm16);
        let result = audio
            .fill_buffer(id, vec![0; 1024])
            .map_err(|e| e.to_string());
        self.record("Audio", "fill_buffer", result);
    }

    fn test_acpi(&mut self) {
        let mut acpi = AcpiSupport::new();
        let result = acpi
            .sleep()
            .and_then(|()| acpi.wake())
            .map_err(|e| e.to_string());
        self.record("ACPI", "sleep_wake", result);
    }

    fn test_rtc(&mut self) {
        let result = RtcClock::new(RtcTime::new(2026, 3, 31, 12, 0, 0))
            .map(|_| ())
            .map_err(|e| e.to_string());
        self.record("RTC", "init", result);
    }

    fn test_keyboard(&mut self) {
        let kb = KeyboardLayout::new();
        let result = kb
            .map_scancode(16) // 'q' in US layout
            .map(|_| ())
            .ok_or_else(|| "scancode 16 not mapped".to_string());
        self.record("Keyboard", "scancode_map", result);
    }

    fn test_mouse(&mut self) {
        let mut mouse = MouseDriver::new(MouseInterface::Ps2, 1920, 1080);
        mouse.move_relative(10, 20);
        let (x, y) = mouse.position();
        let result = if x == 970 && y == 560 {
            Ok(())
        } else {
            Err(format!("unexpected position: ({}, {})", x, y))
        };
        self.record("Mouse", "move_relative", result);
    }
}

impl Default for HardwareTestSuite {
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

    // ── USB3 Driver ──

    #[test]
    fn usb3_enumerate_and_transfer() {
        let mut usb = Usb3Driver::new(32);
        let slot = usb
            .enumerate_device(0x1234, 0x5678, "keyboard", UsbSpeed::Full, vec![])
            .unwrap();
        assert!(usb.transfer(slot, 0, 8).is_ok());
        assert_eq!(usb.device_count(), 1);
        assert_eq!(usb.transfer_count(), 1);
    }

    #[test]
    fn usb3_disconnect() {
        let mut usb = Usb3Driver::new(32);
        let slot = usb
            .enumerate_device(0xAAAA, 0xBBBB, "mouse", UsbSpeed::High, vec![])
            .unwrap();
        assert!(usb.disconnect(slot).is_ok());
        assert_eq!(usb.device_count(), 0);
    }

    #[test]
    fn usb3_transfer_invalid_slot() {
        let mut usb = Usb3Driver::new(32);
        assert!(usb.transfer(99, 0, 64).is_err());
    }

    // ── PCIe Enum V2 ──

    #[test]
    fn pcie_enum_add_and_map_bar() {
        let mut pcie = PcieEnumV2::new();
        pcie.add_device(PcieDevice {
            bus: 0,
            device: 1,
            function: 0,
            vendor_id: 0x8086,
            device_id: 0x1234,
            class: 0x02,
            subclass: 0x00,
            bars: Vec::new(),
            msix_entries: Vec::new(),
        });
        assert!(
            pcie.map_bar(
                0,
                1,
                0,
                PcieBar {
                    index: 0,
                    base: 0xFE00_0000,
                    size: 0x10000,
                    is_memory: true,
                    is_64bit: false,
                    prefetchable: false,
                }
            )
            .is_ok()
        );
        assert_eq!(pcie.device_count(), 1);
    }

    // ── AHCI Driver ──

    #[test]
    fn ahci_write_read_sector() {
        let mut ahci = AhciDriver::new();
        ahci.add_disk(0, "VIRT-DISK", 2048);
        let data = vec![0xDE; 512];
        assert!(ahci.write_sector(0, 100, data.clone()).is_ok());
        let read = ahci.read_sector(0, 100).unwrap();
        assert_eq!(read, data);
    }

    #[test]
    fn ahci_read_unwritten_sector() {
        let ahci = {
            let mut d = AhciDriver::new();
            d.add_disk(0, "VIRT-DISK", 1000);
            d
        };
        let read = ahci.read_sector(0, 50).unwrap();
        assert_eq!(read, vec![0; 512]);
    }

    #[test]
    fn ahci_write_out_of_range() {
        let mut ahci = AhciDriver::new();
        ahci.add_disk(0, "small", 10);
        assert!(ahci.write_sector(0, 100, vec![0; 512]).is_err());
    }

    // ── Audio Driver ──

    #[test]
    fn audio_create_fill_play() {
        let mut audio = AudioDriver::new();
        let id = audio.create_buffer(48000, 2, AudioFormat::Float32);
        assert!(audio.fill_buffer(id, vec![0; 4096]).is_ok());
        assert!(audio.play(id).is_ok());
        assert!(audio.complete_playback(id).is_ok());
    }

    #[test]
    fn audio_play_empty_buffer_fails() {
        let mut audio = AudioDriver::new();
        let id = audio.create_buffer(44100, 1, AudioFormat::Pcm16);
        // Not filled yet.
        assert!(audio.play(id).is_err());
    }

    // ── ACPI ──

    #[test]
    fn acpi_sleep_and_wake() {
        let mut acpi = AcpiSupport::new();
        assert_eq!(acpi.current_state(), AcpiState::S0Working);
        assert!(acpi.sleep().is_ok());
        assert_eq!(acpi.current_state(), AcpiState::S3SuspendToRam);
        assert!(acpi.wake().is_ok());
        assert_eq!(acpi.current_state(), AcpiState::S0Working);
        assert_eq!(acpi.transition_count(), 2);
    }

    #[test]
    fn acpi_shutdown() {
        let mut acpi = AcpiSupport::new();
        assert!(acpi.shutdown().is_ok());
        assert_eq!(acpi.current_state(), AcpiState::S5SoftOff);
    }

    // ── Multi-Monitor ──

    #[test]
    fn multi_monitor_layout() {
        let mut mm = MultiMonitor::new();
        mm.add_output(DisplayOutput {
            name: "HDMI-1".into(),
            resolution: Resolution {
                width: 1920,
                height: 1080,
                refresh_hz: 60,
            },
            position: (0, 0),
            primary: true,
            enabled: true,
        });
        mm.add_output(DisplayOutput {
            name: "DP-1".into(),
            resolution: Resolution {
                width: 2560,
                height: 1440,
                refresh_hz: 144,
            },
            position: (1920, 0),
            primary: false,
            enabled: true,
        });
        assert_eq!(mm.output_count(), 2);
        let (w, h) = mm.virtual_desktop_size();
        assert_eq!(w, 1920 + 2560);
        assert_eq!(h, 1440);
    }

    // ── Keyboard Layout ──

    #[test]
    fn keyboard_us_layout() {
        let kb = KeyboardLayout::new();
        assert_eq!(kb.map_scancode(16), Some('q'));
        assert_eq!(kb.map_scancode(17), Some('w'));
    }

    #[test]
    fn keyboard_de_layout_zy_swap() {
        let mut kb = KeyboardLayout::new();
        kb.set_layout(LayoutId::De);
        // In QWERTZ, scancode 21 is 'z' (was 'y' in US).
        assert_eq!(kb.map_scancode(21), Some('z'));
        // Scancode 44 is 'y' in DE (was 'z' in US).
        assert_eq!(kb.map_scancode(44), Some('y'));
    }

    // ── Mouse Driver ──

    #[test]
    fn mouse_movement_clamped() {
        let mut mouse = MouseDriver::new(MouseInterface::Usb, 100, 100);
        // Start at center (50, 50).
        mouse.move_relative(-1000, -1000);
        assert_eq!(mouse.position(), (0, 0));
        mouse.move_relative(5000, 5000);
        assert_eq!(mouse.position(), (99, 99));
    }

    #[test]
    fn mouse_button_state() {
        let mut mouse = MouseDriver::new(MouseInterface::Ps2, 1920, 1080);
        mouse.set_buttons(MouseButtons {
            left: true,
            right: false,
            middle: false,
        });
        assert!(mouse.buttons.left);
        assert!(!mouse.buttons.right);
    }

    // ── RTC Clock ──

    #[test]
    fn rtc_tick_and_read() {
        let mut rtc = RtcClock::new(RtcTime::new(2026, 3, 31, 23, 59, 58)).unwrap();
        rtc.tick();
        let t = rtc.read_time();
        assert_eq!(t.second, 59);
        rtc.tick();
        let t = rtc.read_time();
        assert_eq!(t.second, 0);
        assert_eq!(t.minute, 0);
        assert_eq!(t.hour, 0); // Midnight rollover.
    }

    #[test]
    fn rtc_invalid_time() {
        assert!(RtcClock::new(RtcTime::new(2026, 13, 1, 0, 0, 0)).is_err());
    }

    #[test]
    fn rtc_display_format() {
        let t = RtcTime::new(2026, 3, 31, 14, 30, 0);
        assert_eq!(format!("{}", t), "2026-03-31 14:30:00");
    }

    // ── Hardware Test Suite ──

    #[test]
    fn hardware_test_suite_all_pass() {
        let mut suite = HardwareTestSuite::new();
        suite.run_all();
        assert!(suite.total_count() >= 7);
        assert_eq!(suite.passed_count(), suite.total_count());
    }
}
