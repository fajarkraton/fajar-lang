# Security — Fajar Lang Security Model

> Dokumen ini menjelaskan model keamanan Fajar Lang secara komprehensif.

## 1. Security Philosophy

**"Security by Construction"** — keamanan adalah properti yang dibuktikan oleh compiler, bukan konvensi yang diharapkan dari programmer.

Filosofi: **If it compiles, it's safe** (dalam domain yang telah dispesifikasikan).

## 2. Tiga Pilar Security

| Pilar | Mekanisme | Enforcement |
|-------|-----------|-------------|
| **Memory Safety** | No use-after-free, no null deref, no buffer overflow, no data race | Compiler (ownership + borrow checker) |
| **Context Isolation** | @kernel ≠ @device, no heap in kernel, no tensor in kernel, no pointer in @safe | Compiler (context analyzer) |
| **Type Safety** | PhysAddr ≠ VirtAddr, tensor shape check, no implicit cast, exhaustive match | Compiler (type checker) |

## 3. Security Layers

```
Layer 5: Application Security         → Developer responsibility
Layer 4: Safe Default (@safe)          → Compiler enforced
Layer 3: Memory Safety                 → Compiler enforced
Layer 2: Context Isolation             → Compiler enforced
Layer 1: Type Safety                   → Compiler enforced
Layer 0: Unsafe Boundary               → Manual audit, clearly marked
```

## 4. Context Isolation

### 4.1 @kernel Context

```fajar
@kernel
fn init_heap(base: addr<u8>, size: usize) {
    // ✅ ALLOWED:
    let region = alloc!(4096)
    map_page!(virt, phys, MEM_READ | MEM_WRITE)
    irq_register!(0x0E, page_fault_handler)
    let val: u8 = port_read!(0x60)

    // ❌ FORBIDDEN (compile errors):
    let s = String::new()     // error[KE001]: heap alloc in @kernel
    let t = zeros(3, 4)       // error[KE002]: tensor op in @kernel
}
```

### 4.2 @device Context

```fajar
@device(cpu)
fn forward(input: Tensor<f32>[784]) -> Tensor<f32>[10] {
    // ✅ ALLOWED: tensor ops, activations, autograd
    let h = relu(input @ W1 + b1)
    softmax(h @ W2 + b2)

    // ❌ FORBIDDEN (compile errors):
    let ptr: *mut u8 = ...      // error[DE001]: raw ptr in @device
    irq_register!(0x21, handler) // error[DE002]: IRQ in @device
    port_write!(0x60, 0xFF)      // error[DE002]: port I/O in @device
    map_page!(va, pa, flags)     // error[DE002]: page map in @device
}
```

### 4.3 @safe Context (Default)

```fajar
// @safe adalah default — tidak perlu ditulis secara eksplisit
@safe
fn compute_statistics(data: &[f32]) -> (f32, f32) {
    // ✅ ALLOWED: semua safe operations
    let mean = data.iter().sum::<f32>() / data.len() as f32
    let variance = data.iter()
        .map(|x| (x - mean) ** 2.0)
        .sum::<f32>() / data.len() as f32
    (mean, variance)

    // ❌ FORBIDDEN: hardware, raw pointers, kernel ops
    // semua @kernel dan @device ops dilarang
}
```

### 4.4 @unsafe Context

```fajar
// @unsafe harus eksplisit — setiap @unsafe block adalah audit target
@unsafe
fn dma_write(phys_addr: PhysAddr, data: &[u8]) {
    // SAFETY:
    //   1. phys_addr is verified to be DMA-capable memory region
    //   2. phys_addr + data.len() does not overflow physical address space
    //   3. No other core is accessing this region (caller holds DMA lock)
    //   4. data.len() is within device DMA transfer limits (max 4MB)
    let ptr = phys_addr.as_mut_ptr::<u8>()
    ptr.copy_from(data.as_ptr(), data.len())
}

// Semua @unsafe functions harus punya:
// 1. SAFETY comment yang menjelaskan preconditions
// 2. Unit test untuk setiap precondition violation
// 3. Fuzz test jika input berasal dari untrusted source
```

## 5. Type Safety

### 5.1 No Implicit Conversions

```fajar
let x: i32 = 42
let y: i64 = x           // COMPILE ERROR: no implicit widening

// Harus eksplisit:
let y: i64 = x as i64    // explicit cast
let y: i64 = i64::from(x) // trait-based conversion (preferred)

// Floating point:
let f: f32 = 3.14
let d: f64 = f            // COMPILE ERROR
let d: f64 = f64::from(f) // OK
```

### 5.2 Type-Safe Hardware Addresses

```fajar
// VirtAddr dan PhysAddr adalah DISTINCT TYPES
// — bukan alias untuk u64 — compiler membedakannya
struct VirtAddr(u64)
struct PhysAddr(u64)

fn map_page(va: VirtAddr, pa: PhysAddr, flags: PageFlags) { ... }

let va = VirtAddr::new(0xFFFF_8000_0000_0000)
let pa = PhysAddr::new(0x0000_0000_0010_0000)

map_page(va, pa, PageFlags::RW)            // ✅ correct
map_page(pa, va, PageFlags::RW)            // ❌ COMPILE ERROR: type mismatch
map_page(0xFFFF_8000..., 0x0000..., flags) // ❌ ERROR: u64 not VirtAddr
```

### 5.3 Tensor Shape Safety

```fajar
// Shape adalah bagian dari tipe — bukan metadata runtime
let w1: Tensor<f32>[784, 128]
let w2: Tensor<f32>[128, 64]

let r1 = w1 @ w2    // ✅ OK: [784,128] @ [128,64] = [784,64]
let r2 = w2 @ w1    // ❌ COMPILE ERROR: shape mismatch

// error[TE002]: matrix multiply shape mismatch
//   left: [128, 64]
//   right: [784, 128]
//   required: left.cols == right.rows (64 ≠ 784)
```

### 5.4 Exhaustive Match

```fajar
enum State { Running, Stopped, Error(str) }

match state {
    State::Running => ...,
    State::Stopped => ...,
    // ❌ COMPILE ERROR: non-exhaustive match
    // missing: State::Error(_)
}
```

## 6. Memory Safety

### 6.1 Ownership (Move Semantics)

```fajar
let data = [1, 2, 3, 4, 5]
let copy = data           // MOVE: data is no longer valid
println(data)             // ❌ COMPILE ERROR: use after move
// error[ME001]: use of moved value 'data'
```

### 6.2 Borrow Rules

```fajar
let mut arr = [1, 2, 3]
let r1 = &arr             // immutable borrow
let r2 = &mut arr         // ❌ COMPILE ERROR
// error[ME003]: cannot borrow as mutable because also borrowed as immutable
```

**Rule:** Pada satu waktu, boleh ada BANYAK immutable borrow ATAU SATU mutable borrow, tidak keduanya.

### 6.3 Null Safety

```fajar
// Tidak ada null di Fajar Lang
// Ketiadaan nilai direpresentasikan dengan Option<T>

fn find_user(id: u64) -> Option<User> {
    if exists(id) { Some(load_user(id)) }
    else { None }
}

// Compiler MEMAKSA penanganan kedua kasus:
match find_user(42) {
    Some(user) => println(user.name),
    None => println("Not found"),
}
```

### 6.4 Integer Overflow

```fajar
let a: u8 = 255
let b = a + 1     // debug: PANIC; release: wraps to 0

// Explicit overflow handling:
let c = a.wrapping_add(1)    // always wraps → 0
let d = a.checked_add(1)     // Option<u8> = None
let e = a.saturating_add(1)  // 255 (saturates at max)
```

### 6.5 Bounds Checking

```fajar
let arr = [1, 2, 3, 4, 5]
let val = arr[10]           // RuntimeError: index out of bounds
let maybe = arr.get(10)     // Option<i32> = None (safe)

// In @kernel: unchecked access possible but must be explicit
@kernel
fn fast_read(arr: &[u8], idx: usize) -> u8 {
    // SAFETY: caller guarantees idx < arr.len()
    @unsafe { arr.get_unchecked(idx) }
}
```

## 7. Unsafe Boundary

### 7.1 Rules for @unsafe

1. **Must be explicit** — no implicit unsafe
2. **Must have SAFETY comment** — compiler warns if missing
3. **Must be minimal** — smallest possible unsafe block
4. **Must be auditable** — `fj audit` generates report of all unsafe usage

### 7.2 Capability-Based Unsafe (Future Enhancement)

```fajar
// Instead of full unsafe, specify exactly what capabilities you need:
@unsafe(capability: [raw_pointer, port_io])
fn device_init() {
    // Only raw pointer and port I/O allowed
    // Other unsafe operations are still forbidden
}
```

## 8. Security Tooling

### 8.1 Audit Command

```bash
fj audit --report
```

```
# Security Report: fajar-lang v0.1.0
# ====================================
# @unsafe blocks:          12
#   - With SAFETY docs:    10 (83%)
#   - With tests:          11 (92%)
#   - With fuzz tests:      4 (33%)
#
# @kernel functions:       28
#   - All verified:        28 (100%) ✅
#
# Null checks bypassed:     0 ✅
# Unchecked array access:   0 ✅
# Missing error handling:   3 ⚠️
#   (see details with --verbose)
```

### 8.2 Fuzzing Support (Phase 6+)

```fajar
// Tandai function sebagai fuzz target
@fuzz
fn parse_packet(data: &[u8]) -> Result<Packet, ParseError> {
    // Fajar Lang akan generate fuzz harness otomatis
    // dan jalankan dengan AFL++ atau libFuzzer
}
```

## 9. Threat Model

### 9.1 Yang Dilindungi

| Ancaman | Mekanisme Perlindungan | Jaminan |
|---------|------------------------|---------|
| Buffer overflow | Bounds checking, owned slices | Compiler + Runtime |
| Use-after-free | Ownership system | Compiler |
| Null dereference | `Option<T>`, no null literal | Compiler |
| Integer overflow | Checked arithmetic default | Runtime (debug) |
| Data race | Single-owner or borrow rules | Compiler |
| Type confusion | Strong static typing, no implicit casts | Compiler |
| Context violation | @kernel/@device/@safe enforcement | Compiler |
| Shape mismatch | Compile-time tensor shape analysis | Compiler |
| Address confusion | VirtAddr ≠ PhysAddr distinct types | Compiler |

### 9.2 Yang TIDAK Dilindungi (Developer Responsibility)

- Logic errors (wrong algorithm)
- Incorrect SAFETY comments
- Timing side-channels
- Physical hardware attacks
- Social engineering
- Supply chain attacks pada dependencies

## 10. Compliance Targets

| Standard | Relevance | Status |
|----------|-----------|--------|
| MISRA (Automotive) | @kernel context restrictions align with MISRA subset | Phase 7 |
| DO-178C (Aerospace) | Formal verification hooks planned | Phase 7 |
| IEC 62304 (Medical) | Compile-time safety guarantees for medical AI devices | Phase 7 |
| ISO 26262 (Automotive) | Context isolation + type safety for ADAS systems | Phase 7 |

---

*Security Version: 1.0 | Philosophy: Security by Construction*
