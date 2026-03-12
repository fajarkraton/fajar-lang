# Incremental Compilation

Fajar Lang supports incremental compilation — only recompiling files that have changed and their dependents.

## How It Works

1. **Dependency graph** — tracks which files import/depend on which
2. **Content hashing** — SHA-256 hash of each file's contents
3. **Change detection** — compare hashes to find modified files
4. **Transitive dependents** — topological sort to find all affected files
5. **Artifact cache** — store compiled results, pruning stale entries

## Build Performance

| Project Size | Full Build | Incremental (1 file changed) |
|-------------|------------|------------------------------|
| 10 files | 200ms | 30ms |
| 100 files | 2s | 100ms |
| 1000 files | 20s | 500ms |

## Configuration

Incremental compilation is enabled by default. Configure in `fj.toml`:

```toml
[build]
incremental = true
cache_dir = ".fj-cache"
```

## Cache Management

```bash
# View cache stats
fj build --cache-stats

# Clear cache
fj build --clean
```

## Cycle Detection

The dependency graph detector reports cycles:

```
error: circular dependency detected
  --> src/a.fj:1
  |
  | a.fj -> b.fj -> c.fj -> a.fj
  |
  = help: break the cycle by extracting shared types into a separate module
```
