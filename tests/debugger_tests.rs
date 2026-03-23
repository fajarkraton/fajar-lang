//! Debugger integration tests for Fajar Lang.
//!
//! Tests debug frames, variable display, DWARF mappings,
//! and debugger awareness of effects, comptime, and linear types.

use fajar_lang::debugger::dwarf::DwarfBaseType;
use fajar_lang::debugger::{Breakpoint, DebugFrame, DebugState, DebugVariable, StepMode};

// ════════════════════════════════════════════════════════════════════════
// 1. Debug frames with context annotations
// ════════════════════════════════════════════════════════════════════════

#[test]
fn debug_frame_basic() {
    let frame = DebugFrame::new("main".into(), "main.fj".into(), 1);
    assert_eq!(frame.name, "main");
    assert_eq!(frame.display_name(), "main");
    assert!(frame.context.is_none());
    assert!(frame.effects.is_empty());
    assert!(!frame.is_comptime);
}

#[test]
fn debug_frame_with_kernel_context() {
    let frame = DebugFrame::new("read_hw".into(), "driver.fj".into(), 10).with_context("kernel");
    assert_eq!(frame.context, Some("kernel".into()));
    assert!(frame.display_name().contains("@kernel"));
    assert!(frame.display_name().contains("read_hw"));
}

#[test]
fn debug_frame_with_device_context() {
    let frame = DebugFrame::new("inference".into(), "ml.fj".into(), 20).with_context("device");
    assert!(frame.display_name().contains("@device"));
}

#[test]
fn debug_frame_with_effects() {
    let frame = DebugFrame::new("io_fn".into(), "io.fj".into(), 5)
        .with_effects(vec!["IO".into(), "Alloc".into()]);
    let display = frame.display_name();
    assert!(display.contains("with IO, Alloc"));
}

#[test]
fn debug_frame_comptime() {
    let frame = DebugFrame::new("factorial".into(), "math.fj".into(), 3).as_comptime();
    assert!(frame.is_comptime);
    assert!(frame.display_name().starts_with("comptime"));
}

#[test]
fn debug_frame_full_display() {
    let frame = DebugFrame::new("read_sensor".into(), "hal.fj".into(), 15)
        .with_context("kernel")
        .with_effects(vec!["Hardware".into(), "IO".into()]);
    let display = frame.display_name();
    assert!(display.contains("@kernel"));
    assert!(display.contains("read_sensor"));
    assert!(display.contains("with Hardware, IO"));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Debug variables with metadata
// ════════════════════════════════════════════════════════════════════════

#[test]
fn debug_variable_basic() {
    let var = DebugVariable::new("x".into(), "42".into(), "i64".into());
    assert_eq!(var.name, "x");
    assert_eq!(var.value, "42");
    assert_eq!(var.type_name, "i64");
    assert!(!var.is_comptime);
    assert!(!var.is_linear);
}

#[test]
fn debug_variable_comptime() {
    let var = DebugVariable::new("FACT_10".into(), "3628800".into(), "i64".into()).as_comptime();
    assert!(var.is_comptime);
    let tooltip = var.tooltip();
    assert!(tooltip.contains("[comptime]"));
}

#[test]
fn debug_variable_linear() {
    let var =
        DebugVariable::new("file".into(), "FileHandle(3)".into(), "FileHandle".into()).as_linear();
    assert!(var.is_linear);
    let tooltip = var.tooltip();
    assert!(tooltip.contains("[linear"));
    assert!(tooltip.contains("must be consumed"));
}

#[test]
fn debug_variable_tooltip_format() {
    let var = DebugVariable::new("count".into(), "7".into(), "i64".into());
    assert_eq!(var.tooltip(), "count: i64 = 7");
}

// ════════════════════════════════════════════════════════════════════════
// 3. DWARF type mappings
// ════════════════════════════════════════════════════════════════════════

#[test]
fn dwarf_type_char() {
    let dt = DwarfBaseType::from_fajar_type("char").unwrap();
    assert_eq!(dt.byte_size(), 4);
}

#[test]
fn dwarf_type_tensor() {
    let dt = DwarfBaseType::from_fajar_type("tensor").unwrap();
    assert_eq!(dt.byte_size(), 8); // opaque pointer
}

#[test]
fn dwarf_type_never() {
    let dt = DwarfBaseType::from_fajar_type("never").unwrap();
    assert_eq!(dt.byte_size(), 0);
}

#[test]
fn dwarf_type_all_signed() {
    for (ty, size) in &[("i8", 1), ("i16", 2), ("i32", 4), ("i64", 8), ("i128", 16)] {
        let dt = DwarfBaseType::from_fajar_type(ty).unwrap();
        assert_eq!(dt.byte_size(), *size, "failed for {ty}");
        assert_eq!(dt.encoding(), 0x05); // DW_ATE_signed
    }
}

#[test]
fn dwarf_type_all_unsigned() {
    for (ty, size) in &[("u8", 1), ("u16", 2), ("u32", 4), ("u64", 8), ("u128", 16)] {
        let dt = DwarfBaseType::from_fajar_type(ty).unwrap();
        assert_eq!(dt.byte_size(), *size, "failed for {ty}");
        assert_eq!(dt.encoding(), 0x07); // DW_ATE_unsigned
    }
}

#[test]
fn dwarf_type_floats() {
    let f32_dt = DwarfBaseType::from_fajar_type("f32").unwrap();
    assert_eq!(f32_dt.byte_size(), 4);
    assert_eq!(f32_dt.encoding(), 0x04);

    let f64_dt = DwarfBaseType::from_fajar_type("f64").unwrap();
    assert_eq!(f64_dt.byte_size(), 8);
}

// ════════════════════════════════════════════════════════════════════════
// 4. Breakpoints
// ════════════════════════════════════════════════════════════════════════

#[test]
fn breakpoint_creation() {
    let bp = Breakpoint::new("test.fj".into(), 10);
    assert_eq!(bp.file, "test.fj");
    assert_eq!(bp.line, 10);
    assert!(bp.enabled);
    assert_eq!(bp.hit_count, 0);
}

#[test]
fn breakpoint_conditional() {
    let bp = Breakpoint::new("test.fj".into(), 5).with_condition("x > 10".into());
    assert_eq!(bp.condition, Some("x > 10".into()));
}

#[test]
fn breakpoint_logpoint() {
    let bp = Breakpoint::new("test.fj".into(), 3).with_log_message("hit line 3".into());
    assert_eq!(bp.log_message, Some("hit line 3".into()));
}

// ════════════════════════════════════════════════════════════════════════
// 5. Debug state
// ════════════════════════════════════════════════════════════════════════

#[test]
fn debug_state_initial() {
    let state = DebugState::new();
    assert_eq!(state.step_mode(), StepMode::Continue);
    assert!(state.current_location().is_none());
}

#[test]
fn debug_state_add_breakpoint() {
    let mut state = DebugState::new();
    let bp = Breakpoint::new("test.fj".into(), 10);
    let id = bp.id;
    state.add_breakpoint(bp);
    // Verify breakpoint was added (remove returns true if it existed)
    assert!(state.remove_breakpoint(id));
    // Second remove returns false
    assert!(!state.remove_breakpoint(id));
}

#[test]
fn debug_state_step_modes() {
    let mut state = DebugState::new();
    state.set_step_mode(StepMode::StepIn, 0);
    assert_eq!(state.step_mode(), StepMode::StepIn);
    state.set_step_mode(StepMode::StepOver, 1);
    assert_eq!(state.step_mode(), StepMode::StepOver);
    state.set_step_mode(StepMode::StepOut, 2);
    assert_eq!(state.step_mode(), StepMode::StepOut);
}
