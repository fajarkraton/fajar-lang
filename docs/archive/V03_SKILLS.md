# Skills — Fajar Lang v0.3 "Dominion"

> Implementation patterns and technical recipes for v0.3 features.
> Read this BEFORE implementing complex tasks.
> Reference: `V03_IMPLEMENTATION_PLAN.md`, `V03_TASKS.md`
> Updated: 2026-03-09 — S1-S8 complete, concurrency/atomic patterns are IMPLEMENTED (not planned)

---

## 1. Cranelift Module Split Patterns (S1 ✅ COMPLETE)

> **Actual result:** 12-file module structure under `src/codegen/cranelift/`.
> S1.2 (expr.rs) and S1.3 (stmt.rs) deferred — expr/stmt remain in compile/mod.rs.

### 1.1 Extracting a Function from cranelift.rs

```rust
// BEFORE: everything in cranelift.rs
// src/codegen/cranelift.rs (17,241 lines)

impl<M: Module> CodegenCtx<'_, M> {
    fn compile_if(&mut self, builder: &mut FunctionBuilder, ...) -> ... { ... }
}

// AFTER: extracted to separate module
// src/codegen/cranelift/control.rs

use super::context::CodegenCtx;
use cranelift_frontend::FunctionBuilder;

pub(crate) fn compile_if<M: Module>(
    cx: &mut CodegenCtx<'_, M>,
    builder: &mut FunctionBuilder,
    ...
) -> ... {
    // Same implementation, but cx is now an explicit parameter
}
```

### 1.2 Shared State Pattern

```rust
// src/codegen/cranelift/context.rs

/// All compilation state shared across codegen sub-modules.
pub(crate) struct CodegenCtx<'a, M: Module> {
    pub module: &'a mut M,
    pub var_map: HashMap<String, Variable>,
    pub var_types: HashMap<String, ClifType>,
    pub string_lens: HashMap<String, Variable>,
    // ... all 34+ fields
}

impl<'a, M: Module> CodegenCtx<'a, M> {
    pub(crate) fn new(/* params */) -> Self { ... }

    // Only truly shared helpers go here
    pub(crate) fn get_var(&self, name: &str) -> Option<Variable> { ... }
    pub(crate) fn track_string(&mut self, name: &str, len_var: Variable) { ... }
}
```

### 1.3 Module Re-Export Pattern

```rust
// src/codegen/cranelift/mod.rs

mod context;
mod expr;
mod stmt;
mod control;
mod strings;
mod arrays;
mod structs;
mod enums;
mod closures;
mod generics;
mod builtins;
mod runtime_fns;

#[cfg(test)]
mod tests;

// Re-export the compiler structs
pub use self::context::CodegenCtx;

// CraneliftCompiler and ObjectCompiler stay in mod.rs
// They call into sub-modules for compilation
pub struct CraneliftCompiler { ... }
pub struct ObjectCompiler { ... }

impl CraneliftCompiler {
    pub fn compile_program(&mut self, program: &Program) -> Result<...> {
        // Orchestration logic stays here
        // Actual compilation delegated to sub-modules
        let mut cx = CodegenCtx::new(...);
        for item in &program.items {
            match item {
                Item::FnDef(fndef) => {
                    // Call into sub-module
                    expr::compile_function(&mut cx, fndef)?;
                }
                ...
            }
        }
    }
}
```

---

## 2. Function Pointer Patterns

### 2.1 Cranelift call_indirect

```rust
// Representing function pointers as I64 values
fn compile_fn_pointer_call(
    cx: &mut CodegenCtx<'_, M>,
    builder: &mut FunctionBuilder,
    fn_ptr_var: Variable,      // holds I64 address
    args: &[Value],
    ret_type: ClifType,
) -> Value {
    let fn_addr = builder.use_var(fn_ptr_var);

    // Create signature for the indirect call
    let mut sig = Signature::new(CallConv::SystemV);
    for arg in args {
        sig.params.push(AbiParam::new(builder.func.dfg.value_type(*arg)));
    }
    sig.returns.push(AbiParam::new(ret_type));

    let sig_ref = builder.import_signature(sig);

    // call_indirect: call through function pointer
    let call = builder.ins().call_indirect(sig_ref, fn_addr, args);
    builder.inst_results(call)[0]
}
```

### 2.2 Getting Function Address

```rust
// Get address of a known function for use as function pointer
fn get_fn_address(
    cx: &mut CodegenCtx<'_, M>,
    builder: &mut FunctionBuilder,
    fn_name: &str,
) -> Value {
    let func_id = cx.functions[fn_name];
    let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
    builder.ins().func_addr(types::I64, func_ref)
}
```

### 2.3 Closure as Argument (Capture Passing)

```rust
// For closure with captures, create a fat pointer: (fn_addr, captures_ptr)
// When calling through the fat pointer:
// 1. Extract fn_addr and captures_ptr
// 2. Prepend captures_ptr as first argument to the call

fn compile_closure_call_indirect(
    cx: &mut CodegenCtx<'_, M>,
    builder: &mut FunctionBuilder,
    closure_var: &str,       // variable holding closure
    explicit_args: &[Value], // user-provided arguments
) -> Value {
    let fn_name = cx.closure_fn_map[closure_var].clone();
    let captures = cx.closure_captures[&fn_name].clone();

    // Build argument list: captures first, then explicit args
    let mut call_args = Vec::new();
    for cap in &captures {
        if let Some(&var) = cx.var_map.get(cap) {
            call_args.push(builder.use_var(var));
        }
    }
    call_args.extend_from_slice(explicit_args);

    // Direct call to lifted closure function
    let func_id = cx.functions[&fn_name];
    let func_ref = cx.module.declare_func_in_func(func_id, builder.func);
    let call = builder.ins().call(func_ref, &call_args);
    builder.inst_results(call)[0]
}
```

---

## 3. Concurrency Patterns (S5-S8 ✅ IMPLEMENTED)

### 3.1 Thread Spawn via Runtime

```rust
// Runtime function signature
extern "C" fn fj_rt_thread_spawn(
    fn_ptr: *const u8,      // function to execute
    arg_ptr: *mut u8,        // argument data (captures)
    arg_size: i64,           // size of argument data
) -> *mut ThreadHandle {
    let fn_ptr = fn_ptr as usize;

    // Copy argument data (captures must be owned by thread)
    let arg_data = if arg_size > 0 {
        let mut data = vec![0u8; arg_size as usize];
        // SAFETY: arg_ptr is valid for arg_size bytes
        unsafe { std::ptr::copy_nonoverlapping(arg_ptr, data.as_mut_ptr(), arg_size as usize); }
        Some(data)
    } else {
        None
    };

    let handle = std::thread::spawn(move || {
        // Reconstruct function pointer and call
        let func: extern "C" fn(*mut u8) -> i64 = unsafe { std::mem::transmute(fn_ptr) };
        let result = if let Some(mut data) = arg_data {
            func(data.as_mut_ptr())
        } else {
            func(std::ptr::null_mut())
        };
        result
    });

    Box::into_raw(Box::new(ThreadHandle { handle: Some(handle), result: None }))
}
```

### 3.2 Mutex via Runtime

```rust
struct FjMutex {
    inner: std::sync::Mutex<i64>,
}

extern "C" fn fj_rt_mutex_new(initial_value: i64) -> *mut FjMutex {
    Box::into_raw(Box::new(FjMutex {
        inner: std::sync::Mutex::new(initial_value),
    }))
}

extern "C" fn fj_rt_mutex_lock(mutex: *mut FjMutex) -> i64 {
    // SAFETY: mutex pointer valid, created by fj_rt_mutex_new
    let mutex = unsafe { &*mutex };
    let guard = mutex.inner.lock().unwrap_or_else(|e| e.into_inner());
    *guard
}

extern "C" fn fj_rt_mutex_unlock(mutex: *mut FjMutex, new_value: i64) {
    // SAFETY: mutex pointer valid
    let mutex = unsafe { &*mutex };
    let mut guard = mutex.inner.lock().unwrap_or_else(|e| e.into_inner());
    *guard = new_value;
    // Guard dropped here, releasing lock
}
```

### 3.3 Channel via Runtime

```rust
use std::sync::mpsc;

struct FjChannel {
    sender: mpsc::Sender<i64>,
    receiver: mpsc::Receiver<i64>,
}

extern "C" fn fj_rt_channel_new() -> *mut FjChannel {
    let (tx, rx) = mpsc::channel();
    Box::into_raw(Box::new(FjChannel { sender: tx, receiver: rx }))
}

extern "C" fn fj_rt_channel_send(channel: *mut FjChannel, value: i64) -> i64 {
    // SAFETY: channel pointer valid
    let ch = unsafe { &*channel };
    match ch.sender.send(value) {
        Ok(()) => 0,  // success
        Err(_) => 1,  // channel closed
    }
}

extern "C" fn fj_rt_channel_recv(channel: *mut FjChannel) -> i64 {
    // SAFETY: channel pointer valid
    let ch = unsafe { &*channel };
    match ch.receiver.recv() {
        Ok(val) => val,
        Err(_) => -1,  // channel closed, sentinel
    }
}
```

### 3.4 Atomic via Cranelift

```rust
// Cranelift has native atomic instructions
fn compile_atomic_load(
    builder: &mut FunctionBuilder,
    addr: Value,         // pointer to atomic
    ordering: MemFlags,  // memory ordering
) -> Value {
    builder.ins().atomic_load(types::I64, ordering, addr)
}

fn compile_atomic_store(
    builder: &mut FunctionBuilder,
    addr: Value,
    value: Value,
    ordering: MemFlags,
) {
    builder.ins().atomic_store(ordering, value, addr);
}

fn compile_atomic_cas(
    builder: &mut FunctionBuilder,
    addr: Value,
    expected: Value,
    desired: Value,
    ordering: MemFlags,
) -> Value {
    builder.ins().atomic_cas(ordering, addr, expected, desired)
}
```

---

## 4. Async/Await Patterns

### 4.1 State Machine Desugaring

```rust
// Original Fajar Lang code:
async fn fetch_two() -> i64 {
    let a = get_value(1).await    // await point 1
    let b = get_value(2).await    // await point 2
    a + b
}

// Desugared to state machine:
enum FetchTwoState {
    Start,
    AfterAwait1 { a: i64 },       // local 'a' saved across await
    Done,
}

struct FetchTwoFuture {
    state: FetchTwoState,
    future1: Option<GetValueFuture>,
    future2: Option<GetValueFuture>,
}

impl Future for FetchTwoFuture {
    type Output = i64;

    fn poll(&mut self, cx: &mut Context) -> Poll<i64> {
        loop {
            match self.state {
                FetchTwoState::Start => {
                    self.future1 = Some(get_value(1));
                    // Fall through to poll future1
                    match self.future1.as_mut().unwrap().poll(cx) {
                        Poll::Ready(a) => {
                            self.state = FetchTwoState::AfterAwait1 { a };
                            continue;
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
                FetchTwoState::AfterAwait1 { a } => {
                    self.future2 = Some(get_value(2));
                    match self.future2.as_mut().unwrap().poll(cx) {
                        Poll::Ready(b) => {
                            self.state = FetchTwoState::Done;
                            return Poll::Ready(a + b);
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
                FetchTwoState::Done => panic!("polled after completion"),
            }
        }
    }
}
```

### 4.2 Executor Pattern

```rust
struct Executor {
    tasks: VecDeque<Task>,
    ready_queue: Arc<Mutex<VecDeque<TaskId>>>,
}

impl Executor {
    fn block_on<F: Future>(&mut self, future: F) -> F::Output {
        let task = Task::new(future);
        let task_id = task.id;
        self.tasks.push_back(task);
        self.ready_queue.lock().unwrap().push_back(task_id);

        loop {
            // Drain ready queue
            let ready: Vec<TaskId> = {
                let mut q = self.ready_queue.lock().unwrap();
                q.drain(..).collect()
            };

            for id in ready {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
                    let waker = self.create_waker(id);
                    let mut cx = Context::from_waker(&waker);

                    match task.future.poll(&mut cx) {
                        Poll::Ready(val) => {
                            if id == task_id {
                                return val; // Main task done
                            }
                            task.completed = true;
                        }
                        Poll::Pending => {
                            // Task will be re-queued when waker fires
                        }
                    }
                }
            }
        }
    }
}
```

---

## 5. Inline Assembly Patterns

### 5.1 Strategy: Runtime Functions (NOT Cranelift native)

> **Research finding (2026-03-08):** Cranelift IR does NOT have an `InlineAsm` instruction.
> For v0.3 we use **Option C**: common kernel operations (port I/O, register access) are
> implemented as `extern "C"` runtime functions written in Rust (which HAS inline asm).
> Raw byte emission is deferred to v0.4.

```rust
// v0.3 approach: asm operations as Rust runtime functions
// These are called via fj_rt_* extern "C" functions from Cranelift codegen.

// Current implementation in runtime_fns.rs:
pub(crate) extern "C" fn fj_rt_asm_nop() {
    // SAFETY: nop is always safe
    #[cfg(target_arch = "x86_64")]
    unsafe { std::arch::asm!("nop") }
}

pub(crate) extern "C" fn fj_rt_compiler_fence() {
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

pub(crate) extern "C" fn fj_rt_memory_fence() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

// For kernel port I/O: also runtime functions wrapping Rust inline asm
```

### 5.2 Architecture-Specific Port I/O (x86_64)

```rust
// Fajar Lang source:
@kernel
fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack))
}

@kernel
fn inb(port: u16) -> u8 {
    let result: u8
    asm!("in al, dx", out("al") result, in("dx") port, options(nomem, nostack))
    result
}

// Compiled to (x86_64):
// outb: 0xEE (out dx, al) with register setup
// inb:  0xEC (in al, dx) with register read
```

---

## 6. GPU Compute Patterns

> **Design decision (2026-03-08):** Primary GPU backend is `wgpu` (cross-platform),
> NOT raw `ash`/Vulkan. The raw Vulkan pattern below is kept for reference only.
> CUDA FFI (§6.3) remains as optional NVIDIA-specific optimization.
> PTX strings must be pre-compiled via `nvcc`; runtime loads via `cuModuleLoadData`.

### 6.1 Vulkan Compute Pipeline (reference — prefer wgpu in practice)

```rust
fn create_vulkan_compute_pipeline(
    device: &ash::Device,
    shader_code: &[u32],    // SPIR-V
) -> vk::Pipeline {
    // 1. Create shader module
    let module_info = vk::ShaderModuleCreateInfo::default()
        .code(shader_code);
    let shader_module = unsafe { device.create_shader_module(&module_info, None) }.unwrap();

    // 2. Create pipeline layout (descriptor set bindings)
    let layout_info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&[descriptor_set_layout]);
    let layout = unsafe { device.create_pipeline_layout(&layout_info, None) }.unwrap();

    // 3. Create compute pipeline
    let stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(shader_module)
        .name(c"main");

    let pipeline_info = vk::ComputePipelineCreateInfo::default()
        .stage(stage)
        .layout(layout);

    unsafe { device.create_compute_pipelines(vk::PipelineCache::null(), &[pipeline_info], None) }
        .unwrap()[0]
}
```

### 6.2 SPIR-V Generation for Element-wise Ops

```rust
// Generate SPIR-V for: output[i] = relu(input[i])
fn generate_relu_spirv() -> Vec<u32> {
    // Use spirv_builder or manual SPIR-V assembly
    // Kernel: each invocation processes one element
    // global_id.x = element index
    // if input[id] > 0: output[id] = input[id]
    // else: output[id] = 0

    spirv_builder::Builder::new()
        .entry_point("main", ExecutionModel::GLCompute)
        .storage_buffer(0, 0) // input
        .storage_buffer(0, 1) // output
        .local_size(256, 1, 1)
        .body(|b| {
            let id = b.global_invocation_id_x();
            let val = b.load_buffer(0, id);
            let zero = b.const_f32(0.0);
            let result = b.select(b.gt(val, zero), val, zero);
            b.store_buffer(1, id, result);
        })
        .build()
}
```

### 6.3 CUDA FFI Pattern

```rust
// Dynamic CUDA loading — no compile-time dependency
use libloading::{Library, Symbol};

struct CudaContext {
    lib: Library,
    context: *mut std::ffi::c_void,
}

impl CudaContext {
    fn new() -> Result<Self, String> {
        let lib = unsafe { Library::new("libcuda.so") }
            .map_err(|e| format!("CUDA not available: {}", e))?;

        // Initialize CUDA
        let cu_init: Symbol<unsafe extern "C" fn(u32) -> i32> =
            unsafe { lib.get(b"cuInit") }.map_err(|e| e.to_string())?;
        let result = unsafe { cu_init(0) };
        if result != 0 {
            return Err(format!("cuInit failed: {}", result));
        }

        // Create context
        // ...

        Ok(CudaContext { lib, context })
    }

    fn launch_kernel(&self, ptx: &str, fn_name: &str, args: &[*mut u8], grid: (u32,u32,u32), block: (u32,u32,u32)) -> Result<(), String> {
        // cuModuleLoadData, cuModuleGetFunction, cuLaunchKernel
        // ...
        Ok(())
    }
}
```

---

## 7. Bare Metal Patterns

### 7.1 Linker Script for x86_64 Kernel

```ld
/* kernel.ld — Fajar Lang kernel linker script */
ENTRY(_start)

SECTIONS {
    . = 1M;  /* Load at 1MB (above BIOS/bootloader) */

    .text BLOCK(4K) : ALIGN(4K) {
        *(.multiboot)     /* Multiboot header first */
        *(.text.boot)     /* Boot code */
        *(.text .text.*)  /* All other code */
    }

    .rodata BLOCK(4K) : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data BLOCK(4K) : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss BLOCK(4K) : ALIGN(4K) {
        *(COMMON)
        *(.bss .bss.*)
    }

    /* Stack at top of memory */
    . = ALIGN(16);
    _stack_bottom = .;
    . += 64K;
    _stack_top = .;
}
```

### 7.2 Custom Allocator: Bump

```rust
// Simplest possible allocator for kernel early boot
struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: AtomicUsize,
}

impl BumpAllocator {
    const fn new(start: usize, size: usize) -> Self {
        BumpAllocator {
            heap_start: start,
            heap_end: start + size,
            next: AtomicUsize::new(start),
        }
    }
}

impl Allocator for BumpAllocator {
    fn allocate(&self, layout: Layout) -> *mut u8 {
        let align = layout.align;
        let size = layout.size;

        loop {
            let current = self.next.load(Ordering::Relaxed);
            let aligned = (current + align - 1) & !(align - 1);
            let new_next = aligned + size;

            if new_next > self.heap_end {
                return std::ptr::null_mut(); // OOM
            }

            if self.next.compare_exchange(current, new_next, Ordering::SeqCst, Ordering::Relaxed).is_ok() {
                return aligned as *mut u8;
            }
            // CAS failed, retry
        }
    }

    fn deallocate(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free individual allocations
        // Call reset() to free everything
    }
}
```

### 7.3 VGA Text Buffer via MMIO

```fajar
// Fajar Lang kernel code
@kernel
const VGA_BUFFER: usize = 0xB8000
const VGA_WIDTH: usize = 80
const VGA_HEIGHT: usize = 25

@kernel
fn vga_putchar(x: usize, y: usize, ch: u8, color: u8) {
    let offset = (y * VGA_WIDTH + x) * 2
    let addr = VGA_BUFFER + offset
    volatile_write(addr as *mut u8, ch)
    volatile_write((addr + 1) as *mut u8, color)
}

@kernel
fn vga_print(s: str, color: u8) {
    let mut x = 0
    let mut y = 0
    for i in 0..len(s) {
        let ch = s.bytes()[i]
        if ch == 10 {  // newline
            x = 0
            y = y + 1
        } else {
            vga_putchar(x, y, ch, color)
            x = x + 1
        }
    }
}
```

---

## 8. Tensor in Native Codegen Pattern

### 8.1 Tensor as Opaque Pointer

```rust
// Tensors represented as opaque *mut TensorValue in native codegen
// All operations go through runtime functions

extern "C" fn fj_rt_tensor_zeros(rows: i64, cols: i64) -> *mut TensorValue {
    let tensor = TensorValue::zeros(vec![rows as usize, cols as usize]);
    Box::into_raw(Box::new(tensor))
}

extern "C" fn fj_rt_tensor_matmul(a: *mut TensorValue, b: *mut TensorValue) -> *mut TensorValue {
    // SAFETY: pointers from fj_rt_tensor_* are valid
    let a = unsafe { &*a };
    let b = unsafe { &*b };
    let result = crate::runtime::ml::ops::matmul(a, b)
        .expect("matmul shape mismatch");
    Box::into_raw(Box::new(result))
}

// In codegen:
fn compile_tensor_builtin(cx: &mut CodegenCtx<'_, M>, builder: &mut FunctionBuilder, name: &str, args: &[Value]) -> Value {
    match name {
        "zeros" => {
            let func = cx.get_runtime_fn("fj_rt_tensor_zeros");
            let call = builder.ins().call(func, &[args[0], args[1]]);
            builder.inst_results(call)[0]  // returns *mut TensorValue as I64
        }
        "matmul" => {
            let func = cx.get_runtime_fn("fj_rt_tensor_matmul");
            let call = builder.ins().call(func, &[args[0], args[1]]);
            builder.inst_results(call)[0]
        }
        _ => todo!("tensor builtin: {}", name),
    }
}
```

---

## 9. ONNX Export Pattern

### 9.1 Layer to ONNX Node Mapping

```rust
fn layer_to_onnx_nodes(layer: &LayerValue, input: &str, output: &str) -> Vec<NodeProto> {
    match layer {
        LayerValue::Dense { weights, bias, .. } => {
            // Dense = MatMul(input, weights) + Add(bias)
            let matmul_out = format!("{}_matmul", output);
            vec![
                NodeProto {
                    op_type: "MatMul".to_string(),
                    input: vec![input.to_string(), format!("{}_weight", output)],
                    output: vec![matmul_out.clone()],
                    ..Default::default()
                },
                NodeProto {
                    op_type: "Add".to_string(),
                    input: vec![matmul_out, format!("{}_bias", output)],
                    output: vec![output.to_string()],
                    ..Default::default()
                },
            ]
        }
        LayerValue::Conv2d { .. } => {
            vec![NodeProto {
                op_type: "Conv".to_string(),
                input: vec![input.to_string(), format!("{}_weight", output)],
                output: vec![output.to_string()],
                attribute: vec![
                    // kernel_shape, strides, pads, etc.
                ],
                ..Default::default()
            }]
        }
        // ... other layers
    }
}
```

---

## 10. Error Code Extensions (v0.3)

```
Existing (v0.2): 71 codes across 9 categories

v0.3 additions:
KE005 — InlineAsmInSafe       "inline assembly not allowed in @safe context"
KE006 — InlineAsmInDevice     "inline assembly not allowed in @device context"
KE007 — VolatileInSafe        "volatile access not allowed in @safe context"

CE011 — IndirectCallTypeMismatch  "function pointer signature doesn't match arguments"
CE012 — FnPointerNotCallable     "variable is not a function pointer"
CE013 — GpuNotAvailable          "GPU compute not available on this system"
CE014 — UnsupportedTarget        "target architecture not supported for this operation"

TE009 — GpuTensorShapeMismatch   "GPU tensor shape doesn't match operation requirements"
TE010 — GpuMemoryExhausted       "GPU memory exhausted"

SE013 — AwaitOutsideAsync        "await can only be used inside async functions"
SE014 — NotSendable              "type is not Send — cannot be transferred between threads"
SE015 — NotSyncable              "type is not Sync — cannot be shared between threads"
SE016 — MutRefInSpawn            "&mut reference cannot be captured by thread::spawn"

RE009 — ThreadPanicked           "thread panicked during execution"
RE010 — ChannelClosed            "channel is closed"
RE011 — MutexPoisoned            "mutex poisoned by panicked thread"
RE012 — DeadlockDetected         "potential deadlock detected"

Total v0.3: ~87 error codes across 9 categories (+16 new)
```

---

## 11. Performance Patterns

### 11.1 Benchmark Template

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_thread_spawn_join(c: &mut Criterion) {
    c.bench_function("thread_spawn_join", |b| {
        b.iter(|| {
            let handle = std::thread::spawn(|| 42);
            handle.join().unwrap()
        })
    });
}

fn bench_channel_throughput(c: &mut Criterion) {
    c.bench_function("channel_1m_messages", |b| {
        b.iter(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let producer = std::thread::spawn(move || {
                for i in 0..1_000_000 {
                    tx.send(i).unwrap();
                }
            });
            let mut count = 0;
            while rx.recv().is_ok() {
                count += 1;
            }
            producer.join().unwrap();
            assert_eq!(count, 1_000_000);
        })
    });
}

criterion_group!(concurrency_benches, bench_thread_spawn_join, bench_channel_throughput);
criterion_main!(concurrency_benches);
```

---

*V03_SKILLS.md v1.0 — Implementation patterns for v0.3 "Dominion" | Created 2026-03-08*
