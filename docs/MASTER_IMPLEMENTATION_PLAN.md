# Fajar Lang — Master Implementation Plan

> **Versi:** 6.0 FINAL | **Tanggal:** 2026-03-23
> **Status:** Dokumen referensi utama pengembangan Fajar Lang
> **Tim:** 10 Engineer + AI Assistant per orang
> **Target OS:** FajarOS x86_64 + FajarOS ARM64 (Radxa Dragon Q6A)
> **Durasi:** 24 minggu (6 bulan)
> **Referensi:** HuggingFace Candle, seL4, Rust, Zig, Koka

---

## Daftar Isi

1. [Posisi Saat Ini](#bagian-1-posisi-saat-ini)
2. [Organisasi Tim & Proses](#bagian-2-organisasi-tim--proses)
3. [Onboarding Plan](#bagian-3-onboarding-plan)
4. [Dependency Graph & Critical Path](#bagian-4-dependency-graph--critical-path)
5. [Implementation Plan Detail](#bagian-5-implementation-plan-detail-24-minggu)
6. [Definition of Done](#bagian-6-definition-of-done)
7. [Arsitektur Target](#bagian-7-arsitektur-target)
8. [ML Runtime & Candle Strategy](#bagian-8-ml-runtime--candle-strategy)
9. [Migration Strategy FajarOS](#bagian-9-migration-strategy-fajaros)
10. [Performance Budget](#bagian-10-performance-budget)
11. [Infrastructure & Hardware](#bagian-11-infrastructure--hardware)
12. [API Stability Policy](#bagian-12-api-stability-policy)
13. [Quality Gates](#bagian-13-quality-gates)
14. [Code Review & PR Process](#bagian-14-code-review--pr-process)
15. [Risk Matrix & Fallback Plans](#bagian-15-risk-matrix--fallback-plans)
16. [Metrik Sukses](#bagian-16-metrik-sukses)

---

## Bagian 1: Posisi Saat Ini

### Compiler Statistics

```
Codebase:        ~290,000 LOC Rust (220+ files)
Tests:           5,582 (0 failures)
Self-hosted:     1,268 LOC Fajar Lang (lexer + parser + analyzer + codegen)
Backends:        Cranelift (dev) + LLVM (release) + Wasm (browser)
Targets:         x86_64, ARM64, RISC-V, Wasm
IDE:             VS Code (LSP semantic tokens + inlay hints + DAP debugger)
Examples:        130+ program .fj
Packages:        7 standard (fj-math, fj-nn, fj-hal, fj-drivers, fj-http, fj-json, fj-crypto)
Builtins:        121 bare-metal runtime functions + tensor aliases
Error Codes:     80+ across 10 categories
```

### Fitur Unik (Tidak Ada di Bahasa Lain)

| Fitur | Status | Deskripsi |
|-------|--------|-----------|
| `@kernel/@device/@safe` | ✅ Implemented | Context annotations enforce domain isolation |
| Effect system (`with IO, Hardware`) | ✅ Implemented | Formal effect tracking, 8 built-in effects |
| Linear types (`linear struct`) | ✅ Implemented | Must-use-exactly-once, ME010 error |
| Comptime evaluation (`comptime {}`) | ✅ Implemented | Zig-style, CT001-CT008 errors |
| First-class tensors | ✅ Implemented | zeros/matmul/relu/softmax native |
| Macro system (`vec![]`, `@derive`) | ✅ Implemented | 11 built-in macros + macro_rules! |

### 10 Problem Kritis

| # | Problem | Dampak | Prioritas |
|---|---------|--------|-----------|
| P1 | **Concatenation hack** — FajarOS cat 75 file jadi 1 | Tidak bisa pisah kernel/userspace | CRITICAL |
| P2 | **@safe tidak fully enforced** — bisa panggil port_outb | Security model bocor | CRITICAL |
| P3 | **Tidak ada multi-binary build** | Microkernel butuh kernel.elf + service ELFs | CRITICAL |
| P4 | **Tidak ada user-mode runtime** | @safe program tak bisa println | HIGH |
| P5 | **IPC raw bytes** — 64-byte buffer tanpa tipe | Bug IPC di runtime | HIGH |
| P6 | **Tensor hanya f64** — tidak ada f16/bf16/INT8 | Embedded AI butuh quantized | HIGH |
| P7 | **Tidak ada device abstraction** | Code harus jalan di CPU/GPU/NPU | HIGH |
| P8 | **Macro system basic** — $ patterns belum work | Metaprogramming terbatas | MEDIUM |
| P9 | **Effect polymorphism belum ada** | Generic over effects tidak bisa | MEDIUM |
| P10 | **Tidak ada cross-service type sharing** | Struct harus di-copy antar ELF | MEDIUM |

---

## Bagian 2: Organisasi Tim & Proses

### 5 Workstream, 10 Engineer

| Tim | ID | Engineer | Fokus | Required Skills |
|-----|----|----------|-------|-----------------|
| **Build System** | A | E1, E2 | Multi-file, multi-binary, linker | Rust codegen, ELF/linker internals |
| **Safety** | B | E3, E4 | @safe enforcement, call gates, capabilities | Type systems, static analysis |
| **IPC & Protocol** | C | E5, E6 | Typed IPC, @message, protocol, service | Parser, code generation |
| **ML Runtime** | D | E7, E8 | Multi-dtype tensor, GPU/NPU, quantization | ML systems, Vulkan/QNN |
| **Platform & Test** | E | E9, E10 | User runtime, QEMU CI, FajarOS migration | OS internals, CI/CD |

### AI Assistant per Engineer

| Engineer | AI Task |
|----------|---------|
| E1-E2 | Generate linker scripts, ELF validation tests, module resolution tests |
| E3-E4 | Generate 200+ safety test cases, audit @safe blocked builtins |
| E5-E6 | Generate IPC serialization code, protocol client stubs |
| E7-E8 | Study Candle architecture, generate dtype conversion tests |
| E9-E10 | Generate QEMU test harness, FajarOS boot scripts, migration scripts |

### Sprint Ceremonies

| Ceremony | Frekuensi | Durasi | Peserta | Format |
|----------|-----------|--------|---------|--------|
| **Daily standup** | Setiap hari, 09:00 | 5 min | Per-tim (2 orang) | Slack async: done/doing/blocked |
| **Weekly sync** | Senin 10:00 | 30 min | Semua 10 engineer | Video call: progress + blockers |
| **Sprint review** | Setiap 2 minggu | 1 jam | Semua + stakeholder | Demo working features |
| **Sprint retro** | Setiap 2 minggu | 30 min | Semua 10 engineer | What worked / what didn't / action items |
| **Demo day** | Bulanan | 2 jam | Publik (community) | Record + publish to YouTube |
| **Architecture review** | Per-phase | 2 jam | Lead dari tiap tim | Design review sebelum phase baru |

### Repository Structure

```
fajar-lang/                  ← Compiler (repo utama, ~290K LOC)
├── src/                     ← Compiler source (Rust)
│   ├── lexer/               ← Tokenizer
│   ├── parser/              ← Recursive descent + Pratt
│   ├── analyzer/            ← Type check, effects, borrow, comptime
│   ├── codegen/             ← Cranelift, LLVM, Wasm, optimizer
│   ├── interpreter/         ← Tree-walking interpreter
│   ├── debugger/            ← DAP server + DWARF
│   ├── lsp/                 ← Language Server Protocol
│   ├── macros.rs            ← Macro expansion engine
│   └── ...
├── stdlib/                  ← Self-hosted compiler (.fj)
│   ├── lexer.fj             ← 381 LOC tokenizer
│   ├── parser.fj            ← 397 LOC parser
│   ├── analyzer.fj          ← 210 LOC type checker
│   └── codegen.fj           ← 280 LOC C emitter
├── tests/                   ← 5,582+ tests
├── examples/                ← 130+ programs including drone_controller.fj
├── book/                    ← The Fajar Lang Book (60+ chapters)
├── editors/vscode/          ← VS Code extension
├── packaging/               ← Docker, Homebrew, Snap
└── docs/                    ← THIS document + specs + plans

fajaros-x86/                 ← OS target x86_64 (20,416 LOC)
├── kernel/                  ← Microkernel (@kernel)
│   ├── core/                ← mm, sched, ipc, syscall, boot, security
│   ├── mm/                  ← Frame allocator, paging, heap, slab
│   ├── sched/               ← Process, scheduler, SMP, spinlock
│   ├── ipc/                 ← Message, pipe, channel, notify, shm
│   ├── syscall/             ← Entry, dispatch, ELF loader
│   ├── interrupts/          ← LAPIC, timer
│   ├── security/            ← Capability, limits, hardening
│   └── hw/                  ← ACPI, PCIe, UEFI, detect
├── drivers/                 ← Device drivers (@kernel)
│   ├── serial.fj, vga.fj, keyboard.fj, pci.fj
│   ├── nvme.fj, virtio_blk.fj, virtio_net.fj
│   └── xhci.fj, gpu.fj
├── services/                ← Userspace services (@safe)
│   ├── init/, shell/, vfs/, blk/, net/, display/, input/, gpu/, gui/, auth/
├── fs/                      ← Filesystems
│   ├── ramfs.fj, fat32.fj, vfs.fj
├── shell/                   ← Shell (200+ commands)
├── apps/                    ← User applications
└── tests/                   ← Kernel tests, context enforcement

fajaros-arm64/               ← OS target ARM64 (Radxa Dragon Q6A)
├── kernel/                  ← Same structure, ARM64 specifics
├── arch/aarch64/            ← GICv3, MMU, EL0/EL1
├── bsp/dragon_q6a/          ← Board Support Package
└── drivers/                 ← ARM64 drivers
```

---

## Bagian 3: Onboarding Plan

### Minggu 0 (Sebelum Sprint 1): Engineer Onboarding

| Hari | Aktivitas | Durasi | Output |
|------|-----------|--------|--------|
| **Hari 1** | Setup environment: clone repo, `cargo build`, `cargo test` | 2h | Build succeeds, 5,582 tests pass |
| **Hari 1** | Read: CLAUDE.md (project identity + architecture) | 1h | Understand compilation pipeline |
| **Hari 1** | Read: docs/V1_RULES.md (coding rules) | 1h | Understand code style + safety rules |
| **Hari 2** | Read: docs/FAJAR_LANG_SPEC.md (language spec) | 2h | Understand syntax + keywords |
| **Hari 2** | Run: `fj run examples/hello.fj`, `fj repl`, `fj doc` | 1h | See compiler in action |
| **Hari 2** | Study: assigned workstream source files (Bagian 5) | 2h | Understand code you'll modify |
| **Hari 3** | Read: docs/MASTER_IMPLEMENTATION_PLAN.md (THIS doc) | 2h | Understand full plan |
| **Hari 3** | Read: fajaros-x86 CLAUDE.md + README.md | 1h | Understand OS target |
| **Hari 3** | Study: docs/COMPILER_ENHANCEMENTS.md dari fajaros-x86 | 1h | Understand what OS needs |
| **Hari 4** | Pair programming session with existing maintainer | 4h | First PR: trivial fix or doc |
| **Hari 5** | Submit first PR (small test, doc fix, or comment) | 2h | CI passes, PR merged |

### Per-Workstream Reading List

| Tim | Files yang HARUS Dibaca | Estimasi |
|-----|------------------------|----------|
| A (Build) | `src/main.rs`, `src/codegen/cranelift/mod.rs`, `src/codegen/linker.rs`, `src/package/manifest.rs` | 8h |
| B (Safety) | `src/analyzer/type_check/`, `src/analyzer/scope.rs`, `src/analyzer/effects.rs`, `src/analyzer/borrow_lite.rs` | 8h |
| C (IPC) | `src/parser/ast.rs`, `src/parser/items.rs`, `src/parser/expr.rs`, `src/macros.rs` | 6h |
| D (ML) | `src/runtime/ml/`, `src/codegen/cranelift/compile/`, HuggingFace Candle `candle-core/src/tensor.rs` | 10h |
| E (Platform) | `src/codegen/cranelift/runtime_fns.rs`, `src/codegen/nostd.rs`, `src/codegen/target.rs`, fajaros-x86 `Makefile` | 8h |

---

## Bagian 4: Dependency Graph & Critical Path

### Task Dependencies

```
A1 (Module Resolution)
 ├── A2 (Multi-Binary) ──── depends on A1
 │    ├── A3 (Linker) ──── depends on A2
 │    ├── C1 (@message IPC) ──── depends on A2 (separate services)
 │    └── E1 (User Runtime) ──── depends on A2 (x86_64-user target)
 │         └── E3 (FajarOS Migration) ──── depends on A2 + E1
 │
B1 (@safe Block) ──── independent, start immediately
 ├── B2 (Call Gates) ──── depends on B1
 │    └── B3 (Capabilities) ──── depends on B2
 │
D1 (Multi-DType) ──── independent, start Minggu 7
 ├── D2 (Device Backend) ──── depends on D1
 │    └── D3 (Quantization) ──── depends on D2
 │         └── G2 (Real ML Models) ──── depends on D3
 │
C1 (@message) ──── depends on A2
 ├── C2 (Protocol) ──── depends on C1
 │    └── C3 (Service Syntax) ──── depends on C2
 │
F1 (Macros) ──── independent
F2 (Effect Poly) ──── independent
F3 (Async IPC) ──── depends on C1
```

### Critical Path (Longest Dependency Chain)

```
A1 (W1-2) → A2 (W3-4) → E1 (W7-8) → E3 (W11-12) → G3 (W23-24)
   2 wk      2 wk         2 wk         2 wk           2 wk

Total critical path: 10 minggu kerja dalam 24 minggu timeline
Slack: 14 minggu (buffer untuk delays + parallel work)
```

### Parallel Execution Map

```
Minggu    1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22  23  24
Tim A:  [--A1--][--A2--][--A3--]
Tim B:  [--B1--][--B2--][--B3--]
Tim C:                          [--C1--][--C2--][--C3--]
Tim D:                          [--D1--][--D2--][--D3--]                          [----G2----]
Tim E:                          [--E1--][--E2--][--E3--]                                      [--G3--]
Adv:                                                      [--F1--][--F2--][--F3--][--G1--]
```

---

## Bagian 5: Implementation Plan Detail (24 Minggu)

### Phase 1: Foundation — Multi-File & Safety (Minggu 1-6)

#### Workstream A: Multi-File Build System

##### Sprint A1: Real Module Resolution (Minggu 1-2)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| A1.1 | Multi-file compilation | E1 | 12h | `fj build dir/` compiles all .fj files, produces 1 ELF |
| A1.2 | Import resolution | E1 | 12h | `use kernel::mm::frame_alloc` finds function across files |
| A1.3 | Symbol table per module | E2 | 8h | Each file has private scope; only `pub` items exported |
| A1.4 | Pub visibility enforcement | E2 | 4h | Non-pub function → SE024 error from other module |
| A1.5 | Dependency graph | E1 | 4h | Files compiled in topological order |
| A1.6 | Circular dependency detection | E2 | 4h | Cycle → error with file names |
| A1.7 | Incremental multi-file | E1 | 8h | Changed file + dependents recompiled, rest cached |
| A1.8 | Tests: 40+ | E2+AI | 8h | Cross-file calls, visibility, cycles, incremental |

##### Sprint A2: Multi-Binary Build (Minggu 3-4)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| A2.1 | fj.toml `[[service]]` | E1 | 4h | Parse multiple build targets from manifest |
| A2.2 | `fj build --all` | E1 | 12h | Produces kernel.elf + N service ELFs |
| A2.3 | Per-target config | E2 | 4h | kernel=x86_64-none, service=x86_64-user |
| A2.4 | Per-target entry point | E2 | 4h | Each service has `@entry fn main()` |
| A2.5 | Output structure | E1 | 2h | `build/{kernel,services/vfs,services/shell}.elf` |
| A2.6 | ARM64 multi-target | E2 | 8h | Same project → both x86_64 + aarch64 ELFs |
| A2.7 | Tests: 30+ | AI | 8h | 4 ELFs from 1 project, both architectures |

##### Sprint A3: Linker & ELF (Minggu 5-6)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| A3.1 | Custom linker script per target | E1 | 6h | Kernel@0x100000, user@0x400000 |
| A3.2 | .initramfs section | E2 | 6h | Kernel ELF embeds service ELFs as data |
| A3.3 | `fj pack` command | E1 | 4h | Creates cpio/tar archive of service ELFs |
| A3.4 | PIE for user ELFs | E2 | 8h | User ELFs position-independent |
| A3.5 | ARM64 ELF | E1 | 8h | aarch64 ELF headers + relocations |
| A3.6 | Debug info per binary | E2 | 4h | DWARF sections in each ELF |
| A3.7 | Tests: 20+ | AI | 4h | ELF layout, sections, entry points |

---

#### Workstream B: Safety Enforcement

##### Sprint B1: @safe Hardware Block (Minggu 1-2)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| B1.1 | Define 121+ blocked builtins | E3 | 6h | Complete list in analyzer |
| B1.2 | SE020 error | E3 | 2h | Clear error: "hardware access not allowed in @safe" |
| B1.3 | Whitelist safe builtins | E4 | 4h | println, len, type_of, math, strings always OK |
| B1.4 | Block asm!() in @safe | E3 | 1h | asm!() → SE020 in @safe |
| B1.5 | Block asm!() in @device | E3 | 1h | asm!() → error in @device |
| B1.6 | Tests: 50+ | E4+AI | 8h | Every blocked builtin tested |

##### Sprint B2: Call Gate Enforcement (Minggu 3-4)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| B2.1 | SE021: @safe→@kernel blocked | E3 | 4h | Error with suggestion "use syscall" |
| B2.2 | SE022: @safe→@device blocked | E3 | 4h | Error with suggestion "use IPC" |
| B2.3 | SE023: @device→@kernel blocked | E4 | 4h | Except defined bridge functions |
| B2.4 | Same-context calls OK | E4 | 2h | @kernel→@kernel always allowed |
| B2.5 | `fj check --call-graph` | E3 | 4h | Report all cross-context calls |
| B2.6 | Syscall whitelist for @safe | E4 | 4h | sys_write, sys_exit, sys_send etc OK |
| B2.7 | Tests: 40+ | AI | 8h | Every cell in call matrix |

##### Sprint B3: Capability Types (Minggu 5-6)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| B3.1 | `Cap<T>` phantom type | E3 | 8h | New generic type in type system |
| B3.2 | 12 capability kinds | E4 | 4h | PortIO, IRQ, DMA, Memory, Timer, IPC, Net, Blk, GPU, NPU, SPI, I2C |
| B3.3 | Function requires Cap | E3 | 4h | `fn driver(cap: Cap<PortIO>)` → checked |
| B3.4 | Kernel grants capability | E4 | 4h | `kernel_grant::<PortIO>(pid)` |
| B3.5 | Revocation | E3 | 4h | `kernel_revoke(pid, cap)` |
| B3.6 | @device(net) auto-cap | E4 | 4h | `@device(net)` auto-grants Cap<Net> |
| B3.7 | Tests: 30+ | AI | 6h | Missing cap → compile error |

---

### Phase 2: IPC, Protocol & Runtime (Minggu 7-12)

#### Workstream C: Type-Safe IPC

##### Sprint C1: @message Typed IPC (Minggu 7-8)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| C1.1 | `@message` struct annotation | E5 | 4h | Parser recognizes @message |
| C1.2 | Auto serialize | E6 | 8h | Struct → 64-byte buffer at compile time |
| C1.3 | Auto deserialize | E6 | 8h | Buffer → struct at compile time |
| C1.4 | Message ID | E5 | 2h | Unique tag per @message type |
| C1.5 | Type-check ipc_send | E5 | 4h | Wrong type → compile error |
| C1.6 | Type-check ipc_recv | E5 | 4h | Wrong type → compile error |
| C1.7 | Size validation | E6 | 2h | >64 bytes → compile error |
| C1.8 | Tests: 30+ | AI | 6h | Type mismatch, size overflow |

##### Sprint C2: Protocol Definitions (Minggu 9-10)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| C2.1 | `protocol` keyword | E5 | 4h | `protocol VfsProto { fn open(...) }` parses |
| C2.2 | `implements` clause | E5 | 4h | `service vfs implements VfsProto` |
| C2.3 | Completeness check | E6 | 4h | Missing method → compile error |
| C2.4 | Client stub auto-gen | E6 | 12h | `VfsClient::open(path)` generates IPC call |
| C2.5 | Version negotiation | E5 | 4h | Protocol version in handshake |
| C2.6 | Tests: 25+ | AI | 6h | Incomplete service → error |

##### Sprint C3: Service Syntax (Minggu 11-12)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| C3.1 | `service` block | E5 | 6h | Top-level declaration parses |
| C3.2 | `on` handler | E6 | 6h | `on VfsOpen(msg) { ... }` |
| C3.3 | Auto IPC loop | E6 | 8h | Compiler generates recv→match→reply |
| C3.4 | Lifecycle hooks | E5 | 4h | `init {}` and `shutdown {}` |
| C3.5 | Health check | E5 | 2h | Kernel can ping service |
| C3.6 | Tests: 20+ | AI | 4h | Service compiles and responds |

---

#### Workstream D: ML Runtime (Candle-Inspired)

##### Sprint D1: Multi-DType Tensor (Minggu 7-8)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| D1.1 | DType enum | E7 | 4h | F16, BF16, F32, F64, I8, U8, I32, I64 |
| D1.2 | Storage per dtype | E7 | 12h | Buffer size matches dtype |
| D1.3 | Dtype conversion | E8 | 4h | `.to_f16()`, `.to_i8()` work |
| D1.4 | Compile-time shape | E8 | 8h | Shape mismatch → compile error |
| D1.5 | Creation per dtype | E7 | 4h | `zeros::<F16>(3, 4)` |
| D1.6 | Tests: 30+ | AI | 6h | All dtypes, conversions, ops |

##### Sprint D2: Device Backend (Minggu 9-10)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| D2.1 | Device enum | E7 | 4h | Cpu, Gpu(id), Npu |
| D2.2 | Backend trait | E8 | 8h | matmul/relu/softmax per backend |
| D2.3 | CPU backend | E7 | 4h | ndarray (existing) |
| D2.4 | GPU backend | E8 | 16h | Vulkan compute for Adreno 643 |
| D2.5 | NPU backend | E8 | 12h | QNN SDK for Hexagon 770 |
| D2.6 | Auto device select | E7 | 4h | Best available |
| D2.7 | Tests: 25+ | AI | 6h | Same result across backends |

##### Sprint D3: Quantization & Models (Minggu 11-12)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| D3.1 | GGUF parser | E7 | 12h | Load llama.cpp format |
| D3.2 | Safetensors parser | E8 | 8h | Load HuggingFace format |
| D3.3 | Q4/Q8 dequantize | E7 | 8h | Quantized matmul correct |
| D3.4 | INT8 quantize | E8 | 4h | Full → INT8 |
| D3.5 | Inference pipeline | E7 | 8h | Load→quantize→infer→output |
| D3.6 | Tests: 20+ | AI | 4h | Accuracy within 1% of f32 |

---

#### Workstream E: Platform & Testing

##### Sprint E1: User-Mode Runtime (Minggu 7-8)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| E1.1 | println via SYS_WRITE | E9 | 4h | @safe program prints to console |
| E1.2 | exit via SYS_EXIT | E9 | 2h | Clean process exit |
| E1.3 | malloc/free via SYS_BRK | E10 | 6h | User heap works |
| E1.4 | IPC wrappers | E9 | 8h | send/recv/call/reply via SYSCALL |
| E1.5 | Auto-link x86_64-user | E10 | 4h | Compiler links user runtime |
| E1.6 | Auto-link aarch64-user | E10 | 4h | ARM64 user runtime |
| E1.7 | Tests: 25+ | AI | 6h | User programs run in QEMU |

##### Sprint E2: QEMU CI (Minggu 9-10)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| E2.1 | Parse ALL 75 FajarOS x86 files | E9 | 8h | 0 parse errors |
| E2.2 | Parse FajarOS ARM64 files | E10 | 4h | 0 parse errors |
| E2.3 | QEMU x86_64 boot test | E9 | 8h | Serial output "FajarOS" verified |
| E2.4 | QEMU aarch64 boot test | E10 | 8h | ARM64 kernel boots |
| E2.5 | GitHub Actions CI | E9 | 4h | PR auto-tests build+boot |
| E2.6 | Perf regression tracking | E10 | 4h | Compile speed per commit |
| E2.7 | Tests: 30+ | AI | 6h | Boot + service communication |

##### Sprint E3: FajarOS Migration (Minggu 11-12)

| # | Task | Owner | Effort | Acceptance Criteria |
|---|------|-------|--------|-------------------|
| E3.1 | Migrate x86 to multi-file | E9 | 16h | `fj build --all` replaces `make build` |
| E3.2 | Split into kernel + 9 services | E10 | 16h | 10 separate ELFs |
| E3.3 | Migrate ARM64 | E9 | 8h | Same structure |
| E3.4 | Verify 200+ commands | E10 | 8h | All shell commands work via IPC |
| E3.5 | Performance comparison | E9 | 4h | Boot time monolith vs micro |

---

### Phase 3: Advanced Features (Minggu 13-18)

| Sprint | Owner | Tasks | Tests |
|--------|-------|-------|-------|
| F1: Complete Macros | E5, E6 | `$` token, fragment specs, repetition, hygiene | 30+ |
| F2: Effect Polymorphism | E3, E4 | Effect vars, inference, subtyping | 20+ |
| F3: Async IPC | E9, E10 | Non-blocking recv, event loop, multi-client | 20+ |

### Phase 4: Production (Minggu 19-24)

| Sprint | Owner | Tasks | Tests |
|--------|-------|-------|-------|
| G1: Formal Verification | E3, E4 | @requires/@ensures enforcement | 20+ |
| G2: Real ML Models | E7, E8 | TinyLLaMA, YOLO, Whisper on Q6A | 20+ |
| G3: v3.0 Release | E9, E10 | Both platforms boot, paper, beta users | 30+ |

---

## Bagian 6: Definition of Done

### Per-Task DoD

Sebuah task dianggap **DONE** jika dan hanya jika:

```
1. Code committed to feature branch
2. All new functions have at least 1 test
3. cargo test — ALL pass (existing + new)
4. cargo clippy -- -D warnings — ZERO warnings
5. cargo fmt — formatted
6. No .unwrap() in src/ (only in tests/)
7. All pub items have /// doc comments
8. PR created with description
9. AI review passed (no obvious bugs)
10. Human review passed (1 reviewer from same workstream)
11. CI green (build + test + clippy + fmt)
12. PR merged to main
```

### Per-Sprint DoD

```
1. All tasks in sprint are DONE (per above)
2. Acceptance criteria met (defined per sprint)
3. 20+ new tests added
4. FajarOS x86 regression: all 75 files still parse
5. FajarOS ARM64 regression: boot files still parse
6. Sprint review demo delivered
7. Documentation updated if user-facing change
```

### Per-Phase DoD

```
1. Phase gate criteria met (Bagian 13)
2. Architecture review completed
3. Performance regression check passed
4. All blockers for next phase resolved
5. Updated Gantt if timeline shifted
```

---

## Bagian 7: Arsitektur Target

### FajarOS x86_64

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe)        — Ring 3                         │
│   apps/compiler/main.fj     Self-hosted Fajar Lang compiler  │
│   apps/editor/main.fj       Text editor                      │
│   apps/mnist.fj             MNIST classifier (@device)       │
├─────────────────────────────────────────────────────────────┤
│ Services (@safe, separate ELFs)  — Ring 3, IPC              │
│   services/shell/    200+ commands, scripting                │
│   services/vfs/      VFS + FAT32 + RamFS                     │
│   services/net/      TCP/IP, DNS, HTTP, TLS                  │
│   services/blk/      NVMe + VirtIO block + journal           │
│   services/display/  VGA + framebuffer                       │
│   services/input/    Keyboard + mouse                        │
│   services/gpu/      GPU compute dispatch                    │
│   services/gui/      Window compositor                       │
│   services/auth/     Authentication                          │
├─────────────────────────────────────────────────────────────┤
│ Microkernel (@kernel, ~2,500 LOC)  — Ring 0                 │
│   mm.fj        Frame alloc, paging, heap, slab               │
│   sched.fj     Process table (16 PIDs), round-robin, SMP     │
│   ipc.fj       Synchronous rendezvous (seL4-style)           │
│   syscall.fj   SYSCALL/SYSRET, 20+ syscalls                  │
│   boot.fj      IDT, TSS, GDT, panic                          │
│   security.fj  Capabilities (12 kinds), limits                │
├─────────────────────────────────────────────────────────────┤
│ Hardware — Intel Core i9-14900HX                              │
│   24 cores / 32 threads, 5.8 GHz, 32 GB DDR5                │
│   NVIDIA RTX 4090 Laptop, NVMe Gen4, UEFI boot              │
└─────────────────────────────────────────────────────────────┘
```

### FajarOS ARM64 (Radxa Dragon Q6A)

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe)        — EL0                            │
│   AI inference, sensor fusion, drone navigation              │
├─────────────────────────────────────────────────────────────┤
│ Services (@safe/@device)    — EL0, IPC                      │
│   npu/       Hexagon NPU inference (12 TOPS, QNN SDK)        │
│   gpu/       Adreno 643 Vulkan compute (773 GFLOPS)          │
│   camera/    IMX219/IMX577, libcamera, 4K pipeline           │
│   net/       WiFi (WCN6750) + Ethernet                       │
│   sensor/    GPIO (40-pin), I2C, SPI                         │
├─────────────────────────────────────────────────────────────┤
│ Microkernel (@kernel)       — EL1                            │
│   core/      Same as x86 (arch-independent IPC/sched/mm)     │
│   aarch64/   GICv3 interrupt controller, MMU (48-bit VA)     │
│   q6a/       BSP: 40-pin GPIO pinout, clock config, DTS      │
├─────────────────────────────────────────────────────────────┤
│ Hardware — Qualcomm QCS6490                                   │
│   Kryo 670: 1×A78@2.7 + 3×A78@2.4 + 4×A55@1.9 GHz          │
│   Adreno 643 GPU @ 812 MHz, Vulkan 1.3                       │
│   Hexagon 770 DSP/NPU, 12 TOPS                               │
│   7.4 GB LPDDR4X, NVMe PCIe Gen3                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Bagian 8: ML Runtime & Candle Strategy

### Adopt dari Candle

| Pattern | Candle Asli | Adaptasi Fajar Lang |
|---------|-----------|-------------------|
| `Device` enum | `Cpu`, `Cuda(id)`, `Metal` | `Cpu`, `Gpu(id)`, `Npu` |
| `DType` enum | f16, bf16, f32, f64, u8, u32, i64 | Sama + I8 untuk quantization |
| `Storage` backend | Per-device buffer management | Fajar `@device` auto-routes |
| GGUF format | llama.cpp compatible | Load pre-quantized LLMs |
| Safetensors | HuggingFace native | Load pre-trained weights |
| `Variable` + backprop | Autograd graph | Extend existing autograd |
| `candle-nn` layers | Linear, Conv2d, LayerNorm | Extend fj-nn package |

### Skip dari Candle

| Feature | Alasan |
|---------|--------|
| Python bindings (pyo3) | Bukan Python ecosystem |
| Flash Attention v3 | Terlalu advanced untuk embedded |
| 80+ model implementations | Fokus 5 model embedded-friendly |
| MKL/Accelerate | Target ARM64, bukan desktop Intel |
| CUDA langsung | Q6A pakai Vulkan + QNN, bukan CUDA |

### Target Model Matrix

| Model | Size | Backend | Latency Target | Platform |
|-------|------|---------|---------------|----------|
| MNIST MLP | 100KB | CPU | <1ms | x86+ARM64 |
| MobileNet v2 | 14MB | GPU | <10ms | Both |
| YOLO-tiny | 15MB | GPU (Vulkan) | <30ms | ARM64 |
| TinyLLaMA 1.1B | 600MB Q4 | NPU (Hexagon) | <100ms/token | ARM64 |
| Whisper-tiny | 75MB | CPU+NPU | <500ms | ARM64 |

---

## Bagian 9: Migration Strategy FajarOS

### Prinsip: Dual-Path Selama Transisi

```
Minggu 1-10:  KEDUA path work — concatenation (existing) + multi-file (new)
Minggu 11:    Multi-file validated, concatenation path deprecated
Minggu 12:    Concatenation Makefile removed, multi-file is the only path
```

### Step-by-Step Migration

| Step | Minggu | Action | Fallback |
|------|--------|--------|----------|
| 1 | 1-2 | `fj build kernel/` works (single ELF, same as concatenation) | `make build` still works |
| 2 | 3-4 | `fj build --all` produces kernel.elf + 1 service ELF | Fall back to step 1 |
| 3 | 5-6 | 3+ service ELFs, kernel embeds initramfs | Fall back to step 2 |
| 4 | 7-8 | User runtime: services can println via syscall | Fall back to step 3 |
| 5 | 9-10 | All 9 services compile as separate ELFs | Fall back to step 4 |
| 6 | 11 | QEMU boot test: microkernel + services running | Fall back to step 5 |
| 7 | 12 | Remove concatenation Makefile, multi-file only | Step 6 is stable fallback |

### FajarOS Code Changes Required

| File/Module | Current | After Migration |
|------------|---------|----------------|
| Makefile | 50+ lines cat command | `fj build --all` (1 line) |
| kernel/main.fj | Last in concat order | Standalone entry, `use kernel::*` |
| services/shell/ | Part of monolith | Separate ELF with `@safe` annotation |
| drivers/ | @kernel but in mono | @kernel, linked into kernel.elf |
| shell/commands.fj | Direct fn calls | IPC calls to kernel services |

---

## Bagian 10: Performance Budget

### Compilation Speed

| Metric | Target | How to Measure |
|--------|--------|---------------|
| Hello world (1 file) | <50ms | `time fj build hello.fj` |
| FajarOS kernel (20+ files) | <3s | `time fj build kernel/` |
| FajarOS all (75+ files) | <10s | `time fj build --all` |
| Incremental (1 file changed) | <500ms | Change 1 file, measure rebuild |
| ARM64 cross-compile | <15s | `fj build --target aarch64-unknown-none` |

### Runtime Performance

| Metric | Target | How to Measure |
|--------|--------|---------------|
| IPC round-trip | <5μs | Ping-pong between 2 services |
| Context switch | <2μs | Scheduler benchmark |
| Syscall overhead | <1μs | SYS_GETPID benchmark |
| Boot to shell | <500ms | QEMU serial timestamp |
| MNIST inference | <1ms | CPU inference benchmark |
| YOLO-tiny inference | <30ms | Adreno GPU via Vulkan |
| TinyLLaMA token | <100ms | Hexagon NPU via QNN |

### Binary Size

| Target | Max Size | Current |
|--------|---------|---------|
| Hello world ELF | <50KB | ~80KB |
| FajarOS kernel ELF | <100KB | ~22KB (microkernel) |
| FajarOS full (kernel + services) | <1MB | ~405KB (monolith) |
| User service ELF | <200KB | N/A (new) |

---

## Bagian 11: Infrastructure & Hardware

### Development Environment

| Item | Specification | Qty | Purpose |
|------|-------------|-----|---------|
| Dev machine | Linux x86_64, 16GB+ RAM, Rust stable | 10 | Per engineer |
| QEMU | qemu-system-x86_64, qemu-system-aarch64 | 10 | Local testing |
| Dragon Q6A | Radxa Dragon Q6A (QCS6490) | 2 | ARM64 hardware testing |
| CI server | GitHub Actions (Linux) | 1 | Automated build + test |
| NVMe test disk | Samsung PM9C1a or similar | 1 | NVMe driver testing |

### CI Pipeline

```
PR Created → GitHub Actions:
  1. cargo fmt -- --check
  2. cargo clippy -- -D warnings
  3. cargo test (all 5,582+ tests)
  4. cargo test --features native (codegen tests)
  5. Parse FajarOS x86 files (regression)
  6. Parse FajarOS ARM64 files (regression)
  7. QEMU x86_64 boot test (weekly, or on kernel changes)
  8. QEMU aarch64 boot test (weekly, or on kernel changes)
  9. Performance benchmark (compile speed, tracked over time)
```

### External Dependencies

| Dependency | Version | Purpose | Upgrade Policy |
|-----------|---------|---------|---------------|
| Rust toolchain | stable (1.85+) | Compiler host | Follow stable releases |
| Cranelift | Latest compatible | Dev backend | Pin in Cargo.toml |
| LLVM (inkwell) | 18.1 | Release backend | Major version only |
| ndarray | 0.16 | Tensor backend | Minor/patch only |
| tower-lsp | 0.20 | LSP server | Pin |
| QEMU | 8.2+ | Testing | System package |
| QNN SDK | 2.40+ | Hexagon NPU | Follow Qualcomm |
| Vulkan (ash) | Latest | GPU compute | Follow Mesa |

---

## Bagian 12: API Stability Policy

### Syntax Stability Levels

| Level | What Changes | Policy | Example |
|-------|-------------|--------|---------|
| **Stable** | Cannot change without deprecation | 6-month notice | `fn`, `let`, `struct`, `enum`, `match` |
| **Beta** | May change with 2-sprint notice | Notify in sprint review | `with` clause, `comptime`, `@derive` |
| **Experimental** | May change any time | Only in feature-gated code | `service`, `protocol`, `Cap<T>` |
| **Internal** | No stability guarantee | Only compiler internals | AST node structure, codegen details |

### FajarOS Compatibility During Development

```
Rule 1: Existing FajarOS code MUST continue to parse (regression test)
Rule 2: New features behind feature flags until Phase gate passes
Rule 3: If syntax must change, provide migration script
Rule 4: Compiler warns on deprecated syntax for 2 sprints before removal
```

### Versioning During 24 Weeks

```
Week 1-6:   v5.0.0-alpha.1 through alpha.6
Week 7-12:  v5.0.0-beta.1 through beta.6
Week 13-18: v5.0.0-rc.1 through rc.6
Week 19-24: v5.0.0 release
```

---

## Bagian 13: Quality Gates

### Per-Sprint Gate

```
□ cargo test — ALL pass (5,582+ dan bertambah)
□ cargo clippy -- -D warnings — ZERO warnings
□ cargo fmt -- --check — formatted
□ No .unwrap() added to src/
□ Acceptance criteria dari sprint terpenuhi
□ 20+ new tests per sprint minimum
□ FajarOS x86 regression: 75 files parse
□ FajarOS ARM64 regression: boot files parse
□ PR reviews completed (AI + human)
□ Sprint review demo delivered
```

### Phase Gates

| Phase | Gate Criteria |
|-------|-------------|
| **Phase 1** (W6) | `fj build --all` → kernel.elf + 3 service ELFs; @safe→port_outb → SE020; 200+ safety tests |
| **Phase 2** (W12) | Typed IPC works in QEMU; user println via SYS_WRITE; tensor f16 matmul; FajarOS migrated |
| **Phase 3** (W18) | Macro $patterns; effect polymorphism; async IPC 2+ clients; GGUF model loads |
| **Phase 4** (W24) | FajarOS x86 v3.0 boots; ARM64 v3.0 on Q6A; TinyLLaMA on NPU; paper submitted |

### Release Criteria (FajarOS v3.0)

```
□ ZERO concatenation — pure `fj build --all`
□ Kernel ≤ 2,500 LOC in Ring 0
□ 9+ services as separate ELFs
□ ALL @safe code CANNOT access hardware (0 bypass)
□ Typed IPC — wrong message = compile error
□ Both x86_64 and ARM64 boot and run
□ 200+ shell commands work via IPC
□ ≥1 ML model runs on Q6A NPU
□ 6,500+ tests (5,582 + 1,000+ new)
□ Book updated with new features
□ Performance budgets met (Bagian 10)
```

---

## Bagian 14: Code Review & PR Process

### PR Workflow

```
1. Engineer creates feature branch: feat/A1-module-resolution
2. Implement + write tests
3. Run locally: cargo test && cargo clippy -- -D warnings && cargo fmt
4. Push branch, create PR with template
5. AI assistant reviews (auto-triggered):
   - Check for .unwrap() in src/
   - Check pub items have doc comments
   - Check test coverage for new functions
   - Check FajarOS regression (parse test)
6. Human reviewer from same workstream reviews:
   - Logic correctness
   - Architecture fit
   - Performance implications
7. CI passes (all checks green)
8. Merge to main (squash or rebase, clean commit message)
9. Delete feature branch
```

### Review Matrix

| PR Author | Reviewer 1 (Same Tim) | Reviewer 2 (Cross Tim) |
|-----------|----------------------|----------------------|
| E1 | E2 | E9 (testing perspective) |
| E2 | E1 | E3 (safety perspective) |
| E3 | E4 | E5 (parser perspective) |
| E4 | E3 | E1 (build perspective) |
| E5 | E6 | E3 (type system perspective) |
| E6 | E5 | E7 (ML perspective) |
| E7 | E8 | E9 (platform perspective) |
| E8 | E7 | E10 (testing perspective) |
| E9 | E10 | E1 (build perspective) |
| E10 | E9 | E5 (integration perspective) |

### Commit Convention

```
Format: <type>(<scope>): <description>

Types: feat, fix, test, refactor, docs, perf, ci, chore
Scope: lexer, parser, analyzer, codegen, runtime, cli, ipc, ml, safety

Examples:
  feat(analyzer): implement SE020 @safe hardware restriction
  feat(codegen): add multi-binary build support
  fix(ipc): correct @message serialization alignment
  test(safety): add 50 @safe builtin block tests
```

---

## Bagian 15: Risk Matrix & Fallback Plans

| # | Risk | Prob | Impact | Mitigation | Fallback |
|---|------|------|--------|-----------|----------|
| R1 | Multi-file build too complex | Med | High | Start with 2 files, expand | Keep concatenation path as backup |
| R2 | FajarOS breaks during migration | High | Med | Dual-path (Bagian 9) | Revert to concatenation |
| R3 | Async IPC too ambitious | Med | Low | Defer to post-v3.0 | Blocking IPC sufficient |
| R4 | Hexagon NPU backend fails | High | Med | QNN SDK complexity | CPU fallback always available |
| R5 | Team coordination overhead | Med | Med | Per-workstream CLAUDE.md | Reduce to 3 workstreams |
| R6 | Capability types too academic | Low | Low | Simple annotation first | Runtime checks (existing) |
| R7 | ARM64 codegen bugs | Med | High | Test every commit on QEMU | x86 as primary, ARM64 secondary |
| R8 | Performance regression | Low | Med | Benchmark CI per commit | Profile + fix before gate |
| R9 | Engineer leaves team | Low | High | Document everything, pair programming | AI can cover gap temporarily |
| R10 | External dep breaks (LLVM, Cranelift) | Low | High | Pin versions in Cargo.toml | Delay upgrade, patch locally |

### Contingency Timeline

```
If Phase 1 takes 8 weeks instead of 6:
  → Compress Phase 3 (macros less critical for FajarOS)
  → Phase 4 starts Week 21 instead of 19

If Phase 2 is blocked by Phase 1:
  → D (ML) and F (macros) can start independently
  → C (IPC) shifts to Week 9 start

If ARM64 not ready by Week 24:
  → Ship x86 v3.0 first
  → ARM64 as v3.1 (4 weeks later)
```

---

## Bagian 16: Metrik Sukses

| Metrik | Saat Ini | Target W12 | Target W24 | World-Class |
|--------|----------|-----------|-----------|-------------|
| Tests | 5,582 | 6,200+ | 7,000+ | 15,000+ |
| FajarOS build | Concatenation | Multi-file | `fj build --all` | 1 command |
| @safe enforcement | Partial | Complete (121+ blocked) | Formally verified | seL4-level |
| IPC safety | Raw bytes | @message types | Protocol + verification | Zero-copy typed |
| Tensor dtypes | f64 only | f16/f32/f64 | + bf16/i8/u8 | Full Candle parity |
| ML models | MNIST MLP | +MobileNet | +YOLO +LLaMA +Whisper | 20+ |
| Platforms | x86_64 | + ARM64 QEMU | + ARM64 Q6A hardware | + RISC-V |
| Production users | 0 | Internal testing | 3 beta | 10+ |
| Conference papers | 0 | Draft | 1 submitted | 1 accepted |
| Self-hosting | 1,268 LOC | 2,000 LOC | 3,000 LOC | Full bootstrap |
| Services (separate ELF) | 0 | 3 | 9+ | 20+ |
| Compile speed (FajarOS) | N/A (concat) | <5s | <3s | <1s |
| Boot time (microkernel) | N/A | <1s | <500ms | <100ms |
| IPC latency | N/A | <50μs | <5μs | <1μs |

---

## Lampiran A: Effort Summary

### Total Hours per Phase

| Phase | Tim A | Tim B | Tim C | Tim D | Tim E | Total |
|-------|-------|-------|-------|-------|-------|-------|
| Phase 1 (W1-6) | 120h | 100h | — | — | — | **220h** |
| Phase 2 (W7-12) | — | — | 120h | 130h | 120h | **370h** |
| Phase 3 (W13-18) | — | 24h | 40h | — | 28h | **92h** |
| Phase 4 (W19-24) | — | 24h | — | 64h | 60h | **148h** |
| **Total** | **120h** | **148h** | **160h** | **194h** | **208h** | **830h** |

### Per-Engineer Load (24 weeks)

| Engineer | Hours | Avg/Week | Note |
|----------|-------|----------|------|
| E1 | ~80h | 3.3h | Build system focus |
| E2 | ~80h | 3.3h | Build system focus |
| E3 | ~80h | 3.3h | Safety + verification |
| E4 | ~68h | 2.8h | Safety + capabilities |
| E5 | ~80h | 3.3h | IPC + protocols |
| E6 | ~80h | 3.3h | IPC + code gen |
| E7 | ~100h | 4.2h | ML runtime (heaviest) |
| E8 | ~94h | 3.9h | GPU/NPU backends |
| E9 | ~104h | 4.3h | Platform + migration |
| E10 | ~104h | 4.3h | Testing + migration |

---

## Lampiran B: Glossary

| Term | Definition |
|------|-----------|
| @kernel | Context annotation for Ring 0 / EL1 code — hardware access allowed |
| @device | Context annotation for compute/ML code — tensor ops allowed |
| @safe | Context annotation for userspace — no hardware, no tensor |
| Effect | Declared side effect (`with IO, Hardware`) on function signature |
| Comptime | Compile-time evaluation block |
| Linear type | Value that MUST be consumed exactly once |
| IPC | Inter-Process Communication (seL4-style synchronous rendezvous) |
| Capability | Type-safe permission token (`Cap<PortIO>`) |
| Concatenation hack | Current FajarOS build: cat 75 files → 1 combined.fj |
| GGUF | Quantized model format (llama.cpp) |
| QNN | Qualcomm Neural Network SDK for Hexagon NPU |
| PubGrub | Conflict-driven dependency resolution algorithm |

---

*Dokumen ini adalah sumber referensi utama untuk pengembangan Fajar Lang.*
*Setiap engineer WAJIB membaca dokumen ini sebelum memulai sprint pertama.*

*Versi 6.0 FINAL — 2026-03-23*
*Target: x86_64 (Intel i9-14900HX) + ARM64 (Qualcomm QCS6490 / Radxa Dragon Q6A)*
