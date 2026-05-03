# crates.io publish unblock plan — P7.F3

> **Status (2026-05-03):** documented + validation script shipped.
> Full closure requires fajarquant repo coordination (separate repo)
> + decision on cranelift-object patch.

## Why this doc exists

`fajar-lang` cannot currently be published to crates.io because of
two `Cargo.toml` constraints. This doc names them and gives the
mechanical sequence to unblock.

## Blocker 1 — `fajarquant` is a git dep

`Cargo.toml` line 20:
```toml
fajarquant = { git = "https://github.com/fajarkraton/fajarquant",
                rev = "b05ecf1705ba79145049373510ad63f42ab5efbe" }
```

`cargo publish` rejects git deps for crates uploaded to crates.io —
the registry only resolves dependencies from crates.io itself. Any
git/path dep must be replaced with a published version.

**Closure sequence:**
1. **In the `fajarkraton/fajarquant` repo:**
   - Bump `Cargo.toml` version to next public release (suggested
     `fajarquant = "0.4.0"` matching the V26 A4 split tag).
   - Run `cargo publish --dry-run` to validate.
   - `cargo publish` to upload.
2. **In this repo (`fajar-lang`):**
   - Edit `Cargo.toml` line 20:
     ```toml
     fajarquant = "0.4.0"     # was: git = ".../fajarquant", rev = "b05ecf17..."
     ```
   - Run `cargo update -p fajarquant` to regenerate Cargo.lock.
   - Run `cargo test --lib` to confirm no API drift between the
     pinned rev and the published 0.4.0.
   - Update `bash scripts/check_publish_ready.sh` baseline if needed.

**Cross-repo coordination required:** founder + fajarquant maintainer
agreement on the version number + ABI freeze for that release.

## Blocker 2 — `cranelift-object` is a `[patch.crates-io]`

`Cargo.toml` line 149:
```toml
[patch.crates-io]
cranelift-object = { path = "patches/cranelift-object" }
```

`[patch.crates-io]` is *technically* permitted in a published crate's
`Cargo.toml`, but it is NOT honored by downstream consumers — anyone
adding `fajar-lang = "32.x"` as a dep will pull the upstream
`cranelift-object` from crates.io, not our local patch. Two paths:

**Option A — drop the patch (preferred for publish).**
- If the patch was a workaround for a now-fixed upstream bug,
  remove the `[patch.crates-io]` block entirely and pin to whatever
  upstream `cranelift-object` version is current.
- Smoke-test the LLVM/Cranelift backends to confirm the workaround
  is no longer needed.

**Option B — fork as a separate crate name.**
- Rename the patched crate to e.g. `cranelift-object-fajar` and
  publish it standalone.
- Update `fajar-lang` Cargo.toml to depend on the fork by name
  (no `[patch]`).

`patches/cranelift-object/README.md` has no FAJAR-specific notes
explaining the original divergence reason; recovering this via
`git log --diff -- patches/cranelift-object/` should surface the
intent.

## Blocker 3 — descriptive metadata for crates.io listing

For a clean listing, ensure `Cargo.toml` has:
- `description` ✓ (already present)
- `license = "Apache-2.0"` ✓ (already present)
- `repository = "https://github.com/fajarkraton/fajar-lang"` (verify)
- `readme = "README.md"` (verify)
- `keywords = [...]` (≤5)
- `categories = [...]` (≤5; e.g. compilers, machine-learning, embedded)

`scripts/check_publish_ready.sh` validates these fields exist.

## Validation script

`scripts/check_publish_ready.sh` (P7.F3 prevention layer per §6.8 R3):
- Detects `git` / `path` deps in `[dependencies]` and `[dev-dependencies]`
- Detects `[patch.crates-io]` blocks
- Verifies required `Cargo.toml` metadata fields
- Exit 0 = ready to publish; non-zero = list of blockers

Run: `bash scripts/check_publish_ready.sh`

The script's exit code is the blocker count; `0` means `cargo publish
--dry-run` should succeed.

## Honest scope

This doc + the validation script are the **engineering-side closure**
of P7.F3. The actual publish step requires:
- founder-side action on the `fajarquant` repo (out of this repo's
  scope)
- decision on cranelift-object patch (Option A or B above)

Both are tractable from the contributor side; neither involves novel
work. P7.F3 is documented + scripted; mechanical closure deferred
until founder window opens.
