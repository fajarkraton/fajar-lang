//! Indirect lock against T4 regression: greps `stdlib/analyzer.fj`
//! source for the old placeholder body `f"var_{idx}"`. If that
//! string ever reappears, T4 dup-fn detection breaks again because
//! `scope_contains` only collides on identical strings — the
//! placeholders only collide by index, never by name.
//!
//! Source of truth: `docs/T4_DUP_FN_PLAN.md` §3.2 + resume protocol
//! `memory/project_resume_lanjut_protocol.md` step T4 A4.

#[test]
fn analyzer_extract_ident_returns_real_text_not_placeholder() {
    let source =
        std::fs::read_to_string("stdlib/analyzer.fj").expect("cannot read stdlib/analyzer.fj");
    // Ignore occurrences inside `//` comment lines (we may legitimately
    // mention the old placeholder by name in docstrings explaining the
    // T4 fix). Any non-comment occurrence is a regression.
    let needle = r#"f"var_{idx}""#;
    let offending: Vec<(usize, &str)> = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(i, line)| (i + 1, line))
        .collect();
    assert!(
        offending.is_empty(),
        "extract_ident reverted to placeholder — T4 dup-fn detection broken; \
         see docs/T4_DUP_FN_PLAN.md §3.2 + memory/project_resume_lanjut_protocol.md.\n\
         Offending lines: {offending:?}"
    );
}
