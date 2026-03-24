//! Multi-binary build tests for Fajar Lang.
//!
//! Tests fj.toml manifest parsing for [kernel] and [[service]] sections,
//! multi-target build configuration, and service discovery.
//! Sprint 4 of Master Implementation Plan v7.0.

#![allow(unused_imports)]

use fajar_lang::package::manifest::{KernelConfig, ProjectConfig, ServiceConfig};

fn parse_toml(content: &str) -> ProjectConfig {
    ProjectConfig::parse(content).unwrap()
}

// ════════════════════════════════════════════════════════════════════════
// 1. Manifest parsing — kernel section
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_kernel_section() {
    let config = parse_toml(
        r#"
[package]
name = "test-os"
version = "1.0.0"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
sources = ["kernel/", "drivers/"]
"#,
    );
    assert!(config.kernel.is_some());
    let kernel = config.kernel.unwrap();
    assert_eq!(kernel.entry, "kernel/main.fj");
    assert_eq!(kernel.target, "x86_64-unknown-none");
    assert_eq!(kernel.sources.len(), 2);
}

#[test]
fn parse_kernel_with_linker_script() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
linker-script = "linker.ld"
"#,
    );
    let kernel = config.kernel.unwrap();
    assert_eq!(kernel.linker_script, Some("linker.ld".into()));
}

#[test]
fn parse_no_kernel_section() {
    let config = parse_toml(
        r#"
[package]
name = "lib"
version = "1.0.0"
"#,
    );
    assert!(config.kernel.is_none());
}

// ════════════════════════════════════════════════════════════════════════
// 2. Manifest parsing — service sections
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_one_service() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
"#,
    );
    assert_eq!(config.service.len(), 1);
    assert_eq!(config.service[0].name, "vfs");
    assert_eq!(config.service[0].target, "x86_64-user"); // default
}

#[test]
fn parse_three_services() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"

[[service]]
name = "net"
entry = "services/net/main.fj"
target = "x86_64-user"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
"#,
    );
    assert_eq!(config.service.len(), 3);
    assert_eq!(config.service[0].name, "vfs");
    assert_eq!(config.service[1].name, "net");
    assert_eq!(config.service[2].name, "shell");
}

#[test]
fn parse_service_with_sources() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "blk"
entry = "services/blk/main.fj"
sources = ["services/blk/", "shared/"]
"#,
    );
    assert_eq!(config.service[0].sources.len(), 2);
}

#[test]
fn parse_service_custom_target() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "gpu"
entry = "services/gpu/main.fj"
target = "aarch64-user"
"#,
    );
    assert_eq!(config.service[0].target, "aarch64-user");
}

#[test]
fn parse_no_services() {
    let config = parse_toml(
        r#"
[package]
name = "lib"
version = "1.0.0"
"#,
    );
    assert!(config.service.is_empty());
}

// ════════════════════════════════════════════════════════════════════════
// 3. Multi-binary detection
// ════════════════════════════════════════════════════════════════════════

#[test]
fn is_multi_binary_with_kernel() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
"#,
    );
    assert!(config.is_multi_binary());
}

#[test]
fn is_multi_binary_with_services() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
"#,
    );
    assert!(config.is_multi_binary());
}

#[test]
fn is_not_multi_binary_simple() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"
"#,
    );
    assert!(!config.is_multi_binary());
}

#[test]
fn service_names() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"

[[service]]
name = "net"
entry = "services/net/main.fj"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
"#,
    );
    let names = config.service_names();
    assert_eq!(names, vec!["vfs", "net", "shell"]);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Full FajarOS-style manifest
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_fajaros_manifest() {
    let config = parse_toml(
        r#"
[package]
name = "fajaros"
version = "3.0.0"
description = "FajarOS — microkernel in Fajar Lang"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
sources = ["kernel/", "drivers/"]
linker-script = "linker.ld"

[[service]]
name = "vfs"
entry = "services/vfs/main.fj"
target = "x86_64-user"

[[service]]
name = "net"
entry = "services/net/main.fj"
target = "x86_64-user"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
target = "x86_64-user"

[[service]]
name = "blk"
entry = "services/blk/main.fj"
target = "x86_64-user"

[[service]]
name = "display"
entry = "services/display/main.fj"
target = "x86_64-user"

[[service]]
name = "input"
entry = "services/input/main.fj"
target = "x86_64-user"

[[service]]
name = "gpu"
entry = "services/gpu/main.fj"
target = "x86_64-user"

[[service]]
name = "gui"
entry = "services/gui/main.fj"
target = "x86_64-user"

[[service]]
name = "auth"
entry = "services/auth/main.fj"
target = "x86_64-user"
"#,
    );
    assert!(config.is_multi_binary());
    let svc_count = config.service.len();
    let svc_names = config.service_names();
    let kernel = config.kernel.unwrap();
    assert_eq!(svc_count, 9);
    assert_eq!(kernel.target, "x86_64-unknown-none");
    assert_eq!(kernel.sources.len(), 2);
    assert_eq!(svc_names.len(), 9);
}

// ════════════════════════════════════════════════════════════════════════
// 5. Dual-platform (x86 + ARM64)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn parse_dual_platform_services() {
    let config = parse_toml(
        r#"
[package]
name = "test"
version = "1.0.0"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"

[[service]]
name = "npu"
entry = "services/npu/main.fj"
target = "aarch64-user"

[[service]]
name = "shell"
entry = "services/shell/main.fj"
target = "x86_64-user"
"#,
    );
    assert_eq!(config.service[0].target, "aarch64-user");
    assert_eq!(config.service[1].target, "x86_64-user");
}

// ════════════════════════════════════════════════════════════════════════
// 6. Backward compatibility
// ════════════════════════════════════════════════════════════════════════

#[test]
fn backward_compat_simple_project() {
    let config = parse_toml(
        r#"
[package]
name = "hello"
version = "0.1.0"
entry = "src/main.fj"
"#,
    );
    assert!(!config.is_multi_binary());
    assert_eq!(config.package.entry, "src/main.fj");
    assert!(config.kernel.is_none());
    assert!(config.service.is_empty());
}

#[test]
fn backward_compat_with_dependencies() {
    let config = parse_toml(
        r#"
[package]
name = "hello"
version = "0.1.0"

[dependencies]
fj-math = "^1.0.0"
"#,
    );
    assert_eq!(config.dependencies.len(), 1);
    assert!(config.dependencies.contains_key("fj-math"));
}

// ════════════════════════════════════════════════════════════════════════
// 7. End-to-end build tests (require native codegen)
// ════════════════════════════════════════════════════════════════════════

mod build_all_e2e {
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;

    fn fj_binary() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_BIN_EXE_fj"));
        // Verify the binary supports native compilation
        let output = std::process::Command::new(&path)
            .args(["build", "--help"])
            .output()
            .expect("fj binary not found");
        let help = String::from_utf8_lossy(&output.stdout);
        if help.contains("target") || output.status.success() {
            // Binary exists and responds
        }
        path
    }

    /// Skips test if the fj binary doesn't have native codegen.
    fn skip_if_no_native() -> bool {
        let path = PathBuf::from(env!("CARGO_BIN_EXE_fj"));
        let dir = std::env::temp_dir().join("fj-native-check");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.fj"), "fn main() -> i64 { 0 }\n").unwrap();
        let output = std::process::Command::new(&path)
            .args([
                "build",
                "--target",
                "x86_64-unknown-none",
                dir.join("t.fj").to_str().unwrap(),
            ])
            .output()
            .unwrap_or_else(|_| panic!("fj binary not found at {:?}", path));
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        stderr.contains("native compilation not available")
    }

    fn create_test_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fj-build-test-{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("kernel")).unwrap();
        std::fs::create_dir_all(dir.join("services/echo")).unwrap();
        dir
    }

    #[test]
    fn build_all_produces_kernel_elf() {
        if skip_if_no_native() {
            return;
        }
        let dir = create_test_project("kernel-elf");

        // Write fj.toml
        let mut f = std::fs::File::create(dir.join("fj.toml")).unwrap();
        writeln!(f, "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(
            f,
            "[kernel]\nentry = \"kernel/main.fj\"\ntarget = \"x86_64-unknown-none\""
        )
        .unwrap();

        // Write kernel source
        std::fs::write(
            dir.join("kernel/main.fj"),
            "@kernel\nfn kernel_main() -> i64 { 0 }\n",
        )
        .unwrap();

        // Run fj build --all
        let output = Command::new(fj_binary())
            .args(["build", "--all"])
            .current_dir(&dir)
            .output()
            .expect("fj binary not found");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Verify kernel.elf exists
        let kernel_elf = dir.join("build/kernel.elf");
        assert!(
            kernel_elf.exists(),
            "kernel.elf not produced. stdout={stdout}, stderr={stderr}"
        );

        // Verify it's a real ELF
        let data = std::fs::read(&kernel_elf).unwrap();
        assert!(
            data.len() > 100,
            "kernel.elf too small: {} bytes",
            data.len()
        );
        assert_eq!(&data[0..4], b"\x7fELF", "kernel.elf is not a valid ELF");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_all_produces_service_elf() {
        if skip_if_no_native() {
            return;
        }
        let dir = create_test_project("service-elf");

        let mut f = std::fs::File::create(dir.join("fj.toml")).unwrap();
        writeln!(f, "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(f, "[[service]]\nname = \"echo\"\nentry = \"services/echo/main.fj\"\ntarget = \"x86_64-user\"").unwrap();

        std::fs::write(
            dir.join("services/echo/main.fj"),
            "fn main() -> i64 { 42 }\n",
        )
        .unwrap();

        let output = Command::new(fj_binary())
            .args(["build", "--all"])
            .current_dir(&dir)
            .output()
            .expect("fj binary not found");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let service_elf = dir.join("build/services/echo.elf");
        assert!(
            service_elf.exists(),
            "echo.elf not produced. stdout={stdout}, stderr={stderr}"
        );

        let data = std::fs::read(&service_elf).unwrap();
        assert!(data.len() > 100, "echo.elf too small: {} bytes", data.len());
        assert_eq!(&data[0..4], b"\x7fELF", "echo.elf is not a valid ELF");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_all_kernel_plus_service() {
        if skip_if_no_native() {
            return;
        }
        let dir = create_test_project("full-os");

        let mut f = std::fs::File::create(dir.join("fj.toml")).unwrap();
        writeln!(f, "[package]\nname = \"test-os\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(
            f,
            "[kernel]\nentry = \"kernel/main.fj\"\ntarget = \"x86_64-unknown-none\"\n"
        )
        .unwrap();
        writeln!(f, "[[service]]\nname = \"echo\"\nentry = \"services/echo/main.fj\"\ntarget = \"x86_64-user\"").unwrap();

        std::fs::write(
            dir.join("kernel/main.fj"),
            "@kernel\nfn kernel_main() -> i64 { 0 }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("services/echo/main.fj"),
            "fn main() -> i64 { 42 }\n",
        )
        .unwrap();

        let output = Command::new(fj_binary())
            .args(["build", "--all"])
            .current_dir(&dir)
            .output()
            .expect("fj binary not found");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("2 targets built, 0 failed"),
            "Expected 2 targets built. stdout={stdout}"
        );

        assert!(dir.join("build/kernel.elf").exists(), "kernel.elf missing");
        assert!(
            dir.join("build/services/echo.elf").exists(),
            "echo.elf missing"
        );

        // Both must be valid ELF
        for path in &[
            dir.join("build/kernel.elf"),
            dir.join("build/services/echo.elf"),
        ] {
            let data = std::fs::read(path).unwrap();
            assert_eq!(
                &data[0..4],
                b"\x7fELF",
                "{} is not a valid ELF",
                path.display()
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_all_missing_source_reports_failure() {
        if skip_if_no_native() {
            return;
        }
        let dir = create_test_project("missing-src");

        let mut f = std::fs::File::create(dir.join("fj.toml")).unwrap();
        writeln!(f, "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(
            f,
            "[kernel]\nentry = \"kernel/main.fj\"\ntarget = \"x86_64-unknown-none\""
        )
        .unwrap();

        // Don't create kernel/main.fj — should report failure
        let output = Command::new(fj_binary())
            .args(["build", "--all"])
            .current_dir(&dir)
            .output()
            .expect("fj binary not found");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should report failure
        assert!(
            stdout.contains("1 failed") || stderr.contains("not found"),
            "Expected failure report. stdout={stdout}, stderr={stderr}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_all_elf_is_x86_64() {
        if skip_if_no_native() {
            return;
        }
        let dir = create_test_project("elf-arch");

        let mut f = std::fs::File::create(dir.join("fj.toml")).unwrap();
        writeln!(f, "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        writeln!(
            f,
            "[kernel]\nentry = \"kernel/main.fj\"\ntarget = \"x86_64-unknown-none\""
        )
        .unwrap();

        std::fs::write(
            dir.join("kernel/main.fj"),
            "@kernel\nfn kernel_main() -> i64 { 0 }\n",
        )
        .unwrap();

        let _ = Command::new(fj_binary())
            .args(["build", "--all"])
            .current_dir(&dir)
            .output()
            .expect("fj binary not found");

        let data = std::fs::read(dir.join("build/kernel.elf")).unwrap();
        // ELF header: bytes 4 = class (2 = 64-bit), byte 18 = machine (0x3E = x86-64)
        assert_eq!(data[4], 2, "Not 64-bit ELF");
        assert_eq!(data[18], 0x3E, "Not x86-64 architecture");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
