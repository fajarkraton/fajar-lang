# HONEST_AUDIT_V36 — full re-audit at v36.0.0+3

> **Date:** 2026-06-12
> **Scope:** comprehensive re-audit of every verifiable claim in
> `CLAUDE.md` §3/§9/§17, `README.md` badges, and `docs/HONEST_AUDIT_V33.md`
> against repo HEAD `f8063e9d` (v36.0.0-3, branch `main`, working tree clean).
> **Predecessor:** `docs/HONEST_AUDIT_V33.md` (perfection-plan exit scorecard,
> 2026-05-03, F1 closed 2026-05-13).
> **Toolchain at audit:** rustc 1.93.0 / cargo 1.93.0.

## Verdict

**The codebase delivers what the docs claim.** Every load-bearing number in
CLAUDE.md §3 reproduces exactly: 6,591 lib + 9,516 integ (80 files, 1
ignored) + 14 doc tests, 0 fail; stress 5/5 at `--test-threads=64`; phase17
byte-equality 4/4; stage1_full 91/91; context_safety 149/149; 0 production
unwraps; error-code gap 0; clippy `--lib` / fmt / strict-rustdoc all clean;
v36.0.0 GitHub release live with 5 platform binaries + SHA256SUMS.

**9 findings**, of which **1 is code** (benches fail
`clippy --all-targets`), **1 is rule-text drift** (§6.4's "ZERO unsafe
outside codegen/+runtime/os/" structurally false; the suspected
SAFETY-comment debt proved to be scanner false positives — see F7
correction), and **7 are documentation drift** (stale counts/comments left
behind by the EOS-29..40 extraction arc). Nothing affects correctness of
shipped functionality. All findings remediated same-day — see Remediation
log at the end.

## Quality gates (all commands run 2026-06-12, logs in /tmp/fj_audit/)

```
cargo test --lib                                    6,591 PASS / 0 FAIL          ✅ exact match
cargo test --tests --no-fail-fast                   9,516 PASS / 0 FAIL / 1 ign  ✅ exact match (80 test files)
cargo test --doc                                       14 PASS / 0 FAIL / 1 ign  ✅ (1 ignored doctest undocumented)
cargo test --lib -- --test-threads=64  (×5)         5/5 PASS                     ✅
cargo test --features llvm,native --lib codegen::   1,825 PASS / 0 FAIL          ✅ (claim "162+ LLVM" satisfied)
cargo clippy --lib -- -D warnings                   exit 0                       ✅
cargo clippy --all-targets -- -D warnings           exit 101                     ❌ F1 (benches/embedded_bench.rs:76)
cargo fmt -- --check                                exit 0                       ✅
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps \
  --lib --document-private-items                    exit 0                       ✅
python3 scripts/audit_error_codes.py --strict       gap=0 (135 cat / 125 cov / 12 fwd)  ✅
python3 scripts/audit_unwrap.py                     0 production unwraps         ✅
bash scripts/check_doc_coverage.sh                  95.93% PASS                  ✅ (docs claim 95.79% — improved, F8)
bash scripts/check_stdlib_docs.sh                   180/180 = 100% PASS          ✅
bash scripts/check_version_sync.sh                  PASS (major 36)              ✅
bash scripts/check_publish_ready.sh                 FAIL — 2 documented blockers ✅ expected (git deps + [patch.crates-io])
bash scripts/multi-repo-check.sh                    fajar-lang ✓ / fajaros-x86 ✓ / fajarquant dirty (untracked logs/ only)
gh release view v36.0.0                             5 binaries + SHA256SUMS.txt  ✅ F1(V33) stays closed
./target/release/fj run|check|run --vm hello.fj     all OK                       ✅ E2E smoke
```

Key integ sub-suites re-confirmed inside `02_test_integ.log`:
`selfhost_phase17_self_compile` 4/4 (incl. `phase17_stage2_native_triple_test`),
`selfhost_stage1_full` 91/91, `context_safety_tests` 149/149.

## Claim-vs-actual cross-check (CLAUDE.md §3 "Current Totals")

| Claim | Actual | Status |
|---|---|---|
| 6,591 lib + 9,516 integ (80 files; 1 ign) + 14 doc = 16,121 | 6,591 + 9,516 (80 files; 1 ign) + 14 (+1 ign) | ✅ exact |
| ~407,744 Rust LOC, 348 files | 407,742 LOC, 348 files (`find src -name '*.rs'`) | ✅ |
| 40 pub mods at lib.rs root, 40 [x] | 40 (`grep -c '^pub mod ' src/lib.rs`); per-module E2E not re-verified this audit — HONEST_STATUS_V26 + 9,516 integ remain the evidence | ✅ |
| 38 CLI subcommands | 38 variants in `#[derive(Subcommand)]` enum, `src/main.rs` | ✅ |
| 19 feature flags | 19 in `[features]` (smt absent ✓) | ✅ |
| CI: 7 workflows | **6** (`ls .github/workflows/` = ci, docs, embedded, nightly, nova, release; already 6 at tag v36.0.0) | ❌ F2 |
| Binary 18 MB | 18,765,336 bytes `target/release/fj` | ✅ |
| MSRV 1.87 | `rust-version = "1.87"` | ✅ |
| 309 .fj examples | 309 | ✅ |
| 95.79% pub-doc | 95.93% (16,042/16,723) | ✅ improved — F8 stale figure |
| 100% stdlib_v3 doc | 180/180 | ✅ |
| Tags: "v36.0.0 deferred to Phase G remainder" | v36.0.0 **tagged + released 2026-05-13** | ❌ F4 |
| §17 map: tests/ 46 files, examples/ 231 .fj, docs/ 157; src lists distributed + wasi_p2 | 80 / 309 / 282 top-level (328 recursive); `src/distributed/` + `src/wasi_p2/` removed at E.5/F.5 | ❌ F3 |
| wasi_p2 + distributed extracted, 12 smoke tests | `fajar-wasi-p2` + `fajar-distributed` wired as git+rev deps; `tests/{wasi_p2,distributed}_integration.rs` present | ✅ |

## Findings

### F1 — `benches/embedded_bench.rs:76` fails clippy 1.93 (code, low severity)

`cargo clippy --all-targets -- -D warnings` exits 101:
`Q16_16::from_f64(3.14)` triggers deny-by-default `clippy::approx_constant`.
The existing gates (`--lib`, V33's `--tests --release`) never compile
benches, so this slipped through and would also fail any future
`--all-targets`/`--benches` CI job.
**Fix:** use `std::f64::consts::PI` (or a non-π constant like `3.5` if the
value is arbitrary).
**Prevention:** extend the per-commit gate or CI clippy job to
`--all-targets`.

### F2 — CLAUDE.md §3 claims "CI: 7 workflows"; actual 6 (doc)

`ls .github/workflows/` = 6 files, and `git ls-tree v36.0.0` shows it was
already 6 at the tag — the figure was wrong when written (likely counted the
since-removed smt job or a deleted workflow).

### F3 — CLAUDE.md §17 repository map stale (doc)

Still lists `distributed, wasi_p2` under `src/` (both removed at Phase
E.5/F.5, commits `62f81f64` + `252359b9`) and carries pre-EOS counts:
`tests/` "46 files" (actual 80), `examples/` "231 .fj" (actual 309),
`docs/` "157" (actual 282 top-level, 328 recursive).

### F4 — CLAUDE.md §3 Tags line says v36.0.0 deferred (doc)

"Path E + F closure tag v36.0.0 deferred to Phase G remainder; see
CHANGELOG.md [Unreleased]" — but v36.0.0 was tagged 2026-05-13, has a full
CHANGELOG section (line 5), and a live GitHub release with 5 binaries.

### F5 — Cargo.toml comment block stale (doc)

Lines near the `fajar-wasi-p2`/`fajar-distributed` deps still say
"Currently dormant: src/wasi_p2/ + src/distributed/ are still
local-authoritative. Re-export shim + local removal land at Phase E.5 +
F.5." Both phases landed; the dirs are gone and the git deps are live.

### F6 — CHANGELOG orphan `[Unreleased]` section (doc)

`## [Unreleased] — 2026-05-04 CI rehab + FAJAROS_100PCT_FJ_PLAN` sits at
line 3132, buried between historical versions instead of being folded into
its release (v33.x era). Keep-a-changelog convention expects at most one
`[Unreleased]` at the top.

### F7 — `unsafe` outside §6.4 allowed dirs + missing SAFETY comments (code-hygiene)

§6.4 says "ZERO `unsafe {}` blocks outside `src/codegen/` and
`src/runtime/os/`". Reality: ~221 unsafe sites (fn/block/impl, excluding
strings/comments) live outside those dirs — concentrated in
`src/runtime/gpu/{cuda,wgpu}_backend.rs`, `src/ffi_v2/`,
`src/runtime/ml/npu/qnn.rs`, `src/bsp/`, `src/hw/`,
`src/interpreter/{ffi,eval/methods}.rs` — i.e. exactly the FFI/GPU/NPU/BSP
surface that cannot avoid unsafe. Meanwhile `src/runtime/os/` (an allowed
dir) contains 0. The rule text predates these modules and is structurally
stale; it should enumerate the real allowed surface instead of being
silently violated.

**Correction during remediation (same day):** the sub-agent's "19 sites
missing SAFETY" did NOT survive full manual verification — every flagged
site is covered, either by a cluster `// SAFETY:` comment a few lines
higher (cuda_backend.rs symbol-load and Drop-path clusters, incl. the
`cuStreamDestroy_v2` block, which has `// SAFETY:` at line 1183) or by an
idiomatic `/// # Safety` doc contract on the `unsafe fn`
(`interpreter/ffi.rs::call_raw`, `runtime/ml/ops.rs::hadamard_row_avx2`)
that the agent's `// SAFETY:`-only 3-line window could not see. One site
(`analyzer/type_check/check.rs:1048`) was a variable named `in_unsafe`,
not the keyword. **SAFETY-comment debt: zero.** The mechanical gate
confirms: 719 production unsafe sites, 0 outside allowlist, 0 missing
SAFETY coverage (`python3 scripts/audit_unsafe.py --strict` exit 0).

What remains real in this finding is the rule text: §6.4's "ZERO outside
codegen/ + runtime/os/" was structurally false.

**Fix:** amend §6.4 to the real policy (enumerated FFI/hardware allowlist,
SAFETY mandatory, cluster comments and `# Safety` doc contracts both
accepted) and add a mechanical `scripts/audit_unsafe.py` drift gate
alongside `audit_unwrap.py`.

### F8 — doc-coverage figure drifted upward (doc, trivial)

CLAUDE.md + README badge say 95.79%; actual 95.93%. Stale in the favorable
direction.

### F9 — expected/benign residuals (info)

- `check_publish_ready.sh` FAIL = the 2 documented crates.io blockers
  (3 git deps + `[patch.crates-io]` cranelift-object) — unchanged founder
  action, see `docs/PATH_A_FOUNDER_ACTION_BURST.md`.
- `~/Documents/fajarquant` dirty = untracked `logs/` only.
- 1 ignored doctest not mentioned in §9.1's "14 doc" phrasing.

## Addendum — GitHub page cross-check (2026-06-12)

The rendered repo page (`github.com/fajarkraton/fajar-lang`, README at
`f8063e9d` — remote == local) was swept claim-by-claim. **Verified
matching:** release badge v36.0.0 + 5 binaries, tests 16K+ (16,121),
stress 5×, unwrap 0, modules 40, LOC 408K, FajarOS Nova v4.0.0 badge
(fajaros-x86 is tagged v4.0.0), examples "242 top-level + 4 aspirational
+ 6 folders" (consistent basis; 309 recursive matches CLAUDE.md), all 14
listed built-in macros implemented (grep-verified in src/macros.rs +
parser), v36.0.0 release-notes deltas (54→40 modules, −29.6K LOC,
6,591/9,516 tests), repo About description + topics.

Three additional README findings:

### G1 — badge "Doc Coverage 95.79%" stale (= F8, publicly visible)
Actual 95.93% per `check_doc_coverage.sh`.

### G2 — badge "JIT 76x speedup" inconsistent with the page's own tables
76x is the v20.8.0-era figure (README line 611). The current benchmark
tables on the same page say 128-156x vs interpreter (i9-14900HX) and 12x
vs C. The badge should cite the current figure or be labeled historical.

### G3 — "Standard packages: 39" — actual 37
`ls packages/` = 37, and `git ls-tree` shows 37 at v33.0.0, v35.6.0, and
v36.0.0 — the figure 39 was never true in this window.

Soft note: the project-stats row "WASI: P1 + P2 (WIT parser, component
model, ...)" is technically accurate (P2 routes through the extracted
`fajar-wasi-p2` crate) but reads as core capability despite the
deprecation warning + announced v37 hard removal; consider a
cross-reference to the companion-crates section.

Author-bio and community metrics (stars/forks) were not audited — outside
engineering-claim scope.

## What was NOT re-verified this audit

- Per-module E2E re-classification of all 40 `[x]` modules (evidence remains
  HONEST_STATUS_V26 + the green 9,516-test integ suite + 309 examples).
- GPU/CUDA runtime on RTX 4090, FajarOS QEMU/hardware boots, FajarQuant
  bit-exactness — out of scope for a single-repo gate re-run; their
  regression suites inside `cargo test --tests` all pass.
- Benchmarks vs C/Rust/Go (`benches/baselines/`) were not re-timed.

## Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| R1 pre-flight audit | YES — this doc is the pre-flight for any follow-up fix phase |
| R2 runnable verification | YES — every gate above is a literal command with logged output |
| R3 prevention layer | PROPOSED — F1 (`--all-targets` gate) + F7 (`audit_unsafe.py`) name their prevention mechanisms; ship with the fixes |
| R4 agent numbers cross-checked | YES — sub-agent SAFETY scan spot-checked by hand; its "19 violations" downgraded (cluster-comment false positives identified) |
| R5 surprise budget | n/a (audit, not estimated plan) |
| R6 mechanical gates | YES — all scripts have exit-code semantics |
| R7 public-artifact sync | FLAGGED — F2/F3/F4/F5/F8 are exactly the public-artifact drift this rule exists to catch |
| R8 multi-repo check | YES — `multi-repo-check.sh` run; fajarquant untracked logs/ noted |

## Self-check (CLAUDE.md §6.6)

| Rule | Status |
|---|---|
| R1 [x] = E2E | YES — binary smoke-tested (run/check/--vm); release assets verified via `gh` |
| R2 verification per claim | YES — see gates table |
| R3 no inflated stats | YES — every number above is from a fresh command, incl. the unfavorable ones (F1, F7) |
| R5 audit before building | YES — this doc precedes any fix work |
| R6 real vs framework | YES — 0 [f]/[s] claim not contradicted by any gate |

## Outcome

v36.0.0 stands as claimed: the strongest verified state of the project, and
the first audit cycle where **every headline test/LOC/module number
reproduced exactly**. The drift that exists is clerical (extraction-arc
leftovers) plus one bench-only clippy error; the suspected SAFETY-comment
debt dissolved under manual verification (see F7 correction).

## Remediation log (2026-06-12, same session)

| Finding | Action |
|---|---|
| F1 | `benches/embedded_bench.rs` operands 3.14/2.71 → exact binary fractions 3.25/2.75; CI clippy job widened `--tests` → `--all-targets` |
| F7 | SAFETY debt = 0 after manual verification (agent false positives). §6.4 rewritten to enumerated-allowlist policy; `scripts/audit_unsafe.py` gate added (719 sites / 0 violations) + wired into ci.yml |
| F2 | CLAUDE.md §3: "CI: 7 workflows" → 6 |
| F3 | CLAUDE.md §17: src map drops distributed/wasi_p2; counts 46→80 test files, 231→309 examples, 157→282 docs |
| F4 | CLAUDE.md §3 Tags line: v36.0.0 marked released (2026-05-13, 5 binaries) |
| F5 | Cargo.toml comment: "currently dormant" block replaced with post-E.5/F.5 reality |
| F6 | CHANGELOG line 3132 `[Unreleased]` retitled "[untagged, folded into v33.x]" with explanation |
| F8/G1 | 95.79% → 95.93% in CLAUDE.md §3, README badge + stats row |
| G2 | README JIT badge 76x → "128x vs interpreter" (matches in-page benchmark table; prose row was already honest) |
| G3 | README "Standard packages: 39" → 37 + verification command |
| soft | README WASI row now names the extracted crate + v37 hard-removal |

Also updated: CLAUDE.md §2 session protocol + §18 index now point at this
doc as the latest audit; §6.5 checklist gained the audit_unsafe gate.

---

*HONEST_AUDIT_V36 — written 2026-06-12 as a standalone re-audit; no plan
cycle attached. Logs preserved at /tmp/fj_audit/ for this session.*
