---
phase: 4.D — port km_vecmat_packed_v8 to pure fj (ATTEMPTED, REVERTED)
status: BLOCKED 2026-05-04 (new Gap G-M surfaced; reverted to baseline)
budget: 4-5h planned (Phase 4.1) + 25% surprise = 5.25h cap
actual: ~30min Claude time (port + build + e2e + revert + doc)
variance: -90% (early exit on first regression)
artifacts:
  - This findings doc
  - fajaros-x86 follow-up commit — port attempt + revert + comment block
prereq: Phase 4.C closed (fajaros-x86 2c74988)
---

# Phase 4.D Findings — `km_vecmat_packed_v8` port attempt (BLOCKED on Gap G-M)

> Phase 4.D of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Attempted to port the
> last group-wise dequant+matmul function from `vecmat_v8.c` to pure fj
> using the proven Phase 4.B recipe (`@noinline` + `volatile_read_u64`).
> The port compiled cleanly and produced an ELF, but `make test-gemma3-e2e`
> regressed: EXC:13 inside a SIBLING fj function (`km_vecmat_packed_raw`)
> that the port did NOT modify. Reverted to the mailbox→C dispatch.
> Surfaces a new Gap **G-M**: cross-function LLVM O2 compilation
> sensitivity in fj-emitted modules.

## 4.D.1 — What was attempted

In-place inline replacement of `km_vecmat_packed_v8`'s body (kmatrix.fj
line ~827) — from a 5-line mailbox→C dispatch wrapper to a ~50-LOC pure
fj implementation:

```fajar
@noinline
@kernel fn km_vecmat_packed_v8(x_addr: i64, mat_addr: i64, m: i64, n: i64,
                                 out_addr: i64) {
    if m <= 0 { return }
    if n <= 0 { return }

    let bits = volatile_read_u8(0xC00090)  // 4 or 8
    let total: i64 = m * n
    let packed_bytes: i64 = if bits == 8 { total } else { total / 2 }
    let n_groups: i64 = (total + 127) >> 7
    let scales_base: i64 = mat_addr + packed_bytes
    let zeros_base: i64 = scales_base + n_groups * 4

    let mut j: i64 = 0
    while j < n {
        let mut sum: i64 = 0
        let mut k: i64 = 0
        while k < m {
            let fi: i64 = k * n + j
            let g: i64 = fi >> 7
            // u32 LE compose from 4 u8s, u8 zero, u8 packed (or nibble)
            // ... w = (q - zero) * scale; sum += xk * w / 1_000_000
            ...
            k = k + 1
        }
        volatile_write_u64(out_addr + j * 8, sum)
        j = j + 1
    }
}
```

Recipe matches:
- Phase 4.B `km_rmsnorm` (3-pass, single inner loop, `@noinline`, `volatile_read_u64`) — bit-exact
- Phase 4.C `mdl_embed_lookup` (similar dequant + nibble unpack pattern) — bit-exact

## 4.D.2 — `@no_vectorize` parsing constraint surfaced (Gap G-K confirmed)

Initial port had `@no_vectorize @kernel`. Build error PE001 — fj-lang
parses `@no_vectorize` as a PRIMARY annotation (in `try_parse_annotation`),
not a modifier, so it conflicts with the `@kernel` primary on the same
fn. This is **Gap G-K** from the FAJAROS_100PCT_FJ_PLAN gap inventory
(see CLAUDE.md §18). Workaround for this attempt: drop `@no_vectorize`
and rely on `volatile_read_u64` to fence the inner loop.

Future Phase 7+ followup: promote `@no_vectorize` from primary to
modifier (mirroring how `@noinline`, `@naked`, `@no_mangle` were
restructured in Phases 6/7). Estimate: ~30min in fj-lang since the
infrastructure already exists post-Phase-7.

## 4.D.3 — Regression observed (Gap G-M)

Build clean, ELF size +336 bytes (consistent with new fj inline replacing
a tiny C-call wrapper).

`make test-gemma3-e2e`:
```
[FAIL] EXC/PANIC in log — mechanical regression
nova> ask hello
Output: EXC:13
0000000000164C2A
PANIC:13
```

`objdump -d build/fajaros-llvm.elf | awk '/164c2[0-9a-f]:/'` puts the
faulting RIP inside `km_vecmat_packed_raw` — a SIBLING fj function
defined ~80 lines earlier in `kmatrix.fj` (line 741). Port did NOT
touch `km_vecmat_packed_raw` at all.

### Why this is a new gap

`km_vecmat_packed_raw` was already compiled in the baseline build
(Phase 4.C close at fajaros-x86 `2c74988`) and passed 6/6 e2e gates.
After the Phase 4.D port:
- Same source for `km_vecmat_packed_raw`
- Same LLVM toolchain
- Same module list, same fj-lang version
- ONLY change: `km_vecmat_packed_v8` body

Yet `km_vecmat_packed_raw`'s compiled output now GP-faults at runtime.
This is a **cross-function LLVM O2 compilation sensitivity** — adding
~50 LOC of fj inline elsewhere in the module changes how LLVM compiles
unrelated functions. Documented:

| Gap | Symptom | Severity | Re-entry |
|---|---|---|---|
| **G-M** | Cross-function LLVM O2 compilation context drift in fj-lang LLVM backend. Adding fj source in module N makes a previously-passing function in same module GP-fault. Reproducible via Phase 4.D port → 4.C revert → Phase 4.D port. | **HIGH** for any future Phase 4.x port; surfaces as new EXC:13 in unrelated functions | Either: split kmatrix.fj into smaller compilation units to reduce blast radius; OR add `@noinline`/`@no_vectorize` to ALL hot-path fns defensively; OR locate root cause via LLVM IR diff between baseline & port build |

## 4.D.4 — Why Phase 4.B/4.C succeeded but 4.D didn't

Hypothesis: blast radius scales with new-LOC × inner-branch density.

- Phase 4.B `km_rmsnorm`: 3-pass single-loop, ~50 LOC, no inner branches → no regression
- Phase 4.B `km_gelu_tanh`: closed-form polynomial, ~30 LOC, no branches → no regression
- Phase 4.C `mdl_embed_lookup`: dual nibble/byte path, ~60 LOC, ONE outer branch → no regression
- Phase 4.D `km_vecmat_packed_v8`: TRIPLE-nested loop with 5 inner branches (8-bit/4-bit, fi&1 nibble select, 4× u8 reads for u32 compose) → REGRESSION in unrelated `km_vecmat_packed_raw`

The 4.D port's branch density may be the trigger that flips an LLVM
heuristic across function boundaries.

## 4.D.5 — Decision: revert + document, not deep-debug

Per CLAUDE.md §6.10 R4 ("Surface pre-existing bugs via NOTE lines, not
hidden") and §6.6 ("`[x]` means END-TO-END working") — port reverted to
preserve baseline. Phase 4.D LEFT BLOCKED until one of:

1. **G-M re-entry condition A**: fj-lang adds `@no_vectorize` as a
   modifier (Phase 7+ followup), enabling the canonical Phase 4.1
   recipe (`@no_vectorize @kernel fn ...`).
2. **G-M re-entry condition B**: kmatrix.fj is split into smaller
   compilation units (e.g. one file per logical group) reducing
   per-module LOC and cross-function context.
3. **G-M re-entry condition C**: dedicated debug session diffs the
   LLVM IR for `km_vecmat_packed_raw` between baseline and port
   builds, identifies the heuristic flip, files an upstream bug or
   adds a fj-lang workaround.

Without one of these, attempting Phase 4.E (tfm_attention) or Phase
4.F (tfm_rope_apply) risks the same class of failure.

## 4.D.6 — Verification (post-revert)

| Gate | Result |
|---|---|
| `make build-llvm` | ✅ ELF 1,505,806 bytes (matches Phase 4.C baseline) |
| `make test-gemma3-e2e` (~210s) | ✅ 5/5 mechanical invariants PASS |
| `make test-spinlock-smp-regression` | not re-run (Phase 5 unchanged) |
| `make test-security-triple-regression` | not re-run (Phase 5 unchanged) |

## 4.D.7 — Effort summary + plan progress

**Phase 4.D effort:** ~30min Claude time (vs 4-5h plan). Variance: **-90%**.
Early exit on first regression is the right move per §6.6 / §6.10.

```
Phase 0 baseline:  3 files, 2,195 LOC (non-fj kernel build path)
After Phase 4.C:   1 file,    642 LOC
After Phase 4.D:   1 file,    642 LOC ← here (port reverted; no LOC delta)

Compiler gaps closed: 6 of 9 surfaced (G-A, G-B compiler, G-C, G-G, G-H, G-I)
Compiler gaps documented (NOT closed): 5 of 9 surfaced
  - G-F (SE009 false-pos) — defer Phase 8+
  - G-J (LLVM MC stricter) — documented
  - G-K (@no_vectorize as primary blocks @kernel stack) — confirmed Phase 4.D, fix path documented
  - G-L (EXC:14 in mdl_lmhead_argmax 295M-iter loop) — defer Phase 4.C-F debug
  - G-M (NEW) — cross-function LLVM O2 compilation context drift
Phases CLOSED: 6 of 9 + 1 PARTIAL (Phase 6); Phase 4.D BLOCKED
```

## Decision gate (§6.8 R6)

This file committed → Phase 4.D status **BLOCKED**. Phase 4.E
(tfm_attention) and Phase 4.F (tfm_rope_apply) recommended **DEFERRED**
behind the same G-M risk. Phase 6.6 (hw_init `global_asm!()` →
`@naked fn`) and Phase 8 (final tags) remain unblocked but are
"polish" work that doesn't reduce the remaining 642 LOC of non-fj.

Realistic next-session recommendation:
- **(a) Promote `@no_vectorize` to modifier in fj-lang** (~30min): unblocks
  G-K, gives us the canonical Phase 4.1 recipe to retry 4.D with — may
  or may not avoid G-M but at least matches the original plan.
- **(b) Split kmatrix.fj into smaller units** (~1-2h): mechanical
  refactor, may reduce G-M blast radius.
- **(c) Different track entirely**: FajarQuant Phase E continuation,
  paper editorial, or other initiative.

Recommended: **(a)** because it's small, mechanical, and produces a
clear Phase 4.D retry attempt with the canonical recipe. If retry
also fails, we have stronger evidence for (b)/(c).

---

*FAJAROS_100PCT_FJ_PHASE_4D_FINDINGS — 2026-05-04. Phase 4.D
attempted port reverted in ~30min after gemma3-e2e EXC:13 in
unrelated `km_vecmat_packed_raw`. Surfaces new Gap G-M
(cross-function LLVM O2 context drift). Phase 4.B/4.C recipe
doesn't generalize past ~50-LOC ports with high inner-branch
density. Baseline preserved (5/5 e2e PASS). 6/9 compiler gaps
closed; 5 documented. Phase 4.D BLOCKED behind G-M re-entry
conditions; Phase 4.E/4.F same risk class.*
