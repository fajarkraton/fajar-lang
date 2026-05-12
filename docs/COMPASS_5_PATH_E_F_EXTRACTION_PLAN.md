# Compass §5 Path E + F — wasi_p2 + distributed extraction plan

> **Status:** PLAN ONLY. Drafted 2026-05-12 EOS-38, post-Compass-§5-remaining-B0.
> **Execution:** Multi-session, ~7-12 days realistic (2 weeks worst case).
> **Triggered by:** Fajar 2026-05-12 — "Kita akan lanjutkan E dan F di sesi berikutnya."
> **Compass §5.1 verdicts driving this plan:**
> - distributed: *"Hapus dari core. Tidak relevan untuk niche embedded. Jadikan side library."*
> - wasi_p2: *"Bekukan. Tidak relevan untuk niche embedded."*
>
> Reference extraction pattern: **FajarQuant** (`fajarkraton/fajarquant`,
> extracted 2026-04-11 from monolithic fajar-lang via V26 Phase A4 split).

---

## §0. Why this plan exists

After the EOS-29..38 dead-code reclaim session, three classes of Compass §5
freeze candidates emerged:

| Class | Verdict | This session's action |
|---|---|---|
| Research code with zero consumers | Delete (Path A/B/C pattern) | ✅ Done — dep-types + GPU non-PTX + SMT-verification (14 modules, -15.6K LOC) |
| Live code requiring repo extraction | "Hapus dari core; jadikan side library" / "Bekukan" | **Path E + F (this plan)** |
| Live code with Compass internal conflict | (e.g., effects.rs vs §4.4 closure) | Path D — Compass override decision file (separate ~10min task) |

This plan covers the Class 2 work. It is NOT a single-session task. It is
NOT a `lanjutkan` candidate. It requires deliberate scheduling, repo
provisioning, and downstream-consumer awareness (anyone with a `run-cluster`
or `build-wasi-p2` CLI dependency).

---

## §1. Scope & non-scope

### In scope
- Extract `src/wasi_p2/` (13,791 LOC, 244 tests across 12 files) →
  new repo `fajarkraton/fajar-wasi-p2` (or similar; name TBD in Phase 0).
- Extract `src/distributed/` (15,343 LOC, 322 tests across 16 files) →
  new repo `fajarkraton/fajar-distributed` (or similar).
- Handle CLI subcommands `build-wasi-p2` + `run-cluster`:
  - Per Compass §5.1 distributed: remove from core (no longer a fajar-lang
    feature).
  - Per Compass §5.1 wasi_p2: "Bekukan" — ambiguous; pre-flight Phase 0
    Decision A determines whether CLI subcommand survives via routing to
    the extracted crate, or is removed entirely.
- Remove sprint N3 (distributed) + N7 (wasi_p2) tests from
  `src/interpreter/eval/mod.rs` (they consume the modules; will be
  replaced by integ tests against the extracted crates if useful).
- Remove or re-route `tests/nova_v2_tests.rs` tests touching `distributed`.
- Update `Cargo.toml` to drop or add path/git deps; update `lib.rs`
  `pub mod` declarations.
- Update CLAUDE.md, README, CHANGELOG with extraction notes.

### Out of scope
- Other Compass §5 candidates (algebraic effects — Path D; older WASI
  v12 — load-bearing for wasm codegen, stays in core).
- Re-implementation or feature work in the extracted crates after
  extraction. They start as faithful mirrors of fajar-lang's current
  code.
- crates.io publishing of the new crates (deferred until they stabilize;
  initial state is git-dep only).
- CI/CD provisioning for the new repos (a follow-up task post-extraction).

---

## §2. Reference pattern: FajarQuant extraction (2026-04-11)

Source: `MEMORY.md` Pinned facts + `Cargo.toml` line 24 + commit history.

| Stage | What happened |
|---|---|
| Pre-flight | V26 Phase A4 audit identified FajarQuant as a clearly-separable subsystem (algorithm/paper/data with its own lifecycle). |
| Repo creation | New repo `fajarkraton/fajarquant` created with the relevant LOC moved. |
| Cargo wire-up | fajar-lang's `Cargo.toml` now has: `fajarquant = { git = "https://github.com/fajarkraton/fajarquant", rev = "b05ecf17..." }`. Path dep available for local-dev iteration but NOT committed. |
| Re-export shim | fajar-lang re-exports FajarQuant symbols where needed (compat layer). |
| Integ tests | 16 integration tests in fajar-lang verify the wire-up still works against the extracted crate. |
| CI awareness | Comment in Cargo.toml: "Path dep during local dev; switch to git/version dep before publishing. Do NOT commit the path form — it breaks CI." |

This plan adapts this pattern for wasi_p2 + distributed. Differences:

| Aspect | FajarQuant | Path E/F |
|---|---|---|
| User-facing CLI | None (FajarQuant is library-only) | YES — `build-wasi-p2` + `run-cluster` subcommands; Phase 0 Decision A handles |
| Internal consumers | analyzer + interpreter via re-export | CLI subcommands + sprint N3/N7 tests in eval/mod.rs + nova_v2_tests.rs |
| Symbol surface | Algorithm types | ComponentBuilder/ComponentInstance/RaftNode/DistributedTask/etc — much wider |
| Test surface | 16 integ tests | 322 (distributed) + 244 (wasi_p2) = 566 internal tests + sprint N3/N7 |

---

## §3. Phase structure & effort estimates

Each phase opens with a B0 pre-flight (Plan Hygiene §6.8 R1) and closes
with a closure findings doc. Effort estimates include +25% surprise
budget per §6.8 R5 (higher-uncertainty phases get +30%).

### Phase 0 — Pre-flight & decision (Session 1, ~2-4h)

Before any repo creation, three decisions must be made and committed
as files (Plan Hygiene §6.8 R6):

- **D-0.1**: Repo names. Candidates: `fajar-wasi-p2` / `fajar-component-model`;
  `fajar-distributed` / `fajar-cluster`. Format: `fajarkraton/<name>`.
- **D-0.2**: CLI subcommand fate.
  - **Option α (clean)**: Remove `build-wasi-p2` + `run-cluster` from core
    entirely. Users who want them: depend on the new crate + build their
    own driver. Aligns with Compass §5.1 "tidak relevan untuk niche."
  - **Option β (routed)**: Keep CLI subcommand stubs in fajar-lang core
    that delegate to the new crates via `dep = "..."`. Preserves user
    API at cost of fajar-lang still pulling the crate.
  - **Option γ (replaced)**: Print a deprecation message pointing to the
    new repo + exit. Users update their workflow. Phased: this version
    prints warning; next version errors; release after removes.
  - **Recommended**: Option α for `distributed` (Compass says "Hapus dari
    core"), Option γ for `wasi_p2` (Compass says "Bekukan" — softer).
- **D-0.3**: Dep mode (path vs git). FajarQuant uses git+rev. For initial
  iteration during extraction, **path dep is acceptable** but must NOT
  be committed; commit form is `git = "..." rev = "..."` with a pinned rev.

**Deliverables:**
- `docs/decisions/2026-MM-DD-path-e-f-prep.md` (covers D-0.1, D-0.2, D-0.3).
- This plan updated with decided names.
- B0 doc updated (re-verify consumer counts unchanged since EOS-38).
- Multi-repo state snapshot (Plan Hygiene §6.8 R8).

**Gates:** decisions reviewed by Fajar; B0 numbers re-verified; multi-repo state clean.

### Phase E — wasi_p2 extraction (Sessions 2-4, ~3-5 days)

**Pre-flight:** Phase 0 closed. B0 audit: `docs/PATH_E_WASI_P2_EXTRACTION_B0_FINDINGS.md`.

| Sub-phase | Purpose | Effort (with +25% buffer) | Deliverable |
|---|---|---|---|
| **E.0** | B0 — full file inventory of `src/wasi_p2/`, symbol exposure map (which 50+ pub items are externally consumed?), API surface freeze | ~1.5h | B0 findings doc |
| **E.1** | Create `fajarkraton/<wasi-p2-repo-name>` on GitHub (Fajar action; outside Claude) + `git init` + initial Cargo skeleton | ~30min (Fajar) | New repo with empty fajar-lang-mirror structure |
| **E.2** | Move 12 source files to new repo. Verify `cargo build` in the new crate. Move 244 internal tests; verify they pass | ~4-6h | New repo at `fajar-lang-faithful-mirror-of-wasi_p2` tag |
| **E.3** | Add Cargo dep to fajar-lang: `wasi-p2 = { git = "...", rev = "<initial-commit>" }`. Add re-export shim in fajar-lang if needed for backwards compat | ~1.5h | fajar-lang builds + tests pass against extracted crate |
| **E.4** | Handle CLI subcommand per Phase 0 Decision D-0.2. If Option α: remove `cmd_build_wasi_p2` from main.rs + remove its CLI clap registration. If Option γ: replace fn body with deprecation print | ~1.5h | CLI behavior matches D-0.2 choice |
| **E.5** | Remove `src/wasi_p2/` directory from fajar-lang. Remove `pub mod wasi_p2;` from lib.rs. Remove sprint N7 tests from eval/mod.rs (n7_1..n7_4+ — verify exact range via grep) | ~2-3h | -13,791 LOC reclaim in fajar-lang |
| **E.6** | Integration test against extracted crate (mirror FajarQuant's 16-test pattern; 4-8 essential round-trip tests for component build path) | ~2-3h | New integration test file `tests/wasi_p2_integration.rs` |
| **E.7** | Closure: findings doc, decision file referenced, CHANGELOG entry, MEMORY.md update, multi-repo push | ~1.5h | All artifacts committed; fajar-lang in sync; new repo in sync |

**Phase E gates:**
- `cargo build` clean in BOTH repos
- `cargo test` green in BOTH repos
- `cargo clippy --lib -- -D warnings` clean in BOTH
- Phase17 self-host gate green in fajar-lang (Stage 2 byte-equality preserved — verify wasi_p2 was never referenced by stdlib/codegen/parser_ast/analyzer .fj)
- Multi-repo `git status -sb` clean for both repos

**Phase E effort total:** ~13-17h base + 25% buffer = ~16-21h realistic. Across 3-4 sessions of 3-5h each.

### Phase F — distributed extraction (Sessions 5-7, ~3-5 days, parallel to E)

Same structure as Phase E, scaled to distributed's larger surface.

| Sub-phase | Effort | Notes |
|---|---|---|
| **F.0** | B0 — file inventory, symbol map, audit of nova_v2_tests.rs consumers | ~2h |
| **F.1** | Repo creation (Fajar action) | ~30min |
| **F.2** | Move 16 source files + 322 tests | ~5-7h (larger than wasi_p2) |
| **F.3** | Cargo wire-up | ~1.5h |
| **F.4** | CLI handling per D-0.2 (Option α recommended: just delete `cmd_run_cluster`) | ~1.5h |
| **F.5** | Remove `src/distributed/` from fajar-lang, drop `pub mod distributed`, remove N3 sprint tests from eval/mod.rs + nova_v2_tests.rs touching distributed | ~3-4h (more eval/mod.rs sites than wasi_p2) |
| **F.6** | Integration tests against extracted crate | ~2-3h |
| **F.7** | Closure | ~1.5h |

**Phase F gates:** same as Phase E.

**Phase F effort total:** ~15-20h base + 25% buffer = ~19-25h realistic. Across 3-4 sessions.

### Phase G — Joint cleanup & re-validate (Session 8, ~3-5h)

After both extractions complete:

- Audit integ test counts (was 10,136 pre-extraction; predict ~9,600-9,800
  after sprint N3/N7 removal + ~100-200 new integ tests for cross-repo
  wire-up).
- Update CLAUDE.md §3 stats (test counts, LOC, module counts — drop -29K
  LOC across 28 source files; lib.rs `pub mod` count drops by 2).
- Update README to mention new crates as "embedded ML companion crates."
- Tag fajar-lang version: `v36.0.0 "Compass §5 closure"` (or appropriate
  semver — Path E + F removes 2 user-facing CLI subcommands, so MAJOR
  bump justified).
- Multi-repo state: ensure all 3 repos (fajar-lang + 2 new) are in sync,
  CI passing if configured.
- Memory housekeeping: archive Path E/F session notes in `memory/archive/`
  if MEMORY.md cap pressure.

**Phase G effort:** ~3-5h.

### Grand total

- **Phase 0:** 2-4h
- **Phase E:** 16-21h (3-4 sessions)
- **Phase F:** 19-25h (3-4 sessions)
- **Phase G:** 3-5h
- **TOTAL:** 40-55h across 7-12 sessions.

Calendar: 2-3 weeks of part-time work, or 1 week of full-time focus.

---

## §4. Symbol-surface freeze (CRITICAL — must be done at Phase 0)

Before any code movement, the externally-consumed API surface from each
module must be **frozen**. This is the contract the new crate must
preserve. Methodology:

### 4.1 wasi_p2 external API

Currently exposed via main.rs CLI:
```rust
use fajar_lang::wasi_p2::component::{
    ComponentBuilder, ComponentFuncType, ComponentTypeKind, ComponentValType,
    ExportKind, validate_component,
};
```

Plus via eval/mod.rs (test-only, removed in E.5):
```rust
use crate::wasi_p2::composition::ComponentInstance;
use crate::wasi_p2::composition::ComponentLinker;
use crate::wasi_p2::wit_parser::parse_wit;  // n10_9 already retained
```

**Phase 0 B0 must grep ALL imports across `src/`, `tests/`, `examples/`,
`benches/` to produce a complete pub-symbol export list.** No new symbol
should leak between freeze and extraction.

### 4.2 distributed external API

Currently exposed via main.rs CLI:
```rust
use fajar_lang::distributed::raft::{self, RaftNode, RaftNodeId, RequestVoteReply};
use fajar_lang::distributed::scheduler::{
    DistributedTask, PlacementStrategy, TaskId, TaskLoadBalancer,
    TaskResources, WorkerId, WorkerNode,
};
```

Plus eval/mod.rs sprint N3 tests + nova_v2_tests.rs (test-only).

Same exhaustive grep required at F.0.

---

## §5. Decision matrix (Phase 0 — file as `docs/decisions/<date>-path-e-f-prep.md`)

### D-0.1 — Repo names

| Candidate | Pro | Con | Recommendation |
|---|---|---|---|
| `fajar-wasi-p2` | Specific; matches Compass | Less catchy | ✓ |
| `fajar-component-model` | Broader scope future-friendly | Implies broader contract than warranted | |
| `fajar-distributed` | Specific | Less catchy | ✓ |
| `fajar-cluster` | Catchy | Narrower than module surface | |

**Recommendation:** `fajar-wasi-p2` + `fajar-distributed` for precision.

### D-0.2 — CLI fate

| Module | Option α (remove) | Option β (route to crate) | Option γ (deprecate + warn) | Compass alignment |
|---|---|---|---|---|
| wasi_p2 | Most aggressive | Preserves user workflow; fajar-lang still depends | Bridges users gracefully | §5.1 "Bekukan" softer → **γ** |
| distributed | Aligns with "Hapus dari core" | Preserves; against verdict | Gradual | §5.1 "Hapus dari core" firm → **α** |

**Recommendation:** wasi_p2 → γ (3-version deprecation: warn → error → remove), distributed → α (remove immediately at Phase F.5).

### D-0.3 — Dep mode

Same as FajarQuant: git+rev for committed Cargo.toml, path for local-dev only. Never commit the path form.

---

## §6. Prevention layer per phase (Plan Hygiene §6.8 R3)

Each phase ships with a prevention mechanism:

| Phase | Prevention added |
|---|---|
| **Phase 0** | `scripts/check-no-path-deps.sh` pre-commit hook — fails if Cargo.toml has any `{ path = "../..." }` committed form. Mirrors FajarQuant's manual rule. |
| **Phase E** | `tests/wasi_p2_integration.rs` — round-trip test the extracted crate's API. CI runs on every push. |
| **Phase F** | `tests/distributed_integration.rs` — same pattern. |
| **Phase G** | `scripts/verify-cli-deprecation.sh` — for Option γ (wasi_p2), confirm deprecation message renders correctly + exit code is non-zero or warning-marked per chosen behavior. |

---

## §7. Verification commands (per phase, runnable, literal)

### Phase 0

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Re-verify external consumer counts haven't changed
grep -rln "use crate::wasi_p2\|use fajar_lang::wasi_p2" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "src/wasi_p2/"
# expect: src/main.rs, src/interpreter/eval/mod.rs

grep -rln "use crate::distributed\|use fajar_lang::distributed" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "src/distributed/"
# expect: src/main.rs, src/interpreter/eval/mod.rs, tests/nova_v2_tests.rs

# Multi-repo state
for r in ~/Documents/Fajar\ Lang ~/Documents/fajaros-x86 ~/Documents/fajarquant; do
    (cd "$r" && echo "=== $r ===" && git status -sb)
done
```

### Phase E (per sub-phase)

```bash
# E.2 — new repo builds clean
cd ~/Documents/fajar-wasi-p2
cargo build && cargo test  # expect 244+ tests pass

# E.3 — fajar-lang builds against extracted crate
cd "/home/primecore/Documents/Fajar Lang"
cargo build --lib  # expect clean

# E.5 — removal verification
grep -rn "wasi_p2" src/ | grep -v "src/codegen/wasm\|wasi_v12"
# expect: empty (only kept items are wasm codegen which uses wasi_v12)

# E.6 — integration tests
cargo test --test wasi_p2_integration  # expect 4-8 passed
```

### Phase F (per sub-phase)

Same shape, substitute `distributed` for `wasi_p2`.

### Phase G — joint gates

```bash
# Stats refresh
cargo test --lib 2>&1 | tail -3                # expect 7,211 - 244 - 322 = ~6,645 lib (drop varies by what sprint tests had)
cargo test --tests --no-fail-fast 2>&1 | grep "test result" | \
    awk -F'passed; ' '{s+=$1} END {print s}'   # expect 10,136 - (sprint N3 + sprint N7 + nova_v2 touches)

# Phase17 byte-equality stays green
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1  # 4/4

# Multi-repo sync
for r in ~/Documents/Fajar\ Lang ~/Documents/fajar-wasi-p2 ~/Documents/fajar-distributed; do
    (cd "$r" && git status -sb)
done
```

---

## §8. Risk register

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Hidden symbol leak (some test imports a wasi_p2/distributed symbol we missed) | MED | Build break in fajar-lang post-removal | Exhaustive grep at E.0/F.0; symbol-surface freeze committed before move |
| `nova_v2_tests.rs` has more distributed-touching tests than expected | MED-HIGH | Phase F.5 boundary creep (echoes Path C eval/mod.rs surprise of +10 tests) | Pre-flight at F.0 produces complete test list; commit message tags variance |
| Stage 2 byte-equality breaks | LOW | Phase17 fails; rollback required | Pre-flight verifies `grep -r "wasi_p2\|distributed" stdlib/` returns empty (likely already empty since both are user-facing Rust-only) |
| CLI users disrupted by α/γ decision | MED | Public-facing breakage | Phase 0 D-0.2 documented in CHANGELOG + README at Phase G; major version bump to v36.0.0 signals breaking change |
| Cargo.lock churn breaking CI | LOW | Build flake | FajarQuant precedent shows manageable; commit Cargo.lock atomically with each phase |
| Extraction takes longer than estimated | HIGH | Calendar slip | +25% buffer baked in; if >50% over, pause and re-plan rather than push through |
| Cross-repo git history confusion | MED | Future archaeology pain | Use `git filter-repo` or `git subtree split` at E.2/F.2 to preserve relevant history; document the extraction commit hash both ways |

---

## §9. Decision gates (mechanical, per Plan Hygiene §6.8 R6)

Each gate produces a committed file that pre-commit/CI checks can verify.

| Gate | Trigger | File | Pre-commit hook check |
|---|---|---|---|
| G-1: Phase 0 closed | All 3 decisions filed | `docs/decisions/<date>-path-e-f-prep.md` | Hook checks the file exists before allowing any Phase E/F edits |
| G-2: Phase E.5 ready | wasi_p2 fully mirrored to new repo + new repo passes tests | `docs/PATH_E_WASI_P2_EXTRACTION_FINDINGS.md` (mid-phase WIP) | Hook checks no live `pub mod wasi_p2;` in lib.rs while extraction findings still in WIP state |
| G-3: Phase E closed | E.7 closure findings doc committed + multi-repo state clean | `docs/PATH_E_WASI_P2_EXTRACTION_FINDINGS.md` (closure version) | (No mechanical check; manual review) |
| G-4..G-6 | Mirror G-1..G-3 for Phase F | `docs/PATH_F_DISTRIBUTED_EXTRACTION_FINDINGS.md` | Mirror G-2 |
| G-7: Phase G closed | All public-artifact updates applied; tag created | `CHANGELOG.md` v36.0.0 entry + `git tag v36.0.0` | Pre-tag hook re-runs full self-host gate |

---

## §10. Re-entry conditions

If Path E or F is paused mid-stream:

| Paused at | Re-entry checklist |
|---|---|
| Mid-E.2 | New repo has partial files; fajar-lang unchanged. Resume by completing remaining files. |
| Post-E.3, pre-E.5 | fajar-lang depends on extracted crate AND still has local `src/wasi_p2/`. Dual-source — must resolve which is authoritative before next edit. Recommend: complete E.5 (remove local) ASAP. |
| Post-E.5, pre-E.6 | wasi_p2 fully extracted but no integ test in fajar-lang. Risk: wire-up regressions go undetected. Add E.6 before any other fajar-lang work. |
| Post-E.7, mid-F | E shipped cleanly. F can resume independently. |

Per-session resume protocol: read `docs/PATH_E_F_EXTRACTION_PLAN.md`
(this file) + last phase findings doc + run §7 verification commands
for current phase.

---

## §11. Self-check (§6.8 audit checklist for this PLAN)

```
[x] Pre-flight audit (B0) exists                                  (Rule 1 — predecessor `COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md`)
[x] Every task has runnable verification command                  (Rule 2 — §7)
[x] Prevention mechanism per phase                                (Rule 3 — §6)
[ ] Agent-produced numbers cross-checked with Bash                (Rule 4 — at each phase ship)
[x] Effort variance tagged: +25% buffer per phase                 (Rule 5)
[x] Decisions are committed files (G-1..G-7)                      (Rule 6 — §9)
[x] Public-artifact sync addressed (Phase G CHANGELOG/README/tag) (Rule 7)
[x] Multi-repo state check in §7                                  (Rule 8)
```

---

## §12. Predecessor patterns & lessons

### From FajarQuant extraction (2026-04-11)
1. Path dep is the right local-dev iteration tool; never commit it.
2. Pinning to a specific rev in the committed git dep avoids surprise
   upstream breakage.
3. Re-export shim (if needed) goes in a single file in fajar-lang
   (`src/<old-name>.rs` re-exports `pub use <new-crate>::*`); this
   lets internal consumers migrate gradually.
4. Integ tests are the contract verification — keep them small and
   focused on round-trip API behavior, not deep functionality.

### From Path A/B/C this session (2026-05-12)
1. **B0 grep MUST be exhaustive.** Path C surfaced 10 unexpected smt-consumer
   tests beyond the initial n1/n4/n10 scan because the actual naming
   pattern was *_smt_* across more sprints. Phase 0 must do a complete
   grep across all sprint prefixes.
2. **Estimate variance up to +900% on architectural detour** (v35.6.0 A.4
   `str_byte_at` carve-out). For each phase, when the +25% buffer is hit,
   pause and re-plan rather than push through.
3. **Pre-push hook is the safety net.** Self-host gate has caught
   regressions multiple times; keep it required at every phase ship.
4. **Honesty upfront in commit messages.** When B0 estimates miss, tag
   the variance explicitly (`[actual ~Xh, est ~Yh, ±Z%]`).

### From v35.6.0 A.4 (2026-05-10)
1. **Architectural detours surface during execution, not B0.** If
   extraction reveals an unexpected coupling (e.g., wasi_p2 imports from
   `crate::ffi_v2` which itself imports from somewhere unexpected),
   surface the gap honestly and pause-replan; don't push through.
2. **`use` cycles between extracted and core are deal-breakers.** Pre-flight
   must verify the extracted crate has ZERO `use crate::<not-self>::*`
   imports. If found, those have to be inverted or refactored before
   extraction is possible.

---

## §13. Authority & changelog

- Plan author: Claude Opus 4.7 (this session)
- Approver: Fajar Putranto ("Kita akan lanjutkan E dan F di sesi berikutnya, buatkan plan yang komprehensif dan lengkap")
- Plan version: 1.0 (2026-05-12)
- Revision triggers: Phase 0 decisions can revise §5; any unexpected
  coupling found in a B0 can revise §3 effort estimates.

---

*Plan written 2026-05-12 EOS-38, post-Compass-§5-remaining-B0 (commit `eb3a3c25`).
Captures full multi-session extraction strategy for wasi_p2 + distributed.
Execution begins next session per Fajar direction. Plan Hygiene §6.8 R1-R8 honored throughout.*
