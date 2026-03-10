# Async/Await

Fajar Lang supports asynchronous programming with `async` functions and `.await` for non-blocking execution.

## Async Functions

Mark a function `async` to make it return a Future:

```fajar
async fn fetch_data() -> i64 {
    sleep(10)  // non-blocking delay
    42
}
```

## Awaiting Futures

Use `.await` to suspend execution until a future completes:

```fajar
async fn compute() -> i64 {
    let a = fetch_data().await   // suspends here
    let b = fetch_data().await   // then suspends here
    a + b
}
```

## Executor

The executor runs async tasks to completion:

```fajar
let exec = Executor::new()

// block_on: run a single future to completion
let result = Executor::block_on(future)

// spawn: queue a future for execution
Executor::spawn(exec, my_future)

// run: execute all spawned futures, returns count completed
let completed = Executor::run(exec)

Executor::free(exec)
```

## Sequential Awaits

Multiple `.await` points execute sequentially. Local variables are preserved across await points:

```fajar
async fn pipeline() -> i64 {
    let x = step1().await      // x = result of step1
    let y = step2(x).await     // y = result of step2(x)
    x + y                      // both values available
}
```

## Timers and Sleep

The `sleep()` builtin provides a simple delay:

```fajar
sleep(100)  // pause for 100 milliseconds
```

For more control, use the Timer wheel:

```fajar
let timer = Timer::new()
let waker = Waker::new()

timer.schedule(100, waker)  // fire after 100ms
timer.tick()                // check and fire expired timers
let remaining = timer.pending()  // count of unfired timers

timer.free()
waker.free()
```

## Comparison with Rust and Go

| Feature | Fajar Lang | Rust | Go |
|---------|-----------|------|-----|
| Async model | Eager (future runs immediately) | Lazy (needs .await to start) | Goroutines (implicit scheduling) |
| Runtime | Minimal executor | tokio/async-std | Built-in goroutine scheduler |
| Cancellation | Manual (free future) | Drop-based | Context cancellation |
| Thread safety | Send/Sync at compile time | Send/Sync at compile time | Race detector (runtime) |
| Channels | Typed, bounded/unbounded | mpsc/crossbeam | Built-in, typed |
| Syntax | `async fn` + `.await` | `async fn` + `.await` | `go func()` + `<-chan` |

### Key Differences

**vs Rust:** Fajar Lang uses an eager execution model (futures start executing when created) and has a simpler executor without the complexity of pinning or wakers. No lifetime annotations needed.

**vs Go:** Fajar Lang provides compile-time thread safety (Send/Sync) instead of Go's runtime race detector. Channels are explicit with bounded/unbounded variants rather than Go's single buffered channel type.
