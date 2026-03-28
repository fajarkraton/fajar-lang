# Fajar Lang Release Process

Checklist for publishing a new Fajar Lang release.

## Pre-Release (1 week before)

- [ ] Announce release freeze on Discord and GitHub Discussions
- [ ] Create `release/vX.Y.Z` branch from `develop`
- [ ] Update version in `Cargo.toml`
- [ ] Update version in `snap/snapcraft.yaml`
- [ ] Update version in `packaging/homebrew/fj.rb`
- [ ] Update version in `packaging/windows/installer.nsi`

## Testing (release branch)

- [ ] Full test suite passes: `cargo test --all-targets`
- [ ] Native codegen tests pass: `cargo test --features native`
- [ ] Clippy clean: `cargo clippy -- -D warnings`
- [ ] Format check: `cargo fmt -- --check`
- [ ] All examples compile and run: `./scripts/test-examples.sh`
- [ ] Cross-compilation check: ARM64 and RISC-V targets build
- [ ] REPL smoke test: start REPL, evaluate basic expressions
- [ ] Binary size check: release build under 10 MB

## Changelog

- [ ] Update `docs/CHANGELOG.md` with all changes since last release
- [ ] Group changes by category: Added, Changed, Fixed, Removed
- [ ] Credit contributors by GitHub handle
- [ ] Note any breaking changes prominently

## Release

- [ ] Merge release branch into `main`
- [ ] Tag release: `git tag -a vX.Y.Z -m "Fajar Lang vX.Y.Z"`
- [ ] Push tag: `git push origin vX.Y.Z`
- [ ] GitHub Actions builds binaries and creates release automatically
- [ ] Verify all platform artifacts are present (Linux, macOS, Windows)
- [ ] Verify SHA256SUMS.txt is attached
- [ ] Download and smoke-test at least one binary

## Post-Release

- [ ] Merge `main` back into `develop`
- [ ] Update Homebrew formula SHA256 hashes
- [ ] Submit snap package: `snapcraft upload`
- [ ] Update Docker Hub image: `docker push fajarlang/fj:vX.Y.Z`
- [ ] Announce on Discord, GitHub Discussions, and social media
- [ ] Update website download links (if applicable)
- [ ] Close the release milestone on GitHub
