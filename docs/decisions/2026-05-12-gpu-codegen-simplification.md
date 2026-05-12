# Decision — GPU Codegen Simplification (v35.7.1 Action C)

> **Date:** 2026-05-12
> **Owner:** Fajar (user decision after B0)
> **Status:** ✅ Decided — Action C (mixed: delete metal+hlsl, freeze spirv)
> **B0 source:** `docs/GPU_CODEGEN_SIMPLIFICATION_B0_FINDINGS.md`
> **Compass source:** `docs/1/STRATEGIC_COMPASS.md` §5.1 (GPU codegen row,
> verdict "Sederhanakan")
> **Plan Hygiene §6.8 R6:** This file is the committed decision.

## Decision

**Adopt Action C (mixed simplification) for v35.7.1.**

### Scope of Action C

1. **DELETE** `src/gpu_codegen/metal.rs` (171 LOC, 6 tests).
2. **DELETE** `src/gpu_codegen/hlsl.rs` (162 LOC, 6 tests).
3. **EDIT** `src/gpu_codegen/mod.rs`:
   - Drop `pub mod metal;` and `pub mod hlsl;` declarations.
   - Remove `GpuKernel::to_metal()` (32 LOC) and `GpuKernel::to_hlsl()` (33 LOC) methods.
   - Remove `gpu_expr_to_metal()` (19 LOC) and `gpu_expr_to_hlsl()` (19 LOC) free helpers.
   - Remove tests `v14_gpu_ir_to_metal` (16 LOC) and `v14_gpu_ir_to_hlsl` (16 LOC).
   - Update module-level doc comment: "PTX/SPIR-V backend".
4. **EDIT** `src/main.rs` — `fj build --target` CLI dispatch:
   - Match guard L562: `"spirv" | "ptx" | "metal" | "hlsl"` → `"spirv" | "ptx"`.
   - Extension mapping L566-567: drop `metal`/`hlsl` arms.
   - AST-driven codegen arms L612-613: drop `metal`/`hlsl` arms.
   - Fallback hardcoded-kernel arms L637-646: drop `metal`/`hlsl` arms.
   - Comment update at L555: "SPIR-V/PTX" (was "SPIR-V/PTX/Metal/HLSL").
5. **FREEZE** `src/gpu_codegen/spirv.rs` — add module-level deprecation header
   referencing this decision file. Preserve all 33 tests and 1,261 LOC.
6. **UPDATE** `tests/nova_v2_tests.rs::v14_n6_6_gpu_codegen_all_backends` — assert
   metal/hlsl files DO NOT exist (post-Action-C reality).
7. **UPDATE** `tests/validation_tests.rs::v14_w15_4_gpu_annotation_parses` — replace
   `to_metal()` structural check with `to_ptx()` equivalent (`.entry kernel`).

### Aggregate impact

| Metric | Before | After | Delta |
|---|---|---|---|
| src/gpu_codegen LOC | 4,773 | ~3,260 | **-1,513** (-31.7%) |
| src/gpu_codegen files | 7 | 5 | **-2** (metal.rs, hlsl.rs) |
| src/main.rs LOC | (unchanged baseline) | -~30 | -30 |
| Lib tests | 7,633 | **7,619** | **-14** |
| gpu_codegen lib tests | 114 | 100 | **-14** |

Total LOC reduction: **~1,543 LOC**. Test count reduction: **14 tests**.

### Why Action C over A or B

| Option | Trim | Risk | Chosen because |
|---|---|---|---|
| Action A (full delete of spirv+metal+hlsl) | ~1,624 LOC + ~57 tests | Medium (requires ~14 SpirV test fn deletes from interpreter/eval/mod.rs) | Throws away 33 tests of SpirV research investment. Anti-honest. |
| Action B (freeze in place, deprecation comments only) | 0 LOC change | Lowest | Doesn't reduce surface area. Compass said "simplify" not "freeze". |
| **Action C (mixed)** | **~1,543 LOC + 14 tests** | **Low-Medium** | **Compass-aligned** ("simplify" achieved), **honest** (SpirV preserved), **manageable** scope (~30-45min actual; came in at ~50min due to B0 scope creep around CLI dispatch). |

### What Action C does NOT do (intentional non-scope)

- Does NOT remove `src/gpu_codegen/spirv.rs` (preserved with deprecation header).
- Does NOT touch interpreter SpirV tests in `src/interpreter/eval/mod.rs:6820+`
  (SpirV stays, so its tests stay).
- Does NOT change `src/gpu_codegen/fusion.rs` or `gpu_memory.rs` (backend-agnostic;
  keep).
- Does NOT change `src/gpu_codegen/ptx.rs` (RTX 4090 verified; actively developed).
- Does NOT remove the `gpu` Cargo feature (`gpu = ["wgpu"]`). PTX path remains.
- Does NOT update README CUDA RTX 4090 badge (PTX path unchanged).
- Does NOT update CHANGELOG (engineering correctness/cleanup, not user-visible feature).

### Re-entry conditions for SpirV / Metal / HLSL revival

#### SpirV (frozen, can be revived)
- A verified user/customer surfaces with a Vulkan compute requirement.
- The embedded niche gains a cross-platform Vulkan-based GPU target.
- Until then, the 33 tests stay green as documentation of working code.

#### Metal / HLSL (deleted, recoverable via git history)
- A verified Apple Metal or DirectX HLSL use-case surfaces.
- The commit shipping Action C (`<TBD>`) can be reverted or its parent
  branched if needed.
- Until then, the simplification stands.

### Verification commands (executed @ ship)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Build is clean under default features
cargo build
# expect: success (no warnings, no errors)

# gpu_codegen lib tests still pass (minus -14 from metal/hlsl)
cargo test --lib gpu_codegen
# expect: 100 passed (was 114, -14 from metal/hlsl/mod.rs deletes)

# Updated tests pass
cargo test --test nova_v2_tests v14_n6_6  # expect: 1 PASS
cargo test --test validation_tests v14_w15_4  # expect: 1 PASS

# Full lib suite unaffected elsewhere
cargo test --lib
# expect: 7,619 passed (was 7,633, -14 as above)

# Quality gates
cargo clippy --lib -- -D warnings  # 0 warnings
cargo fmt -- --check  # clean

# Stage 2 byte-equality (no stdlib change but worth confirming)
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed
```

### Stage 2 byte-equality risk: NONE

Action C touches only Rust files (no stdlib `.fj`). Phase17 unaffected.

### Phase17 byte-equality verification post-ship

Recorded at commit time.

### B0 scope creep (transparency)

The Compass §5 high-level B0 estimated GPU codegen at 4,773 LOC + 114
tests. The GPU-codegen-specific B0 refined this to "~1,624 LOC + ~57 tests"
for Action C scope. The actual scope discovered during implementation
was larger than the B0:

- B0 missed: `src/main.rs` CLI dispatch on `fj build --target metal|hlsl`
  (~30 LOC across 4 sites including the production CLI flow + fallback arms).
- B0 missed: `tests/validation_tests.rs:1227` call to `to_metal()` (1 LOC
  + comment).
- B0 surfaced correctly: file deletes + mod.rs edits + nova_v2_tests
  update + spirv freeze.

Net implementation came in at ~50min vs the estimated 30-45min (+25% over).

The B0 was honest enough to flag the SpirV-in-eval/mod.rs entanglement
but missed the `fj build --target` CLI scope. Future GPU/codegen audits
should grep `src/main.rs` for backend-string dispatches as a standard
B0 step.

## References

- B0: `docs/GPU_CODEGEN_SIMPLIFICATION_B0_FINDINGS.md`
- Compass §5 audit: `docs/COMPASS_5_FREEZE_CANDIDATES_B0_FINDINGS.md`
- Source compass: `docs/1/STRATEGIC_COMPASS.md` §5.1
- Predecessor decisions (same session):
  - `docs/decisions/2026-05-12-cranelift-builtin-list-shape.md` (B-δ)
  - `docs/decisions/2026-05-12-parser-annotation-grammar-shape.md` (D1.A)
