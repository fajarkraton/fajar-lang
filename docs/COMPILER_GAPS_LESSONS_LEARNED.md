---
title: Fajar Lang LLVM Compiler Gaps — Lessons Learned (Post FAJAROS_100PCT)
status: Reference — closed plan retrospective
date: 2026-05-05
sources: docs/FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md (closure proofs); v33.0.0 → v33.2.0 commits
---

# Fajar Lang LLVM Compiler Gaps — Lessons Learned

> **Context.** Between 2026-05-03 and 2026-05-05, FAJAROS_100PCT_FJ_PLAN
> closed 9 fj-lang LLVM compiler gaps that surfaced when we tried to
> migrate FajarOS Nova kernel from a hybrid C+fj implementation to pure
> Fajar Lang. Each gap blocked a real migration; each fix unblocked
> deletion of corresponding C/asm code in fajaros-x86. This document
> captures the patterns for the benefit of future kernel-targeting
> language designers.

## TL;DR

For a self-contained systems language to target a real kernel, it needs
nine compiler features that "general-purpose Rust-like" tutorials rarely
discuss:

| # | Gap | One-line fix |
|---|---|---|
| G-A | LLVM backend native atomics | `build_cmpxchg` + `build_atomicrmw` + `set_atomic_ordering` |
| G-B | `@naked` modifier for ad-hoc naked stubs | LLVM `naked` enum attribute on the `FunctionValue` |
| G-C | `@no_mangle` for impl-block methods | Skip `Type__method` mangling when modifier flag set |
| G-G | `global_asm!()` for raw .S blocks | `LLVMSetModuleInlineAsm2` with concatenated bodies |
| G-H | `r#"..."#` raw strings | Lexer state for raw-string disambiguation |
| G-I | Parser raw strings in asm templates | Recognize `asm!(r#"..."#)` form |
| G-K | `@no_vectorize` as **modifier**, not primary | Move from `try_parse_annotation` to modifier loop |
| G-M | `--code-model kernel` implies `noredzone` | Emit LLVM `noredzone` attribute on every fn |
| G-N | `@naked` codegen needs `noinline` + `ret undef` | Pair `naked` with `noinline`; emit `ret <undef>` not `unreachable` |

Plus 4 documented-not-closed gaps (G-F SE009 cosmetic, G-J LLVM MC stricter,
G-L runtime not compiler, plus ~~G-M~~ now closed).

## The five patterns that surface compiler gaps when you target an OS kernel

### Pattern 1 — Modifier vs primary annotations

**Symptom**: Two annotations that need to coexist (e.g. `@no_vectorize @kernel`)
fail with PE001 "primary annotation conflict."

**Root cause**: Parser puts both in the `try_parse_annotation` slot, which
allows only one primary. Annotations like `@kernel` / `@device` / `@safe` /
`@unsafe` ARE primary (they pick a context). Annotations like `@noinline` /
`@cold` / `@no_vectorize` are MODIFIERS — they stack with the primary.

**Fix pattern**: Separate the modifier flags into FnDef boolean fields
(`fndef.no_inline`, `fndef.naked`, `fndef.no_vectorize`, etc.) and
consume them in the parser's modifier loop BEFORE
`try_parse_annotation` is called. Keep the primary annotation slot
exclusive.

**Gaps that hit this**: G-K (`@no_vectorize`).

### Pattern 2 — `@naked` is more than just suppressing prologue/epilogue

**Symptom**: `@naked fn` works in isolation; in a kernel context it
silently corrupts callers — the caller exits prematurely after the
first call site, with downstream code dead-eliminated.

**Root cause**: LLVM `naked` attribute alone tells the codegen to skip
prologue/epilogue, but the function body — including the asm `ret`
instruction — is still subject to inlining. When LLVM inlines the
asm body into a caller, the inlined `ret` returns from the CALLER
instead of from the helper. The compiler also helpfully emits a
`ret 0` after the asm body (to satisfy LLVM's terminator requirement
on the entry block), which is dead code under `naked`.

**Fix pattern**: `@naked` MUST always be paired with `noinline`. The
`@interrupt` path at `mod.rs:3422` already had this right; we
forgot to mirror it for the new `@naked` modifier. Also emit
`ret undef` (not `unreachable`) for the IR terminator: `unreachable`
triggers IPO `noreturn` propagation that DCEs callers; `ret undef`
is a true terminator that doesn't carry semantic implications across
calls.

**Gaps that hit this**: G-N (closure of G-B's silent failure mode).

### Pattern 3 — Kernel mode forbids the red zone

**Symptom**: A pure-fj port of an existing C function passes tests in
isolation but #GP-faults intermittently in production under timer
IRQ. The fault address is inside the new function but at an
instruction that "shouldn't" fault.

**Root cause**: x86_64 SysV ABI permits leaf functions to use 128
bytes below `%rsp` (the red zone) without adjusting the stack
pointer. In KERNEL MODE this is unsafe: when an interrupt fires,
hardware pushes the IRQ frame BELOW the current `%rsp` (40 bytes
for `#INT`), corrupting anything stashed in the red zone.

gcc with `-mno-red-zone` emits `sub $N, %rsp` proper prologues. fj-lang
with `--code-model kernel` did NOT imply the same; LLVM cheerfully
spilled to red-zone slots like `-0x38(%rsp)`. Under load, timer IRQ
fires mid-function, corrupts the saved `out_addr`, and a subsequent
load+store turns into a #GP fault on a garbage pointer.

**Fix pattern**: When the build flag says "kernel," emit LLVM's
`noredzone` enum attribute on every function. One-liner in fj-lang:
```rust
if matches!(self.target_config.code_model, LlvmCodeModel::Kernel) {
    let kind = Attribute::get_named_enum_kind_id("noredzone");
    function.add_attribute(AttributeLoc::Function, ctx.create_enum_attribute(kind, 0));
}
```

**Gaps that hit this**: G-M. Also retroactively explained G-L
("EXC:14 in 295M-iter loop") — same red-zone class, just exposed
more frequently by longer loop running through more IRQ ticks.

### Pattern 4 — Inline asm `$` escaping

**Symptom**: Multi-instruction asm body with immediate values fails
silently with `error: invalid operand in inline asm:` and produces
a 0-byte object file. The build pipeline reports success but link
fails with "undefined reference."

**Root cause**: LLVM inline-asm template uses `$0`, `$1`, etc. for
constraint references. A literal `$` immediate (e.g. `cmpb $0x0A, %dil`)
gets parsed as constraint #0 followed by `x0A` — invalid. Single-
instruction asms that don't use `$` work by accident.

**Fix pattern**: ESCAPE `$` as `$$` in the asm template string. So
`asm!("cmpb $$0x0A, %dil\n\tret", options(att_syntax))`. This is
a TEMPLATE-level concern, not a fj-lang bug — but fj-lang docs
should highlight it because the failure mode (silent 0-byte .o)
is hostile.

**Gaps that hit this**: discovered while debugging G-N; documented
as a separate "silent codegen failure pattern" in v33.1.1.

### Pattern 5 — Build artifact caching masks bugs

**Symptom**: Code "works" until you `rm -rf build/` and rebuild from
scratch. Then it fails immediately with the same code that just
passed e2e tests yesterday.

**Root cause**: fajaros-x86's ld-wrapper saves intermediate `.o`
files as `.o.saved` between fj build and final link. If a fj build
silently produces a 0-byte `.o` (Pattern 4) but exits 0, the
ld-wrapper still copies that 0-byte file as `.o.saved`. Subsequent
builds fail to regenerate `.o` (because the 0-byte one exists from
"prior success"), and the build pipeline picks up STALE working
`.o.saved` from before the bug was introduced. Result: code that's
"silently broken since last week" appears to work until clean
rebuild.

**Fix pattern**: TWO defenses:
1. fj-lang should treat LLVM `error:` diagnostics as build failures
   (currently treated as warnings). At minimum, refuse to write a
   0-byte `.o`.
2. fajaros-x86's pre-commit/pre-push hooks should run with
   `rm -rf build/` periodically (e.g. nightly CI) to surface
   silent codegen regressions early.

**Gaps that hit this**: G-N's debugging cycle, where I thought
Phase 6.6 console_putchar was working when it had been silently
0-byte-generating for a session.

## What FAJAROS_100PCT taught the language design

### What worked already

- **`@kernel` / `@device` / `@safe` / `@unsafe` context isolation** —
  the type-system-level primary annotation that prevents heap from
  leaking into kernel code, tensor ops from leaking into device,
  etc. This was load-bearing for migrating C functions safely.
- **`@noinline` modifier** existed before this work for V31's
  silent-build-failure prevention; the same modifier-flag mechanism
  was directly reusable for `@naked` / `@no_mangle` / `@no_vectorize`.
- **Cranelift JIT path** as a sanity check — 95% of fj test code
  paths go through Cranelift in dev, only kernel/embedded targets
  go through LLVM. This kept the LLVM backend's bug surface bounded
  and discoverable.

### What needed work

- The 9 gaps above. None were show-stoppers individually; collectively
  they were the difference between "fj can write OS-like code" and
  "fj kernel runs through 6/6 LLM E2E gates with vecmat_v8.c deleted."
- Better LLVM diagnostic-handling — too many warnings became fatal
  silently or vice versa.
- Build pipeline that fails LOUD when intermediate object is empty.

### What the 9-gap pattern says about kernel-targeting languages

Every kernel-targeting systems language eventually faces these issues.
They're **hidden in plain sight** — you don't notice them until you
try to delete the last `.c` file, at which point each one shows up
as a different runtime fault. A language that wants to credibly
claim "100% kernel coverage" needs:

1. Atomic primitives that don't go through libc-style runtime
   (G-A — bare-metal can't link `__atomic_*`)
2. `@naked` for ISR entry stubs (G-B + G-N)
3. Symbol-name control for `extern "C"` interop (G-C)
4. Raw asm escape hatch (G-G + G-H + G-I)
5. Modifier annotations that compose with primaries (G-K)
6. Per-target ABI quirks honored (G-M red zone)

Plus the meta-lesson: every "I thought that just worked" claim
needs a clean rebuild check.

## References

- Per-phase findings: `docs/FAJAROS_100PCT_FJ_PHASE_{0..7,4D,6_6}_FINDINGS.md`
- Plan: `docs/FAJAROS_100PCT_FJ_PLAN.md` (terminal-complete)
- Closure commits in fj-lang: 1b694406 (G-A), 29bfcdba (G-B), 1cf7dc05 (G-C), 85e8b3aa (G-K), df865161 (G-N), 25deb883 (dialect), 211cb8d1 (G-M)
- Closure commits in fajaros-x86: 457e0d0..541db09 (Phase 1 → 4.G + vecmat_v8.c deletion)
- CHANGELOG.md v33.0.0 → v33.2.0 entries for full per-version detail

---

*COMPLETED 2026-05-05. fj-lang v33.2.0 ships with 9/9 compiler gaps
closed; fajaros-x86 kernel build path is ZERO non-fj LOC. Plan goal
"100% Fajar Lang OS" achieved.*
