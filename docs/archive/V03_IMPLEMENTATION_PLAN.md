# Implementation Plan — Fajar Lang v0.3 "Dominion"

> **Codename:** Dominion — "The release where Fajar Lang becomes a real contender"
> **Goal:** Close ALL critical gaps for OS kernel development AND AI/ML infrastructure
> **Timeline:** 12 months (52 sprints), starting from v0.2 complete baseline
> **Baseline:** 1,991 tests | 59,419 LOC | Phases A-F + E complete
> **Current (2026-03-09):** 2,209 tests (1,836 lib + 373 integration) | ~75K LOC | S1-S8 + parts of S9,S13-S18,S31-S37,S41-S42 complete
> **Gap audit (2026-03-09):** 8 P0 gaps (S9.2 ✅ done), 15+ P1 gaps, 6 sprints with deferred subtasks — see §12

---

## 1. Vision & Success Criteria

### 1.1 The Bar We Must Clear

Fajar Lang claims to be "the only language where an OS kernel and a neural network can share the same codebase." To make that real:

**OS Domain (Linux/Windows/iOS kernel-grade):**
- Write a minimal bootable kernel that handles interrupts, paging, and a simple shell
- Cross-compile to ARM64/RISC-V bare metal with real hardware verification
- Concurrency primitives that can implement a preemptive scheduler
- Inline assembly for architecture-specific bootstrap and critical paths

**ML Domain (PyTorch/TensorFlow/JAX-grade):**
- Train a CNN on MNIST end-to-end in native codegen (not just interpreter)
- GPU-accelerated tensor ops via Vulkan compute or CUDA FFI
- Export trained models to ONNX format
- Distributed training across multiple processes

### 1.2 Success Metrics

| Metric | Current (v0.2) | Target (v0.3) |
|--------|---------------|---------------|
| Tests | 2,209 (1,836 lib + 373 integ) | 4,000+ |
| LOC | ~75,000 (was 59K) | ~120,000 |
| Native examples working | 6/15 | 30+ |
| Benchmark: fib(30) native | 3.4ms | <2ms |
| Benchmark: matmul 256x256 | N/A (interp) | <50ms (GPU) |
| Real bare-metal boot | No | Yes (QEMU + RPi4) |
| MNIST training native | No | Yes (<30s) |
| Self-hosting | No | Partial (lexer) |
| Concurrency model | None | Async + threads |

---

## 2. Architecture Evolution

### 2.1 New Modules Required

```
src/
+-- concurrency/                 <- NEW: v0.3
|   +-- mod.rs                   <- Thread, Channel, Mutex, Atomic types
|   +-- thread.rs                <- Thread spawn, join, park/unpark
|   +-- channel.rs               <- MPSC channels (bounded + unbounded)
|   +-- sync.rs                  <- Mutex, RwLock, Condvar, Barrier
|   +-- atomic.rs                <- AtomicI32/I64/Bool, Ordering, CAS
|   +-- async_rt.rs              <- Async runtime (executor, waker, Future)
|
+-- codegen/
|   +-- cranelift.rs             <- Existing (refactor: split into submodules)
|   +-- gpu/                     <- NEW: GPU compute backend
|   |   +-- mod.rs               <- GPU trait abstraction
|   |   +-- vulkan.rs            <- Vulkan compute shader backend
|   |   +-- cuda_ffi.rs          <- CUDA via FFI (libcuda.so)
|   +-- intrinsics.rs            <- NEW: Inline asm, volatile, SIMD
|   +-- linker.rs                <- Existing (extend: real linker integration)
|
+-- runtime/
|   +-- ml/
|   |   +-- distributed.rs       <- NEW: Multi-process training
|   |   +-- onnx.rs              <- NEW: ONNX export
|   |   +-- gpu_tensor.rs        <- NEW: GPU-backed tensor ops
|   |   +-- data_loader.rs       <- NEW: Batched data pipeline
|   +-- os/
|   |   +-- volatile.rs          <- NEW: Volatile read/write
|   |   +-- asm.rs               <- NEW: Inline assembly support
|   |   +-- allocator.rs         <- NEW: Custom allocator trait
|   |   +-- thread.rs            <- NEW: OS thread primitives
```

### 2.2 Dependency Graph (Extended)

```
ALLOWED (v0.3 additions):
  concurrency -> runtime/os (thread primitives)
  codegen/gpu -> runtime/ml (tensor bridge)
  codegen/intrinsics -> codegen/cranelift (IR emission)
  runtime/ml/distributed -> concurrency (multi-thread coordination)
  runtime/ml/gpu_tensor -> codegen/gpu (compute dispatch)

FORBIDDEN (unchanged):
  lexer -> parser (no upward deps)
  parser -> interpreter
  runtime/os <-> runtime/ml (siblings)
  Any cycle
```

### 2.3 Cranelift.rs Refactoring Plan

The current `cranelift.rs` is 17,241 lines. Before adding more features, it MUST be split:

```
src/codegen/
+-- cranelift/
|   +-- mod.rs              <- CraneliftCompiler + ObjectCompiler structs, compile_program
|   +-- context.rs          <- CodegenCtx struct + all fields
|   +-- expr.rs             <- compile_expr, compile_binop, compile_unary, compile_call
|   +-- stmt.rs             <- compile_let, compile_assign, compile_return
|   +-- control.rs          <- compile_if, compile_while, compile_for, compile_match, compile_loop
|   +-- types.rs             <- type lowering, infer_expr_type (move from codegen/types.rs)
|   +-- strings.rs          <- compile_string_*, string runtime functions
|   +-- arrays.rs           <- compile_array_*, array runtime functions
|   +-- structs.rs          <- compile_struct_init, compile_field_access, compile_field_assign
|   +-- enums.rs            <- compile_enum, compile_match patterns
|   +-- closures.rs         <- scan_closures, collect_free_vars, closure compilation
|   +-- generics.rs         <- monomorphize, specialize_fndef, collect_generic_calls
|   +-- builtins.rs         <- compile_math_builtin, compile_io_builtin, format args
|   +-- runtime_fns.rs      <- All extern "C" fj_rt_* functions
|   +-- tests.rs            <- All #[cfg(test)] tests
```

**This refactor is Sprint 0 — must be done before ANY new v0.3 work.**

---

## 3. Phase Structure (12 Months)

```
Quarter 1 (Month 1-3): CONCURRENCY + REFACTORING
  Month 1: Cranelift refactor + Function pointers + HashMap native
  Month 2: Thread model + Channels + Sync primitives
  Month 3: Async/await + Future trait + Executor

Quarter 2 (Month 4-6): OS KERNEL FEATURES
  Month 4: Inline assembly + Volatile MMIO + Custom allocator
  Month 5: Linker scripts + Bare metal boot + Interrupt tables
  Month 6: Kernel demo (QEMU) + Real hardware test (RPi4)

Quarter 3 (Month 7-9): GPU + ML INFRASTRUCTURE
  Month 7: GPU abstraction + Vulkan compute + CUDA FFI
  Month 8: GPU tensor ops + Distributed training
  Month 9: ONNX export + Data pipeline + MNIST native

Quarter 4 (Month 10-12): PRODUCTION + SELF-HOSTING
  Month 10: Optimization passes + SIMD + Benchmarks
  Month 11: Self-hosting (lexer in .fj) + Bootstrap
  Month 12: Documentation + Release + Real-world demos
```

---

## 4. Sprint Breakdown (52 Sprints)

### Quarter 1 — Concurrency & Refactoring

#### Month 1: Foundation Refactoring (S1-S4)

**Sprint 1: Cranelift Module Split** `P0` `CRITICAL` ✅ COMPLETE
> Split 17,241-line cranelift.rs into 12-file module structure
> S1.1, S1.4-S1.8 done. S1.2 (expr.rs), S1.3 (stmt.rs) deferred — remain in compile/mod.rs

**Sprint 2: Function Pointers & Higher-Order Functions** `P0` ✅ COMPLETE (partial)
> S2.1, S2.2, S2.3, S2.4, S2.5, S2.7 done. S2.6 (returning closures) deferred.
> fn pointer type checking in analyzer added (S2.2 analyzer).

**Sprint 3: HashMap in Native Codegen** `P1` ✅ COMPLETE (partial)
> S3.1-S3.4 done. Keys/values iteration deferred (needs array return ABI).

**Sprint 4: Remaining Parity Gaps** `P1` ✅ COMPLETE (partial)
> S4.1-S4.6, S4.9-S4.10 done. S4.7 (multi-type generics), S4.8 (string/struct generics) open.
- S4.4 — Array `.first()`, `.last()` returning Option enum
- S4.5 — Array `.join(sep)` with heap string allocation
- S4.6 — String `.split()` returning heap array of strings
- S4.7 — Multi-type-param generics `<T, U>`
- S4.8 — String/struct monomorphization
- S4.9 — Module system in native (inline `mod`, `use` imports)
- S4.10 — 20 tests across all features

---

#### Month 2: Thread Model (S5-S8)

**Sprint 5: Thread Primitives** `P0` `CRITICAL`
> Core threading: spawn, join, shared state

**STATUS:** S5.1-S5.5, S5.8 done. S5.6 (TLS), S5.7 (Send/Sync) open.

**Sprint 6: Synchronization Primitives** `P0` ✅ COMPLETE
> S6.1-S6.6 all done: Mutex, RwLock, Condvar, Barrier, context integration, integration tests.

**Sprint 7: Channels** `P1` ✅ COMPLETE (partial)
> S7.1-S7.3, S7.5 done. S7.4 (select!) deferred (needs macro system).

**Sprint 8: Atomic Operations** `P0` ✅ COMPLETE
> All done: Atomic new/load/store/add/sub/and/or/xor/cas, fence, spinlock/counter algorithms.

---

#### Month 3: Async/Await (S9-S13)

**Sprint 9: Future Trait & State Machines** `P0`
> Core async abstraction

- S9.1 — `Future` trait: `fn poll(self, cx: &mut Context) -> Poll<T>`
- S9.2 — `Poll` enum: `Ready(T)` | `Pending`
- S9.3 — `async fn` desugaring: transform to state machine enum
- S9.4 — `await` expression: poll loop with waker registration
- S9.5 — Parser: `async` keyword before `fn`, `.await` postfix operator
- S9.6 — Analyzer: async functions return `Future<T>`, type-check `.await`
- S9.7 — 8 tests: async_fn_basic, await_ready, await_pending, async_return_type

**Sprint 10: Async Runtime (Executor)** `P1`
> Single-threaded executor with I/O integration

- S10.1 — `Executor` struct: task queue, run loop
- S10.2 — `Waker` implementation: wake-by-notify mechanism
- S10.3 — `spawn(future) -> JoinHandle` — submit task to executor
- S10.4 — `block_on(future) -> T` — run future to completion
- S10.5 — Timer future: `sleep(duration).await`
- S10.6 — I/O future: `async fn read_file(path) -> String`
- S10.7 — 8 tests: executor_basic, spawn_join, timer_sleep, concurrent_tasks

**Sprint 11: Multi-Threaded Async** `P2`
> Work-stealing executor for parallel async tasks

- S11.1 — Thread pool executor: N worker threads
- S11.2 — Work-stealing queue: local + shared task queues
- S11.3 — `spawn` distributes across threads
- S11.4 — `JoinHandle.await` — cross-thread result retrieval
- S11.5 — Cancellation: `JoinHandle.abort()` — cooperative cancel via flag
- S11.6 — 8 tests: multi_thread_spawn, work_stealing, cancel_task, concurrent_io

**Sprint 12: Async Channels & Streams** `P2`
> Async-compatible channel primitives

- S12.1 — `async_channel::new() -> (AsyncSender, AsyncReceiver)`
- S12.2 — `AsyncSender.send(val).await` — async send
- S12.3 — `AsyncReceiver.recv().await -> Option<T>` — async receive
- S12.4 — `Stream` trait: `fn poll_next(cx) -> Poll<Option<T>>`
- S12.5 — `for await item in stream { }` — async iteration
- S12.6 — 8 tests: async_channel, stream_basic, for_await, channel_close

**Sprint 13: Concurrency Hardening & Integration** `P1`
> End-to-end concurrency tests, safety audit

- S13.1 — Race condition tests: shared counter with atomics vs mutex
- S13.2 — Deadlock detection integration test
- S13.3 — Borrow checker for concurrent code: `Send`/`Sync` enforcement
- S13.4 — Context annotations: `@kernel` + threads (allowed), `@device` + threads (allowed)
- S13.5 — Performance benchmark: channel throughput, mutex contention, atomic ops
- S13.6 — Documentation: concurrency chapter in mdBook
- S13.7 — 10 integration tests: producer_consumer, parallel_sum, async_web_server_mock

---

### Quarter 2 — OS Kernel Features

#### Month 4: Low-Level Primitives (S14-S17)

**Sprint 14: Inline Assembly** `P0` `CRITICAL`
> Direct hardware interaction capability

- S14.1 — Parser: `asm!("instruction", in("reg") val, out("reg") result)` syntax
- S14.2 — AST node: `Expr::InlineAsm { template, operands, clobbers, options }`
- S14.3 — Analyzer: validate operand types, check `@kernel`/`@unsafe` context
- S14.4 — Cranelift codegen: `InlineAsm` node → Cranelift inline_asm or raw bytes
- S14.5 — Output operands: `out("rax") result` → read from register after asm
- S14.6 — Input operands: `in("rdi") value` → write to register before asm
- S14.7 — Clobber list: `clobber("memory", "cc")` — register/memory clobbers
- S14.8 — `global_asm!("section .text\n...")` — module-level assembly blocks
- S14.9 — Architecture-specific asm: x86_64, aarch64, riscv64 variants
- S14.10 — 10 tests: nop, mov, read_cr0 (x86), cpuid, memory_barrier, global_asm

**Sprint 15: Volatile & Memory-Mapped I/O** `P0`
> Safe hardware register access

- S15.1 — `volatile_read<T>(addr: *const T) -> T` intrinsic
- S15.2 — `volatile_write<T>(addr: *mut T, value: T)` intrinsic
- S15.3 — Cranelift: `MemFlags::new().with_notrap().with_aligned()` + volatile flag
- S15.4 — `VolatilePtr<T>` wrapper type: safe abstraction for MMIO regions
- S15.5 — `MmioRegion` struct: base address + size + access permissions
- S15.6 — Analyzer: volatile ops only in `@kernel`/`@unsafe` context
- S15.7 — Fence intrinsics: `compiler_fence()`, `memory_fence()`
- S15.8 — 8 tests: volatile_read_write, mmio_region, fence_ordering, context_check

**Sprint 16: Custom Allocator** `P0`
> Pluggable memory allocator for bare-metal environments

- S16.1 — `Allocator` trait: `fn allocate(layout: Layout) -> *mut u8`
- S16.2 — `Allocator` trait: `fn deallocate(ptr: *mut u8, layout: Layout)`
- S16.3 — `Layout` struct: `size: usize`, `align: usize`
- S16.4 — `#[global_allocator]` attribute for setting system allocator
- S16.5 — `BumpAllocator`: simple bump allocator for kernel early boot
- S16.6 — `FreeListAllocator`: general-purpose allocator with free list
- S16.7 — `PoolAllocator`: fixed-size block allocator for specific object sizes
- S16.8 — Runtime integration: replace `malloc`/`free` with allocator trait calls
- S16.9 — 10 tests: bump_alloc, free_list_alloc, pool_alloc, custom_global, alignment

**Sprint 17: Bare Metal Panic & Entry** `P1`
> no_std support for real bare-metal execution

- S17.1 — `#[panic_handler]` attribute: custom panic handler function
- S17.2 — `#[entry]` attribute: bare-metal entry point (no main)
- S17.3 — `#[no_std]` module attribute: disable standard library
- S17.4 — `core` library subset: types and traits available without std
- S17.5 — Panic handler codegen: call user-defined handler instead of abort
- S17.6 — Entry point codegen: emit `_start` symbol with custom prologue
- S17.7 — 6 tests: panic_handler_called, entry_point, no_std_compiles, core_subset

---

#### Month 5: Kernel Infrastructure (S18-S21)

**Sprint 18: Linker Script Support** `P0`
> Memory layout control for embedded targets

- S18.1 — Linker script parser: `MEMORY { }`, `SECTIONS { }`, `ENTRY()`
- S18.2 — `fj.toml` integration: `[target.aarch64-unknown-none]` with `linker-script`
- S18.3 — Object file sections: `.text`, `.rodata`, `.data`, `.bss`, `.stack`
- S18.4 — `#[section(".isr_vector")]` attribute for placing functions in sections
- S18.5 — `#[link_section = ".data"]` for placing static data
- S18.6 — Cranelift object backend: emit proper ELF sections
- S18.7 — Integration with `ld` / `lld` for final linking
- S18.8 — 6 tests: section_placement, memory_layout, entry_point, custom_section

**Sprint 19: Interrupt Descriptor Table** `P1`
> Real interrupt handling for kernel mode

- S19.1 — `InterruptDescriptorTable` struct with 256 entries
- S19.2 — `#[interrupt]` attribute for interrupt handler functions
- S19.3 — Interrupt handler calling convention: save/restore all registers
- S19.4 — `InterruptStackFrame` type: hardware-pushed state
- S19.5 — `idt.set_handler(vector, handler_fn)` — register handler
- S19.6 — `lidt` instruction emission in codegen
- S19.7 — Timer interrupt handler example (PIT/APIC)
- S19.8 — 6 tests: idt_setup, handler_registration, timer_interrupt, double_fault

**Sprint 20: Page Table Management** `P1`
> Virtual memory for kernel development

- S20.1 — `PageTable` struct: 4-level page table (x86_64) / 3-level (aarch64)
- S20.2 — `PageTableEntry` with flags: Present, Writable, UserAccessible, NX
- S20.3 — `map_page(virt, phys, flags)` — map virtual to physical
- S20.4 — `unmap_page(virt)` — remove mapping
- S20.5 — `translate_addr(virt) -> Option<PhysAddr>` — walk page table
- S20.6 — TLB flush: `invlpg` instruction after mapping changes
- S20.7 — Identity mapping for kernel boot
- S20.8 — 8 tests: map_unmap, translate, flags, identity_map, tlb_flush

**Sprint 21: Kernel Demo (QEMU)** `P1`
> Bootable kernel that proves the concept

- S21.1 — Minimal bootloader: Multiboot2 header for GRUB/QEMU
- S21.2 — VGA text buffer: write characters to `0xB8000`
- S21.3 — Serial port output: `outb(0x3F8, byte)` via inline asm
- S21.4 — GDT + IDT setup in Fajar Lang
- S21.5 — Timer interrupt: PIT at 100Hz, increment counter
- S21.6 — Keyboard interrupt: scancode to ASCII, echo to VGA
- S21.7 — Simple shell: parse commands, execute built-in `help`, `clear`, `echo`
- S21.8 — QEMU test harness: automated boot + serial output validation
- S21.9 — 5 integration tests: boot, vga_output, serial_output, keyboard_input, shell_echo

---

#### Month 6: Kernel Polish & Hardware (S22-S26)

**Sprint 22: ARM64 Bare Metal** `P1`
> Cross-compile to real ARM64 hardware

- S22.1 — AArch64 startup assembly: exception vector, stack setup
- S22.2 — UART driver (PL011): init, putc, getc for Raspberry Pi
- S22.3 — GPIO driver: pin mode, read, write
- S22.4 — Timer driver: ARM generic timer
- S22.5 — Build system: `fj build --target aarch64-unknown-none`
- S22.6 — Flash/deploy tooling: `fj flash --device /dev/ttyUSB0`
- S22.7 — 4 tests: uart_output, gpio_toggle, timer_delay (QEMU aarch64)

**Sprint 23: RISC-V Bare Metal** `P2`
> Cross-compile to RISC-V (emerging embedded target)

- S23.1 — RISC-V startup assembly: trap vector, stack setup
- S23.2 — UART driver (SiFive): init, putc, getc
- S23.3 — PLIC interrupt controller driver
- S23.4 — Build system: `fj build --target riscv64gc-unknown-none-elf`
- S23.5 — QEMU RISC-V test harness
- S23.6 — 4 tests: uart_output, interrupt_handler (QEMU riscv64)

**Sprint 24: Union Types & Bit Fields** `P2`
> Low-level data layout control

- S24.1 — `union` type: overlapping memory layout
- S24.2 — `#[repr(C)]` attribute for C-compatible layout
- S24.3 — `#[repr(packed)]` for packed structs (no padding)
- S24.4 — Bit field syntax: `flags: u8 { present: 1, writable: 1, user: 1 }`
- S24.5 — Cranelift: bit manipulation codegen for bit fields
- S24.6 — 8 tests: union_basic, repr_c, packed_struct, bitfield_read_write

**Sprint 25: DMA & Bus Drivers** `P2`
> Direct Memory Access for high-throughput I/O

- S25.1 — DMA descriptor types: `DmaBuffer`, `DmaDescriptor`
- S25.2 — `dma_alloc(size) -> PhysAddr` — physically contiguous allocation
- S25.3 — I2C bus trait: `fn read(addr, reg, buf)`, `fn write(addr, reg, data)`
- S25.4 — SPI bus trait: `fn transfer(tx, rx)`, `fn write(data)`
- S25.5 — 4 tests: dma_alloc, i2c_mock, spi_mock, dma_transfer

**Sprint 26: OS Sprint Hardening** `P1`
> Integration testing, docs, benchmark

- S26.1 — End-to-end kernel boot test (QEMU x86_64 + aarch64)
- S26.2 — Interrupt latency benchmark
- S26.3 — Memory allocator benchmark (bump vs freelist vs pool)
- S26.4 — OS chapter in mdBook documentation
- S26.5 — Example: `examples/mini_kernel.fj` — complete bootable kernel
- S26.6 — Example: `examples/blinky.fj` — GPIO blink for RPi
- S26.7 — 5 integration tests

---

### Quarter 3 — GPU & ML Infrastructure

#### Month 7: GPU Compute (S27-S30)

**Sprint 27: GPU Abstraction Layer** `P0` `CRITICAL`
> Hardware-agnostic GPU compute interface

- S27.1 — `GpuDevice` trait: `fn name()`, `fn memory()`, `fn compute_units()`
- S27.2 — `GpuBuffer` type: device memory allocation
- S27.3 — `GpuKernel` type: compiled compute shader/kernel
- S27.4 — `gpu::available_devices() -> Vec<GpuDevice>`
- S27.5 — `device.create_buffer(size) -> GpuBuffer`
- S27.6 — `device.upload(host_data, gpu_buffer)`
- S27.7 — `device.download(gpu_buffer, host_data)`
- S27.8 — `device.execute(kernel, grid_size, block_size, args)`
- S27.9 — 6 tests: device_enum, buffer_create, upload_download, execute_nop

**Sprint 28: Vulkan Compute Backend** `P1`
> Vulkan compute shaders for cross-platform GPU

- S28.1 — Vulkan instance + device creation via `ash` crate
- S28.2 — Compute pipeline: shader module → pipeline layout → compute pipeline
- S28.3 — Descriptor sets: buffer bindings for shader inputs/outputs
- S28.4 — Command buffer: record dispatch, submit to compute queue
- S28.5 — SPIR-V shader compilation: simple element-wise ops
- S28.6 — Built-in kernels: `add`, `mul`, `relu`, `sigmoid`, `matmul`
- S28.7 — Memory management: device-local vs host-visible allocation strategy
- S28.8 — 8 tests: vulkan_init, buffer_ops, element_add, matmul_small, relu

**Sprint 29: CUDA FFI Backend** `P1`
> CUDA support via runtime API FFI

- S29.1 — `libcuda.so` / `cuda.dll` dynamic loading via `libloading`
- S29.2 — `cuInit`, `cuDeviceGet`, `cuCtxCreate` — context setup
- S29.3 — `cuMemAlloc`, `cuMemcpyHtoD`, `cuMemcpyDtoH` — memory ops
- S29.4 — `cuModuleLoad`, `cuModuleGetFunction`, `cuLaunchKernel` — kernel launch
- S29.5 — PTX generation: simple kernels compiled to PTX text
- S29.6 — Built-in CUDA kernels: matmul, elementwise, reduction
- S29.7 — 6 tests: cuda_init, mem_alloc, kernel_launch, matmul_cuda (skip if no GPU)

**Sprint 30: GPU Tensor Integration** `P0`
> Connect GPU compute to tensor runtime

- S30.1 — `Tensor.to_gpu(device) -> GpuTensor` — upload tensor to GPU
- S30.2 — `GpuTensor.to_cpu() -> Tensor` — download tensor from GPU
- S30.3 — GPU-accelerated matmul: dispatch to Vulkan/CUDA backend
- S30.4 — GPU-accelerated elementwise: add, sub, mul, div on GPU
- S30.5 — GPU-accelerated activation: relu, sigmoid, softmax on GPU
- S30.6 — Automatic device selection: GPU if available, fallback to CPU
- S30.7 — Memory pool: pre-allocated GPU memory for reduced allocation overhead
- S30.8 — 10 tests: to_gpu, to_cpu, gpu_matmul, gpu_relu, gpu_softmax, auto_device

---

#### Month 8: ML Training Infrastructure (S31-S34)

**Sprint 31: Tensor Ops in Native Codegen** `P0`
> Move interpreter-only tensor ops to Cranelift

- S31.1 — Runtime functions: `fj_rt_tensor_zeros`, `fj_rt_tensor_ones`, `fj_rt_tensor_randn`
- S31.2 — Runtime functions: `fj_rt_tensor_add`, `fj_rt_tensor_sub`, `fj_rt_tensor_mul`
- S31.3 — Runtime functions: `fj_rt_tensor_matmul`, `fj_rt_tensor_transpose`
- S31.4 — Runtime functions: `fj_rt_tensor_relu`, `fj_rt_tensor_sigmoid`, `fj_rt_tensor_softmax`
- S31.5 — Runtime functions: `fj_rt_tensor_reshape`, `fj_rt_tensor_flatten`
- S31.6 — `compile_call` dispatch for tensor builtins
- S31.7 — Tensor as opaque `I64` pointer in codegen (like arrays)
- S31.8 — 15 tests: zeros, ones, randn, add, sub, mul, matmul, transpose, relu, sigmoid, softmax, reshape, flatten, chained_ops, large_tensor

**Sprint 32: Autograd in Native Codegen** `P1`
> Tape-based autodiff accessible from native code

- S32.1 — Runtime: `fj_rt_tensor_requires_grad(tensor, bool)`
- S32.2 — Runtime: `fj_rt_tensor_backward(tensor)` — reverse-mode AD
- S32.3 — Runtime: `fj_rt_tensor_grad(tensor) -> Tensor` — get gradient
- S32.4 — Runtime: `fj_rt_tensor_zero_grad(tensor)` — clear gradient
- S32.5 — Gradient accumulation through matmul, relu, sigmoid, softmax
- S32.6 — `fj_rt_mse_loss(pred, target) -> Tensor` — differentiable loss
- S32.7 — `fj_rt_cross_entropy(pred, target) -> Tensor`
- S32.8 — 10 tests: requires_grad, backward_simple, grad_access, zero_grad, mse_grad, chain_rule, matmul_grad

**Sprint 33: Optimizers & Training Loop in Native** `P1`
> SGD/Adam accessible from native codegen

- S33.1 — Runtime: `fj_rt_sgd_new(lr, momentum) -> Optimizer`
- S33.2 — Runtime: `fj_rt_adam_new(lr, beta1, beta2) -> Optimizer`
- S33.3 — Runtime: `fj_rt_optimizer_step(optimizer, params)` — parameter update
- S33.4 — Runtime: `fj_rt_optimizer_zero_grad(optimizer)`
- S33.5 — Training loop pattern: forward → loss → backward → step
- S33.6 — Epoch/batch iteration with progress tracking
- S33.7 — 8 tests: sgd_step, adam_step, training_loop_simple, loss_decreases

**Sprint 34: Distributed Training** `P2`
> Multi-process training with gradient aggregation

- S34.1 — `distributed::init(world_size, rank)` — process group setup
- S34.2 — `distributed::all_reduce(tensor, op)` — gradient aggregation (sum/mean)
- S34.3 — `distributed::broadcast(tensor, root)` — parameter broadcast
- S34.4 — `distributed::barrier()` — synchronization point
- S34.5 — Data parallelism: split batches across processes
- S34.6 — Communication backend: TCP sockets or shared memory
- S34.7 — 6 tests: init, all_reduce, broadcast, barrier, data_parallel (mock)

---

#### Month 9: ML Ecosystem (S35-S39)

**Sprint 35: ONNX Export** `P1`
> Export trained models for deployment

- S35.1 — ONNX protobuf format: `ModelProto`, `GraphProto`, `NodeProto`
- S35.2 — Layer → ONNX operator mapping: Dense→MatMul+Add, Conv2d→Conv, etc.
- S35.3 — Weight serialization: tensor data → ONNX TensorProto
- S35.4 — `model.export_onnx("model.onnx")` API
- S35.5 — Shape inference: propagate shapes through ONNX graph
- S35.6 — Validation: load exported model with ONNX Runtime (via FFI)
- S35.7 — 6 tests: simple_model, conv_model, attention_model, shapes, roundtrip

**Sprint 36: Data Pipeline** `P1`
> Batched data loading for training

- S36.1 — `Dataset` trait: `fn len()`, `fn get(index) -> (Tensor, Tensor)`
- S36.2 — `DataLoader` struct: batching, shuffling, optional parallel loading
- S36.3 — `MnistDataset`: built-in MNIST loader (download + parse IDX format)
- S36.4 — `CsvDataset`: load CSV files as tensor pairs
- S36.5 — Transforms: normalize, random_crop, random_flip
- S36.6 — Async data loading: prefetch next batch while training
- S36.7 — 8 tests: mnist_load, csv_load, batching, shuffling, transforms, prefetch

**Sprint 37: Model Serialization** `P1`
> Save and load trained models

- S37.1 — `model.save("model.fj_model")` — serialize weights + architecture
- S37.2 — `Model::load("model.fj_model") -> Model` — deserialize
- S37.3 — Format: custom binary format (magic + version + layer defs + weight tensors)
- S37.4 — Checkpoint: `model.save_checkpoint(epoch, loss)` — with metadata
- S37.5 — 6 tests: save_load_roundtrip, checkpoint, corrupted_file_error

**Sprint 38: MNIST End-to-End Native** `P0`
> Prove ML works in native codegen

- S38.1 — MNIST data loading (native codegen, via runtime functions)
- S38.2 — Model definition: Conv2d → ReLU → Flatten → Dense → Softmax
- S38.3 — Training loop: 10 epochs, batch_size=64, Adam optimizer
- S38.4 — Accuracy evaluation on test set
- S38.5 — Performance benchmark: training time, inference time
- S38.6 — Example: `examples/mnist_native.fj`
- S38.7 — 3 tests: train_1_epoch, accuracy_above_90, inference_speed

**Sprint 39: Mixed Precision & Quantization** `P2`
> FP16/BF16 training and INT8 inference

- S39.1 — `f16` type support in parser + type system
- S39.2 — `bf16` type support (brain float)
- S39.3 — Mixed precision training: forward in f16, backward in f32
- S39.4 — Loss scaling: prevent underflow in f16 gradients
- S39.5 — Post-training quantization: f32 → int8 with calibration
- S39.6 — Quantized inference: int8 matmul with int32 accumulator
- S39.7 — 8 tests: f16_basic, bf16_basic, mixed_precision, loss_scaling, ptq, int8_inference

---

### Quarter 4 — Production & Self-Hosting

#### Month 10: Optimization & SIMD (S40-S43)

**Sprint 40: SIMD Intrinsics** `P1`
> Vectorized computation for ML and OS

- S40.1 — `simd::f32x4`, `simd::f32x8`, `simd::i32x4` vector types
- S40.2 — Vector arithmetic: add, sub, mul, div (element-wise)
- S40.3 — Horizontal operations: sum, min, max across lanes
- S40.4 — Load/store: aligned and unaligned memory access
- S40.5 — Cranelift SIMD: `Fxxx` type family, `vconst`, `iadd` on vector types
- S40.6 — Auto-vectorization hints: `@simd` annotation on loops
- S40.7 — 10 tests: f32x4_add, f32x8_mul, horizontal_sum, aligned_load, auto_vectorize

**Sprint 41: Optimization Passes** `P1`
> Compiler optimizations for competitive performance

- S41.1 — Dead code elimination: unreachable blocks, unused variables
- S41.2 — Constant folding: compile-time evaluation of constant expressions
- S41.3 — Constant propagation: replace variables with known constant values
- S41.4 — Loop-invariant code motion: hoist invariant computations out of loops
- S41.5 — Inlining: inline small functions (< 20 IR instructions)
- S41.6 — Common subexpression elimination (CSE)
- S41.7 — Cranelift optimization levels: `OptLevel::None`, `Speed`, `SpeedAndSize`
- S41.8 — 10 tests: dce_removes_dead, const_fold, inline_small, cse_dedup

**Sprint 42: Benchmarks vs C/Rust** `P1`
> Quantitative performance comparison

- S42.1 — Benchmark suite: fibonacci, matrix multiply, sorting, string processing
- S42.2 — C baseline: compile same algorithms with `gcc -O2`
- S42.3 — Rust baseline: compile same algorithms with `rustc --release`
- S42.4 — Fajar baseline: `fj build --release`
- S42.5 — Results table: ops/sec, memory usage, binary size
- S42.6 — Identify bottlenecks, optimize hot paths
- S42.7 — Performance regression CI: criterion benchmarks in GitHub Actions

**Sprint 43: Binary Size & Startup** `P2`
> Optimize for embedded deployment

- S43.1 — Dead function elimination: remove unused functions from object file
- S43.2 — String deduplication: merge identical string constants
- S43.3 — Section GC: linker `--gc-sections` for unused code removal
- S43.4 — LTO-like optimization: cross-function inlining hints
- S43.5 — Startup time: lazy initialization of runtime subsystems
- S43.6 — 4 tests: binary_size_regression, startup_time, dead_fn_removed

---

#### Month 11: Self-Hosting & Bootstrap (S44-S47)

**Sprint 44: Self-Hosted Lexer** `P1`
> Port lexer from Rust to Fajar Lang

- S44.1 — Port `Token`, `TokenKind`, `Span` types to Fajar Lang
- S44.2 — Port `Cursor` struct: `peek()`, `advance()`, `is_eof()`
- S44.3 — Port `tokenize()`: keyword recognition, number/string/char literals
- S44.4 — Port comment handling: `//` line and `/* */` block comments
- S44.5 — Port operator tokenization: all 30+ operator tokens
- S44.6 — Comparison test: self-lexer output == Rust lexer output for all examples
- S44.7 — 10 tests: keywords, numbers, strings, operators, comments, identifiers

**Sprint 45: Self-Hosted Parser (Subset)** `P2`
> Port expression parser to Fajar Lang

- S45.1 — Port `Expr` enum (subset: Literal, Ident, Binary, Unary, Call)
- S45.2 — Port Pratt parser: precedence climbing for expressions
- S45.3 — Port statement parsing: let, return, if, while, fn
- S45.4 — Recursive data structures via `Box<T>` equivalent
- S45.5 — Comparison test: self-parser AST == Rust parser AST for subset
- S45.6 — 8 tests: parse_expr, parse_let, parse_fn, parse_if, parse_while

**Sprint 46: Bootstrap Test** `P2`
> Compile self-hosted compiler with itself

- S46.1 — Compile self-lexer with Rust compiler → binary A
- S46.2 — Run binary A on test input → output A
- S46.3 — Compile self-lexer with self-compiler → binary B (if possible)
- S46.4 — Run binary B on same test input → output B
- S46.5 — Verify output A == output B (bootstrap validation)
- S46.6 — 3 integration tests: self_lex, self_parse_subset, bootstrap_match

**Sprint 47: Self-Hosting Hardening** `P2`
> Fix issues discovered during bootstrap

- S47.1 — Fix any codegen bugs exposed by self-hosting
- S47.2 — Fix any missing features needed by the compiler itself
- S47.3 — Optimize self-compiler performance
- S47.4 — Document bootstrap process
- S47.5 — 5 tests: regression fixes

---

#### Month 12: Release & Polish (S48-S52)

**Sprint 48: Documentation Expansion** `P1`
> Complete mdBook documentation

- S48.1 — Language tutorial: 10 chapters (basics → advanced)
- S48.2 — Standard library reference: all functions documented
- S48.3 — Concurrency guide: threads, channels, async/await
- S48.4 — OS development guide: bare metal, interrupts, page tables
- S48.5 — ML guide: tensors, autograd, training, deployment
- S48.6 — GPU programming guide: Vulkan, CUDA, compute shaders
- S48.7 — API reference: all public Rust API documented

**Sprint 49: Package Ecosystem** `P1`
> Package registry and tooling

- S49.1 — `fj.toml` dependency resolution: semantic versioning
- S49.2 — Package registry API: publish, search, download
- S49.3 — `fj add <package>` — add dependency
- S49.4 — `fj publish` — publish to registry
- S49.5 — Standard packages: `fj-http`, `fj-json`, `fj-crypto`
- S49.6 — 5 tests: publish, install, resolve_deps, version_conflict

**Sprint 50: IDE & Tooling Polish** `P1`
> Developer experience improvements

- S50.1 — LSP: go-to-definition for all symbols
- S50.2 — LSP: auto-completion for methods and fields
- S50.3 — LSP: hover info with type signatures
- S50.4 — LSP: diagnostics with quick-fix suggestions
- S50.5 — VS Code extension: syntax highlighting, snippets, debugging
- S50.6 — Debugger: source-level debugging via DWARF info in Cranelift objects

**Sprint 51: Real-World Demos** `P0`
> Prove Fajar Lang works for its target audience

- S51.1 — Demo 1: Drone flight controller (sensor read → ML inference → actuator)
- S51.2 — Demo 2: MNIST classifier (native, GPU-accelerated, <10ms inference)
- S51.3 — Demo 3: Mini OS kernel (VGA, keyboard, timer, shell)
- S51.4 — Demo 4: Cross-domain bridge (sensor → tensor → prediction → action)
- S51.5 — Demo 5: Package-based project with dependencies
- S51.6 — Blog post / technical writeup for each demo

**Sprint 52: v0.3 Release** `P0`
> Final release preparation

- S52.1 — Version bumps: Cargo.toml, CLI output, docs
- S52.2 — Release notes: comprehensive changelog
- S52.3 — Binary releases: Linux x86_64, macOS arm64, Windows x86_64
- S52.4 — Homebrew formula / APT package
- S52.5 — GitHub Release with source + binaries
- S52.6 — Announcement: social media, forums, HN

---

## 5. Dependency Graph (Full)

```
                        S1 (Refactor)
                            │
              ┌─────────────┼─────────────┐
              v             v             v
         S2 (FnPtr)    S3 (HashMap)   S4 (Parity)
              │
    ┌─────────┼─────────────────┐
    v         v                 v
S5 (Threads) S8 (Atomics)  S14 (InlineAsm)
    │         │                 │
    v         v                 v
S6 (Sync)  S9 (Future)    S15 (Volatile)
    │         │                 │
    v         v                 v
S7 (Chan)  S10 (Executor) S16 (Allocator)
    │         │                 │
    v         v                 v
S13 (Hard) S11 (MultiAsync) S17 (BareMetal)
              │                 │
              v                 v
         S12 (AsyncChan)   S18 (Linker)
                                │
                    ┌───────────┼───────────┐
                    v           v           v
               S19 (IDT)  S20 (PageTable) S21 (Kernel)
                                │           │
                                v           v
                           S22 (ARM64)  S23 (RISC-V)
                                │
                                v
                    ┌───────────┼───────────┐
                    v           v           v
               S27 (GPU)  S31 (TensorNat) S36 (DataPipe)
                    │           │
              ┌─────┼─────┐    v
              v     v     v  S32 (Autograd)
         S28(Vulk) S29(CUDA) │
              │     │        v
              └──┬──┘   S33 (Optim)
                 v           │
            S30 (GpuTen)     v
                 │      S34 (Distrib)
                 v           │
            S38 (MNIST) ─────┘
                 │
                 v
         ┌───────┼───────────┐
         v       v           v
    S35 (ONNX) S37 (Serial) S39 (Mixed)
                             │
              ┌──────────────┼──────────────┐
              v              v              v
         S40 (SIMD)     S41 (Optim)    S44 (SelfHost)
              │              │              │
              v              v              v
         S42 (Bench)    S43 (BinSize)  S45 (Parser)
                                            │
                                            v
                                       S46 (Bootstrap)
                                            │
                                            v
                                       S47 (Harden)
                                            │
                    ┌───────────────────────┼───────────────────────┐
                    v                       v                       v
               S48 (Docs)             S49 (Packages)          S50 (IDE)
                    │                       │                       │
                    └───────────────────────┼───────────────────────┘
                                            v
                                       S51 (Demos)
                                            │
                                            v
                                       S52 (Release)
```

---

## 6. Research Findings & Feasibility Notes

> Researched 2026-03-08 — online research on Cranelift capabilities, GPU APIs, ONNX format

### 6.1 Cranelift Inline Assembly — NO NATIVE SUPPORT

**Finding:** Cranelift IR does **NOT** have an `InlineAsm` instruction. `rustc_codegen_cranelift` handles
inline asm by **invoking an external assembler** and emitting raw machine code bytes, bypassing
Cranelift's IR entirely. As of June 2025, inline asm is supported on x86_64, aarch64, and riscv64
in cg_clif, and is considered stable (except `sym` operands).

**Impact on Sprint 14:**
- Option A: Use external assembler approach (like cg_clif) — assemble template with `nasm`/`as`, embed raw bytes
- Option B: Emit architecture-specific raw bytes directly via `MachBuffer` (lower level, more control)
- Option C: For common patterns (port I/O, CR access), provide runtime functions written in Rust with inline asm

**Recommendation:** Use **Option C for v0.3** — most kernel operations (port I/O, register access)
can be implemented as `extern "C"` runtime functions written in Rust (which HAS inline asm).
Reserve raw byte emission for v0.4. This is simpler and still achieves the kernel demo goal.

### 6.2 Cranelift Atomic Instructions — FULLY SUPPORTED

**Finding:** Cranelift has full native atomic support:
- `atomic_load` (Opcode 183) — atomic read with ordering
- `atomic_store` (Opcode 184) — atomic write with ordering
- `atomic_cas` (Opcode 182) — compare-and-swap, returns old value
- `atomic_rmw` (Opcode 181) — read-modify-write (add, sub, and, or, xor)
- `fence` (Opcode 185) — memory barrier (sequentially consistent)

**Impact on Sprint 8:** Full green light. `builder.ins().atomic_cas(flags, addr, expected, desired)`
works directly. No workaround needed.

### 6.3 Cranelift SIMD — SUPPORTED (with caveats)

**Finding:** Cranelift supports SIMD vector types natively:
- `i32x4`, `i64x2`, `f32x4`, `f64x2` — standard 128-bit vectors
- `f32x8`, `i32x8` — 256-bit vectors
- Operations: lane-wise add/sub/mul, horizontal ops, `vhigh_bits`, widen/narrow
- Dynamic vector types also supported (scalable vectors via RFC)

**Impact on Sprint 40:** Feasible. Use Cranelift's native vector types directly.
Some operations (e.g., `sdiv.i32x4`) had bugs historically — test thoroughly.

### 6.4 Cranelift `call_indirect` — FULLY SUPPORTED

**Finding:** `builder.ins().call_indirect(sig_ref, callee, &args)` is a first-class instruction.
Takes `SigRef` (from `builder.import_signature(sig)`), function pointer as `Value` (I64),
and argument slice. Used for dynamic libraries, function pointers, and virtual dispatch.

**Impact on Sprint 2:** Full green light. Function pointers compile directly to
`func_addr` + `call_indirect`. No workaround needed.

### 6.5 Cranelift Bare Metal — OBJECT FILES ONLY

**Finding:** `cranelift-object` produces relocatable `.o` (ELF) object files, NOT final executables.
An external linker (`ld`, `lld`) with a linker script is required for final bare-metal binary.
Cranelift itself supports `no_std` and can run in constrained environments.

**Impact on Sprint 17-21:** The workflow is:
1. Cranelift compiles Fajar Lang → `.o` object file
2. External `ld`/`lld` links with linker script → final `.elf`
3. `objcopy` converts to `.bin` for flashing

This is the standard bare-metal workflow (same as Rust bare-metal). Fully feasible.

### 6.6 GPU Backend — USE wgpu INSTEAD OF raw ash

**Finding:** `wgpu` provides a dramatically simpler API than raw Vulkan/ash:
- Cross-platform: Vulkan + Metal + D3D12 + OpenGL + WebGPU via single API
- Safe Rust API — no manual synchronization, allocator chains, or descriptor sets
- Headless compute: works without windowing surface
- Used by Firefox, Servo, Deno — production-proven
- Internally uses `ash` for Vulkan backend anyway
- WGSL shader language or SPIR-V via `rust-gpu`

**Impact on Sprint 27-30:** Replace `ash` (raw Vulkan) with `wgpu` for the primary GPU backend.
Keep CUDA FFI (Sprint 29) as optional for NVIDIA-specific optimizations.

**Updated dependencies:**
```toml
wgpu = "24.0"              # Cross-platform GPU (replaces ash)
# ash = "0.38"             # Deferred — only if wgpu insufficient
```

### 6.7 ONNX Export — Rust Libraries Available

**Finding:** Multiple Rust crates for ONNX protobuf:
- `onnx-protobuf` — pre-generated ONNX protobuf types (ModelProto, GraphProto, etc.)
- `onnx-extractor` — prost-based, generates types from `onnx.proto` at build time
- `prost` + `prost-build` — DIY: compile official `onnx.proto` directly

**Impact on Sprint 35:** Use `onnx-protobuf` crate directly. No need to build protobuf from scratch.
For model validation, optionally use `ort` (ONNX Runtime Rust bindings) to load and verify exported models.

**Updated dependencies:**
```toml
onnx-protobuf = "0.2"     # ONNX protobuf types
# ort = "2.0"              # Optional: ONNX Runtime for validation
```

---

## 7. Risk Matrix

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Cranelift lacks inline asm | S14 blocked | Medium | Use raw byte emission or LLVM backend |
| No GPU hardware for testing | S28-29 blocked | Low | QEMU + software Vulkan (lavapipe) |
| 17K cranelift.rs too hard to split | S1 delayed | Medium | Incremental extraction, not big-bang |
| Async state machine codegen complex | S9 delayed | High | Start simple (no nesting), iterate |
| Self-hosting needs features we don't have | S44 blocked | Medium | Implement needed features first |
| Performance not competitive | S42 reveals gaps | Medium | Focus on correctness first, optimize later |

---

## 7. Quality Gates

### Per-Sprint Gate
- [ ] All new tests pass
- [ ] All existing tests still pass (zero regressions)
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo fmt -- --check` — formatted
- [ ] No `.unwrap()` in `src/` (only in tests)
- [ ] All `pub` items have doc comments
- [ ] V03_TASKS.md updated

### Per-Quarter Gate
- [ ] All sprint gates pass
- [ ] At least 1 new example program
- [ ] Benchmark comparison with previous quarter
- [ ] Architecture doc updated
- [ ] No tech debt accumulated (refactor before proceeding)

### Release Gate (v0.3)
- [ ] 4,000+ tests, zero failures
- [ ] 5 real-world demos working
- [ ] Documentation complete (tutorial + reference + guides)
- [ ] Binary releases for 3 platforms
- [ ] Bootstrap test passes (lexer self-hosts)
- [ ] QEMU kernel boots
- [ ] MNIST trains in native codegen

---

## 8. New Cargo Dependencies (Estimated)

```toml
# v0.3 additions
[dependencies]
# GPU (wgpu replaces raw ash — see §6.6 research finding)
wgpu = "24.0"                   # Cross-platform GPU (Vulkan+Metal+D3D12+WebGPU)
# ash = "0.38"                  # Deferred — only if wgpu insufficient

# Concurrency (mostly std library, minimal deps)
crossbeam = "0.8"               # Lock-free data structures
parking_lot = "0.12"            # Fast mutex/rwlock

# ML ecosystem
protobuf = "3.4"                # ONNX protobuf serialization
half = "2.4"                    # f16/bf16 types

# Bare metal (feature-gated)
# No additional deps — bare metal means no deps

[features]
gpu = ["wgpu"]                  # Cross-platform GPU compute
gpu-cuda = []                   # Dynamic loading, no compile-time dep
distributed = []
bare-metal = []
```

---

## 9. Test Target Breakdown

| Category | Baseline (v0.2) | Current (2026-03-09) | Target (v0.3) |
|----------|----------------|---------------------|---------------|
| Unit (default) | 1,074 | ~1,140 | 1,674 |
| Unit (native codegen) | 133 | ~656 | 800+ |
| Integration (eval) | 171 | 171 | 221 |
| Integration (ML) | 39 | 39 | 119 |
| Integration (OS) | 16 | 16 | 76 |
| Integration (concurrency) | 0 | 0 | 80 |
| Integration (GPU) | 0 | 0 | 50 |
| Property | 33 | 33 | 53 |
| Autograd | 13 | 13 | 33 |
| Safety | 76 | 76 | 106 |
| Cross-compile | 9 | 9 | 19 |
| Self-hosting | 0 | 0 | 30 |
| **Total** | **1,991** | **~2,161** | **4,021+** |

---

## 10. Monthly Milestones

| Month | Tag | Milestone Name | Key Deliverable |
|-------|-----|---------------|-----------------|
| 1 | v0.3.0-alpha.1 | "Refactored" | Cranelift split, fn pointers, HashMap |
| 2 | v0.3.0-alpha.2 | "Concurrent" | Threads, Mutex, Channels, Atomics |
| 3 | v0.3.0-alpha.3 | "Async" | async/await, executor, async channels |
| 4 | v0.3.0-alpha.4 | "Bare Metal" | Inline asm, volatile, custom allocator |
| 5 | v0.3.0-alpha.5 | "Kernel" | Linker scripts, IDT, page tables, QEMU boot |
| 6 | v0.3.0-beta.1 | "Hardware" | ARM64 + RISC-V bare metal, DMA |
| 7 | v0.3.0-beta.2 | "GPU" | GPU abstraction, Vulkan, CUDA FFI |
| 8 | v0.3.0-beta.3 | "Training" | Tensor native, autograd native, optimizers |
| 9 | v0.3.0-beta.4 | "Ecosystem" | ONNX, data pipeline, MNIST native |
| 10 | v0.3.0-rc.1 | "Optimized" | SIMD, optimization passes, benchmarks |
| 11 | v0.3.0-rc.2 | "Self-Hosted" | Lexer in .fj, bootstrap test |
| 12 | v0.3.0 | "Dominion" | Full release, demos, docs |

---

---

## 11. Development Environment

| Resource | Version | Purpose |
|----------|---------|---------|
| QEMU x86_64 | 8.2.2 | Bare metal kernel testing (S19-S21) |
| QEMU aarch64 | 8.2.2 | ARM64 bare metal + cross-compile (S17-S21) |
| QEMU riscv64 | 8.2.2 | RISC-V bare metal (S17-S21) |
| NVIDIA RTX 4090 | CC 8.9, Driver 590.48 | GPU compute (S27-S30) |
| CUDA | 12.8 (nvcc + libs) | CUDA FFI (S29) |
| Vulkan | 1.4.325 (NVIDIA ICD) | GPU abstraction (S27-S28) |
| OVMF | 2024.02 | UEFI firmware for QEMU (S19) |
| Platform | x86_64 Linux 6.17.0 | Primary dev environment |

---

## 12. Gap Audit (2026-03-09)

> Comprehensive analysis of all uncompleted P0 and P1 tasks, plus completed sprints with deferred subtasks.

### 12.1 Uncompleted P0 Tasks (9 items — blockers)

| Task | Sprint | Status | Blocker / Notes |
|------|--------|--------|-----------------|
| S1.2 Extract expr compilation | S1 | Deferred | expr/stmt/call remain in compile/mod.rs — low risk, refactor-only |
| S1.3 Extract stmt compilation | S1 | Deferred | Same as S1.2 — can do when compile/mod.rs grows further |
| S9.3 Async state machine codegen | S9 | Not started | Blocked by S9.2 (Future trait in codegen) |
| S9.4 Await compilation | S9 | Not started | Blocked by S9.3 (state machine) |
| S10.1 Single-threaded executor | S10 | Not started | Blocked by S9.3-S9.4 (async codegen) |
| S14.2 Asm operand types | S14 | Not started | Needs register constraint parsing + codegen mapping |
| S30 GPU tensor bridge | S30 | Not started | Blocked by S28 (Vulkan) / S29 (CUDA FFI) |
| S51 End-to-end demos | S51 | Not started | Blocked by most of Q2-Q3 |
| S52 Release | S52 | Not started | Final sprint — all others must be done |

**Critical path:** S9.2 → S9.3 → S9.4 → S10.1 → S10.2 → S11 (async/await chain)

### 12.2 Uncompleted P1 Tasks (15+ items — must have)

| Task | Sprint | Status | Blocker / Notes |
|------|--------|--------|-----------------|
| S10.2 Waker implementation | S10 | Not started | Blocked by S10.1 (executor) |
| S10.3 Timer future | S10 | Not started | Blocked by S10.2 (waker) |
| S13.3 Borrow checker + concurrency | S13 | Not started | Needs move/clone semantics for channel send |
| S13.4 Concurrency benchmarks | S13 | Not started | Needs S10 complete for async benchmarks |
| S13.5 Documentation | S13 | Not started | Write after all concurrency features done |
| S14.5 global_asm! | S14 | Not started | Needs linker-level integration |
| S16.3 Global allocator | S16 | Not started | Set custom allocator as default for all alloc |
| S17.4 Bare metal output | S17 | Not started | Serial/UART output without std IO |
| S35 ONNX export | S35 | Not started | Needs tensor native (S31 ✅) + model serialization |
| S40 SIMD intrinsics | S40 | Not started | SSE/AVX/NEON via Cranelift SIMD types |
| S44 Self-hosted lexer | S44 | Not started | Lexer in .fj — needs string + match + array native |
| S48 Documentation site | S48 | Not started | mdBook + API reference |
| S49 Package manager | S49 | Not started | fj.toml dependencies, registry |
| S50 IDE integration | S50 | Not started | LSP extensions for v0.3 features |

### 12.3 Completed Sprints with Deferred Subtasks

| Sprint.Task | What was deferred | Impact |
|-------------|-------------------|--------|
| S2.5 | map/filter/reduce for fn pointers | Low — convenience, not blocking |
| S3.3 | HashMap keys()/values() iteration | Low — can use direct access |
| S4.10 | Only 6/15 examples work natively | Medium — need more coverage |
| S9.5 | Future return type checking incomplete | Medium — needed for S9.3 |
| S17.3 | `_start` symbol generation | Medium — needed for real bare metal |
| S32 | Gradient through complex ops (matmul etc.) | Medium — basic autograd works |

### 12.4 Recommended Priority Order

**Immediate (unblocks most work):**
1. S9.2 — Future trait codegen (unblocks entire async chain)
2. S14.2 — Asm operand types (unblocks full inline assembly)
3. S1.2/S1.3 — Extract expr/stmt (code hygiene, reduces compile/mod.rs size)

**Short-term (builds on immediate):**
4. S9.3 → S9.4 — Async state machine + await
5. S10.1 → S10.2 → S10.3 — Executor + waker + timer
6. S16.3 — Global allocator
7. S17.4 — Bare metal output (serial UART)

**Medium-term (Q2-Q3 features):**
8. S13.3-S13.5 — Concurrency hardening
9. S35 — ONNX export
10. S40 — SIMD intrinsics

**Long-term (release prep):**
11. S44 — Self-hosted lexer
12. S48-S50 — Docs, packages, IDE
13. S51-S52 — Demos + release

---

*V03_IMPLEMENTATION_PLAN.md v1.2 — 52 sprints, 12 months, ~620 tasks | Updated 2026-03-09*
