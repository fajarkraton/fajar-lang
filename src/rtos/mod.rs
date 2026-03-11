//! RTOS integration module for Fajar Lang.
//!
//! Provides FreeRTOS FFI bindings, language-level abstractions, and
//! real-time analysis tools for embedded systems programming.
//!
//! # Module Structure
//!
//! ```text
//! rtos/
//! ├── freertos.rs      — FreeRTOS FFI bindings + simulation stubs
//! ├── abstractions.rs  — High-level RTOS abstractions (task, queue, mutex, etc.)
//! └── realtime.rs      — Real-time annotations and analysis
//! ```
//!
//! # Feature Gates
//!
//! - Default: simulation mode (all APIs work without FreeRTOS linked)
//! - `freertos` feature: links against actual FreeRTOS C library
//!
//! # Usage
//!
//! The RTOS module is designed for embedded real-time applications
//! using Fajar Lang's `@kernel` and `@safe` contexts.

pub mod abstractions;
pub mod freertos;
pub mod realtime;

// Re-exports for convenience
pub use abstractions::{
    RtosError, RtosEventGroup, RtosMutex, RtosQueue, RtosScheduler, RtosSemaphore, RtosTask,
    SchedulerState, SemaphoreKind, TaskAllocMode,
};
pub use freertos::{
    FreeRtosConfig, FreeRtosError, FreeRtosRuntime, MutexHandle, QueueHandle, SemaphoreHandle,
    TaskHandle, TaskPriority, TaskState, TimerHandle,
};
pub use realtime::{
    FunctionNode, IdleHook, MutexUsage, PeriodicTask, RealtimeConstraint, RealtimeError,
    RealtimeWarning, TickHook, TicklessIdleConfig, WcetEstimator,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtos_module_re_exports_freertos_types() {
        let _handle = TaskHandle(1);
        let _state = TaskState::Ready;
        let _prio = TaskPriority::IDLE;
        let _config = FreeRtosConfig::default();
    }

    #[test]
    fn rtos_module_re_exports_abstraction_types() {
        let task = RtosTask::spawn("test", 1, 256, 0).unwrap();
        assert_eq!(task.name(), "test");

        let queue = RtosQueue::new(10).unwrap();
        assert!(queue.is_empty());

        let mutex = RtosMutex::new();
        assert!(!mutex.is_locked());

        let sem = RtosSemaphore::binary(false);
        assert_eq!(sem.count(), 0);

        let eg = RtosEventGroup::new();
        assert_eq!(eg.bits(), 0);

        let sched = RtosScheduler::new();
        assert_eq!(sched.state(), SchedulerState::NotStarted);
    }

    #[test]
    fn rtos_module_re_exports_realtime_types() {
        let pt = PeriodicTask::new("test", 100, 2, 512, "fn_test");
        assert_eq!(pt.period_ticks(), 100);

        let constraint = RealtimeConstraint::new("fn", 1000, true);
        assert!(constraint.is_no_heap());

        let estimator = WcetEstimator::new(168_000_000, 1.5);
        assert_eq!(estimator.function_count(), 0);

        let idle = IdleHook::sleep_mode();
        let code = idle.generate_c_code();
        assert!(code.contains("IdleHook"));

        let tick = TickHook::watchdog();
        let code = tick.generate_c_code();
        assert!(code.contains("TickHook"));

        let tl = TicklessIdleConfig::default();
        assert!(!tl.enabled);
    }
}
