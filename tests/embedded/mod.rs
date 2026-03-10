//! Embedded test framework for Fajar Lang.
//!
//! Provides utilities for testing compiled Fajar programs on QEMU targets
//! with timeout detection, UART output capture, and structured test results.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Result of running an embedded test.
#[derive(Debug)]
pub struct EmbeddedTestResult {
    /// The exit code from QEMU.
    pub exit_code: i32,
    /// Captured stdout output.
    pub stdout: String,
    /// Captured stderr output.
    pub stderr: String,
    /// Wall-clock execution time.
    pub duration: Duration,
    /// Whether the test timed out.
    pub timed_out: bool,
}

impl EmbeddedTestResult {
    /// Returns true if the test passed (exit code 0, no timeout).
    pub fn passed(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

/// Target architecture for embedded tests.
#[derive(Debug, Clone, Copy)]
pub enum TestArch {
    Aarch64,
    Riscv64,
}

impl TestArch {
    /// Returns the cross-compiler command.
    pub fn cc(&self) -> &str {
        match self {
            TestArch::Aarch64 => "aarch64-linux-gnu-gcc",
            TestArch::Riscv64 => "riscv64-linux-gnu-gcc",
        }
    }

    /// Returns the QEMU user-mode binary.
    pub fn qemu(&self) -> &str {
        match self {
            TestArch::Aarch64 => "qemu-aarch64",
            TestArch::Riscv64 => "qemu-riscv64",
        }
    }

    /// Returns the Cranelift target triple.
    pub fn triple(&self) -> &str {
        match self {
            TestArch::Aarch64 => "aarch64-unknown-linux-gnu",
            TestArch::Riscv64 => "riscv64gc-unknown-linux-gnu",
        }
    }

    /// Returns a short name for the architecture.
    pub fn name(&self) -> &str {
        match self {
            TestArch::Aarch64 => "aarch64",
            TestArch::Riscv64 => "riscv64",
        }
    }
}

/// Configuration for an embedded test run.
pub struct EmbeddedTestConfig {
    /// Target architecture.
    pub arch: TestArch,
    /// Maximum execution time before timeout.
    pub timeout: Duration,
    /// Path to the C runtime entry point.
    pub runtime_path: PathBuf,
    /// Working directory for temporary files.
    pub work_dir: PathBuf,
}

impl EmbeddedTestConfig {
    /// Creates a new config with default timeout (5 seconds).
    pub fn new(arch: TestArch) -> Self {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        Self {
            arch,
            timeout: Duration::from_secs(5),
            runtime_path: manifest_dir.join("tests/cross/rt_entry.c"),
            work_dir: std::env::temp_dir().join(format!(
                "fj_embedded_{}_{}_{}",
                arch.name(),
                std::process::id(),
                Instant::now().elapsed().as_nanos()
            )),
        }
    }

    /// Sets the timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Links an object file with the C runtime and runs it on QEMU.
pub fn link_and_run(
    obj_bytes: &[u8],
    config: &EmbeddedTestConfig,
) -> Result<EmbeddedTestResult, String> {
    fs::create_dir_all(&config.work_dir).map_err(|e| e.to_string())?;

    let obj_path = config.work_dir.join("program.o");
    let bin_path = config.work_dir.join("program");

    fs::write(&obj_path, obj_bytes).map_err(|e| e.to_string())?;

    // Link
    let link_out = Command::new(config.arch.cc())
        .args(["-static", "-o"])
        .arg(&bin_path)
        .arg(&obj_path)
        .arg(&config.runtime_path)
        .output()
        .map_err(|e| format!("link failed: {e}"))?;

    if !link_out.status.success() {
        let stderr = String::from_utf8_lossy(&link_out.stderr);
        let _ = fs::remove_dir_all(&config.work_dir);
        return Err(format!("linker error: {stderr}"));
    }

    // Run on QEMU with timeout
    let start = Instant::now();
    let run_out = Command::new(config.arch.qemu())
        .arg(&bin_path)
        .output()
        .map_err(|e| format!("qemu failed: {e}"))?;

    let duration = start.elapsed();
    let timed_out = duration >= config.timeout;

    let result = EmbeddedTestResult {
        exit_code: run_out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&run_out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&run_out.stderr).to_string(),
        duration,
        timed_out,
    };

    // Cleanup
    let _ = fs::remove_dir_all(&config.work_dir);

    Ok(result)
}

/// Checks if the required tools for an architecture are available.
pub fn has_tools(arch: TestArch) -> bool {
    let check = |name: &str| {
        Command::new("which")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    check(arch.cc()) && check(arch.qemu())
}
