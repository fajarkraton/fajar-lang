//! Hardware linear safety — GPIO pin state machines, DMA buffer linearity,
//! IRQ registration, MMIO regions, clock gates, power domains, @kernel interop.

use std::fmt;

use super::checker::Linearity;

// ═══════════════════════════════════════════════════════════════════════
// S8.1 / S8.2: GPIO Pin Linearity & State Machine
// ═══════════════════════════════════════════════════════════════════════

/// GPIO pin state — each transition consumes the old state and produces a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPinState {
    /// Unconfigured — initial state after claiming.
    Unconfigured,
    /// Input mode.
    Input,
    /// Output mode.
    Output,
    /// Alternate function mode.
    Alternate(u8),
}

impl fmt::Display for GpioPinState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpioPinState::Unconfigured => write!(f, "Unconfigured"),
            GpioPinState::Input => write!(f, "Input"),
            GpioPinState::Output => write!(f, "Output"),
            GpioPinState::Alternate(n) => write!(f, "Alternate({n})"),
        }
    }
}

/// A linear GPIO pin with state tracked at type level.
#[derive(Debug, Clone)]
pub struct GpioPin {
    /// Pin number (0-based).
    pub pin_number: u8,
    /// Current state.
    pub state: GpioPinState,
    /// Port (A, B, C, etc.).
    pub port: char,
}

impl GpioPin {
    /// Claims a GPIO pin (Unconfigured state).
    pub fn claim(port: char, pin_number: u8) -> Self {
        Self {
            pin_number,
            state: GpioPinState::Unconfigured,
            port,
        }
    }

    /// Configures as input — consumes old state, produces Input.
    pub fn into_input(self) -> Self {
        Self {
            state: GpioPinState::Input,
            ..self
        }
    }

    /// Configures as output — consumes old state, produces Output.
    pub fn into_output(self) -> Self {
        Self {
            state: GpioPinState::Output,
            ..self
        }
    }

    /// Configures as alternate function — consumes old state.
    pub fn into_alternate(self, af: u8) -> Self {
        Self {
            state: GpioPinState::Alternate(af),
            ..self
        }
    }

    /// Returns the linearity of this pin (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

impl fmt::Display for GpioPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{}{} [{}]", self.port, self.pin_number, self.state)
    }
}

/// Validates a GPIO state transition.
pub fn validate_gpio_transition(from: GpioPinState, to: GpioPinState) -> Result<(), String> {
    // All transitions from any state are valid (each consumes the old pin).
    // The only invalid case is transitioning from Unconfigured to read/write.
    if from == GpioPinState::Unconfigured
        && !matches!(
            to,
            GpioPinState::Input | GpioPinState::Output | GpioPinState::Alternate(_)
        )
    {
        return Err(format!("invalid GPIO transition from {from} to {to}"));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// S8.3 / S8.4: DMA Buffer Linearity
// ═══════════════════════════════════════════════════════════════════════

/// A linear DMA buffer — exactly one owner at a time.
#[derive(Debug, Clone)]
pub struct DmaBuffer {
    /// Physical address of the buffer.
    pub phys_addr: u64,
    /// Buffer length in bytes.
    pub len: usize,
    /// Whether the buffer is currently in-flight (owned by DMA engine).
    pub in_flight: bool,
}

impl DmaBuffer {
    /// Allocates a new DMA buffer.
    pub fn allocate(phys_addr: u64, len: usize) -> Self {
        Self {
            phys_addr,
            len,
            in_flight: false,
        }
    }

    /// Returns the linearity (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

/// Represents a DMA transfer future — buffer is reclaimed on completion.
#[derive(Debug, Clone)]
pub struct DmaFuture {
    /// The physical address of the in-flight buffer.
    pub buffer_phys_addr: u64,
    /// Buffer length.
    pub buffer_len: usize,
    /// Whether the transfer is complete.
    pub completed: bool,
}

/// Starts a DMA transfer, consuming the buffer.
pub fn dma_start(buf: DmaBuffer) -> DmaFuture {
    DmaFuture {
        buffer_phys_addr: buf.phys_addr,
        buffer_len: buf.len,
        completed: false,
    }
}

/// Completes a DMA transfer, returning the buffer.
pub fn dma_complete(future: DmaFuture) -> DmaBuffer {
    DmaBuffer {
        phys_addr: future.buffer_phys_addr,
        len: future.buffer_len,
        in_flight: false,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.5: IRQ Handler Registration
// ═══════════════════════════════════════════════════════════════════════

/// A linear IRQ registration — must be unregistered before drop.
#[derive(Debug, Clone)]
pub struct IrqRegistration {
    /// IRQ number.
    pub irq: u8,
    /// Whether the handler is currently registered.
    pub registered: bool,
}

impl IrqRegistration {
    /// Registers an IRQ handler.
    pub fn register(irq: u8) -> Self {
        Self {
            irq,
            registered: true,
        }
    }

    /// Unregisters the handler, consuming the registration.
    pub fn unregister(self) -> u8 {
        self.irq
    }

    /// Returns the linearity (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.6: MMIO Region Linearity
// ═══════════════════════════════════════════════════════════════════════

/// A linear MMIO region — exclusive hardware access.
#[derive(Debug, Clone)]
pub struct MmioRegion {
    /// Base address.
    pub base: u64,
    /// Region size in bytes.
    pub size: usize,
    /// Whether the region is currently claimed.
    pub claimed: bool,
}

impl MmioRegion {
    /// Claims an MMIO region.
    pub fn claim(base: u64, size: usize) -> Self {
        Self {
            base,
            size,
            claimed: true,
        }
    }

    /// Releases the MMIO region, consuming the handle.
    pub fn release(self) -> (u64, usize) {
        (self.base, self.size)
    }

    /// Returns the linearity (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.7: Clock Gate Handle
// ═══════════════════════════════════════════════════════════════════════

/// A linear clock gate — enable/disable must be paired.
#[derive(Debug, Clone)]
pub struct ClockGate {
    /// Peripheral ID.
    pub peripheral: u8,
    /// Whether the clock is currently enabled.
    pub enabled: bool,
}

impl ClockGate {
    /// Enables the clock, returning a linear handle.
    pub fn enable(peripheral: u8) -> Self {
        Self {
            peripheral,
            enabled: true,
        }
    }

    /// Disables the clock, consuming the handle.
    pub fn disable(self) -> u8 {
        self.peripheral
    }

    /// Returns the linearity (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.8: Power Domain
// ═══════════════════════════════════════════════════════════════════════

/// A linear power domain — power on/off lifecycle tracked.
#[derive(Debug, Clone)]
pub struct PowerDomain {
    /// Domain ID.
    pub id: u8,
    /// Whether the domain is currently powered on.
    pub powered: bool,
}

impl PowerDomain {
    /// Powers on the domain, returning a linear handle.
    pub fn power_on(id: u8) -> Self {
        Self { id, powered: true }
    }

    /// Powers off the domain, consuming the handle.
    pub fn power_off(self) -> u8 {
        self.id
    }

    /// Returns the linearity (always Linear).
    pub fn linearity(&self) -> Linearity {
        Linearity::Linear
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S8.9: @kernel Linear Integration
// ═══════════════════════════════════════════════════════════════════════

/// Context in which a linear resource is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceContext {
    /// @kernel — hardware resources allowed.
    Kernel,
    /// @device — tensor ops, no raw hardware.
    Device,
    /// @safe — no direct hardware access.
    Safe,
    /// @unsafe — all access.
    Unsafe,
}

/// Checks whether a hardware linear resource can be used in the given context.
pub fn check_context_compatibility(resource: &str, context: ResourceContext) -> Result<(), String> {
    match context {
        ResourceContext::Kernel | ResourceContext::Unsafe => Ok(()),
        ResourceContext::Device => Err(format!(
            "hardware resource `{resource}` cannot be used in @device context"
        )),
        ResourceContext::Safe => Err(format!(
            "hardware resource `{resource}` cannot be used in @safe context"
        )),
    }
}

/// Returns the allowed contexts for a hardware linear type.
pub fn allowed_contexts(_type_name: &str) -> Vec<ResourceContext> {
    vec![ResourceContext::Kernel, ResourceContext::Unsafe]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S8.1 — GPIO Pin Linearity
    #[test]
    fn s8_1_gpio_pin_claim() {
        let pin = GpioPin::claim('A', 5);
        assert_eq!(pin.state, GpioPinState::Unconfigured);
        assert_eq!(pin.linearity(), Linearity::Linear);
        assert_eq!(pin.to_string(), "PA5 [Unconfigured]");
    }

    // S8.2 — Pin State Machine
    #[test]
    fn s8_2_gpio_state_transitions() {
        let pin = GpioPin::claim('B', 3);
        let pin = pin.into_input();
        assert_eq!(pin.state, GpioPinState::Input);
        let pin = pin.into_output();
        assert_eq!(pin.state, GpioPinState::Output);
        let pin = pin.into_alternate(7);
        assert_eq!(pin.state, GpioPinState::Alternate(7));
    }

    #[test]
    fn s8_2_gpio_transition_validation() {
        assert!(validate_gpio_transition(GpioPinState::Unconfigured, GpioPinState::Input).is_ok());
        assert!(validate_gpio_transition(GpioPinState::Input, GpioPinState::Output).is_ok());
    }

    // S8.3 — DMA Buffer
    #[test]
    fn s8_3_dma_buffer_allocate() {
        let buf = DmaBuffer::allocate(0x1000_0000, 4096);
        assert_eq!(buf.phys_addr, 0x1000_0000);
        assert_eq!(buf.len, 4096);
        assert!(!buf.in_flight);
        assert_eq!(buf.linearity(), Linearity::Linear);
    }

    // S8.4 — DMA Transfer
    #[test]
    fn s8_4_dma_transfer_lifecycle() {
        let buf = DmaBuffer::allocate(0x2000_0000, 8192);
        let future = dma_start(buf);
        assert!(!future.completed);
        assert_eq!(future.buffer_phys_addr, 0x2000_0000);
        let reclaimed = dma_complete(future);
        assert_eq!(reclaimed.phys_addr, 0x2000_0000);
        assert_eq!(reclaimed.len, 8192);
    }

    // S8.5 — IRQ Registration
    #[test]
    fn s8_5_irq_lifecycle() {
        let reg = IrqRegistration::register(33);
        assert!(reg.registered);
        assert_eq!(reg.linearity(), Linearity::Linear);
        let irq = reg.unregister();
        assert_eq!(irq, 33);
    }

    // S8.6 — MMIO Region
    #[test]
    fn s8_6_mmio_claim_release() {
        let region = MmioRegion::claim(0x4000_0000, 0x1000);
        assert!(region.claimed);
        assert_eq!(region.linearity(), Linearity::Linear);
        let (base, size) = region.release();
        assert_eq!(base, 0x4000_0000);
        assert_eq!(size, 0x1000);
    }

    // S8.7 — Clock Gate
    #[test]
    fn s8_7_clock_gate_lifecycle() {
        let gate = ClockGate::enable(42);
        assert!(gate.enabled);
        assert_eq!(gate.linearity(), Linearity::Linear);
        let peripheral = gate.disable();
        assert_eq!(peripheral, 42);
    }

    // S8.8 — Power Domain
    #[test]
    fn s8_8_power_domain_lifecycle() {
        let domain = PowerDomain::power_on(7);
        assert!(domain.powered);
        assert_eq!(domain.linearity(), Linearity::Linear);
        let id = domain.power_off();
        assert_eq!(id, 7);
    }

    // S8.9 — @kernel Integration
    #[test]
    fn s8_9_kernel_context_allowed() {
        assert!(check_context_compatibility("GpioPin", ResourceContext::Kernel).is_ok());
        assert!(check_context_compatibility("DmaBuffer", ResourceContext::Unsafe).is_ok());
    }

    #[test]
    fn s8_9_device_context_rejected() {
        assert!(check_context_compatibility("GpioPin", ResourceContext::Device).is_err());
    }

    #[test]
    fn s8_9_safe_context_rejected() {
        assert!(check_context_compatibility("MmioRegion", ResourceContext::Safe).is_err());
    }

    #[test]
    fn s8_9_allowed_contexts() {
        let contexts = allowed_contexts("GpioPin");
        assert!(contexts.contains(&ResourceContext::Kernel));
        assert!(contexts.contains(&ResourceContext::Unsafe));
        assert!(!contexts.contains(&ResourceContext::Device));
    }

    // S8.10 — Additional
    #[test]
    fn s8_10_gpio_state_display() {
        assert_eq!(GpioPinState::Input.to_string(), "Input");
        assert_eq!(GpioPinState::Output.to_string(), "Output");
        assert_eq!(GpioPinState::Alternate(7).to_string(), "Alternate(7)");
    }

    #[test]
    fn s8_10_dma_buffer_sizes() {
        let buf = DmaBuffer::allocate(0x0, 256);
        assert_eq!(buf.len, 256);
        let future = dma_start(buf);
        assert_eq!(future.buffer_len, 256);
    }

    #[test]
    fn s8_10_multiple_gpio_pins() {
        let pa0 = GpioPin::claim('A', 0).into_output();
        let pa1 = GpioPin::claim('A', 1).into_input();
        let pb5 = GpioPin::claim('B', 5).into_alternate(2);
        assert_eq!(pa0.state, GpioPinState::Output);
        assert_eq!(pa1.state, GpioPinState::Input);
        assert_eq!(pb5.state, GpioPinState::Alternate(2));
    }
}
