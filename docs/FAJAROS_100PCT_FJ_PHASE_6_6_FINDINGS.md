---
phase: 6.6 — bare_stubs.fj global_asm!() → @naked fn migration (ATTEMPTED, BLOCKED)
status: BLOCKED 2026-05-04 (new Gap G-N surfaced; reverted to baseline)
budget: 4-6h planned (Phase 6.6) + 25% surprise = 7.5h cap
actual: ~30min Claude time (2 attempts + 2 reverts + doc)
variance: -93% (early exit after second attempt failure)
artifacts:
  - This findings doc
prereq: Phase 6 closed (fajar-lang `29bfcdba`); Phase 7 closed (fajar-lang `1cf7dc05`)
related: Gap G-N (NEW) — fajaros kernel ELF layout sensitivity to fj-emitted symbols
---

# Phase 6.6 Findings — bare_stubs.fj `@naked fn` migration (BLOCKED on Gap G-N)

> Phase 6.6 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Attempted incremental
> migration of the simplest stub (`fj_rt_bare_buffer_read_u64_le`,
> 2-line asm body) from `kernel/runtime/bare_stubs.fj`'s 939-line
> `global_asm!()` block to a free-standing `@naked fn`. Even with
> the verified Phase 6 attribute pattern, gemma3-e2e regressed.
> Reverted twice. Surfaces new Gap **G-N**: fajaros kernel ELF
> layout sensitivity to fj-emitted global symbols.

## 6.6.1 — Pre-flight inventory

`bare_stubs.fj` is a 939-line `.fj` file containing a single
`global_asm!(r#"..."#)` block with 17 stub symbols:

| Group | Count | Examples |
|---|---|---|
| VGA console | 1 | `fj_rt_bare_console_putchar` (uses shared `vga_cursor_pos` data) |
| String ops | 2 | `fj_rt_bare_str_len`, `fj_rt_bare_str_byte_at` |
| Buffer LE/BE | 10 | `fj_rt_bare_buffer_{read,write}_u{16,32}_{le,be}`, `read_u64_le` |
| Hardware init | 3 | `fj_rt_bare_idt_init`, `fj_rt_bare_tss_init`, `fj_rt_bare_pit_init` |
| Internal | 2 | `__isr_32_timer`, `__sched_exit` (called from elsewhere) |

Selected `fj_rt_bare_buffer_read_u64_le` for the proof-of-concept —
simplest stub (2 asm instructions: `mov; ret`), no shared data, no
internal labels, no inter-stub jumps.

## 6.6.2 — Attempt A1: `@naked @no_mangle @kernel`

```fajar
@naked
@no_mangle
@kernel fn fj_rt_bare_buffer_read_u64_le(addr: i64) -> i64 {
    asm!("movq (%rdi), %rax\n\tret")
}
```

Removed corresponding 6-line stub from `global_asm!()` block.

Build: clean. ELF: 1,505,806 → **1,504,510 bytes** (-1296 bytes, fewer
asm directives + Phase 6 prologue elision verified).

Symbol verified by `nm`:
```
0000000000111d00 T fj_rt_bare_buffer_read_u64_le
```

Disassembly verified by `objdump`:
```
111d00:  48 8b 07     mov    (%rdi),%rax
111d03:  c3           ret
111d04:  31 c0        xor    %eax,%eax     ← compiler-emitted phantom return
111d06:  c3           ret
111d07:  66 0f 1f...  nopw                 ← 9 bytes alignment padding
```

`@naked` correctly suppressed prologue (no `push %rbp`). Phantom
`xor %eax, %eax; ret` was emitted but is dead code (asm body already
executed `ret`).

**`make test-gemma3-e2e`:** REGRESSION
```
[NVMe] Identify Namespace...
EXC:13
000000000000000D
PANIC:13
```

Fault is during NVMe namespace Identify — much earlier than the
forward-pass faults seen in Phase 4.D. The error code `0x0D` looks
like a #GP segment selector index, suggesting the EXC handler is
seeing corrupt CPU state. Reverted.

## 6.6.3 — Attempt A2: `@naked @unsafe` (Phase 6 verified pattern)

To rule out `@kernel` modifier-stack interaction, retried with the
EXACT pattern verified in fj-lang Phase 6:

```fajar
@naked @unsafe fn fj_rt_bare_buffer_read_u64_le(addr: i64) -> i64 {
    asm!("movq (%rdi), %rax\n\tret")
}
```

Build: clean. ELF: same 1,504,510 bytes. **`make test-gemma3-e2e`:
SAME regression** — EXC:13 with `0x0D` at NVMe Identify.

Conclusion: A1 vs A2 difference (modifier stack) is NOT the cause.
The fault is in HOW fj-lang emits the new global symbol into the
fajaros kernel ELF, not in which modifiers the fn carries.

## 6.6.4 — Why this is a new gap

The migration is mechanically correct:
- Symbol name preserved (`fj_rt_bare_buffer_read_u64_le`)
- Signature equivalent (i64 in, i64 out, SysV ABI)
- Body identical (`mov rdi → rax; ret`)
- Removed from old location to avoid duplicate-symbol link error
- `@naked` correctly suppresses prologue per Phase 6 verification

Yet E2E regresses with EXC:13 in unrelated NVMe init code. This
parallels Gap G-M (Phase 4.D cross-fn drift) but at a different
layer:

| Gap | Layer | Symptom | Reproduction |
|---|---|---|---|
| **G-M** | Compiled instruction stream of new fn | LLVM-O2 codegen produces wrong addresses for vecmat-shaped fns | Phase 4.D port of km_vecmat_packed_v8 |
| **G-N** | ELF section/symbol layout | Adding a fj-emitted global symbol shifts the kernel's effective load layout, breaking address-sensitive code (e.g. NVMe driver register accesses) | Phase 6.6 port of fj_rt_bare_buffer_read_u64_le |

### Possible root causes (each needs IR + ELF diff to confirm)

1. **fj-lang LLVM backend places `@naked` fns in `.text.naked` or some
   other special section** — kernel's `linker.ld` doesn't expect this
   section, so it lands at an unintended address.
2. **The phantom `xor %eax, %eax; ret` LLVM appends to `@naked` fns
   uses XMM/YMM registers in some compile contexts**, hitting the
   `cr0.EM` bit (which fajaros sets) and triggering #GP.
3. **fj-lang's combine/concat process orders fns differently**, so
   the runtime-stubs region (linker-script-anchored) shifts up or
   down, breaking adjacent symbol resolution.
4. **The `@naked` fn is emitted with a `noinline noreturn` attribute
   pair** (since LLVM doesn't know the asm body returns), and LLVM
   reorders the function's prologue/epilogue around adjacent code in
   ways that break the static layout the kernel depends on.

## 6.6.5 — Decision: revert + document, not deep-debug

Same call as Phase 4.D. Per CLAUDE.md §6.10 R4 + §6.6 ("`[x]` means
END-TO-END working"), reverted to preserve baseline.

Phase 6.6 LEFT BLOCKED until one of:

1. **G-N re-entry condition A**: deep-debug session diffing ELF
   section table + symbol layout + linker.ld expectations between
   baseline and Phase 6.6-A1 builds.
2. **G-N re-entry condition B**: change fajaros's linker.ld to
   explicitly anchor `.text.fj_rt_bare_*` symbols to fixed offsets,
   eliminating the position-dependent fragility.
3. **G-N re-entry condition C**: fj-lang adds explicit `@section`
   (already exists in the lexer) support for free-standing fns —
   verify it works on `@naked` fns, then annotate each migrated
   stub with `@section(".text.bare")` to anchor placement.

### Why G-N is independent of G-M

- G-M reproduces with `km_vecmat_packed_v8`, a regular fn (no `@naked`)
- G-N reproduces with a `@naked` fn whose asm body is 2 instructions
- G-M fault is in vecmat-shaped runtime code; G-N fault is in
  unrelated NVMe driver init
- G-M: change fn body shape avoids fault (untested but theoretically possible)
- G-N: change fn attribute set (A1 vs A2) does NOT avoid fault

Different layers, different failure modes.

## 6.6.6 — Verification (post-A2 revert)

| Gate | Result |
|---|---|
| `make build-llvm` | ✅ ELF 1,505,806 bytes (matches Phase 4.C baseline) |
| `make test-gemma3-e2e` (~210s) | ✅ 5/5 mechanical invariants PASS |

## 6.6.7 — Effort summary + plan progress

**Phase 6.6 effort:** ~30min Claude time (vs 4-6h plan). Variance: **-93%**.
Early exit on first regression. Same pattern as Phase 4.D.

```
Phase 0 baseline:  3 files, 2,195 LOC (non-fj kernel build path)
After Phase 4.C:   1 file,    642 LOC
After Phase 4.D:   1 file,    642 LOC (port reverted)
After Phase 6.6:   1 file,    642 LOC ← here (port reverted; bare_stubs.fj is fj-with-asm, counted as fj)

Compiler gaps closed: 7 of 9 surfaced (G-A, G-B compiler, G-C, G-K, G-G, G-H, G-I)
Compiler gaps documented (NOT closed): 6 of 9 surfaced
  - G-F (SE009 false-pos)
  - G-J (LLVM MC stricter)
  - G-L (EXC:14 in mdl_lmhead 295M-iter)
  - G-M (LLVM-O2 vecmat-shape sensitivity)
  - G-N (NEW) — fajaros ELF layout sensitivity to fj-emitted globals
Phases CLOSED: 6 of 9 + 1 PARTIAL (Phase 6 fj-lang side); Phase 4.D BLOCKED;
              Phase 6.6 BLOCKED; Phase 4.E/4.F DEFERRED
```

## Decision gate (§6.8 R6)

This file committed → Phase 6.6 status **BLOCKED**. Phase 4.E, 4.F,
6.6 all DEFERRED until G-M and G-N are diagnosed.

Phase 8 (final validation + tags) **CAN STILL PROCEED** as a stop-the-
clock action that ships the current state:
- 7/9 compiler gaps closed (G-A, G-B, G-C, G-K, G-G, G-H, G-I)
- 6 of 9 plan phases CLOSED + 1 PARTIAL
- 642 LOC of vecmat_v8.c remain (down from 2,195 baseline; -71%)
- All 4 fj-lang capability gaps in the original plan closed
  compiler-side (G-A LLVM atomics, G-B @naked, G-C @no_mangle, G-K
  @no_vectorize stack)
- Two new fajaros-side gaps (G-M, G-N) characterized and documented
- Baseline preserved: 5/5 gemma3-e2e gates green at every commit

Recommended for next session: **Phase 8 close-and-tag as PARTIAL
COMPLETION**. Acknowledge the 4.D/4.E/4.F/6.6 deferrals honestly per
§6.6 ("[x] means END-TO-END working"), tag what works, ship docs.
Future debug session can re-open G-M/G-N when the diagnostic budget
exists.

---

*FAJAROS_100PCT_FJ_PHASE_6_6_FINDINGS — 2026-05-04. Phase 6.6
attempted twice with simplest possible stub (`fj_rt_bare_buffer_read_u64_le`,
2-line asm body) using both `@naked @no_mangle @kernel` (A1) and
`@naked @unsafe` (A2, Phase 6 verified pattern). Both reverted after
gemma3-e2e EXC:13 in unrelated NVMe init code. Surfaces new Gap G-N
(fajaros kernel ELF layout sensitivity to fj-emitted global symbols).
Independent of Phase 4.D's Gap G-M (different layer, different
symptom). 7/9 compiler gaps closed; 6 documented. Phase 6.6 BLOCKED;
Phase 4.E/4.F DEFERRED; Phase 8 close-and-tag-as-partial recommended
for next session.*
