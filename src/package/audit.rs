//! Dependency vulnerability scanning for Fajar Lang packages.
//!
//! Provides an advisory database, severity classification, version-range
//! matching, and audit reporting for detecting known vulnerabilities in
//! project dependencies. All advisory data is local — no network required.

use std::fmt;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════

/// Errors that can occur during audit operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// Advisory database could not be parsed.
    #[error("advisory database parse error: {0}")]
    DatabaseParseError(String),

    /// Version string is invalid.
    #[error("invalid version: {0}")]
    InvalidVersion(String),

    /// Advisory data is malformed.
    #[error("malformed advisory: {0}")]
    MalformedAdvisory(String),
}

// ═══════════════════════════════════════════════════════════════════════
// Severity
// ═══════════════════════════════════════════════════════════════════════

/// Severity level for a security advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Low severity — minimal impact.
    Low,
    /// Medium severity — limited impact.
    Medium,
    /// High severity — significant impact.
    High,
    /// Critical severity — immediate action required.
    Critical,
}

impl Severity {
    /// Returns an ANSI color code prefix for terminal display.
    pub fn color_code(&self) -> &'static str {
        match self {
            Self::Low => "\x1b[34m",        // blue
            Self::Medium => "\x1b[33m",     // yellow
            Self::High => "\x1b[31m",       // red
            Self::Critical => "\x1b[1;31m", // bold red
        }
    }

    /// Returns the ANSI reset code.
    pub fn color_reset() -> &'static str {
        "\x1b[0m"
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Version Range
// ═══════════════════════════════════════════════════════════════════════

/// A range of affected versions for an advisory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    /// Minimum affected version (inclusive), or `None` for no lower bound.
    pub min_version: Option<String>,
    /// Maximum affected version (exclusive), or `None` for no upper bound.
    pub max_version: Option<String>,
}

impl VersionRange {
    /// Creates a new version range.
    pub fn new(min: Option<String>, max: Option<String>) -> Self {
        Self {
            min_version: min,
            max_version: max,
        }
    }

    /// Checks whether a given version string falls within this range.
    ///
    /// Uses simple semver comparison: `min_version <= version < max_version`.
    pub fn affects(&self, version: &str) -> bool {
        let parts = parse_version_parts(version);
        let parts = match parts {
            Some(p) => p,
            None => return false,
        };

        // Check min bound (inclusive)
        if let Some(ref min) = self.min_version {
            if let Some(min_parts) = parse_version_parts(min) {
                if compare_parts(&parts, &min_parts) < 0 {
                    return false;
                }
            }
        }

        // Check max bound (exclusive)
        if let Some(ref max) = self.max_version {
            if let Some(max_parts) = parse_version_parts(max) {
                if compare_parts(&parts, &max_parts) >= 0 {
                    return false;
                }
            }
        }

        true
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Advisory
// ═══════════════════════════════════════════════════════════════════════

/// A security advisory describing a known vulnerability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advisory {
    /// Unique advisory identifier (e.g., `FJ-2026-001`).
    pub id: String,
    /// Name of the affected package.
    pub package: String,
    /// Range of affected versions.
    pub versions_affected: VersionRange,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable description of the vulnerability.
    pub description: String,
    /// Version that contains a fix, if available.
    pub patched_version: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Advisory Database
// ═══════════════════════════════════════════════════════════════════════

/// A collection of security advisories.
#[derive(Debug, Clone)]
pub struct AdvisoryDatabase {
    /// All advisories in the database.
    pub advisories: Vec<Advisory>,
}

impl AdvisoryDatabase {
    /// Creates an empty advisory database.
    pub fn new() -> Self {
        Self {
            advisories: Vec::new(),
        }
    }

    /// Parses an advisory database from a JSON string.
    ///
    /// Expected format:
    /// ```json
    /// { "advisories": [
    ///   { "id": "...", "package": "...", "min_version": "...",
    ///     "max_version": "...", "severity": "...", "description": "...",
    ///     "patched_version": "..." }
    /// ] }
    /// ```
    pub fn from_json(data: &str) -> Result<Self, AuditError> {
        let trimmed = data.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(AuditError::DatabaseParseError(
                "expected JSON object".to_string(),
            ));
        }

        let advisories = parse_advisories_json(trimmed)?;
        Ok(Self { advisories })
    }

    /// Finds all advisories that affect a given package at a given version.
    pub fn check(&self, package: &str, version: &str) -> Vec<&Advisory> {
        self.advisories
            .iter()
            .filter(|a| a.package == package && a.versions_affected.affects(version))
            .collect()
    }
}

impl Default for AdvisoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Audit Finding & Report
// ═══════════════════════════════════════════════════════════════════════

/// A single finding from an audit scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding {
    /// Name of the affected package.
    pub package: String,
    /// Version of the affected package.
    pub version: String,
    /// The matching advisory.
    pub advisory_id: String,
    /// Severity of the advisory.
    pub severity: Severity,
    /// Description from the advisory.
    pub description: String,
    /// Suggested fix (e.g., upgrade to patched version).
    pub fix_suggestion: Option<String>,
}

/// The result of auditing all project dependencies.
#[derive(Debug, Clone)]
pub struct AuditReport {
    /// Dependencies with known vulnerabilities.
    pub affected: Vec<AuditFinding>,
    /// Number of dependencies with no known issues.
    pub clean_count: usize,
}

impl AuditReport {
    /// Returns `true` if any findings are `Critical` or `High` severity.
    ///
    /// Useful as a CI gate — non-zero exit code when this returns `true`.
    pub fn has_critical_or_high(&self) -> bool {
        self.affected
            .iter()
            .any(|f| f.severity == Severity::Critical || f.severity == Severity::High)
    }

    /// Returns the total number of findings.
    pub fn finding_count(&self) -> usize {
        self.affected.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Audit Ignore
// ═══════════════════════════════════════════════════════════════════════

/// An entry in the audit ignore list.
///
/// Allows suppressing specific advisories with a documented justification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditIgnore {
    /// Advisory ID to ignore (e.g., `FJ-2026-001`).
    pub advisory_id: String,
    /// Justification for ignoring this advisory.
    pub justification: String,
}

// ═══════════════════════════════════════════════════════════════════════
// Core Audit Function
// ═══════════════════════════════════════════════════════════════════════

/// Audits a list of dependencies against an advisory database.
///
/// Each dependency is a `(name, version)` pair. Returns an `AuditReport`
/// with all findings and the count of clean dependencies.
pub fn audit_dependencies(deps: &[(String, String)], db: &AdvisoryDatabase) -> AuditReport {
    audit_dependencies_with_ignores(deps, db, &[])
}

/// Audits dependencies with an ignore list.
///
/// Advisories whose IDs appear in `ignores` are excluded from findings.
pub fn audit_dependencies_with_ignores(
    deps: &[(String, String)],
    db: &AdvisoryDatabase,
    ignores: &[AuditIgnore],
) -> AuditReport {
    let ignore_ids: std::collections::HashSet<&str> =
        ignores.iter().map(|i| i.advisory_id.as_str()).collect();

    let mut affected = Vec::new();
    let mut clean_count = 0;

    for (name, version) in deps {
        let hits = db.check(name, version);
        let filtered: Vec<_> = hits
            .into_iter()
            .filter(|a| !ignore_ids.contains(a.id.as_str()))
            .collect();

        if filtered.is_empty() {
            clean_count += 1;
        } else {
            for advisory in filtered {
                let fix = advisory
                    .patched_version
                    .as_ref()
                    .map(|v| format!("upgrade to {v}"));

                affected.push(AuditFinding {
                    package: name.clone(),
                    version: version.clone(),
                    advisory_id: advisory.id.clone(),
                    severity: advisory.severity,
                    description: advisory.description.clone(),
                    fix_suggestion: fix,
                });
            }
        }
    }

    AuditReport {
        affected,
        clean_count,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Internal Helpers
// ═══════════════════════════════════════════════════════════════════════

/// Parses "major.minor.patch" into (major, minor, patch).
fn parse_version_parts(v: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = v.trim().split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;
    Some((major, minor, patch))
}

/// Compares two version tuples. Returns -1, 0, or 1.
fn compare_parts(a: &(u32, u32, u32), b: &(u32, u32, u32)) -> i32 {
    match a.0.cmp(&b.0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Equal => match a.1.cmp(&b.1) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => match a.2.cmp(&b.2) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Greater => 1,
                std::cmp::Ordering::Equal => 0,
            },
        },
    }
}

/// Parses the advisory array from a JSON string (minimal parser).
fn parse_advisories_json(data: &str) -> Result<Vec<Advisory>, AuditError> {
    // Find the advisories array
    let arr_start = data
        .find('[')
        .ok_or_else(|| AuditError::DatabaseParseError("missing '[' for advisories".to_string()))?;
    let arr_end = data
        .rfind(']')
        .ok_or_else(|| AuditError::DatabaseParseError("missing ']' for advisories".to_string()))?;

    if arr_start >= arr_end {
        return Ok(Vec::new());
    }

    let arr_content = &data[arr_start + 1..arr_end];
    let objects = split_json_objects(arr_content);

    let mut advisories = Vec::new();
    for obj in objects {
        let advisory = parse_advisory_object(&obj)?;
        advisories.push(advisory);
    }

    Ok(advisories)
}

/// Splits a JSON array body into individual object strings.
fn split_json_objects(s: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in s.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            current.push(ch);
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
        }

        if !in_string {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    current.push(ch);
                    objects.push(current.trim().to_string());
                    current = String::new();
                    continue;
                }
            }
            if depth == 0 && (ch == ',' || ch.is_whitespace()) {
                continue;
            }
        }
        if depth > 0 {
            current.push(ch);
        }
    }

    objects
}

/// Parses a single advisory JSON object.
fn parse_advisory_object(obj: &str) -> Result<Advisory, AuditError> {
    let fields = parse_flat_json(obj).map_err(AuditError::MalformedAdvisory)?;

    let id = fields
        .get("id")
        .ok_or_else(|| AuditError::MalformedAdvisory("missing 'id'".to_string()))?
        .clone();

    let package = fields
        .get("package")
        .ok_or_else(|| AuditError::MalformedAdvisory("missing 'package'".to_string()))?
        .clone();

    let severity_str = fields
        .get("severity")
        .ok_or_else(|| AuditError::MalformedAdvisory("missing 'severity'".to_string()))?;

    let severity = parse_severity(severity_str).ok_or_else(|| {
        AuditError::MalformedAdvisory(format!("invalid severity: {severity_str}"))
    })?;

    let description = fields.get("description").cloned().unwrap_or_default();

    let min_version = fields.get("min_version").cloned();
    let max_version = fields.get("max_version").cloned();
    let patched_version = fields.get("patched_version").cloned();

    Ok(Advisory {
        id,
        package,
        versions_affected: VersionRange::new(min_version, max_version),
        severity,
        description,
        patched_version,
    })
}

/// Parses a severity string into a `Severity` enum value.
fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_lowercase().as_str() {
        "critical" => Some(Severity::Critical),
        "high" => Some(Severity::High),
        "medium" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        _ => None,
    }
}

/// Parses a flat JSON object into key-value string pairs.
fn parse_flat_json(s: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let trimmed = s.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("not a JSON object".to_string());
    }
    let inner = &trimmed[1..trimmed.len() - 1];

    let mut fields = std::collections::HashMap::new();
    let parts = split_json_kv_pairs(inner);

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((key, value)) = parse_single_kv(part) {
            fields.insert(key, value);
        }
    }

    Ok(fields)
}

/// Splits JSON key-value pairs respecting quoted strings.
fn split_json_kv_pairs(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;

    for ch in s.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            current.push(ch);
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            current.push(ch);
            continue;
        }
        if ch == ',' && !in_string {
            parts.push(current.clone());
            current.clear();
            continue;
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Parses a `"key": "value"` pair.
fn parse_single_kv(s: &str) -> Option<(String, String)> {
    let colon = find_colon_outside_string(s)?;
    let key_part = s[..colon].trim();
    let val_part = s[colon + 1..].trim();

    let key = unquote(key_part)?;
    let value = if val_part.starts_with('"') {
        unquote(val_part)?
    } else if val_part == "null" {
        return None;
    } else {
        val_part.to_string()
    };

    Some((key, value))
}

/// Finds the first colon not inside a quoted string.
fn find_colon_outside_string(s: &str) -> Option<usize> {
    let mut in_string = false;
    let mut escape_next = false;
    for (i, ch) in s.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == ':' && !in_string {
            return Some(i);
        }
    }
    None
}

/// Removes surrounding double quotes from a JSON string.
fn unquote(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        Some(t[1..t.len() - 1].to_string())
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_db() -> AdvisoryDatabase {
        AdvisoryDatabase {
            advisories: vec![
                Advisory {
                    id: "FJ-2026-001".to_string(),
                    package: "fj-http".to_string(),
                    versions_affected: VersionRange::new(
                        Some("0.1.0".to_string()),
                        Some("0.3.0".to_string()),
                    ),
                    severity: Severity::Critical,
                    description: "Remote code execution via header injection".to_string(),
                    patched_version: Some("0.3.0".to_string()),
                },
                Advisory {
                    id: "FJ-2026-002".to_string(),
                    package: "fj-json".to_string(),
                    versions_affected: VersionRange::new(
                        Some("1.0.0".to_string()),
                        Some("1.2.0".to_string()),
                    ),
                    severity: Severity::Medium,
                    description: "Denial of service via deeply nested input".to_string(),
                    patched_version: Some("1.2.0".to_string()),
                },
                Advisory {
                    id: "FJ-2026-003".to_string(),
                    package: "fj-crypto".to_string(),
                    versions_affected: VersionRange::new(None, Some("2.0.0".to_string())),
                    severity: Severity::High,
                    description: "Weak key generation in ECDSA module".to_string(),
                    patched_version: Some("2.0.0".to_string()),
                },
            ],
        }
    }

    #[test]
    fn s23_1_version_range_affects_in_range() {
        let range = VersionRange::new(Some("1.0.0".to_string()), Some("2.0.0".to_string()));
        assert!(range.affects("1.0.0"));
        assert!(range.affects("1.5.3"));
        assert!(range.affects("1.99.99"));
        assert!(!range.affects("2.0.0")); // exclusive upper bound
        assert!(!range.affects("0.9.9"));
    }

    #[test]
    fn s23_2_version_range_unbounded() {
        let no_min = VersionRange::new(None, Some("2.0.0".to_string()));
        assert!(no_min.affects("0.0.1"));
        assert!(no_min.affects("1.9.9"));
        assert!(!no_min.affects("2.0.0"));

        let no_max = VersionRange::new(Some("1.0.0".to_string()), None);
        assert!(!no_max.affects("0.9.9"));
        assert!(no_max.affects("1.0.0"));
        assert!(no_max.affects("99.99.99"));
    }

    #[test]
    fn s23_3_severity_display_and_ordering() {
        assert_eq!(format!("{}", Severity::Low), "low");
        assert_eq!(format!("{}", Severity::Critical), "critical");
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn s23_4_advisory_database_check_finds_match() {
        let db = sample_db();
        let hits = db.check("fj-http", "0.2.5");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "FJ-2026-001");
    }

    #[test]
    fn s23_5_advisory_database_check_no_match() {
        let db = sample_db();
        let hits = db.check("fj-http", "0.3.0");
        assert!(hits.is_empty());

        let hits2 = db.check("fj-math", "1.0.0");
        assert!(hits2.is_empty());
    }

    #[test]
    fn s23_6_audit_dependencies_full_report() {
        let db = sample_db();
        let deps = vec![
            ("fj-http".to_string(), "0.2.0".to_string()),
            ("fj-json".to_string(), "1.1.0".to_string()),
            ("fj-math".to_string(), "3.0.0".to_string()),
        ];

        let report = audit_dependencies(&deps, &db);
        assert_eq!(report.affected.len(), 2);
        assert_eq!(report.clean_count, 1);
        assert!(report.has_critical_or_high());
    }

    #[test]
    fn s23_7_audit_dependencies_all_clean() {
        let db = sample_db();
        let deps = vec![
            ("fj-http".to_string(), "0.5.0".to_string()),
            ("fj-math".to_string(), "1.0.0".to_string()),
        ];

        let report = audit_dependencies(&deps, &db);
        assert!(report.affected.is_empty());
        assert_eq!(report.clean_count, 2);
        assert!(!report.has_critical_or_high());
    }

    #[test]
    fn s23_8_audit_with_ignore_list() {
        let db = sample_db();
        let deps = vec![("fj-http".to_string(), "0.2.0".to_string())];
        let ignores = vec![AuditIgnore {
            advisory_id: "FJ-2026-001".to_string(),
            justification: "mitigated by WAF".to_string(),
        }];

        let report = audit_dependencies_with_ignores(&deps, &db, &ignores);
        assert!(report.affected.is_empty());
        assert_eq!(report.clean_count, 1);
    }

    #[test]
    fn s23_9_advisory_database_from_json() {
        let json = r#"{
            "advisories": [
                {
                    "id": "FJ-2026-010",
                    "package": "fj-test",
                    "min_version": "0.1.0",
                    "max_version": "0.5.0",
                    "severity": "high",
                    "description": "Buffer overflow in parser",
                    "patched_version": "0.5.0"
                }
            ]
        }"#;

        let db = AdvisoryDatabase::from_json(json).unwrap();
        assert_eq!(db.advisories.len(), 1);
        assert_eq!(db.advisories[0].id, "FJ-2026-010");
        assert_eq!(db.advisories[0].severity, Severity::High);

        let hits = db.check("fj-test", "0.3.0");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn s23_10_fix_suggestion_in_findings() {
        let db = sample_db();
        let deps = vec![("fj-http".to_string(), "0.2.0".to_string())];
        let report = audit_dependencies(&deps, &db);
        assert_eq!(report.affected.len(), 1);
        assert_eq!(
            report.affected[0].fix_suggestion,
            Some("upgrade to 0.3.0".to_string())
        );
    }
}
