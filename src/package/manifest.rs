//! Fajar Lang project manifest (`fj.toml`) parser.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Parsed project configuration from `fj.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    /// The `[package]` section.
    pub package: PackageInfo,
    /// The `[dependencies]` section (optional).
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
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
                assert_eq!(config.package.version, "0.1.0");
            }
        }
    }
}
