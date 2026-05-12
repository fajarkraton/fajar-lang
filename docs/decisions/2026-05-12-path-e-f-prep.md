# Decision: Path E + F Phase 0 — pre-extraction decisions (D-0.1, D-0.2, D-0.3)

**Date:** 2026-05-12 (EOS-41, Phase 0 closure of `COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md`)
**Decider:** Fajar Putranto — "lanjutkan sesuai dengan rekomendasi"
  (executing Phase 0 first-step per plan §3)
**Status:** Phase 0 logic CLOSED. Awaiting Fajar's GitHub repo registration (D-0.1 codename approval).
**Plan:** `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md` (commit `8cdbad97`)
**Predecessor B0:** `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md` (commit `eb3a3c25`)

## Context

Extracting `src/wasi_p2/` (13,791 LOC, 244 tests) + `src/distributed/`
(15,343 LOC, 322 tests) to standalone repos per Compass §5.1 verdicts:

- distributed: *"Hapus dari core. Jadikan side library."*
- wasi_p2: *"Bekukan. Tidak relevan untuk niche embedded."*

Both modules are LIVE in production (CLI subcommands + interpreter
sprint tests). Path A/B/C single-session deletion pattern does not
apply. Multi-session extraction following FajarQuant precedent
(2026-04-11) is required.

This decision file commits the three Phase 0 decisions defined in
plan §5.

---

## §1. Pre-flight re-verification (Phase 0.1 — state unchanged)

Re-ran consumer-trace grep commands from plan §7 at HEAD `8cdbad97`:

```
$ grep -rln "use crate::wasi_p2\|use fajar_lang::wasi_p2" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "src/wasi_p2/"
src/interpreter/eval/mod.rs
src/main.rs

$ grep -rln "use crate::distributed\|use fajar_lang::distributed" \
    src/ tests/ examples/ benches/ stdlib/ | grep -v "src/distributed/"
src/interpreter/eval/mod.rs
src/main.rs
tests/nova_v2_tests.rs
```

**Real production-consumer state UNCHANGED since EOS-38 B0.**

Bonus finding: `examples/aspirational/distributed_mnist.fj` references
`use distributed::{data_parallel, allreduce, checkpoint}` — but this is
**Fajar Lang stdlib namespace** (.fj source-level), NOT Rust
`src/distributed/`. It's an aspirational example (per `aspirational/`
directory naming), intentionally not currently runnable. Treatment in
plan: noted as a Phase F deletion candidate; not a wire-up consumer.

## §2. Multi-repo state snapshot (Phase 0.2 — Plan Hygiene §6.8 R8)

```
/home/primecore/Documents/Fajar Lang:  ## main...origin/main (0/0)
/home/primecore/Documents/fajaros-x86: ## main...origin/main (0/0)
/home/primecore/Documents/fajarquant:  ## main...origin/main (0/0)  [untracked: logs/]
```

All 3 repos clean + in sync with origin/main. Untracked `logs/` in
fajarquant is pre-existing (per MEMORY.md fajarquant note).

## §3. Symbol-surface freeze (Phase 0.3 — plan §4)

### 3.1 wasi_p2 externally-imported contract (FROZEN)

Consumers grep-verified across `src/main.rs` + `src/interpreter/eval/mod.rs`.
The extracted crate MUST preserve these imported symbol paths:

| Module | Symbol | Consumer |
|---|---|---|
| `component` | `ComponentBuilder` | main.rs cmd_build_wasi_p2 |
| `component` | `ComponentFuncType` | main.rs |
| `component` | `ComponentTypeKind` | main.rs |
| `component` | `ComponentValType` | main.rs |
| `component` | `ExportKind` | main.rs + eval/mod.rs (n7 sprint) |
| `component` | `validate_component` | main.rs |
| `composition` | `ComponentInstance` | eval/mod.rs (n7 sprint) |
| `composition` | `ComponentLinker` | eval/mod.rs (n7 sprint) |
| `composition` | `ExportDef` | eval/mod.rs (n7 sprint) |
| `composition` | `ComponentAdapter` | eval/mod.rs (n7 sprint) |
| `wit_parser` | `parse_wit` | eval/mod.rs (n10_9 — KEPT in Path C) |

Internal wasi_p2 sub-modules (kept for completeness of extracted crate):
`component`, `composition`, `deployment`, `filesystem`, `http`,
`resources`, `sockets`, `streams`, `wit_lexer`, `wit_parser`,
`wit_types` (11 sub-modules total).

### 3.2 distributed externally-imported contract (FROZEN)

Consumers grep-verified across `src/main.rs` + `src/interpreter/eval/mod.rs` +
`tests/nova_v2_tests.rs`. The extracted crate MUST preserve these:

| Module | Symbol | Consumer |
|---|---|---|
| `raft` | `RaftNode`, `RaftNodeId`, `RaftRole`, `RequestVoteReply` | main.rs cmd_run_cluster + eval/mod.rs + nova_v2_tests.rs |
| `scheduler` | `DistributedTask`, `PlacementStrategy`, `TaskId`, `TaskLoadBalancer`, `TaskResources`, `WorkerId`, `WorkerNode` | main.rs cmd_run_cluster |
| `cluster` | `ClusterNodeId`, `FailureDetector`, `WorkQueue` | eval/mod.rs (n3 sprint) |
| `discovery` | `DiscoveryMode`, `DiscoveryRegistry`, `ServiceInstance`, `SwimState` | eval/mod.rs + nova_v2_tests.rs |

Internal distributed sub-modules (kept for completeness):
`cluster`, `data_plane`, `deploy`, `discovery`, `dist_bench`,
`fault_tolerance`, `fault_tolerance_v2`, `ml_training`, `raft`,
`rpc`, `rpc_v2`, `scheduler`, `security`, `tensors`, `transport`
(15 sub-modules total).

### 3.3 Freeze enforcement

Between this decision and Phase E.5 / F.5 (the actual `pub mod` removal),
**no new external import** of either module's symbols should be added.
Phase E/F B0s will re-run the grep and any new imports caught are scope
expansion that requires plan revision.

---

## §4. Decision D-0.1 — Repo names

**Recommendation accepted:** `fajarkraton/fajar-wasi-p2` + `fajarkraton/fajar-distributed`.

Rationale:
- Precision over breadth — names match Compass §5.1 entries verbatim.
- `fajar-` prefix matches the `fajarkraton/` org convention (mirrors
  `fajarkraton/fajarquant`).
- Alternatives considered + rejected:
  - `fajar-component-model` — broader than warranted; if a future
    component-model effort extends beyond WASI P2, that's a new crate.
  - `fajar-cluster` — narrower than the module surface (distributed
    contains RPC + transport + ML training + data plane, not just
    cluster).

**Fajar handoff required:** Create the two empty repos on GitHub under
`fajarkraton/` org. Suggested initial state: README only + Apache-2.0
LICENSE (mirroring fajar-lang's licensing). Once repos exist, Phase E.1
+ F.1 can proceed with `git push -u`.

## §5. Decision D-0.2 — CLI subcommand fate per module

### D-0.2-distributed: Option α (REMOVE)

Per Compass §5.1 distributed verdict: *"Hapus dari core. Jadikan side library."*

Action at Phase F.4:
- Delete `fn cmd_run_cluster(path: &PathBuf)` from `src/main.rs:5814`.
- Remove the clap subcommand registration (line ~450).
- README + CHANGELOG note: "`run-cluster` removed in v36.0.0. Migrate
  to `fajar-distributed` crate's standalone CLI."
- Major-version bump justifies the removal.

Rationale: Compass §5.1 is explicit ("Hapus dari core"). Soft-deprecation
(Option γ) would contradict the verdict for distributed.

### D-0.2-wasi_p2: Option γ (DEPRECATE + WARN, 3-version cycle)

Per Compass §5.1 wasi_p2 verdict: *"Bekukan. Tidak relevan untuk niche embedded."*

The "Bekukan" verdict is softer than distributed's "Hapus dari core."
Embedded users may have built `build-wasi-p2` into workflows. Three-version
deprecation:

- **v36.0.0** (this extraction): `cmd_build_wasi_p2` body prints:
  `WARN: build-wasi-p2 is deprecated; install fajar-wasi-p2 crate. This CLI subcommand will error in v37 and be removed in v38.`
  Then routes to `fajar_wasi_p2::component::*` via the new dep
  (works as before; warning is informational).
- **v37.0.0**: same body but exits with non-zero (error tone, no
  functional change).
- **v38.0.0**: `cmd_build_wasi_p2` and its clap registration removed
  entirely.

Action at Phase E.4: only the v36.0.0 step. v37/v38 deferred to future
releases.

### Why asymmetric

distributed has clearer Compass verdict ("Hapus") and likely smaller
user base (run-cluster is a heavy 4-worker training workflow). wasi_p2
"Bekukan" is softer and likely has a broader user base (component-model
build tooling). Asymmetric treatment honors both verdicts at their
respective severities.

## §6. Decision D-0.3 — Dep mode

**Recommendation accepted:** git+rev mode (mirrors FajarQuant).

`Cargo.toml` committed form (template, exact rev TBD post-Phase-E.1/F.1):
```toml
fajar-wasi-p2 = { git = "https://github.com/fajarkraton/fajar-wasi-p2", rev = "<sha>" }
fajar-distributed = { git = "https://github.com/fajarkraton/fajar-distributed", rev = "<sha>" }
```

Local-dev path form (NEVER COMMIT):
```toml
fajar-wasi-p2 = { path = "../fajar-wasi-p2" }
fajar-distributed = { path = "../fajar-distributed" }
```

Prevention: pre-commit hook `scripts/check-no-path-deps.sh` (added in
plan §6 Phase 0 prevention layer) will fail on any path-form commit.

---

## §7. Phase 0 closure status

| Task | Status |
|---|---|
| 0.1 Pre-flight re-verify | ✅ Done (§1) |
| 0.2 Multi-repo state snapshot | ✅ Done (§2) |
| 0.3 Symbol-surface freeze | ✅ Done (§3) |
| 0.4 Decisions D-0.1 + D-0.2 + D-0.3 | ✅ Done (§4 + §5 + §6) |
| 0.5 Commit + push + MEMORY.md handoff | ⏳ In progress (this commit) |
| **Fajar handoff: create 2 empty repos on GitHub** | ⏸️ AWAITING — required before Phase E.1 / F.1 |

## §8. Phase 0 prevention layer (per plan §6 R3)

Hook to add at Phase E.1 (when fajar-lang first acquires the new
deps): `scripts/check-no-path-deps.sh`

```bash
#!/usr/bin/env bash
# Prevention: never commit path-form Cargo.toml deps for extracted crates.
set -euo pipefail
if grep -qE '^(fajar-wasi-p2|fajar-distributed)\s*=\s*\{\s*path\s*=' Cargo.toml; then
    echo "ERROR: Cargo.toml has path-form dep for extracted crate." >&2
    echo "Switch to git+rev form before committing." >&2
    exit 1
fi
```

Wired into `.git/hooks/pre-commit` at Phase E.1.

## §9. Re-entry conditions (if next session pauses Phase 0)

If next session opens with Phase 0 incomplete:
- This decision file exists → D-0.1/0.2/0.3 already committed; skip §4-§6.
- Resume by checking GitHub for the 2 new repos. If exist → proceed to
  Phase E.1 / F.1 in parallel. If not → re-confirm with Fajar before
  proceeding.

## §10. Verification commands

```bash
# Re-confirm Phase 0 state (this decision file present + repos NOT yet committed as deps)
cd "/home/primecore/Documents/Fajar Lang"
test -f docs/decisions/2026-05-12-path-e-f-prep.md && echo "✓ decision committed"
grep -q "fajar-wasi-p2\|fajar-distributed" Cargo.toml && echo "✗ deps already added (unexpected at Phase 0)" || echo "✓ no extracted-crate deps yet"

# Repo existence check (Fajar handoff verification)
gh repo view fajarkraton/fajar-wasi-p2 --json name 2>&1 | head -1
gh repo view fajarkraton/fajar-distributed --json name 2>&1 | head -1
```

## §11. Sign-off

- This file: `docs/decisions/2026-05-12-path-e-f-prep.md`
- Plan reference: `docs/COMPASS_5_PATH_E_F_EXTRACTION_PLAN.md`
- B0 predecessor: `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md`
- Companion this session: 23 commits, ~-15.6K LOC dead-code reclaim
- Fajar action pending: 2 GitHub repos to create

*Phase 0 logic complete 2026-05-12 EOS-41. Next step: Fajar creates
the 2 empty repos on GitHub. Phase E.1 + F.1 (cargo-skeleton + first-push
work) blocked until then.*
