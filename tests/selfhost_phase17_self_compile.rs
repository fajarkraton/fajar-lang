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

/// Common helper: run the chain on a stdlib file, write its C output,
/// gcc -c it, return the path to the .o.
#[cfg(unix)]
fn chain_compile_to_object(label: &str, stdlib_path: &str) -> std::path::PathBuf {
    let driver = format!(
        r#"
fn main() {{
    let src_result = read_file("{stdlib_path}")
    let src = match src_result {{
        Ok(content) => content,
        Err(_) => {{
            println("read_file failed for {stdlib_path}")
            return
        }}
    }}
    let ast = parse_to_ast(src)
    let c_src = emit_program(ast)
    let _ = write_file("/tmp/{label}_self_compile.c", c_src)
    println(f"AST size: {{to_int(len(ast))}}")
}}
"#
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
    let tmp_fj = std::env::temp_dir().join(format!("{label}_self_compile.fj"));
    std::fs::write(&tmp_fj, &combined).unwrap();

    let out = Command::new(fj_binary())
        .args(["run", tmp_fj.to_str().unwrap()])
        .current_dir(workspace())
        .output()
        .expect("fj run");
    assert!(
        out.status.success(),
        "fj run failed for {label}: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let c_path = std::env::temp_dir().join(format!("{label}_self_compile.c"));
    let o_path = std::env::temp_dir().join(format!("{label}_self_compile.o"));
    assert!(
        c_path.exists(),
        "chain failed to write C output for {label}"
    );

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
        "gcc -c failed for {label}:\n{}",
        String::from_utf8_lossy(&cc.stderr)
    );
    o_path
}

#[cfg(unix)]
fn assert_object_exports(o_path: &std::path::Path, required: &[&str]) {
    let nm = Command::new("nm").arg(o_path).output().expect("nm");
    let nm_out = String::from_utf8_lossy(&nm.stdout);
    for sym in required {
        assert!(
            nm_out.contains(&format!(" T {sym}")),
            "missing exported symbol: {sym}\nnm output:\n{nm_out}"
        );
    }
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

#[cfg(unix)]
#[test]
fn phase17_all_three_combined_self_compile_to_object() {
    // Phase 17 milestone #3 — the THIRD and final stdlib module joins the
    // self-compile family: stdlib/codegen.fj + parser_ast.fj +
    // codegen_driver.fj concatenated and run through the chain together.
    // The all-3 source emits a single .c that gcc accepts and that exports
    // the union of the three modules' public APIs as T symbols.
    //
    // Unlocked by v34.5.12: Value::Array → Arc<Vec<Value>> migration. Before
    // that, eval_field deep-cloned every struct.array_field read, making the
    // combined parse 5+ minutes; now ~35s.
    let driver = r#"
fn main() {
    let codegen_src_r = read_file("stdlib/codegen.fj")
    let codegen_src = match codegen_src_r { Ok(c) => c, Err(_) => { println("codegen read failed"); return } }
    let parser_src_r = read_file("stdlib/parser_ast.fj")
    let parser_src = match parser_src_r { Ok(c) => c, Err(_) => { println("parser_ast read failed"); return } }
    let driver_src_r = read_file("stdlib/codegen_driver.fj")
    let driver_src = match driver_src_r { Ok(c) => c, Err(_) => { println("codegen_driver read failed"); return } }
    let combined = concat!(codegen_src, "\n", parser_src, "\n", driver_src)
    let ast = parse_to_ast(combined)
    let c_src = emit_program(ast)
    let _ = write_file("/tmp/all_three_phase17_self_compile.c", c_src)
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
    let tmp_fj = std::env::temp_dir().join("all_three_phase17_probe.fj");
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

    let c_path = std::env::temp_dir().join("all_three_phase17_self_compile.c");
    let o_path = std::env::temp_dir().join("all_three_phase17_self_compile.o");
    assert!(c_path.exists(), "chain failed to write all-3 C output");

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
        "gcc -c failed for all-3:\n{}",
        String::from_utf8_lossy(&cc.stderr)
    );

    // Spot-check a representative subset spanning all three modules.
    let required = [
        // codegen.fj
        "new_codegen",
        "emit_preamble",
        "emit_program",
        // parser_ast.fj
        "parse_to_ast",
        "parse_primary_ast",
        "parse_stmt_ast",
        // codegen_driver.fj
        "find_method_name",
        "map_method",
        "map_binop",
    ];
    assert_object_exports(&o_path, &required);
}

#[cfg(unix)]
#[test]
fn phase17_codegen_fj_self_compile_to_object() {
    // Phase 17 milestone — sister to parser_ast.fj. The chain compiles
    // stdlib/codegen.fj's full source (~541 LOC, 33 fns) into a valid
    // GCC object file. This is the SECOND of three stdlib modules to
    // self-compile cleanly via the chain.
    let o_path = chain_compile_to_object("codegen", "stdlib/codegen.fj");
    let required = [
        "new_codegen",
        "emit_line",
        "indent",
        "dedent",
        "record_var_type",
        "clear_var_types",
        "add_struct_name",
        "is_struct_name",
        "add_fn_ret_type",
        "lookup_fn_ret_type",
        "map_type_ctx",
        "lookup_var_type",
        "emit_preamble",
        "emit_function",
        "emit_function_typed",
        "emit_function_end",
        "emit_let",
        "emit_return",
        "emit_if",
        "emit_else",
        "emit_endif",
        "emit_while",
        "emit_endwhile",
        "emit_call",
        "emit_println",
        "map_binop",
        "generate_c",
        "c_type_for",
        "c_operator",
        "line_count",
        "generate_hello_c",
    ];
    assert_object_exports(&o_path, &required);
}

/// Phase 17.8 / v35.0.0 — Stage 2 self-host TRIPLE TEST.
///
/// 1. The interpreter-driven chain compiles `stdlib/{codegen,parser_ast,
///    codegen_driver,selfhost_main}.fj` → `stage1.c` → gcc → `fjc-stage1`
///    native binary.
/// 2. `fjc-stage1` is given the SAME 4-file combined source as input → produces
///    `stage2.c`. Assert `stage2.c` byte-identical to `stage1.c` (the chain's
///    own output). This proves fj-source → C transformation is reproducible
///    when invoked through the native binary.
/// 3. Build `fjc-stage2` from `stage2.c` via gcc. Run BOTH `fjc-stage1` and
///    `fjc-stage2` on a small third-party fj source. Assert they emit
///    byte-identical C.
///
/// What this proves: the self-hosted compilation pipeline reaches a fixed
/// point — the binary, when re-applied to its own source, reproduces itself
/// exactly.
#[cfg(unix)]
#[test]
fn phase17_stage2_native_triple_test() {
    let probe_driver = r#"
fn main() {
    let codegen_src_r = read_file("stdlib/codegen.fj")
    let codegen_src = match codegen_src_r { Ok(c) => c, Err(_) => { println("codegen read failed"); return } }
    let parser_src_r = read_file("stdlib/parser_ast.fj")
    let parser_src = match parser_src_r { Ok(c) => c, Err(_) => { println("parser_ast read failed"); return } }
    let driver_src_r = read_file("stdlib/codegen_driver.fj")
    let driver_src = match driver_src_r { Ok(c) => c, Err(_) => { println("codegen_driver read failed"); return } }
    let main_src_r = read_file("stdlib/selfhost_main.fj")
    let main_src = match main_src_r { Ok(c) => c, Err(_) => { println("selfhost_main read failed"); return } }
    let combined = concat!(codegen_src, "\n", parser_src, "\n", driver_src, "\n", main_src)
    let ast = parse_to_ast(combined)
    let c_src = emit_program(ast)
    let _ = write_file("/tmp/fjc_triple_stage1.c", c_src)
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
        probe_driver
    );
    let tmp_probe = std::env::temp_dir().join("fjc_triple_probe.fj");
    std::fs::write(&tmp_probe, &combined).unwrap();

    // Step 1: interpreter chain → stage1.c
    let out = Command::new(fj_binary())
        .args(["run", tmp_probe.to_str().unwrap()])
        .current_dir(workspace())
        .output()
        .expect("fj run");
    assert!(
        out.status.success(),
        "interpreter chain failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stage1_c = std::env::temp_dir().join("fjc_triple_stage1.c");
    let fjc_stage1 = std::env::temp_dir().join("fjc-triple-stage1");
    let cc1 = Command::new("gcc")
        .args([
            stage1_c.to_str().unwrap(),
            "-o",
            fjc_stage1.to_str().unwrap(),
            "-w",
        ])
        .output()
        .expect("gcc stage1");
    assert!(
        cc1.status.success(),
        "gcc stage1 failed:\n{}",
        String::from_utf8_lossy(&cc1.stderr)
    );

    // Step 2: stage1 binary on its own source → stage2.c
    let combined_4file = cat_files(&[
        "stdlib/codegen.fj",
        "stdlib/parser_ast.fj",
        "stdlib/codegen_driver.fj",
        "stdlib/selfhost_main.fj",
    ]);
    let combined_4file_path = std::env::temp_dir().join("fjc_triple_combined.fj");
    std::fs::write(&combined_4file_path, &combined_4file).unwrap();
    let stage2_c = std::env::temp_dir().join("fjc_triple_stage2.c");
    let run1 = Command::new(&fjc_stage1)
        .args([
            combined_4file_path.to_str().unwrap(),
            stage2_c.to_str().unwrap(),
        ])
        .current_dir(workspace())
        .output()
        .expect("fjc-stage1 run");
    assert!(
        run1.status.success(),
        "fjc-stage1 self-compile failed: stderr={}",
        String::from_utf8_lossy(&run1.stderr)
    );

    // Invariant 1: stage2.c byte-identical to stage1.c (the chain's output).
    let stage1_bytes = std::fs::read(&stage1_c).unwrap();
    let stage2_bytes = std::fs::read(&stage2_c).unwrap();
    assert_eq!(
        stage1_bytes,
        stage2_bytes,
        "stage2.c diverges from stage1.c — self-host is not a fixed point. \
         stage1 size={}, stage2 size={}",
        stage1_bytes.len(),
        stage2_bytes.len()
    );

    // Step 3: build stage2 binary; run both stages on a third-party fj source.
    let fjc_stage2 = std::env::temp_dir().join("fjc-triple-stage2");
    let cc2 = Command::new("gcc")
        .args([
            stage2_c.to_str().unwrap(),
            "-o",
            fjc_stage2.to_str().unwrap(),
            "-w",
        ])
        .output()
        .expect("gcc stage2");
    assert!(
        cc2.status.success(),
        "gcc stage2 failed:\n{}",
        String::from_utf8_lossy(&cc2.stderr)
    );

    let third = std::env::temp_dir().join("fjc_triple_third.fj");
    std::fs::write(
        &third,
        "fn main() {\n    let x = 21\n    let y = x + x\n    println(y)\n}\n",
    )
    .unwrap();
    let third_c1 = std::env::temp_dir().join("fjc_triple_third_s1.c");
    let third_c2 = std::env::temp_dir().join("fjc_triple_third_s2.c");
    let r1 = Command::new(&fjc_stage1)
        .args([third.to_str().unwrap(), third_c1.to_str().unwrap()])
        .output()
        .expect("stage1 third");
    assert!(r1.status.success(), "stage1 on third-party input failed");
    let r2 = Command::new(&fjc_stage2)
        .args([third.to_str().unwrap(), third_c2.to_str().unwrap()])
        .output()
        .expect("stage2 third");
    assert!(r2.status.success(), "stage2 on third-party input failed");

    // Invariant 2: stage1 and stage2 produce byte-identical output.
    let third_b1 = std::fs::read(&third_c1).unwrap();
    let third_b2 = std::fs::read(&third_c2).unwrap();
    assert_eq!(
        third_b1, third_b2,
        "stage1 and stage2 disagree on third-party input"
    );

    // Sanity: the third-party C compiles + prints `42`.
    let third_bin = std::env::temp_dir().join("fjc_triple_third_bin");
    let cc3 = Command::new("gcc")
        .args([
            third_c1.to_str().unwrap(),
            "-o",
            third_bin.to_str().unwrap(),
            "-w",
        ])
        .output()
        .expect("gcc third");
    assert!(cc3.status.success(), "gcc third failed");
    let third_run = Command::new(&third_bin).output().expect("run third");
    assert!(third_run.status.success(), "third binary did not exit 0");
    assert_eq!(
        String::from_utf8_lossy(&third_run.stdout).trim(),
        "42",
        "third binary stdout != 42"
    );
}
