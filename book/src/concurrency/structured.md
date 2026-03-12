# Structured Concurrency

Fajar Lang provides structured concurrency primitives that guarantee all spawned tasks complete before their parent scope exits.

## Task Scopes

```fajar
fn process_data(items: [Item]) -> [Result] {
    scope(fn(s) {
        let mut handles = []
        for item in items {
            let h = s.spawn(fn() { process(item) })
            handles.push(h)
        }
        // All tasks are automatically joined when scope exits
        handles.iter().map(fn(h) { h.join() }).collect()
    })
}
```

Unlike raw threads, `scope` guarantees:
- All child tasks finish before the scope returns
- If any task panics, all others are cancelled
- No dangling references — children can borrow from parent

## Nurseries

The nursery pattern (inspired by Trio) provides even stricter lifetime management:

```fajar
fn fetch_all(urls: [str]) -> [Response] {
    nursery(fn(n) {
        for url in urls {
            n.spawn(fn() { http_get(url) })
        }
        // If any fetch fails, all others are cancelled
    })
}
```

## Cancellation

Cooperative cancellation via tokens:

```fajar
let token = CancellationToken::new()

scope(fn(s) {
    s.spawn(fn() {
        while !token.is_cancelled() {
            do_work()
        }
    })

    // Cancel after 5 seconds
    s.spawn(fn() {
        sleep_ms(5000)
        token.cancel()
    })
})
```

## Flow Control

### Backpressure

```fajar
let flow = FlowControl::backpressure(max_pending: 100)

for item in producer {
    flow.acquire()  // Blocks if 100 items pending
    spawn(fn() {
        process(item)
        flow.release()
    })
}
```

### Rate Limiting

```fajar
let limiter = FlowControl::rate_limit(max_per_second: 1000)

for request in requests {
    limiter.acquire()  // Throttles to 1000 req/s
    handle(request)
}
```

### Concurrency Limiting

```fajar
let sem = ConcurrencyLimiter::new(max_concurrent: 10)

for task in tasks {
    sem.acquire()  // At most 10 concurrent tasks
    spawn(fn() {
        run(task)
        sem.release()
    })
}
```
