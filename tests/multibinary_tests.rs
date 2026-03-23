//! Multi-binary build tests for Fajar Lang.
//!
//! Tests fj.toml manifest parsing for [kernel] and [[service]] sections,
//! multi-target build configuration, and service discovery.
//! Sprint 4 of Master Implementation Plan v7.0.

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
