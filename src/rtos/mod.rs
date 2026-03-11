//! RTOS integration module for Fajar Lang.
//!
//! Provides FreeRTOS and Zephyr RTOS FFI bindings, language-level
//! abstractions, Arduino compatibility, and real-time analysis tools
//! for embedded systems programming.
//!
//! # Module Structure
//!
//! ```text
//! rtos/
//! ├── freertos.rs              — FreeRTOS FFI bindings + simulation stubs
//! ├── abstractions.rs          — High-level FreeRTOS abstractions (task, queue, mutex, etc.)
//! ├── realtime.rs              — Real-time annotations and analysis
//! ├── zephyr.rs                — Zephyr RTOS FFI bindings + simulation stubs
//! ├── zephyr_abstractions.rs   — High-level Zephyr abstractions (task, msgq, mutex, etc.)
//! └── arduino_compat.rs        — Arduino Core API compatibility layer
//! ```
//!
//! # Feature Gates
//!
//! - Default: simulation mode (all APIs work without hardware linked)
//! - `freertos` feature: links against actual FreeRTOS C library
//! - `zephyr` feature: links against actual Zephyr kernel
//!
//! # Usage
//!
//! The RTOS module is designed for embedded real-time applications
//! using Fajar Lang's `@kernel` and `@safe` contexts.
//!
//! The Zephyr bindings target the STM32H5F5 (Arduino VENTUNO Q) MCU.

pub mod abstractions;
pub mod arduino_compat;
pub mod freertos;
pub mod realtime;
pub mod zephyr;
pub mod zephyr_abstractions;

// Re-exports for convenience
pub use abstractions::{
    RtosError, RtosEventGroup, RtosMutex, RtosQueue, RtosScheduler, RtosSemaphore, RtosTask,
    SchedulerState, SemaphoreKind, TaskAllocMode,
};
pub use arduino_compat::ArduinoCompat;
pub use freertos::{
    FreeRtosConfig, FreeRtosError, FreeRtosRuntime, MutexHandle, QueueHandle, SemaphoreHandle,
    TaskHandle, TaskPriority, TaskState, TimerHandle,
};
pub use realtime::{
    FunctionNode, IdleHook, MutexUsage, PeriodicTask, RealtimeConstraint, RealtimeError,
    RealtimeWarning, TickHook, TicklessIdleConfig, WcetEstimator,
};
pub use zephyr::{
    ZephyrError, ZephyrMsgqHandle, ZephyrMutexHandle, ZephyrSemHandle, ZephyrSim,
    ZephyrThreadHandle, ZephyrThreadState, ZephyrTimerHandle,
};
pub use zephyr_abstractions::{
    ZephyrMsgQ, ZephyrMutex, ZephyrMutexGuard, ZephyrSemaphore, ZephyrTask, ZephyrTimer,
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

    #[test]
    fn rtos_module_re_exports_zephyr_types() {
        let mut sim = ZephyrSim::new();
        let handle = sim.thread_create("test_thread", 5, 512, 0).unwrap();
        assert_eq!(sim.thread_state(handle), Some(ZephyrThreadState::Ready));
        assert_eq!(sim.thread_count(), 1);
    }

    #[test]
    fn rtos_module_re_exports_zephyr_abstraction_types() {
        let mut sim = ZephyrSim::new();
        let task = ZephyrTask::spawn("z_task", 3, 512, 0, &mut sim).unwrap();
        assert_eq!(task.name(), "z_task");
        assert!(task.is_running());

        let q = ZephyrMsgQ::new(4, &mut sim).unwrap();
        assert!(q.is_empty(&sim));

        let m = ZephyrMutex::new(&mut sim).unwrap();
        assert!(!m.is_locked());

        let s = ZephyrSemaphore::new_binary(&mut sim).unwrap();
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn rtos_module_re_exports_arduino_compat() {
        let mut compat = ArduinoCompat::new();
        compat.pin_mode(13, arduino_compat::OUTPUT);
        compat.digital_write(13, true);
        assert!(compat.digital_read(13));
    }
}
