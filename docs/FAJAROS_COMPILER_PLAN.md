# Fajar Lang Compiler Improvements for FajarOS x86

> **Goal:** Make Fajar Lang fully capable of building FajarOS x86 microkernel as a real production OS
> **Team:** 10 engineers + AI assistants
> **Baseline:** FajarOS x86 = 20,416 LOC across 75 .fj files, microkernel architecture
> **Problem:** The OS works but hits compiler limitations that prevent it from being a true microkernel

---

## Executive Summary

FajarOS x86 is a 20K LOC operating system written 100% in Fajar Lang. It has a working microkernel with IPC, scheduler, memory management, NVMe/network drivers, and 200+ shell commands. **However, the OS currently works around compiler limitations rather than being fully supported by the compiler.**

This plan identifies **12 critical compiler improvements** needed, organized into **4 workstreams** for 10 engineers over **12 weeks**.

---

## Critical Gaps Analysis

### What FajarOS Needs vs What Fajar Lang Provides

| FajarOS Need | Current Compiler Status | Gap |
|-------------|------------------------|-----|
| Multi-binary build (kernel.elf + services) | Single-file build only | **CRITICAL** |
| @safe can't touch hardware | @safe partially enforced | **CRITICAL** |
| @safe → @kernel only via syscall | Direct calls allowed | **CRITICAL** |
| User-mode runtime (println via SYS_WRITE) | No user-mode runtime | **HIGH** |
| Typed IPC messages | Raw byte buffers | **HIGH** |
| Capability types (Cap\<PortIO\>) | Runtime checks only | **HIGH** |
| @device(net) hardware subset | @device is all-or-nothing | **MEDIUM** |
| Cross-service type sharing | No cross-binary types | **MEDIUM** |
| Async IPC | Blocking only | **MEDIUM** |
| Service declaration syntax | Manual IPC loops | **LOW** |
| Protocol definition syntax | Documentation only | **LOW** |
| Formal verification hooks | None | **FUTURE** |

### The Concatenation Hack

FajarOS currently uses a **Makefile that concatenates 75 .fj files** into one `combined.fj` before compiling. This is because Fajar Lang has **no real multi-file build system**. This hack:
- Prevents separate compilation of kernel vs userspace
- Makes all symbols global (no module isolation)
- Blocks the microkernel's key feature: separate address spaces

---

## Team Organization (10 Engineers)

| Team | Engineers | Focus | AI Assistant |
|------|----------|-------|-------------|
| **Core Compiler** | Engineer 1-2 | Multi-binary build, linker, module system | Claude for codegen |
| **Safety Enforcement** | Engineer 3-4 | @safe/@kernel/@device enforcement, call gates | Claude for analyzer |
| **IPC & Types** | Engineer 5-6 | Typed IPC, @message, protocol syntax | Claude for parser |
| **Runtime & Platform** | Engineer 7-8 | User-mode runtime, x86_64-user target, async | Claude for runtime |
| **Testing & Integration** | Engineer 9-10 | End-to-end tests, FajarOS build, QEMU CI | Claude for test gen |

---

## Workstream A: Multi-Binary Build System (Weeks 1-4)

> **Owner:** Engineers 1-2
> **Goal:** `fj build` can produce multiple ELFs from a single project

### A1: Module System with Real Imports (Week 1-2)

**Problem:** Fajar Lang's `use` and `mod` are parsed but don't actually resolve cross-file references. FajarOS hacks this with concatenation.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| A1.1 | Multi-file compilation | 8h | `fj build dir/` compiles all .fj files in directory |
| A1.2 | Import resolution | 8h | `use kernel::mm::frame_alloc` resolves across files |
| A1.3 | Symbol visibility | 4h | `pub` controls cross-module access, non-pub is private |
| A1.4 | Dependency ordering | 4h | Topological sort of file dependencies for compilation |
| A1.5 | Incremental multi-file | 4h | Only recompile changed files + dependents |
| A1.6 | Tests: 30+ | 4h | Cross-file function calls, import resolution, visibility |

**Deliverable:** `fj build kernel/` compiles all files in kernel/ directory, resolving imports.

### A2: Multiple Build Targets (Week 2-3)

**Problem:** FajarOS needs kernel.elf (x86_64-none) + vfs.elf (x86_64-user) + shell.elf (x86_64-user) from one project.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| A2.1 | Service manifest in fj.toml | 4h | `[[service]]` sections define per-service build |
| A2.2 | `fj build --all-services` | 8h | Builds each service as separate ELF |
| A2.3 | Per-service target | 4h | Kernel = x86_64-none, services = x86_64-user |
| A2.4 | Per-service entry point | 2h | Each service has its own `main()` or `@entry` |
| A2.5 | Initramfs packing | 4h | `fj pack` creates tar/cpio archive of service ELFs |
| A2.6 | Tests: 20+ | 4h | Build 3 separate ELFs from one project |

**fj.toml example:**
```toml
[project]
name = "fajaros"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
sources = ["kernel/", "drivers/"]

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
target = "x86_64-user"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
target = "x86_64-user"

[[service]]
name = "net"
entry = "services/net/main.fj"
target = "x86_64-user"
```

### A3: Linker Improvements (Week 3-4)

| # | Task | Effort | Detail |
|---|------|--------|--------|
| A3.1 | Custom linker script per target | 4h | Kernel at 0x100000, user at 0x400000 |
| A3.2 | .initramfs section embedding | 4h | Kernel ELF embeds service ELFs |
| A3.3 | Symbol stripping for services | 2h | `--strip-all` for user ELFs |
| A3.4 | Relocatable user ELFs | 4h | PIE support for ASLR |
| A3.5 | Tests: 10+ | 2h | Verify ELF layout, section addresses |

---

## Workstream B: Safety Enforcement (Weeks 1-4)

> **Owner:** Engineers 3-4
> **Goal:** Compiler prevents ALL privilege violations at build time

### B1: Complete @safe Restriction (Week 1)

**Problem:** @safe functions can currently call `port_outb`, `volatile_write`, etc. A @safe shell process could crash the kernel.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| B1.1 | Define 121+ blocked builtins | 4h | All bare-metal runtime functions |
| B1.2 | SE020 error for @safe violations | 2h | "hardware access not allowed in @safe" |
| B1.3 | Whitelist safe builtins | 2h | println, len, type_of, assert, math |
| B1.4 | Block asm!() in @safe | 1h | Inline assembly forbidden in @safe |
| B1.5 | Tests: 40+ | 4h | Every blocked builtin verified |

### B2: Call Gate Enforcement (Week 1-2)

**Problem:** @safe can call @kernel functions directly, bypassing syscall boundary.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| B2.1 | SE021 error for @safe→@kernel | 2h | "use syscall instead of direct call" |
| B2.2 | SE022 error for @safe→@device | 2h | "use IPC instead of direct call" |
| B2.3 | Allow @safe→@safe calls | 1h | Same-context calls always OK |
| B2.4 | Allow @kernel→@kernel calls | 1h | Within-kernel calls OK |
| B2.5 | Cross-context call graph | 4h | Visualize which contexts call which |
| B2.6 | Tests: 30+ | 4h | Every cross-context combination |

### B3: Capability Type System (Week 2-4)

**Problem:** FajarOS uses runtime capability bitmask. Compiler could enforce at build time.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| B3.1 | `Cap<T>` phantom type | 8h | Generic type for capabilities |
| B3.2 | Function requires capability | 4h | `fn driver(cap: Cap<PortIO>)` |
| B3.3 | Kernel grants capability | 4h | `kernel_grant::<PortIO>(pid)` |
| B3.4 | Revocation support | 4h | `kernel_revoke::<PortIO>(pid)` |
| B3.5 | 12 capability types | 4h | PortIO, IRQ, DMA, Memory, IPC, Timer, etc. |
| B3.6 | Tests: 20+ | 4h | Missing capability → compile error |

---

## Workstream C: IPC & Type System (Weeks 3-8)

> **Owner:** Engineers 5-6
> **Goal:** Type-safe IPC, service declarations, protocol definitions

### C1: @message Struct Annotation (Week 3-4)

**Problem:** IPC messages are 64-byte raw buffers. Type errors found only at runtime.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| C1.1 | `@message` struct parsing | 4h | `@message struct VfsOpen { path: str, flags: i64 }` |
| C1.2 | Auto serialize/deserialize | 8h | Struct ↔ 64-byte buffer conversion |
| C1.3 | Message ID assignment | 2h | Each @message gets unique type tag |
| C1.4 | Type-check ipc_send | 4h | `ipc_send(dst, VfsOpen { ... })` verified |
| C1.5 | Type-check ipc_recv | 4h | `let msg: VfsOpen = ipc_recv(src)` verified |
| C1.6 | Tests: 20+ | 4h | Wrong message type → compile error |

### C2: Protocol Definition (Week 5-6)

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| C2.1 | `protocol` keyword parsing | 4h | `protocol VfsProtocol { fn open(...) -> ... }` |
| C2.2 | `implements` clause | 4h | `service vfs implements VfsProtocol` |
| C2.3 | Completeness checking | 4h | Missing method → compile error |
| C2.4 | Client stub generation | 8h | `VfsClient::open(path)` → IPC call |
| C2.5 | Tests: 20+ | 4h | Protocol violations caught |

### C3: Service Declaration Syntax (Week 7-8)

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| C3.1 | `service` block parsing | 4h | `@safe service vfs { on VfsOpen(msg) => { ... } }` |
| C3.2 | Auto IPC loop generation | 8h | Compiler generates recv → match → handler → reply |
| C3.3 | Service lifecycle | 4h | Init, run, shutdown hooks |
| C3.4 | Tests: 15+ | 4h | Service compiles and handles messages |

---

## Workstream D: Runtime & Platform (Weeks 3-8)

> **Owner:** Engineers 7-8
> **Goal:** User-mode runtime, async IPC, platform support

### D1: User-Mode Runtime (Week 3-4)

**Problem:** @safe programs need `println` → SYS_WRITE, `exit` → SYS_EXIT. No user-mode runtime exists.

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| D1.1 | `fj_user_println` | 4h | Printf via SYS_WRITE syscall |
| D1.2 | `fj_user_exit` | 2h | Exit via SYS_EXIT |
| D1.3 | `fj_user_ipc_*` wrappers | 8h | Send/recv/call/reply via SYSCALL |
| D1.4 | `fj_user_malloc/free` | 4h | Heap via SYS_BRK |
| D1.5 | Auto-link for x86_64-user | 2h | Compiler links user runtime |
| D1.6 | Tests: 20+ | 4h | User programs compile and run |

### D2: Async IPC (Week 5-6)

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| D2.1 | `async fn ipc_recv()` | 8h | Non-blocking receive |
| D2.2 | Service event loop | 8h | `select! { msg, timer }` |
| D2.3 | Multi-client handling | 4h | Handle N concurrent requests |
| D2.4 | Tests: 15+ | 4h | Async service serves 2+ clients |

### D3: Cross-Service Type Sharing (Week 7-8)

**Tasks:**

| # | Task | Effort | Detail |
|---|------|--------|--------|
| D3.1 | `@shared` module annotation | 4h | Types shared across ELFs |
| D3.2 | Common header generation | 4h | Struct layout guaranteed identical |
| D3.3 | Import shared types | 4h | `use shared::FileInfo` in both VFS and shell |
| D3.4 | Tests: 10+ | 2h | Same struct layout across binaries |

---

## Workstream E: Testing & Integration (Weeks 1-12, ongoing)

> **Owner:** Engineers 9-10
> **Goal:** Verify Fajar Lang correctly compiles FajarOS

### E1: FajarOS Compilation Tests (Ongoing)

| # | Task | Effort | Detail |
|---|------|--------|--------|
| E1.1 | Parse ALL 75 FajarOS .fj files | 8h | Every file in fajaros-x86 must parse |
| E1.2 | Analyze ALL files | 8h | Type checking passes on real OS code |
| E1.3 | Compile kernel.elf | 8h | Native binary matches current output |
| E1.4 | Compile user services | 8h | VFS, shell, net compile as user ELFs |
| E1.5 | QEMU boot test | 4h | Compiled kernel boots in QEMU |
| E1.6 | Shell interaction test | 4h | Type commands, get output |

### E2: Context Safety Tests (Ongoing)

| # | Task | Effort | Detail |
|---|------|--------|--------|
| E2.1 | Import context_enforcement.fj | 4h | Verify all existing tests pass |
| E2.2 | @safe → hardware blocked | 4h | 121 builtins verified |
| E2.3 | @safe → @kernel blocked | 4h | Call gate enforced |
| E2.4 | @device(net) restriction | 4h | Cross-device access blocked |
| E2.5 | Capability type checks | 4h | Missing capability → error |

### E3: CI Pipeline

| # | Task | Effort | Detail |
|---|------|--------|--------|
| E3.1 | GitHub Actions: build FajarOS | 4h | `make build` in CI |
| E3.2 | QEMU boot test in CI | 4h | Boot + shutdown via QEMU |
| E3.3 | Context safety regression | 2h | All SE020/SE021 tests in CI |
| E3.4 | Multi-binary build test | 2h | 4 ELFs produced in CI |

---

## Timeline

```
Week 1-2:   [A1: Module System] [B1: @safe Restriction] [B2: Call Gates]
Week 3-4:   [A2: Multi-Binary]  [B3: Capabilities]      [C1: @message]  [D1: User Runtime]
Week 5-6:   [A3: Linker]        [C2: Protocols]          [D2: Async IPC]
Week 7-8:   [C3: Services]      [D3: Cross-Service Types]
Week 9-10:  Integration testing, bug fixes, FajarOS migration
Week 11-12: FajarOS v3.0 release preparation, documentation

All weeks:  [E: Testing & Integration — continuous]
```

## Milestone Deliverables

| Week | Milestone | Verification |
|------|-----------|-------------|
| 2 | @safe fully enforced, modules resolve | context_enforcement.fj passes |
| 4 | Multi-binary build works | kernel.elf + vfs.elf + shell.elf produced |
| 6 | Typed IPC, protocols | @message compile-time type checking |
| 8 | Service syntax, async IPC | FajarOS services compile as separate ELFs |
| 10 | End-to-end: FajarOS builds without concatenation hack | `fj build --all-services` produces bootable OS |
| 12 | FajarOS v3.0 "Sovereignty" release | QEMU boot, all services running, context safety proven |

---

## Success Criteria

1. **`make build` replaced by `fj build`** — no more concatenation hack
2. **Kernel = 1,300 LOC** in Ring 0 (down from 20,416 monolithic)
3. **9 services** compile as separate user-mode ELFs
4. **Zero @safe → hardware calls** compile (SE020 blocks all 121 builtins)
5. **Zero @safe → @kernel direct calls** compile (SE021 enforces syscall)
6. **Typed IPC** — wrong message type = compile error
7. **QEMU boot test passes** — kernel + services boot and interact
8. **200+ shell commands** work via IPC to kernel services

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Multi-binary build too complex | Start with 2 targets (kernel + 1 service), expand |
| Existing FajarOS code breaks | Keep concatenation path as fallback during migration |
| Async IPC too ambitious | Defer to post-12-week if needed; blocking IPC sufficient |
| Team coordination overhead | Weekly sync, shared CLAUDE.md per workstream |
| Capability types too academic | Start with simple per-function annotation, generalize later |

---

*"The compiler IS the security model."*
