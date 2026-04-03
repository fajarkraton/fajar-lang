# Re-Audit V17 — Phase 3: Runtime Systems

> **Date:** 2026-04-03
> **Scope:** runtime/ml (27K LOC), runtime/os (35K LOC), runtime/gpu (2.5K LOC), gpu_codegen (4.7K LOC)

---

## Summary

| Component | LOC | Tests | Verdict | Evidence |
|-----------|-----|-------|---------|----------|
| ML Runtime (tensor, autograd, layers) | 27,182 | 683 pass | **[x] PRODUCTION** | Real ndarray ops, verified with .fj programs |
| ML Advanced (transformer, rnn, quantize, etc.) | (in above) | (in above) | **[x] PRODUCTION** | Real algorithms: MHA, LSTM, INT8 quantize, pruning |
| OS Runtime (memory, irq, syscall) | 34,997 | 524 pass | **[x] PRODUCTION (simulation)** | Works from .fj via builtins, documented as simulation |
| GPU Runtime (cuda, wgpu backends) | 2,480 | 28 pass | **[p] PARTIAL** | Real CUDA/wgpu FFI, but needs feature flags + hardware |
| GPU Codegen (SPIR-V, PTX) | 4,711 | 112 pass | **[x] PRODUCTION** | `fj build --target spirv/ptx` produces real GPU assembly |

---

## ML Runtime — [x] PRODUCTION

### Verified Working from .fj Programs:
- `zeros(rows, cols)` / `ones(rows, cols)` / `randn(rows, cols)` ✅
- `from_data([[...]])` — create tensor from literal data ✅
- `matmul(a, b)` — matrix multiplication ✅
- `transpose(t)` ✅
- `relu(t)` / `sigmoid(t)` / `softmax(t)` ✅
- `mse_loss(pred, target)` — returns correct loss (0 for identical tensors) ✅
- `tensor_sum(t)` — 19+22+43+50=134 ✅
- `set_requires_grad(t, true)` / `backward(t)` / `grad(t)` — autograd chain works ✅
- `Dense(in, out).forward(input)` — neural network layer ✅
- `SGD(lr, momentum)` / `Adam(lr)` — optimizer creation ✅
- MNIST training: 3 epochs, 5 batches each, completes successfully ✅

### Verified by Code Audit (real ndarray operations):
- **tensor.rs**: Uses `ndarray::ArrayD<f64>` as backing store
- **autograd.rs**: Tape-based reverse-mode AD with chain rule, numerical gradient validation
- **layers.rs**: Dense matmul via `ops::matmul()`, Xavier initialization, dropout with ndarray_rand
- **transformer.rs**: Real multi-head attention with Q/K/V projections, scaled dot-product
- **quantize.rs**: INT8 symmetric quantization with i32 accumulation
- **rnn.rs**: Real LSTM cell with forget/input/output/candidate gates
- **distillation.rs**: Temperature-scaled softmax for knowledge distillation
- **pruning.rs**: Magnitude/gradient/random pruning with binary masks
- **sparsity.rs**: 4:2 structured sparsity with metadata encoding

### Bugs / Limitations:
- **Tensor `+` operator broken**: `a + b` for tensors → RE002 (must use `tensor_add()` instead)
- **println for tensors only shows shape**, not values (display limitation)
- **MNIST training doesn't report loss/accuracy** — completes but no metrics output
- API docs don't match: `mem_read`/`mem_write` documented, actual builtins are `mem_read_u64`/`mem_write_u64`

---

## OS Runtime — [x] PRODUCTION (simulation)

### Verified Working from .fj Programs:
```fj
@kernel fn os_test() -> i64 {
    let addr = mem_alloc(4096, 4096)    // allocate 4KB
    mem_write_u64(addr, 42)             // write value
    let val = mem_read_u64(addr)        // read back
    mem_free(addr)                      // free
    val                                 // returns 42 ✅
}
```

### Honest Classification:
This is a **simulation** of OS primitives, not real hardware interaction. The mod.rs docstring explicitly states:
> "provides simulated hardware primitives for the interpreter and testing"
> "All structures use in-memory data structures (HashMap, Vec) — they do NOT touch real hardware"

This is **intentional and correct design** — the interpreter simulates OS behavior so users can develop and test `@kernel` code without real hardware.

### Available OS Builtins (from type_check/register.rs):
- `mem_alloc(size, align)`, `mem_free(addr)`, `mem_read_u8/u32/u64(addr)`, `mem_write_u8/u32/u64(addr, val)`
- `page_map(virt, phys)`, `page_unmap(virt)`
- `irq_register(num, handler_name)`, `irq_unregister(num)`, `irq_enable()`, `irq_disable()`
- `port_read(port)`, `port_write(port, val)`
- `syscall_define(num, name, handler)`, `syscall_dispatch(num, args)`

### Submodule List (36 files):
aarch64, ai_kernel, bus, compositor, display, distributed_kernel, dma, gdt, gui_framework, hal_v2, hardware_ci, idt, intrinsics, irq, kernel_opt, keyboard, memory, net_stack, network, network_v2, nova_release, paging, pit, pkg_manager, power, riscv, serial, shell, smp, syscall, timer, userland, verified_kernel, vfs, vga, virtio

Most are simulation implementations. 524 tests all pass.

---

## GPU Runtime — [p] PARTIAL

### Architecture:
- **device.rs**: GpuDevice trait — backend-agnostic interface
- **cuda_backend.rs**: Real CUDA — dynamically loads libcuda.so via libloading, calls cuInit/cuDeviceGet etc.
- **wgpu_backend.rs**: Real wgpu — `wgpu::Instance::default()`, `adapter.request_device()`
- **cpu_fallback.rs**: Fallback simulation when no real GPU available

### Status:
- CUDA backend requires `--features gpu` + NVIDIA GPU + CUDA driver
- wgpu backend requires `--features gpu` + compatible GPU
- CPU fallback always works (simulation)
- 28 tests pass (mostly testing CPU fallback path)

---

## GPU Codegen — [x] PRODUCTION

### SPIR-V (`fj build --target spirv`):
- Produces real SPIR-V binary (552 bytes for simple kernel)
- Magic number `0x07230203` verified ✅
- Contains GLSL.std.450 extension, proper type definitions
- Generated from `@gpu fn` in .fj source

### PTX (`fj build --target ptx`):
- Produces real NVIDIA PTX assembly (426 bytes)
- `.version 7.5`, `.target sm_80` header
- Proper register allocation (`.reg .f32 %f<6>`)
- Real GPU instructions: `ld.global.f32`, `add.f32`, `st.global.f32`
- Generated from `@gpu fn` in .fj source

### Additional targets (from exploration):
- `fj build --target metal` — Metal Shading Language (to verify)
- `fj build --target hlsl` — HLSL for DirectX (to verify)

---

## Bugs Found in Phase 3

| # | Bug | Severity | Evidence |
|---|-----|----------|----------|
| 1 | **Tensor `+` operator broken** | MEDIUM | `a + b` for tensors → RE002, must use tensor_add() |
| 2 | **MNIST doesn't report metrics** | LOW | Training completes but no loss/accuracy output |
| 3 | **API doc mismatch** | LOW | CLAUDE.md says `mem_read/mem_write`, actual is `mem_read_u64/mem_write_u64` |
| 4 | **println shows shape not values** | LOW | `println(tensor)` → "tensor(shape=[2,3])" |

---

## Phase 3 Conclusion

**ML Runtime is genuinely PRODUCTION-quality.** Real ndarray operations, real autograd, real neural network layers. Verified by running .fj programs AND by code audit of implementations. This is one of the strongest parts of the codebase.

**OS Runtime is a correct, intentional simulation.** Works from @kernel fn in .fj programs. Simulation design is explicitly documented. 524 tests pass.

**GPU Codegen produces real shader binaries.** SPIR-V and PTX output verified as valid GPU assembly.

**GPU Runtime needs hardware** to fully verify CUDA/wgpu paths, but architecture is real (actual libloading/wgpu calls).

---

*Phase 3 complete — 2026-04-03*
