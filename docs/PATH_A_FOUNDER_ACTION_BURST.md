# Path A — Founder Action Burst

> **Scope:** sequence the 4 remaining HONEST_AUDIT_V33 items that
> require founder external action (not engineering work) into a
> single copy-paste-able execution checklist. Engineering-side prep
> for each item is already in repo; only the external steps remain.
>
> **Why this doc exists:** HONEST_AUDIT_V33.md §11 documents F1, F3,
> and A1 as "engineering-side closed but blocked on founder external
> action." Plus v36.0.0 tag deferred at Phase G.3 (2026-05-13).
> Founder's window opening = these 4 close together.
>
> **Time estimate:** 30-60 min user-time + ~15 min CI wait for
> Phase 1; Phase 2 spans 4 cross-repo `cargo publish` calls; Phase 3
> is a single GitHub issue file.

---

## Phase 1 — `v36.0.0` tag + F1 binaries (combined; ~10 min user + ~15 min CI)

`.github/workflows/release.yml` triggers on tag push `v*.*.*`. Tag
the release → workflow builds 5 binaries (Linux x86_64 / Linux
aarch64 / macOS x86_64 / macOS arm64 / Windows MSVC) + uploads them
with `softprops/action-gh-release@v2` + auto-generates release
notes. **F1 closes automatically once the workflow lands.**

### Step 1.1 — Bump Cargo.toml version

Cargo.toml line 3 currently says `version = "35.0.0"` (one minor
behind tagged v35.6.0; major-level sync passes but exact version
needs bump for a MAJOR release).

```bash
cd "/home/primecore/Documents/Fajar Lang"
sed -i 's/^version = "35\.0\.0"$/version = "36.0.0"/' Cargo.toml
grep -nE "^version " Cargo.toml  # expect: 3:version = "36.0.0"
bash scripts/check_version_sync.sh  # expect: PASS (major 36) — WILL FAIL until CLAUDE.md updated; see step 1.2
```

### Step 1.2 — Update CLAUDE.md version string

Footer line says `*CLAUDE.md Version: 35.6 + EOS-40 stats refresh …*`.
Bump to 36.0 for the tag.

```bash
# Single edit in CLAUDE.md footer + opening header reference (~2 lines)
# Suggested: leave §3 stats unchanged (already post-EOS-40); just
# update the footer line:
sed -i 's/CLAUDE\.md Version: 35\.6/CLAUDE.md Version: 36.0/' CLAUDE.md
bash scripts/check_version_sync.sh  # expect: PASS (major 36)
```

### Step 1.3 — Commit the version bump

```bash
git add Cargo.toml CLAUDE.md
git commit -m "$(cat <<'EOF'
chore(release): bump version 35.0.0 → 36.0.0 for Path E + F closure tag

v36.0.0 MAJOR bump justified by:
- src/wasi_p2/ + src/distributed/ removed from fajar-lang core
  (Compass §5 Path E + F extraction, EOS-29..40)
- cmd_run_cluster CLI subcommand removed at F.4 per §5.1 Option α
- 2 user-facing CLI surface changes → SemVer MAJOR

Source of truth: docs/COMPASS_5_PATH_E_F_EXTRACTION_FINDINGS.md +
CHANGELOG.md [Unreleased] block.
EOF
)"
git push
```

### Step 1.4 — Tag + push tag

```bash
git tag -a v36.0.0 -m "$(cat <<'EOF'
v36.0.0 "Compass §5 closure" — wasi_p2 + distributed extracted

Removes the local src/wasi_p2/ + src/distributed/ subsystems and
re-homes them as standalone Apache-2.0 crates under
fajarkraton/fajar-wasi-p2 + fajarkraton/fajar-distributed. fj-lang
now depends on them via rev-pinned Cargo git deps.

MAJOR bump justified by removal of `fj run-cluster` CLI subcommand
(Compass §5.1 Option α) + the `fj build --target wasm32-wasi-p2`
deprecation warning (Option γ; hard-removed in v37).

Stats: -29.6K LOC Rust net (across 28 source files); 7,211 → 6,591
lib tests; 39 → 38 CLI subcommands; 42 → 40 root pub mods.

Full release notes: docs/V36_0_0_RELEASE_NOTES.md.
Details: CHANGELOG.md [Unreleased] block; full closure findings in
docs/COMPASS_5_PATH_E_F_EXTRACTION_FINDINGS.md.
EOF
)"
git push origin v36.0.0
```

### Step 1.4b — Replace auto-generated release notes with curated draft

`release.yml` uses `generate_release_notes: true` which produces a
commit-list-based body. Replace with the curated v36.0.0 notes
(mirrors v35.6.0 release pattern):

```bash
# After release.yml workflow finishes + creates the v36.0.0 release:
gh release edit v36.0.0 \
  --repo fajarkraton/fajar-lang \
  --notes-file docs/V36_0_0_RELEASE_NOTES.md
# Verify:
gh release view v36.0.0 --repo fajarkraton/fajar-lang | head -50
```

### Step 1.5 — Verify CI

```bash
# Open GitHub Actions tab in browser, or:
gh run list --workflow=release.yml --limit 3
# Wait for the "Release" workflow to finish (~10-15 min for the
# matrix of 5 platforms + llvm-check job).

# Once green, verify the release page:
gh release view v36.0.0
# Expect: 5 .tar.gz/.zip files + SHA256SUMS.txt + auto-gen release notes.
```

### Step 1.6 — Update HONEST_AUDIT_V33.md F1 row + this doc

After binaries land, edit `docs/HONEST_AUDIT_V33.md` line 92:

```diff
- | F1 | Binary distribution | ✅ engineering-side | `cargo test --release --test release_workflow` (8 PASS) — v32.1.0 binaries pending GitHub Actions runtime |
+ | F1 | Binary distribution | ✅ CLOSED v36.0.0 (5 binaries on Releases) | `cargo test --release --test release_workflow` (8 PASS) + `gh release view v36.0.0` shows 5 .tar.gz/.zip + SHA256SUMS |
```

Plus update §11 "Engineering-side closures awaiting founder action":
remove F1 from the list; bump scorecard `22 of 25 → 23 of 25 PASS`.

**Phase 1 closes:** F1 + v36.0.0 tag. **Two items down, two to go.**

---

## Phase 2 — F3 crates.io publish chain (~45-60 min; 4 cross-repo publishes)

`scripts/check_publish_ready.sh` reports **3 git deps** + **1 patch
block** that need resolution before `cargo publish` works on
fajar-lang itself. Each git dep needs its source crate published
first.

### Step 2.0 — Pre-flight live state check

```bash
bash scripts/check_publish_ready.sh
# Expect (post-regex-fix this session):
# FAIL — 8 blocker line(s):
#   git deps (crates.io rejects these):
#       24:fajarquant = { git = "...", rev = "b05ecf17..." }
#       32:fajar-wasi-p2 = { git = "...", rev = "d57d3b21..." }
#       33:fajar-distributed = { git = "...", rev = "4011a3d5..." }
#   [patch.crates-io] block present:
#       line 159:[patch.crates-io]
```

### Step 2.1 — Publish `fajar-wasi-p2` (new, never published)

```bash
cd ~/Documents/  # check if locally cloned; if not:
# (skip if already present from EOS-38 repo creation)
gh repo clone fajarkraton/fajar-wasi-p2
cd fajar-wasi-p2

# Verify metadata
grep -E "^(name|version|license|description|repository)" Cargo.toml
# Expect: name = "fajar-wasi-p2"; version = "0.1.0"; license = "Apache-2.0".
# If missing description/repository/keywords/categories, add them per
# fajar-lang's Cargo.toml pattern (lines 5-11).

# Dry-run
cargo publish --dry-run --allow-dirty
# Fix any reported issues. Then:
cargo publish
# Wait ~30s for crates.io indexing.
```

### Step 2.2 — Publish `fajar-distributed` (new, never published)

```bash
cd ~/Documents/  # check if locally cloned
gh repo clone fajarkraton/fajar-distributed
cd fajar-distributed
# Same dry-run + publish flow as 2.1
grep -E "^(name|version|license|description|repository)" Cargo.toml
cargo publish --dry-run --allow-dirty
cargo publish
```

### Step 2.3 — Publish `fajarquant 0.4.0`

```bash
cd ~/Documents/fajarquant
# Per docs/CRATES_IO_PUBLISH_PLAN.md Blocker 1: bump to 0.4.0
grep -E "^version " Cargo.toml
# If still < 0.4.0, bump it:
# sed -i 's/^version = "0\.3.*"$/version = "0.4.0"/' Cargo.toml
# (Verify actual current value first — don't assume.)
cargo publish --dry-run --allow-dirty
cargo publish
```

### Step 2.4 — Decide `cranelift-object` patch fate

`Cargo.toml` line 159 `[patch.crates-io]` block. Per
`docs/CRATES_IO_PUBLISH_PLAN.md` Blocker 2, two options:

**Option A (preferred for publish):** drop the patch entirely.

```bash
cd "/home/primecore/Documents/Fajar Lang"
# First, recover the original divergence reason:
git log --diff-filter=A -- patches/cranelift-object/ | head -20
git log -- patches/cranelift-object/ --oneline | head -10
# Read patches/cranelift-object/README.md if exists.

# If the patched issue is fixed upstream:
# 1. Edit Cargo.toml: remove the [patch.crates-io] block (line 159-end)
# 2. Bump cranelift-object pin to current upstream
# 3. Smoke-test:
cargo build --features native --bin fj  # Cranelift JIT path
cargo build --features llvm --bin fj    # LLVM path
cargo test --release --test phase17_self_compile -- --test-threads=1
# If all green: commit + push.
```

**Option B (fork rename):** see `docs/CRATES_IO_PUBLISH_PLAN.md`
"Option B — fork as a separate crate name" — only if Option A's
smoke-test fails.

### Step 2.5 — Replace git deps with published versions in fajar-lang

```bash
cd "/home/primecore/Documents/Fajar Lang"
# Edit Cargo.toml lines 24, 32, 33:
# fajarquant = { git = "...", rev = "..." }
#   →  fajarquant = "0.4.0"
# fajar-wasi-p2 = { git = "...", rev = "..." }
#   →  fajar-wasi-p2 = "0.1.0"
# fajar-distributed = { git = "...", rev = "..." }
#   →  fajar-distributed = "0.1.0"

cargo update -p fajarquant -p fajar-wasi-p2 -p fajar-distributed
cargo test --lib  # Confirm no API drift between rev pin → published 0.X
cargo test --release --test phase17_self_compile -- --test-threads=1

# Re-run blocker check:
bash scripts/check_publish_ready.sh
# Expect: PASS — 0 blocker lines.

# Commit:
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): replace git deps with published crates for v36.0.0 publish

- fajarquant: git rev b05ecf17 → published 0.4.0
- fajar-wasi-p2: git rev d57d3b21 → published 0.1.0
- fajar-distributed: git rev 4011a3d5 → published 0.1.0
- [patch.crates-io] cranelift-object: removed (Option A; see
  docs/CRATES_IO_PUBLISH_PLAN.md)

Closes F3 / P7.F3 (HONEST_AUDIT_V33 §11)."
git push
```

### Step 2.6 — Publish fajar-lang

```bash
cd "/home/primecore/Documents/Fajar Lang"
cargo publish --dry-run --allow-dirty
# Fix any reported issues (e.g., missing files in published set).
cargo publish
# Wait ~30s. Then:
open https://crates.io/crates/fajar-lang  # or just navigate
```

### Step 2.7 — Update HONEST_AUDIT_V33.md F3 row

```diff
- | F3 | crates.io publish blocker | ✅ engineering-side | `bash scripts/check_publish_ready.sh` reports 2 documented blockers + `docs/CRATES_IO_PUBLISH_PLAN.md` closure sequence; cross-repo coordination required for full closure |
+ | F3 | crates.io publish | ✅ CLOSED v36.0.0 | `bash scripts/check_publish_ready.sh` reports 0 blockers; fajar-lang 36.0.0 + fajarquant 0.4.0 + fajar-wasi-p2 0.1.0 + fajar-distributed 0.1.0 all live on crates.io |
```

**Phase 2 closes:** F3. **Three items down, one to go.**

---

## Phase 3 — A1 LLVM upstream filing (~15 min, single GitHub issue)

`docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` already contains the
filing draft (title + body verbatim). Two paths depending on
reproduction state:

### Step 3.1 — Decide reproduction strategy

Re-read `docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` lines 79-117
("Reduced repro (current state)"). Three options:

**Option I — File with current state.** The bug body documents
"reduced repro not yet single-file LLVM IR" honestly. LLVM triagers
may still accept this if Fajar Lang itself is open + reproducible.

**Option II — Reproduce in pure C first.** Per the doc's checklist
step 1: write ~30-line C with `-O2 -ffreestanding -mcmodel=kernel
-mno-red-zone -fno-stack-protector` mirroring the kernel target
flags. If C diverges → cleaner repro for upstream.

**Option III — Reduce LLVM IR via `llvm-reduce`.** Set
`FJ_EMIT_IR=1`, build the affected kernel module, run llvm-reduce
to ~100 lines. Best reproducibility for upstream; ~1-2h work.

**Recommended:** Option I now (clears the V33 milestone), with
Option II/III deferred as a follow-up if LLVM triagers ask for
reduction.

### Step 3.2 — File the issue

```bash
# Open https://github.com/llvm/llvm-project/issues/new in browser
# Title (copy verbatim from docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md):
#   "Loop vectorizer miscompile on packed-quantized vecmat at -O2
#    with no_std + restricted calling convention"
# Body: copy lines 119-150 of docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md
#   "Body" section starting "**LLVM version:** ..."
# Tags: loop-vectorize, miscompile, llvm:codegen (triager adds these
#   typically; you can suggest in a comment if needed)
# Submit.
```

Or via `gh` CLI (requires `gh auth refresh` for llvm/llvm-project
write scope; may need separate auth):

```bash
gh issue create \
  --repo llvm/llvm-project \
  --title "Loop vectorizer miscompile on packed-quantized vecmat at -O2 with no_std + restricted calling convention" \
  --body-file <(sed -n '119,150p' docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md)
```

### Step 3.3 — Update docs with filed issue URL

```bash
# Once filed at e.g. https://github.com/llvm/llvm-project/issues/12345:
# 1. Edit docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md:
#    Replace "## Status (2026-05-03): ..." line with the issue URL.
# 2. Edit docs/HONEST_AUDIT_V33.md A1 row:
#    "✅ CLOSED v36.0.x (filed at llvm/llvm-project#12345)"
# 3. Bump scorecard line 13: "22 of 25 → 25 of 25 work-items PASS"
git add docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md docs/HONEST_AUDIT_V33.md
git commit -m "docs(audit): close A1 + F1 + F3 after founder action burst

V33 scorecard 22/25 → 25/25 PASS:
- F1: v36.0.0 binaries on GitHub Releases (auto-uploaded via release.yml)
- F3: fajar-lang 36.0.0 published to crates.io (with deps chain)
- A1: LLVM miscompile filed at llvm/llvm-project#<NNNN>"
git push
```

**Phase 3 closes:** A1. **All 4 items down.**

---

## Verification (post-execution)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# F1 verification
gh release view v36.0.0 | grep -cE "\.tar\.gz|\.zip"
# Expect: ≥6 (5 platform archives + SHA256SUMS.txt)

# F3 verification
bash scripts/check_publish_ready.sh
# Expect: PASS — 0 blocker lines
curl -sf https://crates.io/api/v1/crates/fajar-lang/36.0.0 | jq -r '.version.num'
# Expect: 36.0.0

# A1 verification
grep -A1 "^### Title" docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md
# Expect: issue URL substituted in.

# V33 final state
grep "22 of 25\|25 of 25" docs/HONEST_AUDIT_V33.md
# Expect: 25 of 25 work-items PASS
```

---

## Re-entry conditions (if any phase blocks)

- **Phase 1 stuck:** release.yml workflow fails on a single
  platform → check `gh run view <ID>`. macOS/Windows runners can
  flake; rerun via `gh run rerun <ID>`.
- **Phase 2 Step 2.4 Option A smoke fails:** cranelift-object
  patch is still load-bearing → switch to Option B (fork-rename).
  Recover divergence reason from `git log -p
  patches/cranelift-object/` to know which upstream issue is
  load-bearing.
- **Phase 2 Step 2.6 cargo publish rejects:** fix the specific
  error (most common: missing files in `include`/`exclude`
  arrays; some files referenced but not listed). Iterate
  `cargo publish --dry-run`.
- **Phase 3 LLVM triagers ask for reduction:** revisit Option II
  (C repro) or Option III (llvm-reduce) per Step 3.1.

---

## What this doc does NOT cover (out of Path A scope)

- **TQ12.6 24h stability on Q6A** — requires user's ARM64 hardware.
  Separate session.
- **F.11 BitNet TL2 runtime activation** — PERMANENT-DEFERRED per
  honest design. Re-entry only if upstream `microsoft/BitNet`
  changes; 4 mechanical re-entry conditions in
  `~/Documents/fajarquant/docs/FJQ_PHASE_F_TAX_VERTICAL_ROADMAP.md`
  §F.11.
- **FajarQuant Phase E (bilingual training)** — multi-week GPU
  compute. Tracked separately.
- **FajarQuant Phase F (hardware acceleration roadmap)** —
  multi-month research. Tracked separately.

---

*This document is the deliverable for "Path A — Founder action burst"
chosen at 2026-05-13. Engineering-side prep complete; execution is
external action only. After completion, HONEST_AUDIT_V33 closes at
25/25 PASS and v36.0.0 is the live tag.*
