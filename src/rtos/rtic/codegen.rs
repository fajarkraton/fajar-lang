//! RTIC code generation — produces interrupt vector table, lock implementations,
//! spawn queues, and timer queues from a validated [`RticApp`].
//!
//! All output is simulation-based. No actual ARM assembly or linker sections
//! are produced; instead, the generated structures represent what _would_ be
//! emitted by a real RTIC code generator targeting Cortex-M.
//!
//! # Generated Artifacts
//!
//! - [`VectorTableEntry`] — interrupt vector -> ISR trampoline mapping
//! - [`IsrTrampoline`] — context save/restore wrapper around task handler
//! - [`BasepriLock`] — BASEPRI-based mutual exclusion for shared resources
//! - [`SpawnQueue`] — SPSC ring buffer for software task message passing
//! - [`MonotonicTimer`] — hardware timer binding for `spawn_after` scheduling
//! - [`TimerQueue`] — sorted list of pending scheduled task spawns
//! - [`RticCodegen`] — complete code generation output

use std::collections::HashMap;

use super::{
    compute_ceilings, validate_app, CriticalSectionAnalysis, DeadlockFreedomProof, RticApp,
    RticError, StackAnalysis,
};

// ═══════════════════════════════════════════════════════════════════════
// Sprint 20: RTIC Code Generation
// ═══════════════════════════════════════════════════════════════════════

/// An entry in the interrupt vector table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorTableEntry {
    /// Interrupt name (e.g., "TIM2", "EXTI0").
    pub interrupt: String,
    /// Vector position in the table (0-based, after fixed system exceptions).
    pub vector_position: u32,
    /// Task name that this vector dispatches to.
    pub task_name: String,
    /// Priority value to write to NVIC priority register.
    pub nvic_priority: u8,
}

/// Cortex-M interrupt name to vector position mapping (simulation).
///
/// In a real system, these come from the SVD/PAC. Here we assign
/// sequential positions starting at 16 (after the 16 system exceptions).
fn interrupt_to_vector(name: &str) -> u32 {
    // Common Cortex-M interrupt names -> simulated positions
    match name {
        "WWDG" => 16,
        "PVD" => 17,
        "TAMP_STAMP" => 18,
        "RTC_WKUP" => 19,
        "FLASH" => 20,
        "RCC" => 21,
        "EXTI0" => 22,
        "EXTI1" => 23,
        "EXTI2" => 24,
        "EXTI3" => 25,
        "EXTI4" => 26,
        "TIM1_BRK_TIM9" | "TIM1" => 40,
        "TIM2" => 44,
        "TIM3" => 45,
        "TIM4" => 46,
        "TIM5" => 66,
        "SPI1" => 51,
        "SPI2" => 52,
        "USART1" => 53,
        "USART2" => 54,
        "DMA1_Stream0" => 27,
        "DMA1_Stream1" => 28,
        "DMA2_Stream0" => 72,
        "PendSV" => 14,
        "SysTick" => 15,
        // Unknown: hash the name for a deterministic position
        _ => {
            let mut h: u32 = 0;
            for b in name.bytes() {
                h = h.wrapping_mul(31).wrapping_add(b as u32);
            }
            80 + (h % 100) // positions 80-179
        }
    }
}

/// An ISR trampoline — the wrapper code that saves context, calls the task
/// handler, and restores context on return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsrTrampoline {
    /// Task name.
    pub task_name: String,
    /// Handler function name.
    pub handler: String,
    /// Interrupt name (for the vector table).
    pub interrupt: String,
    /// Simulated assembly steps.
    pub steps: Vec<String>,
}

impl IsrTrampoline {
    /// Build an ISR trampoline for a task.
    pub fn build(task_name: &str, handler: &str, interrupt: &str) -> Self {
        let steps = vec![
            "push {r4-r11, lr}".to_string(),
            format!("bl {handler}"),
            "pop {r4-r11, pc}".to_string(),
        ];
        Self {
            task_name: task_name.to_string(),
            handler: handler.to_string(),
            interrupt: interrupt.to_string(),
            steps,
        }
    }
}

/// BASEPRI-based mutual exclusion lock.
///
/// On Cortex-M, writing to BASEPRI masks all interrupts with priority
/// numerically >= the written value. This provides a lock-free critical
/// section without disabling all interrupts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasepriLock {
    /// Resource being protected.
    pub resource: String,
    /// Ceiling priority to write to BASEPRI.
    pub ceiling: u8,
    /// Previous BASEPRI value to restore on unlock.
    pub old_basepri: u8,
}

impl BasepriLock {
    /// Create a lock for a resource at a given ceiling priority.
    pub fn new(resource: &str, ceiling: u8) -> Self {
        Self {
            resource: resource.to_string(),
            ceiling,
            old_basepri: 0,
        }
    }

    /// Simulated lock acquisition: save old BASEPRI and raise to ceiling.
    pub fn acquire(&mut self, current_basepri: u8) {
        self.old_basepri = current_basepri;
        // In real hardware: MSR BASEPRI, ceiling_shifted
    }

    /// Simulated lock release: restore old BASEPRI.
    pub fn release(&self) -> u8 {
        self.old_basepri
    }

    /// Returns the NVIC priority value (Cortex-M shifts priorities left by 4).
    pub fn nvic_ceiling(&self) -> u8 {
        self.ceiling.wrapping_shl(4)
    }
}

/// A software task spawn queue — SPSC (Single-Producer Single-Consumer)
/// ring buffer for passing messages between tasks via PendSV.
#[derive(Debug, Clone)]
pub struct SpawnQueue {
    /// Task name this queue spawns.
    pub task_name: String,
    /// Maximum capacity.
    pub capacity: usize,
    /// Current items in the queue (simulated).
    items: Vec<i64>,
    /// Read index.
    read_idx: usize,
    /// Write index.
    write_idx: usize,
}

impl SpawnQueue {
    /// Creates a new spawn queue with the given capacity.
    pub fn new(task_name: &str, capacity: usize) -> Self {
        Self {
            task_name: task_name.to_string(),
            capacity,
            items: Vec::with_capacity(capacity),
            read_idx: 0,
            write_idx: 0,
        }
    }

    /// Enqueue a message (spawn request).
    pub fn enqueue(&mut self, message: i64) -> Result<(), RticError> {
        if self.items.len() >= self.capacity {
            return Err(RticError::SpawnQueueFull {
                task: self.task_name.clone(),
                capacity: self.capacity,
            });
        }
        self.items.push(message);
        self.write_idx = self.write_idx.wrapping_add(1);
        Ok(())
    }

    /// Dequeue a message (consume spawn request).
    pub fn dequeue(&mut self) -> Option<i64> {
        if self.items.is_empty() {
            return None;
        }
        self.read_idx = self.read_idx.wrapping_add(1);
        Some(self.items.remove(0))
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of pending spawn requests.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the queue is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }
}

/// A monotonic timer binding — provides `spawn_after` scheduling capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonotonicTimer {
    /// Timer name (e.g., "tim2_mono").
    pub name: String,
    /// Hardware timer peripheral name.
    pub peripheral: String,
    /// Timer frequency in Hz.
    pub frequency_hz: u32,
    /// Current tick count (simulated).
    pub current_tick: u64,
}

impl MonotonicTimer {
    /// Creates a new monotonic timer binding.
    pub fn new(name: &str, peripheral: &str, frequency_hz: u32) -> Self {
        Self {
            name: name.to_string(),
            peripheral: peripheral.to_string(),
            frequency_hz,
            current_tick: 0,
        }
    }

    /// Advance the timer by a number of ticks (simulation).
    pub fn advance(&mut self, ticks: u64) {
        self.current_tick = self.current_tick.saturating_add(ticks);
    }

    /// Convert a duration in microseconds to ticks.
    pub fn us_to_ticks(&self, microseconds: u64) -> u64 {
        microseconds * self.frequency_hz as u64 / 1_000_000
    }

    /// Convert ticks to microseconds.
    pub fn ticks_to_us(&self, ticks: u64) -> u64 {
        if self.frequency_hz == 0 {
            return 0;
        }
        ticks * 1_000_000 / self.frequency_hz as u64
    }
}

/// An entry in the timer queue — a scheduled task spawn at a future tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerQueueEntry {
    /// Target tick at which the task should be spawned.
    pub deadline_tick: u64,
    /// Task name to spawn.
    pub task_name: String,
    /// Message payload.
    pub payload: i64,
}

/// Timer queue — sorted linked list of pending scheduled task spawns.
///
/// Entries are maintained in ascending deadline order. On each timer tick,
/// all entries whose deadline has passed are dispatched.
#[derive(Debug, Clone)]
pub struct TimerQueue {
    /// Sorted entries (ascending by deadline_tick).
    entries: Vec<TimerQueueEntry>,
    /// Maximum capacity.
    capacity: usize,
}

impl TimerQueue {
    /// Creates a new timer queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Schedule a task spawn at a future tick.
    pub fn schedule(
        &mut self,
        deadline_tick: u64,
        task_name: &str,
        payload: i64,
    ) -> Result<(), RticError> {
        if self.entries.len() >= self.capacity {
            return Err(RticError::TimerQueueError {
                detail: format!("timer queue full (capacity {})", self.capacity),
            });
        }

        let entry = TimerQueueEntry {
            deadline_tick,
            task_name: task_name.to_string(),
            payload,
        };

        // Insert in sorted order (ascending deadline)
        let pos = self
            .entries
            .iter()
            .position(|e| e.deadline_tick > deadline_tick)
            .unwrap_or(self.entries.len());
        self.entries.insert(pos, entry);

        Ok(())
    }

    /// Dispatch all entries whose deadline <= current_tick.
    ///
    /// Returns the dispatched entries (removed from the queue).
    pub fn dispatch(&mut self, current_tick: u64) -> Vec<TimerQueueEntry> {
        let split_pos = self
            .entries
            .iter()
            .position(|e| e.deadline_tick > current_tick)
            .unwrap_or(self.entries.len());

        self.entries.drain(..split_pos).collect()
    }

    /// Returns the number of pending entries.
    pub fn pending_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns the next deadline tick, or None if empty.
    pub fn next_deadline(&self) -> Option<u64> {
        self.entries.first().map(|e| e.deadline_tick)
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Complete RTIC code generation output.
#[derive(Debug, Clone)]
pub struct RticCodegen {
    /// Vector table entries (interrupt -> ISR mapping).
    pub vector_entries: Vec<VectorTableEntry>,
    /// ISR trampolines for each task.
    pub trampolines: Vec<IsrTrampoline>,
    /// Lock implementations for each resource.
    pub lock_implementations: Vec<BasepriLock>,
    /// Software task spawn queues.
    pub spawn_queues: Vec<SpawnQueue>,
    /// Timer queue for scheduled spawns.
    pub timer_queue: TimerQueue,
    /// Stack analysis results.
    pub stack_analysis: StackAnalysis,
    /// Critical section analysis results.
    pub critical_sections: CriticalSectionAnalysis,
}

/// Generate all RTIC code artifacts from a validated application.
///
/// This is the main entry point for RTIC code generation. It:
/// 1. Validates the app definition
/// 2. Computes resource ceilings
/// 3. Proves deadlock freedom
/// 4. Generates vector table, ISR trampolines, locks, and queues
pub fn generate_rtic_code(app: &RticApp) -> Result<RticCodegen, RticError> {
    // Step 1: Validate
    validate_app(app)?;

    // Step 2: Compute ceilings
    let ceilings = compute_ceilings(app);

    // Step 3: Prove deadlock freedom
    DeadlockFreedomProof::check(app)?;

    // Step 4: Generate vector table
    let vector_entries = generate_vector_table(app);

    // Step 5: Generate ISR trampolines
    let trampolines = generate_trampolines(app);

    // Step 6: Generate lock implementations
    let lock_implementations = generate_locks(&ceilings);

    // Step 7: Create spawn queues (default capacity 4 per task)
    let spawn_queues = generate_spawn_queues(app);

    // Step 8: Create timer queue
    let timer_queue = TimerQueue::new(32);

    // Step 9: Stack analysis
    let stack_analysis = StackAnalysis::analyze(app);

    // Step 10: Critical section analysis
    let critical_sections = CriticalSectionAnalysis::analyze(app, &ceilings);

    Ok(RticCodegen {
        vector_entries,
        trampolines,
        lock_implementations,
        spawn_queues,
        timer_queue,
        stack_analysis,
        critical_sections,
    })
}

/// Generate vector table entries from the app's task bindings.
fn generate_vector_table(app: &RticApp) -> Vec<VectorTableEntry> {
    let mut entries: Vec<VectorTableEntry> = app
        .tasks
        .iter()
        .map(|task| VectorTableEntry {
            interrupt: task.binds.clone(),
            vector_position: interrupt_to_vector(&task.binds),
            task_name: task.name.clone(),
            nvic_priority: task.priority,
        })
        .collect();
    entries.sort_by_key(|e| e.vector_position);
    entries
}

/// Generate ISR trampolines for each task.
fn generate_trampolines(app: &RticApp) -> Vec<IsrTrampoline> {
    app.tasks
        .iter()
        .map(|task| IsrTrampoline::build(&task.name, &task.handler, &task.binds))
        .collect()
}

/// Generate BASEPRI lock implementations for each resource.
fn generate_locks(ceilings: &HashMap<String, u8>) -> Vec<BasepriLock> {
    let mut locks: Vec<BasepriLock> = ceilings
        .iter()
        .filter(|(_, &ceil)| ceil > 0)
        .map(|(name, &ceil)| BasepriLock::new(name, ceil))
        .collect();
    locks.sort_by(|a, b| a.resource.cmp(&b.resource));
    locks
}

/// Generate spawn queues for each task (default capacity 4).
fn generate_spawn_queues(app: &RticApp) -> Vec<SpawnQueue> {
    app.tasks
        .iter()
        .map(|task| SpawnQueue::new(&task.name, 4))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtos::rtic::{IdleConfig, InitConfig, RticApp, RticResource, RticTask};

    fn sample_app() -> RticApp {
        let mut app = RticApp::new("stm32f407");

        app.add_resource(RticResource::new("sensor_data", "f32"));
        app.add_resource(RticResource::new("led_state", "bool"));

        app.add_task(
            RticTask::new("sensor_read", 3, "TIM2", "handle_sensor")
                .with_resource("sensor_data")
                .with_stack_frame(256),
        );
        app.add_task(
            RticTask::new("led_toggle", 1, "TIM3", "handle_led")
                .with_resource("led_state")
                .with_stack_frame(128),
        );

        app.set_init(InitConfig::new("init_fn").with_monotonic("tim5_mono"));
        app.set_idle(IdleConfig::new("idle_fn"));

        app
    }

    #[test]
    fn s20_1_vector_table_generation() {
        let app = sample_app();
        let codegen = generate_rtic_code(&app).unwrap();

        assert_eq!(codegen.vector_entries.len(), 2);
        let tim2_entry = codegen
            .vector_entries
            .iter()
            .find(|e| e.interrupt == "TIM2")
            .expect("TIM2 entry");
        assert_eq!(tim2_entry.vector_position, 44);
        assert_eq!(tim2_entry.task_name, "sensor_read");
        assert_eq!(tim2_entry.nvic_priority, 3);
    }

    #[test]
    fn s20_2_isr_trampoline_generation() {
        let app = sample_app();
        let codegen = generate_rtic_code(&app).unwrap();

        assert_eq!(codegen.trampolines.len(), 2);
        let tramp = &codegen.trampolines[0];
        assert_eq!(tramp.steps.len(), 3);
        assert!(tramp.steps[0].contains("push"));
        assert!(tramp.steps[1].contains("bl"));
        assert!(tramp.steps[2].contains("pop"));
    }

    #[test]
    fn s20_3_basepri_lock_generation() {
        let app = sample_app();
        let codegen = generate_rtic_code(&app).unwrap();

        // Both resources should have locks
        assert_eq!(codegen.lock_implementations.len(), 2);

        let sensor_lock = codegen
            .lock_implementations
            .iter()
            .find(|l| l.resource == "sensor_data")
            .expect("sensor lock");
        assert_eq!(sensor_lock.ceiling, 3); // max(3) since only sensor_read(3)
    }

    #[test]
    fn s20_4_basepri_lock_acquire_release() {
        let mut lock = BasepriLock::new("test_res", 5);
        assert_eq!(lock.ceiling, 5);
        assert_eq!(lock.nvic_ceiling(), 80); // 5 << 4

        lock.acquire(2);
        assert_eq!(lock.old_basepri, 2);

        let restored = lock.release();
        assert_eq!(restored, 2);
    }

    #[test]
    fn s20_5_spawn_queue_enqueue_dequeue() {
        let mut q = SpawnQueue::new("blink", 3);
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);

        q.enqueue(42).unwrap();
        q.enqueue(99).unwrap();
        assert_eq!(q.len(), 2);
        assert!(!q.is_full());

        assert_eq!(q.dequeue(), Some(42));
        assert_eq!(q.dequeue(), Some(99));
        assert_eq!(q.dequeue(), None);
        assert!(q.is_empty());
    }

    #[test]
    fn s20_6_spawn_queue_full_error() {
        let mut q = SpawnQueue::new("task_a", 2);
        q.enqueue(1).unwrap();
        q.enqueue(2).unwrap();
        assert!(q.is_full());

        let result = q.enqueue(3);
        assert!(matches!(result, Err(RticError::SpawnQueueFull { .. })));
    }

    #[test]
    fn s20_7_monotonic_timer_conversions() {
        let mut timer = MonotonicTimer::new("tim2_mono", "TIM2", 1_000_000);

        // 1 MHz timer: 1 us = 1 tick
        assert_eq!(timer.us_to_ticks(100), 100);
        assert_eq!(timer.ticks_to_us(100), 100);

        timer.advance(500);
        assert_eq!(timer.current_tick, 500);

        // 72 MHz timer
        let fast = MonotonicTimer::new("sys_mono", "SysTick", 72_000_000);
        assert_eq!(fast.us_to_ticks(1000), 72_000); // 1ms = 72000 ticks
    }

    #[test]
    fn s20_8_timer_queue_schedule_and_dispatch() {
        let mut tq = TimerQueue::new(16);

        // Schedule tasks at various deadlines
        tq.schedule(100, "task_b", 2).unwrap();
        tq.schedule(50, "task_a", 1).unwrap();
        tq.schedule(200, "task_c", 3).unwrap();

        assert_eq!(tq.pending_count(), 3);
        assert_eq!(tq.next_deadline(), Some(50));

        // Dispatch at tick 100 — should get task_a(50) and task_b(100)
        let dispatched = tq.dispatch(100);
        assert_eq!(dispatched.len(), 2);
        assert_eq!(dispatched[0].task_name, "task_a");
        assert_eq!(dispatched[1].task_name, "task_b");

        // task_c still pending
        assert_eq!(tq.pending_count(), 1);
        assert_eq!(tq.next_deadline(), Some(200));
    }

    #[test]
    fn s20_9_deadlock_freedom_in_generated_code() {
        let app = sample_app();
        let codegen = generate_rtic_code(&app).unwrap();

        // Generated code should have stack analysis
        assert!(codegen.stack_analysis.total_wcss > 0);

        // Critical sections should be analyzed
        assert!(
            codegen.critical_sections.lock_free_count > 0
                || codegen.critical_sections.locked_count > 0
        );
    }

    #[test]
    fn s20_10_full_codegen_validates_and_generates() {
        // Build a more complex app
        let mut app = RticApp::new("stm32f407");

        app.add_resource(RticResource::new("shared_buf", "[u8; 256]"));
        app.add_resource(RticResource::new("adc_value", "u16"));
        app.add_resource(RticResource::new("pwm_duty", "u16"));

        app.add_task(
            RticTask::new("adc_read", 4, "TIM2", "handle_adc")
                .with_resource("shared_buf")
                .with_resource("adc_value")
                .with_stack_frame(512),
        );
        app.add_task(
            RticTask::new("process", 2, "EXTI0", "handle_process")
                .with_resource("shared_buf")
                .with_resource("pwm_duty")
                .with_stack_frame(1024),
        );
        app.add_task(
            RticTask::new("pwm_update", 1, "TIM3", "handle_pwm")
                .with_resource("pwm_duty")
                .with_stack_frame(128),
        );

        app.set_init(InitConfig::new("init_hw"));
        app.set_idle(IdleConfig::new("idle_wfi"));

        let codegen = generate_rtic_code(&app).unwrap();

        // 3 vector entries
        assert_eq!(codegen.vector_entries.len(), 3);

        // 3 trampolines
        assert_eq!(codegen.trampolines.len(), 3);

        // All 3 resources should have locks
        assert_eq!(codegen.lock_implementations.len(), 3);

        // 3 spawn queues
        assert_eq!(codegen.spawn_queues.len(), 3);

        // Stack analysis: 3 priority levels
        assert_eq!(codegen.stack_analysis.priority_stacks.len(), 3);
        // WCSS = max(512) at p4 + max(1024) at p2 + max(128) at p1
        assert_eq!(codegen.stack_analysis.total_wcss, 512 + 1024 + 128);
    }
}
