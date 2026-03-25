# FajarOS Nova v1.4.0 "Zenith" — Architecture Document

> x86_64 bare-metal operating system written 100% in Fajar Lang
> 20,176 LOC monolithic kernel | 126 modular .fj files (64K LOC total)
> 757 @kernel functions | 34 syscalls | 240+ shell commands
> Compiler-enforced safety via @kernel/@device/@safe annotations

---

## 1. Architecture Overview

FajarOS Nova is a five-layer OS where the Fajar Lang compiler statically
enforces isolation between layers through context annotations. Code marked
`@kernel` cannot call tensor operations; code marked `@device` cannot
dereference raw pointers. Violations are compile-time errors, not runtime
checks.

```
 ┌─────────────────────────────────────────────────────────────────┐
 │                       APPLICATIONS                              │
 │  user_programs.fj  ring3_hello.fj  mnist.fj  editor  compiler  │
 │  pkgmgr            (Ring 3, ELF64, SYSCALL interface)           │
 │                          @safe                                  │
 ├─────────────────────────────────────────────────────────────────┤
 │                        SERVICES                                 │
 │  init/   net/   blk/   vfs/   auth/   shell/   gpu/   pkg/     │
 │  display/   input/   gui/                                       │
 │                     @safe / @kernel                             │
 ├─────────────────────────────────────────────────────────────────┤
 │                        DRIVERS                                  │
 │  serial  vga  keyboard  pci  nvme  xhci  virtio_blk            │
 │  virtio_net  virtio_gpu  gpu                                    │
 │                         @kernel                                 │
 ├─────────────────────────────────────────────────────────────────┤
 │                       KERNEL CORE                               │
 │  mm/     sched/     syscall/    process/    signal/    ipc/     │
 │  debug/  security/  interrupts/ compute/    auth/               │
 │                         @kernel                                 │
 ├─────────────────────────────────────────────────────────────────┤
 │                        HARDWARE                                 │
 │  x86_64 CPU (Long Mode)  LAPIC Timer  PCI/NVMe  VirtIO         │
 │  Serial COM1/COM2  VGA 0xB8000  PS/2 Keyboard  xHCI USB        │
 └─────────────────────────────────────────────────────────────────┘
```

**Context annotation enforcement matrix:**

| Operation          | @safe   | @kernel | @device |
|--------------------|---------|---------|---------|
| volatile_read/write| ERROR   | OK      | ERROR   |
| port_inb/outb      | ERROR   | OK      | ERROR   |
| Tensor ops         | ERROR   | ERROR   | OK      |
| Function calls     | safe    | any     | safe+dev|
| Raw pointer deref  | ERROR   | OK      | ERROR   |

**Source tree layout (126 .fj files):**

```
fajaros-x86/
├── kernel/           Core subsystems (62 files)
│   ├── boot/         constants.fj
│   ├── mm/           frames, paging, heap, slab, cow
│   ├── sched/        process, scheduler, signals, smp, spinlock
│   ├── syscall/      dispatch, entry, elf
│   ├── process/      fork, exec, wait, exit
│   ├── signal/       signal, jobs
│   ├── ipc/          message, pipe, pipe_v2, channel, notify, shm
│   ├── interrupts/   lapic, timer
│   ├── debug/        gdb_stub, gdb_ext
│   ├── security/     capability, limits, hardening
│   ├── compute/      buffers, kernels (GPU compute)
│   ├── auth/         users, permissions, sessions
│   ├── hw/           detect, acpi, pcie, uefi_boot
│   ├── core/         boot, mm, irq, sched, ipc, syscall, elf_loader, ...
│   ├── stubs/        console, driver_stubs, framebuffer, gpu_stub
│   └── main.fj       Kernel entry point (@entry)
├── drivers/          Hardware drivers (10 files)
├── fs/               Filesystems (9 files)
├── shell/            Shell layer (6 files)
├── services/         System services (11 directories, 25 files)
├── apps/             User applications (6 files)
├── lib/              User-space syscall library
├── tests/            Kernel tests + benchmarks
└── Makefile          Concatenation build system
```

---

## 2. Memory Map

All addresses are physical. Identity-mapped via 64 x 2MB huge pages (128MB).
4KB page mapping supported for per-process user-space pages.

```
 Address         Size     Description
 ──────────────  ───────  ──────────────────────────────────────────────
 0x000000        512 KB   Low Memory (IVT, BDA, real-mode structures)
 0x070000        16 KB    Page Tables (PML4 + PDPT + PD + PT)
   0x070000               PML4 (512 entries x 8B)
   0x071000               PDPT
   0x072000               PD (64 x 2MB huge pages)
 0x0B8000        4 KB     VGA Text Buffer (80x25 x 2B = 4000 bytes)
 0x0E0000        128 KB   BIOS / ACPI tables
 ──────────────  ───────  ──────────────────────────────────────────────
 0x100000        128 KB   Kernel .text + .rodata (ELF loaded here)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x400000        1.5 MB   Heap (freelist allocator, first-fit)
   Header: 16B (size: i64, next/magic: i64)
   Magic: 0xABCD1234 for double-free detection
 0x580000        4 KB     Frame Bitmap (32768 bits = 128MB / 4KB)
 0x581008                 Free-list head pointer
 0x581010                 Heap stats (allocated bytes, heap size, count)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x600000        4 KB     Process Table (16 PIDs x 256B)
   +0   state     +8   pid       +16  ppid      +24  entry
   +32  ticks     +40  CR3       +48  name[16]   +96  brk
   +104 wake_tick +112 saved RSP +120 pgid       +128 cwd[32]
 ──────────────  ───────  ──────────────────────────────────────────────
 0x6F800         64 B     Command Buffer (shell input, null-terminated)
 0x6FB00         256 B    Keyboard Ring Buffer (read_idx at 0x6FBF8)
 0x6FE00         8 B      Current PID (running process)
 0x6FE08         8 B      Tick Counter (incremented by LAPIC timer IRQ)
 0x6F8000        20 KB    Shell State (cursor, history, variables)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x700000        64 KB    Per-Process Kernel Stacks (16 x 16KB each)
   PID N stack: 0x700000 + N * 0x4000 .. + 0x3FF0
 0x710000        832 KB   RamFS Data Region (file contents)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x7F0000        64 KB    Kernel Stack (boot/idle, PID 0)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x806000        512 B    VFS Mount Table (8 entries x 64B)
 0x806200        256 B    VFS Mountpoint Names (8 x 32B)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x860000                 Network State (legacy, pre-v2.0)
 0x878000                 ARP Cache (legacy)
 0x884000        256 B    Syscall Dispatch Table (32 entries x 8B)
 0x890000        4 KB     Process Table V2 (16 x 256B, Sprint B6)
 0x894000        4 KB     FD Table (legacy, 16 procs x 16 FDs x 16B)
 0x898000        32 KB    Pipe Pool (8 pipes x 4KB circular buffers)
   Per pipe: +0 in_use, +8 read_pos, +16 write_pos, +24 refcounts
             +32..+4095 data ring (4064 bytes usable)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x8B7000        256 B    Init Service Table (8 services x 32B)
   Per svc: +0 pid, +8 state, +16 restarts, +24 name_hash
 0x8D0000        4 KB     FD Table V2 (16 procs x 16 FDs x 16B)
   Per FD: +0 type (0=closed,1=console,2=ramfs,3=pipe_r,4=pipe_w,5=fat32)
           +8 data (file index | offset<<32, or pipe slot)
 0x8D1000        1 KB     Signal Table (16 procs x 64B)
   Per proc: +0 pending bitmap, +8 mask, +16 handlers[8]
 0x8D6000        4 KB     Argv Buffer (exec: 16 args x 256B)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x8E0000        512 B    Network Interface State (v2.0)
   +0 state, +8 MAC, +16 IP, +24 mask, +32 GW, +40 DNS
   +48 TX count, +56 RX count, +64 TX bytes, +72 RX bytes
 0x8E0200        512 B    ARP Cache V2 (16 entries x 32B)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x950000        64 KB    CoW Refcount Array (32768 frames x 2B)
 0x950048        8 B      CoW Fault Counter
 ──────────────  ───────  ──────────────────────────────────────────────
 0x980000        8 KB     TCP Connection Table (16 conns x 128B)
   Per conn: +0 state, +8 local_port, +16 remote_ip, +24 remote_port
   +32 seq, +40 ack, +48 window, +56 rtx_timer, +72 send_buf, +88 recv_buf
 0x982000        64 KB    TCP Send/Receive Buffers (16 x 2 x 2048B)
 0x983000        4 KB     TCP Packet Assembly Buffer
 ──────────────  ───────  ──────────────────────────────────────────────
 0x994000        4 KB     GDB Packet Buffer
 0x995000        4 KB     GDB Response Buffer
 0x996000        256 B    GDB State (active, single-step flag)
 0x996100        256 B    GDB Breakpoint Table (16 entries)
 0x996300        64 B     GDB Watchpoint Table (4 entries x 16B)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x9A0000                 GPU State + VirtIO-GPU framebuffer (320x200)
 0x9B0000        1 KB     Service Table V2 (16 services x 64B)
   Per svc: +0 name[16], +16 pid, +24 state, +32 restart_policy
   +40 start_tick, +48 restarts, +56 enabled
 0x9B1000        8 B      Current Runlevel (0/1/3/5)
 0x9B2000        8 KB     Kernel Log Ring Buffer (klog)
 0x9B4000        16 B     Klog head + message count
 0x9B5000        8 KB     Syslog Buffer (timestamped entries)
 0x9B8000        512 B    Crontab (8 entries x 64B)
   Per job: +16 interval, +24 last_run, +32 cmd[24], +56 active
 0x9B9000        256 B    PID File Table (16 x 16B)
 0x9C0000                 Package Database V2
 0x9EF000                 GPU Compute Metadata
 0x9F0000                 GPU Compute Buffers
 ──────────────  ───────  ──────────────────────────────────────────────
 0xA00000                 ext2 Buffers (superblock, block groups)
 0xA04000                 TCP Control Blocks (v2 extended)
 0xA06000                 Network Statistics (counters, histograms)
 ──────────────  ───────  ──────────────────────────────────────────────
 0x800000        120 MB   Free Frame Pool (dynamically allocated)
 0x2800000                User Heap Base (per-process, via brk/sbrk)
 0x7FC0000       256 KB   ELF Load Area (user programs, Ring 3)
 0x7FFFFFF                End of 128MB identity map
```

---

## 3. Process Model

### 3.1 Process Table

16 process slots (PIDs 0-15). Each entry is 256 bytes at `PROC_TABLE + pid * 256`.
PID 0 is the kernel/idle process (always RUNNING). PID 1 is init.

```
  ┌──────────────────────────────────────────────────┐
  │                Process Entry (256B)               │
  ├────────┬──────┬───────┬───────┬──────┬───────────┤
  │+0 state│+8 pid│+16 ppid│+24 entry│+32 ticks│+40 CR3│
  ├────────┴──────┴───────┴───────┴──────┴───────────┤
  │+48 name[16]  +96 brk  +104 wake_tick             │
  │+112 saved_RSP  +120 pgid  +128 cwd[32]           │
  └──────────────────────────────────────────────────┘
```

### 3.2 Process States

```
            fork()              scheduler picks
  FREE ──────────► READY ◄──────────────────── BLOCKED
    ▲                │                            ▲
    │            schedule()                   sleep/wait/
    │                │                        pipe_read
    │                ▼                            │
    │             RUNNING ────────────────────────┘
    │                │           yield/preempt
    │            exit()/signal
    │                │
    │                ▼
    └────────── ZOMBIE ──► (reaped by waitpid)
```

| State   | Value | Description                               |
|---------|-------|-------------------------------------------|
| FREE    | 0     | Slot available for allocation              |
| READY   | 1     | Runnable, waiting for CPU time             |
| RUNNING | 2     | Currently executing on CPU                 |
| BLOCKED | 3     | Waiting (sleep, pipe read, waitpid, SIGSTOP)|
| ZOMBIE  | 4     | Exited, waiting for parent to reap         |

### 3.3 Fork / Exec / Waitpid Lifecycle

```
  Parent                              Child
    │                                   │
    ├── sys_fork() ──────────────────► born (READY)
    │   1. Find free PID                │ state=READY
    │   2. Clone page tables            │ CR3=new PML4
    │   3. Copy FD table                │ FDs inherited
    │   4. Copy context frame           │ RAX=0 (child)
    │   5. Set child READY              │
    │   return child_pid                │
    │                                   │
    │                                   ├── sys_exec(path, argv)
    │                                   │   1. Load ELF from ramfs/FAT32
    │                                   │   2. Validate ELF64 header
    │                                   │   3. Map PT_LOAD segments
    │                                   │   4. Setup user stack + argv
    │                                   │   5. Reset non-stdio FDs
    │                                   │   6. Build Ring 3 IRETQ frame
    │                                   │
    ├── sys_waitpid(child, &status, 0)  │ (running user code)
    │   blocks until child exits        │
    │                                   ├── sys_exit(code)
    │                                   │   state → ZOMBIE
    │   ◄── woken, status=exit_code ────┘   exit_code stored
    │   reap: child state → FREE
    │
```

### 3.4 Context Switch

Timer IRQ (LAPIC, ~10ms) triggers preemptive round-robin scheduling:

```
  IRQ fires
    │
    ▼
  Save 15 GPRs + IRETQ frame (20 x 8 = 160 bytes) to kernel stack
    │
    ▼
  Store RSP in current process entry
    │
    ▼
  Round-robin: scan PIDs 0..15, find next READY
  (also check wake_tick for sleeping processes)
    │
    ▼
  Load new process RSP, switch CR3 if different
    │
    ▼
  Restore 15 GPRs, IRETQ → resume (Ring 0 or Ring 3)
```

### 3.5 Ring 3 User Space

User programs run in Ring 3 with per-process page tables:
- CS = 0x23 (user code, RPL=3), SS = 0x1B (user data, RPL=3)
- SYSCALL via `int 0x80` or `syscall` instruction
- User stack at 0x7FC0000 (64KB mapped with PAGE_USER)
- 5 embedded programs: hello, goodbye, fajar, counter, fibonacci

---

## 4. Filesystem Layers

### 4.1 VFS Architecture

```
  ┌────────────────────────────────────────────┐
  │                   VFS Layer                 │
  │  Mount table (8 slots) at 0x806000         │
  │  Path resolution: longest-prefix match     │
  ├────────┬────────┬─────────┬────────────────┤
  │ ramfs  │ FAT32  │  devfs  │    procfs      │
  │ type=1 │ type=2 │ type=3  │    type=4      │
  │ /      │ /mnt   │ /dev    │    /proc       │
  └────────┴────────┴─────────┴────────────────┘
```

### 4.2 RamFS (In-Memory Filesystem)

- Root filesystem, always mounted at `/`
- Entry table at 0x700000: 64 entries x 64B
- Data region at 0x710000: 832KB for file contents
- Per entry: name[32], size, data_ptr, type (1=file, 2=dir), permissions, uid
- Supports: create, read, write, delete, mkdir, symlinks, hardlinks
- Hierarchical directories with parent tracking

### 4.3 FAT32

- Mounted at `/mnt` when NVMe or VirtIO block device detected
- Reads: MBR -> partition table -> BPB -> FAT chain -> clusters
- Supports: directory traversal, file read, long filename entries
- Read-only currently (write support via journaling shim)

### 4.4 ext2

- Superblock parser, block group descriptors, inode table
- Buffers at 0xA00000
- Read support for ext2-formatted partitions

### 4.5 Journaling (WAL)

- Write-Ahead Log for crash recovery
- fsck command verifies and replays journal

### 4.6 Special Filesystems

- **/dev**: Device nodes (null, zero, random, serial0, nvme0)
- **/proc**: Process information (PID directories, status, cmdline)

---

## 5. Network Stack

### 5.1 Protocol Layers

```
  ┌──────────────────────────────────────────┐
  │  Socket API: tcp_connect, tcp_listen,    │
  │  tcp_send, tcp_recv, tcp_close           │
  │  udp_send, udp_recv                      │
  ├──────────────────────────────────────────┤
  │  TCP (16 connections, full state machine)│
  │  UDP (stateless datagrams)               │
  ├──────────────────────────────────────────┤
  │  ICMP (ping request/reply)               │
  ├──────────────────────────────────────────┤
  │  IPv4 (header build, checksum, routing)  │
  ├──────────────────────────────────────────┤
  │  ARP (16-entry cache, request/reply)     │
  ├──────────────────────────────────────────┤
  │  Ethernet (virtio-net driver, MAC addr)  │
  └──────────────────────────────────────────┘
```

### 5.2 TCP State Machine (RFC 793)

```
                           ┌───────────┐
                     ┌─────│  CLOSED   │◄────────────────┐
                     │     └───────────┘                  │
              connect()      │    ▲                  timeout
                     │    listen()│    │                   │
                     ▼       │    │                        │
              ┌───────────┐  │  ┌───────────┐    ┌───────────┐
              │ SYN_SENT  │  │  │TIME_WAIT  │    │ LAST_ACK  │
              └─────┬─────┘  │  └───────────┘    └─────┬─────┘
           SYN+ACK  │       ▼         ▲                │
              rcvd   │  ┌───────────┐  │           FIN  │
                     │  │  LISTEN   │  │           ACK  │
                     │  └─────┬─────┘  │                │
                     │   SYN  │        │           ┌────┘
                     │   rcvd │   2MSL wait        │
                     ▼        ▼        │           │
              ┌───────────┐ ┌──────────┐   ┌───────────┐
              │ESTABLISHED│ │SYN_RCVD  │   │CLOSE_WAIT │
              └─────┬─────┘ └──────────┘   └─────┬─────┘
                    │                             │
               FIN  │                        close()
               sent │                             │
                    ▼                             ▼
              ┌───────────┐              ┌───────────┐
              │FIN_WAIT_1 │              │ LAST_ACK  │
              └─────┬─────┘              └───────────┘
              ACK   │
              rcvd  ▼
              ┌───────────┐
              │FIN_WAIT_2 │
              └───────────┘
```

- 16 concurrent TCP connections at 0x980000 (128B per connection)
- Per-connection send/receive buffers (2KB each) at 0x982000
- Retransmission timer with max 3 retries
- Sequence/acknowledgment number tracking
- Window size management (default 8192)

### 5.3 Services Built on Network Stack

- **HTTP server** (httpd): Serves static files from ramfs, GET/POST
- **DNS resolver**: UDP-based name resolution
- **Echo server**: TCP echo for testing
- **TLS**: Certificate and handshake framework

### 5.4 Network Interface State

Stored at 0x8E0000: MAC, IPv4, subnet mask, gateway, DNS, packet
counters. Configured via `ifconfig` and `dhcp` shell commands.

---

## 6. Signal Delivery

### 6.1 Signal Table

16 processes x 64B at 0x8D1000. Each entry stores:
- Pending bitmap (8 signals, 1 bit each)
- Blocked mask (signals to defer)
- Handler array (8 slots: SIG_DFL=0 or SIG_IGN=1)

### 6.2 Supported Signals

| Signal  | Num | Slot | Default Action       |
|---------|-----|------|----------------------|
| SIGHUP  |  1  |  0   | Terminate            |
| SIGINT  |  2  |  1   | Terminate (Ctrl+C)   |
| SIGKILL |  9  |  2   | Terminate (uncatchable)|
| SIGSEGV | 11  |  3   | Terminate            |
| SIGTERM | 15  |  4   | Terminate            |
| SIGCHLD | 17  |  5   | Ignore               |
| SIGCONT | 18  |  7   | Resume stopped proc  |
| SIGSTOP | 19  |  6   | Stop (uncatchable)   |

### 6.3 Delivery Flow

```
  signal_send(pid, signum)
    │
    ▼
  Set bit in pending bitmap
    │
    ├── SIGKILL? ──► Immediate: state → ZOMBIE, exit_code = 128+sig
    │                 reparent children, wake waiting parent, close FDs
    │
    ├── SIGCONT? ──► If BLOCKED, set state → READY
    │
    └── Otherwise: deferred to next signal_check_pending()
                      │
                      ▼
                   Scheduler calls signal_check_pending(pid)
                      │
                      ▼
                   deliverable = pending & ~mask
                      │
                      ├── No bits set → return 0 (no signal)
                      │
                      └── Find first set bit (priority: low slot first)
                            │
                            ├── Handler = SIG_DFL → signal_deliver_default()
                            │     SIGKILL/TERM/INT/HUP/SEGV → ZOMBIE
                            │     SIGSTOP/TSTP → BLOCKED
                            │
                            └── Handler = SIG_IGN → clear bit, ignore
```

### 6.4 Syscalls

- `sys_kill(pid, signum)` -- Send signal to process
- `sys_signal(signum, handler)` -- Install handler (SIGKILL/SIGSTOP cannot be caught)

---

## 7. Copy-on-Write (CoW)

### 7.1 Refcount Array

32768 entries (one per 4KB frame) at 0x950000, 2 bytes each (max refcount 65535).
Fault counter at 0x950048 tracks total CoW faults handled.

### 7.2 CoW Fork Flow

```
  Parent calls fork()
    │
    ▼
  Clone PML4: copy kernel entries, for user entries:
    │
    ▼
  Walk page table levels (PML4 → PDPT → PD → PT)
  For each leaf (PT entry):
    │
    ├── Mark BOTH parent and child PT entries:
    │     Clear WRITABLE bit
    │     Set COW bit (bit 9, 0x200)
    │
    └── Increment refcount for shared physical frame
```

### 7.3 Page Fault Handling

```
  Write to CoW page triggers #PF (error code: write + present)
    │
    ▼
  cow_handle_fault(fault_addr)
    │
    ▼
  Walk page tables to find PT entry
    │
    ├── Not present? → return -1 (real fault)
    ├── No COW bit? → return -1 (real fault)
    │
    └── COW bit set:
          │
          ├── refcount <= 1 (sole owner):
          │     Set WRITABLE, clear COW → done (no copy needed)
          │
          └── refcount > 1 (shared):
                1. Allocate new frame
                2. Copy 4096 bytes from old frame to new
                3. Update PT entry: new_phys | WRITABLE, clear COW
                4. Decrement old frame refcount
                5. Set new frame refcount = 1
                6. Flush TLB (write CR3)
                7. Increment fault counter
```

### 7.4 Page Release

On process exit, `cow_release_pages(pml4)` walks the full page table tree.
For each leaf frame, decrement refcount. If refcount reaches 0, free the
frame back to the bitmap allocator.

---

## 8. Init System

### 8.1 Boot Sequence

```
  GRUB/Multiboot loads kernel ELF at 0x100000
    │
    ▼
  _start (assembly stub) → kernel_main()
    │
    ▼
  Hardware init:
    frames_init()        Frame bitmap, mark reserved regions
    heap_init()          Freelist allocator (0x400000-0x580000)
    gdt_load()           GDT with Ring 0 + Ring 3 segments + TSS
    idt_install()        IDT: 256 vectors, keyboard=0x21, timer=0x20
    lapic_init()         LAPIC timer (periodic, ~10ms)
    serial_init()        COM1 @ 0x3F8 (115200 baud)
    vga_init()           Clear screen, cursor at (0,0)
    pci_scan()           Enumerate PCI devices
    nvme_init()          NVMe controller if present
    virtio_net_init()    VirtIO network if present
    │
    ▼
  Filesystem init:
    ramfs_init()         Create /, /dev, /proc, /tmp, /home, /etc
    vfs_init()           Mount table: ramfs@/, devfs@/dev, procfs@/proc
    fat32_probe()        Try mounting NVMe/VirtIO as /mnt
    │
    ▼
  Service init (init_svc_init):
    PID 1: init          Service manager, respawner
    PID 2: blk           Block device service (NVMe, VirtIO-blk)
    PID 3: vfs           Virtual filesystem service
    PID 4: net           Network service (Ethernet/IP/TCP/UDP)
    PID 5: shell         Interactive shell
    + auth, gpu, display, input, gui, pkgmgr
    │
    ▼
  init_autorun()         Run /mnt/INIT.SH if present
    │
    ▼
  Shell prompt: FajarOS>
```

### 8.2 Service Lifecycle

```
  ┌─────────┐    register    ┌─────────┐    start    ┌─────────┐
  │  (none) │ ──────────────►│ STOPPED │ ───────────►│ RUNNING │
  └─────────┘                └─────────┘             └────┬────┘
                                  ▲                       │
                             stop │                  crash/exit
                                  │                       │
                            ┌─────┴─────┐           ┌────▼────┐
                            │  restart  │◄──────────│ FAILED  │
                            └───────────┘  restarts └─────────┘
                                           < max(5)
```

- Service table at 0x9B0000: 16 services x 64B
- Restart policies: NO (0), ALWAYS (1), ON_FAILURE (2)
- Max 5 auto-restarts before giving up (FAILED state)
- `init_check_respawn()` scans for zombied service processes

### 8.3 Runlevels

| Level | Name        | Services Active           |
|-------|-------------|---------------------------|
| 0     | halt        | Shutdown sequence         |
| 1     | single-user | Kernel + shell only       |
| 3     | multi-user  | All services (default)    |
| 5     | graphical   | All services + GUI        |

Stored at 0x9B1000. Switch via `init <level>` command.

### 8.4 Daemons

- **syslogd**: Centralized logging to ring buffer at 0x9B5000 (8KB),
  timestamped `[seconds] message` format, auto-rotation at 64KB
- **crond**: 8 periodic jobs at 0x9B8000, checked each tick,
  interval in ticks (100 = 1 second)
- **klogd**: Kernel message ring at 0x9B2000 (8KB), viewable via `dmesg`

---

## 9. GDB Integration

### 9.1 Architecture

GDB connects via serial port COM2 (0x2F8) using the GDB Remote Serial
Protocol (RSP). The kernel contains a built-in GDB stub that handles
RSP packets without requiring a separate debug agent.

```
  ┌──────────┐       serial (COM2)       ┌──────────────────┐
  │   GDB    │ ◄───────────────────────► │  GDB Stub        │
  │ (host)   │   $packet#checksum        │  (in-kernel)     │
  │          │   +ACK / -NACK            │                  │
  └──────────┘                           │  Packet buf:     │
                                         │    0x994000 (4K) │
                                         │  Response buf:   │
                                         │    0x995000 (4K) │
                                         │  State: 0x996000 │
                                         │  BP:    0x996100 │
                                         │  WP:    0x996300 │
                                         └──────────────────┘
```

### 9.2 RSP Protocol Flow

```
  GDB                          Stub
   │                            │
   ├── $?#3f ──────────────────►│  Query halt reason
   │◄── +$S05#b8 ──────────────┤  Signal 5 (SIGTRAP)
   │                            │
   ├── $g#67 ──────────────────►│  Read all registers
   │◄── +$<hex64 x 16>#xx ────┤  16 x 64-bit GPRs in hex
   │                            │
   ├── $m<addr>,<len>#xx ─────►│  Read memory
   │◄── +$<hex bytes>#xx ──────┤  Memory contents
   │                            │
   ├── $M<addr>,<len>:<hex>#xx►│  Write memory
   │◄── +$OK#9a ───────────────┤  Success
   │                            │
   ├── $Z0,<addr>,1#xx ───────►│  Set software breakpoint
   │◄── +$OK#9a ───────────────┤
   │                            │
   ├── $Z2,<addr>,4#xx ───────►│  Set write watchpoint
   │◄── +$OK#9a ───────────────┤
   │                            │
   ├── $s#73 ──────────────────►│  Single step
   │◄── +$S05#b8 ──────────────┤  Hit breakpoint
   │                            │
   ├── $qfThreadInfo#xx ───────►│  Query threads
   │◄── +$m1,2,3#xx ───────────┤  Thread IDs (PIDs+1)
   │                            │
   ├── $c#63 ──────────────────►│  Continue
   │                            │
```

### 9.3 Breakpoints and Watchpoints

- **Software breakpoints** (Z0): 16 slots at 0x996100, stores address +
  original instruction byte. INT3 (0xCC) inserted at target.
- **Hardware watchpoints** (Z2/Z3/Z4): 4 slots at 0x996300, uses x86
  debug registers (DR0-DR3). Supports write, read, and access watchpoints.

### 9.4 Thread Awareness

`qfThreadInfo` returns all non-FREE PIDs. GDB sees each process as a
thread (thread ID = PID + 1). Register reads pull from the saved context
frame on each process's kernel stack.

---

## 10. Build System

### 10.1 Concatenation Strategy

FajarOS Nova uses a concatenation build: all 126 .fj files are joined into
a single `combined.fj`, then compiled as one translation unit. This avoids
the need for a module/linker system at the OS level while maintaining a
modular source tree.

```
  126 .fj files                    Makefile
  ┌──────────────┐                ┌──────────────────────────────┐
  │ kernel/boot/ │                │ cat in dependency order:     │
  │ kernel/mm/   │                │  1. Constants + primitives   │
  │ kernel/sched/│ ──── cat ────► │  2. Memory (frames→paging→   │
  │ drivers/     │                │     heap→slab→cow)           │
  │ fs/          │                │  3. Drivers (serial→vga→...  │
  │ shell/       │                │     nvme→virtio)             │
  │ services/    │                │  4. Filesystems (ramfs→fat32 │
  │ apps/        │                │     →vfs)                    │
  │ kernel/main  │                │  5. Services + shell         │
  └──────────────┘                │  6. Apps + tests             │
         │                        │  7. kernel/main.fj (LAST)   │
         ▼                        └──────────────────────────────┘
  build/combined.fj                          │
  (single file, ~64K LOC)                    │
         │                                   │
         ▼                                   │
  fj build --target x86_64-none              │
  combined.fj -o build/fajaros.elf           │
         │                                   │
         ▼                                   │
  build/fajaros.elf (ELF64, x86_64)         │
         │                                   │
         ├── make run      → QEMU -nographic │
         ├── make run-kvm  → QEMU + KVM      │
         ├── make run-smp  → QEMU 4 cores    │
         ├── make run-nvme → QEMU + NVMe     │
         ├── make run-net  → QEMU + VirtIO   │
         ├── make debug    → QEMU -s -S :1234│
         └── make iso      → GRUB2 ISO       │
```

### 10.2 Concatenation Order

The order matters because Fajar Lang requires forward declaration. Constants
and low-level functions must appear before their callers:

1. **Constants** (`kernel/boot/constants.fj`)
2. **Memory management** (frames -> paging -> heap -> slab -> cow)
3. **Auth** (users, permissions, sessions)
4. **IPC** (message, pipe, channel, notify, shm)
5. **Scheduler** (process, signals, scheduler, smp, spinlock)
6. **Interrupts** (lapic, timer)
7. **Syscalls** (entry, dispatch, elf)
8. **Process ops** (fork, exec, wait, exit)
9. **Signals** (signal, jobs)
10. **Debug** (gdb_stub, gdb_ext)
11. **Security** (capability, limits, hardening)
12. **Drivers** (serial, vga, keyboard, pci, nvme, virtio_*, xhci, gpu)
13. **GPU compute** (buffers, kernels)
14. **Filesystems** (ramfs, directory, links, journal, fsck, ext2, fat32, vfs)
15. **Shell** (pipes, redirect, vars, control, commands, scripting)
16. **Kernel core** (smp_sched, mm_advanced, security, fast_ipc, stability, ...)
17. **Hardware detection** (detect, acpi, pcie, uefi_boot)
18. **Ring 3 embed** (user program ELF blobs)
19. **Services** (blk, net, display, input, gpu, gui, auth, shell, init, pkg, vfs)
20. **Applications** (editor, compiler, pkgmgr, user_programs, mnist)
21. **Tests + benchmarks**
22. **kernel/main.fj** (entry point, MUST be last)

### 10.3 QEMU Targets

| Target       | Flags                                        | Use Case         |
|-------------|----------------------------------------------|------------------|
| `run`       | `-nographic -cpu qemu64,+avx2,+sse4.2`      | Basic testing    |
| `run-kvm`   | `-enable-kvm -cpu host`                      | Fast execution   |
| `run-vga`   | VGA display enabled                          | GUI testing      |
| `run-smp`   | `-smp 4` + KVM                               | SMP testing      |
| `run-nvme`  | NVMe drive (64MB disk.img)                   | Storage testing  |
| `run-net`   | VirtIO-net, user networking                  | Network testing  |
| `debug`     | `-s -S` (GDB server on :1234)                | Debugging        |
| `iso`       | GRUB2 bootable ISO                           | Real hardware    |

### 10.4 Microkernel Build (v2.0)

A separate `make micro` target builds only the Ring 0 core (8 files,
target <2000 LOC): boot, mm, irq, sched, ipc, syscall, and stubs.
Services are built as separate ELF binaries via `make services`.

---

*ARCHITECTURE_NOVA_V09.md -- FajarOS Nova v1.4.0 "Zenith"*
*126 source files | 64K LOC | 34 syscalls | 240+ commands | 100% Fajar Lang*
