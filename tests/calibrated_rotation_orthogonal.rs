//! Integration tests for B5.L3 — load_calibration, save_calibration, verify_orthogonal.

use fajar_lang::interpreter::Interpreter;

fn run_ok(src: &str) -> String {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => format!("{val}"),
        Err(e) => panic!("expected OK, got error: {e}"),
    }
}

fn run_err(src: &str, substr: &str) {
    let mut interp = Interpreter::new();
    match interp.eval_source(src) {
        Ok(val) => panic!("expected error containing '{substr}', got OK: {val}"),
        Err(e) => {
            let msg = format!("{e}");
            assert!(msg.contains(substr), "expected '{substr}' in error: {msg}");
        }
    }
}

#[test]
fn save_load_roundtrip() {
    // Use env::temp_dir() so the test works on Windows (no /tmp).
    // Forward-slashes avoid Rust/.fj escape ambiguity from Windows backslashes.
    let p = std::env::temp_dir()
        .join("fj_test_cal_rt.bin")
        .to_string_lossy()
        .replace('\\', "/");
    let result = run_ok(&format!(
        r#"
        let t = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        save_calibration(t, "{p}")
        let loaded = load_calibration("{p}", 2, 3)
        loaded
    "#
    ));
    assert!(result.contains("1.0000"), "should contain 1.0: {result}");
    assert!(result.contains("6.0000"), "should contain 6.0: {result}");
}

#[test]
fn verify_orthogonal_identity() {
    let result = run_ok(
        r#"
        let eye = eye(4)
        verify_orthogonal(eye, 1e-10)
    "#,
    );
    assert_eq!(result, "true");
}

#[test]
fn verify_orthogonal_hadamard() {
    // Hadamard matrix is orthogonal by construction
    let result = run_ok(
        r#"
        let rotation = from_data([
            0.5, 0.5, 0.5, 0.5,
            0.5, -0.5, 0.5, -0.5,
            0.5, 0.5, -0.5, -0.5,
            0.5, -0.5, -0.5, 0.5
        ], [4, 4])
        verify_orthogonal(rotation, 1e-10)
    "#,
    );
    assert_eq!(result, "true");
}

#[test]
fn verify_orthogonal_rejects_non_orthogonal() {
    let result = run_ok(
        r#"
        let bad = from_data([1.0, 2.0, 3.0, 4.0], [2, 2])
        verify_orthogonal(bad, 1e-6)
    "#,
    );
    assert_eq!(result, "false");
}

#[test]
fn verify_orthogonal_rejects_non_square() {
    run_err(
        r#"
        let rect = from_data([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], [2, 3])
        verify_orthogonal(rect, 1e-6)
    "#,
        "square matrix",
    );
}

#[test]
fn load_calibration_wrong_size() {
    let p = std::env::temp_dir()
        .join("fj_test_cal_bad.bin")
        .to_string_lossy()
        .replace('\\', "/");
    run_ok(&format!(
        r#"
        let t = from_data([1.0, 2.0], [1, 2])
        save_calibration(t, "{p}")
    "#
    ));
    run_err(
        &format!(
            r#"
        load_calibration("{p}", 3, 3)
    "#
        ),
        "expected",
    );
}

#[test]
fn load_calibration_file_not_found() {
    // Use a path that won't exist on any platform (guaranteed-missing dir).
    let p = std::env::temp_dir()
        .join("fj_nonexistent_dir_1234567890")
        .join("file.bin")
        .to_string_lossy()
        .replace('\\', "/");
    run_err(
        &format!(
            r#"
        load_calibration("{p}", 2, 2)
    "#
        ),
        "load_calibration",
    );
}

#[test]
fn save_load_verify_pipeline() {
    // Full pipeline: create orthogonal matrix → save → load → verify
    let p = std::env::temp_dir()
        .join("fj_test_cal_pipe.bin")
        .to_string_lossy()
        .replace('\\', "/");
    let result = run_ok(&format!(
        r#"
        let rotation = from_data([
            0.5, 0.5, 0.5, 0.5,
            0.5, -0.5, 0.5, -0.5,
            0.5, 0.5, -0.5, -0.5,
            0.5, -0.5, -0.5, 0.5
        ], [4, 4])
        save_calibration(rotation, "{p}")
        let loaded = load_calibration("{p}", 4, 4)
        verify_orthogonal(loaded, 1e-10)
    "#
    ));
    assert_eq!(result, "true");
}
