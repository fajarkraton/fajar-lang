# Building a Production OS in Fajar Lang: 360 Tasks in One Session

> **Author:** Fajar (PrimeCore.id)
> **Date:** 2026-03-25
> **Tags:** OS development, Fajar Lang, x86_64, bare-metal, systems programming

---

## From Demo Shell to Production OS

FajarOS Nova started as a 9,637-line demo shell that could boot on x86_64 and display 148 commands. In a single coding session, we transformed it into a **production-grade UNIX-like operating system** with 18,159 lines of Fajar Lang code, 32 syscalls, multi-user authentication, a journaling filesystem, an HTTP server, and a GDB remote debugger.

This is the story of how we built it — 360 tasks across two major releases (v0.7 "Nexus" + v0.8 "Bastion"), all in 100% Fajar Lang.

---

## The Starting Point: Nova v0.6

Before v0.7, Nova was impressive but fundamentally a demo:

```
Nova v0.6 "Ascension":
  - 12,954 LOC, 181 shell commands
  - Preemptive scheduler (10ms quantum, 16 PIDs)
  - 5 Ring 3 user programs (hardcoded machine code)
  - NVMe + FAT32 + USB mass storage
  - DHCP + TCP + DNS + HTTP wget
  - 5 syscalls (hardcoded cmp/je chain in assembly)
```

The critical gap: **all the pieces existed but didn't work together**. The ELF loader existed but wasn't connected to exec(). The pipe pool existed but wasn't wired to file descriptors. The process table had no fork(). The shell had no scripting.

---

## Phase 1: v0.7 "Nexus" — Connecting Everything

### The Syscall Revolution (Phase F)

The first thing we fixed was the syscall dispatch. The old approach was a hardcoded assembly chain:

```asm
; OLD: 5 syscalls, hardcoded cmp/je
cmp rax, 0          ; SYS_EXIT
je .Lsys_exit
cmp rax, 1          ; SYS_WRITE
je .Lsys_write
; ... 3 more
```

We replaced this with an **indirect call through a function pointer**:

```asm
; NEW: unlimited syscalls via table dispatch
call QWORD PTR [0x884008]  ; syscall_dispatch(num, arg0, arg1, arg2)
```

The Fajar Lang `syscall_dispatch()` function then routes to 32 handlers. Adding a new syscall is now a one-line change instead of modifying assembly.

### fork() with Deep Page Table Copy (Phase G)

The fork implementation walks the 4-level x86_64 page table hierarchy:

```
PML4 → PDPT → PD → PT → Physical Page
```

For each page with the `PAGE_USER` flag, we:
1. Allocate a new physical frame
2. Copy 4KB of data
3. Map in the child's page table with the same flags

The child's context frame gets `RAX=0` (fork returns 0 to child), while the parent gets the child PID. This is classic UNIX semantics, implemented entirely in Fajar Lang's `@kernel` context.

### Pipes That Actually Work (Phase H)

We built a **circular 4064-byte pipe buffer** with reference counting:

```
Pipe Pool (0x898000): 8 pipes x 4KB
  +0:  in_use flag
  +8:  read_pos  (modular, wraps at 4064)
  +16: write_pos (modular, wraps at 4064)
  +32: data[4064]

Refcount Table (0x8D4000): 8 pipes x 16B
  +0: reader_count
  +8: writer_count
```

When all writers close, `reader_count > 0` but `writer_count == 0`, so read returns 0 (EOF). This enables `echo hello | cat` to work correctly — the shell creates a pipe, redirects stdout of `echo` to the write end, stdin of `cat` to the read end, and `cat` sees EOF when `echo` exits.

### Signals and Job Control (Phase I)

We implemented 8 POSIX signals with a bitmap-based pending/delivery system:

| Slot | Signal | Default Action |
|------|--------|---------------|
| 0 | SIGHUP | Terminate |
| 1 | SIGINT | Terminate (Ctrl+C) |
| 2 | SIGKILL | Immediate kill (uncatchable) |
| 3 | SIGSEGV | Terminate |
| 4 | SIGTERM | Terminate |
| 5 | SIGCHLD | Ignore |
| 6 | SIGSTOP | Stop (uncatchable) |
| 7 | SIGCONT | Continue |

Ctrl+C is detected in the keyboard scancode handler (scancode 0x2E + Ctrl state), which calls `signal_fg_group(SIGINT)` to signal all processes in the foreground group.

### Shell Scripting (Phase J)

The shell now supports:
- **Environment variables**: `export FOO=bar`, `echo $FOO`, `$?`, `$$`
- **Script files**: `sh script.sh` reads line-by-line from ramfs
- **Control flow**: `if/then/else/fi`, `for/in/do/done`, `while/do/done`
- **Test builtin**: `test -f file`, `test -d dir`

Variable expansion happens in `shell_expand_vars()` before pipe/redirect processing, which happens before command dispatch. The pipeline: **expand → pipe check → redirect check → dispatch**.

---

## Phase 2: v0.8 "Bastion" — Production Hardening

### Copy-on-Write Fork (Phase L)

The deep-copy fork from v0.7 copies every page on fork — 16 pages × 4KB = 64KB per fork. With CoW, fork is **instant**:

1. Mark all user pages as **read-only + CoW flag** (PTE bit 9)
2. Set refcount = 2 (parent + child share the physical frame)
3. On first write → **page fault** → allocate new frame → copy 4KB → remap writable

The page fault handler is in the linker's `__isr_14`:

```asm
__isr_14:
    ; Read CR2 (faulting address)
    mov rdi, cr2
    ; Call cow_handle_fault(fault_addr) via fn pointer
    call QWORD PTR [0x950040]
    ; If returned 0 → CoW handled, resume with IRETQ
    test rax, rax
    jz .Lpf_cow_ok
    ; Else → real fault, kill process
```

### Multi-User Security (Phase M)

Nova now has a complete user account system:

```
User Table (0x960000): 16 users x 64B
  +0:  uid
  +8:  username[16]
  +24: password_hash (FNV-1a)
  +32: gid
  +40: home[16]
  +56: active
```

File permissions use the standard UNIX model — `rwxrwxrwx` (9 bits, stored as octal in ramfs entry +72). Root (uid=0) bypasses all permission checks. The `chmod 755 file` command parses three octal digits and stores `7*64 + 5*8 + 5 = 493` in the mode field.

### Journaling Filesystem (Phase N)

The write-ahead log ensures crash recovery:

```
Journal (0x970000): 64KB
  Header: entry_count, committed, sequence
  Entries: 1000 x 64B (type, inode, offset, len, data[32])
```

Operations:
1. `journal_add(CREATE, inode, ...)` — log the operation
2. Apply changes to filesystem
3. `journal_commit()` — mark as committed

On boot, `journal_replay()` checks if the journal is committed. If not, it replays uncommitted entries — recovering from a crash mid-write.

### TCP Socket API + HTTP Server (Phase O)

Five new syscalls enable network servers:

```
SYS_SOCKET(27)  → create socket, return FD
SYS_BIND(28)    → bind to local port
SYS_LISTEN(29)  → mark as listening
SYS_ACCEPT(30)  → accept connection, return new FD
SYS_CONNECT(31) → connect to remote
```

The HTTP server parses GET requests, serves static files from ramfs, and provides JSON endpoints:

```
GET /              → "Welcome to FajarOS Nova HTTP Server!"
GET /proc/version  → {"kernel":"FajarOS Nova v1.3.0 Bastion","arch":"x86_64"}
GET /proc/uptime   → {"uptime_seconds":42}
GET /dir/          → HTML directory listing
GET /missing       → 404 Not Found
```

### GDB Remote Debugging (Phase P)

The GDB stub implements the RSP protocol over COM2 (0x2F8):

```
GDB → Nova: $?#3f             (halt reason)
Nova → GDB: +$S05#b8          (SIGTRAP)

GDB → Nova: $g#67             (read registers)
Nova → GDB: +$00000000...#xx  (16 GPRs as hex)

GDB → Nova: $m100000,10#xx    (read 16 bytes at 0x100000)
Nova → GDB: +$48656c6c6f...   (hex dump)

GDB → Nova: $Z0,100040,1#xx   (set breakpoint at 0x100040)
Nova → GDB: +$OK#9a           (breakpoint set, INT3 written)
```

---

## The Numbers

| Metric | v0.6 (Before) | v0.7 "Nexus" | v0.8 "Bastion" |
|--------|--------------|-------------|----------------|
| **LOC** | 12,954 | 15,732 | 18,159 |
| **@kernel fns** | 408 | 535 | 651 |
| **Commands** | 181 | 200 | 229 |
| **Syscalls** | 5 | 26 | 32 |
| **Fork** | None | Deep-copy | Copy-on-Write |
| **Users** | root only | root only | 16 accounts |
| **Filesystem** | ramfs + FAT32 | + pipes/redirect | + journal + symlinks |
| **Network** | TCP client | + pipes | + HTTP server + sockets |
| **Debug** | None | None | GDB remote (14 commands) |
| **Tests** | ~6,000 | ~6,076 | 6,186 |

### Task Breakdown

```
v0.7 "Nexus":   12 sprints, 120 tasks (Phase F-K)
  F: Syscall dispatch      20 tasks
  G: fork/exec/waitpid     30 tasks
  H: Pipes + redirect      20 tasks
  I: Signals + jobs         20 tasks
  J: Shell scripting        20 tasks
  K: Release                10 tasks

v0.8 "Bastion": 12 sprints, 120 tasks (Phase L-Q)
  L: Copy-on-Write fork    20 tasks
  M: Multi-user + perms    30 tasks
  N: Directory tree + journal 20 tasks
  O: Sockets + HTTP server  20 tasks
  P: GDB debugger           20 tasks
  Q: Release                10 tasks

QEMU Verification:  3 sprints, 30 tasks (87 checks passed)
fajaros-x86 Sync:   4 sprints, 40 tasks (11 new modules, 100 .fj files)
Commit + Push:      1 sprint, 10 tasks

TOTAL: 32 sprints, 320 tasks
```

---

## Memory Map

All v0.7+v0.8 allocations in the 0x8D0000-0x996000 range:

```
0x8D0000  FD_TABLE_V2      4KB   File descriptor table (relocated)
0x8D1000  SIGNAL_TABLE     4KB   Signal pending + handlers
0x8D2000  PROC_WAIT_TABLE  4KB   waitpid blocking state
0x8D3000  ENV_TABLE        4KB   Environment variables (128 entries)
0x8D4000  PIPE_REFCOUNT    4KB   Pipe reader/writer counts
0x8D5000  SCRIPT_STATE     4KB   Shell control flow state machine
0x8D6000  ARGV_BUF         8KB   exec() argument passing
0x8D8000  JOB_TABLE        4KB   Background job tracking
0x950000  PAGE_REFCOUNT    64KB  CoW page refcount table
0x960000  USER_TABLE       4KB   User accounts (16 users)
0x962000  LOGIN_HISTORY    1KB   Login/logout events
0x970000  JOURNAL          64KB  Filesystem write-ahead log
0x980000  SOCKET_TABLE     4KB   Network sockets (16 sockets)
0x982000  SOCKET_BUFFERS   64KB  Socket rx/tx buffers
0x990000  HTTP_BUFFERS     12KB  HTTP request + response
0x994000  GDB_BUFFERS      12KB  GDB packet + response
0x996000  GDB_STATE        1KB   Breakpoints + watchpoints
```

---

## Lessons Learned

1. **Fajar Lang's `@kernel` annotation works** — it prevents heap allocation and tensor ops in kernel code, catching bugs at compile time.

2. **Indirect syscall dispatch is essential** — the cmp/je chain doesn't scale past 5 syscalls. A function pointer at a fixed address is clean and extensible.

3. **CoW is worth the complexity** — fork went from copying 64KB per process to zero-copy. The page fault handler adds ~30 lines but saves orders of magnitude on fork-heavy workloads.

4. **The FD abstraction unifies everything** — console, files, pipes, and sockets all go through the same read/write/close interface. Adding a new I/O type is just a new FD_TYPE constant.

5. **10 tasks per sprint is the sweet spot** — small enough to complete in one session, large enough to deliver meaningful features. The quality gate after each phase catches regressions early.

---

## What's Next

Nova v0.9 "Zenith" is planned with:
- VirtIO-GPU framebuffer + compute dispatch
- ext2-like filesystem on NVMe (persistent, with inodes)
- TCP state machine with retransmission
- Init system with service management (syslogd, crond)
- Package manager with dependency resolution

The goal: make FajarOS Nova the first **production-deployed OS written entirely in a new language**.

---

*Built with Fajar Lang + Claude Opus 4.6*
*FajarOS Nova v1.3.0 "Bastion" — 18,159 LOC, 32 syscalls, 229 commands*
*github.com/fajarkraton/fajar-lang*
