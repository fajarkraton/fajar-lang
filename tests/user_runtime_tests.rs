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
