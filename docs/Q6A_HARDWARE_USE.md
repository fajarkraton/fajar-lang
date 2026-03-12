# Radxa Dragon Q6A — Hardware Usage Reference

> Comprehensive digest of https://docs.radxa.com/en/dragon/q6a/hardware-use
> (all 16 sub-pages). For Fajar Lang BSP & V2.0 "Dawn" development.

---

## 1. Documentation Map

| # | Section | URL Slug | Summary |
|---|---------|----------|---------|
| 1 | Hardware Information | `/hardware-info` | Block diagram, SoC overview |
| 2 | Power Interface | `/power-header` | 3 power methods (USB-C PD, header, PoE) |
| 3 | MicroSD Card Slot | `/microsd` | MicroSD/SDHC/SDXC boot + storage |
| 4 | M.2 M Key 2230 Slot | `/nvme` | PCIe Gen3 x2 NVMe SSD |
| 5 | eMMC/UFS Module | `/ufs-emmc-com` | Combo interface, boot + storage |
| 6 | Ethernet Interface | `/eth-poe` | GbE + PoE HAT support |
| 7 | USB Ports | `/usb` | 1x Type-C + 3x USB 2.0 + 1x USB 3.1 |
| 8 | HDMI Interface | `/hdmi` | HDMI display output |
| 9 | 40-Pin GPIO | `/pin-gpio` | UART/SPI/I2C/I3C/I2S/GPIO |
| 10 | MIPI CSI | `/mipi-csi` | 3x camera ports (4+2+2 lanes) |
| 11 | MIPI DSI | `/mipi-dsi` | 1x 4-lane display output |
| 12 | Antenna Interface | `/ante` | WiFi 6 + BT 5.4 antenna connectors |
| 13 | Power Button | `/power` | Power on/off + Power pin header |
| 14 | Headphone Jack | `/headphone` | 3.5mm 4-segment, input + output |
| 15 | RTC Interface | `/rtc` | CR2032 coin cell, DS1307, 2-pin 1.25mm |
| 16 | EDL Button | `/edl` | Emergency Download Mode entry |

---

## 2. Power Interface

### Three Power Supply Methods

| Method | Voltage | Min Current | Connector | Notes |
|--------|---------|-------------|-----------|-------|
| **USB Type-C (recommended)** | 12V | 2A | Type-C PD | Radxa PD 30W adapter recommended |
| **External Header** | 12V | — | 2-pin header | **WARNING: Do not reverse polarity** |
| **PoE+ HAT** | — | — | Ethernet + header | Requires PoE HAT accessory |

### Status LEDs

| LED | Color | Meaning |
|-----|-------|---------|
| Power | Green (solid) | Power OK |
| System | Blue (flashing) | System active |

### Power Button

- **Board off** → press Power button → boot
- **Board on** → press Power button → shutdown menu
- **Power Pin** → short Power pin + GND → same behavior as button

---

## 3. Storage Interfaces

### MicroSD Card Slot

| Feature | Value |
|---------|-------|
| Supported formats | MicroSD / MicroSDHC / MicroSDXC |
| Boot support | Yes (default boot if OS present) |
| Storage mode | Auto-fallback when no OS on card |

### M.2 M Key 2230 (NVMe)

| Feature | Value |
|---------|-------|
| Interface | M.2 M Key, 2230 form factor |
| Protocol | PCIe Gen3 x2 (2 lanes) |
| Read speed | ~1,649 MB/s (measured) |
| Write speed | ~1,467 MB/s (measured) |
| Max theoretical | ~2,000 MB/s |
| Boot support | Yes (if OS present and no higher-priority boot media) |

### eMMC/UFS Module Combo Interface

| Feature | Value |
|---------|-------|
| Supported modules | eMMC and UFS (single combo connector) |
| Installation | Align notch → press gently until click |
| Boot support | Yes |

> **WARNING:** Do not use untested third-party UFS/eMMC modules. Some may short the SBC and cause permanent hardware damage (not covered by warranty).

---

## 4. Connectivity

### Ethernet (Gigabit + PoE)

| Feature | Value |
|---------|-------|
| Speed | Gigabit Ethernet (1000 Mbps) |
| Interface name | `enp1s0` |
| PoE | Supported with PoE HAT accessory |
| Indicator | LED flashes when connected |

### USB Ports

| Port | Type | Mode | Max Speed |
|------|------|------|-----------|
| USB Type-C | Power input | PD 12V | — |
| USB 2.0 Type-A (×3) | Data | HOST only | 480 Mbps |
| USB 3.1 Type-A (×1) | Data | HOST + OTG | 5 Gbps |

USB 3.1 port has blue interior. Measured speeds: ~30.8 MB/s write, ~25.8 MB/s read (via `dd`).

### WiFi 6 + Bluetooth 5.4

| Feature | Value |
|---------|-------|
| WiFi | 802.11ax (WiFi 6) |
| Bluetooth | 5.4 |
| Antenna | Built-in connectors + 2 reserved for enhanced reception |
| Scan command | `sudo nmcli device wifi list` |

---

## 5. Display Interfaces

### HDMI

- Built-in HDMI output for monitor/TV
- Some monitors may be incompatible — test with alternative display if issues

### MIPI DSI

| Feature | Value |
|---------|-------|
| Lanes | 4-lane MIPI DSI |
| Connection | FPC ribbon cable |

**Supported Displays:**

| Model | Size | Resolution |
|-------|------|------------|
| Radxa Display 10 FHD | 10.1" | 1200 × 1920 |
| Radxa Display 8 HD | 8" | 800 × 1280 |

---

## 6. Camera Interfaces (MIPI CSI)

| Port | Lanes | Connector |
|------|-------|-----------|
| CSI 0 | 4-lane | FPC |
| CSI 1 | 2-lane | FPC |
| CSI 2 | 2-lane | FPC |

**Compatible Cameras:**

| Model | Notes |
|-------|-------|
| Radxa Camera 4K | High-resolution |
| Radxa Camera 8M 219 | 8MP, 219° FOV |
| Radxa Camera 12M 577 | 12MP |
| Radxa Camera 13M 214 | 13MP |

---

## 7. 40-Pin GPIO Interface

Uses `/dev/gpiochip4`. Voltage: 3.3V logic.

> **WARNING:** Improper operation may result in damage to the device hardware.

### Complete Pinout

| Pin | Label | GPIO | Func 1 | Func 2 | Func 3 | Func 4 |
|-----|-------|------|--------|--------|--------|--------|
| 1 | 3V3 | — | Power | | | |
| 2 | 5V | — | Power | | | |
| 3 | GPIO_24 | 24 | I2C6_SDA | UART6_CTS | SPI6_MISO | |
| 4 | 5V | — | Power | | | |
| 5 | GPIO_96 | 96 | | | | |
| 6 | GND | — | Ground | | | |
| 7 | — | — | | | | |
| 8 | — | — | | | | |
| 9 | GND | — | Ground | | | |
| 10 | — | — | | | | |
| 11 | GPIO_0 | 0 | I2C0_SDA | SPI0_MISO | I3C0_SDA | UART0_CTS |
| 12 | — | — | | | | |
| 13 | GPIO_97 | 97 | | | | |
| 14 | GND | — | Ground | | | |
| 15 | GPIO_1 | 1 | I2C0_SCL | SPI0_MOSI | I3C0_SCL | UART0_RFR |
| 16 | — | — | | | | |
| 17 | 3V3 | — | Power | | | |
| 18 | — | — | | | | |
| 19 | GPIO_26 | 26 | | UART6_TX | SPI6_SCLK | |
| 20 | GND | — | Ground | | | |
| 21 | GPIO_27 | 27 | | UART6_RX | SPI6_CS_0 | |
| 22 | — | — | | | | |
| 23 | GPIO_48 | 48 | I2C12_SDA | UART12_CTS | SPI12_MISO | |
| 24 | GND | — | Ground | | | |
| 25 | GND | — | Ground | | | |
| 26 | — | — | | | | |
| 27 | GPIO_55 | 55 | | UART13_RX | SPI12_CS_1 | SPI13_CS_0 |
| 28 | — | — | | | | |
| 29 | GPIO_8 | 8 | I2C2_SDA | UART2_CTS | SPI2_MISO | |
| 30 | GND | — | Ground | | | |
| 31 | GPIO_9 | 9 | I2C2_SCL | UART7_RX | SPI7_CS_0 | SPI2_MOSI |
| 32 | — | — | | | | |
| 33 | GPIO_50 | 50 | I2C14_SCL | UART12_TX | SPI12_SCLK | UART14_RFR |
| 34 | GND | — | Ground | | | |
| 35 | GPIO_100 | 100 | MI2S0_WS | | | |
| 36 | — | — | | | | |
| 37 | GPIO_51 | 51 | | UART12_RX | SPI12_CS_0 | |
| 38 | — | — | | | | |
| 39 | GND | — | Ground | | | |
| 40 | — | — | | | | |

### Additional Pins (from pinout table)

| GPIO | Functions |
|------|-----------|
| GPIO_30 | I2C14_SDA, UART14_TX, SPI14_SCLK/SPI14_MISO |
| GPIO_56 | I2C14_SDA, UART14_CTS, SPI14_CS_0/SPI14_MISO |
| GPIO_98 | MI2S0_DATA0 |

### Available Bus Instances

| Bus Type | Instances |
|----------|-----------|
| UART | 0, 2, 5, 6, 7, 12, 13, 14 |
| SPI | 0, 2, 5, 6, 7, 12, 13, 14 |
| I2C | 0, 2, 6, 12, 14 |
| I3C | 0 |
| MI2S | 0 |

### GPIO Python Example

```bash
sudo apt update && sudo apt install -y python3-periphery
```

```python
from periphery import GPIO
import time

# Output: GPIO_25 on /dev/gpiochip4
led = GPIO("/dev/gpiochip4", 25, "out")

# Input: GPIO_96 on /dev/gpiochip4
btn = GPIO("/dev/gpiochip4", 96, "in")

while True:
    led.write(True)   # HIGH
    time.sleep(1)
    led.write(False)  # LOW
    time.sleep(1)
```

---

## 8. Audio

### Headphone Jack

| Feature | Value |
|---------|-------|
| Connector | 3.5mm four-segment |
| Capabilities | Audio input (mic) + output (speakers) |
| Sample rate | 44.1 kHz (CD quality) |
| Channels | Stereo (2) |
| Format | WAV |
| ALSA Playback | Card 0, Device 1 (MultiMedia2 Playback) |
| ALSA Capture | Card 0, Device 2 (MultiMedia3 Capture) |

### Audio Commands

```bash
# Record 20 seconds of audio
sudo arecord -Dhw:0,2 -d 20 -f cd -r 44100 -c 2 -t wav /tmp/recording.wav

# Play audio
sudo aplay -Dhw:0,1 /tmp/recording.wav
```

---

## 9. RTC (Real-Time Clock)

| Feature | Value |
|---------|-------|
| Chip | DS1307 |
| Battery | CR2032 coin cell |
| Connector | 2-Pin 1.25mm |
| Voltage | 3.3V |
| Device | `/dev/rtc0` |

### RTC Commands

```bash
# Verify RTC detection
sudo dmesg | grep rtc
# Expected: "registered as rtc0"

# Read hardware clock
sudo hwclock -r -f /dev/rtc0

# Sync system time to RTC
sudo hwclock -w -f /dev/rtc0

# Show system time
date
```

---

## 10. EDL Button

Emergency Download Mode for Qualcomm devices (firmware flash/repair).

### Entry Procedure

```
1. Hold EDL button
2. Connect 12V Type-C PD adapter
3. Release EDL button
→ Device enters EDL mode (USB 05c6:9008)
```

---

## 11. Fajar Lang BSP Integration Points

### Storage (for deployment)

| Priority | Integration | Description |
|----------|-------------|-------------|
| P0 | NVMe deploy | `fj deploy --storage nvme` — deploy binary to M.2 NVMe SSD |
| P0 | SD card boot | Create bootable SD with Fajar Lang runtime pre-installed |
| P1 | eMMC/UFS | Production deployment to internal storage |

### GPIO (40-pin header)

| Priority | Integration | Description |
|----------|-------------|-------------|
| P0 | GPIO HAL | `gpio_write(chip, pin, value)` / `gpio_read(chip, pin)` |
| P0 | Pin config | `pin_set_function(pin, PinFunction::UartTx)` |
| P1 | I2C HAL | `i2c_read(bus, addr, reg)` / `i2c_write(bus, addr, reg, data)` |
| P1 | SPI HAL | `spi_transfer(bus, tx_data)` → rx_data |
| P2 | I2S audio | Digital audio via MI2S0 pins |

### Audio

| Priority | Integration | Description |
|----------|-------------|-------------|
| P2 | Audio capture | `audio_record(device, duration, sample_rate)` |
| P2 | Audio playback | `audio_play(device, file_path)` |

### RTC

| Priority | Integration | Description |
|----------|-------------|-------------|
| P2 | RTC read | `rtc_read()` → timestamp for timestamped logging |
| P3 | RTC sync | `rtc_sync()` for offline timekeeping |

### Power Management

| Priority | Integration | Description |
|----------|-------------|-------------|
| P1 | Power status | `power_status()` → LED state, power source detection |
| P2 | Shutdown | `system_shutdown()` / `system_reboot()` |

---

*Generated from official Radxa documentation (16 pages) — 2026-03-12*
