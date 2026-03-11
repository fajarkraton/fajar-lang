//! Baseline JIT — fast compilation without optimization, code cache,
//! deoptimization hooks, stack frame compatibility, compilation metrics.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use super::counters::ExecutionTier;

// ═══════════════════════════════════════════════════════════════════════
// S14.1 / S14.2: Baseline Compiler
// ═══════════════════════════════════════════════════════════════════════

/// A baseline-compiled function.
#[derive(Debug, Clone)]
pub struct BaselineCode {
    /// Function name.
    pub name: String,
    /// Compiled code size in bytes.
    pub code_size: usize,
    /// Compilation time.
    pub compile_time: Duration,
    /// Whether this code has deopt points.
    pub has_deopt_points: bool,
    /// Number of basic blocks.
    pub block_count: u32,
    /// Deoptimization metadata per deopt point.
    pub deopt_metadata: Vec<DeoptPoint>,
}

/// Compilation request for baseline JIT.
#[derive(Debug, Clone)]
pub struct BaselineCompileRequest {
    /// Function name.
    pub name: String,
    /// Number of parameters.
    pub param_count: usize,
    /// Number of local variables.
    pub local_count: usize,
    /// Whether the function has loops.
    pub has_loops: bool,
    /// Estimated IR instruction count.
    pub ir_size_estimate: usize,
}

/// Result of baseline compilation.
#[derive(Debug, Clone)]
pub enum CompileResult {
    /// Successfully compiled.
    Success(BaselineCode),
    /// Compilation failed.
    Failed(String),
    /// Function too large for baseline (defer to optimizing).
    TooLarge { ir_size: usize, limit: usize },
}

/// Compiles a function at baseline tier (no optimizations).
pub fn compile_baseline(request: &BaselineCompileRequest) -> CompileResult {
    const MAX_BASELINE_SIZE: usize = 10_000;

    if request.ir_size_estimate > MAX_BASELINE_SIZE {
        return CompileResult::TooLarge {
            ir_size: request.ir_size_estimate,
            limit: MAX_BASELINE_SIZE,
        };
    }

    // Simulated baseline compilation — fast, no optimization
    let code_size = request.ir_size_estimate * 4; // ~4 bytes per IR instruction
    let compile_time = Duration::from_micros((request.ir_size_estimate as u64) * 2);

    let mut deopt_metadata = Vec::new();
    if request.has_loops {
        deopt_metadata.push(DeoptPoint {
            offset: 0,
            kind: DeoptKind::LoopHeader,
            local_mapping: (0..request.local_count).map(|i| (i, i as u32)).collect(),
        });
    }

    CompileResult::Success(BaselineCode {
        name: request.name.clone(),
        code_size,
        compile_time,
        has_deopt_points: !deopt_metadata.is_empty(),
        block_count: (request.ir_size_estimate / 10).max(1) as u32,
        deopt_metadata,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S14.3 / S14.4: No Optimization / Simple Register Allocation
// ═══════════════════════════════════════════════════════════════════════

/// Optimization level for compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No optimization (baseline).
    None,
    /// Basic optimization (some CSE, DCE).
    Basic,
    /// Full optimization (inlining, LICM, speculation).
    Full,
}

impl fmt::Display for OptLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptLevel::None => write!(f, "O0"),
            OptLevel::Basic => write!(f, "O1"),
            OptLevel::Full => write!(f, "O2"),
        }
    }
}

/// Register allocation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegAllocStrategy {
    /// Fast linear scan (baseline).
    LinearScan,
    /// Graph coloring (optimizing).
    GraphColoring,
}

// ═══════════════════════════════════════════════════════════════════════
// S14.5 / S14.6: Code Patching & Cache
// ═══════════════════════════════════════════════════════════════════════

/// A code cache for compiled functions.
#[derive(Debug, Clone, Default)]
pub struct CodeCache {
    /// Function name -> compiled code entry.
    entries: HashMap<String, CacheEntry>,
    /// Total code size in bytes.
    total_code_size: usize,
    /// Maximum cache size in bytes.
    max_size: usize,
}

/// A cache entry for a compiled function.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The compiled code.
    pub code: BaselineCode,
    /// Current execution tier.
    pub tier: ExecutionTier,
    /// Number of times this code was invoked.
    pub invocation_count: u64,
}

impl CodeCache {
    /// Creates a new code cache with the given max size.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            total_code_size: 0,
            max_size,
        }
    }

    /// Inserts compiled code into the cache.
    pub fn insert(&mut self, code: BaselineCode, tier: ExecutionTier) -> bool {
        let size = code.code_size;
        if self.total_code_size + size > self.max_size {
            return false;
        }
        self.total_code_size += size;
        self.entries.insert(
            code.name.clone(),
            CacheEntry {
                code,
                tier,
                invocation_count: 0,
            },
        );
        true
    }

    /// Looks up a cached function.
    pub fn lookup(&self, name: &str) -> Option<&CacheEntry> {
        self.entries.get(name)
    }

    /// Replaces existing code (e.g., baseline -> optimized).
    pub fn replace(&mut self, code: BaselineCode, tier: ExecutionTier) {
        if let Some(entry) = self.entries.get_mut(&code.name) {
            self.total_code_size = self.total_code_size - entry.code.code_size + code.code_size;
            entry.code = code;
            entry.tier = tier;
        }
    }

    /// Returns the number of cached functions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total code size in bytes.
    pub fn total_size(&self) -> usize {
        self.total_code_size
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.7: Deoptimization Hooks
// ═══════════════════════════════════════════════════════════════════════

/// A deoptimization point in compiled code.
#[derive(Debug, Clone)]
pub struct DeoptPoint {
    /// Offset in compiled code.
    pub offset: usize,
    /// Kind of deopt point.
    pub kind: DeoptKind,
    /// Mapping from JIT registers/slots to interpreter local indices.
    pub local_mapping: Vec<(usize, u32)>,
}

/// Kind of deoptimization trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeoptKind {
    /// At a loop header (for OSR entry).
    LoopHeader,
    /// Type guard failure.
    TypeGuard,
    /// Overflow check failure.
    OverflowCheck,
    /// Bounds check failure.
    BoundsCheck,
    /// Explicit deopt request.
    Explicit,
}

impl fmt::Display for DeoptKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeoptKind::LoopHeader => write!(f, "loop-header"),
            DeoptKind::TypeGuard => write!(f, "type-guard"),
            DeoptKind::OverflowCheck => write!(f, "overflow"),
            DeoptKind::BoundsCheck => write!(f, "bounds"),
            DeoptKind::Explicit => write!(f, "explicit"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.8: Stack Frame Compatibility
// ═══════════════════════════════════════════════════════════════════════

/// Stack frame layout for mixed-mode execution.
#[derive(Debug, Clone)]
pub struct FrameLayout {
    /// Function name.
    pub function: String,
    /// Number of local slots.
    pub local_slots: usize,
    /// Frame size in bytes.
    pub frame_size: usize,
    /// Whether this is a JIT frame.
    pub is_jit: bool,
}

impl FrameLayout {
    /// Creates a JIT frame layout.
    pub fn jit(function: &str, local_slots: usize) -> Self {
        Self {
            function: function.into(),
            local_slots,
            frame_size: local_slots * 8 + 16, // 8 bytes per slot + saved rbp + return addr
            is_jit: true,
        }
    }

    /// Creates an interpreter frame layout.
    pub fn interpreter(function: &str, local_slots: usize) -> Self {
        Self {
            function: function.into(),
            local_slots,
            frame_size: 0, // interpreter uses heap-allocated environments
            is_jit: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S14.9: Compilation Metrics
// ═══════════════════════════════════════════════════════════════════════

/// Metrics for a single compilation event.
#[derive(Debug, Clone)]
pub struct CompilationMetrics {
    /// Function name.
    pub function: String,
    /// Compilation tier.
    pub tier: ExecutionTier,
    /// Time from first call to compilation trigger.
    pub time_to_compile: Duration,
    /// Compilation latency.
    pub compile_latency: Duration,
    /// Generated code size in bytes.
    pub code_size: usize,
    /// Number of IR instructions before compilation.
    pub ir_instructions: usize,
}

/// Aggregated compilation statistics.
#[derive(Debug, Clone, Default)]
pub struct CompilationStats {
    /// All compilation events.
    pub events: Vec<CompilationMetrics>,
}

impl CompilationStats {
    /// Creates empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a compilation event.
    pub fn record(&mut self, metrics: CompilationMetrics) {
        self.events.push(metrics);
    }

    /// Average compilation latency.
    pub fn avg_latency(&self) -> Duration {
        if self.events.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.events.iter().map(|e| e.compile_latency).sum();
        total / self.events.len() as u32
    }

    /// Total code size across all compilations.
    pub fn total_code_size(&self) -> usize {
        self.events.iter().map(|e| e.code_size).sum()
    }

    /// Number of baseline compilations.
    pub fn baseline_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.tier == ExecutionTier::BaselineJIT)
            .count()
    }

    /// Number of optimizing compilations.
    pub fn optimizing_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.tier == ExecutionTier::OptimizingJIT)
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S14.1 — Baseline Compiler Entry
    #[test]
    fn s14_1_compile_baseline_success() {
        let req = BaselineCompileRequest {
            name: "factorial".into(),
            param_count: 1,
            local_count: 3,
            has_loops: false,
            ir_size_estimate: 50,
        };
        match compile_baseline(&req) {
            CompileResult::Success(code) => {
                assert_eq!(code.name, "factorial");
                assert!(code.code_size > 0);
            }
            _ => panic!("expected success"),
        }
    }

    // S14.2 — Fast IR Translation
    #[test]
    fn s14_2_sub_millisecond_compile() {
        let req = BaselineCompileRequest {
            name: "simple".into(),
            param_count: 2,
            local_count: 5,
            has_loops: false,
            ir_size_estimate: 100,
        };
        if let CompileResult::Success(code) = compile_baseline(&req) {
            assert!(code.compile_time < Duration::from_millis(1));
        }
    }

    // S14.3 — No Optimization
    #[test]
    fn s14_3_opt_level_display() {
        assert_eq!(OptLevel::None.to_string(), "O0");
        assert_eq!(OptLevel::Basic.to_string(), "O1");
        assert_eq!(OptLevel::Full.to_string(), "O2");
    }

    // S14.4 — Register Allocation
    #[test]
    fn s14_4_regalloc_strategies() {
        assert_ne!(
            RegAllocStrategy::LinearScan,
            RegAllocStrategy::GraphColoring
        );
    }

    // S14.5 — Code Patching (via cache replace)
    #[test]
    fn s14_5_code_cache_insert_lookup() {
        let mut cache = CodeCache::new(1_000_000);
        let code = BaselineCode {
            name: "foo".into(),
            code_size: 200,
            compile_time: Duration::from_micros(100),
            has_deopt_points: false,
            block_count: 5,
            deopt_metadata: Vec::new(),
        };
        assert!(cache.insert(code, ExecutionTier::BaselineJIT));
        assert!(cache.lookup("foo").is_some());
        assert!(cache.lookup("bar").is_none());
    }

    // S14.6 — Code Cache
    #[test]
    fn s14_6_cache_size_limit() {
        let mut cache = CodeCache::new(100);
        let code = BaselineCode {
            name: "big".into(),
            code_size: 200,
            compile_time: Duration::ZERO,
            has_deopt_points: false,
            block_count: 1,
            deopt_metadata: Vec::new(),
        };
        assert!(!cache.insert(code, ExecutionTier::BaselineJIT));
        assert!(cache.is_empty());
    }

    #[test]
    fn s14_6_cache_replace() {
        let mut cache = CodeCache::new(1_000_000);
        let code1 = BaselineCode {
            name: "f".into(),
            code_size: 100,
            compile_time: Duration::ZERO,
            has_deopt_points: false,
            block_count: 3,
            deopt_metadata: Vec::new(),
        };
        cache.insert(code1, ExecutionTier::BaselineJIT);
        let code2 = BaselineCode {
            name: "f".into(),
            code_size: 80,
            compile_time: Duration::ZERO,
            has_deopt_points: false,
            block_count: 5,
            deopt_metadata: Vec::new(),
        };
        cache.replace(code2, ExecutionTier::OptimizingJIT);
        let entry = cache.lookup("f").unwrap();
        assert_eq!(entry.tier, ExecutionTier::OptimizingJIT);
        assert_eq!(entry.code.block_count, 5);
        assert_eq!(cache.total_size(), 80);
    }

    // S14.7 — Deopt Hooks
    #[test]
    fn s14_7_deopt_points_in_loop() {
        let req = BaselineCompileRequest {
            name: "loop_fn".into(),
            param_count: 0,
            local_count: 2,
            has_loops: true,
            ir_size_estimate: 30,
        };
        if let CompileResult::Success(code) = compile_baseline(&req) {
            assert!(code.has_deopt_points);
            assert!(!code.deopt_metadata.is_empty());
            assert_eq!(code.deopt_metadata[0].kind, DeoptKind::LoopHeader);
        }
    }

    #[test]
    fn s14_7_deopt_kind_display() {
        assert_eq!(DeoptKind::TypeGuard.to_string(), "type-guard");
        assert_eq!(DeoptKind::OverflowCheck.to_string(), "overflow");
    }

    // S14.8 — Stack Frame
    #[test]
    fn s14_8_jit_frame_layout() {
        let frame = FrameLayout::jit("foo", 4);
        assert!(frame.is_jit);
        assert_eq!(frame.frame_size, 4 * 8 + 16);
    }

    #[test]
    fn s14_8_interpreter_frame() {
        let frame = FrameLayout::interpreter("bar", 3);
        assert!(!frame.is_jit);
        assert_eq!(frame.frame_size, 0);
    }

    // S14.9 — Compilation Metrics
    #[test]
    fn s14_9_compilation_stats() {
        let mut stats = CompilationStats::new();
        stats.record(CompilationMetrics {
            function: "f".into(),
            tier: ExecutionTier::BaselineJIT,
            time_to_compile: Duration::from_millis(500),
            compile_latency: Duration::from_micros(200),
            code_size: 400,
            ir_instructions: 100,
        });
        stats.record(CompilationMetrics {
            function: "g".into(),
            tier: ExecutionTier::OptimizingJIT,
            time_to_compile: Duration::from_secs(5),
            compile_latency: Duration::from_millis(10),
            code_size: 300,
            ir_instructions: 80,
        });
        assert_eq!(stats.baseline_count(), 1);
        assert_eq!(stats.optimizing_count(), 1);
        assert_eq!(stats.total_code_size(), 700);
    }

    // S14.10 — Additional
    #[test]
    fn s14_10_too_large_function() {
        let req = BaselineCompileRequest {
            name: "huge".into(),
            param_count: 0,
            local_count: 0,
            has_loops: false,
            ir_size_estimate: 20_000,
        };
        match compile_baseline(&req) {
            CompileResult::TooLarge { ir_size, limit } => {
                assert_eq!(ir_size, 20_000);
                assert_eq!(limit, 10_000);
            }
            _ => panic!("expected TooLarge"),
        }
    }

    #[test]
    fn s14_10_cache_len() {
        let mut cache = CodeCache::new(1_000_000);
        assert_eq!(cache.len(), 0);
        let code = BaselineCode {
            name: "a".into(),
            code_size: 10,
            compile_time: Duration::ZERO,
            has_deopt_points: false,
            block_count: 1,
            deopt_metadata: Vec::new(),
        };
        cache.insert(code, ExecutionTier::BaselineJIT);
        assert_eq!(cache.len(), 1);
    }
}
