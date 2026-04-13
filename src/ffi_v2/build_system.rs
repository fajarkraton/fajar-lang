//! Sprint E8: Build System Integration.
//!
//! Integrates FFI dependencies into Fajar Lang's build pipeline. Parses `[ffi]`
//! sections from `fj.toml`, queries pkg-config, locates Python venvs, drives
//! CMake/Cargo builds, manages linker flags, supports cross-compilation,
//! hermetic (vendored) builds, and CI generation.
//!
//! All operations are simulated (no real process spawning) — the types and
//! logic model what a production build system would do.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Build System Error
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during FFI build system operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A required package was not found by pkg-config.
    PackageNotFound(String),
    /// Version constraint not satisfied.
    VersionMismatch {
        package: String,
        required: String,
        found: String,
    },
    /// CMake configure/build/install failed.
    CmakeFailed { phase: CmakePhase, message: String },
    /// Cargo build failed.
    CargoFailed { message: String },
    /// Python venv not found or invalid.
    PythonVenvInvalid { path: String, reason: String },
    /// Cross-compilation target not supported.
    UnsupportedTarget(String),
    /// Checksum verification failed (hermetic builds).
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },
    /// Duplicate library in linker flags.
    DuplicateLib(String),
    /// Configuration error in `fj.toml` `[ffi]` section.
    ConfigError(String),
    /// Generic build error.
    Other(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageNotFound(name) => write!(f, "pkg-config: package '{name}' not found"),
            Self::VersionMismatch {
                package,
                required,
                found,
            } => write!(
                f,
                "version mismatch for '{package}': required {required}, found {found}"
            ),
            Self::CmakeFailed { phase, message } => {
                write!(f, "cmake {phase} failed: {message}")
            }
            Self::CargoFailed { message } => write!(f, "cargo build failed: {message}"),
            Self::PythonVenvInvalid { path, reason } => {
                write!(f, "python venv invalid at '{path}': {reason}")
            }
            Self::UnsupportedTarget(t) => write!(f, "unsupported cross target: {t}"),
            Self::ChecksumMismatch {
                file,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "checksum mismatch for '{file}': expected {expected}, got {actual}"
                )
            }
            Self::DuplicateLib(name) => write!(f, "duplicate library: {name}"),
            Self::ConfigError(msg) => write!(f, "ffi config error: {msg}"),
            Self::Other(msg) => write!(f, "build error: {msg}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.1: FfiConfig — [ffi] section in fj.toml
// ═══════════════════════════════════════════════════════════════════════

/// Parsed `[ffi]` section from `fj.toml`.
///
/// Declares all external FFI dependencies and their build instructions.
///
/// ```toml
/// [ffi]
/// targets = ["x86_64-unknown-linux-gnu"]
/// libs = ["openssl", "zlib"]
/// include_paths = ["/usr/include", "./vendor/include"]
///
/// [[ffi.pkg_config]]
/// name = "openssl"
/// version = ">= 1.1.0"
///
/// [[ffi.cmake]]
/// source_dir = "./vendor/libfoo"
/// build_type = "Release"
/// ```
#[derive(Debug, Clone, Default)]
pub struct FfiConfig {
    /// Target triples this FFI config supports.
    pub targets: Vec<String>,
    /// Library names to link.
    pub libs: Vec<String>,
    /// Include search paths (-I).
    pub include_paths: Vec<String>,
    /// Library search paths (-L).
    pub lib_paths: Vec<String>,
    /// pkg-config queries.
    pub pkg_config: Vec<PkgConfigEntry>,
    /// CMake build entries.
    pub cmake: Vec<CmakeEntry>,
    /// Cargo (Rust) dependencies.
    pub cargo: Vec<CargoEntry>,
    /// Python venv configuration.
    pub python: Option<PythonVenvEntry>,
    /// Cross-compilation configuration.
    pub cross: Option<CrossEntry>,
    /// Vendor/hermetic configuration.
    pub vendor: Option<VendorEntry>,
    /// Custom build commands.
    pub custom_steps: Vec<CustomBuildStep>,
}

/// A pkg-config entry in the `[ffi]` section.
#[derive(Debug, Clone)]
pub struct PkgConfigEntry {
    /// Package name.
    pub name: String,
    /// Version constraint (e.g., ">= 1.1.0").
    pub version: Option<String>,
}

/// A CMake build entry.
#[derive(Debug, Clone)]
pub struct CmakeEntry {
    /// Path to CMakeLists.txt directory.
    pub source_dir: String,
    /// Build type (Debug, Release, RelWithDebInfo, MinSizeRel).
    pub build_type: String,
    /// Additional CMake defines (-D).
    pub defines: HashMap<String, String>,
    /// Install prefix.
    pub install_prefix: Option<String>,
}

/// A Cargo build entry for Rust dependencies.
#[derive(Debug, Clone)]
pub struct CargoEntry {
    /// Crate name.
    pub name: String,
    /// Version requirement.
    pub version: String,
    /// Features to enable.
    pub features: Vec<String>,
    /// Build as cdylib or staticlib.
    pub lib_type: String,
}

/// Python venv entry.
#[derive(Debug, Clone)]
pub struct PythonVenvEntry {
    /// Path to the virtual environment.
    pub path: String,
    /// Required packages (pip).
    pub packages: Vec<String>,
    /// Minimum Python version.
    pub min_version: Option<String>,
}

/// Cross-compilation entry.
#[derive(Debug, Clone)]
pub struct CrossEntry {
    /// Target triple.
    pub target: String,
    /// Sysroot path.
    pub sysroot: Option<String>,
    /// Toolchain prefix (e.g., "aarch64-linux-gnu-").
    pub toolchain: Option<String>,
}

/// Vendor/hermetic entry.
#[derive(Debug, Clone)]
pub struct VendorEntry {
    /// Vendor directory path.
    pub dir: String,
    /// Whether to verify checksums.
    pub verify_checksums: bool,
    /// Checksum file path.
    pub checksum_file: Option<String>,
}

/// Custom build step.
#[derive(Debug, Clone)]
pub struct CustomBuildStep {
    /// Step name.
    pub name: String,
    /// Command to run.
    pub command: String,
    /// Arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub workdir: Option<String>,
    /// Environment variables.
    pub env: HashMap<String, String>,
}

impl FfiConfig {
    /// Parses an `FfiConfig` from a TOML table (simulated).
    ///
    /// In production this would use `toml::Value`; here we parse from a
    /// simplified key-value representation.
    pub fn from_toml_str(toml_content: &str) -> Result<Self, BuildError> {
        let mut config = Self::default();

        for line in toml_content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "targets" => {
                        config.targets = parse_toml_array(value);
                    }
                    "libs" => {
                        config.libs = parse_toml_array(value);
                    }
                    "include_paths" => {
                        config.include_paths = parse_toml_array(value);
                    }
                    "lib_paths" => {
                        config.lib_paths = parse_toml_array(value);
                    }
                    _ => {}
                }
            }
        }

        Ok(config)
    }

    /// Validates the configuration, checking for obvious problems.
    pub fn validate(&self) -> Result<(), Vec<BuildError>> {
        let mut errors = Vec::new();

        // Check for duplicate libs
        let mut seen = std::collections::HashSet::new();
        for lib in &self.libs {
            if !seen.insert(lib.clone()) {
                errors.push(BuildError::DuplicateLib(lib.clone()));
            }
        }

        // Validate pkg-config entries have names
        for entry in &self.pkg_config {
            if entry.name.is_empty() {
                errors.push(BuildError::ConfigError(
                    "pkg-config entry missing name".to_string(),
                ));
            }
        }

        // Validate cmake entries have source_dir
        for entry in &self.cmake {
            if entry.source_dir.is_empty() {
                errors.push(BuildError::ConfigError(
                    "cmake entry missing source_dir".to_string(),
                ));
            }
        }

        // Validate cargo entries have name and version
        for entry in &self.cargo {
            if entry.name.is_empty() {
                errors.push(BuildError::ConfigError(
                    "cargo entry missing crate name".to_string(),
                ));
            }
            if entry.version.is_empty() {
                errors.push(BuildError::ConfigError(format!(
                    "cargo entry '{}' missing version",
                    entry.name
                )));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns the total number of FFI dependency entries.
    pub fn dependency_count(&self) -> usize {
        self.pkg_config.len()
            + self.cmake.len()
            + self.cargo.len()
            + usize::from(self.python.is_some())
            + self.custom_steps.len()
    }
}

/// Helper: parse a TOML-style array string like `["a", "b", "c"]`.
fn parse_toml_array(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// E8.2: pkg-config Integration
// ═══════════════════════════════════════════════════════════════════════

/// Result of a pkg-config query for a system library.
#[derive(Debug, Clone)]
pub struct PkgConfigResult {
    /// Package name.
    pub name: String,
    /// Installed version.
    pub version: String,
    /// Compiler flags (-I, -D).
    pub cflags: Vec<String>,
    /// Linker flags (-l, -L).
    pub libs: Vec<String>,
    /// Library directory paths.
    pub lib_dirs: Vec<String>,
    /// Include directory paths.
    pub include_dirs: Vec<String>,
}

/// Queries pkg-config for a system library (simulated).
///
/// Models the behavior of `pkg-config --cflags --libs <name>`.
#[derive(Debug)]
pub struct PkgConfigQuery {
    /// Simulated database of installed packages.
    packages: HashMap<String, PkgConfigResult>,
}

impl PkgConfigQuery {
    /// Creates a new query engine with no packages.
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    /// Creates a query engine pre-populated with common system packages.
    pub fn with_common_packages() -> Self {
        let mut query = Self::new();

        query.register(PkgConfigResult {
            name: "openssl".to_string(),
            version: "1.1.1w".to_string(),
            cflags: vec!["-I/usr/include/openssl".to_string()],
            libs: vec!["-lssl".to_string(), "-lcrypto".to_string()],
            lib_dirs: vec!["/usr/lib/x86_64-linux-gnu".to_string()],
            include_dirs: vec!["/usr/include/openssl".to_string()],
        });

        query.register(PkgConfigResult {
            name: "zlib".to_string(),
            version: "1.2.13".to_string(),
            cflags: vec!["-I/usr/include".to_string()],
            libs: vec!["-lz".to_string()],
            lib_dirs: vec!["/usr/lib".to_string()],
            include_dirs: vec!["/usr/include".to_string()],
        });

        query.register(PkgConfigResult {
            name: "libpng".to_string(),
            version: "1.6.40".to_string(),
            cflags: vec![
                "-I/usr/include/libpng16".to_string(),
                "-I/usr/include".to_string(),
            ],
            libs: vec!["-lpng16".to_string(), "-lz".to_string()],
            lib_dirs: vec!["/usr/lib".to_string()],
            include_dirs: vec!["/usr/include/libpng16".to_string()],
        });

        query.register(PkgConfigResult {
            name: "sqlite3".to_string(),
            version: "3.44.2".to_string(),
            cflags: vec!["-I/usr/include".to_string()],
            libs: vec!["-lsqlite3".to_string()],
            lib_dirs: vec!["/usr/lib".to_string()],
            include_dirs: vec!["/usr/include".to_string()],
        });

        query
    }

    /// Registers a package in the simulated database.
    pub fn register(&mut self, result: PkgConfigResult) {
        self.packages.insert(result.name.clone(), result);
    }

    /// Queries a package by name.
    pub fn query(&self, name: &str) -> Result<&PkgConfigResult, BuildError> {
        self.packages
            .get(name)
            .ok_or_else(|| BuildError::PackageNotFound(name.to_string()))
    }

    /// Queries a package by name with a version constraint.
    pub fn query_version(
        &self,
        name: &str,
        version_req: &str,
    ) -> Result<&PkgConfigResult, BuildError> {
        let result = self.query(name)?;
        if check_version_constraint(&result.version, version_req) {
            Ok(result)
        } else {
            Err(BuildError::VersionMismatch {
                package: name.to_string(),
                required: version_req.to_string(),
                found: result.version.clone(),
            })
        }
    }

    /// Returns the number of known packages.
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    /// Lists all known package names.
    pub fn list_packages(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.packages.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for PkgConfigQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks a simple version constraint (simulated).
///
/// Supports `>= X.Y.Z`, `= X.Y.Z`, and bare `X.Y.Z` (exact match).
fn check_version_constraint(installed: &str, constraint: &str) -> bool {
    let constraint = constraint.trim();

    if let Some(req) = constraint.strip_prefix(">=") {
        let req = req.trim();
        compare_versions(installed, req) >= 0
    } else if let Some(req) = constraint.strip_prefix('=') {
        let req = req.trim();
        compare_versions(installed, req) == 0
    } else {
        // Bare version = exact match
        compare_versions(installed, constraint) == 0
    }
}

/// Compares two version strings (major.minor.patch).
///
/// Returns -1 (a < b), 0 (a == b), or 1 (a > b).
fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let max_len = va.len().max(vb.len());

    for i in 0..max_len {
        let a_part = va.get(i).copied().unwrap_or(0);
        let b_part = vb.get(i).copied().unwrap_or(0);
        match a_part.cmp(&b_part) {
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    0
}

// ═══════════════════════════════════════════════════════════════════════
// E8.3: Python Venv Integration
// ═══════════════════════════════════════════════════════════════════════

/// Represents a discovered Python virtual environment.
#[derive(Debug, Clone)]
pub struct PythonVenv {
    /// Root path of the venv.
    pub root: String,
    /// Path to the Python interpreter binary.
    pub interpreter: String,
    /// Path to site-packages directory.
    pub site_packages: String,
    /// Path to pip executable.
    pub pip: String,
    /// Python version string.
    pub version: String,
    /// Installed packages (name -> version).
    pub installed_packages: HashMap<String, String>,
    /// Whether the venv was validated.
    pub valid: bool,
}

impl PythonVenv {
    /// Locates and validates a Python venv at the given path (simulated).
    ///
    /// Checks for interpreter, site-packages, and pip.
    pub fn locate(venv_path: &str) -> Result<Self, BuildError> {
        if venv_path.is_empty() {
            return Err(BuildError::PythonVenvInvalid {
                path: venv_path.to_string(),
                reason: "empty path".to_string(),
            });
        }

        // Simulated: in production we'd stat the filesystem
        let is_unix_style = !venv_path.contains('\\');
        let sep = if is_unix_style { "/" } else { "\\" };
        let bin_dir = if is_unix_style { "bin" } else { "Scripts" };

        let interpreter = format!("{venv_path}{sep}{bin_dir}{sep}python3");
        let pip = format!("{venv_path}{sep}{bin_dir}{sep}pip");
        let site_packages = format!("{venv_path}{sep}lib{sep}python3.11{sep}site-packages");

        Ok(Self {
            root: venv_path.to_string(),
            interpreter,
            site_packages,
            pip,
            version: "3.11.0".to_string(),
            installed_packages: HashMap::new(),
            valid: true,
        })
    }

    /// Simulates checking whether a package is installed in the venv.
    pub fn has_package(&self, name: &str) -> bool {
        self.installed_packages.contains_key(name)
    }

    /// Simulates installing a package via pip.
    pub fn install_package(&mut self, name: &str, version: &str) -> Result<(), BuildError> {
        if !self.valid {
            return Err(BuildError::PythonVenvInvalid {
                path: self.root.clone(),
                reason: "venv is not valid".to_string(),
            });
        }
        self.installed_packages
            .insert(name.to_string(), version.to_string());
        Ok(())
    }

    /// Returns linker flags needed for embedding Python.
    pub fn linker_flags(&self) -> LinkerFlags {
        let mut flags = LinkerFlags::new();
        flags.add_lib("python3.11");
        flags.add_lib_path(&format!("{}/lib", self.root));
        flags.add_include_path(&self.site_packages);
        flags
    }

    /// Returns the number of installed packages.
    pub fn package_count(&self) -> usize {
        self.installed_packages.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.4: CMake Integration
// ═══════════════════════════════════════════════════════════════════════

/// CMake build phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmakePhase {
    /// `cmake -S <src> -B <build>` — configure step.
    Configure,
    /// `cmake --build <build>` — compile step.
    Build,
    /// `cmake --install <build>` — install step.
    Install,
}

impl fmt::Display for CmakePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configure => write!(f, "configure"),
            Self::Build => write!(f, "build"),
            Self::Install => write!(f, "install"),
        }
    }
}

/// Drives a CMake build through configure/build/install phases (simulated).
#[derive(Debug, Clone)]
pub struct CmakeBuild {
    /// Source directory.
    pub source_dir: String,
    /// Build directory.
    pub build_dir: String,
    /// Install prefix.
    pub install_prefix: String,
    /// Build type.
    pub build_type: String,
    /// CMake defines (-D flags).
    pub defines: HashMap<String, String>,
    /// Generator (e.g., "Ninja", "Unix Makefiles").
    pub generator: Option<String>,
    /// Completed phases.
    pub completed_phases: Vec<CmakePhase>,
    /// Produced artifacts (library paths).
    pub artifacts: Vec<String>,
}

impl CmakeBuild {
    /// Creates a new CMake build configuration.
    pub fn new(source_dir: &str, build_dir: &str) -> Self {
        Self {
            source_dir: source_dir.to_string(),
            build_dir: build_dir.to_string(),
            install_prefix: format!("{build_dir}/install"),
            build_type: "Release".to_string(),
            defines: HashMap::new(),
            generator: None,
            completed_phases: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    /// Sets the build type.
    pub fn set_build_type(&mut self, build_type: &str) {
        self.build_type = build_type.to_string();
    }

    /// Adds a CMake define.
    pub fn add_define(&mut self, key: &str, value: &str) {
        self.defines.insert(key.to_string(), value.to_string());
    }

    /// Sets the generator.
    pub fn set_generator(&mut self, generator: &str) {
        self.generator = Some(generator.to_string());
    }

    /// Runs the configure phase (simulated).
    pub fn configure(&mut self) -> Result<(), BuildError> {
        if self.source_dir.is_empty() {
            return Err(BuildError::CmakeFailed {
                phase: CmakePhase::Configure,
                message: "source_dir is empty".to_string(),
            });
        }
        self.completed_phases.push(CmakePhase::Configure);
        Ok(())
    }

    /// Runs the build phase (simulated).
    pub fn build(&mut self) -> Result<(), BuildError> {
        if !self.completed_phases.contains(&CmakePhase::Configure) {
            return Err(BuildError::CmakeFailed {
                phase: CmakePhase::Build,
                message: "must configure before building".to_string(),
            });
        }
        self.completed_phases.push(CmakePhase::Build);
        // Simulate producing a library artifact.
        self.artifacts
            .push(format!("{}/lib/libproject.a", self.install_prefix));
        Ok(())
    }

    /// Runs the install phase (simulated).
    pub fn install(&mut self) -> Result<(), BuildError> {
        if !self.completed_phases.contains(&CmakePhase::Build) {
            return Err(BuildError::CmakeFailed {
                phase: CmakePhase::Install,
                message: "must build before installing".to_string(),
            });
        }
        self.completed_phases.push(CmakePhase::Install);
        Ok(())
    }

    /// Runs all three phases in order.
    pub fn run_all(&mut self) -> Result<(), BuildError> {
        self.configure()?;
        self.build()?;
        self.install()
    }

    /// Returns linker flags for the built library.
    pub fn linker_flags(&self) -> LinkerFlags {
        let mut flags = LinkerFlags::new();
        flags.add_lib("project");
        flags.add_lib_path(&format!("{}/lib", self.install_prefix));
        flags.add_include_path(&format!("{}/include", self.install_prefix));
        flags
    }

    /// Returns the command-line arguments that would be passed to cmake (simulated).
    pub fn configure_args(&self) -> Vec<String> {
        let mut args = vec![
            format!("-S{}", self.source_dir),
            format!("-B{}", self.build_dir),
            format!("-DCMAKE_BUILD_TYPE={}", self.build_type),
            format!("-DCMAKE_INSTALL_PREFIX={}", self.install_prefix),
        ];
        if let Some(ref generator) = self.generator {
            args.push(format!("-G{generator}"));
        }
        for (k, v) in &self.defines {
            args.push(format!("-D{k}={v}"));
        }
        args
    }

    /// Returns whether all phases completed successfully.
    pub fn is_complete(&self) -> bool {
        self.completed_phases.contains(&CmakePhase::Configure)
            && self.completed_phases.contains(&CmakePhase::Build)
            && self.completed_phases.contains(&CmakePhase::Install)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.5: Cargo Integration
// ═══════════════════════════════════════════════════════════════════════

/// Drives compilation of a Rust crate dependency (simulated).
#[derive(Debug, Clone)]
pub struct CargoBuild {
    /// Crate name.
    pub crate_name: String,
    /// Version requirement.
    pub version: String,
    /// Features to enable.
    pub features: Vec<String>,
    /// Library type to produce.
    pub lib_type: CargoLibType,
    /// Target triple (None = host).
    pub target: Option<String>,
    /// Build profile.
    pub profile: CargoProfile,
    /// Whether the build completed.
    pub built: bool,
    /// Output artifact path (set after build).
    pub output_path: Option<String>,
}

/// Cargo library output type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoLibType {
    /// Rust static library.
    Staticlib,
    /// C-compatible dynamic library.
    Cdylib,
    /// Rust dylib.
    Dylib,
}

impl fmt::Display for CargoLibType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Staticlib => write!(f, "staticlib"),
            Self::Cdylib => write!(f, "cdylib"),
            Self::Dylib => write!(f, "dylib"),
        }
    }
}

/// Cargo build profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoProfile {
    /// Debug profile (unoptimized).
    Dev,
    /// Release profile (optimized).
    Release,
}

impl CargoBuild {
    /// Creates a new Cargo build configuration.
    pub fn new(crate_name: &str, version: &str) -> Self {
        Self {
            crate_name: crate_name.to_string(),
            version: version.to_string(),
            features: Vec::new(),
            lib_type: CargoLibType::Staticlib,
            target: None,
            profile: CargoProfile::Release,
            built: false,
            output_path: None,
        }
    }

    /// Adds a feature to enable.
    pub fn add_feature(&mut self, feature: &str) {
        self.features.push(feature.to_string());
    }

    /// Sets the library type.
    pub fn set_lib_type(&mut self, lib_type: CargoLibType) {
        self.lib_type = lib_type;
    }

    /// Sets the target triple.
    pub fn set_target(&mut self, target: &str) {
        self.target = Some(target.to_string());
    }

    /// Runs the cargo build (simulated).
    pub fn build(&mut self) -> Result<(), BuildError> {
        if self.crate_name.is_empty() {
            return Err(BuildError::CargoFailed {
                message: "crate name is empty".to_string(),
            });
        }

        let target_dir = match &self.target {
            Some(t) => format!("target/{t}/release"),
            None => "target/release".to_string(),
        };

        let extension = match self.lib_type {
            CargoLibType::Staticlib => "a",
            CargoLibType::Cdylib | CargoLibType::Dylib => "so",
        };

        let lib_name = self.crate_name.replace('-', "_");
        self.output_path = Some(format!("{target_dir}/lib{lib_name}.{extension}"));
        self.built = true;
        Ok(())
    }

    /// Returns the cargo command-line arguments (simulated).
    pub fn build_args(&self) -> Vec<String> {
        let mut args = vec!["build".to_string()];

        if self.profile == CargoProfile::Release {
            args.push("--release".to_string());
        }

        if let Some(ref target) = self.target {
            args.push("--target".to_string());
            args.push(target.clone());
        }

        if !self.features.is_empty() {
            args.push("--features".to_string());
            args.push(self.features.join(","));
        }

        args
    }

    /// Returns linker flags for the built crate.
    pub fn linker_flags(&self) -> LinkerFlags {
        let mut flags = LinkerFlags::new();
        let lib_name = self.crate_name.replace('-', "_");
        flags.add_lib(&lib_name);
        if let Some(ref path) = self.output_path {
            // Extract the directory from the artifact path.
            if let Some(dir) = path.rsplit_once('/') {
                flags.add_lib_path(dir.0);
            }
        }
        flags
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.6: Linker Flag Management
// ═══════════════════════════════════════════════════════════════════════

/// Accumulates linker flags from all FFI build sources.
///
/// Collects `-l` (library names), `-L` (library search paths), and
/// `-I` (include search paths) from pkg-config results, CMake builds,
/// Cargo builds, Python venvs, and custom steps.
#[derive(Debug, Clone, Default)]
pub struct LinkerFlags {
    /// Libraries to link (-l).
    pub libs: Vec<String>,
    /// Library search paths (-L).
    pub lib_paths: Vec<String>,
    /// Include search paths (-I).
    pub include_paths: Vec<String>,
    /// Raw flags (passed through as-is).
    pub raw_flags: Vec<String>,
    /// Framework names (macOS -framework).
    pub frameworks: Vec<String>,
}

impl LinkerFlags {
    /// Creates empty linker flags.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a library name (without -l prefix).
    pub fn add_lib(&mut self, name: &str) {
        if !self.libs.contains(&name.to_string()) {
            self.libs.push(name.to_string());
        }
    }

    /// Adds a library search path (without -L prefix).
    pub fn add_lib_path(&mut self, path: &str) {
        if !self.lib_paths.contains(&path.to_string()) {
            self.lib_paths.push(path.to_string());
        }
    }

    /// Adds an include search path (without -I prefix).
    pub fn add_include_path(&mut self, path: &str) {
        if !self.include_paths.contains(&path.to_string()) {
            self.include_paths.push(path.to_string());
        }
    }

    /// Adds a raw linker flag.
    pub fn add_raw(&mut self, flag: &str) {
        self.raw_flags.push(flag.to_string());
    }

    /// Adds a macOS framework.
    pub fn add_framework(&mut self, name: &str) {
        if !self.frameworks.contains(&name.to_string()) {
            self.frameworks.push(name.to_string());
        }
    }

    /// Merges another `LinkerFlags` into this one, deduplicating entries.
    pub fn merge(&mut self, other: &LinkerFlags) {
        for lib in &other.libs {
            self.add_lib(lib);
        }
        for path in &other.lib_paths {
            self.add_lib_path(path);
        }
        for path in &other.include_paths {
            self.add_include_path(path);
        }
        for flag in &other.raw_flags {
            self.raw_flags.push(flag.clone());
        }
        for fw in &other.frameworks {
            self.add_framework(fw);
        }
    }

    /// Renders the flags as a command-line string for the linker.
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for path in &self.lib_paths {
            args.push(format!("-L{path}"));
        }
        for lib in &self.libs {
            args.push(format!("-l{lib}"));
        }
        for path in &self.include_paths {
            args.push(format!("-I{path}"));
        }
        for fw in &self.frameworks {
            args.push("-framework".to_string());
            args.push(fw.clone());
        }
        args.extend(self.raw_flags.clone());
        args
    }

    /// Returns the total number of flags.
    pub fn total_count(&self) -> usize {
        self.libs.len()
            + self.lib_paths.len()
            + self.include_paths.len()
            + self.raw_flags.len()
            + self.frameworks.len()
    }

    /// Returns true if there are no flags.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }
}

impl fmt::Display for LinkerFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_args().join(" "))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.7: Cross-Compilation
// ═══════════════════════════════════════════════════════════════════════

/// Well-known cross-compilation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownTarget {
    /// x86_64 Linux (GNU libc).
    X86_64LinuxGnu,
    /// AArch64 Linux (GNU libc).
    Aarch64LinuxGnu,
    /// ARMv7 Linux (hard-float).
    Armv7LinuxGnueabihf,
    /// RISC-V 64 Linux.
    Riscv64LinuxGnu,
    /// WASM32 (WASI).
    Wasm32Wasi,
    /// AArch64 macOS.
    Aarch64AppleDarwin,
    /// x86_64 Windows (MSVC).
    X86_64PcWindowsMsvc,
}

impl KnownTarget {
    /// Returns the target triple string.
    pub fn triple(&self) -> &'static str {
        match self {
            Self::X86_64LinuxGnu => "x86_64-unknown-linux-gnu",
            Self::Aarch64LinuxGnu => "aarch64-unknown-linux-gnu",
            Self::Armv7LinuxGnueabihf => "armv7-unknown-linux-gnueabihf",
            Self::Riscv64LinuxGnu => "riscv64gc-unknown-linux-gnu",
            Self::Wasm32Wasi => "wasm32-wasi",
            Self::Aarch64AppleDarwin => "aarch64-apple-darwin",
            Self::X86_64PcWindowsMsvc => "x86_64-pc-windows-msvc",
        }
    }

    /// Returns the default toolchain prefix for GCC/binutils.
    pub fn toolchain_prefix(&self) -> &'static str {
        match self {
            Self::X86_64LinuxGnu => "",
            Self::Aarch64LinuxGnu => "aarch64-linux-gnu-",
            Self::Armv7LinuxGnueabihf => "arm-linux-gnueabihf-",
            Self::Riscv64LinuxGnu => "riscv64-linux-gnu-",
            Self::Wasm32Wasi => "wasm32-wasi-",
            Self::Aarch64AppleDarwin => "aarch64-apple-darwin-",
            Self::X86_64PcWindowsMsvc => "",
        }
    }

    /// Attempts to parse a target triple string into a `KnownTarget`.
    pub fn from_triple(triple: &str) -> Option<Self> {
        match triple {
            "x86_64-unknown-linux-gnu" => Some(Self::X86_64LinuxGnu),
            "aarch64-unknown-linux-gnu" => Some(Self::Aarch64LinuxGnu),
            "armv7-unknown-linux-gnueabihf" => Some(Self::Armv7LinuxGnueabihf),
            "riscv64gc-unknown-linux-gnu" => Some(Self::Riscv64LinuxGnu),
            "wasm32-wasi" => Some(Self::Wasm32Wasi),
            "aarch64-apple-darwin" => Some(Self::Aarch64AppleDarwin),
            "x86_64-pc-windows-msvc" => Some(Self::X86_64PcWindowsMsvc),
            _ => None,
        }
    }
}

/// Cross-compilation configuration.
///
/// Holds the target triple, sysroot location, and toolchain paths needed
/// to cross-compile FFI dependencies for a non-host architecture.
#[derive(Debug, Clone)]
pub struct CrossTarget {
    /// Target triple (e.g., "aarch64-unknown-linux-gnu").
    pub triple: String,
    /// Known target variant (None for custom triples).
    pub known: Option<KnownTarget>,
    /// Path to the target sysroot.
    pub sysroot: String,
    /// Toolchain prefix (e.g., "aarch64-linux-gnu-").
    pub toolchain_prefix: String,
    /// Path to the C compiler for the target.
    pub cc: String,
    /// Path to the C++ compiler for the target.
    pub cxx: String,
    /// Path to the linker for the target.
    pub linker: String,
    /// Additional environment variables for the build.
    pub env: HashMap<String, String>,
}

impl CrossTarget {
    /// Creates a cross-target from a known target enum.
    pub fn from_known(known: KnownTarget, sysroot: &str) -> Self {
        let prefix = known.toolchain_prefix();
        Self {
            triple: known.triple().to_string(),
            known: Some(known),
            sysroot: sysroot.to_string(),
            toolchain_prefix: prefix.to_string(),
            cc: format!("{prefix}gcc"),
            cxx: format!("{prefix}g++"),
            linker: format!("{prefix}ld"),
            env: HashMap::new(),
        }
    }

    /// Creates a cross-target from a custom triple.
    pub fn from_triple(
        triple: &str,
        sysroot: &str,
        toolchain_prefix: &str,
    ) -> Result<Self, BuildError> {
        if triple.is_empty() {
            return Err(BuildError::UnsupportedTarget("empty triple".to_string()));
        }

        let known = KnownTarget::from_triple(triple);

        Ok(Self {
            triple: triple.to_string(),
            known,
            sysroot: sysroot.to_string(),
            toolchain_prefix: toolchain_prefix.to_string(),
            cc: format!("{toolchain_prefix}gcc"),
            cxx: format!("{toolchain_prefix}g++"),
            linker: format!("{toolchain_prefix}ld"),
            env: HashMap::new(),
        })
    }

    /// Returns the environment variables needed for cross-compilation.
    pub fn build_env(&self) -> HashMap<String, String> {
        let mut env = self.env.clone();
        env.insert("CC".to_string(), self.cc.clone());
        env.insert("CXX".to_string(), self.cxx.clone());
        env.insert(
            "CARGO_TARGET_DIR".to_string(),
            format!("target/{}", self.triple),
        );
        if !self.sysroot.is_empty() {
            env.insert("CMAKE_SYSROOT".to_string(), self.sysroot.clone());
            env.insert("PKG_CONFIG_SYSROOT_DIR".to_string(), self.sysroot.clone());
        }
        env
    }

    /// Returns CMake toolchain defines for this target.
    pub fn cmake_defines(&self) -> HashMap<String, String> {
        let mut defines = HashMap::new();
        defines.insert(
            "CMAKE_SYSTEM_NAME".to_string(),
            self.system_name().to_string(),
        );
        defines.insert("CMAKE_C_COMPILER".to_string(), self.cc.clone());
        defines.insert("CMAKE_CXX_COMPILER".to_string(), self.cxx.clone());
        if !self.sysroot.is_empty() {
            defines.insert("CMAKE_SYSROOT".to_string(), self.sysroot.clone());
        }
        defines
    }

    /// Returns the system name for this target (Linux, Darwin, Windows, etc.).
    pub fn system_name(&self) -> &str {
        if self.triple.contains("linux") {
            "Linux"
        } else if self.triple.contains("darwin") || self.triple.contains("apple") {
            "Darwin"
        } else if self.triple.contains("windows") {
            "Windows"
        } else if self.triple.contains("wasi") || self.triple.contains("wasm") {
            "WASI"
        } else {
            "Generic"
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.8: Hermetic Builds
// ═══════════════════════════════════════════════════════════════════════

/// Vendored dependency entry.
#[derive(Debug, Clone)]
pub struct VendoredDep {
    /// Dependency name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Path relative to vendor directory.
    pub path: String,
    /// Expected SHA-256 checksum.
    pub checksum: String,
    /// Whether the checksum has been verified.
    pub verified: bool,
}

/// Configuration for hermetic (offline, vendored) builds.
///
/// All dependencies are pre-downloaded and stored in a vendor directory
/// with checksums for reproducibility and security.
#[derive(Debug, Clone)]
pub struct VendorConfig {
    /// Root vendor directory.
    pub vendor_dir: String,
    /// Vendored dependencies.
    pub deps: Vec<VendoredDep>,
    /// Whether to enforce checksum verification.
    pub enforce_checksums: bool,
    /// Lockfile path for recording verified checksums.
    pub lockfile: Option<String>,
}

impl VendorConfig {
    /// Creates a new vendor configuration.
    pub fn new(vendor_dir: &str) -> Self {
        Self {
            vendor_dir: vendor_dir.to_string(),
            deps: Vec::new(),
            enforce_checksums: true,
            lockfile: None,
        }
    }

    /// Adds a vendored dependency.
    pub fn add_dep(&mut self, dep: VendoredDep) {
        self.deps.push(dep);
    }

    /// Verifies all dependency checksums (simulated).
    ///
    /// In production this would read files and compute SHA-256. Here we
    /// compare the `checksum` field against a simulated "actual" value.
    pub fn verify_all(&mut self) -> Result<(), Vec<BuildError>> {
        let mut errors = Vec::new();

        for dep in &mut self.deps {
            // Simulated: the actual checksum equals the expected one
            // unless the checksum is "INVALID" (for testing failures).
            let actual = if dep.checksum == "INVALID" {
                "0000000000000000000000000000000000000000000000000000000000000000".to_string()
            } else {
                dep.checksum.clone()
            };

            if actual != dep.checksum {
                errors.push(BuildError::ChecksumMismatch {
                    file: dep.path.clone(),
                    expected: dep.checksum.clone(),
                    actual,
                });
                dep.verified = false;
            } else {
                dep.verified = true;
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns the number of verified dependencies.
    pub fn verified_count(&self) -> usize {
        self.deps.iter().filter(|d| d.verified).count()
    }

    /// Returns the total number of vendored dependencies.
    pub fn total_count(&self) -> usize {
        self.deps.len()
    }

    /// Returns linker flags pointing to the vendor directory.
    pub fn linker_flags(&self) -> LinkerFlags {
        let mut flags = LinkerFlags::new();
        flags.add_lib_path(&format!("{}/lib", self.vendor_dir));
        flags.add_include_path(&format!("{}/include", self.vendor_dir));
        for dep in &self.deps {
            if dep.verified {
                flags.add_lib(&dep.name);
            }
        }
        flags
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.9: CI Integration
// ═══════════════════════════════════════════════════════════════════════

/// CI provider target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiProvider {
    /// GitHub Actions.
    GitHubActions,
    /// GitLab CI/CD.
    GitLabCi,
}

impl fmt::Display for CiProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHubActions => write!(f, "GitHub Actions"),
            Self::GitLabCi => write!(f, "GitLab CI/CD"),
        }
    }
}

/// CI configuration for FFI dependency installation.
///
/// Generates CI workflow YAML that installs required system packages,
/// sets up toolchains, and runs the FFI build steps.
#[derive(Debug, Clone)]
pub struct CiConfig {
    /// Target CI provider.
    pub provider: CiProvider,
    /// OS images to run on (e.g., "ubuntu-22.04", "macos-14").
    pub os_images: Vec<String>,
    /// System packages to install via apt/brew.
    pub system_packages: Vec<String>,
    /// Environment variables for the workflow.
    pub env_vars: HashMap<String, String>,
    /// Build commands to run.
    pub build_commands: Vec<String>,
    /// Test commands to run.
    pub test_commands: Vec<String>,
    /// Whether to cache dependencies.
    pub cache_enabled: bool,
    /// Cache key prefix.
    pub cache_key: String,
}

impl CiConfig {
    /// Creates a new CI configuration for the given provider.
    pub fn new(provider: CiProvider) -> Self {
        Self {
            provider,
            os_images: Vec::new(),
            system_packages: Vec::new(),
            env_vars: HashMap::new(),
            build_commands: Vec::new(),
            test_commands: Vec::new(),
            cache_enabled: true,
            cache_key: "ffi-deps-v1".to_string(),
        }
    }

    /// Creates a CI config from an `FfiConfig`, inferring required packages.
    pub fn from_ffi_config(config: &FfiConfig, provider: CiProvider) -> Self {
        let mut ci = Self::new(provider);
        ci.os_images.push("ubuntu-22.04".to_string());

        // Infer system packages from pkg-config entries.
        for pkg in &config.pkg_config {
            let system_pkg = infer_system_package(&pkg.name);
            if !ci.system_packages.contains(&system_pkg) {
                ci.system_packages.push(system_pkg);
            }
        }

        // If cmake entries exist, ensure cmake is installed.
        if !config.cmake.is_empty() && !ci.system_packages.contains(&"cmake".to_string()) {
            ci.system_packages.push("cmake".to_string());
        }

        // Standard build commands.
        ci.build_commands.push("fj build".to_string());
        ci.test_commands.push("fj test".to_string());

        ci
    }

    /// Adds a system package dependency.
    pub fn add_system_package(&mut self, pkg: &str) {
        if !self.system_packages.contains(&pkg.to_string()) {
            self.system_packages.push(pkg.to_string());
        }
    }

    /// Generates a GitHub Actions YAML workflow (simulated).
    pub fn generate_github_actions_yaml(&self) -> String {
        let mut yaml = String::new();
        yaml.push_str("name: FFI Build\n\n");
        yaml.push_str(
            "on:\n  push:\n    branches: [main]\n  pull_request:\n    branches: [main]\n\n",
        );

        yaml.push_str("jobs:\n  build:\n");
        yaml.push_str("    runs-on: ");
        if let Some(os) = self.os_images.first() {
            yaml.push_str(os);
        } else {
            yaml.push_str("ubuntu-latest");
        }
        yaml.push('\n');

        // Environment variables.
        if !self.env_vars.is_empty() {
            yaml.push_str("    env:\n");
            let mut sorted_env: Vec<_> = self.env_vars.iter().collect();
            sorted_env.sort_by_key(|(k, _)| (*k).clone());
            for (k, v) in &sorted_env {
                yaml.push_str(&format!("      {k}: {v}\n"));
            }
        }

        yaml.push_str("    steps:\n");
        yaml.push_str("      - uses: actions/checkout@v4\n");

        // Cache step.
        if self.cache_enabled {
            yaml.push_str("      - uses: actions/cache@v4\n");
            yaml.push_str("        with:\n");
            yaml.push_str(&format!("          key: {}\n", self.cache_key));
            yaml.push_str("          path: |\n");
            yaml.push_str("            ~/.cache/fj\n");
            yaml.push_str("            vendor/\n");
        }

        // Install system packages.
        if !self.system_packages.is_empty() {
            yaml.push_str("      - name: Install system dependencies\n");
            yaml.push_str("        run: |\n");
            yaml.push_str("          sudo apt-get update\n");
            yaml.push_str(&format!(
                "          sudo apt-get install -y {}\n",
                self.system_packages.join(" ")
            ));
        }

        // Build commands.
        if !self.build_commands.is_empty() {
            yaml.push_str("      - name: Build\n");
            yaml.push_str("        run: |\n");
            for cmd in &self.build_commands {
                yaml.push_str(&format!("          {cmd}\n"));
            }
        }

        // Test commands.
        if !self.test_commands.is_empty() {
            yaml.push_str("      - name: Test\n");
            yaml.push_str("        run: |\n");
            for cmd in &self.test_commands {
                yaml.push_str(&format!("          {cmd}\n"));
            }
        }

        yaml
    }

    /// Generates a GitLab CI YAML config (simulated).
    pub fn generate_gitlab_ci_yaml(&self) -> String {
        let mut yaml = String::new();
        yaml.push_str("stages:\n  - build\n  - test\n\n");

        if let Some(os) = self.os_images.first() {
            yaml.push_str(&format!("image: {os}\n\n"));
        }

        // Build job.
        yaml.push_str("build:\n  stage: build\n");
        if !self.system_packages.is_empty() {
            yaml.push_str("  before_script:\n");
            yaml.push_str("    - apt-get update\n");
            yaml.push_str(&format!(
                "    - apt-get install -y {}\n",
                self.system_packages.join(" ")
            ));
        }
        yaml.push_str("  script:\n");
        for cmd in &self.build_commands {
            yaml.push_str(&format!("    - {cmd}\n"));
        }

        // Test job.
        yaml.push_str("\ntest:\n  stage: test\n  script:\n");
        for cmd in &self.test_commands {
            yaml.push_str(&format!("    - {cmd}\n"));
        }

        yaml
    }

    /// Generates YAML for the configured provider.
    pub fn generate_yaml(&self) -> String {
        match self.provider {
            CiProvider::GitHubActions => self.generate_github_actions_yaml(),
            CiProvider::GitLabCi => self.generate_gitlab_ci_yaml(),
        }
    }
}

/// Infers the apt package name from a pkg-config package name.
fn infer_system_package(pkg_name: &str) -> String {
    match pkg_name {
        "openssl" => "libssl-dev".to_string(),
        "zlib" => "zlib1g-dev".to_string(),
        "libpng" => "libpng-dev".to_string(),
        "sqlite3" => "libsqlite3-dev".to_string(),
        "libcurl" => "libcurl4-openssl-dev".to_string(),
        "libffi" => "libffi-dev".to_string(),
        other => format!("lib{other}-dev"),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Build Plan — Ordered build steps
// ═══════════════════════════════════════════════════════════════════════

/// A single step in the FFI build plan.
#[derive(Debug, Clone)]
pub enum BuildStep {
    /// Query pkg-config for a system library.
    PkgConfig {
        name: String,
        version: Option<String>,
    },
    /// Run a CMake configure/build/install.
    CMake {
        source_dir: String,
        build_type: String,
        defines: HashMap<String, String>,
    },
    /// Run a Cargo build for a Rust crate.
    Cargo {
        crate_name: String,
        version: String,
        features: Vec<String>,
        lib_type: CargoLibType,
    },
    /// Set up a Python virtual environment.
    PythonVenv { path: String, packages: Vec<String> },
    /// Run a custom build command.
    Custom {
        name: String,
        command: String,
        args: Vec<String>,
    },
}

impl fmt::Display for BuildStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PkgConfig { name, version } => {
                write!(f, "pkg-config {name}")?;
                if let Some(v) = version {
                    write!(f, " ({v})")?;
                }
                Ok(())
            }
            Self::CMake {
                source_dir,
                build_type,
                ..
            } => write!(f, "cmake {source_dir} [{build_type}]"),
            Self::Cargo {
                crate_name,
                version,
                ..
            } => write!(f, "cargo {crate_name} {version}"),
            Self::PythonVenv { path, .. } => write!(f, "python-venv {path}"),
            Self::Custom { name, .. } => write!(f, "custom: {name}"),
        }
    }
}

/// An ordered plan of build steps for all FFI dependencies.
///
/// Created from an `FfiConfig`, this plan represents the sequence of
/// operations needed to prepare all FFI dependencies for linking.
#[derive(Debug, Clone)]
pub struct FfiBuildPlan {
    /// Ordered build steps.
    pub steps: Vec<BuildStep>,
    /// Accumulated linker flags from all completed steps.
    pub flags: LinkerFlags,
    /// Cross-compilation target (None = host).
    pub cross_target: Option<CrossTarget>,
    /// Number of steps completed.
    pub completed: usize,
    /// Errors encountered during execution.
    pub errors: Vec<BuildError>,
}

impl FfiBuildPlan {
    /// Creates a build plan from an `FfiConfig`.
    pub fn from_config(config: &FfiConfig) -> Self {
        let mut steps = Vec::new();

        // 1. pkg-config queries first (fastest, just queries).
        for pkg in &config.pkg_config {
            steps.push(BuildStep::PkgConfig {
                name: pkg.name.clone(),
                version: pkg.version.clone(),
            });
        }

        // 2. CMake builds (may take time, need configure/build/install).
        for cmake in &config.cmake {
            steps.push(BuildStep::CMake {
                source_dir: cmake.source_dir.clone(),
                build_type: cmake.build_type.clone(),
                defines: cmake.defines.clone(),
            });
        }

        // 3. Cargo builds.
        for cargo in &config.cargo {
            steps.push(BuildStep::Cargo {
                crate_name: cargo.name.clone(),
                version: cargo.version.clone(),
                features: cargo.features.clone(),
                lib_type: CargoLibType::Staticlib,
            });
        }

        // 4. Python venv.
        if let Some(ref py) = config.python {
            steps.push(BuildStep::PythonVenv {
                path: py.path.clone(),
                packages: py.packages.clone(),
            });
        }

        // 5. Custom steps.
        for custom in &config.custom_steps {
            steps.push(BuildStep::Custom {
                name: custom.name.clone(),
                command: custom.command.clone(),
                args: custom.args.clone(),
            });
        }

        // Set up cross-compilation if configured.
        let cross_target = config.cross.as_ref().map(|c| {
            let prefix = c.toolchain.as_deref().unwrap_or("");
            CrossTarget::from_triple(&c.target, c.sysroot.as_deref().unwrap_or(""), prefix)
                .unwrap_or_else(|_| CrossTarget::from_known(KnownTarget::X86_64LinuxGnu, ""))
        });

        // Merge static flags from config.
        let mut flags = LinkerFlags::new();
        for lib in &config.libs {
            flags.add_lib(lib);
        }
        for path in &config.lib_paths {
            flags.add_lib_path(path);
        }
        for path in &config.include_paths {
            flags.add_include_path(path);
        }

        Self {
            steps,
            flags,
            cross_target,
            completed: 0,
            errors: Vec::new(),
        }
    }

    /// Executes all build steps in order (simulated).
    ///
    /// Each step contributes linker flags to the accumulated `flags` field.
    pub fn execute(&mut self, pkg_config: &PkgConfigQuery) -> Result<(), Vec<BuildError>> {
        for step in &self.steps {
            match step {
                BuildStep::PkgConfig { name, version } => {
                    let result = if let Some(v) = version {
                        pkg_config.query_version(name, v)
                    } else {
                        pkg_config.query(name)
                    };

                    match result {
                        Ok(pkg) => {
                            for lib in &pkg.libs {
                                // Strip -l prefix if present.
                                let lib_name = lib.strip_prefix("-l").unwrap_or(lib);
                                self.flags.add_lib(lib_name);
                            }
                            for dir in &pkg.lib_dirs {
                                self.flags.add_lib_path(dir);
                            }
                            for dir in &pkg.include_dirs {
                                self.flags.add_include_path(dir);
                            }
                            self.completed += 1;
                        }
                        Err(e) => self.errors.push(e),
                    }
                }
                BuildStep::CMake {
                    source_dir,
                    build_type,
                    defines,
                } => {
                    let build_dir = format!("{source_dir}/build");
                    let mut cmake = CmakeBuild::new(source_dir, &build_dir);
                    cmake.set_build_type(build_type);
                    for (k, v) in defines {
                        cmake.add_define(k, v);
                    }
                    match cmake.run_all() {
                        Ok(()) => {
                            self.flags.merge(&cmake.linker_flags());
                            self.completed += 1;
                        }
                        Err(e) => self.errors.push(e),
                    }
                }
                BuildStep::Cargo {
                    crate_name,
                    version,
                    features,
                    lib_type,
                } => {
                    let mut cargo = CargoBuild::new(crate_name, version);
                    cargo.set_lib_type(*lib_type);
                    for feat in features {
                        cargo.add_feature(feat);
                    }
                    if let Some(ref ct) = self.cross_target {
                        cargo.set_target(&ct.triple);
                    }
                    match cargo.build() {
                        Ok(()) => {
                            self.flags.merge(&cargo.linker_flags());
                            self.completed += 1;
                        }
                        Err(e) => self.errors.push(e),
                    }
                }
                BuildStep::PythonVenv { path, packages } => match PythonVenv::locate(path) {
                    Ok(mut venv) => {
                        for pkg in packages {
                            let _ = venv.install_package(pkg, "latest");
                        }
                        self.flags.merge(&venv.linker_flags());
                        self.completed += 1;
                    }
                    Err(e) => self.errors.push(e),
                },
                BuildStep::Custom { name, .. } => {
                    // Custom steps are always "successful" in simulation.
                    let _ = name;
                    self.completed += 1;
                }
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    /// Returns the total number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Returns whether all steps completed successfully.
    pub fn is_complete(&self) -> bool {
        self.completed == self.steps.len() && self.errors.is_empty()
    }

    /// Returns a summary string.
    pub fn summary(&self) -> String {
        format!(
            "{}/{} steps complete, {} errors, {} linker flags",
            self.completed,
            self.steps.len(),
            self.errors.len(),
            self.flags.total_count(),
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// E8.10: Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── E8.1: FfiConfig ──

    #[test]
    fn e8_1_ffi_config_parse_toml() {
        let toml = r#"
[ffi]
targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
libs = ["ssl", "crypto", "z"]
include_paths = ["/usr/include", "./vendor/include"]
lib_paths = ["/usr/lib"]
"#;
        let config = FfiConfig::from_toml_str(toml).unwrap();
        assert_eq!(config.targets.len(), 2);
        assert_eq!(config.libs, vec!["ssl", "crypto", "z"]);
        assert_eq!(config.include_paths.len(), 2);
        assert_eq!(config.lib_paths, vec!["/usr/lib"]);
    }

    #[test]
    fn e8_1_ffi_config_validate_catches_duplicates() {
        let mut config = FfiConfig::default();
        config.libs = vec!["ssl".into(), "ssl".into()];
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BuildError::DuplicateLib(n) if n == "ssl"));
    }

    #[test]
    fn e8_1_ffi_config_dependency_count() {
        let mut config = FfiConfig::default();
        config.pkg_config.push(PkgConfigEntry {
            name: "openssl".into(),
            version: None,
        });
        config.cmake.push(CmakeEntry {
            source_dir: "./vendor/foo".into(),
            build_type: "Release".into(),
            defines: HashMap::new(),
            install_prefix: None,
        });
        config.python = Some(PythonVenvEntry {
            path: ".venv".into(),
            packages: vec![],
            min_version: None,
        });
        assert_eq!(config.dependency_count(), 3);
    }

    // ── E8.2: pkg-config ──

    #[test]
    fn e8_2_pkg_config_query_found() {
        let query = PkgConfigQuery::with_common_packages();
        let result = query.query("openssl").unwrap();
        assert_eq!(result.name, "openssl");
        assert_eq!(result.version, "1.1.1w");
        assert!(result.libs.contains(&"-lssl".to_string()));
        assert!(result.libs.contains(&"-lcrypto".to_string()));
    }

    #[test]
    fn e8_2_pkg_config_query_not_found() {
        let query = PkgConfigQuery::new();
        let err = query.query("nonexistent").unwrap_err();
        assert!(matches!(err, BuildError::PackageNotFound(n) if n == "nonexistent"));
    }

    #[test]
    fn e8_2_pkg_config_version_constraint() {
        let query = PkgConfigQuery::with_common_packages();
        // >= 1.1.0 should succeed (installed is 1.1.1w but version parsing treats trailing as 0)
        assert!(query.query_version("zlib", ">= 1.2.0").is_ok());
        // Exact version match
        assert!(query.query_version("zlib", "= 1.2.13").is_ok());
        // Version too high
        let err = query.query_version("zlib", ">= 999.0.0").unwrap_err();
        assert!(matches!(err, BuildError::VersionMismatch { .. }));
    }

    // ── E8.3: Python venv ──

    #[test]
    fn e8_3_python_venv_locate() {
        let venv = PythonVenv::locate("/home/user/.venvs/ml").unwrap();
        assert_eq!(venv.root, "/home/user/.venvs/ml");
        assert!(venv.interpreter.contains("python3"));
        assert!(venv.site_packages.contains("site-packages"));
        assert!(venv.pip.contains("pip"));
        assert!(venv.valid);
    }

    #[test]
    fn e8_3_python_venv_install_package() {
        let mut venv = PythonVenv::locate("/tmp/venv").unwrap();
        assert!(!venv.has_package("numpy"));
        venv.install_package("numpy", "1.26.0").unwrap();
        assert!(venv.has_package("numpy"));
        assert_eq!(venv.package_count(), 1);
    }

    #[test]
    fn e8_3_python_venv_linker_flags() {
        let venv = PythonVenv::locate("/opt/venv").unwrap();
        let flags = venv.linker_flags();
        assert!(flags.libs.contains(&"python3.11".to_string()));
        assert!(!flags.lib_paths.is_empty());
    }

    // ── E8.4: CMake ──

    #[test]
    fn e8_4_cmake_full_build() {
        let mut cmake = CmakeBuild::new("./vendor/libfoo", "./build/libfoo");
        cmake.set_build_type("Release");
        cmake.add_define("BUILD_SHARED_LIBS", "OFF");

        cmake.configure().unwrap();
        assert!(cmake.completed_phases.contains(&CmakePhase::Configure));

        cmake.build().unwrap();
        assert!(!cmake.artifacts.is_empty());

        cmake.install().unwrap();
        assert!(cmake.is_complete());
    }

    #[test]
    fn e8_4_cmake_build_before_configure_fails() {
        let mut cmake = CmakeBuild::new("./src", "./build");
        let err = cmake.build().unwrap_err();
        assert!(matches!(
            err,
            BuildError::CmakeFailed {
                phase: CmakePhase::Build,
                ..
            }
        ));
    }

    #[test]
    fn e8_4_cmake_configure_args() {
        let mut cmake = CmakeBuild::new("./src", "./build");
        cmake.set_generator("Ninja");
        cmake.add_define("FOO", "bar");
        let args = cmake.configure_args();
        assert!(args.iter().any(|a| a.contains("-S")));
        assert!(args.iter().any(|a| a.contains("-GNinja")));
        assert!(args.iter().any(|a| a.contains("-DFOO=bar")));
    }

    // ── E8.5: Cargo ──

    #[test]
    fn e8_5_cargo_build_staticlib() {
        let mut cargo = CargoBuild::new("serde", "1.0.203");
        cargo.add_feature("derive");
        cargo.set_lib_type(CargoLibType::Staticlib);
        cargo.build().unwrap();
        assert!(cargo.built);
        let path = cargo.output_path.as_ref().unwrap();
        assert!(path.ends_with(".a"));
        assert!(path.contains("libserde"));
    }

    #[test]
    fn e8_5_cargo_build_cdylib_cross() {
        let mut cargo = CargoBuild::new("my-ffi-lib", "0.1.0");
        cargo.set_lib_type(CargoLibType::Cdylib);
        cargo.set_target("aarch64-unknown-linux-gnu");
        cargo.build().unwrap();
        let path = cargo.output_path.as_ref().unwrap();
        assert!(path.contains("aarch64-unknown-linux-gnu"));
        assert!(path.ends_with(".so"));
    }

    #[test]
    fn e8_5_cargo_build_args() {
        let mut cargo = CargoBuild::new("tokio", "1.0");
        cargo.add_feature("full");
        cargo.set_target("riscv64gc-unknown-linux-gnu");
        let args = cargo.build_args();
        assert!(args.contains(&"--release".to_string()));
        assert!(args.contains(&"--target".to_string()));
        assert!(args.contains(&"--features".to_string()));
    }

    // ── E8.6: Linker flags ──

    #[test]
    fn e8_6_linker_flags_merge_dedup() {
        let mut a = LinkerFlags::new();
        a.add_lib("ssl");
        a.add_lib("crypto");
        a.add_lib_path("/usr/lib");

        let mut b = LinkerFlags::new();
        b.add_lib("ssl"); // duplicate
        b.add_lib("z");
        b.add_lib_path("/usr/lib"); // duplicate
        b.add_lib_path("/usr/local/lib");

        a.merge(&b);
        assert_eq!(a.libs, vec!["ssl", "crypto", "z"]);
        assert_eq!(a.lib_paths, vec!["/usr/lib", "/usr/local/lib"]);
    }

    #[test]
    fn e8_6_linker_flags_to_args() {
        let mut flags = LinkerFlags::new();
        flags.add_lib_path("/usr/lib");
        flags.add_lib("ssl");
        flags.add_include_path("/usr/include");
        flags.add_framework("Security");
        flags.add_raw("-Wl,-rpath,/opt/lib");

        let args = flags.to_args();
        assert!(args.contains(&"-L/usr/lib".to_string()));
        assert!(args.contains(&"-lssl".to_string()));
        assert!(args.contains(&"-I/usr/include".to_string()));
        assert!(args.contains(&"-framework".to_string()));
        assert!(args.contains(&"Security".to_string()));
        assert!(args.contains(&"-Wl,-rpath,/opt/lib".to_string()));
    }

    #[test]
    fn e8_6_linker_flags_display() {
        let mut flags = LinkerFlags::new();
        flags.add_lib_path("/lib");
        flags.add_lib("z");
        let s = format!("{flags}");
        assert!(s.contains("-L/lib"));
        assert!(s.contains("-lz"));
    }

    // ── E8.7: Cross-compilation ──

    #[test]
    fn e8_7_cross_target_from_known() {
        let ct = CrossTarget::from_known(KnownTarget::Aarch64LinuxGnu, "/usr/aarch64-linux-gnu");
        assert_eq!(ct.triple, "aarch64-unknown-linux-gnu");
        assert_eq!(ct.toolchain_prefix, "aarch64-linux-gnu-");
        assert_eq!(ct.cc, "aarch64-linux-gnu-gcc");
        assert_eq!(ct.system_name(), "Linux");
    }

    #[test]
    fn e8_7_cross_target_build_env() {
        let ct = CrossTarget::from_known(KnownTarget::Riscv64LinuxGnu, "/opt/sysroot");
        let env = ct.build_env();
        assert_eq!(env.get("CC").unwrap(), "riscv64-linux-gnu-gcc");
        assert!(env.get("CMAKE_SYSROOT").unwrap().contains("sysroot"));
    }

    #[test]
    fn e8_7_cross_target_cmake_defines() {
        let ct = CrossTarget::from_known(KnownTarget::Armv7LinuxGnueabihf, "/arm-sysroot");
        let defines = ct.cmake_defines();
        assert_eq!(defines.get("CMAKE_SYSTEM_NAME").unwrap(), "Linux");
        assert!(defines.get("CMAKE_C_COMPILER").unwrap().contains("gcc"));
        assert!(
            defines
                .get("CMAKE_SYSROOT")
                .unwrap()
                .contains("arm-sysroot")
        );
    }

    #[test]
    fn e8_7_known_target_roundtrip() {
        let targets = [
            KnownTarget::X86_64LinuxGnu,
            KnownTarget::Aarch64LinuxGnu,
            KnownTarget::Wasm32Wasi,
            KnownTarget::Aarch64AppleDarwin,
        ];
        for target in &targets {
            let triple = target.triple();
            let parsed = KnownTarget::from_triple(triple).unwrap();
            assert_eq!(parsed.triple(), triple);
        }
    }

    // ── E8.8: Hermetic builds ──

    #[test]
    fn e8_8_vendor_config_verify_all() {
        let mut vendor = VendorConfig::new("./vendor");
        vendor.add_dep(VendoredDep {
            name: "zlib".into(),
            version: "1.2.13".into(),
            path: "vendor/zlib-1.2.13".into(),
            checksum: "abcdef1234567890".into(),
            verified: false,
        });
        vendor.add_dep(VendoredDep {
            name: "openssl".into(),
            version: "1.1.1w".into(),
            path: "vendor/openssl-1.1.1w".into(),
            checksum: "fedcba0987654321".into(),
            verified: false,
        });

        vendor.verify_all().unwrap();
        assert_eq!(vendor.verified_count(), 2);
        assert_eq!(vendor.total_count(), 2);
    }

    #[test]
    fn e8_8_vendor_checksum_mismatch() {
        let mut vendor = VendorConfig::new("./vendor");
        vendor.add_dep(VendoredDep {
            name: "badlib".into(),
            version: "0.1.0".into(),
            path: "vendor/badlib".into(),
            checksum: "INVALID".into(),
            verified: false,
        });

        let errors = vendor.verify_all().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BuildError::ChecksumMismatch { .. }));
    }

    #[test]
    fn e8_8_vendor_linker_flags() {
        let mut vendor = VendorConfig::new("/opt/vendor");
        vendor.add_dep(VendoredDep {
            name: "mylib".into(),
            version: "1.0".into(),
            path: "vendor/mylib".into(),
            checksum: "abc123".into(),
            verified: true,
        });
        let flags = vendor.linker_flags();
        assert!(flags.libs.contains(&"mylib".to_string()));
        assert!(flags.lib_paths.contains(&"/opt/vendor/lib".to_string()));
    }

    // ── E8.9: CI integration ──

    #[test]
    fn e8_9_ci_config_from_ffi_config() {
        let mut config = FfiConfig::default();
        config.pkg_config.push(PkgConfigEntry {
            name: "openssl".into(),
            version: Some(">= 1.1.0".into()),
        });
        config.cmake.push(CmakeEntry {
            source_dir: "./vendor/foo".into(),
            build_type: "Release".into(),
            defines: HashMap::new(),
            install_prefix: None,
        });

        let ci = CiConfig::from_ffi_config(&config, CiProvider::GitHubActions);
        assert!(ci.system_packages.contains(&"libssl-dev".to_string()));
        assert!(ci.system_packages.contains(&"cmake".to_string()));
    }

    #[test]
    fn e8_9_github_actions_yaml_generation() {
        let mut ci = CiConfig::new(CiProvider::GitHubActions);
        ci.os_images.push("ubuntu-22.04".into());
        ci.system_packages.push("libssl-dev".into());
        ci.build_commands.push("fj build".into());
        ci.test_commands.push("fj test".into());

        let yaml = ci.generate_github_actions_yaml();
        assert!(yaml.contains("name: FFI Build"));
        assert!(yaml.contains("ubuntu-22.04"));
        assert!(yaml.contains("libssl-dev"));
        assert!(yaml.contains("fj build"));
        assert!(yaml.contains("fj test"));
        assert!(yaml.contains("actions/checkout@v4"));
        assert!(yaml.contains("actions/cache@v4"));
    }

    #[test]
    fn e8_9_gitlab_ci_yaml_generation() {
        let mut ci = CiConfig::new(CiProvider::GitLabCi);
        ci.os_images.push("ubuntu-22.04".into());
        ci.system_packages.push("cmake".into());
        ci.build_commands.push("fj build".into());
        ci.test_commands.push("fj test".into());

        let yaml = ci.generate_gitlab_ci_yaml();
        assert!(yaml.contains("stages:"));
        assert!(yaml.contains("image: ubuntu-22.04"));
        assert!(yaml.contains("cmake"));
        assert!(yaml.contains("fj build"));
    }

    // ── Build plan integration ──

    #[test]
    fn e8_10_build_plan_from_config() {
        let mut config = FfiConfig::default();
        config.pkg_config.push(PkgConfigEntry {
            name: "zlib".into(),
            version: None,
        });
        config.cmake.push(CmakeEntry {
            source_dir: "./vendor/foo".into(),
            build_type: "Release".into(),
            defines: HashMap::new(),
            install_prefix: None,
        });
        config.cargo.push(CargoEntry {
            name: "serde".into(),
            version: "1.0".into(),
            features: vec!["derive".into()],
            lib_type: "staticlib".into(),
        });
        config.libs = vec!["m".into()];

        let plan = FfiBuildPlan::from_config(&config);
        assert_eq!(plan.step_count(), 3);
        assert!(plan.flags.libs.contains(&"m".to_string()));
    }

    #[test]
    fn e8_10_build_plan_execute_success() {
        let mut config = FfiConfig::default();
        config.pkg_config.push(PkgConfigEntry {
            name: "openssl".into(),
            version: None,
        });
        config.pkg_config.push(PkgConfigEntry {
            name: "zlib".into(),
            version: None,
        });

        let pkg_config = PkgConfigQuery::with_common_packages();
        let mut plan = FfiBuildPlan::from_config(&config);
        plan.execute(&pkg_config).unwrap();

        assert!(plan.is_complete());
        assert_eq!(plan.completed, 2);
        assert!(plan.flags.libs.contains(&"ssl".to_string()));
        assert!(plan.flags.libs.contains(&"z".to_string()));
    }

    #[test]
    fn e8_10_build_plan_execute_partial_failure() {
        let mut config = FfiConfig::default();
        config.pkg_config.push(PkgConfigEntry {
            name: "zlib".into(),
            version: None,
        });
        config.pkg_config.push(PkgConfigEntry {
            name: "nonexistent".into(),
            version: None,
        });

        let pkg_config = PkgConfigQuery::with_common_packages();
        let mut plan = FfiBuildPlan::from_config(&config);
        let errors = plan.execute(&pkg_config).unwrap_err();

        assert_eq!(errors.len(), 1);
        assert_eq!(plan.completed, 1); // zlib succeeded
        assert!(!plan.is_complete());
    }

    #[test]
    fn e8_10_build_plan_summary() {
        let config = FfiConfig::default();
        let plan = FfiBuildPlan::from_config(&config);
        let summary = plan.summary();
        assert!(summary.contains("0/0 steps complete"));
        assert!(summary.contains("0 errors"));
    }

    #[test]
    fn e8_10_build_step_display() {
        let step = BuildStep::PkgConfig {
            name: "openssl".into(),
            version: Some(">= 1.1.0".into()),
        };
        let s = format!("{step}");
        assert!(s.contains("pkg-config openssl"));
        assert!(s.contains(">= 1.1.0"));

        let step2 = BuildStep::Cargo {
            crate_name: "serde".into(),
            version: "1.0".into(),
            features: vec![],
            lib_type: CargoLibType::Staticlib,
        };
        assert!(format!("{step2}").contains("cargo serde 1.0"));
    }

    #[test]
    fn e8_10_error_display() {
        let err = BuildError::PackageNotFound("libfoo".into());
        assert_eq!(format!("{err}"), "pkg-config: package 'libfoo' not found");

        let err2 = BuildError::CmakeFailed {
            phase: CmakePhase::Build,
            message: "compilation failed".into(),
        };
        assert!(format!("{err2}").contains("cmake build failed"));
    }

    #[test]
    fn e8_10_version_compare() {
        assert_eq!(compare_versions("1.2.3", "1.2.3"), 0);
        assert_eq!(compare_versions("1.2.4", "1.2.3"), 1);
        assert_eq!(compare_versions("1.1.0", "1.2.0"), -1);
        assert_eq!(compare_versions("2.0.0", "1.99.99"), 1);
    }

    #[test]
    fn e8_10_infer_system_package() {
        assert_eq!(infer_system_package("openssl"), "libssl-dev");
        assert_eq!(infer_system_package("zlib"), "zlib1g-dev");
        assert_eq!(infer_system_package("sqlite3"), "libsqlite3-dev");
        assert_eq!(infer_system_package("custom"), "libcustom-dev");
    }

    #[test]
    fn e8_10_cross_target_empty_triple_fails() {
        let err = CrossTarget::from_triple("", "", "").unwrap_err();
        assert!(matches!(err, BuildError::UnsupportedTarget(_)));
    }
}
