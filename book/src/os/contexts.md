# Context Annotations

Fajar's unique context annotation system enforces domain isolation at compile time.

## The Four Contexts

| Context | Heap | Tensors | Raw Pointers | IRQ/Syscall |
|---------|------|---------|--------------|-------------|
| `@safe` | Yes | No | No | No |
| `@kernel` | No | No | Yes | Yes |
| `@device` | Yes | Yes | No | No |
| `@unsafe` | Yes | Yes | Yes | Yes |

## @safe (Default)

The safest context. No hardware access, no raw pointers:

```fajar
@safe fn process(data: str) -> i64 {
    let result = parse_int(data)?
    result * 2
}
```

## @kernel

For OS-level code. Access to hardware, no heap allocation:

```fajar
@kernel fn interrupt_handler() {
    let key = port_read(0x60)
    // String::new()  -> KE001: heap not allowed
    // zeros(3,3)     -> KE002: tensor not allowed
}
```

## @device

For ML/compute code. Access to tensors, no hardware:

```fajar
@device fn forward(x: Tensor) -> Tensor {
    relu(matmul(x, weights))
    // port_read(0x60)  -> DE001: hardware not allowed
}
```

## @unsafe

Full access (escape hatch):

```fajar
@unsafe fn raw_access() {
    let ptr = mem_alloc(4096)
    let t = zeros(3, 3)
    port_write(0x64, 0xFE)
}
```

## Cross-Context Calls

| Caller | Can call @safe | Can call @kernel | Can call @device |
|--------|----------------|------------------|------------------|
| @safe | Yes | No | No |
| @kernel | Yes | Yes | No (KE003) |
| @device | Yes | No (DE002) | Yes |
| @unsafe | Yes | Yes | Yes |

## Why This Matters

Context annotations prevent entire classes of bugs:

- **No accidental hardware access from ML code** (buffer overflows, memory corruption)
- **No accidental tensor ops in kernel** (heap fragmentation, unpredictable latency)
- **Compile-time enforcement** (not runtime checks)
