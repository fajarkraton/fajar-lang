//! Release workflow validation — P7.F1 of FAJAR_LANG_PERFECTION_PLAN.
//!
//! Plan §4 P7 F1 PASS criterion: "GitHub Releases v32.x has linux + mac
//! + windows binaries attached".
//!
//! The release.yml workflow auto-builds + auto-publishes binaries when
//! a `v*.*.*` tag lands on origin. These tests validate the workflow's
//! structure so accidental edits that would silently break the release
//! pipeline are caught before merge.

use std::fs;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(project_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

#[test]
fn f1_release_workflow_exists() {
    let path = project_root().join(".github/workflows/release.yml");
    assert!(
        path.exists(),
        "release.yml workflow missing — binary distribution would break"
    );
}

#[test]
fn f1_release_workflow_triggers_on_version_tags() {
    let yml = read(".github/workflows/release.yml");
    assert!(
        yml.contains("push:") && yml.contains("tags:"),
        "release.yml must trigger on push:tags"
    );
    assert!(
        yml.contains("v*.*.*") || yml.contains("v*"),
        "release.yml must filter for version-tag pattern (v*.*.*)"
    );
}

#[test]
fn f1_release_workflow_builds_5_platforms() {
    let yml = read(".github/workflows/release.yml");
    let required_targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ];
    for tgt in &required_targets {
        assert!(
            yml.contains(tgt),
            "release.yml missing target {tgt} — at least one OS would have no binary"
        );
    }
}

#[test]
fn f1_release_workflow_publishes_via_gh_release() {
    let yml = read(".github/workflows/release.yml");
    assert!(
        yml.contains("softprops/action-gh-release"),
        "release.yml must use action-gh-release to publish (or another\
         documented release-publishing action)"
    );
    assert!(
        yml.contains("GITHUB_TOKEN"),
        "release.yml must pass GITHUB_TOKEN to the release-publish step"
    );
}

#[test]
fn f1_release_workflow_uploads_archives() {
    let yml = read(".github/workflows/release.yml");
    assert!(
        yml.contains("tar.gz"),
        "release.yml must produce .tar.gz archives for unix targets"
    );
    assert!(
        yml.contains(".zip") || yml.contains("zip"),
        "release.yml must produce .zip archives for windows"
    );
}

#[test]
fn f1_release_workflow_runs_llvm_verification() {
    // The release pipeline should NOT publish if LLVM backend fails —
    // production binaries depend on LLVM-O3 codegen being healthy.
    let yml = read(".github/workflows/release.yml");
    assert!(
        yml.contains("llvm-check") || yml.contains("--features llvm"),
        "release.yml must run LLVM verification before publishing"
    );
    assert!(
        yml.contains("needs: [build, llvm-check]") || yml.contains("needs: [llvm-check, build]"),
        "the release-publish job must depend on llvm-check passing"
    );
}

#[test]
fn f1_release_workflow_emits_checksums() {
    let yml = read(".github/workflows/release.yml");
    assert!(
        yml.contains("sha256sum") || yml.contains("SHA256SUMS"),
        "release.yml must emit SHA-256 checksums so users can verify\
         binary integrity"
    );
}

#[test]
fn f1_cargo_toml_version_matches_tag_format() {
    // The version in Cargo.toml must be in semver-3 form so it matches
    // the v*.*.* tag pattern that triggers the workflow. A version of
    // "32" or "32.1" would not be tagged as v32.1.0 and the workflow
    // would never fire.
    let cargo = read("Cargo.toml");
    let version_line = cargo
        .lines()
        .find(|l| l.starts_with("version"))
        .expect("Cargo.toml must declare version");
    let v = version_line
        .split('"')
        .nth(1)
        .expect("Cargo.toml version must be quoted");
    let parts: Vec<&str> = v.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "Cargo.toml version `{v}` must be MAJOR.MINOR.PATCH (3 parts) for v*.*.* tag matching"
    );
    for (i, part) in parts.iter().enumerate() {
        assert!(
            part.parse::<u64>().is_ok(),
            "version part {i} (`{part}`) must be numeric"
        );
    }
}
