# V29.P1 Decision — Keep @noinline vs Revert

**Status:** DECISION — committed 2026-04-16 as mechanical gate per
Plan Hygiene Rule 6 (decisions must be committed files, not prose).
**Gated phase:** V29.P1.P4 (FajarOS retest + honesty retrospective)
**Signed by:** Muhamad Fajar Putranto
**Signed at:** 2026-04-16

## Decision

**KEEP `@noinline` in FajarOS kernel hot paths.** Proceed with retest
of V28.5 multilingual output using a kernel binary that actually has
the NoInline attribute applied (not the pre-V29.P1 build that silently
dropped the annotations).

## Alternatives Considered

### Alternative A: KEEP (chosen)
- Keep `@noinline` in kernel/compute/kmatrix.fj + model_loader.fj
- Rebuild FajarOS with V29.P1-enabled `fj` compiler
- Retest V28.5 multilingual; document retest outcome in
  fajaros-x86/docs/V28_5_RETEST.md
- Update V28_5_CLOSED.md with retroactive callout preserving audit
  trail (original claim + honest correction)

### Alternative B: REVERT
- Remove @noinline from kernel hot paths
- Document V28.5 multilingual stability as "uninstrumented / at-risk"
- Skip retest; accept EXC:13-after-50-tokens regression permanently

### Alternative C: PARTIAL
- Keep @noinline only on 1-2 most critical hot paths
- Revert from others
- Retest selectively

## Rationale (Why A Over B or C)

1. **Root cause is now fixed.** V29.P1 Phases P1--P3 closed the lexer
   gap, added the codegen flag wiring, and installed the Makefile
   silent-build-failure gate plus pre-commit check 5/5. The compiler
   now correctly honors `@noinline`. Reverting the kernel-side
   annotations would discard the known stability fix just because we
   had a 2-hour window where it wasn't actually applied.

2. **V28.5 was the real V8 coherence infrastructure milestone; the
   multilingual output itself is valid regardless.** The 7-writing-system
   output documented in commit `fajaros-x86@5670b4e` is real BPE-tokenized
   multilingual text — it came from the V28.5 infrastructure
   (memory-map detector, 16-byte header fix, UTF-8 raw streaming,
   group-wise 4-bit v8 format, robust rmsnorm). Those fixes WERE
   compiled into the kernel that produced the output. Only the
   `@noinline` stabilization was not actually applied — and its
   purpose was to prevent EXC:13 crashes beyond ~50 tokens, not to
   enable the multilingual output in the first place.

3. **EXC:13 behavior needs fresh data.** The original commit claimed
   "stable ~50 multilingual tokens per run" with `@noinline`. Since
   the annotation wasn't compiled in, the observed stability was
   either coincidence (LLVM O2 produced a stable layout anyway) or
   the crash simply took longer than 50 tokens to manifest on that
   run. A retest with `@noinline` actually active will produce
   definitive data:
   - If retest still crashes after ~50 tokens → `@noinline` alone
     is insufficient; EXC:13 root cause investigation escalates
     (candidate causes in V28_2_CLOSED_PARTIAL.md)
   - If retest runs noticeably longer or more stably → `@noinline`
     was working and the original claim was correct in spirit, just
     unverified. Keep the annotations.
   - If retest regresses (shorter runs with `@noinline`) →
     unlikely but possible; would trigger Alternative C (selective
     removal).

4. **Honesty retrospective is independent of keep/revert.** Docs
   (V28_5_CLOSED.md, CHANGELOG, MEMORY.md) get retroactive callouts
   regardless of the decision here, since the audit trail must
   reflect that the original V28.5 claim was not backed by a binary
   containing `@noinline`. Decision A adds a "re-verified on
   <date>" line; decision B adds a "permanent stability
   regression acknowledged" line.

## Retest Plan (P4.2 + P4.3 Specifics)

| # | Action | Success criterion |
|---|--------|--------------------|
| 1 | `make clean && make build-llvm && make iso-llvm` | `[OK] LLVM kernel built` + ELF exists + size ~1.6 MB |
| 2 | Verify @noinline functions in ELF | `objdump -t build/fajaros-llvm.elf \| grep -cE "km_vecmat_packed_v8\|mdl_stream_embed_lookup_raw_v8\|mdl_ram_lmhead_argmax_v8_tied"` → 3 |
| 3 | Boot QEMU with `make test-serial` | Log reaches `nova>` prompt |
| 4 | Run `ask` shell command (spawns Gemma 3 inference) | Capture serial output for 30s — count tokens generated |
| 5 | Record EXC:13 state | Did the crash occur? At what token count? |
| 6 | Interpret | Keep + add `[v]` line in V28_5_CLOSED.md OR escalate to V29.P2 research track |

## Acceptance

Test run result recorded in `fajaros-x86/docs/V28_5_RETEST.md` before
P4.4--P4.7 documentation updates. If retest hangs indefinitely or
produces zero tokens, V29.P1 P4 escalates to "compile path broken
beyond annotation" and blocks P5 handoff until root-caused.

## Sign-Off

Decision committed by **Muhamad Fajar Putranto** on **2026-04-16**,
satisfying:
- V29.P1 Phase P4 entry gate (per
  `fajar-lang/docs/V29_P1_COMPILER_ENHANCEMENT_PLAN.md` §8)
- Plan Hygiene Rule 6 (decisions must be committed files)
- Honesty rule (Rule 7 + memory `feedback_honesty_upfront`)
