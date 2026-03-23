//! Optimization pipeline integration tests for Fajar Lang.
//!
//! Tests the unified optimization pipeline, tail call optimization,
//! dead code elimination, constant folding, and compiler profiling.

use fajar_lang::codegen::optimizer::*;
use fajar_lang::parser::ast::Item;

/// Helper: parse source into a Program.
fn parse_program(source: &str) -> fajar_lang::parser::ast::Program {
    let tokens = fajar_lang::lexer::tokenize(source).unwrap();
    fajar_lang::parser::parse(tokens).unwrap()
}

// ════════════════════════════════════════════════════════════════════════
// 1. Optimization pipeline levels
// ════════════════════════════════════════════════════════════════════════

#[test]
fn pipeline_o0_does_nothing() {
    let program = parse_program("fn dead() { 1 }\nfn main() { 42 }");
    let report = optimize_program(&program, OptLevel::O0);
    assert_eq!(report.constants_folded, 0);
    assert_eq!(report.dead_functions_eliminated, 0);
    assert_eq!(report.tail_calls_optimized, 0);
    assert_eq!(report.inline_candidates, 0);
}

#[test]
fn pipeline_o1_does_const_fold_and_dce() {
    let program = parse_program("fn dead() { 1 }\nfn main() { comptime { 2 + 3 } }");
    let report = optimize_program(&program, OptLevel::O1);
    assert!(report.constants_folded >= 1);
    assert!(report.dead_functions_eliminated >= 1);
    // O1 doesn't do TCO or inlining
    assert_eq!(report.tail_calls_optimized, 0);
    assert_eq!(report.inline_candidates, 0);
}

#[test]
fn pipeline_o2_does_all() {
    let source = r#"
fn tiny() -> i64 { 42 }
fn rec(n: i64) -> i64 { if n <= 0 { 0 } else { rec(n - 1) } }
fn dead() { 99 }
fn main() { tiny() }
"#;
    let program = parse_program(source);
    let report = optimize_program(&program, OptLevel::O2);
    assert!(report.total_functions >= 4);
    assert!(report.dead_functions_eliminated >= 1); // dead() and rec() are dead
    assert!(report.tail_calls_optimized >= 1); // rec() has tail call
    assert!(report.inline_candidates >= 1); // tiny() is small
}

#[test]
fn pipeline_o3_same_as_o2() {
    let source = "fn helper() -> i64 { 1 }\nfn main() { helper() }";
    let r2 = optimize_program(&parse_program(source), OptLevel::O2);
    let r3 = optimize_program(&parse_program(source), OptLevel::O3);
    assert_eq!(r2.total_functions, r3.total_functions);
}

// ════════════════════════════════════════════════════════════════════════
// 2. Dead code elimination
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dce_removes_unreachable_fn() {
    let program = parse_program("fn unreachable() { 1 }\nfn main() { 42 }");
    let report = optimize_program(&program, OptLevel::O2);
    assert!(report.dead_functions_eliminated >= 1);
}

#[test]
fn dce_keeps_called_fn() {
    let program = parse_program("fn helper() { 1 }\nfn main() { helper() }");
    let report = optimize_program(&program, OptLevel::O2);
    assert_eq!(report.dead_functions_eliminated, 0);
}

#[test]
fn dce_keeps_transitive_calls() {
    let program = parse_program("fn a() { 1 }\nfn b() { a() }\nfn main() { b() }");
    let report = optimize_program(&program, OptLevel::O2);
    assert_eq!(report.dead_functions_eliminated, 0);
}

#[test]
fn dce_multiple_dead() {
    let program = parse_program("fn d1() { 1 }\nfn d2() { 2 }\nfn d3() { 3 }\nfn main() { 42 }");
    let report = optimize_program(&program, OptLevel::O2);
    assert!(report.dead_functions_eliminated >= 3);
}

#[test]
fn dce_pub_fn_not_dead() {
    let program = parse_program("pub fn api() { 1 }\nfn main() { 42 }");
    let report = optimize_program(&program, OptLevel::O2);
    // pub fn is an entry point, not dead
    assert_eq!(report.dead_functions_eliminated, 0);
}

// ════════════════════════════════════════════════════════════════════════
// 3. Tail call optimization detection
// ════════════════════════════════════════════════════════════════════════

#[test]
fn tco_simple_recursive() {
    let program =
        parse_program("fn loop_fn(n: i64) -> i64 { if n <= 0 { 0 } else { loop_fn(n - 1) } }");
    if let Item::FnDef(fndef) = &program.items[0] {
        assert!(has_tail_self_call("loop_fn", &fndef.body));
    }
}

#[test]
fn tco_not_tail_when_wrapped() {
    // n * fact(n-1) is NOT tail position
    let program =
        parse_program("fn fact(n: i64) -> i64 { if n <= 1 { 1 } else { n * fact(n - 1) } }");
    if let Item::FnDef(fndef) = &program.items[0] {
        assert!(!has_tail_self_call("fact", &fndef.body));
    }
}

#[test]
fn tco_not_tail_when_added() {
    // 1 + recurse(n-1) is NOT tail position
    let program =
        parse_program("fn sum(n: i64) -> i64 { if n <= 0 { 0 } else { 1 + sum(n - 1) } }");
    if let Item::FnDef(fndef) = &program.items[0] {
        assert!(!has_tail_self_call("sum", &fndef.body));
    }
}

#[test]
fn tco_with_multiple_base_cases() {
    let program = parse_program(
        "fn f(n: i64) -> i64 { if n == 0 { 0 } else { if n == 1 { 1 } else { f(n - 2) } } }",
    );
    if let Item::FnDef(fndef) = &program.items[0] {
        assert!(has_tail_self_call("f", &fndef.body));
    }
}

#[test]
fn tco_analyze_non_recursive_returns_none() {
    let program = parse_program("fn add(a: i64, b: i64) -> i64 { a + b }");
    if let Item::FnDef(fndef) = &program.items[0] {
        assert!(analyze_tail_call(fndef).is_none());
    }
}

#[test]
fn tco_analyze_recursive_returns_info() {
    let program =
        parse_program("fn countdown(n: i64) -> i64 { if n <= 0 { 0 } else { countdown(n - 1) } }");
    if let Item::FnDef(fndef) = &program.items[0] {
        let info = analyze_tail_call(fndef).unwrap();
        assert_eq!(info.name, "countdown");
        assert!(info.is_self_recursive);
    }
}

// ════════════════════════════════════════════════════════════════════════
// 4. Constant folding
// ════════════════════════════════════════════════════════════════════════

#[test]
fn const_fold_comptime_block() {
    let program = parse_program("fn main() { comptime { 1 + 2 } }");
    let report = optimize_program(&program, OptLevel::O1);
    assert!(report.constants_folded >= 1);
}

#[test]
fn const_fold_nested_arithmetic() {
    let program = parse_program("fn f() { 1 + 2 + 3 }");
    let report = optimize_program(&program, OptLevel::O1);
    // Binary operations with literal children
    assert!(report.constants_folded >= 1);
}

#[test]
fn const_fold_no_fold_for_variables() {
    let program = parse_program("fn f(x: i64) { x + 1 }");
    let report = optimize_program(&program, OptLevel::O1);
    // x + 1 cannot be folded (x is variable)
    assert_eq!(report.constants_folded, 0);
}

// ════════════════════════════════════════════════════════════════════════
// 5. Inlining candidates
// ════════════════════════════════════════════════════════════════════════

#[test]
fn inline_one_liner() {
    let program = parse_program("fn id(x: i64) -> i64 { x }\nfn main() { id(1) }");
    let report = optimize_program(&program, OptLevel::O2);
    assert!(report.inline_candidates >= 1);
}

#[test]
fn inline_main_excluded() {
    let program = parse_program("fn main() { 42 }");
    let report = optimize_program(&program, OptLevel::O2);
    assert_eq!(report.inline_candidates, 0);
}

// ════════════════════════════════════════════════════════════════════════
// 6. Compiler profiling
// ════════════════════════════════════════════════════════════════════════

#[test]
fn profile_default_zero() {
    let p = CompileProfile::new();
    assert_eq!(p.total_us, 0);
    assert_eq!(p.lex_us, 0);
    assert_eq!(p.token_count, 0);
}

#[test]
fn profile_bottleneck_identifies_max() {
    let mut p = CompileProfile::new();
    p.parse_us = 500;
    p.codegen_us = 2000;
    p.link_us = 100;
    assert_eq!(p.bottleneck(), "codegen");
}

#[test]
fn profile_bottleneck_lex() {
    let mut p = CompileProfile::new();
    p.lex_us = 9999;
    assert_eq!(p.bottleneck(), "lex");
}

#[test]
fn profile_speed_calculation() {
    let mut p = CompileProfile::new();
    p.source_bytes = 4000; // ~100 lines
    p.total_us = 100_000; // 100ms
    let speed = p.lines_per_second();
    assert!(speed > 900.0 && speed < 1100.0); // ~1000 lines/s
}

#[test]
fn profile_speed_zero_time() {
    let p = CompileProfile::new();
    assert_eq!(p.lines_per_second(), 0.0);
}

#[test]
fn profile_display_contains_phases() {
    let mut p = CompileProfile::new();
    p.lex_us = 100;
    p.parse_us = 200;
    p.analyze_us = 300;
    p.codegen_us = 400;
    let s = format!("{p}");
    assert!(s.contains("lex:"));
    assert!(s.contains("parse:"));
    assert!(s.contains("analyze:"));
    assert!(s.contains("codegen:"));
    assert!(s.contains("bottleneck:"));
}

// ════════════════════════════════════════════════════════════════════════
// 7. Optimization report
// ════════════════════════════════════════════════════════════════════════

#[test]
fn report_display_format() {
    let r = OptimizationReport::new(OptLevel::O2);
    let s = format!("{r}");
    assert!(s.contains("O2"));
    assert!(s.contains("fns analyzed"));
    assert!(s.contains("consts folded"));
    assert!(s.contains("dead fns removed"));
    assert!(s.contains("TCO"));
}

#[test]
fn report_comprehensive_program() {
    let source = r#"
fn tiny() -> i64 { 42 }
fn recursive(n: i64) -> i64 { if n <= 0 { 0 } else { recursive(n - 1) } }
fn dead_fn() { 99 }
fn also_dead() { 100 }
fn main() {
    tiny()
    comptime { 1 + 2 }
}
"#;
    let program = parse_program(source);
    let report = optimize_program(&program, OptLevel::O3);
    assert_eq!(report.total_functions, 5);
    assert!(report.dead_functions_eliminated >= 2); // dead_fn, also_dead, recursive
    assert!(report.tail_calls_optimized >= 1); // recursive
    assert!(report.inline_candidates >= 1); // tiny
    assert!(report.constants_folded >= 1); // comptime
}
