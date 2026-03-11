//! RTIC (Real-Time Interrupt-driven Concurrency) compile-time scheduler for Fajar Lang.
//!
//! Implements a simulation of the RTIC framework's compile-time resource management
//! and priority-based task scheduling model. All analysis is performed statically —
//! no hardware dependencies.
//!
//! # Key Concepts
//!
//! - **Tasks** bind to hardware interrupts with fixed priorities
//! - **Resources** have ceiling priorities computed from accessor tasks
//! - **Critical sections** use BASEPRI-based locking (no OS overhead)
//! - **Deadlock freedom** is guaranteed by static analysis (SRP protocol)
//! - **Single stack per priority** — WCSS computed at compile time
//!
//! # Architecture
//!
//! ```text
//! RticApp
//!   ├── tasks: Vec<RticTask>        (interrupt-bound handlers)
//!   ├── resources: Vec<RticResource> (shared state with ceilings)
//!   └── device: String              (target board name)
//!         │
//!         ▼
//!   Compile-Time Analysis
//!   ├── compute_ceilings()          → resource ceiling priorities
//!   ├── CriticalSectionAnalysis     → lock elision when safe
//!   ├── StackAnalysis               → WCSS per priority level
//!   └── DeadlockFreedomProof        → SRP circular dependency check
//!         │
//!         ▼
//!   Code Generation (codegen.rs)
//!   ├── VectorTableEntry            → ISR vector mapping
//!   ├── BasepriLock                 → BASEPRI mutual exclusion
//!   ├── SpawnQueue                  → software task message passing
//!   ├── TimerQueue                  → scheduled delayed spawns
//!   └── RticCodegen                 → complete generated output
//! ```

pub mod codegen;

use std::collections::{HashMap, HashSet};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from RTIC compile-time analysis and code generation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RticError {
    /// Task references a resource that doesn't exist.
    #[error("RTIC001: task '{task}' references unknown resource '{resource}'")]
    UnknownResource {
        /// Task name.
        task: String,
        /// Resource name.
        resource: String,
    },

    /// Duplicate task name.
    #[error("RTIC002: duplicate task name '{name}'")]
    DuplicateTask {
        /// Task name.
        name: String,
    },

    /// Duplicate resource name.
    #[error("RTIC003: duplicate resource name '{name}'")]
    DuplicateResource {
        /// Resource name.
        name: String,
    },

    /// Priority out of valid range (1-255 for Cortex-M).
    #[error("RTIC004: priority {priority} out of range for task '{task}' (valid: 1-255)")]
    InvalidPriority {
        /// Task name.
        task: String,
        /// Invalid priority value.
        priority: u8,
    },

    /// Circular resource dependency detected (impossible under SRP, but checked).
    #[error("RTIC005: circular resource dependency detected: {cycle}")]
    CircularDependency {
        /// Description of the cycle.
        cycle: String,
    },

    /// Init function missing.
    #[error("RTIC006: @init function is required but not defined")]
    MissingInit,

    /// Interrupt binding conflict — two tasks bound to the same interrupt.
    #[error("RTIC007: interrupt '{interrupt}' bound to both '{task1}' and '{task2}'")]
    InterruptConflict {
        /// Interrupt name.
        interrupt: String,
        /// First task.
        task1: String,
        /// Second task.
        task2: String,
    },

    /// Software task spawn queue capacity exceeded.
    #[error("RTIC008: spawn queue capacity {capacity} exceeded for task '{task}'")]
    SpawnQueueFull {
        /// Task name.
        task: String,
        /// Queue capacity.
        capacity: usize,
    },

    /// Timer queue error.
    #[error("RTIC009: timer queue error: {detail}")]
    TimerQueueError {
        /// Error detail.
        detail: String,
    },

    /// Stack analysis error.
    #[error("RTIC010: stack analysis error: {detail}")]
    StackAnalysisError {
        /// Error detail.
        detail: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Sprint 19: RTIC Compile-Time Scheduler
// ═══════════════════════════════════════════════════════════════════════

/// An RTIC application definition.
///
/// Contains all tasks, shared resources, and device configuration needed
/// for compile-time scheduling analysis and code generation.
#[derive(Debug, Clone)]
pub struct RticApp {
    /// Hardware tasks (interrupt-bound).
    pub tasks: Vec<RticTask>,
    /// Shared resources with computed ceiling priorities.
    pub resources: Vec<RticResource>,
    /// Target device/board name (e.g., "stm32f407").
    pub device: String,
    /// Optional init configuration.
    pub init: Option<InitConfig>,
    /// Optional idle configuration.
    pub idle: Option<IdleConfig>,
}

impl RticApp {
    /// Creates a new RTIC application for a target device.
    pub fn new(device: &str) -> Self {
        Self {
            tasks: Vec::new(),
            resources: Vec::new(),
            device: device.to_string(),
            init: None,
            idle: None,
        }
    }

    /// Adds a task to the application.
    pub fn add_task(&mut self, task: RticTask) {
        self.tasks.push(task);
    }

    /// Adds a resource to the application.
    pub fn add_resource(&mut self, resource: RticResource) {
        self.resources.push(resource);
    }

    /// Sets the init configuration.
    pub fn set_init(&mut self, config: InitConfig) {
        self.init = Some(config);
    }

    /// Sets the idle configuration.
    pub fn set_idle(&mut self, config: IdleConfig) {
        self.idle = Some(config);
    }

    /// Returns the number of tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns the number of resources.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Finds a task by name.
    pub fn find_task(&self, name: &str) -> Option<&RticTask> {
        self.tasks.iter().find(|t| t.name == name)
    }

    /// Finds a resource by name.
    pub fn find_resource(&self, name: &str) -> Option<&RticResource> {
        self.resources.iter().find(|r| r.name == name)
    }
}

/// An RTIC hardware task bound to an interrupt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RticTask {
    /// Task name (unique within the app).
    pub name: String,
    /// Static priority (1 = lowest hardware priority, 255 = highest).
    pub priority: u8,
    /// Interrupt name this task is bound to (e.g., "TIM2", "EXTI0").
    pub binds: String,
    /// Handler function name in Fajar Lang source.
    pub handler: String,
    /// Names of shared resources this task accesses.
    pub shared_resources: Vec<String>,
    /// Estimated stack frame size in bytes.
    pub stack_frame_bytes: usize,
}

impl RticTask {
    /// Creates a new RTIC task.
    pub fn new(name: &str, priority: u8, binds: &str, handler: &str) -> Self {
        Self {
            name: name.to_string(),
            priority,
            binds: binds.to_string(),
            handler: handler.to_string(),
            shared_resources: Vec::new(),
            stack_frame_bytes: 128, // default estimate
        }
    }

    /// Adds a shared resource reference to this task.
    pub fn with_resource(mut self, resource: &str) -> Self {
        self.shared_resources.push(resource.to_string());
        self
    }

    /// Sets the stack frame size estimate.
    pub fn with_stack_frame(mut self, bytes: usize) -> Self {
        self.stack_frame_bytes = bytes;
        self
    }
}

/// An RTIC shared resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RticResource {
    /// Resource name (unique within the app).
    pub name: String,
    /// Type name of the resource (e.g., "u32", "SensorData").
    pub type_name: String,
    /// Ceiling priority — computed as max priority of all accessor tasks.
    /// Set to 0 initially; computed by `compute_ceilings()`.
    pub ceiling_priority: u8,
}

impl RticResource {
    /// Creates a new resource with ceiling = 0 (uncomputed).
    pub fn new(name: &str, type_name: &str) -> Self {
        Self {
            name: name.to_string(),
            type_name: type_name.to_string(),
            ceiling_priority: 0,
        }
    }
}

/// Compute ceiling priorities for all resources using the Stack Resource Policy.
///
/// For each resource, the ceiling = maximum priority among all tasks that access it.
/// Returns a map from resource name to ceiling priority.
pub fn compute_ceilings(app: &RticApp) -> HashMap<String, u8> {
    let mut ceilings: HashMap<String, u8> = HashMap::new();

    // Initialize all known resources
    for r in &app.resources {
        ceilings.insert(r.name.clone(), 0);
    }

    // For each task, raise the ceiling of every resource it accesses
    for task in &app.tasks {
        for res_name in &task.shared_resources {
            let ceiling = ceilings.entry(res_name.clone()).or_insert(0);
            if task.priority > *ceiling {
                *ceiling = task.priority;
            }
        }
    }

    ceilings
}

/// Configuration for the `@init` function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitConfig {
    /// Whether peripheral access is granted.
    pub peripherals: bool,
    /// List of monotonic timer names (e.g., ["tim2_mono"]).
    pub monotonics: Vec<String>,
    /// Init function name.
    pub handler: String,
}

impl InitConfig {
    /// Creates a default init configuration.
    pub fn new(handler: &str) -> Self {
        Self {
            peripherals: true,
            monotonics: Vec::new(),
            handler: handler.to_string(),
        }
    }

    /// Adds a monotonic timer to the init config.
    pub fn with_monotonic(mut self, name: &str) -> Self {
        self.monotonics.push(name.to_string());
        self
    }
}

/// Configuration for the `@idle` function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleConfig {
    /// Idle runs at the lowest priority (always 0).
    pub priority: u8,
    /// Whether to issue WFI (Wait For Interrupt) in the idle loop.
    pub wfi_enabled: bool,
    /// Idle handler function name.
    pub handler: String,
}

impl IdleConfig {
    /// Creates a default idle configuration with WFI enabled.
    pub fn new(handler: &str) -> Self {
        Self {
            priority: 0,
            wfi_enabled: true,
            handler: handler.to_string(),
        }
    }

    /// Disables WFI (busy-wait idle instead).
    pub fn without_wfi(mut self) -> Self {
        self.wfi_enabled = false;
        self
    }
}

/// Per-task context — provides a view of only the resources accessible by a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContext {
    /// Task name this context belongs to.
    pub task_name: String,
    /// Task priority.
    pub task_priority: u8,
    /// Resources accessible by this task, with their ceiling priorities.
    pub accessible_resources: HashMap<String, u8>,
}

impl TaskContext {
    /// Build a task context from the app's ceiling data.
    pub fn build(task: &RticTask, ceilings: &HashMap<String, u8>) -> Self {
        let mut accessible = HashMap::new();
        for res_name in &task.shared_resources {
            let ceiling = ceilings.get(res_name).copied().unwrap_or(0);
            accessible.insert(res_name.clone(), ceiling);
        }
        Self {
            task_name: task.name.clone(),
            task_priority: task.priority,
            accessible_resources: accessible,
        }
    }

    /// Returns true if a lock is needed to access the given resource.
    ///
    /// If `task_priority >= resource_ceiling`, no lock is needed
    /// (the task is already at or above the ceiling priority).
    pub fn needs_lock(&self, resource: &str) -> bool {
        match self.accessible_resources.get(resource) {
            Some(&ceiling) => self.task_priority < ceiling,
            None => false,
        }
    }
}

/// Critical section analysis — determines which resource accesses need locks.
#[derive(Debug, Clone)]
pub struct CriticalSectionAnalysis {
    /// Per-task lock requirements: task_name -> [(resource_name, needs_lock)].
    pub lock_requirements: HashMap<String, Vec<(String, bool)>>,
    /// Number of lock-free accesses (optimization wins).
    pub lock_free_count: usize,
    /// Number of accesses that require locks.
    pub locked_count: usize,
}

impl CriticalSectionAnalysis {
    /// Analyze all tasks and determine lock requirements.
    pub fn analyze(app: &RticApp, ceilings: &HashMap<String, u8>) -> Self {
        let mut lock_requirements = HashMap::new();
        let mut lock_free_count = 0usize;
        let mut locked_count = 0usize;

        for task in &app.tasks {
            let ctx = TaskContext::build(task, ceilings);
            let mut reqs = Vec::new();
            for res_name in &task.shared_resources {
                let needs = ctx.needs_lock(res_name);
                reqs.push((res_name.clone(), needs));
                if needs {
                    locked_count += 1;
                } else {
                    lock_free_count += 1;
                }
            }
            lock_requirements.insert(task.name.clone(), reqs);
        }

        Self {
            lock_requirements,
            lock_free_count,
            locked_count,
        }
    }
}

/// Stack analysis for RTIC — computes worst-case stack size per priority level.
///
/// In RTIC, all tasks at the same priority share a single stack frame.
/// The worst-case stacked stack size (WCSS) at each priority level is the
/// maximum frame size of any task at that level.
#[derive(Debug, Clone)]
pub struct StackAnalysis {
    /// Per-priority-level maximum frame size.
    pub priority_stacks: HashMap<u8, usize>,
    /// Total worst-case stacked stack size (sum of all levels).
    pub total_wcss: usize,
}

impl StackAnalysis {
    /// Compute stack analysis for an RTIC application.
    pub fn analyze(app: &RticApp) -> Self {
        let mut priority_stacks: HashMap<u8, usize> = HashMap::new();

        for task in &app.tasks {
            let entry = priority_stacks.entry(task.priority).or_insert(0);
            if task.stack_frame_bytes > *entry {
                *entry = task.stack_frame_bytes;
            }
        }

        let total_wcss = priority_stacks.values().sum();

        Self {
            priority_stacks,
            total_wcss,
        }
    }

    /// Returns the WCSS for a specific priority level.
    pub fn stack_at_priority(&self, priority: u8) -> usize {
        self.priority_stacks.get(&priority).copied().unwrap_or(0)
    }
}

/// Validate the RTIC application for common errors.
///
/// Checks for:
/// - Duplicate task names
/// - Duplicate resource names
/// - Unknown resource references
/// - Priority = 0 (reserved for idle)
/// - Interrupt binding conflicts
pub fn validate_app(app: &RticApp) -> Result<(), RticError> {
    check_duplicate_tasks(app)?;
    check_duplicate_resources(app)?;
    check_unknown_resources(app)?;
    check_priorities(app)?;
    check_interrupt_conflicts(app)?;
    Ok(())
}

/// Check for duplicate task names.
fn check_duplicate_tasks(app: &RticApp) -> Result<(), RticError> {
    let mut seen = HashSet::new();
    for task in &app.tasks {
        if !seen.insert(&task.name) {
            return Err(RticError::DuplicateTask {
                name: task.name.clone(),
            });
        }
    }
    Ok(())
}

/// Check for duplicate resource names.
fn check_duplicate_resources(app: &RticApp) -> Result<(), RticError> {
    let mut seen = HashSet::new();
    for res in &app.resources {
        if !seen.insert(&res.name) {
            return Err(RticError::DuplicateResource {
                name: res.name.clone(),
            });
        }
    }
    Ok(())
}

/// Check that all task resource references exist.
fn check_unknown_resources(app: &RticApp) -> Result<(), RticError> {
    let known: HashSet<&str> = app.resources.iter().map(|r| r.name.as_str()).collect();
    for task in &app.tasks {
        for res_name in &task.shared_resources {
            if !known.contains(res_name.as_str()) {
                return Err(RticError::UnknownResource {
                    task: task.name.clone(),
                    resource: res_name.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Check that no task has priority 0 (reserved for idle).
fn check_priorities(app: &RticApp) -> Result<(), RticError> {
    for task in &app.tasks {
        if task.priority == 0 {
            return Err(RticError::InvalidPriority {
                task: task.name.clone(),
                priority: 0,
            });
        }
    }
    Ok(())
}

/// Check for interrupt binding conflicts (two tasks on same interrupt).
fn check_interrupt_conflicts(app: &RticApp) -> Result<(), RticError> {
    let mut bindings: HashMap<&str, &str> = HashMap::new();
    for task in &app.tasks {
        if let Some(&existing_task) = bindings.get(task.binds.as_str()) {
            return Err(RticError::InterruptConflict {
                interrupt: task.binds.clone(),
                task1: existing_task.to_string(),
                task2: task.name.clone(),
            });
        }
        bindings.insert(&task.binds, &task.name);
    }
    Ok(())
}

/// Deadlock freedom proof via static analysis.
///
/// Under the Stack Resource Policy (SRP), deadlock is impossible if
/// there are no circular resource dependencies. This function verifies
/// that the resource access graph is acyclic.
pub struct DeadlockFreedomProof;

impl DeadlockFreedomProof {
    /// Check deadlock freedom for an RTIC application.
    ///
    /// Builds a resource dependency graph: if task A holds resource R1
    /// and also accesses R2, there is an edge R1 -> R2. A cycle in this
    /// graph would indicate a potential deadlock.
    ///
    /// Under SRP with BASEPRI locking, cycles are impossible by construction,
    /// but we verify anyway for defense in depth.
    pub fn check(app: &RticApp) -> Result<(), RticError> {
        let graph = build_dependency_graph(app);
        detect_cycle_in_graph(&graph)
    }
}

/// Build a resource dependency graph from task resource access patterns.
///
/// For each task, all pairs of accessed resources form edges:
/// if a task accesses {R1, R2, R3}, edges are R1->R2, R1->R3, R2->R3.
fn build_dependency_graph(app: &RticApp) -> HashMap<String, HashSet<String>> {
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();

    // Initialize nodes for all resources
    for r in &app.resources {
        graph.entry(r.name.clone()).or_default();
    }

    for task in &app.tasks {
        let resources = &task.shared_resources;
        for i in 0..resources.len() {
            for j in (i + 1)..resources.len() {
                let set = graph.entry(resources[i].clone()).or_default();
                set.insert(resources[j].clone());
            }
        }
    }

    graph
}

/// Detect a cycle in a directed graph using DFS.
fn detect_cycle_in_graph(graph: &HashMap<String, HashSet<String>>) -> Result<(), RticError> {
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    for node in graph.keys() {
        if !visited.contains(node) {
            if let Some(cycle) = dfs_cycle(node, graph, &mut visited, &mut in_stack) {
                return Err(RticError::CircularDependency { cycle });
            }
        }
    }

    Ok(())
}

/// DFS helper for cycle detection. Returns cycle description if found.
fn dfs_cycle(
    node: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
) -> Option<String> {
    visited.insert(node.to_string());
    in_stack.insert(node.to_string());

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor.as_str()) {
                if let Some(cycle) = dfs_cycle(neighbor, graph, visited, in_stack) {
                    return Some(cycle);
                }
            } else if in_stack.contains(neighbor.as_str()) {
                return Some(format!("{node} -> {neighbor}"));
            }
        }
    }

    in_stack.remove(node);
    None
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_app() -> RticApp {
        let mut app = RticApp::new("stm32f407");

        app.add_resource(RticResource::new("sensor_data", "f32"));
        app.add_resource(RticResource::new("led_state", "bool"));
        app.add_resource(RticResource::new("counter", "u32"));

        app.add_task(
            RticTask::new("sensor_read", 3, "TIM2", "handle_sensor")
                .with_resource("sensor_data")
                .with_resource("counter"),
        );
        app.add_task(
            RticTask::new("led_toggle", 1, "TIM3", "handle_led")
                .with_resource("led_state")
                .with_resource("counter"),
        );
        app.add_task(
            RticTask::new("control_loop", 5, "TIM4", "handle_control")
                .with_resource("sensor_data")
                .with_resource("led_state"),
        );

        app.set_init(InitConfig::new("init_peripherals"));
        app.set_idle(IdleConfig::new("idle_loop"));

        app
    }

    #[test]
    fn s19_1_rtic_app_construction() {
        let app = sample_app();
        assert_eq!(app.device, "stm32f407");
        assert_eq!(app.task_count(), 3);
        assert_eq!(app.resource_count(), 3);
        assert!(app.init.is_some());
        assert!(app.idle.is_some());
    }

    #[test]
    fn s19_2_rtic_task_shared_resources() {
        let app = sample_app();
        let sensor = app.find_task("sensor_read").expect("task exists");
        assert_eq!(sensor.priority, 3);
        assert_eq!(sensor.binds, "TIM2");
        assert_eq!(sensor.shared_resources, vec!["sensor_data", "counter"]);
    }

    #[test]
    fn s19_3_compute_ceilings() {
        let app = sample_app();
        let ceilings = compute_ceilings(&app);

        // sensor_data accessed by sensor_read(3) and control_loop(5) => ceiling = 5
        assert_eq!(ceilings["sensor_data"], 5);
        // led_state accessed by led_toggle(1) and control_loop(5) => ceiling = 5
        assert_eq!(ceilings["led_state"], 5);
        // counter accessed by sensor_read(3) and led_toggle(1) => ceiling = 3
        assert_eq!(ceilings["counter"], 3);
    }

    #[test]
    fn s19_4_init_and_idle_config() {
        let init = InitConfig::new("init_fn")
            .with_monotonic("tim2_mono")
            .with_monotonic("systick_mono");
        assert!(init.peripherals);
        assert_eq!(init.monotonics.len(), 2);
        assert_eq!(init.handler, "init_fn");

        let idle = IdleConfig::new("my_idle").without_wfi();
        assert_eq!(idle.priority, 0);
        assert!(!idle.wfi_enabled);
    }

    #[test]
    fn s19_5_task_context_needs_lock() {
        let app = sample_app();
        let ceilings = compute_ceilings(&app);

        // sensor_read has priority 3, sensor_data ceiling is 5 => needs lock
        let sensor_task = app.find_task("sensor_read").expect("exists");
        let ctx = TaskContext::build(sensor_task, &ceilings);
        assert!(ctx.needs_lock("sensor_data"));

        // sensor_read has priority 3, counter ceiling is 3 => NO lock needed
        assert!(!ctx.needs_lock("counter"));

        // control_loop has priority 5, sensor_data ceiling is 5 => NO lock needed
        let control_task = app.find_task("control_loop").expect("exists");
        let ctx2 = TaskContext::build(control_task, &ceilings);
        assert!(!ctx2.needs_lock("sensor_data"));
    }

    #[test]
    fn s19_6_critical_section_analysis() {
        let app = sample_app();
        let ceilings = compute_ceilings(&app);
        let cs = CriticalSectionAnalysis::analyze(&app, &ceilings);

        // control_loop (prio 5) accesses sensor_data (ceil 5) and led_state (ceil 5)
        // Both lock-free => 2 lock-free
        let ctrl_reqs = &cs.lock_requirements["control_loop"];
        assert_eq!(ctrl_reqs.len(), 2);
        assert!(ctrl_reqs.iter().all(|(_, needs)| !needs));

        // sensor_read (prio 3) accesses sensor_data (ceil 5) => locked
        // sensor_read (prio 3) accesses counter (ceil 3) => lock-free
        let sensor_reqs = &cs.lock_requirements["sensor_read"];
        let sensor_data_lock = sensor_reqs.iter().find(|(n, _)| n == "sensor_data");
        assert_eq!(sensor_data_lock.map(|(_, l)| *l), Some(true));
        let counter_lock = sensor_reqs.iter().find(|(n, _)| n == "counter");
        assert_eq!(counter_lock.map(|(_, l)| *l), Some(false));

        assert!(cs.lock_free_count > 0);
        assert!(cs.locked_count > 0);
    }

    #[test]
    fn s19_7_stack_analysis() {
        let mut app = RticApp::new("stm32f407");
        app.add_task(RticTask::new("t1", 1, "INT1", "h1").with_stack_frame(256));
        app.add_task(RticTask::new("t2", 1, "INT2", "h2").with_stack_frame(512));
        app.add_task(RticTask::new("t3", 3, "INT3", "h3").with_stack_frame(128));

        let sa = StackAnalysis::analyze(&app);
        // Priority 1: max(256, 512) = 512
        assert_eq!(sa.stack_at_priority(1), 512);
        // Priority 3: 128
        assert_eq!(sa.stack_at_priority(3), 128);
        // Total WCSS: 512 + 128 = 640
        assert_eq!(sa.total_wcss, 640);
    }

    #[test]
    fn s19_8_validate_app_detects_duplicate_task() {
        let mut app = RticApp::new("test");
        app.add_task(RticTask::new("t1", 1, "INT1", "h1"));
        app.add_task(RticTask::new("t1", 2, "INT2", "h2"));

        let result = validate_app(&app);
        assert!(matches!(result, Err(RticError::DuplicateTask { .. })));
    }

    #[test]
    fn s19_9_validate_app_detects_unknown_resource() {
        let mut app = RticApp::new("test");
        app.add_task(RticTask::new("t1", 1, "INT1", "h1").with_resource("nonexistent"));

        let result = validate_app(&app);
        assert!(matches!(result, Err(RticError::UnknownResource { .. })));
    }

    #[test]
    fn s19_10_deadlock_freedom_proof() {
        let app = sample_app();
        // SRP guarantees deadlock freedom for well-formed apps
        let result = DeadlockFreedomProof::check(&app);
        assert!(result.is_ok());
    }
}
