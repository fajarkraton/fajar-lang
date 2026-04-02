//! Software Bill of Materials (SBOM) and supply chain security.
//!
//! Generates CycloneDX 1.6 and SPDX 2.3 format SBOMs, tracks build
//! provenance, and supports reproducible build configuration.
//! All output is simulated JSON — no external dependencies required.

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during SBOM generation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SbomError {
    /// Dependency data is incomplete or invalid.
    #[error("invalid dependency: {0}")]
    InvalidDependency(String),

    /// SBOM format is not supported.
    #[error("unsupported SBOM format: {0}")]
    UnsupportedFormat(String),

    /// Serialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
}

// ═══════════════════════════════════════════════════════════════════════
// SBOM Format
// ═══════════════════════════════════════════════════════════════════════

/// Supported SBOM output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SbomFormat {
    /// CycloneDX 1.6 JSON format.
    CycloneDx,
    /// SPDX 2.3 JSON format.
    Spdx,
}

impl std::fmt::Display for SbomFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycloneDx => write!(f, "CycloneDX 1.6"),
            Self::Spdx => write!(f, "SPDX 2.3"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SBOM Package
// ═══════════════════════════════════════════════════════════════════════

/// A package entry in an SBOM document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbomPackage {
    /// Package name.
    pub name: String,
    /// Package version string.
    pub version: String,
    /// Package URL (purl) identifier: `pkg:fj/name@version`.
    pub purl: String,
    /// SHA-256 hash of the package artifact.
    pub sha256: String,
    /// SPDX license identifier, if known.
    pub license: Option<String>,
}

impl SbomPackage {
    /// Creates a new SBOM package entry with auto-generated purl.
    pub fn new(name: &str, version: &str, sha256: &str, license: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            purl: format!("pkg:fj/{name}@{version}"),
            sha256: sha256.to_string(),
            license,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SBOM Relationship
// ═══════════════════════════════════════════════════════════════════════

/// The kind of relationship between two SBOM packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationshipKind {
    /// Runtime dependency.
    DependsOn,
    /// Development-only dependency.
    DevDependsOn,
}

impl std::fmt::Display for RelationshipKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DependsOn => write!(f, "dependsOn"),
            Self::DevDependsOn => write!(f, "devDependsOn"),
        }
    }
}

/// A dependency relationship between two packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbomRelationship {
    /// The package that depends on another.
    pub from_pkg: String,
    /// The package being depended upon.
    pub to_pkg: String,
    /// The kind of dependency relationship.
    pub kind: RelationshipKind,
}

// ═══════════════════════════════════════════════════════════════════════
// SBOM Document
// ═══════════════════════════════════════════════════════════════════════

/// Creation information for an SBOM document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationInfo {
    /// Tool that generated the SBOM.
    pub tool: String,
    /// Timestamp of generation (ISO 8601).
    pub timestamp: String,
}

impl Default for CreationInfo {
    fn default() -> Self {
        Self {
            tool: "fj-sbom-generator/0.7.0".to_string(),
            timestamp: "2026-03-11T00:00:00Z".to_string(),
        }
    }
}

/// A complete SBOM document.
#[derive(Debug, Clone)]
pub struct SbomDocument {
    /// Output format.
    pub format: SbomFormat,
    /// Packages listed in the SBOM.
    pub packages: Vec<SbomPackage>,
    /// Relationships between packages.
    pub relationships: Vec<SbomRelationship>,
    /// Metadata about SBOM creation.
    pub creation_info: CreationInfo,
}

impl SbomDocument {
    /// Creates a new SBOM document with the given format.
    pub fn new(format: SbomFormat) -> Self {
        Self {
            format,
            packages: Vec::new(),
            relationships: Vec::new(),
            creation_info: CreationInfo::default(),
        }
    }

    /// Adds a package to the SBOM.
    pub fn add_package(&mut self, pkg: SbomPackage) {
        self.packages.push(pkg);
    }

    /// Adds a relationship to the SBOM.
    pub fn add_relationship(&mut self, rel: SbomRelationship) {
        self.relationships.push(rel);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SBOM Generation
// ═══════════════════════════════════════════════════════════════════════

/// Input descriptor for a dependency used during SBOM generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepInfo {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// SHA-256 hash of the package.
    pub sha256: String,
    /// License identifier.
    pub license: Option<String>,
    /// Whether this is a dev-only dependency.
    pub dev_only: bool,
}

/// Generates an SBOM document from a list of dependencies.
///
/// Returns the SBOM as a JSON string in the specified format.
pub fn generate_sbom(
    project_name: &str,
    deps: &[DepInfo],
    format: SbomFormat,
) -> Result<String, SbomError> {
    if project_name.is_empty() {
        return Err(SbomError::InvalidDependency(
            "project name cannot be empty".to_string(),
        ));
    }

    let mut doc = SbomDocument::new(format);

    for dep in deps {
        validate_dep(dep)?;
        let pkg = SbomPackage::new(&dep.name, &dep.version, &dep.sha256, dep.license.clone());
        doc.add_package(pkg);

        let kind = if dep.dev_only {
            RelationshipKind::DevDependsOn
        } else {
            RelationshipKind::DependsOn
        };

        doc.add_relationship(SbomRelationship {
            from_pkg: project_name.to_string(),
            to_pkg: dep.name.clone(),
            kind,
        });
    }

    match format {
        SbomFormat::CycloneDx => render_cyclonedx(&doc),
        SbomFormat::Spdx => render_spdx(&doc, project_name),
    }
}

/// Validates a dependency descriptor.
fn validate_dep(dep: &DepInfo) -> Result<(), SbomError> {
    if dep.name.is_empty() {
        return Err(SbomError::InvalidDependency(
            "dependency name cannot be empty".to_string(),
        ));
    }
    if dep.version.is_empty() {
        return Err(SbomError::InvalidDependency(format!(
            "version missing for '{}'",
            dep.name
        )));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// CycloneDX 1.6 Renderer
// ═══════════════════════════════════════════════════════════════════════

/// Renders an SBOM document as CycloneDX 1.6 JSON.
fn render_cyclonedx(doc: &SbomDocument) -> Result<String, SbomError> {
    let components: Vec<String> = doc.packages.iter().map(render_cdx_component).collect();
    let dep_entries: Vec<String> = doc.relationships.iter().map(render_cdx_dep).collect();

    let json = format!(
        concat!(
            "{{\n",
            "  \"bomFormat\": \"CycloneDX\",\n",
            "  \"specVersion\": \"1.6\",\n",
            "  \"version\": 1,\n",
            "  \"metadata\": {{\n",
            "    \"timestamp\": \"{}\",\n",
            "    \"tools\": [{{ \"name\": \"{}\" }}]\n",
            "  }},\n",
            "  \"components\": [\n{}\n",
            "  ],\n",
            "  \"dependencies\": [\n{}\n",
            "  ]\n",
            "}}"
        ),
        json_escape(&doc.creation_info.timestamp),
        json_escape(&doc.creation_info.tool),
        components.join(",\n"),
        dep_entries.join(",\n"),
    );

    Ok(json)
}

/// Renders a single CycloneDX component entry.
fn render_cdx_component(pkg: &SbomPackage) -> String {
    let license_field = match &pkg.license {
        Some(l) => format!(
            ",\n        \"licenses\": [{{ \"license\": {{ \"id\": \"{}\" }} }}]",
            json_escape(l)
        ),
        None => String::new(),
    };

    format!(
        concat!(
            "      {{\n",
            "        \"type\": \"library\",\n",
            "        \"name\": \"{}\",\n",
            "        \"version\": \"{}\",\n",
            "        \"purl\": \"{}\",\n",
            "        \"hashes\": [{{ \"alg\": \"SHA-256\", \"content\": \"{}\" }}]{}\n",
            "      }}"
        ),
        json_escape(&pkg.name),
        json_escape(&pkg.version),
        json_escape(&pkg.purl),
        json_escape(&pkg.sha256),
        license_field,
    )
}

/// Renders a single CycloneDX dependency entry.
fn render_cdx_dep(rel: &SbomRelationship) -> String {
    format!(
        "      {{ \"ref\": \"{}\", \"dependsOn\": [\"{}\"] }}",
        json_escape(&rel.from_pkg),
        json_escape(&rel.to_pkg),
    )
}

// ═══════════════════════════════════════════════════════════════════════
// SPDX 2.3 Renderer
// ═══════════════════════════════════════════════════════════════════════

/// Renders an SBOM document as SPDX 2.3 JSON.
fn render_spdx(doc: &SbomDocument, project_name: &str) -> Result<String, SbomError> {
    let packages: Vec<String> = doc.packages.iter().map(render_spdx_package).collect();
    let rels: Vec<String> = doc.relationships.iter().map(render_spdx_rel).collect();

    let json = format!(
        concat!(
            "{{\n",
            "  \"spdxVersion\": \"SPDX-2.3\",\n",
            "  \"dataLicense\": \"CC0-1.0\",\n",
            "  \"SPDXID\": \"SPDXRef-DOCUMENT\",\n",
            "  \"name\": \"{}\",\n",
            "  \"creationInfo\": {{\n",
            "    \"created\": \"{}\",\n",
            "    \"creators\": [\"Tool: {}\"]\n",
            "  }},\n",
            "  \"packages\": [\n{}\n",
            "  ],\n",
            "  \"relationships\": [\n{}\n",
            "  ]\n",
            "}}"
        ),
        json_escape(project_name),
        json_escape(&doc.creation_info.timestamp),
        json_escape(&doc.creation_info.tool),
        packages.join(",\n"),
        rels.join(",\n"),
    );

    Ok(json)
}

/// Renders a single SPDX package entry.
fn render_spdx_package(pkg: &SbomPackage) -> String {
    let license_field = match &pkg.license {
        Some(l) => json_escape(l),
        None => "NOASSERTION".to_string(),
    };

    format!(
        concat!(
            "      {{\n",
            "        \"SPDXID\": \"SPDXRef-Package-{}\",\n",
            "        \"name\": \"{}\",\n",
            "        \"versionInfo\": \"{}\",\n",
            "        \"externalRefs\": [{{\n",
            "          \"referenceCategory\": \"PACKAGE-MANAGER\",\n",
            "          \"referenceType\": \"purl\",\n",
            "          \"referenceLocator\": \"{}\"\n",
            "        }}],\n",
            "        \"checksums\": [{{\n",
            "          \"algorithm\": \"SHA256\",\n",
            "          \"checksumValue\": \"{}\"\n",
            "        }}],\n",
            "        \"licenseConcluded\": \"{}\",\n",
            "        \"downloadLocation\": \"NOASSERTION\"\n",
            "      }}"
        ),
        json_escape(&pkg.name),
        json_escape(&pkg.name),
        json_escape(&pkg.version),
        json_escape(&pkg.purl),
        json_escape(&pkg.sha256),
        license_field,
    )
}

/// Renders a single SPDX relationship entry.
fn render_spdx_rel(rel: &SbomRelationship) -> String {
    let rel_type = match rel.kind {
        RelationshipKind::DependsOn => "DEPENDS_ON",
        RelationshipKind::DevDependsOn => "DEV_DEPENDENCY_OF",
    };
    format!(
        concat!(
            "      {{\n",
            "        \"spdxElementId\": \"SPDXRef-Package-{}\",\n",
            "        \"relatedSpdxElement\": \"SPDXRef-Package-{}\",\n",
            "        \"relationshipType\": \"{}\"\n",
            "      }}"
        ),
        json_escape(&rel.from_pkg),
        json_escape(&rel.to_pkg),
        rel_type,
    )
}

// ═══════════════════════════════════════════════════════════════════════
// Reproducible Builds
// ═══════════════════════════════════════════════════════════════════════

/// Configuration for reproducible builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReproducibleBuildConfig {
    /// Strip timestamps from build artifacts.
    pub strip_timestamps: bool,
    /// Sort all outputs deterministically.
    pub deterministic_sort: bool,
}

impl Default for ReproducibleBuildConfig {
    fn default() -> Self {
        Self {
            strip_timestamps: true,
            deterministic_sort: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Build Provenance
// ═══════════════════════════════════════════════════════════════════════

/// Provenance record for a build artifact.
///
/// Records who built the artifact, from what source, and with what
/// configuration — enabling supply chain auditability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProvenance {
    /// Builder identity (CI system or developer).
    pub builder: String,
    /// Source repository URL.
    pub source_repo: String,
    /// Git commit digest of the source.
    pub source_digest: String,
    /// Hash of the build configuration.
    pub build_config_hash: String,
}

/// Provenance input parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceConfig {
    /// Builder identity (e.g., `github-actions`, `local-dev`).
    pub builder: String,
    /// Source repository URL.
    pub source_repo: String,
    /// Git commit hash.
    pub source_digest: String,
    /// Reproducible build config.
    pub build_config: ReproducibleBuildConfig,
}

/// Generates a build provenance record from the given configuration.
///
/// The `build_config_hash` is a deterministic hash of the reproducible
/// build configuration, enabling verification that the same config
/// was used across builds.
pub fn generate_provenance(config: &ProvenanceConfig) -> BuildProvenance {
    let config_str = format!(
        "strip_timestamps={},deterministic_sort={}",
        config.build_config.strip_timestamps, config.build_config.deterministic_sort,
    );
    let config_hash = format!("{:016x}", simple_hash(&config_str));

    BuildProvenance {
        builder: config.builder.clone(),
        source_repo: config.source_repo.clone(),
        source_digest: config.source_digest.clone(),
        build_config_hash: config_hash,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Internal Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Escapes a string for JSON embedding.
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Simple deterministic hash (FNV-1a inspired).
fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_deps() -> Vec<DepInfo> {
        vec![
            DepInfo {
                name: "fj-math".to_string(),
                version: "1.0.0".to_string(),
                sha256: "aabbccdd".to_string(),
                license: Some("MIT".to_string()),
                dev_only: false,
            },
            DepInfo {
                name: "fj-nn".to_string(),
                version: "0.5.0".to_string(),
                sha256: "11223344".to_string(),
                license: Some("Apache-2.0".to_string()),
                dev_only: false,
            },
            DepInfo {
                name: "fj-test-utils".to_string(),
                version: "0.1.0".to_string(),
                sha256: "deadbeef".to_string(),
                license: None,
                dev_only: true,
            },
        ]
    }

    #[test]
    fn s24_1_sbom_package_purl_format() {
        let pkg = SbomPackage::new("fj-math", "1.2.3", "abc123", Some("MIT".to_string()));
        assert_eq!(pkg.purl, "pkg:fj/fj-math@1.2.3");
        assert_eq!(pkg.name, "fj-math");
        assert_eq!(pkg.version, "1.2.3");
    }

    #[test]
    fn s24_2_generate_cyclonedx_sbom() {
        let deps = sample_deps();
        let json = generate_sbom("my-project", &deps, SbomFormat::CycloneDx).unwrap();
        assert!(json.contains("\"bomFormat\": \"CycloneDX\""));
        assert!(json.contains("\"specVersion\": \"1.6\""));
        assert!(json.contains("fj-math"));
        assert!(json.contains("fj-nn"));
        assert!(json.contains("pkg:fj/fj-math@1.0.0"));
        assert!(json.contains("SHA-256"));
    }

    #[test]
    fn s24_3_generate_spdx_sbom() {
        let deps = sample_deps();
        let json = generate_sbom("my-project", &deps, SbomFormat::Spdx).unwrap();
        assert!(json.contains("\"spdxVersion\": \"SPDX-2.3\""));
        assert!(json.contains("\"dataLicense\": \"CC0-1.0\""));
        assert!(json.contains("SPDXRef-Package-fj-math"));
        assert!(json.contains("DEPENDS_ON"));
        assert!(json.contains("DEV_DEPENDENCY_OF"));
    }

    #[test]
    fn s24_4_sbom_relationships() {
        let deps = sample_deps();
        let json = generate_sbom("my-project", &deps, SbomFormat::CycloneDx).unwrap();
        assert!(json.contains("\"dependsOn\""));
        // Should have dependency entries for each dep
        assert!(json.contains("fj-math"));
        assert!(json.contains("fj-nn"));
        assert!(json.contains("fj-test-utils"));
    }

    #[test]
    fn s24_5_sbom_empty_project_name_fails() {
        let deps = sample_deps();
        let result = generate_sbom("", &deps, SbomFormat::CycloneDx);
        assert!(result.is_err());
        match result {
            Err(SbomError::InvalidDependency(msg)) => {
                assert!(msg.contains("project name"));
            }
            other => panic!("expected InvalidDependency, got {other:?}"),
        }
    }

    #[test]
    fn s24_6_sbom_empty_dep_name_fails() {
        let deps = vec![DepInfo {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            sha256: "abc".to_string(),
            license: None,
            dev_only: false,
        }];
        let result = generate_sbom("my-project", &deps, SbomFormat::CycloneDx);
        assert!(result.is_err());
    }

    #[test]
    fn s24_7_reproducible_build_config_defaults() {
        let config = ReproducibleBuildConfig::default();
        assert!(config.strip_timestamps);
        assert!(config.deterministic_sort);
    }

    #[test]
    fn s24_8_build_provenance_generation() {
        let config = ProvenanceConfig {
            builder: "github-actions".to_string(),
            source_repo: "https://github.com/user/fj-project".to_string(),
            source_digest: "abc123def456".to_string(),
            build_config: ReproducibleBuildConfig::default(),
        };

        let prov = generate_provenance(&config);
        assert_eq!(prov.builder, "github-actions");
        assert_eq!(prov.source_repo, "https://github.com/user/fj-project");
        assert_eq!(prov.source_digest, "abc123def456");
        assert!(!prov.build_config_hash.is_empty());
    }

    #[test]
    fn s24_9_provenance_deterministic_hash() {
        let config = ProvenanceConfig {
            builder: "local".to_string(),
            source_repo: "file:///repo".to_string(),
            source_digest: "aaa".to_string(),
            build_config: ReproducibleBuildConfig::default(),
        };

        let prov1 = generate_provenance(&config);
        let prov2 = generate_provenance(&config);
        assert_eq!(prov1.build_config_hash, prov2.build_config_hash);

        // Different config should produce different hash
        let config2 = ProvenanceConfig {
            build_config: ReproducibleBuildConfig {
                strip_timestamps: false,
                deterministic_sort: true,
            },
            ..config
        };
        let prov3 = generate_provenance(&config2);
        assert_ne!(prov1.build_config_hash, prov3.build_config_hash);
    }

    #[test]
    fn s24_10_sbom_format_display() {
        assert_eq!(format!("{}", SbomFormat::CycloneDx), "CycloneDX 1.6");
        assert_eq!(format!("{}", SbomFormat::Spdx), "SPDX 2.3");
    }

    // ── V14 H4.9: SBOM from Cargo.lock parsing ────────────────

    #[test]
    fn v14_h4_9_sbom_from_cargo_lock_parse() {
        // Simulate parsing a Cargo.lock [[package]] block into DepInfo
        let lock_snippet = r#"
[[package]]
name = "serde"
version = "1.0.210"
checksum = "abc123def456"

[[package]]
name = "tokio"
version = "1.40.0"
checksum = "def789ghi012"
"#;
        let deps: Vec<DepInfo> = lock_snippet
            .split("[[package]]")
            .skip(1)
            .filter_map(|block| {
                let name = block
                    .lines()
                    .find(|l| l.starts_with("name"))?
                    .split('"')
                    .nth(1)?
                    .to_string();
                let version = block
                    .lines()
                    .find(|l| l.starts_with("version"))?
                    .split('"')
                    .nth(1)?
                    .to_string();
                let checksum = block
                    .lines()
                    .find(|l| l.starts_with("checksum"))
                    .and_then(|l| l.split('"').nth(1))
                    .unwrap_or("")
                    .to_string();
                Some(DepInfo {
                    name,
                    version,
                    sha256: checksum,
                    license: None,
                    dev_only: false,
                })
            })
            .collect();

        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "serde");
        assert_eq!(deps[1].name, "tokio");

        let json = generate_sbom("fajar-lang", &deps, SbomFormat::CycloneDx).unwrap();
        assert!(json.contains("serde"));
        assert!(json.contains("tokio"));
        assert!(json.contains("CycloneDX"));
    }
}
