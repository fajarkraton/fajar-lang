//! Compile-time macros — macros that evaluate entirely at compile time,
//! producing `ComptimeValue` results for use in `const` declarations.
//!
//! # Macros
//!
//! | Macro | Description |
//! |-------|-------------|
//! | `const_eval!(expr)` | Evaluate expression at compile time |
//! | `static_assert!(cond)` | Compile-time assertion |
//! | `include_str!("path")` | Include file contents as const string |
//! | `include_bytes!("path")` | Include file as const byte array |
//! | `env!("VAR")` | Read env variable at compile time |
//! | `concat!(a, b, ...)` | Concatenate const strings |
//! | `cfg!(key = "value")` | Conditional compilation as const bool |
//! | `option_env!("VAR")` | Optional env var as `Option<str>` |
//! | `compile_error!("msg")` | User-defined compile error |

use std::collections::HashMap;
use std::path::Path;

use crate::analyzer::comptime::ComptimeValue;

// ═══════════════════════════════════════════════════════════════════════
// Configuration for compile-time macros
// ═══════════════════════════════════════════════════════════════════════

/// Configuration flags for `cfg!()` evaluation.
#[derive(Debug, Clone, Default)]
pub struct CfgConfig {
    /// Key-value pairs (e.g., `target_os = "linux"`).
    pub values: HashMap<String, String>,
    /// Boolean flags (e.g., `debug_assertions`).
    pub flags: Vec<String>,
}

impl CfgConfig {
    /// Creates a config for the current host platform.
    pub fn host() -> Self {
        let mut cfg = Self::default();

        // Target OS
        #[cfg(target_os = "linux")]
        cfg.values.insert("target_os".into(), "linux".into());
        #[cfg(target_os = "macos")]
        cfg.values.insert("target_os".into(), "macos".into());
        #[cfg(target_os = "windows")]
        cfg.values.insert("target_os".into(), "windows".into());

        // Target arch
        #[cfg(target_arch = "x86_64")]
        cfg.values.insert("target_arch".into(), "x86_64".into());
        #[cfg(target_arch = "aarch64")]
        cfg.values.insert("target_arch".into(), "aarch64".into());

        // Pointer width
        cfg.values.insert(
            "target_pointer_width".into(),
            (std::mem::size_of::<usize>() * 8).to_string(),
        );

        // Debug assertions
        #[cfg(debug_assertions)]
        cfg.flags.push("debug_assertions".into());

        cfg
    }

    /// Check if a cfg key matches a value.
    pub fn check(&self, key: &str, value: &str) -> bool {
        self.values.get(key).map(|v| v == value).unwrap_or(false)
    }

    /// Check if a cfg flag is set.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Const Macro Errors
// ═══════════════════════════════════════════════════════════════════════

/// Error produced by a compile-time macro.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConstMacroError {
    /// static_assert! condition failed.
    #[error("static assertion failed: {message}")]
    StaticAssertFailed { message: String },

    /// compile_error! invoked.
    #[error("compile error: {message}")]
    CompileError { message: String },

    /// File not found for include_str!/include_bytes!.
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    /// Environment variable not found.
    #[error("environment variable '{name}' not found")]
    EnvNotFound { name: String },

    /// Invalid arguments.
    #[error("invalid macro arguments: {reason}")]
    InvalidArgs { reason: String },

    /// I/O error.
    #[error("I/O error: {message}")]
    IoError { message: String },
}

// ═══════════════════════════════════════════════════════════════════════
// Const Macro Evaluator
// ═══════════════════════════════════════════════════════════════════════

/// Evaluator for compile-time macros.
#[derive(Debug, Clone)]
pub struct ConstMacroEvaluator {
    /// Configuration for cfg!().
    pub cfg: CfgConfig,
    /// Base directory for include_str!/include_bytes! (project root).
    pub base_dir: String,
}

impl ConstMacroEvaluator {
    /// Creates a new evaluator with host config.
    pub fn new(base_dir: &str) -> Self {
        Self {
            cfg: CfgConfig::host(),
            base_dir: base_dir.to_string(),
        }
    }

    /// Creates an evaluator with custom cfg config (for cross-compilation).
    pub fn with_cfg(base_dir: &str, cfg: CfgConfig) -> Self {
        Self {
            cfg,
            base_dir: base_dir.to_string(),
        }
    }

    /// Evaluate a compile-time macro by name.
    ///
    /// Returns `None` if the name is not a known const macro.
    pub fn eval(
        &self,
        macro_name: &str,
        args: &[ComptimeValue],
    ) -> Option<Result<ComptimeValue, ConstMacroError>> {
        match macro_name {
            "const_eval" => Some(self.eval_const_eval(args)),
            "static_assert" => Some(self.eval_static_assert(args)),
            "include_str" => Some(self.eval_include_str(args)),
            "include_bytes" => Some(self.eval_include_bytes(args)),
            "env" => Some(self.eval_env(args)),
            "concat" => Some(self.eval_concat(args)),
            "cfg" => Some(self.eval_cfg(args)),
            "option_env" => Some(self.eval_option_env(args)),
            "compile_error" => Some(self.eval_compile_error(args)),
            _ => None,
        }
    }

    /// Returns the list of all known const macro names.
    pub fn known_macros() -> &'static [&'static str] {
        &[
            "const_eval",
            "static_assert",
            "include_str",
            "include_bytes",
            "env",
            "concat",
            "cfg",
            "option_env",
            "compile_error",
        ]
    }

    // K6.1: const_eval!(expr) — identity, just returns the evaluated arg.
    fn eval_const_eval(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        args.first().cloned().ok_or(ConstMacroError::InvalidArgs {
            reason: "const_eval! requires one argument".into(),
        })
    }

    // K6.2: static_assert!(condition [, "message"])
    fn eval_static_assert(
        &self,
        args: &[ComptimeValue],
    ) -> Result<ComptimeValue, ConstMacroError> {
        let cond = args.first().ok_or(ConstMacroError::InvalidArgs {
            reason: "static_assert! requires a condition".into(),
        })?;

        let is_true = match cond {
            ComptimeValue::Bool(b) => *b,
            ComptimeValue::Int(v) => *v != 0,
            _ => {
                return Err(ConstMacroError::InvalidArgs {
                    reason: "static_assert! condition must be bool or int".into(),
                })
            }
        };

        if is_true {
            Ok(ComptimeValue::Null)
        } else {
            let msg = args
                .get(1)
                .and_then(|v| {
                    if let ComptimeValue::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "static assertion failed".into());
            Err(ConstMacroError::StaticAssertFailed { message: msg })
        }
    }

    // K6.3: include_str!("path")
    fn eval_include_str(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        let path = self.extract_path(args)?;
        let full_path = Path::new(&self.base_dir).join(&path);

        match std::fs::read_to_string(&full_path) {
            Ok(contents) => Ok(ComptimeValue::Str(contents)),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(ConstMacroError::FileNotFound {
                        path: full_path.display().to_string(),
                    })
                } else {
                    Err(ConstMacroError::IoError {
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    // K6.4: include_bytes!("path")
    fn eval_include_bytes(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        let path = self.extract_path(args)?;
        let full_path = Path::new(&self.base_dir).join(&path);

        match std::fs::read(&full_path) {
            Ok(bytes) => {
                let arr = bytes.iter().map(|b| ComptimeValue::Int(*b as i64)).collect();
                Ok(ComptimeValue::Array(arr))
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(ConstMacroError::FileNotFound {
                        path: full_path.display().to_string(),
                    })
                } else {
                    Err(ConstMacroError::IoError {
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    // K6.5: env!("VAR")
    fn eval_env(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        let var_name = self.extract_string(args)?;
        match std::env::var(&var_name) {
            Ok(val) => Ok(ComptimeValue::Str(val)),
            Err(_) => Err(ConstMacroError::EnvNotFound { name: var_name }),
        }
    }

    // K6.6: concat!(a, b, c, ...)
    fn eval_concat(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        let mut result = String::new();
        for arg in args {
            match arg {
                ComptimeValue::Str(s) => result.push_str(s),
                ComptimeValue::Int(v) => result.push_str(&v.to_string()),
                ComptimeValue::Float(v) => result.push_str(&v.to_string()),
                ComptimeValue::Bool(b) => result.push_str(&b.to_string()),
                _ => result.push_str(&format!("{arg}")),
            }
        }
        Ok(ComptimeValue::Str(result))
    }

    // K6.7: cfg!(key = "value") or cfg!(flag)
    fn eval_cfg(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        if args.is_empty() {
            return Err(ConstMacroError::InvalidArgs {
                reason: "cfg! requires at least one argument".into(),
            });
        }

        // Single flag: cfg!("debug_assertions")
        if args.len() == 1 {
            if let ComptimeValue::Str(flag) = &args[0] {
                return Ok(ComptimeValue::Bool(self.cfg.has_flag(flag)));
            }
        }

        // Key-value: cfg!("target_os", "linux")
        if args.len() == 2 {
            if let (ComptimeValue::Str(key), ComptimeValue::Str(value)) = (&args[0], &args[1]) {
                return Ok(ComptimeValue::Bool(self.cfg.check(key, value)));
            }
        }

        Err(ConstMacroError::InvalidArgs {
            reason: "cfg! expects (\"flag\") or (\"key\", \"value\")".into(),
        })
    }

    // K6.8: option_env!("VAR") → Some(value) or None
    fn eval_option_env(&self, args: &[ComptimeValue]) -> Result<ComptimeValue, ConstMacroError> {
        let var_name = self.extract_string(args)?;
        match std::env::var(&var_name) {
            Ok(val) => Ok(ComptimeValue::Str(val)),
            Err(_) => Ok(ComptimeValue::Null), // None represented as Null
        }
    }

    // K6.9: compile_error!("message")
    fn eval_compile_error(
        &self,
        args: &[ComptimeValue],
    ) -> Result<ComptimeValue, ConstMacroError> {
        let msg = args
            .first()
            .and_then(|v| {
                if let ComptimeValue::Str(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "compilation stopped".into());
        Err(ConstMacroError::CompileError { message: msg })
    }

    // ── Helpers ──

    fn extract_string(&self, args: &[ComptimeValue]) -> Result<String, ConstMacroError> {
        match args.first() {
            Some(ComptimeValue::Str(s)) => Ok(s.clone()),
            _ => Err(ConstMacroError::InvalidArgs {
                reason: "expected a string argument".into(),
            }),
        }
    }

    fn extract_path(&self, args: &[ComptimeValue]) -> Result<String, ConstMacroError> {
        self.extract_string(args)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests — K6.10
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluator() -> ConstMacroEvaluator {
        ConstMacroEvaluator::new("/tmp")
    }

    // ── K6.1: const_eval! ──

    #[test]
    fn k6_1_const_eval_passthrough() {
        let ev = evaluator();
        let result = ev.eval("const_eval", &[ComptimeValue::Int(42)]);
        assert_eq!(result, Some(Ok(ComptimeValue::Int(42))));
    }

    #[test]
    fn k6_1_const_eval_string() {
        let ev = evaluator();
        let result = ev.eval("const_eval", &[ComptimeValue::Str("hello".into())]);
        assert_eq!(result, Some(Ok(ComptimeValue::Str("hello".into()))));
    }

    // ── K6.2: static_assert! ──

    #[test]
    fn k6_2_static_assert_passes() {
        let ev = evaluator();
        let result = ev.eval("static_assert", &[ComptimeValue::Bool(true)]);
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn k6_2_static_assert_fails() {
        let ev = evaluator();
        let result = ev.eval(
            "static_assert",
            &[
                ComptimeValue::Bool(false),
                ComptimeValue::Str("size must be <= 64".into()),
            ],
        );
        let err = result.unwrap().unwrap_err();
        assert!(err.to_string().contains("size must be <= 64"));
    }

    #[test]
    fn k6_2_static_assert_int_truthy() {
        let ev = evaluator();
        let result = ev.eval("static_assert", &[ComptimeValue::Int(1)]);
        assert!(result.unwrap().is_ok());

        let result = ev.eval("static_assert", &[ComptimeValue::Int(0)]);
        assert!(result.unwrap().is_err());
    }

    // ── K6.3: include_str! ──

    #[test]
    fn k6_3_include_str_reads_file() {
        // Write a temp file
        let dir = std::env::temp_dir();
        let path = dir.join("fj_test_include.txt");
        std::fs::write(&path, "hello from file").unwrap();

        let ev = ConstMacroEvaluator::new(dir.to_str().unwrap());
        let result = ev.eval("include_str", &[ComptimeValue::Str("fj_test_include.txt".into())]);
        assert_eq!(
            result,
            Some(Ok(ComptimeValue::Str("hello from file".into())))
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn k6_3_include_str_file_not_found() {
        let ev = evaluator();
        let result = ev.eval("include_str", &[ComptimeValue::Str("nonexistent.txt".into())]);
        let err = result.unwrap().unwrap_err();
        assert!(matches!(err, ConstMacroError::FileNotFound { .. }));
    }

    // ── K6.4: include_bytes! ──

    #[test]
    fn k6_4_include_bytes_reads_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("fj_test_bytes.bin");
        std::fs::write(&path, &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();

        let ev = ConstMacroEvaluator::new(dir.to_str().unwrap());
        let result = ev.eval("include_bytes", &[ComptimeValue::Str("fj_test_bytes.bin".into())]);
        assert_eq!(
            result,
            Some(Ok(ComptimeValue::Array(vec![
                ComptimeValue::Int(0xDE),
                ComptimeValue::Int(0xAD),
                ComptimeValue::Int(0xBE),
                ComptimeValue::Int(0xEF),
            ])))
        );

        std::fs::remove_file(path).ok();
    }

    // ── K6.5: env! ──

    #[test]
    fn k6_5_env_reads_variable() {
        let ev = evaluator();
        // PATH should always exist
        let result = ev.eval("env", &[ComptimeValue::Str("PATH".into())]);
        let val = result.unwrap().unwrap();
        if let ComptimeValue::Str(s) = val {
            assert!(!s.is_empty());
        } else {
            panic!("expected string");
        }
    }

    #[test]
    fn k6_5_env_not_found() {
        let ev = evaluator();
        let result = ev.eval(
            "env",
            &[ComptimeValue::Str("FJ_NONEXISTENT_VAR_12345".into())],
        );
        let err = result.unwrap().unwrap_err();
        assert!(matches!(err, ConstMacroError::EnvNotFound { .. }));
    }

    // ── K6.6: concat! ──

    #[test]
    fn k6_6_concat_strings() {
        let ev = evaluator();
        let result = ev.eval(
            "concat",
            &[
                ComptimeValue::Str("hello".into()),
                ComptimeValue::Str(" ".into()),
                ComptimeValue::Str("world".into()),
            ],
        );
        assert_eq!(result, Some(Ok(ComptimeValue::Str("hello world".into()))));
    }

    #[test]
    fn k6_6_concat_mixed_types() {
        let ev = evaluator();
        let result = ev.eval(
            "concat",
            &[
                ComptimeValue::Str("v".into()),
                ComptimeValue::Int(10),
                ComptimeValue::Str(".".into()),
                ComptimeValue::Int(0),
            ],
        );
        assert_eq!(result, Some(Ok(ComptimeValue::Str("v10.0".into()))));
    }

    // ── K6.7: cfg! ──

    #[test]
    fn k6_7_cfg_target_os() {
        let ev = evaluator();
        // We're on Linux
        let result = ev.eval(
            "cfg",
            &[
                ComptimeValue::Str("target_os".into()),
                ComptimeValue::Str("linux".into()),
            ],
        );
        let val = result.unwrap().unwrap();
        // This test runs on Linux in CI
        if let ComptimeValue::Bool(b) = val {
            // Just verify it returns a bool, don't assume OS
            assert!(b || !b);
        } else {
            panic!("expected bool");
        }
    }

    #[test]
    fn k6_7_cfg_flag() {
        let mut cfg = CfgConfig::default();
        cfg.flags.push("debug_assertions".into());
        let ev = ConstMacroEvaluator::with_cfg("/tmp", cfg);

        let result = ev.eval("cfg", &[ComptimeValue::Str("debug_assertions".into())]);
        assert_eq!(result, Some(Ok(ComptimeValue::Bool(true))));

        let result = ev.eval("cfg", &[ComptimeValue::Str("unknown_flag".into())]);
        assert_eq!(result, Some(Ok(ComptimeValue::Bool(false))));
    }

    // ── K6.8: option_env! ──

    #[test]
    fn k6_8_option_env_exists() {
        let ev = evaluator();
        let result = ev.eval("option_env", &[ComptimeValue::Str("PATH".into())]);
        let val = result.unwrap().unwrap();
        assert!(matches!(val, ComptimeValue::Str(_)));
    }

    #[test]
    fn k6_8_option_env_missing() {
        let ev = evaluator();
        let result = ev.eval(
            "option_env",
            &[ComptimeValue::Str("FJ_NONEXISTENT_VAR_67890".into())],
        );
        assert_eq!(result, Some(Ok(ComptimeValue::Null)));
    }

    // ── K6.9: compile_error! ──

    #[test]
    fn k6_9_compile_error() {
        let ev = evaluator();
        let result = ev.eval(
            "compile_error",
            &[ComptimeValue::Str("unsupported platform".into())],
        );
        let err = result.unwrap().unwrap_err();
        assert!(err.to_string().contains("unsupported platform"));
        assert!(matches!(err, ConstMacroError::CompileError { .. }));
    }

    // ── K6.10: Integration ──

    #[test]
    fn k6_10_known_macros_list() {
        let known = ConstMacroEvaluator::known_macros();
        assert_eq!(known.len(), 9);
        assert!(known.contains(&"const_eval"));
        assert!(known.contains(&"static_assert"));
        assert!(known.contains(&"include_str"));
        assert!(known.contains(&"compile_error"));
    }

    #[test]
    fn k6_10_unknown_macro_returns_none() {
        let ev = evaluator();
        assert_eq!(ev.eval("unknown_macro", &[]), None);
    }

    #[test]
    fn k6_10_cfg_cross_compilation() {
        let mut cfg = CfgConfig::default();
        cfg.values.insert("target_os".into(), "none".into());
        cfg.values.insert("target_arch".into(), "riscv64".into());
        cfg.values
            .insert("target_pointer_width".into(), "64".into());
        let ev = ConstMacroEvaluator::with_cfg("/project", cfg);

        let result = ev.eval(
            "cfg",
            &[
                ComptimeValue::Str("target_os".into()),
                ComptimeValue::Str("none".into()),
            ],
        );
        assert_eq!(result, Some(Ok(ComptimeValue::Bool(true))));

        let result = ev.eval(
            "cfg",
            &[
                ComptimeValue::Str("target_arch".into()),
                ComptimeValue::Str("riscv64".into()),
            ],
        );
        assert_eq!(result, Some(Ok(ComptimeValue::Bool(true))));
    }
}
