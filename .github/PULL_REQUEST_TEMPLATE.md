## Summary

<!-- Brief description of what this PR does and why. -->

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Refactor (non-breaking change that restructures code without changing behavior)
- [ ] Documentation (changes to docs, comments, or examples)
- [ ] Performance (optimization without behavior change)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)

## Changes

- Change 1
- Change 2

## Related Issues

<!-- Link related issues: Fixes #123, Closes #456, Related to #789 -->

## Test Plan

<!-- How was this tested? What commands should a reviewer run? -->

```bash
cargo test -- test_name_here
```

## Quality Checklist

- [ ] `cargo test` -- all tests pass
- [ ] `cargo clippy -- -D warnings` -- zero warnings
- [ ] `cargo fmt -- --check` -- code is formatted
- [ ] No `.unwrap()` in `src/` (only allowed in `tests/`)
- [ ] No `unsafe` without `// SAFETY:` comment
- [ ] All new `pub` items have doc comments (`///`)
- [ ] New functions have at least one test
- [ ] Commit messages follow convention: `type(scope): description`

## Documentation

- [ ] Updated relevant docs (if applicable)
- [ ] Updated CHANGELOG.md (if applicable)
- [ ] No inflated claims -- feature works end-to-end, not just framework

## Screenshots / Output

<!-- If applicable, paste compiler output, test results, or screenshots. -->
