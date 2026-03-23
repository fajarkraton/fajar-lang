# Depth Completion Plan — Closing 8 End-to-End Gaps

> **Goal:** Setiap fitur yang di-checkbox harus berfungsi end-to-end, bukan hanya infrastruktur
> **Method:** 1 Engineer + Claude AI
> **Hardware:** i9-14900HX, RTX 4090, QEMU x86+ARM64
> **Estimated:** 5 hari kerja (~30 jam)

---

## Gap Matrix

| # | Gap | Infrastruktur | Missing End-to-End | Effort | Priority |
|---|-----|-------------|-------------------|--------|----------|
| B | `fj build --all` discovers but doesn't compile | fj.toml parsing, cmd_build_all | Per-target codegen invocation | 4h | CRITICAL |
| C | User runtime exists but no proof | runtime_user.rs, set_user_mode | User ELF boots in QEMU Ring 3 | 3h | CRITICAL |
| D | @message size check but ipc_send not typed | IPC001, message_structs | ipc_send(pid, VfsOpen{}) type-checked | 3h | HIGH |
| E | Protocol completeness but no client stubs | Missing method → error | VfsClient::open() auto-generates IPC | 4h | HIGH |
| F | Vulkan wired but no benchmark proof | VulkanBackend implements trait | GPU vs CPU matmul timing | 2h | HIGH |
| G | Safetensors header only | parse_safetensors_header | Load actual tensor data from file | 3h | MEDIUM |
| H | GGUF header only | parse_gguf_header | Load actual tensor data from file | 4h | MEDIUM |
| A | $ token lexes (already complete) | — | — | 0h | DONE |

**Total: ~23h across 7 gaps (A is already complete)**

---

## Execution Order

```
Day 1 (6h):  Gap B — fj build --all compiles real ELFs
Day 2 (6h):  Gap C — User ELF Ring 3 proof + Gap D — ipc_send type check
Day 3 (6h):  Gap E — Client stub generation + Gap F — GPU benchmark
Day 4 (7h):  Gap G — Safetensors data loading + Gap H — GGUF data loading
Day 5 (3h):  End-to-end integration test + final push
```

---

## Gap B: `fj build --all` Must Compile Real ELFs (4h)

### Current State

```rust
// cmd_build_all currently:
// 1. Reads fj.toml ✅
// 2. Discovers kernel + services ✅
// 3. Prints "service 'vfs': path → build/services/vfs.elf" ✅
// 4. Does NOT actually invoke fj build per target ❌
```

### What Must Happen

```rust
// For each target in fj.toml:
// 1. Read source (directory or file)
// 2. Call cmd_build_native(path, target, output) per service
// 3. Produce actual .elf files in build/
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| B.1 | Extract build logic from cmd_build_native into reusable fn | `build_single_target(path, target, output) -> ExitCode` |
| B.2 | Loop over kernel + services in cmd_build_all | Call build_single_target for each |
| B.3 | Handle per-target linker scripts | Kernel: x86_64-none + linker.ld, Service: x86_64-user |
| B.4 | Test: `fj build --all` on test project produces 3 ELFs | Verify files exist and are valid ELF |
| B.5 | Test: FajarOS fj.toml with `fj build --all` | At minimum kernel.elf compiles |

### Acceptance

```
□ fj build --all produces build/kernel.elf (real ELF, not empty)
□ fj build --all produces build/services/vfs.elf (if service sources exist)
□ file build/kernel.elf → "ELF 64-bit LSB executable, x86-64"
□ Test: 5+ new tests
```

---

## Gap C: User ELF Must Boot in Ring 3 (3h)

### Current State

```
runtime_user.rs: fj_rt_user_print, fj_rt_user_exit, etc. ✅
set_user_mode(true) in cmd_build_native ✅
User linker script at 0x400000 ✅
Actually building + running a user ELF in QEMU: ❌
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| C.1 | Write minimal user program: `@safe fn main() { println(42) }` | examples/user_hello.fj |
| C.2 | Compile with `fj build --target x86_64-user` | Produces user ELF |
| C.3 | Verify ELF: file type, entry point, sections | `readelf -h` or `file` |
| C.4 | Test: user ELF structure valid | 3+ tests |
| C.5 | Document: how user ELFs work with FajarOS | Boot sequence diagram |

### Acceptance

```
□ fj build --target x86_64-user examples/user_hello.fj → user ELF
□ ELF has _start entry, .text at 0x400000
□ SYSCALL stubs linked (fj_rt_user_*)
□ Test: 3+ new tests
```

---

## Gap D: ipc_send Must Type-Check @message Arg (3h)

### Current State

```
@message struct VfsOpen { ... } → registered in message_structs ✅
IPC001: size > 64 bytes → error ✅
ipc_send(pid, VfsOpen { ... }) → NOT type-checked ❌
(ipc_send takes i64, i64 — raw args, no struct awareness)
```

### What Must Happen

When calling `ipc_send`, if the second argument is a struct init expression and the struct has `@message` annotation, verify it's a valid @message type.

### Tasks

| # | Task | Detail |
|---|------|--------|
| D.1 | In check_expr for Call: detect ipc_send callee | Match on function name "ipc_send" |
| D.2 | Check second arg: if struct init, verify @message | Look up struct name in message_structs |
| D.3 | IPC002 error if non-@message struct passed | "ipc_send expects @message struct" |
| D.4 | Allow raw i64 args (backward compatible) | Only warn/error on struct args |
| D.5 | Tests: 5+ | @message OK, non-@message → error, raw i64 OK |

### Acceptance

```
□ ipc_send(pid, VfsOpen { ... }) → OK (VfsOpen is @message)
□ ipc_send(pid, Point { ... }) → IPC002 error (Point is NOT @message)
□ ipc_send(pid, 42) → OK (raw i64, backward compatible)
□ Test: 5+ new tests
```

---

## Gap E: Protocol Client Stubs Must Generate (4h)

### Current State

```
protocol VfsProtocol { fn open(...) } → parsed as TraitDef ✅
service vfs implements VfsProtocol → completeness checked ✅
VfsClient::open(path) → auto-generates IPC call → NOT implemented ❌
```

### Architecture

```
// Compiler generates from protocol:
struct VfsClient { pid: i64 }

impl VfsClient {
    fn open(path_len: i64, flags: i64) -> i64 {
        // Auto-generated:
        let msg = [MSG_ID_OPEN, path_len, flags, 0, 0, 0, 0, 0]
        ipc_call(self.pid, msg_ptr, reply_ptr)
        reply[0]  // return first field
    }
}
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| E.1 | Collect protocol methods during analysis | Store in protocol_methods: HashMap |
| E.2 | Generate client struct AST | `struct {Proto}Client { pid: i64 }` |
| E.3 | Generate method stubs | Each protocol fn → IPC call wrapper |
| E.4 | Register generated items in scope | Client struct usable from @safe code |
| E.5 | Tests: 5+ | VfsClient::open parses, analyzes |

### Acceptance

```
□ protocol VfsProto { fn open() } → VfsProtoClient struct auto-created
□ VfsProtoClient::open() calls ipc_call internally
□ @safe code can use VfsProtoClient
□ Test: 5+ new tests
```

---

## Gap F: GPU Matmul Benchmark Proof (2h)

### Current State

```
VulkanBackend implements TensorBackend ✅
RTX 4090 detected via VulkanCompute::is_available() ✅
Actual timing comparison GPU vs CPU: ❌
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| F.1 | Write benchmark: 256×256 matmul CPU vs GPU | Measure both, print times |
| F.2 | Write benchmark: 1024×1024 matmul | Larger matrix = bigger speedup |
| F.3 | Verify results match (f32 tolerance) | GPU result ≈ CPU result |
| F.4 | Print speedup factor | "GPU: 0.5ms, CPU: 50ms, speedup: 100x" |
| F.5 | Test: result correctness (gated behind vulkan feature) | 3+ tests |

### Acceptance

```
□ cargo test --features vulkan gpu_benchmark → prints timing
□ GPU faster than CPU for 1024×1024 matmul
□ Results match within f32 tolerance (1e-3)
□ Test: 3+ new tests (feature-gated)
```

---

## Gap G: Safetensors Data Loading (3h)

### Current State

```
parse_safetensors_header: reads header size + tensor names ✅
Actual tensor data extraction from bytes: ❌
```

### Safetensors Format Detail

```
[8B header_size] [JSON header] [tensor data bytes...]

JSON: {
  "weight": { "dtype": "F32", "shape": [3, 4], "data_offsets": [0, 48] },
  "bias":   { "dtype": "F32", "shape": [4],    "data_offsets": [48, 64] }
}
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| G.1 | Parse data_offsets from JSON header | Extract [start, end] per tensor |
| G.2 | Parse dtype from JSON | "F32" → DType::F32, "F16" → DType::F16 |
| G.3 | Parse shape from JSON | [3, 4] → vec![3, 4] |
| G.4 | Extract raw bytes → Vec<f64> | Read bytes at offset, convert per dtype |
| G.5 | Return ModelTensor with real data | name + shape + dtype + data populated |
| G.6 | Test with synthetic safetensors file | Create in-memory, load, verify values |
| G.7 | Tests: 5+ | Load, dtype parse, shape parse, roundtrip |

### Acceptance

```
□ load_model("test.safetensors") returns tensors with actual data
□ Tensor shapes match JSON header
□ Tensor values are correct f64 (from f32 bytes)
□ Test: 5+ with synthetic file
```

---

## Gap H: GGUF Data Loading (4h)

### Current State

```
parse_gguf_header: reads magic + version + tensor count ✅
GgufQuantType: F32/F16/Q4_0/Q8_0 codes ✅
Actual tensor metadata + data extraction: ❌
```

### GGUF Format Detail

```
[4B "GGUF"] [4B version] [8B tensor_count] [8B metadata_kv_count]
[metadata KV entries...]
[tensor info entries: name, ndims, dims, type, offset]
[alignment padding]
[tensor data blocks]
```

### Tasks

| # | Task | Detail |
|---|------|--------|
| H.1 | Parse metadata KV entries | Read key type, value type, data |
| H.2 | Parse tensor info entries | name (string), ndims, dims[], type, offset |
| H.3 | Read GGUF string format | [8B length] [N bytes UTF-8] |
| H.4 | Extract tensor data at offset | Seek to data_offset + tensor_offset |
| H.5 | Dequantize Q4_0/Q8_0 → f64 | Block dequantization (32 elements/block) |
| H.6 | Return ModelTensor with real data | Full tensor loaded |
| H.7 | Test with synthetic GGUF file | Create minimal valid GGUF, load |
| H.8 | Tests: 5+ | Header, metadata, tensor info, dequantize |

### Q4_0 Dequantization

```
Block (20 bytes): [f16 scale] [16 bytes = 32 × 4-bit values]
For each 4-bit value q: real = scale * (q - 8)
```

### Acceptance

```
□ Full GGUF parsing: header + metadata + tensor info + data
□ Q8_0 dequantize → correct f64 values
□ Q4_0 dequantize → correct f64 values (within tolerance)
□ load_model("model.gguf") returns tensors with actual data
□ Test: 5+ with synthetic GGUF
```

---

## Day 5: End-to-End Integration Test (3h)

### The Ultimate Test

```bash
# This single script must pass:
./tools/test_end_to_end.sh

Steps:
1. fj build --all on test project → 3 ELFs produced
2. file *.elf → all valid ELF x86-64
3. fj build --target x86_64-user user_hello.fj → user ELF valid
4. @message type-check: VfsOpen OK, Point → IPC002
5. Protocol client: VfsProtoClient generated
6. GPU benchmark: RTX 4090 faster than CPU (--features vulkan)
7. Safetensors: load synthetic file, verify tensor values
8. GGUF: load synthetic file, verify dequantized values
9. FajarOS x86 QEMU: boots, serial output
10. FajarOS ARM64 QEMU: boots, serial output
11. All 6,000+ tests pass
```

### Acceptance: "100% Deep"

```
□ fj build --all → real ELFs (not just discovery)
□ User ELF has correct structure (entry, sections)
□ ipc_send type-checks @message structs
□ Protocol generates client stubs
□ GPU provably faster than CPU
□ Safetensors loads real tensor data
□ GGUF loads and dequantizes real tensor data
□ Both x86 + ARM64 boot in QEMU
□ 6,000+ tests (est. 5,862 + ~40 new)
□ Zero "1 inch deep" features remaining
```

---

## Updated Metrik

| Metrik | Before | After Depth Completion |
|--------|--------|----------------------|
| Checkbox completion | 173/173 (100%) | 173/173 (100%) |
| End-to-end depth | ~85% (8 gaps) | **100%** (0 gaps) |
| `fj build --all` | Discover only | **Compile real ELFs** |
| IPC type safety | Size check only | **Call-site type check** |
| Protocol stubs | Completeness only | **Auto-generated client** |
| GPU proof | Wired only | **Benchmark: Nx speedup** |
| Model loading | Header only | **Full tensor extraction** |
| Tests | 5,862 | ~6,000+ |

---

*"Every checkbox backed by end-to-end proof. No '1 inch deep' features."*
*Estimated: 5 days, ~30 hours*
