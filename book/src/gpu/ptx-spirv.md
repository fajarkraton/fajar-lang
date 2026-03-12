# GPU Codegen (PTX & SPIR-V)

Fajar Lang can generate native GPU code for NVIDIA (PTX) and Vulkan (SPIR-V) targets.

## PTX (CUDA)

PTX (Parallel Thread Execution) is NVIDIA's GPU instruction set:

```fajar
@device
fn vector_add(a: Tensor, b: Tensor) -> Tensor {
    // Compiler generates PTX kernel:
    // .entry vector_add(.param .u64 a, .param .u64 b, .param .u64 result) {
    //     ld.param.u64 %rd1, [a];
    //     ld.param.u64 %rd2, [b];
    //     ...
    // }
    tensor_add(a, b)
}
```

PTX features: registers, shared memory, atomics, shuffle instructions, thread synchronization.

## SPIR-V (Vulkan)

SPIR-V is the intermediate representation for Vulkan compute shaders:

```fajar
@device
fn compute_shader(input: Tensor) -> Tensor {
    // Generates SPIR-V compute shader with:
    // - SSBOs for input/output buffers
    // - Barrier synchronization
    // - Workgroup local memory
    tensor_relu(input)
}
```

## Kernel Fusion

The compiler automatically fuses consecutive operations into a single GPU kernel:

```fajar
// These three operations are fused into one kernel launch:
let result = input
    |> tensor_matmul(weights)
    |> tensor_add(bias)
    |> tensor_relu
```

Fusion types:
- **Elementwise chains** — operations with same shape
- **Reduction chains** — reduce followed by elementwise
- **Memory planning** — minimize GPU memory traffic

## Device Memory Management

```fajar
// Allocate on GPU
let gpu_tensor = device_alloc(1024 * 1024)  // 1MB

// Transfer: Host → Device
device_copy(host_data, gpu_tensor, Direction::H2D)

// Compute on GPU
let result = gpu_compute(gpu_tensor)

// Transfer: Device → Host
device_copy(result, host_output, Direction::D2H)
```

The allocator uses a best-fit free-list strategy with fragmentation analysis.

## Multi-GPU

```fajar
let topology = gpu_topology()
println(f"GPUs: {topology.device_count}")
println(f"P2P: {topology.p2p_available}")

// Data parallelism across GPUs
let results = multi_gpu_map(data_shards, fn(shard) {
    forward(shard, model)
})
```
