# Mutexes and Synchronization

Fajar Lang provides several synchronization primitives for protecting shared state.

## Mutex

A Mutex (mutual exclusion lock) protects shared data from concurrent access.

```fajar
let m = Mutex::new(0)

// Lock returns the current value
let val = Mutex::lock(m)     // → 0
Mutex::store(m, val + 1)     // update protected value

let val2 = Mutex::lock(m)    // → 1
Mutex::free(m)
```

### Try-Lock

Non-blocking lock attempt:

```fajar
let m = Mutex::new(42)
let result = Mutex::try_lock(m)  // → 42 (success)
// If another thread holds the lock, try_lock returns 0 with failure flag
```

## RwLock

A reader-writer lock allows multiple concurrent readers OR one exclusive writer.

```fajar
let rw = RwLock::new(100)

// Multiple readers can read simultaneously
let val = RwLock::read(rw)    // → 100
// Another thread can also read at the same time

// Writers get exclusive access
RwLock::write(rw, 200)
let val = RwLock::read(rw)    // → 200

RwLock::free(rw)
```

## Condition Variables

Condvars allow threads to wait for a condition to become true.

```fajar
let m = Mutex::new(0)
let cv = Condvar::new()

// Worker thread: wait for signal
let worker = Thread::spawn(fn(arg) -> i64 {
    Condvar::wait(cv, m)  // releases mutex, waits, re-acquires
    Mutex::lock(m)
}, 0)

// Signal the worker
Mutex::store(m, 42)
Condvar::notify_one(cv)   // wake one waiting thread
// Condvar::notify_all(cv)  // wake ALL waiting threads

worker.join()
```

## Barrier

A Barrier synchronizes multiple threads at a rendezvous point.

```fajar
let barrier = Barrier::new(3)  // 3 threads must arrive

// Each thread calls wait — blocks until all 3 arrive
let h1 = Thread::spawn(fn(b) -> i64 { Barrier::wait(b) }, barrier)
let h2 = Thread::spawn(fn(b) -> i64 { Barrier::wait(b) }, barrier)
Barrier::wait(barrier)  // this thread is the 3rd — all proceed

h1.join()
h2.join()
Barrier::free(barrier)
```

## Pattern: Thread-Safe Counter

```fajar
let counter = Mutex::new(0)

let h1 = Thread::spawn(fn(m) -> i64 {
    let val = Mutex::lock(m)
    Mutex::store(m, val + 1)
    0
}, counter)

let h2 = Thread::spawn(fn(m) -> i64 {
    let val = Mutex::lock(m)
    Mutex::store(m, val + 1)
    0
}, counter)

h1.join()
h2.join()
let final_val = Mutex::lock(counter)  // → 2
```
