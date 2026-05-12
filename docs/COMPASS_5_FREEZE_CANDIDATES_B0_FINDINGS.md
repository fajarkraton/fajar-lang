# Compass §5 Freeze Candidates — B0 Findings

> **Phase:** Strategic Compass §5.1 audit (4-feature subset).
> **Audit date:** 2026-05-12 (EOS-31, post-Option-B closure).
> **Plan Hygiene §6.8 R1:** This B0 produces inventory data only. The actual
> strategic decision is Fajar's call, not Claude's.

## §1. Scope

The strategic compass at `docs/1/STRATEGIC_COMPASS.md` §5 ("Yang Harus
Dipangkas atau Dibekukan") proposes freezing/separating 11 features +
calibrating 7 README claims + resetting versioning to v0.x.y.

This B0 audits the **4 features the EOS-28 resume protocol surfaced**:
algebraic effects, dependent types, WASI P2, GPU codegen. The broader
§5.1 inventory (Distributed runtime, FajarOS Nova/Surya, GUI, HTTP server,
WebSocket/MQTT/BLE/database, SMT verification) is **deferred to a
follow-up audit**.

Per "first step only" rule, this B0 produces inventory data so Fajar
can make the strategic call; no code changes recommended.

## §2. Feature inventory (HEAD `e5290fab`)

### 2.1 Side-by-side numbers

| Feature | src LOC | lib tests | integ tests | internal dependents | Compass verdict |
|---|---|---|---|---|---|
| **Algebraic effects** (`src/analyzer/effects.rs`) | 2,306 | 0 | 100 (effect_tests.rs + effect_poly_tests.rs, 1,199 test LOC) | 1 file | **Bekukan** (move to `effects-research` branch) |
| **Dependent types** (`src/dependent/{mod,arrays,nat,patterns,tensor_shapes}.rs`) | 3,636 | 160 | 0 in dedicated `tests/dependent*` files | 4 files | **Bekukan** (move to `deptypes-research` branch; "mungkin tidak kembali") |
| **WASI P2** (`src/wasi_p2/*.rs`, 10 files) | 13,791 | 244 | 0 dedicated | 4 files | **Bekukan** (not relevant for embedded niche) |
| **GPU codegen** (`src/gpu_codegen/{ptx,spirv,metal,hlsl,fusion,gpu_memory,mod}.rs`) | 4,773 | 114 | 0 dedicated | 1 file | **Sederhanakan** (NPU SDK FFI more important than full GPU codegen for embedded) |
| **TOTAL** | **24,506** | **518** | **100** | — | — |

### 2.2 Per-feature breakdown

#### Algebraic effects (2,306 LOC + 100 integ)

- Single-file: `src/analyzer/effects.rs`
- Heavy test coverage in `tests/effect_tests.rs` (1,020 LOC) and
  `tests/effect_poly_tests.rs` (179 LOC) — 100 integ tests total
- Internal dependents: 1 (`src/analyzer/effects` is consumed by
  `src/analyzer/type_check/mod.rs`)
- Documented in CLAUDE.md as part of analyzer enhancements

**Compass verdict:** Bekukan, pisah ke branch `effects-research`. Come back after core v1.0.

**Blast if frozen:** Medium — analyzer loses effect-row machinery.
Existing programs that don't declare effects continue working. Programs
that DO use effects must be migrated or stay on a pre-freeze tag.

#### Dependent types (3,636 LOC + 160 lib)

- 5 files: `mod.rs` (7 LOC dispatch), `arrays.rs` (502), `nat.rs`
  (1,707, largest), `patterns.rs` (867), `tensor_shapes.rs` (553)
- `tensor_shapes.rs` is functionally adjacent to v0.5's compile-time
  tensor shape checking (one of the V1 selling points)
- Internal dependents: 4 (`const_generics.rs`, analyzer modules)

**Compass verdict:** Bekukan, pisah ke branch `deptypes-research`. "Mungkin tidak kembali."

**Blast if frozen:** HIGH if `tensor_shapes.rs` is load-bearing for `@device`
context's tensor checking. Audit needed before any freeze decision:
verify whether the 9 lib tests in `tensor_shapes.rs` are exercised by
the actual `@device fn` flow, or whether they're dead.

#### WASI P2 (13,791 LOC + 244 lib)

- **LARGEST candidate.** 10 files: component, composition, deployment,
  filesystem, http, resources, sockets, streams, wit_lexer, wit_parser
  (1,854 LOC), wit_types (767)
- 244 lib tests — substantial coverage
- Internal dependents: 4 (own module + 2 internal cross-refs + analyzer
  hook)

**Compass verdict:** Bekukan — not relevant for embedded niche.

**Blast if frozen:** LOW for the core compiler. WASI P2 is an isolated
target backend; freezing means dropping support for WIT/component output
without affecting interpreter, Cranelift, LLVM, or @kernel/@device.

**Biggest single trim:** removing this would cut ~14K LOC + 244 tests
from the codebase. Largest impact-per-decision in §5.1.

#### GPU codegen (4,773 LOC + 114 lib)

- 7 files: ptx.rs (1,113), spirv.rs (1,261), metal.rs (171), plus hlsl,
  fusion, gpu_memory, mod
- 114 lib tests
- Internal dependents: 1 (consumed by `src/codegen/llvm/` for LLVM
  device targeting)

**Compass verdict:** Sederhanakan (not freeze) — for embedded, NPU SDK
FFI more important than full GPU codegen.

**Blast if frozen:** Medium-HIGH. README has CUDA RTX 4090 + GPU compute
badges that depend on this. The CUDA path is operational (RTX 4090 test
verified). Removing breaks user-facing badges + invalidates the "GPU
codegen" claim.

**Best path per Compass:** keep PTX (NVIDIA path, real hw verified), drop
or freeze SPIRV/Metal/HLSL paths that lack user-facing validation.

## §3. Aggregate impact if all 4 are frozen as Compass §5.1 recommends

| Metric | Current | Post-freeze | Delta |
|---|---|---|---|
| src LOC | ~449,000 | ~424,494 | **-24,506 (-5.5%)** |
| lib tests | 7,633 | ~7,115 | **-518 (-6.8%)** |
| integ tests | 10,489 | ~10,389 | **-100 (-1.0%)** |
| Binary size | 18 MB | ~17 MB (if dependencies removed) | -1 MB |
| Code surface to maintain | 4 features wide | 4 features narrower | Significant ergonomic win for a 1-contributor project |

**Honest take:** Freezing all 4 reclaims meaningful surface area
(~5.5% LOC, ~6.8% test suite). For a solo-developer project, this is
a non-trivial discipline win.

## §4. Decision matrix for Fajar (not Claude)

For each of the 4 features, the call is:

| Action | What it means | Cost | When appropriate |
|---|---|---|---|
| **KEEP** | Status quo. Continue maintaining. | Ongoing maintenance burden. | Feature is genuinely load-bearing for the embedded AI niche. |
| **FREEZE in-place** | Stop adding new functionality. Bug-fixes only. Mark in CLAUDE.md as "frozen-pre-v1.0". | Cheapest. No code moves. | Feature works today but isn't a v1.0 priority. |
| **MOVE to research branch** | Cut from main, preserve in `<name>-research` branch. | Moderate (one cut commit + branch creation). | Compass §5.1's preferred verdict for effects + deptypes. |
| **EXTRACT to optional crate** | Move out of main crate; downstream users opt-in via Cargo feature. | High (refactor + dep wiring). | Feature has external value but doesn't belong in core. |
| **DELETE outright** | Remove from all branches. | Highest. Irreversible (modulo git history). | Feature has no users and no future value. |

### Recommended pairings (Claude's best-guess; Fajar overrides):

| Feature | Recommended | Rationale |
|---|---|---|
| Algebraic effects | **MOVE to `effects-research` branch** | Compass-aligned. 2,306 LOC + 100 tests is meaningful research but not v1.0-critical. |
| Dependent types | **AUDIT first, then likely FREEZE in-place** | `tensor_shapes.rs` may be load-bearing for @device. Don't cut blind. |
| WASI P2 | **EXTRACT to optional crate** or **MOVE to `wasi-research` branch** | Biggest trim. Self-contained module. Compass says not relevant for embedded — but the 244 tests pass and someone built it for a reason. Worth preserving accessibly. |
| GPU codegen | **SIMPLIFY** — keep PTX (RTX 4090 verified), freeze SPIR-V/Metal/HLSL | Compass-aligned. README CUDA badge stays. |

## §5. What this B0 does NOT decide

- **Does not pull the trigger on any freeze.** Just inventory.
- **Does not audit §5.1's other 7 features** (Distributed runtime,
  FajarOS Nova/Surya, GUI, HTTP server, WebSocket/MQTT/BLE/database,
  SMT verification). Each warrants its own row.
- **Does not audit §5.2 README claim calibration** (7 items).
- **Does not audit §5.3 versioning reset** (v35.6.0 → v0.5.0 per
  Compass; large operational decision).

## §6. Suggested next steps (for Fajar)

In rough priority order:

1. **Dependent types `tensor_shapes.rs` load-bearing audit** (~30min).
   Critical because freezing dependent types could break @device. If
   tensor_shapes is dead/replaceable, freeze is safe. If load-bearing,
   keep.
2. **WASI P2 extraction plan** (~1-2h). Biggest single trim (~14K LOC).
   If we want this gone, plan the cut carefully.
3. **GPU codegen simplification** (~1h). Keep PTX, drop SPIR-V/Metal/HLSL
   per Compass §5.1. Likely the easiest first ship.
4. **Algebraic effects research-branch move** (~1h). One file (2,306
   LOC) + 100 integ tests — clean cut.
5. **§5.1 broader audit** (the other 7 features). Same B0 template.
6. **§5.2 README calibration** (~1h). 7 claim adjustments.
7. **§5.3 versioning reset** — strategic decision. v35.6.0 → v0.5.0 is
   a major signaling move. Compass argues for it; impacts every public
   artifact.

## §7. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §8)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — N/A for audit-only B0)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all LOC/test counts verified live via wc + grep)
[ ] Effort variance tagged in commit message               (Rule 5 — at commit time)
[ ] Decisions are committed files                          (Rule 6 — decisions are Fajar's, not bundled here)
[x] Public-artifact drift swept                            (Rule 7 — done in R4 earlier this session)
[x] Multi-repo state checked                               (Rule 8 — R6 just done; all 3 repos in sync)
```

## §8. Verification commands

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Re-verify LOC per feature
wc -l src/analyzer/effects.rs
wc -l src/dependent/*.rs
wc -l src/wasi_p2/*.rs
wc -l src/gpu_codegen/*.rs

# Re-verify test counts
grep -rE "^[[:space:]]*#\[test\]" src/analyzer/effects.rs | wc -l   # 0 (in src)
grep -rE "^[[:space:]]*#\[test\]" src/dependent/ | wc -l            # 160
grep -rE "^[[:space:]]*#\[test\]" src/wasi_p2/ | wc -l               # 244
grep -rE "^[[:space:]]*#\[test\]" src/gpu_codegen/ | wc -l           # 114
grep -rE "^[[:space:]]*#\[test\]" tests/effect_*.rs | wc -l         # 100

# Internal dependents per feature
grep -rln "use crate::wasi_p2" src/ | wc -l    # 4
grep -rln "use crate::gpu_codegen" src/ | wc -l # 1
grep -rln "use crate::dependent" src/ | wc -l   # 4
grep -rln "use crate::analyzer::effects" src/ | wc -l  # 1
```

## §9. Source artifacts

- This file: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md`
- Source compass: `docs/1/STRATEGIC_COMPASS.md` §5 (lines 319-368)
- EOS-28 protocol reference: `memory/project_resume_lanjut_protocol.md` §2.C
- Prior session findings (Options A, B): `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md`, `docs/V35_7_PARSER_ANNOTATION_GRAMMAR_B0_FINDINGS.md`, `docs/V35_7_PHASE_2_B0_FINDINGS.md`

---

*B0 written 2026-05-12 EOS-31 session. ~25min actual. Inventory only;
strategic decisions deferred to Fajar. Broader §5.1 audit (7 more
features) + §5.2/§5.3 deferred to a follow-up session.*
