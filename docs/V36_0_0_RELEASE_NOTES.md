## 🪓 Compass §5 closure — wasi_p2 + distributed extracted

The strategic compass §5.1 verdict *"Bekukan. Tidak relevan untuk niche
embedded."* is now fully realized for the two long-tail subsystems
that were never on the embedded ML hot path. **v36.0.0 removes
`src/wasi_p2/` and `src/distributed/` from the fajar-lang core** and
re-homes them as standalone Apache-2.0 crates under the same org. The
extracted crates own their own lib-test surface; fajar-lang depends
on them through rev-pinned Cargo git deps and keeps 12 public-API
smoke tests to catch upstream drift.

**MAJOR bump justified by 2 user-facing CLI surface changes:**
- `fj run-cluster` is **removed** (per Compass §5.1 Option α — no
  embedded user flow consumed it; the standalone
  `fajar-distributed` crate is the canonical path).
- `fj build --target wasm32-wasi-p2` **emits a deprecation warning**
  naming the extracted crate URL + the v37 hard-removal target
  (per Compass §5.1 Option γ — grace window for the WASI P2
  build path).

### Companion crates

| Crate | Repo | Wired-up |
|---|---|---|
| `fajar-wasi-p2` | [`fajarkraton/fajar-wasi-p2`](https://github.com/fajarkraton/fajar-wasi-p2) | `fj build --target wasm32-wasi-p2` routes through the extracted `ComponentBuilder`; deprecation warning on stderr; hard-removed at v37 |
| `fajar-distributed` | [`fajarkraton/fajar-distributed`](https://github.com/fajarkraton/fajar-distributed) | Standalone — no fajar-lang CLI surface; consumers depend on the crate directly |

### Phase-by-phase shipped in this release

| Phase | Commit | What |
|---|---|---|
| **E.0 + F.0** | `5caed58d` | Parallel agent B0 audits — file inventory, symbol-surface freeze, test fn-name resolution map for both subsystems |
| **E.3 + F.3** | `4065979b` | Cargo wire-up — both extracted crates added as git deps with rev pins (`d57d3b21` + `4011a3d5`) |
| **E.4 + F.4** | `10a802d6` | CLI handling per D-0.2: wasi_p2 deprecation warning (Option γ) + `cmd_run_cluster` deletion (Option α) |
| **E.5** | `62f81f64` | Remove `src/wasi_p2/` (12 files, −13,791 LOC) + cascade-clean 19 N7 lib tests in `eval/mod.rs` + 2 integ tests |
| **F.5** | `252359b9` | Remove `src/distributed/` (16 files, −15,343 LOC) + cascade-clean 22 N3 lib tests in `eval/mod.rs` + 10 N3 integ tests |
| **E.6** | `681cae3b` | 6 smoke tests in `tests/wasi_p2_integration.rs` pinning the `ComponentBuilder` API used by `cmd_build_wasi_p2` |
| **F.6** | `5e871965` | 6 smoke tests in `tests/distributed_integration.rs` for the Raft/cluster/discovery surface the deleted N3 tests historically exercised |
| **E.7 + F.7** | `c8e0f465` | Closure findings doc + CHANGELOG `[Unreleased]` block + MEMORY.md pointer refresh |
| **Phase G.1-G.4** | `99401f11` + `e6fbef1c` + (this commit) | CLAUDE.md §3 stats refresh + README companion-crates section + multi-repo sync verify + MEMORY.md housekeeping |

### Stats deltas

| Metric | Pre-extraction | Post-extraction | Δ |
|---|---:|---:|---:|
| Rust LOC (across kernel build path) | ~437,000 (376 files) | **~407,744** (348 files) | **−29,580** |
| Lib tests | 7,211 | **6,591** | −620 |
| Integ tests | 10,136 | **9,516** (80 files; +2 new smoke files) | −620 |
| Root `pub mod` declarations in `src/lib.rs` | 42 | **40** | −2 |
| CLI subcommands | 39 | **38** | −1 (`run-cluster`) |

Lib tests for the extracted subsystems now live in their own crates
(`fajar-wasi-p2` ~244 + `fajar-distributed` ~332 = 576 lib tests
preserved upstream). fajar-lang keeps 12 integ smoke tests covering
the public-API surface.

### Migration impact

For repository code: **none required** — all surface changes are
deletions of dead-code paths.

For external consumers:

- **`fj run-cluster`**: was never exposed in any documented user flow;
  no migration path. Use `fajar-distributed` crate directly.
- **`fj build --target wasm32-wasi-p2 …`**: still works. Migrate to
  the standalone `fajar-wasi-p2` crate before v37 (hard-removal).

For downstream Cargo consumers of `fajar-lang` itself: the
`fajar-lang = "36"` crate.io listing now ships **without** the
in-tree `wasi_p2::*` and `distributed::*` modules. If you were
importing those paths, switch to the standalone crates.

### Engineering gates

| Gate | Result |
|---|---|
| `cargo test --lib` | **6,591 / 6,591** PASS |
| `cargo test --release --test selfhost_stage1_full` | **91 / 91** PASS |
| `cargo test --release --test selfhost_phase17_self_compile` | **4 / 4** PASS — Stage 2 byte-equality preserved through E.5 + F.5 cascade |
| Full integ `--no-fail-fast` | 0 failures across 80 suites (incl. 2 new) |
| `cargo test --release --test context_safety_tests` | **149 / 149** PASS |
| `cargo clippy --lib -- -D warnings` | clean |
| `cargo fmt -- --check` | clean |
| `bash scripts/check_publish_ready.sh` (post-publish-chain) | PASS — 0 blockers (3 git deps replaced with published versions) |

### Combined wall-clock

**~2.8h actual** for Path E + F (E.0..E.6 + F.0..F.6) vs **~35-46h
plan estimate** — **−92% under estimate**, dominated by the parallel
agent-driven Phase E.0/F.0 B0 audits (−97%) and D-0.2-distributed
Option α eliminating the F.4/F.6 design surface.

### Source of truth

- [`docs/COMPASS_5_PATH_E_F_EXTRACTION_FINDINGS.md`](docs/COMPASS_5_PATH_E_F_EXTRACTION_FINDINGS.md) — closure findings with per-phase log, LOC reclaim, surprises, re-entry conditions
- [`docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md`](docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md) — original plan (460 lines)
- [`docs/PATH_E_WASI_P2_EXTRACTION_B0_FINDINGS.md`](docs/PATH_E_WASI_P2_EXTRACTION_B0_FINDINGS.md) — E.0 pre-flight audit
- [`docs/PATH_F_DISTRIBUTED_EXTRACTION_B0_FINDINGS.md`](docs/PATH_F_DISTRIBUTED_EXTRACTION_B0_FINDINGS.md) — F.0 pre-flight audit
- [`docs/decisions/2026-05-12-path-e-f-prep.md`](docs/decisions/2026-05-12-path-e-f-prep.md) — D-0.1/0.2/0.3 (repo names + CLI fate + dep mode)
- [`docs/1/STRATEGIC_COMPASS.md`](docs/1/STRATEGIC_COMPASS.md) §5.1 — the verdict that authorized the extraction

### Surprises observed during this arc

1. **E.6.4 `enable_realloc` is a flag, not bytes-emission in v0.1.0.**
   The integ test was rewritten to pin the `has_realloc()` getter
   contract; the canonical-abi realloc func emission is a TODO upstream.
2. **F.5 mid-gate `Exec format error` on 4 test binaries.** Stale
   binaries built against the pre-deletion crate state; `cargo clean`
   (18.9 GiB) cleared, fresh build green.
3. **F.5 lib-test drop −357 vs B0 −352 prediction** — within +5
   surprise budget per Plan Hygiene §6.8 R5.
4. **Integ-test count was previously inflated.** CLAUDE.md §3 said
   10,136 integ; the real number was 9,516 — double-counting of
   supplementary buckets corrected at Phase G.1.

### Predecessors

v35.6.0 (Compass §4.4 context-dimension closure — `fn` = `@safe`
default) → v35.5.0 (FJARR_LEAK Phase 2 D-FULL — type-system §4.4
closure) → v35.4.x (byte_at/char_at cascade) → v35.0.0..v35.3.x
(Stage 2 self-host + crypto + SQLite + lexer perf).

### Re-entry conditions

The extractions are designed to be reversible if Compass §5.1 is ever
reversed. See `docs/COMPASS_5_PATH_E_F_EXTRACTION_FINDINGS.md` §8 for
the mechanical reversal steps for each path. Upgrading the rev pins
when the extracted crates ship updates is just `Cargo.toml` edit + run
the integ smoke tests (`tests/{wasi_p2,distributed}_integration.rs`)
— they catch surface drift.
