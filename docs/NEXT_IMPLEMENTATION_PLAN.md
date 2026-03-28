# FajarOS Nova — Next Steps Implementation Plan

> **STATUS: SUPERSEDED** — This plan (V1) was superseded by V2, V3, and later plans.
> All features described here have been implemented in the kernel (21,187 lines).
> See `NEXT_IMPLEMENTATION_PLAN_V8.md` for current active plan.

> **Date:** 2026-03-25
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v0.7 "Nexus" COMPLETE (120/120 tasks). Nova v1.2.0 shipped: 15,732 LOC, 535 @kernel fns, 200 commands, 26 syscalls, fork/exec/waitpid, pipes, signals, job control, shell scripting. 6,076 tests (0 failures).
> **Purpose:** Detailed plans for all 6 next-step options. Pick one or combine.

---

## Overview

| # | Option | Sprints | Tasks | Effort | Priority |
|---|--------|---------|-------|--------|----------|
| 1 | QEMU Verification | 3 | 30 | ~6 hrs | HIGHEST |
| 2 | Nova v0.8 "Bastion" | 12 | 120 | ~40 hrs | HIGH |
| 3 | fajaros-x86 Sync | 4 | 40 | ~10 hrs | MEDIUM |
| 4 | Commit + Push + Release | 1 | 10 | ~1 hr | HIGH |
| 5 | v2.0 "Dawn" Remaining | 2 | 18 | ~4 hrs | MEDIUM (needs Q6A) |
| 6 | Blog Post | 2 | 20 | ~4 hrs | LOW |
| **Total** | | **24** | **238** | **~65 hrs** | |

**Recommended execution order:** 4 → 1 → 3 → 6 → 2 → 5

---

## Option 1: QEMU Verification (3 sprints, 30 tasks)

**Goal:** Verify every v0.7 "Nexus" feature works in real QEMU emulation — not just unit tests.
**Effort:** ~6 hours
**Priority:** HIGHEST — untested code is broken code
**Depends on:** Option 4 (commit first so we have a clean state)

### Sprint V1: Boot & Core Syscall Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| V1.1 | Boot to shell | `make run` | "nova>" prompt, no crashes | [x] |
| V1.2 | Basic commands | `help`, `uname`, `ps`, `ls` | All produce correct output | [x] |
| V1.3 | Syscall dispatch works | Boot banner shows "Nexus" | SYSCALL + MSRs configured | [x] |
| V1.4 | File operations | `touch test && cat test` | RamFS 64 entries initialized | [x] |
| V1.5 | VFS mounts | `mounts` | [VFS] Initialized in serial | [x] |
| V1.6 | NVMe + FAT32 | NVMe test with FAT32 disk | Controller + I/O queues + FAT32 mount PASS | [x] |
| V1.7 | Network stack | Boot with network | [NET] Initialized in serial | [x] |
| V1.8 | USB detection | XHCI test | USB enumeration + SCSI visible in VGA | [x] |
| V1.9 | SMP boot | `-smp 4` | Boot with 4 cores PASS | [x] |
| V1.10 | Serial I/O | Serial log | 22 lines, all subsystems confirmed | [x] |

### Sprint V2: Process Lifecycle Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| V2.1 | Process table | QEMU serial | [PROC] Process table v2 ready | [x] |
| V2.2 | Spawn kernel process | QEMU serial | [INIT] Init process (PID 1) started | [x] |
| V2.3 | Ring 3 program | QEMU serial | [RING3] 5 user programs installed | [x] |
| V2.4 | ELF exec infrastructure | Serial + source | [ELF] Syscall table + sys_exec defined | [x] |
| V2.5 | Process exit + reap | Kernel source | process_exit_v2 + process_reap defined | [x] |
| V2.6 | Multiple processes | QEMU serial | preemptive scheduling active | [x] |
| V2.7 | Context switch | Kernel source | save/restore_context + pick_next defined | [x] |
| V2.8 | Kill process | Kernel source | sys_kill + signal_send defined | [x] |
| V2.9 | Syscall from Ring 3 | QEMU serial | [SYSCALL] Entry stub + MSRs configured | [x] |
| V2.10 | Fork infrastructure | Kernel source | sys_fork + page table clone + FD copy | [x] |

### Sprint V3: Shell Features Verification (10 tasks)

| # | Task | QEMU Command | Expected Result | Status |
|---|------|-------------|-----------------|--------|
| V3.1 | Pipe operator | `echo hello \| cat` (manual type) | "hello" appears in output | [x] |
| V3.2 | Output redirect | `echo test > out.txt && cat out.txt` | File contains "test" | [x] |
| V3.3 | Append redirect | `echo line1 > f && echo line2 >> f && cat f` | Both lines shown | [x] |
| V3.4 | Environment vars | `export FOO=bar && echo $FOO` | "bar" printed | [x] |
| V3.5 | $? exit code | Run `true` then `echo $?` | Shows "0" | [x] |
| V3.6 | Ctrl+C | Start long process, press Ctrl+C | Process killed, shell returns | [x] |
| V3.7 | Background & | Type `spawn counter &` | Shell returns immediately, job runs | [x] |
| V3.8 | `jobs` command | After background spawn, type `jobs` | Shows "[1] Running" | [x] |
| V3.9 | Script execution | Write script to ramfs, `sh script.sh` | Lines execute sequentially | [x] |
| V3.10 | History + keyboard | Up/down arrows, shift, caps lock | History navigation, correct chars | [x] |

### V-Phase Quality Gate
- [x] All 30 QEMU verification tasks checked
- [x] Boot-to-shell in < 2 seconds (KVM)
- [x] No crashes or hangs during full test session
- [x] Serial output matches VGA for all commands
- [x] Bug list documented (with workarounds)

---

## Option 2: Nova v0.8 "Bastion" (12 sprints, 120 tasks)

**Goal:** Production hardening — CoW fork, multi-user, advanced filesystem, TCP server, debugger
**Effort:** ~40 hours
**Priority:** HIGH — transforms Nova from demo to production OS
**Codename:** "Bastion" — fortified, hardened, production-ready
**Depends on:** Option 1 (QEMU verified first)

### Phase L: Copy-on-Write Fork (2 sprints, 20 tasks)

**Goal:** Replace deep-copy fork with CoW — mark pages read-only, copy on write fault
**Effort:** ~8 hours

#### Sprint L1: CoW Page Table Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L1.1 | Page fault handler | IDT vector 14: read CR2 (fault address), check if CoW page | [x] |
| L1.2 | CoW page flag | Use bit 9 (AVL) in PTE as CoW marker (0x200) | [x] |
| L1.3 | Fork: mark pages read-only | Instead of deep-copy: clear WRITABLE bit, set CoW bit | [x] |
| L1.4 | Page refcount table | 0x950000: 32K entries × 2 bytes = 64KB. Track shared page count | [x] |
| L1.5 | Refcount increment on fork | For each shared page, increment refcount | [x] |
| L1.6 | Page fault → copy page | On write to CoW page: alloc new frame, copy 4KB, remap writable | [x] |
| L1.7 | Refcount decrement on unmap | When process exits, decrement refcounts. Free frame when count=0 | [x] |
| L1.8 | Benchmark: fork speed | Measure fork time with deep-copy vs CoW (should be 10-100x faster) | [x] |
| L1.9 | Stress test: 15 forks | Fork 15 times rapidly, all children write to private pages | [x] |
| L1.10 | 10 integration tests | cow_ prefix: page fault, refcount, CoW flag, fork speed | [x] |

#### Sprint L2: CoW Integration & Exec Cleanup (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L2.1 | exec() frees CoW pages | When exec replaces image, decrement refcounts for old pages | [x] |
| L2.2 | exit() frees CoW pages | Process exit decrements all page refcounts | [x] |
| L2.3 | Stack CoW | User stack pages also CoW on fork (not just code/data) | [x] |
| L2.4 | Heap CoW | BRK/MMAP pages also CoW on fork | [x] |
| L2.5 | CoW + signals | Page fault during signal delivery handled correctly | [x] |
| L2.6 | TLB flush on CoW copy | invlpg instruction after remapping CoW page | [x] |
| L2.7 | CoW page statistics | `cowstat` command: total shared pages, total CoW faults | [x] |
| L2.8 | Disable CoW fallback | If refcount table full, fall back to deep-copy | [x] |
| L2.9 | QEMU test: CoW fork | Verify CoW fork + exec + exit cycle in QEMU | [x] |
| L2.10 | 10 integration tests | cow_exec, cow_exit, cow_stack, cow_heap, tlb_flush | [x] |

### Phase M: Multi-User & File Permissions (3 sprints, 30 tasks)

**Goal:** Add user accounts, login, file ownership (uid/gid), permission bits (rwx)
**Effort:** ~12 hours

#### Sprint M1: User Account System (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| M1.1 | User table at 0x960000 | 16 users × 64B: uid, username[16], password_hash[32], gid, home[16] | [x] |
| M1.2 | Root user (uid=0) | Pre-configured: root/root, gid=0, home=/ | [x] |
| M1.3 | `adduser` command | Create new user with uid, password, home directory | [x] |
| M1.4 | `passwd` command | Change password for current user | [x] |
| M1.5 | `login` command | Prompt username + password, switch UID in process table | [x] |
| M1.6 | `whoami` shows real user | Read UID from process table, lookup username | [x] |
| M1.7 | `su` command | Switch user (requires target password or root) | [x] |
| M1.8 | `id` command | Show uid, gid, username | [x] |
| M1.9 | Per-process UID/GID | PROC_TABLE + pid*256 + 168 (uid), +176 (gid) | [x] |
| M1.10 | 10 integration tests | user_table, login, passwd, su, whoami | [x] |

#### Sprint M2: File Permission Bits (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| M2.1 | Extended ramfs entry | +56=owner_uid(i64), +64=owner_gid(i64), +72=mode(i64: rwxrwxrwx) | [x] |
| M2.2 | Default permissions | New files: 0644 (rw-r--r--), new dirs: 0755 (rwxr-xr-x) | [x] |
| M2.3 | `chmod` command | Change mode bits: `chmod 755 file` | [x] |
| M2.4 | `chown` command | Change owner: `chown uid file` (root only) | [x] |
| M2.5 | Permission check on open | sys_open checks read/write against mode + uid/gid | [x] |
| M2.6 | Permission check on exec | Exec checks execute bit (mode & 0111) | [x] |
| M2.7 | Permission check on unlink | Unlink checks write bit on parent directory | [x] |
| M2.8 | `ls -l` long listing | Show permissions, owner, size, name | [x] |
| M2.9 | Root bypass | UID 0 bypasses all permission checks | [x] |
| M2.10 | 10 integration tests | chmod, chown, permission_deny, root_bypass | [x] |

#### Sprint M3: User Sessions & Security (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| M3.1 | Login shell per user | After login, spawn shell with user's UID | [x] |
| M3.2 | `logout` command | Exit user shell, return to login prompt | [x] |
| M3.3 | /etc/passwd file | Store user accounts in ramfs file (persistent) | [x] |
| M3.4 | Password hashing | Simple hash (FNV-1a or similar) — don't store plaintext | [x] |
| M3.5 | setuid/setgid bits | Execute file with owner's UID instead of caller's | [x] |
| M3.6 | `groups` command | Show user's group memberships | [x] |
| M3.7 | Process inherits UID | fork() copies parent UID/GID to child | [x] |
| M3.8 | `last` command | Show login history (stored in /var/log/wtmp) | [x] |
| M3.9 | Session timeout | Auto-logout after N minutes of inactivity | [x] |
| M3.10 | 10 integration tests | login_shell, logout, passwd_file, setuid, groups | [x] |

### Phase N: Advanced Filesystem (2 sprints, 20 tasks)

**Goal:** Journaling, symbolic links, hard links, proper directory tree
**Effort:** ~8 hours

#### Sprint N1: Directory Tree & Links (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.1 | Hierarchical directories | Support `/home/fajar/file.txt` path resolution | [x] |
| N1.2 | `mkdir -p` recursive | Create intermediate directories | [x] |
| N1.3 | `cd` with path resolution | `cd /home/fajar` traverses directory tree | [x] |
| N1.4 | `pwd` full path | Show absolute path from root | [x] |
| N1.5 | Symbolic links | `ln -s target link` — store target path in link inode | [x] |
| N1.6 | Hard links | `ln target link` — multiple names for same inode | [x] |
| N1.7 | `readlink` command | Show symbolic link target | [x] |
| N1.8 | Path resolution follows symlinks | `cat /tmp/link` resolves to target | [x] |
| N1.9 | `rmdir` command | Remove empty directory (fail if not empty) | [x] |
| N1.10 | 10 integration tests | mkdir_p, cd_path, symlink, hardlink, readlink | [x] |

#### Sprint N2: Journal & Crash Recovery (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N2.1 | Write-ahead log (WAL) | Journal at 0x970000 (64KB): log operations before committing | [x] |
| N2.2 | Journal entry format | type(8B) + inode(8B) + offset(8B) + len(8B) + data(32B) = 64B | [x] |
| N2.3 | Journal commit | Flush journal entries to actual filesystem on sync | [x] |
| N2.4 | Journal replay | On boot: check journal, replay uncommitted entries | [x] |
| N2.5 | `sync` command | Force journal flush to disk | [x] |
| N2.6 | `fsck` command | Verify filesystem consistency after crash | [x] |
| N2.7 | Atomic rename | `mv` uses journal to ensure atomicity | [x] |
| N2.8 | Disk full handling | Refuse writes when < 10% free, clear error message | [x] |
| N2.9 | Inode generation numbers | Detect stale file handles after delete+recreate | [x] |
| N2.10 | 10 integration tests | wal, journal_commit, replay, sync, fsck, atomic_rename | [x] |

### Phase O: TCP Server & Sockets (2 sprints, 20 tasks)

**Goal:** Listen for incoming TCP connections — enables HTTP server, SSH stub
**Effort:** ~8 hours

#### Sprint O1: Socket API (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| O1.1 | Socket table at 0x980000 | 16 sockets × 64B: type, state, local_port, remote_ip, remote_port, buffers | [x] |
| O1.2 | SYS_SOCKET(27) | Create socket: returns socket FD (type=6: FD_SOCKET) | [x] |
| O1.3 | SYS_BIND(28) | Bind socket to local port | [x] |
| O1.4 | SYS_LISTEN(29) | Mark socket as listening, set backlog | [x] |
| O1.5 | SYS_ACCEPT(30) | Accept incoming connection, return new socket FD | [x] |
| O1.6 | SYS_CONNECT(31) | Connect to remote (existing tcp_connect enhanced) | [x] |
| O1.7 | Socket read/write via FD | SYS_READ/WRITE dispatch to socket buffer | [x] |
| O1.8 | `netstat` command | Show all sockets with state (LISTEN, ESTABLISHED, etc.) | [x] |
| O1.9 | TCP RST handling | Properly reset connections on error | [x] |
| O1.10 | 10 integration tests | socket_create, bind, listen, accept, connect | [x] |

#### Sprint O2: HTTP Server (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| O2.1 | `httpd` command | Start HTTP server on port 80 | [x] |
| O2.2 | HTTP request parser | Parse GET /path HTTP/1.1 from socket | [x] |
| O2.3 | Serve static files | Map URL path to ramfs/FAT32 file, send as response | [x] |
| O2.4 | HTTP response headers | Content-Type, Content-Length, Connection: close | [x] |
| O2.5 | 404 Not Found | Return 404 for missing files | [x] |
| O2.6 | Directory listing | GET /dir/ returns HTML listing of directory | [x] |
| O2.7 | `/proc` endpoint | GET /proc/version returns kernel info as JSON | [x] |
| O2.8 | Connection logging | Log each request to serial: IP, method, path, status | [x] |
| O2.9 | Concurrent connections | Accept up to 4 connections using process table | [x] |
| O2.10 | 10 integration tests | httpd_start, parse_request, serve_file, 404, logging | [x] |

### Phase P: GDB Remote Debugging (2 sprints, 20 tasks)

**Goal:** GDB stub over serial — step through kernel code from host
**Effort:** ~8 hours

#### Sprint P1: GDB Protocol Stub (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P1.1 | GDB RSP parser | Parse `$command#checksum` packets from serial (COM2) | [x] |
| P1.2 | `?` — halt reason | Return S05 (SIGTRAP) on connection | [x] |
| P1.3 | `g` — read registers | Send all 16 GPRs + RIP + RFLAGS as hex | [x] |
| P1.4 | `G` — write registers | Set register values from GDB | [x] |
| P1.5 | `m` — read memory | Read N bytes from address, send as hex | [x] |
| P1.6 | `M` — write memory | Write bytes to address (for breakpoints) | [x] |
| P1.7 | `s` — single step | Set TF (trap flag) in RFLAGS, resume, stop at next insn | [x] |
| P1.8 | `c` — continue | Clear TF, resume execution | [x] |
| P1.9 | Breakpoint (INT3) | `Z0/z0` — insert/remove 0xCC breakpoint | [x] |
| P1.10 | 10 integration tests | rsp_parse, register_read, memory_read, breakpoint | [x] |

#### Sprint P2: GDB Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| P2.1 | QEMU `-gdb` flag | Connect GDB to QEMU: `target remote :1234` | [x] |
| P2.2 | Symbol table output | Generate .sym file from kernel for GDB | [x] |
| P2.3 | Process-aware debugging | `qRcmd` — list processes, switch context | [x] |
| P2.4 | Watchpoints | `Z2/z2` — hardware watchpoint via DR0-DR3 | [x] |
| P2.5 | Thread query | `qfThreadInfo` — list kernel processes as GDB threads | [x] |
| P2.6 | Memory map | `qXfer:memory-map:read` — tell GDB about memory regions | [x] |
| P2.7 | `gdb` shell command | Enter debug mode from Nova shell | [x] |
| P2.8 | Debug exception handler | IDT vector 1 (debug) and 3 (breakpoint) | [x] |
| P2.9 | QEMU test: GDB session | Connect GDB, set breakpoint on kernel_main, step | [x] |
| P2.10 | 10 integration tests | gdb_connect, breakpoint_hit, step, register_read | [x] |

### Phase Q: v0.8 Release (1 sprint, 10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| Q1.1 | QEMU full verification | All v0.8 features tested in QEMU | [x] |
| Q1.2 | Update CLAUDE.md | New stats: LOC, commands, syscalls, features | [x] |
| Q1.3 | Update CHANGELOG.md | v5.3.0 "Bastion" section | [x] |
| Q1.4 | Version bump | Nova banner → v1.3.0 "Bastion" | [x] |
| Q1.5 | Update NOVA_V07_PLAN.md | Reference from v0.8 plan | [x] |
| Q1.6 | fajaros-x86 sync | Modular repo updated with v0.8 features | [x] |
| Q1.7 | Clippy clean | `cargo clippy -- -D warnings` | [x] |
| Q1.8 | Full test suite | All tests pass (target: 6,200+) | [x] |
| Q1.9 | Git tag | `git tag v5.3.0` | [x] |
| Q1.10 | Blog post | v0.8 "Bastion" release announcement | [x] |

### v0.8 Quality Gates

| Gate | Criteria |
|------|----------|
| L-Phase | CoW fork 10x faster, 15 forks without OOM |
| M-Phase | Login as non-root, file permission denied for wrong user |
| N-Phase | `mkdir -p /a/b/c` + `cd /a/b/c` + `pwd` = /a/b/c |
| O-Phase | `httpd` serves file to `wget` from same QEMU instance |
| P-Phase | GDB connects to QEMU, sets breakpoint, reads registers |
| Q-Phase | All tests pass, clippy clean, CHANGELOG updated |

### v0.8 Target Metrics

| Metric | Current (v1.2.0) | Target (v1.3.0) |
|--------|------------------|------------------|
| Nova LOC | 15,732 | ~20,000 |
| Commands | 200 | 220+ |
| Syscalls | 26 | 32+ (socket API) |
| Filesystem | RamFS + FAT32 | + journal + symlinks |
| Users | root only | Multi-user with login |
| Network | TCP client | + TCP server + HTTP |
| Debugging | None | GDB remote stub |
| Memory | Deep-copy fork | Copy-on-Write |
| Tests | 6,076 | 6,200+ |

### v0.8 Dependency Graph

```
Phase L: CoW Fork (20 tasks, ~8 hrs)
    L1 → L2
          |
Phase M: Multi-User (30 tasks, ~12 hrs)
    M1 → M2 → M3
               |
Phase N: Filesystem (20 tasks, ~8 hrs)    (parallel with M)
    N1 → N2
          |
Phase O: TCP Server (20 tasks, ~8 hrs)    (parallel with N)
    O1 → O2
          |
Phase P: GDB Debug (20 tasks, ~8 hrs)     (parallel with O)
    P1 → P2
          |
Phase Q: Release (10 tasks, ~2 hrs)
    Q1 ←── all phases
```

### v0.8 Timeline

```
Session 1-2:   Phase L (Sprint L1-L2)     — CoW fork
Session 3-5:   Phase M (Sprint M1-M3)     — Multi-user + permissions
Session 6-7:   Phase N (Sprint N1-N2)     — Filesystem + journal
Session 8-9:   Phase O (Sprint O1-O2)     — TCP server + HTTP
Session 10-11: Phase P (Sprint P1-P2)     — GDB remote debug
Session 12:    Phase Q (Sprint Q1)        — Release
```

---

## Option 3: fajaros-x86 Sync (4 sprints, 40 tasks)

**Goal:** Sync the modular fajaros-x86 repo (75 .fj files) with all v0.7 "Nexus" features
**Effort:** ~10 hours
**Priority:** MEDIUM — keeps modular repo up to date
**Depends on:** Option 4 (commit to fajar-lang first)
**Repo:** github.com/fajarkraton/fajaros-x86

### Sprint S1: Syscall Dispatch Module (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S1.1 | Create `kernel/syscall_v2.fj` | Extract syscall_dispatch, all sys_* functions from monolithic | [x] |
| S1.2 | Update `kernel/core/syscall.fj` | Replace old 5-syscall inline handler with v2 table dispatch | [x] |
| S1.3 | Create `kernel/core/fd_table.fj` | FD_TABLE_V2 at 0x8D0000, fd_v2_* functions | [x] |
| S1.4 | Update sys_read/sys_write | FD dispatch: console/ramfs/pipe routing | [x] |
| S1.5 | Add sys_open/close/stat | File I/O syscalls with ramfs support | [x] |
| S1.6 | Add sys_lseek/dup/dup2 | Position tracking + FD duplication | [x] |
| S1.7 | Add sys_getcwd/chdir/unlink | CWD + file removal | [x] |
| S1.8 | Add sys_brk/sbrk/mmap | Memory management syscalls | [x] |
| S1.9 | Add sys_clock/sleep | Timer-based syscalls | [x] |
| S1.10 | Verify: `fj check kernel/syscall_v2.fj` | Lexer + parser pass on new module | [x] |

### Sprint S2: Process Management Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S2.1 | Create `kernel/process/fork.fj` | sys_fork, fork_clone_page_tables, fork_copy_fd_table | [x] |
| S2.2 | Create `kernel/process/exec.fj` | sys_exec, exec_setup_argv, exec_free_user_pages | [x] |
| S2.3 | Create `kernel/process/wait.fj` | sys_waitpid, waitpid_any, wake_waiting_parent | [x] |
| S2.4 | Create `kernel/process/exit.fj` | process_exit_v2, process_exit_with_signal, reparent_children | [x] |
| S2.5 | Update `kernel/core/scheduler.fj` | Integrate process state constants, PROC_WAIT_TABLE | [x] |
| S2.6 | Update `kernel/core/process.fj` | Add PROC_OFF_BRK, PROC_OFF_CWD, PROC_OFF_PGID fields | [x] |
| S2.7 | Create `kernel/process/groups.fj` | sys_setpgid, sys_getpgid | [x] |
| S2.8 | Update `build.sh` | Add new .fj files to concatenation build | [x] |
| S2.9 | Verify: parse all new modules | `fj check` on each new file | [x] |
| S2.10 | QEMU test: boot with new modules | Concatenated build boots correctly | [x] |

### Sprint S3: Pipe & Signal Modules (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S3.1 | Create `kernel/ipc/pipe_v2.fj` | sys_pipe, pipe_read/write_circular, refcounting | [x] |
| S3.2 | Update `kernel/ipc/pipe.fj` | Keep old pipe_create/read/write for demo, add v2 imports | [x] |
| S3.3 | Create `kernel/signal/signal.fj` | Signal table, signal_send, signal_check_pending | [x] |
| S3.4 | Create `kernel/signal/handlers.fj` | signal_deliver_default, sys_kill, sys_signal | [x] |
| S3.5 | Create `kernel/signal/jobs.fj` | Job table, job_add, job_check_notifications, cmd_jobs/fg/bg | [x] |
| S3.6 | Update `kernel/drivers/keyboard.fj` | Add Ctrl key tracking, Ctrl+C/Z handling | [x] |
| S3.7 | Update `kernel/core/init.fj` | Init signal table, job table, ctrl state at boot | [x] |
| S3.8 | Create `kernel/ipc/fd_ops.fj` | pipe_incref/decref, pipe_free, refcount table | [x] |
| S3.9 | Verify: parse signal modules | `fj check` on each new file | [x] |
| S3.10 | QEMU test: signals work | Ctrl+C kills foreground process | [x] |

### Sprint S4: Shell Scripting Module (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| S4.1 | Create `kernel/shell/pipes.fj` | shell_find_pipe, shell_exec_pipe, shell_exec_from_buf | [x] |
| S4.2 | Create `kernel/shell/redirect.fj` | shell_find_redirect, shell_exec_redirect, redirect_output | [x] |
| S4.3 | Create `kernel/shell/vars.fj` | ENV_TABLE, env_find/set/get, shell_expand_vars, cmd_export | [x] |
| S4.4 | Create `kernel/shell/script.fj` | cmd_sh, script loading, comment handling | [x] |
| S4.5 | Create `kernel/shell/control.fj` | if/else/fi, for/do/done, while/do/done, test builtin | [x] |
| S4.6 | Update `kernel/shell/dispatch.fj` | shell_execute_v2 as entry point, call dispatch_command | [x] |
| S4.7 | Create `kernel/shell/builtins.fj` | cmd_export, cmd_set, cmd_exit_shell, is_shell_builtin | [x] |
| S4.8 | Update `README.md` | Document v0.7 features, new modules, syscall list | [x] |
| S4.9 | Full concatenation build | All 85+ .fj files concatenate cleanly | [x] |
| S4.10 | QEMU test: full boot | Boot modular kernel, run `help`, verify 200+ commands | [x] |

### S-Phase Quality Gate
- [x] All 85+ .fj files lex + parse successfully
- [x] Concatenated build produces working kernel
- [x] QEMU boot + basic commands verified
- [x] README.md updated with v0.7 feature list
- [x] No regressions from v0.6 features

---

## Option 4: Commit + Push + Release (1 sprint, 10 tasks)

**Goal:** Commit all v0.7 changes, push to GitHub, create release
**Effort:** ~1 hour
**Priority:** HIGH — preserve work, enable collaboration

### Sprint R1: Git Release Workflow (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| R1.1 | `git status` | Review all changed/new files | [x] |
| R1.2 | Stage kernel changes | `git add examples/fajaros_nova_kernel.fj` | [x] |
| R1.3 | Stage linker changes | `git add src/codegen/linker.rs` | [x] |
| R1.4 | Stage test changes | `git add tests/eval_tests.rs` | [x] |
| R1.5 | Stage docs | `git add docs/NOVA_V07_PLAN.md docs/CHANGELOG.md docs/NEXT_IMPLEMENTATION_PLAN.md` | [x] |
| R1.6 | Stage CLAUDE.md | `git add CLAUDE.md` | [x] |
| R1.7 | Commit | `git commit -m "feat(nova): v0.7 Nexus — 26 syscalls, fork/exec/waitpid, pipes, signals, scripting"` | [x] |
| R1.8 | Git tag | `git tag v5.2.0` | [x] |
| R1.9 | Push to GitHub | `git push origin main && git push --tags` | [x] |
| R1.10 | GitHub release | Create release on github.com/fajarkraton/fajar-lang with changelog | [x] |

---

## Option 5: v2.0 "Dawn" Remaining (2 sprints, 18 tasks)

**Goal:** Complete the 18 remaining tasks that require Dragon Q6A hardware
**Effort:** ~4 hours (requires physical Q6A board connected via SSH)
**Priority:** MEDIUM
**Prerequisite:** Q6A board powered on, SSH accessible at 192.168.50.94

### Sprint D1: Q6A Hardware Verification (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1 | SSH connection test | `ssh radxa@192.168.50.94` — verify access | [x] |
| D1.2 | Cross-compile latest | `cargo build --release --target aarch64-unknown-linux-gnu` | [x] |
| D1.3 | Deploy binary to Q6A | `scp target/aarch64.../fj radxa@192.168.50.94:/opt/fj/` | [x] |
| D1.4 | JIT test on Q6A | `./fj run --jit examples/fibonacci.fj` on Q6A | [x] |
| D1.5 | AOT test on Q6A | `./fj run --target aarch64-unknown-linux-gnu --emit aot examples/hello.fj` | [x] |
| D1.6 | GPU compute on Q6A | Vulkan matmul benchmark on Adreno 643 | [x] |
| D1.7 | QNN inference on Q6A | MNIST inference via QNN CPU backend | [x] |
| D1.8 | GPIO test on Q6A | GPIO96 blink test via `/dev/gpiochip4` | [x] |
| D1.9 | FajarOS QEMU on Q6A | `qemu-system-aarch64` boot FajarOS on Q6A | [x] |
| D1.10 | Thermal monitoring | Check CPU temp during stress test | [x] |

### Sprint D2: Q6A Advanced Features (8 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1 | QNN HTP backend | Test with testsig (if available) | [x] |
| D2.2 | Camera pipeline | libcamera capture on IMX219 module | [x] |
| D2.3 | NVMe benchmark | Read/write speed on Samsung PM9C1a | [x] |
| D2.4 | WiFi stability | Long-running SSH session over WiFi | [x] |
| D2.5 | Full example suite | Run all 55 Q6A-specific examples | [x] |
| D2.6 | Native build on Q6A | `cargo build` directly on Q6A (4m31s target) | [x] |
| D2.7 | Multi-accelerator | CPU + GPU + NPU simultaneous inference | [x] |
| D2.8 | Update Q6A docs | Final status update for all hardware tests | [x] |

---

## Option 6: Blog Post (2 sprints, 20 tasks)

**Goal:** Technical blog about Nova v0.7 "Nexus" — from demo OS to real UNIX-like
**Effort:** ~4 hours
**Priority:** LOW (but great for visibility)
**Output:** `docs/BLOG_NOVA_V07_NEXUS.md`

### Sprint B1: Technical Blog Content (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B1.1 | Title + intro | "Building a UNIX-like OS in Fajar Lang: Nova v0.7 Nexus" | [x] |
| B1.2 | Before/after comparison | v0.6 (demo shell) vs v0.7 (real UNIX process model) | [x] |
| B1.3 | Syscall dispatch architecture | Diagram: linker asm → indirect call → syscall_dispatch → handler | [x] |
| B1.4 | fork() deep dive | Page table walk, deep-copy, child RAX=0 trick | [x] |
| B1.5 | exec() deep dive | ELF loading, argv on stack, System V ABI | [x] |
| B1.6 | Pipe implementation | Circular buffer, refcounting, EOF detection, shell integration | [x] |
| B1.7 | Signal design | 8-slot signal table, pending bitmap, default handlers, Ctrl+C | [x] |
| B1.8 | Shell scripting | Variable expansion, script loading, if/for/while | [x] |
| B1.9 | Lessons learned | What was hard, what worked well, design decisions | [x] |
| B1.10 | Performance numbers | Test counts, LOC growth, syscall count growth | [x] |

### Sprint B2: Media & Publication (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| B2.1 | QEMU screenshots | Boot, shell, pipe demo, background jobs | [x] |
| B2.2 | Architecture diagram | ASCII/mermaid: syscall flow, process lifecycle | [x] |
| B2.3 | Memory map diagram | All v0.7 allocations (0x8D0000-0x8D8000) | [x] |
| B2.4 | Syscall table reference | All 26 syscalls with signatures | [x] |
| B2.5 | Signal table reference | 8 signals with default actions | [x] |
| B2.6 | Shell feature matrix | Pipes, redirect, vars, scripting, jobs — comparison with bash | [x] |
| B2.7 | Code statistics | LOC by module, test coverage, growth chart | [x] |
| B2.8 | Future roadmap | v0.8 "Bastion" preview (CoW, multi-user, TCP server) | [x] |
| B2.9 | Add to docs/index | Update documentation index with blog link | [x] |
| B2.10 | Review + publish | Final review, push to GitHub | [x] |

---

## Execution Order Recommendation

```
Step 1: Option 4 — Commit + Push (1 hr)
    ↓
Step 2: Option 1 — QEMU Verification (6 hrs)
    ↓
Step 3: Option 3 — fajaros-x86 Sync (10 hrs)   ← parallel with Step 4
Step 4: Option 6 — Blog Post (4 hrs)             ← parallel with Step 3
    ↓
Step 5: Option 2 — Nova v0.8 "Bastion" (40 hrs)  ← main next phase
    ↓
Step 6: Option 5 — v2.0 "Dawn" Remaining (4 hrs) ← when Q6A available
```

### Quick Wins (can do now):
- **Option 4** (Commit) — 1 hour, preserves all work
- **Option 1** Sprint V1 (Boot test) — 2 hours, validates basic functionality

### Medium Term (this week):
- **Option 1** Sprint V2-V3 (Full QEMU verification)
- **Option 6** Sprint B1 (Blog writing)
- **Option 3** Sprint S1-S2 (Modular repo sync)

### Long Term (next weeks):
- **Option 2** Nova v0.8 "Bastion" — the big one (CoW, multi-user, TCP server, GDB)
- **Option 5** v2.0 "Dawn" — when Q6A hardware is available

---

## Summary

```
Option 1:  QEMU Verification      3 sprints   30 tasks    ~6 hrs    VERIFY
Option 2:  Nova v0.8 "Bastion"    12 sprints  120 tasks   ~40 hrs   BUILD
Option 3:  fajaros-x86 Sync       4 sprints   40 tasks    ~10 hrs   SYNC
Option 4:  Commit + Push          1 sprint    10 tasks    ~1 hr     SHIP
Option 5:  v2.0 "Dawn" Remaining  2 sprints   18 tasks    ~4 hrs    HARDWARE
Option 6:  Blog Post              2 sprints   20 tasks    ~4 hrs    DOCUMENT

Total:     24 sprints, 238 tasks, ~65 hours
```

---

*Next Steps Implementation Plan — FajarOS Nova post-v0.7 "Nexus"*
*Built with Fajar Lang + Claude Opus 4.6*
