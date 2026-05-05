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

#[cfg(unix)]
#[test]
fn full_p9_cross_fn_call() {
    // R8 closure: multi-fn programs with typed parameters and cross-fn call.
    let r = compile_subset_program(
        "full_p9",
        "fn add(a: i64, b: i64) -> i64 { return a + b } fn main() -> i64 { return add(2, 3) }",
    );
    assert_eq!(r.status.code(), Some(5));
}

#[cfg(unix)]
#[test]
fn full_p10_while_loop() {
    let r = compile_subset_program(
        "full_p10",
        "fn main() -> i64 { let mut i = 0; while i < 5 { i = i + 1 }; return i }",
    );
    assert_eq!(r.status.code(), Some(5));
}

#[cfg(unix)]
#[test]
fn full_p11_str_literal_println() {
    let r = compile_subset_program(
        "full_p11",
        "fn main() -> i64 { println(\"hello\"); return 0 }",
    );
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "hello");
}

#[cfg(unix)]
#[test]
fn full_p12_bool_literal_branch() {
    let r = compile_subset_program(
        "full_p12",
        "fn main() -> i64 { let flag = true; if flag { return 1 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn full_p13_float_literal() {
    // Float literal is stored in a typed `double` variable.
    // Stage-1 ret type stays i64, returning a constant.
    let r = compile_subset_program(
        "full_p13",
        "fn main() -> i64 { let pi = 3.14; let s = \"hi\"; return 7 }",
    );
    assert_eq!(r.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn full_p14_cross_fn_with_loop() {
    // Combine cross-fn + while-loop: factorial via accumulator.
    let r = compile_subset_program(
        "full_p14",
        "fn fact(n: i64) -> i64 { let mut acc = 1; let mut i = 1; while i <= n { acc = acc * i; i = i + 1 }; return acc } fn main() -> i64 { return fact(5) }",
    );
    assert_eq!(r.status.code(), Some(120));
}

#[cfg(unix)]
#[test]
fn full_p15_struct_decl() {
    // Struct declaration emits valid C; main returns a literal.
    let r = compile_subset_program(
        "full_p15",
        "struct Point { x: i64, y: i64 } fn main() -> i64 { return 13 }",
    );
    assert_eq!(r.status.code(), Some(13));
}

#[cfg(unix)]
#[test]
fn full_p16_enum_decl() {
    // Enum declaration emits typedef enum; main returns a literal.
    let r = compile_subset_program(
        "full_p16",
        "enum Color { Red, Green, Blue } fn main() -> i64 { return 17 }",
    );
    assert_eq!(r.status.code(), Some(17));
}

#[cfg(unix)]
#[test]
fn full_p17_struct_and_enum_together() {
    // Both decls + a main that uses neither (decls are valid C just by themselves).
    let r = compile_subset_program(
        "full_p17",
        "struct V { a: i64 } enum E { X, Y } fn main() -> i64 { return 19 }",
    );
    assert_eq!(r.status.code(), Some(19));
}

#[cfg(unix)]
#[test]
fn full_p18_struct_literal_and_field_access() {
    // Phase 10: struct DECL is no longer hollow — instances + field reads work.
    let r = compile_subset_program(
        "full_p18",
        "struct Point { x: i64, y: i64 } fn main() -> i64 { let p = Point { x: 10, y: 20 }; return p.x + p.y }",
    );
    assert_eq!(r.status.code(), Some(30));
}

#[cfg(unix)]
#[test]
fn full_p19_enum_variant_use() {
    // Phase 10: enum DECL is no longer hollow — variant access via `EnumName::Variant` works.
    let r = compile_subset_program(
        "full_p19",
        "enum Color { Red, Green, Blue } fn main() -> i64 { let c = Color::Green; return c }",
    );
    // Color_Green == 1 in C enum order
    assert_eq!(r.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn full_p20_for_loop_range() {
    // Phase 10: for loop with `start..end` range syntax → C `for (i = start; i < end; i++)`
    let r = compile_subset_program(
        "full_p20",
        "fn main() -> i64 { let mut s = 0; for i in 0..5 { s = s + i }; return s }",
    );
    // 0+1+2+3+4 = 10
    assert_eq!(r.status.code(), Some(10));
}

#[cfg(unix)]
#[test]
fn full_p21_for_with_field_access_and_struct_lit() {
    // Composability: struct literal + for loop + field access in body.
    let r = compile_subset_program(
        "full_p21",
        "struct Acc { total: i64 } fn main() -> i64 { let mut a = Acc { total: 0 }; for i in 1..6 { a = Acc { total: a.total + i } }; return a.total }",
    );
    // 1+2+3+4+5 = 15
    assert_eq!(r.status.code(), Some(15));
}

#[cfg(unix)]
#[test]
fn full_p22_enum_variant_in_branch() {
    // Composability: enum variant inside if-condition (C eq comparison).
    let r = compile_subset_program(
        "full_p22",
        "enum Mode { On, Off } fn main() -> i64 { let m = Mode::On; if m == Mode::On { return 100 } else { return 200 } }",
    );
    assert_eq!(r.status.code(), Some(100));
}

#[cfg(unix)]
#[test]
fn full_p23_struct_field_write() {
    // R10 closure: mutable struct field writes work alongside reads.
    // Note: exit codes are 8-bit on Unix, so keep result < 256.
    let r = compile_subset_program(
        "full_p23",
        "struct Point { x: i64, y: i64 } fn main() -> i64 { let mut p = Point { x: 1, y: 2 }; p.x = 50; p.y = 70; return p.x + p.y }",
    );
    assert_eq!(r.status.code(), Some(120));
}

#[cfg(unix)]
#[test]
fn full_p24_else_if_chain() {
    // Multi-branch via else-if. v33.7.0 silently dropped this.
    let r = compile_subset_program(
        "full_p24",
        "fn main() -> i64 { let n = 7; if n > 10 { return 1 } else if n > 5 { return 2 } else { return 3 } }",
    );
    assert_eq!(r.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn full_p25_single_line_comment() {
    // Single-line `//` comment. v33.7.0 produced parse error.
    let r = compile_subset_program(
        "full_p25",
        "fn main() -> i64 {\n    // this is a comment\n    return 42\n}",
    );
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p26_block_comment() {
    // Block `/* ... */` comment.
    let r = compile_subset_program(
        "full_p26",
        "fn main() -> i64 { /* skip me */ let x = 5; /* and me */ return x + 8 }",
    );
    assert_eq!(r.status.code(), Some(13));
}

#[cfg(unix)]
#[test]
fn full_p27_match_enum_variants() {
    // Match over enum variants with default `_`.
    let r = compile_subset_program(
        "full_p27",
        "enum Color { Red, Green, Blue } fn main() -> i64 { let c = Color::Green; let v = match c { Color::Red => 100, Color::Green => 200, Color::Blue => 50, _ => 0 }; return v }",
    );
    assert_eq!(r.status.code(), Some(200));
}

#[cfg(unix)]
#[test]
fn full_p28_match_int_literals() {
    // Match over integer literals.
    let r = compile_subset_program(
        "full_p28",
        "fn main() -> i64 { let n = 3; let v = match n { 1 => 10, 2 => 20, 3 => 30, _ => 99 }; return v }",
    );
    assert_eq!(r.status.code(), Some(30));
}

#[cfg(unix)]
#[test]
fn full_p29_match_wildcard_only() {
    // Match where subject doesn't match any specific arm; falls to default.
    let r = compile_subset_program(
        "full_p29",
        "fn main() -> i64 { let n = 99; let v = match n { 1 => 10, 2 => 20, _ => 77 }; return v }",
    );
    assert_eq!(r.status.code(), Some(77));
}

#[cfg(unix)]
#[test]
fn full_p30_match_in_return() {
    // `return match { ... }` — match directly as expression.
    let r = compile_subset_program(
        "full_p30",
        "enum Mode { On, Off } fn main() -> i64 { let m = Mode::On; return match m { Mode::On => 1, Mode::Off => 0 } }",
    );
    assert_eq!(r.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn full_p31_match_in_arithmetic() {
    // Match used inside arithmetic — proves it composes as a regular atom.
    let r = compile_subset_program(
        "full_p31",
        "fn main() -> i64 { let x = 2; let r = match x { 1 => 10, 2 => 20, _ => 0 } + 5; return r }",
    );
    assert_eq!(r.status.code(), Some(25));
}

#[cfg(unix)]
#[test]
fn full_p32_string_param_and_strlen() {
    // Phase 13: string-typed fn param + strlen builtin.
    let r = compile_subset_program(
        "full_p32",
        "fn lengthof(s: str) -> i64 { return strlen(s) } fn main() -> i64 { return lengthof(\"hello\") }",
    );
    assert_eq!(r.status.code(), Some(5));
}

#[cfg(unix)]
#[test]
fn full_p33_string_eq_via_strcmp() {
    // Phase 13: `s == "literal"` lowers to `_fj_streq(s, "literal")` (strcmp wrapper).
    let r = compile_subset_program(
        "full_p33",
        "fn main() -> i64 { let s = \"hello\"; if s == \"hello\" { return 42 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p34_method_call_substring() {
    // Phase 13: `s.substring(a, b)` lowers to `_fj_substring(s, a, b)` C runtime helper.
    let r = compile_subset_program(
        "full_p34",
        "fn main() -> i64 { let s = \"hello world\"; let h = s.substring(0, 5); if h == \"hello\" { return 11 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(11));
}

#[cfg(unix)]
#[test]
fn full_p35_count_vowels_composability() {
    // Phase 13 headline: real string-processing program combines all new features.
    let r = compile_subset_program(
        "full_p35",
        "fn count_vowels(s: str) -> i64 { let mut count = 0; let mut i = 0; let n = strlen(s); while i < n { let c = s.substring(i, i + 1); if c == \"a\" { count = count + 1 }; if c == \"e\" { count = count + 1 }; if c == \"i\" { count = count + 1 }; if c == \"o\" { count = count + 1 }; if c == \"u\" { count = count + 1 }; i = i + 1 }; return count } fn main() -> i64 { return count_vowels(\"hello world\") }",
    );
    // 'hello world' has e, o, o = 3 vowels
    assert_eq!(r.status.code(), Some(3));
}

#[cfg(unix)]
#[test]
fn full_p36_empty_array_lit_and_len() {
    // Phase 14: `let arr: [i64] = []` and `arr.len()` work.
    let r = compile_subset_program(
        "full_p36",
        "fn main() -> i64 { let arr: [i64] = []; return arr.len() }",
    );
    assert_eq!(r.status.code(), Some(0));
}

#[cfg(unix)]
#[test]
fn full_p37_array_lit_with_elems() {
    // Phase 14: `[1, 2, 3]` with .len()
    let r = compile_subset_program(
        "full_p37",
        "fn main() -> i64 { let arr: [i64] = [1, 2, 3, 4, 5]; return arr.len() }",
    );
    assert_eq!(r.status.code(), Some(5));
}

#[cfg(unix)]
#[test]
fn full_p38_array_push_and_index() {
    // Phase 14: arr.push(x) returns updated array; arr[i] indexes.
    let r = compile_subset_program(
        "full_p38",
        "fn main() -> i64 { let mut arr: [i64] = []; arr = arr.push(7); arr = arr.push(11); return arr[0] + arr[1] }",
    );
    assert_eq!(r.status.code(), Some(18));
}

#[cfg(unix)]
#[test]
fn full_p39_sum_first_n_via_array() {
    // Phase 14 headline: build array, push elements in loop, sum via index.
    let r = compile_subset_program(
        "full_p39",
        "fn sum_first_n(n: i64) -> i64 { let mut arr: [i64] = []; let mut i = 0; while i < n { arr = arr.push(i); i = i + 1 }; let mut total = 0; let mut k = 0; while k < arr.len() { total = total + arr[k]; k = k + 1 }; return total } fn main() -> i64 { return sum_first_n(5) }",
    );
    // 0+1+2+3+4 = 10
    assert_eq!(r.status.code(), Some(10));
}

#[cfg(unix)]
#[test]
fn full_p40_array_passed_to_fn() {
    // Phase 14: pass [i64] as fn param + return processed value.
    let r = compile_subset_program(
        "full_p40",
        "fn sum_array(arr: [i64]) -> i64 { let mut total = 0; let mut i = 0; while i < arr.len() { total = total + arr[i]; i = i + 1 }; return total } fn main() -> i64 { let xs: [i64] = [10, 20, 30, 40]; return sum_array(xs) }",
    );
    // 10+20+30+40 = 100
    assert_eq!(r.status.code(), Some(100));
}

#[cfg(unix)]
#[test]
fn full_p41_to_int_conversion() {
    // Phase 15: to_int(s) → atoll(s) wrapper.
    let r = compile_subset_program("full_p41", "fn main() -> i64 { return to_int(\"42\") }");
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p42_to_string_then_strlen() {
    // Phase 15: to_string(n) → snprintf wrapper. Verify by passing through strlen.
    let r = compile_subset_program(
        "full_p42",
        "fn main() -> i64 { let s = to_string(12345); return strlen(s) }",
    );
    // "12345" has length 5
    assert_eq!(r.status.code(), Some(5));
}

#[cfg(unix)]
#[test]
fn full_p43_concat_macro_two_args() {
    // Phase 15: concat!(a, b) → _fj_concat2(a, b).
    let r = compile_subset_program(
        "full_p43",
        "fn main() -> i64 { let s = concat!(\"hi \", \"world\"); if s == \"hi world\" { return 1 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn full_p44_concat_macro_three_args() {
    // Phase 15: concat! with 3 args → nested _fj_concat2 calls.
    let r = compile_subset_program(
        "full_p44",
        "fn main() -> i64 { let s = concat!(\"a\", \"b\", \"c\"); return strlen(s) }",
    );
    // "abc" → 3
    assert_eq!(r.status.code(), Some(3));
}

#[cfg(unix)]
#[test]
fn full_p45_str_array_push_and_get() {
    // Phase 15: [str] dynamic array — push string + read back via _fj_arr_get_str.
    // Note: arr[i] indexing always emits _fj_arr_get_i64 currently (Phase 16 work
    // for proper element-type dispatch). Use the lower-level helper directly.
    let r = compile_subset_program(
        "full_p45",
        "fn main() -> i64 { let mut arr: [str] = []; arr = arr.push(\"hello\"); arr = arr.push(\"world\"); let h = _fj_arr_get_str(arr, 0); if h == \"hello\" { return arr.len() } else { return 0 } }",
    );
    // arr.len() = 2 after two pushes
    assert_eq!(r.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn full_p46_str_array_index_auto_dispatch() {
    // Phase 15.1: arr[i] on declared [str] should auto-dispatch to _fj_arr_get_str.
    // No need for fj source to call the C helper directly anymore.
    let r = compile_subset_program(
        "full_p46",
        "fn main() -> i64 { let mut arr: [str] = []; arr = arr.push(\"foo\"); arr = arr.push(\"bar\"); let s = arr[1]; if s == \"bar\" { return 7 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn full_p47_str_array_push_ident_auto_dispatch() {
    // Phase 15.1: arr.push(s) where s is str-typed var should auto-dispatch
    // to _fj_arr_push_str (was defaulting to _i64).
    let r = compile_subset_program(
        "full_p47",
        "fn main() -> i64 { let s = \"alpha\"; let mut arr: [str] = []; arr = arr.push(s); let r = arr[0]; if r == \"alpha\" { return 9 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(9));
}

#[cfg(unix)]
#[test]
fn full_p48_str_array_in_fn_param() {
    // Phase 15.1: fn param of type [str] should be tracked + arr[i] dispatches correctly.
    let r = compile_subset_program(
        "full_p48",
        "fn first(arr: [str]) -> str { return arr[0] } fn main() -> i64 { let mut xs: [str] = []; xs = xs.push(\"hello\"); let f = first(xs); if f == \"hello\" { return 11 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(11));
}

#[cfg(unix)]
#[test]
fn full_p49_match_string_subject_dispatches_strcmp() {
    // R12 closure: match with string-typed subject lowers cond to _fj_streq, not raw ==.
    let r = compile_subset_program(
        "full_p49",
        "fn classify(s: str) -> i64 { return match s { \"hello\" => 1, \"world\" => 2, _ => 0 } } fn main() -> i64 { return classify(\"world\") }",
    );
    assert_eq!(r.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn full_p50_match_string_default() {
    // R12: string match with no matching arm → default.
    let r = compile_subset_program(
        "full_p50",
        "fn classify(s: str) -> i64 { return match s { \"alpha\" => 10, \"beta\" => 20, _ => 99 } } fn main() -> i64 { return classify(\"gamma\") }",
    );
    assert_eq!(r.status.code(), Some(99));
}

#[cfg(unix)]
#[test]
fn full_p51_match_string_literal_subject() {
    // R12: match where SUBJECT is a string literal (not an ident).
    let r = compile_subset_program(
        "full_p51",
        "fn main() -> i64 { return match \"yes\" { \"no\" => 0, \"yes\" => 42, _ => 99 } }",
    );
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p52_unary_negation_int() {
    // Closure: unary `-` prefix operator. Use 200 then add 50 (positive result).
    // Must use `let neg = 0 - 50` since C exit codes are unsigned-ish; instead test
    // that `-x` parses + emits as `(-x)` correctly.
    let r = compile_subset_program(
        "full_p52",
        "fn main() -> i64 { let x = -50; let y = 100; return y + x }",
    );
    // 100 + (-50) = 50
    assert_eq!(r.status.code(), Some(50));
}

#[cfg(unix)]
#[test]
fn full_p53_unary_logical_not() {
    // Closure: unary `!` prefix operator (logical not).
    let r = compile_subset_program(
        "full_p53",
        "fn main() -> i64 { let f = false; if !f { return 7 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn full_p54_pratt_precedence() {
    // Phase 16: Pratt-style precedence — multiplication binds tighter than addition.
    // `2 + 3 * 4` should = 14, not (2+3)*4 = 20.
    let r = compile_subset_program("full_p54", "fn main() -> i64 { return 2 + 3 * 4 }");
    assert_eq!(r.status.code(), Some(14));
}

#[cfg(unix)]
#[test]
fn full_p55_pratt_compound_logical() {
    // Phase 16: precedence + logical chains. `c >= "0" && c <= "9"` correctly
    // parses as `(c >= "0") && (c <= "9")` — comparison tighter than &&.
    let r = compile_subset_program(
        "full_p55",
        "fn is_digit(c: str) -> bool { return c >= \"0\" && c <= \"9\" } fn main() -> i64 { if is_digit(\"5\") { return 33 } else { return 0 } }",
    );
    assert_eq!(r.status.code(), Some(33));
}

#[cfg(unix)]
#[test]
fn full_p56_parenthesized_expression() {
    // Phase 16: `(expr)` parsed as transparent passthrough.
    let r = compile_subset_program("full_p56", "fn main() -> i64 { return (2 + 3) * 4 }");
    assert_eq!(r.status.code(), Some(20));
}

#[cfg(unix)]
#[test]
fn full_p57_parser_ast_helpers_subset() {
    // Phase 16 headline: subset of stdlib/parser_ast.fj helpers (is_digit_ast,
    // is_alpha_ast, is_alnum_ast) compile through the chain and produce
    // correct results. Validates that fj-source compiler can compile a
    // meaningful chunk of the fj-source compiler's own source code.
    let r = compile_subset_program(
        "full_p57",
        "fn is_digit_ast(c: str) -> bool { return c >= \"0\" && c <= \"9\" } fn is_alpha_ast(c: str) -> bool { return c == \"_\" || (c >= \"a\" && c <= \"z\") || (c >= \"A\" && c <= \"Z\") } fn is_alnum_ast(c: str) -> bool { return is_alpha_ast(c) || is_digit_ast(c) } fn main() -> i64 { let mut count = 0; if is_digit_ast(\"5\") { count = count + 1 }; if is_alpha_ast(\"a\") { count = count + 2 }; if is_alnum_ast(\"_\") { count = count + 4 }; return count }",
    );
    // 1+2+4 = 7
    assert_eq!(r.status.code(), Some(7));
}

#[cfg(unix)]
#[test]
fn full_p58_skip_ws_read_word_read_int() {
    // Phase 16: skip_ws + read_word + read_int helpers from parser_ast.fj
    // compile + run correctly. Adds the to_int(strlen(s)) cast dispatch.
    let r = compile_subset_program(
        "full_p58",
        "fn is_digit_ast(c: str) -> bool { return c >= \"0\" && c <= \"9\" } fn is_alpha_ast(c: str) -> bool { return c == \"_\" || (c >= \"a\" && c <= \"z\") || (c >= \"A\" && c <= \"Z\") } fn is_alnum_ast(c: str) -> bool { return is_alpha_ast(c) || is_digit_ast(c) } fn skip_spaces(src: str, pos: i64) -> i64 { let n = to_int(strlen(src)); let mut p = pos; while p < n { let c = src.substring(p, p + 1); if c == \" \" { p = p + 1 } else { return p } }; return p } fn read_word(src: str, pos: i64) -> i64 { let n = to_int(strlen(src)); let mut p = pos; while p < n && is_alnum_ast(src.substring(p, p + 1)) { p = p + 1 }; return p } fn read_int_at(src: str, pos: i64) -> i64 { let n = to_int(strlen(src)); let mut p = pos; while p < n && is_digit_ast(src.substring(p, p + 1)) { p = p + 1 }; return p } fn main() -> i64 { let p1 = skip_spaces(\"   abc\", 0); let p2 = read_word(\"hello123 world\", 0); let p3 = read_int_at(\"42abc\", 0); return p1 + p2 + p3 }",
    );
    // 3 + 8 + 2 = 13
    assert_eq!(r.status.code(), Some(13));
}

#[cfg(unix)]
#[test]
fn full_p59_implicit_return_from_expr_body() {
    // Phase 16 sub-task 1: `fn f() -> i64 { expr }` (no explicit `return`).
    // Many parser_ast.fj fns end with a bare expression — emit_fn now
    // detects that the last stmt is BEGIN_EXPR_STMT for a non-void fn and
    // emits `return <expr>;` instead of `<expr>;`.
    let r = compile_subset_program(
        "full_p59",
        "fn twice(x: i64) -> i64 { x + x } fn add_one(y: i64) -> i64 { y + 1 } fn main() -> i64 { let a = twice(7); let b = add_one(a); return b }",
    );
    // twice(7) = 14; add_one(14) = 15
    assert_eq!(r.status.code(), Some(15));
}

#[cfg(unix)]
#[test]
fn full_p60_implicit_return_with_let_then_expr() {
    // Phase 16 sub-task 1: implicit return after intermediate let bindings.
    let r = compile_subset_program(
        "full_p60",
        "fn compute(x: i64) -> i64 { let a = x * 2; let b = a + 3; b * 5 } fn main() -> i64 { return compute(4) }",
    );
    // (4*2 + 3) * 5 = 11 * 5 = 55
    assert_eq!(r.status.code(), Some(55));
}

#[cfg(unix)]
#[test]
fn full_p61_implicit_return_str_method_chain() {
    // Phase 16 sub-task 1: implicit return for str-typed expression body
    // (mirrors many parser_ast.fj helpers like `read_word` that end with
    // a bare expression).
    let r = compile_subset_program(
        "full_p61",
        "fn first_char(s: str) -> str { s.substring(0, 1) } fn main() -> i64 { let c = first_char(\"hello\"); if c == \"h\" { return 11 } else { return 99 } }",
    );
    assert_eq!(r.status.code(), Some(11));
}

#[cfg(unix)]
#[test]
fn full_p62_struct_typed_fn_signature() {
    // Phase 16 sub-task 2: `fn f() -> ParseResult { ... }` lowers to
    // `ParseResult f() { ... }` instead of `int64_t f() { ... }`. Pre-scan
    // collects struct names; map_type_ctx returns the bare typedef name
    // when the type matches a declared struct. Same applies to params.
    let r = compile_subset_program(
        "full_p62",
        "struct ParseResult { val: i64, pos: i64, error: bool } fn pr_ok(v: i64, p: i64) -> ParseResult { return ParseResult { val: v, pos: p, error: false } } fn pr_err(p: i64) -> ParseResult { return ParseResult { val: 0, pos: p, error: true } } fn try_parse(src: str, pos: i64) -> ParseResult { let n = to_int(strlen(src)); if pos >= n { return pr_err(pos) }; let c = src.substring(pos, pos + 1); if c == \"x\" { return pr_ok(42, pos + 1) }; return pr_err(pos) } fn main() -> i64 { let r1 = try_parse(\"xyz\", 0); if r1.error { return 99 }; return r1.val }",
    );
    // try_parse("xyz", 0) → pr_ok(42, 1); r1.val = 42
    assert_eq!(r.status.code(), Some(42));
}

#[cfg(unix)]
#[test]
fn full_p63_state_passing_struct_through_chain() {
    // Phase 16 sub-task 2: state-passing pattern (mirrors parser_ast.fj
    // ParseResult flow). Each fn takes state, returns updated state.
    let r = compile_subset_program(
        "full_p63",
        "struct State { count: i64, active: bool } fn bump(s: State) -> State { return State { count: s.count + 1, active: s.active } } fn deactivate(s: State) -> State { return State { count: s.count, active: false } } fn main() -> i64 { let s0 = State { count: 0, active: true }; let s1 = bump(s0); let s2 = bump(s1); let s3 = deactivate(s2); if s3.active { return 99 }; return s3.count }",
    );
    // Three bump-and-deactivate steps: count becomes 2, active false → return 2
    assert_eq!(r.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn full_p64_struct_typed_let_via_call_no_annotation() {
    // Phase 16 sub-task 2: `let r = struct_returning_fn(...)` (no explicit
    // type annotation) — lookup_fn_ret_type derives the struct typedef.
    let r = compile_subset_program(
        "full_p64",
        "struct Box { v: i64 } fn make_box(v: i64) -> Box { return Box { v: v } } fn main() -> i64 { let b = make_box(77); return b.v }",
    );
    assert_eq!(r.status.code(), Some(77));
}
