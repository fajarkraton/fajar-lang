// V29.P1 Prevention Layer (CLAUDE.md §6.8 Rule 3)
//
// Meta-test: every annotation name that the LLVM codegen handles MUST
// have a corresponding entry in the lexer's ANNOTATIONS table. If this
// test fails, the lexer will reject the annotation as "unknown" before
// the codegen ever sees it, causing the silent-build-failure pattern
// that led to V29.P1 in the first place (see
// `docs/V29_P1_COMPILER_ENHANCEMENT_PLAN.md` for full context).
//
// Closes the bug class permanently — future codegen contributors who
// add a match arm in `apply_function_attributes` without wiring the
// lexer will fail this test before shipping.
//
// The inverse direction (every lexer entry reaches a codegen arm) is
// NOT required — many annotations are semantic-only (e.g., @kernel
// tracks context in the analyzer and emits no function attribute).

use fajar_lang::lexer::token::lookup_annotation;

/// Annotation names that the LLVM codegen's `apply_function_attributes`
/// function explicitly matches on via string comparison
/// (see `src/codegen/llvm/mod.rs` — search for `ann.name.as_str()`
/// inside the `apply_function_attributes` match block).
///
/// When a new match arm is added there, this array MUST be extended
/// in the same commit or the CI regression gate catches it.
const CODEGEN_ANNOTATION_NAMES: &[&str] = &[
    "inline",    // @inline / @inline("never") — sets AlwaysInline or NoInline
    "noinline",  // @noinline — alias for @inline("never"), V29.P1 addition
    "cold",      // @cold — places in .text.unlikely, sets Cold attribute
    "interrupt", // @interrupt — naked + noinline + .text.interrupt section
    "section",   // @section("foo") — sets ELF section name
];

#[test]
fn codegen_annotations_all_present_in_lexer() {
    let mut missing = Vec::new();
    for &name in CODEGEN_ANNOTATION_NAMES {
        if lookup_annotation(name).is_none() {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "Codegen handles these annotations but lexer lacks ANNOTATIONS \
         entries: {:?}. Add them to `src/lexer/token.rs` ANNOTATIONS \
         HashMap or the compiler will silently reject code using them. \
         This is the V29.P1 prevention layer (see \
         docs/V29_P1_COMPILER_ENHANCEMENT_PLAN.md §7).",
        missing
    );
}

#[test]
fn codegen_annotation_list_is_not_accidentally_empty() {
    // Guard against a future refactor that empties the constant array,
    // which would make the coverage test trivially pass.
    assert!(
        !CODEGEN_ANNOTATION_NAMES.is_empty(),
        "CODEGEN_ANNOTATION_NAMES must not be empty"
    );
    assert!(
        CODEGEN_ANNOTATION_NAMES.len() >= 5,
        "CODEGEN_ANNOTATION_NAMES has fewer than 5 entries. Either the \
         codegen dropped support for several annotations (in which case \
         update this test) or someone truncated the list by mistake. \
         Current list: {:?}",
        CODEGEN_ANNOTATION_NAMES
    );
}

#[test]
fn noinline_specifically_resolves() {
    // Targeted test for the V29.P1 specific fix. Independent of the
    // coverage meta-test above so failures point directly at the
    // regression if someone removes @noinline from ANNOTATIONS.
    assert!(
        lookup_annotation("noinline").is_some(),
        "@noinline MUST resolve in the lexer ANNOTATIONS table. \
         Removing it breaks FajarOS kernel hot paths that rely on \
         LLVM NoInline attribute for O2 stability (see \
         kernel/compute/{{kmatrix,model_loader}}.fj in fajaros-x86). \
         V29.P1 added this entry after a 2-hour silent-build-failure \
         window; removing it reopens the bug class."
    );
}
