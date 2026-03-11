//! Conditional Compilation — cfg attributes, feature flags, target
//! architecture, cfg combinators, platform modules, cfg checking.

use std::collections::HashSet;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S23.1: Cfg Attributes
// ═══════════════════════════════════════════════════════════════════════

/// A cfg predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfgPredicate {
    /// Key-value: `@cfg(key = "value")`.
    KeyValue { key: String, value: String },
    /// Key-only: `@cfg(test)`.
    Flag(String),
    /// All: `@cfg(all(a, b))`.
    All(Vec<CfgPredicate>),
    /// Any: `@cfg(any(a, b))`.
    Any(Vec<CfgPredicate>),
    /// Not: `@cfg(not(a))`.
    Not(Box<CfgPredicate>),
}

impl fmt::Display for CfgPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CfgPredicate::KeyValue { key, value } => write!(f, "{key} = \"{value}\""),
            CfgPredicate::Flag(name) => write!(f, "{name}"),
            CfgPredicate::All(preds) => {
                let inner: Vec<String> = preds.iter().map(|p| p.to_string()).collect();
                write!(f, "all({})", inner.join(", "))
            }
            CfgPredicate::Any(preds) => {
                let inner: Vec<String> = preds.iter().map(|p| p.to_string()).collect();
                write!(f, "any({})", inner.join(", "))
            }
            CfgPredicate::Not(pred) => write!(f, "not({pred})"),
        }
    }
}

/// Evaluation context for cfg predicates.
#[derive(Debug, Clone)]
pub struct CfgContext {
    /// Active flags (e.g., "test", "bench").
    pub flags: HashSet<String>,
    /// Key-value pairs (e.g., target_os="linux").
    pub values: std::collections::HashMap<String, String>,
}

impl CfgContext {
    /// Creates a new context.
    pub fn new() -> Self {
        Self {
            flags: HashSet::new(),
            values: std::collections::HashMap::new(),
        }
    }

    /// Sets a flag.
    pub fn set_flag(&mut self, flag: &str) {
        self.flags.insert(flag.to_string());
    }

    /// Sets a key-value pair.
    pub fn set_value(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    /// Evaluates a cfg predicate against this context.
    pub fn evaluate(&self, pred: &CfgPredicate) -> bool {
        match pred {
            CfgPredicate::Flag(name) => self.flags.contains(name),
            CfgPredicate::KeyValue { key, value } => {
                self.values.get(key).is_some_and(|v| v == value)
            }
            CfgPredicate::All(preds) => preds.iter().all(|p| self.evaluate(p)),
            CfgPredicate::Any(preds) => preds.iter().any(|p| self.evaluate(p)),
            CfgPredicate::Not(pred) => !self.evaluate(pred),
        }
    }
}

impl Default for CfgContext {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S23.2: Feature Flags
// ═══════════════════════════════════════════════════════════════════════

/// Feature definition from fj.toml.
#[derive(Debug, Clone)]
pub struct FeatureDef {
    /// Feature name.
    pub name: String,
    /// Sub-features this feature enables.
    pub enables: Vec<String>,
    /// Whether this is a default feature.
    pub is_default: bool,
}

/// Feature set configuration from fj.toml.
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// All defined features.
    pub features: Vec<FeatureDef>,
    /// Default features.
    pub defaults: Vec<String>,
}

impl FeatureSet {
    /// Creates a new feature set.
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
            defaults: Vec::new(),
        }
    }

    /// Adds a feature.
    pub fn add_feature(&mut self, name: &str, enables: Vec<String>, is_default: bool) {
        if is_default {
            self.defaults.push(name.to_string());
        }
        self.features.push(FeatureDef {
            name: name.to_string(),
            enables,
            is_default,
        });
    }

    /// Resolves all enabled features (transitively) given user-selected features.
    pub fn resolve(&self, selected: &[String]) -> HashSet<String> {
        let mut enabled: HashSet<String> = selected.iter().cloned().collect();
        let mut changed = true;

        while changed {
            changed = false;
            for feat in &self.features {
                if enabled.contains(&feat.name) {
                    for sub in &feat.enables {
                        if enabled.insert(sub.clone()) {
                            changed = true;
                        }
                    }
                }
            }
        }

        enabled
    }

    /// Returns the default set of features.
    pub fn default_features(&self) -> HashSet<String> {
        self.resolve(&self.defaults)
    }
}

impl Default for FeatureSet {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S23.3: Target Architecture
// ═══════════════════════════════════════════════════════════════════════

/// Known target architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    /// x86_64 (amd64).
    X86_64,
    /// AArch64 (arm64).
    Aarch64,
    /// RISC-V 64-bit.
    Riscv64,
    /// RISC-V 32-bit.
    Riscv32,
    /// WebAssembly 32-bit.
    Wasm32,
    /// ARM 32-bit (Thumb).
    Arm,
    /// Xtensa (ESP32).
    Xtensa,
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetArch::X86_64 => write!(f, "x86_64"),
            TargetArch::Aarch64 => write!(f, "aarch64"),
            TargetArch::Riscv64 => write!(f, "riscv64"),
            TargetArch::Riscv32 => write!(f, "riscv32"),
            TargetArch::Wasm32 => write!(f, "wasm32"),
            TargetArch::Arm => write!(f, "arm"),
            TargetArch::Xtensa => write!(f, "xtensa"),
        }
    }
}

/// Parses a target architecture from string.
pub fn parse_target_arch(s: &str) -> Option<TargetArch> {
    match s {
        "x86_64" | "amd64" => Some(TargetArch::X86_64),
        "aarch64" | "arm64" => Some(TargetArch::Aarch64),
        "riscv64" => Some(TargetArch::Riscv64),
        "riscv32" => Some(TargetArch::Riscv32),
        "wasm32" => Some(TargetArch::Wasm32),
        "arm" | "thumbv7em" | "thumbv6m" => Some(TargetArch::Arm),
        "xtensa" => Some(TargetArch::Xtensa),
        _ => None,
    }
}

/// Known target OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOs {
    /// Linux.
    Linux,
    /// macOS.
    Macos,
    /// Windows.
    Windows,
    /// None (bare metal).
    None,
    /// Zephyr RTOS.
    Zephyr,
    /// FreeRTOS.
    FreeRtos,
}

impl fmt::Display for TargetOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetOs::Linux => write!(f, "linux"),
            TargetOs::Macos => write!(f, "macos"),
            TargetOs::Windows => write!(f, "windows"),
            TargetOs::None => write!(f, "none"),
            TargetOs::Zephyr => write!(f, "zephyr"),
            TargetOs::FreeRtos => write!(f, "freertos"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S23.4: Cfg Combinators (covered by CfgPredicate::All/Any/Not above)
// ═══════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════
// S23.5: Platform Modules
// ═══════════════════════════════════════════════════════════════════════

/// A conditionally compiled module.
#[derive(Debug, Clone)]
pub struct ConditionalModule {
    /// Module name.
    pub name: String,
    /// Cfg predicate (must be true to include).
    pub predicate: CfgPredicate,
}

/// Filters modules based on cfg context.
pub fn filter_modules(modules: &[ConditionalModule], ctx: &CfgContext) -> Vec<String> {
    modules
        .iter()
        .filter(|m| ctx.evaluate(&m.predicate))
        .map(|m| m.name.clone())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// S23.6: Cfg in Tests
// ═══════════════════════════════════════════════════════════════════════

/// Standard cfg flags.
pub const CFG_TEST: &str = "test";
/// Benchmark cfg flag.
pub const CFG_BENCH: &str = "bench";
/// Debug cfg flag.
pub const CFG_DEBUG: &str = "debug_assertions";

// ═══════════════════════════════════════════════════════════════════════
// S23.7: Cfg Checking
// ═══════════════════════════════════════════════════════════════════════

/// Known cfg keys for typo detection.
const KNOWN_CFG_KEYS: &[&str] = &[
    "target_os",
    "target_arch",
    "target_family",
    "target_env",
    "target_endian",
    "target_pointer_width",
    "feature",
    "test",
    "bench",
    "debug_assertions",
];

/// Checks a cfg key for typos.
pub fn check_cfg_key(key: &str) -> Option<CfgWarning> {
    if KNOWN_CFG_KEYS.contains(&key) {
        return None;
    }
    // Check for close matches
    let suggestion = KNOWN_CFG_KEYS
        .iter()
        .filter(|&&known| edit_distance(key, known) <= 2)
        .min_by_key(|&&known| edit_distance(key, known))
        .map(|s| s.to_string());

    Some(CfgWarning {
        key: key.to_string(),
        suggestion,
    })
}

/// A cfg key warning (possible typo).
#[derive(Debug, Clone)]
pub struct CfgWarning {
    /// The suspicious key.
    pub key: String,
    /// Suggested correction.
    pub suggestion: Option<String>,
}

impl fmt::Display for CfgWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref suggestion) = self.suggestion {
            write!(
                f,
                "Unknown cfg key '{}' — did you mean '{suggestion}'?",
                self.key
            )
        } else {
            write!(f, "Unknown cfg key '{}'", self.key)
        }
    }
}

/// Simple edit distance (Levenshtein).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate().take(n + 1) {
        *val = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S23.1 — Cfg Attributes
    #[test]
    fn s23_1_cfg_flag() {
        let mut ctx = CfgContext::new();
        ctx.set_flag("test");
        let pred = CfgPredicate::Flag("test".into());
        assert!(ctx.evaluate(&pred));
        assert!(!ctx.evaluate(&CfgPredicate::Flag("bench".into())));
    }

    #[test]
    fn s23_1_cfg_key_value() {
        let mut ctx = CfgContext::new();
        ctx.set_value("target_os", "linux");
        let pred = CfgPredicate::KeyValue {
            key: "target_os".into(),
            value: "linux".into(),
        };
        assert!(ctx.evaluate(&pred));
    }

    // S23.2 — Feature Flags
    #[test]
    fn s23_2_feature_set() {
        let mut fs = FeatureSet::new();
        fs.add_feature("std", vec![], true);
        fs.add_feature("gpu", vec!["cuda".into()], false);
        fs.add_feature("cuda", vec![], false);

        let resolved = fs.resolve(&["gpu".into()]);
        assert!(resolved.contains("gpu"));
        assert!(resolved.contains("cuda"));
        assert!(!resolved.contains("std"));
    }

    #[test]
    fn s23_2_default_features() {
        let mut fs = FeatureSet::new();
        fs.add_feature("std", vec![], true);
        fs.add_feature("gpu", vec![], false);

        let defaults = fs.default_features();
        assert!(defaults.contains("std"));
        assert!(!defaults.contains("gpu"));
    }

    // S23.3 — Target Architecture
    #[test]
    fn s23_3_parse_target_arch() {
        assert_eq!(parse_target_arch("x86_64"), Some(TargetArch::X86_64));
        assert_eq!(parse_target_arch("aarch64"), Some(TargetArch::Aarch64));
        assert_eq!(parse_target_arch("wasm32"), Some(TargetArch::Wasm32));
        assert_eq!(parse_target_arch("unknown"), None);
    }

    #[test]
    fn s23_3_target_arch_display() {
        assert_eq!(TargetArch::X86_64.to_string(), "x86_64");
        assert_eq!(TargetArch::Aarch64.to_string(), "aarch64");
        assert_eq!(TargetArch::Riscv64.to_string(), "riscv64");
    }

    // S23.4 — Cfg Combinators
    #[test]
    fn s23_4_cfg_all() {
        let mut ctx = CfgContext::new();
        ctx.set_value("target_os", "linux");
        ctx.set_flag("gpu");

        let pred = CfgPredicate::All(vec![
            CfgPredicate::KeyValue {
                key: "target_os".into(),
                value: "linux".into(),
            },
            CfgPredicate::Flag("gpu".into()),
        ]);
        assert!(ctx.evaluate(&pred));
    }

    #[test]
    fn s23_4_cfg_any() {
        let mut ctx = CfgContext::new();
        ctx.set_value("target_os", "windows");

        let pred = CfgPredicate::Any(vec![
            CfgPredicate::KeyValue {
                key: "target_os".into(),
                value: "linux".into(),
            },
            CfgPredicate::KeyValue {
                key: "target_os".into(),
                value: "windows".into(),
            },
        ]);
        assert!(ctx.evaluate(&pred));
    }

    #[test]
    fn s23_4_cfg_not() {
        let mut ctx = CfgContext::new();
        ctx.set_value("target_os", "linux");

        let pred = CfgPredicate::Not(Box::new(CfgPredicate::KeyValue {
            key: "target_os".into(),
            value: "windows".into(),
        }));
        assert!(ctx.evaluate(&pred));
    }

    // S23.5 — Platform Modules
    #[test]
    fn s23_5_filter_modules() {
        let mut ctx = CfgContext::new();
        ctx.set_value("target_os", "linux");

        let modules = vec![
            ConditionalModule {
                name: "linux_impl".into(),
                predicate: CfgPredicate::KeyValue {
                    key: "target_os".into(),
                    value: "linux".into(),
                },
            },
            ConditionalModule {
                name: "win_impl".into(),
                predicate: CfgPredicate::KeyValue {
                    key: "target_os".into(),
                    value: "windows".into(),
                },
            },
        ];

        let active = filter_modules(&modules, &ctx);
        assert_eq!(active, vec!["linux_impl"]);
    }

    // S23.6 — Cfg in Tests
    #[test]
    fn s23_6_test_cfg() {
        let mut ctx = CfgContext::new();
        ctx.set_flag(CFG_TEST);
        assert!(ctx.evaluate(&CfgPredicate::Flag(CFG_TEST.into())));
        assert!(!ctx.evaluate(&CfgPredicate::Flag(CFG_BENCH.into())));
    }

    // S23.7 — Cfg Checking
    #[test]
    fn s23_7_known_key() {
        assert!(check_cfg_key("target_os").is_none());
        assert!(check_cfg_key("feature").is_none());
    }

    #[test]
    fn s23_7_typo_detection() {
        let warning = check_cfg_key("taget_os").unwrap();
        assert_eq!(warning.suggestion.as_deref(), Some("target_os"));
    }

    #[test]
    fn s23_7_unknown_key() {
        let warning = check_cfg_key("completely_unknown_zzzz").unwrap();
        assert!(warning.suggestion.is_none());
    }

    // S23.8-S23.9 — Default Features & Dependencies
    #[test]
    fn s23_8_feature_deps() {
        let mut fs = FeatureSet::new();
        fs.add_feature("std", vec!["alloc".into()], true);
        fs.add_feature("alloc", vec![], false);
        fs.add_feature("gpu", vec!["cuda".into(), "driver".into()], false);
        fs.add_feature("cuda", vec![], false);
        fs.add_feature("driver", vec![], false);

        let resolved = fs.resolve(&["gpu".into()]);
        assert!(resolved.contains("cuda"));
        assert!(resolved.contains("driver"));
        assert!(!resolved.contains("std"));
    }

    // S23.10 — Display / Integration
    #[test]
    fn s23_10_predicate_display() {
        let p = CfgPredicate::All(vec![
            CfgPredicate::Flag("test".into()),
            CfgPredicate::KeyValue {
                key: "target_os".into(),
                value: "linux".into(),
            },
        ]);
        let s = p.to_string();
        assert!(s.contains("all("));
        assert!(s.contains("test"));
        assert!(s.contains("target_os"));
    }

    #[test]
    fn s23_10_target_os_display() {
        assert_eq!(TargetOs::Linux.to_string(), "linux");
        assert_eq!(TargetOs::None.to_string(), "none");
        assert_eq!(TargetOs::Zephyr.to_string(), "zephyr");
    }

    #[test]
    fn s23_10_cfg_warning_display() {
        let w = CfgWarning {
            key: "taget_os".into(),
            suggestion: Some("target_os".into()),
        };
        assert!(w.to_string().contains("did you mean"));
    }
}
