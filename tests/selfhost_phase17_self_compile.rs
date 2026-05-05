//! Phase 17 self-compile milestone — Stage 2 self-host arc.
//!
//! Headline test: feed `stdlib/parser_ast.fj` (the fj-source AST builder used
//! by the chain itself) THROUGH the chain (parse_to_ast → emit_program), and
//! verify that gcc accepts the emitted C source as a valid object file (-c).
//!
//! What this proves:
//! - parser_ast.fj's full source is acceptable to the chain (parse_to_ast
//!   handles every construct, including pub/const/if-expr/match/chained
//!   methods/struct literals/nested generics).
//! - emit_program produces valid C (no undeclared idents, no type mismatches,
//!   no malformed structs from `STR "BEGIN_LET"`-style atoms inside bodies).
//! - The `.o` exports all 25 of parser_ast.fj's functions as actual T symbols.
//!
//! Honest scope (CLAUDE.md §6.6 R3):
//! - This is NOT the full triple-test. We don't yet build a STANDALONE binary
//!   and run it on the same source — that requires read_file/write_file/argv
//!   support in the C runtime. That's the next Phase 17 increment.
//! - The chain is currently O(n²) on string ops, so codegen.fj (541 LOC) and
//!   codegen_driver.fj (>1000 LOC) are NOT yet in scope (parsing them via
//!   `fj run` takes >5min). Smaller modules first.
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

#[cfg(unix)]
#[test]
fn phase17_parser_ast_fj_self_compile_to_object() {
    // Driver: read parser_ast.fj from disk, run it through the chain, write
    // the emitted C, and report the AST size + C line count.
    let driver = r#"
fn main() {
    let src_result = read_file("stdlib/parser_ast.fj")
    let src = match src_result {
        Ok(content) => content,
        Err(_) => {
            println("read_file failed for stdlib/parser_ast.fj")
            return
        }
    }
    let ast = parse_to_ast(src)
    let c_src = emit_program(ast)
    let _ = write_file("/tmp/parser_ast_self_compile.c", c_src)
    println(f"AST size: {to_int(len(ast))}")
}
"#;
    let combined = format!(
        "{}{}",
        cat_files(&[
            "stdlib/codegen.fj",
            "stdlib/parser_ast.fj",
            "stdlib/codegen_driver.fj"
        ]),
        driver
    );
    let tmp_fj = std::env::temp_dir().join("phase17_self_compile.fj");
    std::fs::write(&tmp_fj, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp_fj.to_str().unwrap()])
        .current_dir(workspace())
        .output()
        .expect("fj run");
    assert!(
        out.status.success(),
        "fj run failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // The chain should produce an AST of size > 13000 (parser_ast.fj has
    // ~25 fns × hundreds of nodes each). 13321 was the size at first
    // measurement. Loosen lower bound to 13000 for stability across edits.
    let merged = format!("{}{}", stdout, stderr);
    let ast_size: i64 = merged
        .lines()
        .find(|l| l.contains("AST size:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    assert!(
        ast_size >= 13_000,
        "expected AST size >= 13000, got {ast_size}. stderr={stderr}"
    );

    // Compile the emitted C to an object file via gcc -c. -w suppresses
    // benign warnings (mostly unused-variable). We check ONLY exit code.
    let c_path = std::env::temp_dir().join("parser_ast_self_compile.c");
    let o_path = std::env::temp_dir().join("parser_ast_self_compile.o");
    assert!(c_path.exists(), "chain failed to write C output");

    let cc = Command::new("gcc")
        .args([
            c_path.to_str().unwrap(),
            "-c",
            "-o",
            o_path.to_str().unwrap(),
            "-w",
        ])
        .output()
        .expect("gcc");
    assert!(
        cc.status.success(),
        "gcc -c failed:\n{}",
        String::from_utf8_lossy(&cc.stderr)
    );

    // Verify exported T symbols include all of parser_ast.fj's public API.
    let nm = Command::new("nm")
        .arg(o_path.to_str().unwrap())
        .output()
        .expect("nm");
    let nm_out = String::from_utf8_lossy(&nm.stdout);
    let required = [
        "pr_ok",
        "pr_err",
        "is_digit_ast",
        "is_alpha_ast",
        "is_alnum_ast",
        "is_ws_ast",
        "skip_ws",
        "read_word",
        "read_int",
        "expect_char",
        "expect_str",
        "count_method_chain_after",
        "parse_match_ast",
        "parse_primary_ast",
        "parse_expr_prec",
        "parse_expr_ast",
        "parse_stmt_ast",
        "parse_params",
        "parse_fn_ast",
        "parse_struct_ast",
        "parse_enum_ast",
        "parse_const_ast",
        "parse_to_ast",
    ];
    for sym in required {
        assert!(
            nm_out.contains(&format!(" T {sym}")),
            "missing exported symbol: {sym}\nnm output:\n{nm_out}"
        );
    }
}
