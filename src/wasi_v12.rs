//! V12 WASI Deployment — WebAssembly System Interface.
//!
//! Types and infrastructure for WASI Preview 1/2 compilation targets:
//! - W1-W2: WASI P1 completeness + P2 component model
//! - W3-W5: Component composition, resource types, async WASI
//! - W6-W7: WASI-nn for ML, browser target
//! - W8-W10: Edge deployment, size optimization, production verification

use std::fmt;
use std::path::PathBuf;

// ═══════════════════════════════════════════════════════════════════════
// W1: WASI Preview 1 Types
// ═══════════════════════════════════════════════════════════════════════

/// WASI system call categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasiSyscall {
    /// `args_get` / `args_sizes_get`
    Args,
    /// `environ_get` / `environ_sizes_get`
    Environ,
    /// `fd_read` / `fd_write` / `fd_seek` / `fd_close`
    FileDescriptor,
    /// `path_open` / `path_create_directory` / `path_remove_directory`
    Path,
    /// `clock_time_get`
    Clock,
    /// `random_get`
    Random,
    /// `proc_exit`
    Process,
}

impl fmt::Display for WasiSyscall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasiSyscall::Args => write!(f, "args"),
            WasiSyscall::Environ => write!(f, "environ"),
            WasiSyscall::FileDescriptor => write!(f, "fd"),
            WasiSyscall::Path => write!(f, "path"),
            WasiSyscall::Clock => write!(f, "clock"),
            WasiSyscall::Random => write!(f, "random"),
            WasiSyscall::Process => write!(f, "proc"),
        }
    }
}

/// WASI import function specification.
#[derive(Debug, Clone)]
pub struct WasiImport {
    /// Module name (e.g., "wasi_snapshot_preview1").
    pub module: String,
    /// Function name (e.g., "fd_write").
    pub name: String,
    /// Parameter types.
    pub params: Vec<WasmType>,
    /// Return type.
    pub result: Option<WasmType>,
    /// Syscall category.
    pub category: WasiSyscall,
}

/// WebAssembly value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

impl fmt::Display for WasmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasmType::I32 => write!(f, "i32"),
            WasmType::I64 => write!(f, "i64"),
            WasmType::F32 => write!(f, "f32"),
            WasmType::F64 => write!(f, "f64"),
        }
    }
}

/// Returns all WASI Preview 1 import specifications.
pub fn wasi_preview1_imports() -> Vec<WasiImport> {
    vec![
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_write".into(),
            params: vec![WasmType::I32, WasmType::I32, WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::FileDescriptor,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_read".into(),
            params: vec![WasmType::I32, WasmType::I32, WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::FileDescriptor,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "proc_exit".into(),
            params: vec![WasmType::I32],
            result: None,
            category: WasiSyscall::Process,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "clock_time_get".into(),
            params: vec![WasmType::I32, WasmType::I64, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::Clock,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "args_get".into(),
            params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::Args,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "args_sizes_get".into(),
            params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::Args,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "environ_get".into(),
            params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::Environ,
        },
        WasiImport {
            module: "wasi_snapshot_preview1".into(),
            name: "random_get".into(),
            params: vec![WasmType::I32, WasmType::I32],
            result: Some(WasmType::I32),
            category: WasiSyscall::Random,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// W2: WASI Preview 2 — Component Model
// ═══════════════════════════════════════════════════════════════════════

/// WIT (WebAssembly Interface Types) interface definition.
#[derive(Debug, Clone)]
pub struct WitInterface {
    /// Interface name (e.g., "wasi:io/streams").
    pub name: String,
    /// Functions in this interface.
    pub functions: Vec<WitFunction>,
    /// Types defined in this interface.
    pub types: Vec<WitType>,
}

/// A function in a WIT interface.
#[derive(Debug, Clone)]
pub struct WitFunction {
    /// Function name.
    pub name: String,
    /// Parameter names and types.
    pub params: Vec<(String, String)>,
    /// Result type (if any).
    pub result: Option<String>,
}

/// A type in a WIT interface.
#[derive(Debug, Clone)]
pub struct WitType {
    /// Type name.
    pub name: String,
    /// Type kind: "record", "enum", "variant", "flags", "resource".
    pub kind: String,
}

/// A WASI component world definition.
#[derive(Debug, Clone)]
pub struct ComponentWorld {
    /// World name (e.g., "wasi:cli/command").
    pub name: String,
    /// Imported interfaces.
    pub imports: Vec<String>,
    /// Exported interfaces.
    pub exports: Vec<String>,
}

/// Returns the standard WASI CLI command world.
pub fn wasi_cli_command_world() -> ComponentWorld {
    ComponentWorld {
        name: "wasi:cli/command".into(),
        imports: vec![
            "wasi:io/streams".into(),
            "wasi:filesystem/types".into(),
            "wasi:cli/stdin".into(),
            "wasi:cli/stdout".into(),
            "wasi:cli/stderr".into(),
            "wasi:clocks/monotonic-clock".into(),
            "wasi:random/random".into(),
        ],
        exports: vec!["wasi:cli/run".into()],
    }
}

/// Returns the WASI HTTP proxy world.
pub fn wasi_http_proxy_world() -> ComponentWorld {
    ComponentWorld {
        name: "wasi:http/proxy".into(),
        imports: vec![
            "wasi:http/types".into(),
            "wasi:http/outgoing-handler".into(),
            "wasi:io/streams".into(),
        ],
        exports: vec!["wasi:http/incoming-handler".into()],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W8-W10: Deployment & Size Optimization
// ═══════════════════════════════════════════════════════════════════════

/// WASM build configuration for deployment.
#[derive(Debug, Clone)]
pub struct WasmBuildConfig {
    /// Target: "wasm32-unknown-unknown" or "wasm32-wasi".
    pub target: String,
    /// Optimization level: "O0", "Os", "Oz".
    pub opt_level: String,
    /// Whether to run wasm-opt for additional size reduction.
    pub wasm_opt: bool,
    /// Maximum memory pages (64KB each).
    pub max_memory_pages: u32,
    /// Whether to strip debug info.
    pub strip_debug: bool,
    /// Output path.
    pub output: PathBuf,
}

impl Default for WasmBuildConfig {
    fn default() -> Self {
        Self {
            target: "wasm32-wasi".into(),
            opt_level: "Oz".into(),
            wasm_opt: true,
            max_memory_pages: 1024, // 64 MB
            strip_debug: true,
            output: PathBuf::from("output.wasm"),
        }
    }
}

impl WasmBuildConfig {
    /// Returns the expected output size category.
    pub fn size_category(&self) -> &'static str {
        match self.opt_level.as_str() {
            "Oz" | "Os" => "small (<1MB)",
            "O2" => "medium (1-5MB)",
            _ => "large (>5MB)",
        }
    }

    /// Maximum memory in bytes.
    pub fn max_memory_bytes(&self) -> u64 {
        self.max_memory_pages as u64 * 65536
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w1_wasi_preview1_imports_count() {
        let imports = wasi_preview1_imports();
        assert!(imports.len() >= 8, "should have 8+ WASI P1 imports");
        assert!(imports.iter().any(|i| i.name == "fd_write"));
        assert!(imports.iter().any(|i| i.name == "proc_exit"));
        assert!(imports.iter().any(|i| i.name == "random_get"));
    }

    #[test]
    fn w1_wasi_import_module() {
        let imports = wasi_preview1_imports();
        for import in &imports {
            assert_eq!(import.module, "wasi_snapshot_preview1");
        }
    }

    #[test]
    fn w1_wasm_type_display() {
        assert_eq!(format!("{}", WasmType::I32), "i32");
        assert_eq!(format!("{}", WasmType::F64), "f64");
    }

    #[test]
    fn w1_syscall_categories() {
        let imports = wasi_preview1_imports();
        let fd_imports: Vec<_> = imports
            .iter()
            .filter(|i| i.category == WasiSyscall::FileDescriptor)
            .collect();
        assert!(fd_imports.len() >= 2);
    }

    #[test]
    fn w2_cli_command_world() {
        let world = wasi_cli_command_world();
        assert_eq!(world.name, "wasi:cli/command");
        assert!(!world.imports.is_empty());
        assert!(world.imports.contains(&"wasi:io/streams".to_string()));
        assert!(world.exports.contains(&"wasi:cli/run".to_string()));
    }

    #[test]
    fn w2_http_proxy_world() {
        let world = wasi_http_proxy_world();
        assert_eq!(world.name, "wasi:http/proxy");
        assert!(world.imports.contains(&"wasi:http/types".to_string()));
        assert!(
            world
                .exports
                .contains(&"wasi:http/incoming-handler".to_string())
        );
    }

    #[test]
    fn w2_wit_interface() {
        let iface = WitInterface {
            name: "wasi:io/streams".into(),
            functions: vec![WitFunction {
                name: "read".into(),
                params: vec![("len".into(), "u64".into())],
                result: Some("list<u8>".into()),
            }],
            types: vec![WitType {
                name: "input-stream".into(),
                kind: "resource".into(),
            }],
        };
        assert_eq!(iface.functions.len(), 1);
        assert_eq!(iface.types[0].kind, "resource");
    }

    #[test]
    fn w8_wasm_build_config_default() {
        let config = WasmBuildConfig::default();
        assert_eq!(config.target, "wasm32-wasi");
        assert_eq!(config.opt_level, "Oz");
        assert!(config.wasm_opt);
        assert!(config.strip_debug);
    }

    #[test]
    fn w8_size_category() {
        let mut config = WasmBuildConfig::default();
        assert_eq!(config.size_category(), "small (<1MB)");
        config.opt_level = "O2".into();
        assert_eq!(config.size_category(), "medium (1-5MB)");
        config.opt_level = "O0".into();
        assert_eq!(config.size_category(), "large (>5MB)");
    }

    #[test]
    fn w8_max_memory() {
        let config = WasmBuildConfig::default();
        assert_eq!(config.max_memory_bytes(), 1024 * 65536); // 64MB
    }

    #[test]
    fn w1_syscall_display() {
        assert_eq!(format!("{}", WasiSyscall::Args), "args");
        assert_eq!(format!("{}", WasiSyscall::FileDescriptor), "fd");
        assert_eq!(format!("{}", WasiSyscall::Process), "proc");
    }

    #[test]
    fn w2_component_world_fields() {
        let world = ComponentWorld {
            name: "custom".into(),
            imports: vec!["a".into()],
            exports: vec!["b".into()],
        };
        assert_eq!(world.name, "custom");
        assert_eq!(world.imports.len(), 1);
        assert_eq!(world.exports.len(), 1);
    }
}
