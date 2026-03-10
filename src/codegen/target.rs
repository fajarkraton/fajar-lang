//! Target triple parsing and Cranelift ISA configuration.
//!
//! Parses target triples like `aarch64-unknown-none` and configures
//! the corresponding Cranelift ISA for code generation.

use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable, Flags};
use target_lexicon::Triple;

use super::CodegenError;

/// Supported target architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// x86-64 (AMD64).
    X86_64,
    /// ARM 64-bit (AArch64).
    Aarch64,
    /// RISC-V 64-bit with GC extensions.
    Riscv64,
}

/// Supported operating systems / environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// Linux with GNU libc.
    Linux,
    /// macOS (Darwin).
    MacOs,
    /// No OS (bare metal).
    None,
}

/// Parsed and validated target configuration.
#[derive(Debug, Clone)]
pub struct TargetConfig {
    /// The target triple as parsed by target-lexicon.
    pub triple: Triple,
    /// Architecture.
    pub arch: Arch,
    /// Operating system.
    pub os: Os,
    /// Whether this is a bare-metal target (no OS).
    pub is_bare_metal: bool,
    /// Calling convention for this target.
    pub call_conv: CallConv,
}

impl TargetConfig {
    /// Parses a target triple string into a validated TargetConfig.
    ///
    /// Supported triples:
    /// - `x86_64-unknown-linux-gnu` (or just `x86_64-linux`)
    /// - `aarch64-unknown-linux-gnu`
    /// - `aarch64-unknown-none` (bare metal ARM64)
    /// - `riscv64gc-unknown-linux-gnu`
    /// - `riscv64gc-unknown-none-elf` (bare metal RISC-V)
    /// - Host triple via `host`
    pub fn from_triple(target: &str) -> Result<Self, CodegenError> {
        let triple = if target == "host" {
            Triple::host()
        } else {
            target.parse::<Triple>().map_err(|e| {
                CodegenError::AbiError(format!("invalid target triple '{target}': {e}"))
            })?
        };

        let arch = match triple.architecture {
            target_lexicon::Architecture::X86_64 => Arch::X86_64,
            target_lexicon::Architecture::Aarch64(_) => Arch::Aarch64,
            target_lexicon::Architecture::Riscv64(_) => Arch::Riscv64,
            other => {
                return Err(CodegenError::AbiError(format!(
                    "unsupported architecture: {other}"
                )))
            }
        };

        let os = match triple.operating_system {
            target_lexicon::OperatingSystem::Linux => Os::Linux,
            target_lexicon::OperatingSystem::Darwin(_) => Os::MacOs,
            target_lexicon::OperatingSystem::None_ | target_lexicon::OperatingSystem::Unknown => {
                Os::None
            }
            other => return Err(CodegenError::AbiError(format!("unsupported OS: {other}"))),
        };

        let is_bare_metal = os == Os::None;

        // Ensure bare-metal targets use ELF format (required by ObjectBuilder)
        let triple =
            if is_bare_metal && triple.binary_format == target_lexicon::BinaryFormat::Unknown {
                let mut fixed = triple;
                fixed.binary_format = target_lexicon::BinaryFormat::Elf;
                fixed
            } else {
                triple
            };

        let call_conv = match (arch, os) {
            (Arch::X86_64, Os::Linux) => CallConv::SystemV,
            (Arch::X86_64, Os::MacOs) => CallConv::SystemV,
            (Arch::Aarch64, Os::Linux) => CallConv::SystemV,
            (Arch::Aarch64, Os::MacOs) => CallConv::SystemV,
            (Arch::Aarch64, Os::None) => CallConv::SystemV,
            (Arch::Riscv64, _) => CallConv::SystemV,
            (Arch::X86_64, Os::None) => CallConv::SystemV,
        };

        Ok(Self {
            triple,
            arch,
            os,
            is_bare_metal,
            call_conv,
        })
    }

    /// Returns the Cranelift settings flags for this target.
    pub fn cranelift_flags(&self) -> Result<Flags, CodegenError> {
        let mut builder = settings::builder();
        builder
            .set("opt_level", "speed")
            .map_err(|e| CodegenError::Internal(format!("failed to set opt_level: {e}")))?;

        if self.is_bare_metal {
            builder
                .set("is_pic", "false")
                .map_err(|e| CodegenError::Internal(format!("failed to set is_pic: {e}")))?;
        }

        Ok(Flags::new(builder))
    }

    /// Creates a Cranelift ISA (Instruction Set Architecture) for this target.
    pub fn cranelift_isa(
        &self,
    ) -> Result<std::sync::Arc<dyn cranelift_codegen::isa::TargetIsa>, CodegenError> {
        let flags = self.cranelift_flags()?;
        cranelift_codegen::isa::lookup(self.triple.clone())
            .map_err(|e| {
                CodegenError::AbiError(format!(
                    "Cranelift does not support target '{}': {e}",
                    self.triple
                ))
            })?
            .finish(flags)
            .map_err(|e| {
                CodegenError::Internal(format!("failed to create ISA for '{}': {e}", self.triple))
            })
    }

    /// Returns a human-readable description of this target.
    pub fn description(&self) -> String {
        let arch = match self.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Riscv64 => "riscv64",
        };
        let os = match self.os {
            Os::Linux => "linux",
            Os::MacOs => "macos",
            Os::None => "none (bare metal)",
        };
        format!("{arch}-{os}")
    }
}

/// Returns the host target configuration.
pub fn host_target() -> Result<TargetConfig, CodegenError> {
    TargetConfig::from_triple("host")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_triple() {
        let target = TargetConfig::from_triple("host").unwrap();
        // Host should resolve to a real architecture
        assert!(
            target.arch == Arch::X86_64
                || target.arch == Arch::Aarch64
                || target.arch == Arch::Riscv64
        );
    }

    #[test]
    fn parse_x86_64_linux() {
        let target = TargetConfig::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(target.arch, Arch::X86_64);
        assert_eq!(target.os, Os::Linux);
        assert!(!target.is_bare_metal);
        assert_eq!(target.call_conv, CallConv::SystemV);
    }

    #[test]
    fn parse_aarch64_linux() {
        let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
        assert_eq!(target.arch, Arch::Aarch64);
        assert_eq!(target.os, Os::Linux);
        assert!(!target.is_bare_metal);
    }

    #[test]
    fn parse_aarch64_bare_metal() {
        let target = TargetConfig::from_triple("aarch64-unknown-none").unwrap();
        assert_eq!(target.arch, Arch::Aarch64);
        assert_eq!(target.os, Os::None);
        assert!(target.is_bare_metal);
    }

    #[test]
    fn parse_riscv64_linux() {
        let target = TargetConfig::from_triple("riscv64gc-unknown-linux-gnu").unwrap();
        assert_eq!(target.arch, Arch::Riscv64);
        assert_eq!(target.os, Os::Linux);
    }

    #[test]
    fn parse_riscv64_bare_metal() {
        let target = TargetConfig::from_triple("riscv64gc-unknown-none-elf").unwrap();
        assert_eq!(target.arch, Arch::Riscv64);
        assert_eq!(target.os, Os::None);
        assert!(target.is_bare_metal);
    }

    #[test]
    fn invalid_triple_returns_error() {
        let result = TargetConfig::from_triple("not-a-real-target");
        assert!(result.is_err());
    }

    #[test]
    fn cranelift_flags_host() {
        let target = host_target().unwrap();
        let flags = target.cranelift_flags().unwrap();
        assert_eq!(
            flags.opt_level(),
            cranelift_codegen::settings::OptLevel::Speed
        );
    }

    #[test]
    fn cranelift_isa_host() {
        let target = host_target().unwrap();
        let isa = target.cranelift_isa().unwrap();
        assert_eq!(isa.triple(), &target.triple);
    }

    #[test]
    fn description_format() {
        let target = TargetConfig::from_triple("aarch64-unknown-none").unwrap();
        assert_eq!(target.description(), "aarch64-none (bare metal)");
    }

    #[test]
    fn bare_metal_flags_no_pic() {
        let target = TargetConfig::from_triple("aarch64-unknown-none").unwrap();
        let flags = target.cranelift_flags().unwrap();
        assert!(!flags.is_pic());
    }
}
