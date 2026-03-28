# Fajar Lang RFC Process

RFCs (Requests for Comments) are the mechanism for proposing substantial changes to Fajar Lang.

## When is an RFC Required?

An RFC is required for:
- New language features (syntax, keywords, operators)
- Changes to the type system or safety model
- New context annotations or modifications to existing ones
- Significant standard library additions
- Breaking changes to the CLI or build system

An RFC is NOT required for:
- Bug fixes
- Performance improvements with no API change
- Documentation updates
- Minor tooling improvements

## RFC Lifecycle

### 1. Pre-RFC Discussion
- Open a GitHub Discussion in the "Ideas" category
- Gather initial feedback from the community
- Refine the idea before writing a formal RFC

### 2. Draft RFC
- Fork the repository and create `rfcs/NNNN-short-title.md`
- Use the template below
- Submit as a Pull Request with the `rfc` label

### 3. Review Period (minimum 14 days)
- Community members comment on the PR
- Author addresses feedback and revises the RFC
- Core team members provide technical review

### 4. Final Comment Period (FCP) (7 days)
- A core team member proposes FCP with disposition: merge, close, or postpone
- Last chance for community objections
- Announced in Discord and GitHub Discussions

### 5. Resolution
- **Merged:** RFC is accepted. Implementation can begin. PR is merged.
- **Closed:** RFC is rejected with explanation. May be revisited later.
- **Postponed:** Good idea, but not the right time. Kept open for future.

## RFC Template

```markdown
# RFC NNNN: Title

- **Author:** Your Name (@github)
- **Date:** YYYY-MM-DD
- **Status:** Draft

## Summary
One-paragraph description of the proposal.

## Motivation
Why is this change needed? What problem does it solve?

## Detailed Design
Technical specification of the proposed change.

## Drawbacks
Why should we NOT do this?

## Alternatives
What other approaches were considered?

## Unresolved Questions
What aspects need further discussion?
```

## Numbering

RFCs are numbered sequentially starting from 0001. The number is assigned when the PR is opened.
