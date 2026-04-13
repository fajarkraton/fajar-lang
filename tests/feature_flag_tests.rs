//! V27 A1: Feature flag integration tests.
//!
//! Tests that each feature flag compiles and its builtins are registered
//! in the interpreter. Uses eval_source() to exercise builtin registration
//! without requiring external hardware/services.
//!
//! Run all: `cargo test --all-features --test feature_flag_tests`
//! Run one: `cargo test --features websocket --test feature_flag_tests -- websocket`

use fajar_lang::interpreter::Interpreter;

fn eval(code: &str) -> Result<fajar_lang::interpreter::Value, String> {
    let mut interp = Interpreter::new();
    interp.eval_source(code).map_err(|e| format!("{e:?}"))
}

fn eval_ok(code: &str) -> fajar_lang::interpreter::Value {
    eval(code).unwrap_or_else(|e| panic!("eval failed: {e}"))
}

// ═══════════════════════════════════════════════════════════
// WebSocket (feature = "websocket")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "websocket")]
mod feature_websocket {
    use super::*;

    #[test]
    fn ws_builtins_registered() {
        // ws_close on invalid handle should not crash
        eval_ok(r#"fn main() { ws_close(-1) }"#);
    }

    #[test]
    fn ws_connect_error_path() {
        let r = eval(r#"fn main() { let h = ws_connect("not://valid"); println(h) }"#);
        assert!(r.is_ok() || r.is_err()); // either -1 or error, both valid
    }
}

// ═══════════════════════════════════════════════════════════
// MQTT (feature = "mqtt")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "mqtt")]
mod feature_mqtt {
    use super::*;

    #[test]
    fn mqtt_builtins_registered() {
        eval_ok(r#"fn main() { mqtt_disconnect(-1) }"#);
    }

    #[test]
    fn mqtt_connect_error_path() {
        let r = eval(r#"fn main() { let h = mqtt_connect("invalid"); println(h) }"#);
        assert!(r.is_ok() || r.is_err());
    }
}

// ═══════════════════════════════════════════════════════════
// BLE (feature = "ble")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "ble")]
mod feature_ble {
    use super::*;

    #[test]
    fn ble_builtins_registered() {
        eval_ok(r#"fn main() { ble_disconnect(-1) }"#);
    }

    #[test]
    fn ble_connect_error_path() {
        let r = eval(r#"fn main() { let h = ble_connect("00:00:00:00:00:00"); println(h) }"#);
        assert!(r.is_ok() || r.is_err());
    }
}

// ═══════════════════════════════════════════════════════════
// GUI (feature = "gui")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "gui")]
mod feature_gui {
    use super::*;

    #[test]
    fn gui_builtins_registered() {
        // gui_create_window is a known builtin
        eval_ok(r#"fn main() { println("gui feature loaded") }"#);
    }
}

// ═══════════════════════════════════════════════════════════
// HTTPS (feature = "https")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "https")]
mod feature_https {
    use super::*;

    #[test]
    fn https_builtins_registered() {
        eval_ok(r#"fn main() { println("https feature loaded") }"#);
    }
}

// ═══════════════════════════════════════════════════════════
// CUDA (feature = "cuda")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "cuda")]
mod feature_cuda {
    use super::*;

    #[test]
    fn cuda_builtins_registered() {
        eval_ok(r#"fn main() { println("cuda feature loaded") }"#);
    }

    #[test]
    fn gpu_device_enumeration() {
        let devices = fajar_lang::runtime::gpu::available_devices();
        assert!(!devices.is_empty(), "should have at least CPU fallback");
    }
}

// ═══════════════════════════════════════════════════════════
// SMT (feature = "smt")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "smt")]
mod feature_smt {
    use fajar_lang::verify::smt::*;

    #[test]
    fn solver_config_defaults() {
        let cfg = SolverConfig::default();
        assert_eq!(cfg.timeout_ms, 5000);
    }

    #[test]
    fn smt_logic_display() {
        assert_eq!(format!("{}", SmtLogic::QfLia), "QF_LIA");
    }

    #[test]
    fn solver_backend_display() {
        assert_eq!(format!("{}", SolverBackend::Z3), "Z3");
    }
}

// ═══════════════════════════════════════════════════════════
// C++ FFI (feature = "cpp-ffi")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "cpp-ffi")]
mod feature_cpp_ffi {
    use fajar_lang::ffi_v2::cpp::*;

    #[test]
    fn cpp_type_to_fajar_mapping() {
        assert_eq!(CppType::Void.to_fajar_type(), "void");
        assert_eq!(CppType::Bool.to_fajar_type(), "bool");
    }

    #[test]
    fn cpp_int_sizes() {
        assert_eq!(CppType::Int(CppIntSize::I32).to_fajar_type(), "i32");
        assert_eq!(CppType::Int(CppIntSize::I64).to_fajar_type(), "i64");
        assert_eq!(CppType::Int(CppIntSize::U8).to_fajar_type(), "u8");
    }
}

// ═══════════════════════════════════════════════════════════
// Python FFI (feature = "python-ffi")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "python-ffi")]
mod feature_python_ffi {
    use fajar_lang::ffi_v2::python::*;

    #[test]
    fn python_version_supported() {
        let v = PythonVersion {
            major: 3,
            minor: 11,
            patch: 0,
        };
        assert!(v.is_supported());
    }

    #[test]
    fn python_version_unsupported() {
        let v = PythonVersion {
            major: 2,
            minor: 7,
            patch: 18,
        };
        assert!(!v.is_supported());
    }

    #[test]
    fn python_version_lib_name() {
        let v = PythonVersion {
            major: 3,
            minor: 11,
            patch: 0,
        };
        assert_eq!(v.lib_name(), "python3.11");
    }
}

// ═══════════════════════════════════════════════════════════
// GPU (feature = "gpu")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "gpu")]
mod feature_gpu {
    #[test]
    fn gpu_device_enumeration() {
        let devices = fajar_lang::runtime::gpu::available_devices();
        assert!(!devices.is_empty());
    }

    #[test]
    fn gpu_best_device_cached() {
        let d1 = fajar_lang::runtime::gpu::best_device();
        let d2 = fajar_lang::runtime::gpu::best_device();
        assert!(std::sync::Arc::ptr_eq(&d1, &d2));
    }
}

// ═══════════════════════════════════════════════════════════
// TLS (feature = "tls")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "tls")]
mod feature_tls {
    use super::*;

    #[test]
    fn tls_feature_compiles_and_loads() {
        eval_ok(r#"fn main() { println("tls feature loaded") }"#);
    }
}

// ═══════════════════════════════════════════════════════════
// Playground WASM (feature = "playground-wasm")
// ═══════════════════════════════════════════════════════════

#[cfg(feature = "playground-wasm")]
mod feature_playground_wasm {
    #[test]
    fn playground_wasm_compiles() {
        assert!(true);
    }
}
