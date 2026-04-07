# V25 "Production" — Complete Roadmap to Full Production

> **Date:** 2026-04-07
> **Author:** Fajar (TaxPrime / PrimeCore.id)
> **Source:** V24 Deep Audit (3 parallel audits: Fajar Lang, FajarOS, FajarQuant)
> **Standard:** Every task has concrete verification. [x] only when E2E works.

---

## Phase A: Fajar Lang Critical Fixes (Target: 3-5 days)

> **Goal:** Fix 5 critical bugs + wire 9 framework modules → 95% production
> **Current:** 79% production (V24 audit)
> **After Phase A:** 95% production

### A1: Fix @kernel/@device Context Enforcement (P0, ~3 hours)

**Bug:** `is_inside_kernel()` always returns false — @kernel/@device annotations ignored.
**Root Cause:** Function context not pushed when entering annotated functions.
**File:** `src/analyzer/type_check/check.rs` — `check_function_def()` doesn't push context.

| # | Task | File | Verification |
|---|------|------|-------------|
| A1.1 | Add `context_stack: Vec<ContextKind>` to TypeChecker | `src/analyzer/type_check/mod.rs` | Compiles |
| A1.2 | Push context on `check_function_def()` for @kernel/@device/@safe | `check.rs` | `is_inside_kernel()` returns true |
| A1.3 | Pop context on function exit | `check.rs` | Stack balanced |
| A1.4 | Verify KE001 fires: heap alloc in @kernel | `tests/context_safety_tests.rs` | `let s = "hello"` in @kernel → KE001 |
| A1.5 | Verify KE002 fires: tensor in @kernel | test | `zeros([3])` in @kernel → KE002 |
| A1.6 | Verify DE001 fires: raw pointer in @device | test | Pointer deref in @device → DE001 |

**Gate:** `cargo test --test context_safety_tests` + `cargo test --test fajarquant_safety_tests` — all pass with REAL enforcement.

### A2: Fix HashMap Builtins (P0, ~3 hours)

**Bug:** `map_insert()` doesn't persist — len=0 after insert, `map_get()` returns None.
**Root Cause:** Value::Map is cloned (not mutated in place) due to Arc<Mutex> ownership.
**File:** `src/interpreter/eval/builtins.rs` — `builtin_map_insert()`

| # | Task | File | Verification |
|---|------|------|-------------|
| A2.1 | Audit `builtin_map_insert` — trace value flow | `builtins.rs` | Identify clone vs mutate |
| A2.2 | Fix map mutation: ensure insert modifies env binding | `builtins.rs` + `env.rs` | `map_insert(m, "k", "v"); map_get(m, "k")` → "v" |
| A2.3 | Fix map_remove, map_keys, map_values | `builtins.rs` | All 8 HashMap methods work |
| A2.4 | Add integration test for full HashMap lifecycle | `tests/eval_tests.rs` | insert → get → remove → len correct |

**Gate:** `fj run examples/hashmap_demo.fj` produces correct output.

### A3: Fix JIT String Codegen (P1, ~3 hours)

**Bug:** `fj run --native` prints raw pointers for strings.
**Root Cause:** Cranelift codegen emits pointer, not string value for String operations.
**File:** `src/codegen/cranelift/compile/values.rs` or `strings.rs`

| # | Task | File | Verification |
|---|------|------|-------------|
| A3.1 | Audit string representation in Cranelift IR | `codegen/cranelift/` | Identify Value::Str → IR mapping |
| A3.2 | Fix println for string literals | codegen | `fj run --native` + `println("hello")` → "hello" |
| A3.3 | Fix f-string interpolation | codegen | `f"x={x}"` outputs correctly |
| A3.4 | Fix match→string result | codegen | `match x { 0 => "zero" }` returns string |

**Gate:** `fj run --native examples/hello.fj` prints "Hello, World!" (not `0x7f...`).

### A4: Fix AOT Linking (P1, ~4 hours)

**Bug:** `fj build` produces ELF but linking fails — undefined reference to runtime functions.
**Root Cause:** Runtime library (println, builtins) not linked into binary.
**File:** `src/codegen/cranelift/aot.rs` or `src/main.rs` build pipeline

| # | Task | File | Verification |
|---|------|------|-------------|
| A4.1 | Identify which runtime symbols are missing | linker error output | List all undefined references |
| A4.2 | Generate runtime stubs (fj_rt_println, etc.) | `codegen/cranelift/` | Stubs compile to .o |
| A4.3 | Link runtime stubs into AOT binary | `main.rs` build cmd | `fj build hello.fj` → working ELF |
| A4.4 | Test AOT binary execution | shell | `./hello` prints output |

**Gate:** `fj build examples/hello.fj -o hello && ./hello` prints "Hello, World!".

### A5: Sync LLVM Backend (P2, ~8 hours)

**Bug:** 80+ compile errors — AST structs changed but LLVM codegen not updated.
**Root Cause:** Struct fields added/renamed in parser AST, LLVM backend references old names.
**File:** `src/codegen/llvm/mod.rs` (12,800 lines)

| # | Task | File | Verification |
|---|------|------|-------------|
| A5.1 | List all 80+ compile errors | `cargo build --features llvm` | Categorize by type |
| A5.2 | Fix struct field mismatches (batch 1: Expr) | `llvm/mod.rs` | Errors < 40 |
| A5.3 | Fix struct field mismatches (batch 2: Stmt, FnDef) | `llvm/mod.rs` | Errors < 10 |
| A5.4 | Fix remaining type/lifetime errors | `llvm/mod.rs` | 0 errors |
| A5.5 | Run LLVM E2E tests | `cargo test --features llvm` | 43 LLVM E2E pass |

**Gate:** `cargo build --features llvm` compiles, `fj run --backend llvm hello.fj` executes.

### A6: Wire Framework Modules to CLI (P2, ~12 hours)

| # | Module | CLI Command | File | Hours |
|---|--------|------------|------|-------|
| A6.1 | concurrency_v2 | `fj actor-demo` | `main.rs` | 1 |
| A6.2 | debugger_v2 | `fj debug --record/--replay` | `main.rs` | 2 |
| A6.3 | ml_advanced | `fj diffusion-demo` | `main.rs` | 1 |
| A6.4 | deployment | `fj deploy` (real Docker gen) | `main.rs` | 2 |
| A6.5 | jit | `fj run --tiered` | `main.rs` | 2 |
| A6.6 | lsp_v3 | Wire semantic tokens to lsp server | `lsp/server.rs` | 1 |
| A6.7 | playground | `fj playground` (HTML gen) | `main.rs` | 1 |
| A6.8 | plugin | `fj plugin load` | `main.rs` | 2 |
| A6.9 | wasi_p2 | `fj run --wasi file.fj` | `main.rs` | 2 |

**Gate:** All 9 modules callable from CLI. `fj --help` lists all commands.

### Phase A Summary

| Metric | Before (V24) | After (V25) |
|--------|-------------|-------------|
| Critical bugs | 5 | 0 |
| Production modules | 49 [x] | 56+ [x] |
| CLI commands production | 25/35 | 33/35 |
| @kernel/@device | BROKEN | ENFORCED |
| HashMap | BROKEN | WORKING |
| `fj build` | FAILS | PRODUCES WORKING ELF |
| `fj run --native` | BROKEN STRINGS | CORRECT OUTPUT |
| `fj run --llvm` | 80+ ERRORS | WORKS |

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

## Overall Timeline

```
Week 1-2:  Phase A — Fajar Lang critical fixes (5 bugs + 9 modules)
Week 3-4:  Phase B — FajarOS honest fixes (NVMe, ELF loader, FS write)
Week 5-9:  Phase C — FajarQuant paper submission (real data + baselines)
Week 10:   V25 "Production" release
```

## Success Criteria for V25 "Production"

| Project | V24 Score | V25 Target | Metric |
|---------|-----------|------------|--------|
| Fajar Lang | 79% | **95%** | 0 critical bugs, 56+ [x] modules |
| FajarOS | 40% | **65%** | NVMe works, ELF loads, FS writes |
| FajarQuant | Pre-print (5/10) | **8/10** | Real data, baselines, perplexity |

---

*V25 "Production" Plan — honest roadmap based on V24 deep audit*
*Created: 2026-04-07*
