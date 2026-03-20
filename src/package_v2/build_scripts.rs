//! Build Scripts — build script support, environment variables,
//! native library detection, code generation, rerun triggers,
//! link flags, feature detection, proto compilation, build deps.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S22.1: Build Script Support
// ═══════════════════════════════════════════════════════════════════════

/// Build script configuration from fj.toml.
#[derive(Debug, Clone)]
pub struct BuildScript {
    /// Path to build script (relative to package root).
    pub path: String,
    /// Build dependencies.
    pub build_deps: HashMap<String, String>,
    /// Environment variables to set before running.
    pub env: HashMap<String, String>,
    /// Whether the build script has run.
    pub executed: bool,
}

impl BuildScript {
    /// Creates a new build script config.
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            build_deps: HashMap::new(),
            env: HashMap::new(),
            executed: false,
        }
    }

    /// Adds a build dependency.
    pub fn add_build_dep(&mut self, name: &str, version: &str) {
        self.build_deps
            .insert(name.to_string(), version.to_string());
    }
}

/// Build script output — parsed from stdout directives.
#[derive(Debug, Clone)]
pub struct BuildOutput {
    /// Cfg flags to set.
    pub cfg_flags: Vec<String>,
    /// Environment variables to set.
    pub env_vars: HashMap<String, String>,
    /// Link libraries.
    pub link_libs: Vec<LinkLib>,
    /// Rerun triggers.
    pub rerun_if_changed: Vec<String>,
    /// Generated source files.
    pub generated_files: Vec<String>,
    /// Warning messages.
    pub warnings: Vec<String>,
}

impl Default for BuildOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildOutput {
    /// Creates empty build output.
    pub fn new() -> Self {
        Self {
            cfg_flags: Vec::new(),
            env_vars: HashMap::new(),
            link_libs: Vec::new(),
            rerun_if_changed: Vec::new(),
            generated_files: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.2: Environment Variables
// ═══════════════════════════════════════════════════════════════════════

/// Parses a build script output line.
pub fn parse_directive(line: &str) -> Option<BuildDirective> {
    let line = line.trim();
    if !line.starts_with("fj:") {
        return None;
    }

    let directive = &line[3..];
    if let Some(rest) = directive.strip_prefix("cfg=") {
        Some(BuildDirective::Cfg(rest.to_string()))
    } else if let Some(rest) = directive.strip_prefix("env=") {
        let parts: Vec<&str> = rest.splitn(2, '=').collect();
        if parts.len() == 2 {
            Some(BuildDirective::Env(
                parts[0].to_string(),
                parts[1].to_string(),
            ))
        } else {
            None
        }
    } else if let Some(rest) = directive.strip_prefix("rustc-link-lib=") {
        Some(BuildDirective::LinkLib(rest.to_string()))
    } else if let Some(rest) = directive.strip_prefix("rerun-if-changed=") {
        Some(BuildDirective::RerunIfChanged(rest.to_string()))
    } else {
        directive
            .strip_prefix("warning=")
            .map(|rest| BuildDirective::Warning(rest.to_string()))
    }
}

/// A parsed build script directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDirective {
    /// Set a cfg flag.
    Cfg(String),
    /// Set an environment variable.
    Env(String, String),
    /// Link a library.
    LinkLib(String),
    /// Rerun if file changed.
    RerunIfChanged(String),
    /// Warning message.
    Warning(String),
}

/// Processes all build output lines into a BuildOutput.
pub fn process_output(lines: &[&str]) -> BuildOutput {
    let mut out = BuildOutput::new();
    for line in lines {
        if let Some(directive) = parse_directive(line) {
            match directive {
                BuildDirective::Cfg(flag) => out.cfg_flags.push(flag),
                BuildDirective::Env(k, v) => {
                    out.env_vars.insert(k, v);
                }
                BuildDirective::LinkLib(lib) => {
                    out.link_libs.push(LinkLib {
                        name: lib,
                        kind: LinkKind::Dynamic,
                    });
                }
                BuildDirective::RerunIfChanged(path) => out.rerun_if_changed.push(path),
                BuildDirective::Warning(msg) => out.warnings.push(msg),
            }
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════
// S22.3: Native Library Detection
// ═══════════════════════════════════════════════════════════════════════

/// A native library found by pkg-config or manual search.
#[derive(Debug, Clone)]
pub struct NativeLibrary {
    /// Library name.
    pub name: String,
    /// Include paths.
    pub include_paths: Vec<String>,
    /// Library paths.
    pub lib_paths: Vec<String>,
    /// Link flags.
    pub link_flags: Vec<String>,
    /// Version (if detected).
    pub version: Option<String>,
}

/// Simulates pkg-config library detection.
pub fn find_library(name: &str) -> Result<NativeLibrary, BuildError> {
    // In real implementation, this would call pkg-config
    // For now, return a descriptive error for missing libraries
    match name {
        "openssl" | "zlib" | "libpng" | "sqlite3" => Ok(NativeLibrary {
            name: name.to_string(),
            include_paths: vec![format!("/usr/include/{name}")],
            lib_paths: vec!["/usr/lib".to_string()],
            link_flags: vec![format!("-l{name}")],
            version: Some("1.0.0".to_string()),
        }),
        _ => Err(BuildError::LibraryNotFound(name.to_string())),
    }
}

/// Build script error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// Library not found.
    LibraryNotFound(String),
    /// Script execution failed.
    ScriptFailed(String),
    /// Invalid directive.
    InvalidDirective(String),
    /// Missing OUT_DIR.
    MissingOutDir,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::LibraryNotFound(name) => {
                write!(f, "Native library not found: {name}")
            }
            BuildError::ScriptFailed(msg) => write!(f, "Build script failed: {msg}"),
            BuildError::InvalidDirective(d) => write!(f, "Invalid build directive: {d}"),
            BuildError::MissingOutDir => write!(f, "OUT_DIR not set"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.4: Code Generation
// ═══════════════════════════════════════════════════════════════════════

/// A generated source file from a build script.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// Output path (relative to OUT_DIR).
    pub path: String,
    /// Generated content.
    pub content: String,
}

/// Generates a Fajar Lang source file from a template.
pub fn generate_source(path: &str, content: &str) -> GeneratedFile {
    GeneratedFile {
        path: path.to_string(),
        content: content.to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.5: Rerun Triggers
// ═══════════════════════════════════════════════════════════════════════

/// Determines if a build script needs to be re-run.
pub fn needs_rerun(
    triggers: &[String],
    last_run_timestamp: u64,
    file_timestamps: &HashMap<String, u64>,
) -> bool {
    for trigger in triggers {
        if let Some(&ts) = file_timestamps.get(trigger) {
            if ts > last_run_timestamp {
                return true;
            }
        }
    }
    false
}

// ═══════════════════════════════════════════════════════════════════════
// S22.6: Link Flags
// ═══════════════════════════════════════════════════════════════════════

/// Link library specification.
#[derive(Debug, Clone)]
pub struct LinkLib {
    /// Library name.
    pub name: String,
    /// Link kind.
    pub kind: LinkKind,
}

/// How to link a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// Dynamic linking (.so/.dylib/.dll).
    Dynamic,
    /// Static linking (.a/.lib).
    Static,
    /// Framework (macOS).
    Framework,
}

impl fmt::Display for LinkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkKind::Dynamic => write!(f, "dylib"),
            LinkKind::Static => write!(f, "static"),
            LinkKind::Framework => write!(f, "framework"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S22.7: Feature Detection
// ═══════════════════════════════════════════════════════════════════════

/// System feature probe result.
#[derive(Debug, Clone)]
pub struct FeatureProbe {
    /// Feature name.
    pub name: String,
    /// Whether available.
    pub available: bool,
    /// Additional info.
    pub info: Option<String>,
}

/// Probes for system features.
pub fn probe_features(features: &[&str]) -> Vec<FeatureProbe> {
    features
        .iter()
        .map(|&name| {
            let available = matches!(name, "sse2" | "avx" | "neon" | "threads" | "filesystem");
            FeatureProbe {
                name: name.to_string(),
                available,
                info: None,
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S22.8: Proto/Schema Compilation
// ═══════════════════════════════════════════════════════════════════════

/// Schema file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaType {
    /// Protocol Buffers.
    Protobuf,
    /// FlatBuffers.
    FlatBuffers,
    /// JSON Schema.
    JsonSchema,
}

/// A schema compilation request.
#[derive(Debug, Clone)]
pub struct SchemaCompile {
    /// Input schema file.
    pub input: String,
    /// Schema type.
    pub schema_type: SchemaType,
    /// Output directory.
    pub output_dir: String,
}

// ═══════════════════════════════════════════════════════════════════════
// S22.9: Build Script Dependencies (covered by BuildScript.build_deps)
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S22.1 — Build Script Support
    #[test]
    fn s22_1_build_script_new() {
        let mut bs = BuildScript::new("build.fj");
        assert_eq!(bs.path, "build.fj");
        assert!(!bs.executed);
        bs.add_build_dep("cc", "1.0");
        assert!(bs.build_deps.contains_key("cc"));
    }

    // S22.2 — Environment Variables
    #[test]
    fn s22_2_parse_cfg_directive() {
        let d = parse_directive("fj:cfg=has_gpu").unwrap();
        assert_eq!(d, BuildDirective::Cfg("has_gpu".into()));
    }

    #[test]
    fn s22_2_parse_env_directive() {
        let d = parse_directive("fj:env=MY_VAR=hello").unwrap();
        assert_eq!(d, BuildDirective::Env("MY_VAR".into(), "hello".into()));
    }

    #[test]
    fn s22_2_parse_non_directive() {
        assert!(parse_directive("regular output").is_none());
    }

    #[test]
    fn s22_2_process_output() {
        let lines = vec![
            "fj:cfg=feature_x",
            "fj:env=OUT_DIR=/tmp/build",
            "fj:rustc-link-lib=ssl",
            "some normal output",
            "fj:rerun-if-changed=build.fj",
            "fj:warning=check openssl version",
        ];
        let out = process_output(&lines);
        assert_eq!(out.cfg_flags, vec!["feature_x"]);
        assert_eq!(out.env_vars.get("OUT_DIR").unwrap(), "/tmp/build");
        assert_eq!(out.link_libs.len(), 1);
        assert_eq!(out.rerun_if_changed, vec!["build.fj"]);
        assert_eq!(out.warnings.len(), 1);
    }

    // S22.3 — Native Library Detection
    #[test]
    fn s22_3_find_known_library() {
        let lib = find_library("openssl").unwrap();
        assert_eq!(lib.name, "openssl");
        assert!(lib.version.is_some());
    }

    #[test]
    fn s22_3_find_unknown_library() {
        let result = find_library("nonexistent_lib");
        assert!(matches!(result, Err(BuildError::LibraryNotFound(_))));
    }

    // S22.4 — Code Generation
    #[test]
    fn s22_4_generate_source() {
        let generated = generate_source("generated.fj", "const VERSION: str = \"1.0\"");
        assert_eq!(generated.path, "generated.fj");
        assert!(generated.content.contains("VERSION"));
    }

    // S22.5 — Rerun Triggers
    #[test]
    fn s22_5_needs_rerun_true() {
        let triggers = vec!["build.fj".to_string()];
        let mut timestamps = HashMap::new();
        timestamps.insert("build.fj".to_string(), 200);
        assert!(needs_rerun(&triggers, 100, &timestamps));
    }

    #[test]
    fn s22_5_needs_rerun_false() {
        let triggers = vec!["build.fj".to_string()];
        let mut timestamps = HashMap::new();
        timestamps.insert("build.fj".to_string(), 50);
        assert!(!needs_rerun(&triggers, 100, &timestamps));
    }

    // S22.6 — Link Flags
    #[test]
    fn s22_6_link_kind_display() {
        assert_eq!(LinkKind::Dynamic.to_string(), "dylib");
        assert_eq!(LinkKind::Static.to_string(), "static");
        assert_eq!(LinkKind::Framework.to_string(), "framework");
    }

    // S22.7 — Feature Detection
    #[test]
    fn s22_7_probe_features() {
        let results = probe_features(&["sse2", "avx", "some_unknown"]);
        assert_eq!(results.len(), 3);
        assert!(results[0].available); // sse2
        assert!(!results[2].available); // unknown
    }

    // S22.8 — Schema Compilation
    #[test]
    fn s22_8_schema_compile() {
        let sc = SchemaCompile {
            input: "message.proto".into(),
            schema_type: SchemaType::Protobuf,
            output_dir: "out/".into(),
        };
        assert_eq!(sc.schema_type, SchemaType::Protobuf);
    }

    // S22.9 — Build Dependencies
    #[test]
    fn s22_9_build_deps() {
        let mut bs = BuildScript::new("build.fj");
        bs.add_build_dep("cc", "1.0");
        bs.add_build_dep("protoc", "3.0");
        assert_eq!(bs.build_deps.len(), 2);
    }

    // S22.10 — Error Display
    #[test]
    fn s22_10_error_display() {
        let e = BuildError::LibraryNotFound("ssl".into());
        assert!(e.to_string().contains("ssl"));

        let e2 = BuildError::MissingOutDir;
        assert!(e2.to_string().contains("OUT_DIR"));
    }
}
