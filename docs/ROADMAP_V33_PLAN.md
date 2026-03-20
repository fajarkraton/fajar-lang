# Fajar Lang + FajarOS Roadmap — Q2 2026

> **Date:** 2026-03-21
> **Author:** Fajar (PrimeCore.id) + Claude Opus 4.6
> **Context:** v3.2.1 shipped, Nova v0.3.0 shipped, 34 commits in one session
> **Philosophy:** Solidify → Validate → Document → Enhance → Expand

---

## Priority Order (Most Important First)

```
Phase I:   Testing + Quality          — solidify what we built (trust the foundation)
Phase II:  Real Hardware Validation   — prove it works beyond QEMU
Phase III: Blog + Documentation       — share the story, attract contributors
Phase IV:  Fajar Lang v3.3            — language improvements for long-term productivity
Phase V:   Nova v0.4                  — the most ambitious expansion
```

**Rationale:** We wrote ~4,000+ lines in one session. Before building more, we MUST:
1. Test what exists (find bugs before they compound)
2. Prove it works on real metal (not just QEMU simulation)
3. Document the architecture (while it's fresh)
4. Then improve the language, then expand the OS.

---

## Phase I: Testing + Quality (4 sprints, 40 tasks)

**Goal:** Zero known bugs, CI covers QEMU boot + NVMe + FAT32, fuzz coverage
**Timeline:** 1 week
**Why first:** 34 commits = high risk of latent bugs. Test now or pay later.

### Sprint T1: QEMU Integration Tests (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T1.1 | QEMU boot test script | Boot Nova, verify serial output, auto-exit | [ ] |
| T1.2 | NVMe init verification | grep "[NVMe] I/O queues ready" from serial | [ ] |
| T1.3 | FAT32 mount verification | grep "[FAT32] Mounted successfully" | [ ] |
| T1.4 | VFS init verification | grep "[VFS] Initialized" | [ ] |
| T1.5 | All-subsystem boot test | Verify 10 serial lines in order | [ ] |
| T1.6 | NVMe read test | Pre-populate FAT32, verify sector read | [ ] |
| T1.7 | Timeout handling | QEMU exits cleanly after 8s | [ ] |
| T1.8 | Multi-config test | Test with/without NVMe, with/without SMP | [ ] |
| T1.9 | CI workflow: `nova-boot.yml` | GitHub Actions with QEMU + NVMe disk | [ ] |
| T1.10 | CI badge in README | Nova boot status badge | [ ] |

### Sprint T2: Native Codegen Tests (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T2.1 | Test volatile_u64 roundtrip | Write + read back u64 value | [ ] |
| T2.2 | Test buffer LE/BE roundtrip | All 12 functions with known values | [ ] |
| T2.3 | Test port_inb/outb compilation | no_std compile test | [ ] |
| T2.4 | Test ltr/lgdt/lidt compilation | no_std compile test | [ ] |
| T2.5 | Test fn pointer: conditional | `if cond { fn_a } else { fn_b }; f(x)` | [ ] |
| T2.6 | Test fn pointer: array | `[fn_a, fn_b]; arr[0](x)` | [ ] |
| T2.7 | Test parser: `(expr)` after call | Verify no chaining on new line | [ ] |
| T2.8 | Test memcmp_buf/memcpy_buf/memset_buf | Roundtrip verification | [ ] |
| T2.9 | Test cpuid_eax/ebx/ecx/edx | Verify returns non-zero on x86_64 | [ ] |
| T2.10 | Regression: run full test suite | cargo test --features native: all pass | [ ] |

### Sprint T3: Fajar Lang Fuzzing (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T3.1 | Fuzz lexer | Random byte input → no panic | [ ] |
| T3.2 | Fuzz parser | Random token streams → no panic | [ ] |
| T3.3 | Fuzz interpreter | Random .fj programs → no UB | [ ] |
| T3.4 | Fuzz analyzer | Malformed ASTs → no crash | [ ] |
| T3.5 | Edge case: deep nesting | 100-level nested if/while/fn | [ ] |
| T3.6 | Edge case: huge arrays | [0; 1000000] → OOM not panic | [ ] |
| T3.7 | Edge case: recursive fn | fib(1000) → stack overflow not crash | [ ] |
| T3.8 | Edge case: empty input | Empty .fj file → clean error | [ ] |
| T3.9 | Fuzz smoke test in CI | 60-second fuzz run per PR | [ ] |
| T3.10 | Fix any bugs found | All fuzzer-discovered issues resolved | [ ] |

### Sprint T4: Benchmarks (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| T4.1 | Interpreter: fibonacci(30) | Baseline timing | [ ] |
| T4.2 | JIT: fibonacci(30) | Compare vs interpreter | [ ] |
| T4.3 | Tensor: matmul 100×100 | ndarray BLAS backend | [ ] |
| T4.4 | String ops: concat 10000 | String performance | [ ] |
| T4.5 | Array: sort 10000 elements | Algorithm benchmark | [ ] |
| T4.6 | Nova: NVMe sector read latency | rdtsc before/after read | [ ] |
| T4.7 | Nova: FAT32 file read throughput | Read 1MB file, measure MB/s | [ ] |
| T4.8 | Nova: context switch time | Process switch rdtsc delta | [ ] |
| T4.9 | Benchmark results: BENCHMARKS.md | Document all results | [ ] |
| T4.10 | CI benchmark tracking | Compare against baseline per commit | [ ] |

---

## Phase II: Real Hardware Validation (3 sprints, 30 tasks)

**Goal:** Nova boots on real i9-14900HX via KVM, NVMe R/W on real SSD
**Timeline:** 3-4 days
**Why second:** QEMU != real hardware. NVMe timing, ACPI quirks, LAPIC behavior differ.

### Sprint H1: KVM Boot (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| H1.1 | Build bootable USB/ISO | GRUB2 + Nova ELF on USB stick | [ ] |
| H1.2 | KVM boot on i9-14900HX | `qemu-system-x86_64 -enable-kvm` | [ ] |
| H1.3 | Serial output capture | COM1 → host serial or virtio-serial | [ ] |
| H1.4 | Verify NVMe detection | Real Samsung NVMe vs QEMU emulated | [ ] |
| H1.5 | Verify ACPI tables | RSDP/MADT on real hardware | [ ] |
| H1.6 | Verify CPU count | MADT should show 24 cores (i9-14900HX) | [ ] |
| H1.7 | Verify LAPIC/IOAPIC | Real APIC vs QEMU emulation | [ ] |
| H1.8 | Keyboard test | Real PS/2 keyboard input via port_inb | [ ] |
| H1.9 | VGA output | Real VGA text mode 80×25 | [ ] |
| H1.10 | Document: hardware quirks | Differences from QEMU behavior | [ ] |

### Sprint H2: NVMe Real SSD (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| H2.1 | Detect real NVMe SSD | Model/serial from Identify Controller | [ ] |
| H2.2 | Read sector 0 from real SSD | Verify boot signature | [ ] |
| H2.3 | Read FAT32 from real partition | Mount real FAT32 on NVMe | [ ] |
| H2.4 | Write test file to real SSD | fatwrite on real hardware | [ ] |
| H2.5 | Reboot and verify persistence | File survives real reboot | [ ] |
| H2.6 | NVMe performance | Real SSD latency vs QEMU | [ ] |
| H2.7 | Multi-sector read | 4KB, 64KB, 1MB reads | [ ] |
| H2.8 | Error handling | Bad LBA, timeout recovery | [ ] |
| H2.9 | SMART data | Temperature, wear level from real SSD | [ ] |
| H2.10 | Document: NVMe on real hardware | Benchmark results + quirks | [ ] |

### Sprint H3: SMP + USB on Real Hardware (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| H3.1 | Boot APs on real i9 | smpboot command on 24 cores | [ ] |
| H3.2 | Verify AP trampoline | All APs reach long mode | [ ] |
| H3.3 | Per-CPU LAPIC timer | Real LAPIC timer calibration | [ ] |
| H3.4 | USB XHCI on real hardware | Detect real Intel XHCI controller | [ ] |
| H3.5 | USB device enumeration | List connected USB devices | [ ] |
| H3.6 | Real keyboard via USB | XHCI HID input (stretch goal) | [ ] |
| H3.7 | Thermal monitoring | Read CPU temp from MSR | [ ] |
| H3.8 | Power management | ACPI S5 shutdown on real hardware | [ ] |
| H3.9 | Stress test | Run all 135 commands on real hardware | [ ] |
| H3.10 | Blog: BLOG_NOVA_REAL_HW.md | Real hardware boot story + photos | [ ] |

---

## Phase III: Blog + Documentation (3 sprints, 30 tasks)

**Goal:** Complete technical documentation, blog series, contributor guide
**Timeline:** 3-4 days
**Why third:** Document while fresh. Attracts contributors and validates design decisions.

### Sprint D1: Technical Blog Series (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D1.1 | Blog: Writing an NVMe Driver in Fajar Lang | Deep-dive: admin queue, phase bit, sector I/O | [ ] |
| D1.2 | Blog: FAT32 from Scratch | BPB, clusters, directory entries, write ops | [ ] |
| D1.3 | Blog: Ring 3 User Space | GDT, TSS, SYSCALL/SYSRET, IRETQ | [ ] |
| D1.4 | Blog: SMP on x86_64 | AP trampoline, INIT-SIPI-SIPI, per-CPU | [ ] |
| D1.5 | Blog: Building a TCP/IP Stack | Ethernet, ARP, IPv4, ICMP in bare-metal | [ ] |
| D1.6 | Blog: ELF Loading | ELF64 format, PT_LOAD, memory mapping | [ ] |
| D1.7 | Blog: Compiler → OS Pipeline | How Fajar Lang compiles to bare-metal ELF | [ ] |
| D1.8 | Blog: Parser Bug Post-Mortem | The `(expr)` after call bug — root cause + fix | [ ] |
| D1.9 | Blog: Edition 2024 Migration | Rust edition migration: gen keyword, patterns | [ ] |
| D1.10 | Index page | Blog index with links + summaries | [ ] |

### Sprint D2: Architecture Documentation (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D2.1 | NOVA_ARCHITECTURE.md refresh | Updated with v0.3 memory map + subsystems | [ ] |
| D2.2 | NOVA_SYSCALL_ABI.md | Syscall numbers, register convention, examples | [ ] |
| D2.3 | NOVA_DRIVERS.md | NVMe, FAT32, VGA, PS/2, network, USB | [ ] |
| D2.4 | NOVA_BOOT_SEQUENCE.md refresh | v0.3 boot: 12 init stages | [ ] |
| D2.5 | NOVA_COMMANDS.md refresh | All 135 commands categorized | [ ] |
| D2.6 | NOVA_MEMORY_MAP.md | Complete physical memory layout | [ ] |
| D2.7 | FAJAR_LANG_SPEC.md refresh | New builtins (port I/O, CPU, buffer) | [ ] |
| D2.8 | STDLIB_SPEC.md refresh | All builtin functions catalogued | [ ] |
| D2.9 | CONTRIBUTING.md refresh | How to contribute to Nova + Fajar Lang | [ ] |
| D2.10 | README.md update | Nova v0.3 stats + build instructions | [ ] |

### Sprint D3: Tutorials + Examples (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| D3.1 | Tutorial: Hello World in Fajar Lang | Zero to running program | [ ] |
| D3.2 | Tutorial: Build FajarOS Nova | From source to QEMU boot | [ ] |
| D3.3 | Tutorial: Add a Shell Command | Step-by-step guide | [ ] |
| D3.4 | Tutorial: Write an NVMe Driver | Simplified walkthrough | [ ] |
| D3.5 | Tutorial: Deploy to Edge (Q6A) | Cross-compile + SSH + run | [ ] |
| D3.6 | Example: ML inference pipeline | End-to-end tensor ops | [ ] |
| D3.7 | Example: Network ping | ICMP from bare-metal | [ ] |
| D3.8 | Example: File persistence | Write → reboot → read | [ ] |
| D3.9 | Video: Demo screencast | Terminal recording of Nova boot | [ ] |
| D3.10 | Update examples/ index | All 130+ examples categorized | [ ] |

---

## Phase IV: Fajar Lang v3.3 (5 sprints, 50 tasks)

**Goal:** Language improvements that benefit both application and OS development
**Timeline:** 1 week
**Why fourth:** Language improvements have compound returns — every future line of code benefits.

### Sprint L1: Const Fn + Compile-Time Eval (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L1.1 | `const fn` declaration | Mark functions as compile-time evaluable | [ ] |
| L1.2 | Const fn body validation | Only const-safe ops allowed | [ ] |
| L1.3 | Const fn call in const context | `const X = const_add(1, 2)` | [ ] |
| L1.4 | Const arrays | `const TABLE: [i64; 4] = [1, 2, 3, 4]` | [ ] |
| L1.5 | Const struct initialization | `const POINT = Point { x: 1, y: 2 }` | [ ] |
| L1.6 | Const if/match | Compile-time branching | [ ] |
| L1.7 | Const loop unrolling | `const fn sum(n) = if n == 0 { 0 } else { n + sum(n-1) }` | [ ] |
| L1.8 | @kernel const tables | Lookup tables computed at compile time | [ ] |
| L1.9 | Tests: const fn eval | 10 test cases | [ ] |
| L1.10 | Document const fn | FAJAR_LANG_SPEC.md section | [ ] |

### Sprint L2: Array Repeat + Slice (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L2.1 | `[expr; count]` syntax | Parser: `[0; 512]` creates 512 zeros | [ ] |
| L2.2 | Interpreter eval | Evaluate repeat expression | [ ] |
| L2.3 | Codegen emit | Native: memset for zero, loop for other | [ ] |
| L2.4 | @kernel support | Bare-metal array init without loops | [ ] |
| L2.5 | Slice type `&[T]` | Reference to contiguous sub-array | [ ] |
| L2.6 | Slice from array | `arr[2..5]` creates slice | [ ] |
| L2.7 | Slice len/index | `slice.len()`, `slice[i]` | [ ] |
| L2.8 | Slice in function params | `fn sum(data: &[i64]) -> i64` | [ ] |
| L2.9 | Tests: array repeat + slice | 15 test cases | [ ] |
| L2.10 | Document array/slice | FAJAR_LANG_SPEC.md update | [ ] |

### Sprint L3: Error Handling v2 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L3.1 | `?` operator in more contexts | Works in closures and nested calls | [ ] |
| L3.2 | `try {} catch {}` blocks | Alternative to match on Result | [ ] |
| L3.3 | Error chaining | `.context("while reading file")` | [ ] |
| L3.4 | Custom error types | `enum MyError { IoError(str), ParseError(str) }` | [ ] |
| L3.5 | `From` trait for error conversion | Automatic error type coercion | [ ] |
| L3.6 | `anyhow`-style dynamic errors | `Box<dyn Error>` equivalent | [ ] |
| L3.7 | Panic hook | Custom panic handler for @kernel | [ ] |
| L3.8 | Stack trace on error | Function call chain in error message | [ ] |
| L3.9 | Tests: error handling | 20 test cases | [ ] |
| L3.10 | Document error handling v2 | Tutorial + spec update | [ ] |

### Sprint L4: Trait Improvements (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L4.1 | Trait aliases | `trait ReadWrite = Read + Write` | [ ] |
| L4.2 | `impl Trait` in return position | `fn make() -> impl Iterator` | [ ] |
| L4.3 | Default method implementations | `trait Foo { fn bar() -> i64 { 0 } }` | [ ] |
| L4.4 | Supertraits | `trait B: A { }` — B requires A | [ ] |
| L4.5 | Associated types | `trait Iter { type Item; fn next() -> Option<Item> }` | [ ] |
| L4.6 | Trait object safety improvements | More methods allowed in `dyn Trait` | [ ] |
| L4.7 | `where` clause | `fn foo<T>(x: T) where T: Display` | [ ] |
| L4.8 | Blanket impls | `impl<T: Display> ToString for T` | [ ] |
| L4.9 | Tests: trait improvements | 20 test cases | [ ] |
| L4.10 | Document trait system | FAJAR_LANG_SPEC.md update | [ ] |

### Sprint L5: Async v2 + LSP (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| L5.1 | Async fn improvements | Better type inference for async return | [ ] |
| L5.2 | `select!` macro | Wait on multiple futures | [ ] |
| L5.3 | Async iterators | `async fn next() -> Option<T>` | [ ] |
| L5.4 | Timeout support | `timeout(duration, future)` | [ ] |
| L5.5 | LSP: go to definition | Jump to function/struct/trait source | [ ] |
| L5.6 | LSP: find references | Show all usages of symbol | [ ] |
| L5.7 | LSP: code actions | Quick fixes for common errors | [ ] |
| L5.8 | LSP: workspace symbols | Search across entire project | [ ] |
| L5.9 | Tests: async + LSP | 15 test cases | [ ] |
| L5.10 | Version bump to v3.3.0 | Cargo.toml + CHANGELOG + tag | [ ] |

---

## Phase V: Nova v0.4 "Resilience" (6 sprints, 60 tasks)

**Goal:** Actual user programs running in Ring 3, real virtio-net, multi-user shell
**Timeline:** 1-2 weeks
**Why last:** Most ambitious. Needs solid foundation (testing), validated hardware, good docs, and improved language.

### Sprint N1: Real Ring 3 Execution (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N1.1 | Minimal user binary | 50-byte ELF that calls SYS_WRITE + SYS_EXIT | [ ] |
| N1.2 | User code on FAT32 | Write hello.elf to disk image | [ ] |
| N1.3 | exec → IRETQ → user | Actually run code in Ring 3 | [ ] |
| N1.4 | SYS_WRITE from user | User writes to VGA via syscall | [ ] |
| N1.5 | SYS_EXIT from user | Clean return to kernel | [ ] |
| N1.6 | Page fault handler | Catch invalid user access | [ ] |
| N1.7 | GPF handler | Catch privilege violations | [ ] |
| N1.8 | Multiple user programs | Run 3 programs sequentially | [ ] |
| N1.9 | User heap (SYS_BRK) | Dynamic memory in user space | [ ] |
| N1.10 | Test: hello world from user | End-to-end: compile → FAT32 → exec → output | [ ] |

### Sprint N2: Virtio-Net Real TX/RX (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N2.1 | Virtio-net virtqueue init | RX + TX descriptor rings | [ ] |
| N2.2 | RX buffer allocation | Pre-fill RX ring with buffers | [ ] |
| N2.3 | Packet transmit | TX ring descriptor → kick → complete | [ ] |
| N2.4 | Packet receive | RX interrupt → read packet → process | [ ] |
| N2.5 | ARP real send/receive | Send ARP request, receive reply | [ ] |
| N2.6 | ICMP real ping | Send ping, receive pong | [ ] |
| N2.7 | TCP handshake | SYN → SYN-ACK → ACK | [ ] |
| N2.8 | TCP data transfer | Send/receive payload bytes | [ ] |
| N2.9 | Shell: working `ping` | Real ICMP with RTT measurement | [ ] |
| N2.10 | Test: QEMU TAP network | Verify ping from Nova to host | [ ] |

### Sprint N3: USB Mass Storage (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N3.1 | XHCI operational init | Reset → run → port status | [ ] |
| N3.2 | Device slot allocation | Address device → slot assignment | [ ] |
| N3.3 | USB descriptor read | Device, configuration, interface descriptors | [ ] |
| N3.4 | Mass storage class | Bulk-Only Transport (BOT) | [ ] |
| N3.5 | SCSI command: INQUIRY | Get device info | [ ] |
| N3.6 | SCSI command: READ(10) | Read sectors from USB drive | [ ] |
| N3.7 | SCSI command: WRITE(10) | Write sectors to USB drive | [ ] |
| N3.8 | Register as blk_dev | USB mass storage as block device | [ ] |
| N3.9 | Mount FAT32 from USB | Read files from USB stick | [ ] |
| N3.10 | Shell: `mount /dev/usb0` | USB mount command | [ ] |

### Sprint N4: Multi-User Shell (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N4.1 | Login prompt | "login: " → username → "password: " | [ ] |
| N4.2 | /etc/passwd file | username:uid:home_dir (no crypto yet) | [ ] |
| N4.3 | Per-user home directory | /home/<user> on FAT32 | [ ] |
| N4.4 | Per-user environment | PATH, HOME, USER variables | [ ] |
| N4.5 | Shell prompt with username | `user@nova:/$` | [ ] |
| N4.6 | `su` command | Switch user | [ ] |
| N4.7 | `who` command | List logged-in users | [ ] |
| N4.8 | `logout` command | Return to login prompt | [ ] |
| N4.9 | File permissions (basic) | Owner read/write per file | [ ] |
| N4.10 | Test: multi-user session | Login → create file → logout → login → verify | [ ] |

### Sprint N5: Init System v2 (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N5.1 | PID 1 init process | First process, spawns login shell | [ ] |
| N5.2 | /etc/inittab | Service definitions: name, command, restart policy | [ ] |
| N5.3 | Service start/stop | `service start sshd`, `service stop httpd` | [ ] |
| N5.4 | Auto-restart on crash | Exponential backoff restart | [ ] |
| N5.5 | Runlevels (basic) | 0=halt, 1=single, 3=multi-user, 6=reboot | [ ] |
| N5.6 | Orphan process reaping | PID 1 waits for zombies | [ ] |
| N5.7 | Shutdown sequence v2 | Signal all → wait timeout → force kill → sync → halt | [ ] |
| N5.8 | Boot log | Timestamped boot messages to /var/log/boot | [ ] |
| N5.9 | `dmesg` from persistent log | Read boot log from FAT32 | [ ] |
| N5.10 | Test: full boot → login → work → shutdown cycle | [ ] |

### Sprint N6: Release + Polish (10 tasks)

| # | Task | Detail | Status |
|---|------|--------|--------|
| N6.1 | Update Nova plan | Mark all sprints complete | [ ] |
| N6.2 | CHANGELOG.md update | Nova v0.4 release notes | [ ] |
| N6.3 | Performance benchmarks | NVMe, FAT32, syscall, context switch | [ ] |
| N6.4 | Security audit | Review all Ring 0↔3 transitions | [ ] |
| N6.5 | CI: full QEMU test suite | Boot + NVMe + FAT32 + user exec | [ ] |
| N6.6 | README.md update | Nova v0.4 features + screenshots | [ ] |
| N6.7 | Tag nova-v0.4.0 | Git tag + push | [ ] |
| N6.8 | Blog: BLOG_NOVA_V04.md | Release announcement + deep-dive | [ ] |
| N6.9 | Deploy to Q6A (if available) | Cross-compile Nova for ARM64 | [ ] |
| N6.10 | Roadmap v0.5 planning | Next features: GPU driver, sound, ext4 | [ ] |

---

## Dependency Graph

```
Phase I: Testing + Quality
    │ (confidence)
    ▼
Phase II: Real Hardware
    │ (validation)
    ▼
Phase III: Blog + Docs
    │ (knowledge sharing)
    ▼
Phase IV: Fajar Lang v3.3
    │ (better language)
    ▼
Phase V: Nova v0.4
    (bigger OS features)
```

## Timeline Summary

```
Week 1:  Phase I (Testing)        — QEMU CI, native tests, fuzzing, benchmarks
Week 1:  Phase II (Hardware)      — KVM boot, real NVMe, SMP, USB
Week 2:  Phase III (Docs)         — 10 blog posts, architecture docs, tutorials
Week 2:  Phase IV (Fajar v3.3)    — const fn, array repeat, error handling, traits
Week 3+: Phase V (Nova v0.4)      — Ring 3 exec, virtio-net, USB mass storage
```

## Target Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Fajar Lang tests | 6,045 native | 7,000+ |
| Nova LOC | 8,327 | 12,000+ |
| Nova commands | 135 | 160+ |
| Blog posts | 3 (v2.0, v3.2, Nova) | 13+ |
| CI jobs | 16 | 20+ |
| Real hardware verified | QEMU only | i9-14900HX |

---

*"Solidify → Validate → Document → Enhance → Expand"*
