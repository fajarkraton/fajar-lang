//! Community + adoption readiness tests for Fajar Lang.
//!
//! Final verification that all community infrastructure exists.

use std::path::Path;

// ════════════════════════════════════════════════════════════════════════
// 1. Repository files
// ════════════════════════════════════════════════════════════════════════

#[test]
fn code_of_conduct_exists() {
    assert!(Path::new("CODE_OF_CONDUCT.md").exists());
}

#[test]
fn dockerfile_exists() {
    assert!(Path::new("Dockerfile").exists());
    let content = std::fs::read_to_string("Dockerfile").unwrap();
    assert!(content.contains("FROM rust"));
    assert!(content.contains("fj"));
    assert!(content.contains("ENTRYPOINT"));
}

#[test]
fn homebrew_formula_exists() {
    assert!(Path::new("packaging/homebrew/fajarlang.rb").exists());
    let content = std::fs::read_to_string("packaging/homebrew/fajarlang.rb").unwrap();
    assert!(content.contains("class Fajarlang"));
    assert!(content.contains("cargo"));
}

#[test]
fn snap_package_exists() {
    assert!(Path::new("packaging/snap/snapcraft.yaml").exists());
    let content = std::fs::read_to_string("packaging/snap/snapcraft.yaml").unwrap();
    assert!(content.contains("name: fj"));
}

// ════════════════════════════════════════════════════════════════════════
// 2. GitHub templates
// ════════════════════════════════════════════════════════════════════════

#[test]
fn bug_report_template_exists() {
    assert!(Path::new(".github/ISSUE_TEMPLATE/bug_report.md").exists());
}

#[test]
fn feature_request_template_exists() {
    assert!(Path::new(".github/ISSUE_TEMPLATE/feature_request.md").exists());
}

#[test]
fn pr_template_exists() {
    assert!(Path::new(".github/PULL_REQUEST_TEMPLATE.md").exists());
}

#[test]
fn ci_workflow_exists() {
    assert!(Path::new(".github/workflows/ci.yml").exists());
}

// ════════════════════════════════════════════════════════════════════════
// 3. Community docs
// ════════════════════════════════════════════════════════════════════════

#[test]
fn community_doc_exists() {
    assert!(Path::new("docs/COMMUNITY.md").exists());
    let content = std::fs::read_to_string("docs/COMMUNITY.md").unwrap();
    assert!(content.contains("Discord"));
    assert!(content.contains("Contributing"));
    assert!(content.contains("Newsletter"));
    assert!(content.contains("Bounty"));
    assert!(content.contains("University"));
}

#[test]
fn beta_program_exists() {
    assert!(Path::new("docs/BETA_PROGRAM.md").exists());
    let content = std::fs::read_to_string("docs/BETA_PROGRAM.md").unwrap();
    assert!(content.contains("Beta Program"));
    assert!(content.contains("Ideal Beta Users"));
    assert!(content.contains("Application"));
}

#[test]
fn certification_roadmap_exists() {
    assert!(Path::new("docs/CERTIFICATION_ROADMAP.md").exists());
    let content = std::fs::read_to_string("docs/CERTIFICATION_ROADMAP.md").unwrap();
    assert!(content.contains("ISO 26262"));
    assert!(content.contains("DO-178C"));
    assert!(content.contains("context isolation"));
}

// ════════════════════════════════════════════════════════════════════════
// 4. Final completeness check
// ════════════════════════════════════════════════════════════════════════

#[test]
fn world_class_plan_exists() {
    assert!(Path::new("docs/WORLD_CLASS_PLAN.md").exists());
}

#[test]
fn all_phases_documented() {
    let plan = std::fs::read_to_string("docs/WORLD_CLASS_PLAN.md").unwrap();
    assert!(plan.contains("Phase 1"));
    assert!(plan.contains("Phase 2"));
    assert!(plan.contains("Phase 3"));
    assert!(plan.contains("Phase 4"));
    assert!(plan.contains("Phase 5"));
    assert!(plan.contains("Phase 6"));
    assert!(plan.contains("Phase 7"));
    assert!(plan.contains("Phase 8"));
}

#[test]
fn contributing_guide_exists() {
    assert!(Path::new("docs/CONTRIBUTING.md").exists());
}
