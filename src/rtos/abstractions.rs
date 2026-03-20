//! Language-level RTOS abstractions for Fajar Lang.
//!
//! Provides high-level, type-safe wrappers around the FreeRTOS FFI
//! primitives. These abstractions are what Fajar Lang programs interact
//! with directly, using Rust-style ownership and generics.
//!
//! # Abstractions
//!
//! - [`RtosTask`] — Spawnable task with priority and stack size
//! - [`RtosQueue`] — Generic FIFO queue with send/receive
//! - [`RtosMutex`] — Mutex with priority inheritance
//! - [`RtosSemaphore`] — Binary and counting semaphores
//! - [`RtosEventGroup`] — Event flag group with set/wait/sync
//! - [`RtosScheduler`] — Scheduler control (start/suspend/resume)
//! - Static task allocation support for @kernel/@safe contexts

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

use super::freertos::{TaskPriority, TaskState};

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from RTOS abstraction operations.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RtosError {
    /// Task spawn failed.
    #[error("task spawn failed: {reason}")]
    SpawnFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Invalid task handle.
    #[error("invalid task handle: {0}")]
    InvalidTask(u64),

    /// Queue operation failed.
    #[error("queue error: {reason}")]
    QueueError {
        /// Reason for failure.
        reason: String,
    },

    /// Mutex operation failed.
    #[error("mutex error: {reason}")]
    MutexError {
        /// Reason for failure.
        reason: String,
    },

    /// Semaphore operation failed.
    #[error("semaphore error: {reason}")]
    SemaphoreError {
        /// Reason for failure.
        reason: String,
    },

    /// Event group operation failed.
    #[error("event group error: {reason}")]
    EventGroupError {
        /// Reason for failure.
        reason: String,
    },

    /// Scheduler operation failed.
    #[error("scheduler error: {reason}")]
    SchedulerError {
        /// Reason for failure.
        reason: String,
    },

    /// Static allocation buffer too small.
    #[error("static buffer too small: need {needed} words, have {available}")]
    StaticBufferTooSmall {
        /// Words needed.
        needed: u32,
        /// Words available.
        available: u32,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Handle types
// ═══════════════════════════════════════════════════════════════════════

static NEXT_RTOS_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Generates a unique RTOS handle.
fn next_handle() -> u64 {
    NEXT_RTOS_HANDLE.fetch_add(1, Ordering::Relaxed)
}

// ═══════════════════════════════════════════════════════════════════════
// RtosTask
// ═══════════════════════════════════════════════════════════════════════

/// Allocation mode for a task's stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAllocMode {
    /// Dynamic allocation (heap-based). Not available in @kernel context.
    Dynamic,
    /// Static allocation from a pre-allocated buffer. Safe for @kernel.
    Static,
}

/// A high-level RTOS task wrapper.
///
/// Wraps a FreeRTOS task handle with spawn/delete/suspend/resume.
/// Tracks its own state for safe lifecycle management.
#[derive(Debug, Clone)]
pub struct RtosTask {
    /// Unique task ID.
    id: u64,
    /// Task name.
    name: String,
    /// Task priority.
    priority: TaskPriority,
    /// Stack size in words.
    stack_size: u32,
    /// Function ID (simulated entry point).
    fn_id: u64,
    /// Current state.
    state: TaskState,
    /// Allocation mode.
    alloc_mode: TaskAllocMode,
    /// Stack usage watermark (high-water mark in words).
    stack_watermark: u32,
}

impl RtosTask {
    /// Spawns a new task with the given parameters.
    ///
    /// # Arguments
    /// * `name` - Task name
    /// * `priority` - Task priority (0 = idle, higher = more urgent)
    /// * `stack_size` - Stack size in words (min 128)
    /// * `fn_id` - Function pointer/ID for the task entry
    pub fn spawn(
        name: &str,
        priority: u32,
        stack_size: u32,
        fn_id: u64,
    ) -> Result<Self, RtosError> {
        if stack_size < 128 {
            return Err(RtosError::SpawnFailed {
                reason: format!("stack size {stack_size} too small (min: 128)"),
            });
        }
        if priority >= 56 {
            return Err(RtosError::SpawnFailed {
                reason: format!("priority {priority} out of range (max: 55)"),
            });
        }

        Ok(Self {
            id: next_handle(),
            name: name.to_string(),
            priority: TaskPriority(priority),
            stack_size,
            fn_id,
            state: TaskState::Ready,
            alloc_mode: TaskAllocMode::Dynamic,
            stack_watermark: stack_size,
        })
    }

    /// Spawns a task with static allocation (for @kernel context).
    ///
    /// # Arguments
    /// * `name` - Task name
    /// * `priority` - Task priority
    /// * `static_stack_words` - Pre-allocated static stack buffer size
    /// * `fn_id` - Function pointer/ID
    pub fn spawn_static(
        name: &str,
        priority: u32,
        static_stack_words: u32,
        fn_id: u64,
    ) -> Result<Self, RtosError> {
        if static_stack_words < 128 {
            return Err(RtosError::StaticBufferTooSmall {
                needed: 128,
                available: static_stack_words,
            });
        }
        if priority >= 56 {
            return Err(RtosError::SpawnFailed {
                reason: format!("priority {priority} out of range (max: 55)"),
            });
        }

        Ok(Self {
            id: next_handle(),
            name: name.to_string(),
            priority: TaskPriority(priority),
            stack_size: static_stack_words,
            fn_id,
            state: TaskState::Ready,
            alloc_mode: TaskAllocMode::Static,
            stack_watermark: static_stack_words,
        })
    }

    /// Returns the task ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the task name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the task priority.
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }

    /// Returns the stack size in words.
    pub fn stack_size(&self) -> u32 {
        self.stack_size
    }

    /// Returns the function ID.
    pub fn fn_id(&self) -> u64 {
        self.fn_id
    }

    /// Returns the current task state.
    pub fn state(&self) -> TaskState {
        self.state
    }

    /// Returns the allocation mode.
    pub fn alloc_mode(&self) -> TaskAllocMode {
        self.alloc_mode
    }

    /// Returns the stack usage watermark (minimum free stack ever).
    pub fn stack_watermark(&self) -> u32 {
        self.stack_watermark
    }

    /// Suspends this task.
    pub fn suspend(&mut self) {
        if self.state != TaskState::Deleted {
            self.state = TaskState::Suspended;
        }
    }

    /// Resumes this task from suspended state.
    pub fn resume(&mut self) {
        if self.state == TaskState::Suspended {
            self.state = TaskState::Ready;
        }
    }

    /// Marks this task as deleted.
    pub fn delete(&mut self) {
        self.state = TaskState::Deleted;
    }

    /// Simulates stack usage (reduces watermark).
    pub fn simulate_stack_usage(&mut self, words_used: u32) {
        let remaining = self.stack_size.saturating_sub(words_used);
        if remaining < self.stack_watermark {
            self.stack_watermark = remaining;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RtosQueue<T>
// ═══════════════════════════════════════════════════════════════════════

/// A generic RTOS queue with FIFO semantics.
///
/// Models FreeRTOS queues with type safety. Items are stored as `u64`
/// values internally (matching the FreeRTOS simulation).
#[derive(Debug, Clone)]
pub struct RtosQueue {
    /// Queue ID.
    id: u64,
    /// Items in the queue.
    items: Vec<u64>,
    /// Maximum capacity.
    capacity: u32,
}

impl RtosQueue {
    /// Creates a new queue with the given capacity.
    pub fn new(capacity: u32) -> Result<Self, RtosError> {
        if capacity == 0 {
            return Err(RtosError::QueueError {
                reason: "capacity must be > 0".to_string(),
            });
        }
        Ok(Self {
            id: next_handle(),
            items: Vec::new(),
            capacity,
        })
    }

    /// Returns the queue ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the queue capacity.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> u32 {
        self.items.len() as u32
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns whether the queue is full.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity as usize
    }

    /// Sends an item to the queue.
    pub fn send(&mut self, item: u64) -> Result<(), RtosError> {
        if self.is_full() {
            return Err(RtosError::QueueError {
                reason: format!("queue full (capacity: {})", self.capacity),
            });
        }
        self.items.push(item);
        Ok(())
    }

    /// Receives an item from the queue (FIFO).
    pub fn receive(&mut self) -> Result<u64, RtosError> {
        if self.is_empty() {
            return Err(RtosError::QueueError {
                reason: "queue empty".to_string(),
            });
        }
        Ok(self.items.remove(0))
    }

    /// ISR-safe send (non-blocking).
    pub fn send_from_isr(&mut self, item: u64) -> bool {
        if self.is_full() {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Peeks at the front item without removing it.
    pub fn peek(&self) -> Option<u64> {
        self.items.first().copied()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RtosMutex
// ═══════════════════════════════════════════════════════════════════════

/// An RTOS mutex with priority inheritance support.
///
/// When a high-priority task blocks on a mutex held by a lower-priority
/// task, the holder's priority is temporarily boosted to prevent
/// priority inversion.
#[derive(Debug, Clone)]
pub struct RtosMutex {
    /// Mutex ID.
    id: u64,
    /// Whether the mutex is locked.
    locked: bool,
    /// Task ID that holds the lock.
    holder_task_id: Option<u64>,
    /// Original priority of the holder (before inheritance).
    original_priority: Option<TaskPriority>,
    /// Whether priority inheritance is enabled.
    priority_inheritance: bool,
}

impl RtosMutex {
    /// Creates a new mutex with priority inheritance enabled.
    pub fn new() -> Self {
        Self {
            id: next_handle(),
            locked: false,
            holder_task_id: None,
            original_priority: None,
            priority_inheritance: true,
        }
    }

    /// Creates a new mutex without priority inheritance (recursive mutex).
    pub fn new_no_inheritance() -> Self {
        Self {
            id: next_handle(),
            locked: false,
            holder_task_id: None,
            original_priority: None,
            priority_inheritance: false,
        }
    }

    /// Returns the mutex ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns whether the mutex is locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Returns the task ID of the holder.
    pub fn holder(&self) -> Option<u64> {
        self.holder_task_id
    }

    /// Returns whether priority inheritance is enabled.
    pub fn has_priority_inheritance(&self) -> bool {
        self.priority_inheritance
    }

    /// Locks the mutex for a given task.
    pub fn lock(&mut self, task_id: u64) -> Result<(), RtosError> {
        if self.locked {
            return Err(RtosError::MutexError {
                reason: format!(
                    "mutex already held by task {}",
                    self.holder_task_id.unwrap_or(0)
                ),
            });
        }
        self.locked = true;
        self.holder_task_id = Some(task_id);
        Ok(())
    }

    /// Unlocks the mutex.
    pub fn unlock(&mut self) -> Result<(), RtosError> {
        if !self.locked {
            return Err(RtosError::MutexError {
                reason: "mutex not locked".to_string(),
            });
        }
        self.locked = false;
        self.holder_task_id = None;
        self.original_priority = None;
        Ok(())
    }

    /// Applies priority inheritance: boosts the holder's priority.
    ///
    /// Returns the original priority that was saved.
    pub fn apply_priority_inheritance(
        &mut self,
        holder_priority: TaskPriority,
        waiter_priority: TaskPriority,
    ) -> Option<TaskPriority> {
        if !self.priority_inheritance {
            return None;
        }
        if waiter_priority > holder_priority && self.locked {
            self.original_priority = Some(holder_priority);
            Some(holder_priority)
        } else {
            None
        }
    }

    /// Returns the saved original priority (before inheritance).
    pub fn original_priority(&self) -> Option<TaskPriority> {
        self.original_priority
    }
}

impl Default for RtosMutex {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RtosSemaphore
// ═══════════════════════════════════════════════════════════════════════

/// Semaphore type (binary or counting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemaphoreKind {
    /// Binary semaphore (max count = 1).
    Binary,
    /// Counting semaphore (max count > 1).
    Counting,
}

/// An RTOS semaphore (binary or counting).
#[derive(Debug, Clone)]
pub struct RtosSemaphore {
    /// Semaphore ID.
    id: u64,
    /// Semaphore kind.
    kind: SemaphoreKind,
    /// Current count.
    count: u32,
    /// Maximum count.
    max_count: u32,
}

impl RtosSemaphore {
    /// Creates a binary semaphore (max count = 1).
    pub fn binary(initial: bool) -> Self {
        Self {
            id: next_handle(),
            kind: SemaphoreKind::Binary,
            count: if initial { 1 } else { 0 },
            max_count: 1,
        }
    }

    /// Creates a counting semaphore.
    pub fn counting(max_count: u32, initial: u32) -> Self {
        let initial = if initial > max_count {
            max_count
        } else {
            initial
        };
        Self {
            id: next_handle(),
            kind: SemaphoreKind::Counting,
            count: initial,
            max_count,
        }
    }

    /// Returns the semaphore ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the semaphore kind.
    pub fn kind(&self) -> SemaphoreKind {
        self.kind
    }

    /// Returns the current count.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Returns the maximum count.
    pub fn max_count(&self) -> u32 {
        self.max_count
    }

    /// Gives (signals) the semaphore.
    pub fn give(&mut self) -> Result<(), RtosError> {
        if self.count >= self.max_count {
            return Err(RtosError::SemaphoreError {
                reason: format!("count at maximum ({})", self.max_count),
            });
        }
        self.count += 1;
        Ok(())
    }

    /// Takes (waits on) the semaphore.
    pub fn take(&mut self) -> Result<(), RtosError> {
        if self.count == 0 {
            return Err(RtosError::SemaphoreError {
                reason: "semaphore count is 0".to_string(),
            });
        }
        self.count -= 1;
        Ok(())
    }

    /// ISR-safe give (non-blocking).
    pub fn give_from_isr(&mut self) -> bool {
        if self.count >= self.max_count {
            return false;
        }
        self.count += 1;
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RtosEventGroup
// ═══════════════════════════════════════════════════════════════════════

/// An event group for multi-bit event synchronization.
///
/// Models FreeRTOS event groups (xEventGroupCreate). Each event group
/// holds up to 24 event bits that tasks can set, clear, and wait on.
#[derive(Debug, Clone)]
pub struct RtosEventGroup {
    /// Event group ID.
    id: u64,
    /// Current event bits (24 usable bits).
    bits: u32,
}

/// Maximum usable event bits (FreeRTOS reserves top 8 bits).
pub const EVENT_GROUP_MAX_BITS: u32 = 0x00FF_FFFF;

impl RtosEventGroup {
    /// Creates a new event group with all bits cleared.
    pub fn new() -> Self {
        Self {
            id: next_handle(),
            bits: 0,
        }
    }

    /// Returns the event group ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the current event bits.
    pub fn bits(&self) -> u32 {
        self.bits & EVENT_GROUP_MAX_BITS
    }

    /// Sets the specified bits (xEventGroupSetBits equivalent).
    pub fn set_bits(&mut self, bits_to_set: u32) -> u32 {
        self.bits |= bits_to_set & EVENT_GROUP_MAX_BITS;
        self.bits & EVENT_GROUP_MAX_BITS
    }

    /// Clears the specified bits (xEventGroupClearBits equivalent).
    pub fn clear_bits(&mut self, bits_to_clear: u32) -> u32 {
        let prev = self.bits & EVENT_GROUP_MAX_BITS;
        self.bits &= !(bits_to_clear & EVENT_GROUP_MAX_BITS);
        prev
    }

    /// Waits for the specified bits to be set.
    ///
    /// # Arguments
    /// * `bits_to_wait` - Bits to wait for
    /// * `wait_all` - If true, ALL bits must be set; if false, ANY bit
    /// * `clear_on_exit` - Whether to clear the matched bits
    ///
    /// Returns `Ok(current_bits)` if the condition is met, or
    /// `Err` if the bits are not currently set.
    pub fn wait_bits(
        &mut self,
        bits_to_wait: u32,
        wait_all: bool,
        clear_on_exit: bool,
    ) -> Result<u32, RtosError> {
        let masked = bits_to_wait & EVENT_GROUP_MAX_BITS;
        let current = self.bits & EVENT_GROUP_MAX_BITS;

        let condition_met = if wait_all {
            (current & masked) == masked
        } else {
            (current & masked) != 0
        };

        if condition_met {
            let result = current;
            if clear_on_exit {
                self.bits &= !masked;
            }
            Ok(result)
        } else {
            Err(RtosError::EventGroupError {
                reason: format!(
                    "bits not ready: wanted {:#010X}, have {:#010X}",
                    masked, current
                ),
            })
        }
    }

    /// Synchronization point (xEventGroupSync equivalent).
    ///
    /// Sets `bits_to_set`, then waits for `bits_to_wait`.
    /// Used for rendezvous/barrier patterns.
    pub fn sync(&mut self, bits_to_set: u32, bits_to_wait: u32) -> Result<u32, RtosError> {
        self.set_bits(bits_to_set);
        self.wait_bits(bits_to_wait, true, true)
    }
}

impl Default for RtosEventGroup {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RtosScheduler
// ═══════════════════════════════════════════════════════════════════════

/// Scheduler state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerState {
    /// Scheduler has not started.
    NotStarted,
    /// Scheduler is running.
    Running,
    /// Scheduler is suspended.
    Suspended,
}

/// RTOS scheduler control.
///
/// Manages the overall scheduler lifecycle and provides system-wide
/// operations like yielding and querying the idle task status.
#[derive(Debug)]
pub struct RtosScheduler {
    /// Current scheduler state.
    state: SchedulerState,
    /// Registered tasks.
    tasks: HashMap<u64, RtosTask>,
    /// System tick count.
    tick_count: u64,
    /// Suspend nesting count.
    suspend_nesting: u32,
}

impl RtosScheduler {
    /// Creates a new scheduler (not yet started).
    pub fn new() -> Self {
        Self {
            state: SchedulerState::NotStarted,
            tasks: HashMap::new(),
            tick_count: 0,
            suspend_nesting: 0,
        }
    }

    /// Returns the scheduler state.
    pub fn state(&self) -> SchedulerState {
        self.state
    }

    /// Returns the current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Returns the number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Registers a task with the scheduler.
    pub fn register_task(&mut self, task: RtosTask) {
        self.tasks.insert(task.id(), task);
    }

    /// Returns a reference to a task by ID.
    pub fn get_task(&self, id: u64) -> Option<&RtosTask> {
        self.tasks.get(&id)
    }

    /// Returns a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, id: u64) -> Option<&mut RtosTask> {
        self.tasks.get_mut(&id)
    }

    /// Starts the scheduler (vTaskStartScheduler equivalent).
    pub fn start(&mut self) -> Result<(), RtosError> {
        if self.state != SchedulerState::NotStarted {
            return Err(RtosError::SchedulerError {
                reason: "scheduler already started".to_string(),
            });
        }
        self.state = SchedulerState::Running;
        Ok(())
    }

    /// Suspends the scheduler (vTaskSuspendAll equivalent).
    ///
    /// Can be called multiple times (nested); each call increments the
    /// suspend count.
    pub fn suspend(&mut self) -> Result<(), RtosError> {
        if self.state == SchedulerState::NotStarted {
            return Err(RtosError::SchedulerError {
                reason: "scheduler not started".to_string(),
            });
        }
        self.suspend_nesting += 1;
        self.state = SchedulerState::Suspended;
        Ok(())
    }

    /// Resumes the scheduler (xTaskResumeAll equivalent).
    ///
    /// Must be called once for each `suspend()` call.
    pub fn resume(&mut self) -> Result<(), RtosError> {
        if self.state != SchedulerState::Suspended {
            return Err(RtosError::SchedulerError {
                reason: "scheduler not suspended".to_string(),
            });
        }
        self.suspend_nesting = self.suspend_nesting.saturating_sub(1);
        if self.suspend_nesting == 0 {
            self.state = SchedulerState::Running;
        }
        Ok(())
    }

    /// Returns the suspend nesting depth.
    pub fn suspend_nesting(&self) -> u32 {
        self.suspend_nesting
    }

    /// Simulates a scheduler tick.
    ///
    /// Increments tick count and advances blocked tasks.
    pub fn tick(&mut self) {
        if self.state == SchedulerState::Running {
            self.tick_count += 1;
        }
    }

    /// Returns the highest-priority ready task (scheduling decision).
    pub fn highest_priority_ready(&self) -> Option<&RtosTask> {
        self.tasks
            .values()
            .filter(|t| t.state() == TaskState::Ready)
            .max_by_key(|t| t.priority())
    }
}

impl Default for RtosScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── RtosTask tests ──────────────────────────────────────────────

    #[test]
    fn task_spawn_creates_ready_task() {
        let task = RtosTask::spawn("sensor_read", 2, 256, 0x1000).unwrap();
        assert_eq!(task.name(), "sensor_read");
        assert_eq!(task.priority(), TaskPriority(2));
        assert_eq!(task.stack_size(), 256);
        assert_eq!(task.state(), TaskState::Ready);
        assert_eq!(task.alloc_mode(), TaskAllocMode::Dynamic);
    }

    #[test]
    fn task_spawn_rejects_small_stack() {
        let err = RtosTask::spawn("bad", 1, 64, 0).unwrap_err();
        assert!(matches!(err, RtosError::SpawnFailed { .. }));
    }

    #[test]
    fn task_spawn_rejects_high_priority() {
        let err = RtosTask::spawn("bad", 56, 256, 0).unwrap_err();
        assert!(matches!(err, RtosError::SpawnFailed { .. }));
    }

    #[test]
    fn task_spawn_static_allocation() {
        let task = RtosTask::spawn_static("kernel_task", 3, 512, 0x2000).unwrap();
        assert_eq!(task.alloc_mode(), TaskAllocMode::Static);
        assert_eq!(task.stack_size(), 512);
    }

    #[test]
    fn task_suspend_resume_cycle() {
        let mut task = RtosTask::spawn("t1", 1, 256, 0).unwrap();
        task.suspend();
        assert_eq!(task.state(), TaskState::Suspended);
        task.resume();
        assert_eq!(task.state(), TaskState::Ready);
    }

    #[test]
    fn task_delete_marks_deleted() {
        let mut task = RtosTask::spawn("t1", 1, 256, 0).unwrap();
        task.delete();
        assert_eq!(task.state(), TaskState::Deleted);
        // Suspend after delete should not change state
        task.suspend();
        assert_eq!(task.state(), TaskState::Deleted);
    }

    #[test]
    fn task_stack_watermark_tracking() {
        let mut task = RtosTask::spawn("t1", 1, 256, 0).unwrap();
        assert_eq!(task.stack_watermark(), 256);
        task.simulate_stack_usage(100);
        assert_eq!(task.stack_watermark(), 156);
        task.simulate_stack_usage(200);
        assert_eq!(task.stack_watermark(), 56);
        // Watermark only decreases, not increases
        task.simulate_stack_usage(50);
        assert_eq!(task.stack_watermark(), 56);
    }

    // ─── RtosQueue tests ─────────────────────────────────────────────

    #[test]
    fn queue_send_receive_fifo() {
        let mut q = RtosQueue::new(10).unwrap();
        q.send(1).unwrap();
        q.send(2).unwrap();
        q.send(3).unwrap();
        assert_eq!(q.len(), 3);
        assert_eq!(q.receive().unwrap(), 1);
        assert_eq!(q.receive().unwrap(), 2);
        assert_eq!(q.receive().unwrap(), 3);
        assert!(q.is_empty());
    }

    #[test]
    fn queue_full_returns_error() {
        let mut q = RtosQueue::new(2).unwrap();
        q.send(1).unwrap();
        q.send(2).unwrap();
        assert!(q.is_full());
        let err = q.send(3).unwrap_err();
        assert!(matches!(err, RtosError::QueueError { .. }));
    }

    #[test]
    fn queue_empty_returns_error() {
        let mut q = RtosQueue::new(10).unwrap();
        let err = q.receive().unwrap_err();
        assert!(matches!(err, RtosError::QueueError { .. }));
    }

    #[test]
    fn queue_isr_send() {
        let mut q = RtosQueue::new(2).unwrap();
        assert!(q.send_from_isr(42));
        assert!(q.send_from_isr(43));
        assert!(!q.send_from_isr(44)); // full
    }

    #[test]
    fn queue_peek_does_not_remove() {
        let mut q = RtosQueue::new(10).unwrap();
        q.send(99).unwrap();
        assert_eq!(q.peek(), Some(99));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_zero_capacity_rejected() {
        let err = RtosQueue::new(0).unwrap_err();
        assert!(matches!(err, RtosError::QueueError { .. }));
    }

    // ─── RtosMutex tests ─────────────────────────────────────────────

    #[test]
    fn mutex_lock_unlock() {
        let mut m = RtosMutex::new();
        assert!(!m.is_locked());
        m.lock(1).unwrap();
        assert!(m.is_locked());
        assert_eq!(m.holder(), Some(1));
        m.unlock().unwrap();
        assert!(!m.is_locked());
        assert_eq!(m.holder(), None);
    }

    #[test]
    fn mutex_double_lock_fails() {
        let mut m = RtosMutex::new();
        m.lock(1).unwrap();
        let err = m.lock(2).unwrap_err();
        assert!(matches!(err, RtosError::MutexError { .. }));
    }

    #[test]
    fn mutex_unlock_when_not_locked_fails() {
        let mut m = RtosMutex::new();
        let err = m.unlock().unwrap_err();
        assert!(matches!(err, RtosError::MutexError { .. }));
    }

    #[test]
    fn mutex_priority_inheritance() {
        let mut m = RtosMutex::new();
        assert!(m.has_priority_inheritance());
        m.lock(1).unwrap();
        // High-priority task (5) waits on mutex held by low-priority task (1)
        let original = m.apply_priority_inheritance(TaskPriority(1), TaskPriority(5));
        assert_eq!(original, Some(TaskPriority(1)));
        assert_eq!(m.original_priority(), Some(TaskPriority(1)));
    }

    #[test]
    fn mutex_no_inheritance_skips_boost() {
        let mut m = RtosMutex::new_no_inheritance();
        assert!(!m.has_priority_inheritance());
        m.lock(1).unwrap();
        let original = m.apply_priority_inheritance(TaskPriority(1), TaskPriority(5));
        assert_eq!(original, None);
    }

    // ─── RtosSemaphore tests ─────────────────────────────────────────

    #[test]
    fn binary_semaphore_give_take() {
        let mut sem = RtosSemaphore::binary(false);
        assert_eq!(sem.kind(), SemaphoreKind::Binary);
        assert_eq!(sem.count(), 0);
        sem.give().unwrap();
        assert_eq!(sem.count(), 1);
        // Cannot give again (binary)
        let err = sem.give().unwrap_err();
        assert!(matches!(err, RtosError::SemaphoreError { .. }));
        sem.take().unwrap();
        assert_eq!(sem.count(), 0);
    }

    #[test]
    fn counting_semaphore() {
        let mut sem = RtosSemaphore::counting(5, 2);
        assert_eq!(sem.kind(), SemaphoreKind::Counting);
        assert_eq!(sem.count(), 2);
        assert_eq!(sem.max_count(), 5);
        sem.give().unwrap();
        sem.give().unwrap();
        sem.give().unwrap();
        assert_eq!(sem.count(), 5);
        let err = sem.give().unwrap_err();
        assert!(matches!(err, RtosError::SemaphoreError { .. }));
    }

    #[test]
    fn semaphore_take_empty_fails() {
        let mut sem = RtosSemaphore::binary(false);
        let err = sem.take().unwrap_err();
        assert!(matches!(err, RtosError::SemaphoreError { .. }));
    }

    #[test]
    fn semaphore_isr_give() {
        let mut sem = RtosSemaphore::binary(false);
        assert!(sem.give_from_isr());
        assert!(!sem.give_from_isr()); // already at max
    }

    // ─── RtosEventGroup tests ────────────────────────────────────────

    #[test]
    fn event_group_set_and_wait_bits() {
        let mut eg = RtosEventGroup::new();
        assert_eq!(eg.bits(), 0);

        eg.set_bits(0x03); // set bits 0 and 1
        assert_eq!(eg.bits(), 0x03);

        // Wait for bit 1 (any)
        let result = eg.wait_bits(0x02, false, false).unwrap();
        assert_eq!(result & 0x02, 0x02);
    }

    #[test]
    fn event_group_wait_all_bits() {
        let mut eg = RtosEventGroup::new();
        eg.set_bits(0x01);

        // Wait for bits 0 AND 1 (wait_all = true)
        let err = eg.wait_bits(0x03, true, false).unwrap_err();
        assert!(matches!(err, RtosError::EventGroupError { .. }));

        // Set bit 1 too
        eg.set_bits(0x02);
        let result = eg.wait_bits(0x03, true, false).unwrap();
        assert_eq!(result & 0x03, 0x03);
    }

    #[test]
    fn event_group_clear_on_exit() {
        let mut eg = RtosEventGroup::new();
        eg.set_bits(0x07); // bits 0, 1, 2

        // Wait for bits 0 and 1, clear on exit
        let result = eg.wait_bits(0x03, true, true).unwrap();
        assert_eq!(result & 0x07, 0x07); // all were set
        // Bits 0 and 1 should be cleared
        assert_eq!(eg.bits(), 0x04); // only bit 2 remains
    }

    #[test]
    fn event_group_sync_rendezvous() {
        let mut eg = RtosEventGroup::new();
        // Task A sets bit 0, waits for bit 0 | bit 1
        // Simulate task A setting its bit and task B already set
        eg.set_bits(0x02); // task B already set bit 1
        let result = eg.sync(0x01, 0x03).unwrap(); // set bit 0, wait for 0|1
        assert_eq!(result & 0x03, 0x03);
    }

    #[test]
    fn event_group_clear_bits() {
        let mut eg = RtosEventGroup::new();
        eg.set_bits(0xFF);
        let prev = eg.clear_bits(0x0F);
        assert_eq!(prev & 0xFF, 0xFF);
        assert_eq!(eg.bits() & 0xFF, 0xF0);
    }

    // ─── RtosScheduler tests ─────────────────────────────────────────

    #[test]
    fn scheduler_start_and_tick() {
        let mut sched = RtosScheduler::new();
        assert_eq!(sched.state(), SchedulerState::NotStarted);
        sched.start().unwrap();
        assert_eq!(sched.state(), SchedulerState::Running);
        sched.tick();
        sched.tick();
        assert_eq!(sched.tick_count(), 2);
    }

    #[test]
    fn scheduler_double_start_fails() {
        let mut sched = RtosScheduler::new();
        sched.start().unwrap();
        let err = sched.start().unwrap_err();
        assert!(matches!(err, RtosError::SchedulerError { .. }));
    }

    #[test]
    fn scheduler_suspend_resume_nesting() {
        let mut sched = RtosScheduler::new();
        sched.start().unwrap();

        sched.suspend().unwrap();
        assert_eq!(sched.state(), SchedulerState::Suspended);
        assert_eq!(sched.suspend_nesting(), 1);

        sched.suspend().unwrap();
        assert_eq!(sched.suspend_nesting(), 2);

        sched.resume().unwrap();
        assert_eq!(sched.state(), SchedulerState::Suspended); // still nested
        assert_eq!(sched.suspend_nesting(), 1);

        sched.resume().unwrap();
        assert_eq!(sched.state(), SchedulerState::Running);
        assert_eq!(sched.suspend_nesting(), 0);
    }

    #[test]
    fn scheduler_register_and_query_tasks() {
        let mut sched = RtosScheduler::new();
        let task = RtosTask::spawn("high_prio", 5, 256, 0).unwrap();
        let id = task.id();
        sched.register_task(task);

        assert_eq!(sched.task_count(), 1);
        let t = sched.get_task(id).unwrap();
        assert_eq!(t.name(), "high_prio");
    }

    #[test]
    fn scheduler_highest_priority_ready() {
        let mut sched = RtosScheduler::new();

        let t1 = RtosTask::spawn("low", 1, 256, 0).unwrap();
        let t2 = RtosTask::spawn("high", 5, 256, 0).unwrap();
        let t3 = RtosTask::spawn("mid", 3, 256, 0).unwrap();

        sched.register_task(t1);
        sched.register_task(t2);
        sched.register_task(t3);

        let best = sched.highest_priority_ready().unwrap();
        assert_eq!(best.priority(), TaskPriority(5));
        assert_eq!(best.name(), "high");
    }

    #[test]
    fn scheduler_tick_ignored_when_not_running() {
        let mut sched = RtosScheduler::new();
        sched.tick(); // should not increment
        assert_eq!(sched.tick_count(), 0);

        sched.start().unwrap();
        sched.tick();
        assert_eq!(sched.tick_count(), 1);

        sched.suspend().unwrap();
        sched.tick(); // suspended, should not increment
        assert_eq!(sched.tick_count(), 1);
    }

    #[test]
    fn static_task_rejects_small_buffer() {
        let err = RtosTask::spawn_static("bad", 1, 64, 0).unwrap_err();
        assert!(matches!(err, RtosError::StaticBufferTooSmall { .. }));
    }
}
