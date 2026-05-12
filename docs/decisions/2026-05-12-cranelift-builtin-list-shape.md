# Decision — Cranelift builtin-list shape (v35.6.x Option A)

> **Date:** 2026-05-12
> **Owner:** Fajar (user decision after B0)
> **Status:** ✅ Decided — B-δ this session, B-γ deferred to v36.x
> **B0 source:** `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md`
> **Plan Hygiene §6.8 R6:** This file is the committed decision; downstream
> work is gated on this shape.

## Decision

**Adopt B-δ (plug-the-hole minimal) for v35.6.x.** Defer the full
refactor (B-γ shared `context_safety` module) to v36.x as a separate
phase with its own B0.

### Scope of B-δ

1. Add `analyze(&program)` pre-pass to `src/main.rs` `cmd_run_native`
   (after parse, before Cranelift dispatch).
2. Add `analyze(&program)` pre-pass to `src/main.rs` `cmd_run_llvm`
   (after parse, before LLVM dispatch).
3. Add regression test `tests/cli_native_llvm_analyzer_regression.rs`
   (mechanical content-guard on the two fn bodies + companion analyzer
   invariant checks).
4. Leave Cranelift's `check_call_name` / H4 hook untouched. It now
   functions as belt-and-suspenders (analyzer runs first, so the H4
   hook's drifted list is no longer load-bearing).
5. Leave LLVM codegen untouched (no context check needed once analyzer
   pre-pass is in place).
6. The 3 H4-hook-direct tests at `src/codegen/cranelift/tests.rs:15724-15791`
   remain valid: they still verify Cranelift's defense-in-depth even
   when bypassing the analyzer. Not deleted.

### Why B-δ over the alternatives

| Option | Effort | Why rejected/deferred |
|---|---|---|
| **B-α** Cranelift mirrors analyzer | ~2-3h | Adds duplication. Maintenance burden on every new builtin. Defense-in-depth value is real but smaller than the cost. |
| **B-β** Drop Cranelift list | ~1-2h | Requires analyzer pre-pass first (which is B-δ). Once B-δ ships, B-β becomes a v36.x cleanup option; not urgent. |
| **B-γ** Shared module across analyzer + Cranelift + LLVM | ~3-5h | The right end-state but a larger refactor crossing analyzer/codegen boundaries. Needs its own B0 and dedicated phase. Deferred. |
| **B-δ** Plug-the-hole | ~30-45min | **Chosen.** Smallest scope, largest immediate safety win (closes LLVM hole that previously had zero context check). Zero risk to Stage 2 byte-equality (no fj-source touched). Belt-and-suspenders preserved without removing anything. |

### What B-δ does NOT do (intentional non-scope)

- Does NOT remove Cranelift's drifted list (deferred to B-γ or B-β v36.x).
- Does NOT extract a shared `context_safety` module (B-γ v36.x).
- Does NOT add a defense-in-depth hook to LLVM (analyzer pre-pass is
  sufficient; LLVM-specific hook can be added if a future
  analyzer-bypass path emerges).
- Does NOT rewrite the 3 Cranelift H4 tests at `tests.rs:15724-15791`.

### Re-entry conditions for B-γ (v36.x)

Open the full shared-module refactor if any of these occur:

1. A new analyzer-canonical builtin is added but forgotten in Cranelift's
   `check_call_name` (drift caught in code review or by a new test).
2. A user reports a `fj run --native`/`--llvm` accepting code that
   `fj run` (interpreter) rejects.
3. A third codegen backend is added (WASM, RISC-V native, etc.) — at
   which point the duplication cost crosses the refactor break-even.
4. CI adds a "no-bypass" check that requires analyzer-canonical lists to
   be consulted by every codegen.

Until then, B-δ's prevention layer (the regression test that locks in
the analyzer pre-pass for both `cmd_run_native` and `cmd_run_llvm`) is
the operative guard.

### Verification commands (post-ship)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Mechanical guard tests
cargo test --test cli_native_llvm_analyzer_regression
# expect: 4 passed (cmd_run_native_calls_analyze, cmd_run_llvm_calls_analyze,
#         analyzer_rejects_kernel_with_tensor_op, analyzer_rejects_device_with_raw_pointer)

# Cranelift H4 hook tests still green (single-filter form; cargo test takes
# at most one positional TESTNAME, so use the common `context_` prefix)
cargo test --lib --features native context_
# expect: includes 5 cranelift H4 tests (context_kernel_rejects_tensor,
#         context_kernel_rejects_read_file, context_device_rejects_raw_pointer,
#         context_safe_allows_normal_code, context_unsafe_allows_everything)
#         plus other context_* tests across modules; verified 26 passed @ HEAD.

# Full context_safety_tests integration suite unaffected
cargo test --test context_safety_tests
# expect: 149+ tests pass

# Stage 2 byte-equality preserved (B-δ touches Rust only)
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed (phase17 byte-equality is the canonical Stage-2 check)
```

### Stage 2 byte-equality risk: NONE

B-δ touches only `src/main.rs` (Rust) and adds a new tests file. No
stdlib `.fj` files modified, no codegen logic changed. Phase17 byte-
equality is unchanged from HEAD `bd11e8e3`.

## References

- B0 findings: `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md` (commit `2bf6f7c6`)
- Resume protocol: `memory/project_resume_lanjut_protocol.md` §2.A
- Predecessor §4.4 closure: `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §3 micro-gap #2
- D-α companion decision: `docs/decisions/2026-05-10-default-safe-bridge.md`
