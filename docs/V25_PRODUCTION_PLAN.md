# V25 "Production" — Complete Roadmap to Full Production

> **Date:** 2026-04-07
> **Author:** Fajar (TaxPrime / PrimeCore.id)
> **Source:** V24 Deep Audit (3 parallel audits: Fajar Lang, FajarOS, FajarQuant)
> **Standard:** Every task has concrete verification. [x] only when E2E works.

---

## Phase A: Fajar Lang Fixes (Target: 2-3 days)

> **Goal:** Fix 1 real bug + wire framework modules → 95% production
> **Current:** ~90% production (V24 re-audit: most "bugs" were false alarms)
> **After Phase A:** 95% production

### Re-Audit Results (2026-04-07, hands-on verification)

The initial V24 audit (code-reading) reported 5 critical bugs. Hands-on testing
(running actual code) revealed **4 of 5 were false alarms:**

| Claimed Bug | Hands-On Test | Result |
|-------------|--------------|--------|
| A1: @kernel/@device BROKEN | `@kernel fn` + `zeros()` → KE002 fires ✅ | **FALSE ALARM** — 148 tests pass |
| A2: HashMap BROKEN | `map_insert(null, k, v)` was broken | **FIXED** in commit `30ef65b` |
| A3: JIT strings BROKEN | `fj run --native` + f-strings → correct output | **FALSE ALARM** — works perfectly |
| A4: AOT linking FAILS | `fj build` + run binary → fib(30)=832040 | **FALSE ALARM** — works perfectly |
| A5: LLVM 80+ errors | `cargo build --features llvm` → 0 errors | **FALSE ALARM** — compiles clean |
| A5b: LLVM println segfault | `fj run --llvm` + `println()` → SIGSEGV | **REAL BUG** — only actual issue |

**Lesson learned:** Never trust code-reading audits alone. Always run the code.

### A1: Fix LLVM println Segfault (P1, ~3 hours)

**Bug:** `fj run --llvm` segfaults when calling `println()`. Pure integer code works.
**Scope:** LLVM JIT runtime function linkage for string-producing builtins.
**File:** `src/codegen/llvm/mod.rs` — runtime function registration.

**Verified behavior:**
- `fn main() -> i64 { 100 + 200 }` → **300** ✅ (works)
- `fn main() { println("hello") }` → **SIGSEGV** ❌ (crashes)
- `fn add(a: i64, b: i64) -> i64 { a + b } fn main() -> i64 { add(20, 22) }` → **42** ✅

| # | Task | File | Verification |
|---|------|------|-------------|
| A1.1 | Debug segfault: run with `RUST_BACKTRACE=1` | llvm/mod.rs | Identify crash location |
| A1.2 | Check `fj_rt_println` linkage in LLVM JIT | llvm/mod.rs | Symbol resolved correctly |
| A1.3 | Fix runtime function pointer registration | llvm/mod.rs | println linked |
| A1.4 | Test `println("hello")` via LLVM | shell | Output: "hello" |
| A1.5 | Test f-strings + string ops via LLVM | shell | No segfault |

**Gate:** `fj run --llvm file.fj` with `println("hello")` → "hello" (no crash).

### A2: Wire Framework Modules to CLI (P2, ~12 hours)

> **NOTE:** Before implementing, each module must be verified by running code.
> The audit agents claimed 9 framework modules — verify each is actually unwired.

| # | Module | Verify First | CLI Command | Hours |
|---|--------|-------------|------------|-------|
| A2.1 | concurrency_v2 | Test `actor_spawn` from .fj | `fj actor-demo` | 1 |
| A2.2 | debugger_v2 | Test `fj debug --record` | `fj debug --record/--replay` | 2 |
| A2.3 | ml_advanced | Test `diffusion_create` from .fj | `fj diffusion-demo` | 1 |
| A2.4 | deployment | Test `fj deploy` actual output | `fj deploy` (real Docker gen) | 2 |
| A2.5 | jit | Test `fj run --jit` behavior | `fj run --tiered` | 2 |
| A2.6 | lsp_v3 | Test LSP semantic tokens | Wire to `lsp/server.rs` | 1 |
| A2.7 | playground | Test `fj playground` output | `fj playground` (HTML gen) | 1 |
| A2.8 | plugin | Test plugin dlopen | `fj plugin load` | 2 |
| A2.9 | wasi_p2 | Test WASI component model | `fj run --wasi file.fj` | 2 |

**IMPORTANT:** Verify each module is actually [f] before spending time wiring it.
Some may already be [x] (like the audit falsely claimed @kernel was broken).

**Gate:** Verified modules callable from CLI. `fj --help` lists new commands.

### Phase A Summary

| Metric | Before (V24) | After (V25) |
|--------|-------------|-------------|
| Real bugs | 1 (LLVM println) | 0 |
| HashMap | FIXED (`30ef65b`) | WORKING |
| @kernel/@device | WORKS (was false alarm) | WORKS |
| JIT strings | WORKS (was false alarm) | WORKS |
| AOT build | WORKS (was false alarm) | WORKS |
| LLVM compile | CLEAN (was false alarm) | CLEAN |
| `fj run --llvm` + println | SEGFAULT | FIX NEEDED |
| Framework modules wired | TBD (verify first) | Up to 9 more |

---

## Phase B: FajarOS Honest Fixes (Target: 1-2 weeks)

> **Goal:** Fix inflated claims, make core subsystems real → 65% production
> **Current:** 40% production (V24 audit: "advanced hobby OS")
> **After Phase B:** 65% production (usable research OS)

### B1: Fix NVMe Initialization (P0, ~8 hours)

**Bug:** NVMe init fails with error -1 on QEMU.
**Root Cause:** BAR0 MMIO mapping or admin queue setup issue.
**File:** `drivers/nvme.fj` in fajaros-x86 repo

| # | Task | Verification |
|---|------|-------------|
| B1.1 | Debug NVMe init with QEMU `-d guest_errors` | Identify exact failure point |
| B1.2 | Fix BAR0 mapping (check page_map for MMIO) | PCI BAR0 reads return valid data |
| B1.3 | Fix admin queue setup (doorbell writes) | Controller transitions to READY |
| B1.4 | Test NVMe read (sector 0) | `nova> disk-read 0` returns data |
| B1.5 | Test NVMe write + readback | Data persists across reads |

**Gate:** `nova> nvme-info` shows controller + `nova> disk-read 0` returns data.

### B2: Fix Documentation Claims (P0, ~2 hours)

| # | Task | Verification |
|---|------|-------------|
| B2.1 | Update README: NVMe → "framework (init fails on QEMU)" | Honest |
| B2.2 | Update CLAUDE.md: "90/90 commands" → "~60 functional, ~30 stubs" | Honest |
| B2.3 | Update README: "GUI framebuffer" → "initialized, never renders" | Honest |
| B2.4 | Separate "commands that execute" from "commands that work correctly" | Clear distinction |

### B3: Complete ELF Loader + exec() (P1, ~12 hours)

| # | Task | Verification |
|---|------|-------------|
| B3.1 | Parse ELF64 headers (PT_LOAD segments) | ELF header fields correct |
| B3.2 | Map ELF segments into user address space | Pages mapped with correct perms |
| B3.3 | Set up user stack + argc/argv | Stack layout correct |
| B3.4 | Jump to entry point via IRETQ | User program runs |
| B3.5 | `nova> exec /bin/hello` runs ring3_hello.elf | Output appears on console |

**Gate:** `nova> exec hello` runs embedded ELF and returns to shell.

### B4: Implement Filesystem Write (P1, ~8 hours)

| # | Task | Verification |
|---|------|-------------|
| B4.1 | Implement VFS write() syscall for RamFS | `nova> echo "data" > /tmp/test` |
| B4.2 | Implement VFS create (touch) | `nova> touch /tmp/new && ls /tmp` shows file |
| B4.3 | Implement VFS delete (rm) | `nova> rm /tmp/test && ls /tmp` file gone |
| B4.4 | FAT32 write (if NVMe works) | Persist to disk |

**Gate:** `nova> echo test > /tmp/file && cat /tmp/file` outputs "test".

### B5: Multi-Process Scheduler (P2, ~16 hours)

| # | Task | Verification |
|---|------|-------------|
| B5.1 | Implement timer-based preemptive scheduling | Two processes alternate |
| B5.2 | Context switch (save/restore registers) | No corruption |
| B5.3 | Process create from shell (fork+exec) | `nova> exec prog &` backgrounds |
| B5.4 | Waitpid/exit cleanup | Zombie processes reaped |
| B5.5 | `nova> ps` shows running processes | Process list correct |

### B6: Complete Networking TX (P2, ~20 hours)

| # | Task | Verification |
|---|------|-------------|
| B6.1 | VirtIO-Net driver: complete init + interrupt | Device ready |
| B6.2 | Ethernet frame TX | Frame appears on QEMU tap |
| B6.3 | ARP request/reply | `nova> ping` resolves MAC |
| B6.4 | IP + ICMP echo | `nova> ping 10.0.2.2` responds |
| B6.5 | TCP handshake (SYN/SYN-ACK/ACK) | Connection established |

### Phase B Summary

| Metric | Before | After |
|--------|--------|-------|
| NVMe | BROKEN (falls back to ramdisk) | WORKING (read/write) |
| Commands working correctly | ~60 | ~80 |
| User programs | hello.elf only | ELF loader + exec |
| Filesystem write | MISSING | RamFS write works |
| Multi-process | MISSING | Preemptive scheduler |
| Networking | MISSING | ARP + ICMP ping |
| Honest documentation | INFLATED | CORRECTED |

---

## Phase C: FajarQuant Paper Submission (Target: 3-5 weeks)

> **Goal:** Paper submission-ready for MLSys/NeurIPS
> **Current:** Pre-print only (all experiments synthetic)
> **After Phase C:** Conference submission with real data

### C1: Extract Real KV Cache Data (P0, ~3 days)

| # | Task | Verification |
|---|------|-------------|
| C1.1 | Set up HuggingFace transformers + Llama 2 7B | `python -c "from transformers import ..."` |
| C1.2 | Write KV cache extraction script | Saves K/V tensors per layer/head |
| C1.3 | Extract on 500 diverse prompts (OpenWebText) | `data/kv_cache_llama7b/` directory |
| C1.4 | Analyze variance/eigenvalue structure | Compare vs synthetic assumptions |
| C1.5 | Verify FajarQuant improvement on REAL data | MSE improvement measured |

**Gate:** Table 1 regenerated with real KV cache data (not synthetic).

### C2: Implement KIVI Baseline (P0, ~4 days)

| # | Task | Verification |
|---|------|-------------|
| C2.1 | Implement per-channel key quantization | Matches KIVI paper description |
| C2.2 | Implement per-token value quantization | Matches paper |
| C2.3 | Implement KIVI's residual coding | Full algorithm |
| C2.4 | Run on same real KV cache data | MSE/perplexity comparable |
| C2.5 | Generate comparison table: FajarQuant vs KIVI vs TurboQuant | Table for paper |

**Gate:** Fair 3-way comparison table on real data.

### C3: Perplexity Evaluation (P0, ~5 days)

| # | Task | Verification |
|---|------|-------------|
| C3.1 | Implement quantized KV cache inference loop | Model generates text with quantized cache |
| C3.2 | Measure perplexity on WikiText-2 | ppl_full vs ppl_quantized |
| C3.3 | Sweep bit-widths (1, 2, 3, 4) | Quality-compression tradeoff curve |
| C3.4 | Compare: FajarQuant vs KIVI vs full precision | Fair comparison |
| C3.5 | Test on Mistral 7B as second model | Generalization |

**Gate:** Perplexity table shows FajarQuant competitive with KIVI at same bit budget.

### C4: Ablation Studies (P1, ~4 days)

| # | Task | Verification |
|---|------|-------------|
| C4.1 | Adaptive rotation only (no fused, no hierarchical) | MSE improvement isolated |
| C4.2 | Fused attention only (no adaptive, no hierarchical) | Memory savings isolated |
| C4.3 | Hierarchical only (no adaptive, no fused) | Bit savings isolated |
| C4.4 | All three combined | Full system |
| C4.5 | Generate ablation table for paper | Each innovation's contribution clear |

### C5: Fix Paper Discrepancies (P1, ~1 day)

| # | Task | Verification |
|---|------|-------------|
| C5.1 | Fix 65.3% vs 48.7% inconsistency | N=4K and N=10K both reported correctly |
| C5.2 | Add confidence intervals to all results | Error bars in tables |
| C5.3 | Clarify "structured data" assumption | Explicit statement in methodology |
| C5.4 | Update all tables with real data numbers | No synthetic-only results |

### C6: Embedded Device Benchmarks (P2, ~2 weeks)

| # | Task | Verification |
|---|------|-------------|
| C6.1 | Cross-compile FajarQuant for ARM64 | Runs on Radxa Q6A |
| C6.2 | Measure latency: quantized vs full precision | Speedup ratio |
| C6.3 | Measure memory: peak RSS during inference | Reduction ratio |
| C6.4 | Measure power: using perf counters if available | Energy efficiency |
| C6.5 | Compare vs PyTorch quantization on same device | Fair baseline |

### C7: Paper Revision (P1, ~3 days)

| # | Task | Verification |
|---|------|-------------|
| C7.1 | Rewrite evaluation section with real data | All tables updated |
| C7.2 | Strengthen Theorem 3 proof | Formal or demoted to conjecture |
| C7.3 | Add ablation table | Section 6 expanded |
| C7.4 | Update abstract/conclusion with real numbers | Consistent |
| C7.5 | Proofread entire paper | No inconsistencies |
| C7.6 | Prepare supplementary material (code, data, scripts) | Reproducible |

### Phase C Summary

| Metric | Before | After |
|--------|--------|-------|
| Data source | Synthetic only | Real KV cache (Llama 2, Mistral) |
| Baselines | TurboQuant (random rotation) only | + KIVI, + full precision |
| Evaluation | MSE on synthetic | Perplexity on WikiText-2 |
| Ablation | None | 3 ablation configurations |
| Device testing | None | ARM64 (Radxa Q6A) |
| Paper status | Pre-print | Conference-ready |
| Target venue | None | MLSys / NeurIPS |

---

## Overall Timeline (Revised after Re-Audit)

```
Week 1:    Phase A — Fajar Lang (1 real bug + verify/wire framework modules)
Week 2-3:  Phase B — FajarOS honest fixes (NVMe, ELF loader, FS write)
Week 4-8:  Phase C — FajarQuant paper submission (real data + baselines)
Week 9:    V25 "Production" release
```

**Note:** Phase A reduced from 2 weeks to 1 week after re-audit found 4/5 claimed
bugs were false alarms. Fajar Lang is closer to production than initially assessed.

## Success Criteria for V25 "Production"

| Project | V24 Score | V25 Target | Metric |
|---------|-----------|------------|--------|
| Fajar Lang | ~90% (re-audit) | **95%** | LLVM println fixed, framework modules wired |
| FajarOS | 40% | **65%** | NVMe works, ELF loads, FS writes |
| FajarQuant | Pre-print (5/10) | **8/10** | Real data, baselines, perplexity |

## Audit Methodology

**CRITICAL:** All future audits MUST use hands-on verification (run the code),
not code-reading analysis. The V24 audit produced 4 false positives because
the audit agents read code and made assumptions without running tests.

**Correct approach:**
1. Write a minimal .fj test case
2. Run it: `fj run test.fj`, `fj build test.fj`, `fj run --llvm test.fj`
3. Check actual output vs expected
4. Only then categorize as bug or working

**Incorrect approach (produced false alarms):**
- "I see push_scope but no pop_scope" → wrong (pop was inside emit_unused_warnings)
- "Struct fields changed so LLVM must be broken" → wrong (LLVM was already synced)
- "JIT strings output pointers" → wrong (was fixed in a previous version)

---

*V25 "Production" Plan — revised after hands-on re-audit*
*Created: 2026-04-07, Revised: 2026-04-07 (re-audit eliminated 4 false alarms)*
