//! Real-time annotations and analysis for Fajar Lang.
//!
//! Provides compile-time and static analysis tools for real-time systems:
//!
//! - [`PeriodicTask`] — Periodic task with `vTaskDelayUntil` code generation
//! - [`RealtimeConstraint`] — Deadline and heap-allocation restriction analysis
//! - [`WcetEstimate`] — Worst-case execution time estimation from call graphs
//! - [`IdleHook`] / [`TickHook`] — Hook code generation
//! - Stack size estimation via call graph analysis
//! - Priority inversion detection
//! - Tickless idle support configuration

use std::collections::{HashMap, HashSet};
use thiserror::Error;

use super::freertos::TaskPriority;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from real-time analysis.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RealtimeError {
    /// Deadline violation detected statically.
    #[error("deadline violation: WCET {wcet_us}us exceeds deadline {deadline_us}us for task '{task_name}'")]
    DeadlineViolation {
        /// Task name.
        task_name: String,
        /// Estimated WCET in microseconds.
        wcet_us: u64,
        /// Required deadline in microseconds.
        deadline_us: u64,
    },

    /// Heap allocation detected in real-time context.
    #[error("heap allocation in real-time task '{task_name}': {alloc_site}")]
    HeapAllocInRealtime {
        /// Task name.
        task_name: String,
        /// Description of the allocation site.
        alloc_site: String,
    },

    /// Priority inversion detected.
    #[error("priority inversion: task '{high_task}' (prio {high_prio}) blocked on mutex held by '{low_task}' (prio {low_prio})")]
    PriorityInversion {
        /// High-priority task name.
        high_task: String,
        /// High-priority task priority.
        high_prio: u32,
        /// Low-priority task (mutex holder) name.
        low_task: String,
        /// Low-priority task priority.
        low_prio: u32,
    },

    /// Stack overflow risk detected.
    #[error("stack overflow risk in task '{task_name}': estimated {estimated_words} words, allocated {allocated_words} words")]
    StackOverflowRisk {
        /// Task name.
        task_name: String,
        /// Estimated stack usage in words.
        estimated_words: u32,
        /// Allocated stack size in words.
        allocated_words: u32,
    },

    /// Function not found in call graph.
    #[error("function '{name}' not found in call graph")]
    FunctionNotFound {
        /// Function name.
        name: String,
    },

    /// Recursive call detected (unbounded WCET).
    #[error("recursive call detected in '{name}': cannot estimate WCET")]
    RecursiveCall {
        /// Function name.
        name: String,
    },
}

/// Real-time analysis warning (non-fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeWarning {
    /// Warning message.
    pub message: String,
    /// Severity level (0 = info, 1 = warn, 2 = critical).
    pub severity: u8,
}

// ═══════════════════════════════════════════════════════════════════════
// PeriodicTask
// ═══════════════════════════════════════════════════════════════════════

/// A periodic real-time task with a fixed period.
///
/// Generates code using the `vTaskDelayUntil` pattern to ensure
/// deterministic periodicity regardless of execution time variation.
#[derive(Debug, Clone)]
pub struct PeriodicTask {
    /// Task name.
    name: String,
    /// Period in ticks.
    period_ticks: u32,
    /// Task priority.
    priority: TaskPriority,
    /// Stack size in words.
    stack_size: u32,
    /// Function name (entry point).
    fn_name: String,
    /// Optional deadline in microseconds (must be <= period).
    deadline_us: Option<u64>,
}

impl PeriodicTask {
    /// Creates a new periodic task definition.
    ///
    /// # Arguments
    /// * `name` - Task name
    /// * `period_ticks` - Period in RTOS ticks
    /// * `priority` - Task priority
    /// * `stack_size` - Stack size in words
    /// * `fn_name` - Entry function name
    pub fn new(
        name: &str,
        period_ticks: u32,
        priority: u32,
        stack_size: u32,
        fn_name: &str,
    ) -> Self {
        Self {
            name: name.to_string(),
            period_ticks,
            priority: TaskPriority(priority),
            stack_size,
            fn_name: fn_name.to_string(),
            deadline_us: None,
        }
    }

    /// Sets a deadline constraint for this task.
    pub fn with_deadline(mut self, deadline_us: u64) -> Self {
        self.deadline_us = Some(deadline_us);
        self
    }

    /// Returns the task name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the period in ticks.
    pub fn period_ticks(&self) -> u32 {
        self.period_ticks
    }

    /// Returns the priority.
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }

    /// Returns the stack size.
    pub fn stack_size(&self) -> u32 {
        self.stack_size
    }

    /// Returns the function name.
    pub fn fn_name(&self) -> &str {
        &self.fn_name
    }

    /// Returns the deadline in microseconds (if set).
    pub fn deadline_us(&self) -> Option<u64> {
        self.deadline_us
    }

    /// Generates C code for the vTaskDelayUntil pattern.
    ///
    /// This produces a task wrapper that ensures the task runs at
    /// a fixed period using `vTaskDelayUntil`.
    pub fn generate_c_wrapper(&self) -> String {
        let mut code = String::new();
        code.push_str(&format!(
            "/* Auto-generated periodic task wrapper for '{}' */\n",
            self.name
        ));
        code.push_str(&format!(
            "void {}_wrapper(void *pvParameters) {{\n",
            self.name
        ));
        code.push_str("    TickType_t xLastWakeTime;\n");
        code.push_str(&format!(
            "    const TickType_t xFrequency = {};\n\n",
            self.period_ticks
        ));
        code.push_str("    /* Initialize the xLastWakeTime variable with the current time. */\n");
        code.push_str("    xLastWakeTime = xTaskGetTickCount();\n\n");
        code.push_str("    for (;;) {\n");
        code.push_str(&format!("        {}();\n", self.fn_name));
        code.push_str("        vTaskDelayUntil(&xLastWakeTime, xFrequency);\n");
        code.push_str("    }\n");
        code.push_str("}\n");
        code
    }

    /// Generates a Fajar Lang snippet for the periodic task.
    pub fn generate_fj_snippet(&self) -> String {
        let mut code = String::new();
        code.push_str(&format!(
            "// Periodic task: {} (period: {} ticks)\n",
            self.name, self.period_ticks
        ));
        code.push_str(&format!("@kernel fn {}_task() {{\n", self.name));
        code.push_str("    let mut last_wake: u32 = rtos_get_tick_count()\n");
        code.push_str("    loop {\n");
        code.push_str(&format!("        {}()\n", self.fn_name));
        code.push_str(&format!(
            "        rtos_delay_until(&mut last_wake, {})\n",
            self.period_ticks
        ));
        code.push_str("    }\n");
        code.push_str("}\n");
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RealtimeConstraint
// ═══════════════════════════════════════════════════════════════════════

/// A real-time constraint applied to a function or task.
///
/// Specifies the deadline and tracks detected constraint violations
/// such as heap allocations in the real-time path.
#[derive(Debug, Clone)]
pub struct RealtimeConstraint {
    /// Function/task name.
    name: String,
    /// Deadline in microseconds.
    deadline_us: u64,
    /// Whether heap allocation is forbidden.
    no_heap: bool,
    /// Detected heap allocation sites.
    heap_alloc_sites: Vec<String>,
    /// Estimated WCET in microseconds (if computed).
    estimated_wcet_us: Option<u64>,
}

impl RealtimeConstraint {
    /// Creates a new real-time constraint.
    ///
    /// # Arguments
    /// * `name` - Function/task name
    /// * `deadline_us` - Maximum allowed execution time in microseconds
    /// * `no_heap` - Whether heap allocation is forbidden
    pub fn new(name: &str, deadline_us: u64, no_heap: bool) -> Self {
        Self {
            name: name.to_string(),
            deadline_us,
            no_heap,
            heap_alloc_sites: Vec::new(),
            estimated_wcet_us: None,
        }
    }

    /// Returns the constraint name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the deadline in microseconds.
    pub fn deadline_us(&self) -> u64 {
        self.deadline_us
    }

    /// Returns whether heap allocation is forbidden.
    pub fn is_no_heap(&self) -> bool {
        self.no_heap
    }

    /// Returns detected heap allocation sites.
    pub fn heap_alloc_sites(&self) -> &[String] {
        &self.heap_alloc_sites
    }

    /// Returns the estimated WCET.
    pub fn estimated_wcet_us(&self) -> Option<u64> {
        self.estimated_wcet_us
    }

    /// Records a detected heap allocation site.
    pub fn add_heap_alloc_site(&mut self, site: &str) {
        self.heap_alloc_sites.push(site.to_string());
    }

    /// Sets the estimated WCET.
    pub fn set_wcet(&mut self, wcet_us: u64) {
        self.estimated_wcet_us = Some(wcet_us);
    }

    /// Validates the constraint and returns any violations.
    pub fn validate(&self) -> Vec<RealtimeError> {
        let mut errors = Vec::new();

        // Check heap allocation violations
        if self.no_heap {
            for site in &self.heap_alloc_sites {
                errors.push(RealtimeError::HeapAllocInRealtime {
                    task_name: self.name.clone(),
                    alloc_site: site.clone(),
                });
            }
        }

        // Check deadline violations
        if let Some(wcet) = self.estimated_wcet_us {
            if wcet > self.deadline_us {
                errors.push(RealtimeError::DeadlineViolation {
                    task_name: self.name.clone(),
                    wcet_us: wcet,
                    deadline_us: self.deadline_us,
                });
            }
        }

        errors
    }
}

// ═══════════════════════════════════════════════════════════════════════
// WcetEstimate — Call graph analysis
// ═══════════════════════════════════════════════════════════════════════

/// A function node in the call graph for WCET estimation.
#[derive(Debug, Clone)]
pub struct FunctionNode {
    /// Function name.
    pub name: String,
    /// Instruction count estimate for the function body (excluding callees).
    pub instruction_count: u32,
    /// Names of functions called by this function.
    pub callees: Vec<String>,
    /// Stack frame size in words.
    pub stack_frame_words: u32,
}

/// Worst-case execution time estimator.
///
/// Traverses a call graph to estimate the total instruction count
/// for a given entry function. Detects recursive calls and provides
/// stack depth estimation.
#[derive(Debug)]
pub struct WcetEstimator {
    /// Function nodes indexed by name.
    functions: HashMap<String, FunctionNode>,
    /// CPU clock frequency for time conversion.
    cpu_freq_hz: u32,
    /// Average cycles per instruction (CPI).
    cpi: f64,
}

impl WcetEstimator {
    /// Creates a new WCET estimator.
    ///
    /// # Arguments
    /// * `cpu_freq_hz` - CPU clock frequency in Hz
    /// * `cpi` - Average cycles per instruction
    pub fn new(cpu_freq_hz: u32, cpi: f64) -> Self {
        Self {
            functions: HashMap::new(),
            cpu_freq_hz,
            cpi,
        }
    }

    /// Returns the CPU frequency.
    pub fn cpu_freq_hz(&self) -> u32 {
        self.cpu_freq_hz
    }

    /// Returns the CPI assumption.
    pub fn cpi(&self) -> f64 {
        self.cpi
    }

    /// Returns the number of functions in the call graph.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Adds a function node to the call graph.
    pub fn add_function(&mut self, node: FunctionNode) {
        self.functions.insert(node.name.clone(), node);
    }

    /// Estimates the total instruction count for a function (including callees).
    ///
    /// Returns `Err(RecursiveCall)` if a cycle is detected.
    pub fn estimate_instructions(&self, fn_name: &str) -> Result<u32, RealtimeError> {
        let mut visited = HashSet::new();
        self.estimate_instructions_inner(fn_name, &mut visited)
    }

    fn estimate_instructions_inner(
        &self,
        fn_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<u32, RealtimeError> {
        if visited.contains(fn_name) {
            return Err(RealtimeError::RecursiveCall {
                name: fn_name.to_string(),
            });
        }

        let node = self
            .functions
            .get(fn_name)
            .ok_or_else(|| RealtimeError::FunctionNotFound {
                name: fn_name.to_string(),
            })?;

        visited.insert(fn_name.to_string());

        let mut total = node.instruction_count;
        for callee in &node.callees {
            total += self.estimate_instructions_inner(callee, visited)?;
        }

        visited.remove(fn_name);
        Ok(total)
    }

    /// Estimates WCET in microseconds for a function.
    pub fn estimate_wcet_us(&self, fn_name: &str) -> Result<u64, RealtimeError> {
        let instructions = self.estimate_instructions(fn_name)?;
        let cycles = (instructions as f64) * self.cpi;
        let seconds = cycles / self.cpu_freq_hz as f64;
        let microseconds = (seconds * 1_000_000.0) as u64;
        Ok(microseconds)
    }

    /// Estimates stack depth (in words) for a function call chain.
    pub fn estimate_stack_depth(&self, fn_name: &str) -> Result<u32, RealtimeError> {
        let mut visited = HashSet::new();
        self.estimate_stack_inner(fn_name, &mut visited)
    }

    fn estimate_stack_inner(
        &self,
        fn_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<u32, RealtimeError> {
        if visited.contains(fn_name) {
            return Err(RealtimeError::RecursiveCall {
                name: fn_name.to_string(),
            });
        }

        let node = self
            .functions
            .get(fn_name)
            .ok_or_else(|| RealtimeError::FunctionNotFound {
                name: fn_name.to_string(),
            })?;

        visited.insert(fn_name.to_string());

        let mut max_callee_depth: u32 = 0;
        for callee in &node.callees {
            let depth = self.estimate_stack_inner(callee, visited)?;
            if depth > max_callee_depth {
                max_callee_depth = depth;
            }
        }

        visited.remove(fn_name);
        Ok(node.stack_frame_words + max_callee_depth)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Priority inversion detection
// ═══════════════════════════════════════════════════════════════════════

/// A mutex usage record for priority inversion detection.
#[derive(Debug, Clone)]
pub struct MutexUsage {
    /// Mutex name/ID.
    pub mutex_name: String,
    /// Task name that uses this mutex.
    pub task_name: String,
    /// Task priority.
    pub task_priority: u32,
}

/// Detects potential priority inversion scenarios.
///
/// Analyzes mutex usage across tasks to find cases where a high-priority
/// task might block on a mutex held by a lower-priority task without
/// priority inheritance protection.
pub fn detect_priority_inversions(usages: &[MutexUsage]) -> Vec<RealtimeWarning> {
    let mut warnings = Vec::new();

    // Group usages by mutex
    let mut by_mutex: HashMap<&str, Vec<&MutexUsage>> = HashMap::new();
    for usage in usages {
        by_mutex.entry(&usage.mutex_name).or_default().push(usage);
    }

    // For each mutex, check if tasks with different priorities share it
    for (mutex_name, tasks) in &by_mutex {
        if tasks.len() < 2 {
            continue;
        }

        let min_prio = tasks.iter().map(|t| t.task_priority).min().unwrap_or(0);
        let max_prio = tasks.iter().map(|t| t.task_priority).max().unwrap_or(0);

        if max_prio > min_prio {
            let high_task = tasks
                .iter()
                .find(|t| t.task_priority == max_prio)
                .map(|t| t.task_name.as_str())
                .unwrap_or("unknown");
            let low_task = tasks
                .iter()
                .find(|t| t.task_priority == min_prio)
                .map(|t| t.task_name.as_str())
                .unwrap_or("unknown");

            warnings.push(RealtimeWarning {
                message: format!(
                    "potential priority inversion on mutex '{}': '{}' (prio {}) may block on '{}' (prio {})",
                    mutex_name, high_task, max_prio, low_task, min_prio
                ),
                severity: if max_prio - min_prio > 2 { 2 } else { 1 },
            });
        }
    }

    warnings
}

// ═══════════════════════════════════════════════════════════════════════
// Stack size estimation
// ═══════════════════════════════════════════════════════════════════════

/// Validates that a task's stack size is sufficient for its call chain.
pub fn validate_stack_size(
    estimator: &WcetEstimator,
    task_name: &str,
    fn_name: &str,
    allocated_stack_words: u32,
) -> Result<(), RealtimeError> {
    let estimated = estimator.estimate_stack_depth(fn_name)?;
    // Add safety margin (20%)
    let with_margin = estimated + (estimated / 5);
    if with_margin > allocated_stack_words {
        return Err(RealtimeError::StackOverflowRisk {
            task_name: task_name.to_string(),
            estimated_words: with_margin,
            allocated_words: allocated_stack_words,
        });
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Hook code generation
// ═══════════════════════════════════════════════════════════════════════

/// Idle hook configuration.
///
/// Code that runs whenever the idle task executes (no other tasks ready).
/// Commonly used for entering low-power sleep modes.
#[derive(Debug, Clone)]
pub struct IdleHook {
    /// Whether to enter low-power mode.
    pub enter_sleep: bool,
    /// Custom function name to call (if any).
    pub custom_fn: Option<String>,
}

impl IdleHook {
    /// Creates a default idle hook (sleep mode).
    pub fn sleep_mode() -> Self {
        Self {
            enter_sleep: true,
            custom_fn: None,
        }
    }

    /// Creates an idle hook with a custom function.
    pub fn custom(fn_name: &str) -> Self {
        Self {
            enter_sleep: false,
            custom_fn: Some(fn_name.to_string()),
        }
    }

    /// Generates C code for the idle hook.
    pub fn generate_c_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated idle hook */\n");
        code.push_str("void vApplicationIdleHook(void) {\n");
        if self.enter_sleep {
            code.push_str("    __WFI(); /* Wait For Interrupt — enter sleep */\n");
        }
        if let Some(ref fn_name) = self.custom_fn {
            code.push_str(&format!("    {}();\n", fn_name));
        }
        code.push_str("}\n");
        code
    }
}

/// Tick hook configuration.
///
/// Code that runs on every RTOS tick (typically every 1ms).
/// Used for periodic housekeeping or watchdog kicking.
#[derive(Debug, Clone)]
pub struct TickHook {
    /// Custom function name to call on each tick.
    pub custom_fn: Option<String>,
    /// Whether to kick the watchdog.
    pub kick_watchdog: bool,
}

impl TickHook {
    /// Creates a tick hook that kicks the watchdog.
    pub fn watchdog() -> Self {
        Self {
            custom_fn: None,
            kick_watchdog: true,
        }
    }

    /// Creates a tick hook with a custom function.
    pub fn custom(fn_name: &str) -> Self {
        Self {
            custom_fn: Some(fn_name.to_string()),
            kick_watchdog: false,
        }
    }

    /// Generates C code for the tick hook.
    pub fn generate_c_code(&self) -> String {
        let mut code = String::new();
        code.push_str("/* Auto-generated tick hook */\n");
        code.push_str("void vApplicationTickHook(void) {\n");
        if self.kick_watchdog {
            code.push_str("    HAL_IWDG_Refresh(&hiwdg); /* Kick watchdog */\n");
        }
        if let Some(ref fn_name) = self.custom_fn {
            code.push_str(&format!("    {}();\n", fn_name));
        }
        code.push_str("}\n");
        code
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tickless idle configuration
// ═══════════════════════════════════════════════════════════════════════

/// Tickless idle mode configuration.
///
/// When no tasks are ready to run, the RTOS can suppress the periodic
/// tick interrupt and enter deep sleep, waking only when needed.
#[derive(Debug, Clone)]
pub struct TicklessIdleConfig {
    /// Whether tickless idle is enabled.
    pub enabled: bool,
    /// Minimum idle ticks before entering tickless mode.
    pub min_idle_ticks: u32,
    /// Expected idle time for power estimation (ticks).
    pub expected_idle_time: u32,
}

impl TicklessIdleConfig {
    /// Creates a new tickless idle configuration.
    pub fn new(enabled: bool, min_idle_ticks: u32) -> Self {
        Self {
            enabled,
            min_idle_ticks,
            expected_idle_time: 0,
        }
    }

    /// Generates the FreeRTOS configuration defines for tickless idle.
    pub fn generate_config_defines(&self) -> String {
        let mut config = String::new();
        config.push_str("/* Tickless idle configuration */\n");
        config.push_str(&format!(
            "#define configUSE_TICKLESS_IDLE       {}\n",
            if self.enabled { 1 } else { 0 }
        ));
        if self.enabled {
            config.push_str(&format!(
                "#define configEXPECTED_IDLE_TIME_BEFORE_SLEEP  {}\n",
                self.min_idle_ticks
            ));
        }
        config
    }
}

impl Default for TicklessIdleConfig {
    fn default() -> Self {
        Self::new(false, 2)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── PeriodicTask tests ──────────────────────────────────────────

    #[test]
    fn periodic_task_generates_c_wrapper() {
        let pt = PeriodicTask::new("sensor_read", 100, 3, 512, "read_sensors");
        let code = pt.generate_c_wrapper();
        assert!(code.contains("sensor_read_wrapper"));
        assert!(code.contains("vTaskDelayUntil"));
        assert!(code.contains("xFrequency = 100"));
        assert!(code.contains("read_sensors()"));
    }

    #[test]
    fn periodic_task_generates_fj_snippet() {
        let pt = PeriodicTask::new("motor_ctrl", 50, 5, 256, "control_loop");
        let code = pt.generate_fj_snippet();
        assert!(code.contains("motor_ctrl_task"));
        assert!(code.contains("control_loop()"));
        assert!(code.contains("rtos_delay_until"));
        assert!(code.contains("50"));
    }

    #[test]
    fn periodic_task_with_deadline() {
        let pt = PeriodicTask::new("critical", 10, 5, 512, "do_work").with_deadline(5000);
        assert_eq!(pt.deadline_us(), Some(5000));
        assert_eq!(pt.period_ticks(), 10);
        assert_eq!(pt.priority(), TaskPriority(5));
    }

    #[test]
    fn periodic_task_accessors() {
        let pt = PeriodicTask::new("my_task", 200, 4, 1024, "entry_fn");
        assert_eq!(pt.name(), "my_task");
        assert_eq!(pt.fn_name(), "entry_fn");
        assert_eq!(pt.stack_size(), 1024);
    }

    // ─── RealtimeConstraint tests ────────────────────────────────────

    #[test]
    fn constraint_validates_no_violations() {
        let mut constraint = RealtimeConstraint::new("fast_fn", 1000, true);
        constraint.set_wcet(500);
        let errors = constraint.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn constraint_detects_deadline_violation() {
        let mut constraint = RealtimeConstraint::new("slow_fn", 100, false);
        constraint.set_wcet(200);
        let errors = constraint.validate();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], RealtimeError::DeadlineViolation { .. }));
    }

    #[test]
    fn constraint_detects_heap_alloc() {
        let mut constraint = RealtimeConstraint::new("rt_fn", 1000, true);
        constraint.add_heap_alloc_site("Vec::push at line 42");
        constraint.add_heap_alloc_site("String::new at line 55");
        let errors = constraint.validate();
        assert_eq!(errors.len(), 2);
        for err in &errors {
            assert!(matches!(err, RealtimeError::HeapAllocInRealtime { .. }));
        }
    }

    #[test]
    fn constraint_allows_heap_when_not_restricted() {
        let mut constraint = RealtimeConstraint::new("normal_fn", 1000, false);
        constraint.add_heap_alloc_site("Vec::push at line 42");
        let errors = constraint.validate();
        assert!(errors.is_empty()); // no_heap is false
    }

    // ─── WcetEstimator tests ─────────────────────────────────────────

    fn make_estimator() -> WcetEstimator {
        let mut est = WcetEstimator::new(168_000_000, 1.5); // STM32F4 @ 168MHz

        est.add_function(FunctionNode {
            name: "main_loop".to_string(),
            instruction_count: 50,
            callees: vec!["read_sensor".to_string(), "compute".to_string()],
            stack_frame_words: 8,
        });
        est.add_function(FunctionNode {
            name: "read_sensor".to_string(),
            instruction_count: 30,
            callees: vec!["spi_transfer".to_string()],
            stack_frame_words: 4,
        });
        est.add_function(FunctionNode {
            name: "spi_transfer".to_string(),
            instruction_count: 20,
            callees: vec![],
            stack_frame_words: 2,
        });
        est.add_function(FunctionNode {
            name: "compute".to_string(),
            instruction_count: 100,
            callees: vec![],
            stack_frame_words: 16,
        });

        est
    }

    #[test]
    fn wcet_estimate_instructions() {
        let est = make_estimator();
        // main_loop (50) + read_sensor (30) + spi_transfer (20) + compute (100) = 200
        let total = est.estimate_instructions("main_loop").unwrap();
        assert_eq!(total, 200);
    }

    #[test]
    fn wcet_estimate_leaf_function() {
        let est = make_estimator();
        let total = est.estimate_instructions("spi_transfer").unwrap();
        assert_eq!(total, 20);
    }

    #[test]
    fn wcet_estimate_us() {
        let est = make_estimator();
        // 200 instructions * 1.5 CPI = 300 cycles
        // 300 / 168_000_000 = ~1.78us
        let wcet = est.estimate_wcet_us("main_loop").unwrap();
        assert!(wcet < 10); // Should be ~1-2 us
    }

    #[test]
    fn wcet_detects_recursive_call() {
        let mut est = WcetEstimator::new(168_000_000, 1.5);
        est.add_function(FunctionNode {
            name: "a".to_string(),
            instruction_count: 10,
            callees: vec!["b".to_string()],
            stack_frame_words: 4,
        });
        est.add_function(FunctionNode {
            name: "b".to_string(),
            instruction_count: 10,
            callees: vec!["a".to_string()],
            stack_frame_words: 4,
        });

        let err = est.estimate_instructions("a").unwrap_err();
        assert!(matches!(err, RealtimeError::RecursiveCall { .. }));
    }

    #[test]
    fn wcet_unknown_function_returns_error() {
        let est = make_estimator();
        let err = est.estimate_instructions("nonexistent").unwrap_err();
        assert!(matches!(err, RealtimeError::FunctionNotFound { .. }));
    }

    // ─── Stack estimation tests ──────────────────────────────────────

    #[test]
    fn stack_depth_estimation() {
        let est = make_estimator();
        // main_loop (8) -> max(read_sensor path, compute path)
        // read_sensor (4) -> spi_transfer (2) = 6
        // compute (16) = 16
        // Total: 8 + 16 = 24
        let depth = est.estimate_stack_depth("main_loop").unwrap();
        assert_eq!(depth, 24);
    }

    #[test]
    fn stack_validation_passes() {
        let est = make_estimator();
        // 24 words estimated + 20% margin = 28.8 -> 28
        // Allocate 32 words: should pass
        let result = validate_stack_size(&est, "task1", "main_loop", 32);
        assert!(result.is_ok());
    }

    #[test]
    fn stack_validation_fails_insufficient() {
        let est = make_estimator();
        // 24 words + 20% = 28 words needed, only 20 allocated
        let err = validate_stack_size(&est, "task1", "main_loop", 20).unwrap_err();
        assert!(matches!(err, RealtimeError::StackOverflowRisk { .. }));
    }

    // ─── Priority inversion detection tests ──────────────────────────

    #[test]
    fn detect_priority_inversion_warning() {
        let usages = vec![
            MutexUsage {
                mutex_name: "sensor_mutex".to_string(),
                task_name: "high_prio_ctrl".to_string(),
                task_priority: 5,
            },
            MutexUsage {
                mutex_name: "sensor_mutex".to_string(),
                task_name: "low_prio_log".to_string(),
                task_priority: 1,
            },
        ];
        let warnings = detect_priority_inversions(&usages);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("priority inversion"));
        assert_eq!(warnings[0].severity, 2); // diff > 2
    }

    #[test]
    fn no_inversion_when_same_priority() {
        let usages = vec![
            MutexUsage {
                mutex_name: "m".to_string(),
                task_name: "t1".to_string(),
                task_priority: 3,
            },
            MutexUsage {
                mutex_name: "m".to_string(),
                task_name: "t2".to_string(),
                task_priority: 3,
            },
        ];
        let warnings = detect_priority_inversions(&usages);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_inversion_with_single_user() {
        let usages = vec![MutexUsage {
            mutex_name: "m".to_string(),
            task_name: "t1".to_string(),
            task_priority: 5,
        }];
        let warnings = detect_priority_inversions(&usages);
        assert!(warnings.is_empty());
    }

    // ─── Hook tests ──────────────────────────────────────────────────

    #[test]
    fn idle_hook_sleep_mode_generates_wfi() {
        let hook = IdleHook::sleep_mode();
        let code = hook.generate_c_code();
        assert!(code.contains("vApplicationIdleHook"));
        assert!(code.contains("__WFI()"));
    }

    #[test]
    fn idle_hook_custom_fn() {
        let hook = IdleHook::custom("my_idle_handler");
        let code = hook.generate_c_code();
        assert!(code.contains("my_idle_handler()"));
        assert!(!code.contains("__WFI()"));
    }

    #[test]
    fn tick_hook_watchdog() {
        let hook = TickHook::watchdog();
        let code = hook.generate_c_code();
        assert!(code.contains("vApplicationTickHook"));
        assert!(code.contains("HAL_IWDG_Refresh"));
    }

    #[test]
    fn tick_hook_custom_fn() {
        let hook = TickHook::custom("my_tick_handler");
        let code = hook.generate_c_code();
        assert!(code.contains("my_tick_handler()"));
    }

    // ─── Tickless idle config tests ──────────────────────────────────

    #[test]
    fn tickless_idle_enabled_config() {
        let config = TicklessIdleConfig::new(true, 5);
        let defines = config.generate_config_defines();
        assert!(defines.contains("configUSE_TICKLESS_IDLE       1"));
        assert!(defines.contains("EXPECTED_IDLE_TIME_BEFORE_SLEEP  5"));
    }

    #[test]
    fn tickless_idle_disabled_config() {
        let config = TicklessIdleConfig::new(false, 2);
        let defines = config.generate_config_defines();
        assert!(defines.contains("configUSE_TICKLESS_IDLE       0"));
        assert!(!defines.contains("EXPECTED_IDLE_TIME"));
    }

    #[test]
    fn tickless_idle_default() {
        let config = TicklessIdleConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.min_idle_ticks, 2);
    }

    #[test]
    fn wcet_estimator_accessors() {
        let est = WcetEstimator::new(168_000_000, 1.5);
        assert_eq!(est.cpu_freq_hz(), 168_000_000);
        assert!((est.cpi() - 1.5).abs() < f64::EPSILON);
        assert_eq!(est.function_count(), 0);
    }
}
