//! Phase E.6 — integration smoke tests against the extracted
//! `fajarkraton/fajar-wasi-p2` crate.
//!
//! These tests pin the public-API contract that `cmd_build_wasi_p2`
//! (src/main.rs) depends on. They mirror the FajarQuant extraction's
//! integ-test pattern, scaled down to the 6 round-trips that cover
//! the component-build path actually exercised by the CLI.
//!
//! If any of these stop compiling or pass-rate drops, the extracted
//! crate's rev pin in Cargo.toml has drifted past the surface used
//! here — file an issue against `fajar-wasi-p2` rather than patching
//! these tests locally.

use fajar_wasi_p2::component::{
    ComponentBuilder, ComponentFuncType, ComponentTypeKind, ComponentValType, ExportKind,
    validate_component,
};

/// E.6.1 — Empty builder produces non-empty bytes that round-trip
/// through `validate_component` without panicking.
#[test]
fn e6_1_component_builder_empty_builds_bytes() {
    let bytes = ComponentBuilder::new().build();
    assert!(!bytes.is_empty(), "empty builder should still emit header");
    let report = validate_component(&bytes).expect("validate should not error on empty");
    assert!(report.magic_valid, "magic bytes must be valid");
    assert!(report.version_valid, "version must be valid");
}

/// E.6.2 — Adding a `Func` type with a `Result_` returns an index that
/// can be re-used in `add_export`. Mirrors lines 5867-5877 of main.rs.
#[test]
fn e6_2_add_func_type_with_result() {
    let mut builder = ComponentBuilder::new();
    let ft = ComponentFuncType {
        name: "run".into(),
        params: Vec::new(),
        result: Some(ComponentValType::Result_ {
            ok: None,
            err: None,
        }),
    };
    let _idx = builder.add_type(ComponentTypeKind::Func(ft));
    let bytes = builder.build();
    let report = validate_component(&bytes).expect("validate after add_type");
    assert!(report.has_type_section, "type section must be present");
}

/// E.6.3 — `add_export` makes the validator report an export section.
#[test]
fn e6_3_add_export_records_in_bytes() {
    let mut builder = ComponentBuilder::new();
    let ft = ComponentFuncType {
        name: "run".into(),
        params: Vec::new(),
        result: Some(ComponentValType::Result_ {
            ok: None,
            err: None,
        }),
    };
    let idx = builder.add_type(ComponentTypeKind::Func(ft));
    builder.add_export("wasi:cli/run", ExportKind::Func, idx);
    let bytes = builder.build();
    let report = validate_component(&bytes).expect("validate after add_export");
    assert!(report.has_export_section, "export section expected");
}

/// E.6.4 — `enable_realloc` flips the public flag queryable via
/// `has_realloc()`. (The flag does not yet affect emitted bytes in
/// v0.1.0 of the extracted crate — emission is a TODO upstream — so
/// this test pins the *flag* contract only; if/when bytes start
/// changing, add a sibling test for that surface.)
#[test]
fn e6_4_enable_realloc_sets_flag() {
    let mut builder = ComponentBuilder::new();
    assert!(!builder.has_realloc(), "default should be off");
    builder.enable_realloc();
    assert!(builder.has_realloc(), "enable_realloc should flip flag");
    let _bytes = builder.build();
}

/// E.6.5 — Full pipeline matches `cmd_build_wasi_p2`'s builder
/// sequence verbatim (lines 5867-5895 of main.rs). The resulting
/// component must validate green with both type and export sections.
#[test]
fn e6_5_validate_full_pipeline_matches_cmd_build_wasi_p2() {
    let mut builder = ComponentBuilder::new();
    let ft = ComponentFuncType {
        name: "run".into(),
        params: Vec::new(),
        result: Some(ComponentValType::Result_ {
            ok: None,
            err: None,
        }),
    };
    let idx = builder.add_type(ComponentTypeKind::Func(ft));
    builder.add_export("wasi:cli/run", ExportKind::Func, idx);
    builder.enable_realloc();
    let bytes = builder.build();

    let report = validate_component(&bytes).expect("validate should succeed");
    assert!(report.valid, "component must validate green");
    assert!(report.magic_valid, "magic_valid");
    assert!(report.version_valid, "version_valid");
    assert!(report.has_type_section, "has_type_section");
    assert!(report.has_export_section, "has_export_section");
    assert!(report.section_count > 0, "section_count > 0");
    assert_eq!(
        report.total_size,
        bytes.len(),
        "total_size must equal byte length"
    );
}

/// E.6.6 — `ExportKind::Func` is the variant fajar-lang uses; assert
/// it is constructible and distinguishable from no-export at the
/// validator level (basic enum sanity to detect API drift).
#[test]
fn e6_6_export_kind_func_round_trip() {
    let mut with_export = ComponentBuilder::new();
    let ft = ComponentFuncType {
        name: "run".into(),
        params: Vec::new(),
        result: Some(ComponentValType::Result_ {
            ok: None,
            err: None,
        }),
    };
    let idx = with_export.add_type(ComponentTypeKind::Func(ft));
    with_export.add_export("test:cli/run", ExportKind::Func, idx);
    let with_bytes = with_export.build();
    let with_report = validate_component(&with_bytes).expect("validate with export");

    let without_bytes = ComponentBuilder::new().build();
    let without_report = validate_component(&without_bytes).expect("validate without export");

    assert!(with_report.has_export_section);
    assert!(!without_report.has_export_section);
}
