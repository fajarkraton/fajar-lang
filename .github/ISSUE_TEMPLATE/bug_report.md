---
name: Bug Report
about: Report a bug in the Fajar Lang compiler or tools
title: '[BUG] '
labels: bug
assignees: ''
---

## Description

A clear and concise description of the bug.

## Steps to Reproduce

1. Create a file `test.fj` with the following content:

```fajar
// Minimal reproduction code here
```

2. Run the command:

```bash
fj run test.fj
```

3. Observe the error.

## Expected Behavior

What you expected to happen.

## Actual Behavior

What actually happened. Include the full error message and output:

```
Paste error output here
```

## Environment

- **Fajar Lang version:** (output of `fj --version`)
- **OS:** (e.g., Ubuntu 24.04, macOS 15, Windows 11)
- **Architecture:** (e.g., x86_64, aarch64)
- **Rust version:** (output of `rustc --version`, if built from source)
- **Install method:** (cargo install / binary download / from source)

## Minimal Reproduction

Please provide the smallest `.fj` file that reproduces the issue:

```fajar
// Paste minimal .fj code that triggers the bug
```

## Additional Context

- Related issues or PRs (if any)
- Screenshots (if applicable)
- Whether this is a regression (did it work in a previous version?)
