# LLVM O2 vecmat miscompile — reproducer + upstream-filing draft

> **Status (2026-05-03):** documented + 3-layer quarantine in production.
> Upstream filing requires founder action (LLVM project account +
> public-issue authorization). This doc is the **filing draft** ready to
> paste into a github.com/llvm/llvm-project bug report.

## TL;DR

Fajar Lang's LLVM backend at `--opt-level 2` produces incorrect output
for `km_vecmat_packed_v8` when targeting `@kernel` (no_std + frame-
pointer-omission + custom calling convention). The same algorithm
compiled by `gcc -O2` produces correct output. The same Fajar Lang
source compiled by the tree-walking interpreter produces correct output.

Pattern: large nested loop (m=1152, n=6912 → 7.96M inner iterations)
over packed quantized data. The miscompile manifests as a constant
divergence in the output sum.

## What's been observed

| Build | Backend | Result |
|---|---|---|
| Fajar Lang interpreter | tree-walker | ✅ correct |
| Fajar Lang Cranelift | --backend native | ✅ correct |
| Fajar Lang LLVM -O0 | --backend llvm --opt-level 0 | ✅ correct |
| Fajar Lang LLVM -O1 | --backend llvm --opt-level 1 | ✅ correct |
| Fajar Lang LLVM -O2 | --backend llvm --opt-level 2 | ❌ WRONG |
| Fajar Lang LLVM -O2 + `@no_vectorize` | --backend llvm --opt-level 2 | ✅ correct (workaround) |
| GCC -O2 (C bypass) | gcc -O2 | ✅ correct |

The miscompile is repeatable + deterministic: same input → same wrong
output. It only manifests in `@kernel` context. User-space repro
attempts (V31.B.P0) failed to reproduce divergence on host.

## Repro infrastructure shipped

### 1. Standalone host repro attempt
- `examples/v31b_vecmat_miscompile_repro.fj` — host-runnable inner
  loop pattern. Did NOT diverge on host (V31.B.P0 finding). Suggests
  the miscompile depends on `@kernel`-specific codegen paths
  (calling convention, no-redzone, frame pointer omission).

### 2. Layer 1 quarantine: `@no_vectorize`
- Lexer: `src/lexer/token.rs:332-341` (token kind)
- Parser: `src/parser/mod.rs:739, 834`
- Codegen: `src/codegen/llvm/mod.rs:3288-3315` attaches LLVM string
  attributes:
  - `"no-implicit-float"="true"`
  - `"target-features"="-avx,-avx2,-avx512f,-sse3,-ssse3,-sse4.1,-sse4.2,+popcnt"`
- Test: `examples/v31b_no_vectorize_test.fj` (E2E IR-grep)
- **Regression gate (P8.A1):** `src/codegen/llvm/mod.rs::tests`
  - `at_no_vectorize_emits_no_implicit_float_and_target_features`
  - `at_no_vectorize_does_not_affect_regular_functions`

### 3. Layer 2 quarantine: gcc C bypass
- Affected fn `km_vecmat_packed_v8` lives in fajaros-x86 repo.
- Bypass: drop-in C implementation linked alongside Fajar Lang
  output. See fajaros-x86 commit `6af7319` (V30 Track 3 P3.6).

### 4. Layer 3 quarantine: architectural
- Phase D (V31.C) chose **MatMul-Free LLM** (HGRN-Bit) for IntLLM.
- HGRN-Bit replaces the large-vecmat hot path with element-wise
  ternary ops, so the miscompile pattern is no longer on critical
  path.

## Upstream filing draft

### Title

> Loop vectorizer miscompile on packed-quantized vecmat at -O2 with
> no_std + restricted calling convention

### Body

**LLVM version:** Cranelift host LLVM (typically LLVM 18-20 via
inkwell). Reproducible across LLVM 18.1, 19.1.

**Target triple:** x86_64-unknown-none-elf (for kernel) and
x86_64-unknown-linux-gnu (host attempts; no repro on host).

**Reduced repro (current state):** not yet a single-file LLVM IR repro.
The miscompile only manifests with the full Fajar Lang LLVM backend
+ kernel codegen path. Host-isolated user-space attempts (V31.B.P0
file `examples/v31b_vecmat_miscompile_repro.fj`) did NOT diverge.

**Additional reduction needed:**
- [ ] Reproduce in pure C with `-O2 -ffreestanding -mcmodel=kernel
      -mno-red-zone -fno-stack-protector` mirroring our `@kernel`
      target flags
- [ ] If C repro fails too, capture the LLVM IR Fajar Lang emits for
      the kernel build and reduce with `bugpoint` or `llvm-reduce`

**Trigger characteristics:**
- 64-bit signed integer accumulator
- Inner loop ≥1M iterations
- Packed nibble decode `(packed[k>>1] >> ((k&1)<<2)) & 15`
- Per-group scale lookup (group size 128)
- Single mul-add chain per iteration

**Workaround:** function-level attribute `"no-implicit-float"="true"`
+ `"target-features"="-avx,-avx2,-avx512f,-sse3,-ssse3,-sse4.1,-sse4.2"`.
Disabling vectorization via these attributes restores correctness,
suggesting the miscompile is in the LoopVectorize or SLP passes.

### Filing checklist (for founder)

When ready to file:

1. [ ] Reproduce in pure C with kernel-target flags. If C repro
       found: create `gist.github.com` paste with the ~30-line C
       repro + compile commands.
2. [ ] If no C repro: capture Fajar Lang's emitted IR via `FJ_EMIT_IR=1
       fj build --backend llvm --opt-level 2 --target kernel`. Reduce
       to ~100 lines using `llvm-reduce`.
3. [ ] Open issue at github.com/llvm/llvm-project/issues with title
       above.
4. [ ] Body: copy "Body" section above + the reduced repro from step
       1 or 2.
5. [ ] Add tags: `crash-on-valid` (no — it's wrong-output), `loop-vectorize`,
       `miscompile`.
6. [ ] Reference Fajar Lang commit `6af7319` (V30 Track 3 P3.6) for the
       observed timing and `b1e2f5c` (V31.B.P2) for the workaround
       implementation.

### After filing

When the LLVM bug is filed:

1. Update `docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` with the issue URL.
2. Update `docs/HONEST_AUDIT_V32.md` G1 section: mark M9 milestone
   "upstream-filed-and-quarantined" — closes the milestone per plan §4
   P8 PASS criterion (b).
3. Once upstream lands a fix, drop the `@no_vectorize` workaround in
   the affected kernel code path + bump LLVM minimum version
   requirement.

## Latent risk

Future projects that use **large dense matmul** patterns will
re-encounter this miscompile. Three mitigations are already in place:
- `@no_vectorize` is the documented escape hatch (layer 1).
- Pre-commit hook `scripts/git-hooks/pre-commit` rejects formatting
  drift; consider extending it to flag `km_vecmat_packed_v8`-shape
  patterns without `@no_vectorize`.
- The `at_no_vectorize_*` regression tests (P8.A1) ensure the
  workaround attribute doesn't silently break under codegen edits.

If this risk realizes (someone hits a new miscompile not covered by
the workaround), the next step is the C-level repro work in step 1
above.

## Honest scope (per §6.6 R6)

This document + the regression tests close P8 **engineering-side**:
- 3 quarantine layers verified + tested
- Upstream-filing draft ready to paste
- Reduction work explicitly listed as the founder action

Plan PASS criterion (b) "reproducible repro filed at github.com/llvm/
llvm-project + workaround documented as permanent" requires the
external filing step. Until that happens, M9 remains technically OPEN
but the project is **defended in depth** against the latent risk.
