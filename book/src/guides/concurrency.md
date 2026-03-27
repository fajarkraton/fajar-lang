# Concurrency Guide

Fajar Lang provides threads, channels, mutexes, atomics, and async/await
for concurrent programming. All are designed to prevent data races at
compile time through the ownership system.

## Threads

Spawn a thread and wait for it to complete:

```fajar
use std::thread

fn main() {
    let handle = thread::spawn(|| {
        println("Hello from a thread!")
        42
    })

    let result = handle.join()
    println(f"Thread returned: {result}")
}
```

### Move Semantics with Threads

Data is moved into threads by default. The compiler prevents shared
mutable access across threads:

```fajar
let mut data = [1, 2, 3, 4, 5]

let handle = thread::spawn(move || {
    // `data` is moved into this thread
    data.push(6)
    data
})

// data is no longer accessible here -- moved into the thread
let result = handle.join()
```

## Channels

Channels provide message-passing between threads:

```fajar
use std::channel

fn main() {
    let (tx, rx) = channel::new()

    thread::spawn(move || {
        tx.send("hello")
        tx.send("world")
    })

    println(rx.recv())    // "hello"
    println(rx.recv())    // "world"
}
```

### Multiple Producers

Clone the sender for fan-in patterns:

```fajar
let (tx, rx) = channel::new()

for i in 0..4 {
    let tx_clone = tx.clone()
    thread::spawn(move || {
        tx_clone.send(f"Worker {i} done")
    })
}

for _ in 0..4 {
    println(rx.recv())
}
```

## Mutexes

Protect shared state with a mutex:

```fajar
use std::sync::{Mutex, Arc}

fn main() {
    let counter = Arc::new(Mutex::new(0))
    let mut handles = []

    for _ in 0..10 {
        let c = counter.clone()
        let h = thread::spawn(move || {
            let mut val = c.lock()
            *val += 1
        })  // MutexGuard drops here via RAII
        handles.push(h)
    }

    for h in handles {
        h.join()
    }

    println(f"Counter: {*counter.lock()}")
}
```

## Atomic Operations

For simple counters, atomics avoid mutex overhead:

```fajar
use std::sync::AtomicI64

let counter = AtomicI64::new(0)
counter.fetch_add(1, Ordering::SeqCst)
let val = counter.load(Ordering::SeqCst)
```

## Async/Await

For I/O-bound workloads, use async functions:

```fajar
async fn fetch_data(url: str) -> Result<str, str> {
    let response = http_get(url).await?
    Ok(response.body)
}

async fn main() {
    let a = fetch_data("https://api.example.com/a")
    let b = fetch_data("https://api.example.com/b")

    // Run both concurrently
    let (result_a, result_b) = join(a, b).await
    println(f"A: {result_a}, B: {result_b}")
}
```

### Task Spawning

```fajar
async fn serve() {
    loop {
        let conn = listener.accept().await
        spawn(async move {
            handle_connection(conn).await
        })
    }
}
```

## Patterns

### Worker Pool

```fajar
fn worker_pool(tasks: [Task], num_workers: i32) -> [Result] {
    let (tx_task, rx_task) = channel::new()
    let (tx_result, rx_result) = channel::new()

    for _ in 0..num_workers {
        let rx = rx_task.clone()
        let tx = tx_result.clone()
        thread::spawn(move || {
            while let Some(task) = rx.recv() {
                tx.send(task.execute())
            }
        })
    }

    for task in tasks { tx_task.send(task) }
    tasks.iter().map(|_| rx_result.recv()).collect()
}
```

### Select on Multiple Channels

```fajar
select {
    msg = rx1.recv() => println(f"Channel 1: {msg}"),
    msg = rx2.recv() => println(f"Channel 2: {msg}"),
    timeout(1000)    => println("Timed out"),
}
```
