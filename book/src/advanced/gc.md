# Garbage Collection Modes

By default, Fajar Lang uses ownership-based memory management (like Rust). For rapid prototyping or scripting, optional GC modes are available.

## Memory Modes

| Mode | Flag | Description | Use Case |
|------|------|-------------|----------|
| Owned | (default) | Move semantics, RAII | Production, embedded |
| RefCounted | `--gc rc` | Reference counting + cycle collector | Scripting, prototyping |
| Tracing | `--gc tracing` | Tri-color mark-sweep | Large heap, complex graphs |

```bash
fj run --gc rc script.fj        # reference counting
fj run --gc tracing script.fj   # tracing GC
```

## Reference Counting

Each allocation carries a strong and weak reference count. When the strong count reaches zero, the object is freed.

```fajar
let a = [1, 2, 3]      // refcount = 1
let b = a               // refcount = 2 (shared, not moved)
// b goes out of scope → refcount = 1
// a goes out of scope → refcount = 0 → freed
```

### Cycle Collection

Cycles (A → B → A) would leak with naive reference counting. The cycle collector uses DFS to detect and break cycles.

## Tracing GC

Tri-color mark-sweep with generational collection:

- **Young generation** — frequent, fast collections (most objects die young)
- **Old generation** — infrequent, full collections
- **Write barriers** — track cross-generation references

### Finalization

Objects with destructors are finalized before collection:

```fajar
struct Connection {
    fd: i32,
}

impl Drop for Connection {
    fn drop(&mut self) {
        close_fd(self.fd)  // Called by GC before freeing
    }
}
```

## Kernel Restriction

`@kernel` context prohibits all GC modes — only ownership-based management is allowed in kernel code. This ensures predictable latency with no GC pauses.

```fajar
@kernel
fn interrupt_handler() {
    // No GC pauses possible here
    // Only stack allocation and explicit mem_alloc/mem_free
}
```

## Benchmarks

| Metric | Owned | RefCounted | Tracing |
|--------|-------|------------|---------|
| Throughput | Highest | Medium | Medium |
| Latency | Predictable | Predictable | Pauses |
| Memory overhead | None | 16 bytes/obj | Mark bits |
| Cycle handling | N/A (compiler) | Collector | Automatic |
