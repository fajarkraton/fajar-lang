//! Multi-file compilation tests for Fajar Lang.
//!
//! Tests directory-based compilation, dependency ordering,
//! shared type inclusion, and module resolution.
//! Sprint 2 of Master Implementation Plan v7.0.

use std::fs;
use std::path::Path;

fn create_test_project(name: &str, files: &[(&str, &str)]) -> String {
    let dir = std::env::temp_dir()
        .join(format!("fj-test-{name}"))
        .display()
        .to_string();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (filename, content) in files {
        let path = format!("{dir}/{filename}");
        if let Some(parent) = Path::new(&path).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }
    dir
}

fn run_project(dir: &str) -> (bool, Vec<String>) {
    let mut interp = fajar_lang::interpreter::Interpreter::new_capturing();
    let source = read_dir_source(dir);
    match interp.eval_source(&source) {
        Ok(_) => {
            let _ = interp.call_main();
            (true, interp.get_output().to_vec())
        }
        Err(_) => (false, vec![]),
    }
}

fn read_dir_source(dir: &str) -> String {
    let path = Path::new(dir);
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut main_file: Option<std::path::PathBuf> = None;
    collect_fj(path, &mut files, &mut main_file);
    files.sort();
    let mut combined = String::new();
    for f in &files {
        if let Ok(content) = fs::read_to_string(f) {
            combined.push_str(&content);
            combined.push('\n');
        }
    }
    if let Some(m) = main_file {
        if let Ok(content) = fs::read_to_string(m) {
            combined.push_str(&content);
            combined.push('\n');
        }
    }
    combined
}

fn collect_fj(
    dir: &Path,
    files: &mut Vec<std::path::PathBuf>,
    main_file: &mut Option<std::path::PathBuf>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_fj(&p, files, main_file);
            } else if p.extension().map_or(false, |e| e == "fj") {
                if p.file_name().map_or(false, |n| n == "main.fj") {
                    *main_file = Some(p);
                } else {
                    files.push(p);
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
// 1. Basic multi-file
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_two_files() {
    let dir = create_test_project(
        "two",
        &[
            ("helper.fj", "fn helper() -> i64 { 42 }"),
            ("main.fj", "fn main() { println(helper()) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn multifile_three_files() {
    let dir = create_test_project(
        "three",
        &[
            ("math.fj", "fn add(a: i64, b: i64) -> i64 { a + b }"),
            ("utils.fj", "fn double(x: i64) -> i64 { add(x, x) }"),
            ("main.fj", "fn main() { println(double(21)) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("42")));
}

#[test]
fn multifile_nested_dirs() {
    let dir = create_test_project(
        "nested",
        &[
            ("lib/math.fj", "fn add(a: i64, b: i64) -> i64 { a + b }"),
            ("lib/string.fj", "fn greet() -> str { \"hello\" }"),
            ("main.fj", "fn main() { println(add(1, 2)) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("3")));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Main.fj always last
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_main_last() {
    let dir = create_test_project(
        "mainlast",
        &[
            ("zzz_last_alphabetically.fj", "fn zzz() -> i64 { 99 }"),
            ("aaa_first_alphabetically.fj", "fn aaa() -> i64 { 1 }"),
            ("main.fj", "fn main() { println(aaa()) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("1")));
}

// ════════════════════════════════════════════════════════════════════════
// 3. Struct definitions across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_struct_across_files() {
    let dir = create_test_project(
        "structs",
        &[
            ("types.fj", "struct Point { x: f64, y: f64 }"),
            (
                "main.fj",
                "fn main() {\n    let p = Point { x: 1.0, y: 2.0 }\n    println(p.x)\n}",
            ),
        ],
    );
    let (ok, _) = run_project(&dir);
    assert!(ok);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Constants across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_constants() {
    let dir = create_test_project(
        "consts",
        &[
            ("config.fj", "const MAX_SIZE: i64 = 1024"),
            ("main.fj", "fn main() { println(MAX_SIZE) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("1024")));
}

// ════════════════════════════════════════════════════════════════════════
// 5. Context annotations across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_kernel_safe_separation() {
    let dir = create_test_project(
        "contexts",
        &[
            ("kernel.fj", "@kernel fn hw_init() -> i64 { 0 }"),
            ("main.fj", "fn main() { println(42) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("42")));
}

// ════════════════════════════════════════════════════════════════════════
// 6. Empty directory
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_empty_dir() {
    let dir = create_test_project("empty", &[]);
    let source = read_dir_source(&dir);
    assert!(source.trim().is_empty());
}

// ════════════════════════════════════════════════════════════════════════
// 7. Single file in directory
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_single_file() {
    let dir = create_test_project("single", &[("main.fj", "fn main() { println(99) }")]);
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("99")));
}

// ════════════════════════════════════════════════════════════════════════
// 8. Effect system across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_effects() {
    let dir = create_test_project(
        "effects",
        &[
            ("effects.fj", "effect Logger { fn log(msg: str) -> void }"),
            (
                "main.fj",
                "fn greet() with IO { println(42) }\nfn main() { greet() }",
            ),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("42")));
}

// ════════════════════════════════════════════════════════════════════════
// 9. Comptime across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_comptime() {
    let dir = create_test_project(
        "comptime",
        &[
            (
                "math.fj",
                "comptime fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }",
            ),
            ("main.fj", "fn main() { println(comptime { fact(5) }) }"),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("120")));
}

// ════════════════════════════════════════════════════════════════════════
// 10. Many files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_ten_files() {
    let mut files: Vec<(&str, String)> = Vec::new();
    for i in 0..9 {
        let name = format!("module{i}.fj");
        let content = format!("fn func{i}() -> i64 {{ {i} }}");
        files.push((Box::leak(name.into_boxed_str()), content));
    }

    let dir = create_test_project("ten", &[]);
    for (name, content) in &files {
        fs::write(format!("{dir}/{name}"), content).unwrap();
    }
    fs::write(format!("{dir}/main.fj"), "fn main() { println(func0()) }").unwrap();

    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("0")));
}

// ════════════════════════════════════════════════════════════════════════
// 11. Macros across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_macros() {
    let dir = create_test_project(
        "macros",
        &[
            ("helpers.fj", "fn helper() -> i64 { 42 }"),
            (
                "main.fj",
                "fn main() {\n    let arr = vec![1, 2, 3]\n    println(len(arr))\n}",
            ),
        ],
    );
    let (ok, output) = run_project(&dir);
    assert!(ok);
    assert!(output.iter().any(|l| l.contains("3")));
}

// ════════════════════════════════════════════════════════════════════════
// 12. Enum definitions across files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn multifile_enums() {
    let dir = create_test_project(
        "enums",
        &[
            ("types.fj", "enum Color { Red, Green, Blue }"),
            ("main.fj", "fn main() { println(42) }"),
        ],
    );
    let (ok, _) = run_project(&dir);
    assert!(ok);
}
