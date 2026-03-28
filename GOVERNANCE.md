# Governance Model

## Project Leadership

**BDFL (Benevolent Dictator For Life):** Muhamad Fajar Putranto, SE., SH., MH.

The BDFL has final authority on all project decisions, including language design, major architectural changes, and release schedules.

## Core Maintainers

Core maintainers are trusted contributors with merge permissions. They are responsible for:
- Reviewing and merging pull requests
- Triaging issues and bug reports
- Enforcing code quality standards (V1_RULES.md)
- Mentoring new contributors

### Becoming a Core Maintainer
1. Sustained, high-quality contributions over 6+ months
2. Demonstrated understanding of the codebase and design principles
3. Nomination by an existing maintainer + BDFL approval

## Decision-Making Process

### Minor Changes (Bug fixes, docs, small features)
- Single maintainer approval required
- Must pass CI (tests, clippy, fmt)

### Significant Changes (New language features, API changes)
- RFC required (see community/RFC_PROCESS.md)
- 14-day review period
- 7-day Final Comment Period (FCP)
- BDFL approval for language changes

### Breaking Changes
- RFC with explicit migration guide
- Minimum 1 release cycle deprecation warning
- BDFL approval required

## Release Cycle

| Release | Frequency | Scope |
|---------|-----------|-------|
| Patch (x.y.Z) | As needed | Bug fixes only |
| Minor (x.Y.0) | Monthly | New features, backward compatible |
| Major (X.0.0) | Quarterly | Breaking changes allowed |

See community/RELEASE_PROCESS.md for detailed checklist.

## Code of Conduct

All participants must follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Communication Channels

- **GitHub Issues** — Bug reports, feature requests
- **GitHub Discussions** — Q&A, ideas, general discussion
- **RFCs** — Language and compiler design proposals
