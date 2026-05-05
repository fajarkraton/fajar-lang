//! Stage-1-Full self-host integration test (Phase 8).
//!
//! Unlike `selfhost_stage1_subset.rs` which drives codegen.fj via direct
//! emit_* calls hardcoded for 5 program shapes, THIS suite hands the
//! fj-source compiler an arbitrary fj source STRING and verifies that
//! `parse_to_ast(src) → emit_program(ast) → gcc → executable` produces
//! the expected exit code (and stdout, where applicable).
//!
//! Requires `gcc` on PATH — gated to Unix targets.

use std::path::PathBuf;
use std::process::Command;

fn fj_binary() -> PathBuf {
    let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    PathBuf::from(target).join("release/fj")
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cat_files(files: &[&str]) -> String {
    let mut s = String::new();
    for f in files {
        s.push_str(&std::fs::read_to_string(workspace().join(f)).unwrap());
        s.push('\n');
    }
    s
}

fn compile_subset_program(label: &str, fj_source: &str) -> std::process::Output {
    let driver = format!(
        r#"
fn main() {{
    let src = "{}"
    let ast = parse_to_ast(src)
    let c_src = emit_program(ast)
    println(c_src)
}}
"#,
        fj_source.replace('"', "\\\"")
    );
    let combined = format!(
        "{}{}",
        cat_files(&[
            "stdlib/codegen.fj",
            "stdlib/parser_ast.fj",
            "stdlib/codegen_driver.fj"
        ]),
        driver
    );
    let tmp_fj = std::env::temp_dir().join(format!("{label}.fj"));
    std::fs::write(&tmp_fj, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp_fj.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(
        out.status.success(),
        "fj run failed for {label}: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c_path = std::env::temp_dir().join(format!("{label}.c"));
    let bin_path = std::env::temp_dir().join(format!("{label}.bin"));
    std::fs::write(&c_path, &out.stdout).unwrap();

    let cc = Command::new("gcc")
        .args([c_path.to_str().unwrap(), "-o", bin_path.to_str().unwrap()])
        .output()
        .expect("gcc");
    assert!(
        cc.status.success(),
        "gcc failed for {label}: {}",
        String::from_utf8_lossy(&cc.stderr)
    );

    Command::new(&bin_path).output().expect("run binary")
}

#[cfg(unix)]
#[test]
fn full_p1_return_42() {
    let r = compile_subset_program("full_p1", "fn main() -> i64 { return 42 }");
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p2_let_and_return() {
    let r = compile_subset_program("full_p2", "fn main() -> i64 { let x = 7; return x }");
    assert_eq!(r.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn full_p3_two_lets_plus_binop() {
    let r = compile_subset_program(
        "full_p3",
        "fn main() -> i64 { let x = 10; let y = 20; return x + y }",
    );
    assert_eq!(r.status.code(), Some(30));
}

#[cfg(unix)]
#[test]
fn full_p4_if_else_branch() {
    let r = compile_subset_program(
        "full_p4",
        "fn main() -> i64 { let n = 5; if n > 3 { return 111 } else { return 222 } }",
    );
    assert_eq!(r.status.code(), Some(111));
}

#[cfg(unix)]
#[test]
fn full_p5_println_runtime() {
    let r = compile_subset_program("full_p5", "fn main() -> i64 { println(777); return 0 }");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "777");
}

#[cfg(unix)]
#[test]
fn full_p6_chained_binop() {
    let r = compile_subset_program(
        "full_p6",
        "fn main() -> i64 { let x = 5; let y = 10; let z = 2; return x + y + z }",
    );
    assert_eq!(r.status.code(), Some(17));
}

#[cfg(unix)]
#[test]
fn full_p7_multiplication() {
    let r = compile_subset_program(
        "full_p7",
        "fn main() -> i64 { let a = 6; let b = 7; return a * b }",
    );
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p8_subtract_and_compare() {
    let r = compile_subset_program(
        "full_p8",
        "fn main() -> i64 { let x = 50; let y = 30; if x - y > 10 { return 99 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(99));
}
