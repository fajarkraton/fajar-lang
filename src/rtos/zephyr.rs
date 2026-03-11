//! Zephyr RTOS FFI bindings for Fajar Lang.
//!
//! Provides Rust-side type definitions modeling the Zephyr RTOS API, with
//! extern "C" FFI declarations behind `#[cfg(feature = "zephyr")]` and
//! simulation stubs for testing when the feature is not enabled.
//!
//! # API Groups
//!
//! - **Thread API**: create, abort, suspend, resume, sleep, yield
//! - **Message Queue API**: create, put, get, peek, purge
//! - **Mutex API**: create, lock, unlock with priority inheritance
//! - **Semaphore API**: binary and counting semaphores
//! - **Timer API**: periodic and one-shot timers
//! - **Work Queue API**: submit and schedule deferred work items
//!
//! # Target
//!
//! The STM32H5F5 (Arduino VENTUNO Q) runs Zephyr OS. These bindings
//! model Zephyr's kernel primitives for use from Fajar Lang.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from Zephyr RTOS operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ZephyrError {
    /// Thread creation failed.
    #[error("thread creation failed: {reason}")]
    ThreadCreateFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Thread abort failed.
    #[error("thread abort failed: {reason}")]
    ThreadAbortFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Message queue is full (put timed out).
    #[error("message queue full (capacity: {capacity})")]
    MsgqFull {
        /// Queue capacity.
        capacity: u32,
    },

    /// Message queue is empty (get timed out).
    #[error("message queue empty")]
    MsgqEmpty,

    /// Message queue operation timed out.
    #[error("message queue operation timed out after {timeout_ms} ms")]
    MsgqTimeout {
        /// Timeout in milliseconds.
        timeout_ms: u32,
    },

    /// Mutex lock timed out.
    #[error("mutex lock timed out after {timeout_ms} ms")]
    MutexTimeout {
        /// Timeout in milliseconds.
        timeout_ms: u32,
    },

    /// Mutex unlock by non-owner.
    #[error("mutex not owned by current thread")]
    MutexNotOwner,

    /// Semaphore take timed out.
    #[error("semaphore take timed out after {timeout_ms} ms")]
    SemTimeout {
        /// Timeout in milliseconds.
        timeout_ms: u32,
    },

    /// Timer creation failed.
    #[error("timer creation failed: {reason}")]
    TimerCreateFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Work submission failed.
    #[error("work submit failed: {reason}")]
    WorkSubmitFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid parameter.
    #[error("invalid parameter: {reason}")]
    InvalidParam {
        /// Reason for failure.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════

/// Minimum stack size in bytes for Zephyr threads.
pub const ZEPHYR_MIN_STACK_SIZE: u32 = 256;

/// Maximum thread priority (Zephyr uses negative = cooperative, positive = preemptive).
/// We model 0..31 for simplicity in simulation.
pub const ZEPHYR_MAX_PRIORITY: i32 = 31;

/// Wait forever timeout sentinel.
pub const K_FOREVER: u32 = u32::MAX;

/// No wait (non-blocking).
pub const K_NO_WAIT: u32 = 0;

// ═══════════════════════════════════════════════════════════════════════
// Handle types
// ═══════════════════════════════════════════════════════════════════════

/// Opaque Zephyr thread handle (wraps k_tid_t).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZephyrThreadHandle(pub u64);

/// Opaque Zephyr message queue handle (wraps struct k_msgq*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZephyrMsgqHandle(pub u64);

/// Opaque Zephyr mutex handle (wraps struct k_mutex*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZephyrMutexHandle(pub u64);

/// Opaque Zephyr semaphore handle (wraps struct k_sem*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZephyrSemHandle(pub u64);

/// Opaque Zephyr timer handle (wraps struct k_timer*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZephyrTimerHandle(pub u64);

// ═══════════════════════════════════════════════════════════════════════
// Thread state
// ═══════════════════════════════════════════════════════════════════════

/// Zephyr thread state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZephyrThreadState {
    /// Thread is ready to be scheduled.
    Ready,
    /// Thread is currently executing.
    Running,
    /// Thread is suspended (k_thread_suspend).
    Suspended,
    /// Thread has been terminated (k_thread_abort or returned).
    Terminated,
}

// ═══════════════════════════════════════════════════════════════════════
// Simulated types
// ═══════════════════════════════════════════════════════════════════════

/// Simulated Zephyr thread descriptor.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SimZephyrThread {
    /// Thread name.
    name: String,
    /// Thread priority (lower = higher priority in Zephyr).
    priority: i32,
    /// Stack size in bytes.
    stack_size: u32,
    /// Entry function ID.
    entry_fn: u64,
    /// Current thread state.
    state: ZephyrThreadState,
}

/// Simulated Zephyr message queue.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SimZephyrMsgq {
    /// Message buffer (FIFO).
    messages: VecDeque<i64>,
    /// Maximum message size in bytes.
    msg_size: u32,
    /// Maximum number of messages.
    max_msgs: u32,
}

/// Simulated Zephyr mutex.
#[derive(Debug, Clone)]
struct SimZephyrMutex {
    /// Whether the mutex is locked.
    locked: bool,
    /// Thread ID that owns the lock.
    owner: Option<u64>,
}

/// Simulated Zephyr semaphore.
#[derive(Debug, Clone)]
struct SimZephyrSem {
    /// Current count.
    count: u32,
    /// Maximum count (limit).
    limit: u32,
}

/// Simulated Zephyr timer.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SimZephyrTimer {
    /// Timer name.
    name: String,
    /// Period in milliseconds (0 = one-shot).
    period_ms: u32,
    /// Whether the timer is currently running.
    running: bool,
    /// Number of times the timer has expired.
    expiry_count: u64,
    /// Whether the timer auto-reloads (periodic).
    auto_reload: bool,
    /// Ticks remaining until next expiry.
    ticks_remaining: u32,
}

/// Simulated Zephyr work item.
#[derive(Debug, Clone)]
struct SimZephyrWork {
    /// Work item ID.
    work_id: u64,
    /// Delay before execution (0 = immediate).
    delay_remaining: u32,
}

// ═══════════════════════════════════════════════════════════════════════
// Zephyr simulation runtime
// ═══════════════════════════════════════════════════════════════════════

static NEXT_ZEPHYR_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Generates a unique Zephyr handle value.
fn next_handle() -> u64 {
    NEXT_ZEPHYR_HANDLE.fetch_add(1, Ordering::Relaxed)
}

/// Simulated Zephyr RTOS runtime for testing without real hardware.
///
/// Provides the full Zephyr kernel API surface using in-memory simulation,
/// allowing unit tests and development without an STM32H5F5 target board.
#[derive(Debug)]
pub struct ZephyrSim {
    /// Registered threads.
    threads: HashMap<u64, SimZephyrThread>,
    /// Registered message queues.
    msgqs: HashMap<u64, SimZephyrMsgq>,
    /// Registered mutexes.
    mutexes: HashMap<u64, SimZephyrMutex>,
    /// Registered semaphores.
    semaphores: HashMap<u64, SimZephyrSem>,
    /// Registered timers.
    timers: HashMap<u64, SimZephyrTimer>,
    /// Pending work items.
    work_items: Vec<SimZephyrWork>,
    /// System tick count (milliseconds).
    tick_count: u64,
}

impl ZephyrSim {
    /// Creates a new Zephyr simulation runtime.
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
            msgqs: HashMap::new(),
            mutexes: HashMap::new(),
            semaphores: HashMap::new(),
            timers: HashMap::new(),
            work_items: Vec::new(),
            tick_count: 0,
        }
    }

    /// Returns the current system tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Returns the number of active threads.
    pub fn thread_count(&self) -> usize {
        self.threads
            .values()
            .filter(|t| t.state != ZephyrThreadState::Terminated)
            .count()
    }

    // ─── Thread API ──────────────────────────────────────────────────

    /// Creates a new Zephyr thread (k_thread_create equivalent).
    ///
    /// # Arguments
    /// * `name` - Thread name
    /// * `priority` - Thread priority (0..ZEPHYR_MAX_PRIORITY)
    /// * `stack_size` - Stack size in bytes (min ZEPHYR_MIN_STACK_SIZE)
    /// * `entry_fn` - Entry function ID
    pub fn thread_create(
        &mut self,
        name: &str,
        priority: i32,
        stack_size: u32,
        entry_fn: u64,
    ) -> Result<ZephyrThreadHandle, ZephyrError> {
        if !(0..=ZEPHYR_MAX_PRIORITY).contains(&priority) {
            return Err(ZephyrError::ThreadCreateFailed {
                reason: format!("priority {priority} out of range (0..{ZEPHYR_MAX_PRIORITY})"),
            });
        }
        if stack_size < ZEPHYR_MIN_STACK_SIZE {
            return Err(ZephyrError::ThreadCreateFailed {
                reason: format!("stack size {stack_size} too small (min: {ZEPHYR_MIN_STACK_SIZE})"),
            });
        }

        let handle = next_handle();
        let thread = SimZephyrThread {
            name: name.to_string(),
            priority,
            stack_size,
            entry_fn,
            state: ZephyrThreadState::Ready,
        };
        self.threads.insert(handle, thread);
        Ok(ZephyrThreadHandle(handle))
    }

    /// Aborts a thread (k_thread_abort equivalent).
    pub fn thread_abort(&mut self, handle: ZephyrThreadHandle) -> Result<(), ZephyrError> {
        let thread =
            self.threads
                .get_mut(&handle.0)
                .ok_or_else(|| ZephyrError::ThreadAbortFailed {
                    reason: format!("invalid thread handle: {}", handle.0),
                })?;
        thread.state = ZephyrThreadState::Terminated;
        Ok(())
    }

    /// Suspends a thread (k_thread_suspend equivalent).
    pub fn thread_suspend(&mut self, handle: ZephyrThreadHandle) -> Result<(), ZephyrError> {
        let thread = self
            .threads
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid thread handle: {}", handle.0),
            })?;
        if thread.state != ZephyrThreadState::Terminated {
            thread.state = ZephyrThreadState::Suspended;
        }
        Ok(())
    }

    /// Resumes a suspended thread (k_thread_resume equivalent).
    pub fn thread_resume(&mut self, handle: ZephyrThreadHandle) -> Result<(), ZephyrError> {
        let thread = self
            .threads
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid thread handle: {}", handle.0),
            })?;
        if thread.state == ZephyrThreadState::Suspended {
            thread.state = ZephyrThreadState::Ready;
        }
        Ok(())
    }

    /// Returns the state of a thread.
    pub fn thread_state(&self, handle: ZephyrThreadHandle) -> Option<ZephyrThreadState> {
        self.threads.get(&handle.0).map(|t| t.state)
    }

    /// Returns the name of a thread.
    pub fn thread_name(&self, handle: ZephyrThreadHandle) -> Option<&str> {
        self.threads.get(&handle.0).map(|t| t.name.as_str())
    }

    /// Returns the priority of a thread.
    pub fn thread_priority(&self, handle: ZephyrThreadHandle) -> Option<i32> {
        self.threads.get(&handle.0).map(|t| t.priority)
    }

    /// Sleeps the current context for the given milliseconds (k_sleep equivalent).
    ///
    /// In simulation, this advances the tick count.
    pub fn sleep_ms(&mut self, ms: u32) {
        self.tick_count += ms as u64;
    }

    /// Yields the CPU to the next ready thread (k_yield equivalent).
    ///
    /// In simulation, this is a no-op.
    pub fn yield_cpu(&self) {
        // No-op in simulation
    }

    // ─── Message Queue API ───────────────────────────────────────────

    /// Creates a message queue (k_msgq_init equivalent).
    ///
    /// # Arguments
    /// * `msg_size` - Size of each message in bytes
    /// * `max_msgs` - Maximum number of messages
    pub fn msgq_create(
        &mut self,
        msg_size: u32,
        max_msgs: u32,
    ) -> Result<ZephyrMsgqHandle, ZephyrError> {
        if msg_size == 0 || max_msgs == 0 {
            return Err(ZephyrError::InvalidParam {
                reason: "msg_size and max_msgs must be > 0".to_string(),
            });
        }
        let handle = next_handle();
        let msgq = SimZephyrMsgq {
            messages: VecDeque::new(),
            msg_size,
            max_msgs,
        };
        self.msgqs.insert(handle, msgq);
        Ok(ZephyrMsgqHandle(handle))
    }

    /// Puts a message into the queue (k_msgq_put equivalent).
    ///
    /// # Arguments
    /// * `handle` - Message queue handle
    /// * `data` - Message data (i64 value)
    /// * `timeout_ms` - Timeout in milliseconds (K_NO_WAIT = non-blocking)
    pub fn msgq_put(
        &mut self,
        handle: ZephyrMsgqHandle,
        data: i64,
        timeout_ms: u32,
    ) -> Result<(), ZephyrError> {
        let msgq = self
            .msgqs
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid msgq handle: {}", handle.0),
            })?;

        if msgq.messages.len() >= msgq.max_msgs as usize {
            if timeout_ms == K_NO_WAIT {
                return Err(ZephyrError::MsgqFull {
                    capacity: msgq.max_msgs,
                });
            }
            return Err(ZephyrError::MsgqTimeout { timeout_ms });
        }

        msgq.messages.push_back(data);
        Ok(())
    }

    /// Gets a message from the queue (k_msgq_get equivalent).
    ///
    /// # Arguments
    /// * `handle` - Message queue handle
    /// * `timeout_ms` - Timeout in milliseconds (K_NO_WAIT = non-blocking)
    pub fn msgq_get(
        &mut self,
        handle: ZephyrMsgqHandle,
        timeout_ms: u32,
    ) -> Result<i64, ZephyrError> {
        let msgq = self
            .msgqs
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid msgq handle: {}", handle.0),
            })?;

        match msgq.messages.pop_front() {
            Some(data) => Ok(data),
            None => {
                if timeout_ms == K_NO_WAIT {
                    Err(ZephyrError::MsgqEmpty)
                } else {
                    Err(ZephyrError::MsgqTimeout { timeout_ms })
                }
            }
        }
    }

    /// Peeks at the front message without removing it.
    pub fn msgq_peek(&self, handle: ZephyrMsgqHandle) -> Option<i64> {
        self.msgqs
            .get(&handle.0)
            .and_then(|q| q.messages.front().copied())
    }

    /// Purges all messages from the queue (k_msgq_purge equivalent).
    pub fn msgq_purge(&mut self, handle: ZephyrMsgqHandle) -> Result<(), ZephyrError> {
        let msgq = self
            .msgqs
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid msgq handle: {}", handle.0),
            })?;
        msgq.messages.clear();
        Ok(())
    }

    /// Returns the number of messages currently in the queue.
    pub fn msgq_count(&self, handle: ZephyrMsgqHandle) -> usize {
        self.msgqs
            .get(&handle.0)
            .map(|q| q.messages.len())
            .unwrap_or(0)
    }

    // ─── Mutex API ───────────────────────────────────────────────────

    /// Creates a mutex (k_mutex_init equivalent).
    pub fn mutex_create(&mut self) -> Result<ZephyrMutexHandle, ZephyrError> {
        let handle = next_handle();
        let mutex = SimZephyrMutex {
            locked: false,
            owner: None,
        };
        self.mutexes.insert(handle, mutex);
        Ok(ZephyrMutexHandle(handle))
    }

    /// Locks a mutex (k_mutex_lock equivalent).
    ///
    /// # Arguments
    /// * `handle` - Mutex handle
    /// * `timeout_ms` - Timeout in milliseconds
    pub fn mutex_lock(
        &mut self,
        handle: ZephyrMutexHandle,
        timeout_ms: u32,
    ) -> Result<(), ZephyrError> {
        let mutex = self
            .mutexes
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid mutex handle: {}", handle.0),
            })?;

        if mutex.locked {
            return Err(ZephyrError::MutexTimeout { timeout_ms });
        }

        mutex.locked = true;
        // Use handle value as simulated thread owner
        mutex.owner = Some(handle.0);
        Ok(())
    }

    /// Unlocks a mutex (k_mutex_unlock equivalent).
    pub fn mutex_unlock(&mut self, handle: ZephyrMutexHandle) -> Result<(), ZephyrError> {
        let mutex = self
            .mutexes
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid mutex handle: {}", handle.0),
            })?;

        if !mutex.locked {
            return Err(ZephyrError::MutexNotOwner);
        }

        mutex.locked = false;
        mutex.owner = None;
        Ok(())
    }

    /// Returns whether the mutex is locked.
    pub fn mutex_is_locked(&self, handle: ZephyrMutexHandle) -> bool {
        self.mutexes
            .get(&handle.0)
            .map(|m| m.locked)
            .unwrap_or(false)
    }

    // ─── Semaphore API ───────────────────────────────────────────────

    /// Creates a semaphore (k_sem_init equivalent).
    ///
    /// # Arguments
    /// * `initial` - Initial count
    /// * `limit` - Maximum count
    pub fn sem_create(&mut self, initial: u32, limit: u32) -> Result<ZephyrSemHandle, ZephyrError> {
        if limit == 0 {
            return Err(ZephyrError::InvalidParam {
                reason: "semaphore limit must be > 0".to_string(),
            });
        }
        let initial = if initial > limit { limit } else { initial };
        let handle = next_handle();
        let sem = SimZephyrSem {
            count: initial,
            limit,
        };
        self.semaphores.insert(handle, sem);
        Ok(ZephyrSemHandle(handle))
    }

    /// Gives (signals) the semaphore (k_sem_give equivalent).
    pub fn sem_give(&mut self, handle: ZephyrSemHandle) -> Result<(), ZephyrError> {
        let sem = self
            .semaphores
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid semaphore handle: {}", handle.0),
            })?;

        if sem.count < sem.limit {
            sem.count += 1;
        }
        // In Zephyr, k_sem_give silently caps at limit (no error).
        Ok(())
    }

    /// Takes (waits on) the semaphore (k_sem_take equivalent).
    ///
    /// # Arguments
    /// * `handle` - Semaphore handle
    /// * `timeout_ms` - Timeout in milliseconds
    pub fn sem_take(
        &mut self,
        handle: ZephyrSemHandle,
        timeout_ms: u32,
    ) -> Result<(), ZephyrError> {
        let sem = self
            .semaphores
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid semaphore handle: {}", handle.0),
            })?;

        if sem.count > 0 {
            sem.count -= 1;
            Ok(())
        } else {
            Err(ZephyrError::SemTimeout { timeout_ms })
        }
    }

    /// Returns the current semaphore count (k_sem_count_get equivalent).
    pub fn sem_count(&self, handle: ZephyrSemHandle) -> u32 {
        self.semaphores.get(&handle.0).map(|s| s.count).unwrap_or(0)
    }

    // ─── Timer API ───────────────────────────────────────────────────

    /// Creates a timer (k_timer_init + config equivalent).
    ///
    /// # Arguments
    /// * `name` - Timer name
    /// * `period_ms` - Period in milliseconds
    /// * `auto_reload` - Whether to auto-reload (periodic) or one-shot
    pub fn timer_create(
        &mut self,
        name: &str,
        period_ms: u32,
        auto_reload: bool,
    ) -> Result<ZephyrTimerHandle, ZephyrError> {
        if period_ms == 0 {
            return Err(ZephyrError::TimerCreateFailed {
                reason: "period must be > 0".to_string(),
            });
        }
        let handle = next_handle();
        let timer = SimZephyrTimer {
            name: name.to_string(),
            period_ms,
            running: false,
            expiry_count: 0,
            auto_reload,
            ticks_remaining: period_ms,
        };
        self.timers.insert(handle, timer);
        Ok(ZephyrTimerHandle(handle))
    }

    /// Starts a timer (k_timer_start equivalent).
    pub fn timer_start(&mut self, handle: ZephyrTimerHandle) -> Result<(), ZephyrError> {
        let timer = self
            .timers
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid timer handle: {}", handle.0),
            })?;
        timer.running = true;
        timer.ticks_remaining = timer.period_ms;
        Ok(())
    }

    /// Stops a timer (k_timer_stop equivalent).
    pub fn timer_stop(&mut self, handle: ZephyrTimerHandle) -> Result<(), ZephyrError> {
        let timer = self
            .timers
            .get_mut(&handle.0)
            .ok_or_else(|| ZephyrError::InvalidParam {
                reason: format!("invalid timer handle: {}", handle.0),
            })?;
        timer.running = false;
        Ok(())
    }

    /// Returns whether the timer is currently running.
    pub fn timer_is_running(&self, handle: ZephyrTimerHandle) -> bool {
        self.timers
            .get(&handle.0)
            .map(|t| t.running)
            .unwrap_or(false)
    }

    /// Returns the remaining ticks until next expiry.
    pub fn timer_remaining(&self, handle: ZephyrTimerHandle) -> u32 {
        self.timers
            .get(&handle.0)
            .map(|t| if t.running { t.ticks_remaining } else { 0 })
            .unwrap_or(0)
    }

    /// Returns the number of times the timer has expired.
    pub fn timer_expiry_count(&self, handle: ZephyrTimerHandle) -> u64 {
        self.timers
            .get(&handle.0)
            .map(|t| t.expiry_count)
            .unwrap_or(0)
    }

    // ─── Work Queue API ──────────────────────────────────────────────

    /// Submits a work item for immediate execution (k_work_submit equivalent).
    pub fn work_submit(&mut self, work_id: u64) -> Result<(), ZephyrError> {
        self.work_items.push(SimZephyrWork {
            work_id,
            delay_remaining: 0,
        });
        Ok(())
    }

    /// Schedules a work item with a delay (k_work_schedule equivalent).
    pub fn work_schedule(&mut self, work_id: u64, delay_ms: u32) -> Result<(), ZephyrError> {
        self.work_items.push(SimZephyrWork {
            work_id,
            delay_remaining: delay_ms,
        });
        Ok(())
    }

    /// Returns IDs of pending work items (delay_remaining == 0).
    pub fn work_pending(&self) -> Vec<u64> {
        self.work_items
            .iter()
            .filter(|w| w.delay_remaining == 0)
            .map(|w| w.work_id)
            .collect()
    }

    // ─── Tick / System ───────────────────────────────────────────────

    /// Advances the simulation by one millisecond tick.
    ///
    /// Processes:
    /// - Decrements delayed work items
    /// - Expires running timers
    pub fn tick(&mut self) {
        self.tick_count += 1;

        // Process delayed work items
        for work in &mut self.work_items {
            if work.delay_remaining > 0 {
                work.delay_remaining -= 1;
            }
        }

        // Process timers
        let timer_handles: Vec<u64> = self.timers.keys().copied().collect();
        for handle in timer_handles {
            if let Some(timer) = self.timers.get_mut(&handle) {
                if timer.running {
                    timer.ticks_remaining = timer.ticks_remaining.saturating_sub(1);
                    if timer.ticks_remaining == 0 {
                        timer.expiry_count += 1;
                        if timer.auto_reload {
                            timer.ticks_remaining = timer.period_ms;
                        } else {
                            timer.running = false;
                        }
                    }
                }
            }
        }
    }
}

impl Default for ZephyrSim {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// FFI declarations (behind feature gate)
// ═══════════════════════════════════════════════════════════════════════

/// Zephyr kernel timeout type (opaque in actual Zephyr).
#[cfg(feature = "zephyr")]
#[repr(C)]
pub struct KTimeout {
    /// Timeout ticks.
    pub ticks: i64,
}

/// Zephyr kernel thread ID type.
#[cfg(feature = "zephyr")]
pub type KTidT = *mut core::ffi::c_void;

/// Opaque k_msgq struct pointer.
#[cfg(feature = "zephyr")]
pub type KMsgq = core::ffi::c_void;

/// Opaque k_mutex struct pointer.
#[cfg(feature = "zephyr")]
pub type KMutex = core::ffi::c_void;

/// Opaque k_sem struct pointer.
#[cfg(feature = "zephyr")]
pub type KSem = core::ffi::c_void;

/// Opaque k_timer struct pointer.
#[cfg(feature = "zephyr")]
pub type KTimer = core::ffi::c_void;

/// Timer expiry callback type.
#[cfg(feature = "zephyr")]
pub type KTimerExpiryT = Option<unsafe extern "C" fn(timer: *mut KTimer)>;

/// Timer stop callback type.
#[cfg(feature = "zephyr")]
pub type KTimerStopT = Option<unsafe extern "C" fn(timer: *mut KTimer)>;

#[cfg(feature = "zephyr")]
extern "C" {
    /// Creates a Zephyr thread.
    fn k_thread_create(
        new_thread: *mut core::ffi::c_void,
        stack: *mut core::ffi::c_void,
        stack_size: usize,
        entry: unsafe extern "C" fn(
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
        ),
        p1: *mut core::ffi::c_void,
        p2: *mut core::ffi::c_void,
        p3: *mut core::ffi::c_void,
        prio: i32,
        options: u32,
        delay: KTimeout,
    ) -> KTidT;

    /// Aborts a Zephyr thread.
    fn k_thread_abort(tid: KTidT);

    /// Suspends a Zephyr thread.
    fn k_thread_suspend(tid: KTidT);

    /// Resumes a Zephyr thread.
    fn k_thread_resume(tid: KTidT);

    /// Sleeps for the given timeout.
    fn k_sleep(timeout: KTimeout) -> i32;

    /// Yields the CPU.
    fn k_yield();

    /// Initializes a message queue.
    fn k_msgq_init(msgq: *mut KMsgq, buffer: *mut u8, msg_size: usize, max_msgs: u32);

    /// Puts a message into a queue.
    fn k_msgq_put(msgq: *mut KMsgq, data: *const u8, timeout: KTimeout) -> i32;

    /// Gets a message from a queue.
    fn k_msgq_get(msgq: *mut KMsgq, data: *mut u8, timeout: KTimeout) -> i32;

    /// Initializes a mutex.
    fn k_mutex_init(mutex: *mut KMutex) -> i32;

    /// Locks a mutex.
    fn k_mutex_lock(mutex: *mut KMutex, timeout: KTimeout) -> i32;

    /// Unlocks a mutex.
    fn k_mutex_unlock(mutex: *mut KMutex) -> i32;

    /// Initializes a semaphore.
    fn k_sem_init(sem: *mut KSem, initial: u32, limit: u32) -> i32;

    /// Gives (signals) a semaphore.
    fn k_sem_give(sem: *mut KSem);

    /// Takes (waits on) a semaphore.
    fn k_sem_take(sem: *mut KSem, timeout: KTimeout) -> i32;

    /// Initializes a timer.
    fn k_timer_init(timer: *mut KTimer, expiry_fn: KTimerExpiryT, stop_fn: KTimerStopT);

    /// Starts a timer.
    fn k_timer_start(timer: *mut KTimer, duration: KTimeout, period: KTimeout);

    /// Stops a timer.
    fn k_timer_stop(timer: *mut KTimer);
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Thread tests ────────────────────────────────────────────────

    #[test]
    fn thread_create_returns_ready_thread() {
        let mut sim = ZephyrSim::new();
        let handle = sim.thread_create("sensor", 5, 512, 0x100).unwrap();
        assert_eq!(sim.thread_state(handle), Some(ZephyrThreadState::Ready));
        assert_eq!(sim.thread_name(handle), Some("sensor"));
        assert_eq!(sim.thread_priority(handle), Some(5));
    }

    #[test]
    fn thread_create_rejects_invalid_priority() {
        let mut sim = ZephyrSim::new();
        let err = sim.thread_create("bad", 100, 512, 0).unwrap_err();
        assert!(matches!(err, ZephyrError::ThreadCreateFailed { .. }));
    }

    #[test]
    fn thread_create_rejects_small_stack() {
        let mut sim = ZephyrSim::new();
        let err = sim.thread_create("bad", 5, 64, 0).unwrap_err();
        assert!(matches!(err, ZephyrError::ThreadCreateFailed { .. }));
    }

    #[test]
    fn thread_lifecycle_abort_suspend_resume() {
        let mut sim = ZephyrSim::new();
        let h = sim.thread_create("worker", 3, 1024, 0).unwrap();

        // Suspend
        sim.thread_suspend(h).unwrap();
        assert_eq!(sim.thread_state(h), Some(ZephyrThreadState::Suspended));

        // Resume
        sim.thread_resume(h).unwrap();
        assert_eq!(sim.thread_state(h), Some(ZephyrThreadState::Ready));

        // Abort
        sim.thread_abort(h).unwrap();
        assert_eq!(sim.thread_state(h), Some(ZephyrThreadState::Terminated));

        // Suspend after abort does not change state
        sim.thread_suspend(h).unwrap();
        assert_eq!(sim.thread_state(h), Some(ZephyrThreadState::Terminated));
    }

    #[test]
    fn sleep_ms_advances_tick_count() {
        let mut sim = ZephyrSim::new();
        assert_eq!(sim.tick_count(), 0);
        sim.sleep_ms(100);
        assert_eq!(sim.tick_count(), 100);
        sim.sleep_ms(50);
        assert_eq!(sim.tick_count(), 150);
    }

    // ─── Message Queue tests ─────────────────────────────────────────

    #[test]
    fn msgq_roundtrip_put_get() {
        let mut sim = ZephyrSim::new();
        let q = sim.msgq_create(8, 4).unwrap();

        sim.msgq_put(q, 42, K_NO_WAIT).unwrap();
        sim.msgq_put(q, 99, K_NO_WAIT).unwrap();
        assert_eq!(sim.msgq_count(q), 2);

        let v1 = sim.msgq_get(q, K_NO_WAIT).unwrap();
        let v2 = sim.msgq_get(q, K_NO_WAIT).unwrap();
        assert_eq!(v1, 42);
        assert_eq!(v2, 99);
        assert_eq!(sim.msgq_count(q), 0);
    }

    #[test]
    fn msgq_full_returns_error() {
        let mut sim = ZephyrSim::new();
        let q = sim.msgq_create(8, 2).unwrap();

        sim.msgq_put(q, 1, K_NO_WAIT).unwrap();
        sim.msgq_put(q, 2, K_NO_WAIT).unwrap();
        let err = sim.msgq_put(q, 3, K_NO_WAIT).unwrap_err();
        assert!(matches!(err, ZephyrError::MsgqFull { capacity: 2 }));
    }

    #[test]
    fn msgq_empty_returns_error() {
        let mut sim = ZephyrSim::new();
        let q = sim.msgq_create(8, 4).unwrap();
        let err = sim.msgq_get(q, K_NO_WAIT).unwrap_err();
        assert!(matches!(err, ZephyrError::MsgqEmpty));
    }

    #[test]
    fn msgq_peek_does_not_remove() {
        let mut sim = ZephyrSim::new();
        let q = sim.msgq_create(8, 4).unwrap();
        sim.msgq_put(q, 77, K_NO_WAIT).unwrap();
        assert_eq!(sim.msgq_peek(q), Some(77));
        assert_eq!(sim.msgq_count(q), 1);
    }

    #[test]
    fn msgq_purge_clears_all() {
        let mut sim = ZephyrSim::new();
        let q = sim.msgq_create(8, 4).unwrap();
        sim.msgq_put(q, 1, K_NO_WAIT).unwrap();
        sim.msgq_put(q, 2, K_NO_WAIT).unwrap();
        sim.msgq_purge(q).unwrap();
        assert_eq!(sim.msgq_count(q), 0);
    }

    // ─── Mutex tests ─────────────────────────────────────────────────

    #[test]
    fn mutex_lock_unlock_cycle() {
        let mut sim = ZephyrSim::new();
        let m = sim.mutex_create().unwrap();

        assert!(!sim.mutex_is_locked(m));
        sim.mutex_lock(m, K_FOREVER).unwrap();
        assert!(sim.mutex_is_locked(m));
        sim.mutex_unlock(m).unwrap();
        assert!(!sim.mutex_is_locked(m));
    }

    #[test]
    fn mutex_double_lock_returns_timeout() {
        let mut sim = ZephyrSim::new();
        let m = sim.mutex_create().unwrap();
        sim.mutex_lock(m, K_FOREVER).unwrap();
        let err = sim.mutex_lock(m, 100).unwrap_err();
        assert!(matches!(err, ZephyrError::MutexTimeout { timeout_ms: 100 }));
    }

    #[test]
    fn mutex_unlock_when_not_locked_returns_not_owner() {
        let mut sim = ZephyrSim::new();
        let m = sim.mutex_create().unwrap();
        let err = sim.mutex_unlock(m).unwrap_err();
        assert!(matches!(err, ZephyrError::MutexNotOwner));
    }

    // ─── Semaphore tests ─────────────────────────────────────────────

    #[test]
    fn sem_binary_give_take() {
        let mut sim = ZephyrSim::new();
        let s = sim.sem_create(0, 1).unwrap();
        assert_eq!(sim.sem_count(s), 0);

        sim.sem_give(s).unwrap();
        assert_eq!(sim.sem_count(s), 1);

        // Give again should cap at limit (no error in Zephyr)
        sim.sem_give(s).unwrap();
        assert_eq!(sim.sem_count(s), 1);

        sim.sem_take(s, K_NO_WAIT).unwrap();
        assert_eq!(sim.sem_count(s), 0);

        let err = sim.sem_take(s, K_NO_WAIT).unwrap_err();
        assert!(matches!(err, ZephyrError::SemTimeout { .. }));
    }

    #[test]
    fn sem_counting_multiple() {
        let mut sim = ZephyrSim::new();
        let s = sim.sem_create(3, 5).unwrap();
        assert_eq!(sim.sem_count(s), 3);

        sim.sem_give(s).unwrap();
        sim.sem_give(s).unwrap();
        assert_eq!(sim.sem_count(s), 5);

        // At limit, give is silently capped
        sim.sem_give(s).unwrap();
        assert_eq!(sim.sem_count(s), 5);
    }

    // ─── Timer tests ─────────────────────────────────────────────────

    #[test]
    fn timer_periodic_expiry() {
        let mut sim = ZephyrSim::new();
        let t = sim.timer_create("heartbeat", 10, true).unwrap();

        sim.timer_start(t).unwrap();
        assert!(sim.timer_is_running(t));
        assert_eq!(sim.timer_remaining(t), 10);

        // Tick 10 times -> one expiry
        for _ in 0..10 {
            sim.tick();
        }
        assert_eq!(sim.timer_expiry_count(t), 1);
        assert!(sim.timer_is_running(t)); // auto-reload
        assert_eq!(sim.timer_remaining(t), 10); // reset

        // Another 10 ticks -> second expiry
        for _ in 0..10 {
            sim.tick();
        }
        assert_eq!(sim.timer_expiry_count(t), 2);
    }

    #[test]
    fn timer_oneshot_stops_after_expiry() {
        let mut sim = ZephyrSim::new();
        let t = sim.timer_create("timeout", 5, false).unwrap();

        sim.timer_start(t).unwrap();
        for _ in 0..5 {
            sim.tick();
        }
        assert_eq!(sim.timer_expiry_count(t), 1);
        assert!(!sim.timer_is_running(t)); // one-shot stops
    }

    #[test]
    fn timer_stop_prevents_expiry() {
        let mut sim = ZephyrSim::new();
        let t = sim.timer_create("cancel_me", 10, true).unwrap();
        sim.timer_start(t).unwrap();
        for _ in 0..5 {
            sim.tick();
        }
        sim.timer_stop(t).unwrap();
        for _ in 0..10 {
            sim.tick();
        }
        assert_eq!(sim.timer_expiry_count(t), 0);
        assert!(!sim.timer_is_running(t));
    }

    // ─── Work Queue tests ────────────────────────────────────────────

    #[test]
    fn work_submit_immediately_pending() {
        let mut sim = ZephyrSim::new();
        sim.work_submit(1).unwrap();
        sim.work_submit(2).unwrap();
        let pending = sim.work_pending();
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&1));
        assert!(pending.contains(&2));
    }

    #[test]
    fn work_schedule_delayed_becomes_pending_after_ticks() {
        let mut sim = ZephyrSim::new();
        sim.work_schedule(42, 3).unwrap();
        assert_eq!(sim.work_pending().len(), 0);

        sim.tick();
        sim.tick();
        assert_eq!(sim.work_pending().len(), 0);

        sim.tick();
        assert_eq!(sim.work_pending().len(), 1);
        assert_eq!(sim.work_pending()[0], 42);
    }

    #[test]
    fn thread_count_excludes_terminated() {
        let mut sim = ZephyrSim::new();
        let h1 = sim.thread_create("t1", 1, 256, 0).unwrap();
        let _h2 = sim.thread_create("t2", 2, 256, 0).unwrap();
        assert_eq!(sim.thread_count(), 2);

        sim.thread_abort(h1).unwrap();
        assert_eq!(sim.thread_count(), 1);
    }
}
