# Radxa Dragon Q6A — Accessories Reference

> Comprehensive digest of https://docs.radxa.com/en/dragon/q6a/accessories
> For Fajar Lang BSP & V2.0 "Dawn" development.

---

## 1. Accessories Overview

| Category | Accessory | Key Spec |
|----------|-----------|----------|
| **Power** | Power PD 30W | 30W USB Type-C PD adapter |
| **Display** | Radxa Display 8 HD | 8" 800×1280 touch, MIPI DSI |
| **Display** | Radxa Display 10 FHD | 10.1" 1920×1200, MIPI DSI |
| **Display** | Waveshare 3.5" RPi LCD | 480×320 SPI TFT, touch (ADS7846) |
| **Camera** | Radxa Camera 4K | 4K, 31-pin FPC, MIPI CSI |
| **Camera** | Radxa Camera 8M 219 | IMX219, 8MP, 15-pin FPC |
| **Camera** | Radxa Camera 12M 577 | IMX577, 12MP, 31-pin FPC |
| **Camera** | Radxa Camera 13M 214 | 13MP, 31-pin FPC |
| **Storage** | eMMC Module | 16/32/64/128 GB |
| **Storage** | eMMC to uSD Adapter | eMMC ↔ microSD slot |
| **Storage** | eMMC USB3 Reader | Flash eMMC via USB 3.0 |
| **Storage** | UFS Module | High-performance storage |
| **PoE** | 25W PoE+ HAT for X4 | IEEE 802.3at, 25W |

---

## 2. Camera Modules

All cameras connect via MIPI CSI using FPC cables. Enable via `rsetup` → Overlays → Manage overlays.

### Common Setup (libcamera)

```bash
# Install dependencies
sudo apt update
sudo apt install build-essential git pkg-config meson ninja-build -y
sudo apt install python3-pip python3-yaml python3-jinja2 python3-ply -y
sudo apt install libyaml-dev libevent-dev libudev-dev libgnutls28-dev -y
sudo apt install libdrm-dev libjpeg-dev libglib2.0-dev -y
sudo apt install qt6-base-dev qt6-wayland-dev qtbase5-dev -y

# Build libcamera
git clone https://git.linuxtv.org/libcamera.git
cd libcamera
meson setup build --wipe -Dpipelines=simple -Dcam=enabled \
  -Dgstreamer=disabled -Dv4l2=enabled -Dqcam=enabled
ninja -C build -j$(nproc)
sudo ninja -C build install
sudo ldconfig

# Set DMA heap permissions
sudo chmod 666 /dev/dma_heap/*

# Preview (all cameras)
cd libcamera/build/src/apps/qcam/
./qcam --renderer=gles --stream pixelformat=YUYV,width=1920,height=1080
```

### Camera Comparison

| Camera | Sensor | MP | Connector | FPC Pitch | Rsetup Overlay |
|--------|--------|----|-----------|-----------|----------------|
| Camera 4K | — | 4K | 31-pin | 0.3mm | "Enable Radxa Camera 4K on CAM1" |
| Camera 8M 219 | IMX219 | 8MP | 15-pin | 1.0mm | "Enable IMX219 on CAM1" |
| Camera 12M 577 | IMX577 | 12MP | 31-pin | 0.3mm | "Enable IMX577 camera on CAM1" |
| Camera 13M 214 | — | 13MP | 31-pin | 0.3mm | "Enable Camera 13M 214 on CAM1" |

### Camera 8M 219 (IMX219) — Special Config

The IMX219 requires a sensor-specific YAML config for white balance and color correction:

```
/usr/local/share/libcamera/ipa/simple/imx219.yaml
```

### FPC Cable Types

| Camera | Cable Spec |
|--------|-----------|
| 4K, 12M 577, 13M 214 | 31-pin to 31-pin, 0.3mm pitch, opposite-side FPC |
| 8M 219 (IMX219) | 15-pin 1.0mm (camera) → 31-pin 0.3mm (board) adapter |

### Connection Checklist

1. Insert FPC cable metal contacts into camera connector
2. Insert opposite end into board MIPI CSI connector (CAM1/CAM2/CAM3)
3. Verify alignment — no skew, no exposed contacts
4. Ensure cable has no creases and connector latch is fully closed

---

## 3. Waveshare 3.5" SPI LCD

| Feature | Value |
|---------|-------|
| Size | 3.5 inches |
| Resolution | 480 × 320 pixels |
| Interface | SPI (SPI1), 125 MHz high-speed |
| Connection | 26-pin to 40-pin GPIO header |
| Touch | ADS7846 touchscreen controller |
| Framebuffer | `/dev/fb0` or `/dev/fb1` |
| Driver | fbdev (framebuffer device) |

### Enable via Rsetup

```bash
rsetup
# Navigate: Overlays → Manage overlays
# Select: "Enable Waveshare 3.5 inch Display on SPI1"
# Reboot
```

### X11 Display Config

Create `/etc/X11/xorg.conf.d/20-modesetting.conf`:
```
Section "Device"
    Identifier "fbdev"
    Driver "fbdev"
    Option "fbdev" "/dev/fb1"
EndSection
```

### Touch Calibration

Create `/etc/X11/xorg.conf.d/99-touchscreen-calibration.conf`:
```
Section "InputClass"
    Identifier "ADS7846 Touchscreen"
    MatchProduct "ADS7846"
    Option "TransformationMatrix" "-1 0 1 0 1 0 0 0 1"
EndSection
```

---

## 4. Display Modules (MIPI DSI)

| Model | Size | Resolution | Connection |
|-------|------|------------|------------|
| Radxa Display 8 HD | 8" | 800 × 1280 | 4-lane MIPI DSI via FPC |
| Radxa Display 10 FHD | 10.1" | 1920 × 1200 | 4-lane MIPI DSI via FPC |

Both displays connect via FPC ribbon cable to the single 4-lane MIPI DSI connector.

---

## 5. Storage Modules

| Module | Capacity | Interface |
|--------|----------|-----------|
| eMMC Module | 16 / 32 / 64 / 128 GB | eMMC/UFS combo connector |
| UFS Module | — | eMMC/UFS combo connector |

### Accessories for eMMC

| Accessory | Purpose |
|-----------|---------|
| eMMC to uSD Adapter | Insert eMMC module into microSD slot on other devices |
| eMMC USB3 Reader | Flash eMMC module via USB 3.0 from host PC |

> **WARNING:** Do not use untested third-party UFS/eMMC modules — may short the SBC.

---

## 6. Power & PoE

### Power PD 30W

- 30W USB Type-C Power Delivery adapter
- 12V output, PD protocol
- Recommended primary power source

### 25W PoE+ HAT for X4

| Feature | Value |
|---------|-------|
| Standard | IEEE 802.3at (PoE+) |
| Power | 25W |
| Connection | Ethernet + GPIO header |
| Use case | Single-cable power + data |

---

## 7. Fajar Lang Integration Points

### Camera → NPU Pipeline (P0)

```
Camera (MIPI CSI) → libcamera → Frame buffer
    → Fajar Lang tensor_from_image()
    → NPU inference (QNN HTP backend)
    → GPIO output (actuator control)
```

| Priority | Integration | Description |
|----------|-------------|-------------|
| P0 | Camera capture | `camera_capture(port, width, height, format)` via V4L2/libcamera |
| P0 | Frame → Tensor | `tensor_from_frame(frame_data, width, height)` for NPU input |
| P1 | SPI display | `spi_display_write(fb_data)` for Waveshare 3.5" LCD |
| P1 | DSI display | Overlay rendering on MIPI DSI displays |
| P2 | Touch input | Read ADS7846 touch events for UI interaction |
| P2 | eMMC flash | `fj deploy --storage emmc` via eMMC USB3 Reader |

### Rsetup Automation

```bash
# Future: fj board-setup --camera imx577 --display waveshare-35
# Wraps rsetup overlay enable + reboot
```

---

*Generated from official Radxa documentation — 2026-03-12*
