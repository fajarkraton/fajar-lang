# Compass §5 remaining freeze candidates — B0 Findings

> **Phase:** Compass §5.1 closure beyond dep-types + GPU + SMT-verification.
> Candidates: algebraic effects + WASI P2 + distributed runtime (Raft).
> **Audit date:** 2026-05-12 EOS-38 (on `lanjutkan` first-step).
> **Plan Hygiene §6.8 R1:** Audit only. Strategic decision is Fajar's.
> **Predecessor:** Compass §5 freeze closures via Paths A/B/C
> (`docs/decisions/2026-05-12-{tensor-shapes,arrays-patterns,device-proofs,verify-path-b,verify-path-c}-deletion.md`).

## §1. Scope

Inventory remaining Compass §5.1 freeze candidates and determine which
can follow the Path A/B/C deletion pattern (zero-consumer or test-only).
Pre-flight only — no code changes.

## §2. Compass §5.1 verdicts (remaining)

From `docs/1/STRATEGIC_COMPASS.md` §5.1:

| Compass entry | Verdict |
|---|---|
| Algebraic effects + handlers | **Bekukan**. Pisah ke branch `effects-research`. Kembali setelah core v1.0 stabil. |
| WASI P2 component model | **Bekukan**. Tidak relevan untuk niche embedded. |
| Distributed runtime (Raft) | **Hapus dari core**. Tidak relevan untuk niche embedded. Jadikan side library. |

Plus the Decision Framework:
| Tambah algebraic effects? | ⏸️ Bekukan (research, post-v1.0) |

## §3. Module inventory (HEAD `79134a72`)

### 3.1 `src/distributed/` — 15,343 LOC, 322 tests across 16 files

Production consumers:
- **`src/main.rs:5814 fn cmd_run_cluster(path)`** — `run-cluster` CLI subcommand.
  Imports `distributed::raft::{RaftNode, RaftNodeId, RequestVoteReply}` +
  `distributed::scheduler::{DistributedTask, PlacementStrategy, TaskId,
  TaskLoadBalancer, TaskResources, WorkerId, WorkerNode}`.
- `src/interpreter/eval/mod.rs` — sprint N3 tests (n3_1..n3_4 + others)
  using `distributed::raft::RaftNode` and `distributed::discovery::DiscoveryRegistry`.
- `tests/nova_v2_tests.rs` — additional sprint coverage.

Status: **LIVE in production CLI.** Cannot be deleted blindly.

### 3.2 `src/wasi_p2/` — 13,791 LOC, 244 tests across 12 files

Production consumers:
- **`src/main.rs:5921 fn cmd_build_wasi_p2(path, output, verbose)`** —
  `build-wasi-p2` CLI subcommand. Imports `wasi_p2::component::{
  ComponentBuilder, ComponentFuncType, ComponentTypeKind, ComponentValType,
  ExportKind, validate_component}`.
- `src/interpreter/eval/mod.rs` — sprint N7 tests (n7_1..n7_4 + others)
  using `wasi_p2::composition::ComponentInstance`, `ComponentLinker`.

Status: **LIVE in production CLI.** Cannot be deleted blindly.

### 3.3 `src/analyzer/effects.rs` — 2,306 LOC, 53 tests (single file)

Production consumers:
- **`src/analyzer/type_check/check.rs`** — algebraic effects checked
  during type analysis (production analyzer hot path).
- **`src/analyzer/type_check/register.rs`** — effect registration.
- **`src/analyzer/type_check/mod.rs`** — module wire-up.
- `src/interpreter/eval/mod.rs` — runtime effect handling.
- `tests/effect_tests.rs` + `tests/error_code_coverage.rs`.

Status: **LIVE in production analyzer (`@kernel`/`@device`/`@safe` type
checking depends on effects analysis).** Cannot be deleted — would
break the type checker.

### 3.4 `src/wasi_v12.rs` — 395 LOC, 12 tests (single file, separate from wasi_p2)

Production consumers:
- `src/codegen/wasm/mod.rs` — WASM codegen path.

Status: **LIVE in production codegen.** Not specifically called out by
Compass §5.1 (older WASI version), but still production-load-bearing.

## §4. Aggregate

| Module | LOC | Tests | Status | Compass verdict | Can apply Path A/B/C? |
|---|---|---|---|---|---|
| `src/distributed/` | 15,343 | 322 | **LIVE** (CLI `run-cluster`) | Hapus dari core; jadikan side library | NO — needs extraction first |
| `src/wasi_p2/` | 13,791 | 244 | **LIVE** (CLI `build-wasi-p2`) | Bekukan; tidak relevan | NO — needs extraction first |
| `src/analyzer/effects.rs` | 2,306 | 53 | **LIVE** (analyzer type_check hot path) | Bekukan; effects-research branch | NO — load-bearing for safety analysis |
| `src/wasi_v12.rs` | 395 | 12 | **LIVE** (codegen/wasm) | (not in §5.1) | NO — codegen consumer |
| **Total** | **~31,835** | **631** | **All LIVE** | | **None deletable as-is** |

## §5. Key finding: §5 freeze pattern does NOT extend cleanly to these modules

Paths A/B/C succeeded because the Compass §5.1 verdict (Bekukan dep-types,
GPU non-PTX, SMT-verification) was **empirically backed by zero-consumer
reality** — the modules were research prototypes that never crossed the
research→production threshold. Deletion was honest cleanup.

For the remaining §5.1 candidates, the verdict is the same word ("Bekukan")
but the **empirical reality is opposite**: each has real production consumers
wired into CLI subcommands or the analyzer hot path.

Two scenarios fit:

### Scenario A: Compass §5.1 is right; current code over-extends scope
- Distributed, wasi_p2 CLI subcommands were built despite being out-of-niche.
- To honor §5.1, the user-visible features must be removed FIRST, then
  the modules deleted. That breaks documented capabilities.
- This is multi-week work per module: deprecation notice → next-release
  CLI removal → next-release module deletion. NOT a single-session task.

### Scenario B: Compass §5.1 is wrong; these are live features the language wants
- `@kernel`/`@device`/`@safe` context isolation DEPENDS on effects analysis
  (Compass §4.4 closure of v35.6.0 used effects internally). Algebraic
  effects ARE the niche-safety story.
- WASI P2 + distributed are bigger niche-mismatches but a non-trivial
  amount of work has been invested. Throwaway cost is high.
- Compass §5.1 verdict on these may need revision in a future Compass
  update rather than executed verbatim.

## §6. Three honest paths forward (Fajar to decide)

### Path D-effects-only (analyzer effects — Compass-conflict resolution)
Algebraic effects is the most strategically loaded. v35.6.0's `@safe`
default closure used effects analysis to track which contexts are
hardware-touching. Removing effects = breaking that closure.

Recommendation: **OVERRIDE Compass §5.1 for algebraic effects** —
write a decision file noting that effects.rs is load-bearing for the
context-safety story (Compass §4.4 closure) and the §5.1 freeze verdict
is suppressed. ~5min, no code change.

### Path E-wasi-p2 — multi-session extraction (per Compass §5.1)
Move `wasi_p2/` + `build-wasi-p2` CLI + integ tests to a standalone
repo (mirror of FajarQuant extraction pattern, 2026-04-11). Multi-session
project: deprecation phase + extraction phase + re-link via Cargo path
or git dep + CI re-glue.

Recommendation: **DEFER to a dedicated session.** Out of scope for
single-step `lanjutkan`.

### Path F-distributed — same multi-session extraction
Same shape as Path E for `distributed/`. Even bigger surface (15K LOC,
322 tests). Higher reward (Compass §5.1 explicitly says "Hapus dari core,
jadikan side library").

Recommendation: **DEFER.** Same reasoning as Path E.

### Path Z — accept current state, no action
Document the §5 verdicts as known-deviations in a single tracker file +
mark Compass §5.1 verdicts as "deferred per practical engineering reality"
for these three entries.

Recommendation: **VIABLE if Fajar wants to pause Compass §5 follow-ups
and switch to other work** (fajarquant Phase E, hardware-bound TQ12.6,
or new feature work).

## §7. Recommendation: Path D + Path Z

For this session's "lanjutkan" first-step:
1. **Path D** — Write a single decision file noting Compass §5.1 algebraic-effects
   freeze is **suppressed** because the v35.6.0 `@safe`-default context-safety
   closure makes effects.rs load-bearing. ~5-10min, clarifies one §5 entry.
2. **Path Z** — Acknowledge wasi_p2 + distributed extractions as deferred,
   note in the decision file that these are multi-session projects not
   single-step `lanjutkan` candidates.

Rationale: closes the §5 dimension cleanly without forcing multi-week
extraction work. Path E/F remain available for future dedicated sessions
when Fajar wants to invest 1-2 weeks per module.

## §8. Re-entry conditions (for Path E/F)

If/when Fajar decides to execute Path E (wasi_p2 extraction) or Path F
(distributed extraction):
1. Mirror FajarQuant extraction pattern (`fajarkraton/fajarquant` repo,
   ~2-3 days extraction + re-link).
2. Pre-flight: deprecation release with CLI subcommand warning
   ("`run-cluster` deprecated; will be removed in vNext").
3. Extraction: new repo + Cargo path/git dep + re-export shim.
4. Removal: next release after extraction stabilizes.
5. Each phase = Plan Hygiene §6.8 B0/plan/phased ship.

## §9. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists                          (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §3 grep commands)
[x] Prevention mechanism — Path D decision file documents Compass override (Rule 3 — at ship)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all live-verified)
[ ] Effort variance tagged in commit message               (Rule 5 — at ship time)
[ ] Decisions are committed files                          (Rule 6 — pending Fajar's choice)
[x] Public-artifact drift swept                            (Rule 7 — done EOS-38 stats refresh)
[x] Multi-repo state checked                               (Rule 8 — done earlier this session)
```

## §10. Source artifacts

- This file: `docs/COMPASS_5_FREEZE_REMAINING_B0_FINDINGS.md`
- Compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 + Decision Framework
- Predecessors: 4 successful Compass §5 closures this session
  (tensor_shapes/arrays+patterns/device_proofs/verify-paths)

---

*B0 written 2026-05-12 EOS-38. ~25min actual. Verdict: remaining §5.1
candidates (algebraic effects + WASI P2 + distributed) are ALL LIVE in
production CLI/analyzer — cannot be deleted via Path A/B/C pattern. Path D
(suppress §5.1 effects-freeze with decision file, ~5-10min) recommended
for this session. Path E/F (wasi_p2 + distributed repo extractions,
multi-session) deferred. Decision pending Fajar.*
