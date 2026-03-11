//! Hardware CI simulation for embedded testing.
//!
//! Provides abstractions for flashing, running tests on physical boards
//! (or QEMU fallback), capturing serial output, and generating JUnit XML
//! reports. All operations are simulation stubs for testing CI pipelines.

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors from hardware CI operations.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum HardwareCiError {
    /// Flash operation failed.
    #[error("flash failed for board '{board}': {reason}")]
    FlashFailed {
        /// Target board identifier.
        board: String,
        /// Description of the failure.
        reason: String,
    },

    /// Test execution timed out.
    #[error("test timed out after {timeout_secs}s on '{board}'")]
    Timeout {
        /// Target board.
        board: String,
        /// Configured timeout.
        timeout_secs: u32,
    },

    /// Serial port error.
    #[error("serial error on {port}: {reason}")]
    SerialError {
        /// Serial port path.
        port: String,
        /// Description.
        reason: String,
    },

    /// QEMU execution failed.
    #[error("QEMU failed for machine '{machine}': {reason}")]
    QemuFailed {
        /// QEMU machine type.
        machine: String,
        /// Description.
        reason: String,
    },

    /// Board not available.
    #[error("board '{board}' not available")]
    BoardUnavailable {
        /// Board identifier.
        board: String,
    },

    /// Binary not found or invalid.
    #[error("invalid binary: {reason}")]
    InvalidBinary {
        /// Description.
        reason: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Probe types and CI configuration
// ═══════════════════════════════════════════════════════════════════════

/// Debug probe types supported for flashing and debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProbeType {
    /// STMicroelectronics ST-LINK probe.
    StLink,
    /// Segger J-Link probe.
    JLink,
    /// Raspberry Pi Picoprobe (SWD via RP2040).
    Picoprobe,
    /// Espressif esptool (UART-based flashing).
    Esptool,
}

impl ProbeType {
    /// Returns the CLI tool name used by this probe type.
    pub fn tool_name(&self) -> &'static str {
        match self {
            Self::StLink => "st-flash",
            Self::JLink => "JLinkExe",
            Self::Picoprobe => "openocd",
            Self::Esptool => "esptool.py",
        }
    }
}

/// Configuration for a hardware CI runner.
#[derive(Debug, Clone)]
pub struct HardwareCiConfig {
    /// CI runner label (e.g., "arm-runner-1").
    pub runner_label: String,
    /// Debug probe type.
    pub probe_type: ProbeType,
    /// Test execution timeout in seconds.
    pub timeout_secs: u32,
}

impl HardwareCiConfig {
    /// Creates a new CI configuration.
    pub fn new(runner_label: &str, probe_type: ProbeType, timeout_secs: u32) -> Self {
        Self {
            runner_label: runner_label.to_string(),
            probe_type,
            timeout_secs,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Test output and results
// ═══════════════════════════════════════════════════════════════════════

/// Output captured from a test run.
#[derive(Debug, Clone)]
pub struct TestOutput {
    /// Captured stdout text.
    pub stdout: String,
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// Duration of the run in milliseconds.
    pub duration_ms: u64,
}

/// Result of a single test case.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test name.
    pub name: String,
    /// Whether the test passed.
    pub passed: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Captured output (if any).
    pub output: Option<String>,
    /// Error message (if failed).
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Test runner
// ═══════════════════════════════════════════════════════════════════════

/// Runs tests on physical hardware (or simulation).
#[derive(Debug)]
pub struct TestRunner {
    /// CI configuration.
    config: HardwareCiConfig,
    /// Target board identifier.
    board: String,
    /// Accumulated test results.
    results: Vec<TestResult>,
}

impl TestRunner {
    /// Creates a new test runner for the given board.
    pub fn new(config: HardwareCiConfig, board: &str) -> Self {
        Self {
            config,
            board: board.to_string(),
            results: Vec::new(),
        }
    }

    /// Returns the target board name.
    pub fn board(&self) -> &str {
        &self.board
    }

    /// Returns accumulated test results.
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }

    /// Returns the CI configuration.
    pub fn config(&self) -> &HardwareCiConfig {
        &self.config
    }

    /// Simulates flashing a binary to the board and running it.
    ///
    /// In simulation, returns a successful `TestOutput` with the
    /// board name in stdout. Real implementations would invoke the
    /// debug probe CLI tool.
    pub fn flash_and_run(
        &mut self,
        binary_path: &str,
        board: &str,
    ) -> Result<TestOutput, HardwareCiError> {
        self.validate_binary_path(binary_path)?;

        // Simulation: produce synthetic output
        let output = TestOutput {
            stdout: format!(
                "[{}] Flashed {} to {} via {}\nAll tests passed.",
                self.config.runner_label,
                binary_path,
                board,
                self.config.probe_type.tool_name()
            ),
            exit_code: 0,
            duration_ms: 150, // simulated
        };

        self.results.push(TestResult {
            name: format!("flash_and_run:{board}"),
            passed: true,
            duration_ms: output.duration_ms,
            output: Some(output.stdout.clone()),
            error: None,
        });

        Ok(output)
    }

    /// Validates that the binary path is non-empty.
    fn validate_binary_path(&self, path: &str) -> Result<(), HardwareCiError> {
        if path.is_empty() {
            return Err(HardwareCiError::InvalidBinary {
                reason: "binary path is empty".to_string(),
            });
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Embedded test harness
// ═══════════════════════════════════════════════════════════════════════

/// Represents a single embedded test annotated with `#[embedded_test]`.
#[derive(Debug, Clone)]
pub struct EmbeddedTest {
    /// Human-readable test name.
    pub name: String,
    /// Function name in the source.
    pub function_name: String,
    /// Expected output for assertion (if any).
    pub expected_output: Option<String>,
}

/// A compiled test binary ready for flashing.
#[derive(Debug, Clone)]
pub struct TestBinary {
    /// Path to the binary file.
    pub path: String,
    /// Target triple (e.g., "thumbv7em-none-eabihf").
    pub target: String,
    /// Binary size in bytes.
    pub size_bytes: u64,
}

/// Generates a test binary from a list of embedded tests.
///
/// In simulation, returns a synthetic `TestBinary` with estimated size.
pub fn generate_test_binary(
    tests: &[EmbeddedTest],
    board: &str,
) -> Result<TestBinary, HardwareCiError> {
    if tests.is_empty() {
        return Err(HardwareCiError::InvalidBinary {
            reason: "no tests provided".to_string(),
        });
    }

    let target = board_to_target(board);
    let estimated_size = 4096 + (tests.len() as u64 * 256); // base + per-test

    Ok(TestBinary {
        path: format!("target/{}/release/test_harness", target),
        target,
        size_bytes: estimated_size,
    })
}

/// Maps a board name to a target triple.
fn board_to_target(board: &str) -> String {
    match board {
        "stm32f4" | "stm32f407" => "thumbv7em-none-eabihf".to_string(),
        "stm32h7" | "stm32h743" => "thumbv7em-none-eabihf".to_string(),
        "nrf52840" => "thumbv7em-none-eabihf".to_string(),
        "rp2040" => "thumbv6m-none-eabi".to_string(),
        "esp32" => "xtensa-esp32-none-elf".to_string(),
        _ => format!("unknown-{board}-none"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// QEMU fallback
// ═══════════════════════════════════════════════════════════════════════

/// QEMU configuration for hardware-less testing.
#[derive(Debug, Clone)]
pub struct QemuConfig {
    /// QEMU machine type (e.g., "lm3s6965evb").
    pub machine: String,
    /// CPU model (e.g., "cortex-m3").
    pub cpu: String,
    /// Memory size in MB.
    pub memory_mb: u32,
}

impl QemuConfig {
    /// Creates a new QEMU config.
    pub fn new(machine: &str, cpu: &str, memory_mb: u32) -> Self {
        Self {
            machine: machine.to_string(),
            cpu: cpu.to_string(),
            memory_mb,
        }
    }

    /// Returns the default QEMU config for Cortex-M testing.
    pub fn cortex_m_default() -> Self {
        Self::new("lm3s6965evb", "cortex-m3", 256)
    }
}

/// Detects whether a physical board is connected.
///
/// In simulation, always returns `None` (no physical board).
pub fn detect_physical_board() -> Option<String> {
    // Simulation stub — real implementation would probe USB/serial
    None
}

/// Runs a binary in QEMU.
///
/// In simulation, returns synthetic output indicating QEMU execution.
pub fn run_qemu(binary_path: &str, config: &QemuConfig) -> Result<TestOutput, HardwareCiError> {
    if binary_path.is_empty() {
        return Err(HardwareCiError::QemuFailed {
            machine: config.machine.clone(),
            reason: "binary path is empty".to_string(),
        });
    }

    Ok(TestOutput {
        stdout: format!(
            "QEMU: {} -cpu {} -m {}MB -kernel {}\nSimulation complete.",
            config.machine, config.cpu, config.memory_mb, binary_path
        ),
        exit_code: 0,
        duration_ms: 50,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Serial output capture
// ═══════════════════════════════════════════════════════════════════════

/// Captures serial output from a test run.
#[derive(Debug, Clone)]
pub struct SerialCapture {
    /// Serial port path (e.g., "/dev/ttyACM0").
    pub port: String,
    /// Baud rate.
    pub baud_rate: u32,
    /// Captured output buffer.
    buffer: String,
}

impl SerialCapture {
    /// Creates a new serial capture on the given port.
    pub fn new(port: &str, baud_rate: u32) -> Self {
        Self {
            port: port.to_string(),
            baud_rate,
            buffer: String::new(),
        }
    }

    /// Appends data to the capture buffer (simulation).
    pub fn feed(&mut self, data: &str) {
        self.buffer.push_str(data);
    }

    /// Returns the captured output.
    pub fn output(&self) -> &str {
        &self.buffer
    }

    /// Clears the capture buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Returns whether the buffer contains the expected string.
    pub fn contains(&self, expected: &str) -> bool {
        self.buffer.contains(expected)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Board matrix
// ═══════════════════════════════════════════════════════════════════════

/// A single board configuration in the test matrix.
#[derive(Debug, Clone)]
pub struct BoardConfig {
    /// Board identifier.
    pub name: String,
    /// Target triple.
    pub target: String,
    /// Whether the board is available for testing.
    pub available: bool,
}

/// A matrix of boards for CI testing.
#[derive(Debug)]
pub struct BoardMatrix {
    /// Board configurations.
    boards: Vec<BoardConfig>,
    /// Whether to skip unavailable boards instead of failing.
    pub skip_unavailable: bool,
}

impl BoardMatrix {
    /// Creates a new board matrix.
    pub fn new(skip_unavailable: bool) -> Self {
        Self {
            boards: Vec::new(),
            skip_unavailable,
        }
    }

    /// Adds a board to the matrix.
    pub fn add_board(&mut self, name: &str, target: &str, available: bool) {
        self.boards.push(BoardConfig {
            name: name.to_string(),
            target: target.to_string(),
            available,
        });
    }

    /// Returns all boards in the matrix.
    pub fn boards(&self) -> &[BoardConfig] {
        &self.boards
    }

    /// Returns only available boards.
    pub fn available_boards(&self) -> Vec<&BoardConfig> {
        self.boards.iter().filter(|b| b.available).collect()
    }

    /// Returns boards that should be tested (respects skip_unavailable).
    pub fn testable_boards(&self) -> Result<Vec<&BoardConfig>, HardwareCiError> {
        if self.skip_unavailable {
            Ok(self.available_boards())
        } else {
            // If not skipping, fail on first unavailable board
            for b in &self.boards {
                if !b.available {
                    return Err(HardwareCiError::BoardUnavailable {
                        board: b.name.clone(),
                    });
                }
            }
            Ok(self.boards.iter().collect())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// JUnit XML report
// ═══════════════════════════════════════════════════════════════════════

/// Generates a JUnit XML report from test results.
///
/// The output is compatible with CI systems that parse JUnit XML
/// (GitHub Actions, Jenkins, GitLab CI, etc.).
pub fn junit_xml_report(results: &[TestResult]) -> String {
    let total = results.len();
    let failures = results.iter().filter(|r| !r.passed).count();
    let total_time_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
    let total_time_s = total_time_ms as f64 / 1000.0;

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<testsuite name=\"embedded\" tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n",
        total, failures, total_time_s
    ));

    for result in results {
        append_test_case(&mut xml, result);
    }

    xml.push_str("</testsuite>\n");
    xml
}

/// Appends a single `<testcase>` element to the XML string.
fn append_test_case(xml: &mut String, result: &TestResult) {
    let time_s = result.duration_ms as f64 / 1000.0;
    xml.push_str(&format!(
        "  <testcase name=\"{}\" time=\"{:.3}\"",
        escape_xml(&result.name),
        time_s
    ));

    if result.passed && result.output.is_none() {
        xml.push_str(" />\n");
        return;
    }

    xml.push_str(">\n");

    if let Some(ref err) = result.error {
        xml.push_str(&format!(
            "    <failure message=\"{}\">{}</failure>\n",
            escape_xml(err),
            escape_xml(err)
        ));
    }

    if let Some(ref out) = result.output {
        xml.push_str(&format!(
            "    <system-out>{}</system-out>\n",
            escape_xml(out)
        ));
    }

    xml.push_str("  </testcase>\n");
}

/// Escapes special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ═══════════════════════════════════════════════════════════════════════
// Benchmark suite (Sprint 28)
// ═══════════════════════════════════════════════════════════════════════

/// Result of a single benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Benchmark name.
    pub name: String,
    /// Backend used (e.g., "cranelift", "wasm").
    pub backend: String,
    /// Number of iterations.
    pub iterations: u64,
    /// Mean time per iteration in nanoseconds.
    pub mean_ns: u64,
    /// Minimum time per iteration in nanoseconds.
    pub min_ns: u64,
    /// Maximum time per iteration in nanoseconds.
    pub max_ns: u64,
}

/// A suite of benchmarks for comparing backends.
#[derive(Debug)]
pub struct BenchmarkSuite {
    /// Accumulated results.
    results: Vec<BenchmarkResult>,
}

impl BenchmarkSuite {
    /// Creates a new empty benchmark suite.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Returns all accumulated results.
    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }

    /// Adds a result to the suite.
    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// Finds results for the given benchmark name.
    pub fn results_for(&self, name: &str) -> Vec<&BenchmarkResult> {
        self.results.iter().filter(|r| r.name == name).collect()
    }
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs a simulated benchmark suite comparing Wasm vs Cranelift.
///
/// Returns synthetic results for testing CI reporting pipelines.
pub fn run_benchmark_suite() -> Vec<BenchmarkResult> {
    vec![
        BenchmarkResult {
            name: "fibonacci_20".to_string(),
            backend: "cranelift".to_string(),
            iterations: 1000,
            mean_ns: 25_000,
            min_ns: 22_000,
            max_ns: 35_000,
        },
        BenchmarkResult {
            name: "fibonacci_20".to_string(),
            backend: "wasm".to_string(),
            iterations: 1000,
            mean_ns: 45_000,
            min_ns: 40_000,
            max_ns: 60_000,
        },
        BenchmarkResult {
            name: "loop_1000".to_string(),
            backend: "cranelift".to_string(),
            iterations: 1000,
            mean_ns: 5_000,
            min_ns: 4_500,
            max_ns: 8_000,
        },
        BenchmarkResult {
            name: "loop_1000".to_string(),
            backend: "wasm".to_string(),
            iterations: 1000,
            mean_ns: 12_000,
            min_ns: 10_000,
            max_ns: 18_000,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Regression test helpers (Sprint 28)
// ═══════════════════════════════════════════════════════════════════════

/// Verifies that the current test count meets or exceeds a baseline.
///
/// Returns `Ok(())` if `actual >= expected_min`, otherwise returns
/// an error describing the regression.
pub fn verify_test_baseline(actual: usize, expected_min: usize) -> Result<(), String> {
    if actual >= expected_min {
        Ok(())
    } else {
        Err(format!(
            "test count regression: expected >= {}, got {}",
            expected_min, actual
        ))
    }
}

/// Checks that a changelog string contains an entry for the given version.
pub fn verify_changelog_entry(changelog: &str, version: &str) -> Result<(), String> {
    if changelog.contains(version) {
        Ok(())
    } else {
        Err(format!("changelog missing entry for version {version}"))
    }
}

/// Checks that a Cargo.toml string contains the expected version.
pub fn verify_cargo_version(cargo_toml: &str, expected: &str) -> Result<(), String> {
    let needle = format!("version = \"{expected}\"");
    if cargo_toml.contains(&needle) {
        Ok(())
    } else {
        Err(format!(
            "Cargo.toml version mismatch: expected '{expected}'"
        ))
    }
}

/// Validates that a `.fj` source string contains valid-looking syntax.
///
/// This is a lightweight check: ensures the source is non-empty and
/// contains at least one `fn` definition.
pub fn validate_fj_example(source: &str) -> Result<(), String> {
    if source.trim().is_empty() {
        return Err("example source is empty".to_string());
    }
    if !source.contains("fn ") {
        return Err("example source contains no function definitions".to_string());
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S26.1: HardwareCiConfig and ProbeType
    #[test]
    fn ci_config_and_probe_types() {
        let config = HardwareCiConfig::new("arm-runner-1", ProbeType::StLink, 60);
        assert_eq!(config.runner_label, "arm-runner-1");
        assert_eq!(config.probe_type, ProbeType::StLink);
        assert_eq!(config.timeout_secs, 60);

        assert_eq!(ProbeType::StLink.tool_name(), "st-flash");
        assert_eq!(ProbeType::JLink.tool_name(), "JLinkExe");
        assert_eq!(ProbeType::Picoprobe.tool_name(), "openocd");
        assert_eq!(ProbeType::Esptool.tool_name(), "esptool.py");
    }

    // S26.2: TestRunner flash_and_run
    #[test]
    fn test_runner_flash_and_run() {
        let config = HardwareCiConfig::new("runner-1", ProbeType::JLink, 30);
        let mut runner = TestRunner::new(config, "stm32f4");

        let output = runner.flash_and_run("firmware.bin", "stm32f4").unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("stm32f4"));
        assert!(output.stdout.contains("JLinkExe"));
        assert_eq!(runner.results().len(), 1);
        assert!(runner.results()[0].passed);
    }

    // S26.3: TestRunner invalid binary
    #[test]
    fn test_runner_empty_binary_fails() {
        let config = HardwareCiConfig::new("runner-1", ProbeType::StLink, 30);
        let mut runner = TestRunner::new(config, "stm32f4");
        assert!(runner.flash_and_run("", "stm32f4").is_err());
    }

    // S26.4: Embedded test harness
    #[test]
    fn embedded_test_harness_generate_binary() {
        let tests = vec![
            EmbeddedTest {
                name: "led_blink".to_string(),
                function_name: "test_led_blink".to_string(),
                expected_output: Some("LED ON".to_string()),
            },
            EmbeddedTest {
                name: "uart_echo".to_string(),
                function_name: "test_uart_echo".to_string(),
                expected_output: None,
            },
        ];

        let binary = generate_test_binary(&tests, "stm32f4").unwrap();
        assert!(binary.path.contains("thumbv7em"));
        assert_eq!(binary.target, "thumbv7em-none-eabihf");
        assert!(binary.size_bytes > 4096);

        // Empty tests should fail
        assert!(generate_test_binary(&[], "stm32f4").is_err());
    }

    // S26.5: QEMU fallback
    #[test]
    fn qemu_fallback_execution() {
        let config = QemuConfig::cortex_m_default();
        assert_eq!(config.machine, "lm3s6965evb");
        assert_eq!(config.cpu, "cortex-m3");

        let output = run_qemu("test.elf", &config).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("QEMU"));
        assert!(output.stdout.contains("lm3s6965evb"));

        // Empty binary path fails
        assert!(run_qemu("", &config).is_err());
    }

    // S26.6: detect_physical_board returns None in simulation
    #[test]
    fn detect_physical_board_simulation() {
        assert!(detect_physical_board().is_none());
    }

    // S26.7: Serial output capture
    #[test]
    fn serial_capture_operations() {
        let mut cap = SerialCapture::new("/dev/ttyACM0", 115200);
        assert_eq!(cap.port, "/dev/ttyACM0");
        assert_eq!(cap.baud_rate, 115200);
        assert!(cap.output().is_empty());

        cap.feed("Hello ");
        cap.feed("World\n");
        assert!(cap.contains("Hello World"));
        assert_eq!(cap.output(), "Hello World\n");

        cap.clear();
        assert!(cap.output().is_empty());
    }

    // S26.8: Board matrix with skip
    #[test]
    fn board_matrix_skip_unavailable() {
        let mut matrix = BoardMatrix::new(true);
        matrix.add_board("stm32f4", "thumbv7em-none-eabihf", true);
        matrix.add_board("nrf52840", "thumbv7em-none-eabihf", false);
        matrix.add_board("rp2040", "thumbv6m-none-eabi", true);

        assert_eq!(matrix.boards().len(), 3);
        assert_eq!(matrix.available_boards().len(), 2);

        // skip_unavailable = true: returns only available
        let testable = matrix.testable_boards().unwrap();
        assert_eq!(testable.len(), 2);
    }

    // S26.9: Board matrix fail on unavailable
    #[test]
    fn board_matrix_fail_unavailable() {
        let mut matrix = BoardMatrix::new(false);
        matrix.add_board("stm32f4", "thumbv7em-none-eabihf", true);
        matrix.add_board("nrf52840", "thumbv7em-none-eabihf", false);

        assert!(matrix.testable_boards().is_err());
    }

    // S26.10: JUnit XML report generation
    #[test]
    fn junit_xml_report_generation() {
        let results = vec![
            TestResult {
                name: "test_led".to_string(),
                passed: true,
                duration_ms: 100,
                output: Some("LED OK".to_string()),
                error: None,
            },
            TestResult {
                name: "test_uart".to_string(),
                passed: false,
                duration_ms: 200,
                output: None,
                error: Some("timeout waiting for response".to_string()),
            },
        ];

        let xml = junit_xml_report(&results);
        assert!(xml.contains("<?xml version=\"1.0\""));
        assert!(xml.contains("tests=\"2\""));
        assert!(xml.contains("failures=\"1\""));
        assert!(xml.contains("test_led"));
        assert!(xml.contains("test_uart"));
        assert!(xml.contains("<failure"));
        assert!(xml.contains("LED OK"));
        assert!(xml.contains("timeout waiting for response"));
    }

    // ── Sprint 28: Release Preparation ────────────────────────────────

    // S28.1: Benchmark suite runs and returns results
    #[test]
    fn benchmark_suite_returns_results() {
        let results = run_benchmark_suite();
        assert_eq!(results.len(), 4);

        // Check both backends present for fibonacci_20
        let fib_results: Vec<_> = results
            .iter()
            .filter(|r| r.name == "fibonacci_20")
            .collect();
        assert_eq!(fib_results.len(), 2);
        assert!(fib_results.iter().any(|r| r.backend == "cranelift"));
        assert!(fib_results.iter().any(|r| r.backend == "wasm"));
    }

    // S28.2: BenchmarkSuite struct accumulation
    #[test]
    fn benchmark_suite_struct_operations() {
        let mut suite = BenchmarkSuite::new();
        assert!(suite.results().is_empty());

        for result in run_benchmark_suite() {
            suite.add_result(result);
        }
        assert_eq!(suite.results().len(), 4);

        let fib = suite.results_for("fibonacci_20");
        assert_eq!(fib.len(), 2);
        let loop_results = suite.results_for("loop_1000");
        assert_eq!(loop_results.len(), 2);
    }

    // S28.3: Cranelift faster than Wasm (simulation invariant)
    #[test]
    fn benchmark_cranelift_faster_than_wasm() {
        let results = run_benchmark_suite();
        let fib_cranelift = results
            .iter()
            .find(|r| r.name == "fibonacci_20" && r.backend == "cranelift")
            .unwrap();
        let fib_wasm = results
            .iter()
            .find(|r| r.name == "fibonacci_20" && r.backend == "wasm")
            .unwrap();
        assert!(fib_cranelift.mean_ns < fib_wasm.mean_ns);
    }

    // S28.4: Benchmark result min <= mean <= max
    #[test]
    fn benchmark_result_min_mean_max_ordering() {
        for result in run_benchmark_suite() {
            assert!(
                result.min_ns <= result.mean_ns,
                "{}: min {} > mean {}",
                result.name,
                result.min_ns,
                result.mean_ns
            );
            assert!(
                result.mean_ns <= result.max_ns,
                "{}: mean {} > max {}",
                result.name,
                result.mean_ns,
                result.max_ns
            );
        }
    }

    // S28.5: Test baseline regression check passes
    #[test]
    fn verify_test_baseline_passes() {
        assert!(verify_test_baseline(2500, 2430).is_ok());
        assert!(verify_test_baseline(2430, 2430).is_ok());
    }

    // S28.6: Test baseline regression check fails
    #[test]
    fn verify_test_baseline_fails_on_regression() {
        let result = verify_test_baseline(2000, 2430);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("regression"));
    }

    // S28.7: Changelog entry verification
    #[test]
    fn verify_changelog_entry_present() {
        let changelog = "## [0.7.0] — 2026-03-11\n### Added\n- Power management\n";
        assert!(verify_changelog_entry(changelog, "0.7.0").is_ok());
        assert!(verify_changelog_entry(changelog, "0.8.0").is_err());
    }

    // S28.8: Cargo.toml version verification
    #[test]
    fn verify_cargo_version_check() {
        let cargo = "[package]\nname = \"fajar-lang\"\nversion = \"0.3.0\"\n";
        assert!(verify_cargo_version(cargo, "0.3.0").is_ok());
        assert!(verify_cargo_version(cargo, "0.7.0").is_err());
    }

    // S28.9: Example .fj syntax validation
    #[test]
    fn validate_fj_example_syntax() {
        let good = "fn main() -> i64 {\n    println(\"hello\")\n    0\n}\n";
        assert!(validate_fj_example(good).is_ok());

        // Empty source
        assert!(validate_fj_example("").is_err());
        assert!(validate_fj_example("   ").is_err());

        // No function definition
        assert!(validate_fj_example("let x = 42").is_err());
    }

    // S28.10: RTIC blinky example is valid
    #[test]
    fn rtic_blinky_example_is_valid() {
        // Validate the rtic_blinky.fj example content structure
        let source = include_str!("../../../examples/rtic_blinky.fj");
        assert!(validate_fj_example(source).is_ok());
        assert!(source.contains("fn main"));
        assert!(source.contains("LED"));
    }
}
