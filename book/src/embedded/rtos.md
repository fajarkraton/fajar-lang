# RTOS Integration

Fajar Lang integrates with real-time operating systems for predictable, deadline-driven embedded applications.

## FreeRTOS

```fajar
use os::rtos::freertos

@kernel
fn main() {
    // Create tasks with priorities
    freertos::task_create("sensor", 2, fn() {
        loop {
            let data = read_sensor()
            freertos::queue_send(sensor_queue, data)
            freertos::delay_ms(100)
        }
    })

    freertos::task_create("control", 3, fn() {
        loop {
            let data = freertos::queue_receive(sensor_queue)
            let action = compute_control(data)
            actuate(action)
        }
    })

    freertos::start_scheduler()
}
```

## RTOS Primitives

| Primitive | Description |
|-----------|-------------|
| `task_create(name, priority, fn)` | Create a task with given priority |
| `queue_send(q, item)` | Send item to queue |
| `queue_receive(q)` | Receive from queue (blocks) |
| `mutex_take(m)` | Lock mutex |
| `mutex_give(m)` | Unlock mutex |
| `semaphore_take(s)` | Take semaphore |
| `semaphore_give(s)` | Give semaphore |
| `timer_create(name, period, fn)` | Create software timer |
| `delay_ms(ms)` | Delay task for milliseconds |

## Zephyr

```fajar
use os::rtos::zephyr

@kernel
fn main() {
    zephyr::thread_create("worker", 1024, 5, fn() {
        // Thread with 1KB stack, priority 5
    })
}
```

## RTIC (Compile-Time Scheduling)

RTIC provides zero-cost interrupt-driven concurrency:

```fajar
@rtic
mod app {
    #[task(binds = TIM2, priority = 2)]
    fn timer_handler() {
        toggle_led()
    }

    #[task(binds = EXTI0, priority = 3)]
    fn button_handler() {
        trigger_measurement()
    }
}
```

Priority ceiling protocol ensures deadlock-free access to shared resources at compile time.

## Real-Time Annotations

```fajar
@realtime(deadline_us = 1000)
fn control_loop() {
    // Compiler warns if this function might exceed 1ms
}

@realtime(period_ms = 10)
fn periodic_task() {
    // Must complete within 10ms period
}
```
