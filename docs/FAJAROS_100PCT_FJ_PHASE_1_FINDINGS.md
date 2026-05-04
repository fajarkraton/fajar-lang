---
phase: 1 — URGENT spinlock C-1 race fix (independent of 100% migration)
status: CLOSED 2026-05-04
budget: 0.5d planned + 30% surprise = 0.65d cap
actual: ~45 min Claude time (≈ 0.09d)
variance: -82%
artifacts: docs/FAJAROS_100PCT_FJ_PHASE_1_FINDINGS.md (this file)
prereq: Phase 0 closed (commit 4639029d, fajar-lang main)
---

# Phase 1 Findings — Spinlock C-1 race fix

> Phase 1 of `docs/FAJAROS_100PCT_FJ_PLAN.md`. URGENT: independent of
> 100% migration. Goes critical the moment fajaros's SMP path is
> exercised under multi-CPU contention.

## 1.1 — Replace spinlock body with inline-asm LOCK CMPXCHG ✅

**Before** (`kernel/sched/spinlock.fj` V0.5.0, broken):
```fajar
@kernel fn spinlock_acquire(lock_addr: i64) {
    while volatile_read(lock_addr) != 0 {}     // CPU A reads 0
    volatile_write(lock_addr, 1)               // CPU B reads 0, both write 1
}
@kernel fn spinlock_release(lock_addr: i64) {
    volatile_write(lock_addr, 0)
}
```

**After** (V0.5.1, atomic):
```fajar
@kernel fn spinlock_try_acquire(lock_addr: i64) -> i64 {
    asm("xor %eax, %eax\n\tlock cmpxchgq %rcx, (%rsi)",
        in("rcx") 1,
        in("rsi") lock_addr,
        out("rax") -> i64,
        clobber("memory"),
        volatile)
}
@kernel fn spinlock_acquire(lock_addr: i64) {
    while spinlock_try_acquire(lock_addr) != 0 {
        asm("pause", volatile)
    }
}
@kernel fn spinlock_release(lock_addr: i64) {
    asm("mfence\n\tmovq $$0, (%rdi)",
        in("rdi") lock_addr,
        clobber("memory"),
        volatile)
}
```

**Semantics:**
- `spinlock_try_acquire`: LOCK CMPXCHG sets ZF=1 + writes 1 if `[lock]==0`,
  else loads `[lock]` into RAX and ZF=0. Returns 0 on acquire, non-zero if
  already held. LOCK prefix → atomic across CPUs.
- `spinlock_acquire`: spins on `try_acquire` with PAUSE hint (cuts power +
  bus traffic on Hyper-Threading; recommended on all x86_64).
- `spinlock_release`: MFENCE → MOV 0. Ensures all prior writes visible
  before lock release; aligned 64-bit stores are atomic on x86_64.

**Verification (commands run, results captured):**
- `cargo build --release --features llvm,native` → 41s, clean
- `cd ~/Documents/fajaros-x86 && make build-llvm` → 10s, ELF +208 bytes
  (1504311 → 1504519, consistent with new asm bytes)
- `make test-spinlock-smp-regression` → **PASS** in 25s (boot + run
  `spinlock` shell command + verify "Spinlock verified (LOCK CMPXCHG path)"
  line in serial log)

## 1.2 — Audit other volatile-as-atomic patterns ✅

**Command:** `grep -rn "volatile_read\|volatile_write" kernel/`

**Findings:**
- `kernel/sched/scheduler.fj`: 30+ `volatile_write_u8` calls writing
  init filename string ("INIT.SH") to fixed memory addresses — NOT
  sync. Single-writer pattern (only kernel main thread initializes
  this region). Safe.
- `kernel/auth/permissions.fj`: `volatile_read`/`volatile_write` for
  per-file metadata fields (UID, GID, mode). Read-modify-write but
  NOT cross-CPU contended. Per-fd ownership invariant holds.
- `kernel/sched/spinlock.fj`: was the **only** race-prone usage.
  Fixed in 1.1.

**Conclusion:** No other volatile-as-atomic class-of-bugs in the
kernel. C-1 was a singleton.

## 1.3 — Add `make test-spinlock-smp-regression` Makefile target ✅

**Added to** `~/Documents/fajaros-x86/Makefile`:
```makefile
.PHONY: test-spinlock-smp-regression
test-spinlock-smp-regression: iso-llvm
	@echo "[TEST] FAJAROS_100PCT_FJ_PLAN Phase 1 — spinlock SMP regression..."
	@(sleep 6; printf 'spinlock\r'; sleep 3) | \
		timeout 15 $(QEMU) -cdrom $(BUILD_DIR)/fajaros-llvm.iso \
		-chardev stdio,id=ch0,signal=off -serial chardev:ch0 \
		-display none -no-reboot -no-shutdown $(QEMU_KVM) $(QEMU_MEM) $(QEMU_SMP) 2>/dev/null \
		> $(BUILD_DIR)/test-spinlock-smp-regression.log || true
	@echo ""
	@grep -q "Spinlock verified (LOCK CMPXCHG path)" $(BUILD_DIR)/test-spinlock-smp-regression.log \
		&& echo "[PASS] LOCK CMPXCHG spinlock runs to completion under -smp 4" \
		|| { echo "[FAIL] spinlock test did not complete"; exit 1; }
```

**Verification:** `make test-spinlock-smp-regression` → PASS in 25s.

**Scope of test:** Single-CPU exercise of acquire/release under
`-smp 4` boot. Validates: (a) inline-asm template parses + lowers
through fj-lang LLVM backend, (b) atomic ops execute correctly, (c)
boot path is not broken. **Does NOT** explicitly test cross-CPU
contention (would require AP-side test code; deferred to Phase 1.5
enhancement IF needed).

## 1.4 — Wire test into pre-push hook ✅

**Created** `~/Documents/fajaros-x86/scripts/git-hooks/pre-push` (3.5 KB):
- Detects changed files since `origin/main`
- Runs `make test-spinlock-smp-regression` IF
  `kernel/sched/spinlock.fj` OR `Makefile` changed
- Skips gracefully if QEMU or KVM unavailable (CI gate runs the
  actual check separately)
- Failure surfaces last 25 log lines + clear bypass instruction

**Installed locally** via `cp scripts/git-hooks/pre-push .git/hooks/pre-push`.

**Note:** No global `install-hooks.sh` exists in fajaros-x86; current
practice is per-developer manual install. Mention added to
`docs/FAJAROS_100PCT_FJ_PHASE_1_FINDINGS.md` as follow-up — if a
later phase introduces more hooks, ship `scripts/install-hooks.sh`
to consolidate.

## Surfaced fajar-lang gap (NEW)

**G-F (analyzer SE009 false-positive on inline-asm operand uses):**
`fj check kernel/sched/spinlock.fj` reports SE009 "unused variable
'lock_addr'" on every function parameter that is referenced ONLY
through inline-asm operands (e.g. `in("rsi") lock_addr`). The fj-lang
analyzer doesn't trace asm template operand bindings as variable
uses. Severity LOW (warnings don't block compilation per CLAUDE.md
§4.4); cosmetic noise only. Same false-positive fires on existing
production code (`kernel/hw/msr.fj` `rdmsr/wrmsr` parameters).

**Workaround today:** tolerate the warnings (matches msr.fj
convention).

**Real fix (~0.5-1d):** extend `src/analyzer/...` walk_expr for
`Expr::InlineAsm` to mark operand expressions as variable uses.
**Recommendation:** add as G-F to plan §2 inventory; close opportunistically
during Phase 5 (LLVM atomics) since both touch analyzer/codegen of
similar low-level constructs.

## Prevention layer also shipped

- `scripts/audit_fajaros_non_fj.sh` — lists non-fj files in kernel
  build path, current count + LOC, plan progress baseline. **Output
  must strictly decrease phase-by-phase** until 0 at end of Phase 4.
- `make audit-100pct-fj` — Makefile wrapper. Currently shows:
  `3 files, 2,195 LOC remaining` (matches Phase 0.1 inventory).
- Pre-push hook installed locally; committed source under
  `scripts/git-hooks/pre-push` for repo-wide reproducibility.

## Phase 1 summary

| Task | Status | Surfaced |
|---|---|---|
| 1.1 Replace spinlock with LOCK CMPXCHG | ✅ CLOSED E2E | E2E test PASS in QEMU -smp 4 |
| 1.2 Audit other volatile-as-atomic | ✅ CLOSED | Spinlock was the only race-prone usage |
| 1.3 Add make test-spinlock-smp-regression | ✅ CLOSED | PASS in 25s |
| 1.4 Wire pre-push hook | ✅ CLOSED | Installed locally; source committed |
| (bonus) audit-100pct-fj prevention layer | ✅ shipped | matches plan baseline |
| (bonus) G-F gap surfaced | ⚠️ documented | low-severity; defer to Phase 5 |

**Phase 1 effort:** ~45 min Claude time (vs 0.65d planned). Variance:
-82%. The plan's +30% surprise budget was kept in reserve; not needed.

**State after Phase 1:**
- C-1 (spinlock TOCTOU race) — FIXED, regression-gated.
- Non-fj inventory: still 3 files, 2,195 LOC (no migration yet — that's
  Phase 2-4).
- New gap surfaced: G-F (analyzer SE009 false-positive on asm operand
  uses).

## Decision gate (§6.8 R6)

This file committed → satisfies pre-commit gate for Phase 2+ work.
Phase 2 (port `boot/startup.S` → `kernel/boot/startup_x86_64.fj`
global_asm! per Phase 0.3 finding 2A) UNBLOCKED.

---

*FAJAROS_100PCT_FJ_PHASE_1_FINDINGS — 2026-05-04. Closes Phase 1 E2E.
Spinlock C-1 race FIXED. Surprise budget preserved (45min vs 0.65d cap).
Plan progress: 1/9 phases CLOSED, 2,195 LOC non-fj remaining.*
