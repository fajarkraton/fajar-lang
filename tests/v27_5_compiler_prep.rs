//! V27.5 P5: Comprehensive integration tests for compiler prep features.
//!
//! Validates P1-P4 features end-to-end via `.fj` source code compilation
//! and interpreter execution. Serves as regression baseline for V28+.
//!
//! Run: `cargo test --test v27_5_compiler_prep`

use fajar_lang::interpreter::Interpreter;

fn eval(code: &str) -> Result<fajar_lang::interpreter::Value, String> {
    let mut interp = Interpreter::new();
    interp.eval_source(code).map_err(|e| format!("{e:?}"))
}

fn eval_call_main(code: &str) -> Result<fajar_lang::interpreter::Value, String> {
    let mut interp = Interpreter::new();
    interp.eval_source(code).map_err(|e| format!("{e:?}"))?;
    interp.call_main().map_err(|e| format!("{e:?}"))
}

// ═══════════════════════════════════════════════════════════
// P1.2: AI Scheduler Builtins
// ═══════════════════════════════════════════════════════════

#[test]
fn p1_2_tensor_workload_hint_computes_flop_cost() {
    // 4x4 matmul → 4*4*4 = 64 FLOPs
    let result = eval_call_main(r#"fn main() -> i64 { tensor_workload_hint(4, 4) }"#);
    assert_eq!(format!("{:?}", result.unwrap()), "Int(64)");
}

#[test]
fn p1_2_tensor_workload_hint_larger() {
    // 128x128 matmul (Gemma 3 head)
    let result = eval_call_main(r#"fn main() -> i64 { tensor_workload_hint(128, 128) }"#);
    assert_eq!(format!("{:?}", result.unwrap()), "Int(2097152)");
}

#[test]
fn p1_2_schedule_ai_task_priority_ordering() {
    // Higher priority → lower slot number (scheduled sooner)
    let high = eval_call_main(r#"fn main() -> i64 { schedule_ai_task(1, 9, 100) }"#).unwrap();
    let low = eval_call_main(r#"fn main() -> i64 { schedule_ai_task(1, 1, 100) }"#).unwrap();
    let high_s = format!("{high:?}");
    let low_s = format!("{low:?}");
    // Parse the Int(N) value — high priority must come before low
    let high_n: i64 = high_s
        .trim_start_matches("Int(")
        .trim_end_matches(')')
        .parse()
        .unwrap();
    let low_n: i64 = low_s
        .trim_start_matches("Int(")
        .trim_end_matches(')')
        .parse()
        .unwrap();
    assert!(
        high_n < low_n,
        "high priority {high_n} should be < low {low_n}"
    );
}

// ═══════════════════════════════════════════════════════════
// P1.4: VESA Framebuffer Extensions
// ═══════════════════════════════════════════════════════════

#[test]
fn p1_4_fb_set_base_accepted() {
    let result = eval_call_main(r#"fn main() -> i64 { fb_set_base(0xE0000000) }"#);
    assert!(result.is_ok());
}

#[test]
fn p1_4_fb_scroll_accepted() {
    let result = eval_call_main(r#"fn main() -> i64 { fb_init(800, 600); fb_scroll(20) }"#);
    assert!(result.is_ok());
}

#[test]
fn p1_4_fb_full_pipeline() {
    // init → set base → pixel → rect → scroll → all succeed
    let result = eval(
        r#"fn main() {
            fb_init(1920, 1080)
            fb_set_base(0xE0000000)
            fb_write_pixel(100, 100, 0xFF0000)
            fb_fill_rect(200, 200, 50, 50, 0x00FF00)
            fb_scroll(10)
        }"#,
    );
    assert!(result.is_ok());
}

// ═══════════════════════════════════════════════════════════
// P3.1: @app Annotation
// ═══════════════════════════════════════════════════════════

#[test]
fn p3_1_at_app_compiles() {
    let result = eval(r#"@app fn main() -> i64 { 0 }"#);
    assert!(result.is_ok());
}

#[test]
fn p3_1_at_app_runs_like_regular_main() {
    let result = eval_call_main(r#"@app fn main() -> i64 { 42 }"#);
    assert_eq!(format!("{:?}", result.unwrap()), "Int(42)");
}

// ═══════════════════════════════════════════════════════════
// P3.2: @host Annotation
// ═══════════════════════════════════════════════════════════

#[test]
fn p3_2_at_host_compiles() {
    let result = eval(
        r#"@host fn read_source(path: str) -> str { "stub" }
           fn main() { println(read_source("foo.fj")) }"#,
    );
    assert!(result.is_ok());
}

// ═══════════════════════════════════════════════════════════
// P4.1: Refinement Type Parameter Checking
// ═══════════════════════════════════════════════════════════

#[test]
fn p4_1_refinement_param_accepts_valid() {
    let result = eval_call_main(
        r#"fn take_positive(x: { n: i64 | n > 0 }) -> i64 { x }
           fn main() -> i64 { take_positive(5) }"#,
    );
    assert_eq!(format!("{:?}", result.unwrap()), "Int(5)");
}

#[test]
fn p4_1_refinement_param_rejects_invalid() {
    let result = eval_call_main(
        r#"fn take_positive(x: { n: i64 | n > 0 }) -> i64 { x }
           fn main() -> i64 { take_positive(-1) }"#,
    );
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("refinement violation"));
    assert!(err.contains("param"));
}

#[test]
fn p4_1_refinement_let_still_works() {
    // Regression test: let-bind refinement check still functional
    let result = eval_call_main(r#"fn main() -> i64 { let x: { n: i64 | n > 0 } = 10; x }"#);
    assert_eq!(format!("{:?}", result.unwrap()), "Int(10)");
}

// ═══════════════════════════════════════════════════════════
// P4.2: Capability Type Cap<T>
// ═══════════════════════════════════════════════════════════

#[test]
fn p4_2_cap_lifecycle_create_use_consume() {
    let result = eval_call_main(
        r#"fn main() -> i64 {
              let c = cap_new(42)
              cap_unwrap(c)
           }"#,
    );
    assert_eq!(format!("{:?}", result.unwrap()), "Int(42)");
}

#[test]
fn p4_2_cap_is_valid_transitions() {
    // valid before unwrap, invalid after
    let before = eval_call_main(
        r#"fn main() -> i64 {
              let c = cap_new(99)
              cap_is_valid(c)
           }"#,
    )
    .unwrap();
    assert_eq!(format!("{before:?}"), "Int(1)");

    let after = eval_call_main(
        r#"fn main() -> i64 {
              let c = cap_new(99)
              let v = cap_unwrap(c)
              cap_is_valid(c)
           }"#,
    )
    .unwrap();
    assert_eq!(format!("{after:?}"), "Int(0)");
}

#[test]
fn p4_2_cap_double_unwrap_errors() {
    let result = eval_call_main(
        r#"fn main() -> i64 {
              let c = cap_new(99)
              let v1 = cap_unwrap(c)
              let v2 = cap_unwrap(c)
              v2
           }"#,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already consumed"));
}

// ═══════════════════════════════════════════════════════════
// Cross-Feature Integration
// ═══════════════════════════════════════════════════════════

#[test]
fn p_all_features_coexist_in_one_program() {
    // Combine: @app, refinement type, Cap<T>, AI scheduler, framebuffer
    let result = eval_call_main(
        r#"@app fn main() -> i64 {
              // AI scheduler
              let cost = tensor_workload_hint(8, 8)
              // Framebuffer
              fb_init(1920, 1080)
              fb_set_base(0xE0000000)
              // Capability
              let c = cap_new(cost)
              let unwrapped = cap_unwrap(c)
              // Refinement-checked fn call
              check_positive(unwrapped)
           }
           fn check_positive(x: { n: i64 | n > 0 }) -> i64 { x * 2 }"#,
    );
    // cost = 8*8*8 = 512, * 2 = 1024
    assert_eq!(format!("{:?}", result.unwrap()), "Int(1024)");
}
