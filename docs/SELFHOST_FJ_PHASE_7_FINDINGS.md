---
phase: 7 — v33.4.0 Release (final phase)
status: CLOSED 2026-05-05; release artifacts ready
budget: ~0.5d planned + 25% surprise = 0.625d cap
actual: ~30min Claude time
variance: -97%
artifacts:
  - This findings doc (Phase 7 release closure)
  - Cargo.toml: 33.3.0 → 33.4.0
  - CHANGELOG.md: new [v33.4.0] entry covering all 7 self-host phases
  - README.md: badge + Quick Stats + Release History row updated
  - CLAUDE.md: completion-status header bumped
  - All quality gates GREEN: cargo fmt --check, cargo clippy -D warnings,
    cargo test --lib (7629 PASS), cargo test --test selfhost_stage1_subset (5/5)
prereq: Phase 6 closed (`docs/SELFHOST_FJ_PHASE_6_FINDINGS.md`)
---

# fj-lang Self-Hosting — Phase 7 Findings

> **Stage-1-Subset Self-Hosting milestone tagged as v33.4.0.** All
> 7 phases of the self-host plan closed. Sister Rust compiler stays
> as production reference; fj-source bootstrap chain is a parallel
> proof point that can compile subset-fj programs to native binaries
> via gcc.

## 7.1 — Release artifacts

| Artifact | Status |
|---|---|
| Cargo.toml `version = "33.4.0"` | ✅ |
| CHANGELOG.md `[v33.4.0] — 2026-05-05 Stage-1-Subset Self-Hosting` entry | ✅ |
| README.md release badge → v33.4.0 | ✅ |
| README.md Quick Stats Release row → v33.4.0 | ✅ |
| README.md Release History v33.4.0 row prepended | ✅ |
| CLAUDE.md "Completion Status" header → v33.4.0 | ✅ |
| Phase 7 findings doc (this) | ✅ |

## 7.2 — Quality gate verification

```
cargo fmt -- --check                                          ✅ PASS (0 diff)
cargo clippy --lib --tests -- -D warnings                     ✅ PASS (0 warnings)
cargo test --release --lib                                    ✅ 7629 PASS, 0 fail
cargo test --release --test selfhost_stage1_subset            ✅ 5 PASS, 0 fail (0.04s)
cargo build --release                                         ✅ clean, 38s
```

## 7.3 — CI integration (no new workflow needed)

Existing `.github/workflows/ci.yml` line 57 (`cargo test --lib --bins
--tests --examples`) automatically picks up `tests/selfhost_stage1_subset.rs`
via the `--tests` flag.

Test gated to `#[cfg(unix)]` so Windows runner skips (gcc not
universally available). All 5 tests run on ubuntu + macos matrix.

No `.github/workflows/` edit required.

## 7.4 — Tag + GH Release plan

```bash
git add Cargo.toml CHANGELOG.md README.md CLAUDE.md \
        docs/SELFHOST_FJ_PHASE_7_FINDINGS.md \
        tests/selfhost_stage1_subset.rs

git commit -m "release(v33.4.0): Stage-1-Subset Self-Hosting"

git tag -a v33.4.0 -m "v33.4.0 Stage-1-Subset Self-Hosting"

git push origin main
git push origin v33.4.0
```

GH Release will be triggered by the v33.4.0 tag (release.yml workflow
in `.github/workflows/`). 5-platform binaries produced by CI.

## 7.5 — Effort recap

| Task | Plan | Actual |
|---|---|---|
| 7.A Cargo.toml version bump | 5min | 1min |
| 7.B CHANGELOG.md entry | 30min | 10min |
| 7.C README.md sync | 30min | 10min |
| 7.D CLAUDE.md sync | 5min | 2min |
| 7.E Phase 7 findings + tag | 30min | 15min |
| **Total** | **~2h** | **~30min** |
| **Variance** | — | **-75%** |

## 7.6 — Risk register final

| ID | Risk | Status |
|---|---|---|
| R1 | fj-lang feature gaps | NONE surfaced across all 7 phases |
| R2 | Cranelift FFI shim | RESOLVED Phase 4 (gcc backend pivot) |
| R3 | Stage1 ≢ Stage0 | Behavior-equivalent verified per phase |
| R4 | Generics/traits leak | Subset hand-curated; no leaks |
| R5 | Performance | Adequate for Stage-1 proof |
| R6 | Ident text placeholder | Documented; deferred to Stage-1-Full |
| R7 | Driver narrow | Mitigated to 5 shapes; full closure = parser AST-builder ~1d post-v33.4.0 |

## 7.7 — Cumulative state at v33.4.0

| Metric | Value |
|---|---|
| Self-host phases closed | 7/7 ✅ |
| Phase 5 chain proof | 1 program (RC=99) |
| Phase 6 E2E tests | 5/5 PASS in 0.04s |
| Lib tests | 7629 PASS |
| fmt + clippy | clean |
| Cumulative Claude time | ~3.5h |
| Plan estimate | 5-10 days |
| Variance | -97% to -99% |

## 7.8 — What v33.4.0 ships

✅ **Stage-1-Subset self-hosting**: `fn main() -> i64 { ... }` shapes
   covering let bindings, binops, if-else, fn calls, runtime println —
   compile via fj-source codegen → gcc → native binary.

✅ **All 4 stdlib bootstrap modules** (lexer/parser/analyzer/codegen).

✅ **5 Rust integration tests** as regression gate.

✅ **7 detailed phase findings docs** as audit trail.

✅ **Sister Rust compiler unchanged** — production reference stays.

## 7.9 — What v33.4.0 does NOT yet ship (honest scope per CLAUDE.md §6.6 R1)

❌ **Stage-1-Full self-hosting** — needs parser AST-builder upgrade
   (~1d fj refactor: every `parse_*` returns `(new_pos, ast_chunk)`).

❌ **Stage 2 triple-test** (Stage 1 binary == Stage 2 binary) —
   roadmap-only.

❌ **`fj selfhost compile <file>` CLI subcommand** — Phase 5/6
   demonstrated chain via `fj run` of combined sources; CLI sugar
   deferred.

❌ **Identifier-text-aware analyzer** — placeholder `var_{idx}` still
   in `extract_ident`; blocks duplicate-name detection by source text.

These are queued as post-v33.4.0 work; not blocking the milestone.

## Decision gate (§6.8 R6)

This file committed → release commit + v33.4.0 tag ready. Self-host
plan terminal-complete at this milestone. Future Stage-1-Full / Stage 2
work tracked separately.

---

*SELFHOST_FJ_PHASE_7_FINDINGS — 2026-05-05. v33.4.0 release artifacts
prepared in ~30min vs ~2h budget (-75%). All quality gates green.
Stage-1-Subset Self-Hosting milestone closed; sister Rust compiler
stays as production reference; future Stage-1-Full work deferred to
post-v33.4.0 with clear closure path documented.*
