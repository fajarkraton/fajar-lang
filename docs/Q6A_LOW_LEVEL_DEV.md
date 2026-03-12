# Radxa Dragon Q6A — Low-Level Development Reference

> Comprehensive digest of https://docs.radxa.com/en/dragon/q6a/low-level-dev
> and resource downloads. For Fajar Lang BSP & V2.0 "Dawn" development.

---

## 1. Documentation Map

| Section | URL Path | Content |
|---------|----------|---------|
| **BIOS Introduction** | `/low-level-dev/bios` | UEFI BIOS menus, boot config, EDL entry from BIOS |
| **Build System** | `/low-level-dev/build-system` | Hub for env setup, kernel, OS build |
| ├─ Environment Setup | `/build-system/install-env` | Docker + VS Code Dev Container |
| ├─ Kernel Development | `/build-system/kernel` | linux-qcom repo, `make deb` |
| └─ Radxa OS Development | `/build-system/radxa-os` | rsdk TUI, system image build |
| **Entering EDL Mode** | `/low-level-dev/edl-mode` | Emergency download mode (Qualcomm 9008) |
| **Flashing SPI Boot Firmware** | `/low-level-dev/spi-fw` | SPI NOR firmware flash/erase/recovery |

---

## 2. BIOS (UEFI Firmware)

The Dragon Q6A uses a UEFI BIOS — the first firmware to run at power-on. It initializes hardware and hands off to the OS bootloader.

### Navigation

| Key | Action |
|-----|--------|
| Arrow keys | Navigate menu items |
| Enter | Open/confirm option |
| ESC | Back / exit setup |

### Menu Sections

| Menu | Purpose |
|------|---------|
| **Language Configuration** | Switch BIOS language (English / Chinese) |
| **Radxa Platform Configuration** | Board-specific: GPIO config, camera settings |
| **Device Manager** | View/enable/disable hardware (CPU, memory, storage, peripherals) |
| **Boot Manager** | Temporary boot order override (USB, network, NVMe, eMMC, SD) |
| **Boot Maintenance Manager** | Create/modify/delete custom boot entries |

### System Functions

| Function | Description |
|----------|-------------|
| **Continue** | Save settings, resume boot |
| **Reset** | System restart |
| **Enter EDL Mode** | Enter Qualcomm Emergency Download (9008) mode from BIOS |
| **UEFI Shell** | Drop to UEFI command-line interface |
| **Boot Menu** | Select boot device |

### Fajar Lang Integration Points

- BIOS GPIO/camera settings affect BSP peripheral availability
- Boot order config relevant for NVMe vs eMMC vs SD deployment
- UEFI Shell can be used for pre-OS hardware diagnostics
- EDL mode entry from BIOS (alternative to hardware button)

---

## 3. EDL Mode (Emergency Download)

Qualcomm Emergency Download (EDL) mode — also called Qualcomm 9008 mode — is used for emergency firmware flashing, recovery, and device unlocking.

### Entry Procedure

```
1. Press and HOLD the EDL button (do NOT release)
2. Connect 12V Type-C power adapter (PD protocol)
3. Release EDL button after power connects
4. Connect USB 3.1 Type-A port to host PC (dual Type-A cable)
```

### Verification

**Linux:**
```bash
lsusb
# Expected output:
# Bus 001 Device 008: ID 05c6:9008 Qualcomm, Inc. Gobi Wireless Modem (EDL mode)
```

**Windows:**
1. Download EDL driver: `QUD_CustomInst_1.00.91.7.zip` from Resource Downloads
2. Extract and run `Install.bat` as administrator
3. Check Device Manager for Qualcomm device

### Key Details

| Parameter | Value |
|-----------|-------|
| USB Vendor:Product ID | `05c6:9008` |
| Power requirement | 12V PD via Type-C |
| Data connection | USB 3.1 Type-A (dual-A cable) |
| Entry method | Hardware button (hold before power) OR BIOS menu |

---

## 4. SPI Boot Firmware

The SPI boot firmware (stored in SPI NOR flash) is the first code executed at power-on. It contains the BootROM and Bootloader.

### Boot Chain

```
Power On
  → SPI NOR Flash (BootROM)
    → Bootloader (XBL)
      → UEFI BIOS
        → OS Kernel
```

### Boot Initialization Stages

```
Stage 1: CPU initialization
Stage 2: Memory (LPDDR5) initialization
Stage 3: Storage controller initialization
Stage 4: Load OS kernel from storage
```

### When to Flash

- Factory firmware is pre-installed — no action needed normally
- Flash only when **system fails to boot** (corrupt bootloader, failed update)

### Prerequisites

1. Device in EDL mode (see Section 3)
2. `edl-ng` tool installed
3. SPI firmware package extracted:
   - `prog_firehose_ddr.elf` — Firehose programmer
   - `rawprogram0.xml` — Partition layout
   - `patch0.xml` — Patch definitions

### Flash Commands

**Linux:**
```bash
# Install edl-ng system-wide
sudo ln -s /path/to/edl-ng /usr/local/bin/edl-ng
edl-ng --version  # verify

# Flash SPI firmware
sudo edl-ng --memory=spinor \
  rawprogram rawprogram0.xml patch0.xml \
  --loader=prog_firehose_ddr.elf
```

**Windows:**
```cmd
.\edl-ng.exe --memory=spinor ^
  --loader C:\path\to\prog_firehose_ddr.elf ^
  rawprogram C:\path\to\rawprogram0.xml ^
  C:\path\to\patch0.xml
```

### Erase SPI Partitions (Recovery)

> **WARNING:** Erasing prevents device boot. Re-flash immediately after erasing.

```bash
# Erase individual SPI partitions
sudo edl-ng --memory spinor erase-part ddr    -l prog_firehose_ddr.elf
sudo edl-ng --memory spinor erase-part uefi   -l prog_firehose_ddr.elf
sudo edl-ng --memory spinor erase-part devcfg -l prog_firehose_ddr.elf
sudo edl-ng --memory spinor erase-part xbl    -l prog_firehose_ddr.elf
```

### SPI Partition Layout

| Partition | Purpose |
|-----------|---------|
| `ddr` | LPDDR5 memory training data |
| `uefi` | UEFI BIOS firmware |
| `devcfg` | Device configuration |
| `xbl` | eXtensible Bootloader |

### Troubleshooting

| Problem | Solution |
|---------|----------|
| Windows: "Unable to load DLL 'libusb-1.0'" | Install Visual C++ Redistributable |
| Boot failure after update | Enter EDL mode, re-flash SPI firmware |
| edl-ng not found | Create symlink: `sudo ln -s /path/to/edl-ng /usr/local/bin/edl-ng` |

---

## 5. Build System — Environment Setup

Development uses Docker + VS Code Dev Containers for reproducible builds.

### Host Requirements

| Requirement | Details |
|-------------|---------|
| Architecture | x86_64 |
| OS | Ubuntu (recommended) |
| Software | Docker Engine/Desktop + VS Code + Dev Containers extension |

### Docker Installation (Linux)

```bash
# Docker Engine
sudo apt update
sudo apt install curl -y
sudo curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Add user to docker group (avoids sudo)
sudo usermod -aG docker $USER
# Restart system for group change

# Verify
docker --version
```

### Docker Desktop (Alternative)

```bash
# Download .deb from Docker Desktop page
sudo apt-get install ./docker-desktop-amd64.deb
```

### VS Code Setup

```bash
# Install VS Code
sudo apt-get install ./code_xxx_amd64.deb

# Install "Dev Containers" extension from marketplace
```

---

## 6. Kernel Development

### Source Repository

```bash
git clone --recurse-submodules https://github.com/radxa-pkg/linux-qcom.git
```

| Field | Value |
|-------|-------|
| Repository | `https://github.com/radxa-pkg/linux-qcom.git` |
| Build tool | Docker Dev Container |
| Build command | `make deb` |
| Output | Debian packages (`.deb`) |

### Development Workflow

```
1. Clone repo with submodules
2. Open directory in VS Code
3. Dev Container auto-detects config → "Reopen in Container"
4. Wait for first-time dependency install
5. Run: make deb
6. Output: .deb kernel packages
```

### Key Files (from linux-qcom repo)

| Item | Typical Location |
|------|-----------------|
| Device tree | `arch/arm64/boot/dts/qcom/` |
| Defconfig | `arch/arm64/configs/` |
| QCS6490 DTS | `qcs6490-*.dts` / `sc7280-*.dts` (QCS6490 is in SC7280 family) |
| Kernel modules | Built as part of `make deb` |

### Fajar Lang Relevance

- Custom kernel modules for Fajar Lang runtime (NPU access, GPIO fast-path)
- Device tree overlays for BSP peripheral configuration
- Kernel headers needed for FFI/syscall development
- QCS6490 is in the SC7280 SoC family — relevant for device tree lookup

---

## 7. Radxa OS Development

### Source Repository (rsdk)

```bash
git clone --recurse-submodules https://github.com/RadxaOS-SDK/rsdk.git
```

| Field | Value |
|-------|-------|
| Repository | `https://github.com/RadxaOS-SDK/rsdk.git` |
| Build tool | Docker Dev Container + rsdk TUI |
| Build command | `rsdk` (launches TUI) |
| Output | `out/<board>/output.img` |

### Build Workflow

```
1. Clone rsdk repo with submodules
2. Open in VS Code → Reopen in Dev Container
3. Wait for dependency install (first time)
4. Run: rsdk
5. Select "Build system image" from TUI
6. Choose target: radxa-dragon-q6a
7. Confirm build
8. Output: out/radxa-dragon-q6a/output.img
```

### Image Types

| Image | Sector Size | Boot Target |
|-------|-------------|-------------|
| `*_512.img.xz` | 512 bytes | MicroSD / USB / eMMC / NVMe |
| `*_4096.img.xz` | 4096 bytes | UFS |

### Fajar Lang Relevance

- Custom OS image with Fajar Lang toolchain pre-installed
- Modified rootfs with `fj` binary, stdlib, and packages
- Custom kernel modules for NPU/GPIO fast-path
- Minimal image variant for embedded deployment (no desktop)

---

## 8. Resource Downloads

### System Images

| File | Description | Source |
|------|-------------|--------|
| `radxa-dragon-q6a_noble_gnome_r2.output_512.img.xz` | Ubuntu 24.04 Noble GNOME R2 (SD/USB/eMMC/NVMe) | github.com/radxa-build/radxa-dragon-q6a/releases |
| `radxa-dragon-q6a_noble_gnome_r2.output_4096.img.xz` | Ubuntu 24.04 Noble GNOME R2 (UFS boot) | github.com/radxa-build/radxa-dragon-q6a/releases |

> **Requirement:** SPI boot firmware version `20251230` or newer

### Boot Firmware

| File | Description | Source |
|------|-------------|--------|
| `dragon-q6a_flat_build_wp_260120.zip` | SPI boot firmware (latest) | dl.radxa.com/dragon/q6a/images/ |

### Development Tools

| File | Description | Source |
|------|-------------|--------|
| `edl-ng-dist.zip` | EDL flashing tool (Linux/Windows) | dl.radxa.com/q6a/images/ |
| `QUD_CustomInst_1.00.91.7.zip` | Qualcomm USB driver (Windows) | dl.radxa.com/dragon/q6a/images/ |

### Hardware Documentation

All files version 1.21:

| File | Description | Source |
|------|-------------|--------|
| Component Placement Diagram | PDF — component locations | dl.radxa.com/dragon/q6a/hw/ |
| Schematic | PDF — full circuit schematic | dl.radxa.com/dragon/q6a/hw/ |
| 2D Dimensions Diagram | PDF — board dimensions (85x56mm) | dl.radxa.com/dragon/q6a/hw/ |
| 2D DXF File | ZIP — CAD-compatible dimensions | dl.radxa.com/dragon/q6a/hw/ |
| 3D STEP File | ZIP — 3D model for enclosure design | dl.radxa.com/dragon/q6a/hw/ |

---

## 9. Boot Architecture Summary

```
SPI NOR Flash
├── xbl          (eXtensible Bootloader — first stage)
├── devcfg       (Device configuration)
├── ddr          (LPDDR5 training data)
└── uefi         (UEFI BIOS firmware)
        │
        ▼
    UEFI BIOS
    ├── Platform Config (GPIO, Camera)
    ├── Boot Manager (NVMe/eMMC/SD/USB)
    └── System Functions (EDL, Shell)
        │
        ▼
    Storage (NVMe / eMMC / microSD)
    ├── EFI System Partition
    ├── Linux Kernel (6.16.x)
    ├── Device Tree (qcs6490-*.dtb)
    └── Root Filesystem (Ubuntu 24.04)
```

---

## 10. Fajar Lang Integration Summary

### Boot & Recovery

| Integration Point | Priority | Description |
|-------------------|----------|-------------|
| EDL recovery script | P2 | `fj deploy --recover` wraps edl-ng for firmware recovery |
| SPI firmware check | P2 | `fj board-info` verifies SPI firmware version ≥ 20251230 |
| Boot config | P3 | Document BIOS settings for optimal Fajar Lang deployment |

### Build System

| Integration Point | Priority | Description |
|-------------------|----------|-------------|
| Cross-compile kernel module | P1 | Custom `.ko` for NPU fast-path (`/dev/fastrpc-cdsp`) |
| Custom OS image | P2 | rsdk-based image with `fj` toolchain pre-installed |
| Device tree overlay | P2 | GPIO/peripheral config for Fajar Lang HAL |
| Kernel headers | P1 | Required for FFI syscall development on target |

### Deployment Pipeline

```
Host (x86_64)                          Target (Dragon Q6A)
┌────────────────┐                    ┌──────────────────┐
│ fj build       │                    │                  │
│ --board q6a    │   SSH / rsync      │ /usr/local/bin/  │
│ --target       │ ──────────────►    │   my_app         │
│ aarch64-linux  │                    │                  │
│                │                    │ systemd service   │
│ edl-ng         │   USB (EDL)        │                  │
│ (recovery)     │ ──────────────►    │ SPI NOR Flash    │
└────────────────┘                    └──────────────────┘
```

### Source Repositories

| Repo | URL | Purpose |
|------|-----|---------|
| Kernel | `https://github.com/radxa-pkg/linux-qcom.git` | Kernel source + device tree |
| OS SDK | `https://github.com/RadxaOS-SDK/rsdk.git` | System image builder |
| OS Releases | `https://github.com/radxa-build/radxa-dragon-q6a/releases` | Pre-built images |

---

*Generated from official Radxa documentation — 2026-03-12*
