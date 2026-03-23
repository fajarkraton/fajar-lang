//! User-mode runtime tests for Fajar Lang.
//!
//! Verifies that @safe programs can use syscall-based runtime functions
//! when compiled with x86_64-user target.
//! Sprint 6 of Master Implementation Plan v7.0.

use std::path::Path;

// ════════════════════════════════════════════════════════════════════════
// 1. Runtime file exists
// ════════════════════════════════════════════════════════════════════════

#[test]
fn user_runtime_module_exists() {
    assert!(Path::new("src/codegen/cranelift/runtime_user.rs").exists());
}

#[test]
fn user_runtime_has_print() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_print"));
    assert!(source.contains("fj_rt_user_print_i64"));
    assert!(source.contains("fj_rt_user_println"));
}

#[test]
fn user_runtime_has_exit() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_exit"));
}

#[test]
fn user_runtime_has_ipc() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_ipc_send"));
    assert!(source.contains("fj_rt_user_ipc_recv"));
    assert!(source.contains("fj_rt_user_ipc_call"));
    assert!(source.contains("fj_rt_user_ipc_reply"));
    assert!(source.contains("fj_rt_user_ipc_try_recv"));
    assert!(source.contains("fj_rt_user_ipc_notify"));
    assert!(source.contains("fj_rt_user_ipc_select"));
}

#[test]
fn user_runtime_has_mmap() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_mmap"));
}

#[test]
fn user_runtime_has_getpid() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_getpid"));
}

#[test]
fn user_runtime_has_yield() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_yield"));
}

#[test]
fn user_runtime_has_read() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("fj_rt_user_read"));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Syscall constants
// ════════════════════════════════════════════════════════════════════════

#[test]
fn syscall_constants_defined() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("SYS_EXIT"));
    assert!(source.contains("SYS_WRITE"));
    assert!(source.contains("SYS_READ"));
    assert!(source.contains("SYS_GETPID"));
    assert!(source.contains("SYS_YIELD"));
    assert!(source.contains("SYS_IPC_SEND"));
    assert!(source.contains("SYS_IPC_RECV"));
    assert!(source.contains("SYS_IPC_CALL"));
    assert!(source.contains("SYS_IPC_REPLY"));
    assert!(source.contains("SYS_MMAP"));
}

#[test]
fn syscall_numbering_consistent() {
    let source = std::fs::read_to_string("src/codegen/cranelift/runtime_user.rs").unwrap();
    assert!(source.contains("SYS_EXIT: i64 = 0"));
    assert!(source.contains("SYS_WRITE: i64 = 1"));
    assert!(source.contains("SYS_READ: i64 = 2"));
    assert!(source.contains("SYS_IPC_SEND: i64 = 10"));
}

// ════════════════════════════════════════════════════════════════════════
// 3. Target configuration
// ════════════════════════════════════════════════════════════════════════

#[cfg(feature = "native")]
#[test]
fn x86_64_user_target_parses() {
    let target = fajar_lang::codegen::target::TargetConfig::from_triple("x86_64-user").unwrap();
    assert!(target.is_user_mode);
    assert!(!target.is_bare_metal);
}

#[cfg(feature = "native")]
#[test]
fn x86_64_none_is_not_user() {
    let target = fajar_lang::codegen::target::TargetConfig::from_triple("x86_64-none").unwrap();
    assert!(!target.is_user_mode);
    assert!(target.is_bare_metal);
}

#[cfg(feature = "native")]
#[test]
fn user_mode_linker_config() {
    use fajar_lang::codegen::linker::LinkerConfig;
    use fajar_lang::codegen::target::Arch;
    let config = LinkerConfig::for_user_mode(Arch::X86_64);
    assert!(config.is_user_mode());
    assert_eq!(config.entry, "main");
    assert_eq!(config.regions[0].origin, 0x400000);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Auto-link wiring in main.rs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn main_rs_has_user_mode_handling() {
    let source = std::fs::read_to_string("src/main.rs").unwrap();
    assert!(source.contains("target.is_user_mode"));
    assert!(source.contains("set_user_mode(true)"));
    assert!(source.contains("set_no_std(true)"));
}

#[test]
fn main_rs_generates_user_linker_script() {
    let source = std::fs::read_to_string("src/main.rs").unwrap();
    // User-mode has a dedicated linker script section
    assert!(source.contains("User-mode"));
    assert!(source.contains("ENTRY(_start)"));
}

#[test]
fn main_rs_generates_user_startup() {
    let source = std::fs::read_to_string("src/main.rs").unwrap();
    // User-mode startup stubs generated
    assert!(source.contains("syscall-based runtime"));
}

// ════════════════════════════════════════════════════════════════════════
// 5. @safe programs can compile (interpreter mode)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_program_println() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source("@safe fn main() { println(42) }")
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn safe_program_arithmetic() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source("@safe fn main() { println(2 + 3 * 4) }")
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("14")));
}

#[test]
fn safe_program_string() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source(r#"@safe fn main() { println("hello from safe") }"#)
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("hello from safe")));
}

#[test]
fn safe_program_function_call() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source(
            "@safe fn double(x: i64) -> i64 { x * 2 }\n@safe fn main() { println(double(21)) }",
        )
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn safe_program_if_else() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source("@safe fn main() {\n    let x = if true { 1 } else { 0 }\n    println(x)\n}")
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("1")));
}

#[test]
fn safe_program_loop() {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    interp
        .eval_source("@safe fn main() {\n    let mut sum: i64 = 0\n    let mut i: i64 = 0\n    while i < 10 { sum = sum + i\n i = i + 1 }\n    println(sum)\n}")
        .unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();
    assert!(output.iter().any(|l| l.contains("45")));
}

// ════════════════════════════════════════════════════════════════════════
// 6. @safe CANNOT access hardware (confirm Sprint 1)
// ════════════════════════════════════════════════════════════════════════

#[test]
fn safe_cannot_port_outb() {
    let tokens = fajar_lang::lexer::tokenize("@safe fn f() { port_outb(0x3F8, 65) }").unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(result.is_err());
}

#[test]
fn safe_cannot_call_kernel_fn() {
    let tokens =
        fajar_lang::lexer::tokenize("@kernel fn hw() -> i64 { 0 }\n@safe fn bad() { hw() }")
            .unwrap();
    let program = fajar_lang::parser::parse(tokens).unwrap();
    let result = fajar_lang::analyzer::analyze(&program);
    assert!(result.is_err());
}

// ════════════════════════════════════════════════════════════════════════
// Gap C: End-to-end user ELF structure tests
// ════════════════════════════════════════════════════════════════════════

mod user_elf_structure {
    use std::path::PathBuf;
    use std::process::Command;

    fn fj_binary() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_fj"))
    }

    /// Returns true if the fj binary lacks native codegen (skip test).
    fn skip_if_no_native() -> bool {
        let path = fj_binary();
        let dir = std::env::temp_dir().join("fj-native-check-user");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("t.fj"), "fn main() -> i64 { 0 }\n").unwrap();
        let output = Command::new(&path)
            .args(["build", "--target", "x86_64-unknown-none", dir.join("t.fj").to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        stderr.contains("native compilation not available")
    }

    fn build_user_elf(source: &str, name: &str) -> (PathBuf, Vec<u8>) {
        let dir = std::env::temp_dir().join(format!("fj-user-elf-{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let src_path = dir.join("prog.fj");
        std::fs::write(&src_path, source).unwrap();

        let elf_path = dir.join("prog.elf");
        let output = Command::new(fj_binary())
            .args([
                "build",
                "--target",
                "x86_64-user",
                src_path.to_str().unwrap(),
                "-o",
                elf_path.to_str().unwrap(),
            ])
            .output()
            .expect("fj binary not found");

        assert!(
            elf_path.exists(),
            "ELF not produced. stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let data = std::fs::read(&elf_path).unwrap();
        (dir, data)
    }

    #[test]
    fn user_elf_is_valid_elf64() {
        if skip_if_no_native() { return; }
        let (dir, data) = build_user_elf("fn main() -> i64 { 42 }\n", "valid-elf64");
        assert_eq!(&data[0..4], b"\x7fELF", "not ELF");
        assert_eq!(data[4], 2, "not 64-bit");      // EI_CLASS = ELFCLASS64
        assert_eq!(data[5], 1, "not little-endian"); // EI_DATA = ELFDATA2LSB
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_elf_has_start_entry() {
        if skip_if_no_native() { return; }
        let (dir, data) = build_user_elf("fn main() -> i64 { 0 }\n", "start-entry");
        // Entry point should be in the 0x400000 range
        let entry = u64::from_le_bytes(data[24..32].try_into().unwrap());
        assert!(
            entry >= 0x400000 && entry < 0x500000,
            "entry point {:#x} not in 0x400000..0x500000 range",
            entry
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_elf_text_at_0x400000() {
        if skip_if_no_native() { return; }
        let (dir, data) = build_user_elf("fn main() -> i64 { 1 }\n", "text-addr");
        // Find LOAD segment: program header offset at bytes 32-39
        let phoff = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes(data[54..56].try_into().unwrap()) as usize;
        // First LOAD segment virtual address at phoff + 16
        let vaddr = u64::from_le_bytes(data[phoff + 16..phoff + 24].try_into().unwrap());
        assert_eq!(vaddr, 0x400000, ".text segment not at 0x400000");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_elf_is_x86_64() {
        if skip_if_no_native() { return; }
        let (dir, data) = build_user_elf("fn main() -> i64 { 99 }\n", "x86-64");
        assert_eq!(data[18], 0x3E, "e_machine not EM_X86_64");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
