# Atomic Operations

Atomics provide lock-free concurrent access to shared integers. They are the lowest-level synchronization primitive and the building block for higher-level abstractions.

## Creating and Using Atomics

```fajar
let counter = Atomic::new(0)

Atomic::store(counter, 42)
let val = Atomic::load(counter)  // → 42

Atomic::free(counter)
```

## Arithmetic Operations

All arithmetic operations are atomic and return the **previous** value:

```fajar
let a = Atomic::new(10)

let prev = Atomic::add(a, 5)    // prev = 10, new value = 15
let prev = Atomic::sub(a, 3)    // prev = 15, new value = 12
```

## Bitwise Operations

```fajar
let a = Atomic::new(0xFF)

let prev = Atomic::and(a, 0x0F)  // prev = 0xFF, new = 0x0F
let prev = Atomic::or(a, 0xF0)   // prev = 0x0F, new = 0xFF
let prev = Atomic::xor(a, 0xFF)  // prev = 0xFF, new = 0x00
```

## Compare-and-Swap (CAS)

CAS is the fundamental atomic operation. It atomically compares the current value with `expected` and, if equal, replaces it with `desired`. Returns the value that was found.

```fajar
let a = Atomic::new(42)

// CAS succeeds: old value (42) matches expected (42)
let old = Atomic::cas(a, 42, 100)  // old = 42, new value = 100

// CAS fails: old value (100) doesn't match expected (42)
let old = Atomic::cas(a, 42, 200)  // old = 100, value unchanged
```

## Memory Fences

Memory fences enforce ordering of memory operations:

```fajar
Atomic::fence()  // full memory barrier (SeqCst)
```

## Pattern: Spinlock

Build a simple spinlock using CAS:

```fajar
let lock = Atomic::new(0)  // 0 = unlocked, 1 = locked

// Acquire: spin until CAS succeeds
fn acquire(lock) {
    loop {
        let old = Atomic::cas(lock, 0, 1)
        if old == 0 { break }  // acquired
    }
}

// Release
Atomic::store(lock, 0)
```

## Pattern: Lock-Free Counter

Multiple threads incrementing without locks:

```fajar
let counter = Atomic::new(0)

let h1 = Thread::spawn(fn(c) -> i64 {
    let mut i = 0
    while i < 1000 {
        Atomic::add(c, 1)
        i = i + 1
    }
    0
}, counter)

let h2 = Thread::spawn(fn(c) -> i64 {
    let mut i = 0
    while i < 1000 {
        Atomic::add(c, 1)
        i = i + 1
    }
    0
}, counter)

h1.join()
h2.join()
let total = Atomic::load(counter)  // → 2000 (always correct)
```

## Performance

Atomics are significantly faster than mutex-based synchronization for simple operations:

| Operation | Throughput |
|-----------|-----------|
| Atomic add | ~309M ops/sec |
| Atomic CAS | ~290M ops/sec |
| Mutex lock/unlock | ~70M ops/sec |

Use atomics for counters, flags, and simple shared state. Use mutexes when you need to protect complex data structures.
