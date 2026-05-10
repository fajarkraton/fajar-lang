# PHASE17 PERF B0 — D-FULL has no measurable phase17 regression

**Date:** 2026-05-10 (EOS-27, 3rd lanjut)
**Status:** B0 CLOSED — no regression to optimize
**Trigger:** Resume-protocol flag #4 + CHANGELOG v35.5.0 claim "2× slowdown"
**Author:** Claude session-end audit

---

## TL;DR

**The "2× phase17 perf regression" attributed to D-FULL does not exist.**
It was a measurement-protocol artifact: the CHANGELOG/resume-protocol
"54s baseline" was measured with default parallel `cargo test`; the
"106s post-ship" was measured with `-- --test-threads=1` (serial). When
the same protocol is applied to both tags, the runtimes are within noise.

| Tag | Parallel (default) | Serial (`--test-threads=1`) |
|-----|--------------------|------------------------------|
| **v35.4.1** | ~50s | ~103s |
| **v35.5.0** | ~50s | ~101s |

D-FULL ownership semantics + COW `_FjArr` + Rc/Arc-share `.clone()` add
**~0s** to phase17 self-compile when measured consistently.

---

## How the wrong claim got into CHANGELOG / resume protocol

EOS-25 memory recorded `phase17 4/4 @ 53.97s` from a v35.4.1 ship-time
gate run. That run almost certainly used default `cargo test` (parallel).

EOS-26 D-FULL gating was performed via `cargo test ... -- --test-threads=1`
(per the project's pre-push hook + protocol convention). It returned ~106s.
The numbers were filed as "54s baseline → 106s post-ship → 2× regression"
without checking that the protocol matched.

The phase17 test file has **4 sub-tests** that fan out to ~14 cores
during parallel execution; the largest two (~46s each, serial) overlap
under parallelism, dropping wall-clock from ~103s to ~50s. This 2×
parallel-speedup explains the entire "regression."

---

## Reproduction

### Per-test serial timings

Both measured at HEAD (v35.5.0) and at tag `v35.4.1` (in
`/tmp/fajar-v35.4.1` worktree clone). Serial = `-- --test-threads=1`.

| Sub-test | v35.4.1 | v35.5.0 | Δ |
|----------|---------|---------|---|
| `phase17_codegen_fj_self_compile_to_object` | 2.16s | 2.14s | -0.02s |
| `phase17_parser_ast_fj_self_compile_to_object` | 9.66s | 9.45s | -0.21s |
| `phase17_all_three_combined_self_compile_to_object` | 45.11s | 45.90s | +0.79s |
| `phase17_stage2_native_triple_test` | 46.70s | 45.99s | -0.71s |
| **Total (sum)** | **103.63s** | **103.48s** | **-0.15s** |

### Full-suite serial timings (3 runs at v35.5.0; 2 runs at v35.4.1)

- v35.5.0 serial: 100.87s, 101.86s, 100.55s — mean **101.1s**
- v35.4.1 serial: 103.38s, 102.53s — mean **102.96s**
- Δ mean serial: **-1.85s** (v35.5.0 is fractionally **faster**, within noise)

### Full-suite parallel timings (2 runs each)

- v35.5.0 parallel: 48.58s, 51.11s — mean **49.85s**
- v35.4.1 parallel: 49.56s, 50.08s — mean **49.82s**
- Δ mean parallel: **+0.03s** (within sub-100ms noise)

### Conclusion

**Zero measurable performance impact** from D-FULL's COW + universal
`.clone()` + cascade `.clone()` insertions. The COW refcount-bump and
Rc/Arc-share design successfully kept the runtime cost off the hot path.

---

## What this means for v35.5.0

### Public-facing claims that need correction

1. **CHANGELOG.md v35.5.0** §"COW runtime (Phase 5)" currently says:
   > "phase17 self-compile completes in ~106s (baseline ~54s, 2× slowdown for full affine semantics)."

   **Correct text:**
   > "phase17 self-compile completes in ~50s parallel / ~101s serial,
   > unchanged from v35.4.1 baseline (within noise). COW + Rc/Arc-share
   > `.clone()` keeps D-FULL ownership semantics off the hot path."

2. **GitHub Release v35.5.0** notes carry the same "~96–106s vs baseline ~54s"
   wording. Should be amended via `gh release edit v35.5.0 --notes ...`.

3. **`memory/project_resume_lanjut_protocol.md`** flag #4 ("Phase 17
   perf 2× regression") should be marked CLOSED — non-issue.

### What this does NOT change

- v35.5.0's correctness, gates, byte-equality preservation: unchanged.
- D-FULL ownership semantics: still default-on, still closes Compass §4.4.
- COW runtime: still required (without it, OOM at ~26 GB during chain
  bootstrap; this finding is preserved in the FJARR_LEAK_PHASE_2_D_FULL
  closure proof and is independent of phase17 wall-clock).

---

## Methodology corrections to lock in

For future ship-cycle perf claims:

1. **Always specify the test-thread protocol** when quoting wall-clock.
   `cargo test foo` (parallel) and `cargo test foo -- --test-threads=1`
   (serial) can differ by 2× on phase17. Cite the flag.
2. **Always re-measure both old and new at the same protocol** before
   claiming a regression or improvement. Apples to apples.
3. **Pre-push hook protocol is canonical for "did we regress?"** — the
   hook runs `--test-threads=1`. Compare to the same flag's prior reading.

This is a corollary to Plan Hygiene Rule 4 (multi-agent audit cross-check):
single-protocol perf numbers are agent-unverifiable; future perf claims
need the literal command + flag in the commit message.

---

## Disposition

- ✅ Findings doc committed locally (this file).
- ⏸️ CHANGELOG correction: **awaits user decision** (see questions).
- ⏸️ GitHub Release amendment: **awaits user decision**.
- ✅ Resume-protocol memory will be updated to close perf flag.
- ✅ B0 closes here. No optimization needed; D-FULL is already free.

---

## Self-check (Plan Hygiene §6.8)

```
[x] Pre-flight audit (B0) hands-on verifies baseline?      (R1)
[x] Verification commands runnable and quoted in doc?       (R2)
[x] Prevention layer added (methodology corrections §)?     (R3)
[x] Numbers cross-checked with Bash, not just memory?       (R4)
[x] Effort variance tagged in commit message?               (R5)
[ ] Decision file blocks downstream?                         (R6) — N/A (B0 result is "no work")
[x] Internal doc fix audited for public-artifact drift?     (R7) — surfaced for user decision
[ ] Multi-repo state check?                                  (R8) — N/A (single repo)
```

5/6 applicable YES. R6 is N/A because the B0 outcome is "no follow-on
phase needed" — there is no downstream to gate. R8 is N/A.
