//! Launch readiness tests for Fajar Lang.
//!
//! Verifies all launch content exists and is complete.

use std::path::Path;

// ════════════════════════════════════════════════════════════════════════
// 1. Launch content exists
// ════════════════════════════════════════════════════════════════════════

#[test]
fn blog_post_exists() {
    assert!(Path::new("docs/BLOG_LAUNCH.md").exists());
}

#[test]
fn blog_post_has_content() {
    let content = std::fs::read_to_string("docs/BLOG_LAUNCH.md").unwrap();
    assert!(content.contains("Introducing Fajar Lang"));
    assert!(content.contains("@kernel"));
    assert!(content.contains("@device"));
    assert!(content.contains("@safe"));
    assert!(content.contains("effect"));
    assert!(content.contains("comptime"));
    assert!(content.contains("Getting Started"));
}

#[test]
fn paper_abstract_exists() {
    assert!(Path::new("docs/PAPER_ABSTRACT.md").exists());
}

#[test]
fn paper_abstract_has_content() {
    let content = std::fs::read_to_string("docs/PAPER_ABSTRACT.md").unwrap();
    assert!(content.contains("Abstract"));
    assert!(content.contains("context annotations"));
    assert!(content.contains("algebraic effect"));
    assert!(content.contains("drone controller"));
    assert!(content.contains("PLDI"));
    assert!(content.contains("Key Contributions"));
}

#[test]
fn launch_checklist_exists() {
    assert!(Path::new("docs/LAUNCH_CHECKLIST.md").exists());
}

#[test]
fn launch_checklist_comprehensive() {
    let content = std::fs::read_to_string("docs/LAUNCH_CHECKLIST.md").unwrap();
    assert!(content.contains("Repository"));
    assert!(content.contains("Documentation"));
    assert!(content.contains("Demo"));
    assert!(content.contains("Compiler"));
    assert!(content.contains("IDE Support"));
    assert!(content.contains("Self-Hosting"));
    assert!(content.contains("Community"));
}

// ════════════════════════════════════════════════════════════════════════
// 2. Core docs exist
// ════════════════════════════════════════════════════════════════════════

#[test]
fn claude_md_exists() {
    assert!(Path::new("CLAUDE.md").exists());
}

#[test]
fn changelog_exists() {
    assert!(Path::new("docs/CHANGELOG.md").exists());
}

#[test]
fn error_codes_exists() {
    assert!(Path::new("docs/ERROR_CODES.md").exists());
}

#[test]
fn language_spec_exists() {
    assert!(Path::new("docs/FAJAR_LANG_SPEC.md").exists());
}

#[test]
fn architecture_exists() {
    assert!(Path::new("docs/ARCHITECTURE.md").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 3. Demo and examples
// ════════════════════════════════════════════════════════════════════════

#[test]
fn drone_demo_exists() {
    assert!(Path::new("examples/drone_controller.fj").exists());
}

#[test]
fn hello_world_exists() {
    assert!(Path::new("examples/hello.fj").exists());
}

#[test]
fn examples_dir_has_many_files() {
    let count = std::fs::read_dir("examples")
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|ext| ext == "fj"))
                .unwrap_or(false)
        })
        .count();
    assert!(count >= 100, "should have 100+ examples, got {count}");
}

// ════════════════════════════════════════════════════════════════════════
// 4. Self-hosted compiler
// ════════════════════════════════════════════════════════════════════════

#[test]
fn selfhost_complete() {
    assert!(Path::new("stdlib/lexer.fj").exists());
    assert!(Path::new("stdlib/parser.fj").exists());
    assert!(Path::new("stdlib/analyzer.fj").exists());
    assert!(Path::new("stdlib/codegen.fj").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 5. IDE support
// ════════════════════════════════════════════════════════════════════════

#[test]
fn vscode_extension_exists() {
    assert!(Path::new("editors/vscode/package.json").exists());
}

#[test]
fn vscode_extension_js_exists() {
    assert!(Path::new("editors/vscode/extension.js").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 6. Book
// ════════════════════════════════════════════════════════════════════════

#[test]
fn book_summary_exists() {
    assert!(Path::new("book/src/SUMMARY.md").exists());
}

#[test]
fn book_has_migration_guides() {
    assert!(Path::new("book/src/migration/from-rust.md").exists());
    assert!(Path::new("book/src/migration/from-cpp.md").exists());
    assert!(Path::new("book/src/migration/from-python.md").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 7. Blog post quality
// ════════════════════════════════════════════════════════════════════════

#[test]
fn blog_has_code_examples() {
    let content = std::fs::read_to_string("docs/BLOG_LAUNCH.md").unwrap();
    assert!(content.contains("```fajar"));
    assert!(content.contains("fn read_imu"));
    assert!(content.contains("fn classify"));
}

#[test]
fn blog_has_stats_table() {
    let content = std::fs::read_to_string("docs/BLOG_LAUNCH.md").unwrap();
    assert!(content.contains("| Metric |"));
    assert!(content.contains("5,547"));
    assert!(content.contains("1,268"));
}
