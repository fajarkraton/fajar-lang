//! FFI v2 — C++, Python, and Rust interop for Fajar Lang.
//!
//! Access entire ecosystems (PyTorch, OpenCV, Tokio) without reimplementing.

pub mod bindgen;
pub mod build_system;
pub mod cpp;
pub mod cpp_smart_ptr;
pub mod cpp_stl;
pub mod cpp_templates;
pub mod docs;
pub mod python;
pub mod python_async;
pub mod python_numpy;
pub mod rust_bridge;
pub mod rust_traits;
pub mod safety;

// ═══════════════════════════════════════════════════════════════════════
// V14 Phase 12: External Library Detection
// ═══════════════════════════════════════════════════════════════════════

/// Detected external library information.
#[derive(Debug, Clone)]
pub struct ExternalLibrary {
    /// Library name.
    pub name: String,
    /// Whether the library is available on this system.
    pub available: bool,
    /// Detected version (if available).
    pub version: Option<String>,
    /// Library path (if found).
    pub path: Option<String>,
}

/// Detect available external libraries for FFI.
///
/// Checks for common ML/DB/vision libraries by looking for:
/// - pkg-config entries
/// - Common library paths
/// - Python packages
pub fn detect_external_libraries() -> Vec<ExternalLibrary> {
    let mut libs = Vec::new();

    // OpenCV
    let opencv_available = std::process::Command::new("pkg-config")
        .args(["--modversion", "opencv4"])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let opencv_version = if opencv_available {
        std::process::Command::new("pkg-config")
            .args(["--modversion", "opencv4"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        None
    };
    libs.push(ExternalLibrary {
        name: "opencv".into(),
        available: opencv_available,
        version: opencv_version,
        path: None,
    });

    // PostgreSQL (libpq)
    let pg_available = std::process::Command::new("pg_config")
        .arg("--version")
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let pg_version = if pg_available {
        std::process::Command::new("pg_config")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    } else {
        None
    };
    libs.push(ExternalLibrary {
        name: "postgresql".into(),
        available: pg_available,
        version: pg_version,
        path: None,
    });

    // Python + PyTorch
    let python_available = std::process::Command::new("python3")
        .args(["--version"])
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let pytorch_available = if python_available {
        std::process::Command::new("python3")
            .args(["-c", "import torch; print(torch.__version__)"])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };
    libs.push(ExternalLibrary {
        name: "python3".into(),
        available: python_available,
        version: None,
        path: None,
    });
    libs.push(ExternalLibrary {
        name: "pytorch".into(),
        available: pytorch_available,
        version: None,
        path: None,
    });

    libs
}

/// Check if QEMU is available for boot testing.
pub fn detect_qemu() -> Option<String> {
    std::process::Command::new("qemu-system-x86_64")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().next().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v14_ffi_detect_libraries_runs() {
        // Detection runs without panic regardless of what's installed
        let libs = detect_external_libraries();
        assert!(libs.len() >= 4);
        assert!(libs.iter().any(|l| l.name == "opencv"));
        assert!(libs.iter().any(|l| l.name == "postgresql"));
        assert!(libs.iter().any(|l| l.name == "python3"));
        assert!(libs.iter().any(|l| l.name == "pytorch"));
    }

    #[test]
    fn v14_ffi_detect_qemu_runs() {
        // QEMU detection runs without panic
        let _qemu = detect_qemu();
    }

    #[test]
    fn v14_ffi_external_library_struct() {
        let lib = ExternalLibrary {
            name: "test".into(),
            available: true,
            version: Some("1.0".into()),
            path: Some("/usr/lib".into()),
        };
        assert!(lib.available);
        assert_eq!(lib.version.as_deref(), Some("1.0"));
    }
}
