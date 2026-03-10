//! Package publishing — validation and local registry publishing.
//!
//! Validates a package before publishing: checks manifest, version,
//! entry file existence, and publishes to a local registry.

use std::path::Path;

use super::manifest::ProjectConfig;
use super::registry::{Registry, SemVer};

/// Validation error found during pre-publish check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    /// Manifest file not found.
    ManifestNotFound(String),
    /// Manifest is invalid.
    ManifestInvalid(String),
    /// Entry file specified in manifest doesn't exist.
    EntryNotFound(String),
    /// Version string is not valid semver.
    InvalidVersion(String),
    /// Version already exists in registry.
    VersionExists {
        /// Package name.
        name: String,
        /// Version that already exists.
        version: SemVer,
    },
    /// Package name is empty or contains invalid characters.
    InvalidName(String),
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ManifestNotFound(p) => write!(f, "manifest not found: {p}"),
            Self::ManifestInvalid(e) => write!(f, "invalid manifest: {e}"),
            Self::EntryNotFound(p) => write!(f, "entry file not found: {p}"),
            Self::InvalidVersion(v) => write!(f, "invalid version: {v}"),
            Self::VersionExists { name, version } => {
                write!(
                    f,
                    "version {version} of '{name}' already exists in registry"
                )
            }
            Self::InvalidName(n) => write!(f, "invalid package name: {n}"),
        }
    }
}

/// Result of a successful validation.
#[derive(Debug, Clone)]
pub struct ValidatedPackage {
    /// Parsed project config.
    pub config: ProjectConfig,
    /// Parsed semver version.
    pub version: SemVer,
    /// Project root directory.
    pub root: std::path::PathBuf,
}

/// Validates a package name (lowercase alphanumeric + hyphens, 1-64 chars).
fn validate_name(name: &str) -> Result<(), PublishError> {
    if name.is_empty() || name.len() > 64 {
        return Err(PublishError::InvalidName(
            "name must be 1-64 characters".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(PublishError::InvalidName(
            "name must contain only lowercase letters, digits, and hyphens".into(),
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(PublishError::InvalidName(
            "name cannot start or end with a hyphen".into(),
        ));
    }
    Ok(())
}

/// Validates a package at the given project root before publishing.
pub fn validate_package(project_root: &Path) -> Result<ValidatedPackage, PublishError> {
    let manifest_path = project_root.join("fj.toml");
    if !manifest_path.exists() {
        return Err(PublishError::ManifestNotFound(
            manifest_path.display().to_string(),
        ));
    }

    let config = ProjectConfig::from_file(&manifest_path).map_err(PublishError::ManifestInvalid)?;

    // Validate name
    validate_name(&config.package.name)?;

    // Validate version
    let version = SemVer::parse(&config.package.version).map_err(PublishError::InvalidVersion)?;

    // Validate entry file exists
    let entry_path = project_root.join(&config.package.entry);
    if !entry_path.exists() {
        return Err(PublishError::EntryNotFound(
            entry_path.display().to_string(),
        ));
    }

    Ok(ValidatedPackage {
        config,
        version,
        root: project_root.to_path_buf(),
    })
}

/// Publishes a validated package to the local registry.
pub fn publish_to_registry(
    registry: &mut Registry,
    package: &ValidatedPackage,
) -> Result<(), PublishError> {
    // Check if version already exists
    if let Some(entry) = registry.lookup(&package.config.package.name) {
        if entry.versions.contains(&package.version) {
            return Err(PublishError::VersionExists {
                name: package.config.package.name.clone(),
                version: package.version.clone(),
            });
        }
    }

    // Publish to registry
    registry.publish(
        &package.config.package.name,
        package.version.clone(),
        &format!("{} v{}", package.config.package.name, package.version),
    );

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Name validation ──

    #[test]
    fn valid_package_names() {
        assert!(validate_name("fj-math").is_ok());
        assert!(validate_name("fj-nn").is_ok());
        assert!(validate_name("hello").is_ok());
        assert!(validate_name("my-cool-package-123").is_ok());
        assert!(validate_name("a").is_ok());
    }

    #[test]
    fn invalid_package_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name("-leading").is_err());
        assert!(validate_name("trailing-").is_err());
        assert!(validate_name("Has_Uppercase").is_err());
        assert!(validate_name("has spaces").is_err());
        assert!(validate_name("has.dots").is_err());
    }

    #[test]
    fn name_too_long() {
        let long_name = "a".repeat(65);
        assert!(validate_name(&long_name).is_err());
    }

    // ── Package validation ──

    #[test]
    fn validate_existing_package() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        let hal_dir = packages_dir.join("fj-hal");
        if !hal_dir.exists() {
            return;
        }
        let result = validate_package(&hal_dir);
        assert!(result.is_ok());
        let pkg = result.unwrap();
        assert_eq!(pkg.config.package.name, "fj-hal");
        assert_eq!(pkg.version, SemVer::new(0, 1, 0));
    }

    #[test]
    fn validate_nonexistent_project() {
        let result = validate_package(Path::new("/tmp/nonexistent_fj_pkg"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PublishError::ManifestNotFound(_)
        ));
    }

    #[test]
    fn validate_missing_entry_file() {
        let tmp = std::env::temp_dir().join("fj_test_publish_no_entry");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("fj.toml"),
            "[package]\nname = \"test-pkg\"\nversion = \"1.0.0\"\nentry = \"src/nonexistent.fj\"\n",
        )
        .unwrap();

        let result = validate_package(&tmp);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PublishError::EntryNotFound(_)
        ));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Publishing ──

    #[test]
    fn publish_to_empty_registry() {
        let mut reg = Registry::new();
        let pkg = ValidatedPackage {
            config: ProjectConfig::parse("[package]\nname = \"fj-math\"\nversion = \"0.1.0\"\n")
                .unwrap(),
            version: SemVer::new(0, 1, 0),
            root: std::path::PathBuf::from("/tmp"),
        };

        let result = publish_to_registry(&mut reg, &pkg);
        assert!(result.is_ok());

        // Verify it's in the registry
        let entry = reg.lookup("fj-math").unwrap();
        assert_eq!(entry.versions.len(), 1);
        assert_eq!(entry.versions[0], SemVer::new(0, 1, 0));
    }

    #[test]
    fn publish_duplicate_version_rejected() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(0, 1, 0), "fj-math v0.1.0");

        let pkg = ValidatedPackage {
            config: ProjectConfig::parse("[package]\nname = \"fj-math\"\nversion = \"0.1.0\"\n")
                .unwrap(),
            version: SemVer::new(0, 1, 0),
            root: std::path::PathBuf::from("/tmp"),
        };

        let result = publish_to_registry(&mut reg, &pkg);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PublishError::VersionExists { .. }
        ));
    }

    #[test]
    fn publish_new_version_accepted() {
        let mut reg = Registry::new();
        reg.publish("fj-math", SemVer::new(0, 1, 0), "fj-math v0.1.0");

        let pkg = ValidatedPackage {
            config: ProjectConfig::parse("[package]\nname = \"fj-math\"\nversion = \"0.2.0\"\n")
                .unwrap(),
            version: SemVer::new(0, 2, 0),
            root: std::path::PathBuf::from("/tmp"),
        };

        let result = publish_to_registry(&mut reg, &pkg);
        assert!(result.is_ok());

        let entry = reg.lookup("fj-math").unwrap();
        assert_eq!(entry.versions.len(), 2);
    }

    #[test]
    fn publish_all_core_packages() {
        let packages_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");
        if !packages_dir.exists() {
            return;
        }
        let mut reg = Registry::new();

        for pkg_name in &[
            "fj-hal",
            "fj-nn",
            "fj-drivers",
            "fj-math",
            "fj-http",
            "fj-json",
            "fj-crypto",
        ] {
            let pkg_dir = packages_dir.join(pkg_name);
            if !pkg_dir.exists() {
                continue;
            }
            let validated = validate_package(&pkg_dir).unwrap();
            publish_to_registry(&mut reg, &validated).unwrap();
        }

        // All 7 packages should be in registry
        assert!(reg.lookup("fj-hal").is_some());
        assert!(reg.lookup("fj-nn").is_some());
        assert!(reg.lookup("fj-drivers").is_some());
        assert!(reg.lookup("fj-math").is_some());
        assert!(reg.lookup("fj-http").is_some());
        assert!(reg.lookup("fj-json").is_some());
        assert!(reg.lookup("fj-crypto").is_some());
    }
}
