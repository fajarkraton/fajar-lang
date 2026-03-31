//! @kernel Safety Proofs — Sprint V3: 10 tasks.
//!
//! Proves safety properties for `@kernel` annotated code: no-heap allocation,
//! no-tensor usage, stack bound safety, interrupt safety, MMIO safety,
//! DMA buffer safety, concurrency safety, panic-freedom, and termination.
//! All simulated (constraint-based, no real Z3).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// V3.1: Safety Violation Types
// ═══════════════════════════════════════════════════════════════════════

/// A safety violation found during @kernel proof checking.
#[derive(Debug, Clone, PartialEq)]
pub struct SafetyViolation {
    /// Violation kind.
    pub kind: KernelViolationKind,
    /// Description of the violation.
    pub description: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Suggested fix.
    pub suggestion: String,
}

impl fmt::Display for SafetyViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} [{}] {} (fix: {})",
            self.file, self.line, self.kind, self.description, self.suggestion
        )
    }
}

/// Kinds of @kernel safety violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelViolationKind {
    /// Heap allocation in @kernel context (KE001).
    HeapAlloc,
    /// Tensor operation in @kernel context (KE002).
    TensorOp,
    /// Stack overflow risk.
    StackBound,
    /// Interrupt safety violation.
    InterruptSafety,
    /// Invalid MMIO access.
    MmioSafety,
    /// DMA buffer alignment/bounds violation.
    DmaBuffer,
    /// Concurrency safety violation (data race, deadlock).
    ConcurrencySafety,
    /// Possible panic in @kernel code.
    PanicFreedom,
    /// Non-termination risk.
    Termination,
}

impl fmt::Display for KernelViolationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::HeapAlloc => "KE001:heap-alloc",
            Self::TensorOp => "KE002:tensor-op",
            Self::StackBound => "KE-STACK",
            Self::InterruptSafety => "KE-IRQ",
            Self::MmioSafety => "KE-MMIO",
            Self::DmaBuffer => "KE-DMA",
            Self::ConcurrencySafety => "KE-CONCURRENCY",
            Self::PanicFreedom => "KE-PANIC",
            Self::Termination => "KE-TERM",
        };
        write!(f, "{s}")
    }
}

// ═══════════════════════════════════════════════════════════════════════
// V3.2: Function Representation for Analysis
// ═══════════════════════════════════════════════════════════════════════

/// Represents a @kernel function for safety checking.
#[derive(Debug, Clone, Default)]
pub struct KernelFunction {
    /// Function name.
    pub name: String,
    /// Source file.
    pub file: String,
    /// Source line.
    pub line: u32,
    /// Operations performed in the function body.
    pub operations: Vec<KernelOp>,
    /// Functions called from this function.
    pub callees: Vec<String>,
    /// Maximum estimated stack usage in bytes.
    pub estimated_stack_bytes: u64,
    /// Whether the function contains loops.
    pub has_loops: bool,
    /// Whether the function contains recursion (direct or via callees).
    pub has_recursion: bool,
    /// Lock acquisitions in the function body.
    pub lock_acquisitions: Vec<String>,
    /// Shared variables accessed.
    pub shared_accesses: Vec<SharedAccess>,
}

/// An operation performed inside a @kernel function.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelOp {
    /// Stack allocation (local variable).
    StackAlloc { name: String, size_bytes: u64 },
    /// Heap allocation (Box, Vec, String, etc.) -- forbidden.
    HeapAlloc { call_site: String },
    /// Tensor operation -- forbidden.
    TensorOp { op_name: String },
    /// MMIO read.
    MmioRead { address: String, size_bytes: u32 },
    /// MMIO write.
    MmioWrite { address: String, size_bytes: u32 },
    /// DMA buffer setup.
    DmaSetup {
        buffer_addr: String,
        size_bytes: u64,
        alignment: u64,
    },
    /// IRQ enable/disable.
    IrqControl { enable: bool },
    /// Lock acquire.
    LockAcquire { lock_name: String },
    /// Lock release.
    LockRelease { lock_name: String },
    /// Panic/assert that could panic.
    PanicSite { reason: String },
    /// Function call.
    Call { target: String },
}

/// A shared variable access.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedAccess {
    /// Variable name.
    pub variable: String,
    /// Whether the access is a write.
    pub is_write: bool,
    /// Whether it is protected by a lock.
    pub under_lock: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// V3.3: Individual Proof Checkers
// ═══════════════════════════════════════════════════════════════════════

/// Checks for heap allocations in @kernel code.
pub fn check_no_heap(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    for op in &func.operations {
        if let KernelOp::HeapAlloc { call_site } = op {
            violations.push(SafetyViolation {
                kind: KernelViolationKind::HeapAlloc,
                description: format!(
                    "heap allocation in @kernel function '{}' at '{call_site}'",
                    func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "use stack allocation or static buffers instead".to_string(),
            });
        }
    }
    violations
}

/// Checks for tensor operations in @kernel code.
pub fn check_no_tensor(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    for op in &func.operations {
        if let KernelOp::TensorOp { op_name } = op {
            violations.push(SafetyViolation {
                kind: KernelViolationKind::TensorOp,
                description: format!(
                    "tensor operation '{op_name}' in @kernel function '{}'",
                    func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "move tensor operations to @device context".to_string(),
            });
        }
    }
    violations
}

/// Checks stack bounds: total estimated stack usage must be within limit.
pub fn check_stack_bounds(func: &KernelFunction, stack_limit: u64) -> Vec<SafetyViolation> {
    let mut total_stack: u64 = 0;
    for op in &func.operations {
        if let KernelOp::StackAlloc { size_bytes, .. } = op {
            total_stack = total_stack.saturating_add(*size_bytes);
        }
    }
    total_stack = total_stack.max(func.estimated_stack_bytes);

    let mut violations = Vec::new();
    if total_stack > stack_limit {
        violations.push(SafetyViolation {
            kind: KernelViolationKind::StackBound,
            description: format!(
                "@kernel function '{}' uses ~{total_stack} bytes of stack, limit is {stack_limit}",
                func.name
            ),
            file: func.file.clone(),
            line: func.line,
            suggestion: "reduce local variable sizes or use static buffers".to_string(),
        });
    }
    violations
}

/// Checks interrupt safety: no deadlock risk when holding locks across IRQ.
pub fn check_interrupt_safety(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    let mut irq_enabled = true;
    let mut locks_held: Vec<String> = Vec::new();

    for op in &func.operations {
        match op {
            KernelOp::IrqControl { enable } => {
                irq_enabled = *enable;
            }
            KernelOp::LockAcquire { lock_name } => {
                if irq_enabled {
                    violations.push(SafetyViolation {
                        kind: KernelViolationKind::InterruptSafety,
                        description: format!(
                            "lock '{lock_name}' acquired with interrupts enabled in '{}' \
                             (risk: IRQ handler may deadlock)",
                            func.name
                        ),
                        file: func.file.clone(),
                        line: func.line,
                        suggestion: "disable interrupts before acquiring kernel locks".to_string(),
                    });
                }
                locks_held.push(lock_name.clone());
            }
            KernelOp::LockRelease { lock_name } => {
                locks_held.retain(|l| l != lock_name);
            }
            _ => {}
        }
    }

    // Check for unreleased locks
    for lock in &locks_held {
        violations.push(SafetyViolation {
            kind: KernelViolationKind::InterruptSafety,
            description: format!(
                "lock '{lock}' not released at end of @kernel function '{}'",
                func.name
            ),
            file: func.file.clone(),
            line: func.line,
            suggestion: "ensure all locks are released before function returns".to_string(),
        });
    }

    violations
}

/// Checks MMIO safety: addresses must be within valid ranges and properly sized.
pub fn check_mmio_safety(
    func: &KernelFunction,
    valid_ranges: &[(u64, u64)],
) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        let (addr_str, size) = match op {
            KernelOp::MmioRead {
                address,
                size_bytes,
            } => (address, *size_bytes),
            KernelOp::MmioWrite {
                address,
                size_bytes,
            } => (address, *size_bytes),
            _ => continue,
        };

        // Parse address (support hex)
        let addr = if let Some(hex) = addr_str.strip_prefix("0x") {
            u64::from_str_radix(hex, 16).ok()
        } else {
            addr_str.parse::<u64>().ok()
        };

        match addr {
            Some(a) => {
                let end = a.saturating_add(size as u64);
                let in_range = valid_ranges.iter().any(|(lo, hi)| a >= *lo && end <= *hi);
                if !in_range {
                    violations.push(SafetyViolation {
                        kind: KernelViolationKind::MmioSafety,
                        description: format!(
                            "MMIO access at {addr_str} (size={size}) outside valid ranges in '{}'",
                            func.name
                        ),
                        file: func.file.clone(),
                        line: func.line,
                        suggestion: "verify MMIO address is within device register map".to_string(),
                    });
                }
            }
            None => {
                // Symbolic address -- cannot verify statically
                violations.push(SafetyViolation {
                    kind: KernelViolationKind::MmioSafety,
                    description: format!(
                        "symbolic MMIO address '{addr_str}' in '{}' cannot be verified",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "use concrete MMIO addresses or add @requires constraint"
                        .to_string(),
                });
            }
        }
    }

    violations
}

/// Checks DMA buffer safety: alignment and size constraints.
pub fn check_dma_safety(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let KernelOp::DmaSetup {
            buffer_addr,
            size_bytes,
            alignment,
        } = op
        {
            if *alignment == 0 || (*alignment & (*alignment - 1)) != 0 {
                violations.push(SafetyViolation {
                    kind: KernelViolationKind::DmaBuffer,
                    description: format!(
                        "DMA buffer '{buffer_addr}' has non-power-of-2 alignment ({alignment}) in '{}'",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "DMA buffers must be aligned to power-of-2 boundaries".to_string(),
                });
            }
            if *size_bytes == 0 {
                violations.push(SafetyViolation {
                    kind: KernelViolationKind::DmaBuffer,
                    description: format!(
                        "DMA buffer '{buffer_addr}' has zero size in '{}'",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "DMA buffer size must be > 0".to_string(),
                });
            }
        }
    }

    violations
}

/// Checks concurrency safety: shared variable access must be under a lock.
pub fn check_concurrency_safety(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    for access in &func.shared_accesses {
        if access.is_write && access.under_lock.is_none() {
            violations.push(SafetyViolation {
                kind: KernelViolationKind::ConcurrencySafety,
                description: format!(
                    "write to shared variable '{}' without lock in '{}'",
                    access.variable, func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "acquire a lock before writing to shared state".to_string(),
            });
        }
    }

    // Check for potential deadlock: multiple locks acquired in inconsistent order
    let lock_order: Vec<&str> = func
        .operations
        .iter()
        .filter_map(|op| match op {
            KernelOp::LockAcquire { lock_name } => Some(lock_name.as_str()),
            _ => None,
        })
        .collect();

    if lock_order.len() >= 2 {
        // Simple check: if the same lock appears twice, potential deadlock
        let mut seen = std::collections::HashSet::new();
        for lock in &lock_order {
            if !seen.insert(*lock) {
                violations.push(SafetyViolation {
                    kind: KernelViolationKind::ConcurrencySafety,
                    description: format!(
                        "lock '{lock}' acquired multiple times in '{}' (deadlock risk)",
                        func.name
                    ),
                    file: func.file.clone(),
                    line: func.line,
                    suggestion: "avoid nested acquisition of the same lock".to_string(),
                });
            }
        }
    }

    violations
}

/// Checks panic-freedom: no panic!, unwrap, or assert that could fail.
pub fn check_panic_freedom(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    for op in &func.operations {
        if let KernelOp::PanicSite { reason } = op {
            violations.push(SafetyViolation {
                kind: KernelViolationKind::PanicFreedom,
                description: format!(
                    "potential panic in @kernel function '{}': {reason}",
                    func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "use Result/Option instead of panicking in @kernel code".to_string(),
            });
        }
    }

    violations
}

/// Checks termination: no unbounded loops or recursion.
pub fn check_termination(func: &KernelFunction) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    if func.has_recursion {
        violations.push(SafetyViolation {
            kind: KernelViolationKind::Termination,
            description: format!(
                "@kernel function '{}' contains recursion (may not terminate)",
                func.name
            ),
            file: func.file.clone(),
            line: func.line,
            suggestion: "convert recursion to iteration with bounded loop".to_string(),
        });
    }

    if func.has_loops {
        // Check if any loop variant (termination measure) is provided
        // For simulated checking, we flag loops without known bounds
        let has_bounded_loops = func.operations.iter().any(
            |op| matches!(op, KernelOp::StackAlloc { name, .. } if name.starts_with("loop_bound")),
        );

        if !has_bounded_loops {
            violations.push(SafetyViolation {
                kind: KernelViolationKind::Termination,
                description: format!(
                    "@kernel function '{}' contains loops without proven bounds",
                    func.name
                ),
                file: func.file.clone(),
                line: func.line,
                suggestion: "add @invariant and @decreases annotations to loops".to_string(),
            });
        }
    }

    violations
}

// ═══════════════════════════════════════════════════════════════════════
// V3.10: Kernel Safety Checker (Orchestrator)
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for kernel safety checking.
#[derive(Debug, Clone)]
pub struct KernelCheckConfig {
    /// Maximum stack size in bytes.
    pub stack_limit: u64,
    /// Valid MMIO address ranges.
    pub mmio_ranges: Vec<(u64, u64)>,
    /// Whether to check interrupt safety.
    pub check_interrupts: bool,
    /// Whether to check termination.
    pub check_termination: bool,
    /// Whether to check panic-freedom.
    pub check_panic_freedom: bool,
}

impl Default for KernelCheckConfig {
    fn default() -> Self {
        Self {
            stack_limit: 8192,                             // 8KB default kernel stack
            mmio_ranges: vec![(0x4000_0000, 0x5000_0000)], // Default peripheral range
            check_interrupts: true,
            check_termination: true,
            check_panic_freedom: true,
        }
    }
}

/// Kernel safety checker: runs all proof checks on @kernel functions.
#[derive(Debug)]
pub struct KernelSafetyChecker {
    /// Configuration.
    pub config: KernelCheckConfig,
    /// All violations found.
    pub violations: Vec<SafetyViolation>,
    /// Per-function violation counts.
    pub per_function: HashMap<String, Vec<SafetyViolation>>,
    /// Statistics.
    pub stats: KernelCheckStats,
}

/// Statistics for kernel safety checking.
#[derive(Debug, Clone, Default)]
pub struct KernelCheckStats {
    /// Total functions checked.
    pub functions_checked: u64,
    /// Total violations found.
    pub total_violations: u64,
    /// Violations by kind.
    pub by_kind: HashMap<KernelViolationKind, u64>,
    /// Functions that passed all checks.
    pub functions_clean: u64,
}

impl KernelSafetyChecker {
    /// Creates a new checker with the given configuration.
    pub fn new(config: KernelCheckConfig) -> Self {
        Self {
            config,
            violations: Vec::new(),
            per_function: HashMap::new(),
            stats: KernelCheckStats::default(),
        }
    }

    /// Creates a new checker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(KernelCheckConfig::default())
    }

    /// Runs all safety checks on a single @kernel function.
    pub fn check_function(&mut self, func: &KernelFunction) -> Vec<SafetyViolation> {
        let mut func_violations = Vec::new();

        // Mandatory checks
        func_violations.extend(check_no_heap(func));
        func_violations.extend(check_no_tensor(func));
        func_violations.extend(check_stack_bounds(func, self.config.stack_limit));
        func_violations.extend(check_dma_safety(func));
        func_violations.extend(check_mmio_safety(func, &self.config.mmio_ranges));
        func_violations.extend(check_concurrency_safety(func));

        // Optional checks
        if self.config.check_interrupts {
            func_violations.extend(check_interrupt_safety(func));
        }
        if self.config.check_panic_freedom {
            func_violations.extend(check_panic_freedom(func));
        }
        if self.config.check_termination {
            func_violations.extend(check_termination(func));
        }

        // Update stats
        self.stats.functions_checked += 1;
        if func_violations.is_empty() {
            self.stats.functions_clean += 1;
        }
        for v in &func_violations {
            *self.stats.by_kind.entry(v.kind).or_insert(0) += 1;
        }
        self.stats.total_violations += func_violations.len() as u64;

        self.per_function
            .insert(func.name.clone(), func_violations.clone());
        self.violations.extend(func_violations.clone());

        func_violations
    }

    /// Runs all safety checks on multiple @kernel functions.
    pub fn check_all(&mut self, functions: &[KernelFunction]) -> Vec<SafetyViolation> {
        for func in functions {
            self.check_function(func);
        }
        self.violations.clone()
    }

    /// Returns true if no violations were found.
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Returns the number of violations of a specific kind.
    pub fn count_kind(&self, kind: KernelViolationKind) -> u64 {
        self.stats.by_kind.get(&kind).copied().unwrap_or(0)
    }

    /// Generates a summary report.
    pub fn report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!(
            "Kernel Safety Report: {} functions checked\n",
            self.stats.functions_checked
        ));
        report.push_str(&format!(
            "  Clean: {}, With violations: {}\n",
            self.stats.functions_clean,
            self.stats.functions_checked - self.stats.functions_clean
        ));
        report.push_str(&format!(
            "  Total violations: {}\n",
            self.stats.total_violations
        ));
        if !self.stats.by_kind.is_empty() {
            report.push_str("  By kind:\n");
            let mut kinds: Vec<_> = self.stats.by_kind.iter().collect();
            kinds.sort_by_key(|(k, _)| format!("{k}"));
            for (kind, count) in kinds {
                report.push_str(&format!("    {kind}: {count}\n"));
            }
        }
        report
    }
}

impl Default for KernelSafetyChecker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clean_func(name: &str) -> KernelFunction {
        KernelFunction {
            name: name.to_string(),
            file: "kernel.fj".to_string(),
            line: 1,
            operations: vec![KernelOp::StackAlloc {
                name: "buf".to_string(),
                size_bytes: 256,
            }],
            callees: vec![],
            estimated_stack_bytes: 256,
            has_loops: false,
            has_recursion: false,
            lock_acquisitions: vec![],
            shared_accesses: vec![],
        }
    }

    // --- V3.1: No-Heap Proof ---

    #[test]
    fn v3_1_no_heap_clean() {
        let func = make_clean_func("init");
        let violations = check_no_heap(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_1_no_heap_violation() {
        let mut func = make_clean_func("bad_alloc");
        func.operations.push(KernelOp::HeapAlloc {
            call_site: "Vec::new()".to_string(),
        });
        let violations = check_no_heap(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::HeapAlloc);
        assert!(violations[0].description.contains("Vec::new()"));
    }

    // --- V3.2: No-Tensor Proof ---

    #[test]
    fn v3_2_no_tensor_clean() {
        let func = make_clean_func("read_sensor");
        let violations = check_no_tensor(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_2_no_tensor_violation() {
        let mut func = make_clean_func("bad_tensor");
        func.operations.push(KernelOp::TensorOp {
            op_name: "matmul".to_string(),
        });
        let violations = check_no_tensor(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::TensorOp);
    }

    // --- V3.3: Stack Bounds ---

    #[test]
    fn v3_3_stack_within_limit() {
        let func = make_clean_func("small_fn");
        let violations = check_stack_bounds(&func, 8192);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_3_stack_exceeds_limit() {
        let mut func = make_clean_func("big_fn");
        func.operations.push(KernelOp::StackAlloc {
            name: "huge_buf".to_string(),
            size_bytes: 16384,
        });
        let violations = check_stack_bounds(&func, 8192);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::StackBound);
    }

    // --- V3.4: Interrupt Safety ---

    #[test]
    fn v3_4_irq_safe() {
        let mut func = make_clean_func("irq_safe_fn");
        func.operations = vec![
            KernelOp::IrqControl { enable: false },
            KernelOp::LockAcquire {
                lock_name: "spinlock".to_string(),
            },
            KernelOp::LockRelease {
                lock_name: "spinlock".to_string(),
            },
            KernelOp::IrqControl { enable: true },
        ];
        let violations = check_interrupt_safety(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_4_irq_unsafe_lock() {
        let mut func = make_clean_func("irq_unsafe_fn");
        func.operations = vec![
            // IRQ enabled by default
            KernelOp::LockAcquire {
                lock_name: "spinlock".to_string(),
            },
            KernelOp::LockRelease {
                lock_name: "spinlock".to_string(),
            },
        ];
        let violations = check_interrupt_safety(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == KernelViolationKind::InterruptSafety)
        );
    }

    // --- V3.5: MMIO Safety ---

    #[test]
    fn v3_5_mmio_valid_range() {
        let mut func = make_clean_func("mmio_fn");
        func.operations.push(KernelOp::MmioRead {
            address: "0x40001000".to_string(),
            size_bytes: 4,
        });
        let ranges = vec![(0x4000_0000, 0x5000_0000)];
        let violations = check_mmio_safety(&func, &ranges);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_5_mmio_invalid_range() {
        let mut func = make_clean_func("mmio_fn");
        func.operations.push(KernelOp::MmioWrite {
            address: "0x10000000".to_string(),
            size_bytes: 4,
        });
        let ranges = vec![(0x4000_0000, 0x5000_0000)];
        let violations = check_mmio_safety(&func, &ranges);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::MmioSafety);
    }

    // --- V3.6: DMA Safety ---

    #[test]
    fn v3_6_dma_valid() {
        let mut func = make_clean_func("dma_fn");
        func.operations.push(KernelOp::DmaSetup {
            buffer_addr: "0x80000000".to_string(),
            size_bytes: 4096,
            alignment: 4096,
        });
        let violations = check_dma_safety(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_6_dma_bad_alignment() {
        let mut func = make_clean_func("dma_fn");
        func.operations.push(KernelOp::DmaSetup {
            buffer_addr: "0x80000000".to_string(),
            size_bytes: 4096,
            alignment: 3, // not power of 2
        });
        let violations = check_dma_safety(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::DmaBuffer);
    }

    #[test]
    fn v3_6_dma_zero_size() {
        let mut func = make_clean_func("dma_fn");
        func.operations.push(KernelOp::DmaSetup {
            buffer_addr: "0x80000000".to_string(),
            size_bytes: 0,
            alignment: 64,
        });
        let violations = check_dma_safety(&func);
        assert!(!violations.is_empty());
    }

    // --- V3.7: Concurrency Safety ---

    #[test]
    fn v3_7_shared_write_under_lock() {
        let mut func = make_clean_func("conc_fn");
        func.shared_accesses.push(SharedAccess {
            variable: "counter".to_string(),
            is_write: true,
            under_lock: Some("counter_lock".to_string()),
        });
        let violations = check_concurrency_safety(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_7_shared_write_no_lock() {
        let mut func = make_clean_func("conc_fn");
        func.shared_accesses.push(SharedAccess {
            variable: "counter".to_string(),
            is_write: true,
            under_lock: None,
        });
        let violations = check_concurrency_safety(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::ConcurrencySafety);
    }

    // --- V3.8: Panic-Freedom ---

    #[test]
    fn v3_8_no_panic() {
        let func = make_clean_func("safe_fn");
        let violations = check_panic_freedom(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_8_has_panic() {
        let mut func = make_clean_func("panicky_fn");
        func.operations.push(KernelOp::PanicSite {
            reason: "unwrap() on None".to_string(),
        });
        let violations = check_panic_freedom(&func);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, KernelViolationKind::PanicFreedom);
    }

    // --- V3.9: Termination ---

    #[test]
    fn v3_9_no_loops() {
        let func = make_clean_func("straight_fn");
        let violations = check_termination(&func);
        assert!(violations.is_empty());
    }

    #[test]
    fn v3_9_recursion() {
        let mut func = make_clean_func("recurse_fn");
        func.has_recursion = true;
        let violations = check_termination(&func);
        assert!(!violations.is_empty());
        assert!(
            violations
                .iter()
                .any(|v| v.kind == KernelViolationKind::Termination)
        );
    }

    // --- V3.10: Full Checker ---

    #[test]
    fn v3_10_checker_clean() {
        let mut checker = KernelSafetyChecker::with_defaults();
        let func = make_clean_func("init");
        let violations = checker.check_function(&func);
        assert!(violations.is_empty());
        assert!(checker.is_clean());
        assert_eq!(checker.stats.functions_checked, 1);
        assert_eq!(checker.stats.functions_clean, 1);
    }

    #[test]
    fn v3_10_checker_with_violations() {
        let mut checker = KernelSafetyChecker::with_defaults();
        let mut func = make_clean_func("bad_fn");
        func.operations.push(KernelOp::HeapAlloc {
            call_site: "String::new()".to_string(),
        });
        func.operations.push(KernelOp::TensorOp {
            op_name: "relu".to_string(),
        });
        let violations = checker.check_function(&func);
        assert!(violations.len() >= 2);
        assert!(!checker.is_clean());
        assert_eq!(checker.count_kind(KernelViolationKind::HeapAlloc), 1);
        assert_eq!(checker.count_kind(KernelViolationKind::TensorOp), 1);
    }

    #[test]
    fn v3_10_check_all_multiple() {
        let mut checker = KernelSafetyChecker::with_defaults();
        let clean = make_clean_func("good");
        let mut bad = make_clean_func("bad");
        bad.operations.push(KernelOp::HeapAlloc {
            call_site: "Box::new()".to_string(),
        });
        let violations = checker.check_all(&[clean, bad]);
        assert!(!violations.is_empty());
        assert_eq!(checker.stats.functions_checked, 2);
        assert_eq!(checker.stats.functions_clean, 1);
    }

    #[test]
    fn v3_10_report() {
        let mut checker = KernelSafetyChecker::with_defaults();
        let func = make_clean_func("ok");
        checker.check_function(&func);
        let report = checker.report();
        assert!(report.contains("1 functions checked"));
        assert!(report.contains("Clean: 1"));
    }

    #[test]
    fn v3_10_violation_display() {
        let v = SafetyViolation {
            kind: KernelViolationKind::HeapAlloc,
            description: "heap alloc in kernel".to_string(),
            file: "driver.fj".to_string(),
            line: 42,
            suggestion: "use stack alloc".to_string(),
        };
        let s = format!("{v}");
        assert!(s.contains("driver.fj:42"));
        assert!(s.contains("KE001"));
        assert!(s.contains("use stack alloc"));
    }
}
