# Runtime Management

Production runtime features for long-running Fajar Lang services.

## Graceful Shutdown

```fajar
use deployment::runtime

let shutdown = ShutdownController::new()

// Register shutdown hooks
shutdown.on_draining(fn() {
    // Stop accepting new connections
    listener.stop()
})

shutdown.on_flushing(fn() {
    // Flush pending writes
    logger.flush()
    metrics.flush()
})

shutdown.on_stopped(fn() {
    // Final cleanup
    db.close()
})

// Trigger shutdown (e.g., on SIGTERM)
shutdown.initiate()
// Phases: Running → Draining → Flushing → Stopped
```

## Hot Reload

Reload configuration without restarting:

```fajar
let config = HotReloadConfig::new("config.toml")

// Automatically detects file changes
config.on_change(fn(new_config) {
    update_log_level(new_config.log_level)
    update_rate_limits(new_config.rate_limits)
})
```

## Feature Flags

```fajar
let flags = FlagRegistry::new()

flags.register("new_algorithm", FlagState::Rollout(25))  // 25% of users

if flags.is_enabled("new_algorithm", user_id) {
    new_algorithm(data)
} else {
    old_algorithm(data)
}
```

Rollout is deterministic — the same user always gets the same result (hash-based).

## Connection Draining

```fajar
let drainer = ConnectionDrainer::new(timeout_ms: 30_000)

// On shutdown signal:
drainer.stop_accepting()     // No new connections
drainer.drain()              // Wait for in-flight to finish (max 30s)
```

## Process Supervision

```fajar
let supervisor = Supervisor::new(RestartPolicy::ExponentialBackoff {
    initial_delay_ms: 100,
    max_delay_ms: 30_000,
    max_retries: 10,
})

supervisor.run(fn() {
    start_worker()  // Automatically restarted on crash
})
```

## Memory Limits

```fajar
let limiter = MemoryLimiter::new(max_bytes: 512 * 1024 * 1024)  // 512MB

limiter.on_oom(OomAction::RejectNew)  // Or: DropOldest, WarnOnly
```

## Runtime Info

```bash
curl http://localhost:8080/info
```

Returns JSON with version, uptime, memory usage, thread count, and configuration.
