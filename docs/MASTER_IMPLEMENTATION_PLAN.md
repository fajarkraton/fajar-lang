# Fajar Lang — Master Implementation Plan

> **Versi:** 5.0 | **Tanggal:** 2026-03-23
> **Tim:** 10 Engineer + AI Assistant per orang
> **Target OS:** FajarOS x86_64 + FajarOS ARM64 (Radxa Dragon Q6A)
> **Durasi:** 24 minggu (6 bulan)
> **Referensi:** HuggingFace Candle, seL4, Rust, Zig, Koka

---

## Bagian 0: Posisi Saat Ini

### Apa yang SUDAH Ada

```
Compiler:        ~290,000 LOC Rust | 5,582 tests (0 failures)
Self-hosted:     1,268 LOC Fajar Lang (lexer + parser + analyzer + codegen)
Backends:        Cranelift (dev) + LLVM (release) + Wasm (browser)
Target:          x86_64, ARM64, RISC-V, Wasm
IDE:             VS Code (LSP + DAP debugger)
Examples:        130+ program .fj
Packages:        7 standard (fj-math, fj-nn, fj-hal, fj-drivers, fj-http, fj-json, fj-crypto)
```

### Fitur Unik (Tidak Ada di Bahasa Lain)

| Fitur | Status | Deskripsi |
|-------|--------|-----------|
| `@kernel/@device/@safe` | Implemented | Context annotations enforce domain isolation |
| Effect system (`with`) | Implemented | Formal effect tracking di function signatures |
| Linear types | Implemented | Must-use-exactly-once ownership |
| Comptime evaluation | Implemented | Zig-style compile-time execution |
| First-class tensors | Implemented | Tensor sebagai tipe native |

### Problem Kritis yang Harus Diselesaikan

| # | Problem | Dampak | Prioritas |
|---|---------|--------|-----------|
| P1 | **Concatenation hack** — FajarOS cat 75 file jadi 1 | Tidak bisa pisah kernel/userspace | CRITICAL |
| P2 | **@safe tidak fully enforced** — bisa panggil port_outb | Security model bocor | CRITICAL |
| P3 | **Tidak ada multi-binary build** | Microkernel butuh kernel.elf + service ELFs | CRITICAL |
| P4 | **Tidak ada user-mode runtime** | @safe program tak bisa println | HIGH |
| P5 | **IPC raw bytes** — 64-byte buffer tanpa tipe | Bug IPC di runtime, bukan compile | HIGH |
| P6 | **Tensor hanya f64** — tidak ada f16/bf16/INT8 | Embedded AI butuh quantized inference | HIGH |
| P7 | **Tidak ada device abstraction** | Sama code harus jalan di CPU/GPU/NPU | HIGH |
| P8 | **Macro system basic** — $ patterns belum work | Metaprogramming terbatas | MEDIUM |
| P9 | **Effect polymorphism belum ada** | Generic over effects tidak bisa | MEDIUM |
| P10 | **Tidak ada cross-service type sharing** | Struct harus di-copy antar ELF | MEDIUM |

---

## Bagian 1: Organisasi Tim

### 10 Engineer + AI

| Tim | Engineer | Fokus | Skill Set | AI Role |
|-----|----------|-------|-----------|---------|
| **A: Build System** | E1, E2 | Multi-file, multi-binary, linker | Rust codegen, ELF format | Generate linker scripts, test ELF |
| **B: Safety** | E3, E4 | @safe enforcement, call gates, capability types | Type systems, static analysis | Generate 200+ safety test cases |
| **C: IPC & Protocol** | E5, E6 | Typed IPC, @message, protocol, service syntax | Parser, analyzer, codegen | Generate IPC serialization code |
| **D: ML Runtime** | E7, E8 | Multi-dtype tensor, device backend, quantization | ML systems, CUDA/NPU | Study Candle architecture, port patterns |
| **E: Platform & Test** | E9, E10 | User runtime, QEMU CI, FajarOS integration | OS internals, CI/CD | Generate QEMU test harness, boot tests |

### Komunikasi

```
Daily:    Slack standup per tim (5 menit)
Weekly:   All-hands sync (Senin 10:00, 30 menit)
Per-PR:   AI review + cargo test + clippy
Monthly:  Demo day (tunjukkan progress ke stakeholder)
```

### Repository Structure

```
fajar-lang/              ← Compiler (repo utama)
├── src/                 ← Compiler source (Rust)
├── stdlib/              ← Self-hosted stdlib (.fj)
├── tests/               ← 5,582+ tests
├── examples/            ← 130+ examples
├── book/                ← The Fajar Lang Book
└── docs/                ← Specifications & plans

fajaros-x86/             ← OS target x86_64
├── kernel/              ← Microkernel (@kernel)
├── drivers/             ← Device drivers (@kernel)
├── services/            ← Userspace services (@safe)
├── shell/               ← Shell (@safe)
└── apps/                ← Applications (@safe)

fajaros-arm64/           ← OS target ARM64 (Dragon Q6A)
├── kernel/              ← Same structure, ARM64 specifics
├── bsp/                 ← Board Support Package (Q6A)
└── drivers/             ← ARM64 drivers (GICv3, etc.)
```

---

## Bagian 2: Implementation Plan (24 Minggu)

### Phase 1: Foundation — Multi-File & Safety (Minggu 1-6)

#### Workstream A: Multi-File Build System

**Goal:** Hilangkan concatenation hack. `fj build` compile multi-file project.

##### Sprint A1: Real Module Resolution (Minggu 1-2)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| A1.1 | Multi-file compilation | E1 | 12h | `fj build dir/` compiles semua .fj dalam directory |
| A1.2 | Import resolution | E1 | 12h | `use kernel::mm::frame_alloc` resolve cross-file |
| A1.3 | Symbol table per module | E2 | 8h | Setiap file punya scope sendiri |
| A1.4 | Pub visibility enforcement | E2 | 4h | Non-pub function tidak visible dari luar module |
| A1.5 | Dependency graph | E1 | 4h | Topological sort file compilation order |
| A1.6 | Circular dependency detection | E2 | 4h | Error jelas jika A imports B imports A |
| A1.7 | Incremental multi-file | E1 | 8h | Hanya recompile file yang berubah + dependents |
| A1.8 | Tests: 40+ | E2+AI | 8h | Cross-file calls, visibility, circular deps |

**Acceptance criteria:**
```bash
# Ini harus work:
fj build kernel/           # compile semua .fj di kernel/
fj build services/vfs/     # compile VFS service saja
```

##### Sprint A2: Multi-Binary Build (Minggu 3-4)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| A2.1 | fj.toml `[[service]]` sections | E1 | 4h | Define multiple build targets |
| A2.2 | `fj build --all` | E1 | 12h | Build kernel + semua services |
| A2.3 | Per-target configuration | E2 | 4h | kernel=x86_64-none, service=x86_64-user |
| A2.4 | Per-target entry point | E2 | 4h | Setiap service punya `@entry fn main()` |
| A2.5 | Output directory structure | E1 | 2h | build/kernel.elf, build/services/*.elf |
| A2.6 | ARM64 multi-target | E2 | 8h | Same project → x86_64 + aarch64 ELFs |
| A2.7 | Tests: 30+ | E1+AI | 8h | 4 ELFs dari 1 project |

**fj.toml format:**
```toml
[project]
name = "fajaros"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
sources = ["kernel/", "drivers/"]

[kernel.arm64]
entry = "kernel/main.fj"
target = "aarch64-unknown-none"
sources = ["kernel/", "drivers/", "arch/aarch64/"]

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
target = "x86_64-user"

[[service]]
name = "net"
entry = "services/net/main.fj"
target = "x86_64-user"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
target = "x86_64-user"
```

##### Sprint A3: Linker & ELF (Minggu 5-6)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| A3.1 | Custom linker script per target | E1 | 6h | Kernel@0x100000, user@0x400000 |
| A3.2 | .initramfs section | E2 | 6h | Kernel ELF embeds service ELFs |
| A3.3 | `fj pack` command | E1 | 4h | Pack service ELFs into initramfs archive |
| A3.4 | PIE support for user ELFs | E2 | 8h | Position-independent for ASLR |
| A3.5 | ARM64 ELF generation | E1 | 8h | aarch64 ELF headers, relocations |
| A3.6 | Debug info per binary | E2 | 4h | DWARF sections for each ELF |
| A3.7 | Tests: 20+ | AI | 4h | ELF layout verification |

---

#### Workstream B: Complete Safety Enforcement

**Goal:** Compiler prevents ALL privilege violations.

##### Sprint B1: @safe Hardware Block (Minggu 1-2)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| B1.1 | Define 121+ blocked builtins | E3 | 6h | All `fj_rt_bare_*` functions |
| B1.2 | SE020 error implementation | E3 | 2h | "hardware access not allowed in @safe" |
| B1.3 | Whitelist safe builtins | E4 | 4h | println, len, type_of, math, string ops |
| B1.4 | Block asm!() in @safe | E3 | 1h | Inline assembly forbidden in @safe |
| B1.5 | Block asm!() in @device | E3 | 1h | Inline assembly forbidden in @device |
| B1.6 | Tests: 50+ | E4+AI | 8h | Setiap blocked builtin diverifikasi |

**Blocked builtins (partial list):**
```
port_outb, port_inb, port_outw, port_inw, port_outl, port_inl,
volatile_read, volatile_write, volatile_read_u8, volatile_write_u8,
volatile_read_u16, volatile_write_u16, volatile_read_u32, volatile_write_u32,
volatile_read_u64, volatile_write_u64,
read_cr0, read_cr2, read_cr3, read_cr4, write_cr0, write_cr3, write_cr4,
read_msr, write_msr, cpuid, rdtsc, rdrand,
cli, sti, hlt, invlpg, iretq_to_user, fxsave, fxrstor,
pci_read, pci_write, dma_alloc, dma_free,
irq_register, irq_enable, irq_disable, irq_acknowledge,
gpio_set, gpio_read, spi_transfer, i2c_read, i2c_write, uart_write,
nvme_read, nvme_write, frame_alloc, frame_free, map_page, unmap_page,
set_current_pid, context_switch, ...
```

##### Sprint B2: Call Gate Enforcement (Minggu 3-4)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| B2.1 | SE021: @safe → @kernel blocked | E3 | 4h | "use syscall instead of direct call" |
| B2.2 | SE022: @safe → @device blocked | E3 | 4h | "use IPC instead of direct call" |
| B2.3 | SE023: @device → @kernel blocked | E4 | 4h | Except via defined bridge |
| B2.4 | Allow same-context calls | E4 | 2h | @kernel → @kernel OK |
| B2.5 | Cross-context call graph report | E3 | 4h | `fj check --call-graph` shows violations |
| B2.6 | Syscall whitelist for @safe | E4 | 4h | `sys_write`, `sys_exit`, `sys_send` etc OK |
| B2.7 | Tests: 40+ | AI | 8h | Every cross-context combination |

**Call matrix:**
```
            Caller
            @kernel  @device  @safe
Callee
@kernel     ✅ OK    ❌ SE023  ❌ SE021
@device     ❌ KE003 ✅ OK     ❌ SE022
@safe       ✅ OK    ✅ OK     ✅ OK
syscall     ✅ OK    ✅ OK     ✅ OK
```

##### Sprint B3: Capability Types (Minggu 5-6)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| B3.1 | `Cap<T>` phantom type | E3 | 8h | Generic capability type |
| B3.2 | 12 capability kinds | E4 | 4h | PortIO, IRQ, DMA, Memory, Timer, IPC, Net, Blk, GPU, NPU, SPI, I2C |
| B3.3 | Function requires Cap | E3 | 4h | `fn driver(cap: Cap<PortIO>)` |
| B3.4 | Kernel grants capability | E4 | 4h | `kernel_grant::<PortIO>(pid)` |
| B3.5 | Revocation | E3 | 4h | `kernel_revoke(pid, cap)` |
| B3.6 | @device(net) → Cap<Net> auto | E4 | 4h | `@device(net)` auto-gets Cap<Net> |
| B3.7 | Tests: 30+ | AI | 6h | Missing cap → compile error |

---

### Phase 2: IPC, Protocol & Runtime (Minggu 7-12)

#### Workstream C: Type-Safe IPC & Services

##### Sprint C1: @message Typed IPC (Minggu 7-8)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| C1.1 | `@message` struct annotation | E5 | 4h | `@message struct VfsOpen { path: str, flags: i64 }` |
| C1.2 | Auto serialize to 64-byte buffer | E6 | 8h | Struct → packed bytes at compile time |
| C1.3 | Auto deserialize from buffer | E6 | 8h | Packed bytes → struct at compile time |
| C1.4 | Message ID assignment | E5 | 2h | Setiap @message dapat unique tag |
| C1.5 | Type-check ipc_send | E5 | 4h | `ipc_send(dst, VfsOpen { ... })` verified |
| C1.6 | Type-check ipc_recv | E5 | 4h | `let msg: VfsOpen = ipc_recv(src)` verified |
| C1.7 | Size validation | E6 | 2h | @message struct harus ≤ 64 bytes |
| C1.8 | Tests: 30+ | AI | 6h | Wrong message type → compile error |

**Syntax:**
```fajar
@message struct VfsOpen {
    path_offset: i64,
    path_len: i64,
    flags: i64
}

@message struct VfsReply {
    fd: i64,
    status: i64
}

@safe fn open_file(path: str) -> i64 {
    let msg = VfsOpen { path_offset: 0, path_len: len(path), flags: 0 }
    ipc_send(VFS_PID, msg)    // Compile-time type checked!
    let reply: VfsReply = ipc_recv(VFS_PID)  // Type checked!
    reply.fd
}
```

##### Sprint C2: Protocol Definitions (Minggu 9-10)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| C2.1 | `protocol` keyword | E5 | 4h | Interface contract |
| C2.2 | `implements` clause | E5 | 4h | Service declares protocol |
| C2.3 | Completeness check | E6 | 4h | Missing method → compile error |
| C2.4 | Client stub auto-gen | E6 | 12h | `VfsClient::open(path)` → IPC call |
| C2.5 | Version negotiation | E5 | 4h | Protocol version in handshake |
| C2.6 | Tests: 25+ | AI | 6h | Protocol violations caught |

**Syntax:**
```fajar
protocol VfsProtocol {
    fn open(path: str, flags: i64) -> (fd: i64, status: i64)
    fn read(fd: i64, len: i64) -> (data: [u8], actual: i64)
    fn write(fd: i64, data: [u8]) -> (written: i64, status: i64)
    fn close(fd: i64) -> (status: i64)
    fn stat(path: str) -> (size: i64, kind: i64, status: i64)
}

@safe service vfs implements VfsProtocol {
    on open(path, flags) {
        let fd = fs_open(path, flags)
        reply(fd, 0)
    }
    on read(fd, len) {
        let data = fs_read(fd, len)
        reply(data, len(data))
    }
}
```

##### Sprint C3: Service Syntax & Lifecycle (Minggu 11-12)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| C3.1 | `service` block parsing | E5 | 6h | Top-level service declaration |
| C3.2 | `on` message handler | E6 | 6h | Pattern-match IPC dispatch |
| C3.3 | Auto IPC loop generation | E6 | 8h | recv → match → handler → reply |
| C3.4 | `init` / `shutdown` hooks | E5 | 4h | Service lifecycle |
| C3.5 | Health check protocol | E5 | 2h | Kernel can ping service |
| C3.6 | Tests: 20+ | AI | 4h | Service compiles, handles messages |

---

#### Workstream D: ML Runtime (Candle-Inspired)

**Referensi:** HuggingFace Candle arsitektur

##### Sprint D1: Multi-DType Tensor (Minggu 7-8)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| D1.1 | DType enum | E7 | 4h | F16, BF16, F32, F64, I8, U8, I32, I64 |
| D1.2 | Tensor storage per dtype | E7 | 12h | Internal buffer adapts to dtype |
| D1.3 | Dtype conversion | E8 | 4h | `tensor.to_f16()`, `tensor.to_i8()` |
| D1.4 | Shape type at compile time | E8 | 8h | `Tensor<F32, [3, 224, 224]>` |
| D1.5 | Tensor creation per dtype | E7 | 4h | `zeros::<F16>(3, 4)`, `ones::<I8>(10)` |
| D1.6 | Tests: 30+ | AI | 6h | All dtypes, conversions, ops |

**Syntax:**
```fajar
let x = Tensor::zeros::<F16>(3, 224, 224)    // FP16 image tensor
let w = Tensor::load::<I8>("model.bin")        // INT8 weights
let y = matmul(x.to_f32(), w.to_f32())        // compute in FP32
let result = y.to_f16()                        // convert back
```

##### Sprint D2: Device Backend Abstraction (Minggu 9-10)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| D2.1 | Device enum | E7 | 4h | `Device::Cpu`, `Device::Gpu(id)`, `Device::Npu` |
| D2.2 | Backend trait | E8 | 8h | `trait Backend { fn matmul(...); fn relu(...); }` |
| D2.3 | CPU backend | E7 | 4h | ndarray implementation (existing) |
| D2.4 | GPU backend (Adreno/Vulkan) | E8 | 16h | Vulkan compute for Q6A |
| D2.5 | NPU backend (Hexagon) | E8 | 12h | QNN SDK integration |
| D2.6 | Auto device selection | E7 | 4h | Best available backend |
| D2.7 | Tests: 25+ | AI | 6h | Same result across backends |

**Target devices:**
```
x86_64:   CPU (AVX2/AVX-512) + GPU (NVIDIA CUDA/RTX 4090)
ARM64:    CPU (NEON) + GPU (Adreno 643/Vulkan) + NPU (Hexagon 12 TOPS)
```

##### Sprint D3: Quantization & Model Loading (Minggu 11-12)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| D3.1 | GGUF format parser | E7 | 12h | Load llama.cpp models |
| D3.2 | Safetensors parser | E8 | 8h | Load HuggingFace models |
| D3.3 | Q4_0 / Q8_0 dequantize | E7 | 8h | Quantized matmul |
| D3.4 | INT8 symmetric quantize | E8 | 4h | Full-precision → INT8 |
| D3.5 | Model inference pipeline | E7 | 8h | Load → quantize → infer → output |
| D3.6 | Tests: 20+ | AI | 4h | GGUF load, quantized matmul accuracy |

---

#### Workstream E: Platform, Runtime & Testing

##### Sprint E1: User-Mode Runtime (Minggu 7-8)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| E1.1 | `fj_user_println` | E9 | 4h | Printf via SYS_WRITE syscall |
| E1.2 | `fj_user_exit` | E9 | 2h | Exit via SYS_EXIT |
| E1.3 | `fj_user_malloc/free` | E10 | 6h | Heap via SYS_BRK |
| E1.4 | `fj_user_ipc_*` wrappers | E9 | 8h | send/recv/call/reply via SYSCALL |
| E1.5 | Auto-link for x86_64-user | E10 | 4h | Compiler auto-links user runtime |
| E1.6 | Auto-link for aarch64-user | E10 | 4h | ARM64 user runtime |
| E1.7 | Tests: 25+ | AI | 6h | User programs compile and run |

##### Sprint E2: QEMU CI & Boot Tests (Minggu 9-10)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| E2.1 | Parse ALL 75 FajarOS x86 files | E9 | 8h | Every .fj file passes parser |
| E2.2 | Parse FajarOS ARM64 files | E10 | 4h | ARM64 boot code passes |
| E2.3 | QEMU x86_64 boot test | E9 | 8h | Kernel boots, serial output verified |
| E2.4 | QEMU aarch64 boot test | E10 | 8h | ARM64 kernel boots |
| E2.5 | GitHub Actions CI | E9 | 4h | Automated build + boot on every PR |
| E2.6 | Performance regression test | E10 | 4h | Compile speed tracked per commit |
| E2.7 | Tests: 30+ | AI | 6h | Boot verification, service communication |

##### Sprint E3: FajarOS Migration (Minggu 11-12)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| E3.1 | Migrate FajarOS x86 to multi-file build | E9 | 16h | Remove concatenation Makefile |
| E3.2 | Split monolith into kernel + 9 services | E10 | 16h | Separate ELFs |
| E3.3 | Migrate FajarOS ARM64 | E9 | 8h | Same structure for ARM64 |
| E3.4 | Verify 200+ shell commands | E10 | 8h | All commands work via IPC |
| E3.5 | Performance comparison | E9 | 4h | Monolith vs microkernel boot time |

---

### Phase 3: Advanced Features (Minggu 13-18)

##### Sprint F1: Complete Macro System (Minggu 13-14)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| F1.1 | `$` token in lexer | E5 | 2h | Dollar sign as capture prefix |
| F1.2 | Fragment specifiers | E5 | 8h | `$x:expr`, `$n:ident`, `$t:ty` |
| F1.3 | Repetition `$(...)*` | E6 | 12h | Zero-or-more pattern matching |
| F1.4 | Macro hygiene | E6 | 8h | Generated names don't conflict |
| F1.5 | Recursive expansion | E5 | 4h | Macros can invoke other macros |
| F1.6 | Tests: 30+ | AI | 6h | vec!, hashmap!, format! with repetition |

##### Sprint F2: Effect Polymorphism (Minggu 15-16)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| F2.1 | Effect variables in generics | E3 | 8h | `fn map<E>(f: Fn(A)->B with E)` |
| F2.2 | Effect inference | E4 | 8h | Auto-detect effects from body |
| F2.3 | Effect subtyping | E3 | 4h | `{IO}` is subtype of `{IO, Alloc}` |
| F2.4 | Tests: 20+ | AI | 4h | Effect-polymorphic functions |

##### Sprint F3: Async IPC (Minggu 17-18)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| F3.1 | `async fn ipc_recv()` | E9 | 8h | Non-blocking receive |
| F3.2 | Service event loop | E10 | 8h | `select! { msg, timer }` |
| F3.3 | Multi-client handling | E9 | 8h | N concurrent requests |
| F3.4 | Tests: 20+ | AI | 4h | Async service serves 2+ clients |

---

### Phase 4: Production Hardening (Minggu 19-24)

##### Sprint G1: Formal Verification Hooks (Minggu 19-20)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| G1.1 | `@requires(expr)` enforcement | E3 | 8h | Precondition checked at compile time |
| G1.2 | `@ensures(expr)` enforcement | E4 | 8h | Postcondition checked |
| G1.3 | `@invariant(expr)` for structs | E3 | 4h | Struct invariant maintained |
| G1.4 | Tests: 20+ | AI | 4h | Contract violations → error |

##### Sprint G2: Real-World ML Models (Minggu 21-22)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| G2.1 | TinyLLaMA inference | E7 | 16h | 1.1B model on Q6A NPU |
| G2.2 | YOLO-tiny object detection | E8 | 12h | Real-time on Adreno GPU |
| G2.3 | Whisper-tiny speech-to-text | E7 | 12h | Audio inference |
| G2.4 | Drone demo: YOLO + flight control | E8 | 8h | Update drone_controller.fj |
| G2.5 | Benchmark vs Candle | E7 | 4h | Performance comparison |

##### Sprint G3: FajarOS v3.0 Release (Minggu 23-24)

| # | Task | Owner | Effort | Detail |
|---|------|-------|--------|--------|
| G3.1 | FajarOS x86 v3.0 "Sovereignty" | E9 | 16h | Microkernel release |
| G3.2 | FajarOS ARM64 v3.0 | E10 | 16h | Q6A hardware verified |
| G3.3 | Documentation update | E5+E6 | 8h | Updated architecture docs |
| G3.4 | Blog post + demo video | E1 | 8h | Public announcement |
| G3.5 | Conference paper submission | E3 | 8h | EMSOFT/LCTES submission |
| G3.6 | Beta program launch | E9 | 4h | 5 external beta users |

---

## Bagian 3: Arsitektur Target

### FajarOS x86_64 Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe)        — Ring 3                         │
│   apps/compiler/main.fj     Self-hosted compiler             │
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
│ Microkernel (@kernel)       — Ring 0, ~2,500 LOC            │
│   kernel/core/mm.fj         Frame alloc, paging, heap        │
│   kernel/core/sched.fj      Process table, scheduler, SMP    │
│   kernel/core/ipc.fj        Synchronous IPC (seL4-style)     │
│   kernel/core/syscall.fj    SYSCALL/SYSRET entry, dispatch   │
│   kernel/core/boot.fj       Boot, IDT, TSS, panic            │
│   kernel/core/security.fj   Capabilities, limits             │
├─────────────────────────────────────────────────────────────┤
│ Hardware                    — Intel Core i9-14900HX           │
│   24 cores, RTX 4090, NVMe Gen4, 32GB DDR5                  │
└─────────────────────────────────────────────────────────────┘
```

### FajarOS ARM64 Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Applications (@safe)        — EL0                            │
│   AI inference, sensor fusion, navigation                    │
├─────────────────────────────────────────────────────────────┤
│ Services (@safe/@device)    — EL0, IPC                      │
│   services/npu/      Hexagon NPU inference (12 TOPS)         │
│   services/gpu/      Adreno 643 Vulkan compute               │
│   services/camera/   libcamera, 4K video pipeline            │
│   services/net/      WiFi + Ethernet                         │
│   services/sensor/   GPIO, I2C, SPI sensors                  │
├─────────────────────────────────────────────────────────────┤
│ Microkernel (@kernel)       — EL1                            │
│   kernel/core/       Same as x86 (arch-independent)          │
│   arch/aarch64/      GICv3, MMU, EL0/EL1 transition          │
│   bsp/dragon_q6a/   Board-specific: pinout, clocks, DTS     │
├─────────────────────────────────────────────────────────────┤
│ Hardware                    — Qualcomm QCS6490                │
│   Kryo 670 (8-core), Adreno 643, Hexagon 770, 7.4GB RAM     │
└─────────────────────────────────────────────────────────────┘
```

---

## Bagian 4: Candle Integration Strategy

### Apa yang Di-adopt dari Candle

| Candle Pattern | Fajar Lang Adaptation | Priority |
|---------------|----------------------|----------|
| `Device` enum (Cpu/Cuda/Metal) | `Device::Cpu / Device::Gpu(id) / Device::Npu` | HIGH |
| `DType` enum (f16/bf16/f32/f64) | Native dtype in tensor type system | HIGH |
| `Storage` backend per device | Per-device memory management | HIGH |
| GGUF quantization format | Load llama.cpp models on Q6A | HIGH |
| Safetensors format | Load HuggingFace models | MEDIUM |
| `candle-nn` layer pattern | Extend existing fj-nn package | MEDIUM |
| `candle-transformers` | Attention, rotary embedding patterns | MEDIUM |

### Apa yang TIDAK Di-adopt

| Candle Feature | Alasan Skip |
|---------------|-------------|
| Python bindings | Fajar Lang bukan Python ecosystem |
| Flash Attention v3 | Terlalu advanced; mulai dari basic attention |
| 80+ model implementations | Fokus 3-5 model berguna untuk embedded |
| MKL/Accelerate backend | Target ARM64 (Q6A), bukan desktop Intel |
| CUDA backend langsung | Q6A pakai Adreno (Vulkan) dan Hexagon (QNN) |

### Target Model Matrix

| Model | Size | Backend | Use Case | Platform |
|-------|------|---------|----------|----------|
| MNIST MLP | 100KB | CPU | Demo, testing | x86+ARM64 |
| YOLO-tiny | 15MB | GPU (Vulkan) | Object detection, drone | ARM64 (Q6A) |
| TinyLLaMA 1.1B | 600MB Q4 | NPU (Hexagon) | Text inference | ARM64 (Q6A) |
| Whisper-tiny | 75MB | CPU+NPU | Speech-to-text | ARM64 (Q6A) |
| MobileNet v2 | 14MB | GPU | Image classification | Both |

---

## Bagian 5: Quality Gates

### Per-Sprint Gate

```
□ cargo test — ALL pass (5,582+ dan bertambah)
□ cargo clippy -- -D warnings — ZERO warnings
□ cargo fmt -- --check — formatted
□ Acceptance criteria dari sprint terpenuhi
□ 20+ new tests per sprint minimum
□ FajarOS x86 files tetap parse (regression check)
□ FajarOS ARM64 files tetap parse (regression check)
```

### Per-Phase Gate

```
□ Phase 1: `fj build --all` menghasilkan kernel.elf + 3 service ELFs
           @safe → port_outb → SE020 compile error
           Context enforcement: 200+ test cases pass

□ Phase 2: Typed IPC compiles dan jalan di QEMU
           User-mode println works via SYS_WRITE
           Protocol definition generates client stub
           Tensor f16/f32 matmul works on CPU

□ Phase 3: Macro $patterns work, effect polymorphism works
           Async IPC serves 2+ clients
           GGUF model loads dan inference jalan

□ Phase 4: FajarOS x86 v3.0 boots (microkernel, separate services)
           FajarOS ARM64 v3.0 boots on Q6A
           TinyLLaMA inference on Hexagon NPU
           Conference paper submitted
```

### Release Criteria (v3.0)

```
□ ZERO concatenation — pure multi-file build
□ Kernel ≤ 2,500 LOC in Ring 0
□ 9+ services as separate ELFs
□ ALL @safe code CANNOT access hardware (0 bypass paths)
□ Typed IPC — wrong message type = compile error
□ Both x86_64 and ARM64 boot and run
□ 200+ shell commands work via IPC
□ At least 1 ML model runs on Q6A NPU
□ 6,500+ tests (current 5,582 + 1,000+ new)
□ Documentation complete (book updated)
```

---

## Bagian 6: Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| Multi-file build terlalu complex | Medium | High | Mulai 2 file dulu, expand gradual |
| FajarOS code breaks saat migration | High | Medium | Keep concatenation path as fallback |
| Async IPC terlalu ambisius | Medium | Low | Blocking IPC cukup untuk v3.0 |
| NPU backend (Hexagon) sulit | High | Medium | CPU fallback selalu available |
| Tim coordination overhead | Medium | Medium | Weekly sync + per-workstream CLAUDE.md |
| Capability types terlalu academic | Low | Low | Mulai sederhana, per-function annotation |
| ARM64 codegen bugs | Medium | High | Test setiap commit di QEMU aarch64 |
| Performance regression | Low | Medium | Benchmark CI per commit |

---

## Bagian 7: Metrik Sukses

| Metrik | Saat Ini | Target 6 Bulan | World-Class |
|--------|----------|----------------|-------------|
| Tests | 5,582 | 7,000+ | 15,000+ |
| FajarOS build | Concatenation | Multi-file | Single `fj build` |
| @safe enforcement | Partial | Complete (121+ blocked) | Formally verified |
| IPC type safety | Raw bytes | @message types | Protocol + verification |
| Tensor dtypes | f64 only | f16/bf16/f32/f64/i8 | Full Candle parity |
| ML models | MNIST MLP | 5 models | 20+ models |
| Target platforms | x86_64 | x86_64 + ARM64 | + RISC-V + Wasm |
| Production users | 0 | 3 beta | 10+ |
| Conference papers | 0 | 1 submitted | 1 accepted |
| Self-hosting | 1,268 LOC | 3,000 LOC | Full bootstrap |

---

*"Satu-satunya bahasa di mana OS kernel dan neural network bisa hidup di codebase yang sama, type system yang sama, dan compiler yang sama — dengan safety guarantees yang di-enforce oleh compiler melalui context annotations."*

*Target: x86_64 (Intel i9-14900HX) + ARM64 (Qualcomm QCS6490 / Radxa Dragon Q6A)*

*— Fajar Lang Master Implementation Plan v5.0*
