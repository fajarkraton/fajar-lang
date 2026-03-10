# Channels

Channels provide message-passing concurrency, inspired by Go's channels and Rust's `mpsc`. Fajar Lang supports both unbounded and bounded channels.

## Unbounded Channels

Unbounded channels have no capacity limit. Sends never block.

```fajar
let ch = Channel::new()

// Producer thread
let producer = Thread::spawn(fn(ch) -> i64 {
    Channel::send(ch, 1)
    Channel::send(ch, 2)
    Channel::send(ch, 3)
    0
}, ch)

// Consumer
let a = Channel::recv(ch)  // → 1
let b = Channel::recv(ch)  // → 2
let c = Channel::recv(ch)  // → 3

producer.join()
Channel::free(ch)
```

## Bounded Channels

Bounded channels have a fixed capacity. Sends block when full.

```fajar
let ch = Channel::bounded(2)  // capacity = 2

Channel::send(ch, 10)   // succeeds immediately
Channel::send(ch, 20)   // succeeds immediately
// Channel::send(ch, 30) would block until a recv

let val = Channel::recv(ch)  // → 10 (FIFO order)
```

### Try-Send

Non-blocking send attempt. Returns 1 on success, 0 if channel is full.

```fajar
let ch = Channel::bounded(1)
Channel::send(ch, 42)
let ok = Channel::try_send(ch, 99)  // → 0 (full)
```

## Closing Channels

Close a channel to signal no more messages will be sent. Receivers get 0 after the channel is drained.

```fajar
let ch = Channel::new()
Channel::send(ch, 42)
ch.close()

let val = Channel::recv(ch)  // → 42
let end = Channel::recv(ch)  // → 0 (closed, no more data)
```

## Pipeline Pattern

Chain channels together for multi-stage processing:

```fajar
let ch1 = Channel::new()
let ch2 = Channel::new()

// Stage 1: produce data
let p = Thread::spawn(fn(ch) -> i64 {
    Channel::send(ch, 10)
    Channel::send(ch, 20)
    0
}, ch1)

// Stage 2: transform
let t = Thread::spawn(fn(arg) -> i64 {
    let val = Channel::recv(ch1)
    Channel::send(ch2, val * 2)
    let val = Channel::recv(ch1)
    Channel::send(ch2, val * 2)
    0
}, 0)

let a = Channel::recv(ch2)  // → 20
let b = Channel::recv(ch2)  // → 40
```
