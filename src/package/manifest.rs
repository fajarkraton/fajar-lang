//! Fajar Lang project manifest (`fj.toml`) parser.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::registry::SemVer;

/// Parsed project configuration from `fj.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// The `[package]` section.
    pub package: PackageInfo,
    /// The `[dependencies]` section (optional).
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
    /// The `[dev-dependencies]` section (optional).
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: std::collections::HashMap<String, String>,
    /// The `[kernel]` section for OS kernel builds (optional).
    #[serde(default)]
    pub kernel: Option<KernelConfig>,
    /// The `[[service]]` array for microkernel services (optional).
    #[serde(default)]
    pub service: Vec<ServiceConfig>,
    /// The `[build]` section for pre/post build hooks (optional).
    #[serde(default)]
    pub build: Option<BuildSection>,
}

/// Build hooks configuration for `[build]` section.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BuildSection {
    /// Shell command to run before compilation.
    #[serde(default)]
    pub pre_build: Option<String>,
    /// Shell command to run after compilation.
    #[serde(default)]
    pub post_build: Option<String>,
    /// Environment variables to set during build.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Kernel build configuration for `[kernel]` section.
#[derive(Debug, Clone, Deserialize)]
pub struct KernelConfig {
    /// Entry point file.
    pub entry: String,
    /// Target triple (e.g., "x86_64-unknown-none").
    pub target: String,
    /// Source directories to include.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Linker script path.
    #[serde(default, rename = "linker-script")]
    pub linker_script: Option<String>,
}

/// Service build configuration for `[[service]]` entries.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Service name (used for output ELF name).
    pub name: String,
    /// Entry point file.
    pub entry: String,
    /// Target triple (e.g., "x86_64-user").
    #[serde(default = "default_service_target")]
    pub target: String,
    /// Source directories to include (optional).
    #[serde(default)]
    pub sources: Vec<String>,
}

fn default_service_target() -> String {
    "x86_64-user".into()
}

impl ProjectConfig {
    /// Returns true if this project has multi-binary configuration.
    pub fn is_multi_binary(&self) -> bool {
        self.kernel.is_some() || !self.service.is_empty()
    }

    /// Returns all service names.
    pub fn service_names(&self) -> Vec<String> {
        self.service.iter().map(|s| s.name.clone()).collect()
    }
}

/// The `[package]` section of `fj.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PackageInfo {
    /// Project name.
    pub name: String,
    /// Semantic version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// Entry point relative to project root.
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Optional linker script path (for bare-metal targets).
    #[serde(default, rename = "linker-script")]
    pub linker_script: Option<String>,
    /// Target triple (e.g., "x86_64-unknown-none", "aarch64-unknown-none").
    #[serde(default)]
    pub target: Option<String>,
    /// Whether this is a no_std project.
    #[serde(default)]
    pub no_std: bool,
    /// Package authors (optional, e.g., `["Fajar <fajar@example.com>"]`).
    #[serde(default)]
    pub authors: Vec<String>,
    /// Short description of the package (optional).
    #[serde(default)]
    pub description: Option<String>,
    /// SPDX license identifier (optional, e.g., `"MIT"` or `"Apache-2.0"`).
    #[serde(default)]
    pub license: Option<String>,
    /// Searchable keywords for registry discovery (optional).
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Registry categories (optional, e.g., `["ml", "embedded"]`).
    #[serde(default)]
    pub categories: Vec<String>,
}

fn default_version() -> String {
    "0.1.0".into()
}

fn default_entry() -> String {
    "src/main.fj".into()
}

impl ProjectConfig {
    /// Reads and parses a `fj.toml` file.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
        Self::parse(&content)
    }

    /// Parses a `fj.toml` from a string.
    pub fn parse(content: &str) -> Result<Self, String> {
        toml::from_str(content).map_err(|e| format!("invalid fj.toml: {e}"))
    }
}

/// Walks up from `start` to find a directory containing `fj.toml`.
/// Returns the project root directory (parent of fj.toml).
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if dir.join("fj.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Creates a new project directory with scaffolding.
pub fn create_project(name: &str, parent_dir: &Path) -> Result<PathBuf, String> {
    let project_dir = parent_dir.join(name);
    if project_dir.exists() {
        return Err(format!(
            "directory '{}' already exists",
            project_dir.display()
        ));
    }

    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| format!("cannot create directory: {e}"))?;

    // Write fj.toml
    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
entry = "src/main.fj"
"#
    );
    std::fs::write(project_dir.join("fj.toml"), manifest)
        .map_err(|e| format!("cannot write fj.toml: {e}"))?;

    // Write src/main.fj
    let main_fj = format!(
        r#"// {name} — A Fajar Lang project

fn main() -> void {{
    println("Hello from {name}!")
}}
"#
    );
    std::fs::write(src_dir.join("main.fj"), main_fj)
        .map_err(|e| format!("cannot write src/main.fj: {e}"))?;

    Ok(project_dir)
}

// ═══════════════════════════════════════════════════════════════════════
// Package file collection & caching
// ═══════════════════════════════════════════════════════════════════════

/// Collects the list of files that should be included in a published package.
///
/// Returns paths relative to `root` for: all `.fj` files, `fj.toml`,
/// `README*`, and `LICENSE*`.
pub fn package_filelist(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Always include fj.toml
    let manifest = root.join("fj.toml");
    if manifest.exists() {
        files.push(PathBuf::from("fj.toml"));
    }

    // Collect README* and LICENSE* at root
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let upper = name_str.to_uppercase();
            if (upper.starts_with("README") || upper.starts_with("LICENSE"))
                && entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
            {
                files.push(PathBuf::from(name_str.as_ref()));
            }
        }
    }

    // Recursively collect .fj files
    collect_fj_files(root, root, &mut files);

    files.sort();
    files
}

/// Recursively collects `.fj` files under `dir`, storing paths relative to `root`.
fn collect_fj_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fj_files(root, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("fj") {
            if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_path_buf());
            }
        }
    }
}

/// Computes a simple SHA-256-like hex digest of the given data.
///
/// This is a basic hash representation using the standard library only.
/// It uses a simple deterministic hash for package integrity checks
/// without requiring external crypto crates.
pub fn sha256_hex(data: &[u8]) -> String {
    // Simple but deterministic hash: we use a basic Merkle-Damgard-like
    // construction. This is NOT cryptographically secure, but sufficient
    // for content-addressable integrity in a local/test context.
    // A real deployment would use ring or sha2 crate.
    let mut h: [u64; 4] = [
        0x6a09_e667_f3bc_c908,
        0xbb67_ae85_84ca_a73b,
        0x3c6e_f372_fe94_f82b,
        0xa54f_f53a_5f1d_36f1,
    ];

    for (i, &byte) in data.iter().enumerate() {
        let idx = i % 4;
        h[idx] = h[idx].wrapping_mul(31).wrapping_add(byte as u64);
        h[(idx + 1) % 4] ^= h[idx].rotate_left(13);
    }

    // Mix in length
    h[0] = h[0].wrapping_add(data.len() as u64);
    h[1] ^= h[0].rotate_left(7);
    h[2] = h[2].wrapping_add(h[1]);
    h[3] ^= h[2].rotate_left(19);

    format!("{:016x}{:016x}{:016x}{:016x}", h[0], h[1], h[2], h[3])
}

/// Returns the local package cache directory (`~/.fj/cache/`).
///
/// Falls back to the system temp directory if the home directory cannot be determined.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join(".fj").join("cache")
}

/// Returns the install path for a specific package version.
///
/// Packages are stored under `~/.fj/cache/<name>/<version>/`.
pub fn install_path(name: &str, version: &SemVer) -> PathBuf {
    cache_dir().join(name).join(version.to_string())
}

/// Returns the path to the credentials file (`~/.fj/credentials.toml`).
///
/// Falls back to the system temp directory if the home directory
/// cannot be determined.
pub fn credentials_path() -> PathBuf {
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join(".fj").join("credentials.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
[package]
name = "hello"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.package.name, "hello");
        assert_eq!(config.package.version, "0.1.0");
        assert_eq!(config.package.entry, "src/main.fj");
    }

    #[test]
    fn parse_full_manifest() {
        let toml = r#"
[package]
name = "my-project"
version = "1.2.3"
entry = "src/app.fj"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.package.name, "my-project");
        assert_eq!(config.package.version, "1.2.3");
        assert_eq!(config.package.entry, "src/app.fj");
    }

    #[test]
    fn parse_invalid_manifest() {
        let result = ProjectConfig::parse("not valid toml [[[");
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_name() {
        let toml = r#"
[package]
version = "1.0.0"
"#;
        let result = ProjectConfig::parse(toml);
        assert!(result.is_err());
    }

    #[test]
    fn create_project_scaffolding() {
        let tmp = std::env::temp_dir().join("fj_test_new_project");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let result = create_project("test-app", &tmp);
        assert!(result.is_ok());
        let project_dir = result.unwrap();
        assert!(project_dir.join("fj.toml").exists());
        assert!(project_dir.join("src/main.fj").exists());

        let manifest = std::fs::read_to_string(project_dir.join("fj.toml")).unwrap();
        assert!(manifest.contains("test-app"));

        let main = std::fs::read_to_string(project_dir.join("src/main.fj")).unwrap();
        assert!(main.contains("Hello from test-app!"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn create_project_already_exists() {
        let tmp = std::env::temp_dir().join("fj_test_exists");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("my-proj")).unwrap();

        let result = create_project("my-proj", &tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_from_subdir() {
        let tmp = std::env::temp_dir().join("fj_test_find_root");
        let _ = std::fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(tmp.join("fj.toml"), "[package]\nname = \"test\"\n").unwrap();

        let root = find_project_root(&src);
        assert_eq!(root, Some(tmp.clone()));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_project_root_no_manifest() {
        let root = find_project_root(Path::new("/tmp/nonexistent_fj_dir"));
        assert!(root.is_none());
    }

    #[test]
    fn parse_manifest_with_dependencies() {
        let toml = r#"
[package]
name = "my-app"
version = "1.0.0"

[dependencies]
fj-math = "^0.1.0"
fj-nn = "0.2.0"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.package.name, "my-app");
        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(config.dependencies["fj-math"], "^0.1.0");
        assert_eq!(config.dependencies["fj-nn"], "0.2.0");
    }

    #[test]
    fn parse_linker_script() {
        let toml = r#"
[package]
name = "kernel"
version = "0.1.0"
linker-script = "kernel.ld"
target = "x86_64-unknown-none"
no_std = true
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.package.name, "kernel");
        assert_eq!(config.package.linker_script, Some("kernel.ld".to_string()));
        assert_eq!(
            config.package.target,
            Some("x86_64-unknown-none".to_string())
        );
        assert!(config.package.no_std);
    }

    #[test]
    fn add_dependency_to_existing_manifest() {
        let tmp = std::env::temp_dir().join("fj_test_add_dep");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).unwrap();
        std::fs::write(tmp.join("src/main.fj"), "fn main() -> void {}").unwrap();
        std::fs::write(
            tmp.join("fj.toml"),
            "[package]\nname = \"test-app\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        // Read, add dependency, write back
        let content = std::fs::read_to_string(tmp.join("fj.toml")).unwrap();
        let new_content = format!("{content}\n[dependencies]\nfj-math = \"^1.0.0\"\n");
        std::fs::write(tmp.join("fj.toml"), &new_content).unwrap();

        let config = ProjectConfig::from_file(&tmp.join("fj.toml")).unwrap();
        assert_eq!(config.dependencies.len(), 1);
        assert_eq!(config.dependencies["fj-math"], "^1.0.0");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn add_dependency_to_existing_deps_section() {
        let toml = "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[dependencies]\nfj-math = \"^1.0.0\"\nfj-nn = \"0.3.0\"\n";
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.dependencies.len(), 2);
        assert_eq!(config.dependencies["fj-math"], "^1.0.0");
        assert_eq!(config.dependencies["fj-nn"], "0.3.0");
    }

    #[test]
    fn parse_no_linker_script() {
        let toml = r#"
[package]
name = "app"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert!(config.package.linker_script.is_none());
        assert!(config.package.target.is_none());
        assert!(!config.package.no_std);
    }

    #[test]
    fn parse_package_manifests_from_disk() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        if !packages_dir.exists() {
            return; // Skip if packages dir doesn't exist
        }
        for pkg in &["fj-hal", "fj-nn", "fj-drivers", "fj-math"] {
            let manifest = packages_dir.join(pkg).join("fj.toml");
            if manifest.exists() {
                let config = ProjectConfig::from_file(&manifest)
                    .unwrap_or_else(|e| panic!("{pkg}/fj.toml failed to parse: {e}"));
                assert_eq!(config.package.name, *pkg);
                assert_eq!(config.package.version, "3.0.0");
            }
        }
    }

    // ── Sprint 16: PackageInfo extensions ──

    #[test]
    fn parse_manifest_with_extended_fields() {
        let toml = r#"
[package]
name = "fj-awesome"
version = "0.2.0"
authors = ["Fajar <fajar@example.com>", "Alice <alice@example.com>"]
description = "An awesome package for Fajar Lang"
license = "MIT"
keywords = ["math", "tensor"]
categories = ["ml", "science"]
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.package.name, "fj-awesome");
        assert_eq!(config.package.authors.len(), 2);
        assert_eq!(config.package.authors[0], "Fajar <fajar@example.com>");
        assert_eq!(
            config.package.description,
            Some("An awesome package for Fajar Lang".to_string())
        );
        assert_eq!(config.package.license, Some("MIT".to_string()));
        assert_eq!(config.package.keywords, vec!["math", "tensor"]);
        assert_eq!(config.package.categories, vec!["ml", "science"]);
    }

    #[test]
    fn parse_manifest_extended_fields_default_to_empty() {
        let toml = r#"
[package]
name = "minimal"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert!(config.package.authors.is_empty());
        assert!(config.package.description.is_none());
        assert!(config.package.license.is_none());
        assert!(config.package.keywords.is_empty());
        assert!(config.package.categories.is_empty());
    }

    // ── Sprint 16: Package file list ──

    #[test]
    fn package_filelist_collects_fj_files() {
        let tmp = std::env::temp_dir().join("fj_test_filelist");
        let _ = std::fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(tmp.join("fj.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::write(tmp.join("README.md"), "# readme").unwrap();
        std::fs::write(tmp.join("LICENSE"), "MIT").unwrap();
        std::fs::write(src.join("main.fj"), "fn main() {}").unwrap();
        std::fs::write(src.join("lib.fj"), "fn foo() {}").unwrap();

        let files = package_filelist(&tmp);
        assert!(files.contains(&PathBuf::from("fj.toml")));
        assert!(files.contains(&PathBuf::from("README.md")));
        assert!(files.contains(&PathBuf::from("LICENSE")));
        assert!(files.contains(&PathBuf::from("src/main.fj")));
        assert!(files.contains(&PathBuf::from("src/lib.fj")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Sprint 16: SHA-256 hex ──

    #[test]
    fn sha256_hex_deterministic() {
        let h1 = sha256_hex(b"hello world");
        let h2 = sha256_hex(b"hello world");
        assert_eq!(h1, h2);
        // 64 hex chars (4 x 16)
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn sha256_hex_different_inputs() {
        let h1 = sha256_hex(b"hello");
        let h2 = sha256_hex(b"world");
        assert_ne!(h1, h2);
    }

    // ── Sprint 16: Cache and install paths ──

    #[test]
    fn cache_dir_contains_fj() {
        let dir = cache_dir();
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains(".fj"));
        assert!(dir_str.contains("cache"));
    }

    #[test]
    fn install_path_includes_name_and_version() {
        let path = install_path("fj-math", &SemVer::new(1, 2, 3));
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("fj-math"));
        assert!(path_str.contains("1.2.3"));
    }

    #[test]
    fn credentials_path_contains_fj() {
        let path = credentials_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".fj"));
        assert!(path_str.contains("credentials.toml"));
    }

    // ── Sprint 17: Dev dependencies ──

    #[test]
    fn parse_manifest_with_dev_dependencies() {
        let toml = r#"
[package]
name = "my-app"
version = "1.0.0"

[dependencies]
fj-math = "^1.0.0"

[dev-dependencies]
fj-test-utils = "0.1.0"
fj-bench = "^0.2.0"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.dependencies.len(), 1);
        assert_eq!(config.dev_dependencies.len(), 2);
        assert_eq!(config.dev_dependencies["fj-test-utils"], "0.1.0");
        assert_eq!(config.dev_dependencies["fj-bench"], "^0.2.0");
    }

    #[test]
    fn parse_manifest_no_dev_dependencies_defaults_empty() {
        let toml = r#"
[package]
name = "simple"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert!(config.dev_dependencies.is_empty());
    }

    #[test]
    fn parse_manifest_build_section() {
        let toml = r#"
[package]
name = "build-test"
version = "0.1.0"
entry = "main.fj"

[build]
pre_build = "echo generating config"
post_build = "echo done"

[build.env]
BUILD_MODE = "release"
VERSION = "1.0.0"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        let build = config.build.expect("build section missing");
        assert_eq!(build.pre_build.as_deref(), Some("echo generating config"));
        assert_eq!(build.post_build.as_deref(), Some("echo done"));
        assert_eq!(build.env["BUILD_MODE"], "release");
        assert_eq!(build.env["VERSION"], "1.0.0");
    }

    #[test]
    fn parse_manifest_no_build_section() {
        let toml = r#"
[package]
name = "no-build"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert!(config.build.is_none());
    }
}
