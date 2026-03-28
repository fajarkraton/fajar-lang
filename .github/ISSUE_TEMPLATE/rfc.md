---
name: RFC (Request for Comments)
about: Propose a significant design change or new language feature
title: '[RFC] '
labels: rfc
assignees: ''
---

## Summary

One-paragraph explanation of the proposed change.

## Motivation

Why are we doing this? What use cases does it support? What is the expected outcome?

Describe the problem this RFC solves. If there are existing workarounds, explain why they are insufficient.

## Detailed Design

Describe the design in enough detail that someone familiar with Fajar Lang could implement it. Include:

- **Syntax changes** (if any):

```fajar
// Proposed syntax
```

- **Semantic rules:** How does the type checker handle this? Which contexts (@safe, @kernel, @device) allow it?

- **Runtime behavior:** What happens at runtime? Any performance implications?

- **Error messages:** What new error codes are needed? Example diagnostics?

- **Standard library changes:** Does this require new builtins or stdlib modules?

## Drawbacks

Why should we NOT do this? Consider:

- Complexity added to the compiler
- Learning curve for users
- Interaction with existing features
- Performance cost

## Alternatives

What other designs have been considered? Why is this design the best choice?

1. **Alternative A:** Description and trade-offs.
2. **Alternative B:** Description and trade-offs.
3. **Do nothing:** What happens if we don't implement this?

## Prior Art

How have other languages solved this problem?

- **Rust:** ...
- **Zig:** ...
- **Mojo:** ...
- **Other:** ...

## Unresolved Questions

What aspects of the design are still TBD? What do we need to figure out during implementation?

1. ...
2. ...

## Implementation Plan

If you have thoughts on implementation, outline the steps:

- [ ] Step 1
- [ ] Step 2
- [ ] Step 3
