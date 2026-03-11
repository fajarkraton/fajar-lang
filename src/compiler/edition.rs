//! # Edition System & API Stability
//!
//! Provides edition management for Fajar Lang (Edition2025, Edition2026),
//! deprecation tracking, migration tooling, and API surface diffing
//! with SemVer validation.
//!
//! ## Architecture
//!
//! ```text
//! Edition config → DeprecationWarning → MigrationTool → MigrationFix
//! ApiSurface (old) + ApiSurface (new) → ApiDiff → SemVer validation
//! ```

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════════════════

/// Errors arising from edition parsing and API stability checks.
#[derive(Debug, Error)]
pub enum EditionError {
    /// An unrecognized edition string was provided.
    #[error("unknown edition: `{value}` (expected `2025` or `2026`)")]
    UnknownEdition {
        /// The invalid edition string.
        value: String,
    },

    /// A SemVer validation rule was violated.
    #[error("semver violation: {0}")]
    SemVer(SemVerError),
}

/// Semantic versioning violations detected during API diff validation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SemVerError {
    /// A breaking change was introduced in a minor version bump.
    #[error("breaking change in minor bump: {item}")]
    MajorChangeInMinor {
        /// The API item that introduced the breaking change.
        item: String,
    },

    /// A feature addition was introduced in a patch version bump.
    #[error("feature addition in patch bump: {item}")]
    MinorChangeInPatch {
        /// The API item that was added in a patch bump.
        item: String,
    },

    /// Items were removed without a major version bump.
    #[error("removal without major bump: {item}")]
    RemovalWithoutMajor {
        /// The removed API item.
        item: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Edition enum
// ═══════════════════════════════════════════════════════════════════════

/// The language edition for a Fajar Lang project.
///
/// Editions allow introducing new keywords, syntax changes, and
/// deprecations without breaking existing code on older editions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Edition {
    /// The 2025 edition (first stable release).
    Edition2025,
    /// The 2026 edition (current default, introduces new features).
    #[default]
    Edition2026,
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Edition::Edition2025 => write!(f, "2025"),
            Edition::Edition2026 => write!(f, "2026"),
        }
    }
}

impl FromStr for Edition {
    type Err = EditionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_edition(s)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Edition parsing
// ═══════════════════════════════════════════════════════════════════════

/// Parses an edition string into an [`Edition`] value.
///
/// Accepts `"2025"` or `"2026"`. Returns an error for any other input.
pub fn parse_edition(s: &str) -> Result<Edition, EditionError> {
    match s.trim() {
        "2025" => Ok(Edition::Edition2025),
        "2026" => Ok(Edition::Edition2026),
        other => Err(EditionError::UnknownEdition {
            value: other.to_string(),
        }),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// EditionConfig
// ═══════════════════════════════════════════════════════════════════════

/// Configuration derived from a specific edition.
///
/// Controls which keywords are reserved and which features are deprecated
/// for a given edition.
#[derive(Debug, Clone)]
pub struct EditionConfig {
    /// The active edition.
    pub edition: Edition,
    /// Keywords reserved in this edition (unavailable as identifiers).
    pub reserved_keywords: Vec<String>,
    /// Features deprecated in this edition.
    pub deprecated_features: Vec<String>,
}

impl EditionConfig {
    /// Creates a new edition configuration with the given parameters.
    pub fn new(
        edition: Edition,
        reserved_keywords: Vec<String>,
        deprecated_features: Vec<String>,
    ) -> Self {
        Self {
            edition,
            reserved_keywords,
            deprecated_features,
        }
    }

    /// Returns the default configuration for the given edition.
    pub fn for_edition(edition: Edition) -> Self {
        match edition {
            Edition::Edition2025 => Self::new(edition, default_keywords_2025(), Vec::new()),
            Edition::Edition2026 => Self::new(
                edition,
                default_keywords_2026(),
                default_deprecations_2026(),
            ),
        }
    }
}

/// Returns the reserved keywords for Edition2025.
fn default_keywords_2025() -> Vec<String> {
    [
        "let", "mut", "fn", "struct", "enum", "if", "else", "while", "for", "return", "match",
        "use", "mod", "pub", "impl", "trait",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Returns the reserved keywords for Edition2026 (superset of 2025).
fn default_keywords_2026() -> Vec<String> {
    let mut kw = default_keywords_2025();
    kw.extend(
        ["async", "await", "yield", "effect"]
            .iter()
            .map(|s| s.to_string()),
    );
    kw
}

/// Returns features deprecated in Edition2026.
fn default_deprecations_2026() -> Vec<String> {
    vec!["implicit_return_void".to_string()]
}

// ═══════════════════════════════════════════════════════════════════════
// StabilityLevel / FeatureGate
// ═══════════════════════════════════════════════════════════════════════

/// The stability classification of a language or API feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StabilityLevel {
    /// The feature is stable and covered by SemVer guarantees.
    Stable,
    /// The feature is experimental and may change without notice.
    Unstable,
    /// The feature is deprecated and will be removed in a future edition.
    Deprecated,
}

impl fmt::Display for StabilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StabilityLevel::Stable => write!(f, "stable"),
            StabilityLevel::Unstable => write!(f, "unstable"),
            StabilityLevel::Deprecated => write!(f, "deprecated"),
        }
    }
}

/// A feature gate controlling access to unstable or deprecated features.
#[derive(Debug, Clone)]
pub struct FeatureGate {
    /// The feature name (e.g., `"async_generators"`).
    pub name: String,
    /// The stability level of this feature.
    pub stability: StabilityLevel,
    /// The version since which this feature has its current stability.
    pub since_version: String,
    /// Optional tracking issue URL or number.
    pub tracking_issue: Option<String>,
}

impl FeatureGate {
    /// Creates a new feature gate.
    pub fn new(
        name: String,
        stability: StabilityLevel,
        since_version: String,
        tracking_issue: Option<String>,
    ) -> Self {
        Self {
            name,
            stability,
            since_version,
            tracking_issue,
        }
    }

    /// Returns `true` if this feature is usable without opting in.
    pub fn is_available(&self) -> bool {
        self.stability == StabilityLevel::Stable
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Deprecation checking
// ═══════════════════════════════════════════════════════════════════════

/// A deprecation warning for an API item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationWarning {
    /// The name of the deprecated item.
    pub item_name: String,
    /// The version since which the item is deprecated.
    pub since_version: String,
    /// A human-readable deprecation message.
    pub message: String,
    /// The suggested replacement (if any).
    pub replacement: Option<String>,
}

/// Checks whether an item is deprecated in the given edition.
///
/// Returns a [`DeprecationWarning`] if the item is deprecated, or `None`.
pub fn check_deprecation(item: &str, edition: Edition) -> Option<DeprecationWarning> {
    let table = build_deprecation_table();
    let key = (item.to_string(), edition);
    table.get(&key).cloned()
}

/// Builds the known deprecation lookup table.
fn build_deprecation_table() -> HashMap<(String, Edition), DeprecationWarning> {
    let mut table = HashMap::new();

    // Edition2026 deprecates implicit_return_void
    table.insert(
        ("implicit_return_void".to_string(), Edition::Edition2026),
        DeprecationWarning {
            item_name: "implicit_return_void".to_string(),
            since_version: "0.9.0".to_string(),
            message: "implicit void return is deprecated; use explicit `-> void`".to_string(),
            replacement: Some("-> void".to_string()),
        },
    );

    // Edition2026 deprecates old_style_cast
    table.insert(
        ("old_style_cast".to_string(), Edition::Edition2026),
        DeprecationWarning {
            item_name: "old_style_cast".to_string(),
            since_version: "0.9.0".to_string(),
            message: "C-style casts are deprecated; use `as` operator".to_string(),
            replacement: Some("as".to_string()),
        },
    );

    table
}

// ═══════════════════════════════════════════════════════════════════════
// Edition compatibility
// ═══════════════════════════════════════════════════════════════════════

/// Checks whether a library compiled with `lib_edition` is compatible
/// with an application using `app_edition`.
///
/// Libraries from older editions are compatible with newer application
/// editions, but not vice versa.
pub fn is_compatible(lib_edition: Edition, app_edition: Edition) -> bool {
    let lib_year = edition_year(lib_edition);
    let app_year = edition_year(app_edition);
    lib_year <= app_year
}

/// Returns the numeric year for an edition.
fn edition_year(edition: Edition) -> u32 {
    match edition {
        Edition::Edition2025 => 2025,
        Edition::Edition2026 => 2026,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Migration tool
// ═══════════════════════════════════════════════════════════════════════

/// A suggested code migration fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFix {
    /// The 1-based line number where the fix applies.
    pub line: usize,
    /// The old code that should be replaced.
    pub old_code: String,
    /// The new code to replace it with.
    pub new_code: String,
    /// A human-readable description of the migration.
    pub description: String,
}

/// A tool that suggests code migration fixes between editions.
#[derive(Debug)]
pub struct MigrationTool {
    /// Migration rules keyed by `(from_edition, to_edition)`.
    rules: Vec<MigrationRule>,
}

/// A single migration rule that matches patterns and suggests replacements.
#[derive(Debug, Clone)]
struct MigrationRule {
    /// The source edition this rule applies from.
    from: Edition,
    /// The target edition this rule applies to.
    to: Edition,
    /// The pattern to search for in source lines.
    pattern: String,
    /// The replacement text.
    replacement: String,
    /// Description of the migration.
    description: String,
}

impl MigrationTool {
    /// Creates a new migration tool with built-in rules.
    pub fn new() -> Self {
        Self {
            rules: default_migration_rules(),
        }
    }

    /// Suggests migration fixes for the given source code.
    ///
    /// Scans each line for patterns that need updating when moving
    /// from `from` edition to `to` edition.
    pub fn suggest_fixes(&self, code: &str, from: Edition, to: Edition) -> Vec<MigrationFix> {
        let mut fixes = Vec::new();

        for (line_idx, line) in code.lines().enumerate() {
            self.check_line_for_fixes(line, line_idx + 1, from, to, &mut fixes);
        }

        fixes
    }

    /// Checks a single line against all applicable migration rules.
    fn check_line_for_fixes(
        &self,
        line: &str,
        line_number: usize,
        from: Edition,
        to: Edition,
        fixes: &mut Vec<MigrationFix>,
    ) {
        for rule in &self.rules {
            if rule.from != from || rule.to != to {
                continue;
            }
            if line.contains(&rule.pattern) {
                let new_line = line.replace(&rule.pattern, &rule.replacement);
                fixes.push(MigrationFix {
                    line: line_number,
                    old_code: line.to_string(),
                    new_code: new_line,
                    description: rule.description.clone(),
                });
            }
        }
    }
}

impl Default for MigrationTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the built-in migration rules for edition transitions.
fn default_migration_rules() -> Vec<MigrationRule> {
    vec![
        MigrationRule {
            from: Edition::Edition2025,
            to: Edition::Edition2026,
            pattern: "yield_val".to_string(),
            replacement: "yield".to_string(),
            description: "rename `yield_val` to `yield` keyword (Edition2026)".to_string(),
        },
        MigrationRule {
            from: Edition::Edition2025,
            to: Edition::Edition2026,
            pattern: "async_fn".to_string(),
            replacement: "async fn".to_string(),
            description: "use `async fn` syntax instead of `async_fn` (Edition2026)".to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// API Stability — ApiItem, ApiSurface
// ═══════════════════════════════════════════════════════════════════════

/// The kind of an API item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiItemKind {
    /// A function.
    Fn,
    /// A struct type.
    Struct,
    /// An enum type.
    Enum,
    /// A trait definition.
    Trait,
    /// A constant value.
    Const,
    /// A type alias.
    Type,
}

impl fmt::Display for ApiItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiItemKind::Fn => write!(f, "fn"),
            ApiItemKind::Struct => write!(f, "struct"),
            ApiItemKind::Enum => write!(f, "enum"),
            ApiItemKind::Trait => write!(f, "trait"),
            ApiItemKind::Const => write!(f, "const"),
            ApiItemKind::Type => write!(f, "type"),
        }
    }
}

/// A single public API item in a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiItem {
    /// The fully-qualified name of the item.
    pub name: String,
    /// The kind of item (function, struct, enum, etc.).
    pub kind: ApiItemKind,
    /// Whether the item is publicly visible.
    pub visibility: bool,
    /// The stability level of this item.
    pub stability: StabilityLevel,
}

impl ApiItem {
    /// Creates a new API item descriptor.
    pub fn new(
        name: String,
        kind: ApiItemKind,
        visibility: bool,
        stability: StabilityLevel,
    ) -> Self {
        Self {
            name,
            kind,
            visibility,
            stability,
        }
    }
}

/// A collection of all public API items in a module or crate.
#[derive(Debug, Clone)]
pub struct ApiSurface {
    /// The API items indexed by name.
    pub items: HashMap<String, ApiItem>,
}

impl ApiSurface {
    /// Creates an empty API surface.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Adds an item to the API surface.
    pub fn add_item(&mut self, item: ApiItem) {
        self.items.insert(item.name.clone(), item);
    }

    /// Returns the number of items in the API surface.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the API surface has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for ApiSurface {
    fn default() -> Self {
        Self::new()
    }
}

/// Collects the API surface from a list of module item descriptors.
///
/// Each descriptor is `(name, kind_str, is_pub, stability)`.
pub fn collect_api_surface(
    module_items: &[(String, ApiItemKind, bool, StabilityLevel)],
) -> ApiSurface {
    let mut surface = ApiSurface::new();
    for (name, kind, vis, stab) in module_items {
        if *vis {
            surface.add_item(ApiItem::new(name.clone(), *kind, *vis, *stab));
        }
    }
    surface
}

// ═══════════════════════════════════════════════════════════════════════
// StabilityChecker
// ═══════════════════════════════════════════════════════════════════════

/// Checks stability annotations on public API items.
#[derive(Debug)]
pub struct StabilityChecker {
    /// Required stability level for public items.
    pub required_level: StabilityLevel,
}

impl StabilityChecker {
    /// Creates a new stability checker requiring the given minimum level.
    pub fn new(required_level: StabilityLevel) -> Self {
        Self { required_level }
    }

    /// Checks all items in the API surface and returns unstable public items.
    pub fn check(&self, surface: &ApiSurface) -> Vec<String> {
        let mut violations = Vec::new();
        for item in surface.items.values() {
            if item.visibility && !self.meets_requirement(item.stability) {
                violations.push(item.name.clone());
            }
        }
        violations.sort();
        violations
    }

    /// Returns `true` if the given level meets the required threshold.
    fn meets_requirement(&self, level: StabilityLevel) -> bool {
        match self.required_level {
            StabilityLevel::Stable => level == StabilityLevel::Stable,
            StabilityLevel::Unstable => {
                level == StabilityLevel::Stable || level == StabilityLevel::Unstable
            }
            StabilityLevel::Deprecated => true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// API Diff
// ═══════════════════════════════════════════════════════════════════════

/// A change to a single API item between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiChange {
    /// The name of the changed item.
    pub item_name: String,
    /// The old signature or description.
    pub old_signature: String,
    /// The new signature or description.
    pub new_signature: String,
    /// Whether this change is breaking (requires major bump).
    pub breaking: bool,
}

/// The difference between two API surfaces.
#[derive(Debug, Clone)]
pub struct ApiDiff {
    /// Items present in new but not in old.
    pub added: Vec<ApiItem>,
    /// Items present in old but not in new.
    pub removed: Vec<ApiItem>,
    /// Items present in both but with changes.
    pub changed: Vec<ApiChange>,
}

impl ApiDiff {
    /// Returns `true` if the diff contains any breaking changes.
    pub fn has_breaking_changes(&self) -> bool {
        !self.removed.is_empty() || self.changed.iter().any(|c| c.breaking)
    }

    /// Returns `true` if the diff contains any additions.
    pub fn has_additions(&self) -> bool {
        !self.added.is_empty()
    }

    /// Returns `true` if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Computes the difference between two API surfaces.
///
/// Identifies added, removed, and changed items. A change in item kind
/// or stability is considered a modification; removal of a public item
/// is always a breaking change.
pub fn compute_api_diff(old: &ApiSurface, new: &ApiSurface) -> ApiDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    // Find removed and changed items
    for (name, old_item) in &old.items {
        match new.items.get(name) {
            None => removed.push(old_item.clone()),
            Some(new_item) => {
                detect_item_changes(old_item, new_item, &mut changed);
            }
        }
    }

    // Find added items
    for (name, new_item) in &new.items {
        if !old.items.contains_key(name) {
            added.push(new_item.clone());
        }
    }

    // Sort for deterministic output
    added.sort_by(|a, b| a.name.cmp(&b.name));
    removed.sort_by(|a, b| a.name.cmp(&b.name));
    changed.sort_by(|a, b| a.item_name.cmp(&b.item_name));

    ApiDiff {
        added,
        removed,
        changed,
    }
}

/// Detects changes between two versions of the same API item.
fn detect_item_changes(old_item: &ApiItem, new_item: &ApiItem, changed: &mut Vec<ApiChange>) {
    let kind_changed = old_item.kind != new_item.kind;
    let stability_changed = old_item.stability != new_item.stability;
    let vis_changed = old_item.visibility != new_item.visibility;

    if kind_changed || stability_changed || vis_changed {
        let breaking = kind_changed || (old_item.visibility && !new_item.visibility);

        changed.push(ApiChange {
            item_name: old_item.name.clone(),
            old_signature: format!(
                "{} {} ({})",
                old_item.kind, old_item.name, old_item.stability
            ),
            new_signature: format!(
                "{} {} ({})",
                new_item.kind, new_item.name, new_item.stability
            ),
            breaking,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SemVer validation
// ═══════════════════════════════════════════════════════════════════════

/// A parsed semantic version (major.minor.patch).
#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u32,
    minor: u32,
    patch: u32,
}

/// Parses a semantic version string like `"1.2.3"`.
fn parse_semver(version: &str) -> Result<SemVer, EditionError> {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if parts.len() != 3 {
        return Err(EditionError::UnknownEdition {
            value: format!("invalid semver: {version}"),
        });
    }
    let parse_part = |s: &str| -> Result<u32, EditionError> {
        s.parse::<u32>().map_err(|_| EditionError::UnknownEdition {
            value: format!("invalid semver component: {s}"),
        })
    };
    Ok(SemVer {
        major: parse_part(parts[0])?,
        minor: parse_part(parts[1])?,
        patch: parse_part(parts[2])?,
    })
}

/// Determines the kind of version bump between two versions.
fn bump_kind(old: &SemVer, new: &SemVer) -> BumpKind {
    if new.major > old.major {
        BumpKind::Major
    } else if new.minor > old.minor {
        BumpKind::Minor
    } else {
        BumpKind::Patch
    }
}

/// The kind of semantic version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BumpKind {
    Major,
    Minor,
    Patch,
}

/// Validates that the API diff is consistent with the SemVer bump.
///
/// - **Major bump**: anything goes.
/// - **Minor bump**: additions OK, but no removals or breaking changes.
/// - **Patch bump**: no additions, no removals, no breaking changes.
pub fn validate_semver(old_ver: &str, new_ver: &str, diff: &ApiDiff) -> Result<(), EditionError> {
    let old = parse_semver(old_ver)?;
    let new = parse_semver(new_ver)?;
    let kind = bump_kind(&old, &new);

    validate_removals(kind, diff)?;
    validate_breaking_changes(kind, diff)?;
    validate_additions(kind, diff)?;

    Ok(())
}

/// Validates that removals are only in major bumps.
fn validate_removals(kind: BumpKind, diff: &ApiDiff) -> Result<(), EditionError> {
    if kind != BumpKind::Major {
        if let Some(item) = diff.removed.first() {
            return Err(EditionError::SemVer(SemVerError::RemovalWithoutMajor {
                item: item.name.clone(),
            }));
        }
    }
    Ok(())
}

/// Validates that breaking changes are only in major bumps.
fn validate_breaking_changes(kind: BumpKind, diff: &ApiDiff) -> Result<(), EditionError> {
    if kind != BumpKind::Major {
        for change in &diff.changed {
            if change.breaking {
                return Err(EditionError::SemVer(SemVerError::MajorChangeInMinor {
                    item: change.item_name.clone(),
                }));
            }
        }
    }
    Ok(())
}

/// Validates that additions are only in minor or major bumps.
fn validate_additions(kind: BumpKind, diff: &ApiDiff) -> Result<(), EditionError> {
    if kind == BumpKind::Patch {
        if let Some(item) = diff.added.first() {
            return Err(EditionError::SemVer(SemVerError::MinorChangeInPatch {
                item: item.name.clone(),
            }));
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// API report generation
// ═══════════════════════════════════════════════════════════════════════

/// Generates a human-readable API diff report.
///
/// The report lists added, removed, and changed items with their
/// signatures and breaking-change annotations.
pub fn generate_api_report(diff: &ApiDiff) -> String {
    let mut report = String::new();
    report.push_str("=== API Diff Report ===\n\n");

    append_added_section(&mut report, &diff.added);
    append_removed_section(&mut report, &diff.removed);
    append_changed_section(&mut report, &diff.changed);
    append_summary(&mut report, diff);

    report
}

/// Appends the "Added" section to the report.
fn append_added_section(report: &mut String, added: &[ApiItem]) {
    if added.is_empty() {
        return;
    }
    report.push_str("## Added\n");
    for item in added {
        report.push_str(&format!(
            "  + {} {} ({})\n",
            item.kind, item.name, item.stability
        ));
    }
    report.push('\n');
}

/// Appends the "Removed" section to the report.
fn append_removed_section(report: &mut String, removed: &[ApiItem]) {
    if removed.is_empty() {
        return;
    }
    report.push_str("## Removed [BREAKING]\n");
    for item in removed {
        report.push_str(&format!("  - {} {}\n", item.kind, item.name));
    }
    report.push('\n');
}

/// Appends the "Changed" section to the report.
fn append_changed_section(report: &mut String, changed: &[ApiChange]) {
    if changed.is_empty() {
        return;
    }
    report.push_str("## Changed\n");
    for change in changed {
        let marker = if change.breaking { " [BREAKING]" } else { "" };
        report.push_str(&format!(
            "  ~ {}{}\n    old: {}\n    new: {}\n",
            change.item_name, marker, change.old_signature, change.new_signature,
        ));
    }
    report.push('\n');
}

/// Appends a summary line to the report.
fn append_summary(report: &mut String, diff: &ApiDiff) {
    let breaking = if diff.has_breaking_changes() {
        "YES"
    } else {
        "no"
    };
    report.push_str(&format!(
        "Summary: {} added, {} removed, {} changed (breaking: {})\n",
        diff.added.len(),
        diff.removed.len(),
        diff.changed.len(),
        breaking,
    ));
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Sprint 25: Edition System ───────────────────────────────────

    #[test]
    fn s25_1_edition_display_and_default() {
        assert_eq!(Edition::Edition2025.to_string(), "2025");
        assert_eq!(Edition::Edition2026.to_string(), "2026");
        assert_eq!(Edition::default(), Edition::Edition2026);
    }

    #[test]
    fn s25_2_parse_edition_valid_and_invalid() {
        assert_eq!(parse_edition("2025").unwrap(), Edition::Edition2025);
        assert_eq!(parse_edition("2026").unwrap(), Edition::Edition2026);
        assert!(parse_edition("2024").is_err());
        assert!(parse_edition("").is_err());
        assert!(parse_edition("abc").is_err());

        // FromStr trait
        let ed: Edition = "2025".parse().unwrap();
        assert_eq!(ed, Edition::Edition2025);
    }

    #[test]
    fn s25_3_edition_config_for_edition() {
        let cfg_2025 = EditionConfig::for_edition(Edition::Edition2025);
        assert_eq!(cfg_2025.edition, Edition::Edition2025);
        assert!(cfg_2025.reserved_keywords.contains(&"let".to_string()));
        assert!(!cfg_2025.reserved_keywords.contains(&"async".to_string()));
        assert!(cfg_2025.deprecated_features.is_empty());

        let cfg_2026 = EditionConfig::for_edition(Edition::Edition2026);
        assert_eq!(cfg_2026.edition, Edition::Edition2026);
        assert!(cfg_2026.reserved_keywords.contains(&"async".to_string()));
        assert!(cfg_2026.reserved_keywords.contains(&"await".to_string()));
        assert!(!cfg_2026.deprecated_features.is_empty());
    }

    #[test]
    fn s25_4_deprecation_check_returns_warning() {
        let warning = check_deprecation("implicit_return_void", Edition::Edition2026);
        assert!(warning.is_some());
        let w = warning.unwrap();
        assert_eq!(w.item_name, "implicit_return_void");
        assert_eq!(w.since_version, "0.9.0");
        assert!(w.replacement.is_some());
        assert_eq!(w.replacement.unwrap(), "-> void");

        // Not deprecated in 2025
        assert!(check_deprecation("implicit_return_void", Edition::Edition2025).is_none());

        // Unknown item not deprecated
        assert!(check_deprecation("nonexistent_feature", Edition::Edition2026).is_none());
    }

    #[test]
    fn s25_5_migration_tool_suggest_fixes() {
        let tool = MigrationTool::new();
        let code = "let x = async_fn()\nlet y = yield_val(42)\nlet z = 100";
        let fixes = tool.suggest_fixes(code, Edition::Edition2025, Edition::Edition2026);

        assert_eq!(fixes.len(), 2);

        // yield_val -> yield
        let yield_fix = fixes.iter().find(|f| f.line == 2).unwrap();
        assert!(yield_fix.old_code.contains("yield_val"));
        assert!(yield_fix.new_code.contains("yield"));
        assert!(!yield_fix.new_code.contains("yield_val"));

        // async_fn -> async fn
        let async_fix = fixes.iter().find(|f| f.line == 1).unwrap();
        assert!(async_fix.old_code.contains("async_fn"));
        assert!(async_fix.new_code.contains("async fn"));
    }

    #[test]
    fn s25_6_migration_no_fixes_same_edition() {
        let tool = MigrationTool::new();
        let code = "let x = async_fn()\nlet y = yield_val(42)";
        let fixes = tool.suggest_fixes(code, Edition::Edition2025, Edition::Edition2025);
        assert!(fixes.is_empty());
    }

    #[test]
    fn s25_7_edition_compatibility() {
        // Older lib is compatible with newer app
        assert!(is_compatible(Edition::Edition2025, Edition::Edition2026));
        // Same edition is compatible
        assert!(is_compatible(Edition::Edition2025, Edition::Edition2025));
        assert!(is_compatible(Edition::Edition2026, Edition::Edition2026));
        // Newer lib is NOT compatible with older app
        assert!(!is_compatible(Edition::Edition2026, Edition::Edition2025));
    }

    #[test]
    fn s25_8_stability_level_display() {
        assert_eq!(StabilityLevel::Stable.to_string(), "stable");
        assert_eq!(StabilityLevel::Unstable.to_string(), "unstable");
        assert_eq!(StabilityLevel::Deprecated.to_string(), "deprecated");
    }

    #[test]
    fn s25_9_feature_gate_creation_and_availability() {
        let stable_gate = FeatureGate::new(
            "pattern_matching".to_string(),
            StabilityLevel::Stable,
            "0.3.0".to_string(),
            None,
        );
        assert!(stable_gate.is_available());

        let unstable_gate = FeatureGate::new(
            "async_generators".to_string(),
            StabilityLevel::Unstable,
            "0.9.0".to_string(),
            Some("#42".to_string()),
        );
        assert!(!unstable_gate.is_available());
        assert_eq!(unstable_gate.tracking_issue, Some("#42".to_string()));
    }

    #[test]
    fn s25_10_edition_config_custom_construction() {
        let cfg = EditionConfig::new(
            Edition::Edition2025,
            vec!["custom_kw".to_string()],
            vec!["old_feature".to_string()],
        );
        assert_eq!(cfg.edition, Edition::Edition2025);
        assert_eq!(cfg.reserved_keywords, vec!["custom_kw".to_string()]);
        assert_eq!(cfg.deprecated_features, vec!["old_feature".to_string()]);
    }

    // ─── Sprint 26: API Stability ────────────────────────────────────

    #[test]
    fn s26_1_api_item_creation() {
        let item = ApiItem::new(
            "my_func".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        );
        assert_eq!(item.name, "my_func");
        assert_eq!(item.kind, ApiItemKind::Fn);
        assert!(item.visibility);
        assert_eq!(item.stability, StabilityLevel::Stable);
    }

    #[test]
    fn s26_2_api_surface_collect_and_filter() {
        let items = vec![
            (
                "add".to_string(),
                ApiItemKind::Fn,
                true,
                StabilityLevel::Stable,
            ),
            (
                "_helper".to_string(),
                ApiItemKind::Fn,
                false,
                StabilityLevel::Stable,
            ),
            (
                "Point".to_string(),
                ApiItemKind::Struct,
                true,
                StabilityLevel::Stable,
            ),
        ];
        let surface = collect_api_surface(&items);
        // Only public items are collected
        assert_eq!(surface.len(), 2);
        assert!(surface.items.contains_key("add"));
        assert!(surface.items.contains_key("Point"));
        assert!(!surface.items.contains_key("_helper"));
    }

    #[test]
    fn s26_3_stability_checker_finds_violations() {
        let mut surface = ApiSurface::new();
        surface.add_item(ApiItem::new(
            "stable_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        surface.add_item(ApiItem::new(
            "unstable_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Unstable,
        ));
        surface.add_item(ApiItem::new(
            "deprecated_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Deprecated,
        ));

        let checker = StabilityChecker::new(StabilityLevel::Stable);
        let violations = checker.check(&surface);

        assert_eq!(violations.len(), 2);
        assert!(violations.contains(&"unstable_fn".to_string()));
        assert!(violations.contains(&"deprecated_fn".to_string()));
        assert!(!violations.contains(&"stable_fn".to_string()));
    }

    #[test]
    fn s26_4_compute_api_diff_additions() {
        let old = ApiSurface::new();
        let mut new = ApiSurface::new();
        new.add_item(ApiItem::new(
            "new_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));

        let diff = compute_api_diff(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "new_fn");
        assert!(diff.removed.is_empty());
        assert!(diff.changed.is_empty());
        assert!(diff.has_additions());
        assert!(!diff.has_breaking_changes());
    }

    #[test]
    fn s26_5_compute_api_diff_removals() {
        let mut old = ApiSurface::new();
        old.add_item(ApiItem::new(
            "old_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let new = ApiSurface::new();

        let diff = compute_api_diff(&old, &new);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "old_fn");
        assert!(diff.has_breaking_changes());
    }

    #[test]
    fn s26_6_compute_api_diff_changes() {
        let mut old = ApiSurface::new();
        old.add_item(ApiItem::new(
            "my_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let mut new = ApiSurface::new();
        // Kind changed: Fn -> Struct (breaking)
        new.add_item(ApiItem::new(
            "my_fn".to_string(),
            ApiItemKind::Struct,
            true,
            StabilityLevel::Stable,
        ));

        let diff = compute_api_diff(&old, &new);
        assert_eq!(diff.changed.len(), 1);
        assert!(diff.changed[0].breaking);
        assert_eq!(diff.changed[0].item_name, "my_fn");
        assert!(diff.has_breaking_changes());
    }

    #[test]
    fn s26_7_validate_semver_major_allows_breaking() {
        let mut old = ApiSurface::new();
        old.add_item(ApiItem::new(
            "removed".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let new = ApiSurface::new();
        let diff = compute_api_diff(&old, &new);

        // Major bump allows removals
        assert!(validate_semver("1.0.0", "2.0.0", &diff).is_ok());
    }

    #[test]
    fn s26_8_validate_semver_minor_rejects_breaking() {
        let mut old = ApiSurface::new();
        old.add_item(ApiItem::new(
            "removed".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let new = ApiSurface::new();
        let diff = compute_api_diff(&old, &new);

        // Minor bump rejects removals
        let result = validate_semver("1.0.0", "1.1.0", &diff);
        assert!(result.is_err());
        match result {
            Err(EditionError::SemVer(SemVerError::RemovalWithoutMajor { item })) => {
                assert_eq!(item, "removed");
            }
            other => panic!("expected RemovalWithoutMajor, got: {:?}", other),
        }
    }

    #[test]
    fn s26_9_validate_semver_patch_rejects_additions() {
        let old = ApiSurface::new();
        let mut new = ApiSurface::new();
        new.add_item(ApiItem::new(
            "new_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let diff = compute_api_diff(&old, &new);

        // Patch bump rejects additions
        let result = validate_semver("1.0.0", "1.0.1", &diff);
        assert!(result.is_err());
        match result {
            Err(EditionError::SemVer(SemVerError::MinorChangeInPatch { item })) => {
                assert_eq!(item, "new_fn");
            }
            other => panic!("expected MinorChangeInPatch, got: {:?}", other),
        }
    }

    #[test]
    fn s26_10_generate_api_report() {
        let mut old = ApiSurface::new();
        old.add_item(ApiItem::new(
            "old_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));
        let mut new = ApiSurface::new();
        new.add_item(ApiItem::new(
            "new_fn".to_string(),
            ApiItemKind::Fn,
            true,
            StabilityLevel::Stable,
        ));

        let diff = compute_api_diff(&old, &new);
        let report = generate_api_report(&diff);

        assert!(report.contains("API Diff Report"));
        assert!(report.contains("Added"));
        assert!(report.contains("new_fn"));
        assert!(report.contains("Removed"));
        assert!(report.contains("old_fn"));
        assert!(report.contains("BREAKING"));
        assert!(report.contains("Summary:"));
        assert!(report.contains("1 added"));
        assert!(report.contains("1 removed"));
    }
}
