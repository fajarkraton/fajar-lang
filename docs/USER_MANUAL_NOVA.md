# FajarOS Nova User Manual

> **Version 1.4.0 "Zenith"** -- x86_64 bare-metal OS written 100% in Fajar Lang
> 240+ shell commands | 34 syscalls | Preemptive multitasking | 115 modular .fj files

---

## 1. Getting Started

### 1.1 Prerequisites

Install the following before building FajarOS Nova:

- **Fajar Lang compiler** (`fj`) -- https://github.com/fajarkraton/fajar-lang
- **QEMU x86_64** -- `sudo apt install qemu-system-x86`
- **GRUB2 tools** (for ISO builds) -- `sudo apt install grub-pc-bin grub-common xorriso mtools`

### 1.2 Building

FajarOS Nova uses a concatenation build: 115 modular `.fj` files are combined into a single `build/combined.fj`, then compiled to a kernel ELF.

```
make build
```

This produces `build/fajaros.elf`. To see the line count breakdown:

```
make loc
```

### 1.3 Booting in QEMU

| Command | Description |
|---------|-------------|
| `make run` | Serial console, no KVM, no graphics |
| `make run-kvm` | KVM acceleration (fastest) |
| `make run-vga` | VGA display (80x25 color console) |
| `make run-smp` | 4 CPU cores (SMP with AP trampoline) |
| `make run-nvme` | NVMe storage (auto-creates 64MB disk.img) |
| `make run-net` | Networking (virtio-net, user-mode) |
| `make debug` | GDB server on port 1234 (paused at entry) |

For a bootable ISO via GRUB2:

```
make iso
make run-iso
```

### 1.4 First Commands

After boot, the shell prompt appears as `nova>`. Try these:

```
nova> help          # list all available commands
nova> version       # show FajarOS Nova version
nova> uname         # show kernel info (x86_64, Fajar Lang)
nova> uptime        # show time since boot
nova> clear         # clear the screen
```

### 1.5 Command History

The shell stores the last 8 commands. Use the up/down arrow keys to navigate history.

---

## 2. Shell Guide

### 2.1 Pipes

Chain commands with `|`. The output of the left command feeds into the right:

```
nova> cat /etc/motd | wc
nova> ls | grep .txt
nova> dmesg | head 10
```

Pipes use a circular 4KB buffer with refcounting and EOF detection.

### 2.2 I/O Redirection

| Operator | Action |
|----------|--------|
| `>` | Write stdout to file (overwrites) |
| `>>` | Append stdout to file |
| `<` | Read stdin from file |

```
nova> echo Hello > /tmp/greeting
nova> echo World >> /tmp/greeting
nova> cat < /tmp/greeting
```

### 2.3 Environment Variables

Set and use variables with `$VAR` expansion:

```
nova> export NAME=Nova
nova> echo $NAME
Nova
nova> export PATH=/bin
nova> env                   # list all variables
```

The special variable `$?` holds the last exit code. The shell supports up to 128 environment variables (16-char keys, 16-char values).

### 2.4 Control Flow

The shell supports `if`, `for`, and `while` constructs:

**if/then/else:**
```
nova> if test -f /etc/motd; then echo exists; else echo missing; fi
```

**for loop:**
```
nova> for i in 1 2 3; do echo $i; done
```

**while loop:**
```
nova> while test -f /tmp/flag; do echo waiting; done
```

### 2.5 Scripts

Write shell commands to a file and execute it:

```
nova> echo "echo Hello from script" > /tmp/myscript
nova> sh /tmp/myscript
Hello from script
```

The `test` builtin supports `-f` (file exists) and `-d` (directory exists) flags.

---

## 3. User Management

FajarOS Nova supports up to 16 user accounts. The `root` user (UID 0) is created at boot with default password `root`.

### 3.1 Commands

| Command | Description |
|---------|-------------|
| `whoami` | Print current username |
| `id` | Show UID and GID |
| `users` | List all active user accounts |
| `adduser <name> <pass>` | Create new user (root only) |
| `passwd <newpass>` | Change current user's password |
| `login <user>` | Switch to another user (prompts for password) |
| `logout` | End current session, return to login |
| `su <user>` | Switch user within the same session |

### 3.2 Permissions

Files use Unix-style `rwxrwxrwx` permission bits (owner/group/other):

```
nova> chmod 755 /tmp/script     # rwxr-xr-x
nova> chown fajar /tmp/myfile   # change owner
nova> ls -l                     # show permissions
```

Only root (UID 0) can run `adduser` and `chown`. Passwords are stored as FNV-1a hashes.

---

## 4. File Operations

### 4.1 Basic Commands

| Command | Description |
|---------|-------------|
| `ls` | List files in current directory |
| `ls -l` | Long listing with size, permissions, type |
| `cat <file>` | Print file contents |
| `touch <file>` | Create empty file |
| `rm <file>` | Delete file |
| `cp <src> <dst>` | Copy file |
| `mv <src> <dst>` | Move/rename file |
| `mkdir <dir>` | Create directory |
| `rmdir <dir>` | Remove empty directory |
| `ln <target> <link>` | Create hard link |
| `ln -s <target> <link>` | Create symbolic link |
| `stat <file>` | Show file metadata (size, type, inode) |
| `cd <dir>` | Change directory |
| `pwd` | Print working directory |

### 4.2 Text Processing

| Command | Description |
|---------|-------------|
| `head <n> <file>` | Print first n lines |
| `tail <n> <file>` | Print last n lines |
| `wc <file>` | Count lines, words, bytes |
| `grep <pattern> <file>` | Search for text in file |
| `sort <file>` | Sort lines alphabetically |
| `uniq <file>` | Remove adjacent duplicate lines |
| `cut <file>` | Extract fields from lines |
| `xxd <file>` | Hex dump |
| `strings <file>` | Print printable character sequences |
| `md5 <file>` | Compute MD5 checksum |

### 4.3 Filesystems

FajarOS Nova supports three filesystem types:

| Filesystem | Mount Point | Description |
|------------|-------------|-------------|
| **RamFS** | `/` | In-memory filesystem (64 entries, 832KB data). Default root. |
| **FAT32** | `/mnt` | Read/write FAT32 from NVMe or VirtIO block devices. |
| **ext2** | user-defined | Create with `mkfs.ext2`, mount manually. |

Special directories:

```
/dev           # Device files: null, zero, random
/proc          # Kernel info: version, uptime, meminfo, cpuinfo
/mnt           # Mount point for block devices
```

VFS commands:

```
nova> mount                     # show mount table
nova> df                        # show filesystem usage
nova> fsck                      # run filesystem consistency check
```

The journaling subsystem uses write-ahead logging (WAL) for crash recovery.

---

## 5. Process Management

### 5.1 Process Table

FajarOS Nova supports 16 concurrent processes (PIDs 0-15). PID 0 is the idle task, PID 1 is init. Processes have four states: ready, running, blocked, and zombie.

### 5.2 Commands

| Command | Description |
|---------|-------------|
| `ps` | List all processes with PID, state, and name |
| `top` | Show process resource usage |
| `kill <pid>` | Send SIGTERM to a process |
| `kill -9 <pid>` | Send SIGKILL (immediate termination) |
| `jobs` | List background jobs |
| `fg <job>` | Bring job to foreground |
| `bg <job>` | Resume job in background |

### 5.3 Fork/Exec Model

Processes are created with `fork()` (Copy-on-Write page tables with refcounting) and replaced with `exec()` (ELF64 loader from ramfs or FAT32). The page fault handler triggers CoW duplication on write.

```
nova> spawn /bin/hello          # fork + exec a Ring 3 program
nova> wait                      # wait for child process
nova> cowstat                   # show CoW statistics
```

### 5.4 Signals

| Signal | Number | Action |
|--------|--------|--------|
| SIGINT | 2 | Keyboard interrupt (Ctrl+C) |
| SIGKILL | 9 | Immediate termination (cannot be caught) |
| SIGSEGV | 11 | Segmentation fault |
| SIGTERM | 15 | Graceful termination |
| SIGCONT | 18 | Resume stopped process |
| SIGSTOP | 19 | Pause process |

### 5.5 Ring 3 User Programs

Five embedded user programs run in Ring 3 (user mode) via SYSCALL/SYSRET:

```
nova> exec hello        # "Hello from Ring 3!"
nova> exec counter      # counts 1 to 10
nova> exec fibonacci    # prints Fibonacci sequence
nova> exec fajar        # "Fajar Lang is running in Ring 3!"
nova> exec goodbye      # "Goodbye from user space!"
```

---

## 6. Networking

### 6.1 Overview

Networking requires the virtio-net driver. Boot with `make run-net` to enable it.

The network stack implements: Ethernet framing, ARP (request/reply with cache), IPv4 (header + checksum), ICMP (echo/reply), TCP (RFC 793 state machine), UDP, DNS resolution, DHCP, and HTTP.

### 6.2 Commands

| Command | Description |
|---------|-------------|
| `ifconfig` | Show network interface (MAC, IP, mask, gateway, stats) |
| `ping <ip>` | Send ICMP echo request |
| `wget <url>` | Fetch a resource over HTTP |
| `netstat` | Show active TCP/UDP connections |
| `tcpstat` | Show TCP state machine statistics |
| `arp` | Display ARP cache table |
| `dns <hostname>` | Resolve hostname to IP |
| `dhcp` | Request IP configuration via DHCP |

### 6.3 Servers

```
nova> httpd start       # Start HTTP server (serves /www)
nova> httpd stop        # Stop HTTP server
nova> echo-server 7     # Start echo server on port 7
```

The TCP implementation includes SYN, SYN-ACK, ACK, FIN handshakes with sequence number tracking. The socket API supports bind, listen, accept, connect, send, and recv operations.

### 6.4 Configuration

```
nova> ifconfig eth0 10.0.2.15 netmask 255.255.255.0
nova> route add default gw 10.0.2.2
```

With QEMU user-mode networking, the default gateway is `10.0.2.2` and host-forwarded ports are accessible via `10.0.2.15`.

---

## 7. Package Management

### 7.1 Commands

| Command | Description |
|---------|-------------|
| `pkg list` | List all packages (installed and available) |
| `pkg search <name>` | Search for a package by name |
| `pkg install <name>` | Install a package |
| `pkg remove <name>` | Uninstall a package |
| `pkg update` | Refresh the package registry |
| `pkg upgrade` | Upgrade all installed packages |
| `pkg info <name>` | Show package details (version, size, deps) |

### 7.2 Standard Packages

| Package | Description |
|---------|-------------|
| `fj-math` | Math library (trig, linear algebra, statistics) |
| `fj-nn` | Neural network layers and optimizers |
| `fj-hal` | Hardware abstraction layer |
| `fj-http` | HTTP client and server |
| `fj-crypto` | Cryptographic primitives (AES, SHA, HMAC) |

The package database stores up to 32 packages, each with name, version, state, dependencies, file count, size, and checksum.

### 7.3 Example

```
nova> pkg update
[OK] Registry refreshed: 5 packages available
nova> pkg install fj-math
[OK] fj-math 1.0.0 installed
nova> pkg list
  fj-math     1.0.0  [installed]
  fj-nn       1.0.0  [available]
  fj-hal      1.0.0  [available]
  fj-http     1.0.0  [available]
  fj-crypto   1.0.0  [available]
```

---

## 8. Service Management

### 8.1 Init System

FajarOS Nova has an init system with 16 service slots. Services support three restart policies: no restart, restart always, and restart on failure.

### 8.2 Commands

| Command | Description |
|---------|-------------|
| `service list` | List all registered services and their status |
| `service start <name>` | Start a service |
| `service stop <name>` | Stop a service |
| `service status <name>` | Show service state (stopped/running/failed/starting) |
| `service restart <name>` | Stop then start a service |
| `runlevel` | Show current runlevel |
| `init <level>` | Switch runlevel |
| `crontab` | Show scheduled tasks |
| `syslog` | View system log messages |

### 8.3 Runlevels

| Level | Name | Description |
|-------|------|-------------|
| 0 | halt | System shutdown |
| 1 | single-user | Maintenance mode, minimal services |
| 3 | multi-user | Default boot level, all services |
| 5 | graphical | Multi-user with VirtIO-GPU framebuffer |

```
nova> runlevel
Current runlevel: 3 (multi-user)
nova> init 1
Switching to runlevel 1
```

### 8.4 Built-in Services

The init system (PID 1) manages these services via IPC message passing:

- **blk** -- Block device service (NVMe, VirtIO-blk, journal)
- **vfs** -- Virtual filesystem service
- **net** -- Network service (TCP/UDP/DNS/DHCP, PID 4)
- **shell** -- Interactive shell service
- **display** -- VGA/framebuffer display
- **input** -- Keyboard input handler
- **gpu** -- VirtIO-GPU compute dispatch
- **auth** -- User authentication service
- **compiler** -- In-kernel Fajar Lang compiler

Services auto-restart based on their restart policy. Use `service list` to see restart counts.

---

## 9. Debugging

### 9.1 GDB Remote Debugging

FajarOS Nova includes a GDB remote stub over COM2 (serial port 0x2F8), supporting the RSP protocol with register read/write, memory read/write, breakpoints (up to 16), watchpoints, and thread queries.

**Step 1:** Start QEMU in debug mode (paused at entry):

```
make debug
```

**Step 2:** In another terminal, connect GDB:

```
gdb build/fajaros.elf -ex "target remote :1234"
```

**Step 3:** Use GDB as normal:

```
(gdb) break kernel_main
(gdb) continue
(gdb) info registers
(gdb) x/16x 0x100000
(gdb) step
```

### 9.2 Kernel Diagnostics

| Command | Description |
|---------|-------------|
| `dmesg` | Kernel message log (boot messages, driver init) |
| `meminfo` | Memory usage (frames allocated/free, heap, slab) |
| `cowstat` | Copy-on-Write page fault statistics |
| `tcpstat` | TCP connection state machine stats |
| `lspci` | List PCI devices (vendor:device, class, BAR) |
| `ioports` | Show registered I/O port ranges |
| `interrupts` | Show interrupt counts per IRQ |
| `slabinfo` | Slab allocator statistics (per size class) |

### 9.3 Kernel Self-Tests

```
nova> test run          # run built-in kernel test suite
make test               # run tests in QEMU with auto-exit
```

The test framework verifies memory allocation, IPC message passing, process lifecycle, filesystem operations, and network protocol correctness.

---

## 10. Hardware Info

### 10.1 Commands

| Command | Description |
|---------|-------------|
| `cpuinfo` | CPU model, frequency, features (SSE, AVX2) |
| `smp` | SMP status (AP count, per-CPU LAPIC IDs) |
| `gpu` | GPU detection (NVIDIA/Intel/AMD via PCI class) |
| `nvme` | NVMe controller info (model, capacity, queues) |
| `lspci` | Full PCI bus enumeration |
| `pcie-scan` | PCIe extended config space scan (via ECAM/MCFG) |
| `acpi` | ACPI table info (RSDP, MADT, MCFG, FADT, HPET) |
| `thermal` | CPU temperature reading (if ACPI thermal zone available) |

### 10.2 ACPI

The kernel scans `0xE0000-0xFFFFF` for the RSDP signature, then walks RSDT/XSDT to parse:

- **MADT** -- LAPIC IDs, I/O APIC address, interrupt source overrides
- **MCFG** -- PCIe ECAM base address for extended configuration
- **FADT** -- Power management (PM1a control, PM timer, SCI interrupt)
- **HPET** -- High Precision Event Timer base address

```
nova> acpi
RSDP found at 0xF0010 (rev 2)
XSDT at 0x7FFE0000 (4 entries)
MADT: 4 CPUs, IOAPIC at 0xFEC00000
MCFG: ECAM base 0xB0000000
FADT: SCI IRQ 9, PM1a 0x600
```

### 10.3 GPU Compute

FajarOS Nova includes a VirtIO-GPU driver with a framebuffer (320x200) and compute dispatch:

```
nova> gpu info                  # show GPU device and framebuffer
nova> gpu draw 10 10 0xFF0000   # draw red pixel at (10,10)
nova> gpu rect 0 0 100 50 0x00FF00  # green rectangle
nova> gpu clear                 # clear framebuffer
```

GPU compute kernels (matmul, vecadd) are dispatched via syscalls 35-36, using a 16-slot buffer pool of 4KB each.

### 10.4 Storage

```
nova> nvme info         # NVMe controller status
nova> nvme read 0       # read sector 0
nova> blkstat           # block device statistics
nova> df                # filesystem disk usage
```

NVMe uses PCI BAR0 for MMIO, with admin queue (slot 0) and I/O queue (slot 1) for sector-level read/write.

---

## Quick Reference Card

```
help                    Show all commands
version / uname         System info
ls / cat / touch / rm   File basics
ps / kill / jobs        Process management
ifconfig / ping         Networking
pkg list / pkg install  Package management
service list / init     Service control
dmesg / meminfo / lspci Diagnostics
make run                Boot in QEMU
make debug              GDB debugging
```

---

*FajarOS Nova v1.4.0 "Zenith" -- 34,000+ LOC | 115 .fj files | 100% Fajar Lang*
*Built with Fajar Lang + Claude Opus 4.6*
