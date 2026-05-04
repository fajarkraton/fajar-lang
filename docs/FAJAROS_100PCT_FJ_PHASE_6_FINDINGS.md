---
phase: 6 — @naked modifier attribute (Gap G-B closure, partial)
status: CLOSED-PARTIAL 2026-05-04 (compiler side; deployment side deferred)
budget: 3-5d planned + 25% surprise = 3.75-6.25d cap
actual: ~50min Claude time (~30min WIP fc4689da + ~20min E2E this session)
variance: -98%
artifacts:
  - This findings doc
  - fajar-lang fc4689da — lexer + parser + AST + LLVM codegen (WIP)
  - fajar-lang follow-up commit — 2 regression tests + this doc
prereq: Phase 5 closed (fajar-lang 1b694406)
---

# Phase 6 Findings — `@naked` modifier (Gap G-B partial closure)

> Phase 6 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Closes Gap G-B compiler-side:
> "no `@naked` attribute". With `@naked`, a function declared in a kernel/
> unsafe context whose body is an `asm!()` block can opt out of compiler-
> inserted prologue/epilogue, mirroring Rust's `#[unsafe(naked)]`. The
> deployment-side migration of `hw_init.fj` IDT/TSS/PIT stubs from
> `global_asm!()` to `@naked fn` is deferred.

## 6.1 — What landed

### Lexer (commit `fc4689da`)
- `AtNaked` token added to `src/lexer/token.rs::Token` + Display impl.
- `"naked"` registered in the `lookup_annotation` keyword map.

### Parser + AST (commit `fc4689da`)
- `@naked` accepted as a MODIFIER. Stacks with `@kernel`/`@unsafe` primary
  in the same way `@noinline` does. Order-independent.
- `FnDef.naked: bool` field bumped onto AST. 11 construction sites updated
  to default `naked: false` (codegen tests, fixture builders, etc.).

### LLVM codegen (commit `fc4689da`, lines `src/codegen/llvm/mod.rs:3457+`)
- When `fndef.naked == true`, the function gets the LLVM `naked` enum
  attribute via `Attribute::get_named_enum_kind_id("naked")` +
  `function.add_attribute(AttributeLoc::Function, ...)`.
- Mirrors the existing `@interrupt` path (line 3423) that already uses
  the same primitive — so the inkwell API surface is well-trodden.

### Regression tests (this session, 2 new)
- `at_naked_emits_naked_attribute` — IR contains `naked` for `@naked` fn.
- `regular_fn_does_not_receive_naked_attribute` — defensive: regular fn
  must NOT have `naked` on its `define` line. Prevents leakage.

Total: 8,969 → **8,971 lib tests pass** (under `--features llvm,native`).

## 6.2 — End-to-end verification

### Test program (`/tmp/test_naked.fj`)

```fajar
@naked @unsafe fn naked_fn() {
    asm!("xor %eax, %eax\n\tret", options(att_syntax))
}

fn main() {
    println("naked test program")
}
```

Built with `fj build --backend llvm /tmp/test_naked.fj -o /tmp/test_naked`.

### `objdump -d` of `naked_fn` (with @naked)

```
00000000000011e0 <naked_fn>:
    11e0:  31 c0           xor    %eax,%eax
    11e2:  c3              ret
```

### `objdump -d` of `not_naked_fn` (same body, no @naked)

```
00000000000011e0 <not_naked_fn>:
    11e0:  50              push   %rax       ← prologue
    11e1:  31 c0           xor    %eax,%eax
    11e3:  c3              ret
    11e4:  58              pop    %rax       ← epilogue
    11e5:  c3              ret
```

5 bytes saved per call site (1×`push`, 1×`pop`, 1×fallthrough `ret`).
More importantly: when the asm body needs **bit-exact** stack layout for
ISR/handler boilerplate (e.g. an IRQ stub that has to leave RSP/registers
in a specific shape for `iretq`), the absence of compiler scratch is
correctness-critical, not just a perf knob.

## 6.3 — Why @naked + @unsafe (not @naked + @kernel)

The WIP test program uses `@naked @unsafe` rather than `@naked @kernel`.
Both contexts permit `asm!()` per CLAUDE.md §5.3. `@kernel` would also
work but it forbids heap + tensor — not relevant for naked stubs which
are pure asm. The codegen path doesn't care which primary stacked.

Phase 6.6 (deployed) will use `@kernel @naked` everywhere in
`hw_init.fj` because the surrounding module already standardizes on
`@kernel`. No behavior split.

## 6.4 — Deferred from Phase 6 plan

The Phase 6 task table in `docs/FAJAROS_100PCT_FJ_PLAN.md` listed 8
sub-tasks. This closure covers **3 of 8** (lexer, parser, LLVM codegen
+ tests). Rest deferred:

| Sub-task | Status | Why deferred |
|---|---|---|
| 6.1 lexer + 6.2 parser + 6.4 LLVM codegen | ✅ CLOSED Phase 6 | — |
| 6.3 analyzer KE006 (strict: only `@kernel`/`@unsafe`, body must be single `asm!()`) | ⏳ DEFERRED | Current acceptance is permissive; analyzer hardening is polish, not blocker. WIP works because the lexer/parser stack is order-agnostic. |
| 6.5 Cranelift parity OR explicit CE-XX rejection | ⏳ DEFERRED | LLVM is the production backend for fajaros (`make build-llvm`). Cranelift is JIT-host, not bare-metal. Explicit error preferred over silent ignore — but not blocker for FAJAROS_100PCT_FJ migration. |
| 6.6 Migrate `kernel/runtime/hw_init.fj` IDT/TSS/PIT stubs from `global_asm!()` → `@naked fn` | ⏳ DEFERRED | Mechanical migration; ~4-6h work; no compiler dependency remaining. Best done as part of Phase 4.D-F debug session (G-L) so all hw_init churn lands together. |
| 6.7 docs (CLAUDE.md §5.3 + SECURITY.md caveat) | ⏳ DEFERRED | Will land when 6.6 deploys. |
| 6.8 findings doc | ✅ this file | — |

The compiler attribute mechanism is the load-bearing piece. Migration is
mechanical follow-up that doesn't introduce new compiler risk.

## 6.5 — Verification

| Gate | Result |
|---|---|
| `cargo build --release --features llvm,native` | ✅ clean, no warnings |
| `cargo test --features llvm,native --lib at_naked` | ✅ 1/1 PASS |
| `cargo test --features llvm,native --lib regular_fn_does_not_receive_naked` | ✅ 1/1 PASS |
| `cargo test --features llvm,native --lib` (full) | ✅ 8,971 / 8,971 PASS (1 ignored) |
| `fj build --backend llvm /tmp/test_naked.fj` | ✅ ELF emitted |
| `objdump -d /tmp/test_naked` (naked_fn no prologue) | ✅ 2 bytes asm body, no `push`/`pop` |
| `objdump -d /tmp/test_not_naked` (regression — non-@naked still has prologue) | ✅ `push %rax` + `pop %rax` present |

## 6.6 — Gap status

| Gap | Status |
|---|---|
| **G-B** `@naked` attribute (compiler side) | ✅ **CLOSED Phase 6** |
| **G-B** `@naked` attribute (analyzer KE006 strict + Cranelift parity + hw_init migration) | ⏳ DEFERRED — see §6.4 |
| G-A LLVM backend native atomics | ✅ CLOSED Phase 5 |
| G-G LLVM global_asm! emission | ✅ CLOSED Phase 2.A |
| G-H r#"..."# raw strings | ✅ CLOSED Phase 2.A.2 |
| G-I parser raw strings in asm templates | ✅ CLOSED Phase 2.A.2 |
| G-F SE009 false-positive on asm operand uses | ⏳ defer Phase 7+ |
| G-J LLVM MC stricter than GAS | ⏳ documented |
| G-K @no_vectorize + @kernel parser mutex | ⏳ defer Phase 7+ |
| G-L EXC:14 in inlined fj fns w/ byte+u32 reads in tight loops | ⏳ defer Phase 4.C-F debug |

## 6.7 — Effort summary + plan progress

**Phase 6 effort:** ~50min Claude time (vs 3-5d planned). Variance: **-98%**.

Breakdown:
- Lexer + parser + AST + LLVM codegen + AST construction site bumps: ~30min (commit `fc4689da`)
- E2E objdump verification + 2 regression tests + this findings doc: ~20min (this session)

**Why so under:** the @interrupt path at `mod.rs:3423` already used the
exact inkwell primitive (`get_named_enum_kind_id("naked")` +
`add_attribute`). The Phase 6 WIP just exposed that primitive to user-
facing `@naked`. The 11 FnDef construction sites were the real surface-area
work — but mechanical and clippy-checked.

```
Phase 0 baseline:  3 files, 2,195 LOC (non-fj kernel build path)
After Phase 2:     2 files, 1,680 LOC
After Phase 3:     1 file,    768 LOC
After Phase 4.A:   1 file,    728 LOC
After Phase 4.B:   1 file,    642 LOC
After Phase 5:     1 file,    642 LOC (G-A closed; vecmat_v8.c untouched)
After Phase 6:     1 file,    642 LOC ← here (G-B compiler closed; migration deferred)

Compiler gaps closed: 5 of 8 surfaced (G-A, G-B compiler, G-G, G-H, G-I)
Compiler gaps documented: 4 of 8 surfaced (G-F, G-J, G-K, G-L)
Phases CLOSED:     5 of 9 (Phase 0, 1, 2, 3, 4.A, 4.B, 5) + 1 PARTIAL (6 compiler)
```

## Decision gate (§6.8 R6)

This file committed → Phase 7 (`@no_mangle` attribute) UNBLOCKED.
Phase 6.6 (`hw_init.fj` migration) groups naturally with Phase 4.C-F
(vecmat_v8.c remainder + G-L debug session) since both touch hw_init/
runtime layer; can land as a single follow-up commit when that debug
session opens.

---

*FAJAROS_100PCT_FJ_PHASE_6_FINDINGS — 2026-05-04. Phase 6 CLOSED-PARTIAL
in ~50min vs 3-5d plan (-98%). G-B compiler closure verified by ELF
disasm — fj-lang LLVM backend now honors `@naked` and emits user asm
without compiler prologue/epilogue. 2 regression tests added (8969 →
8971 lib tests). hw_init.fj migration + Cranelift parity + analyzer
KE006 strictness deferred for grouped landing. 5/9 phases CLOSED + 1
PARTIAL, 5/8 compiler gaps closed.*
