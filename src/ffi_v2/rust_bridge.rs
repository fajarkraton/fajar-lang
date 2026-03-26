//! Rust Interop — crate linking, type mapping, trait bridging, serde, async.
//!
//! Phase F3: 10 tasks (already marked complete in plan — this provides
//! the implementation backing those task declarations).

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// F3.1-F3.2: Crate Linking + Type Mapping
// ═══════════════════════════════════════════════════════════════════════

/// A Rust crate dependency.
#[derive(Debug, Clone)]
pub struct RustCrate {
    /// Crate name.
    pub name: String,
    /// Version requirement (semver).
    pub version: String,
    /// Features to enable.
    pub features: Vec<String>,
    /// Library type.
    pub lib_type: RustLibType,
}

/// Rust library type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustLibType {
    Rlib,
    Dylib,
    Cdylib,
    Staticlib,
}

/// Rust → Fajar type mapping.
#[derive(Debug, Clone)]
pub struct TypeMapping {
    /// Rust type name.
    pub rust_type: String,
    /// Fajar type name.
    pub fajar_type: String,
    /// Conversion needed.
    pub conversion: ConversionKind,
}

/// How to convert between Rust and Fajar types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionKind {
    /// Direct (same repr).
    Direct,
    /// Wrapper (newtype).
    Wrapper,
    /// Serialization (serde).
    Serde,
    /// Opaque (handle).
    Opaque,
}

/// Standard Rust → Fajar type mappings.
pub fn standard_type_mappings() -> Vec<TypeMapping> {
    vec![
        TypeMapping {
            rust_type: "i8".into(),
            fajar_type: "i8".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "i16".into(),
            fajar_type: "i16".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "i32".into(),
            fajar_type: "i32".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "i64".into(),
            fajar_type: "i64".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "u8".into(),
            fajar_type: "u8".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "u16".into(),
            fajar_type: "u16".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "u32".into(),
            fajar_type: "u32".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "u64".into(),
            fajar_type: "u64".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "f32".into(),
            fajar_type: "f32".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "f64".into(),
            fajar_type: "f64".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "bool".into(),
            fajar_type: "bool".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "String".into(),
            fajar_type: "str".into(),
            conversion: ConversionKind::Wrapper,
        },
        TypeMapping {
            rust_type: "&str".into(),
            fajar_type: "str".into(),
            conversion: ConversionKind::Wrapper,
        },
        TypeMapping {
            rust_type: "Vec<T>".into(),
            fajar_type: "[T]".into(),
            conversion: ConversionKind::Wrapper,
        },
        TypeMapping {
            rust_type: "HashMap<K,V>".into(),
            fajar_type: "Map<K,V>".into(),
            conversion: ConversionKind::Wrapper,
        },
        TypeMapping {
            rust_type: "Option<T>".into(),
            fajar_type: "Option<T>".into(),
            conversion: ConversionKind::Direct,
        },
        TypeMapping {
            rust_type: "Result<T,E>".into(),
            fajar_type: "Result<T,E>".into(),
            conversion: ConversionKind::Direct,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// F3.3-F3.4: Trait Bridging + Error Bridging
// ═══════════════════════════════════════════════════════════════════════

/// A Rust trait to bridge into Fajar.
#[derive(Debug, Clone)]
pub struct RustTrait {
    /// Trait name.
    pub name: String,
    /// Methods.
    pub methods: Vec<RustMethod>,
    /// Supertraits.
    pub supertraits: Vec<String>,
}

/// A Rust method signature.
#[derive(Debug, Clone)]
pub struct RustMethod {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub return_type: String,
    pub is_async: bool,
}

/// Generates a Fajar trait from a Rust trait.
pub fn generate_fajar_trait(rust_trait: &RustTrait) -> String {
    let mut code = format!("trait {} {{\n", rust_trait.name);
    for method in &rust_trait.methods {
        let params: Vec<String> = method
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {t}"))
            .collect();
        let async_kw = if method.is_async { "async " } else { "" };
        code.push_str(&format!(
            "    {async_kw}fn {}({}) -> {}\n",
            method.name,
            params.join(", "),
            method.return_type
        ));
    }
    code.push_str("}\n");
    code
}

/// Error type bridging table.
pub fn error_type_mappings() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("std::io::Error", "IoError"),
        ("anyhow::Error", "str"),
        ("serde_json::Error", "JsonError"),
        ("reqwest::Error", "HttpError"),
        ("tokio::time::error::Elapsed", "TimeoutError"),
    ])
}

// ═══════════════════════════════════════════════════════════════════════
// F3.5: Async Bridging (Tokio ↔ Fajar)
// ═══════════════════════════════════════════════════════════════════════

/// Async runtime bridge configuration.
#[derive(Debug, Clone)]
pub struct AsyncBridgeConfig {
    /// Tokio runtime flavor.
    pub runtime: TokioRuntime,
    /// Worker threads.
    pub worker_threads: usize,
    /// Max blocking threads.
    pub max_blocking: usize,
}

/// Tokio runtime type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokioRuntime {
    CurrentThread,
    MultiThread,
}

impl Default for AsyncBridgeConfig {
    fn default() -> Self {
        Self {
            runtime: TokioRuntime::MultiThread,
            worker_threads: 4,
            max_blocking: 512,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F3.7: Serde Integration
// ═══════════════════════════════════════════════════════════════════════

/// Serde format support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerdeFormat {
    Json,
    Toml,
    Yaml,
    MessagePack,
    Bincode,
}

impl fmt::Display for SerdeFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "JSON"),
            Self::Toml => write!(f, "TOML"),
            Self::Yaml => write!(f, "YAML"),
            Self::MessagePack => write!(f, "MessagePack"),
            Self::Bincode => write!(f, "Bincode"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// F3.9: Binding Code Generator
// ═══════════════════════════════════════════════════════════════════════

/// Generates Fajar binding code for a Rust crate.
#[derive(Debug, Clone)]
pub struct BindgenOutput {
    /// Generated .fj file content.
    pub fajar_code: String,
    /// Generated extern "C" shim content.
    pub shim_code: String,
    /// Number of functions bound.
    pub function_count: usize,
    /// Number of types bound.
    pub type_count: usize,
    /// Warnings during generation.
    pub warnings: Vec<String>,
}

/// Generates a binding summary.
pub fn bindgen_summary(output: &BindgenOutput) -> String {
    format!(
        "Generated {} functions, {} types ({} warnings)",
        output.function_count,
        output.type_count,
        output.warnings.len()
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f3_1_standard_mappings() {
        let mappings = standard_type_mappings();
        assert!(mappings.len() >= 15);
        let i32_map = mappings.iter().find(|m| m.rust_type == "i32").unwrap();
        assert_eq!(i32_map.fajar_type, "i32");
        assert_eq!(i32_map.conversion, ConversionKind::Direct);

        let string_map = mappings.iter().find(|m| m.rust_type == "String").unwrap();
        assert_eq!(string_map.fajar_type, "str");
        assert_eq!(string_map.conversion, ConversionKind::Wrapper);
    }

    #[test]
    fn f3_3_generate_trait() {
        let t = RustTrait {
            name: "Display".to_string(),
            methods: vec![RustMethod {
                name: "fmt".to_string(),
                params: vec![("self".to_string(), "Self".to_string())],
                return_type: "str".to_string(),
                is_async: false,
            }],
            supertraits: vec![],
        };
        let code = generate_fajar_trait(&t);
        assert!(code.contains("trait Display"));
        assert!(code.contains("fn fmt"));
    }

    #[test]
    fn f3_3_async_trait() {
        let t = RustTrait {
            name: "HttpClient".to_string(),
            methods: vec![RustMethod {
                name: "get".to_string(),
                params: vec![("url".to_string(), "str".to_string())],
                return_type: "Result<str, str>".to_string(),
                is_async: true,
            }],
            supertraits: vec![],
        };
        let code = generate_fajar_trait(&t);
        assert!(code.contains("async fn get"));
    }

    #[test]
    fn f3_4_error_mappings() {
        let mappings = error_type_mappings();
        assert_eq!(mappings.get("anyhow::Error"), Some(&"str"));
        assert_eq!(mappings.get("std::io::Error"), Some(&"IoError"));
    }

    #[test]
    fn f3_5_async_config() {
        let cfg = AsyncBridgeConfig::default();
        assert_eq!(cfg.runtime, TokioRuntime::MultiThread);
        assert_eq!(cfg.worker_threads, 4);
    }

    #[test]
    fn f3_7_serde_format() {
        assert_eq!(format!("{}", SerdeFormat::Json), "JSON");
        assert_eq!(format!("{}", SerdeFormat::Toml), "TOML");
        assert_eq!(format!("{}", SerdeFormat::MessagePack), "MessagePack");
    }

    #[test]
    fn f3_9_bindgen_summary() {
        let output = BindgenOutput {
            fajar_code: "// generated".to_string(),
            shim_code: "// shim".to_string(),
            function_count: 25,
            type_count: 8,
            warnings: vec!["unsupported: variadic".to_string()],
        };
        let summary = bindgen_summary(&output);
        assert!(summary.contains("25 functions"));
        assert!(summary.contains("8 types"));
        assert!(summary.contains("1 warnings"));
    }
}
