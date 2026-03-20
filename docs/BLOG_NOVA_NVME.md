# Writing an NVMe Driver in a Custom Language — From Scratch

> **Author:** Fajar (PrimeCore.id)
> **Date:** 2026-03-21
> **Project:** FajarOS Nova — bare-metal x86_64 OS written 100% in Fajar Lang

---

## The Challenge

Write a working NVMe driver from scratch — no C, no Rust, no existing drivers.
The entire OS, including the NVMe driver, is written in **Fajar Lang**, a custom systems programming language.

**Result:** NVMe admin queue + I/O queue + sector read/write + FAT32 mount, verified on QEMU and KVM (Intel i9-14900HX).

---

## NVMe Architecture in 60 Seconds

NVMe is a register-level protocol for talking to SSDs over PCIe:

```
Host (CPU)                    NVMe Controller (SSD)
    │                              │
    │  1. Write command to SQ      │
    │ ──────────────────────────►  │
    │  2. Ring doorbell            │
    │ ──────────────────────────►  │
    │                              │  3. DMA: read command from SQ
    │                              │  4. Execute (read/write sectors)
    │                              │  5. DMA: write result to CQ
    │  6. Poll CQ for completion   │
    │ ◄──────────────────────────  │
    │  7. Ring CQ doorbell         │
    │ ──────────────────────────►  │
```

**Key data structures:**
- **Submission Queue (SQ):** 64-byte command entries
- **Completion Queue (CQ):** 16-byte result entries
- **Doorbells:** MMIO registers to notify controller

---

## Step 1: Find the NVMe Controller via PCI

Every NVMe device is a PCI device with class 0x01 (storage), subclass 0x08 (NVMe):

```fajar
@kernel fn nvme_find_controller() -> i64 {
    let mut dev: i64 = 0
    while dev < 32 {
        let id = pci_read32(0, dev, 0, 0)
        if id != 0xFFFFFFFF && id != 0 {
            let cl = pci_read32(0, dev, 0, 8)
            let class = (cl >> 24) & 0xFF
            let subclass = (cl >> 16) & 0xFF
            if class == 1 && subclass == 8 { return dev }
        }
        dev = dev + 1
    }
    -1
}
```

## Step 2: Map BAR0 + Enable Bus Master

The NVMe registers live in MMIO space at BAR0:

```fajar
let bar0 = pci_read32(0, dev, 0, 0x10)
let bar_addr = bar0 & 0xFFFFF000

// Enable PCI bus master (bit 2) + memory space (bit 1)
let cmd = pci_read32(0, dev, 0, 0x04)
pci_write32(0, dev, 0, 0x04, cmd | 0x06)

// Map BAR0 into our page tables
map_page(bar_addr, bar_addr, PAGE_PRESENT | PAGE_WRITABLE)
```

## Step 3: Admin Queue Setup

The admin queue is how we talk to the controller before any I/O:

```fajar
const NVME_ASQ: i64 = 0x800000  // 4KB aligned
const NVME_ACQ: i64 = 0x801000

// Tell controller where our queues are
volatile_write_u32(bar_addr + 0x24, 63 | (63 << 16))  // AQA: 64 entries each
volatile_write_u32(bar_addr + 0x28, NVME_ASQ)          // ASQ base low
volatile_write_u32(bar_addr + 0x2C, 0)                 // ASQ base high
volatile_write_u32(bar_addr + 0x30, NVME_ACQ)          // ACQ base low
volatile_write_u32(bar_addr + 0x34, 0)                 // ACQ base high

// Enable: CC = EN(1) | IOSQES(6<<16) | IOCQES(4<<20)
volatile_write_u32(bar_addr + 0x14, 1 | (6 << 16) | (4 << 20))

// Wait for CSTS.RDY = 1
while (volatile_read_u32(bar_addr + 0x1C) & 1) == 0 { }
```

## Step 4: The Phase Bit Bug

This was our hardest bug. NVMe uses a **phase bit** to distinguish new completions from old ones.

**The bug:** We read `dw3 & 1` (bit 0 = Command ID LSB) instead of `(dw3 >> 16) & 1` (bit 16 = Phase Tag).

```
CQE DW3 layout:
  Bits 31:17 = Status Code
  Bit  16    = Phase Tag    ← THIS is what we need
  Bits 15:0  = Command ID   ← We were reading this
```

**Symptom:** All admin commands "timed out" — the completion was there, but our phase check never matched.

**Fix:** One line change:
```fajar
// Before (WRONG):
let cq_phase = dw3 & 1

// After (CORRECT):
let cq_phase = (dw3 >> 16) & 1
```

## Step 5: Sector Read/Write

Once I/O queues are created, reading a sector is:

```fajar
@kernel fn nvme_read_sectors(lba: i64, count: i64, buf: i64) -> i64 {
    // Build NVM Read command (opcode 0x02)
    nvme_io_sq_write(slot, 0, 0x02 | (cmd_id << 16))  // CDW0
    nvme_io_sq_write(slot, 1, nsid)                     // NSID
    nvme_io_sq_write(slot, 6, buf & 0xFFFFFFFF)        // PRP1 low
    nvme_io_sq_write(slot, 10, lba & 0xFFFFFFFF)       // Starting LBA
    nvme_io_sq_write(slot, 12, (count - 1) & 0xFFFF)   // Sector count

    // Ring doorbell
    memory_fence()
    volatile_write_u32(bar + doorbell_offset, new_tail)

    // Wait for completion
    nvme_io_wait_completion()
}
```

## Step 6: FAT32 on NVMe

With sector read working, we mount FAT32:

```
[NVMe] Sector 0 read OK — boot signature found (0x55AA)
[FAT32] Mounted successfully
```

The FAT32 BPB is parsed from sector 0, cluster chains are followed via FAT entries, and files are read by following the chain.

---

## Results

```
NVMe init:     ~3s (QEMU) / ~8s (KVM)
Sector read:   Working (512B and 4KB)
FAT32 mount:   Working (read + write)
Persistence:   Files survive reboot
Hardware:      Verified on QEMU + KVM (i9-14900HX)
Code:          ~600 lines of Fajar Lang
```

## What We Learned

1. **NVMe is simpler than expected** — the spec is complex, but a minimal driver is ~600 lines
2. **Phase bits are critical** — one wrong bit position = silent timeout
3. **PCI bus master** — must be enabled or DMA silently fails
4. **QEMU boot order** — `-boot d` when NVMe disk is attached
5. **KVM timing differs** — real PCI config access is slower than emulated

---

*Written in Fajar Lang — where an OS kernel and an NVMe driver share the same compiler.*
