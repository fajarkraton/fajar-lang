# Annotations

Annotations in Fajar Lang begin with `@` and modify the behavior of functions, structs, and other declarations.

## Context Annotations

Context annotations control what operations are permitted inside a function. They enforce compile-time isolation between OS, ML, and general-purpose code.

### `@kernel`

Marks a function for OS kernel context. Enables raw memory, IRQ, and hardware access. Disables heap allocation and tensor operations.

**Allowed:** `mem_alloc`, `mem_free`, `port_read`, `port_write`, `irq_register`, `page_map`, raw pointers
**Forbidden:** `String::new()`, `zeros()`, `relu()`, calling `@device` functions

```fajar
@kernel
fn handle_page_fault(addr: usize) {
    let frame = mem_alloc(4096)
    page_map(addr, frame as usize, 0x03)
}
```

### `@device`

Marks a function for ML/device context. Enables tensor operations and neural network layers. Disables raw pointers and hardware access.

**Allowed:** `zeros`, `matmul`, `relu`, `Dense`, `backward`, calling `@safe` functions
**Forbidden:** raw pointers (`*mut T`), `irq_register`, `port_write`, `asm!()`

```fajar
@device
fn forward(x: Tensor) -> Tensor {
    let h = relu(matmul(x, weights))
    softmax(h)
}
```

### `@safe`

The default context. Provides general-purpose programming without direct hardware or tensor access. Can call both `@kernel` and `@device` functions, acting as a bridge.

```fajar
@safe
fn process_sensor() -> Action {
    let raw = read_sensor()              // calls @kernel
    let prediction = infer(raw)          // calls @device
    Action::from_prediction(prediction)
}
```

Since `@safe` is the default, it can be omitted:

```fajar
fn main() {
    println("This is implicitly @safe")
}
```

### `@unsafe`

Grants unrestricted access to all operations. Required for FFI calls and operations that cannot be verified safe by the compiler.

```fajar
@unsafe
fn raw_memory_op(ptr: *mut u8, tensor: Tensor) {
    mem_write(ptr, 0, 42)       // OK: kernel ops allowed
    let out = relu(tensor)       // OK: device ops allowed
}
```

### Context Compatibility Matrix

| Operation | `@safe` | `@kernel` | `@device` | `@unsafe` |
|-----------|---------|-----------|-----------|-----------|
| Variables, math, control flow | OK | OK | OK | OK |
| Heap allocation (`String`, `Vec`) | OK | Error KE001 | OK | OK |
| Tensor ops (`zeros`, `relu`) | Error | Error KE002 | OK | OK |
| Raw pointers (`*mut T`) | Error | OK | Error DE001 | OK |
| Hardware (`irq`, `port`) | Error | OK | Error DE002 | OK |
| Call `@kernel` fn | OK | OK | Error DE002 | OK |
| Call `@device` fn | OK | Error KE003 | OK | OK |

## Test Annotations

### `@test`

Marks a function as a test case. Test functions take no arguments and return void. Run with `fj test`.

```fajar
@test
fn test_addition() {
    assert_eq(2 + 2, 4)
}
```

### `@should_panic`

Used with `@test` to indicate the test should panic to pass. If the function completes without panicking, the test fails.

```fajar
@test
@should_panic
fn test_division_by_zero() {
    let _ = 1 / 0
}
```

### `@ignore`

Used with `@test` to skip the test during normal runs. Ignored tests can be run explicitly with `fj test --include-ignored`.

```fajar
@test
@ignore
fn test_slow_integration() {
    // This test takes 30 seconds
    run_full_benchmark()
}
```

## Entry Point Annotation

### `@entry`

Marks the program entry point. Used in bare-metal and OS kernel contexts where `main` is not the conventional entry.

```fajar
@entry
@kernel
fn _start() {
    // bare-metal initialization
    setup_gdt()
    setup_idt()
    init_memory()
}
```

## FFI Annotation

### `@ffi`

Marks a function as a foreign function interface declaration. The function body is provided by external (typically C) code linked at compile time.

```fajar
@ffi
fn c_malloc(size: usize) -> *mut u8

@ffi
fn c_free(ptr: *mut u8)

@unsafe
fn use_c_memory() {
    let ptr = c_malloc(1024)
    mem_write(ptr, 0, 42)
    c_free(ptr)
}
```

## Repr Annotations

### `@repr_c`

Ensures the struct has C-compatible memory layout for FFI.

```fajar
@repr_c
struct CPoint {
    x: f64,
    y: f64,
}
```

### `@repr_packed`

Removes padding between struct fields.

```fajar
@repr_packed
struct PackedHeader {
    magic: u16,
    version: u8,
    flags: u8,
}
```

## Other Annotations

### `@no_std`

Disables the standard library for bare-metal targets.

```fajar
@no_std
mod kernel {
    // no heap, no file I/O, no println
}
```

### `@simd`

Hints that a function should use SIMD instructions.

```fajar
@simd
fn vector_add(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0]+b[0], a[1]+b[1], a[2]+b[2], a[3]+b[3]]
}
```

### `@panic_handler`

Designates a custom panic handler for bare-metal environments.

```fajar
@panic_handler
@kernel
fn on_panic(msg: str) -> never {
    // write to serial port
    port_write(0x3F8, 'P' as u8)
    loop {}
}
```
