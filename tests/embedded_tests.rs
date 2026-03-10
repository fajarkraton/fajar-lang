//! Embedded test framework integration tests.
//!
//! Tests the embedded test framework itself and runs sample programs
//! on QEMU targets to verify the full pipeline works.

mod embedded;

#[cfg(feature = "native")]
mod framework_tests {
    use super::embedded::{has_tools, link_and_run, EmbeddedTestConfig, TestArch};
    use fajar_lang::codegen::cranelift::ObjectCompiler;
    use fajar_lang::codegen::target::TargetConfig;
    use fajar_lang::lexer::tokenize;
    use fajar_lang::parser::parse;
    use std::time::Duration;

    fn compile(src: &str, triple: &str, name: &str) -> Vec<u8> {
        let target = TargetConfig::from_triple(triple).unwrap();
        let tokens = tokenize(src).unwrap();
        let program = parse(tokens).unwrap();
        let mut compiler = ObjectCompiler::new_with_target(name, &target).unwrap();
        compiler.compile_program(&program).unwrap();
        compiler.finish().emit().unwrap()
    }

    // ── Framework tests ─────────────────────────────────────────────

    #[test]
    fn test_result_passed() {
        let result = super::embedded::EmbeddedTestResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        assert!(result.passed());
    }

    #[test]
    fn test_result_failed_exit_code() {
        let result = super::embedded::EmbeddedTestResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(10),
            timed_out: false,
        };
        assert!(!result.passed());
    }

    #[test]
    fn test_result_failed_timeout() {
        let result = super::embedded::EmbeddedTestResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_secs(10),
            timed_out: true,
        };
        assert!(!result.passed());
    }

    #[test]
    fn test_arch_properties() {
        assert_eq!(TestArch::Aarch64.name(), "aarch64");
        assert_eq!(TestArch::Riscv64.name(), "riscv64");
        assert_eq!(TestArch::Aarch64.triple(), "aarch64-unknown-linux-gnu");
        assert_eq!(TestArch::Riscv64.triple(), "riscv64gc-unknown-linux-gnu");
    }

    // ── Aarch64 QEMU tests via framework ────────────────────────────

    #[test]
    fn framework_aarch64_simple() {
        if !has_tools(TestArch::Aarch64) {
            eprintln!("SKIP: aarch64 tools not available");
            return;
        }
        let src = "fn fj_main() -> i64 { 0 }";
        let obj = compile(src, TestArch::Aarch64.triple(), "fw_a64_simple");
        let config = EmbeddedTestConfig::new(TestArch::Aarch64);
        let result = link_and_run(&obj, &config).unwrap();
        assert!(result.passed());
        assert!(result.duration < Duration::from_secs(5));
    }

    #[test]
    fn framework_aarch64_computation() {
        if !has_tools(TestArch::Aarch64) {
            return;
        }
        let src = r#"
            fn gcd(a: i64, b: i64) -> i64 {
                if b == 0 { a } else { gcd(b, a - b * (a / b)) }
            }
            fn fj_main() -> i64 {
                let result = gcd(48, 18)
                if result == 6 { 0 } else { 1 }
            }
        "#;
        let obj = compile(src, TestArch::Aarch64.triple(), "fw_a64_gcd");
        let config = EmbeddedTestConfig::new(TestArch::Aarch64);
        let result = link_and_run(&obj, &config).unwrap();
        assert!(result.passed());
    }

    // ── Riscv64 QEMU tests via framework ────────────────────────────

    #[test]
    fn framework_riscv64_simple() {
        if !has_tools(TestArch::Riscv64) {
            eprintln!("SKIP: riscv64 tools not available");
            return;
        }
        let src = "fn fj_main() -> i64 { 0 }";
        let obj = compile(src, TestArch::Riscv64.triple(), "fw_rv64_simple");
        let config = EmbeddedTestConfig::new(TestArch::Riscv64);
        let result = link_and_run(&obj, &config).unwrap();
        assert!(result.passed());
    }

    #[test]
    fn framework_riscv64_computation() {
        if !has_tools(TestArch::Riscv64) {
            return;
        }
        let src = r#"
            fn gcd(a: i64, b: i64) -> i64 {
                if b == 0 { a } else { gcd(b, a - b * (a / b)) }
            }
            fn fj_main() -> i64 {
                let result = gcd(48, 18)
                if result == 6 { 0 } else { 1 }
            }
        "#;
        let obj = compile(src, TestArch::Riscv64.triple(), "fw_rv64_gcd");
        let config = EmbeddedTestConfig::new(TestArch::Riscv64);
        let result = link_and_run(&obj, &config).unwrap();
        assert!(result.passed());
    }

    #[test]
    fn framework_custom_timeout() {
        if !has_tools(TestArch::Aarch64) {
            return;
        }
        let src = "fn fj_main() -> i64 { 0 }";
        let obj = compile(src, TestArch::Aarch64.triple(), "fw_timeout");
        let config =
            EmbeddedTestConfig::new(TestArch::Aarch64).with_timeout(Duration::from_secs(10));
        let result = link_and_run(&obj, &config).unwrap();
        assert!(result.passed());
        assert!(!result.timed_out);
    }
}
