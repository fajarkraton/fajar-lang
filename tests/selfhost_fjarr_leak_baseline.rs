//! FJARR_LEAK Phase 1 — `_FjArr` leak regression GREEN gate.
//!
//! Per `docs/FJARR_LEAK_PLAN.md` row 18.A.2 + decision file
//! `docs/decisions/2026-05-07-fjarr-leak-strategy.md` (Choice F: A-now arena +
//! D-Phase-19 linear types). Locks in the **GREEN state** post-18.A.1
//! arena migration: `definitely lost + indirectly lost == 0` for the
//! emitted reproducer. Arena chunks themselves are `still reachable` (we do
//! NOT sum that bucket), and `_fj_arena_free_all` registered via atexit
//! reaps them at program exit so the whole class closes cleanly.
//!
//! ## Lifecycle history (for future maintainers)
//! - **Pre-18.A.1 (commit `f13ac484`)**: asserted leak >= 88 bytes (RED
//!   baseline per `docs/FJARR_LEAK_B0_FINDINGS.md` §B0.4).
//! - **Post-18.A.1 (this commit)**: flipped to `assert_eq!(lost, 0)`. If
//!   this regresses, the arena migration in `stdlib/codegen.fj`
//!   `_fj_arr_new` / `_fj_arr_grow` has been reverted or broken.
//!
//! ## Why ignored by default
//! Chain run + gcc + valgrind sweep ~30-50s on a hot release build. Slow for
//! the default suite, fast enough for the pre-push hook + per-PR CI gate.
//! Exercise via `cargo test --test selfhost_fjarr_leak_baseline -- --include-ignored`.
//!
//! ## Why silently skipped without valgrind
//! macOS sandboxes + a few CI runners don't ship `valgrind`. The B0 evidence
//! was captured on Linux with valgrind-3.22.0; that's the supported path.
//! Missing valgrind = test passes with a stderr note rather than failing CI.
//!
//! Requires `gcc` + `valgrind` on PATH; gated to Unix targets.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn tmp_dir() -> PathBuf {
    PathBuf::from("/tmp")
}

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

fn have_valgrind() -> bool {
    Command::new("valgrind")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Sum the bytes reported on `definitely lost:` and `indirectly lost:` lines.
/// Valgrind format: `==PID==    definitely lost: 1,234 bytes in 56 blocks`.
///
/// When the program is leak-free, valgrind emits "All heap blocks were freed
/// -- no leaks are possible" and OMITS the per-class lost lines entirely, so
/// the absence of those lines combined with a present HEAP SUMMARY block is
/// taken as `lost == 0`. If HEAP SUMMARY is missing, valgrind didn't run
/// correctly (e.g., binary crashed) and we return None for diagnostic.
fn parse_valgrind_lost(stderr: &str) -> Option<u64> {
    if !stderr.contains("HEAP SUMMARY:") {
        return None;
    }
    let mut total: u64 = 0;
    for line in stderr.lines() {
        for tag in &["definitely lost:", "indirectly lost:"] {
            if let Some(idx) = line.find(tag) {
                let rest = &line[idx + tag.len()..];
                let digits: String = rest
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit() || *c == ',')
                    .filter(|c| *c != ',')
                    .collect();
                if let Ok(n) = digits.parse::<u64>() {
                    total += n;
                }
            }
        }
    }
    Some(total)
}

#[test]
#[ignore]
fn fjarr_leak_baseline_minimal_array() {
    if !have_valgrind() {
        eprintln!("skipping: valgrind not on PATH (Linux+valgrind-3.x is the supported path)");
        return;
    }

    let repro_src =
        "fn main() { let v: [i64] = [1, 2, 3]; let n = len(v); println(to_string(n)) }\n";
    let repro_path = tmp_dir().join("fjarr_leak_baseline_repro.fj");
    std::fs::write(&repro_path, repro_src).unwrap();

    let driver = r#"
fn main() {
    let src_result = read_file("/tmp/fjarr_leak_baseline_repro.fj")
    let src = match src_result {
        Ok(content) => content,
        Err(_) => {
            println("read_file failed for reproducer")
            return
        }
    }
    let ast = parse_to_ast(src)
    let c_src = emit_program(ast)
    let _ = write_file("/tmp/fjarr_leak_baseline_emitted.c", c_src)
}
"#;

    let combined = format!(
        "{}{}",
        cat_files(&[
            "stdlib/codegen.fj",
            "stdlib/parser_ast.fj",
            "stdlib/codegen_driver.fj",
        ]),
        driver
    );
    let chain_path = tmp_dir().join("fjarr_leak_baseline_chain.fj");
    std::fs::write(&chain_path, &combined).unwrap();

    let chain_out = Command::new(fj_binary())
        .args(["run", chain_path.to_str().unwrap()])
        .current_dir(workspace())
        .output()
        .expect("fj run chain");
    assert!(
        chain_out.status.success(),
        "fj run chain failed: stderr={}",
        String::from_utf8_lossy(&chain_out.stderr)
    );

    let emitted_c = tmp_dir().join("fjarr_leak_baseline_emitted.c");
    assert!(
        emitted_c.exists(),
        "chain did not produce /tmp/fjarr_leak_baseline_emitted.c (chain stdout: {})",
        String::from_utf8_lossy(&chain_out.stdout)
    );

    let bin = tmp_dir().join("fjarr_leak_baseline_bin");
    let cc = Command::new("gcc")
        .args([
            emitted_c.to_str().unwrap(),
            "-o",
            bin.to_str().unwrap(),
            "-w",
            "-Wno-error=int-conversion",
            "-Wno-error=incompatible-pointer-types",
            "-Wno-error=implicit-function-declaration",
            "-Wno-error=implicit-int",
        ])
        .output()
        .expect("gcc");
    assert!(
        cc.status.success(),
        "gcc failed:\n{}",
        String::from_utf8_lossy(&cc.stderr)
    );

    let val = Command::new("valgrind")
        .args([
            "--leak-check=full",
            "--show-leak-kinds=all",
            bin.to_str().unwrap(),
        ])
        .output()
        .expect("valgrind");

    let stderr = String::from_utf8_lossy(&val.stderr);
    let lost = parse_valgrind_lost(&stderr).unwrap_or_else(|| {
        panic!(
            "could not parse `definitely lost`/`indirectly lost` lines from valgrind stderr:\n{}",
            stderr
        )
    });

    assert_eq!(
        lost, 0,
        "expected leak == 0 (GREEN, post-18.A.1 arena migration); got {lost} bytes.\n\
         If this is failing, the _fj_arr_new / _fj_arr_grow arena migration in\n\
         stdlib/codegen.fj has regressed. See docs/FJARR_LEAK_PLAN.md row 18.A.1\n\
         and the existing pre-commit gate in scripts/git-hooks/pre-commit.\n\
         Full valgrind stderr:\n{stderr}"
    );

    eprintln!("GREEN: leak == 0 (post-18.A.1 arena migration). _FjArr realloc-leak class closed.");
}
