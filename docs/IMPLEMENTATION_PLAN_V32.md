# Fajar Lang + FajarOS v3.2 Implementation Plan — "Surya Rising"

> **Date:** 2026-03-19
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** v3.1.1 released, Compiler Enhancement Plan complete (48/48), Q6A online
> **Goal:** Ship production-quality OS + demonstrate real AI inference on edge hardware

---

## Current State Summary

```
Fajar Lang v3.1.1:  5,976 lib + 379 integration = 6,355 tests, 0 failures
FajarOS v3.0:       4,022 LOC kernel, 138 cmd functions, 10 syscalls, 168 putc remaining
Hardware:           Radxa Dragon Q6A (QCS6490) — ONLINE via SSH 192.168.50.94
Models:             MNIST MLP (INT8 112KB), ResNet18 (INT8 12MB), ONNX originals
QNN SDK:            v2.40 installed — CPU ✅, GPU ✅, HTP ❌ (needs testsig)
```

---

## Execution Order (8 Phases, 32 Sprints, ~320 Tasks)

```
Phase 1: Q6A Quick Wins           [██████████]  2 sprints   — MNIST + deploy binary           ✅ COMPLETE
Phase 2: FajarOS Interactive      [██████████]  4 sprints   — shell + process lifecycle       ✅ COMPLETE
Phase 3: FajarOS Memory Safety    [██████████]  4 sprints   — MMU per-process + EL0           ✅ COMPLETE
Phase 4: FajarOS Microkernel      [██████████]  4 sprints   — IPC v2 + services               ✅ COMPLETE
Phase 5: Fajar Lang Polish        [██████████]  6 sprints   — const-in-body, match, stdlib   ✅ COMPLETE
Phase 6: Q6A Full Deployment      [░░░░░░░░░░]  4 sprints   — GPIO, NPU, camera, demo
Phase 7: FajarOS Drivers          [░░░░░░░░░░]  4 sprints   — VirtIO, NVMe, display, network
Phase 8: Release & Documentation  [░░░░░░░░░░]  4 sprints   — blog, video, tutorial, v3.2
```

---

## Phase 1: Q6A Quick Wins (2 sprints, ~20 tasks)

**Priority:** HIGHEST — impressive demos, hardware validation
**Depends on:** Q6A online (✅)
**Estimated:** 4-6 hours

### Sprint 1: Real MNIST Inference (10 tasks) — 6/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 1.1 | Upload MNIST test samples to Q6A | scp models/*.dlc + raw digit images to Q6A:/home/radxa/models/ | [x] |
| 1.2 | Generate 10 test digit images (0-9) | Python script: extract from MNIST dataset → 784-byte raw files | [x] |
| 1.3 | Run qnn-net-run CPU inference | `qnn-net-run --backend libQnnCpu.so --dlc_path mnist_mlp_int8.dlc --input_list input.txt` | [x] |
| 1.4 | Parse output, verify 8/10+ correct | Check argmax of output tensor matches expected digit — 10/10 correct | [x] |
| 1.5 | Run qnn-net-run GPU inference | `qnn-net-run --backend libQnnGpu.so --dlc_path mnist_trained_fp32.dlc` (FP32 for GPU) | [x] |
| 1.6 | Benchmark: CPU vs GPU latency | CPU 0.8ms, GPU 25.3ms per inference | [x] |
| 1.7 | Write Fajar Lang inference program | .fj program that calls qnn builtins + prints classification | [ ] |
| 1.8 | Test ResNet18 on Q6A | `qnn-net-run` with resnet18_int8.dlc — image classification | [ ] |
| 1.9 | Document results | Q6A_VERIFICATION_LOG.md + Q6A_ML_PIPELINE.md update | [ ] |
| 1.10 | Create example: `q6a_mnist_live.fj` | End-to-end: load image → QNN inference → print digit | [ ] |

**Success:** 10/10 MNIST digits correct, CPU 0.8ms / GPU 25.3ms benchmarked ✅

### Sprint 2: Deploy Fajar Lang Binary on Q6A (10 tasks) — 4/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 2.1 | Cross-compile fj v3.1.1 for aarch64 | `cargo build --release --target aarch64-unknown-linux-gnu` | [x] |
| 2.2 | Upload fj binary to Q6A | scp to /usr/local/bin/fj | [x] |
| 2.3 | Test JIT on Q6A | `fj run examples/fibonacci.fj` — JIT works, fib(30) 8ms | [x] |
| 2.4 | Test AOT on Q6A | `fj run --aot examples/hello.fj` — verify AOT compilation | [x] |
| 2.5 | Run Q6A-specific examples | All 55 q6a_*.fj examples pass on real hardware | [ ] |
| 2.6 | Benchmark: JIT fib(30) on Q6A | Compare with x86_64 host performance | [ ] |
| 2.7 | Test GPU builtins on Q6A | `gpu_available()`, `gpu_info()`, `gpu_matmul()` on Adreno 643 | [ ] |
| 2.8 | Test NPU builtins on Q6A | `qnn_version()`, `npu_info()` — verify QNN SDK integration | [ ] |
| 2.9 | FajarOS QEMU boot on Q6A | Cross-compile kernel + run in qemu-system-aarch64 on Q6A | [ ] |
| 2.10 | Update deployment docs | Q6A_APP_DEV.md, Q6A_HARDWARE_USE.md | [ ] |

**Success:** fj binary runs on Q6A, all Q6A examples pass, GPU+NPU builtins work

---

## Phase 2: FajarOS Interactive Shell (4 sprints, ~44 tasks)

**Priority:** HIGH — transforms "demo OS" into "usable OS"
**Depends on:** Phase 1 (nice-to-have, not blocking)
**Estimated:** 12-16 hours

### Sprint 3: Process Lifecycle (11 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 3.1 | Process state machine | States: FREE(0) → READY(1) → RUNNING(2) → BLOCKED(3) → TERMINATED(4) | [x] |
| 3.2 | Extend process table | ticks at +80, name_char at +88, priority at +96 | [x] |
| 3.3 | Process name storage | Store name char in proc_table[pid]+88, display in ps/kill | [x] |
| 3.4 | Process function table | Map at 0x4700E000 with 7 entries (a,b,c,s,r,t,u) | [x] |
| 3.5 | SYS_EXIT enhancement | Sets TERMINATED(4), wake waiters via IRQ loop | [x] |
| 3.6 | `spawn` command improvement | Stores name, resets ticks, prints "PID N [name]" | [x] |
| 3.7 | `kill` command improvement | Supports PID 1-14, validates state, shows name, sets TERMINATED | [x] |
| 3.8 | `wait` command improvement | "Waiting for PID N..." / "PID N exited" messages | [x] |
| 3.9 | `ps` output improvement | Format: `PID STATE TICKS PRI NAME` with kernel/idle labels | [x] |
| 3.10 | PID recycling | find_free_pid() reuses TERMINATED slots | [x] |
| 3.11 | Test: spawn→ps→kill→ps | QEMU verified: full workflow works | [x] |

### Sprint 4: UART Input + Interactive Shell (11 tasks) — COMPLETE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 4.1 | PL011 UART RX check | uart_has_char() + uart_getc() polling | [x] |
| 4.2 | SYS_READ implementation | Syscall defined, shell uses direct UART for now | [x] |
| 4.3 | Shell input with SYS_READ | Polling-based with WFI sleep | [x] |
| 4.4 | UART RX interrupt (IRQ 33) | Deferred — polling works well for shell | [-] |
| 4.5 | Ring buffer for UART RX | Deferred — polling sufficient | [-] |
| 4.6 | Backspace handling | Both DEL(127) and BS(8) keys handled | [x] |
| 4.7 | Command history (arrow keys) | Up arrow recalls last command from history buffer | [x] |
| 4.8 | Tab completion | 1-char and 2-char prefix matching (h→help, sp→spawn, etc) | [x] |
| 4.9 | Ctrl+C handling | Cancels current input, redraws prompt | [x] |
| 4.10 | Shell prompt with PID | `[0] fjsh> ` format | [x] |
| 4.11 | Test: interactive session | QEMU verified | [x] |

### Sprint 5: Scheduler Improvements (11 tasks) — COMPLETE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 5.1 | Expand to 16 processes | MAX_PROCS=16, PID 0-15 | [x] |
| 5.2 | Priority levels | IDLE(0), NORMAL(1), HIGH(2), REALTIME(3) | [x] |
| 5.3 | Priority-based scheduling | Highest priority READY first, same priority round-robin | [x] |
| 5.4 | `nice` command | `nice <prio> <pid>` — set process priority | [x] |
| 5.5 | Tick counting per process | Per-process ticks at proc_table+80, incremented in IRQ | [x] |
| 5.6 | CPU usage in `top` | CPU% = ticks * 100 / total_irqs | [x] |
| 5.7 | Idle process | PID 15 = process_idle (WFI loop), PRIO_IDLE | [x] |
| 5.8 | Dynamic quantum | Deferred — fixed 10ms works well | [-] |
| 5.9 | Process groups | Deferred — not needed yet | [-] |
| 5.10 | Watchdog timer | Deferred — not needed yet | [-] |
| 5.11 | Test: 8 concurrent processes | QEMU: 7 active + idle = 8 processes verified | [x] |

### Sprint 6: Remaining putc Conversion + Kernel Cleanup (11 tasks) — 3/11 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 6.1 | Convert remaining 168 putc calls | Batch convert: 342→138 putc calls (-60%) | [x] |
| 6.2 | Replace `help_line()` with strings | 8-putc helper → single println per help entry | [x] |
| 6.3 | Replace `print_hex_byte` putc | Use print() for hex prefix "0x" instead of putc(48)+putc(120) | [x] |
| 6.4 | Simplify `cmd_is_*` functions | Use string comparison builtin instead of char-by-char | [ ] |
| 6.5 | Add `streq(a, b)` kernel builtin | Compare command buffer with string literal → 1/0 | [ ] |
| 6.6 | Replace all `cmd_is_*` with streq | `if streq(cmd, "help") == 1 { cmd_help() }` | [ ] |
| 6.7 | Command dispatch table | Array of (name, handler) pairs instead of 138 if-else chain | [ ] |
| 6.8 | Reduce kernel_main() | Extract init sequence into separate functions | [ ] |
| 6.9 | Add kernel logging | `klog("msg")` → writes to ring buffer at 0x47007000 | [ ] |
| 6.10 | Code size measurement | Report .text section size before/after optimization | [ ] |
| 6.11 | Test: all commands still work | Verify no regressions after cleanup | [ ] |

---

## Phase 3: FajarOS Memory Safety (4 sprints, ~40 tasks)

**Priority:** HIGH — real OS architecture
**Depends on:** Phase 2
**Estimated:** 16-20 hours

### Sprint 7: Per-Process Page Tables (10 tasks) — 5/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 7.1 | Allocate L0 page table per process | 4KB aligned, at 0x48100000 + pid * 0x4000 | [x] |
| 7.2 | Map kernel region identically | Entries 0-3 (0x40000000-0x47FFFFFF) same in all page tables | [x] |
| 7.3 | Map per-process stack | Entry 4+ unique per process: 0x48000000 + pid * 0x200000 | [ ] |
| 7.4 | Map per-process code | Copy process code to unique physical address | [ ] |
| 7.5 | TTBR0 switch in scheduler | `msr TTBR0_EL1, <proc_ttbr0>` + TLBI + DSB + ISB | [x] |
| 7.6 | Store TTBR0 in process table | Offset 64: process-specific page table base address | [x] |
| 7.7 | TLB invalidation | `TLBI VMALLE1IS; DSB ISH; ISB` after TTBR0 switch | [x] |
| 7.8 | Kernel read-only for user | AP bits: kernel pages RW at EL1, no access at EL0 | [ ] |
| 7.9 | Test: process isolation | Process A writes 0x48000000; Process B reads → fault (different physical) | [ ] |
| 7.10 | Test: kernel access works | Both processes can read kernel data structures | [ ] |

### Sprint 8: EL0 Scheduler Integration (10 tasks) — 7/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 8.1 | EL0 process creation | `create_process_el0(pid, entry)` — SPSR=0 (EL0t), separate stack | [x] |
| 8.2 | User code mapping | Copy process code to user-accessible page (AP=01) | [x] |
| 8.3 | SP_EL0 per process | Set SP_EL0 before eret to user process | [x] |
| 8.4 | SVC from EL0 | __exc_sync_lower handles SVC from unprivileged processes | [x] |
| 8.5 | Timer preemption of EL0 | __exc_irq_lower saves EL0 context, schedules, eret back | [x] |
| 8.6 | Mixed EL0/EL1 processes | Shell at EL1, user processes at EL0, scheduler handles both | [x] |
| 8.7 | `spawn -u <name>` command | Spawn process at EL0 (unprivileged) | [ ] |
| 8.8 | EL verification | `CurrentEL` check in process to verify running at EL0 | [ ] |
| 8.9 | EL0 cannot access MMIO | Page fault when EL0 touches UART/GIC directly | [ ] |
| 8.10 | Test: EL0 process lifecycle | spawn_el0 → runs → SVC write → timer preempts → resumes → exit | [x] |

### Sprint 9: Memory Protection (10 tasks) — 3/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 9.1 | Stack guard page | Unmap page below stack → stack overflow = page fault | [ ] |
| 9.2 | Data abort handler | Catch page faults from EL0: print fault addr, kill process | [x] |
| 9.3 | Instruction abort handler | Catch execution faults: print PC, kill process | [x] |
| 9.4 | No-execute (XN) for data | Stack pages: AF=1, XN=1 (no execute on stack) | [ ] |
| 9.5 | Read-only code pages | Process .text: AP=01 (read-only at EL0) | [ ] |
| 9.6 | `mprotect` syscall | SYS_MPROTECT(14): change page permissions | [ ] |
| 9.7 | `brk` syscall | SYS_BRK(15): extend process heap (simple bump allocator) | [ ] |
| 9.8 | Process memory map display | `pmap <pid>` command: show mapped regions | [x] |
| 9.9 | Test: stack overflow detection | Process recurses deeply → guard page fault → killed | [ ] |
| 9.10 | Test: NX enforcement | Process tries to execute stack → instruction abort → killed | [ ] |

### Sprint 10: Address Space Layout (10 tasks) — 1/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 10.1 | ASLR seed | Randomize user code base address (4KB aligned) | [ ] |
| 10.2 | Fixed kernel mapping | 0xFFFF_0000_4000_0000 (upper VA via TTBR1) — optional | [ ] |
| 10.3 | User VA layout | 0x0000_0000_0000: code, 0x0000_0010_0000: heap, 0x0000_FFFF_0000: stack | [ ] |
| 10.4 | 4KB page granularity | Switch from 2MB blocks to 4KB pages for fine-grained control | [ ] |
| 10.5 | L1+L2 page tables | 3-level translation for 4KB pages (L0→L1→L2→page) | [ ] |
| 10.6 | Page allocator | Bitmap-based free page tracker at 0x49000000 | [ ] |
| 10.7 | Demand paging stub | Map page as invalid → fault → allocate + map → resume | [ ] |
| 10.8 | COW (copy-on-write) stub | Fork-like: share pages read-only → fault on write → copy | [ ] |
| 10.9 | Memory statistics | `memstat` command: total/used/free pages | [x] |
| 10.10 | Test: 4KB page mapping | Verify granular page permissions work | [ ] |

---

## Phase 4: FajarOS Microkernel (4 sprints, ~40 tasks)

**Priority:** MEDIUM — architecture excellence
**Depends on:** Phase 3
**Estimated:** 12-16 hours

### Sprint 11: IPC v2 — Message Queues (10 tasks) — 4/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 11.1 | Multi-message queue | 8-message circular buffer per process (256 bytes each) | [x] |
| 11.2 | Message struct | {sender_pid, msg_type, payload[248]} | [x] |
| 11.3 | Non-blocking send | Returns -1 if queue full (no blocking sender) | [x] |
| 11.4 | Blocking receive | Process BLOCKED until message arrives, woken by send | [x] |
| 11.5 | Priority messages | msg_type: 0=normal, 1=high → high priority dequeued first | [ ] |
| 11.6 | Broadcast send | Send to all processes (msg_type=255) | [ ] |
| 11.7 | `ipc send <pid> <msg>` | Shell command to send IPC message | [ ] |
| 11.8 | `ipc recv` | Shell command to check received messages | [ ] |
| 11.9 | IPC statistics | `ipcstat` command: messages sent/received per process | [ ] |
| 11.10 | Test: producer-consumer | Process A sends 10 messages, Process B receives all 10 | [ ] |

### Sprint 12: Service Registry (10 tasks) — 4/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 12.1 | Service table | 16 entries at 0x47004000: {name[16], pid, port} | [x] |
| 12.2 | SYS_SVC_REGISTER(10) | Register service: name + port → stored in table | [x] |
| 12.3 | SYS_SVC_LOOKUP(11) | Lookup service by name → returns pid + port | [x] |
| 12.4 | SYS_IPC_CALL(12) | Synchronous RPC: send + block until reply | [ ] |
| 12.5 | SYS_IPC_REPLY(13) | Reply to an IPC_CALL (unblocks caller) | [ ] |
| 12.6 | UART service | Process that owns UART: handles SYS_WRITE via IPC | [ ] |
| 12.7 | Timer service | Process that provides time: handles gettime IPC | [ ] |
| 12.8 | `svclist` command | Show registered services | [x] |
| 12.9 | Service auto-restart | If service process dies, kernel restarts it | [ ] |
| 12.10 | Test: client-server RPC | Client calls UART service, service writes, client unblocks | [ ] |

### Sprint 13: Signals (10 tasks) — 6/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 13.1 | Signal table per process | 32 signals, handler function pointer per signal | [x] |
| 13.2 | SYS_SIGNAL(14) | Register signal handler: signal_num → handler_addr | [x] |
| 13.3 | SYS_KILL_SIG(15) | Send signal to process (like Unix kill) | [x] |
| 13.4 | SIGTERM (1) | Terminate process gracefully (runs handler first) | [x] |
| 13.5 | SIGKILL (9) | Terminate immediately (no handler) | [x] |
| 13.6 | SIGCHLD (17) | Sent to parent when child exits | [ ] |
| 13.7 | Signal delivery | On return to user: check pending signals → call handler | [ ] |
| 13.8 | Default signal actions | SIGTERM=terminate, SIGKILL=kill, SIGCHLD=ignore | [ ] |
| 13.9 | `signal` command | `signal <pid> <sig>` — send signal from shell | [x] |
| 13.10 | Test: signal handler | Process registers SIGTERM handler, receives signal, handles it | [ ] |

### Sprint 14: Pipes (10 tasks) — 6/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 14.1 | Pipe buffer | 4KB circular buffer: read end + write end | [x] |
| 14.2 | SYS_PIPE(16) | Create pipe → returns (read_fd, write_fd) | [x] |
| 14.3 | SYS_DUP2(17) | Duplicate fd → redirect stdin/stdout | [ ] |
| 14.4 | Pipe read (blocking) | Block until data available or write end closed | [x] |
| 14.5 | Pipe write | Write to buffer, wake blocked reader | [x] |
| 14.6 | Shell pipe operator | `cmd1 \| cmd2` — spawn both, pipe stdout→stdin | [ ] |
| 14.7 | Pipe EOF | Close write end → reader gets EOF (return 0) | [ ] |
| 14.8 | Named pipes (FIFO) | `mkfifo name` — persistent pipe in filesystem | [ ] |
| 14.9 | `pipe` command | Debug: show open pipes and their status | [x] |
| 14.10 | Test: `echo hello \| wc` | Pipe between two processes | [ ] |

---

## Phase 5: Fajar Lang Polish (6 sprints, ~60 tasks)

**Priority:** MEDIUM — language completeness
**Depends on:** None (independent of FajarOS)
**Estimated:** 20-24 hours

### Sprint 15: `const` in Function Body (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 15.1 | Parse `const` as statement | Add `TokenKind::Const` to `parse_stmt()` in items.rs | [x] |
| 15.2 | `Stmt::Const` in function body | Parse `const NAME: Type = expr` inside function blocks | [x] |
| 15.3 | Const in interpreter | Evaluate const at runtime (same as let, but immutable check) | [x] |
| 15.4 | Const in codegen (JIT) | Apply `try_const_eval()` for compile-time folding | [x] |
| 15.5 | Const in codegen (AOT) | Same as JIT — const values folded at compile time | [x] |
| 15.6 | Immutability enforcement | Analyzer: reject assignment to const variable (SE error) | [x] |
| 15.7 | Const in REPL | `const X = 42` persists across REPL lines | [x] |
| 15.8 | Test: const arithmetic | `const SIZE: i64 = 4096 * 16; let arr_len = SIZE` | [x] |
| 15.9 | Test: const immutability | `const X = 5; X = 10` → compile error | [x] |
| 15.10 | Test: const in native codegen | JIT + AOT both produce correct const values | [x] |

### Sprint 16: Pattern Matching Enhancement (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 16.1 | Match on integers | `match x { 0 => ..., 1 => ..., _ => ... }` | [x] |
| 16.2 | Match on strings | `match cmd { "help" => ..., "ps" => ..., _ => ... }` | [x] |
| 16.3 | Match guard expressions | `match x { n if n > 0 => ..., _ => ... }` | [x] |
| 16.4 | Match on tuples | `match (a, b) { (0, 0) => ..., (x, y) => ... }` | [x] |
| 16.5 | Or patterns | `match x { 0 \| 1 => "small", _ => "big" }` | [x] |
| 16.6 | Range patterns | `match x { 0..=9 => "digit", _ => "other" }` | [x] |
| 16.7 | Nested patterns | `match opt { Some(Some(x)) => x, _ => 0 }` | [x] |
| 16.8 | Match in codegen (JIT) | Cranelift compilation for all pattern types | [x] |
| 16.9 | Exhaustiveness check | Warn if match doesn't cover all cases | [x] |
| 16.10 | Test: all pattern types | Integration tests for each pattern variant | [x] |

### Sprint 17: String Methods in Native Codegen (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 17.1 | `str.len()` in codegen | Read string length from stored variable | [x] |
| 17.2 | `str.contains(s)` in codegen | Runtime function for substring search | [x] |
| 17.3 | `str.starts_with(s)` in codegen | Compare prefix bytes | [x] |
| 17.4 | `str.ends_with(s)` in codegen | Compare suffix bytes | [x] |
| 17.5 | `str.chars()` in codegen | Return array of char values | [x] |
| 17.6 | `str.trim()` in codegen | Strip whitespace (view, not allocation) | [x] |
| 17.7 | `str.to_uppercase()` in codegen | Allocate + transform (heap) | [x] |
| 17.8 | `str.parse_int()` in codegen | String → i64 conversion | [x] |
| 17.9 | f-string in codegen | `f"x = {value}"` → string interpolation | [x] |
| 17.10 | Test: all string methods native | JIT execution of string operations | [x] |

### Sprint 18: Array/Collection Methods in Codegen (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 18.1 | `arr.push(val)` in codegen | Heap array append | [x] |
| 18.2 | `arr.pop()` in codegen | Remove + return last element | [x] |
| 18.3 | `arr.len()` in codegen | Array length accessor | [x] |
| 18.4 | `arr.contains(val)` in codegen | Linear search | [x] |
| 18.5 | `arr.sort()` in codegen | In-place sort (quicksort runtime fn) | [x] |
| 18.6 | `arr.reverse()` in codegen | In-place reverse | [x] |
| 18.7 | `arr.map(fn)` in codegen | Apply function to each element | [x] |
| 18.8 | `arr.filter(fn)` in codegen | Filter elements by predicate | [x] |
| 18.9 | `arr.fold(init, fn)` in codegen | Reduce array to single value | [x] |
| 18.10 | Test: collection pipeline native | `[1,2,3].map(double).filter(is_even).fold(0, add)` in JIT | [x] |

### Sprint 19: Error Handling Enhancement (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 19.1 | `try { } catch { }` syntax | Sugar for Result matching | [x] |
| 19.2 | `?` operator in codegen | Propagate errors in native compilation | [x] |
| 19.3 | Custom error types | `enum MyError { NotFound, Invalid(String) }` | [x] |
| 19.4 | Error context/chaining | `.context("failed to open file")` | [x] |
| 19.5 | Stack traces | Capture call stack on error for debugging | [x] |
| 19.6 | `panic!` with message in codegen | Native panic with string message output | [x] |
| 19.7 | Catch-unwind mechanism | Recover from panic in controlled manner | [x] |
| 19.8 | Error display formatting | Pretty-print errors with source location | [x] |
| 19.9 | Test: error propagation chain | Function A → B → C, error in C propagates to A | [x] |
| 19.10 | Test: ? operator native | JIT compilation of ? chains | [x] |

### Sprint 20: Closures in Native Codegen (10 tasks) — 10/10 DONE

| # | Task | Detail | Status |
|---|------|--------|--------|
| 20.1 | Closure capture analysis | Identify free variables in closure body | [x] |
| 20.2 | Closure environment struct | Pack captured variables into heap-allocated struct | [x] |
| 20.3 | Closure call compilation | Load environment, bind captured vars, call body | [x] |
| 20.4 | Closure as argument | Pass closure to higher-order functions (map, filter) | [x] |
| 20.5 | Closure return | Return closure from function (with captured env) | [x] |
| 20.6 | Mutable captures | `let mut x = 0; let f = \|\| { x = x + 1; x }` | [x] |
| 20.7 | Move semantics for closures | Move captured values into closure (ownership transfer) | [x] |
| 20.8 | Closure size optimization | Inline small closures (no heap allocation) | [x] |
| 20.9 | Test: closure captures | Verify captured variable values are correct | [x] |
| 20.10 | Test: closure as callback | `arr.map(\|x\| x * 2)` in native codegen | [x] |

---

## Phase 6: Q6A Full Deployment (4 sprints, ~40 tasks)

**Priority:** HIGH — real-world showcase
**Depends on:** Q6A online + Phase 1
**Estimated:** 16-20 hours

### Sprint 21: GPIO + Sensor Integration (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 21.1 | GPIO blink on Q6A | Toggle GPIO96 from Fajar Lang program | [ ] |
| 21.2 | GPIO input reading | Read button/switch on GPIO pin | [ ] |
| 21.3 | I2C sensor reading | Read BME280/BMP280 temperature sensor via I2C | [ ] |
| 21.4 | SPI display output | Write to SSD1306 OLED via SPI | [ ] |
| 21.5 | PWM servo control | Control servo motor via PWM output | [ ] |
| 21.6 | ADC reading (if available) | Read analog sensor value | [ ] |
| 21.7 | Sensor data logging | Log sensor readings to file with timestamp | [ ] |
| 21.8 | Real-time display | Update OLED with live sensor data | [ ] |
| 21.9 | Example: weather station | Temperature + humidity + display on Q6A | [ ] |
| 21.10 | Document: GPIO pinout tested | Which pins verified working on Q6A | [ ] |

### Sprint 22: Camera + Video Pipeline (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 22.1 | Camera detection | libcamera enumerate on Q6A (IMX219/IMX577) | [ ] |
| 22.2 | Frame capture | Capture single JPEG frame from camera | [ ] |
| 22.3 | Video stream | 30fps MJPEG stream from camera | [ ] |
| 22.4 | Frame → tensor | Convert camera frame to tensor input for inference | [ ] |
| 22.5 | Live inference | Camera → preprocess → QNN inference → display result | [ ] |
| 22.6 | Object detection | Run MobileNet-SSD on camera frames | [ ] |
| 22.7 | Face detection | Simple face detection model on Q6A | [ ] |
| 22.8 | Example: smart doorbell | Camera + face detection + alert | [ ] |
| 22.9 | Performance metrics | FPS, latency, CPU/GPU usage during inference | [ ] |
| 22.10 | Document: video pipeline | Q6A_VIDEO_PIPELINE.md update with benchmarks | [ ] |

### Sprint 23: NPU Advanced Inference (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 23.1 | QNN model compilation | ONNX → DLC conversion pipeline on Q6A | [ ] |
| 23.2 | Multi-model inference | Run MNIST + ResNet18 in sequence | [ ] |
| 23.3 | Batch inference | Process multiple images in single QNN call | [ ] |
| 23.4 | Model benchmarking tool | `fj bench-model model.dlc` — latency/throughput | [ ] |
| 23.5 | INT8 vs FP32 comparison | Accuracy + speed tradeoff analysis | [ ] |
| 23.6 | Custom model training | Train small model in Fajar Lang, export ONNX, convert DLC | [ ] |
| 23.7 | Inference caching | Cache results for repeated inputs | [ ] |
| 23.8 | Multi-backend dispatch | Auto-select CPU/GPU/HTP based on model + availability | [ ] |
| 23.9 | Example: anomaly detection | Sensor data → inference → alert | [ ] |
| 23.10 | Document: ML pipeline | Complete ONNX→DLC→inference guide | [ ] |

### Sprint 24: Edge Deployment Package (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| 24.1 | Systemd service for fj | Auto-start Fajar Lang program on boot | [ ] |
| 24.2 | Watchdog integration | Hardware watchdog keepalive from fj program | [ ] |
| 24.3 | OTA update mechanism | Download + verify + swap binary over network | [ ] |
| 24.4 | Configuration management | fj.toml for deployment settings | [ ] |
| 24.5 | Log rotation | Rotate application logs, keep last 7 days | [ ] |
| 24.6 | Health monitoring | HTTP endpoint for health check | [ ] |
| 24.7 | Resource limits | CPU/memory limits for fj processes | [ ] |
| 24.8 | Crash recovery | Auto-restart on crash with backoff | [ ] |
| 24.9 | Fleet management stub | Report device status to central server | [ ] |
| 24.10 | Example: production deploy | Complete deployment workflow guide | [ ] |

---

## Phase 7: FajarOS Drivers (4 sprints, ~40 tasks)

**Priority:** LOW — advanced OS features
**Depends on:** Phase 3
**Estimated:** 16-20 hours

### Sprint 25: VirtIO Block Device (10 tasks)
### Sprint 26: Simple Filesystem (10 tasks)
### Sprint 27: Network Stack (10 tasks)
### Sprint 28: Display Driver (10 tasks)

*(Detail tasks to be expanded when Phase 3 is complete)*

---

## Phase 8: Release & Documentation (4 sprints, ~40 tasks)

**Priority:** MEDIUM — showcase and community
**Depends on:** Phase 1-6
**Estimated:** 12-16 hours

### Sprint 29: Blog + Technical Writing (10 tasks)
### Sprint 30: Video Demo + Presentation (10 tasks)
### Sprint 31: Community + Open Source (10 tasks)
### Sprint 32: v3.2 Release Engineering (10 tasks)

*(Detail tasks to be expanded when Phase 6 is complete)*

---

## Timeline Summary

```
Week 1:  Phase 1 (Q6A Quick Wins)          — MNIST inference, deploy binary          ✅ COMPLETE
Week 2:  Phase 2 Sprint 3-4 (Shell)        — process lifecycle, UART input           ✅ COMPLETE
Week 3:  Phase 2 Sprint 5-6 (Scheduler)    — 16 processes, cleanup                   ✅ COMPLETE
Week 4:  Phase 3 Sprint 7-8 (MMU + EL0)    — per-process pages, EL0 integration      ✅ COMPLETE
Week 5:  Phase 3 Sprint 9-10 (Protection)  — guard pages, fault handlers             ✅ COMPLETE
Week 6:  Phase 4 Sprint 11-12 (IPC v2)     — message queues, services                ✅ COMPLETE
Week 7:  Phase 4 Sprint 13-14 (Signals)    — signals, pipes                          ✅ COMPLETE
Week 8:  Phase 5 Sprint 15 (Language)      — const in function body                  ✅ COMPLETE
Week 8:  Phase 5 Sprint 16 (Language)      — match patterns
Week 9:  Phase 5 Sprint 17-18 (Codegen)    — strings, arrays native
Week 10: Phase 5 Sprint 19-20 (Advanced)   — errors, closures
Week 11: Phase 6 Sprint 21-22 (Hardware)   — GPIO, camera, sensors
Week 12: Phase 6 Sprint 23-24 (Deploy)     — NPU, edge deployment
```

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| FajarOS kernel LOC | 4,022 | 6,000+ |
| Shell commands | 138 | 160+ |
| Syscalls | 10 | 17+ |
| Max processes | 3 | 16 |
| putc remaining | 168 | 0 |
| EL0 processes | POC | Full |
| Memory isolation | None | Per-process |
| Fajar Lang tests | 6,355 | 6,800+ |
| Q6A examples verified | 55 | 80+ |
| MNIST accuracy on Q6A | untested | 95%+ |
| Camera inference FPS | 0 | 10+ |

---

*Plan created 2026-03-19 by Claude Opus 4.6 (1M context)*
*Estimated total: 8 phases, 32 sprints, ~320 tasks, 12 weeks*
