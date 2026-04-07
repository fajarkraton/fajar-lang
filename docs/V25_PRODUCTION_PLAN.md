# V25 "Production" — Complete Roadmap to Commercial Release

> **Date:** 2026-04-07
> **Author:** Fajar (TaxPrime / PrimeCore.id)
> **Vision:** Production-grade embedded ML + OS platform for commercial deployment
> **Audit method:** ALL claims verified by running code, NOT code-reading
> **Standard:** [x] only when `fj run` / QEMU boot / benchmark produces correct output

---

## Hands-On Re-Audit Summary (2026-04-07)

Three rounds of hands-on verification eliminated false alarms from code-reading audits:

| Phase | Initial Audit (code-reading) | Hands-On Re-Audit | Savings |
|-------|-----------------------------|--------------------|---------|
| **A** | 5 critical bugs, 33 hours | **1 real bug** (LLVM println), 15 hours | 18 hours saved |
| **B** | "NVMe completely broken", 8h fix | **NVMe controller works**, sector read fix 4h | 4 hours saved |
| **C** | "Paper incomplete", 3-5 weeks | **Paper structurally complete**, ~2 weeks | 1-3 weeks saved |

**Rule:** Never trust code-reading audits alone. Always run the code.

---

## Phase A: Fajar Lang — Final Fixes (Target: 3-5 days)

> **Current:** ~90% production (re-audit verified)
> **After Phase A:** 95% production
> **Commercial impact:** Language compiler ready for customer use

### What WORKS (verified by running code)

| Feature | Test Command | Result |
|---------|-------------|--------|
| Interpreter | `fj run hello.fj` | ✅ Correct output |
| REPL | `echo 'println(42)' \| fj repl` | ✅ Works |
| Type checker | `fj check` with type mismatch | ✅ SE004 fires |
| Formatter | `fj fmt file.fj` | ✅ Reformats correctly |
| Bytecode VM | `fj run --vm file.fj` | ✅ Correct output |
| Native JIT | `fj run --native` + fib(30) | ✅ 832040, f-strings work |
| AOT build | `fj build file.fj -o bin && ./bin` | ✅ Produces working ELF |
| LLVM (int math) | `fj run --llvm` + arithmetic | ✅ Correct result |
| CUDA GPU | `gpu_matmul()` on RTX 4090 | ✅ 3x speedup |
| Test framework | `@test fn` + assert_eq | ✅ Pass/fail reporting |
| @kernel/@device | `@kernel fn` + `zeros()` | ✅ KE002 fires (148 tests) |
| HashMap | `map_new()` + `map_insert()` + `map_get()` | ✅ Fixed (commit `30ef65b`) |
| FajarQuant | fused attention, hierarchical | ✅ All 30 tests + 5 demos |
| Package manager | `fj add`, `fj tree`, `fj audit` | ✅ Works with fj.toml |
| hw-info | `fj hw-info` | ✅ Detects CPU + GPU |

### A1: Fix LLVM println Segfault (P1, ~3 hours)

**Bug:** `fj run --llvm` + `println("hello")` → SIGSEGV. Pure int math works.
**Scope:** Runtime function pointer linkage for string builtins in LLVM JIT.

| # | Task | Verification |
|---|------|-------------|
| A1.1 | Debug: trace segfault to specific function call | Crash location identified |
| A1.2 | Check `fj_rt_println_str` symbol resolution in JIT | Symbol address valid |
| A1.3 | Fix runtime function pointer registration | println linked |
| A1.4 | Test: `println("hello")` via LLVM | Output: "hello" |
| A1.5 | Test: f-strings + string ops via LLVM | No segfault |

**Gate:** `fj run --llvm file.fj` with `println("hello")` → "hello" (no crash).

### A2: Verify + Wire Framework Modules (P2, ~12 hours)

> **Rule:** Verify FIRST by running code, wire ONLY if actually broken.

| # | Module | Verify Command | Wire If Needed | Est. |
|---|--------|---------------|---------------|------|
| A2.1 | concurrency_v2 | `actor_spawn("fn", input)` from .fj | CLI demo | 1h |
| A2.2 | debugger_v2 | `fj debug --record` | CLI replay | 2h |
| A2.3 | ml_advanced | `diffusion_create()` from .fj | CLI demo | 1h |
| A2.4 | deployment | `fj deploy` actual output | Wire to containers.rs | 2h |
| A2.5 | jit | `fj run --jit` behavior | Wire tiered compilation | 2h |
| A2.6 | lsp_v3 | LSP semantic tokens | Wire to server.rs | 1h |
| A2.7 | playground | `fj playground` output | Wire HTML gen | 1h |
| A2.8 | plugin | `fj plugin load` | Wire dlopen | 2h |
| A2.9 | wasi_p2 | WASI component model | Wire to CLI | 2h |

**Gate:** Each module verified → wired → tested from CLI.

### Phase A Summary

| Metric | Before | After |
|--------|--------|-------|
| Real bugs remaining | 1 (LLVM println) | 0 |
| Framework modules | Up to 9 need verification | Verified + wired |
| `fj run --llvm` + println | SEGFAULT | WORKING |
| Production readiness | ~90% | **95%** |

---

## Phase B: FajarOS — Production Embedded OS (Target: 1-2 weeks)

> **Current:** ~60% production (re-audit: 12/15 kernel tests PASS, boots to shell)
> **After Phase B:** 80% production
> **Commercial target:** Embedded ML device OS (drone, robot, IoT)
> **NOT a hobby/research OS — this is a commercial product**

### What WORKS (verified by QEMU boot + serial commands)

| Subsystem | Verification | Result |
|-----------|-------------|--------|
| Boot pipeline | GRUB ISO → kernel → `nova>` prompt | ✅ Boots reliably |
| Frame allocator | kernel test `frame_alloc_free` | ✅ PASS |
| Page tables | kernel test `page_map_unmap` | ✅ PASS |
| Heap allocator | kernel test `heap_alloc_free` | ✅ PASS |
| Slab allocator | kernel test `slab_alloc_free` | ✅ PASS |
| Process lifecycle | kernel test `process_lifecycle` | ✅ PASS |
| Contiguous frames | kernel test `frame_contiguous` | ✅ PASS |
| Spinlocks | kernel test `spinlock` | ✅ PASS |
| Page table clone | kernel test `clone_page_table` | ✅ PASS |
| Perf counters | kernel test `perf_counters` | ✅ PASS |
| IPC queue | kernel test `ipc_queue_roundtrip` | ✅ PASS |
| IPC channels | kernel test `ipc_channel_register` | ✅ PASS |
| IPC notify | kernel test `ipc_notify_poll` | ✅ PASS |
| PCI enumeration | `lspci` shows 5 devices (VGA, NVMe, Ethernet, etc.) | ✅ WORKS |
| NVMe controller | Enable + identify ("QEMU NVMe Ctrl") + I/O queues | ✅ WORKS |
| VirtIO-Net | Virtqueues configured (RX=0, TX=1) | ✅ INIT WORKS |
| Serial console | UART input/output via COM1 | ✅ WORKS |
| Shell | `help`, `version`, `uname`, `cpuinfo` respond | ✅ WORKS |
| GUI compositor | 14 modules initialized | ✅ INIT WORKS |
| Ring 3 | SYSCALL/SYSRET + user page tables | ✅ WORKS |
| 100% Fajar Lang | No C source files (only .S for startup/stubs) | ✅ UNIQUE |

### What FAILS (verified by hands-on)

| Subsystem | Test | Status | Fix Estimate |
|-----------|------|--------|-------------|
| NVMe sector read | `[NVMe] Sector read FAILED` | ❌ DMA/PRP issue | 4h |
| Heap multi-alloc | kernel test | ❌ Fragmentation | 2h |
| IPC shared memory | kernel test | ❌ SHM mapping | 2h |
| IPC fast path | kernel test | ❌ Optimized path | 2h |
| FAT32 mount | Depends on NVMe read | ❌ Blocked by B1 | 2h after B1 |
| Output interleaving | GUI init overlaps shell | ⚠️ Cosmetic | 1h |

### B1: Fix NVMe Sector Read (P0, ~4 hours)

**Status verified:** Controller enables ✅, identifies ✅, I/O CQ/SQ created ✅.
**Only broken:** Sector read command submission/completion.
**Likely cause:** DMA buffer PRP address or NVMe command format.

| # | Task | Verification |
|---|------|-------------|
| B1.1 | Trace SQ submission: NVMe Read command (opcode 0x02) format | Command fields correct |
| B1.2 | Verify DMA buffer physical address is page-aligned | PRP1 address valid |
| B1.3 | Fix sector read (PRP or doorbell timing) | `[NVMe] Sector read OK` in boot log |
| B1.4 | Test write + readback | Data roundtrips correctly |

**Gate:** FajarOS boots with `[NVMe] Sector read OK` and FAT32 mounts.

### B2: Fix Kernel Test Failures (P1, ~6 hours)

| # | Task | Verification |
|---|------|-------------|
| B2.1 | Fix heap_multi_alloc (fragmentation/size limit) | kernel test PASS |
| B2.2 | Fix ipc_shm_create_destroy (SHM address mapping) | kernel test PASS |
| B2.3 | Fix ipc_fast_path (optimized IPC) | kernel test PASS |
| B2.4 | Fix output interleaving (GUI init vs shell) | Clean serial output |
| B2.5 | Run `test-all`: target 15/15 PASS (currently 12/15) | All PASS |

**Gate:** `nova> test-all` shows 15/15 PASS.

### B3: Complete ELF Loader + exec() (P1, ~12 hours)

| # | Task | Verification |
|---|------|-------------|
| B3.1 | Parse ELF64 PT_LOAD segments | Header fields correct |
| B3.2 | Map segments into user address space | Pages mapped |
| B3.3 | Set up user stack + argc/argv | Stack layout correct |
| B3.4 | Jump to entry point via IRETQ | User program runs |
| B3.5 | `nova> exec hello` runs ring3_hello.elf | Output on console |

**Gate:** `nova> exec hello` runs ELF, prints output, returns to shell.

### B4: Filesystem Write (P1, ~8 hours)

| # | Task | Verification |
|---|------|-------------|
| B4.1 | RamFS write() | `echo "data" > /tmp/test` works |
| B4.2 | RamFS create (touch) | `touch /tmp/new && ls /tmp` shows file |
| B4.3 | RamFS delete (rm) | `rm /tmp/test` removes file |
| B4.4 | FAT32 write (after NVMe fix) | Data persists to disk |

**Gate:** `nova> echo test > /tmp/file && cat /tmp/file` outputs "test".

### B5: Multi-Process Scheduler (P2, ~16 hours)

| # | Task | Verification |
|---|------|-------------|
| B5.1 | Timer-based preemptive scheduling | Two processes alternate |
| B5.2 | Context switch (save/restore registers) | No corruption |
| B5.3 | Process create from shell | `nova> exec prog &` backgrounds |
| B5.4 | Waitpid/exit cleanup | Zombie reaped |
| B5.5 | `nova> ps` shows running processes | Correct list |

### B6: Networking TX (P2, ~12 hours)

| # | Task | Verification |
|---|------|-------------|
| B6.1 | VirtIO-Net driver complete init + interrupt | Device ready |
| B6.2 | Ethernet frame TX | Frame on QEMU tap |
| B6.3 | ARP request/reply | MAC resolved |
| B6.4 | ICMP echo | `nova> ping 10.0.2.2` responds |

### Phase B Summary

| Metric | Before (re-audit) | After Phase B |
|--------|-------------------|---------------|
| Kernel tests | 12/15 PASS | **15/15 PASS** |
| NVMe | Controller OK, read FAILS | **Read + Write WORK** |
| User programs | ring3_hello.elf exists | **ELF loader + exec from shell** |
| Filesystem | Read-only RamFS | **Read/Write RamFS + FAT32** |
| Multi-process | Process table, no scheduler | **Preemptive scheduler** |
| Networking | VirtIO-Net init | **ARP + ICMP ping** |
| Production readiness | ~60% | **80%** |

---

## Phase C: FajarQuant — Paper + Commercial Product (Target: ~2 weeks)

> **Current:** Algorithm complete, all code works, paper structurally complete
> **Gap:** All experiments on synthetic data, no real LLM KV cache
> **After Phase C:** Paper submission-ready, commercial quantization library
> **Commercial target:** Embedded ML inference with KV cache compression

### What WORKS (verified by running code)

| Component | Verification | Result |
|-----------|-------------|--------|
| Lloyd-Max quantizer | 7 unit tests, codebook sorted ✅ | **CORRECT** |
| Adaptive PCA rotation | 6 tests, orthogonality verified ✅ | **CORRECT** |
| Fused attention | 4 tests, matches dequant-dot ✅ | **CORRECT** |
| Hierarchical schedule | 5 tests, bit savings correct ✅ | **CORRECT** |
| Safety tests | 8 @kernel/@device tests ✅ | **ALL PASS** |
| 5 demo examples | All exit 0 ✅ | **ALL RUN** |
| Paper compilation | pdflatex → 4 pages, 376KB PDF ✅ | **COMPILES** |
| Paper structure | 10 sections, 5 tables, 3 theorems ✅ | **COMPLETE** |
| References | 10 entries in references.bib ✅ | **COMPLETE** |
| TODOs/placeholders | grep → 0 found ✅ | **CLEAN** |
| TurboQuant baseline | `compare_adaptive_vs_random()` ✅ | **EXISTS in code** |
| DataLoader | 875 LOC, InMemoryDataset + batching ✅ | **EXISTS** |
| MNIST data | data/mnist/ (4 files) ✅ | **EXISTS** |
| MNIST builtins | `mnist_load_images/labels` wired ✅ | **EXISTS** |

### What's MISSING (confirmed by hands-on)

| Gap | Status | Impact |
|-----|--------|--------|
| PyTorch | Not installed (numpy only) | Need `pip install torch transformers` |
| Real KV cache data | No extraction script | Need Python script for LLM |
| KIVI baseline | Paper cites, code doesn't implement | Need fair comparison |
| Perplexity evaluation | No LLM inference pipeline | Need downstream metric |
| GPU VRAM | 15.9GB free on RTX 4090 | ✅ Sufficient for 7B model |

### C1: Setup + Extract Real KV Cache (P0, ~2 days)

| # | Task | Verification |
|---|------|-------------|
| C1.1 | `pip install torch transformers datasets` | Import succeeds |
| C1.2 | Write KV cache extraction script (Python) | Saves K/V per layer/head |
| C1.3 | Extract from Llama 2 7B on 500 prompts | `data/kv_cache/` directory |
| C1.4 | Analyze eigenvalue structure vs synthetic | Distribution comparison |
| C1.5 | Run FajarQuant on real KV data | MSE measured on real data |

**Gate:** Table 1 in paper regenerated with real data.

### C2: KIVI Baseline (P0, ~3 days)

| # | Task | Verification |
|---|------|-------------|
| C2.1 | Per-channel key quantization | Matches KIVI paper |
| C2.2 | Per-token value quantization | Matches paper |
| C2.3 | Run on same real KV cache data | Fair comparison |
| C2.4 | 3-way table: FajarQuant vs KIVI vs TurboQuant | Paper table |

**Gate:** Fair comparison table on real data.

### C3: Perplexity Evaluation (P0, ~3 days)

| # | Task | Verification |
|---|------|-------------|
| C3.1 | Quantized KV cache inference loop | Model generates text |
| C3.2 | Perplexity on WikiText-2 | ppl_full vs ppl_quantized |
| C3.3 | Sweep bit-widths (1, 2, 3, 4) | Tradeoff curve |
| C3.4 | Compare all methods | Fair comparison |
| C3.5 | Test on Mistral 7B (second model) | Generalization |

**Gate:** Perplexity table: FajarQuant competitive with KIVI.

### C4: Ablation + Paper Revision (P1, ~3 days)

| # | Task | Verification |
|---|------|-------------|
| C4.1 | Ablation: each innovation isolated | Contribution quantified |
| C4.2 | Fix 65.3% vs 48.7% discrepancy | Consistent numbers |
| C4.3 | Add confidence intervals | Error bars in tables |
| C4.4 | Rewrite evaluation with real numbers | All tables updated |
| C4.5 | Proofread + supplementary material | Reproducible |

### C5: Embedded Device Benchmark (P2, if hardware available)

| # | Task | Verification |
|---|------|-------------|
| C5.1 | Cross-compile for ARM64 | Runs on target device |
| C5.2 | Measure latency + memory + power | Metrics collected |
| C5.3 | Compare vs PyTorch on same device | Fair baseline |

### Phase C Summary

| Metric | Before | After |
|--------|--------|-------|
| Data source | Synthetic only | **Real KV cache (Llama 2, Mistral)** |
| Baselines | TurboQuant only | **+ KIVI, + full precision** |
| Evaluation | MSE on synthetic | **Perplexity on WikiText-2** |
| Ablation | None | **3 configs (each innovation)** |
| Paper status | Pre-print (synthetic) | **Conference-ready (real data)** |
| Target venue | TBD | **MLSys / NeurIPS** |

---

## Overall Timeline

```
Week 1:    Phase A — Fajar Lang (1 bug fix + verify/wire modules)
Week 2-3:  Phase B — FajarOS (NVMe read, kernel tests, ELF loader, FS, scheduler)
Week 4-5:  Phase C — FajarQuant (real data + KIVI baseline + perplexity + paper)
Week 6:    V25 "Production" release
```

## Success Criteria

| Project | Current (re-audit) | V25 Target | Key Metric |
|---------|-------------------|------------|------------|
| Fajar Lang | ~90% | **95%** | LLVM println fixed, modules wired |
| FajarOS | ~60% | **80%** | 15/15 tests, NVMe, ELF, FS, networking |
| FajarQuant | Algorithm complete | **Paper ready** | Real data, baselines, perplexity |

## Audit Methodology (Mandatory)

```
CORRECT:  Write test → Run code → Check output → Classify
WRONG:    Read code → Assume behavior → Classify

Examples of false alarms from code-reading:
  ✗ "push_scope without pop_scope" → pop was inside emit_unused_warnings()
  ✗ "LLVM 80+ compile errors" → LLVM was already synced, compiles clean
  ✗ "JIT string codegen broken" → was fixed in previous version
  ✗ "NVMe completely broken" → controller + I/O queues work, only read fails
  ✗ "Paper incomplete" → 10 sections, compiles to 4-page PDF

Total time saved by hands-on re-audit: ~22 hours + 1-3 weeks
```

---

*V25 "Production" Plan v3.0 — all claims verified by running code*
*Created: 2026-04-07 | Re-audited: 2026-04-07 (3 rounds of hands-on verification)*
