# V31 Track B Phase P0 — Pre-Flight Findings

**Session:** V31.FAJARLANG (Track B, LLVM O2 codegen fix)
**Date:** 2026-04-20
**Plan:** `docs/V31_MASTER_PLAN.md` §3

## P0.1 Minimal repro attempt — result: user-space does NOT reproduce

`examples/v31b_vecmat_miscompile_repro.fj` isolates the inner-loop
pattern of the kernel's `km_vecmat_packed_v8`:

- Outer loop `j ∈ [0, n)`, inner loop `k ∈ [0, m)`
- Nibble-style extraction via `((k*37 + j*17) & 15)` (replaces
  `volatile_read_u8` + nibble unpack from the kernel version)
- Group-wise scale lookup via `(g & 31) * 100`
- Accumulator `sum = sum + (v * w_x_1M) / v8_scale_fp` matching kernel
- Same magnitudes (i64, scale=1e6, values 5-digit)

Results at real FFN-gate dimensions (m=1152, n=6912 = 7.96M inner iters):

| Backend | sum_total | Match Python |
|---|---|---|
| Python reference | -345192 | — |
| `fj run` tree-walker | -345192 | ✅ |
| `fj build --backend llvm --opt-level 2` | -345192 | ✅ (**no miscompile**) |

**The standalone user-space binary produces CORRECT output at the
same iteration count that triggered the kernel miscompile.**

## Interpretation

The kernel miscompile is NOT caused by the loop shape alone. It
depends on one or more of:

1. **Volatile memory accesses** — kernel uses `volatile_read_u8`,
   `volatile_read_u64`, `mdl_read_u32` instead of regular memory.
   LLVM treats volatile ops as side-effectful, which can constrain
   alias analysis and vectorization choices.

2. **`@kernel` function attribute + build flags** — kernel is
   built with `--target-cpu x86-64 --target-features="-avx,-avx2,-avx512f,+popcnt,+aes" --code-model kernel --reloc static`.
   LLVM with AVX disabled still has SSE2-class vectorization
   available; the interaction with the kernel code model may
   differ from user-space.

3. **Link-time effects** — kernel links with `runtime_stubs.o`
   and goes through a custom linker script. The user-space binary
   is statically linked to `fj_rt` directly.

4. **`no-red-zone` + `mno-avx2`** — kernel Makefile passes
   additional flags to gcc for C bypass; the Fajar Lang compile
   side may inherit differently.

## P0.2 Pre-opt IR capture — result: captured but not yet diagnostic

`FJ_EMIT_IR=1 fj build --backend llvm --opt-level 2 ...` wrote
`examples/v31b_vecmat_miscompile_repro.ll` (64 lines).

Observation: the emitted IR appears to be POST some optimizations
(uses `tail call`, `nonnull`, phi-node merging). This is useful for
seeing the optimized shape but not for a pass-by-pass bisect.

To fully capture pre-opt IR, need either:
- A new `--emit-ir pre-opt` flag in fj
- Or pipe through `opt -O2 -print-after-all` externally on the
  pre-opt .ll file

## B.P1 scope change — cannot do simple pass bisect in user-space

Since user-space doesn't reproduce, the plan's B.P1 strategy of
`opt -print-after-all` on a minimal case **won't work**. The
bisect would need to run inside a kernel-flag build, which is
much harder to isolate.

Feasible alternatives for B.P1:

**Alt 1: Kernel-side pass bisect** (3-5 days)
- Build a dedicated kernel test binary that does ONLY the vecmat
  and prints output to serial
- Disable LLVM passes one-by-one until miscompile disappears
- Requires custom Makefile target with `-Xllvm -disable-licm`
  etc. style flags through fj

**Alt 2: Kernel-flag user-space repro** (1-2 days)
- Replicate kernel build flags: `--target-features="..."
  --code-model kernel --reloc static` on the user-space program
- Mimic volatile ops by forcing `volatile_read_u8`-style accesses
- If repro works, pass-bisect as originally planned
- Risk: kernel-code-model user-space binary may fail to link/run

**Alt 3: Accept C bypass as permanent** (immediate)
- V30/V31 already ship the C bypass for 10 functions
- Phase D (Track C) is an integer-native arch — may not need
  vecmat in the same way
- Defer Fajar Lang fix until a concrete Phase D op needs it

## Recommendation

**Alt 3 for immediate path, Alt 1 for long-term cleanliness.**

The C bypass pattern is proven (10 functions shipped, V31.R3 H1+H3
success demonstrate it works). Phase D (custom IntLLM) is 6-8 weeks
of research work where the bottleneck is ARCHITECTURE not codegen.
Fixing Fajar Lang's LLVM O2 miscompile is a 3-5 day deep-dive that
might NOT be blocking if Phase D avoids the large-vecmat pattern.

## Decision

**Gate G3 partial close:** P0 done. B.P1 scope DEFERRED pending
Phase D architecture decision. Full B.P1 plan (via Alt 1 kernel
bisect) will re-open once Phase D arch is chosen — if Phase D uses
large-matrix ops, fix Fajar Lang. If Phase D is state-space /
RWKV / integer-attention without large matmul, maintain C bypass.

**Alternative scope for Track B (immediate work available):**

Even without fixing the root-cause miscompile, we can ship these
Fajar Lang improvements that help V32+:

| Task | Effort | Benefit |
|---|---|---|
| Implement `@no_vectorize` attribute (V31.B.P2) | 1-2 days | Preventive mechanism; ships even if miscompile never fixed |
| `i128` codegen audit (V31.B.P3) | 0.5 day | Phase D wider-fixed-point ready |
| Update FJ_EMIT_IR to dump TRUE pre-opt (no intermediate passes) | 0.5 day | Better diagnostic tool |

Total: 2-3 days of incrementally useful Fajar Lang work that
doesn't require repro of the miscompile.

## Files

- `examples/v31b_vecmat_miscompile_repro.fj` — repro attempt
- `examples/v31b_vecmat_miscompile_repro.ll` — post-opt IR dump
- `docs/V31_FAJARLANG_P0_FINDINGS.md` — this file

## Budget

Budget B.P0: 1 day. Actual: ~0.5h. Under-ran because the repro
result (standalone DOESN'T miscompile) was an early-exit finding
that collapses P1 scope.
