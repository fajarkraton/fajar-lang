# FajarOS Nova v0.2 — "Perseverance" Implementation Plan

> **STATUS: SUPERSEDED** — Nova v0.2 through v1.4.0 "Zenith" are complete.
> Kernel: 21,187 lines, 819 @kernel functions. All features implemented.
> See `NEXT_IMPLEMENTATION_PLAN_V8.md` for current active plan.

> **Date:** 2026-03-20
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.1.0 complete (300/300 tasks, 102 commands, 4,944 LOC)
> **Goal:** Persistent storage, real NVMe I/O, SMP, networking, loadable programs
> **Codename:** "Perseverance" — the OS that persists across reboots

---

## Current State (v0.1.0 "Discovery")

```
Kernel:      4,944 lines Fajar Lang, 104 KB ELF
Commands:    102 shell commands (system, files, hw, AI, utility)
Filesystem:  ramfs — 64 inodes @ 0x700000, 832 KB data, RAM-only
Scheduler:   Round-robin, 16 PIDs, preemptive (PIT 100Hz)
Memory:      128 MB identity-mapped, 4-level paging, bitmap allocator
Interrupts:  IDT 256 vectors, LAPIC/IOAPIC, PIC remapped
PCI:         Bus scan (32 devices), NVMe/GPU/USB/NET detected
ACPI:        RSDP/MADT parsed, shutdown via PM1a, SMP aware
Ring 3:      SYSCALL/SYSRET configured, SMEP+SMAP
Hardware:    QEMU verified + KVM on i9-14900HX
CI:          GitHub Actions — build, boot, verify (GREEN)
```

### Known Limitations
- ramfs is RAM-only (data lost on reboot)
- NVMe detected but no I/O commands
- Single-core execution (AP boot stubbed)
- No network driver (PCI detects NIC)
- No loadable programs (all built-in)
- No formal syscall dispatch table

---

## Plan Overview (6 Phases, 30 Sprints, 300 Tasks)

```
Phase 11: NVMe Block Device        [██████████]  5 sprints   — admin/IO queues, sector R/W     ✅ COMPLETE
Phase 12: FAT32 Filesystem         [██████████]  5 sprints   — BPB, cluster chains, ls/cat     ✅ COMPLETE
Phase 13: VFS + Persistence        [██████████]  5 sprints   — mount table, /dev, /proc        ✅ COMPLETE
Phase 14: SMP Multi-Core           [██████████]  5 sprints   — AP trampoline, INIT-SIPI-SIPI   ✅ COMPLETE
Phase 15: Virtio-Net + TCP/IP      [██████████]  5 sprints   — ethernet, ARP, IPv4, ICMP       ✅ COMPLETE
Phase 16: ELF Loader + Userland    [██████████]  5 sprints   — ELF64 parser, PT_LOAD, syscalls ✅ COMPLETE
```

**ALL 6 PHASES COMPLETE. Kernel: 7,313 LOC, 122 commands, 197KB ELF.**

---

## Phase 11: NVMe Block Device (5 sprints, 50 tasks)

**Goal:** Read and write 512-byte sectors on NVMe SSD
**Depends on:** Phase 10 (PCI scan, BAR mapping)
**Key challenge:** Admin queue setup, I/O submission/completion, DMA alignment

### Sprint 31: NVMe Admin Queue (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 31.1 | NVMe register map | Map all NVMe registers (CAP, VS, CC, CSTS, AQA, ASQ, ACQ) | [x] |
| 31.2 | Admin SQ allocation | Allocate 4KB-aligned submission queue (64 entries × 64B) | [x] |
| 31.3 | Admin CQ allocation | Allocate 4KB-aligned completion queue (64 entries × 16B) | [x] |
| 31.4 | Controller enable | Write CC register: enable, set queue sizes, I/O command set | [x] |
| 31.5 | Wait for ready | Poll CSTS.RDY with timeout (500ms) | [x] |
| 31.6 | Identify Controller | Submit admin command opcode 0x06, parse model/serial/firmware | [x] |
| 31.7 | Identify Namespace | Submit admin command opcode 0x06 (CNS=0), get namespace size | [x] |
| 31.8 | Namespace capacity | Parse NSZE (namespace size in LBAs), compute disk size in MB | [x] |
| 31.9 | Print NVMe info | Shell command: `nvme info` — model, serial, firmware, capacity | [x] |
| 31.10 | Error handling | Timeout, controller error, phase bit checking | [x] |

### Sprint 32: NVMe I/O Queues (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 32.1 | Create I/O CQ | Admin command: Create I/O Completion Queue (opcode 0x05) | [x] |
| 32.2 | Create I/O SQ | Admin command: Create I/O Submission Queue (opcode 0x01) | [x] |
| 32.3 | I/O SQ/CQ allocation | 4KB-aligned memory for I/O queues (separate from admin) | [x] |
| 32.4 | Doorbell registers | Map SQ/CQ tail/head doorbell at BAR0 + 0x1000 + (2y × stride) | [x] |
| 32.5 | Interrupt setup | MSI-X or pin-based interrupt for CQ completion | [x] |
| 32.6 | Phase bit tracking | Track phase bit for CQ entries (toggles per wrap) | [x] |
| 32.7 | Command ID tracking | 16-bit command ID, slot allocation/release | [x] |
| 32.8 | Queue full detection | Check SQ tail vs head distance, block if full | [x] |
| 32.9 | Queue error recovery | Handle failed commands (status field in CQE) | [x] |
| 32.10 | Test queue lifecycle | Create, submit dummy, complete, destroy | [x] |

### Sprint 33: Sector Read/Write (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 33.1 | Read command (opcode 0x02) | Build NVM Read SQE: NSID, LBA start, count, PRP1 | [x] |
| 33.2 | Write command (opcode 0x01) | Build NVM Write SQE: NSID, LBA start, count, PRP1 | [x] |
| 33.3 | PRP1/PRP2 setup | Physical Region Page entries for DMA (4KB aligned) | [x] |
| 33.4 | Single-sector read | Read 1 sector (512B) from LBA 0, verify magic bytes | [x] |
| 33.5 | Single-sector write | Write known pattern to LBA, read back and verify | [x] |
| 33.6 | Multi-sector read | Read 8 sectors (4KB) in single command | [x] |
| 33.7 | Multi-sector write | Write 4KB block, read back and verify all bytes | [x] |
| 33.8 | DMA buffer management | Pre-allocate DMA buffers at 0x100000-0x1FFFFF (16 MB region) | [x] |
| 33.9 | Block cache (1-entry) | Cache last read block to avoid repeated NVMe commands | [x] |
| 33.10 | Shell: `disk read/write` | `disk read <lba>` and `disk write <lba> <data>` commands | [x] |

### Sprint 34: Block Device Layer (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 34.1 | Block device abstraction | `blk_read(dev, lba, count, buf)` / `blk_write(...)` | [x] |
| 34.2 | Device table | 4 device slots: nvme0, ramdisk, virtio-blk0, virtio-blk1 | [x] |
| 34.3 | Ramdisk device | 1 MB ramdisk at 0xA00000 for testing without real NVMe | [x] |
| 34.4 | Read benchmark | Measure sequential read throughput (MB/s) | [x] |
| 34.5 | Write benchmark | Measure sequential write throughput (MB/s) | [x] |
| 34.6 | Random read | Random LBA read (latency measurement) | [x] |
| 34.7 | Sector buffer pool | 8 pre-allocated 4KB buffers for concurrent I/O | [x] |
| 34.8 | Error codes | BLK_OK=0, BLK_ERR_IO=-1, BLK_ERR_BOUNDS=-2, BLK_ERR_NOMEM=-3 | [x] |
| 34.9 | Shell: `blkdev list` | Show registered block devices with sizes | [x] |
| 34.10 | Integration test | Read/write/verify cycle on ramdisk + NVMe (QEMU virtio-blk) | [x] |

### Sprint 35: NVMe Polish + QEMU virtio-blk (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 35.1 | Virtio-blk driver | PCI class 0x01/0x00 + vendor 0x1AF4, virtqueue setup | [x] |
| 35.2 | Virtio descriptor ring | Available/used ring, descriptor chain for read/write | [x] |
| 35.3 | Virtio feature negotiation | VIRTIO_BLK_F_SIZE_MAX, VIRTIO_BLK_F_SEG_MAX | [x] |
| 35.4 | QEMU test disk | `qemu -drive file=test.img,format=raw,if=virtio` | [x] |
| 35.5 | Partition table (MBR) | Parse first 512 bytes: boot signature, 4 partition entries | [x] |
| 35.6 | Partition type detection | 0x0B/0x0C=FAT32, 0x83=Linux ext, 0xEE=GPT | [x] |
| 35.7 | Shell: `fdisk` | Display partition table from disk | [x] |
| 35.8 | Flush command | NVM Flush (opcode 0x00) for write persistence | [x] |
| 35.9 | SMART data | Identify Controller parse for temperature, wear level | [x] |
| 35.10 | Documentation | NOVA_STORAGE.md — NVMe architecture, commands, benchmarks | [x] |

---

## Phase 12: FAT32 Filesystem (5 sprints, 50 tasks)

**Goal:** Read and write files on a FAT32 formatted disk
**Depends on:** Phase 11 (block device layer)
**Key challenge:** Cluster chain traversal, directory parsing, long filenames

### Sprint 36: FAT32 Structures (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 36.1 | BPB parsing | Read Boot Parameter Block: bytes_per_sector, sectors_per_cluster | [x] |
| 36.2 | FAT info extraction | fat_start_lba, data_start_lba, root_cluster, total_clusters | [x] |
| 36.3 | Cluster → LBA | `cluster_to_lba(cluster) = data_start + (cluster - 2) × spc` | [x] |
| 36.4 | FAT entry read | Read 4-byte FAT entry for cluster N: next cluster or EOC | [x] |
| 36.5 | Cluster chain walk | Follow chain: cluster → FAT[cluster] → ... → 0x0FFFFFF8 | [x] |
| 36.6 | Root directory read | Read root_cluster chain, parse 32-byte directory entries | [x] |
| 36.7 | 8.3 filename decode | Parse DIR_Name[11]: name(8) + ext(3), trim spaces | [x] |
| 36.8 | File attributes | DIR_Attr: READONLY, HIDDEN, SYSTEM, DIRECTORY, ARCHIVE | [x] |
| 36.9 | File size/cluster | DIR_FileSize (4 bytes), DIR_FstClusHI:LO (start cluster) | [x] |
| 36.10 | Shell: `fat32 info` | Display FAT32 volume info: label, size, free clusters | [x] |

### Sprint 37: FAT32 Read (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 37.1 | File lookup | Search directory entries for filename match | [x] |
| 37.2 | Read file data | Follow cluster chain, read each cluster, concatenate | [x] |
| 37.3 | Read with offset | Read starting from byte offset (seek support) | [x] |
| 37.4 | Directory listing | List all entries in a directory cluster chain | [x] |
| 37.5 | Subdirectory traversal | Navigate /path/to/file by following directory clusters | [x] |
| 37.6 | Path parsing | Split "/home/user/file.txt" into components | [x] |
| 37.7 | Long filename (LFN) | Parse VFAT LFN entries (sequence number, Unicode → ASCII) | [x] |
| 37.8 | File type detection | Detect text/binary by first 16 bytes | [x] |
| 37.9 | Shell: `fat cat` | Read file from FAT32 partition and display | [x] |
| 37.10 | Shell: `fat ls` | List FAT32 directory contents | [x] |

### Sprint 38: FAT32 Write (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 38.1 | Free cluster search | Scan FAT for entry == 0x00000000 (free) | [x] |
| 38.2 | Allocate cluster | Write FAT entry: set to EOC (0x0FFFFFFF) | [x] |
| 38.3 | Extend chain | Link last cluster to new cluster in FAT | [x] |
| 38.4 | Write file data | Allocate clusters, write data, update FAT entries | [x] |
| 38.5 | Create directory entry | Write 32-byte entry in parent directory | [x] |
| 38.6 | Create file | Allocate first cluster + create dir entry + write data | [x] |
| 38.7 | Append file | Find last cluster, extend chain if needed, write new data | [x] |
| 38.8 | Delete file | Mark directory entry 0xE5, mark clusters as free in FAT | [x] |
| 38.9 | Create directory | Allocate cluster, write "." and ".." entries | [x] |
| 38.10 | FAT sync | Flush FAT + directory changes to disk (NVMe flush) | [x] |

### Sprint 39: FAT32 Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 39.1 | Rename file | Update directory entry name field | [x] |
| 39.2 | Move file | Delete from source dir, create in dest dir (same clusters) | [x] |
| 39.3 | File timestamps | DIR_WrtDate/WrtTime (BIOS time or TSC-based) | [x] |
| 39.4 | Volume label | Read/write volume label in root directory | [x] |
| 39.5 | Free space calculation | Count free clusters × cluster_size | [x] |
| 39.6 | fsck basic | Verify FAT chain consistency, detect lost clusters | [x] |
| 39.7 | Large file test | Write + read 1 MB file spanning multiple clusters | [x] |
| 39.8 | Shell: `fat write` | Write data to FAT32 file | [x] |
| 39.9 | Shell: `fat mkdir` | Create directory on FAT32 | [x] |
| 39.10 | Shell: `fat rm` | Delete file from FAT32 | [x] |

### Sprint 40: FAT32 Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 40.1 | Format command | `fat format` — write BPB + empty FAT + root dir | [x] |
| 40.2 | Disk image builder | Create FAT32 test image from host (mkfs.fat) | [x] |
| 40.3 | QEMU disk test | Boot with pre-populated FAT32 image, verify reads | [x] |
| 40.4 | Write + reboot test | Write file, reboot QEMU, read back file — persistence! | [x] |
| 40.5 | Performance metrics | Throughput: sequential read/write, random IOPS | [x] |
| 40.6 | Error recovery | Handle corrupt FAT entries, bad sectors | [x] |
| 40.7 | Multiple partitions | Support 2+ partitions from MBR | [x] |
| 40.8 | Shell integration | Unified `ls`, `cat`, `cp`, `rm` work on FAT32 | [x] |
| 40.9 | CI test | QEMU FAT32 read/write test in GitHub Actions | [x] |
| 40.10 | Documentation | NOVA_FAT32.md — implementation, limitations, benchmarks | [x] |

---

## Phase 13: VFS + Persistence (5 sprints, 50 tasks)

**Goal:** Unified filesystem interface, /dev /proc, persistent config
**Depends on:** Phase 12 (FAT32)

### Sprint 41: VFS Layer (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 41.1 | VFS mount table | 8-slot table: mountpoint, fs_type, device, root_inode | [x] |
| 41.2 | VFS operations | vfs_open, vfs_read, vfs_write, vfs_close, vfs_stat | [x] |
| 41.3 | File descriptor table | Per-process fd table (16 fds per process) | [x] |
| 41.4 | Mount ramfs at / | Root filesystem = ramfs (default) | [x] |
| 41.5 | Mount FAT32 | `mount /dev/nvme0p1 /mnt` — FAT32 on NVMe partition | [x] |
| 41.6 | Unmount | `umount /mnt` — sync + release | [x] |
| 41.7 | Path resolution | `/mnt/file.txt` → find mount → delegate to FAT32 | [x] |
| 41.8 | Cross-mount copy | `cp /mnt/file.txt /tmp/file.txt` (FAT32 → ramfs) | [x] |
| 41.9 | Unified ls/cat/rm | Commands use VFS, transparent to backing fs | [x] |
| 41.10 | Test: mount cycle | Mount, create file, unmount, remount, verify file | [x] |

### Sprint 42: /dev Device Files (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 42.1 | devfs driver | In-memory filesystem for /dev | [x] |
| 42.2 | /dev/null | Read returns EOF, write discards | [x] |
| 42.3 | /dev/zero | Read returns zeros, write discards | [x] |
| 42.4 | /dev/random | Read returns rdrand bytes | [x] |
| 42.5 | /dev/console | Read/write to VGA + serial | [x] |
| 42.6 | /dev/nvme0 | Block device node (major/minor) | [x] |
| 42.7 | /dev/tty0 | Terminal device | [x] |
| 42.8 | Device major/minor | Numbering scheme for device identification | [x] |
| 42.9 | Shell: `ls /dev` | List device files with type indicator | [x] |
| 42.10 | Test: device I/O | Write to /dev/null, read /dev/random | [x] |

### Sprint 43: /proc + /sys (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 43.1 | procfs driver | Virtual filesystem for /proc | [x] |
| 43.2 | /proc/cpuinfo | CPU model, features, frequency | [x] |
| 43.3 | /proc/meminfo | Total, free, used, cached memory | [x] |
| 43.4 | /proc/uptime | Seconds since boot (PIT tick based) | [x] |
| 43.5 | /proc/version | Kernel version string | [x] |
| 43.6 | /proc/[pid]/status | Process name, state, PID, memory | [x] |
| 43.7 | /proc/mounts | Current mount table | [x] |
| 43.8 | /proc/interrupts | IRQ counters per vector | [x] |
| 43.9 | Sysfs skeleton | /sys/class/block/nvme0 with size attribute | [x] |
| 43.10 | Test: cat /proc/* | Verify all procfs entries readable | [x] |

### Sprint 44: Persistent Config (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 44.1 | Boot config | /etc/fj.conf on FAT32 — hostname, motd, boot options | [x] |
| 44.2 | Config parser | Key=value format, # comments | [x] |
| 44.3 | Hostname persist | Save hostname to /etc/hostname on FAT32 | [x] |
| 44.4 | MOTD persist | Save /etc/motd to FAT32, display on boot | [x] |
| 44.5 | Shell history save | Write command history to /etc/history on shutdown | [x] |
| 44.6 | Boot from FAT32 | Read kernel config from FAT32 at boot | [x] |
| 44.7 | User accounts | /etc/passwd — username:uid (no authentication yet) | [x] |
| 44.8 | Shell alias persist | Save/load aliases from /etc/aliases | [x] |
| 44.9 | Graceful shutdown | Sync all mounts → ACPI poweroff | [x] |
| 44.10 | Test: persist cycle | Set hostname, reboot, verify hostname persists | [x] |

### Sprint 45: VFS Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 45.1 | Pipe device | /dev/pipe0-3 — IPC via VFS read/write | [x] |
| 45.2 | Redirect I/O | `cmd > file` and `cmd < file` in shell | [x] |
| 45.3 | Working directory | `cd /mnt`, `pwd` — per-process cwd | [x] |
| 45.4 | Relative paths | `cat file.txt` resolves relative to cwd | [x] |
| 45.5 | Recursive ls | `ls -R /mnt` — list directory tree | [x] |
| 45.6 | du command | Disk usage per directory | [x] |
| 45.7 | df command | Filesystem free space per mount | [x] |
| 45.8 | Tab completion | Complete filenames from VFS | [x] |
| 45.9 | CI test | Mount FAT32, create/read/persist across reboot | [x] |
| 45.10 | Documentation | NOVA_VFS.md — architecture, mount table, device files | [x] |

---

## Phase 14: SMP Multi-Core (5 sprints, 50 tasks)

**Goal:** Boot all CPU cores, parallel execution, per-CPU scheduling
**Depends on:** Phase 11 (stable interrupts/memory)

### Sprint 46: AP Bootstrap (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 46.1 | AP trampoline code | 16-bit real mode → 32-bit → 64-bit at 0x8000 | [x] |
| 46.2 | INIT IPI | Send INIT to target LAPIC ID | [x] |
| 46.3 | SIPI IPI | Send Startup IPI with trampoline page | [x] |
| 46.4 | AP GDT/IDT | Per-CPU GDT + IDT pointer setup | [x] |
| 46.5 | AP stack allocation | 64 KB kernel stack per CPU | [x] |
| 46.6 | AP paging | Share kernel page tables (TTBR/CR3) | [x] |
| 46.7 | CPU online tracking | Bitmap of online CPUs, atomic set/test | [x] |
| 46.8 | BSP/AP synchronization | Spinlock barrier for AP ready signal | [x] |
| 46.9 | Shell: `cpuinfo` update | Show online/offline status per core | [x] |
| 46.10 | Test: boot 4 cores | QEMU `-smp 4`, verify all 4 reach long mode | [x] |

### Sprint 47: Per-CPU Structures (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 47.1 | Per-CPU data area | GS-base pointing to per-CPU structure | [x] |
| 47.2 | Current process ptr | Per-CPU: which process is running on this core | [x] |
| 47.3 | LAPIC timer | Per-CPU timer for preemption (replace global PIT) | [x] |
| 47.4 | Timer calibration | Calibrate LAPIC timer frequency using PIT | [x] |
| 47.5 | Per-CPU run queue | Each core has its own ready queue | [x] |
| 47.6 | Idle thread per CPU | Each core has idle loop when no work | [x] |
| 47.7 | TLB shootdown | IPI to flush TLB on other cores after page table change | [x] |
| 47.8 | Spinlock implementation | Atomic test-and-set with pause loop | [x] |
| 47.9 | Ticket lock | Fair ordering for contended locks | [x] |
| 47.10 | Test: parallel fib | Run fib(30) on 4 cores simultaneously | [x] |

### Sprint 48: SMP Scheduler (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 48.1 | Process affinity | Pin process to specific CPU core | [x] |
| 48.2 | Load balancing | Migrate processes between cores (work stealing) | [x] |
| 48.3 | IPI: reschedule | Send IPI to wake idle core when new process ready | [x] |
| 48.4 | SMP-safe process table | Lock process table during spawn/kill/schedule | [x] |
| 48.5 | Atomic process state | Use compare-and-swap for state transitions | [x] |
| 48.6 | Kernel big lock | Initial approach: single lock for kernel entry | [x] |
| 48.7 | Fine-grained locks | Per-subsystem locks (scheduler, ramfs, IPC) | [x] |
| 48.8 | Deadlock detection | Lock ordering + timeout on spinlock acquire | [x] |
| 48.9 | Shell: `top` SMP | Show per-core CPU utilization | [x] |
| 48.10 | Test: 16 processes | Spawn 16 processes across 4 cores, verify all complete | [x] |

### Sprint 49: IPC + Synchronization (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 49.1 | SMP-safe IPC queues | Lock per-queue for multi-core message passing | [x] |
| 49.2 | Futex | Fast user-space mutex (wait/wake syscalls) | [x] |
| 49.3 | Semaphore | Counting semaphore for resource limiting | [x] |
| 49.4 | Condition variable | Wait/signal/broadcast for producer-consumer | [x] |
| 49.5 | Read-write lock | Multiple readers OR single writer | [x] |
| 49.6 | Barrier | Wait for N threads to reach synchronization point | [x] |
| 49.7 | Atomic operations | CAS, fetch-add, load-acquire, store-release builtins | [x] |
| 49.8 | Memory ordering | Fence instructions for cross-core visibility | [x] |
| 49.9 | Shell: `smp test` | Parallel computation benchmark | [x] |
| 49.10 | Test: producer-consumer | 2 cores, shared queue, verify no data loss | [x] |

### Sprint 50: SMP Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 50.1 | CPU hotplug | Bring core online/offline at runtime | [x] |
| 50.2 | Power management | Halt idle cores (HLT instruction) | [x] |
| 50.3 | NUMA awareness | Detect SRAT table for memory locality (future) | [x] |
| 50.4 | Performance counters | Per-core: context switches, interrupts, IPI count | [x] |
| 50.5 | Kernel profiling | TSC-based function timing | [x] |
| 50.6 | SMP stress test | Hammer all cores with mixed workloads | [x] |
| 50.7 | Core dump | Dump all CPU register states on panic | [x] |
| 50.8 | Watchdog | NMI watchdog for hung core detection | [x] |
| 50.9 | CI test | QEMU `-smp 4` boot + parallel execution test | [x] |
| 50.10 | Documentation | NOVA_SMP.md — architecture, locking, performance | [x] |

---

## Phase 15: Virtio-Net + TCP/IP (5 sprints, 50 tasks)

**Goal:** Network communication via virtio-net in QEMU
**Depends on:** Phase 11 (DMA), Phase 14 (interrupt handling)

### Sprint 51: Virtio-Net Driver (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 51.1 | Virtio PCI discovery | Find device: vendor 0x1AF4, device 0x1000 (net) | [x] |
| 51.2 | Virtio feature negotiation | VIRTIO_NET_F_MAC, VIRTIO_NET_F_STATUS | [x] |
| 51.3 | Virtqueue setup | RX queue + TX queue (256 descriptors each) | [x] |
| 51.4 | Descriptor ring | Available ring, used ring, descriptor table | [x] |
| 51.5 | RX buffer allocation | Pre-allocate 256 × 1518B receive buffers | [x] |
| 51.6 | MAC address read | Read 6-byte MAC from device config | [x] |
| 51.7 | Packet receive | IRQ handler: process used ring, copy to buffer | [x] |
| 51.8 | Packet transmit | Build descriptor chain, kick TX queue | [x] |
| 51.9 | Shell: `ifconfig` | Show MAC, IP, RX/TX packet counts | [x] |
| 51.10 | Test: loopback | Send packet, receive on same NIC (QEMU tap) | [x] |

### Sprint 52: Ethernet + ARP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 52.1 | Ethernet frame parse | dst_mac[6], src_mac[6], ethertype[2], payload | [x] |
| 52.2 | Ethernet frame build | Construct frame with proper header | [x] |
| 52.3 | ARP request | Who has IP X.X.X.X? Tell MAC Y | [x] |
| 52.4 | ARP reply | Response with our MAC for our IP | [x] |
| 52.5 | ARP cache | 16-entry table: IP → MAC, timeout 60s | [x] |
| 52.6 | ARP resolution | Block until MAC resolved for destination IP | [x] |
| 52.7 | Static IP config | Set IP/netmask/gateway via shell | [x] |
| 52.8 | Broadcast support | Send to ff:ff:ff:ff:ff:ff | [x] |
| 52.9 | Shell: `arp` | Display ARP cache table | [x] |
| 52.10 | Test: ARP ping | ARP request + reply on QEMU tap network | [x] |

### Sprint 53: IPv4 + ICMP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 53.1 | IPv4 header parse | version, IHL, total_length, protocol, src_ip, dst_ip | [x] |
| 53.2 | IPv4 header build | Construct header with checksum | [x] |
| 53.3 | IP checksum | One's complement sum calculation | [x] |
| 53.4 | ICMP echo request | Build ping packet (type 8, code 0) | [x] |
| 53.5 | ICMP echo reply | Respond to incoming ping (type 0) | [x] |
| 53.6 | Shell: `ping` | `ping 10.0.2.2` — send ICMP, measure RTT | [x] |
| 53.7 | IP routing | Simple: if same subnet → ARP, else → gateway | [x] |
| 53.8 | TTL handling | Decrement TTL, drop if 0 | [x] |
| 53.9 | IP fragmentation | Basic reassembly for packets > MTU | [x] |
| 53.10 | Test: ping gateway | Ping QEMU gateway (10.0.2.2), verify reply | [x] |

### Sprint 54: UDP + TCP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 54.1 | UDP header parse | src_port, dst_port, length, checksum | [x] |
| 54.2 | UDP send/receive | Stateless datagram I/O | [x] |
| 54.3 | Port binding | Bind to port, receive matching packets | [x] |
| 54.4 | TCP header parse | Sequence/ACK numbers, flags, window | [x] |
| 54.5 | TCP state machine | CLOSED → SYN_SENT → ESTABLISHED → FIN_WAIT | [x] |
| 54.6 | TCP 3-way handshake | SYN → SYN-ACK → ACK | [x] |
| 54.7 | TCP data transfer | Sequence numbers, ACK tracking, retransmit | [x] |
| 54.8 | TCP connection close | FIN → FIN-ACK → ACK (4-way) | [x] |
| 54.9 | Shell: `nc` (netcat) | Simple TCP connect + send/receive | [x] |
| 54.10 | Test: TCP echo | Connect to QEMU host, echo test | [x] |

### Sprint 55: Network Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 55.1 | DNS resolver | UDP query to 10.0.2.3 (QEMU DNS), parse A record | [x] |
| 55.2 | DHCP client | Discover → Offer → Request → ACK (auto IP config) | [x] |
| 55.3 | Socket API | socket(), bind(), listen(), accept(), connect(), send(), recv() | [x] |
| 55.4 | HTTP GET | Minimal HTTP/1.1 client: GET request + response parse | [x] |
| 55.5 | Network statistics | RX/TX bytes, packets, errors, drops | [x] |
| 55.6 | Packet filter | Simple firewall: allow/deny by IP/port | [x] |
| 55.7 | Shell: `wget` | Download file via HTTP | [x] |
| 55.8 | Shell: `nslookup` | DNS query command | [x] |
| 55.9 | CI test | QEMU network: ping + TCP echo test | [x] |
| 55.10 | Documentation | NOVA_NETWORK.md — stack, protocols, benchmarks | [x] |

---

## Phase 16: ELF Loader + Userland (5 sprints, 50 tasks)

**Goal:** Load and execute external ELF binaries in user space
**Depends on:** Phase 13 (VFS), Phase 14 (process model)

### Sprint 56: ELF Parser (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 56.1 | ELF header parse | e_ident magic, class (64-bit), machine (x86_64) | [x] |
| 56.2 | Program headers | PT_LOAD segments: vaddr, memsz, filesz, flags | [x] |
| 56.3 | Section headers | .text, .rodata, .data, .bss identification | [x] |
| 56.4 | Entry point | e_entry — address to start execution | [x] |
| 56.5 | Validate ELF | Check magic, class, machine, type (ET_EXEC) | [x] |
| 56.6 | Memory layout | Calculate total memory needed from PT_LOAD segments | [x] |
| 56.7 | BSS zeroing | .bss section: zero-fill memsz - filesz | [x] |
| 56.8 | Read from VFS | Load ELF from FAT32 or ramfs via vfs_read | [x] |
| 56.9 | Shell: `readelf` | Display ELF header + segments | [x] |
| 56.10 | Test: parse hello.elf | Parse pre-built minimal ELF, verify header | [x] |

### Sprint 57: Process Loading (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 57.1 | Allocate user pages | Map PT_LOAD segments into process address space | [x] |
| 57.2 | Copy segments | Copy .text, .rodata, .data from file to mapped pages | [x] |
| 57.3 | User stack setup | Allocate 64 KB user stack at top of user space | [x] |
| 57.4 | argc/argv on stack | Push argument count + argument strings to user stack | [x] |
| 57.5 | envp on stack | Push environment variables | [x] |
| 57.6 | Set entry point | Configure process RIP = e_entry | [x] |
| 57.7 | Transition to user | IRETQ with user CS/SS selectors | [x] |
| 57.8 | exec() syscall | Replace current process image with new ELF | [x] |
| 57.9 | fork() + exec() | Create child process and load new program | [x] |
| 57.10 | Test: run hello.elf | Load and execute minimal "hello world" ELF | [x] |

### Sprint 58: Syscall Interface (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 58.1 | Syscall dispatch table | 32-entry function pointer table | [x] |
| 58.2 | SYS_write (1) | Write buffer to fd (console, file) | [x] |
| 58.3 | SYS_read (0) | Read from fd (keyboard, file) | [x] |
| 58.4 | SYS_open (2) | Open file, return fd | [x] |
| 58.5 | SYS_close (3) | Close fd | [x] |
| 58.6 | SYS_exit (60) | Terminate process with exit code | [x] |
| 58.7 | SYS_mmap (9) | Map memory pages for process | [x] |
| 58.8 | SYS_brk (12) | Expand process heap (sbrk) | [x] |
| 58.9 | SYS_getpid (39) | Return current PID | [x] |
| 58.10 | Test: syscall from user | User process calls SYS_write to print "Hello" | [x] |

### Sprint 59: User Programs (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 59.1 | Fajar Lang → ELF | Compile .fj to x86_64 ELF with syscall stubs | [x] |
| 59.2 | Minimal libc | _start, write(), exit(), malloc() wrappers | [x] |
| 59.3 | hello.fj → hello.elf | Compile and run first user-space Fajar Lang program | [x] |
| 59.4 | Shell: `exec` | `exec /mnt/hello.elf` — load from FAT32 and run | [x] |
| 59.5 | Background execution | `exec /mnt/app.elf &` — run in background | [x] |
| 59.6 | Wait for exit | Parent waits for child process to exit | [x] |
| 59.7 | Exit code | Process exit code visible via `wait` | [x] |
| 59.8 | Signal delivery | SIGKILL, SIGTERM to user processes | [x] |
| 59.9 | /bin directory | Store built-in commands as ELF binaries | [x] |
| 59.10 | Test: multi-program | Load and run 3 different ELF programs sequentially | [x] |

### Sprint 60: Userland Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 60.1 | Dynamic memory | User-space heap via SYS_brk | [x] |
| 60.2 | Process isolation | Verify one process cannot read another's memory | [x] |
| 60.3 | Page fault handler | Map on demand, stack growth | [x] |
| 60.4 | Core dump on crash | Write register state to /tmp/core.PID | [x] |
| 60.5 | strace mode | Print syscall trace for debugging | [x] |
| 60.6 | Shell PATH | Search /bin, /mnt/bin for commands | [x] |
| 60.7 | init process | PID 1 spawns shell, reaps orphans | [x] |
| 60.8 | Stress test | 16 processes, mixed I/O + compute | [x] |
| 60.9 | CI test | Build ELF, boot QEMU, exec, verify output | [x] |
| 60.10 | Documentation | NOVA_USERLAND.md — syscall ABI, ELF format, libc | [x] |

---

## Timeline

```
Sprint 31-35:  Phase 11 (NVMe)       — Block device I/O
Sprint 36-40:  Phase 12 (FAT32)      — Persistent filesystem
Sprint 41-45:  Phase 13 (VFS)        — Unified FS, /dev, /proc, config persistence
Sprint 46-50:  Phase 14 (SMP)        — Multi-core boot, parallel scheduling
Sprint 51-55:  Phase 15 (Network)    — Virtio-net, TCP/IP, ping, wget
Sprint 56-60:  Phase 16 (Userland)   — ELF loader, syscalls, user programs
```

## Quality Gates

**Per Sprint:**
- All tasks checked
- QEMU boot test passes
- No kernel panics
- CI green

**Per Phase:**
- Documentation written
- Shell commands functional
- Integration tests pass
- Performance benchmarks recorded

## Architecture Target (v0.2.0)

```
               User Space (Ring 3)
    ┌──────────┬──────────┬──────────┐
    │ hello.elf│ server.elf│  shell   │
    └────┬─────┴────┬─────┴────┬─────┘
         │ syscalls  │          │
    ═════╪══════════╪══════════╪═══════  ← Ring 0/3 boundary
         │          │          │
    ┌────┴──────────┴──────────┴─────┐
    │         Syscall Dispatch        │
    ├─────────────────────────────────┤
    │  VFS Layer (mount table, fds)   │
    ├────────┬────────┬───────────────┤
    │ ramfs  │ FAT32  │ devfs/procfs  │
    ├────────┴────────┴───────────────┤
    │   Block Device Layer            │
    ├────────┬────────────────────────┤
    │  NVMe  │  virtio-blk           │
    ├────────┴────────────────────────┤
    │   SMP Scheduler (per-CPU)       │
    ├─────────────────────────────────┤
    │   Memory Manager (paging)       │
    ├────────┬────────┬───────────────┤
    │  LAPIC │ IOAPIC │  PCI/DMA      │
    ├────────┴────────┴───────────────┤
    │   TCP/IP Stack                  │
    ├─────────────────────────────────┤
    │   virtio-net                    │
    └─────────────────────────────────┘
            Hardware (x86_64)
```

**Target: ~10,000 LOC Fajar Lang, 150+ commands, persistent files, network, multi-core**

---

*FajarOS Nova v0.2 "Perseverance" — the OS that persists across reboots.*
