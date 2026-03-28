# Fajar Lang Maintainer Guide

> Operational guide for maintainers responsible for issue triage, code review, releases, and community health.

---

## 1. Issue Triage Process

### When to Triage

All issues with the `needs-triage` label must be triaged within **48 hours** of creation. The triage process runs daily (check the Issues tab filtered by `needs-triage`).

### Triage Steps

1. **Read the issue completely.** Understand the reporter's environment, steps to reproduce, and expected vs. actual behavior.

2. **Verify validity.** Can you reproduce the issue? Is it a known limitation documented in GAP_ANALYSIS_V2.md? Is it a duplicate?

3. **Apply labels.** Remove `needs-triage` and apply the appropriate labels from `.github/labels.yml`:

   | Situation | Labels to Apply |
   |-----------|----------------|
   | Confirmed bug | `bug` + component label (e.g., `parser`, `codegen`) |
   | Feature request | `enhancement` + component label |
   | Newcomer-friendly fix | `good-first-issue` + `help-wanted` |
   | Security issue | `security` (then follow Security Report Handling below) |
   | Documentation gap | `documentation` |
   | Performance regression | `performance` + `bug` |
   | Design discussion needed | `rfc` |
   | Not a bug / out of scope | `wontfix` (with explanation) |
   | Already reported | `duplicate` (link to original) |

4. **Assign priority.** Add a priority label:

   | Priority | Response SLA | Description |
   |----------|-------------|-------------|
   | P0-critical | 24 hours | Compiler crash, data loss, security vulnerability |
   | P1-high | 1 week | Incorrect codegen, wrong type checking, regression |
   | P2-medium | 1 month | Missing feature, poor error message, documentation gap |
   | P3-low | Backlog | Nice-to-have, cosmetic, future enhancement |

5. **Add context.** Leave a comment acknowledging the issue and providing any relevant information (e.g., "This is a known limitation of the current borrow checker -- tracked in V8 Option 0, task BQ3.2").

6. **Assign or tag.** If you know who should work on it, assign them. Otherwise, apply `help-wanted` for community contributions.

### Issue Templates

The project has three issue templates in `.github/ISSUE_TEMPLATE/`:
- `bug_report.yml` -- structured bug report with environment info
- `feature_request.yml` -- feature proposal with use case
- `rfc.md` -- design discussion for significant changes

Encourage reporters to use templates. If an issue doesn't use a template, ask the reporter to provide the missing information before triaging.

---

## 2. Code Review Guidelines

### What to Check

Every pull request must be reviewed against the following checklist before approval:

#### Correctness
- [ ] Does the change do what it claims? Read the PR description and verify against the diff.
- [ ] Are edge cases handled? Check boundary conditions, empty inputs, overflow, null/None.
- [ ] Do the tests actually test the feature? Watch for tests that pass trivially.

#### Safety
- [ ] No `.unwrap()` in `src/` (only allowed in `tests/` and `benches/`).
- [ ] No `unsafe` blocks without `// SAFETY:` comments.
- [ ] No new `unsafe` outside `src/codegen/` and `src/runtime/os/`.

#### Quality
- [ ] `cargo test --lib` passes with 0 failures.
- [ ] `cargo clippy -- -D warnings` reports 0 warnings.
- [ ] `cargo fmt -- --check` reports no formatting issues.
- [ ] All new `pub` items have doc comments.
- [ ] Functions are under 50 lines (per V1_RULES.md).

#### Architecture
- [ ] Dependencies flow downward (lexer <- parser <- analyzer <- interpreter). No upward deps.
- [ ] No cross-dependencies between `runtime/os` and `runtime/ml`.
- [ ] New modules are placed in the correct location per the repository structure.

#### Documentation
- [ ] PR description explains the "why", not just the "what".
- [ ] If behavior changes, relevant docs are updated (CLAUDE.md, STDLIB_SPEC.md, ERROR_CODES.md).
- [ ] Task file updated with accurate status (`[x]` for end-to-end working, `[f]` for framework-only).

### When to Approve

Approve when all checklist items pass and you are confident the change is correct. Do not approve out of politeness or time pressure.

### When to Request Changes

- Any checklist item fails.
- The PR is too large to review confidently (suggest splitting).
- The design approach has a fundamental issue (suggest an alternative, link to relevant code).
- Tests are missing for new functionality.

### Review Etiquette

- Be specific: "Line 42: this `unwrap()` should be `map_err()`" not "fix the error handling."
- Be constructive: suggest solutions, not just problems.
- Be timely: review within 48 hours of assignment, or un-assign yourself.
- Acknowledge good work: "Nice approach to the pattern matching here" costs nothing and builds community.

---

## 3. Release Cutting Process

### Pre-Release Checklist

Before cutting any release:

1. **All tests pass.** Run `cargo test --lib` and `cargo test --features native` (if applicable).
2. **Clippy clean.** Run `cargo clippy -- -D warnings`.
3. **Formatted.** Run `cargo fmt -- --check`.
4. **No regressions.** Run `cargo bench` and compare to previous release.
5. **Changelog updated.** Update `docs/CHANGELOG.md` with all changes since last release.
6. **Version bumped.** Update version in `Cargo.toml`.
7. **Examples work.** Spot-check at least 5 examples with `cargo run -- run examples/<file>.fj`.
8. **Documentation builds.** Run `cargo doc --no-deps` to verify doc generation.

### Release Process

The full release process is documented in `.github/RELEASE_PROCESS.md`. Summary:

1. Create a release branch: `git checkout -b release/vX.Y.Z`
2. Run the pre-release checklist above.
3. Commit version bump and changelog: `git commit -m "chore: prepare vX.Y.Z release"`
4. Tag the release: `git tag -a vX.Y.Z -m "Fajar Lang vX.Y.Z \"<codename>\""`
5. Push branch and tag: `git push origin release/vX.Y.Z --tags`
6. Create GitHub release with changelog excerpt.
7. Merge release branch to `main`.
8. Announce on community channels (GitHub Discussions, Discord, social media).

### Versioning Scheme

- **Major (X):** Breaking language syntax or compiler API changes.
- **Minor (Y):** New features, new compiler passes, new runtime capabilities.
- **Patch (Z):** Bug fixes, documentation, performance improvements.

---

## 4. Security Report Handling

### Receiving Reports

Security vulnerabilities should be reported via:
- GitHub Security Advisories (preferred)
- Email to security@fajarlang.dev
- Private message to a maintainer

**Never** discuss security vulnerabilities in public issues before a fix is available.

### Response Process

| Step | Timeline | Action |
|------|----------|--------|
| 1. Acknowledge | Within 24 hours | Confirm receipt, assign to a maintainer |
| 2. Assess | Within 72 hours | Determine severity (Critical/High/Medium/Low) |
| 3. Fix | Based on severity | Critical: 7 days. High: 14 days. Medium: 30 days. |
| 4. Release | With fix | Patch release with security advisory |
| 5. Disclose | After fix ships | Public advisory with CVE if applicable |

### Severity Classification

| Severity | Description | Example |
|----------|-------------|---------|
| Critical | Remote code execution, compiler generates unsafe code | Buffer overflow in codegen output |
| High | Context isolation bypass, memory safety violation | @device code accessing raw pointers |
| Medium | Information disclosure, denial of service | Compiler crash on crafted input |
| Low | Minor issue, requires unusual conditions | Incorrect error message for edge case |

---

## 5. New Maintainer Onboarding

### Step 1: Contributor Phase (2-4 weeks)

Before becoming a maintainer, candidates must demonstrate:
- At least 5 merged PRs (code, docs, or tests).
- Participation in at least 3 code reviews (as reviewer or reviewee).
- Familiarity with the codebase structure (can navigate src/ without guidance).
- Understanding of the TDD workflow and quality gates.

### Step 2: Mentored Maintainer (4-8 weeks)

New maintainers work alongside an existing maintainer:
- Triage issues together (existing maintainer confirms label choices).
- Review PRs together (new maintainer writes review, existing maintainer validates).
- Cut one release together (new maintainer leads, existing maintainer supervises).
- Read and internalize: CLAUDE.md, V1_RULES.md, GAP_ANALYSIS_V2.md, this guide.

### Step 3: Full Maintainer

After the mentored phase:
- Added to the GitHub team with write access.
- Added to the maintainer communication channel.
- Listed in MAINTAINERS.md (or equivalent).
- Can independently triage, review, merge, and release.

### Maintainer Expectations

- Triage assigned issues within 48 hours.
- Review assigned PRs within 48 hours.
- Participate in release planning (quarterly).
- Follow the Code of Conduct at all times.
- If you need to step back (life happens), communicate proactively and hand off responsibilities.

---

## 6. Operational Runbook

### Common Situations

| Situation | Action |
|-----------|--------|
| CI is red on main | P0: investigate immediately. Revert if fix isn't obvious. |
| Spam issue/PR | Close, lock, and report the account. |
| Heated discussion | De-escalate. Remind participants of Code of Conduct. Lock if needed. |
| Contributor goes silent on PR | Wait 2 weeks, then comment asking for status. After 4 weeks, close with a kind message inviting them to reopen. |
| Dependency CVE | Check if affected, update dependency, cut patch release. |
| Feature request outside scope | Apply `wontfix`, explain the project's focus, suggest a fork or plugin approach. |

### Key Files to Know

| File | Purpose | When to Update |
|------|---------|----------------|
| `CLAUDE.md` | Master reference, auto-loaded by Claude Code | Major changes, new features |
| `docs/GAP_ANALYSIS_V2.md` | Honest codebase audit | Semi-annually or after major releases |
| `docs/CHANGELOG.md` | Version history | Every release |
| `.github/labels.yml` | Label definitions | When adding new labels |
| `Cargo.toml` | Dependencies and version | Every release, dependency updates |
| `docs/V1_RULES.md` | Coding rules | Rarely (rules are stable) |

---

*Maintainer Guide v1.0 -- Fajar Lang Project*
