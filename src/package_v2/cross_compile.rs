//! Cross-Compilation Matrix — target triples, sysroot management,
//! cross-linker detection, build matrix, target features, QEMU
//! testing, Docker cross-build, release matrix, platform tiers.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// S24.1: Target Triple
// ═══════════════════════════════════════════════════════════════════════

/// A parsed target triple: `<arch>-<vendor>-<os>-<abi>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetTriple {
    /// Architecture (e.g., "aarch64", "x86_64").
    pub arch: String,
    /// Vendor (e.g., "unknown", "apple").
    pub vendor: String,
    /// Operating system (e.g., "linux", "none").
    pub os: String,
    /// ABI (e.g., "gnu", "musl", "eabi").
    pub abi: Option<String>,
}

impl TargetTriple {
    /// Parses a target triple string.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            arch: parts[0].to_string(),
            vendor: parts[1].to_string(),
            os: parts[2].to_string(),
            abi: parts.get(3).map(|s| s.to_string()),
        })
    }

    /// Whether this is a bare-metal target (no OS).
    pub fn is_bare_metal(&self) -> bool {
        self.os == "none"
            || self.os == "unknown"
            || self.vendor == "none"
            || self.os == "eabi"
            || self.os == "eabihf"
    }

    /// Whether this targets Linux.
    pub fn is_linux(&self) -> bool {
        self.os == "linux"
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}", self.arch, self.vendor, self.os)?;
        if let Some(ref abi) = self.abi {
            write!(f, "-{abi}")?;
        }
        Ok(())
    }
}

/// Well-known target triples.
pub const AARCH64_LINUX_GNU: &str = "aarch64-unknown-linux-gnu";
/// x86_64 Linux GNU.
pub const X86_64_LINUX_GNU: &str = "x86_64-unknown-linux-gnu";
/// RISC-V 64 Linux GNU.
pub const RISCV64_LINUX_GNU: &str = "riscv64gc-unknown-linux-gnu";
/// ARM bare-metal.
pub const THUMBV7EM_NONE_EABI: &str = "thumbv7em-none-eabi";
/// WebAssembly.
pub const WASM32_UNKNOWN_UNKNOWN: &str = "wasm32-unknown-unknown";

// ═══════════════════════════════════════════════════════════════════════
// S24.2: Sysroot Management
// ═══════════════════════════════════════════════════════════════════════

/// Sysroot configuration for a target.
#[derive(Debug, Clone)]
pub struct Sysroot {
    /// Target triple.
    pub target: TargetTriple,
    /// Sysroot path.
    pub path: String,
    /// Whether downloaded and cached.
    pub cached: bool,
}

/// Computes the sysroot cache path for a target.
pub fn sysroot_cache_path(cache_dir: &str, target: &TargetTriple) -> String {
    format!("{cache_dir}/sysroots/{target}")
}

// ═══════════════════════════════════════════════════════════════════════
// S24.3: Cross Linker
// ═══════════════════════════════════════════════════════════════════════

/// Cross-linker configuration.
#[derive(Debug, Clone)]
pub struct CrossLinker {
    /// Linker binary name or path.
    pub path: String,
    /// Target triple this linker supports.
    pub target: String,
    /// Additional linker flags.
    pub flags: Vec<String>,
}

/// Detects the appropriate cross-linker for a target.
pub fn detect_cross_linker(target: &TargetTriple) -> Option<CrossLinker> {
    let linker = match target.arch.as_str() {
        "aarch64" => format!("{}-{}-gcc", target.arch, target.os),
        "riscv64gc" | "riscv64" => format!("riscv64-{}-gcc", target.os),
        "arm" | "thumbv7em" => "arm-none-eabi-gcc".to_string(),
        "wasm32" => "wasm-ld".to_string(),
        _ => return None,
    };

    Some(CrossLinker {
        path: linker,
        target: target.to_string(),
        flags: Vec::new(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S24.4: Build Matrix
// ═══════════════════════════════════════════════════════════════════════

/// A build matrix entry.
#[derive(Debug, Clone)]
pub struct BuildMatrixEntry {
    /// Target triple.
    pub target: TargetTriple,
    /// Build profile (debug/release).
    pub profile: BuildProfile,
    /// Features to enable.
    pub features: Vec<String>,
    /// Build status.
    pub status: BuildStatus,
}

/// Build profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    /// Debug (unoptimized).
    Debug,
    /// Release (optimized).
    Release,
}

impl fmt::Display for BuildProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildProfile::Debug => write!(f, "debug"),
            BuildProfile::Release => write!(f, "release"),
        }
    }
}

/// Build status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStatus {
    /// Not started.
    Pending,
    /// Building.
    Building,
    /// Success.
    Success,
    /// Failed.
    Failed,
    /// Skipped.
    Skipped,
}

/// Parses a comma-separated target list into target triples.
pub fn parse_target_list(targets: &str) -> Vec<TargetTriple> {
    targets
        .split(',')
        .filter_map(|t| TargetTriple::parse(t.trim()))
        .collect()
}

/// Generates a build matrix from targets and profiles.
pub fn generate_matrix(
    targets: &[TargetTriple],
    profiles: &[BuildProfile],
) -> Vec<BuildMatrixEntry> {
    let mut entries = Vec::new();
    for target in targets {
        for &profile in profiles {
            entries.push(BuildMatrixEntry {
                target: target.clone(),
                profile,
                features: Vec::new(),
                status: BuildStatus::Pending,
            });
        }
    }
    entries
}

// ═══════════════════════════════════════════════════════════════════════
// S24.5: Target Feature Detection
// ═══════════════════════════════════════════════════════════════════════

/// CPU feature for a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFeature {
    /// Feature name (e.g., "neon", "sse2", "avx2").
    pub name: String,
    /// Whether enabled by default for this target.
    pub enabled_by_default: bool,
}

/// Returns known CPU features for an architecture.
pub fn features_for_arch(arch: &str) -> Vec<TargetFeature> {
    match arch {
        "x86_64" => vec![
            TargetFeature {
                name: "sse2".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "sse4.1".into(),
                enabled_by_default: false,
            },
            TargetFeature {
                name: "avx".into(),
                enabled_by_default: false,
            },
            TargetFeature {
                name: "avx2".into(),
                enabled_by_default: false,
            },
            TargetFeature {
                name: "avx512f".into(),
                enabled_by_default: false,
            },
        ],
        "aarch64" => vec![
            TargetFeature {
                name: "neon".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "sve".into(),
                enabled_by_default: false,
            },
            TargetFeature {
                name: "sve2".into(),
                enabled_by_default: false,
            },
        ],
        "riscv64gc" | "riscv64" => vec![
            TargetFeature {
                name: "m".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "a".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "f".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "d".into(),
                enabled_by_default: true,
            },
            TargetFeature {
                name: "v".into(),
                enabled_by_default: false,
            },
        ],
        _ => vec![],
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.6: QEMU Testing
// ═══════════════════════════════════════════════════════════════════════

/// QEMU runner configuration.
#[derive(Debug, Clone)]
pub struct QemuRunner {
    /// QEMU binary (e.g., "qemu-aarch64").
    pub binary: String,
    /// Additional QEMU flags.
    pub flags: Vec<String>,
    /// Sysroot for QEMU (for dynamic linking).
    pub sysroot: Option<String>,
}

/// Detects the QEMU runner for a target.
pub fn qemu_runner_for(target: &TargetTriple) -> Option<QemuRunner> {
    let binary = match target.arch.as_str() {
        "aarch64" => "qemu-aarch64",
        "riscv64gc" | "riscv64" => "qemu-riscv64",
        "arm" | "thumbv7em" => "qemu-arm",
        _ => return None,
    };

    Some(QemuRunner {
        binary: binary.to_string(),
        flags: vec!["-L".to_string()],
        sysroot: None,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// S24.7: Docker Cross-Build
// ═══════════════════════════════════════════════════════════════════════

/// Generates a Dockerfile for cross-compilation.
pub fn generate_dockerfile(target: &TargetTriple) -> String {
    let toolchain = match target.arch.as_str() {
        "aarch64" => "gcc-aarch64-linux-gnu",
        "riscv64gc" | "riscv64" => "gcc-riscv64-linux-gnu",
        "arm" => "gcc-arm-none-eabi",
        _ => "gcc",
    };

    format!(
        "FROM ubuntu:22.04\n\
         RUN apt-get update && apt-get install -y {toolchain} build-essential\n\
         WORKDIR /build\n\
         COPY . .\n\
         RUN fj build --target={target}\n"
    )
}

// ═══════════════════════════════════════════════════════════════════════
// S24.8: Release Matrix
// ═══════════════════════════════════════════════════════════════════════

/// A release artifact.
#[derive(Debug, Clone)]
pub struct ReleaseArtifact {
    /// Target triple.
    pub target: TargetTriple,
    /// Binary name.
    pub binary_name: String,
    /// Archive format.
    pub archive: ArchiveFormat,
    /// File size estimate (bytes).
    pub size_estimate: usize,
}

/// Archive format for release binaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// .tar.gz (Linux, macOS).
    TarGz,
    /// .zip (Windows).
    Zip,
    /// .deb (Debian/Ubuntu).
    Deb,
    /// .rpm (Fedora/RHEL).
    Rpm,
}

impl fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveFormat::TarGz => write!(f, ".tar.gz"),
            ArchiveFormat::Zip => write!(f, ".zip"),
            ArchiveFormat::Deb => write!(f, ".deb"),
            ArchiveFormat::Rpm => write!(f, ".rpm"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// S24.9: Platform Support Tiers
// ═══════════════════════════════════════════════════════════════════════

/// Platform support tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportTier {
    /// Tier 1: CI-tested, guaranteed to build and pass tests.
    Tier1,
    /// Tier 2: Cross-compiles, not CI-tested.
    Tier2,
    /// Tier 3: Best-effort, may not compile.
    Tier3,
}

impl fmt::Display for SupportTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupportTier::Tier1 => write!(f, "Tier 1 (CI-tested)"),
            SupportTier::Tier2 => write!(f, "Tier 2 (cross-compiles)"),
            SupportTier::Tier3 => write!(f, "Tier 3 (best-effort)"),
        }
    }
}

/// Returns the support tier for a target triple.
pub fn support_tier(target: &TargetTriple) -> SupportTier {
    match (target.arch.as_str(), target.os.as_str()) {
        ("x86_64", "linux") => SupportTier::Tier1,
        ("aarch64", "linux") => SupportTier::Tier1,
        ("x86_64", "macos" | "darwin") => SupportTier::Tier1,
        ("aarch64", "macos" | "darwin") => SupportTier::Tier1,
        ("riscv64gc" | "riscv64", "linux") => SupportTier::Tier2,
        ("wasm32", _) => SupportTier::Tier2,
        ("thumbv7em" | "arm", "none") => SupportTier::Tier2,
        _ => SupportTier::Tier3,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S24.1 — Target Triple
    #[test]
    fn s24_1_parse_triple() {
        let t = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        assert_eq!(t.arch, "aarch64");
        assert_eq!(t.vendor, "unknown");
        assert_eq!(t.os, "linux");
        assert_eq!(t.abi.as_deref(), Some("gnu"));
    }

    #[test]
    fn s24_1_parse_bare_metal() {
        let t = TargetTriple::parse("thumbv7em-none-eabi").unwrap();
        assert!(t.is_bare_metal());
    }

    #[test]
    fn s24_1_display() {
        let t = TargetTriple::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(t.to_string(), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn s24_1_invalid_triple() {
        assert!(TargetTriple::parse("invalid").is_none());
        assert!(TargetTriple::parse("a-b").is_none());
    }

    // S24.2 — Sysroot
    #[test]
    fn s24_2_sysroot_path() {
        let t = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        let path = sysroot_cache_path("/home/.fj", &t);
        assert!(path.contains("aarch64"));
    }

    // S24.3 — Cross Linker
    #[test]
    fn s24_3_detect_aarch64_linker() {
        let t = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        let linker = detect_cross_linker(&t).unwrap();
        assert!(linker.path.contains("aarch64"));
    }

    #[test]
    fn s24_3_detect_wasm_linker() {
        let t = TargetTriple::parse("wasm32-unknown-unknown").unwrap();
        let linker = detect_cross_linker(&t).unwrap();
        assert_eq!(linker.path, "wasm-ld");
    }

    // S24.4 — Build Matrix
    #[test]
    fn s24_4_generate_matrix() {
        let targets = vec![
            TargetTriple::parse("x86_64-unknown-linux-gnu").unwrap(),
            TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap(),
        ];
        let matrix = generate_matrix(&targets, &[BuildProfile::Debug, BuildProfile::Release]);
        assert_eq!(matrix.len(), 4); // 2 targets × 2 profiles
        assert!(matrix.iter().all(|e| e.status == BuildStatus::Pending));
    }

    #[test]
    fn s24_4_parse_target_list() {
        let targets = parse_target_list(
            "aarch64-unknown-linux-gnu, x86_64-unknown-linux-gnu, wasm32-unknown-unknown",
        );
        assert_eq!(targets.len(), 3);
    }

    // S24.5 — Target Features
    #[test]
    fn s24_5_x86_features() {
        let feats = features_for_arch("x86_64");
        assert!(feats
            .iter()
            .any(|f| f.name == "sse2" && f.enabled_by_default));
        assert!(feats
            .iter()
            .any(|f| f.name == "avx2" && !f.enabled_by_default));
    }

    #[test]
    fn s24_5_aarch64_features() {
        let feats = features_for_arch("aarch64");
        assert!(feats
            .iter()
            .any(|f| f.name == "neon" && f.enabled_by_default));
    }

    // S24.6 — QEMU Testing
    #[test]
    fn s24_6_qemu_runner() {
        let t = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        let runner = qemu_runner_for(&t).unwrap();
        assert_eq!(runner.binary, "qemu-aarch64");
    }

    #[test]
    fn s24_6_qemu_no_runner() {
        let t = TargetTriple::parse("x86_64-unknown-linux-gnu").unwrap();
        assert!(qemu_runner_for(&t).is_none());
    }

    // S24.7 — Docker Cross-Build
    #[test]
    fn s24_7_dockerfile() {
        let t = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        let df = generate_dockerfile(&t);
        assert!(df.contains("gcc-aarch64-linux-gnu"));
        assert!(df.contains("fj build"));
    }

    // S24.8 — Release Matrix
    #[test]
    fn s24_8_archive_format_display() {
        assert_eq!(ArchiveFormat::TarGz.to_string(), ".tar.gz");
        assert_eq!(ArchiveFormat::Zip.to_string(), ".zip");
        assert_eq!(ArchiveFormat::Deb.to_string(), ".deb");
    }

    // S24.9 — Platform Tiers
    #[test]
    fn s24_9_tier1_targets() {
        let t = TargetTriple::parse("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(support_tier(&t), SupportTier::Tier1);

        let t2 = TargetTriple::parse("aarch64-unknown-linux-gnu").unwrap();
        assert_eq!(support_tier(&t2), SupportTier::Tier1);
    }

    #[test]
    fn s24_9_tier2_targets() {
        let t = TargetTriple::parse("wasm32-unknown-unknown").unwrap();
        assert_eq!(support_tier(&t), SupportTier::Tier2);
    }

    #[test]
    fn s24_9_tier3_targets() {
        let t = TargetTriple::parse("mips-unknown-linux-gnu").unwrap();
        assert_eq!(support_tier(&t), SupportTier::Tier3);
    }

    #[test]
    fn s24_9_tier_display() {
        assert!(SupportTier::Tier1.to_string().contains("CI-tested"));
        assert!(SupportTier::Tier2.to_string().contains("cross-compiles"));
    }

    // S24.10 — Build Profile
    #[test]
    fn s24_10_build_profile_display() {
        assert_eq!(BuildProfile::Debug.to_string(), "debug");
        assert_eq!(BuildProfile::Release.to_string(), "release");
    }
}
