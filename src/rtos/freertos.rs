//! FreeRTOS FFI bindings for Fajar Lang.
//!
//! Provides Rust-side type definitions modeling the FreeRTOS API, with
//! extern "C" FFI declarations behind `#[cfg(feature = "freertos")]` and
//! simulation stubs for testing when the feature is not enabled.
//!
//! # API Groups
//!
//! - **Task API**: create, delete, delay, suspend, resume tasks
//! - **Queue API**: create, send, receive with timeout
//! - **Mutex API**: create, lock, unlock with priority inheritance
//! - **Semaphore API**: binary and counting semaphores
//! - **Timer API**: software timers with auto-reload
//! - **ISR-safe variants**: `_from_isr` versions of queue/semaphore ops
//! - **FreeRTOSConfig.h generation**: per-BSP board configuration

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from FreeRTOS FFI operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FreeRtosError {
    /// Task creation failed (insufficient memory or invalid params).
    #[error("task creation failed: {reason}")]
    TaskCreateFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid task handle.
    #[error("invalid task handle: {handle}")]
    InvalidTaskHandle {
        /// The invalid handle value.
        handle: u64,
    },

    /// Queue operation timed out.
    #[error("queue operation timed out after {timeout_ticks} ticks")]
    QueueTimeout {
        /// Number of ticks waited.
        timeout_ticks: u32,
    },

    /// Queue is full (send failed).
    #[error("queue full (length: {length})")]
    QueueFull {
        /// Maximum queue length.
        length: u32,
    },

    /// Queue is empty (receive failed).
    #[error("queue empty")]
    QueueEmpty,

    /// Invalid queue handle.
    #[error("invalid queue handle: {handle}")]
    InvalidQueueHandle {
        /// The invalid handle value.
        handle: u64,
    },

    /// Mutex operation timed out.
    #[error("mutex lock timed out after {timeout_ticks} ticks")]
    MutexTimeout {
        /// Number of ticks waited.
        timeout_ticks: u32,
    },

    /// Invalid mutex handle.
    #[error("invalid mutex handle: {handle}")]
    InvalidMutexHandle {
        /// The invalid handle value.
        handle: u64,
    },

    /// Mutex is not locked (unlock failed).
    #[error("mutex not locked")]
    MutexNotLocked,

    /// Semaphore operation timed out.
    #[error("semaphore take timed out after {timeout_ticks} ticks")]
    SemaphoreTimeout {
        /// Number of ticks waited.
        timeout_ticks: u32,
    },

    /// Semaphore count exceeded maximum.
    #[error("semaphore count at maximum ({max_count})")]
    SemaphoreAtMax {
        /// Maximum count.
        max_count: u32,
    },

    /// Invalid semaphore handle.
    #[error("invalid semaphore handle: {handle}")]
    InvalidSemaphoreHandle {
        /// The invalid handle value.
        handle: u64,
    },

    /// Timer creation failed.
    #[error("timer creation failed: {reason}")]
    TimerCreateFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid timer handle.
    #[error("invalid timer handle: {handle}")]
    InvalidTimerHandle {
        /// The invalid handle value.
        handle: u64,
    },

    /// Invalid priority value.
    #[error("invalid priority: {priority} (max: {max})")]
    InvalidPriority {
        /// The invalid priority value.
        priority: u32,
        /// Maximum allowed priority.
        max: u32,
    },

    /// Invalid stack size.
    #[error("invalid stack size: {size} (min: {min})")]
    InvalidStackSize {
        /// The invalid stack size.
        size: u32,
        /// Minimum allowed stack size.
        min: u32,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Maximum wait time (block indefinitely).
pub const PORT_MAX_DELAY: u32 = u32::MAX;

/// Minimum stack size in words (128 words = 512 bytes on 32-bit).
pub const CONFIG_MINIMAL_STACK_SIZE: u32 = 128;

/// Maximum priority levels (0 = idle, max-1 = highest).
pub const CONFIG_MAX_PRIORITIES: u32 = 56;

/// Tick rate in Hz (default: 1000 = 1ms tick).
pub const CONFIG_TICK_RATE_HZ: u32 = 1000;

// ═══════════════════════════════════════════════════════════════════════
// Handle types
// ═══════════════════════════════════════════════════════════════════════

/// Opaque task handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskHandle(pub u64);

/// Opaque queue handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueueHandle(pub u64);

/// Opaque mutex handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MutexHandle(pub u64);

/// Opaque semaphore handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemaphoreHandle(pub u64);

/// Opaque timer handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerHandle(pub u64);

// ═══════════════════════════════════════════════════════════════════════
// Task state
// ═══════════════════════════════════════════════════════════════════════

/// FreeRTOS task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Task is ready to run.
    Ready,
    /// Task is running (current task).
    Running,
    /// Task is blocked (waiting for event/timeout).
    Blocked,
    /// Task is suspended.
    Suspended,
    /// Task has been deleted.
    Deleted,
}

/// Task priority (0 = idle priority, higher = more urgent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskPriority(pub u32);

impl TaskPriority {
    /// Idle task priority (lowest).
    pub const IDLE: Self = Self(0);
    /// Low priority.
    pub const LOW: Self = Self(1);
    /// Normal priority.
    pub const NORMAL: Self = Self(2);
    /// High priority.
    pub const HIGH: Self = Self(3);
    /// Real-time priority.
    pub const REALTIME: Self = Self(CONFIG_MAX_PRIORITIES - 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated task
// ═══════════════════════════════════════════════════════════════════════

/// Simulated FreeRTOS task descriptor.
#[derive(Debug, Clone)]
struct SimTask {
    /// Task name.
    name: String,
    /// Task priority.
    priority: TaskPriority,
    /// Stack size in words.
    _stack_size: u32,
    /// Function pointer (simulated as a name/id).
    _fn_id: u64,
    /// Current task state.
    state: TaskState,
    /// Remaining delay ticks (if blocked on delay).
    delay_remaining: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated queue
// ═══════════════════════════════════════════════════════════════════════

/// Simulated FreeRTOS queue.
#[derive(Debug, Clone)]
struct SimQueue {
    /// Items in the queue (stored as raw bytes/u64 values).
    items: Vec<u64>,
    /// Maximum number of items.
    max_length: u32,
    /// Item size in bytes.
    _item_size: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated mutex
// ═══════════════════════════════════════════════════════════════════════

/// Simulated FreeRTOS mutex.
#[derive(Debug, Clone)]
struct SimMutex {
    /// Whether the mutex is locked.
    locked: bool,
    /// Task that holds the lock (if any).
    holder: Option<TaskHandle>,
    /// Whether priority inheritance is enabled.
    _priority_inheritance: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated semaphore
// ═══════════════════════════════════════════════════════════════════════

/// Simulated FreeRTOS semaphore.
#[derive(Debug, Clone)]
struct SimSemaphore {
    /// Current count.
    count: u32,
    /// Maximum count.
    max_count: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated timer
// ═══════════════════════════════════════════════════════════════════════

/// Simulated FreeRTOS software timer.
#[derive(Debug, Clone)]
struct SimTimer {
    /// Timer name.
    name: String,
    /// Period in ticks.
    period_ticks: u32,
    /// Whether the timer auto-reloads.
    auto_reload: bool,
    /// Callback function ID.
    callback_fn_id: u64,
    /// Whether the timer is running.
    running: bool,
    /// Ticks remaining until next expiry.
    ticks_remaining: u32,
    /// Number of times the timer has expired.
    expiry_count: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// FreeRTOS simulation runtime
// ═══════════════════════════════════════════════════════════════════════

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Generates a unique handle value.
fn next_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

/// Simulated FreeRTOS runtime for testing without real hardware.
///
/// This struct provides the full FreeRTOS API surface using in-memory
/// simulation, allowing unit tests and development without a target board.
#[derive(Debug)]
pub struct FreeRtosRuntime {
    /// Registered tasks.
    tasks: HashMap<u64, SimTask>,
    /// Registered queues.
    queues: HashMap<u64, SimQueue>,
    /// Registered mutexes.
    mutexes: HashMap<u64, SimMutex>,
    /// Registered semaphores.
    semaphores: HashMap<u64, SimSemaphore>,
    /// Registered timers.
    timers: HashMap<u64, SimTimer>,
    /// Whether the scheduler is running.
    scheduler_running: bool,
    /// Current tick count.
    tick_count: u64,
}

impl FreeRtosRuntime {
    /// Creates a new FreeRTOS simulation runtime.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            queues: HashMap::new(),
            mutexes: HashMap::new(),
            semaphores: HashMap::new(),
            timers: HashMap::new(),
            scheduler_running: false,
            tick_count: 0,
        }
    }

    /// Returns the current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Returns whether the scheduler is running.
    pub fn is_scheduler_running(&self) -> bool {
        self.scheduler_running
    }

    // ─── Task API ─────────────────────────────────────────────────────

    /// Creates a new task (xTaskCreate equivalent).
    ///
    /// # Arguments
    /// * `name` - Task name (max 16 chars in real FreeRTOS)
    /// * `priority` - Task priority (0..CONFIG_MAX_PRIORITIES)
    /// * `stack_size` - Stack size in words (min CONFIG_MINIMAL_STACK_SIZE)
    /// * `fn_ptr` - Function pointer (simulated as u64 ID)
    pub fn task_create(
        &mut self,
        name: &str,
        priority: u32,
        stack_size: u32,
        fn_ptr: u64,
    ) -> Result<TaskHandle, FreeRtosError> {
        if priority >= CONFIG_MAX_PRIORITIES {
            return Err(FreeRtosError::InvalidPriority {
                priority,
                max: CONFIG_MAX_PRIORITIES - 1,
            });
        }
        if stack_size < CONFIG_MINIMAL_STACK_SIZE {
            return Err(FreeRtosError::InvalidStackSize {
                size: stack_size,
                min: CONFIG_MINIMAL_STACK_SIZE,
            });
        }

        let handle = next_handle();
        let task = SimTask {
            name: name.to_string(),
            priority: TaskPriority(priority),
            _stack_size: stack_size,
            _fn_id: fn_ptr,
            state: TaskState::Ready,
            delay_remaining: 0,
        };
        self.tasks.insert(handle, task);
        Ok(TaskHandle(handle))
    }

    /// Deletes a task (vTaskDelete equivalent).
    pub fn task_delete(&mut self, handle: TaskHandle) -> Result<(), FreeRtosError> {
        let task = self
            .tasks
            .get_mut(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        task.state = TaskState::Deleted;
        Ok(())
    }

    /// Delays a task for the given number of ticks (vTaskDelay equivalent).
    pub fn task_delay(&mut self, handle: TaskHandle, ticks: u32) -> Result<(), FreeRtosError> {
        let task = self
            .tasks
            .get_mut(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        task.state = TaskState::Blocked;
        task.delay_remaining = ticks;
        Ok(())
    }

    /// Suspends a task (vTaskSuspend equivalent).
    pub fn task_suspend(&mut self, handle: TaskHandle) -> Result<(), FreeRtosError> {
        let task = self
            .tasks
            .get_mut(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        task.state = TaskState::Suspended;
        Ok(())
    }

    /// Resumes a suspended task (vTaskResume equivalent).
    pub fn task_resume(&mut self, handle: TaskHandle) -> Result<(), FreeRtosError> {
        let task = self
            .tasks
            .get_mut(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        if task.state == TaskState::Suspended {
            task.state = TaskState::Ready;
        }
        Ok(())
    }

    /// Returns the state of a task.
    pub fn task_state(&self, handle: TaskHandle) -> Result<TaskState, FreeRtosError> {
        let task = self
            .tasks
            .get(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        Ok(task.state)
    }

    /// Returns the name of a task.
    pub fn task_name(&self, handle: TaskHandle) -> Result<&str, FreeRtosError> {
        let task = self
            .tasks
            .get(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        Ok(&task.name)
    }

    /// Returns the priority of a task.
    pub fn task_priority(&self, handle: TaskHandle) -> Result<TaskPriority, FreeRtosError> {
        let task = self
            .tasks
            .get(&handle.0)
            .ok_or(FreeRtosError::InvalidTaskHandle { handle: handle.0 })?;
        Ok(task.priority)
    }

    /// Returns the number of active (non-deleted) tasks.
    pub fn task_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.state != TaskState::Deleted)
            .count()
    }

    // ─── Queue API ────────────────────────────────────────────────────

    /// Creates a queue (xQueueCreate equivalent).
    ///
    /// # Arguments
    /// * `length` - Maximum number of items
    /// * `item_size` - Size of each item in bytes
    pub fn queue_create(
        &mut self,
        length: u32,
        item_size: u32,
    ) -> Result<QueueHandle, FreeRtosError> {
        if length == 0 {
            return Err(FreeRtosError::QueueFull { length: 0 });
        }
        let handle = next_handle();
        let queue = SimQueue {
            items: Vec::new(),
            max_length: length,
            _item_size: item_size,
        };
        self.queues.insert(handle, queue);
        Ok(QueueHandle(handle))
    }

    /// Sends an item to a queue (xQueueSend equivalent).
    ///
    /// Returns `Err(QueueFull)` if the queue is full and timeout would expire.
    pub fn queue_send(
        &mut self,
        queue: QueueHandle,
        item: u64,
        _timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let q = self
            .queues
            .get_mut(&queue.0)
            .ok_or(FreeRtosError::InvalidQueueHandle { handle: queue.0 })?;
        if q.items.len() >= q.max_length as usize {
            return Err(FreeRtosError::QueueFull {
                length: q.max_length,
            });
        }
        q.items.push(item);
        Ok(())
    }

    /// Receives an item from a queue (xQueueReceive equivalent).
    ///
    /// Returns `Err(QueueEmpty)` if no item is available.
    pub fn queue_receive(
        &mut self,
        queue: QueueHandle,
        _timeout_ticks: u32,
    ) -> Result<u64, FreeRtosError> {
        let q = self
            .queues
            .get_mut(&queue.0)
            .ok_or(FreeRtosError::InvalidQueueHandle { handle: queue.0 })?;
        if q.items.is_empty() {
            return Err(FreeRtosError::QueueEmpty);
        }
        // FIFO: remove from front
        Ok(q.items.remove(0))
    }

    /// Returns the number of items waiting in a queue.
    pub fn queue_messages_waiting(&self, queue: QueueHandle) -> Result<u32, FreeRtosError> {
        let q = self
            .queues
            .get(&queue.0)
            .ok_or(FreeRtosError::InvalidQueueHandle { handle: queue.0 })?;
        Ok(q.items.len() as u32)
    }

    /// ISR-safe queue send (xQueueSendFromISR equivalent).
    ///
    /// Same as `queue_send` in simulation but marks that it was
    /// called from ISR context (no blocking allowed).
    pub fn queue_send_from_isr(
        &mut self,
        queue: QueueHandle,
        item: u64,
    ) -> Result<bool, FreeRtosError> {
        let q = self
            .queues
            .get_mut(&queue.0)
            .ok_or(FreeRtosError::InvalidQueueHandle { handle: queue.0 })?;
        if q.items.len() >= q.max_length as usize {
            return Ok(false); // ISR variant returns false instead of blocking
        }
        q.items.push(item);
        Ok(true) // higher_priority_task_woken
    }

    /// ISR-safe queue receive (xQueueReceiveFromISR equivalent).
    pub fn queue_receive_from_isr(
        &mut self,
        queue: QueueHandle,
    ) -> Result<Option<u64>, FreeRtosError> {
        let q = self
            .queues
            .get_mut(&queue.0)
            .ok_or(FreeRtosError::InvalidQueueHandle { handle: queue.0 })?;
        if q.items.is_empty() {
            return Ok(None);
        }
        Ok(Some(q.items.remove(0)))
    }

    // ─── Mutex API ────────────────────────────────────────────────────

    /// Creates a mutex (xSemaphoreCreateMutex equivalent).
    pub fn mutex_create(&mut self) -> MutexHandle {
        let handle = next_handle();
        let mutex = SimMutex {
            locked: false,
            holder: None,
            _priority_inheritance: true,
        };
        self.mutexes.insert(handle, mutex);
        MutexHandle(handle)
    }

    /// Locks a mutex (xSemaphoreTake on mutex equivalent).
    ///
    /// Returns `Err(MutexTimeout)` if already locked and timeout expires.
    pub fn mutex_lock(
        &mut self,
        mutex: MutexHandle,
        timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let m = self
            .mutexes
            .get_mut(&mutex.0)
            .ok_or(FreeRtosError::InvalidMutexHandle { handle: mutex.0 })?;
        if m.locked {
            return Err(FreeRtosError::MutexTimeout { timeout_ticks });
        }
        m.locked = true;
        Ok(())
    }

    /// Locks a mutex and records which task holds it.
    pub fn mutex_lock_by(
        &mut self,
        mutex: MutexHandle,
        task: TaskHandle,
        timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let m = self
            .mutexes
            .get_mut(&mutex.0)
            .ok_or(FreeRtosError::InvalidMutexHandle { handle: mutex.0 })?;
        if m.locked {
            return Err(FreeRtosError::MutexTimeout { timeout_ticks });
        }
        m.locked = true;
        m.holder = Some(task);
        Ok(())
    }

    /// Unlocks a mutex (xSemaphoreGive on mutex equivalent).
    pub fn mutex_unlock(&mut self, mutex: MutexHandle) -> Result<(), FreeRtosError> {
        let m = self
            .mutexes
            .get_mut(&mutex.0)
            .ok_or(FreeRtosError::InvalidMutexHandle { handle: mutex.0 })?;
        if !m.locked {
            return Err(FreeRtosError::MutexNotLocked);
        }
        m.locked = false;
        m.holder = None;
        Ok(())
    }

    /// Returns whether a mutex is currently locked.
    pub fn mutex_is_locked(&self, mutex: MutexHandle) -> Result<bool, FreeRtosError> {
        let m = self
            .mutexes
            .get(&mutex.0)
            .ok_or(FreeRtosError::InvalidMutexHandle { handle: mutex.0 })?;
        Ok(m.locked)
    }

    /// Returns the task holding the mutex (if any).
    pub fn mutex_holder(&self, mutex: MutexHandle) -> Result<Option<TaskHandle>, FreeRtosError> {
        let m = self
            .mutexes
            .get(&mutex.0)
            .ok_or(FreeRtosError::InvalidMutexHandle { handle: mutex.0 })?;
        Ok(m.holder)
    }

    // ─── Semaphore API ────────────────────────────────────────────────

    /// Creates a counting semaphore (xSemaphoreCreateCounting equivalent).
    ///
    /// # Arguments
    /// * `max_count` - Maximum count value
    /// * `initial_count` - Initial count value
    pub fn sem_create(
        &mut self,
        max_count: u32,
        initial_count: u32,
    ) -> Result<SemaphoreHandle, FreeRtosError> {
        let initial = if initial_count > max_count {
            max_count
        } else {
            initial_count
        };
        let handle = next_handle();
        let sem = SimSemaphore {
            count: initial,
            max_count,
        };
        self.semaphores.insert(handle, sem);
        Ok(SemaphoreHandle(handle))
    }

    /// Gives (increments) a semaphore (xSemaphoreGive equivalent).
    pub fn sem_give(&mut self, sem: SemaphoreHandle) -> Result<(), FreeRtosError> {
        let s = self
            .semaphores
            .get_mut(&sem.0)
            .ok_or(FreeRtosError::InvalidSemaphoreHandle { handle: sem.0 })?;
        if s.count >= s.max_count {
            return Err(FreeRtosError::SemaphoreAtMax {
                max_count: s.max_count,
            });
        }
        s.count += 1;
        Ok(())
    }

    /// Takes (decrements) a semaphore (xSemaphoreTake equivalent).
    ///
    /// Returns `Err(SemaphoreTimeout)` if count is 0 and timeout would expire.
    pub fn sem_take(
        &mut self,
        sem: SemaphoreHandle,
        timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let s = self
            .semaphores
            .get_mut(&sem.0)
            .ok_or(FreeRtosError::InvalidSemaphoreHandle { handle: sem.0 })?;
        if s.count == 0 {
            return Err(FreeRtosError::SemaphoreTimeout { timeout_ticks });
        }
        s.count -= 1;
        Ok(())
    }

    /// Returns the current count of a semaphore.
    pub fn sem_count(&self, sem: SemaphoreHandle) -> Result<u32, FreeRtosError> {
        let s = self
            .semaphores
            .get(&sem.0)
            .ok_or(FreeRtosError::InvalidSemaphoreHandle { handle: sem.0 })?;
        Ok(s.count)
    }

    /// ISR-safe semaphore give (xSemaphoreGiveFromISR equivalent).
    pub fn sem_give_from_isr(&mut self, sem: SemaphoreHandle) -> Result<bool, FreeRtosError> {
        let s = self
            .semaphores
            .get_mut(&sem.0)
            .ok_or(FreeRtosError::InvalidSemaphoreHandle { handle: sem.0 })?;
        if s.count >= s.max_count {
            return Ok(false);
        }
        s.count += 1;
        Ok(true) // higher_priority_task_woken
    }

    /// ISR-safe semaphore take (xSemaphoreTakeFromISR equivalent).
    pub fn sem_take_from_isr(&mut self, sem: SemaphoreHandle) -> Result<bool, FreeRtosError> {
        let s = self
            .semaphores
            .get_mut(&sem.0)
            .ok_or(FreeRtosError::InvalidSemaphoreHandle { handle: sem.0 })?;
        if s.count == 0 {
            return Ok(false);
        }
        s.count -= 1;
        Ok(true)
    }

    // ─── Timer API ────────────────────────────────────────────────────

    /// Creates a software timer (xTimerCreate equivalent).
    ///
    /// # Arguments
    /// * `name` - Timer name
    /// * `period_ticks` - Period in ticks
    /// * `auto_reload` - Whether the timer auto-reloads after expiry
    /// * `callback_fn` - Callback function ID
    pub fn timer_create(
        &mut self,
        name: &str,
        period_ticks: u32,
        auto_reload: bool,
        callback_fn: u64,
    ) -> Result<TimerHandle, FreeRtosError> {
        if period_ticks == 0 {
            return Err(FreeRtosError::TimerCreateFailed {
                reason: "period must be > 0".to_string(),
            });
        }
        let handle = next_handle();
        let timer = SimTimer {
            name: name.to_string(),
            period_ticks,
            auto_reload,
            callback_fn_id: callback_fn,
            running: false,
            ticks_remaining: period_ticks,
            expiry_count: 0,
        };
        self.timers.insert(handle, timer);
        Ok(TimerHandle(handle))
    }

    /// Starts a timer (xTimerStart equivalent).
    pub fn timer_start(
        &mut self,
        timer: TimerHandle,
        _timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let t = self
            .timers
            .get_mut(&timer.0)
            .ok_or(FreeRtosError::InvalidTimerHandle { handle: timer.0 })?;
        t.running = true;
        t.ticks_remaining = t.period_ticks;
        Ok(())
    }

    /// Stops a timer (xTimerStop equivalent).
    pub fn timer_stop(
        &mut self,
        timer: TimerHandle,
        _timeout_ticks: u32,
    ) -> Result<(), FreeRtosError> {
        let t = self
            .timers
            .get_mut(&timer.0)
            .ok_or(FreeRtosError::InvalidTimerHandle { handle: timer.0 })?;
        t.running = false;
        Ok(())
    }

    /// Returns whether a timer is running.
    pub fn timer_is_running(&self, timer: TimerHandle) -> Result<bool, FreeRtosError> {
        let t = self
            .timers
            .get(&timer.0)
            .ok_or(FreeRtosError::InvalidTimerHandle { handle: timer.0 })?;
        Ok(t.running)
    }

    /// Returns the name of a timer.
    pub fn timer_name(&self, timer: TimerHandle) -> Result<&str, FreeRtosError> {
        let t = self
            .timers
            .get(&timer.0)
            .ok_or(FreeRtosError::InvalidTimerHandle { handle: timer.0 })?;
        Ok(&t.name)
    }

    /// Returns the expiry count of a timer.
    pub fn timer_expiry_count(&self, timer: TimerHandle) -> Result<u64, FreeRtosError> {
        let t = self
            .timers
            .get(&timer.0)
            .ok_or(FreeRtosError::InvalidTimerHandle { handle: timer.0 })?;
        Ok(t.expiry_count)
    }

    // ─── Scheduler / tick simulation ─────────────────────────────────

    /// Starts the scheduler (vTaskStartScheduler equivalent).
    pub fn start_scheduler(&mut self) {
        self.scheduler_running = true;
    }

    /// Stops the scheduler (vTaskEndScheduler equivalent).
    pub fn stop_scheduler(&mut self) {
        self.scheduler_running = false;
    }

    /// Simulates a single scheduler tick.
    ///
    /// Decrements delay counters for blocked tasks, advances software
    /// timers, and returns a list of expired timer callback IDs.
    pub fn tick(&mut self) -> Vec<u64> {
        self.tick_count += 1;

        // Process task delays
        let handles: Vec<u64> = self.tasks.keys().copied().collect();
        for h in handles {
            if let Some(task) = self.tasks.get_mut(&h) {
                if task.state == TaskState::Blocked && task.delay_remaining > 0 {
                    task.delay_remaining -= 1;
                    if task.delay_remaining == 0 {
                        task.state = TaskState::Ready;
                    }
                }
            }
        }

        // Process software timers
        let mut expired_callbacks = Vec::new();
        let timer_handles: Vec<u64> = self.timers.keys().copied().collect();
        for h in timer_handles {
            if let Some(timer) = self.timers.get_mut(&h) {
                if timer.running {
                    if timer.ticks_remaining > 0 {
                        timer.ticks_remaining -= 1;
                    }
                    if timer.ticks_remaining == 0 {
                        timer.expiry_count += 1;
                        expired_callbacks.push(timer.callback_fn_id);
                        if timer.auto_reload {
                            timer.ticks_remaining = timer.period_ticks;
                        } else {
                            timer.running = false;
                        }
                    }
                }
            }
        }

        expired_callbacks
    }
}

impl Default for FreeRtosRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// extern "C" FFI declarations (feature-gated)
// ═══════════════════════════════════════════════════════════════════════

/// FFI declarations for actual FreeRTOS linkage.
///
/// These are only available when the `freertos` feature is enabled,
/// i.e., when cross-compiling for a real target with FreeRTOS linked.
#[cfg(feature = "freertos")]
extern "C" {
    /// xTaskCreate
    pub fn xTaskCreate(
        task_code: extern "C" fn(*mut core::ffi::c_void),
        name: *const core::ffi::c_char,
        stack_depth: u16,
        parameters: *mut core::ffi::c_void,
        priority: u32,
        task_handle: *mut *mut core::ffi::c_void,
    ) -> i32;

    /// vTaskDelete
    pub fn vTaskDelete(task_handle: *mut core::ffi::c_void);

    /// vTaskDelay
    pub fn vTaskDelay(ticks_to_delay: u32);

    /// vTaskDelayUntil
    pub fn vTaskDelayUntil(previous_wake_time: *mut u32, time_increment: u32);

    /// xQueueCreate
    pub fn xQueueCreate(length: u32, item_size: u32) -> *mut core::ffi::c_void;

    /// xQueueSend
    pub fn xQueueSend(
        queue: *mut core::ffi::c_void,
        item: *const core::ffi::c_void,
        ticks_to_wait: u32,
    ) -> i32;

    /// xQueueReceive
    pub fn xQueueReceive(
        queue: *mut core::ffi::c_void,
        buffer: *mut core::ffi::c_void,
        ticks_to_wait: u32,
    ) -> i32;

    /// xQueueSendFromISR
    pub fn xQueueSendFromISR(
        queue: *mut core::ffi::c_void,
        item: *const core::ffi::c_void,
        higher_priority_task_woken: *mut i32,
    ) -> i32;

    /// xSemaphoreCreateMutex
    pub fn xSemaphoreCreateMutex() -> *mut core::ffi::c_void;

    /// xSemaphoreTake
    pub fn xSemaphoreTake(semaphore: *mut core::ffi::c_void, ticks_to_wait: u32) -> i32;

    /// xSemaphoreGive
    pub fn xSemaphoreGive(semaphore: *mut core::ffi::c_void) -> i32;

    /// xSemaphoreCreateCounting
    pub fn xSemaphoreCreateCounting(max_count: u32, initial_count: u32) -> *mut core::ffi::c_void;

    /// xTimerCreate
    pub fn xTimerCreate(
        name: *const core::ffi::c_char,
        period_in_ticks: u32,
        auto_reload: i32,
        timer_id: *mut core::ffi::c_void,
        callback: extern "C" fn(*mut core::ffi::c_void),
    ) -> *mut core::ffi::c_void;

    /// xTimerStart
    pub fn xTimerStart(timer: *mut core::ffi::c_void, ticks_to_wait: u32) -> i32;

    /// xTimerStop
    pub fn xTimerStop(timer: *mut core::ffi::c_void, ticks_to_wait: u32) -> i32;

    /// vTaskStartScheduler
    pub fn vTaskStartScheduler();

    /// vTaskSuspend
    pub fn vTaskSuspend(task_handle: *mut core::ffi::c_void);

    /// vTaskResume
    pub fn vTaskResume(task_handle: *mut core::ffi::c_void);
}

// ═══════════════════════════════════════════════════════════════════════
// FreeRTOSConfig.h template generation
// ═══════════════════════════════════════════════════════════════════════

/// Board-specific FreeRTOS configuration parameters.
#[derive(Debug, Clone)]
pub struct FreeRtosConfig {
    /// CPU clock frequency in Hz.
    pub cpu_clock_hz: u32,
    /// Tick rate in Hz (typically 1000).
    pub tick_rate_hz: u32,
    /// Maximum priority levels.
    pub max_priorities: u32,
    /// Minimum stack size in words.
    pub minimal_stack_size: u32,
    /// Total heap size in bytes.
    pub total_heap_size: u32,
    /// Maximum task name length.
    pub max_task_name_len: u32,
    /// Enable preemption.
    pub use_preemption: bool,
    /// Enable time slicing.
    pub use_time_slicing: bool,
    /// Enable mutexes.
    pub use_mutexes: bool,
    /// Enable counting semaphores.
    pub use_counting_semaphores: bool,
    /// Enable software timers.
    pub use_timers: bool,
    /// Timer task priority.
    pub timer_task_priority: u32,
    /// Timer queue length.
    pub timer_queue_length: u32,
    /// Timer task stack depth.
    pub timer_task_stack_depth: u32,
    /// Enable tickless idle.
    pub use_tickless_idle: bool,
    /// Enable idle hook.
    pub use_idle_hook: bool,
    /// Enable tick hook.
    pub use_tick_hook: bool,
}

impl FreeRtosConfig {
    /// Creates a default configuration for a given CPU clock frequency.
    pub fn for_cpu(cpu_clock_hz: u32) -> Self {
        Self {
            cpu_clock_hz,
            tick_rate_hz: 1000,
            max_priorities: 56,
            minimal_stack_size: 128,
            total_heap_size: 32 * 1024,
            max_task_name_len: 16,
            use_preemption: true,
            use_time_slicing: true,
            use_mutexes: true,
            use_counting_semaphores: true,
            use_timers: true,
            timer_task_priority: 2,
            timer_queue_length: 10,
            timer_task_stack_depth: 256,
            use_tickless_idle: false,
            use_idle_hook: false,
            use_tick_hook: false,
        }
    }

    /// Generates the FreeRTOSConfig.h content as a string.
    pub fn generate_header(&self) -> String {
        let mut h = String::new();
        h.push_str("/* Auto-generated FreeRTOSConfig.h for Fajar Lang */\n");
        h.push_str("#ifndef FREERTOS_CONFIG_H\n");
        h.push_str("#define FREERTOS_CONFIG_H\n\n");

        h.push_str(&format!(
            "#define configCPU_CLOCK_HZ            ({}UL)\n",
            self.cpu_clock_hz
        ));
        h.push_str(&format!(
            "#define configTICK_RATE_HZ            ({})\n",
            self.tick_rate_hz
        ));
        h.push_str(&format!(
            "#define configMAX_PRIORITIES          ({})\n",
            self.max_priorities
        ));
        h.push_str(&format!(
            "#define configMINIMAL_STACK_SIZE      ({})\n",
            self.minimal_stack_size
        ));
        h.push_str(&format!(
            "#define configTOTAL_HEAP_SIZE         ({})\n",
            self.total_heap_size
        ));
        h.push_str(&format!(
            "#define configMAX_TASK_NAME_LEN       ({})\n",
            self.max_task_name_len
        ));
        h.push_str(&format!(
            "#define configUSE_PREEMPTION          {}\n",
            self.use_preemption as u8
        ));
        h.push_str(&format!(
            "#define configUSE_TIME_SLICING        {}\n",
            self.use_time_slicing as u8
        ));
        h.push_str(&format!(
            "#define configUSE_MUTEXES             {}\n",
            self.use_mutexes as u8
        ));
        h.push_str(&format!(
            "#define configUSE_COUNTING_SEMAPHORES {}\n",
            self.use_counting_semaphores as u8
        ));
        h.push_str(&format!(
            "#define configUSE_TIMERS              {}\n",
            self.use_timers as u8
        ));
        h.push_str(&format!(
            "#define configTIMER_TASK_PRIORITY      ({})\n",
            self.timer_task_priority
        ));
        h.push_str(&format!(
            "#define configTIMER_QUEUE_LENGTH       ({})\n",
            self.timer_queue_length
        ));
        h.push_str(&format!(
            "#define configTIMER_TASK_STACK_DEPTH   ({})\n",
            self.timer_task_stack_depth
        ));
        h.push_str(&format!(
            "#define configUSE_TICKLESS_IDLE       {}\n",
            self.use_tickless_idle as u8
        ));
        h.push_str(&format!(
            "#define configUSE_IDLE_HOOK           {}\n",
            self.use_idle_hook as u8
        ));
        h.push_str(&format!(
            "#define configUSE_TICK_HOOK           {}\n",
            self.use_tick_hook as u8
        ));

        h.push_str("\n/* Cortex-M specific */\n");
        h.push_str("#ifdef __NVIC_PRIO_BITS\n");
        h.push_str("  #define configPRIO_BITS __NVIC_PRIO_BITS\n");
        h.push_str("#else\n");
        h.push_str("  #define configPRIO_BITS 4\n");
        h.push_str("#endif\n\n");

        h.push_str(
            "#define configLIBRARY_LOWEST_INTERRUPT_PRIORITY      ((1 << configPRIO_BITS) - 1)\n",
        );
        h.push_str("#define configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY  5\n");
        h.push_str("#define configKERNEL_INTERRUPT_PRIORITY               (configLIBRARY_LOWEST_INTERRUPT_PRIORITY << (8 - configPRIO_BITS))\n");
        h.push_str("#define configMAX_SYSCALL_INTERRUPT_PRIORITY          (configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY << (8 - configPRIO_BITS))\n\n");

        h.push_str("/* Handler names for Cortex-M ports */\n");
        h.push_str("#define vPortSVCHandler    SVC_Handler\n");
        h.push_str("#define xPortPendSVHandler PendSV_Handler\n");
        h.push_str("#define xPortSysTickHandler SysTick_Handler\n\n");

        h.push_str("#endif /* FREERTOS_CONFIG_H */\n");
        h
    }
}

impl Default for FreeRtosConfig {
    fn default() -> Self {
        Self::for_cpu(168_000_000) // Default: STM32F4 @ 168MHz
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_runtime() -> FreeRtosRuntime {
        FreeRtosRuntime::new()
    }

    // ─── Task tests ───────────────────────────────────────────────────

    #[test]
    fn task_create_returns_valid_handle() {
        let mut rt = make_runtime();
        let handle = rt.task_create("test_task", 2, 256, 0x1000).unwrap();
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Ready);
        assert_eq!(rt.task_name(handle).unwrap(), "test_task");
    }

    #[test]
    fn task_create_rejects_invalid_priority() {
        let mut rt = make_runtime();
        let err = rt
            .task_create("bad", CONFIG_MAX_PRIORITIES, 256, 0)
            .unwrap_err();
        assert!(matches!(err, FreeRtosError::InvalidPriority { .. }));
    }

    #[test]
    fn task_create_rejects_small_stack() {
        let mut rt = make_runtime();
        let err = rt.task_create("bad", 1, 10, 0).unwrap_err();
        assert!(matches!(err, FreeRtosError::InvalidStackSize { .. }));
    }

    #[test]
    fn task_delete_marks_as_deleted() {
        let mut rt = make_runtime();
        let handle = rt.task_create("t1", 1, 256, 0).unwrap();
        rt.task_delete(handle).unwrap();
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Deleted);
    }

    #[test]
    fn task_delay_blocks_and_unblocks_on_tick() {
        let mut rt = make_runtime();
        let handle = rt.task_create("t1", 1, 256, 0).unwrap();
        rt.task_delay(handle, 3).unwrap();
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Blocked);

        rt.tick(); // tick 1 -> 2 remaining
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Blocked);
        rt.tick(); // tick 2 -> 1 remaining
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Blocked);
        rt.tick(); // tick 3 -> 0 remaining, becomes Ready
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Ready);
    }

    #[test]
    fn task_suspend_and_resume() {
        let mut rt = make_runtime();
        let handle = rt.task_create("t1", 1, 256, 0).unwrap();
        rt.task_suspend(handle).unwrap();
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Suspended);
        rt.task_resume(handle).unwrap();
        assert_eq!(rt.task_state(handle).unwrap(), TaskState::Ready);
    }

    #[test]
    fn task_count_excludes_deleted() {
        let mut rt = make_runtime();
        let h1 = rt.task_create("t1", 1, 256, 0).unwrap();
        let _h2 = rt.task_create("t2", 1, 256, 0).unwrap();
        assert_eq!(rt.task_count(), 2);
        rt.task_delete(h1).unwrap();
        assert_eq!(rt.task_count(), 1);
    }

    // ─── Queue tests ─────────────────────────────────────────────────

    #[test]
    fn queue_send_and_receive_fifo() {
        let mut rt = make_runtime();
        let q = rt.queue_create(10, 4).unwrap();
        rt.queue_send(q, 42, 0).unwrap();
        rt.queue_send(q, 99, 0).unwrap();
        assert_eq!(rt.queue_receive(q, 0).unwrap(), 42);
        assert_eq!(rt.queue_receive(q, 0).unwrap(), 99);
    }

    #[test]
    fn queue_full_returns_error() {
        let mut rt = make_runtime();
        let q = rt.queue_create(2, 4).unwrap();
        rt.queue_send(q, 1, 0).unwrap();
        rt.queue_send(q, 2, 0).unwrap();
        let err = rt.queue_send(q, 3, 0).unwrap_err();
        assert!(matches!(err, FreeRtosError::QueueFull { .. }));
    }

    #[test]
    fn queue_empty_returns_error() {
        let mut rt = make_runtime();
        let q = rt.queue_create(10, 4).unwrap();
        let err = rt.queue_receive(q, 0).unwrap_err();
        assert!(matches!(err, FreeRtosError::QueueEmpty));
    }

    #[test]
    fn queue_isr_send_receive() {
        let mut rt = make_runtime();
        let q = rt.queue_create(10, 4).unwrap();
        let woken = rt.queue_send_from_isr(q, 77).unwrap();
        assert!(woken);
        let val = rt.queue_receive_from_isr(q).unwrap();
        assert_eq!(val, Some(77));
        let empty = rt.queue_receive_from_isr(q).unwrap();
        assert_eq!(empty, None);
    }

    // ─── Mutex tests ─────────────────────────────────────────────────

    #[test]
    fn mutex_lock_unlock_cycle() {
        let mut rt = make_runtime();
        let m = rt.mutex_create();
        assert!(!rt.mutex_is_locked(m).unwrap());
        rt.mutex_lock(m, 0).unwrap();
        assert!(rt.mutex_is_locked(m).unwrap());
        rt.mutex_unlock(m).unwrap();
        assert!(!rt.mutex_is_locked(m).unwrap());
    }

    #[test]
    fn mutex_double_lock_returns_timeout() {
        let mut rt = make_runtime();
        let m = rt.mutex_create();
        rt.mutex_lock(m, 0).unwrap();
        let err = rt.mutex_lock(m, 100).unwrap_err();
        assert!(matches!(err, FreeRtosError::MutexTimeout { .. }));
    }

    #[test]
    fn mutex_unlock_when_not_locked_returns_error() {
        let mut rt = make_runtime();
        let m = rt.mutex_create();
        let err = rt.mutex_unlock(m).unwrap_err();
        assert!(matches!(err, FreeRtosError::MutexNotLocked));
    }

    // ─── Semaphore tests ─────────────────────────────────────────────

    #[test]
    fn semaphore_counting_give_take() {
        let mut rt = make_runtime();
        let sem = rt.sem_create(5, 0).unwrap();
        assert_eq!(rt.sem_count(sem).unwrap(), 0);

        rt.sem_give(sem).unwrap();
        rt.sem_give(sem).unwrap();
        assert_eq!(rt.sem_count(sem).unwrap(), 2);

        rt.sem_take(sem, 0).unwrap();
        assert_eq!(rt.sem_count(sem).unwrap(), 1);
    }

    #[test]
    fn semaphore_at_max_returns_error() {
        let mut rt = make_runtime();
        let sem = rt.sem_create(2, 2).unwrap();
        let err = rt.sem_give(sem).unwrap_err();
        assert!(matches!(err, FreeRtosError::SemaphoreAtMax { .. }));
    }

    #[test]
    fn semaphore_take_empty_returns_timeout() {
        let mut rt = make_runtime();
        let sem = rt.sem_create(5, 0).unwrap();
        let err = rt.sem_take(sem, 100).unwrap_err();
        assert!(matches!(err, FreeRtosError::SemaphoreTimeout { .. }));
    }

    #[test]
    fn semaphore_isr_give_take() {
        let mut rt = make_runtime();
        let sem = rt.sem_create(3, 0).unwrap();
        let woken = rt.sem_give_from_isr(sem).unwrap();
        assert!(woken);
        assert_eq!(rt.sem_count(sem).unwrap(), 1);

        let took = rt.sem_take_from_isr(sem).unwrap();
        assert!(took);
        assert_eq!(rt.sem_count(sem).unwrap(), 0);

        let empty = rt.sem_take_from_isr(sem).unwrap();
        assert!(!empty);
    }

    // ─── Timer tests ─────────────────────────────────────────────────

    #[test]
    fn timer_create_start_stop() {
        let mut rt = make_runtime();
        let timer = rt.timer_create("heartbeat", 100, true, 0x42).unwrap();
        assert!(!rt.timer_is_running(timer).unwrap());
        rt.timer_start(timer, 0).unwrap();
        assert!(rt.timer_is_running(timer).unwrap());
        rt.timer_stop(timer, 0).unwrap();
        assert!(!rt.timer_is_running(timer).unwrap());
    }

    #[test]
    fn timer_expires_after_period() {
        let mut rt = make_runtime();
        let timer = rt.timer_create("t1", 3, false, 0xAA).unwrap();
        rt.timer_start(timer, 0).unwrap();

        // Tick 1, 2: no expiry
        assert!(rt.tick().is_empty());
        assert!(rt.tick().is_empty());
        // Tick 3: timer expires
        let expired = rt.tick();
        assert_eq!(expired, vec![0xAA]);
        // One-shot: not running anymore
        assert!(!rt.timer_is_running(timer).unwrap());
    }

    #[test]
    fn timer_auto_reload_repeats() {
        let mut rt = make_runtime();
        let timer = rt.timer_create("periodic", 2, true, 0xBB).unwrap();
        rt.timer_start(timer, 0).unwrap();

        // First period
        assert!(rt.tick().is_empty());
        let expired1 = rt.tick();
        assert_eq!(expired1, vec![0xBB]);
        assert!(rt.timer_is_running(timer).unwrap());

        // Second period
        assert!(rt.tick().is_empty());
        let expired2 = rt.tick();
        assert_eq!(expired2, vec![0xBB]);
        assert_eq!(rt.timer_expiry_count(timer).unwrap(), 2);
    }

    #[test]
    fn timer_zero_period_rejected() {
        let mut rt = make_runtime();
        let err = rt.timer_create("bad", 0, false, 0).unwrap_err();
        assert!(matches!(err, FreeRtosError::TimerCreateFailed { .. }));
    }

    // ─── Scheduler / tick tests ──────────────────────────────────────

    #[test]
    fn scheduler_start_stop() {
        let mut rt = make_runtime();
        assert!(!rt.is_scheduler_running());
        rt.start_scheduler();
        assert!(rt.is_scheduler_running());
        rt.stop_scheduler();
        assert!(!rt.is_scheduler_running());
    }

    #[test]
    fn tick_increments_count() {
        let mut rt = make_runtime();
        assert_eq!(rt.tick_count(), 0);
        rt.tick();
        assert_eq!(rt.tick_count(), 1);
        rt.tick();
        rt.tick();
        assert_eq!(rt.tick_count(), 3);
    }

    // ─── Config tests ────────────────────────────────────────────────

    #[test]
    fn freertos_config_generates_valid_header() {
        let config = FreeRtosConfig::for_cpu(168_000_000);
        let header = config.generate_header();
        assert!(header.contains("#ifndef FREERTOS_CONFIG_H"));
        assert!(header.contains("#define configCPU_CLOCK_HZ"));
        assert!(header.contains("168000000"));
        assert!(header.contains("#define configTICK_RATE_HZ"));
        assert!(header.contains("#define configMAX_PRIORITIES"));
        assert!(header.contains("#define configUSE_PREEMPTION"));
        assert!(header.contains("#endif"));
    }

    #[test]
    fn freertos_config_default() {
        let config = FreeRtosConfig::default();
        assert_eq!(config.cpu_clock_hz, 168_000_000);
        assert_eq!(config.tick_rate_hz, 1000);
        assert!(config.use_preemption);
        assert!(config.use_mutexes);
    }

    #[test]
    fn mutex_lock_by_tracks_holder() {
        let mut rt = make_runtime();
        let m = rt.mutex_create();
        let task = rt.task_create("holder", 2, 256, 0).unwrap();
        rt.mutex_lock_by(m, task, 0).unwrap();
        assert_eq!(rt.mutex_holder(m).unwrap(), Some(task));
        rt.mutex_unlock(m).unwrap();
        assert_eq!(rt.mutex_holder(m).unwrap(), None);
    }

    #[test]
    fn queue_messages_waiting_count() {
        let mut rt = make_runtime();
        let q = rt.queue_create(10, 4).unwrap();
        assert_eq!(rt.queue_messages_waiting(q).unwrap(), 0);
        rt.queue_send(q, 1, 0).unwrap();
        rt.queue_send(q, 2, 0).unwrap();
        assert_eq!(rt.queue_messages_waiting(q).unwrap(), 2);
        rt.queue_receive(q, 0).unwrap();
        assert_eq!(rt.queue_messages_waiting(q).unwrap(), 1);
    }

    #[test]
    fn invalid_handles_return_errors() {
        let mut rt = make_runtime();
        let bad_task = TaskHandle(999999);
        let bad_queue = QueueHandle(999999);
        let bad_mutex = MutexHandle(999999);
        let bad_sem = SemaphoreHandle(999999);
        let bad_timer = TimerHandle(999999);

        assert!(rt.task_state(bad_task).is_err());
        assert!(rt.queue_send(bad_queue, 0, 0).is_err());
        assert!(rt.mutex_lock(bad_mutex, 0).is_err());
        assert!(rt.sem_give(bad_sem).is_err());
        assert!(rt.timer_start(bad_timer, 0).is_err());
    }
}
