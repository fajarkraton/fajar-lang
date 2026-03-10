# Threads

Fajar Lang provides native thread support for parallel computation. Threads run OS-level threads via `Thread::spawn()`.

## Spawning Threads

```fajar
let handle = Thread::spawn(fn(arg) -> i64 { arg * 2 }, 21)
let result = handle.join()  // blocks until thread completes → 42
```

Threads accept a function pointer and an argument. The function runs on a new OS thread.

### No-Argument Threads

```fajar
let handle = Thread::spawn_noarg(fn() -> i64 { 100 })
let result = handle.join()  // → 100
```

## Thread Safety: Send and Sync

Fajar Lang enforces thread safety at compile time:

- **Send** types can be transferred across thread boundaries
- **Sync** types can be shared between threads via references

| Type | Send | Sync |
|------|------|------|
| `i64`, `f64`, `bool` | Yes | Yes |
| `String` | Yes | Yes |
| `Arc<T>` | Yes (if T: Send+Sync) | Yes |
| `Mutex<T>` | Yes (if T: Send) | Yes |
| `&mut T` | No | No |

Attempting to send a non-Send type across threads produces a compile error:

```fajar
// ERROR SE018: type &mut i64 is not Send
let handle = Thread::spawn(fn(r: &mut i64) -> i64 { *r }, &mut x)
```

## Checking Thread Status

```fajar
let handle = Thread::spawn(worker, 0)
let done = handle.is_finished()  // non-blocking: 1 if done, 0 if running
let result = handle.join()       // blocking: waits for completion
```

## Arc: Shared Ownership Across Threads

`Arc` (Atomic Reference Counted) allows multiple threads to share data:

```fajar
let shared = Arc::new(42)
let clone1 = Arc::clone(shared)
let clone2 = Arc::clone(shared)

let h1 = Thread::spawn(fn(a) -> i64 { Arc::load(a) }, clone1)
let h2 = Thread::spawn(fn(a) -> i64 { Arc::load(a) }, clone2)

h1.join()  // → 42
h2.join()  // → 42
Arc::drop(shared)
```
