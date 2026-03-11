//! STM32H5F5 board support package (Arduino VENTUNO Q MCU).
//!
//! Memory map:
//! - Flash: 4MB @ 0x0800_0000
//! - SRAM1: 640KB @ 0x2000_0000
//! - SRAM2: 640KB @ 0x2004_0000
//! - SRAM3: 320KB @ 0x2006_0000
//!
//! CPU: ARM Cortex-M33 @ 250MHz (HSE 25MHz -> PLL1)

use super::{Board, BspArch, MemoryAttr, MemoryRegion, Peripheral};

// ═══════════════════════════════════════════════════════════════════════
// Board Definition (Sprint 29)
// ═══════════════════════════════════════════════════════════════════════

/// STM32H5F5 board (Cortex-M33 @ 250MHz).
pub struct Stm32H5 {
    _private: (),
}

impl Stm32H5 {
    /// Creates a new STM32H5 board instance.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Returns GPIO peripheral for a given port (A-I).
    ///
    /// Base address: 0x4202_0000 (GPIOA), stride 0x400.
    fn gpio_peripheral(port: char, base: u32) -> Peripheral {
        let mut p = Peripheral::new(&format!("GPIO{port}"), base);
        p.add_register("MODER", 0x00, 4);
        p.add_register("OTYPER", 0x04, 4);
        p.add_register("OSPEEDR", 0x08, 4);
        p.add_register("PUPDR", 0x0C, 4);
        p.add_register("IDR", 0x10, 4);
        p.add_register("ODR", 0x14, 4);
        p.add_register("BSRR", 0x18, 4);
        p.add_register("AFRL", 0x20, 4);
        p.add_register("AFRH", 0x24, 4);
        p
    }

    /// Returns USART peripheral with STM32H5 register layout.
    ///
    /// The STM32H5 uses ISR/TDR/RDR instead of SR/DR.
    fn usart_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("CR1", 0x00, 4);
        p.add_register("CR2", 0x04, 4);
        p.add_register("CR3", 0x08, 4);
        p.add_register("BRR", 0x0C, 4);
        p.add_register("GTPR", 0x10, 4);
        p.add_register("RTOR", 0x14, 4);
        p.add_register("RQR", 0x18, 4);
        p.add_register("ISR", 0x1C, 4);
        p.add_register("ICR", 0x20, 4);
        p.add_register("RDR", 0x24, 4);
        p.add_register("TDR", 0x28, 4);
        p.add_register("PRESC", 0x2C, 4);
        p
    }

    /// Returns SPI peripheral with STM32H5 register layout.
    fn spi_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("CR1", 0x00, 4);
        p.add_register("CR2", 0x04, 4);
        p.add_register("CFG1", 0x08, 4);
        p.add_register("CFG2", 0x0C, 4);
        p.add_register("IER", 0x10, 4);
        p.add_register("SR", 0x14, 4);
        p.add_register("IFCR", 0x18, 4);
        p.add_register("TXDR", 0x20, 4);
        p.add_register("RXDR", 0x30, 4);
        p
    }

    /// Returns I2C peripheral with STM32H5 register layout.
    fn i2c_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("CR1", 0x00, 4);
        p.add_register("CR2", 0x04, 4);
        p.add_register("OAR1", 0x08, 4);
        p.add_register("OAR2", 0x0C, 4);
        p.add_register("TIMINGR", 0x10, 4);
        p.add_register("TIMEOUTR", 0x14, 4);
        p.add_register("ISR", 0x18, 4);
        p.add_register("ICR", 0x1C, 4);
        p.add_register("PECR", 0x20, 4);
        p.add_register("RXDR", 0x24, 4);
        p.add_register("TXDR", 0x28, 4);
        p
    }

    /// Returns FDCAN peripheral with STM32H5 register layout.
    fn fdcan_peripheral(name: &str, base: u32) -> Peripheral {
        let mut p = Peripheral::new(name, base);
        p.add_register("CCCR", 0x18, 4);
        p.add_register("NBTP", 0x1C, 4);
        p.add_register("TSCC", 0x20, 4);
        p.add_register("TOCC", 0x28, 4);
        p.add_register("ECR", 0x40, 4);
        p.add_register("PSR", 0x44, 4);
        p.add_register("DBTP", 0x0C, 4);
        p.add_register("TDCR", 0x48, 4);
        p.add_register("IR", 0x50, 4);
        p.add_register("IE", 0x54, 4);
        p.add_register("ILS", 0x58, 4);
        p.add_register("TXBAR", 0xD0, 4);
        p.add_register("RXF0C", 0xA0, 4);
        p.add_register("RXF0S", 0xA4, 4);
        p.add_register("RXF1C", 0xB0, 4);
        p.add_register("RXF1S", 0xB4, 4);
        p
    }

    /// Returns the GPIO base address for a port letter.
    pub fn gpio_base(port: char) -> u32 {
        let offset = (port as u32) - ('A' as u32);
        0x4202_0000 + offset * 0x400
    }
}

impl Default for Stm32H5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for Stm32H5 {
    fn name(&self) -> &str {
        "STM32H5F5"
    }

    fn arch(&self) -> BspArch {
        BspArch::ArmCortexM33
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion::new("FLASH", 0x0800_0000, 4 * 1024 * 1024, MemoryAttr::Rx),
            MemoryRegion::new("SRAM1", 0x2000_0000, 640 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("SRAM2", 0x2004_0000, 640 * 1024, MemoryAttr::Rw),
            MemoryRegion::new("SRAM3", 0x2006_0000, 320 * 1024, MemoryAttr::Rw),
        ]
    }

    fn peripherals(&self) -> Vec<Peripheral> {
        vec![
            // GPIO ports A-I (9 ports)
            Self::gpio_peripheral('A', 0x4202_0000),
            Self::gpio_peripheral('B', 0x4202_0400),
            Self::gpio_peripheral('C', 0x4202_0800),
            Self::gpio_peripheral('D', 0x4202_0C00),
            Self::gpio_peripheral('E', 0x4202_1000),
            Self::gpio_peripheral('F', 0x4202_1400),
            Self::gpio_peripheral('G', 0x4202_1800),
            Self::gpio_peripheral('H', 0x4202_1C00),
            Self::gpio_peripheral('I', 0x4202_2000),
            // USART / UART peripherals
            Self::usart_peripheral("USART1", 0x4000_C800),
            Self::usart_peripheral("USART2", 0x4000_4400),
            Self::usart_peripheral("USART3", 0x4000_4800),
            Self::usart_peripheral("UART4", 0x4000_4C00),
            Self::usart_peripheral("UART5", 0x4000_5000),
            Self::usart_peripheral("LPUART1", 0x4400_2000),
            // SPI peripherals (SPI1-SPI6)
            Self::spi_peripheral("SPI1", 0x4001_3000),
            Self::spi_peripheral("SPI2", 0x4000_3800),
            Self::spi_peripheral("SPI3", 0x4000_3C00),
            Self::spi_peripheral("SPI4", 0x4001_3400),
            Self::spi_peripheral("SPI5", 0x4001_5000),
            Self::spi_peripheral("SPI6", 0x4400_1400),
            // I2C peripherals (I2C1-I2C4)
            Self::i2c_peripheral("I2C1", 0x4000_5400),
            Self::i2c_peripheral("I2C2", 0x4000_5800),
            Self::i2c_peripheral("I2C3", 0x4400_2800),
            Self::i2c_peripheral("I2C4", 0x4400_2C00),
            // FDCAN peripherals
            Self::fdcan_peripheral("FDCAN1", 0x4000_A400),
            Self::fdcan_peripheral("FDCAN2", 0x4000_A800),
            // RCC (Reset and Clock Control)
            {
                let mut p = Peripheral::new("RCC", 0x4402_0C00);
                p.add_register("CR", 0x00, 4);
                p.add_register("HSICFGR", 0x10, 4);
                p.add_register("CRRCR", 0x14, 4);
                p.add_register("CFGR1", 0x1C, 4);
                p.add_register("CFGR2", 0x20, 4);
                p.add_register("PLL1CFGR", 0x28, 4);
                p.add_register("PLL1DIVR", 0x34, 4);
                p.add_register("PLL1FRACR", 0x38, 4);
                p.add_register("PLL2CFGR", 0x2C, 4);
                p.add_register("PLL2DIVR", 0x3C, 4);
                p.add_register("CIER", 0x50, 4);
                p.add_register("AHB1ENR", 0x88, 4);
                p.add_register("AHB2ENR", 0x8C, 4);
                p.add_register("APB1LENR", 0x9C, 4);
                p.add_register("APB1HENR", 0xA0, 4);
                p.add_register("APB2ENR", 0xA4, 4);
                p.add_register("APB3ENR", 0xA8, 4);
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
            // ICACHE
            {
                let mut p = Peripheral::new("ICACHE", 0x4000_1400);
                p.add_register("CR", 0x00, 4);
                p.add_register("SR", 0x04, 4);
                p.add_register("IER", 0x08, 4);
                p.add_register("FCR", 0x0C, 4);
                p.add_register("HMONR", 0x10, 4);
                p.add_register("MMONR", 0x14, 4);
                p
            },
            // TIM1
            {
                let mut p = Peripheral::new("TIM1", 0x4001_2C00);
                p.add_register("CR1", 0x00, 4);
                p.add_register("CR2", 0x04, 4);
                p.add_register("PSC", 0x28, 4);
                p.add_register("ARR", 0x2C, 4);
                p
            },
        ]
    }

    fn vector_table_size(&self) -> usize {
        // 16 Cortex-M system exceptions + 150 STM32H5 IRQs = 166
        166
    }

    fn cpu_frequency(&self) -> u32 {
        250_000_000 // 250 MHz (HSE 25MHz -> PLL1)
    }

    fn generate_startup_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated startup code for ");
        code.push_str(self.name());
        code.push_str(" */\n\n");

        code.push_str(".syntax unified\n");
        code.push_str(".cpu cortex-m33\n");
        code.push_str(".fpu fpv5-sp-d16\n");
        code.push_str(".thumb\n\n");

        code.push_str(".global Reset_Handler\n");
        code.push_str(".type Reset_Handler, %function\n");
        code.push_str("Reset_Handler:\n");

        // Copy .data from Flash to RAM
        code.push_str("  ldr r0, =_sdata\n");
        code.push_str("  ldr r1, =_edata\n");
        code.push_str("  ldr r2, =_sidata\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b CopyDataLoop\n\n");
        code.push_str("CopyDataInit:\n");
        code.push_str("  ldr r4, [r2, r3]\n");
        code.push_str("  str r4, [r0, r3]\n");
        code.push_str("  adds r3, r3, #4\n\n");
        code.push_str("CopyDataLoop:\n");
        code.push_str("  adds r4, r0, r3\n");
        code.push_str("  cmp r4, r1\n");
        code.push_str("  bcc CopyDataInit\n\n");

        // Zero .bss
        code.push_str("  ldr r2, =_sbss\n");
        code.push_str("  ldr r4, =_ebss\n");
        code.push_str("  movs r3, #0\n");
        code.push_str("  b ZeroBssLoop\n\n");
        code.push_str("ZeroBssInit:\n");
        code.push_str("  str r3, [r2]\n");
        code.push_str("  adds r2, r2, #4\n\n");
        code.push_str("ZeroBssLoop:\n");
        code.push_str("  cmp r2, r4\n");
        code.push_str("  bcc ZeroBssInit\n\n");

        // Enable FPU (Cortex-M33 with FPv5)
        code.push_str("  /* Enable FPU */\n");
        code.push_str("  ldr r0, =0xE000ED88\n");
        code.push_str("  ldr r1, [r0]\n");
        code.push_str("  orr r1, r1, #(0xF << 20)\n");
        code.push_str("  str r1, [r0]\n");
        code.push_str("  dsb\n");
        code.push_str("  isb\n\n");

        // Enable ICACHE
        code.push_str("  /* Enable ICACHE */\n");
        code.push_str("  ldr r0, =0x40001400\n");
        code.push_str("  ldr r1, [r0]\n");
        code.push_str("  orr r1, r1, #1\n");
        code.push_str("  str r1, [r0]\n\n");

        // Call main
        code.push_str("  bl main\n");
        code.push_str("  b .\n\n");

        // Vector table
        code.push_str(".section .isr_vector, \"a\", %progbits\n");
        code.push_str(".type g_pfnVectors, %object\n");
        code.push_str("g_pfnVectors:\n");
        code.push_str("  .word _estack\n");
        code.push_str("  .word Reset_Handler\n");

        let handler_names = [
            "NMI_Handler",
            "HardFault_Handler",
            "MemManage_Handler",
            "BusFault_Handler",
            "UsageFault_Handler",
        ];
        for name in &handler_names {
            code.push_str(&format!("  .word {name}\n"));
        }
        // Reserved + SecureFault (Cortex-M33 TrustZone)
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word SecureFault_Handler\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word SVC_Handler\n");
        code.push_str("  .word DebugMon_Handler\n");
        code.push_str("  .word 0  /* Reserved */\n");
        code.push_str("  .word PendSV_Handler\n");
        code.push_str("  .word SysTick_Handler\n");

        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RCC Clock Configuration (Sprint 29)
// ═══════════════════════════════════════════════════════════════════════

/// Clock frequencies computed from RCC configuration.
#[derive(Debug, Clone)]
pub struct H5ClockFrequencies {
    /// System clock (SYSCLK) in Hz.
    pub sysclk: u32,
    /// AHB clock (HCLK) in Hz.
    pub ahb: u32,
    /// APB1 clock in Hz.
    pub apb1: u32,
    /// APB2 clock in Hz.
    pub apb2: u32,
    /// APB3 clock in Hz.
    pub apb3: u32,
}

/// RCC clock configuration for STM32H5.
///
/// HSE (25 MHz external crystal) -> PLL1 -> 250 MHz system clock.
#[derive(Debug, Clone)]
pub struct H5RccConfig {
    /// HSI frequency in Hz (always 64 MHz on H5).
    pub hsi: u32,
    /// HSE crystal frequency in Hz.
    pub hse: u32,
    /// PLL1 M divider (1-63).
    pub pll_m: u8,
    /// PLL1 N multiplier (4-512).
    pub pll_n: u16,
    /// PLL1 P divider (1-128).
    pub pll_p: u8,
    /// PLL1 Q divider (1-128).
    pub pll_q: u8,
    /// Target system clock in Hz.
    pub system_clock: u32,
    /// AHB prescaler (1, 2, 4, 8, 16, 64, 128, 256, 512).
    pub ahb_prescaler: u16,
    /// APB1 prescaler (1, 2, 4, 8, 16).
    pub apb1_prescaler: u8,
    /// APB2 prescaler (1, 2, 4, 8, 16).
    pub apb2_prescaler: u8,
    /// APB3 prescaler (1, 2, 4, 8, 16).
    pub apb3_prescaler: u8,
}

impl H5RccConfig {
    /// Default configuration for 250 MHz from 25 MHz HSE.
    ///
    /// HSE=25MHz, PLLM=5, PLLN=100, PLLP=2, PLLQ=5
    /// -> VCO_in = 25/5 = 5 MHz
    /// -> VCO_out = 5 * 100 = 500 MHz
    /// -> SYSCLK = 500/2 = 250 MHz
    /// -> USB = 500/5 * 2 = ... (via PLL2/PLL3 in real usage)
    pub fn default_250mhz() -> Self {
        Self {
            hsi: 64_000_000,
            hse: 25_000_000,
            pll_m: 5,
            pll_n: 100,
            pll_p: 2,
            pll_q: 5,
            system_clock: 250_000_000,
            ahb_prescaler: 1,
            apb1_prescaler: 2, // 250/2 = 125 MHz
            apb2_prescaler: 2, // 250/2 = 125 MHz
            apb3_prescaler: 2, // 250/2 = 125 MHz
        }
    }

    /// Returns the PLL1CFGR register value.
    ///
    /// Configures PLL1 source (HSE), M divider, and enables P/Q outputs.
    pub fn pllcfgr_value(&self) -> u32 {
        let mut val = 0u32;
        // PLL1SRC: HSE = 0b11 (bits 0-1)
        val |= 0b11;
        // PLL1M: bits 8-13
        val |= (self.pll_m as u32 & 0x3F) << 8;
        // PLL1PEN: enable P output (bit 16)
        val |= 1 << 16;
        // PLL1QEN: enable Q output (bit 17)
        val |= 1 << 17;
        val
    }

    /// Returns the PLL1DIVR register value.
    ///
    /// Contains N, P, Q dividers.
    pub fn pll1divr_value(&self) -> u32 {
        let mut val = 0u32;
        // PLL1N: bits 0-8 (value - 1)
        val |= ((self.pll_n - 1) as u32) & 0x1FF;
        // PLL1P: bits 9-15 (value - 1)
        val |= (((self.pll_p - 1) as u32) & 0x7F) << 9;
        // PLL1Q: bits 16-22 (value - 1)
        val |= (((self.pll_q - 1) as u32) & 0x7F) << 16;
        val
    }

    /// Returns the CFGR1 register value.
    ///
    /// Selects PLL1 as system clock source.
    pub fn cfgr_value(&self) -> u32 {
        let mut val = 0u32;
        // SW: PLL1 as system clock = 0b11 (bits 0-2)
        val |= 0b011;
        val
    }

    /// Computes clock frequencies from the configuration.
    pub fn compute_clocks(&self) -> H5ClockFrequencies {
        let vco_in = self.hse / self.pll_m as u32;
        let vco_out = vco_in * self.pll_n as u32;
        let sysclk = vco_out / self.pll_p as u32;
        let ahb = sysclk / self.ahb_prescaler as u32;
        let apb1 = ahb / self.apb1_prescaler as u32;
        let apb2 = ahb / self.apb2_prescaler as u32;
        let apb3 = ahb / self.apb3_prescaler as u32;

        H5ClockFrequencies {
            sysclk,
            ahb,
            apb1,
            apb2,
            apb3,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// GPIO HAL (Sprint 30)
// ═══════════════════════════════════════════════════════════════════════

/// GPIO pin mode for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5GpioMode {
    /// Input mode.
    Input = 0b00,
    /// General-purpose output.
    Output = 0b01,
    /// Alternate function.
    AlternateFunction = 0b10,
    /// Analog.
    Analog = 0b11,
}

/// GPIO output type for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5GpioOutputType {
    /// Push-pull (default).
    PushPull = 0,
    /// Open-drain.
    OpenDrain = 1,
}

/// GPIO speed for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5GpioSpeed {
    /// Low speed.
    Low = 0b00,
    /// Medium speed.
    Medium = 0b01,
    /// High speed.
    High = 0b10,
    /// Very high speed.
    VeryHigh = 0b11,
}

/// GPIO pull resistor for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5GpioPull {
    /// No pull.
    None = 0b00,
    /// Pull-up.
    PullUp = 0b01,
    /// Pull-down.
    PullDown = 0b10,
}

/// GPIO pin configuration for STM32H5.
#[derive(Debug, Clone)]
pub struct H5GpioConfig {
    /// Pin number (0-15).
    pub pin: u8,
    /// Port letter ('A'-'I').
    pub port: char,
    /// Pin mode.
    pub mode: H5GpioMode,
    /// Output type.
    pub output_type: H5GpioOutputType,
    /// Speed.
    pub speed: H5GpioSpeed,
    /// Pull resistor.
    pub pull: H5GpioPull,
    /// Alternate function number (0-15).
    pub alternate_function: u8,
}

impl H5GpioConfig {
    /// Creates a default GPIO config (input, no pull).
    pub fn new(port: char, pin: u8) -> Self {
        Self {
            pin,
            port,
            mode: H5GpioMode::Input,
            output_type: H5GpioOutputType::PushPull,
            speed: H5GpioSpeed::Low,
            pull: H5GpioPull::None,
            alternate_function: 0,
        }
    }

    /// Configures as output push-pull.
    pub fn as_output(mut self) -> Self {
        self.mode = H5GpioMode::Output;
        self
    }

    /// Configures as alternate function.
    pub fn as_alt_func(mut self, af: u8) -> Self {
        self.mode = H5GpioMode::AlternateFunction;
        self.alternate_function = af;
        self
    }

    /// Sets the speed.
    pub fn with_speed(mut self, speed: H5GpioSpeed) -> Self {
        self.speed = speed;
        self
    }

    /// Sets the pull resistor.
    pub fn with_pull(mut self, pull: H5GpioPull) -> Self {
        self.pull = pull;
        self
    }

    /// Returns the MODER register value for this pin's 2-bit field.
    pub fn moder_value(&self) -> u32 {
        (self.mode as u32) << (self.pin * 2)
    }

    /// Returns the MODER register mask for this pin's 2-bit field.
    pub fn moder_mask(&self) -> u32 {
        0b11 << (self.pin * 2)
    }

    /// Returns the OTYPER register value for this pin's 1-bit field.
    pub fn otyper_value(&self) -> u32 {
        (self.output_type as u32) << self.pin
    }

    /// Returns the OSPEEDR register value for this pin's 2-bit field.
    pub fn ospeedr_value(&self) -> u32 {
        (self.speed as u32) << (self.pin * 2)
    }

    /// Returns the OSPEEDR register mask for this pin's 2-bit field.
    pub fn ospeedr_mask(&self) -> u32 {
        0b11 << (self.pin * 2)
    }

    /// Returns the PUPDR register value for this pin's 2-bit field.
    pub fn pupdr_value(&self) -> u32 {
        (self.pull as u32) << (self.pin * 2)
    }

    /// Returns the PUPDR register mask for this pin's 2-bit field.
    pub fn pupdr_mask(&self) -> u32 {
        0b11 << (self.pin * 2)
    }

    /// Returns the AFR register value (AFRL for pins 0-7, AFRH for 8-15).
    pub fn afr_value(&self) -> (u32, u32) {
        let af = self.alternate_function as u32 & 0x0F;
        if self.pin < 8 {
            // AFRL: 4 bits per pin
            let shift = self.pin * 4;
            (af << shift, 0)
        } else {
            // AFRH: 4 bits per pin
            let shift = (self.pin - 8) * 4;
            (0, af << shift)
        }
    }

    /// Returns the port base address.
    pub fn port_base_address(&self) -> u32 {
        Stm32H5::gpio_base(self.port)
    }

    /// Returns the BSRR value to set this pin high.
    pub fn bsrr_set(&self) -> u32 {
        1 << self.pin
    }

    /// Returns the BSRR value to reset this pin low.
    pub fn bsrr_reset(&self) -> u32 {
        1 << (self.pin + 16)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// UART HAL (Sprint 30)
// ═══════════════════════════════════════════════════════════════════════

/// UART word length for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5UartWordLength {
    /// 7-bit word.
    Bits7,
    /// 8-bit word (default).
    Bits8,
    /// 9-bit word.
    Bits9,
}

/// UART stop bits for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5UartStopBits {
    /// 1 stop bit (default).
    Stop1 = 0b00,
    /// 0.5 stop bits.
    Stop0_5 = 0b01,
    /// 2 stop bits.
    Stop2 = 0b10,
    /// 1.5 stop bits.
    Stop1_5 = 0b11,
}

/// UART parity for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5UartParity {
    /// No parity (default).
    None,
    /// Even parity.
    Even,
    /// Odd parity.
    Odd,
}

/// UART oversampling mode for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5UartOversampling {
    /// 16x oversampling (default).
    Over16,
    /// 8x oversampling.
    Over8,
}

/// UART configuration for STM32H5.
#[derive(Debug, Clone)]
pub struct H5UartConfig {
    /// Baud rate.
    pub baud_rate: u32,
    /// Word length.
    pub word_length: H5UartWordLength,
    /// Stop bits.
    pub stop_bits: H5UartStopBits,
    /// Parity.
    pub parity: H5UartParity,
    /// Oversampling mode.
    pub oversampling: H5UartOversampling,
    /// FIFO mode enabled.
    pub fifo_enabled: bool,
}

impl H5UartConfig {
    /// Creates a default UART config (115200 8N1, 16x oversampling, no FIFO).
    pub fn new(baud_rate: u32) -> Self {
        Self {
            baud_rate,
            word_length: H5UartWordLength::Bits8,
            stop_bits: H5UartStopBits::Stop1,
            parity: H5UartParity::None,
            oversampling: H5UartOversampling::Over16,
            fifo_enabled: false,
        }
    }

    /// Computes the BRR register value for a given peripheral clock.
    ///
    /// For OVER16: BRR = clock_hz / baud_rate
    /// For OVER8:  BRR = 2 * clock_hz / baud_rate (with special bit arrangement)
    pub fn brr_value(&self, clock_hz: u32) -> u32 {
        match self.oversampling {
            H5UartOversampling::Over16 => {
                // BRR = f_ck / baud_rate (rounded to nearest)
                (clock_hz + self.baud_rate / 2) / self.baud_rate
            }
            H5UartOversampling::Over8 => {
                // USARTDIV = 2 * f_ck / baud_rate
                let usartdiv = (2 * clock_hz + self.baud_rate / 2) / self.baud_rate;
                // BRR[15:4] = USARTDIV[15:4]
                // BRR[3] = 0 (must be kept cleared)
                // BRR[2:0] = USARTDIV[3:1] (right-shifted by 1)
                let mantissa = usartdiv & 0xFFF0;
                let fraction = (usartdiv & 0x000F) >> 1;
                mantissa | fraction
            }
        }
    }

    /// Returns the CR1 register value.
    ///
    /// Configures word length, parity, oversampling, FIFO, and enables UE/TE/RE.
    pub fn cr1_value(&self) -> u32 {
        let mut cr1 = 0u32;
        // UE: USART enable (bit 0)
        cr1 |= 1;
        // TE: Transmitter enable (bit 3)
        cr1 |= 1 << 3;
        // RE: Receiver enable (bit 2)
        cr1 |= 1 << 2;

        // Word length: M1 (bit 28), M0 (bit 12)
        match self.word_length {
            H5UartWordLength::Bits7 => {
                cr1 |= 1 << 28; // M1=1, M0=0
            }
            H5UartWordLength::Bits8 => {} // M1=0, M0=0 (default)
            H5UartWordLength::Bits9 => {
                cr1 |= 1 << 12; // M1=0, M0=1
            }
        }

        // Parity
        match self.parity {
            H5UartParity::None => {}
            H5UartParity::Even => {
                cr1 |= 1 << 10; // PCE
            }
            H5UartParity::Odd => {
                cr1 |= 1 << 10; // PCE
                cr1 |= 1 << 9; // PS (odd)
            }
        }

        // Oversampling
        if self.oversampling == H5UartOversampling::Over8 {
            cr1 |= 1 << 15; // OVER8
        }

        // FIFO mode
        if self.fifo_enabled {
            cr1 |= 1 << 29; // FIFOEN
        }

        cr1
    }

    /// Returns the CR2 register value.
    ///
    /// Configures stop bits.
    pub fn cr2_value(&self) -> u32 {
        let mut cr2 = 0u32;
        // STOP bits (bits 13:12)
        cr2 |= (self.stop_bits as u32) << 12;
        cr2
    }

    /// Returns the CR3 register value.
    ///
    /// Currently returns 0 (default configuration).
    pub fn cr3_value(&self) -> u32 {
        0
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SPI HAL (Sprint 30)
// ═══════════════════════════════════════════════════════════════════════

/// SPI role for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5SpiRole {
    /// Master mode.
    Master,
    /// Slave mode.
    Slave,
}

/// SPI prescaler for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5SpiPrescaler {
    /// Divide by 2.
    Div2 = 0b000,
    /// Divide by 4.
    Div4 = 0b001,
    /// Divide by 8.
    Div8 = 0b010,
    /// Divide by 16.
    Div16 = 0b011,
    /// Divide by 32.
    Div32 = 0b100,
    /// Divide by 64.
    Div64 = 0b101,
    /// Divide by 128.
    Div128 = 0b110,
    /// Divide by 256.
    Div256 = 0b111,
}

/// SPI configuration for STM32H5.
#[derive(Debug, Clone)]
pub struct H5SpiConfig {
    /// Master or slave.
    pub role: H5SpiRole,
    /// Clock polarity (false = idle low, true = idle high).
    pub cpol: bool,
    /// Clock phase (false = first edge, true = second edge).
    pub cpha: bool,
    /// Prescaler.
    pub prescaler: H5SpiPrescaler,
    /// Frame size in bits (4-32).
    pub frame_size: u8,
    /// FIFO threshold level (1-16).
    pub fifo_threshold: u8,
}

impl H5SpiConfig {
    /// Creates a default SPI config (master, mode 0, div8, 8-bit, FIFO threshold 4).
    pub fn new() -> Self {
        Self {
            role: H5SpiRole::Master,
            cpol: false,
            cpha: false,
            prescaler: H5SpiPrescaler::Div8,
            frame_size: 8,
            fifo_threshold: 4,
        }
    }

    /// Returns the CFG1 register value.
    ///
    /// Configures prescaler, frame size (DSIZE), and FIFO threshold (FTHLV).
    pub fn cfg1_value(&self) -> u32 {
        let mut val = 0u32;
        // DSIZE: bits 4:0 (frame_size - 1)
        val |= ((self.frame_size.saturating_sub(1)) as u32) & 0x1F;
        // FTHLV: bits 7:5 (fifo_threshold - 1)
        val |= (((self.fifo_threshold.saturating_sub(1)) as u32) & 0x07) << 5;
        // MBR: bits 30:28 (prescaler)
        val |= (self.prescaler as u32) << 28;
        val
    }

    /// Returns the CFG2 register value.
    ///
    /// Configures CPOL, CPHA, and master/slave mode.
    pub fn cfg2_value(&self) -> u32 {
        let mut val = 0u32;
        // CPOL: bit 25
        if self.cpol {
            val |= 1 << 25;
        }
        // CPHA: bit 24
        if self.cpha {
            val |= 1 << 24;
        }
        // MASTER: bit 22
        if self.role == H5SpiRole::Master {
            val |= 1 << 22;
        }
        // SSOE: bit 29 (SS output enable for master)
        if self.role == H5SpiRole::Master {
            val |= 1 << 29;
        }
        val
    }

    /// Returns the CR1 register value.
    ///
    /// Enables the SPI peripheral (SPE bit).
    pub fn cr1_value(&self) -> u32 {
        // SPE: bit 0 (SPI enable)
        1
    }
}

impl Default for H5SpiConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// I2C HAL (Sprint 30)
// ═══════════════════════════════════════════════════════════════════════

/// I2C speed mode for STM32H5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H5I2cSpeed {
    /// Standard mode (100 kHz).
    Standard100k,
    /// Fast mode (400 kHz).
    Fast400k,
    /// Fast mode plus (1 MHz).
    FastPlus1M,
}

/// I2C configuration for STM32H5.
#[derive(Debug, Clone)]
pub struct H5I2cConfig {
    /// Speed mode.
    pub speed: H5I2cSpeed,
}

impl H5I2cConfig {
    /// Creates a new I2C config with the given speed.
    pub fn new(speed: H5I2cSpeed) -> Self {
        Self { speed }
    }

    /// Computes the TIMINGR register value for a given I2C kernel clock.
    ///
    /// Returns a packed TIMINGR value with PRESC, SCLDEL, SDADEL, SCLH, SCLL.
    /// These values are pre-calculated for common clock frequencies.
    pub fn timingr_value(&self, clock_hz: u32) -> u32 {
        // Pre-calculated timing values for common configurations.
        // Format: (PRESC << 28) | (SCLDEL << 20) | (SDADEL << 16) | (SCLH << 8) | SCLL
        match (self.speed, clock_hz) {
            // 125 MHz I2C kernel clock (APB1 = 125 MHz)
            (H5I2cSpeed::Standard100k, 125_000_000) => {
                // PRESC=3, SCLDEL=4, SDADEL=2, SCLH=0xC7, SCLL=0xC3
                (3 << 28) | (4 << 20) | (2 << 16) | (0xC7 << 8) | 0xC3
            }
            (H5I2cSpeed::Fast400k, 125_000_000) => {
                // PRESC=1, SCLDEL=3, SDADEL=2, SCLH=0x27, SCLL=0x62
                (1 << 28) | (3 << 20) | (2 << 16) | (0x27 << 8) | 0x62
            }
            (H5I2cSpeed::FastPlus1M, 125_000_000) => {
                // PRESC=0, SCLDEL=2, SDADEL=0, SCLH=0x19, SCLL=0x31
                (2 << 20) | (0x19 << 8) | 0x31
            }
            // Fallback: compute approximate values
            (H5I2cSpeed::Standard100k, _) => {
                let presc = (clock_hz / 4_000_000).saturating_sub(1).min(15);
                let prescaled = clock_hz / (presc + 1);
                let period = prescaled / 100_000;
                let sclh = period / 2;
                let scll = period - sclh;
                (presc << 28) | (4 << 20) | (2 << 16) | ((sclh.min(255)) << 8) | scll.min(255)
            }
            (H5I2cSpeed::Fast400k, _) => {
                let presc = (clock_hz / 16_000_000).saturating_sub(1).min(15);
                let prescaled = clock_hz / (presc + 1);
                let period = prescaled / 400_000;
                let sclh = period / 3; // Asymmetric: duty ~33/67%
                let scll = period - sclh;
                (presc << 28) | (3 << 20) | (2 << 16) | ((sclh.min(255)) << 8) | scll.min(255)
            }
            (H5I2cSpeed::FastPlus1M, _) => {
                let presc = (clock_hz / 32_000_000).saturating_sub(1).min(15);
                let prescaled = clock_hz / (presc + 1);
                let period = prescaled / 1_000_000;
                let sclh = period / 3;
                let scll = period - sclh;
                (presc << 28) | (2 << 20) | ((sclh.min(255)) << 8) | scll.min(255)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SysTick (Sprint 30)
// ═══════════════════════════════════════════════════════════════════════

/// SysTick configuration for STM32H5.
#[derive(Debug, Clone)]
pub struct H5SysTickConfig {
    _private: (),
}

impl H5SysTickConfig {
    /// Returns the SysTick LOAD value for 1ms tick at the given clock frequency.
    ///
    /// LOAD = clock_hz / 1000 - 1
    pub fn tick_1ms(clock_hz: u32) -> u32 {
        clock_hz / 1000 - 1
    }

    /// Returns assembly code for a busy-wait delay using SysTick.
    pub fn delay_ms_code(clock_hz: u32) -> String {
        format!(
            r#"/* delay_ms({clock_hz}Hz, 1ms tick) */
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
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ICACHE (Sprint 29)
// ═══════════════════════════════════════════════════════════════════════

/// ICACHE configuration for STM32H5.
pub struct H5Icache;

impl H5Icache {
    /// Returns assembly code to enable the instruction cache.
    ///
    /// The ICACHE peripheral is at 0x4000_1400.
    pub fn icache_enable_code() -> String {
        String::from(
            r#"/* Enable STM32H5 ICACHE */
  ldr r0, =0x40001400  /* ICACHE_CR */
  ldr r1, [r0]
  orr r1, r1, #1       /* EN bit */
  str r1, [r0]
  dsb
  isb
"#,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FDCAN HAL (Sprint 31)
// ═══════════════════════════════════════════════════════════════════════

/// FDCAN bit timing for STM32H5.
///
/// Used for both nominal and data bit rate phases.
#[derive(Debug, Clone)]
pub struct H5FdcanBitTiming {
    /// Prescaler (1-512 for nominal, 1-32 for data).
    pub prescaler: u16,
    /// Synchronization jump width (1-128 for nominal, 1-16 for data).
    pub sjw: u8,
    /// Time segment 1 (1-256 for nominal, 1-32 for data).
    pub tseg1: u16,
    /// Time segment 2 (1-128 for nominal, 1-16 for data).
    pub tseg2: u8,
}

impl H5FdcanBitTiming {
    /// Returns the NBTP (Nominal Bit Timing and Prescaler) register value.
    ///
    /// NSJW [31:25], NBRP [24:16], NTSEG1 [15:8], NTSEG2 [6:0]
    pub fn nbtp_value(&self) -> u32 {
        let mut val = 0u32;
        // NSJW: bits 31:25 (value - 1)
        val |= ((self.sjw.saturating_sub(1) as u32) & 0x7F) << 25;
        // NBRP: bits 24:16 (prescaler - 1)
        val |= ((self.prescaler.saturating_sub(1) as u32) & 0x1FF) << 16;
        // NTSEG1: bits 15:8 (value - 1)
        val |= ((self.tseg1.saturating_sub(1) as u32) & 0xFF) << 8;
        // NTSEG2: bits 6:0 (value - 1)
        val |= (self.tseg2.saturating_sub(1) as u32) & 0x7F;
        val
    }

    /// Returns the DBTP (Data Bit Timing and Prescaler) register value.
    ///
    /// DSJW [3:0], DTSEG2 [7:4], DTSEG1 [12:8], DBRP [20:16], TDC [23]
    pub fn dbtp_value(&self) -> u32 {
        let mut val = 0u32;
        // DSJW: bits 3:0 (value - 1)
        val |= (self.sjw.saturating_sub(1) as u32) & 0x0F;
        // DTSEG2: bits 7:4 (value - 1)
        val |= ((self.tseg2.saturating_sub(1) as u32) & 0x0F) << 4;
        // DTSEG1: bits 12:8 (value - 1)
        val |= ((self.tseg1.saturating_sub(1) as u32) & 0x1F) << 8;
        // DBRP: bits 20:16 (prescaler - 1)
        val |= ((self.prescaler.saturating_sub(1) as u32) & 0x1F) << 16;
        val
    }
}

/// Calculates CAN-FD bit timing parameters for a given clock and target bitrate.
///
/// Uses sample point at ~87.5% for nominal, ~80% for data.
/// Returns `None` if no valid timing can be found.
pub fn calculate_can_timing(clock_hz: u32, target_bitrate: u32) -> Option<H5FdcanBitTiming> {
    // Try prescalers from 1..=512
    for prescaler in 1..=512u16 {
        let tq_freq = clock_hz / prescaler as u32;
        if tq_freq == 0 || !tq_freq.is_multiple_of(target_bitrate) {
            continue;
        }
        let total_tq = tq_freq / target_bitrate;
        // Total TQ = 1 (sync) + tseg1 + tseg2
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
        return Some(H5FdcanBitTiming {
            prescaler,
            sjw,
            tseg1: tseg1 as u16,
            tseg2: tseg2 as u8,
        });
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprint 29: Board Definition ─────────────────────────────

    #[test]
    fn stm32h5_board_identity() {
        let board = Stm32H5::new();
        assert_eq!(board.name(), "STM32H5F5");
        assert_eq!(board.arch(), BspArch::ArmCortexM33);
        assert_eq!(board.cpu_frequency(), 250_000_000);
    }

    #[test]
    fn stm32h5_memory_regions_correct() {
        let board = Stm32H5::new();
        let regions = board.memory_regions();
        assert_eq!(regions.len(), 4);

        assert_eq!(regions[0].name, "FLASH");
        assert_eq!(regions[0].origin, 0x0800_0000);
        assert_eq!(regions[0].length, 4 * 1024 * 1024);
        assert_eq!(regions[0].attr, MemoryAttr::Rx);

        assert_eq!(regions[1].name, "SRAM1");
        assert_eq!(regions[1].origin, 0x2000_0000);
        assert_eq!(regions[1].length, 640 * 1024);

        assert_eq!(regions[2].name, "SRAM2");
        assert_eq!(regions[2].origin, 0x2004_0000);
        assert_eq!(regions[2].length, 640 * 1024);

        assert_eq!(regions[3].name, "SRAM3");
        assert_eq!(regions[3].origin, 0x2006_0000);
        assert_eq!(regions[3].length, 320 * 1024);
    }

    #[test]
    fn stm32h5_gpio_peripherals_complete() {
        let board = Stm32H5::new();
        let periphs = board.peripherals();
        let gpio_count = periphs
            .iter()
            .filter(|p| p.name.starts_with("GPIO"))
            .count();
        assert_eq!(gpio_count, 9); // A through I

        let gpioa = periphs.iter().find(|p| p.name == "GPIOA").unwrap();
        assert_eq!(gpioa.base_address, 0x4202_0000);
        assert_eq!(gpioa.register_address("MODER"), Some(0x4202_0000));
        assert_eq!(gpioa.register_address("ODR"), Some(0x4202_0014));
        assert_eq!(gpioa.register_address("BSRR"), Some(0x4202_0018));
        assert_eq!(gpioa.register_address("AFRL"), Some(0x4202_0020));
        assert_eq!(gpioa.register_address("AFRH"), Some(0x4202_0024));
    }

    #[test]
    fn stm32h5_usart_peripherals_addresses() {
        let board = Stm32H5::new();
        let periphs = board.peripherals();

        let usart1 = periphs.iter().find(|p| p.name == "USART1").unwrap();
        assert_eq!(usart1.base_address, 0x4000_C800);

        let usart2 = periphs.iter().find(|p| p.name == "USART2").unwrap();
        assert_eq!(usart2.base_address, 0x4000_4400);

        let usart3 = periphs.iter().find(|p| p.name == "USART3").unwrap();
        assert_eq!(usart3.base_address, 0x4000_4800);
    }

    #[test]
    fn stm32h5_spi_and_i2c_peripherals() {
        let board = Stm32H5::new();
        let periphs = board.peripherals();

        let spi_count = periphs.iter().filter(|p| p.name.starts_with("SPI")).count();
        assert_eq!(spi_count, 6); // SPI1-SPI6

        let i2c_count = periphs.iter().filter(|p| p.name.starts_with("I2C")).count();
        assert_eq!(i2c_count, 4); // I2C1-I2C4
    }

    #[test]
    fn stm32h5_fdcan_peripherals() {
        let board = Stm32H5::new();
        let periphs = board.peripherals();

        let fdcan1 = periphs.iter().find(|p| p.name == "FDCAN1").unwrap();
        assert_eq!(fdcan1.base_address, 0x4000_A400);
        assert_eq!(fdcan1.register_address("CCCR"), Some(0x4000_A418));
        assert_eq!(fdcan1.register_address("NBTP"), Some(0x4000_A41C));
        assert_eq!(fdcan1.register_address("DBTP"), Some(0x4000_A40C));
        assert_eq!(fdcan1.register_address("TXBAR"), Some(0x4000_A4D0));

        let fdcan2 = periphs.iter().find(|p| p.name == "FDCAN2").unwrap();
        assert_eq!(fdcan2.base_address, 0x4000_A800);
    }

    #[test]
    fn stm32h5_rcc_clock_250mhz() {
        let rcc = H5RccConfig::default_250mhz();
        assert_eq!(rcc.hsi, 64_000_000);
        assert_eq!(rcc.hse, 25_000_000);
        assert_eq!(rcc.system_clock, 250_000_000);

        let clocks = rcc.compute_clocks();
        assert_eq!(clocks.sysclk, 250_000_000);
        assert_eq!(clocks.ahb, 250_000_000);
        assert_eq!(clocks.apb1, 125_000_000);
        assert_eq!(clocks.apb2, 125_000_000);
        assert_eq!(clocks.apb3, 125_000_000);
    }

    #[test]
    fn stm32h5_rcc_pllcfgr_register() {
        let rcc = H5RccConfig::default_250mhz();
        let pllcfgr = rcc.pllcfgr_value();
        // PLL1SRC = HSE (bits 0-1 = 0b11)
        assert_eq!(pllcfgr & 0x03, 0b11);
        // PLL1M = 5 (bits 8-13)
        assert_eq!((pllcfgr >> 8) & 0x3F, 5);
        // PLL1PEN (bit 16)
        assert_ne!(pllcfgr & (1 << 16), 0);
        // PLL1QEN (bit 17)
        assert_ne!(pllcfgr & (1 << 17), 0);
    }

    #[test]
    fn stm32h5_systick_1ms() {
        let load = H5SysTickConfig::tick_1ms(250_000_000);
        assert_eq!(load, 249_999); // 250_000 - 1
    }

    #[test]
    fn stm32h5_linker_script_generation() {
        let board = Stm32H5::new();
        let script = board.generate_linker_script();
        assert!(script.contains("STM32H5F5"));
        assert!(script.contains("FLASH"));
        assert!(script.contains("SRAM1"));
        assert!(script.contains("SRAM2"));
        assert!(script.contains("SRAM3"));
        assert!(script.contains("0x08000000"));
        assert!(script.contains("4096K"));
        assert!(script.contains("ENTRY(Reset_Handler)"));
    }

    #[test]
    fn stm32h5_startup_code_cortex_m33() {
        let board = Stm32H5::new();
        let code = board.generate_startup_code();
        assert!(code.contains(".cpu cortex-m33"));
        assert!(code.contains(".fpu fpv5-sp-d16"));
        assert!(code.contains("Reset_Handler"));
        assert!(code.contains("Enable FPU"));
        assert!(code.contains("Enable ICACHE"));
        assert!(code.contains("bl main"));
        assert!(code.contains("SecureFault_Handler"));
    }

    #[test]
    fn stm32h5_icache_enable_code() {
        let code = H5Icache::icache_enable_code();
        assert!(code.contains("0x40001400"));
        assert!(code.contains("EN bit"));
        assert!(code.contains("dsb"));
        assert!(code.contains("isb"));
    }

    #[test]
    fn stm32h5_default_trait() {
        let board = Stm32H5::default();
        assert_eq!(board.name(), "STM32H5F5");
    }

    #[test]
    fn stm32h5_vector_table_size() {
        let board = Stm32H5::new();
        assert_eq!(board.vector_table_size(), 166);
    }

    // ── Sprint 30: GPIO HAL ─────────────────────────────────────

    #[test]
    fn h5_gpio_config_output() {
        let cfg = H5GpioConfig::new('B', 5).as_output();
        assert_eq!(cfg.port, 'B');
        assert_eq!(cfg.pin, 5);
        assert_eq!(cfg.mode, H5GpioMode::Output);
        assert_eq!(cfg.output_type, H5GpioOutputType::PushPull);
    }

    #[test]
    fn h5_gpio_config_alt_func() {
        let cfg = H5GpioConfig::new('A', 9).as_alt_func(7);
        assert_eq!(cfg.mode, H5GpioMode::AlternateFunction);
        assert_eq!(cfg.alternate_function, 7);
    }

    #[test]
    fn h5_gpio_moder_value() {
        let cfg = H5GpioConfig::new('A', 3).as_output();
        // Output = 0b01, pin 3 => shift by 6 bits
        assert_eq!(cfg.moder_value(), 0b01 << 6);
        assert_eq!(cfg.moder_mask(), 0b11 << 6);
    }

    #[test]
    fn h5_gpio_ospeedr_value() {
        let cfg = H5GpioConfig::new('A', 2).with_speed(H5GpioSpeed::VeryHigh);
        assert_eq!(cfg.ospeedr_value(), 0b11 << 4);
        assert_eq!(cfg.ospeedr_mask(), 0b11 << 4);
    }

    #[test]
    fn h5_gpio_pupdr_value() {
        let cfg = H5GpioConfig::new('C', 1).with_pull(H5GpioPull::PullUp);
        assert_eq!(cfg.pupdr_value(), 0b01 << 2);
    }

    #[test]
    fn h5_gpio_afr_value_low_pin() {
        let cfg = H5GpioConfig::new('A', 5).as_alt_func(7);
        let (afrl, afrh) = cfg.afr_value();
        assert_eq!(afrl, 7 << 20); // pin 5, 4 bits each
        assert_eq!(afrh, 0);
    }

    #[test]
    fn h5_gpio_afr_value_high_pin() {
        let cfg = H5GpioConfig::new('A', 10).as_alt_func(4);
        let (afrl, afrh) = cfg.afr_value();
        assert_eq!(afrl, 0);
        assert_eq!(afrh, 4 << 8); // pin 10 => (10-8)*4 = 8
    }

    #[test]
    fn h5_gpio_port_base_address() {
        assert_eq!(H5GpioConfig::new('A', 0).port_base_address(), 0x4202_0000);
        assert_eq!(H5GpioConfig::new('B', 0).port_base_address(), 0x4202_0400);
        assert_eq!(H5GpioConfig::new('I', 0).port_base_address(), 0x4202_2000);
    }

    #[test]
    fn h5_gpio_bsrr_values() {
        let cfg = H5GpioConfig::new('D', 7);
        assert_eq!(cfg.bsrr_set(), 1 << 7);
        assert_eq!(cfg.bsrr_reset(), 1 << 23);
    }

    // ── Sprint 30: UART HAL ────────────────────────────────────

    #[test]
    fn h5_uart_config_defaults() {
        let cfg = H5UartConfig::new(115_200);
        assert_eq!(cfg.baud_rate, 115_200);
        assert_eq!(cfg.word_length, H5UartWordLength::Bits8);
        assert_eq!(cfg.stop_bits, H5UartStopBits::Stop1);
        assert_eq!(cfg.parity, H5UartParity::None);
        assert_eq!(cfg.oversampling, H5UartOversampling::Over16);
        assert!(!cfg.fifo_enabled);
    }

    #[test]
    fn h5_uart_brr_over16() {
        let cfg = H5UartConfig::new(115_200);
        let brr = cfg.brr_value(125_000_000);
        // BRR = 125_000_000 / 115_200 ≈ 1085
        assert!(brr >= 1084 && brr <= 1086);
    }

    #[test]
    fn h5_uart_brr_over8() {
        let mut cfg = H5UartConfig::new(115_200);
        cfg.oversampling = H5UartOversampling::Over8;
        let brr = cfg.brr_value(125_000_000);
        // Should produce a valid BRR value
        assert!(brr > 0);
    }

    #[test]
    fn h5_uart_cr1_basic() {
        let cfg = H5UartConfig::new(115_200);
        let cr1 = cfg.cr1_value();
        assert_ne!(cr1 & 1, 0); // UE
        assert_ne!(cr1 & (1 << 3), 0); // TE
        assert_ne!(cr1 & (1 << 2), 0); // RE
        assert_eq!(cr1 & (1 << 12), 0); // M0=0 (8-bit)
        assert_eq!(cr1 & (1 << 28), 0); // M1=0
        assert_eq!(cr1 & (1 << 10), 0); // PCE=0 (no parity)
    }

    #[test]
    fn h5_uart_cr1_9bit_odd_parity_fifo() {
        let mut cfg = H5UartConfig::new(9600);
        cfg.word_length = H5UartWordLength::Bits9;
        cfg.parity = H5UartParity::Odd;
        cfg.fifo_enabled = true;
        let cr1 = cfg.cr1_value();
        assert_ne!(cr1 & (1 << 12), 0); // M0=1 (9-bit)
        assert_ne!(cr1 & (1 << 10), 0); // PCE=1
        assert_ne!(cr1 & (1 << 9), 0); // PS=1 (odd)
        assert_ne!(cr1 & (1 << 29), 0); // FIFOEN
    }

    #[test]
    fn h5_uart_cr2_stop_bits() {
        let mut cfg = H5UartConfig::new(115_200);
        cfg.stop_bits = H5UartStopBits::Stop2;
        let cr2 = cfg.cr2_value();
        assert_eq!((cr2 >> 12) & 0x03, 0b10);
    }

    // ── Sprint 30: SPI HAL ─────────────────────────────────────

    #[test]
    fn h5_spi_config_defaults() {
        let cfg = H5SpiConfig::new();
        assert_eq!(cfg.role, H5SpiRole::Master);
        assert!(!cfg.cpol);
        assert!(!cfg.cpha);
        assert_eq!(cfg.frame_size, 8);
    }

    #[test]
    fn h5_spi_cfg1_value() {
        let cfg = H5SpiConfig::new();
        let cfg1 = cfg.cfg1_value();
        // DSIZE = 8-1 = 7 (bits 4:0)
        assert_eq!(cfg1 & 0x1F, 7);
        // MBR = Div8 = 0b010 (bits 30:28)
        assert_eq!((cfg1 >> 28) & 0x07, 0b010);
    }

    #[test]
    fn h5_spi_cfg2_master_cpol_cpha() {
        let mut cfg = H5SpiConfig::new();
        cfg.cpol = true;
        cfg.cpha = true;
        let cfg2 = cfg.cfg2_value();
        assert_ne!(cfg2 & (1 << 25), 0); // CPOL
        assert_ne!(cfg2 & (1 << 24), 0); // CPHA
        assert_ne!(cfg2 & (1 << 22), 0); // MASTER
        assert_ne!(cfg2 & (1 << 29), 0); // SSOE
    }

    #[test]
    fn h5_spi_cr1_enable() {
        let cfg = H5SpiConfig::new();
        assert_eq!(cfg.cr1_value(), 1); // SPE bit
    }

    // ── Sprint 30: I2C HAL ─────────────────────────────────────

    #[test]
    fn h5_i2c_timingr_standard_125mhz() {
        let cfg = H5I2cConfig::new(H5I2cSpeed::Standard100k);
        let timingr = cfg.timingr_value(125_000_000);
        // PRESC should be 3 (bits 31:28)
        assert_eq!((timingr >> 28) & 0x0F, 3);
        assert!(timingr > 0);
    }

    #[test]
    fn h5_i2c_timingr_fast_125mhz() {
        let cfg = H5I2cConfig::new(H5I2cSpeed::Fast400k);
        let timingr = cfg.timingr_value(125_000_000);
        // PRESC should be 1
        assert_eq!((timingr >> 28) & 0x0F, 1);
        assert!(timingr > 0);
    }

    #[test]
    fn h5_i2c_timingr_fast_plus_125mhz() {
        let cfg = H5I2cConfig::new(H5I2cSpeed::FastPlus1M);
        let timingr = cfg.timingr_value(125_000_000);
        // PRESC should be 0
        assert_eq!((timingr >> 28) & 0x0F, 0);
        assert!(timingr > 0);
    }

    #[test]
    fn h5_systick_delay_ms_code() {
        let code = H5SysTickConfig::delay_ms_code(250_000_000);
        assert!(code.contains("delay_ms"));
        assert!(code.contains("COUNTFLAG"));
        assert!(code.contains("SysTick CTRL"));
    }

    // ── Sprint 31: CAN-FD ──────────────────────────────────────

    #[test]
    fn can_timing_500kbps() {
        let timing = calculate_can_timing(250_000_000, 500_000);
        assert!(timing.is_some());
        let t = timing.unwrap();
        // Verify the timing produces correct bitrate
        let total_tq = 1 + t.tseg1 as u32 + t.tseg2 as u32;
        let actual_bitrate = 250_000_000 / (t.prescaler as u32 * total_tq);
        assert_eq!(actual_bitrate, 500_000);
    }

    #[test]
    fn can_timing_1mbps() {
        let timing = calculate_can_timing(250_000_000, 1_000_000);
        assert!(timing.is_some());
        let t = timing.unwrap();
        let total_tq = 1 + t.tseg1 as u32 + t.tseg2 as u32;
        let actual_bitrate = 250_000_000 / (t.prescaler as u32 * total_tq);
        assert_eq!(actual_bitrate, 1_000_000);
    }

    #[test]
    fn can_timing_nbtp_register() {
        let timing = H5FdcanBitTiming {
            prescaler: 5,
            sjw: 4,
            tseg1: 31,
            tseg2: 8,
        };
        let nbtp = timing.nbtp_value();
        // NSJW = 4-1 = 3 at bits 31:25
        assert_eq!((nbtp >> 25) & 0x7F, 3);
        // NBRP = 5-1 = 4 at bits 24:16
        assert_eq!((nbtp >> 16) & 0x1FF, 4);
        // NTSEG1 = 31-1 = 30 at bits 15:8
        assert_eq!((nbtp >> 8) & 0xFF, 30);
        // NTSEG2 = 8-1 = 7 at bits 6:0
        assert_eq!(nbtp & 0x7F, 7);
    }

    #[test]
    fn can_timing_dbtp_register() {
        let timing = H5FdcanBitTiming {
            prescaler: 2,
            sjw: 3,
            tseg1: 10,
            tseg2: 5,
        };
        let dbtp = timing.dbtp_value();
        // DSJW = 3-1 = 2 at bits 3:0
        assert_eq!(dbtp & 0x0F, 2);
        // DTSEG2 = 5-1 = 4 at bits 7:4
        assert_eq!((dbtp >> 4) & 0x0F, 4);
        // DTSEG1 = 10-1 = 9 at bits 12:8
        assert_eq!((dbtp >> 8) & 0x1F, 9);
        // DBRP = 2-1 = 1 at bits 20:16
        assert_eq!((dbtp >> 16) & 0x1F, 1);
    }

    #[test]
    fn stm32h5_gpio_base_calculation() {
        assert_eq!(Stm32H5::gpio_base('A'), 0x4202_0000);
        assert_eq!(Stm32H5::gpio_base('B'), 0x4202_0400);
        assert_eq!(Stm32H5::gpio_base('I'), 0x4202_2000);
    }

    #[test]
    fn h5_rcc_pll1divr_value() {
        let rcc = H5RccConfig::default_250mhz();
        let divr = rcc.pll1divr_value();
        // PLL1N = 100-1 = 99 (bits 8:0)
        assert_eq!(divr & 0x1FF, 99);
        // PLL1P = 2-1 = 1 (bits 15:9)
        assert_eq!((divr >> 9) & 0x7F, 1);
        // PLL1Q = 5-1 = 4 (bits 22:16)
        assert_eq!((divr >> 16) & 0x7F, 4);
    }

    #[test]
    fn h5_gpio_otyper_open_drain() {
        let mut cfg = H5GpioConfig::new('A', 0).as_output();
        cfg.output_type = H5GpioOutputType::OpenDrain;
        assert_eq!(cfg.otyper_value(), 1); // bit 0
    }
}
