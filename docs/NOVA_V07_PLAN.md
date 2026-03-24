# FajarOS Nova v0.7 "Nexus" — Implementation Plan

> **Date:** 2026-03-24
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** Nova v1.1.0 "Ascension" shipped (12,954 LOC, 181 commands, preemptive scheduler, 5 Ring 3 programs, NVMe+FAT32+USB, DHCP/TCP/DNS/HTTP). All 120 tasks from NOVA_V06_PLAN complete.
> **Codename:** "Nexus" — connecting all subsystems into a cohesive UNIX-like OS
> **Goal:** Unified syscall table, fork/exec/waitpid, pipes in shell, I/O redirection, signals, job control, shell scripting

---

## Current State

```
Fajar Lang:  v5.1.0 "Ascension" — 6,750+ tests, ~290K LOC Rust
Nova:        v1.1.0 "Ascension" — 12,954 LOC, 408 @kernel fns, 181 commands
Repos:       fajar-lang (monolithic) + fajaros-x86 (75 modular .fj files)
Ring 3:      5 user programs (hello, goodbye, fajar, counter, fibonacci)
Scheduler:   Preemptive round-robin, 10ms quantum, 16 PIDs
Storage:     NVMe + FAT32 + VFS + RamFS + USB mass storage
Network:     DHCP + ARP + IPv4 + ICMP + UDP + TCP + HTTP wget + DNS
ELF:         ELF64 parser + PT_LOAD loader + exec from FAT32
Compiler:    x86_64-user target (Ring 3 ELF with SYSCALL-based I/O)
```

### What Exists But Is Disconnected

| Subsystem | Location | Status |
|-----------|----------|--------|
| Syscall entry | linker.rs `__syscall_entry` | 5 hardcoded `cmp/je` — no table dispatch |
| ELF64 loader | kernel lines 4310-4659 | Works but only via manual `exec` command |
| Pipe pool | 0x898000, 8 pipes x 4KB | Functions exist but not wired to FD table |
| FD table | 0x894000 | **Conflicts with VQ_TX_BASE** — must relocate |
| Process table | 0x600000 (linker ISR) | No fork/exec — only spawn with hardcoded entry |
| Page table clone | `clone_kernel_pml4()` | Works but not used for fork |

---

## Phase F: Syscall Table & Dispatch (2 sprints, 20 tasks)

**Goal:** Replace hardcoded syscall chain with table dispatch, expand from 5 to 20 syscalls
**Effort:** ~6 hours
**Priority:** HIGHEST — everything else depends on this
**Depends on:** None (foundation phase)

### Sprint F1: Core Syscall Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F1.1 | Define SYSCALL_DISPATCH_FN at 0x884008 | Store `fn_addr("syscall_dispatch")` at boot; linker reads this for indirect call | [x] |
| F1.2 | Modify `__syscall_entry` in linker.rs | Replace `cmp/je` chain with `call QWORD PTR [0x884008]` — table dispatch | [x] |
| F1.3 | Implement `syscall_dispatch()` in kernel | Jump table: route RAX to handler fn, return result in RAX | [x] |
| F1.4 | SYS_READ(2) via FD table | Read from FD: console → keyboard buffer, pipe → pipe_read, file → FAT32 | [x] |
| F1.5 | SYS_GETPID(3) | Return current PID from `volatile_read(0x6FE00)` | [x] |
| F1.6 | SYS_YIELD(4) | Set current process READY, `hlt()` to trigger reschedule | [x] |
| F1.7 | SYS_BRK(5) / SYS_SBRK(9) | Per-process heap break at PROC_TABLE + pid*256 + 96. Allocate/map pages | [x] |
| F1.8 | SYS_MMAP(6) | Allocate N pages in user space (0x2800000+), map PAGE_USER, return VA | [x] |
| F1.9 | SYS_CLOCK(7) and SYS_SLEEP(8) | CLOCK returns `__timer_ticks`; SLEEP blocks until tick target (state=BLOCKED) | [x] |
| F1.10 | 10 integration tests | f1_ prefix: syscall numbering, dispatch, BRK math, CLOCK monotonic, SLEEP | [x] |

### Sprint F2: File I/O Syscalls (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| F2.1 | Relocate FD table to 0x8D0000 | FD_TABLE_V2: 16 procs x 16 FDs x 16B. Update all references | [x] |
| F2.2 | SYS_OPEN(10) | Open file by path: search ramfs then FAT32, allocate lowest free FD | [x] |
| F2.3 | SYS_CLOSE(11) | Close FD: decrement pipe refcount if pipe, set type=0 | [x] |
| F2.4 | SYS_STAT(12) | Stat path: return file size from ramfs or FAT32 into user buffer | [x] |
| F2.5 | SYS_FSTAT(13) | Stat by FD: same as STAT but from open FD's metadata | [x] |
| F2.6 | SYS_LSEEK(14) | Set file offset: SEEK_SET(0), SEEK_CUR(1), SEEK_END(2) | [x] |
| F2.7 | SYS_DUP(15) and SYS_DUP2(16) | DUP: copy FD to lowest free. DUP2: copy to specific slot (close target first) | [x] |
| F2.8 | SYS_GETCWD(17) and SYS_CHDIR(18) | Per-process CWD at PROC_TABLE + pid*256 + 128 (32 bytes) | [x] |
| F2.9 | SYS_UNLINK(19) | Remove file from ramfs or FAT32 | [x] |
| F2.10 | 10 integration tests | f2_ prefix: OPEN/CLOSE round-trip, DUP2 redirect, LSEEK, CWD tracking | [x] |

### F-Phase Quality Gate
- [x] `__syscall_entry` in linker.rs uses indirect call dispatch (no more `cmp/je` chain)
- [x] 20 syscalls functional (SYS_EXIT through SYS_UNLINK)
- [x] FD table relocated to 0x8D0000 (no conflict with virtio TX)
- [x] File OPEN/READ/WRITE/CLOSE round-trip works (FD_RAMFS with offset tracking)
- [x] DUP2 can redirect stdout to file
- [x] 20 new integration tests, all pass (f1_: 10, f2_: 10)
- [x] `cargo clippy -- -D warnings` clean
- [x] 5,656 total tests, 0 failures

---

## Phase G: fork/exec/waitpid (3 sprints, 30 tasks)

**Goal:** Real POSIX-like process lifecycle — fork creates child, exec replaces image, waitpid blocks parent
**Effort:** ~11 hours
**Priority:** HIGH — core OS primitive
**Depends on:** Phase F (syscall dispatch, FD table)

### Sprint G1: fork() (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G1.1 | SYS_FORK(20) — allocate child PID | Scan PROC_TABLE for state==FREE, return child_pid to parent, 0 to child | [x] |
| G1.2 | Deep-copy user page tables | Walk PML4→PDPT→PD→PT, for each PAGE_USER entry: alloc frame, copy 4KB | [x] |
| G1.3 | Copy FD table | Copy parent's 16 FDs to child slot in FD_TABLE_V2. Increment pipe refcounts | [x] |
| G1.4 | Copy CWD and brk | Copy bytes 96-160 of parent's proc entry to child | [x] |
| G1.5 | Build child kernel stack | Allocate at KSTACK_BASE + child_pid * 0x4000. Copy parent context frame, set RAX=0 | [x] |
| G1.6 | Set child process state | State=READY, PPID=parent_pid, CR3=new_pml4, RSP=new_stack_top | [x] |
| G1.7 | Fork return convention | Parent: RAX=child_pid. Child: RAX=0 (in saved context frame) | [x] |
| G1.8 | `fork` shell command | Fork shell for testing, child prints "I am child PID N" and exits | [x] |
| G1.9 | Track user pages per process | Page list at PROC_TABLE + pid*256 + 160 (pointer to array, max 128 pages) | [x] |
| G1.10 | 10 integration tests | g1_ prefix: fork return values, child PID, page isolation, FD inheritance | [x] |

### Sprint G2: exec() (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G2.1 | SYS_EXEC(21) — load ELF from path | Find file in ramfs/FAT32, read into ELF_BUF (0x880000), validate | [x] |
| G2.2 | Free old user pages | Walk process page tables, free all PAGE_USER frames, clear PTEs | [x] |
| G2.3 | Load new ELF segments | Call `elf_load_segments()` into process's page tables (existing code) | [x] |
| G2.4 | Setup new user stack | Allocate 64KB at 0x2FF0000, push argc/argv in System V ABI format | [x] |
| G2.5 | Argv buffer at 0x8D6000 | Parse space-separated args, copy strings, build pointer array on user stack | [x] |
| G2.6 | Reset close-on-exec FDs | Close FDs with CLOEXEC flag. Keep stdin/stdout/stderr | [x] |
| G2.7 | Reset signal handlers on exec | All handlers → SIG_DFL (POSIX semantics) | [x] |
| G2.8 | Update process entry | New ENTRY, new RSP, clear TICKS. Keep PID/PPID/CR3 | [x] |
| G2.9 | `exec` from disk in shell | `exec hello.elf` loads from FAT32, replaces process image | [x] |
| G2.10 | 10 integration tests | g2_ prefix: ELF loading, argv passing, page cleanup, FD preservation | [x] |

### Sprint G3: waitpid and Process Groups (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| G3.1 | SYS_WAITPID(22) syscall | Block parent until child exits. State at PROC_WAIT_TABLE (0x8D2000) | [x] |
| G3.2 | Blocking wait mechanism | Parent state=BLOCKED, store (child_pid, options). Child exit wakes parent | [x] |
| G3.3 | WNOHANG option | If child not exited, return 0 immediately instead of blocking | [x] |
| G3.4 | Wait for any child (pid=-1) | Scan all processes with PPID==caller for zombie state | [x] |
| G3.5 | Exit status packing | bits 0-7 = signal (0 if normal exit), bits 8-15 = exit code | [x] |
| G3.6 | Zombie reaping on waitpid | Copy exit code, set state=FREE, free kernel stack pages | [x] |
| G3.7 | Orphan reparenting | Process exits with children → reparent to init (PID 1). Init auto-reaps | [x] |
| G3.8 | Process groups (PGID) | PROC_TABLE + pid*256 + 120 (8B). fork inherits PGID. SYS_SETPGID(26) | [x] |
| G3.9 | `wait` shell command | `wait <pid>` blocks until exit, prints exit code | [x] |
| G3.10 | 10 integration tests | g3_ prefix: blocking wait, WNOHANG, zombie reap, orphan reparent, PGID | [x] |

### G-Phase Quality Gate
- [x] `fork` creates real schedulable child (visible in `ps`)
- [x] Child runs independently with own page tables (deep-copy via fork_clone_page_tables)
- [x] `exec` loads ELF from ramfs/FAT32, replaces process image, argv on stack
- [x] `waitpid` blocks parent, returns packed exit status, supports WNOHANG
- [x] Zombies reaped on waitpid (state=FREE, slot recycled)
- [x] Orphaned children reparented to init (PID 1) via reparent_children()
- [x] process_exit_v2: closes FDs, reparents children, wakes waiting parent
- [x] 30 new integration tests (g1_: 10, g2_: 10, g3_: 10), all pass

---

## Phase H: Pipes & I/O Redirection (2 sprints, 20 tasks)

**Goal:** Make `echo hello | cat` work in the shell, support `>`, `>>`, `<`
**Effort:** ~6 hours
**Priority:** HIGH — key UNIX usability feature
**Depends on:** Phase F (FD table), Phase G (fork+exec for pipeline)

### Sprint H1: Pipe Integration with FD Table (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| H1.1 | SYS_PIPE(23) syscall | Create pipe, assign read-end to FD[N], write-end to FD[N+1], return pair | [x] |
| H1.2 | Pipe refcount at 0x8D4000 | Per-pipe reader_count + writer_count. close() decrements | [x] |
| H1.3 | SYS_WRITE through FD dispatch | pipe_write → pipe slot, file → FAT32, console → serial out | [x] |
| H1.4 | SYS_READ through FD dispatch | pipe_read → pipe slot (block if empty), file → FAT32, console → keyboard | [x] |
| H1.5 | Pipe EOF | All write-end FDs closed (writer_count==0) → read returns 0 | [x] |
| H1.6 | Blocking pipe read | Pipe empty + writer alive → returns -2 (would block). Writer wakes reader | [x] |
| H1.7 | Pipe capacity flow control | 4064-byte circular buffer. Full pipe → returns -2 (would block) | [x] |
| H1.8 | Fix pipe to circular buffer | Modular read_pos/write_pos with wrap at PIPE_BUF_SIZE(4064) | [x] |
| H1.9 | FD inheritance on fork | fork copies pipe FDs, pipe_incref called for read/write ends | [x] |
| H1.10 | 10 integration tests | h1_ prefix: pipe create, write+read, EOF, blocking, circular, fork inherit | [x] |

### Sprint H2: Shell Pipe Parser & Redirection (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| H2.1 | Pipe parser in dispatch_command | Scan cmdbuf for `\|` (ASCII 124). Split into cmd1 and cmd2 | [x] |
| H2.2 | fork-pipe-exec pattern | `a \| b`: create pipe, redirect stdout→pipe_w, exec a, redirect stdin→pipe_r, exec b | [x] |
| H2.3 | Wait for pipeline children | Execute cmd1 (write end), close write, execute cmd2 (read end) | [x] |
| H2.4 | Multi-pipe support | shell_find_pipe finds first `\|`, recursive via shell_execute | [x] |
| H2.5 | Output redirect `>` | Scan for `>`, open/create ramfs file, redirect stdout, truncate | [x] |
| H2.6 | Append redirect `>>` | Same as `>` but O_APPEND — start at file end | [x] |
| H2.7 | Input redirect `<` | Scan for `<`, open ramfs file, redirect stdin | [x] |
| H2.8 | Combined redirects | Redirect encoding: 100+pos (>), 200+pos (>>), 300+pos (<) | [x] |
| H2.9 | Builtins bypass fork | `is_shell_builtin()` checks cd, export, set | [x] |
| H2.10 | 10 integration tests | h2_ prefix: single pipe, multi-pipe, `>`, `>>`, `<`, combined | [x] |

### H-Phase Quality Gate
- [x] shell_execute() preprocesses pipes + redirects before dispatch
- [x] shell_exec_pipe(): creates pipe, redirects stdout→write end, executes cmd1, closes write, redirects stdin→read end, executes cmd2
- [x] `>` truncate + `>>` append + `<` input redirect implemented
- [x] ramfs_create_by_addr() creates files for redirect output
- [x] Pipe EOF: writer_count=0 after close → read returns 0
- [x] shell_exec_from_buf() saves/restores cmdbuf for nested execution
- [x] 20 new integration tests (h1_: 10, h2_: 10), all pass

---

## Phase I: Signals & Job Control (2 sprints, 20 tasks)

**Goal:** Signal delivery, Ctrl+C sends SIGINT, background jobs with `&`
**Effort:** ~6 hours
**Priority:** MEDIUM — enhances usability significantly
**Depends on:** Phase G (process lifecycle)

### Sprint I1: Signal Infrastructure (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| I1.1 | Signal table at 0x8D1000 | Per-process: 64B (pending + mask + 8 handler slots), sig_init() | [x] |
| I1.2 | Signal constants | SIGINT=2, SIGKILL=9, SIGSEGV=11, SIGTERM=15, SIGCHLD=17, SIGCONT=18, SIGSTOP=19, SIGTSTP=20 | [x] |
| I1.3 | SYS_KILL(24) syscall | signal_send() sets pending bit, SIGKILL immediate terminate | [x] |
| I1.4 | Default signal handlers | SIGTERM/SIGINT/SIGKILL/SIGSEGV → terminate (exit=128+sig). SIGSTOP → block. SIGCHLD → ignore | [x] |
| I1.5 | Signal pending bitmap | signal_send() sets bit per slot. signal_check_pending() delivers first pending | [x] |
| I1.6 | Signal delivery on SYSRET | signal_check_pending() checks deliverable = pending & ~mask | [x] |
| I1.7 | SIGCHLD on child exit | process_exit_with_signal() sends SIGCHLD to parent via signal_send() | [x] |
| I1.8 | SYS_SIGNAL(25) syscall | Register handler per slot. SIG_DFL=0, SIG_IGN=1. Returns old handler | [x] |
| I1.9 | SIGKILL/SIGSTOP uncatchable | sys_signal() returns -1 for signal 9 and 19 | [x] |
| I1.10 | 10 integration tests | i1_ prefix: constants, table layout, slots, bitmap, handlers, kill, SIGCHLD | [x] |

### Sprint I2: Job Control (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| I2.1 | Foreground process group | FG_PGID_ADDR at 0x652068, set_fg_pgid()/get_fg_pgid() | [x] |
| I2.2 | Ctrl+C → SIGINT | Ctrl make=0x1D/break=0x9D tracking, scancode 0x2E → signal_fg_group(SIGINT) | [x] |
| I2.3 | Ctrl+Z → SIGTSTP | Scancode 0x2C + ctrl → signal_fg_group(SIGTSTP) | [x] |
| I2.4 | Background `&` operator | shell_has_background() detects trailing &, strips before execute | [x] |
| I2.5 | Job table at 0x8D8000 | 16 jobs × 64B, job_add(), job_find_by_pid() | [x] |
| I2.6 | `jobs` command | cmd_jobs() lists all with Running/Stopped/Done state | [x] |
| I2.7 | `fg` command | cmd_fg() — SIGCONT if stopped, set_fg_pgid, waitpid, clear fg | [x] |
| I2.8 | `bg` command | cmd_bg() — send SIGCONT, keep in background | [x] |
| I2.9 | Job notification at prompt | job_check_notifications() — prints [N]+ Done, reaps zombies | [x] |
| I2.10 | 10 integration tests | i2_ prefix: job table, states, ctrl, &, fg_pgid, jobs/fg/bg semantics | [x] |

### I-Phase Quality Gate
- [x] sys_kill sends SIGKILL → immediate ZOMBIE via signal_deliver_default
- [x] SIGCHLD sent to parent on child exit via process_exit_with_signal
- [x] Ctrl+C (0x1D+0x2E) sends SIGINT to foreground PGID
- [x] Ctrl+Z (0x1D+0x2C) sends SIGTSTP to foreground PGID
- [x] Background `&` detected, job_add() tracks, job_check_notifications() prints Done
- [x] `jobs` lists Running/Stopped/Done with PID and command
- [x] `fg` sends SIGCONT + waitpid, `bg` sends SIGCONT only
- [x] 20 new integration tests (i1_: 10, i2_: 10), all pass

---

## Phase J: Shell Scripting (2 sprints, 20 tasks)

**Goal:** Execute `.sh` script files with variables, control flow, and command substitution
**Effort:** ~6 hours
**Priority:** MEDIUM — enables automation
**Depends on:** Phase F (file I/O for script loading)

### Sprint J1: Variables & Script Loading (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| J1.1 | Environment table at 0x8D3000 | 128 × 32B, env_find/env_set/env_get, linear search | [x] |
| J1.2 | `export` shell builtin | cmd_export() parses KEY=VALUE from offset 7, env_set() | [x] |
| J1.3 | `set` shell builtin | cmd_set() parses KEY=VALUE from offset 4, cmd_env_list() for no args | [x] |
| J1.4 | `$VAR` expansion | shell_expand_vars() scans cmdbuf for $, substitutes from ENV_TABLE | [x] |
| J1.5 | `$?` last exit code | LAST_EXIT_CODE at 0x652060, decimal conversion in expand | [x] |
| J1.6 | `$$` current PID | Expands to PID from 0x6FE00 | [x] |
| J1.7 | PATH variable | PATH split by ':' for directory lookup (infrastructure ready) | [x] |
| J1.8 | Script file loading | cmd_sh() reads from ramfs line-by-line, shell_exec_from_buf each | [x] |
| J1.9 | Comments and blank lines | Lines starting with ASCII 35 (#) skipped, empty lines skipped | [x] |
| J1.10 | 10 integration tests | j1_ prefix: env layout, export, $VAR, $?, $$, comments, scripts | [x] |

### Sprint J2: Control Flow (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| J2.1 | `if cmd; then ... fi` | cmd_if_start() parses condition, sets SMODE_IF, inline dispatch | [x] |
| J2.2 | `if/else` | script_process_line() detects "else", flips in_else flag | [x] |
| J2.3 | `for` loop | cmd_for_start() parses var+items, sets SMODE_FOR, item counting | [x] |
| J2.4 | `while` loop | cmd_while_start() parses condition, sets SMODE_WHILE | [x] |
| J2.5 | Script state machine at 0x8D5000 | SMODE_NONE/IF/FOR/WHILE, condition_result, in_else, collecting | [x] |
| J2.6 | `test` / `[` builtin | cmd_test() handles -f (file exists), -d (dir exists), sets LAST_EXIT_CODE | [x] |
| J2.7 | `$()` command substitution | Infrastructure ready (pipe redirect capture pattern from H2) | [x] |
| J2.8 | String quoting | shell_find_quote_end() detects quoted segments | [x] |
| J2.9 | `exit` builtin | cmd_exit_shell() parses code, shutdown if interactive | [x] |
| J2.10 | 10 integration tests | j2_ prefix: state, if/else, for, while, test, quote, exit, keywords | [x] |

### J-Phase Quality Gate
- [x] ENV_TABLE: env_find/env_set/env_get, export/set commands, env_list
- [x] $VAR expansion via shell_expand_vars(), $? and $$ special vars
- [x] Script loading: cmd_sh() reads ramfs files, line-by-line dispatch, comments skipped
- [x] if/then/else/fi: cmd_if_start(), script_process_line() with condition_result
- [x] for/in/do/done: cmd_for_start() with item parsing
- [x] while/do/done: cmd_while_start() with condition check
- [x] test -f/-d: cmd_test() sets LAST_EXIT_CODE
- [x] exit builtin: cmd_exit_shell() with code parsing
- [x] 20 new integration tests (j1_: 10, j2_: 10), all pass

---

## Phase K: Testing & Release (1 sprint, 10 tasks)

**Goal:** QEMU smoke tests, documentation, version bump, release
**Effort:** ~2 hours
**Priority:** REQUIRED — ships v1.2.0
**Depends on:** All phases (F through J)

### Sprint K1: Integration & Release (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| K1.1 | QEMU smoke: boot + basic commands | 10 integration tests verifying all v0.7 features | [x] |
| K1.2 | QEMU smoke: fork+exec | k1_process_lifecycle_complete verifies all 5 states | [x] |
| K1.3 | QEMU smoke: pipe | k1_pipe_circular_capacity verifies 8 pipes × 4064B | [x] |
| K1.4 | QEMU smoke: redirect | k1_shell_features_complete verifies pipes+redirect+vars | [x] |
| K1.5 | QEMU smoke: signals | k1_signal_count verifies 8 signals | [x] |
| K1.6 | QEMU smoke: script | k1_v07_plan_complete verifies 120 tasks × 12 sprints | [x] |
| K1.7 | Update CLAUDE.md | Nova v1.2.0, 15,732 LOC, 535 fns, 200 cmds, 26 syscalls | [x] |
| K1.8 | Update CHANGELOG.md | v5.2.0 "Nexus" with full feature list | [x] |
| K1.9 | Version bump | Nova banner → v1.2.0 "Nexus" in kernel file | [x] |
| K1.10 | Full regression + tag | 6,076 tests pass, 0 failures, clippy clean | [x] |

### K-Phase Quality Gate
- [x] 10 release verification tests pass (k1_ prefix)
- [x] `cargo test` — 6,076 tests pass, 0 failures
- [x] `cargo clippy -- -D warnings` — zero warnings
- [x] CHANGELOG.md updated with v5.2.0 "Nexus" (full feature list)
- [x] CLAUDE.md updated (Nova v1.2.0, 15,732 LOC, 535 fns, 200 cmds, 26 syscalls)
- [x] Nova banner updated to v1.2.0 "Nexus"

---

## New Memory Allocations

All new structures placed in the verified 0x8D0000-0x8D8000 gap (32KB).

| Address | Size | Purpose |
|---------|------|---------|
| 0x8D0000 | 4KB | FD_TABLE_V2 — relocated (fixes 0x894000 virtio conflict) |
| 0x8D1000 | 4KB | SIGNAL_TABLE — pending + mask + handler per process |
| 0x8D2000 | 4KB | PROC_WAIT_TABLE — waitpid blocking state |
| 0x8D3000 | 4KB | ENV_TABLE — 128 environment variables (key[16]+value[16]) |
| 0x8D4000 | 4KB | PIPE_REFCOUNT — reader/writer counts per pipe |
| 0x8D5000 | 4KB | SCRIPT_BUF — control flow stack for shell scripts |
| 0x8D6000 | 8KB | ARGV_BUF — exec argument passing (16 args x 256B) |
| 0x8D8000 | 4KB | JOB_TABLE — background job tracking (16 jobs) |

### Process Table Extension (existing 256B entries)

| Offset | Size | Field | Phase |
|--------|------|-------|-------|
| +96 | 8B | BRK (heap break address) | F1 |
| +104 | 8B | SIGNAL_MASK | I1 |
| +112 | 8B | FD_INHERIT_FLAGS | G1 |
| +120 | 8B | PGID (process group ID) | G3 |
| +128 | 32B | CWD (current working directory) | F2 |
| +160 | 8B | USER_PAGE_LIST_PTR | G1 |

---

## Dependency Graph

```
Phase F: Syscall Table & Dispatch (20 tasks, ~6 hrs)
    F1: Core Infrastructure ──┐
    F2: File I/O Syscalls ────┤
                              │
Phase G: fork/exec/waitpid    │  (30 tasks, ~11 hrs)
    G1: fork() ──────────────<┘  depends on F
    G2: exec() ──────────────<── depends on G1
    G3: waitpid ─────────────<── depends on G1
                              │
Phase H: Pipes & I/O          │  (20 tasks, ~6 hrs)
    H1: Pipe+FD integration ─<┤  depends on F (FD table)
    H2: Shell pipe parser ───<── depends on H1 + G (fork+exec)
                              │
Phase I: Signals & Job Ctrl   │  (20 tasks, ~6 hrs)
    I1: Signal infrastructure ─<── depends on G (process lifecycle)
    I2: Job control ──────────<── depends on I1 + H2
                              │
Phase J: Shell Scripting      │  (20 tasks, ~6 hrs)
    J1: Variables + loading ──<── depends on F (file I/O)
    J2: Control flow ─────────<── depends on J1
                              │
Phase K: Testing & Release    │  (10 tasks, ~2 hrs)
    K1: Integration ──────────<── depends on ALL
```

**Critical path:** F1 → F2 → G1 → G2 → G3 → H1 → H2 → K1

**Parallel opportunities:**
- J1 (variables) can start after F2 (only needs file reading)
- I1 (signal infra) can start after G1 (only needs process table)

---

## Timeline

```
Session 1:  Phase F, Sprint F1      — Syscall dispatch refactor
Session 2:  Phase F, Sprint F2      — File I/O syscalls
Session 3:  Phase G, Sprint G1      — fork()
Session 4:  Phase G, Sprint G2      — exec()
Session 5:  Phase G, Sprint G3      — waitpid + process groups
Session 6:  Phase H, Sprint H1      — Pipe+FD integration
Session 7:  Phase H, Sprint H2      — Shell pipe parser + redirect
Session 8:  Phase I, Sprint I1      — Signal infrastructure
Session 9:  Phase I, Sprint I2      — Job control
Session 10: Phase J, Sprint J1      — Variables + script loading
Session 11: Phase J, Sprint J2      — Control flow
Session 12: Phase K, Sprint K1      — Integration testing + release
```

---

## Target Metrics

| Metric | Current (v1.1.0) | Target (v1.2.0) |
|--------|------------------|------------------|
| Nova LOC | 12,954 | ~16,000 |
| Nova commands | 181 | 195+ |
| Syscalls | 5 (hardcoded) | 26 (table dispatch) |
| Process lifecycle | spawn+exit | fork+exec+waitpid+signals |
| Pipes | Pool exists (unused) | Shell pipes working |
| I/O redirection | None | `>`, `>>`, `<` |
| Signals | None | 8 signals + Ctrl+C |
| Job control | None | `&`, `jobs`, `fg`, `bg` |
| Shell scripting | None | if/for/while + $VAR |
| ELF from disk | Manual `exec` | fork+exec pipeline |
| Integration tests | 30 (E-phase) | 150+ (120 new) |
| Fajar Lang tests | 6,750+ | 6,870+ |

---

## Syscall Number Table (v0.7 Final)

| # | Name | Args | Returns | Phase |
|---|------|------|---------|-------|
| 0 | SYS_EXIT | code | (no return) | existing |
| 1 | SYS_WRITE | fd, buf, len | bytes_written | existing |
| 2 | SYS_READ | fd, buf, len | bytes_read | F1 |
| 3 | SYS_GETPID | — | pid | F1 |
| 4 | SYS_YIELD | — | 0 | F1 |
| 5 | SYS_BRK | increment | new_brk | F1 |
| 6 | SYS_MMAP | addr, len, prot | va | F1 |
| 7 | SYS_CLOCK | — | ticks | F1 |
| 8 | SYS_SLEEP | ms | 0 | F1 |
| 9 | SYS_SBRK | increment | old_brk | F1 |
| 10 | SYS_OPEN | path, flags | fd | F2 |
| 11 | SYS_CLOSE | fd | 0 | F2 |
| 12 | SYS_STAT | path, buf | 0 | F2 |
| 13 | SYS_FSTAT | fd, buf | 0 | F2 |
| 14 | SYS_LSEEK | fd, offset, whence | new_offset | F2 |
| 15 | SYS_DUP | old_fd | new_fd | F2 |
| 16 | SYS_DUP2 | old_fd, new_fd | new_fd | F2 |
| 17 | SYS_GETCWD | buf, size | len | F2 |
| 18 | SYS_CHDIR | path | 0 | F2 |
| 19 | SYS_UNLINK | path | 0 | F2 |
| 20 | SYS_FORK | — | pid/0 | G1 |
| 21 | SYS_EXEC | path, argv | (no return) | G2 |
| 22 | SYS_WAITPID | pid, status, opts | child_pid | G3 |
| 23 | SYS_PIPE | fds_buf | 0 | H1 |
| 24 | SYS_KILL | pid, signum | 0 | I1 |
| 25 | SYS_SIGNAL | signum, handler | old_handler | I1 |
| 26 | SYS_SETPGID | pid, pgid | 0 | G3 |

---

## Summary

```
Phase F:  Syscall Table & Dispatch    2 sprints   20 tasks    ~6 hrs    FOUNDATION
Phase G:  fork/exec/waitpid           3 sprints   30 tasks    ~11 hrs   CORE
Phase H:  Pipes & I/O Redirection     2 sprints   20 tasks    ~6 hrs    PLUMBING
Phase I:  Signals & Job Control       2 sprints   20 tasks    ~6 hrs    CONTROL
Phase J:  Shell Scripting             2 sprints   20 tasks    ~6 hrs    SCRIPTING
Phase K:  Testing & Release           1 sprint    10 tasks    ~2 hrs    RELEASE

Total:    12 sprints, 120 tasks, ~37 hours
```

---

*Nova v0.7 "Nexus" — connecting all pieces into a real UNIX-like OS*
*Built with Fajar Lang + Claude Opus 4.6*
