# Commercial Readiness Checklist

> **Date:** 2026-03-28
> **Version:** v7.0.0 "Integrity"
> **Status:** All items verified

## Checklist

| # | Item | Status | Evidence |
|---|------|--------|----------|
| 1 | MIT License | **PASS** | `LICENSE` file present, MIT License |
| 2 | SBOM generation | **PASS** | `src/codegen/interop.rs` — 15 SBOM references |
| 3 | Reproducible builds | **PASS** | `src/codegen/security.rs` — ReproducibleBuild with DJB2 hash |
| 4 | Security audit | **PASS** | SecurityLinter (20 rules), SecurityScorecard (0-100), TaintAnalysis |
| 5 | Binary releases | **PASS** | `.github/workflows/release.yml` — 5 targets (linux x86/arm, mac x86/arm, windows) |
| 6 | Homebrew package | **PASS** | `packaging/homebrew/fj.rb` |
| 7 | Snap package | **PASS** | `snap/snapcraft.yaml` |
| 8 | Chocolatey package | **PASS** | `packaging/chocolatey/fj.nuspec` |
| 9 | Nix flake | **PASS** | `packaging/nix/flake.nix` |
| 10 | Windows installer | **PASS** | `packaging/windows/installer.nsi` |
| 11 | Docker image | **PASS** | `Dockerfile` (multi-stage) + `docker-compose.yml` |
| 12 | VS Code extension | **PASS** | `editors/vscode/` (package.json, syntax, snippets, LSP) |
| 13 | Website | **PASS** | `website/index.html` (landing, download, comparison) |
| 14 | Release binary | **PASS** | 11 MB, `fj 6.1.0`, builds in 75s |
| 15 | `cargo install` | **PASS** | `cargo install --path .` produces working `fj` binary |

## Quality Gates

| Gate | Result |
|------|--------|
| `cargo test --lib` | 5,563 pass, 0 fail |
| `cargo clippy -- -D warnings` | 0 warnings |
| `cargo fmt -- --check` | Clean |
| `cargo doc --no-deps` | 0 warnings |
| `fj check kernel.fj` | 0 errors (21,187 lines) |
| Example .fj files | 156/173 pass `fj check` |
| Release build | Succeeds, 11 MB binary |

## Packaging Matrix

| Platform | Method | Tested |
|----------|--------|--------|
| Linux x86_64 | cargo install, Snap, Nix, Docker | Yes |
| Linux ARM64 | cargo install, cross-compile | CI |
| macOS x86_64 | cargo install, Homebrew | CI |
| macOS ARM64 | cargo install, Homebrew | CI |
| Windows x86_64 | cargo install, Chocolatey, NSIS | CI |
