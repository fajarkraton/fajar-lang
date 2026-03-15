# Dragon Q6A Deployment Guide

> Deploy Fajar Lang on Radxa Dragon Q6A (QCS6490) edge AI SBC.

---

## Quick Start (5 minutes)

### 1. Cross-Compile on Host (x86_64)

```bash
# Install cross-compilation toolchain (one-time)
rustup target add aarch64-unknown-linux-gnu
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

# Build for Q6A
./scripts/cross-build-q6a.sh
# Output: target/aarch64-unknown-linux-gnu/release/fj (~5.5MB stripped)
```

### 2. Deploy to Q6A

```bash
# Option A: Use deploy script
./scripts/deploy-q6a.sh <Q6A_IP>

# Option B: Manual SCP
scp target/aarch64-unknown-linux-gnu/release/fj radxa@<Q6A_IP>:~/bin/
scp -r examples/*.fj radxa@<Q6A_IP>:~/fj-examples/
```

### 3. Run on Q6A

```bash
ssh radxa@<Q6A_IP>

# Add to PATH (one-time)
sudo ln -s ~/bin/fj /usr/local/bin/fj

# Run examples
fj run ~/fj-examples/q6a_blinky.fj
fj run ~/fj-examples/q6a_uart_echo.fj
fj run ~/fj-examples/q6a_npu_classify.fj

# Start REPL
fj repl
```

---

## Board Setup

### Flash Ubuntu 24.04

1. Download image: https://docs.radxa.com/en/dragon/q6a
2. Flash to NVMe/eMMC using Qualcomm EDL mode
3. Boot and configure WiFi/SSH

### Initial Configuration

```bash
# On Q6A:
sudo apt update && sudo apt upgrade -y
sudo apt install build-essential git

# Enable GPIO access (no sudo needed)
sudo usermod -aG gpio $USER
# Log out and back in

# Verify GPIO
ls /dev/gpiochip4   # Should exist
```

---

## Hardware Builtins

Fajar Lang provides built-in functions for Q6A hardware access. On x86_64 host, these run in simulation mode.

### GPIO

```fajar
let pin = gpio_open(25)              // Open GPIO pin
gpio_set_direction(pin, "out")       // Set as output ("in" or "out")
gpio_write(pin, 1)                   // Write HIGH (1) or LOW (0)
let level = gpio_read(pin)           // Read current level
gpio_toggle(pin)                     // Toggle output level
gpio_close(pin)                      // Release pin
```

### UART

```fajar
let port = uart_open(5, 115200)      // Open UART5 at 115200 baud
uart_write_byte(port, 65)            // Write byte (0x41 = 'A')
uart_write_str(port, "Hello")        // Write string
let byte = uart_read_byte(port)      // Read byte (-1 if no data)
uart_close(port)                     // Close port
```

### PWM

```fajar
let ch = pwm_open(0)                // Open PWM channel 0 (GPIO5, pin 32)
pwm_set_frequency(ch, 50)           // Set frequency (Hz)
pwm_set_duty(ch, 75)                // Set duty cycle (0-100%)
pwm_enable(ch)                      // Enable PWM output
pwm_disable(ch)                     // Disable PWM output
pwm_close(ch)                       // Release channel
```

### SPI

```fajar
let bus = spi_open(12, 8000000)      // Open SPI12 at 8MHz
let rx = spi_transfer(bus, 0xAE)     // Full-duplex: send+receive byte
spi_write(bus, "data")               // Write string bytes
spi_close(bus)                       // Close bus
```

### NPU

```fajar
let avail = npu_available()          // true if Hexagon 770 detected
let info = npu_info()                // NPU info string
let model = npu_load("/path/to/model.bin")  // Load QNN model
let result = npu_infer(model, input) // Run NPU inference
```

### Timing

```fajar
delay_ms(100)                        // Sleep 100 milliseconds
delay_us(500)                        // Sleep 500 microseconds
```

---

## GPIO Pinout (40-pin Header)

| Pin | Function | GPIO Line | Notes |
|-----|----------|-----------|-------|
| 3 | I2C6_SDA | GPIO24 | 3.3V |
| 5 | I2C6_SCL | GPIO25 | 3.3V |
| 7 | GPIO96 | MCLK | General purpose |
| 8 | UART5_TX | GPIO22 | Serial TX |
| 10 | UART5_RX | GPIO23 | Serial RX |
| 11 | GPIO25 | Line 25 | General purpose |
| 13 | GPIO0 | Line 0 | General purpose |
| 16 | UART6_TX | GPIO8 | Serial TX |
| 18 | UART6_RX | GPIO9 | Serial RX |
| 19 | SPI12_MOSI | GPIO49 | SPI data out |
| 21 | SPI12_MISO | GPIO48 | SPI data in |
| 23 | SPI12_CLK | GPIO50 | SPI clock |
| 24 | SPI12_CS | GPIO51 | SPI chip select |
| 32 | PWM0 | GPIO5 | PWM capable |
| 33 | PWM1 | GPIO6 | PWM capable |

GPIO device: `/dev/gpiochip4`

---

## UART Ports

| Port | Device | Pins | Usage |
|------|--------|------|-------|
| UART0 | /dev/ttyMSM0 | — | Debug console |
| UART2 | /dev/ttyMSM2 | — | Bluetooth |
| UART5 | /dev/ttyMSM5 | 8, 10 | GPIO header |
| UART6 | /dev/ttyMSM6 | 16, 18 | GPIO header |
| UART7 | /dev/ttyMSM7 | — | Internal |
| UART12 | /dev/ttyMSM12 | — | Internal |
| UART14 | /dev/ttyMSM14 | — | Internal |

---

## I2C Buses

| Bus | Device | Pins | Usage |
|-----|--------|------|-------|
| I2C0 | /dev/i2c-0 | — | Internal |
| I2C2 | /dev/i2c-2 | 3, 5 | GPIO header |
| I2C6 | /dev/i2c-6 | 27, 28 | GPIO header |
| I2C7 | /dev/i2c-7 | — | Internal |
| I2C12 | /dev/i2c-12 | — | Internal |
| I2C14 | /dev/i2c-14 | — | Internal |

---

## SPI Buses

| Bus | Device | Pins | Usage |
|-----|--------|------|-------|
| SPI12 | /dev/spidev12.0 | 19, 21, 23, 24 | GPIO header |

---

## NPU (Hexagon 770)

The Hexagon 770 V68 NPU provides 12 TOPS INT8 inference via the QNN SDK.

### Setup

```bash
# On Q6A: verify NPU is available
ls /dev/fastrpc-cdsp     # FastRPC compute DSP
ls /dev/fastrpc-adsp     # Application DSP
```

### Model Deployment Pipeline

```
Host (x86_64):
  1. Train model in Fajar Lang
  2. Export: fj export --onnx model.onnx
  3. Convert: qairt-converter --input_network model.onnx
  4. Quantize: qairt-quantizer --input_dlc model.dlc --input_list calib.txt
  5. Generate: qnn-context-binary-generator --backend libQnnHtp.so

Q6A (aarch64):
  6. Deploy model.bin to /opt/fj/models/
  7. Run: fj run app.fj (uses npu_load/npu_infer builtins)
```

---

## GPU (Adreno 643)

| Metric | Performance |
|--------|------------|
| FP32 scalar | 773 GFLOPS |
| FP16 vec4 | 1,581 GFLOPS |
| INT8 dotprod | 1,176 GIOPS |
| Memory bandwidth | 9.06 GB/s |
| Vulkan | 1.1 |
| OpenCL | 2.0 |

---

## Performance Notes

- **CPU**: Kryo 670 ARMv8.2-A — 1x A78@2.7GHz + 3x A78@2.4GHz + 4x A55@1.9GHz
- **ndarray**: Automatically uses ARM NEON SIMD for tensor operations
- **Binary size**: ~5.5MB stripped (aarch64 release build)
- **Startup**: < 1 second for interpreter mode

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `fj: Exec format error` | Binary not built for aarch64; re-run `cross-build-q6a.sh` |
| `Permission denied: /dev/gpiochip4` | Add user to gpio group: `sudo usermod -aG gpio $USER` |
| `No such file: /dev/ttyMSM5` | UART5 not enabled; check device tree overlay |
| `Permission denied: /dev/i2c-2` | `sudo usermod -aG i2c $USER` |
| Cross-compile linker error | Install `gcc-aarch64-linux-gnu` |
| Binary too large | Use `aarch64-linux-gnu-strip` to strip debug symbols |

---

## Project Structure

```
fajar-lang/               # Compiler + language (this repo)
├── examples/q6a_*.fj     # Dragon Q6A example programs
├── scripts/cross-build-q6a.sh
├── scripts/deploy-q6a.sh
└── .cargo/config.toml     # Cross-compile linker config

fajar-q6a/                 # Hardware runtime library (separate repo)
├── src/gpio/              # GPIO via /dev/gpiochip4
├── src/uart/              # UART via /dev/ttyMSM*
├── src/i2c/               # I2C via /dev/i2c-*
├── src/spi/               # SPI via /dev/spidev*
├── src/npu/               # Hexagon 770 NPU (QNN SDK)
├── src/gpu/               # Adreno 643 GPU (OpenCL/Vulkan)
└── src/camera/            # MIPI CSI cameras (V4L2)
```

---

*Dragon Q6A Deployment Guide | Fajar Lang v2.0 "Dawn" | Updated: 2026-03-12*
