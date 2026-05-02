//! STM32F407VG Discovery board support package.
//!
//! Memory map:
//! - Flash: 1MB @ 0x0800_0000
//! - SRAM1: 112KB @ 0x2000_0000
//! - SRAM2: 16KB @ 0x2001_C000
//! - CCM:   64KB @ 0x1000_0000 (core-coupled memory, no DMA)
//!
//! CPU: ARM Cortex-M4F @ 168MHz (HSE 8MHz → PLL)

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};

/// STM32F407VG Discovery board.
pub struct Stm32f407 {
    _private: (),
}

impl Stm32f407 {
    /// Creates a new STM32F407 board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns GPIO peripheral for a given port (A-I).
    fn gpio_peripheral(port: char, base: u32) -> Peripheral {
        let mut p = Peripheral::new(&format!("GPIO{port}"), base);
        p.add_register("MODER", 0x00, 4);
        p.add_register("OTYPER", 0x04, 4);
        p.add_register("OSPEEDR", 0x08, 4);
        p.add_register("PUPDR", 0x0C, 4);
        p.add_register("IDR", 0x10, 4);
        p.add_register("ODR", 0x14, 4);
        p.add_register("BSRR", 0x18, 4);
        p.add_register("LCKR", 0x1C, 4);
        p.add_register("AFRL", 0x20, 4);
        p.add_register("AFRH", 0x24, 4);
        p
    }

    /// Returns USART peripheral with standard register layout.
    fn usart_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("SR", 0x00, 4);
        p.add_register("DR", 0x04, 4);
        p.add_register("BRR", 0x08, 4);
        p.add_register("CR1", 0x0C, 4);
        p.add_register("CR2", 0x10, 4);
        p.add_register("CR3", 0x14, 4);
        p.add_register("GTPR", 0x18, 4);
        p
    }

    /// Returns SPI peripheral with standard register layout.
    fn spi_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("CR1", 0x00, 4);
        p.add_register("CR2", 0x04, 4);
        p.add_register("SR", 0x08, 4);
        p.add_register("DR", 0x0C, 4);
        p.add_register("CRCPR", 0x10, 4);
        p.add_register("RXCRCR", 0x14, 4);
        p.add_register("TXCRCR", 0x18, 4);
        p
    }
}

impl Default for Stm32f407 {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for Stm32f407 {
    fn name(&self) -> &str {
        "STM32F407VG Discovery"
    }

    fn arch(&self) -> BspArch {
        BspArch::Thumbv7em
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("FLASH", 0x0800_0000, 1024 * 1024, MemoryAttr::Rx),
            MemoryRegion::new("SRAM1", 0x2000_0000, 112 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("SRAM2", 0x2001_C000, 16 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("CCM", 0x1000_0000, 64 * 1024, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        vec![
            // GPIO ports A-I on AHB1 bus
            Self::gpio_peripheral('A', 0x4002_0000),
            Self::gpio_peripheral('B', 0x4002_0400),
            Self::gpio_peripheral('C', 0x4002_0800),
            Self::gpio_peripheral('D', 0x4002_0C00),
            Self::gpio_peripheral('E', 0x4002_1000),
            // USART peripherals
            Self::usart_peripheral("USART1", 0x4001_1000),
            Self::usart_peripheral("USART2", 0x4000_4400),
            Self::usart_peripheral("USART3", 0x4000_4800),
            Self::usart_peripheral("USART6", 0x4001_1400),
            // SPI peripherals
            Self::spi_peripheral("SPI1", 0x4001_3000),
            Self::spi_peripheral("SPI2", 0x4000_3800),
            Self::spi_peripheral("SPI3", 0x4000_3C00),
            // I2C peripherals
            {
                let mut p = Peripheral::new("I2C1", 0x4000_5400);
                p.add_register("CR1", 0x00, 4);
                p.add_register("CR2", 0x04, 4);
                p.add_register("OAR1", 0x08, 4);
                p.add_register("OAR2", 0x0C, 4);
                p.add_register("DR", 0x10, 4);
                p.add_register("SR1", 0x14, 4);
                p.add_register("SR2", 0x18, 4);
                p.add_register("CCR", 0x1C, 4);
                p.add_register("TRISE", 0x20, 4);
                p
            },
            // RCC (Reset and Clock Control)
            {
                let mut p = Peripheral::new("RCC", 0x4002_3800);
                p.add_register("CR", 0x00, 4);
                p.add_register("PLLCFGR", 0x04, 4);
                p.add_register("CFGR", 0x08, 4);
                p.add_register("AHB1ENR", 0x30, 4);
                p.add_register("AHB2ENR", 0x34, 4);
                p.add_register("APB1ENR", 0x40, 4);
                p.add_register("APB2ENR", 0x44, 4);
                p
            },
            // SysTick (core peripheral)
            {
                let mut p = Peripheral::new("SYSTICK", 0xE000_E010);
                p.add_register("CTRL", 0x00, 4);
                p.add_register("LOAD", 0x04, 4);
                p.add_register("VAL", 0x08, 4);
                p.add_register("CALIB", 0x0C, 4);
                p
            },
        ]
    }

    fn vector_table_size(&self) -> usize {
        // 16 Cortex-M system exceptions + 82 STM32F407 IRQs = 98
        98
    }

    fn cpu_frequency(&self) -> u32 {
        168_000_000 // 168 MHz (HSE 8MHz → PLL)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPIO HAL
// ═══════════════════════════════════════════════════════════════════════

/// GPIO pin mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioMode {
    /// Input mode (reset state).
    Input = 0b00,
    /// General-purpose output.
    Output = 0b01,
    /// Alternate function (USART, SPI, I2C, etc.).
    AltFunc = 0b10,
    /// Analog (ADC/DAC).
    Analog = 0b11,
}

/// GPIO output type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioOutputType {
    /// Push-pull (default).
    PushPull = 0,
    /// Open-drain.
    OpenDrain = 1,
}

/// GPIO output speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioSpeed {
    /// Low speed (2 MHz).
    Low = 0b00,
    /// Medium speed (25 MHz).
    Medium = 0b01,
    /// Fast speed (50 MHz).
    Fast = 0b10,
    /// High speed (100 MHz).
    High = 0b11,
}

/// GPIO pull-up / pull-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioPull {
    /// No pull.
    None = 0b00,
    /// Pull-up.
    Up = 0b01,
    /// Pull-down.
    Down = 0b10,
}

/// GPIO pin configuration for a single pin on a port.
#[derive(Debug, Clone)]
pub struct GpioConfig {
    /// Port letter ('A' - 'I').
    pub port: char,
    /// Pin number (0-15).
    pub pin: u8,
    /// Pin mode.
    pub mode: GpioMode,
    /// Output type.
    pub output_type: GpioOutputType,
    /// Speed.
    pub speed: GpioSpeed,
    /// Pull-up/pull-down.
    pub pull: GpioPull,
    /// Alternate function number (0-15), only relevant in AltFunc mode.
    pub alt_func: u8,
}

impl GpioConfig {
    /// Creates a default GPIO config for the given port and pin (input, no pull).
    pub fn new(port: char, pin: u8) -> Self {
        Self {
            port,
            pin,
            mode: GpioMode::Input,
            output_type: GpioOutputType::PushPull,
            speed: GpioSpeed::Low,
            pull: GpioPull::None,
            alt_func: 0,
        }
    }

    /// Configures as output push-pull.
    pub fn as_output(mut self) -> Self {
        self.mode = GpioMode::Output;
        self
    }

    /// Configures as alternate function.
    pub fn as_alt_func(mut self, af: u8) -> Self {
        self.mode = GpioMode::AltFunc;
        self.alt_func = af;
        self
    }

    /// Sets the speed.
    pub fn with_speed(mut self, speed: GpioSpeed) -> Self {
        self.speed = speed;
        self
    }

    /// Sets pull-up/pull-down.
    pub fn with_pull(mut self, pull: GpioPull) -> Self {
        self.pull = pull;
        self
    }

    /// Returns the base address for the GPIO port.
    pub fn port_base_address(&self) -> u32 {
        let offset = (self.port as u32) - ('A' as u32);
        0x4002_0000 + offset * 0x400
    }

    /// Returns the register write sequence to configure this pin.
    ///
    /// Each entry is `(register_offset, bit_position, value, bit_width)`.
    pub fn register_writes(&self) -> Vec<(u32, u8, u32, u8)> {
        let pin = self.pin;
        vec![
            // MODER: 2 bits per pin
            (0x00, pin * 2, self.mode as u32, 2),
            // OTYPER: 1 bit per pin
            (0x04, pin, self.output_type as u32, 1),
            // OSPEEDR: 2 bits per pin
            (0x08, pin * 2, self.speed as u32, 2),
            // PUPDR: 2 bits per pin
            (0x0C, pin * 2, self.pull as u32, 2),
        ]
    }

    /// Returns the BSRR value to set this pin high.
    pub fn bsrr_set(&self) -> u32 {
        1 << self.pin
    }

    /// Returns the BSRR value to reset this pin low.
    pub fn bsrr_reset(&self) -> u32 {
        1 << (self.pin + 16)
    }

    /// Returns the BSRR value to toggle this pin.
    /// Note: toggle is read-modify-write on ODR, but BSRR set/reset is atomic.
    pub fn odr_mask(&self) -> u32 {
        1 << self.pin
    }
}

// ═══════════════════════════════════════════════════════════════════════
// UART HAL
// ═══════════════════════════════════════════════════════════════════════

/// UART configuration.
#[derive(Debug, Clone)]
pub struct UartConfig {
    /// USART peripheral number (1, 2, 3, or 6).
    pub usart_num: u8,
    /// Baud rate.
    pub baud_rate: u32,
    /// Word length (8 or 9 bits).
    pub word_length: u8,
    /// Number of stop bits (1 or 2).
    pub stop_bits: u8,
    /// Parity enabled.
    pub parity: bool,
}

impl UartConfig {
    /// Creates a default UART config (115200 8N1) for the given USART.
    pub fn new(usart_num: u8) -> Self {
        Self {
            usart_num,
            baud_rate: 115_200,
            word_length: 8,
            stop_bits: 1,
            parity: false,
        }
    }

    /// Sets the baud rate.
    pub fn with_baud_rate(mut self, baud: u32) -> Self {
        self.baud_rate = baud;
        self
    }

    /// Returns the base address for this USART peripheral.
    pub fn base_address(&self) -> u32 {
        match self.usart_num {
            1 => 0x4001_1000,
            2 => 0x4000_4400,
            3 => 0x4000_4800,
            6 => 0x4001_1400,
            _ => 0x4001_1000, // default to USART1
        }
    }

    /// Returns the APB bus clock for this USART (APB2 for USART1/6, APB1 for USART2/3).
    pub fn apb_clock(&self) -> u32 {
        match self.usart_num {
            1 | 6 => 84_000_000, // APB2 = 84 MHz
            _ => 42_000_000,     // APB1 = 42 MHz
        }
    }

    /// Computes the BRR value for the configured baud rate.
    ///
    /// BRR = f_ck / baud_rate (with fractional part in lower 4 bits).
    pub fn brr_value(&self) -> u32 {
        let fck = self.apb_clock();
        // USARTDIV = f_ck / (16 * baud)
        // BRR = mantissa << 4 | fraction
        // Using fixed-point: BRR = f_ck / baud (when OVER8=0)
        let usartdiv_x100 = (fck as u64 * 100) / (self.baud_rate as u64 * 16);
        let mantissa = (usartdiv_x100 / 100) as u32;
        let fraction = (((usartdiv_x100 % 100) * 16 + 50) / 100) as u32;
        (mantissa << 4) | (fraction & 0x0F)
    }

    /// Returns CR1 value for the configuration.
    ///
    /// Enables UE (USART enable), TE (transmitter), RE (receiver).
    pub fn cr1_value(&self) -> u32 {
        let mut cr1 = 0u32;
        cr1 |= 1 << 13; // UE: USART enable
        cr1 |= 1 << 3; // TE: Transmitter enable
        cr1 |= 1 << 2; // RE: Receiver enable
        if self.word_length == 9 {
            cr1 |= 1 << 12; // M: 9-bit word length
        }
        if self.parity {
            cr1 |= 1 << 10; // PCE: Parity control enable
        }
        cr1
    }

    /// Returns the RCC enable bit position for this USART.
    pub fn rcc_enable(&self) -> (u32, u8) {
        match self.usart_num {
            1 => (0x44, 4),  // APB2ENR, bit 4
            2 => (0x40, 17), // APB1ENR, bit 17
            3 => (0x40, 18), // APB1ENR, bit 18
            6 => (0x44, 5),  // APB2ENR, bit 5
            _ => (0x44, 4),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RCC Clock Configuration
// ═══════════════════════════════════════════════════════════════════════

/// RCC clock configuration for 168 MHz operation.
///
/// HSE (8 MHz external crystal) → PLL → 168 MHz system clock.
#[derive(Debug, Clone)]
pub struct RccConfig {
    /// HSE crystal frequency in Hz.
    pub hse_freq: u32,
    /// Target system clock in Hz.
    pub sysclk: u32,
    /// AHB prescaler (1, 2, 4, 8, ..., 512).
    pub ahb_prescaler: u16,
    /// APB1 prescaler (1, 2, 4, 8, 16) — max 42 MHz.
    pub apb1_prescaler: u8,
    /// APB2 prescaler (1, 2, 4, 8, 16) — max 84 MHz.
    pub apb2_prescaler: u8,
    /// PLL M divider (2-63).
    pub pll_m: u8,
    /// PLL N multiplier (50-432).
    pub pll_n: u16,
    /// PLL P divider (2, 4, 6, 8).
    pub pll_p: u8,
    /// PLL Q divider for USB/SDIO (2-15).
    pub pll_q: u8,
}

impl RccConfig {
    /// Default configuration for 168 MHz from 8 MHz HSE.
    ///
    /// HSE=8MHz, PLLM=8, PLLN=336, PLLP=2, PLLQ=7
    /// → VCO_in = 8/8 = 1 MHz
    /// → VCO_out = 1 * 336 = 336 MHz
    /// → SYSCLK = 336/2 = 168 MHz
    /// → USB = 336/7 = 48 MHz
    pub fn default_168mhz() -> Self {
        Self {
            hse_freq: 8_000_000,
            sysclk: 168_000_000,
            ahb_prescaler: 1,
            apb1_prescaler: 4, // 168/4 = 42 MHz
            apb2_prescaler: 2, // 168/2 = 84 MHz
            pll_m: 8,
            pll_n: 336,
            pll_p: 2,
            pll_q: 7,
        }
    }

    /// Returns the PLLCFGR register value.
    pub fn pllcfgr_value(&self) -> u32 {
        let mut val = 0u32;
        val |= self.pll_m as u32; // PLLM: bits 0-5
        val |= (self.pll_n as u32) << 6; // PLLN: bits 6-14
        val |= (((self.pll_p / 2) - 1) as u32) << 16; // PLLP: bits 16-17
        val |= 1 << 22; // PLLSRC: HSE
        val |= (self.pll_q as u32) << 24; // PLLQ: bits 24-27
        val
    }

    /// Returns the CFGR register value for AHB/APB prescalers.
    pub fn cfgr_value(&self) -> u32 {
        let mut val = 0u32;
        // SW: PLL as system clock
        val |= 0b10;
        // HPRE: AHB prescaler
        val |= match self.ahb_prescaler {
            1 => 0b0000,
            2 => 0b1000,
            4 => 0b1001,
            8 => 0b1010,
            16 => 0b1011,
            64 => 0b1100,
            128 => 0b1101,
            256 => 0b1110,
            512 => 0b1111,
            _ => 0b0000,
        } << 4;
        // PPRE1: APB1 prescaler
        val |= match self.apb1_prescaler {
            1 => 0b000,
            2 => 0b100,
            4 => 0b101,
            8 => 0b110,
            16 => 0b111,
            _ => 0b000,
        } << 10;
        // PPRE2: APB2 prescaler
        val |= match self.apb2_prescaler {
            1 => 0b000,
            2 => 0b100,
            4 => 0b101,
            8 => 0b110,
            16 => 0b111,
            _ => 0b000,
        } << 13;
        val
    }

    /// Returns AHB clock frequency.
    pub fn ahb_clock(&self) -> u32 {
        self.sysclk / self.ahb_prescaler as u32
    }

    /// Returns APB1 clock frequency.
    pub fn apb1_clock(&self) -> u32 {
        self.ahb_clock() / self.apb1_prescaler as u32
    }

    /// Returns APB2 clock frequency.
    pub fn apb2_clock(&self) -> u32 {
        self.ahb_clock() / self.apb2_prescaler as u32
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SysTick Timer
// ═══════════════════════════════════════════════════════════════════════

/// SysTick timer configuration for periodic interrupts.
#[derive(Debug, Clone)]
pub struct SysTickConfig {
    /// System clock frequency in Hz.
    pub clock_freq: u32,
    /// Tick period in microseconds.
    pub tick_us: u32,
}

impl SysTickConfig {
    /// Creates a 1ms tick configuration.
    pub fn tick_1ms(clock_freq: u32) -> Self {
        Self {
            clock_freq,
            tick_us: 1000,
        }
    }

    /// Returns the LOAD register value.
    ///
    /// LOAD = (clock_freq / ticks_per_second) - 1
    pub fn load_value(&self) -> u32 {
        let ticks_per_second = 1_000_000 / self.tick_us;
        (self.clock_freq / ticks_per_second) - 1
    }

    /// Returns the CTRL register value.
    ///
    /// Enables SysTick, interrupt, and uses processor clock.
    pub fn ctrl_value(&self) -> u32 {
        let mut ctrl = 0u32;
        ctrl |= 1 << 0; // ENABLE
        ctrl |= 1 << 1; // TICKINT (enable interrupt)
        ctrl |= 1 << 2; // CLKSOURCE (processor clock)
        ctrl
    }

    /// Returns delay_ms implementation as startup code snippet.
    pub fn delay_ms_code(&self) -> String {
        format!(
            r#"/* delay_ms({clock_freq}Hz, {tick_us}us tick) */
.global delay_ms
.type delay_ms, %function
delay_ms:
  push {{r4, lr}}
  mov r4, r0
delay_loop:
  cbz r4, delay_done
  /* Wait for SysTick COUNTFLAG */
  ldr r0, =0xE000E010  /* SysTick CTRL */
wait_tick:
  ldr r1, [r0]
  tst r1, #(1 << 16)   /* COUNTFLAG */
  beq wait_tick
  subs r4, r4, #1
  b delay_loop
delay_done:
  pop {{r4, pc}}
"#,
            clock_freq = self.clock_freq,
            tick_us = self.tick_us
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Discovery Board LED Pins
// ═══════════════════════════════════════════════════════════════════════

/// STM32F407VG Discovery board LED definitions.
pub struct DiscoveryLeds;

impl DiscoveryLeds {
    /// Green LED: PD12.
    pub fn green() -> GpioConfig {
        GpioConfig::new('D', 12).as_output()
    }

    /// Orange LED: PD13.
    pub fn orange() -> GpioConfig {
        GpioConfig::new('D', 13).as_output()
    }

    /// Red LED: PD14.
    pub fn red() -> GpioConfig {
        GpioConfig::new('D', 14).as_output()
    }

    /// Blue LED: PD15.
    pub fn blue() -> GpioConfig {
        GpioConfig::new('D', 15).as_output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stm32f407_board_identity() {
        let board = Stm32f407::new();
        assert_eq!(board.name(), "STM32F407VG Discovery");
        assert_eq!(board.arch(), BspArch::Thumbv7em);
        assert_eq!(board.cpu_frequency(), 168_000_000);
    }

    #[test]
    fn stm32f407_memory_regions() {
        let board = Stm32f407::new();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 4);

        // Flash: 1MB @ 0x0800_0000
        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].origin, 0x0800_0000);
        assert_eq!(regions[0].length, 1024 * 1024);
        assert_eq!(regions[0].attr, MemoryAttr::Rx);

        // SRAM1: 112KB @ 0x2000_0000
        assert_eq!(regions[1].name, "SRAM1");
        assert_eq!(regions[1].origin, 0x2000_0000);
        assert_eq!(regions[1].length, 112 * 1024);

        // SRAM2: 16KB @ 0x2001_C000
        assert_eq!(regions[2].name, "SRAM2");
        assert_eq!(regions[2].origin, 0x2001_C000);
        assert_eq!(regions[2].length, 16 * 1024);

        // CCM: 64KB @ 0x1000_0000
        assert_eq!(regions[3].name, "CCM");
        assert_eq!(regions[3].origin, 0x1000_0000);
        assert_eq!(regions[3].length, 64 * 1024);
    }

    #[test]
    fn stm32f407_gpio_peripherals() {
        let board = Stm32f407::new();
        let periphs = board.peripherals();
        let gpioa = periphs.iter().find(|p| p.name == "GPIOA").unwrap();

        assert_eq!(gpioa.base_address, 0x4002_0000);
        assert_eq!(gpioa.register_address("MODER"), Some(0x4002_0000));
        assert_eq!(gpioa.register_address("ODR"), Some(0x4002_0014));
        assert_eq!(gpioa.register_address("BSRR"), Some(0x4002_0018));
    }

    #[test]
    fn stm32f407_usart_peripherals() {
        let board = Stm32f407::new();
        let periphs = board.peripherals();
        let usart1 = periphs.iter().find(|p| p.name == "USART1").unwrap();

        assert_eq!(usart1.base_address, 0x4001_1000);
        assert_eq!(usart1.register_address("SR"), Some(0x4001_1000));
        assert_eq!(usart1.register_address("DR"), Some(0x4001_1004));
        assert_eq!(usart1.register_address("BRR"), Some(0x4001_1008));
    }

    #[test]
    fn stm32f407_spi_peripherals() {
        let board = Stm32f407::new();
        let periphs = board.peripherals();
        let spi1 = periphs.iter().find(|p| p.name == "SPI1").unwrap();

        assert_eq!(spi1.base_address, 0x4001_3000);
        assert_eq!(spi1.register_address("CR1"), Some(0x4001_3000));
        assert_eq!(spi1.register_address("DR"), Some(0x4001_300C));
    }

    #[test]
    fn stm32f407_rcc_peripheral() {
        let board = Stm32f407::new();
        let periphs = board.peripherals();
        let rcc = periphs.iter().find(|p| p.name == "RCC").unwrap();

        assert_eq!(rcc.base_address, 0x4002_3800);
        assert_eq!(rcc.register_address("AHB1ENR"), Some(0x4002_3830));
        assert_eq!(rcc.register_address("APB1ENR"), Some(0x4002_3840));
    }

    #[test]
    fn stm32f407_systick_peripheral() {
        let board = Stm32f407::new();
        let periphs = board.peripherals();
        let systick = periphs.iter().find(|p| p.name == "SYSTICK").unwrap();

        assert_eq!(systick.base_address, 0xE000_E010);
        assert_eq!(systick.register_address("CTRL"), Some(0xE000_E010));
        assert_eq!(systick.register_address("LOAD"), Some(0xE000_E014));
    }

    #[test]
    fn stm32f407_vector_table_size() {
        let board = Stm32f407::new();
        assert_eq!(board.vector_table_size(), 98);
    }

    #[test]
    fn stm32f407_linker_script() {
        let board = Stm32f407::new();
        let script = board.generate_linker_script();

        assert!(script.contains("STM32F407VG Discovery"));
        assert!(script.contains("FLASH"));
        assert!(script.contains("SRAM1"));
        assert!(script.contains("SRAM2"));
        assert!(script.contains("CCM"));
        assert!(script.contains("0x08000000"));
        assert!(script.contains("1024K"));
        assert!(script.contains("ENTRY(Reset_Handler)"));
    }

    #[test]
    fn stm32f407_startup_code() {
        let board = Stm32f407::new();
        let code = board.generate_startup_code();

        assert!(code.contains(".cpu cortex-m4"));
        assert!(code.contains(".fpu fpv4-sp-d16"));
        assert!(code.contains("Reset_Handler"));
        assert!(code.contains("Enable FPU"));
        assert!(code.contains("bl main"));
    }

    #[test]
    fn stm32f407_default_trait() {
        let board = Stm32f407::default();
        assert_eq!(board.name(), "STM32F407VG Discovery");
    }

    // ── GPIO HAL Tests ──────────────────────────────────────────

    #[test]
    fn gpio_config_output() {
        let cfg = GpioConfig::new('D', 12).as_output();
        assert_eq!(cfg.port, 'D');
        assert_eq!(cfg.pin, 12);
        assert_eq!(cfg.mode, GpioMode::Output);
        assert_eq!(cfg.output_type, GpioOutputType::PushPull);
    }

    #[test]
    fn gpio_config_alt_func() {
        let cfg = GpioConfig::new('A', 9).as_alt_func(7); // USART1_TX
        assert_eq!(cfg.mode, GpioMode::AltFunc);
        assert_eq!(cfg.alt_func, 7);
    }

    #[test]
    fn gpio_port_base_address() {
        assert_eq!(GpioConfig::new('A', 0).port_base_address(), 0x4002_0000);
        assert_eq!(GpioConfig::new('B', 0).port_base_address(), 0x4002_0400);
        assert_eq!(GpioConfig::new('D', 0).port_base_address(), 0x4002_0C00);
    }

    #[test]
    fn gpio_register_writes() {
        let cfg = GpioConfig::new('D', 12).as_output();
        let writes = cfg.register_writes();
        // MODER: offset 0x00, bit 24 (pin*2), value 0b01, width 2
        assert_eq!(writes[0], (0x00, 24, 0b01, 2));
    }

    #[test]
    fn gpio_bsrr_values() {
        let cfg = GpioConfig::new('D', 12);
        assert_eq!(cfg.bsrr_set(), 1 << 12);
        assert_eq!(cfg.bsrr_reset(), 1 << 28);
        assert_eq!(cfg.odr_mask(), 1 << 12);
    }

    #[test]
    fn discovery_leds() {
        let green = DiscoveryLeds::green();
        assert_eq!(green.port, 'D');
        assert_eq!(green.pin, 12);
        assert_eq!(green.mode, GpioMode::Output);

        let red = DiscoveryLeds::red();
        assert_eq!(red.pin, 14);
    }

    // ── UART HAL Tests ──────────────────────────────────────────

    #[test]
    fn uart_config_defaults() {
        let cfg = UartConfig::new(1);
        assert_eq!(cfg.baud_rate, 115_200);
        assert_eq!(cfg.word_length, 8);
        assert_eq!(cfg.stop_bits, 1);
        assert!(!cfg.parity);
    }

    #[test]
    fn uart_base_addresses() {
        assert_eq!(UartConfig::new(1).base_address(), 0x4001_1000);
        assert_eq!(UartConfig::new(2).base_address(), 0x4000_4400);
        assert_eq!(UartConfig::new(3).base_address(), 0x4000_4800);
        assert_eq!(UartConfig::new(6).base_address(), 0x4001_1400);
    }

    #[test]
    fn uart_brr_calculation() {
        let cfg = UartConfig::new(1); // APB2 = 84 MHz, 115200 baud
        let brr = cfg.brr_value();
        // Expected: 84_000_000 / (16 * 115_200) ≈ 45.57
        // Mantissa = 45, Fraction = 0.57 * 16 ≈ 9
        // BRR = (45 << 4) | 9 = 0x2D9
        assert!(brr > 0);
        let mantissa = brr >> 4;
        assert!((44..=46).contains(&mantissa)); // ~45
    }

    #[test]
    fn uart_cr1_value() {
        let cfg = UartConfig::new(1);
        let cr1 = cfg.cr1_value();
        assert_ne!(cr1 & (1 << 13), 0); // UE enabled
        assert_ne!(cr1 & (1 << 3), 0); // TE enabled
        assert_ne!(cr1 & (1 << 2), 0); // RE enabled
        assert_eq!(cr1 & (1 << 12), 0); // M=0 (8-bit word)
        assert_eq!(cr1 & (1 << 10), 0); // PCE=0 (no parity)
    }

    #[test]
    fn uart_rcc_enable() {
        assert_eq!(UartConfig::new(1).rcc_enable(), (0x44, 4)); // APB2ENR
        assert_eq!(UartConfig::new(2).rcc_enable(), (0x40, 17)); // APB1ENR
    }

    // ── RCC Clock Tests ──────────────────────────────────────────

    #[test]
    fn rcc_default_168mhz() {
        let rcc = RccConfig::default_168mhz();
        assert_eq!(rcc.sysclk, 168_000_000);
        assert_eq!(rcc.ahb_clock(), 168_000_000);
        assert_eq!(rcc.apb1_clock(), 42_000_000);
        assert_eq!(rcc.apb2_clock(), 84_000_000);
    }

    #[test]
    fn rcc_pllcfgr_value() {
        let rcc = RccConfig::default_168mhz();
        let pllcfgr = rcc.pllcfgr_value();
        // PLLM = 8 (bits 0-5)
        assert_eq!(pllcfgr & 0x3F, 8);
        // PLLN = 336 (bits 6-14)
        assert_eq!((pllcfgr >> 6) & 0x1FF, 336);
        // PLLSRC = HSE (bit 22)
        assert_ne!(pllcfgr & (1 << 22), 0);
        // PLLQ = 7 (bits 24-27)
        assert_eq!((pllcfgr >> 24) & 0x0F, 7);
    }

    #[test]
    fn rcc_cfgr_value() {
        let rcc = RccConfig::default_168mhz();
        let cfgr = rcc.cfgr_value();
        // SW = PLL (bits 0-1 = 0b10)
        assert_eq!(cfgr & 0x03, 0b10);
    }

    // ── SysTick Tests ──────────────────────────────────────────

    #[test]
    fn systick_1ms_config() {
        let systick = SysTickConfig::tick_1ms(168_000_000);
        // LOAD = 168_000_000 / 1000 - 1 = 167_999
        assert_eq!(systick.load_value(), 167_999);
    }

    #[test]
    fn systick_ctrl_value() {
        let systick = SysTickConfig::tick_1ms(168_000_000);
        let ctrl = systick.ctrl_value();
        assert_ne!(ctrl & 1, 0); // ENABLE
        assert_ne!(ctrl & (1 << 1), 0); // TICKINT
        assert_ne!(ctrl & (1 << 2), 0); // CLKSOURCE
    }

    #[test]
    fn systick_delay_ms_code() {
        let systick = SysTickConfig::tick_1ms(168_000_000);
        let code = systick.delay_ms_code();
        assert!(code.contains("delay_ms"));
        assert!(code.contains("COUNTFLAG"));
        assert!(code.contains("SysTick CTRL"));
    }
}
