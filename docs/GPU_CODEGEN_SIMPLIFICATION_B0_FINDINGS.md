# GPU Codegen Simplification — B0 Findings

> **Phase:** Strategic Compass §5.1 implementation — GPU codegen "Sederhanakan".
> **Audit date:** 2026-05-12 (EOS-31, post-Compass §5 B0).
> **Plan Hygiene §6.8 R1:** Pre-flight audit before any code work.

## §1. Scope

Compass §5.1 verdict for GPU codegen: **Sederhanakan** (simplify), not
freeze. Specifically: "Untuk niche embedded, NPU SDK FFI lebih penting
daripada full GPU codegen."

This B0 audits what "simplify" concretely means: which backend paths
should be cut, what tests/uses depend on them, and how much cleanup is
required.

The earlier Compass §5 B0 (`docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md`)
estimated GPU codegen at 4,773 LOC + 114 tests as a single block. This
B0 refines that estimate by per-file inventory.

## §2. Per-file inventory (HEAD `22ae5c61`)

### 2.1 Numbers

| File | LOC | tests | Role |
|---|---|---|---|
| `src/gpu_codegen/mod.rs` | 558 | 6 | GpuIr definition + lowering + helpers |
| `src/gpu_codegen/ptx.rs` | 1,113 | 29 | **NVIDIA PTX backend (RTX 4090 verified)** |
| `src/gpu_codegen/spirv.rs` | 1,261 | 33 | Vulkan SPIR-V backend |
| `src/gpu_codegen/metal.rs` | 171 | 6 | Apple Metal Shading Language |
| `src/gpu_codegen/hlsl.rs` | 162 | 6 | DirectX HLSL |
| `src/gpu_codegen/fusion.rs` | 708 | 17 | **Backend-agnostic kernel fusion** |
| `src/gpu_codegen/gpu_memory.rs` | 800 | 17 | **Backend-agnostic memory mgmt** |
| **TOTAL** | **4,773** | **114** | — |

### 2.2 Backend-agnostic helpers (KEEP regardless of decision)

- `fusion.rs` (708 LOC + 17 tests): no spirv/metal/hlsl/ptx references.
  Pure GPU IR fusion logic. Useful for any backend.
- `gpu_memory.rs` (800 LOC + 17 tests): no backend-specific refs. Pure
  device-memory allocator + transfer logic.

**Keep both.** They support PTX (which stays) and are independent of
the freeze decision.

### 2.3 PTX (KEEP per Compass alignment)

`ptx.rs` (1,113 LOC, 29 tests). RTX 4090 verified (per README CUDA badge).
Compass §5.1 says "simplify" — NPU FFI more important than full GPU. But
PTX is the operational NVIDIA path. **Keep.**

### 2.4 SPIR-V / Metal / HLSL (the cut candidates)

#### spirv.rs (1,261 LOC, 33 tests)
- Largest single backend. 33 tests is substantial research.
- Vulkan / cross-platform GPU compute.
- Interpreter test integration: `src/interpreter/eval/mod.rs` has
  ~16 `use crate::gpu_codegen::spirv::*` lines, ALL inside `#[test]`
  functions (verified via context check at L6820-6830). No production
  interpreter code depends on SpirV.

#### metal.rs (171 LOC, 6 tests)
- Small. Apple Metal Shading Language emission.
- Tests: v14_gs3_1..v14_gs3_5 + v14_gs3_1_metal_cli_e2e.
- No external dependents (no `.fj` examples use it, no production code).

#### hlsl.rs (162 LOC, 6 tests)
- Small. DirectX HLSL emission.
- Same pattern as metal.rs.

### 2.5 mod.rs helpers tied to metal/hlsl

`gpu_codegen/mod.rs` has 2 helpers:
- L403: `fn gpu_expr_to_metal(expr: &GpuExpr) -> String`
- L423: `fn gpu_expr_to_hlsl(expr: &GpuExpr) -> String`

Plus 2 tests: `v14_gpu_ir_to_metal` (L479), `v14_gpu_ir_to_hlsl` (L496).

These are entangled with metal/hlsl — if those backends go, these
helpers + tests go with them.

### 2.6 Test that asserts all 4 backend files EXIST

`tests/nova_v2_tests.rs:603`:
```rust
fn v14_n6_6_gpu_codegen_all_backends() {
    assert!(std::path::Path::new("src/gpu_codegen/spirv.rs").exists());
    assert!(std::path::Path::new("src/gpu_codegen/ptx.rs").exists());
    assert!(std::path::Path::new("src/gpu_codegen/metal.rs").exists());
    assert!(std::path::Path::new("src/gpu_codegen/hlsl.rs").exists());
}
```

This test must be updated whenever any backend file is removed.

### 2.7 .fj-level dependencies

Grep `examples/` + `stdlib/` for `SpirV`/`Metal`/`HLSL` (case-sensitive,
word-boundary). Matches found are all false positives ("Bare-Metal"
hyphenated). **No .fj-level code depends on SpirV/Metal/HLSL.**

## §3. Three concrete action paths

### Action A — Full delete of SpirV + Metal + HLSL

**Scope:**
- Delete: spirv.rs (1,261), metal.rs (171), hlsl.rs (162) = **1,594 LOC**
- Edit mod.rs: remove `gpu_expr_to_metal`, `gpu_expr_to_hlsl`, and their
  2 tests = ~30 LOC
- Edit eval/mod.rs: remove the ~14 `#[test]` fns that import `SpirVModule`
  (gs1_*, gs2_*, gs3_*, gs4_* sprint tests). Count via grep:

```bash
grep -cE "use crate::gpu_codegen::spirv" src/interpreter/eval/mod.rs  # 16
```
≈ ~14-16 test fns to remove (some import lines may share fns).
- Edit nova_v2_tests.rs: rewrite `v14_n6_6_gpu_codegen_all_backends` to
  assert only PTX exists.

**Aggregate:** ~1,624 LOC removed + ~55-60 tests removed.
**Effort:** ~1-1.5h.
**Risk:** Medium. Test removal is the main churn. Compile errors if any
import dangles.

### Action B — Freeze in place (no code change)

**Scope:**
- Add module-level deprecation comments to spirv.rs, metal.rs, hlsl.rs:
  ```rust
  //! FROZEN: per Strategic Compass §5.1 (2026-05-12). Not under active
  //! development. Re-entry conditions: a verified user surfaces.
  ```
- Document in CLAUDE.md §5 or a new "Frozen modules" section.
- Optionally: gate behind a `gpu-extended` Cargo feature so they don't
  build by default. (Adds ~1h Cargo wiring.)

**Aggregate:** 0 LOC removed. Documentation only.
**Effort:** ~10min (just comments) or ~1h (with Cargo feature gating).
**Risk:** Lowest. No deletion = nothing to break.

### Action C — Mixed (Recommended)

**Scope:**
- DELETE metal.rs + hlsl.rs (small, 6 tests each, ~341 LOC combined).
- DELETE mod.rs `gpu_expr_to_metal` + `gpu_expr_to_hlsl` + their 2 tests.
- FREEZE spirv.rs in place with deprecation comment header (preserve 33
  tests + 1,261 LOC of researched work — too valuable to blind-delete).
- UPDATE nova_v2_tests `v14_n6_6_gpu_codegen_all_backends` to assert
  ptx+spirv exist (not metal/hlsl).
- DO NOT touch eval/mod.rs (SpirV stays, so its tests stay).

**Aggregate:** ~341 LOC + 14 tests removed.
**Effort:** ~30-45min.
**Risk:** Low-Medium. Smaller churn than Action A; SpirV's 33 tests are
preserved.

## §4. Recommendation

**Action C (mixed) for v35.7.1.**

Rationale:

1. **Aligned with Compass §5.1 intent.** Compass said "simplify" — Action
   C does precisely that: drop the smallest least-valuable paths
   (metal/hlsl), preserve the substantial work in SpirV.
2. **Honest about uncertainty.** SpirV has 33 tests reflecting real
   research investment. Deleting it blind is anti-honest (it would
   throw away verified work). Freeze-in-place lets us reconsider later.
3. **Manageable scope.** ~30-45min, well within "first step" sizing.
4. **Smallest blast radius.** No interpreter/eval test churn (SpirV
   stays). No `.fj`-level impact. Only fairly isolated cuts.
5. **Phase17 byte-equality risk: NONE.** No stdlib `.fj` touched.

If Fajar prefers a bigger trim, Action A is the alternative. Action B
(pure freeze) preserves everything but doesn't reduce LOC.

## §5. Stage 2 byte-equality risk

None of the three actions touch stdlib `.fj` files. Phase17 unaffected.

## §6. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §7)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — Action C ships with updated v14_n6_6 test as guard)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all LOC + test counts verified live)
[ ] Effort variance tagged in commit message               (Rule 5 — at commit time)
[ ] Decisions are committed files                          (Rule 6 — decision doc still TBD after Fajar picks)
[x] Public-artifact drift swept                            (Rule 7 — done in R4 earlier this session)
[x] Multi-repo state checked                               (Rule 8 — R6 done earlier this session)
```

## §7. Verification commands

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Pre-action sanity: full gpu_codegen test suite green
cargo test --lib gpu_codegen
# expect: all PASS

# After Action C ship:
ls src/gpu_codegen/  # expect: mod.rs, ptx.rs, spirv.rs, fusion.rs, gpu_memory.rs (5 files instead of 7)
cargo test --lib gpu_codegen  # expect: all remaining tests PASS (-14 metal/hlsl/mod.rs tests)
cargo test --test nova_v2_tests v14_n6_6  # expect: updated assertion PASS
cargo clippy --lib -- -D warnings  # expect: 0 warnings
cargo fmt -- --check  # expect: clean

# Phase17 byte-equality (no stdlib change but worth verifying)
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
```

## §8. Open decisions for the user

| Decision | Default | Alternative |
|---|---|---|
| Action A / B / C | **C (mixed)** | A (full delete, larger trim) or B (freeze-only, zero LOC change) |
| README CUDA badge | Keep (PTX path verified) | Add a footnote if any GPU paths are frozen |
| Cargo feature flag | Not in scope for Action C | Could gate spirv behind `gpu-extended` if frozen |

## §9. Source artifacts

- This file: `docs/GPU_CODEGEN_SIMPLIFICATION_B0_FINDINGS.md`
- Predecessor: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md` §2.4
- Source compass: `docs/1/STRATEGIC_COMPASS.md` §5.1 (GPU codegen row)
- Decision file (to write when Fajar picks): `docs/decisions/2026-05-12-gpu-codegen-simplification.md`

---

*B0 written 2026-05-12 EOS-31 session. ~30min actual. Inventory + 3
action paths surfaced. Compass-aligned recommendation: Action C (mixed
delete + freeze).*
