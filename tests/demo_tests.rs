//! Killer demo integration tests for Fajar Lang.
//!
//! Verifies the drone controller demo showcases all three domains
//! with compiler-enforced context isolation.

use fajar_lang::interpreter::Interpreter;
use std::path::Path;

fn load_demo() -> String {
    std::fs::read_to_string("examples/drone_controller.fj").unwrap()
}

// ════════════════════════════════════════════════════════════════════════
// 1. Demo file structure
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_file_exists() {
    assert!(Path::new("examples/drone_controller.fj").exists());
}

#[test]
fn demo_file_significant_size() {
    let source = load_demo();
    let lines = source.lines().count();
    assert!(lines >= 500, "demo should be 500+ lines, got {lines}");
}

#[test]
fn demo_parses_cleanly() {
    let source = load_demo();
    let tokens = fajar_lang::lexer::tokenize(&source).unwrap();
    let _program = fajar_lang::parser::parse(tokens).unwrap();
}

// ════════════════════════════════════════════════════════════════════════
// 2. Three domains present
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_has_kernel_functions() {
    let source = load_demo();
    assert!(source.contains("@kernel fn"));
    assert!(source.contains("read_imu"));
    assert!(source.contains("set_motor_pwm"));
    assert!(source.contains("emergency_stop"));
    assert!(source.contains("arm_escs"));
}

#[test]
fn demo_has_device_functions() {
    let source = load_demo();
    assert!(source.contains("@device fn"));
    assert!(source.contains("extract_features"));
    assert!(source.contains("classify_object"));
    assert!(source.contains("run_inference"));
}

#[test]
fn demo_has_safe_functions() {
    let source = load_demo();
    assert!(source.contains("fn pid_update"));
    assert!(source.contains("fn plan_next_waypoint"));
    assert!(source.contains("fn avoid_obstacle"));
    assert!(source.contains("fn mix_motors"));
    assert!(source.contains("fn check_battery"));
}

// ════════════════════════════════════════════════════════════════════════
// 3. Effect system used
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_uses_effect_annotations() {
    let source = load_demo();
    assert!(source.contains("with Hardware"));
    assert!(source.contains("with Tensor"));
    // IO appears combined: "with Hardware, IO"
    assert!(source.contains("IO"));
}

// ════════════════════════════════════════════════════════════════════════
// 4. Comptime used
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_uses_comptime() {
    let source = load_demo();
    assert!(source.contains("comptime fn"));
    assert!(source.contains("comptime {"));
}

// ════════════════════════════════════════════════════════════════════════
// 5. Key structures
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_has_drone_structs() {
    let source = load_demo();
    assert!(source.contains("struct DroneState"));
    assert!(source.contains("struct ImuReading"));
    assert!(source.contains("struct MotorOutputs"));
    assert!(source.contains("struct PidState"));
    assert!(source.contains("struct Waypoint"));
    assert!(source.contains("struct Detection"));
    assert!(source.contains("struct Vec3"));
}

// ════════════════════════════════════════════════════════════════════════
// 6. Demo runs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_runs_and_produces_output() {
    let source = load_demo();
    let mut interp = Interpreter::new_capturing();
    interp.eval_source(&source).unwrap();
    interp.call_main().unwrap();
    let output = interp.get_output();

    assert!(!output.is_empty(), "demo should produce output");
    assert!(output.iter().any(|l| l.contains("Drone Controller")));
    assert!(output.iter().any(|l| l.contains("[ARM]")));
    assert!(output.iter().any(|l| l.contains("[DONE]")));
    assert!(output.iter().any(|l| l.contains("[DISARM]")));
    assert!(output.iter().any(|l| l.contains("No other language")));
}

// ════════════════════════════════════════════════════════════════════════
// 7. Cross-domain bridge
// ════════════════════════════════════════════════════════════════════════

#[test]
fn demo_has_bridge_function() {
    let source = load_demo();
    assert!(source.contains("fn control_loop_iteration"));
    // The bridge reads from @kernel, passes to @device, uses @safe logic
    assert!(source.contains("read_imu()"));
    assert!(source.contains("run_inference"));
    assert!(source.contains("apply_motors"));
}

#[test]
fn demo_explains_safety_guarantee() {
    let source = load_demo();
    assert!(source.contains("compiler GUARANTEES"));
    assert!(source.contains("no heap allocation"));
    assert!(source.contains("no raw pointer"));
}
