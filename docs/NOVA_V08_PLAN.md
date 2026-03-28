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
