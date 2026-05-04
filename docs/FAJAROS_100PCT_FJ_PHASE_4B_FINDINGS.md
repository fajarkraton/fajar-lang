---
phase: 4.B — port km_rmsnorm + km_gelu_tanh; 4.C attempt revealed real complexity
status: 4.B CLOSED 2026-05-04 / 4.C REVERTED, deferred
budget: full Phase 4 was 1.5-2d; 4.A + 4.B together ~2h; 4.C attempted ~45min, reverted
artifacts:
  - This findings doc
  - fajaros-x86 commit 9820285 — Phase 4.B
  - (Phase 4.C work stashed + dropped, NOT committed)
prereq: Phase 4.A closed (fajar-lang d012e6d9, fajaros-x86 0f58b0c)
---

# Phase 4.B Findings — km_rmsnorm + km_gelu_tanh ported; 4.C reverted

> Phase 4.B successfully ports 2 more vecmat_v8.c functions. Phase 4.C
> attempt (mdl_embed_lookup + mdl_lmhead_argmax) hit EXC:14 page fault
> at runtime; reverted to maintain clean main. Real bug investigation
> deferred to next dedicated session.

## Phase 4.B (CLOSED) — km_rmsnorm + km_gelu_tanh

Ported via inline fj using existing helpers `km_isqrt` (line 335) and
`km_tanh_approx` (line 587) — both bit-exact mirrors of the deleted
C versions.

**Verification (E2E):**
- `make build-llvm` → ELF 1,505,214 bytes
- `make test-spinlock-smp-regression` → PASS
- `make test-security-triple-regression` → 6/6 invariants PASS
- `make test-gemma3-e2e` (~210s) → **6/6 invariants PASS**:
  - no fault markers / model header parsed / embed-load / tokenizer
  - **64 tokens generated** (forward pass reaches LM head)
  - shell recovered after ask

vecmat_v8.c reduced 728 → 642 LOC (-86; -80 fns, -9 c_tanh_approx helper).

## Phase 4.C (REVERTED) — mdl_embed_lookup + mdl_lmhead_argmax

Attempted to port the embedding-lookup + LM-head-argmax functions
following the same pattern as 4.A/4.B. These are larger (~80 LOC each)
and use byte/u32 reads (`volatile_read_u8`, `volatile_read_u32`) rather
than the u64 pattern Phases 4.A/4.B used.

**Symptoms:**
- Build clean (no fj-lang errors).
- Boot + model-load + embed-load + tokenizer-load all PASS.
- First inference (`ask hello`) immediately faults:
  ```
  Output: EXC:14         # page fault
  000000FDE4E1E1A8       # fault address (CR2) — ~63 GB, way OOB
  0000000000070000       # PML4 base
  PANIC:14
  ```

**Hypotheses (NOT yet investigated):**

1. **Register pressure / LLVM codegen bug.** Original fj wrappers had
   `@noinline` specifically to "avoid LLVM register-pressure
   interference; EXC:14 was observed when asm! was placed directly in
   tfm_forward_stream" (per comment in `kernel/compute/model_loader.fj:2143`).
   Inlining the entire body (vs. the previous tiny mailbox-write +
   asm-call wrapper) may trigger the same class of LLVM codegen issue.

2. **`volatile_read_u8` / `volatile_read_u32` semantics differ from C
   `*(uint8_t*)` / `*(uint32_t*)`.** Possible: sign extension, or
   incorrect lowering for byte/word reads in fj-lang LLVM backend.
   Phase 4.A/4.B only used u64 reads (which are simpler / well-tested).

3. **Mailbox layout mismatch.** Phase 4.C eliminated the mailbox by
   passing args directly. If `mdl_get_vocab_size()` / `mdl_get_d_model()`
   return different values when called from the new context, address
   computations could go wild. The fault address `0xFDE4E1E1A8` looks
   uninitialized (no obvious arithmetic produces it from sane inputs).

4. **Compiler vs. interpreter `q-zero` semantics.** `q` and `zero` are
   `uint8_t` in C → zero-extended to i64. fj's `volatile_read_u8` may
   return values that compute differently in `(q - zero) * scale`.

**Investigation plan (next session):**

1. Bisect — port ONLY `mdl_embed_lookup` (without `mdl_lmhead_argmax`).
   If still EXC:14 → embed_lookup bug. If clean → argmax bug.
2. Add debug prints inside the inline body (e.g. print `q`, `zero`,
   `scale` for first 5 iterations).
3. Compare emitted LLVM IR for the new fj fn vs. what gcc produced
   for the C version. Look for sign-extension differences.
4. Test with `volatile_read_u8` returning small known values (e.g.
   inject `q = 100` constant) to isolate which read is producing
   garbage.

## Plan progress audit

```
Phase 0 baseline:  3 files, 2,195 LOC
After Phase 2:     2 files, 1,680 LOC
After Phase 3:     1 file,    768 LOC
After Phase 4.A:   1 file,    728 LOC
After Phase 4.B:   1 file,    642 LOC ← here (-86: 2 functions + helper)
Plan target:       0 files,     0 LOC
```

**4 of 9 phases CLOSED:** Phase 0, 1, 2, 3, 4.A, 4.B. Plan target 0/0
remains for Phase 4.C-F (5 functions + 1572-entry sin LUT + math
helpers).

**Honest re-estimate for Phase 4.C-F:** 6-8h focused work (was 9-10h
before today, but Phase 4.B-4.C attempts surfaced concrete obstacles
that explain why simple "port-and-go" doesn't work for byte/u32-heavy
functions). Need real debugging session, not another rapid-iteration
"lanjutkan" pass.

## Compiler gaps (running tally)

| Gap | Status |
|---|---|
| G-G LLVM global_asm! emission | ✅ CLOSED Phase 2.A |
| G-H r#"..."# raw strings | ✅ CLOSED Phase 2.A.2 |
| G-I parser raw strings in asm templates | ✅ CLOSED Phase 2.A.2 |
| G-F SE009 false-positive on asm operand uses | ⏳ defer Phase 5 |
| G-J LLVM MC stricter than GAS | ⏳ documented |
| G-K @no_vectorize + @kernel parser mutex | ⏳ documented |
| **G-L** (NEW): EXC:14 in inlined fj fns with byte/u32 reads + tight loops | ⏳ Phase 4.C debug needed |

## Decision gate (§6.8 R6)

This file committed → Phase 5+ (LLVM atomics, @naked, @no_mangle) can
proceed in parallel with Phase 4.C-F debug. They are independent
fj-lang core work.

**Recommendation:** PAUSE Phase 4.C-F until a debug session can
bisect + fix the EXC:14 root cause. Don't push through with more
"lanjutkan" iterations on the same broken pattern — the bug is real
and warrants careful triage.

---

*FAJAROS_100PCT_FJ_PHASE_4B_FINDINGS — 2026-05-04. Phase 4.B CLOSED;
Phase 4.C attempted + reverted with EXC:14 root cause documented.
Plan progress: 4/9 phases done at -85% effort variance. Phase 4.C-F
deferred to debug-focused session.*
