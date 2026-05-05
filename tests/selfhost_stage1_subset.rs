//! Stage-1-Subset self-host bootstrap chain end-to-end test.
//!
//! Phase 6: this test runs the fj-source bootstrap chain
//! (stdlib/lexer.fj + stdlib/analyzer.fj + stdlib/codegen.fj +
//! a driver) over multiple subset programs, gcc-compiles the
//! emitted C, runs the resulting binaries, and asserts exit codes.

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

#[test]
fn p1_return_42() {
    let driver = r#"
fn main() {
    let mut cg = new_codegen()
    cg = emit_preamble(cg)
    cg = emit_function(cg, "main", [], "i64")
    cg = emit_return(cg, "42")
    cg = emit_function_end(cg)
    let mut i = 0
    while i < to_int(len(cg.lines)) {
        println(cg.lines[i])
        i = i + 1
    }
}
"#;
    let combined = format!("{}\n{}", cat_files(&["stdlib/codegen.fj"]), driver);
    let tmp = std::env::temp_dir().join("p1.fj");
    std::fs::write(&tmp, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(out.status.success(), "fj run failed");
    let c_src = String::from_utf8_lossy(&out.stdout);

    let c_path = std::env::temp_dir().join("p1.c");
    let bin_path = std::env::temp_dir().join("p1.bin");
    std::fs::write(&c_path, c_src.as_bytes()).unwrap();

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
    assert_eq!(run.status.code(), Some(42), "binary should return 42");
}

#[test]
fn p2_let_and_return() {
    let driver = r#"
fn main() {
    let mut cg = new_codegen()
    cg = emit_preamble(cg)
    cg = emit_function(cg, "main", [], "i64")
    cg = emit_let(cg, "x", "7")
    cg = emit_return(cg, "x")
    cg = emit_function_end(cg)
    let mut i = 0
    while i < to_int(len(cg.lines)) {
        println(cg.lines[i])
        i = i + 1
    }
}
"#;
    let combined = format!("{}\n{}", cat_files(&["stdlib/codegen.fj"]), driver);
    let tmp = std::env::temp_dir().join("p2.fj");
    std::fs::write(&tmp, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(out.status.success());
    let c_src = String::from_utf8_lossy(&out.stdout);

    let c_path = std::env::temp_dir().join("p2.c");
    let bin_path = std::env::temp_dir().join("p2.bin");
    std::fs::write(&c_path, c_src.as_bytes()).unwrap();

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
    assert_eq!(run.status.code(), Some(7));
}

#[test]
fn p3_two_lets_plus_binop() {
    let driver = r#"
fn main() {
    let mut cg = new_codegen()
    cg = emit_preamble(cg)
    cg = emit_function(cg, "main", [], "i64")
    cg = emit_let(cg, "x", "10")
    cg = emit_let(cg, "y", "20")
    cg = emit_return(cg, "x + y")
    cg = emit_function_end(cg)
    let mut i = 0
    while i < to_int(len(cg.lines)) {
        println(cg.lines[i])
        i = i + 1
    }
}
"#;
    let combined = format!("{}\n{}", cat_files(&["stdlib/codegen.fj"]), driver);
    let tmp = std::env::temp_dir().join("p3.fj");
    std::fs::write(&tmp, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(out.status.success());
    let c_src = String::from_utf8_lossy(&out.stdout);

    let c_path = std::env::temp_dir().join("p3.c");
    let bin_path = std::env::temp_dir().join("p3.bin");
    std::fs::write(&c_path, c_src.as_bytes()).unwrap();

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
    assert_eq!(run.status.code(), Some(30));
}

#[test]
fn p4_if_else_branch() {
    let driver = r#"
fn main() {
    let mut cg = new_codegen()
    cg = emit_preamble(cg)
    cg = emit_function(cg, "main", [], "i64")
    cg = emit_let(cg, "n", "5")
    cg = emit_if(cg, "n > 3")
    cg = emit_return(cg, "111")
    cg = emit_else(cg)
    cg = emit_return(cg, "222")
    cg = emit_endif(cg)
    cg = emit_return(cg, "0")
    cg = emit_function_end(cg)
    let mut i = 0
    while i < to_int(len(cg.lines)) {
        println(cg.lines[i])
        i = i + 1
    }
}
"#;
    let combined = format!("{}\n{}", cat_files(&["stdlib/codegen.fj"]), driver);
    let tmp = std::env::temp_dir().join("p4.fj");
    std::fs::write(&tmp, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(out.status.success());
    let c_src = String::from_utf8_lossy(&out.stdout);

    let c_path = std::env::temp_dir().join("p4.c");
    let bin_path = std::env::temp_dir().join("p4.bin");
    std::fs::write(&c_path, c_src.as_bytes()).unwrap();

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
    assert_eq!(run.status.code(), Some(111));
}

#[test]
fn p5_println_runtime() {
    let driver = r#"
fn main() {
    let mut cg = new_codegen()
    cg = emit_preamble(cg)
    cg = emit_function(cg, "main", [], "i64")
    cg = emit_println(cg, "777")
    cg = emit_return(cg, "0")
    cg = emit_function_end(cg)
    let mut i = 0
    while i < to_int(len(cg.lines)) {
        println(cg.lines[i])
        i = i + 1
    }
}
"#;
    let combined = format!("{}\n{}", cat_files(&["stdlib/codegen.fj"]), driver);
    let tmp = std::env::temp_dir().join("p5.fj");
    std::fs::write(&tmp, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp.to_str().unwrap()])
        .output()
        .expect("fj run");
    assert!(out.status.success());
    let c_src = String::from_utf8_lossy(&out.stdout);

    let c_path = std::env::temp_dir().join("p5.c");
    let bin_path = std::env::temp_dir().join("p5.bin");
    std::fs::write(&c_path, c_src.as_bytes()).unwrap();

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
    assert_eq!(run.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(stdout.trim(), "777");
}
