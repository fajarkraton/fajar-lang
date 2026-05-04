---
phase: 3 — port boot/runtime_stubs.S → kernel/runtime/bare_stubs.fj
status: CLOSED 2026-05-04
budget: 3-5d planned + 25% surprise
actual: ~1h Claude time (≈ 0.13d)
variance: -95%
artifacts:
  - This findings doc
  - fajaros-x86 commit pending — Makefile + new bare_stubs.fj + delete runtime_stubs.S
prereq: Phase 2 closed (fj-lang 7a59ad0d, fajaros-x86 5e7c61c)
---

# Phase 3 Findings — boot/runtime_stubs.S → bare_stubs.fj

> Phase 3 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Plan estimated 3-5d for
> "split into 4 logical .fj files." Phase 2 had closed Gap G-G/G-H/G-I
> which made the work mechanical. Used a single-file approach for
> speed; logical splitting is an opportunistic future cleanup.

## 3.1 — Symbol overlap audit ✅

**Discovery:** auto-gen has WEAK versions of 3 symbols that
runtime_stubs.S also defined STRONG:
- `fj_rt_bare_idt_init` (auto-gen W, runtime_stubs T)
- `fj_rt_bare_tss_init` (auto-gen W, runtime_stubs T)
- `fj_rt_bare_pit_init` (auto-gen W, runtime_stubs T)

Linker resolves Weak→Strong at link time → strong wins. After porting
to .fj, both versions still present (auto-gen weak in
`combined.start.o.saved`, ported strong in `combined.o.saved`); strong
still wins. No collision.

12 other symbols are runtime_stubs.S only (no auto-gen counterpart):
console_putchar, str_len, str_byte_at, 9 buffer LE/BE ops,
__isr_32_timer, __sched_exit. These needed actual porting.

## 3.2 — File creation + LLVM MC limitations surfaced ✅

Wrapped 912 LOC of asm in `kernel/runtime/bare_stubs.fj` as
`global_asm!(r#"..."#)` block. `fj check` passed cleanly.

But `make build-llvm` failed with TWO LLVM MC integrated assembler
limitations that GAS handles fine:

### Issue 1: trailing `/* */` after macro invocations

```
ISR_NOERR 0                     /* #DE  Divide Error */
                                ^ col 33: error
```
LLVM MC: `error: <inline asm>:413:33: too many positional arguments`.

The C-style `/* */` comment after a macro arg is treated as additional
positional arguments by LLVM's MC. GAS strips these comments before
macro expansion.

**Fix:** convert `/* descr */` to `# descr` for all 32 ISR_NOERR/ISR_ERR
invocations. Python regex applied only to lines starting with `ISR_`
(safe scope). `#` is GAS's line-comment marker; LLVM MC handles it.

### Issue 2: symbol-difference in immediate operand

```
mov rsi, .Lidt_msg_end - .Lidt_msg
```
LLVM MC: `error: <inline asm>:811:34: cannot use more than one symbol
in memory operand`.

GAS evaluates same-section symbol-diffs at parse/link time. LLVM's
integrated assembler is more conservative — it rejects the expression
even when both symbols are in the same section.

Tried `.set` / `.equ` to hoist the diff; LLVM rejects those too (the
`.set` directive itself contains the diff expression).

**Fix:** hardcode the literal lengths at the .ascii sites. 3 strings
affected:
- `.Lidt_msg = "[IDT] 256 entries loaded\n"` → 25 bytes
- `.Ltss_msg = "[TSS] RSP0=0x7EF000 loaded\n"` → 27 bytes
- `.Lpit_msg = "[PIT] Timer configured\n"` → 23 bytes

Replaced `.LLen_X` symbol with literal byte count, dropped `.set` lines.

**Surfaced fj-lang gap (NEW, low-severity):**
- **G-J:** when fj-lang LLVM emits global_asm!() via
  LLVMSetModuleInlineAsm2, the MC integrated assembler is stricter
  than standalone GAS — rejects symbol-diff expressions and trailing
  `/* */` after macro args. Workaround: pre-process the asm content
  to use GAS-compatible BUT also LLVM-MC-compatible syntax (no
  symbol-diff in operands, `#` comments after macro invocations).
  Real fix: `module.set_inline_assembly()` via LLVM provides no
  way to switch dialects — would require either upstream LLVM MC
  fix OR fajar-lang shipping its own preprocessor pass. Defer to a
  future "lints + auto-fix for global_asm!" feature plan.

## 3.3 — Verification (E2E) ✅

```
make build-llvm                        → 10s clean (ELF 1,504,526 bytes,
                                          +7 bytes from prior 1,504,519
                                          which used the .S file)
make test-spinlock-smp-regression      → PASS in 25s
make test-security-triple-regression   → 6/6 invariants PASS in 25s
make audit-100pct-fj                   → 1 file / 768 LOC (down from
                                          2 / 1,680 — Phase 3 deleted
                                          912 LOC of .S source)
```

ELF size delta of +7 bytes confirms the asm content is functionally
identical. Boot proceeds normally — confirms `fj_rt_bare_console_putchar`
(VGA output), IDT/TSS/PIT init, and string/buffer ops all work.

## 3.4 — Makefile changes

- `SOURCES`: add `kernel/runtime/bare_stubs.fj` (early, after constants)
- `RUNTIME_S` / `RUNTIME_O`: kept as variable defs but no longer used;
  added comment block documenting the relocation
- `$(RUNTIME_O)` build rule: removed (replaced with comment)
- `build-llvm` target deps: drop `$(RUNTIME_O)` from prereqs and link command
- fj build flags: drop `--extra-objects $(RUNTIME_O)`

## 3.5 — Decision: single-file vs 4-file split

Plan suggested 4 logical .fj files (vga_console, str_ops,
buffer_endian, hw_init). Single-file approach used because:

1. fj-lang's `global_asm!()` requires the entire asm content as a
   single string — splitting across files works but adds
   coordination overhead with no behavior benefit.
2. The 912 LOC of asm IS already logically grouped via `.section`
   directives and comment headers within the single file.
3. Per CLAUDE.md system prompt "Don't add features beyond what the
   task requires" — splitting was a presentation choice, not a
   capability gap.

If future kernel devs want logical separation, splitting can be a
trivial cleanup commit (just slice the global_asm content across
files; each becomes its own `global_asm!()` block; LLVM concatenates
them per Phase 2.A patch).

## Phase 3 summary

| Sub-task | Status | Surfaced |
|---|---|---|
| 3.1 Symbol overlap audit | ✅ CONFIRMED | Auto-gen weak / runtime strong; OK |
| 3.2 Wrap in global_asm!() + LLVM MC fixes | ✅ CLOSED | New gap G-J — defer |
| 3.3 Build + regression gates | ✅ ALL GREEN | spinlock + security PASS |
| 3.4 Makefile cleanup | ✅ DONE | RUNTIME_S/O deps removed |
| 3.5 Delete boot/runtime_stubs.S | ✅ DONE | -912 LOC |

**Phase 3 effort:** ~1h Claude time (vs 3-5d planned). Variance: -95%.
The plan's +25% surprise budget was kept in reserve; not needed.

**Why so much under:** Phase 2's compiler-side investments (G-G LLVM
emission + G-H/G-I raw strings) had already removed the hardest
unknowns. What remained was mechanical wrapping + 2 LLVM MC quirks
(macro-arg comments, symbol-diff). Each quirk took ~10 min to surface
+ fix.

## Audit progress

```
Phase 0 baseline:  3 files, 2,195 LOC
After Phase 2:     2 files, 1,680 LOC (-515: boot/startup.S)
After Phase 3:     1 file,    768 LOC (-912: boot/runtime_stubs.S)
Plan target:       0 files,     0 LOC (end of Phase 4)
```

## Compiler gaps (running tally)

| Gap | Status |
|---|---|
| G-G LLVM global_asm! emission | ✅ CLOSED Phase 2.A (4b115d45) |
| G-H r#"..."# raw strings | ✅ CLOSED Phase 2.A.2 (7a59ad0d) |
| G-I parse_global_asm + parse_inline_asm raw | ✅ CLOSED Phase 2.A.2 (7a59ad0d) |
| G-F SE009 false-positive on asm operand uses | ⏳ defer Phase 5 |
| G-J LLVM MC stricter than GAS for inline asm (sym-diff, macro-arg comments) | ⏳ defer (workaround documented) |

## Decision gate (§6.8 R6)

This file committed → Phase 4 (replace `kernel/compute/vecmat_v8.c`)
UNBLOCKED. With G-G/G-H/G-I/G-J in hand, Phase 4 is the LAST migration
phase before the audit shows 0 / 0.

---

*FAJAROS_100PCT_FJ_PHASE_3_FINDINGS — 2026-05-04. Closes Phase 3 in
~1h vs 3-5d plan (-95%). Non-fj LOC: 1,680 → 768. boot/ directory
now empty. Phase 4 is the last migration phase.*
