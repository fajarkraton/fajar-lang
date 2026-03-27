# Lesson 12: Concurrency

## Objectives

By the end of this lesson, you will be able to:

- Spawn threads and wait for them to finish
- Communicate between threads with channels
- Protect shared data with mutexes
- Use `async`/`await` for asynchronous programming

## Threads

Threads let you run code in parallel. Use `thread::spawn` to create a new thread and `join()` to wait for it to finish.

```fajar
fn main() {
    let handle = thread::spawn(|| {
        for i in 0..5 {
            println(f"Thread: {i}")
        }
    })

    for i in 0..5 {
        println(f"Main: {i}")
    }

    handle.join()   // Wait for the thread to finish
}
```

Output will interleave -- both threads run concurrently.

### Returning Values from Threads

```fajar
fn main() {
    let handle = thread::spawn(|| {
        let mut sum = 0
        for i in 1..=100 {
            sum = sum + i
        }
        sum
    })

    let result = handle.join()
    println(f"Sum 1..100 = {result}")   // 5050
}
```

## Channels

Channels provide a safe way to send data between threads. One end sends, the other receives.

```fajar
fn main() {
    let (tx, rx) = channel::new()

    // Producer thread
    thread::spawn(move || {
        for i in 0..5 {
            tx.send(i * 10)
        }
    })

    // Consumer (main thread)
    for _ in 0..5 {
        let value = rx.recv()
        println(f"Received: {value}")
    }
}
```

**Expected output:**

```
Received: 0
Received: 10
Received: 20
Received: 30
Received: 40
```

### Multiple Producers

```fajar
fn main() {
    let (tx, rx) = channel::new()

    // Spawn 3 producer threads
    for id in 0..3 {
        let tx_clone = tx.clone()
        thread::spawn(move || {
            for i in 0..3 {
                tx_clone.send(f"Thread {id}: message {i}")
            }
        })
    }

    // Receive all 9 messages
    for _ in 0..9 {
        let msg = rx.recv()
        println(msg)
    }
}
```

## Mutexes

When multiple threads need to modify the same data, use a `Mutex` to ensure only one thread accesses it at a time.

```fajar
fn main() {
    let counter = Mutex::new(0)

    let mut handles = []

    for _ in 0..10 {
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                let mut guard = counter.lock()
                *guard = *guard + 1
            }
        })
        handles.push(handle)
    }

    for h in handles {
        h.join()
    }

    println(f"Counter: {counter.lock()}")   // 10000
}
```

Without the mutex, concurrent increments would lose updates (a data race).

## Atomics

For simple counters, atomics are faster than mutexes:

```fajar
fn main() {
    let count = AtomicI64::new(0)

    let mut handles = []
    for _ in 0..10 {
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                count.fetch_add(1)
            }
        })
        handles.push(handle)
    }

    for h in handles {
        h.join()
    }

    println(f"Count: {count.load()}")   // 10000
}
```

## Async/Await

For I/O-bound tasks (network, file), `async`/`await` is more efficient than threads because it does not block.

```fajar
async fn fetch_data(url: str) -> str {
    // Simulated async HTTP request
    let response = await http_get(url)
    response.body
}

async fn main() {
    let data = await fetch_data("https://api.example.com/data")
    println(data)
}
```

### Running Multiple Async Tasks

```fajar
async fn compute(id: i64) -> i64 {
    // Simulate some async work
    await sleep(100)
    id * id
}

async fn main() {
    // Run three tasks concurrently
    let results = await join_all([
        compute(1),
        compute(2),
        compute(3)
    ])

    for r in results {
        println(r)
    }
}
```

**Expected output:**

```
1
4
9
```

## Practical Example: Parallel Word Count

```fajar
fn count_words(text: str) -> i64 {
    text.split(" ").len()
}

fn main() {
    let documents = [
        "the quick brown fox",
        "jumped over the lazy dog",
        "hello world from fajar lang"
    ]

    let (tx, rx) = channel::new()

    for doc in documents {
        let tx_clone = tx.clone()
        thread::spawn(move || {
            let count = count_words(doc)
            tx_clone.send(count)
        })
    }

    let mut total = 0
    for _ in 0..len(documents) {
        total = total + rx.recv()
    }

    println(f"Total words: {total}")
}
```

**Expected output:**

```
Total words: 14
```

## Exercises

### Exercise 12.1: Parallel Sum (*)

Split an array of 1000 numbers across 4 threads. Each thread sums its portion and sends the result via a channel. The main thread collects and sums the partial results.

**Expected output:**

```
Total: 500500
```

### Exercise 12.2: Producer-Consumer (**)

Create a producer thread that generates numbers 1 through 20 and sends them on a channel. Create a consumer thread that receives each number, squares it, and prints the result. Use a "done" signal to indicate completion.

**Expected output:**

```
1
4
9
16
...
400
Done
```
