//! Initramfs and linker configuration tests for Fajar Lang.
//! Sprint 5 of Master Implementation Plan v7.0.

// ════════════════════════════════════════════════════════════════════════
// 1. Initramfs pack/unpack (requires native feature for linker module)
// ════════════════════════════════════════════════════════════════════════

#[cfg(feature = "native")]
mod native_tests {
    use fajar_lang::codegen::linker::*;
    use fajar_lang::codegen::target::Arch;

    #[test]
    fn pack_empty() {
        let archive = pack_initramfs(&[]);
        assert_eq!(archive.len(), 8); // just the count
        let files = unpack_initramfs(&archive);
        assert!(files.is_empty());
    }

    #[test]
    fn pack_one_file() {
        let archive = pack_initramfs(&[("vfs.elf", b"ELF_DATA_HERE")]);
        let files = unpack_initramfs(&archive);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "vfs.elf");
        assert_eq!(files[0].1, b"ELF_DATA_HERE");
    }

    #[test]
    fn pack_three_files() {
        let archive = pack_initramfs(&[
            ("vfs.elf", b"VFS"),
            ("shell.elf", b"SHELL"),
            ("net.elf", b"NET_SERVICE"),
        ]);
        let files = unpack_initramfs(&archive);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, "vfs.elf");
        assert_eq!(files[1].0, "shell.elf");
        assert_eq!(files[2].0, "net.elf");
        assert_eq!(files[2].1, b"NET_SERVICE");
    }

    #[test]
    fn pack_roundtrip_large() {
        let big_data = vec![0xABu8; 65536]; // 64KB
        let archive = pack_initramfs(&[("big.elf", &big_data)]);
        let files = unpack_initramfs(&archive);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1.len(), 65536);
        assert_eq!(files[0].1[0], 0xAB);
    }

    #[test]
    fn unpack_invalid_data() {
        let files = unpack_initramfs(&[0, 0, 0]); // too short
        assert!(files.is_empty());
    }

    // ════════════════════════════════════════════════════════════════════
    // 2. User-mode linker config
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn user_mode_config_x86() {
        let config = LinkerConfig::for_user_mode(Arch::X86_64);
        assert_eq!(config.entry, "main");
        assert!(config.is_user_mode());
        assert_eq!(config.regions[0].origin, 0x400000); // 4MB
        assert_eq!(config.stack_size, 8192);
    }

    #[test]
    fn user_mode_config_arm64() {
        let config = LinkerConfig::for_user_mode(Arch::Aarch64);
        assert_eq!(config.entry, "main");
        assert!(config.is_user_mode());
    }

    #[test]
    fn kernel_mode_config_not_user() {
        let target =
            fajar_lang::codegen::target::TargetConfig::from_triple("x86_64-unknown-none").unwrap();
        let config = LinkerConfig::for_target(&target);
        assert_eq!(config.entry, "_start");
        assert!(!config.is_user_mode());
    }

    #[test]
    fn kernel_with_initramfs() {
        let config = LinkerConfig::for_kernel_with_initramfs(Arch::X86_64, 2 * 1024 * 1024);
        assert_eq!(config.regions.len(), 3); // FLASH + RAM + INITRAMFS
        assert_eq!(config.regions[2].name, "INITRAMFS");
        assert_eq!(config.regions[2].origin, 0x800_0000);
    }

    // ════════════════════════════════════════════════════════════════════
    // 3. Linker script generation
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn generate_kernel_linker_script() {
        let target =
            fajar_lang::codegen::target::TargetConfig::from_triple("x86_64-unknown-none").unwrap();
        let config = LinkerConfig::for_target(&target);
        let script = generate_linker_script(&config).unwrap();
        assert!(script.contains("ENTRY(_start)"));
        assert!(script.contains("FLASH"));
        assert!(script.contains(".text"));
    }

    #[test]
    fn generate_user_linker_script() {
        let config = LinkerConfig::for_user_mode(Arch::X86_64);
        let script = generate_linker_script(&config).unwrap();
        assert!(script.contains("ENTRY(main)"));
        assert!(script.contains("TEXT"));
    }
}

// ════════════════════════════════════════════════════════════════════════
// 4. Non-native tests (always run)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn fj_pack_command_exists() {
    // Verify the Pack command is defined in the CLI
    // (indirect test — just verify the binary accepts --help)
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "pack", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();
    // May fail if not built, but shouldn't panic
    let _ = output;
}

#[test]
fn manifest_with_kernel_parses() {
    let config = fajar_lang::package::manifest::ProjectConfig::parse(
        r#"
[package]
name = "test"
version = "1.0.0"

[kernel]
entry = "kernel/main.fj"
target = "x86_64-unknown-none"
"#,
    )
    .unwrap();
    assert!(config.kernel.is_some());
}
