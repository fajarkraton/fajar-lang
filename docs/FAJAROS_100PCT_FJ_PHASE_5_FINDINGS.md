---
phase: 5 — LLVM backend native atomics (Gap G-A closure)
status: CLOSED 2026-05-04
budget: 2-3d planned + 30% surprise = 2.6-3.9d cap
actual: ~1h Claude time
variance: -95%
artifacts:
  - This findings doc
  - fajar-lang commit pending — atomic builtins + 3 regression tests
  - fajaros-x86 commit pending — spinlock V0.5.1 → V0.5.2 (uses atomics)
prereq: Phase 4.B closed (fj-lang bb6b7d2b, fajaros-x86 9820285)
---

# Phase 5 Findings — LLVM atomics (Gap G-A closure)

> Phase 5 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Closes Gap G-A:
> "LLVM backend has no native atomics" (Cranelift had runtime-call-based
> atomics; LLVM had nothing). With native atomic instruction emission,
> fajaros's spinlock can use a high-level fj-lang primitive instead of
> raw inline asm — same x86 LOCK CMPXCHG instruction, cleaner source.

## 5.1 — Cranelift audit (informational)

`src/codegen/cranelift/compile/{call.rs:2330+, method.rs:335+}` has
`Atomic` / `AtomicI32` / `AtomicI64` / `AtomicBool` types that lower to
runtime function calls (`__atomic_load`, `__atomic_store`,
`__atomic_load_relaxed`, etc.). Works for JIT host targets where the
runtime is linked, but NOT for bare-metal kernels.

The runtime-call approach was rejected for LLVM backend because:
- Bare-metal targets have no `__atomic_*` runtime
- LLVM has native atomic instruction emission via inkwell
- Native instructions are faster (no call overhead) and lower to
  `LOCK CMPXCHG` / `LOCK XADD` directly

## 5.2 — LLVM atomic emission added

Inkwell 0.8.0 API used:
- `Builder::build_cmpxchg(ptr, cmp, new, success_order, failure_order)`
- `Builder::build_atomicrmw(op, ptr, value, ordering)`
- `InstructionValue::set_atomic_ordering(ordering)` for atomic
  load/store on regular `build_load`/`build_store`

All ops use `AtomicOrdering::SequentiallyConsistent` — strongest
available, matches x86 LOCK prefix natural semantics, simpler mental
model than Acquire/Release split. Future enhancement could parameterize
ordering; for now SeqCst-only is honest.

### 4 new fj-lang builtins

```rust
atomic_load_u64(addr: i64) -> i64           // LLVM: load with seq_cst
atomic_store_u64(addr: i64, val: i64)       // LLVM: store with seq_cst
atomic_cas_u64(addr, expected, new) -> i64  // LLVM: cmpxchg → returns prev
atomic_fetch_add_u64(addr, delta) -> i64    // LLVM: atomicrmw add → returns prev
```

Registered in:
- `src/codegen/llvm/mod.rs` (lines ~800 + ~1786) — codegen
- `src/analyzer/type_check/register.rs:333+` — type analyzer

### ELF lowering verified

Standalone test program:
```fajar
@unsafe fn main() -> i64 {
    let prev = atomic_cas_u64(0xDEAD0000, 0, 42)
    let inc = atomic_fetch_add_u64(0xDEAD0000, 1)
    prev + inc
}
```

`objdump -d`:
```
  113c:	f0 48 0f b1 11        lock cmpxchg %rdx,(%rcx)
  1164:	f0 48 0f c1 01        lock xadd %rax,(%rcx)
```

Both `LOCK CMPXCHG` and `LOCK XADD` x86 atomic instructions emitted.

### Regression tests (3 new)

`src/codegen/llvm/mod.rs::tests`:
- `atomic_cas_emits_cmpxchg_instruction`
- `atomic_fetch_add_emits_atomicrmw_add`
- `atomic_load_store_set_ordering` (greps for `seq_cst` in IR)

Total: 8,966 → 8,969 lib tests pass.

## 5.3 — fajaros spinlock V0.5.1 → V0.5.2

Replaces inline-asm CMPXCHG with high-level `atomic_cas_u64`:

**Before (V0.5.1):**
```fajar
@kernel fn spinlock_try_acquire(lock_addr: i64) -> i64 {
    asm("xor %eax, %eax\n\tlock cmpxchgq %rcx, (%rsi)",
        in("rcx") 1, in("rsi") lock_addr,
        out("rax") -> i64, clobber("memory"), volatile)
}

@kernel fn spinlock_release(lock_addr: i64) {
    asm("mfence\n\tmovq $$0, (%rdi)",
        in("rdi") lock_addr, clobber("memory"), volatile)
}
```

**After (V0.5.2):**
```fajar
@kernel fn spinlock_try_acquire(lock_addr: i64) -> i64 {
    atomic_cas_u64(lock_addr, 0, 1)
}

@kernel fn spinlock_release(lock_addr: i64) {
    atomic_store_u64(lock_addr, 0)
}
```

### ELF disasm equivalent (verified)

```
0000000000130540 <spinlock_try_acquire>:
  130540: b9 01 00 00 00       mov    $0x1,%ecx
  130545: 31 c0                xor    %eax,%eax
  130547: f0 48 0f b1 0f       lock cmpxchg %rcx,(%rdi)
  13054c: c3                   ret

0000000000130570 <spinlock_release>:
  130570: 31 c0                xor    %eax,%eax
  130572: 48 87 07             xchg   %rax,(%rdi)   ← SeqCst store lowered to XCHG
  130575: c3                   ret
```

XCHG has implicit LOCK prefix → atomic + full memory barrier. So the
new V0.5.2 release path is actually slightly tighter than V0.5.1's
explicit MFENCE+MOV (one instruction vs two).

## 5.4 — Verification

| Gate | Result |
|---|---|
| `cargo test --features llvm,native --lib` | 8,969 / 8,969 PASS (incl. 3 new atomic tests) |
| `make build-llvm` (fajaros) | ELF 1,505,118 bytes (-96 vs V0.5.1; LLVM optimizes) |
| `make test-spinlock-smp-regression` | ✅ PASS in 25s |
| `make test-security-triple-regression` | ✅ 6/6 invariants PASS in 25s |
| `make test-gemma3-e2e` (~210s) | 4/5 PASS — boot + load all green; "64 tokens generated" gate timing-margin (test got 4 tokens within sleep 140 budget; not a Phase 5 functional regression — verified by re-running prior commits showing similar variance) |

## 5.5 — Other inline-asm sync primitives audit

Quick `grep -rn "asm!.*lock\|atomic\|cmpxchg" kernel/` showed only the
spinlock used inline-asm for atomic ops. Other lock-using sites
(`spinlock_acquire/release` callers like sched/signals.fj) call into
the fj wrappers, so they automatically benefit from V0.5.2.

No other migrations needed in Phase 5.

## 5.6 — Gap status

| Gap | Status |
|---|---|
| **G-A** LLVM backend native atomics | ✅ **CLOSED Phase 5** |
| G-G LLVM global_asm! emission | ✅ CLOSED Phase 2.A |
| G-H r#"..."# raw strings | ✅ CLOSED Phase 2.A.2 |
| G-I parser raw strings in asm templates | ✅ CLOSED Phase 2.A.2 |
| G-F SE009 false-positive on asm operand uses | ⏳ defer Phase 6 |
| G-J LLVM MC stricter than GAS | ⏳ documented |
| G-K @no_vectorize + @kernel parser mutex | ⏳ defer Phase 6 |
| G-L EXC:14 in inlined fj fns w/ byte+u32 reads in tight loops | ⏳ defer Phase 4.C-F debug |

## 5.7 — Effort summary + plan progress

**Phase 5 effort:** ~1h Claude time (vs 2-3d planned). Variance: -95%.

**Why so under:** Phase 2.A's prior infrastructure (LLVMSetModuleInlineAsm,
parser raw strings) wasn't directly applicable, but the inkwell API for
atomics is straightforward — `build_cmpxchg`, `build_atomicrmw`,
`set_atomic_ordering` are clean primitives. No surprises.

```
Phase 0 baseline:  3 files, 2,195 LOC (non-fj kernel build path)
After Phase 2:     2 files, 1,680 LOC
After Phase 3:     1 file,    768 LOC
After Phase 4.A:   1 file,    728 LOC
After Phase 4.B:   1 file,    642 LOC
After Phase 5:     1 file,    642 LOC ← here (G-A closed; vecmat_v8.c untouched)

Compiler gaps closed: 4 of 8 surfaced (G-A, G-G, G-H, G-I)
Compiler gaps documented: 4 of 8 surfaced (G-F, G-J, G-K, G-L)
Phases CLOSED: 5 of 9 (Phase 0, 1, 2, 3, 4.A, 4.B, 5)
```

## Decision gate (§6.8 R6)

This file committed → Phase 6 (`@naked` attribute) and Phase 7
(`@no_mangle` attribute) UNBLOCKED. Phase 4.C-F (vecmat_v8.c remainder)
still pending dedicated debug session for G-L root cause.

---

*FAJAROS_100PCT_FJ_PHASE_5_FINDINGS — 2026-05-04. Phase 5 CLOSED in
~1h vs 2-3d plan (-95%). G-A closure verified by ELF disasm — fj-lang
LLVM backend now emits native LOCK CMPXCHG / LOCK XADD instructions.
fajaros spinlock V0.5.2 uses high-level atomic_cas_u64 builtin instead
of raw inline asm. 5/9 phases CLOSED, 4/8 compiler gaps closed.*
