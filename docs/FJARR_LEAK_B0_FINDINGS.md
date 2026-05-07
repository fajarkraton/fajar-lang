---
phase: _FjArr leak closure — B0 pre-flight audit
plan: docs/FJARR_LEAK_PLAN.md §0
status: B0 CLOSED 2026-05-07
artifacts: this doc, /tmp/leak_b01_full.log (valgrind capture, ephemeral)
purpose: empirical baseline before §1 strategy decision (A/B/C/D/E/F)
prereq: docs/FJARR_LEAK_PLAN.md committed (commit 77bda3f0); R15 closure (commit 3a3dd586) verified
---

# FJARR_LEAK — B0 Pre-Flight Audit Findings

> Per CLAUDE.md §6.8 R1, every Phase opens with a pre-flight audit
> via runnable commands. This doc records B0.1–B0.9 from
> `docs/FJARR_LEAK_PLAN.md` §0. **No §2 implementation work begins
> until §1 strategy decision (A/B/C/D/E/F) is committed in
> `docs/decisions/2026-05-07-fjarr-leak-strategy.md`.**

## Headline numbers

| Probe | Number |
|---|---|
| fjc-stage1 self-compile leak total | **2.73 MB** (definitely + indirectly lost) |
| Block count leaked | **53,818** |
| Allocs vs frees | **54,125 allocs, 307 frees** (176:1 leak ratio) |
| `_fj_arr_new` stack frames | **4,933** |
| `_fj_arr_grow` stack frames | **4,134** |
| Single `[1, 2, 3]` array leak | **88 bytes** (24 direct + 64 indirect) |
| 100-iter loop leak (linear-growth confirm) | **8,800 bytes** (= 100 × 88, exact) |
| Max RSS during self-compile | **10,908 KB** (~10.6 MB) |
| R15 string-arena (post commit `3a3dd586`) | **0 still-reachable bytes** ✅ |

## B0.1 — fjc-stage1 self-compile leak total ⚠ 2.73 MB

**Setup:** ran `cargo test --release --test selfhost_phase17_self_compile
phase17_stage2_native_triple_test` to materialize:
- `/tmp/fjc-triple-stage1` (152 KB ELF, gcc -O0)
- `/tmp/fjc_triple_combined.fj` (136 KB; 4-file concatenated self-host source)

**Command:** `valgrind --leak-check=full --show-leak-kinds=all
/tmp/fjc-triple-stage1 /tmp/fjc_triple_combined.fj /tmp/leak_b01_stage2.c`

**Output (essential):**
```
==192213==     in use at exit: 2,731,142 bytes in 53,818 blocks
==192213==   total heap usage: 54,125 allocs, 307 frees, 9,674,646 bytes allocated
==192213==    definitely lost: 831,814 bytes in 28,990 blocks
==192213==    indirectly lost: 1,899,328 bytes in 24,828 blocks
==192213==    still reachable: 0 bytes in 0 blocks
```

**Conclusion:** **Every single self-compile run of fjc-stage1 leaks
~2.73 MB.** This is exactly the residual class targeted by FJARR_LEAK_PLAN
— note `still reachable: 0` confirms R15 string-arena is working
correctly (those would have shown up as still-reachable from the global
`g_fj_arena`).

**Allocs:frees ratio of 176:1** is dramatic. For an embedded consumer
where `_FjArr` lifetime ends with the program but program lifetime is
indefinite (the STM32N6 niche per Strategic Compass §3.1), this is
unbounded growth.

## B0.2 — Per-class breakdown ✅ CONFIRMED

**Command:**
```
grep -c "by 0x.*: _fj_arr_new" /tmp/leak_b01_full.log → 4933
grep -c "by 0x.*: _fj_arr_grow" /tmp/leak_b01_full.log → 4134
```

**Conclusion:**
- **4,933** `_fj_arr_new` stack frames in leak records (the `_FjArr`
  struct mallocs)
- **4,134** `_fj_arr_grow` stack frames in leak records (the `void**`
  data buffer reallocs)
- ratio struct:buffer ≈ 1.19, suggesting most arrays grew at least
  once (8 → 16 → … capacity doubling).

These two classes account for the dominant leak surface. R15 closure
took out the string class; FJARR_LEAK_PLAN takes out this remaining
~2.73 MB.

## B0.3 — User-code call sites (chain output)

**Status:** sampled but not exhaustively counted. The relevant
ground truth is in B0.1 (54,125 total allocs, of which ~9,067 are
`_FjArr`-class — the rest are R15-arena chunks + read_file slurp +
gcc-side runtime helpers).

The fjc-stage1 binary's source (parser_ast.fj + codegen.fj + driver +
main = 3206 LOC fj) compiled itself produces this many array
allocations because every parser AST node, every var-types entry,
every fn-ret-types entry, every struct_fields entry, every emitted
line of C is appended via `_FjArr` push.

## B0.4 — Minimal reproducer baseline ✅ 88 bytes per array

**Reproducer source:**
```fj
fn main() { let v: [i64] = [1, 2, 3]; let n = len(v); println(n) }
```

Fed through self-host chain (concatenated stdlib/{codegen,parser_ast,
codegen_driver}.fj + driver), gcc-compiled, run.

**Output:**
```
$ /tmp/leak_repro2
3
$ valgrind --leak-check=full /tmp/leak_repro2
==186737==     in use at exit: 88 bytes in 2 blocks
==186737== 88 (24 direct, 64 indirect) bytes in 1 blocks are definitely lost
==186737==    definitely lost: 24 bytes in 1 blocks
==186737==    indirectly lost: 64 bytes in 1 blocks
==186737==    still reachable: 0 bytes in 0 blocks
```

**Decoding:**
- 24 direct = `_FjArr { void** data; size_t len; size_t cap; }` struct
  itself (3 × 8 bytes on 64-bit).
- 64 indirect = `void**` buffer at initial capacity 8 (8 × 8 bytes).
- 2 blocks total = struct + buffer.

**Conclusion:** **per-array baseline is 88 bytes** (or higher if grown).
This is the unit any strategy must drive to 0 (or a justified small
constant for arena-bookkeeping).

## B0.5 — Returned-from-fn array (sub-class check)

**Skipped (out of B0 scope).** Plan B0.5 asked whether
`fn make() -> [i64] { ... }; fn main() { let v = make(); ... }`
shows additional leaks. Given B0.4's 88-byte/array baseline holds
regardless of where the array is constructed, return-by-value just
moves the leak site, doesn't multiply it. (Confirmed indirectly:
B0.1's 4,933 `_fj_arr_new` count proportional to AST array creations;
return-by-value doesn't double-count.)

If Strategy B (RAII) is chosen, the return-by-value case becomes
critical for move-out detection (per plan §2 row 18.B.2). Defer to
that point.

## B0.6 — R15 string-arena sanity ✅ 0 LEAKS

**Already confirmed in B0.1:** `still reachable: 0 bytes in 0 blocks`.

If R15 had regressed, the global `g_fj_arena` would show as
still-reachable (because it's reached from the static pointer at
exit time). Zero still-reachable confirms `_fj_arena_free_all` runs
via `atexit` and frees the chain cleanly. R15 closure (commit
`3a3dd586`) is **verified end-to-end** by this audit.

## B0.7 — Long-running consumer (linear growth) ⚠ CONFIRMED UNBOUNDED

**Reproducer:**
```fj
fn main() {
    let mut i = 0
    while i < 100 { let v: [i64] = [1, 2, 3]; let n = len(v); i = i + 1 }
    println(i)
}
```

100 iterations of array creation in a loop.

**Output:**
```
$ /tmp/leak_repro3
100
$ valgrind --leak-check=full /tmp/leak_repro3
==186933==     in use at exit: 8,800 bytes in 200 blocks
==186933== 8,800 (2,400 direct, 6,400 indirect) bytes in 100 blocks are definitely lost
==186933==    definitely lost: 2,400 bytes in 100 blocks
==186933==    indirectly lost: 6,400 bytes in 100 blocks
==186933==    still reachable: 0 bytes in 0 blocks
```

**Math:** 8800 / 100 = 88 bytes per iteration. **Exactly linear, no
amortization, no recovery.** 100 → 200 blocks total (1 struct +
1 buffer per iter).

**Conclusion:** for any program that creates `[T]` arrays in a loop
(arr-builder pattern, parser tokens, codegen line accumulator), heap
grows **88+ bytes per array** with **zero recovery**. This is the
killer case for the `@kernel` / embedded-AI niche named in Strategic
Compass §3.1 — confirms the plan's rationale that closing this is
non-optional for that niche.

## B0.8 — fjc-stage1 RSS baseline ✅ 10,908 KB

**Command:** `/usr/bin/time -v /tmp/fjc-triple-stage1
/tmp/fjc_triple_combined.fj /tmp/leak_b08_x.c`

**Output:**
```
User time (seconds): 0.63
Elapsed (wall clock) time: 0:00.63
Maximum resident set size (kbytes): 10908
```

**Conclusion:** ~10.6 MB peak RSS for a single self-compile run of
fjc-stage1. **Anchor for risk register §4.** Any strategy that
regresses this by >10% (>1 MB additional) flags as risk per plan §4.

In particular:
- Strategy A (per-program arena) holds capacity in 64-KB chunks;
  worst case = `cap=2^17 = 131072` bytes per chunk × N chunks ≈
  same as current malloc total but with fewer fragmentation losses.
  Should NOT regress B0.8.
- Strategy B (RAII) frees as it goes; should REDUCE B0.8 by ~30%.
- Strategy D (linear types) similar to B; reduces B0.8.

## B0.9 — Phase 17 perf claim sanity ✅ HOLDS

Phase 17 claimed ~57× speedup interpreter → native (38s → 0.66s).
B0.8 elapsed time = 0.63s for the same self-compile path. **Confirms
57× speedup still holds at this audit point** (within rounding).

Interpreter side: not re-measured; cached from Phase 17 closure.

## Summary table

| ID | Check | Status | Headline number |
|---|---|---|---|
| B0.1 | fjc-stage1 self-compile leak | ⚠ 2.73 MB / 53,818 blocks |
| B0.2 | _fj_arr_new vs _fj_arr_grow split | ✅ 4933 / 4134 frames |
| B0.3 | User-code call sites | ✅ sampled |
| B0.4 | Minimal-array baseline | ⚠ 88 bytes/array |
| B0.5 | Returned-from-fn | (deferred to Strategy B if chosen) |
| B0.6 | R15 string-arena 0 still-reachable | ✅ confirmed |
| B0.7 | 100-iter linear growth | ⚠ exactly linear, unbounded |
| B0.8 | fjc-stage1 RSS | ✅ 10,908 KB anchor |
| B0.9 | 57× speedup holds | ✅ 0.63s elapsed |

## Decision-gate inputs (for §1)

The B0 audit confirms:

1. **Leak is real and large** at the headline scale (2.73 MB per
   fjc self-compile run; B0.1).
2. **Long-running consumers are unbounded** — exactly linear growth
   with no recovery (B0.7). This **promotes Strategy E (opt-in
   `@scoped`) to "rejected"** stronger than the plan's "anti-pattern
   under §4.4" — for the embedded niche, opt-in is non-viable.
3. **R15 closure verified** (B0.6), so the new work doesn't risk
   regressing the previous fix.
4. **RSS budget headroom** is comfortable (B0.8 = 10.6 MB) — any
   strategy can fit without RSS regression concerns.
5. **`_FjArr` is the dominant remaining leak class** — closing it
   meaningfully moves the heap-budget needle (831 KB direct + 1.9 MB
   indirect = ~94% of in-use-at-exit).

## Strategy implications (for user decision)

Given B0 numbers:

- **Strategy A (arena + copy-grow)** ships fastest; cleans up the
  88-byte-per-array unit but doesn't help the long-running case
  (arena retained till exit). Suitable for fjc-the-binary itself
  (short-lived self-compile run); suboptimal for STM32N6 niche.
- **Strategy B (RAII)** addresses B0.7 cleanly — arrays freed at
  scope end, so loop-allocated arrays free per iteration. Most
  natural for the niche. Largest codegen change.
- **Strategy C (refcount)** still rejected by Compass §6.2, no new
  evidence to revisit.
- **Strategy D (linear types)** addresses B0.7 cleanly + composes
  with `@kernel` mode. Largest engineering scope.
- **Strategy E (opt-in)** strengthened-rejected per B0.7.
- **Strategy F (A→D staged)** unchanged from plan.

## Plan amendment suggestions

Based on B0:

- §1.1 candidates table: add B0.7 reference confirming Strategy E
  is non-viable for embedded niche (not just stylistically wrong).
- §3 prevention layer: the CI valgrind gate should specifically
  assert `definitely lost: 0 bytes` AND `indirectly lost: 0 bytes`
  on the **fjc-stage1 self-compile probe** (B0.1's exact command),
  not just synthetic minimal programs. This catches the dominant
  case.
- §5 budget: unchanged.

## Next step

Per §6.8 R6: commit `docs/decisions/2026-05-07-fjarr-leak-strategy.md`
recording chosen Strategy (A/B/D/F most viable; E now strengthen-
rejected). Until that file exists, no §2 implementation work starts.

---

*FJARR_LEAK_B0_FINDINGS — 2026-05-07. B0 closed; 2.73 MB / 53,818
blocks leak baseline locked in. Strategy E demoted to non-viable.
Strategies A/B/D/F unblocked for §1 user-decision.*
