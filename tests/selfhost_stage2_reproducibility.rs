//! Stage 2 reproducibility — Phase 12.
//!
//! "Stage 2 Lite" reproducibility test for fj-source compiler chain.
//! Each test compiles a target subset program TWICE through the chain
//! (lex → parse_to_ast → emit_program) and verifies:
//!   - generated C source is **byte-identical** across runs
//!   - both runs produce binaries that return the **same exit code**
//!
//! This proves the **fj-source compiler chain is deterministic** —
//! same input source → same C output every time.
//!
//! Honest scope (CLAUDE.md §6.6 R3):
//!
//! 1. Binary BYTE equality is NOT tested. gcc/linker embed
//!    path-dependent strings (input filename, build timestamps) into
//!    ELF binaries that differ between runs even with identical C
//!    input. Binary byte determinism is a gcc/linker concern, not a
//!    fj-source compiler concern. We test C source equality (which
//!    IS fully under our compiler's control) + behavioral equivalence.
//!
//! 2. This is NOT a full Stage 2 triple-test. Genuine triple-test
//!    requires fj-source compiler compiling its OWN source code,
//!    which needs codegen enrichment to handle method calls, dynamic
//!    arrays, and stdlib-builtin lowering — a much larger scope
//!    (3-7d). Current codegen emits C for the Stage-1 subset only,
//!    not for stdlib/parser_ast.fj or stdlib/codegen_driver.fj
//!    themselves.
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

/// Compile a fj source string twice and return both generated C strings.
/// If the chain is deterministic, the two strings are byte-identical.
fn compile_twice_via_chain(label: &str, fj_source: &str) -> (Vec<u8>, Vec<u8>) {
    let driver = format!(
        r#"
fn main() {{
    let src = "{}"
    let ast = parse_to_ast(src)
    let c = emit_program(ast)
    println(c)
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
    let tmp_fj = std::env::temp_dir().join(format!("{label}_repro.fj"));
    std::fs::write(&tmp_fj, &combined).unwrap();

    // Run twice
    let out1 = Command::new(fj_binary())
        .args(["run", tmp_fj.to_str().unwrap()])
        .output()
        .expect("fj run #1");
    assert!(
        out1.status.success(),
        "fj run #1 failed: {}",
        String::from_utf8_lossy(&out1.stderr)
    );

    let out2 = Command::new(fj_binary())
        .args(["run", tmp_fj.to_str().unwrap()])
        .output()
        .expect("fj run #2");
    assert!(
        out2.status.success(),
        "fj run #2 failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );

    (out1.stdout, out2.stdout)
}

/// Compile generated C with gcc, run, return exit code.
fn gcc_run_bin(label: &str, c_src: &[u8]) -> i32 {
    let c_path = std::env::temp_dir().join(format!("{label}.c"));
    let bin_path = std::env::temp_dir().join(format!("{label}.bin"));
    std::fs::write(&c_path, c_src).unwrap();

    let cc = Command::new("gcc")
        .args([c_path.to_str().unwrap(), "-o", bin_path.to_str().unwrap()])
        .output()
        .expect("gcc");
    assert!(
        cc.status.success(),
        "gcc failed: {}",
        String::from_utf8_lossy(&cc.stderr)
    );

    let run = Command::new(&bin_path).output().expect("run binary");
    run.status.code().unwrap_or(-1)
}

// ─────────────────────────────────────────────────────────────────────
// Determinism tests: same input → same C output, same binary, same RC
// ─────────────────────────────────────────────────────────────────────

#[cfg(unix)]
#[test]
fn stage2_p1_c_source_byte_identical() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p1",
        "fn main() -> i64 { let x = 10; let y = 20; return x + y }",
    );
    assert_eq!(c1, c2, "fj-source compiler chain must be deterministic");
}

#[cfg(unix)]
#[test]
fn stage2_p2_if_else_program_reproducible() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p2",
        "fn main() -> i64 { let n = 5; if n > 3 { return 111 } else { return 222 } }",
    );
    assert_eq!(c1, c2, "C source must be byte-identical");
    let rc1 = gcc_run_bin("s2_p2_a", &c1);
    let rc2 = gcc_run_bin("s2_p2_b", &c2);
    assert_eq!(rc1, 111);
    assert_eq!(rc2, 111);
}

#[cfg(unix)]
#[test]
fn stage2_p3_loop_program_reproducible() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p3",
        "fn main() -> i64 { let mut s = 0; for i in 0..10 { s = s + i }; return s }",
    );
    assert_eq!(c1, c2);
    let rc = gcc_run_bin("s2_p3", &c1);
    assert_eq!(rc, 45); // 0+1+...+9 = 45
}

#[cfg(unix)]
#[test]
fn stage2_p4_struct_program_reproducible() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p4",
        "struct P { x: i64, y: i64 } fn main() -> i64 { let p = P { x: 10, y: 20 }; return p.x + p.y }",
    );
    assert_eq!(c1, c2);
    let rc = gcc_run_bin("s2_p4", &c1);
    assert_eq!(rc, 30);
}

#[cfg(unix)]
#[test]
fn stage2_p5_match_program_reproducible() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p5",
        "enum Color { Red, Green, Blue } fn main() -> i64 { let c = Color::Green; return match c { Color::Red => 100, Color::Green => 200, _ => 0 } }",
    );
    assert_eq!(c1, c2);
    let rc = gcc_run_bin("s2_p5", &c1);
    assert_eq!(rc, 200);
}

#[cfg(unix)]
#[test]
fn stage2_p6_cross_fn_program_reproducible() {
    let (c1, c2) = compile_twice_via_chain(
        "s2_p6",
        "fn fact(n: i64) -> i64 { let mut acc = 1; let mut i = 1; while i <= n { acc = acc * i; i = i + 1 }; return acc } fn main() -> i64 { return fact(5) }",
    );
    assert_eq!(c1, c2);
    let rc = gcc_run_bin("s2_p6", &c1);
    assert_eq!(rc, 120);
}
