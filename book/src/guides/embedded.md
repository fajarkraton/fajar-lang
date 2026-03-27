# Embedded Development Guide

Fajar Lang supports cross-compilation to ARM64 and RISC-V targets
with bare-metal `no_std` support and hardware abstraction layers.

## Cross-Compilation

### ARM64

```bash
fj build --target arm64
```

In `fj.toml`:

```toml
[build]
target = "arm64"

[build.cross]
linker = "aarch64-linux-gnu-gcc"
sysroot = "/usr/aarch64-linux-gnu"
```

### RISC-V

```bash
fj build --target riscv64
```

```toml
[build]
target = "riscv64"

[build.cross]
linker = "riscv64-linux-gnu-gcc"
```

## Bare-Metal (no_std)

For firmware and kernel development without an OS:

```fajar
@kernel
fn _start() {
    // No heap, no std library -- just hardware access
    let gpio = 0x3F200000 as *mut u32
    unsafe { *gpio = 1 << 21 }  // Set GPIO pin

    loop {
        halt()
    }
}
```

Configure in `fj.toml`:

```toml
[build]
target = "arm64"
no_std = true

[build.linker]
script = "linker.ld"
entry = "_start"
```

## HAL Traits

Fajar Lang defines hardware abstraction layer traits for portable drivers:

```fajar
trait GpioPin {
    fn set_high(&mut self)
    fn set_low(&mut self)
    fn read(&self) -> bool
    fn toggle(&mut self)
}

trait SpiDevice {
    fn transfer(&mut self, data: &[u8]) -> &[u8]
    fn write(&mut self, data: &[u8])
}

trait I2cDevice {
    fn write_read(&mut self, addr: u8, write: &[u8], read: &mut [u8])
}

trait UartDevice {
    fn write_byte(&mut self, byte: u8)
    fn read_byte(&self) -> Option<u8>
}
```

### Implementing HAL for a Board

```fajar
struct Rp2040Gpio {
    base: *mut u32,
    pin: u8,
}

impl GpioPin for Rp2040Gpio {
    fn set_high(&mut self) {
        unsafe {
            let reg = self.base.offset(self.pin as i32)
            *reg = *reg | 1
        }
    }

    fn set_low(&mut self) {
        unsafe {
            let reg = self.base.offset(self.pin as i32)
            *reg = *reg & !1
        }
    }

    fn read(&self) -> bool {
        unsafe {
            let reg = self.base.offset(self.pin as i32) as *const u32
            (*reg & 1) != 0
        }
    }

    fn toggle(&mut self) {
        if self.read() { self.set_low() } else { self.set_high() }
    }
}
```

## Board Support Packages

Pre-built BSPs are available for common boards:

| BSP | Target | Board |
|-----|--------|-------|
| `fj-bsp-q6a` | ARM64 | Radxa Dragon Q6A (QCS6490) |
| `fj-bsp-rpi4` | ARM64 | Raspberry Pi 4 |
| `fj-bsp-riscv` | RISC-V | SiFive HiFive |

```fajar
use fj_bsp_q6a::{gpio, qnn}

@device
fn run_on_q6a() {
    let pin = gpio::Pin::new(12)
    pin.set_high()

    let model = qnn::load("model.dlc")
    let result = model.infer(sensor_data)
}
```

## Blinky Example

The classic embedded "hello world":

```fajar
use fj_hal::{GpioPin, delay_ms}

@kernel
fn blinky(led: &mut impl GpioPin) {
    loop {
        led.set_high()
        delay_ms(500)
        led.set_low()
        delay_ms(500)
    }
}
```

## Memory-Constrained Environments

For MCUs with limited RAM:

```fajar
@kernel
fn main() {
    // Static allocation -- no heap
    let mut buffer: [u8; 256] = [0; 256]

    // Stack-only computation
    let reading = adc_read(0)
    buffer[0] = (reading >> 8) as u8
    buffer[1] = (reading & 0xFF) as u8

    spi_write(&buffer[0..2])
}
```

## Quantized Inference on MCU

Run ML models on microcontrollers with INT8 quantization:

```fajar
@device
fn classify_gesture(accel: [f32; 3]) -> i32 {
    let model = load_quantized("gesture.bin")  // INT8, ~50KB
    let input = Tensor::from_slice(accel)
    argmax(model.forward(input))
}
```
