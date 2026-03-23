# Remaining Items Implementation Plan

> **Hardware:** Intel Core i9-14900HX, 32GB DDR5, RTX 4090 Laptop, QEMU 8.2.2, Vulkan 1.3
> **Method:** 1 Engineer + Claude AI
> **FajarOS x86 repo:** Available locally at /home/primecore/Documents/fajaros-x86/
> **Dragon Q6A:** Offline (skip NPU/hardware items)

---

## Prioritas

| # | Item | Impact | Effort | Why This Order |
|---|------|--------|--------|---------------|
| **1** | Windows CI fix | HIGH | 1h | Unblock green CI badge — visible credibility |
| **2** | FajarOS x86 QEMU boot | CRITICAL | 4h | Proves compiler actually produces bootable OS |
| **3** | FajarOS ARM64 QEMU boot | HIGH | 3h | Dual-platform claim verified |
| **4** | GPU Vulkan backend (RTX 4090) | HIGH | 8h | Real ML acceleration, leverages best hardware |
| **5** | Effect polymorphism | MEDIUM | 4h | Fills last type-system gap |

**Total: ~20h | 3-4 hari kerja**

---

## Item 1: Windows CI Fix (1 jam)

### Problem

Windows tests fail karena hardcoded `/tmp/` path di test files. Windows pakai `C:\Users\...\AppData\Local\Temp\`.

### Root Cause

```rust
// multifile_tests.rs — BROKEN on Windows:
let dir = format!("/tmp/fj-test-{name}");

// fajaros_regression_tests.rs — BROKEN on Windows:
const FAJAROS_DIR: &str = "/home/primecore/Documents/fajaros-x86";
```

### Fix

```rust
// Gunakan std::env::temp_dir() untuk cross-platform:
let dir = std::env::temp_dir().join(format!("fj-test-{name}"));

// FajarOS tests: skip gracefully jika path tidak ada
if !fajaros_exists() { return; }  // ← sudah ada, tapi path hardcoded Linux
```

### Tasks

| # | Task | File | Detail |
|---|------|------|--------|
| 1.1 | Replace `/tmp/` dengan `std::env::temp_dir()` | tests/multifile_tests.rs | 5 occurrences |
| 1.2 | Replace `/tmp/` di perf tests | tests/perf_tests.rs | temp_dir() |
| 1.3 | Replace `/tmp/` di initramfs tests | tests/initramfs_tests.rs | temp_dir() |
| 1.4 | Verify FAJAROS_DIR skip on non-Linux | tests/fajaros_regression_tests.rs | Already skips if not found |
| 1.5 | Verify: `RUSTFLAGS="-D warnings" cargo test` | Local | 0 errors |
| 1.6 | Push + wait CI | GitHub | Windows jobs green |

### Acceptance

```
□ CI green on windows-latest (stable + nightly)
□ CI green on ubuntu-24.04 (stable + nightly)
□ CI green on macos-14 (stable + nightly)
□ All Cross jobs still green
```

---

## Item 2: FajarOS x86 QEMU Boot (4 jam)

### Problem

Kita sudah buktikan 90/90 FajarOS files lex dan combined.fj parses. Tapi belum ada bukti bahwa compiler menghasilkan **bootable ELF** yang jalan di QEMU.

### Prerequisites

```
✅ FajarOS x86 repo: /home/primecore/Documents/fajaros-x86/
✅ QEMU: qemu-system-x86_64 8.2.2
✅ Compiler: fj build --target x86_64-none (Cranelift native)
✅ GRUB tools: grub-pc-bin, xorriso, mtools
```

### Tasks

| # | Task | Effort | Detail |
|---|------|--------|--------|
| 2.1 | Verify `make build` in fajaros-x86 | 30m | Existing Makefile produces kernel ELF |
| 2.2 | Verify `make run` boots in QEMU | 30m | Serial output "FajarOS Nova" appears |
| 2.3 | Capture boot serial output | 15m | `timeout 10 make run` → save output |
| 2.4 | Verify shell prompt appears | 15m | "nova>" prompt in serial |
| 2.5 | Test 5 basic commands | 30m | help, version, uptime, cpuid, mem |
| 2.6 | Test context enforcement at runtime | 30m | Verify @kernel fns actually work |
| 2.7 | Add QEMU boot test script | 30m | tools/test_qemu_boot.sh in fajar-lang |
| 2.8 | Add CI-compatible boot test | 30m | GitHub Actions with QEMU (optional) |
| 2.9 | Document boot results | 15m | Screenshot/log of successful boot |

### Verification Script

```bash
#!/bin/bash
# tools/test_qemu_boot.sh
set -e

FAJAROS_DIR="${FAJAROS_DIR:-/home/primecore/Documents/fajaros-x86}"

echo "=== Building FajarOS x86 ==="
cd "$FAJAROS_DIR"
make build

echo "=== Booting in QEMU (10s timeout) ==="
timeout 10 qemu-system-x86_64 \
    -kernel build/fajaros.elf \
    -nographic \
    -serial stdio \
    -no-reboot \
    -m 128M 2>&1 | tee /tmp/fajaros_boot.log || true

echo "=== Checking boot output ==="
if grep -q "FajarOS" /tmp/fajaros_boot.log; then
    echo "✅ BOOT SUCCESS: FajarOS banner detected"
else
    echo "❌ BOOT FAILED: No FajarOS banner found"
    exit 1
fi
```

### Acceptance

```
□ make build produces build/fajaros.elf
□ QEMU boots, serial shows "FajarOS Nova"
□ Shell prompt "nova>" appears
□ At least 5 shell commands produce output
□ Boot time < 2 seconds
□ test_qemu_boot.sh passes
```

---

## Item 3: FajarOS ARM64 QEMU Boot (3 jam)

### Problem

ARM64 target belum diverifikasi di QEMU aarch64. FajarOS punya `arch/aarch64/boot.fj` tapi belum pernah di-boot di environment ini.

### Prerequisites

```
✅ QEMU: qemu-system-aarch64 8.2.2
✅ Cross-compiler: fj build --target aarch64-unknown-none
✅ FajarOS ARM64 code: fajaros-x86/arch/aarch64/
```

### Tasks

| # | Task | Effort | Detail |
|---|------|--------|--------|
| 3.1 | Check existing ARM64 build in fajaros-x86 | 30m | Does `make build-arm64` exist? |
| 3.2 | Cross-compile ARM64 kernel | 45m | `fj build --target aarch64-unknown-none` |
| 3.3 | Create QEMU aarch64 launch script | 30m | virt machine, GIC, serial |
| 3.4 | Boot test aarch64 | 30m | Serial output verification |
| 3.5 | Document ARM64 boot results | 15m | Capture serial log |
| 3.6 | Add to test script | 15m | tools/test_qemu_arm64.sh |

### QEMU ARM64 Command

```bash
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a72 \
    -m 256M \
    -kernel build/fajaros-arm64.elf \
    -nographic \
    -serial stdio
```

### Acceptance

```
□ ARM64 kernel compiles without error
□ QEMU aarch64 boots (or clear error if code needs fixes)
□ Serial output visible
□ Boot test script works
```

---

## Item 4: GPU Vulkan Backend — RTX 4090 (8 jam)

### Problem

`Device::Gpu` falls back to CPU. RTX 4090 dengan Vulkan 1.3 tersedia — ini GPU paling powerful di laptop ini.

### Architecture

```
@device fn inference(input: Tensor) -> Tensor with Tensor {
    matmul(input, weights)     // → dispatches to VulkanBackend
}

VulkanBackend                   CpuBackend (fallback)
├── ash crate (Vulkan bindings) ├── ndarray
├── Compute shader (GLSL→SPIR-V)├── Pure Rust
├── Device memory management    ├── System memory
└── RTX 4090 (16384 CUDA cores) └── CPU (24 cores)
```

### Prerequisites

```
✅ Vulkan 1.3.275 installed
✅ RTX 4090 available (Compute Cap 8.9)
✅ ash crate (already in Cargo.toml as optional dep)
✅ TensorBackend trait defined (Sprint 11)
```

### Tasks

| # | Task | Effort | Detail |
|---|------|--------|--------|
| 4.1 | Verify Vulkan device detection | 30m | List physical devices, check compute queue |
| 4.2 | Create VulkanBackend struct | 1h | Instance, device, queue, command pool |
| 4.3 | Write GLSL matmul shader | 1h | matrix_multiply.comp → SPIR-V |
| 4.4 | Implement buffer upload/download | 1h | Host → device → host memory transfer |
| 4.5 | Implement matmul via compute shader | 1.5h | Dispatch compute, read result |
| 4.6 | Implement relu/softmax/sigmoid shaders | 1h | Element-wise compute shaders |
| 4.7 | Wire into TensorBackend trait | 30m | VulkanBackend implements TensorBackend |
| 4.8 | Auto-detect GPU in Device::best_available() | 30m | Probe Vulkan, return Device::Gpu(0) |
| 4.9 | Benchmark: GPU vs CPU matmul | 30m | 1000x1000 matrix, compare times |
| 4.10 | Tests: 15+ | 30m | GPU matmul result == CPU result |

### Key Code: Matmul Compute Shader

```glsl
// shaders/matmul.comp
#version 450
layout(local_size_x = 16, local_size_y = 16) in;

layout(binding = 0) readonly buffer A { float a[]; };
layout(binding = 1) readonly buffer B { float b[]; };
layout(binding = 2) writeonly buffer C { float c[]; };

layout(push_constant) uniform Params {
    uint M, K, N;
};

void main() {
    uint row = gl_GlobalInvocationID.y;
    uint col = gl_GlobalInvocationID.x;
    if (row >= M || col >= N) return;

    float sum = 0.0;
    for (uint i = 0; i < K; i++) {
        sum += a[row * K + i] * b[i * N + col];
    }
    c[row * N + col] = sum;
}
```

### Performance Target

| Operation | CPU (ndarray) | GPU (RTX 4090) | Speedup |
|-----------|-------------|----------------|---------|
| matmul 256×256 | ~2ms | ~0.1ms | 20x |
| matmul 1000×1000 | ~50ms | ~0.5ms | 100x |
| matmul 4096×4096 | ~5s | ~10ms | 500x |
| relu 1M elements | ~1ms | ~0.01ms | 100x |

### Acceptance

```
□ Vulkan device detected on RTX 4090
□ matmul via compute shader produces correct result
□ relu/softmax via compute shader correct
□ Device::best_available() returns Gpu(0)
□ GPU result matches CPU result (within f32 tolerance)
□ Benchmark shows significant speedup for large matrices
□ Fallback to CPU when Vulkan unavailable
```

---

## Item 5: Effect Polymorphism (4 jam)

### Problem

Satu-satunya unchecked item dari WORLD_CLASS_PLAN:
> "Effect polymorphism: generic over effects"

### What It Means

```fajar
// CURRENT: effect harus di-specify per function
fn map_io(f: fn(i64) -> i64, x: i64) -> i64 with IO { f(x) }
fn map_pure(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }

// GOAL: generic over effects
fn map<E>(f: fn(i64) -> i64 with E, x: i64) -> i64 with E { f(x) }
//     ^ effect variable E — inferred from f's effect
```

### Architecture

```
Parser:    GenericParam { name: "E", is_effect: true }
Analyzer:  resolve E from actual argument's effect set
Codegen:   monomorphize per-effect-set (like type generics)
```

### Tasks

| # | Task | Effort | Detail |
|---|------|--------|--------|
| 5.1 | Add `is_effect` to GenericParam | 15m | AST extension |
| 5.2 | Parse effect params in generics: `<E: Effect>` | 30m | After `is_comptime` check |
| 5.3 | Effect variable in `with` clause | 30m | `with E` resolves to effect var |
| 5.4 | Effect inference from arguments | 1h | If arg has `with IO`, E = IO |
| 5.5 | Effect substitution in type checker | 1h | Replace E with concrete effects |
| 5.6 | Tests: 20+ | 30m | Polymorphic map, filter, fold |
| 5.7 | Update WORLD_CLASS_PLAN checkbox | 5m | Mark as [x] |

### Acceptance

```
□ fn map<E>(f: fn(A)->B with E) with E — parses
□ Calling map with IO function → map infers E=IO
□ Calling map with pure function → map infers E={}
□ Effect mismatch detected at call site
□ 20+ tests pass
□ WORLD_CLASS_PLAN: 0 unchecked items
```

---

## Timeline

```
Hari 1 (4 jam):
  09:00-10:00  Item 1: Windows CI fix (push, verify green)
  10:00-12:00  Item 2: FajarOS x86 QEMU boot (build + test)
  13:00-15:00  Item 2 continued + Item 3: ARM64 QEMU boot

Hari 2 (8 jam):
  09:00-17:00  Item 4: GPU Vulkan backend (full day)
               - Morning: Vulkan setup + shader compilation
               - Afternoon: matmul implementation + benchmark

Hari 3 (4 jam):
  09:00-13:00  Item 5: Effect polymorphism
               - Parser + type checker + tests

Hari 4 (2 jam, buffer):
  09:00-11:00  Fix any issues from Hari 1-3
               Final push + CI verification
               Update all documentation
```

---

## Effort Summary

| Item | Effort | Tests | Impact |
|------|--------|-------|--------|
| 1. Windows CI fix | 1h | 0 (fix existing) | CI badge green |
| 2. FajarOS x86 QEMU | 4h | +5 (boot script) | Proves compiler works for OS |
| 3. FajarOS ARM64 QEMU | 3h | +3 (boot script) | Dual-platform proven |
| 4. GPU Vulkan backend | 8h | +15 (GPU tests) | Real ML acceleration |
| 5. Effect polymorphism | 4h | +20 (type system) | Last type-system gap filled |
| **TOTAL** | **20h** | **+43** | **All achievable items done** |

---

## After Completion: Updated Status

```
Before:                          After:
CI: ❌ Windows failing           CI: ✅ All green (except FajarOS*)
QEMU x86: ❌ Not tested          QEMU x86: ✅ Boots, shell works
QEMU ARM64: ❌ Not tested        QEMU ARM64: ✅ Boots in QEMU
GPU backend: ❌ CPU fallback      GPU backend: ✅ RTX 4090 Vulkan compute
Effect poly: ❌ Not implemented   Effect poly: ✅ Generic over effects
Q6A NPU: ❌ Not reachable        Q6A NPU: ❌ Still needs Q6A online

* FajarOS CI needs fajaros-x86 as submodule — separate task
```

---

## Remaining After This (Needs Q6A Online)

| Item | When Q6A Available |
|------|-------------------|
| TinyLLaMA on Hexagon NPU | SSH to Q6A, install QNN SDK, run inference |
| Adreno GPU backend | Test Vulkan compute on Adreno 643 |
| Hardware GPIO/I2C/SPI | Physical pin testing |
| FajarOS on real hardware | Flash and boot on Q6A |

These items are **blocked until Dragon Q6A is back online** (WiFi/Ethernet reconnect).

---

*Plan version: 1.0 | Date: 2026-03-23*
*Hardware: Lenovo Legion Pro — i9-14900HX, RTX 4090, 32GB DDR5*
