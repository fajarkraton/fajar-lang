# FajarOS Nova v2.0 "Phoenix" — Architecture & Guide

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Status:** ALL 140 TASKS COMPLETE (14 sprints, 5 phases)
> **Language:** 100% Fajar Lang (@kernel context)

---

## Overview

Nova v2.0 "Phoenix" adds five major subsystems to FajarOS Nova, transforming it from a
text-mode OS into a graphical, multimedia-capable, POSIX-compliant operating system.

| Phase | Subsystem | Sprints | Tasks | LOC | Key Files |
|-------|-----------|---------|-------|-----|-----------|
| N1 | GUI Framework | 4 | 40 | ~2,500 | `nova_phoenix_gui.fj` |
| N2 | Audio Driver | 2 | 20 | ~800 | `nova_phoenix_audio.fj` |
| N3 | Real Persistence | 3 | 30 | ~900 | `nova_phoenix_persist.fj` |
| N4 | POSIX v2 | 3 | 30 | ~1,100 | `nova_phoenix_posix.fj` |
| N5 | Networking v4 | 2 | 20 | ~1,000 | `nova_phoenix_net.fj` |
| **Total** | | **14** | **140** | **~6,300** | |

---

## Phase N1: GUI Framework

### Architecture

```
VirtIO-GPU (MMIO)
    ↓
Framebuffer (640×480 BGRA, double-buffered)
    ↓
Primitives (pixel, line, rect, circle, text)
    ↓
Window Manager (16 windows, z-order, drag, taskbar)
    ↓
Widget Toolkit (button, label, textinput, checkbox, listview)
    ↓
Applications (terminal, file manager, editor, sysmon, settings)
```

### Key Components

| Component | Functions | Description |
|-----------|-----------|-------------|
| **VirtIO-GPU** | `gpu_init()`, `gpu_cmd_*()` | Device detection, resource creation, scanout |
| **Framebuffer** | `draw_pixel()`, `fb_clear()`, `fb_swap()` | Double-buffered 640×480×32bpp |
| **Primitives** | `draw_line()`, `fill_rect()`, `draw_circle()`, `fill_circle()` | Bresenham line, midpoint circle |
| **Font** | `font_init()`, `draw_char()`, `draw_text_str()` | 8×16 bitmap font from VGA BIOS |
| **Window Manager** | `create_window()`, `wm_raise_window()`, `wm_focus_window()` | 16 windows, z-order stacking |
| **Widgets** | `button_create()`, `textinput_create()`, `checkbox_create()` | Event dispatch, focus, Tab navigation |
| **Desktop** | `wm_draw_desktop()`, `wm_draw_taskbar()` | Gradient background, Start button, clock |
| **Mouse** | `mouse_init()`, `draw_cursor()`, `wm_hit_test()` | Arrow cursor, click → window/widget dispatch |

### Color Palette

16 named colors in BGRA format: `COLOR_BLACK`, `COLOR_WHITE`, `COLOR_RED`, `COLOR_GREEN`,
`COLOR_BLUE`, `COLOR_CYAN`, `COLOR_MAGENTA`, `COLOR_YELLOW`, `COLOR_GRAY`, `COLOR_DARK_GRAY`,
`COLOR_LIGHT_GRAY`, `COLOR_ORANGE`, `COLOR_PINK`, `COLOR_BROWN`, `COLOR_NAVY`, `COLOR_TEAL`.

Custom: `rgb(r, g, b)`, `color_blend(c1, c2, alpha)`.

### Applications

- **Terminal emulator** — 80×24 character buffer, scroll, cursor, shell integration
- **File manager** — ramfs directory listing via ListView widget
- **Text editor** — 64KB buffer, insert/delete, line numbers, syntax highlighting
- **System monitor** — CPU bars, memory usage, process list
- **Settings** — hostname, resolution, dark theme toggle
- **Screenshot** — capture framebuffer to ramfs
- **Calculator** — GUI window with 16 buttons

### Launch

```
startx          # switch from text to GUI mode
```

QEMU flags: `-device virtio-gpu-pci -device virtio-keyboard-pci -device virtio-mouse-pci`

---

## Phase N2: Audio Driver (Intel HDA)

### Pipeline

```
PCI Detect (class 0x04, subclass 0x03)
    ↓
Controller Reset + CORB/RIRB setup
    ↓
Codec Enumeration (verbs via CORB)
    ↓
DAC + Pin Configuration
    ↓
Stream Setup (BDL, DMA buffer, format)
    ↓
PCM Playback / Sound Generation
```

### Supported Formats

| Format | Sample Rate | Bits | Channels |
|--------|-------------|------|----------|
| `PCM_FORMAT_16BIT_48K_STEREO` | 48,000 Hz | 16 | 2 |
| `PCM_FORMAT_16BIT_44K_STEREO` | 44,100 Hz | 16 | 2 |
| `PCM_FORMAT_24BIT_48K_STEREO` | 48,000 Hz | 24 | 2 |

### System Sounds

| Sound | Function | Notes |
|-------|----------|-------|
| Startup | `sound_startup()` | C5 → E5 → G5 ascending chord |
| Error | `sound_error()` | Low descending beep |
| Notification | `sound_notification()` | A5 → C6 |
| Click | `sound_click()` | 10ms 1kHz beep |
| Shutdown | `sound_shutdown()` | G5 → E5 → C5 descending |

### Commands

```
audio_init      # detect + configure HDA controller
volume 80       # set master volume (0-100)
mute            # toggle mute
beep            # 440Hz test beep
play file.wav   # play WAV file from ramfs
```

QEMU flags: `-device intel-hda -device hda-duplex`

---

## Phase N3: Real Persistence (ext2 + Journal)

### Journal Architecture (JBD2-compatible)

```
journal_begin_txn()
    ↓
journal_add_block(block, data)   ← collect dirty blocks
    ↓
journal_commit_txn()
    ├── Write descriptor block (tags)
    ├── Write data blocks to journal
    ├── Write commit block (CRC32)
    └── Write blocks to final locations
```

### Recovery

On boot, if superblock state ≠ clean:
1. `journal_recover()` scans journal from tail to head
2. Finds descriptor blocks, replays data to final locations
3. Marks filesystem clean

### fsck Phases

| Phase | Check | Action |
|-------|-------|--------|
| 1 | Superblock magic + consistency | Fix free block count |
| 2 | Journal replay | Replay uncommitted transactions |
| 3 | Inode table | Detect orphans, size vs blocks |
| 4 | Block bitmap | Count free blocks per group |
| 5 | Directory structure | Verify `.` and `..` entries |
| 6 | Mark clean | Write SB_STATE = 1 |

### GRUB Integration

- Multiboot2 header verification (`0xE85250D6`)
- `grub.cfg` generation for chainloading
- Boot path: `GRUB → multiboot2 /boot/nova.elf`

---

## Phase N4: POSIX v2

### mmap (File-Backed)

```
sys_mmap(addr, length, prot, flags, fd, offset) → vaddr
sys_munmap(addr, length)
sys_msync(addr, length, flags)     # write-back shared mappings
```

- **32 VMAs** per process, demand-paged
- **MAP_SHARED**: writes visible to other mappers, msync to disk
- **MAP_PRIVATE**: copy-on-write (CoW)
- **MAP_ANONYMOUS**: zero-filled pages
- Page faults handled by `mmap_page_fault_handler()`

### select/poll

```
sys_poll(fds, nfds, timeout_ms) → ready_count
sys_select(nfds, readfds, writefds, exceptfds, timeout)
```

- Supports pipes, sockets, regular files, devices
- `POLLIN`, `POLLOUT`, `POLLERR`, `POLLHUP`, `POLLNVAL`
- select internally converts to poll

### Pipe v3

- **64KB buffers** (up from 4KB)
- **Named pipes** (`sys_mkfifo`)
- **Reader/writer refcounting** with proper EOF
- **Blocking I/O** with process wake-up

### /proc Filesystem

| Path | Content |
|------|---------|
| `/proc/cpuinfo` | Processor model, MHz, cores, flags |
| `/proc/meminfo` | MemTotal, MemFree, MemAvailable |
| `/proc/uptime` | Seconds since boot |
| `/proc/version` | OS version string |
| `/proc/loadavg` | Load averages |
| `/proc/<pid>/status` | Name, state, PID, PPid, threads |

### Signal Queue

- **32 signals** (POSIX standard numbers)
- **Signal queue** (16 entries per process) with sender PID
- `sys_sigaction()` — register handler with SA_RESTART, SA_SIGINFO
- `sys_sigprocmask()` — block/unblock signals (never SIGKILL/SIGSTOP)
- `sys_kill()` — send signal with automatic delivery

---

## Phase N5: Networking v4

### DHCP v2 (Full State Machine)

```
INIT → SELECTING → REQUESTING → BOUND → RENEWING → REBINDING
         ↑                         │         │
         └─────────────────────────┘─────────┘ (NAK)
```

- Automatic lease renewal at T1 (50% of lease)
- Rebinding at T2 (87.5% of lease)
- Hostname announcement (option 12)
- DNS + domain name parsing

### NTP Time Sync

- RFC 5905 client implementation
- Offset calculation: `((T2-T1) + (T3-T4)) / 2`
- System clock adjustment
- `ntp_get_unix_time()` for accurate timestamps

### Multicast (IGMPv2)

- `igmp_join(group_ip)` — join multicast group, send Membership Report
- `igmp_leave(group_ip)` — leave group, send Leave message
- Automatic multicast MAC filter (`01:00:5E:xx:xx:xx`)
- Up to 8 simultaneous multicast groups

### IPv6 Stub

- Link-local address generation from MAC (EUI-64)
- Neighbor Discovery Protocol (NDP)
- Duplicate Address Detection (DAD) via Neighbor Solicitation
- EtherType 0x86DD

### HTTP/2 Stub

- Connection preface + SETTINGS exchange
- Frame parser (9-byte header + payload)
- Frame types: DATA, HEADERS, SETTINGS, PING, GOAWAY, WINDOW_UPDATE
- HPACK-ready (static table indices)

---

## QEMU Test Command

```bash
qemu-system-x86_64 \
    -kernel nova.elf \
    -m 256M \
    -device virtio-gpu-pci \
    -device virtio-keyboard-pci \
    -device virtio-mouse-pci \
    -device intel-hda -device hda-duplex \
    -drive file=nova.img,format=raw,if=none,id=nvm \
    -device nvme,serial=nova,drive=nvm \
    -netdev user,id=net0 \
    -device virtio-net-pci,netdev=net0 \
    -serial stdio
```

---

## File Summary

| File | Lines | Phase | Content |
|------|-------|-------|---------|
| `examples/nova_phoenix_gui.fj` | ~2,500 | N1 | GUI framework (framebuffer → apps) |
| `examples/nova_phoenix_audio.fj` | ~800 | N2 | Intel HDA audio driver |
| `examples/nova_phoenix_persist.fj` | ~900 | N3 | ext2 journaling + fsck |
| `examples/nova_phoenix_posix.fj` | ~1,100 | N4 | mmap + poll + pipes + procfs + signals |
| `examples/nova_phoenix_net.fj` | ~1,000 | N5 | DHCP v2 + NTP + multicast + IPv6 + HTTP/2 |
| `docs/NOVA_PHOENIX.md` | ~200 | — | This document |

**Total: ~6,500 lines of Fajar Lang, 140 tasks across 14 sprints.**
