# Editions & Stability

Fajar Lang uses editions to evolve the language without breaking existing code.

## Editions

| Edition | Year | Changes |
|---------|------|---------|
| `2025` | Stable | Initial stable semantics |
| `2026` | Current | New keywords reserved, deprecated features removed |

Specify the edition in `fj.toml`:

```toml
[package]
edition = "2025"
```

Old editions continue to compile. New editions can introduce breaking changes (new keywords, removed syntax) without affecting existing projects.

## Stability Levels

| Level | Meaning |
|-------|---------|
| `Stable` | Guaranteed to work across editions |
| `Unstable` | May change — requires `#[allow(unstable)]` |
| `Deprecated` | Still works but will be removed in next edition |

```fajar
#[deprecated(since = "0.9.0", note = "use new_api() instead")]
fn old_api() { ... }
```

## Feature Gates

Unstable features are behind feature gates:

```fajar
#![feature(dependent_types)]

fn safe_index<N: Nat>(arr: DependentArray<i64, Succ(N)>, i: usize) -> i64 {
    // ...
}
```

## API Stability Checking

```bash
fj api-diff v2.0.0..v3.0.0
```

Detects breaking changes:
- Removed public functions/types
- Changed function signatures
- Narrowed trait implementations

Validates that version bumps follow SemVer (major for breaking, minor for additive, patch for fixes).

## Migration Tool

```bash
fj migrate --edition 2026
```

Automatically applies migration suggestions:
- Rename reserved keywords used as identifiers
- Update deprecated API calls
- Add required annotations
