---
phase: 0 — Pre-flight audit (mandatory per §6.8 R1)
status: CLOSED 2026-05-04
budget: 0.5-1d planned + 25% surprise = 0.6-1.3d cap
actual: ~30 min Claude time (≈ 0.06d)
variance: -90% (Phase 0 was almost pure measurement; no surprises in 0.1, 0.2, 0.4, 0.5; 0.3 surfaced one downstream-affecting finding)
artifacts: docs/FAJAROS_100PCT_FJ_PHASE_0_FINDINGS.md (this file)
---

# Phase 0 Findings — Pre-flight Audit

> Phase 0 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. Mandatory per §6.8 R1.
> Findings below are evidence-based: each task has runnable command,
> output captured, and conclusion stated.

## 0.1 — Inventory non-fj files in fajaros-x86 kernel build path ✅

**Command:**
```bash
cd ~/Documents/fajaros-x86 && find . -type f \( -name "*.S" -o -name "*.c" -o -name "*.cpp" -o -name "*.asm" -o -name "*.nasm" -o -name "*.S.in" \) -not -path "*/target/*" -not -path "*/.git/*" -not -path "*/build/*"
```

**Result (3 files, 2,195 LOC):**
- `./boot/startup.S` — 515 LOC (boot trampoline + 32→64 transition + GDT + serial + entry)
- `./boot/runtime_stubs.S` — 912 LOC (15 asm symbols: VGA console, str ops, buffer LE/BE 10×, IDT/TSS/PIT init)
- `./kernel/compute/vecmat_v8.c` — 768 LOC (LLVM O2 vecmat miscompile bypass)

**Cross-check vs Makefile:**
```makefile
STARTUP_S := boot/startup.S
RUNTIME_S := boot/runtime_stubs.S
VECMAT_C  := kernel/compute/vecmat_v8.c
TL2_SRC   := ../fajarquant/cpu_kernels/bitnet_tl2/wrapper.cpp   # out of scope
```

**Conclusion:** Plan inventory CONFIRMED. No surprise files. Plan §2 numbers accurate.

## 0.2 — Inline asm operand support depth ✅

**Command:** read `src/parser/expr.rs::parse_inline_asm` body + grep real usage in fajaros.

**fajar-lang supports:**
- Template string (first arg)
- Comma-separated operands
- `options(...)` accepts: `nomem`, `nostack`, `readonly`, `preserves_flags`, `pure`, `att_syntax`, `volatile`
- `clobber_abi("...")` and `clobber("...")` — register clobbers
- Bare `volatile` keyword (legacy/shorthand)

**Real usage in fajaros-x86 covers:**
- `kernel/hw/msr.fj`: RDMSR/WRMSR with `in("ecx") val`, `out("rax") -> i64`, `clobber("rdx")`, `volatile`
- `kernel/hw/cpuid.fj`: CPUID with leaf 0x00000001 / 0x00000007 / 0x80000001
- `kernel/mm/frames.fj`: BSF (bit-scan-forward) + POPCNTQ (population count)

**Operand classes NOT visibly used (and not visibly parsed):**
- `inout` (read-write) — likely not supported
- `lateout` — likely not supported
- `sym` — likely not supported
- `const` — likely not supported

**Conclusion:** Inline asm support is **sufficient for Phase 3 (port runtime_stubs.S)**. The 15 stubs use straightforward in/out/clobber/volatile operands. No need to extend `parse_inline_asm` for this plan.

## 0.3 — Verify auto-generated startup vs fajaros's `boot/startup.S` ⚠️ AFFECTS PHASE 2

**Command:** read `src/codegen/linker.rs:1418` `generate_x86_64_startup()` body + diff against `~/Documents/fajaros-x86/boot/startup.S`.

**Auto-gen contains:**
1. Multiboot2 header (magic 0xE85250D6, address tag, **entry tag — NO framebuffer tag**)
2. 32-bit trampoline encoded as raw `.byte` sequences (cli, esp=0x200000, save mb2 ptr in edi)
3. Early serial debug `[BOOT32]\n` to COM1 0x3F8
4. Page tables zeroed at 0x70000 (3 × 4KB)
5. Identity map first 128MB (64 × 2MB huge pages, present+write+user+huge)
6. PML4 → CR3, enable PAE, set EFER.LME, enable CR0.PG → long mode active
7. GDT (null + kcode + kdata + udata + ucode + tss × 2)
8. Far jump to `_start64` (64-bit code)
9. Reload data segments to 0x10
10. Stack at 0x7F00000 (near top of 128MB region)
11. Save MB2 info ptr to R12 + memory location 0x6FF00
12. COM1 serial init (16550 UART, 115200 baud, 8N1, FIFO enable, MCR RTS+DTR+OUT2)
13. Call user-supplied entry function
14. CLI/HLT halt loop on return

**fajaros `boot/startup.S` differs in:**
- **MB2 framebuffer tag** (type=5, 1024×768×32) — fajaros REQUIRES this for VGA-graphics mode; auto-gen DOESN'T provide
- **Page table addresses** — fajaros uses 0x800000, auto-gen uses 0x70000
- **Identity map size** — fajaros likely maps differently (need second pass to verify exact size)
- **Stack/BSS layout** — likely fajaros-specific, ties into linker.ld constants
- **CPUID feature detection** — auto-gen lacks; fajaros startup.S enumerates (per file header line 5: "CPUID feature detection")

**Implication for Phase 2:**

The original Phase 2 plan was "use auto-gen, delete startup.S." This is **NOT viable as-is** — auto-gen would lose framebuffer tag (breaks VGA), page table layout would shift (breaks any kernel code that depends on specific physical address ranges).

**Phase 2 plan ADJUSTMENT (2 options, pick at Phase 2 start):**

- **Option 2A (low-risk, recommended):** Port `boot/startup.S` 1:1 into `kernel/boot/startup_x86_64.fj` as a `global_asm!()` block. Mechanically equivalent (same bytes assembled). Effort: ~0.5d. Result: still 100% fj source (asm goes through `global_asm!()` inside `.fj`), and ALL current behavior preserved.
- **Option 2B (deeper, slower):** Enhance `generate_x86_64_startup()` to accept config: framebuffer-tag toggle, page table base config, identity map size, etc. Then make fajaros use config-driven auto-gen. Effort: ~2-3d. Result: genuine reuse but more compiler-side work.

Recommendation: **Option 2A for this plan** (faster path to "100% fj" goal); **defer 2B as a separate fj-lang enhancement** (ergonomic improvement, not capability gap).

## 0.4 — Spinlock C-1 race verification ✅ (URGENT FIX VALIDATED)

**Command:** read `~/Documents/fajaros-x86/kernel/sched/spinlock.fj`.

**Implementation (lines 9-17):**
```fajar
@kernel fn spinlock_acquire(lock_addr: i64) {
    while volatile_read(lock_addr) != 0 {}     // step 1: read 0
    volatile_write(lock_addr, 1)               // step 2: write 1
}
```

**The race:**
| Time | CPU A | CPU B |
|---|---|---|
| t=0 | reads 0 (lock free) | |
| t=1 | (about to write 1) | reads 0 (lock free) |
| t=2 | writes 1 (thinks it owns) | writes 1 (thinks it owns) |

Both CPUs pass the `while` check, both write 1, both believe they hold the lock. **Classic TOCTOU between read-check and write.**

**No SMP regression test exists** (`grep -rln "test.*smp\|smp.*test" Makefile tests/` returns only `tests/benchmarks.fj` which is unrelated). The race is **silently latent** because fajaros boots single-CPU in default QEMU runs (no `-smp 2+`).

**Conclusion:** C-1 confirmed. Phase 1 fix (replace with inline-asm `LOCK CMPXCHG`) is URGENT and **independent of the 100% migration**. Should ship even if rest of plan slips.

## 0.5 — Cross-repo state check (§6.8 R8) ✅

**Command:**
```bash
for dir in fajar-lang fajaros-x86 fajarquant; do git status -sb && git rev-list --count origin/main..main; done
```

**Result:**
- fajar-lang: clean, 0 ahead (after CHANGELOG push)
- fajaros-x86: clean, 0 ahead
- fajarquant: clean, 0 ahead (1 untracked `logs/` dir — cosmetic, gitignored material)

**Conclusion:** All 3 repos in sync with origin. Safe to start commits.

## Phase 0 summary

| Task | Status | Surfaced |
|---|---|---|
| 0.1 Inventory | ✅ CLOSED | Plan §2 numbers confirmed; no surprise files |
| 0.2 Inline asm depth | ✅ CLOSED | Sufficient for Phase 3; no compiler change needed |
| 0.3 Auto-gen startup vs boot/startup.S | ⚠️ ADJUSTS PHASE 2 | Auto-gen is not 1:1 (framebuffer tag, page tables, layout differ); Phase 2 should use Option 2A (global_asm! port) not naive auto-gen swap |
| 0.4 Spinlock C-1 race | ✅ CONFIRMED | TOCTOU race in `while volatile_read != 0; volatile_write 1` — Phase 1 fix is URGENT |
| 0.5 Cross-repo state | ✅ CLEAN | All 3 repos 0 ahead of origin |

**Phase 0 effort:** ~30 min Claude time (vs 0.6-1.3d planned). Variance: -90% — Phase 0 was almost pure measurement against existing code, no debugging needed.

## Phase 2 plan adjustment

Update `docs/FAJAROS_100PCT_FJ_PLAN.md` Phase 2 description:

> ~~Phase 2 — Replace `boot/startup.S` with auto-generated startup~~

→

> Phase 2 — Port `boot/startup.S` to `kernel/boot/startup_x86_64.fj` as `global_asm!()` block (Option 2A from Phase 0.3 finding); auto-gen enhancement (Option 2B) deferred to separate plan.

This change preserves all fajaros boot behavior (framebuffer tag, page table layout, CPUID feature detection) and still satisfies "no `.S` files in kernel build path." Effort: ~0.5-1d (down from 1-1.5d original).

## Decision gate (§6.8 R6)

This file committed → satisfies pre-commit gate for Phase 1+ work. Phase 1 (URGENT spinlock fix) UNBLOCKED.

## Prevention layer

`scripts/audit_fajaros_non_fj.sh` will be added at Phase 1 start (combined with Phase 1 prevention layer to avoid 2 small commits). Acts as the "non-fj LOC count strictly decreasing" mechanical gate per phase.

---

*FAJAROS_100PCT_FJ_PHASE_0_FINDINGS — 2026-05-04. Closes Phase 0; unblocks
Phase 1+. Surprise budget remaining: full +25% available since Phase 0 ran
~10× under plan.*
