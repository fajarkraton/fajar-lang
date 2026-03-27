# Security Guide

Fajar Lang enforces security through its type system and context annotations.
This guide covers the security model, safe coding practices, and auditing.

## Context Annotations

Fajar Lang uses four context levels to enforce isolation:

| Context | Purpose | Allowed | Forbidden |
|---------|---------|---------|-----------|
| `@safe` | Default, safest subset | Basic computation | Raw pointers, hardware, tensors |
| `@kernel` | OS kernel code | Pointers, IRQ, memory | Heap allocation, tensors |
| `@device` | ML/device code | Tensors, accelerators | Raw pointers, IRQ |
| `@unsafe` | Full access | Everything | Nothing forbidden |

### How Contexts Work

```fajar
@kernel
fn kernel_code() {
    page_alloc(4096)           // OK -- kernel can allocate pages
    let t = zeros(3, 3)       // ERROR KE002 -- no tensors in kernel
}

@device
fn device_code() {
    let t = zeros(3, 3)       // OK -- device can use tensors
    let p: *mut u8 = 0 as _   // ERROR DE001 -- no raw pointers in device
}

@safe
fn safe_code() {
    let x = 42                 // OK -- basic computation
    page_alloc(4096)           // ERROR -- no hardware in safe context
}
```

### Cross-Context Calls

```fajar
@safe fn bridge() {
    let raw = read_sensor()     // Can call @kernel functions
    let pred = classify(raw)    // Can call @device functions
}

@kernel fn read_sensor() -> [f32; 4] { ... }
@device fn classify(data: [f32; 4]) -> i32 { ... }
```

The `@safe` context acts as a bridge between `@kernel` and `@device`,
which cannot call each other directly.

## Memory Safety

### Ownership Prevents Use-After-Free

```fajar
let data = create_buffer()
let moved = data              // data is moved
println(data)                 // COMPILE ERROR ME001: use after move
```

### Borrow Rules Prevent Data Races

```fajar
let mut x = 42
let r1 = &x                  // OK: immutable borrow
let r2 = &x                  // OK: multiple immutable borrows
let w = &mut x                // ERROR ME003: cannot borrow mutably while immutably borrowed
```

### No Null Pointer Dereference

```fajar
// There is no null -- use Option<T>
fn find(id: i32) -> Option<User> {
    if id > 0 { Some(user) } else { None }
}

// Must handle the None case
match find(42) {
    Some(user) => println(user.name),
    None => println("Not found"),
}
```

## Type Safety

### No Implicit Conversions

```fajar
let x: i32 = 42
let y: i64 = x               // ERROR SE004 -- must use explicit cast
let y: i64 = x as i64        // OK
```

### Distinct Address Types

```fajar
@kernel
fn map_page(virt: VirtAddr, phys: PhysAddr) {
    // Cannot accidentally swap virtual and physical addresses
    // They are distinct types even though both are u64 internally
}
```

## Safe Coding Practices

### Input Validation

```fajar
fn process_input(data: str) -> Result<Output, str> {
    if len(data) > MAX_INPUT_SIZE {
        return Err("Input too large")
    }
    if !is_valid_utf8(data) {
        return Err("Invalid encoding")
    }
    // Process validated input
    Ok(parse(data)?)
}
```

### Bounds Checking

Array access is bounds-checked at runtime:

```fajar
let arr = [1, 2, 3]
let val = arr[5]              // Runtime error RE005: index out of bounds
```

### Integer Overflow

Overflow is checked in debug mode:

```fajar
let x: i32 = 2147483647
let y = x + 1                // Runtime error RE001: integer overflow (debug)
                              // Wraps silently in release mode
```

## Auditing Your Code

### Static Analysis

```bash
fj check --strict my_project/    # Full type check with extra warnings
```

### Count Unsafe Blocks

Search for `@unsafe` usage and audit each one:

```bash
grep -rn "@unsafe" src/
```

Every `@unsafe` block should have a comment explaining why it is needed
and what invariants the programmer guarantees.

### Dependency Audit

Review third-party packages:

```bash
fj audit                         # Check dependencies for known issues
```

## Security Checklist

- [ ] Minimize `@unsafe` usage -- prefer `@safe` or `@kernel`/`@device`
- [ ] Validate all external input before processing
- [ ] Use `Result`/`Option` instead of panicking
- [ ] Never store secrets in source code
- [ ] Use `fj-crypto` for cryptographic operations (not custom implementations)
- [ ] Review all FFI boundaries for safety
- [ ] Run `fj check --strict` in CI
