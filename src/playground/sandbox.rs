//! Wasm sandbox — memory limits, execution timeout, print capture, error formatting.
//!
//! This module provides the core Wasm-safe execution environment for
//! running Fajar Lang programs in the browser playground.

use std::cell::RefCell;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Wasm Build Configuration (S29.1)
// ═══════════════════════════════════════════════════════════════════════

/// Wasm build configuration for wasm32-unknown-unknown target.
#[derive(Debug, Clone)]
pub struct WasmBuildConfig {
    /// Target triple.
    pub target: String,
    /// Feature exclusions for Wasm build.
    pub excluded_features: Vec<String>,
    /// wasm-opt optimization level.
    pub opt_level: WasmOptLevel,
    /// Maximum Wasm binary size target in bytes.
    pub max_binary_size: usize,
}

/// wasm-opt optimization level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmOptLevel {
    /// No optimization.
    O0,
    /// Basic optimization.
    O1,
    /// Full optimization.
    O2,
    /// Aggressive size optimization.
    Oz,
    /// Maximum size optimization.
    Os,
}

impl fmt::Display for WasmOptLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::O0 => write!(f, "-O0"),
            Self::O1 => write!(f, "-O1"),
            Self::O2 => write!(f, "-O2"),
            Self::Oz => write!(f, "-Oz"),
            Self::Os => write!(f, "-Os"),
        }
    }
}

impl Default for WasmBuildConfig {
    fn default() -> Self {
        Self {
            target: "wasm32-unknown-unknown".to_string(),
            excluded_features: vec![
                "native".to_string(),
                "llvm".to_string(),
                "gpu".to_string(),
                "cuda".to_string(),
            ],
            opt_level: WasmOptLevel::Oz,
            max_binary_size: 5 * 1024 * 1024, // 5MB compressed
        }
    }
}

impl WasmBuildConfig {
    /// Returns cargo build arguments for Wasm target.
    pub fn cargo_args(&self) -> Vec<String> {
        vec![
            "build".to_string(),
            "--target".to_string(),
            self.target.clone(),
            "--release".to_string(),
            "--no-default-features".to_string(),
        ]
    }

    /// Returns wasm-opt arguments.
    pub fn wasm_opt_args(&self, input: &str, output: &str) -> Vec<String> {
        vec![
            format!("{}", self.opt_level),
            input.to_string(),
            "-o".to_string(),
            output.to_string(),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wasm API Exports (S29.2, S29.3, S29.4)
// ═══════════════════════════════════════════════════════════════════════

/// Entry points exported via wasm-bindgen.
#[derive(Debug, Clone)]
pub struct WasmExport {
    /// Export name visible to JavaScript.
    pub js_name: String,
    /// Rust function name.
    pub rust_fn: String,
    /// Input type description.
    pub input: String,
    /// Output type description.
    pub output: String,
}

/// Returns the list of functions exported to JavaScript via wasm-bindgen.
pub fn wasm_exports() -> Vec<WasmExport> {
    vec![
        WasmExport {
            js_name: "tokenize".to_string(),
            rust_fn: "wasm_tokenize".to_string(),
            input: "source: &str".to_string(),
            output: "JsValue (JSON array of tokens)".to_string(),
        },
        WasmExport {
            js_name: "parse".to_string(),
            rust_fn: "wasm_parse".to_string(),
            input: "source: &str".to_string(),
            output: "JsValue (JSON AST)".to_string(),
        },
        WasmExport {
            js_name: "eval_source".to_string(),
            rust_fn: "wasm_eval_source".to_string(),
            input: "source: &str".to_string(),
            output: "JsValue ({ result, stdout, errors, elapsed_ms })".to_string(),
        },
        WasmExport {
            js_name: "check".to_string(),
            rust_fn: "wasm_check".to_string(),
            input: "source: &str".to_string(),
            output: "JsValue ({ ok, errors })".to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Memory Sandbox (S29.5)
// ═══════════════════════════════════════════════════════════════════════

/// Memory limits for the Wasm sandbox.
#[derive(Debug, Clone)]
pub struct MemoryLimits {
    /// Maximum linear memory in bytes.
    pub max_memory_bytes: usize,
    /// Maximum stack depth.
    pub max_stack_depth: usize,
    /// Maximum heap allocations.
    pub max_heap_allocations: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024, // 64MB
            max_stack_depth: 256,
            max_heap_allocations: 100_000,
        }
    }
}

/// OOM error for the sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomError {
    /// Current usage when OOM occurred.
    pub current_bytes: usize,
    /// Maximum allowed.
    pub max_bytes: usize,
    /// What triggered OOM.
    pub trigger: String,
}

impl fmt::Display for OomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "out of memory: {} bytes used, {} bytes max (triggered by {})",
            self.current_bytes, self.max_bytes, self.trigger
        )
    }
}

/// Memory tracker for sandbox enforcement.
#[derive(Debug, Clone)]
pub struct MemoryTracker {
    /// Current allocated bytes.
    pub allocated: usize,
    /// Peak allocated bytes.
    pub peak: usize,
    /// Memory limits.
    pub limits: MemoryLimits,
    /// Number of allocations.
    pub allocation_count: usize,
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new(MemoryLimits::default())
    }
}

impl MemoryTracker {
    /// Creates a new tracker with given limits.
    pub fn new(limits: MemoryLimits) -> Self {
        Self {
            allocated: 0,
            peak: 0,
            limits,
            allocation_count: 0,
        }
    }

    /// Attempts to allocate bytes, returns error if OOM.
    pub fn allocate(&mut self, bytes: usize) -> Result<(), OomError> {
        if self.allocated + bytes > self.limits.max_memory_bytes {
            return Err(OomError {
                current_bytes: self.allocated,
                max_bytes: self.limits.max_memory_bytes,
                trigger: format!("allocation of {bytes} bytes"),
            });
        }
        if self.allocation_count + 1 > self.limits.max_heap_allocations {
            return Err(OomError {
                current_bytes: self.allocated,
                max_bytes: self.limits.max_memory_bytes,
                trigger: "max heap allocations exceeded".to_string(),
            });
        }
        self.allocated += bytes;
        self.allocation_count += 1;
        if self.allocated > self.peak {
            self.peak = self.allocated;
        }
        Ok(())
    }

    /// Frees bytes.
    pub fn free(&mut self, bytes: usize) {
        self.allocated = self.allocated.saturating_sub(bytes);
    }

    /// Returns usage as a fraction (0.0 to 1.0).
    pub fn usage_fraction(&self) -> f64 {
        self.allocated as f64 / self.limits.max_memory_bytes as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Execution Timeout (S29.6)
// ═══════════════════════════════════════════════════════════════════════

/// Execution limits for sandbox programs.
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    /// Maximum instruction count before abort.
    pub max_instructions: u64,
    /// Maximum execution time in milliseconds.
    pub max_time_ms: u64,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_instructions: 10_000_000, // 10M instructions
            max_time_ms: 5_000,           // 5 seconds
        }
    }
}

/// Instruction counter for timeout enforcement.
#[derive(Debug, Clone)]
pub struct InstructionCounter {
    /// Current instruction count.
    pub count: u64,
    /// Limits.
    pub limits: ExecutionLimits,
}

impl Default for InstructionCounter {
    fn default() -> Self {
        Self::new(ExecutionLimits::default())
    }
}

impl InstructionCounter {
    /// Creates a new counter.
    pub fn new(limits: ExecutionLimits) -> Self {
        Self { count: 0, limits }
    }

    /// Increments the counter, returns error if limit exceeded.
    pub fn tick(&mut self) -> Result<(), TimeoutError> {
        self.count += 1;
        if self.count > self.limits.max_instructions {
            return Err(TimeoutError::InstructionLimit {
                count: self.count,
                limit: self.limits.max_instructions,
            });
        }
        Ok(())
    }

    /// Resets the counter.
    pub fn reset(&mut self) {
        self.count = 0;
    }
}

/// Timeout error when execution limits are exceeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeoutError {
    /// Instruction count exceeded.
    InstructionLimit {
        /// Actual count.
        count: u64,
        /// Maximum allowed.
        limit: u64,
    },
    /// Wall-clock time exceeded.
    TimeLimit {
        /// Elapsed milliseconds.
        elapsed_ms: u64,
        /// Maximum allowed.
        limit_ms: u64,
    },
}

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstructionLimit { count, limit } => {
                write!(
                    f,
                    "execution timeout: {count} instructions executed (limit: {limit})"
                )
            }
            Self::TimeLimit {
                elapsed_ms,
                limit_ms,
            } => {
                write!(
                    f,
                    "execution timeout: {elapsed_ms}ms elapsed (limit: {limit_ms}ms)"
                )
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Print Capture (S29.7)
// ═══════════════════════════════════════════════════════════════════════

thread_local! {
    static CAPTURED_OUTPUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Captures a print output line (used to intercept println/print in Wasm).
pub fn capture_print(line: &str) {
    CAPTURED_OUTPUT.with(|output| {
        output.borrow_mut().push(line.to_string());
    });
}

/// Drains all captured output and returns it.
pub fn drain_captured_output() -> Vec<String> {
    CAPTURED_OUTPUT.with(|output| {
        let mut buf = output.borrow_mut();
        let lines = buf.clone();
        buf.clear();
        lines
    })
}

/// Returns captured output as a single string.
pub fn get_captured_stdout() -> String {
    drain_captured_output().join("\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Error Formatting (S29.8)
// ═══════════════════════════════════════════════════════════════════════

/// A formatted error for browser display (no ANSI codes).
#[derive(Debug, Clone)]
pub struct PlainError {
    /// Error code (e.g., "SE004").
    pub code: String,
    /// Error message.
    pub message: String,
    /// Source line number (1-based).
    pub line: usize,
    /// Source column (1-based).
    pub column: usize,
    /// Source line text.
    pub source_line: String,
    /// Suggestion for fixing.
    pub suggestion: Option<String>,
}

impl fmt::Display for PlainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error[{}]: {}", self.code, self.message)?;
        writeln!(f, "  --> line {}:{}", self.line, self.column)?;
        writeln!(f, "   |")?;
        writeln!(f, "{:>3} | {}", self.line, self.source_line)?;
        write!(f, "   | {}^", " ".repeat(self.column.saturating_sub(1)),)?;
        if let Some(ref sug) = self.suggestion {
            write!(f, "\n   = help: {sug}")?;
        }
        Ok(())
    }
}

/// Formats errors as plain text suitable for browser display.
pub fn format_errors_plain(errors: &[PlainError]) -> String {
    errors
        .iter()
        .map(|e| format!("{e}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ═══════════════════════════════════════════════════════════════════════
// Execution Result (S29.4)
// ═══════════════════════════════════════════════════════════════════════

/// Result of executing a Fajar Lang program in the Wasm sandbox.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Whether execution succeeded.
    pub success: bool,
    /// Return value (as string).
    pub result: Option<String>,
    /// Captured stdout.
    pub stdout: String,
    /// Error messages (plain text).
    pub errors: Vec<String>,
    /// Execution time in milliseconds.
    pub elapsed_ms: f64,
    /// Instructions executed.
    pub instructions: u64,
    /// Peak memory usage.
    pub peak_memory_bytes: usize,
}

impl ExecutionResult {
    /// Serializes to JSON for JavaScript consumption.
    pub fn to_json(&self) -> String {
        let result_json = self
            .result
            .as_ref()
            .map(|r| format!(r#""{}""#, r.replace('"', "\\\"")))
            .unwrap_or_else(|| "null".to_string());
        let errors_json: Vec<String> = self
            .errors
            .iter()
            .map(|e| format!(r#""{}""#, e.replace('"', "\\\"").replace('\n', "\\n")))
            .collect();
        format!(
            r#"{{"success":{},"result":{},"stdout":"{}","errors":[{}],"elapsed_ms":{},"instructions":{},"peak_memory_bytes":{}}}"#,
            self.success,
            result_json,
            self.stdout.replace('"', "\\\"").replace('\n', "\\n"),
            errors_json.join(","),
            self.elapsed_ms,
            self.instructions,
            self.peak_memory_bytes,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Wasm Bundle Metrics (S29.9)
// ═══════════════════════════════════════════════════════════════════════

/// Wasm bundle size metrics.
#[derive(Debug, Clone)]
pub struct BundleMetrics {
    /// Raw Wasm size in bytes.
    pub raw_size: usize,
    /// Optimized (wasm-opt) size in bytes.
    pub optimized_size: usize,
    /// Gzip-compressed size in bytes.
    pub compressed_size: usize,
    /// Brotli-compressed size in bytes.
    pub brotli_size: usize,
}

impl BundleMetrics {
    /// Returns the compression ratio (compressed/raw).
    pub fn compression_ratio(&self) -> f64 {
        if self.raw_size == 0 {
            return 0.0;
        }
        self.compressed_size as f64 / self.raw_size as f64
    }

    /// Checks if the bundle meets the size target.
    pub fn meets_target(&self, max_compressed_bytes: usize) -> bool {
        self.compressed_size <= max_compressed_bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S29.1: Wasm build target
    #[test]
    fn s29_1_wasm_build_config() {
        let cfg = WasmBuildConfig::default();
        assert_eq!(cfg.target, "wasm32-unknown-unknown");
        assert!(cfg.excluded_features.contains(&"native".to_string()));
        assert!(cfg.excluded_features.contains(&"llvm".to_string()));
        let args = cfg.cargo_args();
        assert!(args.contains(&"--target".to_string()));
        assert!(args.contains(&"wasm32-unknown-unknown".to_string()));
    }

    #[test]
    fn s29_1_wasm_opt_args() {
        let cfg = WasmBuildConfig::default();
        let args = cfg.wasm_opt_args("input.wasm", "output.wasm");
        assert!(args.contains(&"-Oz".to_string()));
        assert!(args.contains(&"input.wasm".to_string()));
    }

    // S29.2-S29.3: Wasm exports
    #[test]
    fn s29_2_wasm_exports_cover_all_apis() {
        let exports = wasm_exports();
        assert!(exports.len() >= 4);
        assert!(exports.iter().any(|e| e.js_name == "tokenize"));
        assert!(exports.iter().any(|e| e.js_name == "parse"));
        assert!(exports.iter().any(|e| e.js_name == "eval_source"));
        assert!(exports.iter().any(|e| e.js_name == "check"));
    }

    // S29.5: Memory sandbox
    #[test]
    fn s29_5_memory_limits_default() {
        let limits = MemoryLimits::default();
        assert_eq!(limits.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(limits.max_stack_depth, 256);
    }

    #[test]
    fn s29_5_memory_tracker_allocation() {
        let mut tracker = MemoryTracker::default();
        assert!(tracker.allocate(1024).is_ok());
        assert_eq!(tracker.allocated, 1024);
        assert_eq!(tracker.peak, 1024);

        tracker.free(512);
        assert_eq!(tracker.allocated, 512);
        assert_eq!(tracker.peak, 1024); // peak unchanged
    }

    #[test]
    fn s29_5_memory_tracker_oom() {
        let limits = MemoryLimits {
            max_memory_bytes: 1024,
            max_stack_depth: 64,
            max_heap_allocations: 10,
        };
        let mut tracker = MemoryTracker::new(limits);
        assert!(tracker.allocate(512).is_ok());
        assert!(tracker.allocate(600).is_err()); // would exceed 1024
    }

    #[test]
    fn s29_5_memory_usage_fraction() {
        let mut tracker = MemoryTracker::default();
        tracker.allocate(32 * 1024 * 1024).unwrap(); // 32MB of 64MB
        let frac = tracker.usage_fraction();
        assert!((frac - 0.5).abs() < 0.001);
    }

    // S29.6: Execution timeout
    #[test]
    fn s29_6_instruction_counter() {
        let limits = ExecutionLimits {
            max_instructions: 100,
            max_time_ms: 1000,
        };
        let mut counter = InstructionCounter::new(limits);
        for _ in 0..100 {
            assert!(counter.tick().is_ok());
        }
        assert!(counter.tick().is_err()); // 101st exceeds limit
    }

    #[test]
    fn s29_6_timeout_error_display() {
        let err = TimeoutError::InstructionLimit {
            count: 10_000_001,
            limit: 10_000_000,
        };
        let msg = format!("{err}");
        assert!(msg.contains("10000001"));
        assert!(msg.contains("10000000"));
    }

    #[test]
    fn s29_6_counter_reset() {
        let mut counter = InstructionCounter::default();
        counter.tick().unwrap();
        counter.tick().unwrap();
        assert_eq!(counter.count, 2);
        counter.reset();
        assert_eq!(counter.count, 0);
    }

    // S29.7: Print capture
    #[test]
    fn s29_7_print_capture() {
        capture_print("hello");
        capture_print("world");
        let output = drain_captured_output();
        assert_eq!(output, vec!["hello", "world"]);

        // After drain, should be empty
        let output2 = drain_captured_output();
        assert!(output2.is_empty());
    }

    #[test]
    fn s29_7_get_captured_stdout() {
        capture_print("line 1");
        capture_print("line 2");
        let stdout = get_captured_stdout();
        assert_eq!(stdout, "line 1\nline 2");
    }

    // S29.8: Error formatting
    #[test]
    fn s29_8_plain_error_format() {
        let err = PlainError {
            code: "SE004".to_string(),
            message: "type mismatch: expected i32, got str".to_string(),
            line: 5,
            column: 12,
            source_line: "let x: i32 = \"hello\"".to_string(),
            suggestion: Some("convert with to_int()".to_string()),
        };
        let formatted = format!("{err}");
        assert!(formatted.contains("SE004"));
        assert!(formatted.contains("type mismatch"));
        assert!(formatted.contains("line 5:12"));
        assert!(formatted.contains("to_int()"));
        assert!(!formatted.contains("\x1b")); // no ANSI codes
    }

    #[test]
    fn s29_8_format_multiple_errors() {
        let errors = vec![
            PlainError {
                code: "LE001".to_string(),
                message: "unexpected char".to_string(),
                line: 1,
                column: 1,
                source_line: "@!".to_string(),
                suggestion: None,
            },
            PlainError {
                code: "PE003".to_string(),
                message: "expected expression".to_string(),
                line: 2,
                column: 5,
                source_line: "let x =".to_string(),
                suggestion: Some("add a value".to_string()),
            },
        ];
        let output = format_errors_plain(&errors);
        assert!(output.contains("LE001"));
        assert!(output.contains("PE003"));
    }

    // S29.9: Bundle metrics
    #[test]
    fn s29_9_bundle_metrics() {
        let metrics = BundleMetrics {
            raw_size: 10_000_000,
            optimized_size: 7_000_000,
            compressed_size: 3_000_000,
            brotli_size: 2_500_000,
        };
        assert!((metrics.compression_ratio() - 0.3).abs() < 0.001);
        assert!(metrics.meets_target(5_000_000));
        assert!(!metrics.meets_target(2_000_000));
    }

    // S29.10: Execution result
    #[test]
    fn s29_10_execution_result_json() {
        let result = ExecutionResult {
            success: true,
            result: Some("42".to_string()),
            stdout: "hello world".to_string(),
            errors: Vec::new(),
            elapsed_ms: 12.5,
            instructions: 1000,
            peak_memory_bytes: 4096,
        };
        let json = result.to_json();
        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""result":"42""#));
        assert!(json.contains(r#""elapsed_ms":12.5"#));
    }

    #[test]
    fn s29_10_execution_result_error_json() {
        let result = ExecutionResult {
            success: false,
            result: None,
            stdout: String::new(),
            errors: vec!["type mismatch".to_string()],
            elapsed_ms: 0.5,
            instructions: 10,
            peak_memory_bytes: 1024,
        };
        let json = result.to_json();
        assert!(json.contains(r#""success":false"#));
        assert!(json.contains(r#""result":null"#));
        assert!(json.contains("type mismatch"));
    }

    // S29.1: Opt level display
    #[test]
    fn s29_1_opt_level_display() {
        assert_eq!(format!("{}", WasmOptLevel::Oz), "-Oz");
        assert_eq!(format!("{}", WasmOptLevel::O2), "-O2");
    }

    // S29.5: OOM display
    #[test]
    fn s29_5_oom_error_display() {
        let err = OomError {
            current_bytes: 60_000_000,
            max_bytes: 67_108_864,
            trigger: "allocation of 10000000 bytes".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("out of memory"));
    }

    // S29.6: Time limit error
    #[test]
    fn s29_6_time_limit_error() {
        let err = TimeoutError::TimeLimit {
            elapsed_ms: 5001,
            limit_ms: 5000,
        };
        assert_eq!(
            format!("{err}"),
            "execution timeout: 5001ms elapsed (limit: 5000ms)"
        );
    }
}
