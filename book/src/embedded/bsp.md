# Board Support Packages

Fajar Lang provides a Board Support Package (BSP) framework for targeting specific hardware platforms.

## Supported Boards

| Board | MCU | Flash | RAM | Features |
|-------|-----|-------|-----|----------|
| STM32F407 | Cortex-M4 | 1MB | 192KB | FPU, DMA, USB |
| ESP32-S3 | Xtensa LX7 | 4MB | 512KB | WiFi, BLE, AI |
| nRF52840 | Cortex-M4 | 1MB | 256KB | BLE 5.0, NFC |
| RPi4 | Cortex-A72 | SD | 1-8GB | PCIe, USB3, Ethernet |
| Jetson Orin | Cortex-A78 | eMMC | 8-64GB | GPU, NPU, CUDA |

## Configuration

Specify your target board in `fj.toml`:

```toml
[target]
board = "stm32f407"
```

The BSP auto-configures: clock speed, memory layout, linker script, peripheral addresses, and HAL implementation.

## HAL Traits

All BSPs implement the Hardware Abstraction Layer traits:

```fajar
trait HalGpio {
    fn configure(pin: u8, mode: PinMode) -> void
    fn write(pin: u8, value: bool) -> void
    fn read(pin: u8) -> bool
}

trait HalSpi {
    fn init(config: SpiConfig) -> void
    fn transfer(data: [u8]) -> [u8]
}

trait HalI2c {
    fn write(addr: u8, data: [u8]) -> Result<(), I2cError>
    fn read(addr: u8, len: usize) -> Result<[u8], I2cError>
}

trait HalUart {
    fn init(baud: u32) -> void
    fn write(data: [u8]) -> void
    fn read() -> u8
}
```

## Flash & Run

```bash
# Flash to connected board
fj flash --board stm32f407

# Run on QEMU
fj run --qemu --board stm32f407 examples/blinky.fj
```

## Custom BSP

Create a BSP for your own board:

```fajar
@kernel
struct MyBoard {
    led_pin: u8,
    uart_baud: u32,
}

impl Board for MyBoard {
    fn init() -> Self {
        MyBoard { led_pin: 13, uart_baud: 115200 }
    }
    fn clock_hz() -> u32 { 168_000_000 }
    fn memory_layout() -> MemoryLayout { ... }
}
```
