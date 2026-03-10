//! Cross-compilation + QEMU execution integration test.
//!
//! Compiles Fajar programs to aarch64 and riscv64 object files,
//! links with a C runtime entry point, and runs on QEMU user-mode.
//!
//! Requires: qemu-aarch64, qemu-riscv64, aarch64-linux-gnu-gcc, riscv64-linux-gnu-gcc

#[cfg(feature = "native")]
mod cross_tests {
    use fajar_lang::codegen::cranelift::ObjectCompiler;
    use fajar_lang::codegen::target::TargetConfig;
    use fajar_lang::lexer::tokenize;
    use fajar_lang::parser::parse;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Helper: compile Fajar source to an object file for the given target triple.
    fn compile_to_object(src: &str, triple: &str, name: &str) -> Vec<u8> {
        let target = TargetConfig::from_triple(triple).unwrap();
        let tokens = tokenize(src).unwrap();
        let program = parse(tokens).unwrap();
        let mut compiler = ObjectCompiler::new_with_target(name, &target).unwrap();
        compiler.compile_program(&program).unwrap();
        let product = compiler.finish();
        product.emit().unwrap()
    }

    /// Helper: write object, link with C runtime, run on QEMU, return exit code.
    fn link_and_run(obj_bytes: &[u8], arch: &str) -> Result<i32, String> {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmp_dir =
            std::env::temp_dir().join(format!("fj_cross_{arch}_{}_{id}", std::process::id()));
        fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

        let obj_path = tmp_dir.join("program.o");
        let bin_path = tmp_dir.join("program");
        let rt_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cross/rt_entry.c");

        fs::write(&obj_path, obj_bytes).map_err(|e| e.to_string())?;

        let (cc, qemu) = match arch {
            "aarch64" => ("aarch64-linux-gnu-gcc", "qemu-aarch64"),
            "riscv64" => ("riscv64-linux-gnu-gcc", "qemu-riscv64"),
            _ => return Err(format!("unknown arch: {arch}")),
        };

        // Link: cc -static -o binary program.o rt_entry.c
        let link_output = Command::new(cc)
            .args(["-static", "-o"])
            .arg(&bin_path)
            .arg(&obj_path)
            .arg(&rt_path)
            .output()
            .map_err(|e| format!("link failed: {e}"))?;

        if !link_output.status.success() {
            let stderr = String::from_utf8_lossy(&link_output.stderr);
            // Clean up
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(format!("linker error: {stderr}"));
        }

        // Run on QEMU
        let run_output = Command::new(qemu)
            .arg(&bin_path)
            .output()
            .map_err(|e| format!("qemu failed: {e}"))?;

        let stdout = String::from_utf8_lossy(&run_output.stdout).to_string();
        let exit_code = run_output.status.code().unwrap_or(-1);

        // Clean up
        let _ = fs::remove_dir_all(&tmp_dir);

        if !stdout.is_empty() {
            eprintln!("QEMU stdout: {stdout}");
        }

        Ok(exit_code)
    }

    fn has_tool(name: &str) -> bool {
        Command::new("which")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // ── aarch64 QEMU tests ──────────────────────────────────────────

    #[test]
    fn qemu_aarch64_return_42() {
        if !has_tool("qemu-aarch64") || !has_tool("aarch64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-aarch64 or cross-compiler not found");
            return;
        }
        // fj_main returns 42, which becomes the exit code
        let src = "fn fj_main() -> i64 { 42 }";
        let obj = compile_to_object(src, "aarch64-unknown-linux-gnu", "aarch64_42");
        let exit_code = link_and_run(&obj, "aarch64").unwrap();
        assert_eq!(exit_code, 42);
    }

    #[test]
    fn qemu_aarch64_fibonacci() {
        if !has_tool("qemu-aarch64") || !has_tool("aarch64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-aarch64 or cross-compiler not found");
            return;
        }
        let src = r#"
            fn fib(n: i64) -> i64 {
                if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
            }
            fn fj_main() -> i64 { fib(10) }
        "#;
        let obj = compile_to_object(src, "aarch64-unknown-linux-gnu", "aarch64_fib");
        let exit_code = link_and_run(&obj, "aarch64").unwrap();
        assert_eq!(exit_code, 55); // fib(10) = 55
    }

    #[test]
    fn qemu_aarch64_loop_sum() {
        if !has_tool("qemu-aarch64") || !has_tool("aarch64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-aarch64 or cross-compiler not found");
            return;
        }
        let src = r#"
            fn fj_main() -> i64 {
                let mut sum = 0
                let mut i = 1
                while i <= 10 {
                    sum = sum + i
                    i = i + 1
                }
                sum
            }
        "#;
        let obj = compile_to_object(src, "aarch64-unknown-linux-gnu", "aarch64_loop");
        let exit_code = link_and_run(&obj, "aarch64").unwrap();
        assert_eq!(exit_code, 55); // 1+2+...+10 = 55
    }

    #[test]
    fn qemu_aarch64_bare_metal_style() {
        if !has_tool("qemu-aarch64") || !has_tool("aarch64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-aarch64 or cross-compiler not found");
            return;
        }
        // Bare-metal style: no stdlib, pure computation
        let src = r#"
            fn compute_checksum(n: i64) -> i64 {
                let mut sum = 0
                let mut i = 0
                while i < n {
                    sum = sum + i * 3
                    i = i + 1
                }
                sum
            }
            fn fj_main() -> i64 {
                let result = compute_checksum(10)
                if result == 135 { 0 } else { 1 }
            }
        "#;
        let obj = compile_to_object(src, "aarch64-unknown-linux-gnu", "aarch64_bare");
        // Verify small binary size (bare-metal friendly)
        assert!(
            obj.len() < 16384,
            "object file too large: {} bytes",
            obj.len()
        );
        let exit_code = link_and_run(&obj, "aarch64").unwrap();
        assert_eq!(exit_code, 0);
    }

    // ── riscv64 QEMU tests ──────────────────────────────────────────

    #[test]
    fn qemu_riscv64_return_42() {
        if !has_tool("qemu-riscv64") || !has_tool("riscv64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-riscv64 or cross-compiler not found");
            return;
        }
        let src = "fn fj_main() -> i64 { 42 }";
        let obj = compile_to_object(src, "riscv64gc-unknown-linux-gnu", "riscv64_42");
        let exit_code = link_and_run(&obj, "riscv64").unwrap();
        assert_eq!(exit_code, 42);
    }

    #[test]
    fn qemu_riscv64_fibonacci() {
        if !has_tool("qemu-riscv64") || !has_tool("riscv64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-riscv64 or cross-compiler not found");
            return;
        }
        let src = r#"
            fn fib(n: i64) -> i64 {
                if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
            }
            fn fj_main() -> i64 { fib(10) }
        "#;
        let obj = compile_to_object(src, "riscv64gc-unknown-linux-gnu", "riscv64_fib");
        let exit_code = link_and_run(&obj, "riscv64").unwrap();
        assert_eq!(exit_code, 55);
    }

    #[test]
    fn qemu_riscv64_loop_sum() {
        if !has_tool("qemu-riscv64") || !has_tool("riscv64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-riscv64 or cross-compiler not found");
            return;
        }
        let src = r#"
            fn fj_main() -> i64 {
                let mut sum = 0
                let mut i = 1
                while i <= 10 {
                    sum = sum + i
                    i = i + 1
                }
                sum
            }
        "#;
        let obj = compile_to_object(src, "riscv64gc-unknown-linux-gnu", "riscv64_loop");
        let exit_code = link_and_run(&obj, "riscv64").unwrap();
        assert_eq!(exit_code, 55);
    }

    #[test]
    fn qemu_riscv64_bare_metal_style() {
        if !has_tool("qemu-riscv64") || !has_tool("riscv64-linux-gnu-gcc") {
            eprintln!("SKIP: qemu-riscv64 or cross-compiler not found");
            return;
        }
        let src = r#"
            fn compute_checksum(n: i64) -> i64 {
                let mut sum = 0
                let mut i = 0
                while i < n {
                    sum = sum + i * 3
                    i = i + 1
                }
                sum
            }
            fn fj_main() -> i64 {
                let result = compute_checksum(10)
                if result == 135 { 0 } else { 1 }
            }
        "#;
        let obj = compile_to_object(src, "riscv64gc-unknown-linux-gnu", "riscv64_bare");
        assert!(
            obj.len() < 16384,
            "object file too large: {} bytes",
            obj.len()
        );
        let exit_code = link_and_run(&obj, "riscv64").unwrap();
        assert_eq!(exit_code, 0);
    }
}
