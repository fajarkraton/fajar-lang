# V26 "Final" — Path to 100% Production Across All Three Products

> **Version:** 1.0 (2026-04-11)
> **Author:** Muhamad Fajar Putranto, SE., SH., MH. (TaxPrime / PrimeCore.id)
> **Predecessor:** V25 v5.0 "Production" (2026-04-07) — partial completion
> **Audit method:** Hands-on verification (run + read + cross-check), not document trust
> **Standard:** [x] only when actual execution produces verifiable correct output
> **Model:** Claude Opus 4.6 exclusively
> **Status:** PLANNING

---

## 1. Vision & Mission (Carry-over from V25)

### Unified Vision

> **"Build the world's first vertically integrated language–OS–ML platform
> where the compiler, operating system, and machine learning runtime share
> the same codebase, type system, and safety guarantees — surpassing
> existing solutions in each domain."**

### Three Products, One Ecosystem

| Product | Mission | Surpasses |
|---------|---------|-----------|
| **Fajar Lang** | The best systems programming language for ML + OS integration — explicitness, dual-context safety, native tensor types | Rust (no lifetime annotations), C++ (compile-time ML safety), Python (bare-metal capable) |
| **FajarOS** | A production OS written 100% in Fajar Lang with kernel-native LLM inference (SmolLM-135M) — no userspace, no syscall, no driver — pure Ring 0 | Linux/macOS (none have kernel LLM), seL4 (no ML), MINIX (no GPU) |
| **FajarQuant** | State-of-the-art adaptive vector quantization for LLM KV cache — wins at 2-3 bit on real Gemma 4 E2B perplexity, with compile-time safety guarantees no PyTorch implementation has | TurboQuant (2-3 bit), KIVI (memory + perplexity), AQLM (deployment safety) |

---

## 2. Verified Current State (2026-04-11, V26 Audit)

### Audit Methodology

```
MANDATORY: Run command → Capture output → Compare claim → Classify
FORBIDDEN: Read code → Assume behavior → Classify
SOURCE:    docs/HONEST_AUDIT_V26.md (full evidence)
```

### Fajar Lang — **~98% Production** (was ~95%, up after Phase A1+A2 partial)

| Subsystem | Verified | Status |
|-----------|----------|--------|
| Test suite | `cargo test --lib` → **7,581 pass, 0 flakes** (80/80 stress runs at `--test-threads=64`) | ✅ |
| Clippy | `cargo clippy --lib -- -D warnings` → 0 warnings | ✅ |
| Format | `cargo fmt --check` → exit 0 (was 6 diffs, fixed in `7ee1025`) | ✅ |
| Pre-commit hook | `scripts/git-hooks/pre-commit` rejects fmt drift (commit `6775e44`) | ✅ |
| CI flake stress | New `flake-stress` job runs `--test-threads=64 × 5` per push (commit `73ed3f0`) | ✅ |
| Production `.unwrap()` | **0** verified by `scripts/audit_unwrap.py` (was claimed 174, real was 3, all replaced with `.expect()` in `968beaa`) | ✅ |
| @kernel transitive heap taint | Commit `849943d` — V17 critical bug FIXED, now blocks indirect heap alloc | ✅ |
| LLVM backend | 30+ enhancements + recent fixes (`b14f136`, `3e5bae0`, `e48afe8`, `d36661e`) | ✅ |
| All V17 critical bugs | HashMap, JIT strings, AOT linking, native test crash, tensor `+` | ✅ ALL FIXED |
| CLI commands | 23/23 subcommands declared in `src/main.rs` | ✅ |
| Examples | `ls examples/*.fj | wc -l` → **228** (CLAUDE.md says 285 — drift, fix in A4) | ⚠️ doc drift |
| Framework [f] modules | 5 remaining: `const_alloc`, `const_generics`, `const_traits`, `gui` (partial), `demos/` | ⚠️ A3 |
| Modules logical | 49 [x], 0 [sim], 5 [f], 2 [s] (per V20.5 honest classification) | ✅ |
| **Test hygiene rules** | CLAUDE.md §6.7 forbids wall-clock assertions in unit tests (commit `73ed3f0`) | ✅ |

### FajarOS — **~80% Production** (up from ~65% in V25 v5.0)

| Subsystem | Verified | Status |
|-----------|----------|--------|
| Boot sequence | 62 init stages → `nova>` shell, reliably | ✅ |
| LOC total | `find` count: **47,821 lines / 163 .fj files** (V25 said 41K — was kernel only) | ✅ grown |
| Kernel tests defined | 20 tests in `tests/kernel_tests.fj` (5 mem + 5 IPC + 5 services + 5 misc) | ⚠️ no CI |
| LLM inference E2E | SmolLM-135M v5/v6 quantized, generates diverse text with repetition penalty K=8 | ✅ |
| LLM shell commands | `model-load`, `model-info`, `embed-load`, `layer-load`, `ram-load`, `weight-status`, `tokenize`, `tok-info`, `tok-load`, `tok-reset`, `infer`, `ask`, `gen`, `tfm-info` — ALL EXIST in `shell/commands.fj` | ✅ |
| ELF loader + exec() | `kernel/core/elf_loader.fj` — V25 task B3 DONE | ✅ |
| Multi-process scheduler | `kernel/sched/scheduler.fj` + `smp_sched.fj` — V25 task B5 DONE | ✅ |
| Networking stack | `drivers/virtio_net.fj` (31 KB) + `services/net/*` (TCP v3, UDP, HTTP, DNS) — V25 task B6 DONE | ✅ |
| NVMe R/W | Sector R/W verified, PRP fix in `7a7c35b`, layer-streaming working | ✅ |
| FajarQuant in kernel | Phase 1+2 done: `kernel/compute/fajarquant.fj` (708 LOC) + `kmatrix.fj` (1,035 LOC) | ✅ |
| LLVM backend hot path | `e48afe8`+`b14f136`+`3e5bae0`+`d36661e` fixes — string display + AVX2 i64 working | ✅ |
| **fork() syscall** | `kernel/core/syscall.fj` — `TODO: return actual PID from scheduler` | ❌ broken |
| **Process exit cleanup** | `kernel/sched/process.fj` + `kernel/core/sched.fj` — `TODO: signal parent, free resources` | ❌ leak |
| **VFS write path** | RamFS read-only; FAT32/ext2 write path exists but untested | ⚠️ |
| **SMEP** | Disabled (`kernel/main.fj:107` — TODO until U/S=0 verified) | ⚠️ security |
| **CPUID guarding** | Assumes Nehalem+ AVX2/AES; no runtime feature detect | ⚠️ portability |
| **v5_4bit sample** | Workaround: dispatch to argmax (LLVM O2 wild-pointer crash with inline loops) | ⚠️ mitigated |
| **LLM coherence ceiling** | SmolLM-135M @ 2-bit/4-bit produces diverse but not coherent sentences | ⚠️ model size |

### FajarQuant — **~75% Production** (up from V25)

| Subsystem | Verified | Status |
|-----------|----------|--------|
| Algorithm modules | `src/runtime/ml/fajarquant/` — **1,743 LOC** in 5 files (adaptive 518 + fused_attention 320 + hierarchical 401 + kivi 493 + mod 11) | ✅ |
| Test count | **38 tests pass:** 22 unit + 8 e2e + 8 safety (V25 promised 31) | ✅ exceeds |
| Demos | **5 demos** in `examples/`: adaptive, benchmark, fused, kv_cache, paper_benchmark (V25 promised 6 — `hierarchical_demo` missing) | ⚠️ |
| Paper | `paper/fajarquant.tex` 407 lines → **5-page PDF** (507 KB), 6 tables, 7 references, ablation, reproducibility | ✅ |
| Real KV cache data | Gemma 4 E2B, 50 prompts, `data/kv_cache/` populated | ✅ |
| 3-way comparison | FajarQuant **WINS at 2-bit** (80.14 ppl) and **3-bit** (75.65 ppl); LOSES at 4-bit (157 vs TurboQuant 92.84) | ✅ design tradeoff |
| Ablation study | PCA rotation 4-6%, fused attention 524,288× memory reduction, hierarchical 48.7% bit savings @ 10K context | ✅ |
| Kernel port (Phase 1-2) | `kernel/compute/fajarquant.fj` + `kmatrix.fj` (1,743 LOC, all `@kernel`-safe) | ✅ |
| **Multi-model validation** | Only Gemma 4 E2B. NO Mistral, Llama, Qwen | ❌ P0 |
| **Latency benchmarks** | Paper has MSE + perplexity, **NO wall-clock** comparison | ❌ P0 |
| **Kernel Phase 3-8** | Plan describes 8 phases; only 1-2 done. "Kernel-native LLM" claim premature | ⚠️ scope clarity |
| **Reproducibility automation** | Scripts work IF Gemma 4 E2B available; no fallback | ⚠️ |
| **Per-function rustdoc** | Section-level `//!` exists; per-`pub fn` sparse | ⚠️ |
| **Ablation JSON** | `data/kv_cache/ablation_results.json:80` malformed (paper tables OK) | ⚠️ |

---

## 3. Phase A — Fajar Lang Polish (~95% → 100%)

> **Goal:** Eliminate every drift, every unwrap, every doc lie.
> **Duration:** 1 week (revised: A1+A2 done in ~6h instead of estimated 15h)
> **Effort:** ~25 hours estimated; **~6h actual so far** (A1+A2 effort revised down after audit)
> **Risk:** Low

### A1: Code Quality Hygiene — ✅ ALL DONE

| # | Task | Verification | Status | Commit |
|---|------|-------------|--------|--------|
| A1.1 | Run `cargo fmt` to fix 6 diffs in `src/codegen/llvm/mod.rs` (AVX2 i64 commit `e48afe8`) | `cargo fmt --check` exit 0 | ✅ DONE | `7ee1025` |
| A1.2 | Add pre-commit hook: reject commits with fmt drift | Hook installed via `scripts/install-git-hooks.sh`; tested 3 scenarios | ✅ DONE | `6775e44`, `0fdf477` |
| A1.3 | Investigate flake — turned out to be 14 wall-clock timing tests across 4 files (not just `i10_10_report_display`) | 80/80 stress runs at `--test-threads=64` after fix | ✅ DONE | `13aa9e3` |
| A1.4 | Fix root cause — was wall-clock antipattern (not shared global state as plan hypothesized); also added prevention | CI flake-stress job + CLAUDE.md §6.7 rule + memory entry | ✅ DONE | `73ed3f0` |

**Discovery during A1.3:** initial audit found 1 flaky test (`i10_10_report_display`); stress testing revealed 14 vulnerable tests across 4 files all sharing the same root cause: wall-clock `assert!(elapsed < N_ms)` on simulated/microsecond-scale work, unreliable under parallel test load. Pre-fix flake rate was ~20% per full run; post-fix is 0% across 80 consecutive runs.

**Gate:** `cargo test --lib && cargo clippy --lib -- -D warnings && cargo fmt --check` exit 0, **80 consecutive runs at `--test-threads=64`, 0 failures**. ✅ PASSED.

### A2: Production `.unwrap()` Audit

> **Reality:** 174 production `.unwrap()` (verified, not 4062 — that's including `#[cfg(test)]` modules).
> **Target:** ≤30 production `.unwrap()`, all justified by `// SAFETY:` comment or in unsafe block.

### A2: Production `.unwrap()` Audit — ✅ A2.1-A2.3 DONE, A2.5 PENDING

> **MAJOR DISCOVERY (A2.1):** the V26 plan assumed 174 production unwraps based
> on the V26 audit's initial figure. After three layers of false-positive
> filtering (file-level `#[cfg(test)]` declarations, inline test modules,
> doc comments, string literals), the **real production count is 3**, not 174.
> The 4,062 figure from the V26 audit agent was inflated 1,353× by missing
> filters. See `audit/A2_unwrap_inventory.md` for the full audit trail.
>
> **Effort revision: 16 hours estimated → ~1.5 hours actual.**

| # | Task | Verification | Status | Commit |
|---|------|-------------|--------|--------|
| A2.1 | Inventory production `.unwrap()` via `scripts/audit_unwrap.py` | `audit/unwrap_inventory.csv` created | ✅ DONE | `99a5133` |
| A2.2 | Categorize: all 3 are `infallible-by-construction` | Documented in `audit/A2_unwrap_inventory.md` | ✅ DONE | (in `99a5133`) |
| A2.3 | Replace 3 unwraps with `.expect("rationale")` | `python3 scripts/audit_unwrap.py --summary` → 0 hits | ✅ DONE | `968beaa` |
| A2.4 | ~~Remaining files~~ — **N/A**, superseded by A2.1 reality (no remaining files) | — | ⚪ N/A | — |
| A2.5 | Add `clippy::unwrap_used` lint at crate root, scoped to non-test code | `cargo clippy -- -D clippy::unwrap_used` exit 0 | ⬜ TODO | — |

**The 3 real production unwraps (now fixed):**
1. `compiler/incremental/rebuild_bench.rs:334` → `.expect("synthetic project graph from generate_project is acyclic by construction")`
2. `compiler/incremental/rebuild_bench.rs:338` → (same expect)
3. `distributed/dist_bench.rs:415` → `.expect("points.len() ≥ 2 guaranteed by guard above")`

**Gate (revised):** `python3 scripts/audit_unwrap.py --summary` → **0 production unwraps**. ✅ ACHIEVED.
**Lint gate (A2.5):** `cargo clippy --lib -- -D clippy::unwrap_used` → exit 0 with `#[cfg_attr(not(test), ...)]` scoping.

### A3: Module Wiring (Framework → Production)

> **Goal:** Move 3 of 5 [f] modules to [x]. Two ([demos/], [gui partial]) accepted as non-core.

| # | Task | Verification | Est. |
|---|------|-------------|------|
| A3.1 | Wire `const_alloc!()` macro syntax → callable from `.fj` | `examples/const_alloc_demo.fj` runs, returns ConstAlloc map | 4 h |
| A3.2 | Wire `const_generics` exposure: parse `const N: usize` in fn signature | `examples/const_generics_demo.fj` compiles | 6 h |
| A3.3 | Wire `const_traits` exposure: parse `const fn` in trait body | `examples/const_traits_demo.fj` compiles | 4 h |
| A3.4 | Reclassify `gui` as [x] (already callable, just doc fix) and `demos/` as `[r]` reference (not [f]) | HONEST_STATUS_V26.md updated | 1 h |

**Gate:** Module count: **52 [x], 0 [sim], 2 [r], 2 [s]** — zero framework modules.

### A4: Documentation Truth

| # | Task | Verification | Est. |
|---|------|-------------|------|
| A4.1 | Update `CLAUDE.md`: 7,580 lib tests (was 11,395), 228 examples (was 285) | Numbers match `cargo test --lib` + `ls examples/*.fj` | 30 min |
| A4.2 | Create `docs/HONEST_STATUS_V26.md` — replaces V20.5 status | New doc with 52 [x] count | 1 h |
| A4.3 | Update `MEMORY.md` current status section | Reflects V26 numbers | 30 min |

**Gate:** Every number in `CLAUDE.md` and `MEMORY.md` matches a runnable command output.

### Phase A Success Criteria

```
✅ cargo fmt --check          → exit 0                                       (A1.1)
✅ Pre-commit hook installed  → rejects fmt drift                            (A1.2)
✅ cargo clippy -- -D warnings → exit 0                                      (ongoing)
✅ cargo test --lib            → 0 failures, 0 flakes (80 stress runs)       (A1.3+A1.4)
✅ CI flake-stress job         → runs --test-threads=64 × 5 per push         (A1.4)
✅ Production .unwrap() count  → 0 (verified by scripts/audit_unwrap.py)     (A2.3)
⬜ cargo clippy -- -D unwrap_used → exit 0 with cfg_attr(not(test))          (A2.5)
⬜ Module count                → 52 [x], 0 [f]                                (A3)
⬜ Doc numbers                 → 100% match runnable commands                 (A4)
```

**Phase A current: 95% → ~98% production. Target 100% pending A2.5 + A3 + A4.**

### Phase A Progress Snapshot (2026-04-11)

| Subphase | Estimate | Actual | Commits |
|---|---|---|---|
| A1 (Code Quality) | 4.5 h | ~5 h (slightly over due to flake hunt depth) | `7ee1025`, `6775e44`, `0fdf477`, `13aa9e3`, `73ed3f0` |
| A2 (Unwrap audit) | 16 h | **~1.5 h** (count was 3 not 174) | `99a5133`, `968beaa`, A2.5 pending |
| A3 (Module wiring) | 15 h | not started | — |
| A4 (Doc truth) | 2 h | not started | — |
| **Phase A total** | **37.5 h** | **~6.5 h done**, ~17 h remaining | — |

---

## 4. Phase B — FajarOS Hardening (~80% → 95%)

> **Goal:** Fix every TODO blocking production. Ship a kernel that boots, runs LLM, and survives stress.
> **Duration:** 2 weeks
> **Effort:** ~80 hours
> **Risk:** Medium (LLVM O2 fragility, kernel debugging)

### B1: Critical Kernel Bugs

| # | Task | File:Line | Verification | Est. |
|---|------|-----------|-------------|------|
| B1.1 | Fix `fork()` syscall to return actual PID from scheduler | `kernel/core/syscall.fj` (TODO line) | Userland fork() returns child PID > 0 | 2 h |
| B1.2 | Implement process exit: signal parent + free resources (frames, fd table, IPC) | `kernel/sched/process.fj`, `kernel/core/sched.fj` | `ps` after exit shows no zombie; free frames reclaimed | 4 h |
| B1.3 | Add zombie process reaping via `waitpid()` syscall | `kernel/core/syscall.fj` | Parent receives child exit code | 3 h |
| B1.4 | Document v5_4bit sample workaround in `kernel/compute/transformer.fj:1426` with sentry test | Comment block + test that detects regression | 1 h |

**Gate:** `nova> fork-test` spawns 5 children, all exit cleanly, no zombies, no leaked frames.

### B2: Test Infrastructure & CI

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B2.1 | Create `.github/workflows/qemu-test.yml`: build LLVM ELF, boot QEMU, run `test-all`, parse `RESULT:PASS:`/`RESULT:FAIL:` markers | CI runs on every push, ≥18/20 tests pass | 4 h |
| B2.2 | Add LLM E2E inference test: `nova> ask hello` → assert output ≥10 unique tokens | New test in `tests/kernel_tests.fj` | 2 h |
| B2.3 | Add LLVM O2 sentry test: small program exercising `mdl_ram_lmhead_argmax_v5_4bit` hot path; fail if output corrupts | Sentry test in CI | 3 h |
| B2.4 | Add memory regression test: boot, run 100 fork/exit cycles, check free frame count returns to baseline ±5 frames | Memory test in `kernel_tests.fj` | 2 h |

**Gate:** CI green for 5 consecutive commits. ≥18/20 kernel tests pass. Sentry test catches simulated O2 regression.

### B3: VFS Completeness

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B3.1 | Implement RamFS write path: `vfs_write()` syscall, file create, append, truncate | `nova> echo "data" > /tmp/test && cat /tmp/test` | 4 h |
| B3.2 | Implement FAT32 write: directory entry create, FAT chain extend, cluster alloc | `nova> echo "data" > /mnt/fat/test && reboot && cat /mnt/fat/test` | 8 h |
| B3.3 | Implement ext2 write (existing scaffold in `fs/ext2_ops.fj`) | `nova> ext2-write /mnt/ext2/x` returns OK | 6 h |
| B3.4 | Add `nova> df` command showing VFS stats (used/free per mount) | Output matches actual usage | 1 h |

**Gate:** Roundtrip write+reboot+read test passes for FAT32 and RamFS.

### B4: Security Hardening

| # | Task | Verification | Est. |
|---|------|-------------|------|
| B4.1 | Audit all kernel pages: verify U/S=0 on every mapping in `kernel/mm/paging.fj` | Page table dump shows U/S=0 for all kernel-only pages | 3 h |
| B4.2 | Enable SMEP via CR4.SMEP set (`kernel/main.fj:107` TODO) after audit passes | `nova> smep-test` triggers fault on user-mode kernel access | 1 h |
| B4.3 | Add CPUID feature detection at boot: AVX2, AES, POPCNT, BSF, x2APIC, NX | `nova> cpufeatures` lists detected, gates accordingly | 4 h |
| B4.4 | Add CR4.SMAP (Supervisor Mode Access Prevention) | `STAC`/`CLAC` wrappers for user pointer access | 4 h |
| B4.5 | Add KASLR: randomize kernel base address at boot | Kernel base differs across reboots | 6 h |

**Gate:** SMEP + SMAP enabled. CPUID detection prevents boot on pre-Nehalem CPUs gracefully.

### B5: LLM Quality Decision (Strategic)

> **Decision required before B5 begins.** Three options:

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A: Stay with SmolLM-135M v5/v6** | Already working, in production | Output not coherent sentences | 0 h |
| **B: Upgrade to SmolLM-360M** | 90 MB @ 2-bit, fits 512 MB QEMU, ~3x param count, better quality | Need export script + tensor pool extension to 1024 dim | ~12 h |
| **C: Port Gemma 3 270M** | Modern arch, 6 norms/layer, sliding window, 256K vocab | Significant kernel work: q_norm/k_norm, dual RoPE theta, RECENT_BITSET 6KB→32KB, all hot paths revisit + LLVM O2 whack-a-mole | ~40 h, multi-session |

**Recommendation:** **Option B (SmolLM-360M)** — best ROI. Reserve Option C for V27.

| # | Task (Option B) | Verification | Est. |
|---|------|-------------|------|
| B5.1 | Adapt `scripts/export_smollm_v5.py` for SmolLM-360M (d_model=960, n_heads=15, 32 layers) | `.fjm v5` file generated, ~90 MB | 2 h |
| B5.2 | Extend tensor pool to 1024-dim slots in `kernel/compute/tensor.fj` | `tensor_alloc_large(960)` works | 4 h |
| B5.3 | Update model loader for 32 layers, 15 heads | `model-load nvme 0` reads correct metadata | 2 h |
| B5.4 | E2E test: load 360M, run `ask hello`, assert ≥20 unique tokens | Generates more coherent output than 135M | 2 h |
| B5.5 | Document file sizes, memory layout, load times | Updated `kernel/compute/model_loader.fj` comments | 1 h |

**Gate:** `nova> ask "what is 2+2"` produces a recognizable answer (even if imperfect).

### Phase B Success Criteria

```
✅ fork()                       → returns correct PID
✅ Process exit                 → no zombie, no frame leak
✅ ≥18/20 kernel tests pass    → in CI
✅ VFS write                    → FAT32 + RamFS roundtrip
✅ SMEP + SMAP                  → enabled
✅ LLM upgrade                  → SmolLM-360M working OR documented decision to defer
✅ CI green                     → 5 consecutive commits
```

**Phase B target: 80% → 95% production.**

---

## 5. Phase C — FajarQuant Multi-Model + Paper (~75% → 100%)

> **Goal:** Paper submission-ready for top-tier venue. Algorithm validated across 4+ models. Performance characterized end-to-end.
> **Duration:** 3 weeks
> **Effort:** ~80 hours (mostly GPU time + scripts)
> **Risk:** Medium (model availability, GPU compute time)

### C1: Multi-Model Validation (P0 Blocker)

> **Goal:** Prove FajarQuant's 2-bit/3-bit win generalizes beyond Gemma 4 E2B.

| # | Task | Verification | Est. |
|---|------|-------------|------|
| C1.1 | Adapt `scripts/extract_kv_cache.py` for HuggingFace models with `transformers` | Script accepts `--model <name>` arg | 2 h |
| C1.2 | Extract KV cache: **Mistral 7B** (50 prompts, 32 layers × 8 KV heads × 128 dim) | `data/kv_cache/mistral_7b/` populated | 4 h GPU |
| C1.3 | Extract KV cache: **Llama 2 7B** (50 prompts, 32 layers × 32 KV heads × 128 dim) | `data/kv_cache/llama2_7b/` populated | 4 h GPU |
| C1.4 | Extract KV cache: **Qwen 7B** or **Phi-3 mini** (modern arch, sliding window) | `data/kv_cache/qwen_7b/` populated | 4 h GPU |
| C1.5 | Run 3-way comparison (FajarQuant vs KIVI vs TurboQuant) on each model at 2/3/4-bit | `comparison_results_<model>.json` for each | 8 h |
| C1.6 | Run perplexity eval on WikiText-2 for each model × bit width | 3 models × 3 bit widths × 3 algorithms = 27 PPL numbers | 6 h GPU |
| C1.7 | Update paper Table 1-5 with multi-model results | Tables show consistent 2-3 bit win | 4 h |

**Gate:** FajarQuant wins ≥2/3 of models at 2-bit and 3-bit, OR paper transparently documents where it loses and why.

### C2: Performance Characterization (P0 Blocker)

> **Goal:** Wall-clock numbers, not just MSE/PPL.

| # | Task | Verification | Est. |
|---|------|-------------|------|
| C2.1 | Latency profiling: `quantize_kv_cache()` per-layer time on RTX 4090 | Microbenchmark report in `bench/results/quant_latency.csv` | 4 h |
| C2.2 | Throughput: tokens/sec for KV-quantized inference vs FP16 baseline | Benchmark on Llama 2 7B, batch sizes 1/4/16 | 4 h |
| C2.3 | Memory profiling: peak RSS for each algorithm at 16K context | `bench/results/memory_profile.csv` | 3 h |
| C2.4 | Wall-clock vs TurboQuant: head-to-head on identical hardware | Bar chart in paper | 4 h |
| C2.5 | Add Section "Performance Evaluation" to paper with latency/throughput tables | Paper updated | 4 h |

**Gate:** Paper has wall-clock numbers for all 3 algorithms on at least 1 model.

### C3: Paper Polish

| # | Task | Verification | Est. |
|---|------|-------------|------|
| C3.1 | Honest split: clearly distinguish "Rust runtime FajarQuant (Gemma 4 E2B benchmark)" vs "FajarOS kernel FajarQuant (SmolLM-135M demonstration)" | Section 5.2 rewritten | 2 h |
| C3.2 | Choose target venue: **MLSys 2027** (best fit), or NeurIPS 2026 ML Systems workshop | Decision documented in `paper/SUBMISSION.md` | 1 h |
| C3.3 | Format for chosen venue (column width, font, citation style) | LaTeX template applied | 2 h |
| C3.4 | Write supplementary materials: full reproduction commands, dataset checksums, model weights provenance | `paper/supplementary.tex` | 4 h |
| C3.5 | Add broader impact statement (quantization affects model interpretability) | New section in paper | 1 h |
| C3.6 | Add author affiliation, ORCID, code/data DOI | Title page updated | 1 h |
| C3.7 | Proofread (3 passes: technical, grammar, clarity) | Clean reading | 4 h |

**Gate:** Paper PDF compiles, fits venue page limit, all references complete, supplementary materials linked.

### C4: Reproducibility & Polish

| # | Task | Verification | Est. |
|---|------|-------------|------|
| C4.1 | Add download fallback: if Gemma 4 E2B unavailable, use SmolLM-135M as smoke test | Script runs without GPU access | 2 h |
| C4.2 | Create `reproduce.sh` one-script entry point: extract → compare → eval → ablation → tables | Single command produces all paper results | 3 h |
| C4.3 | Add 6th demo: `examples/fajarquant_hierarchical_demo.fj` (V25 promised, never delivered) | Demo runs, exits 0 | 1 h |
| C4.4 | Per-function rustdoc for all `pub fn` in `src/runtime/ml/fajarquant/*.rs` | `cargo doc` shows complete API docs | 4 h |
| C4.5 | Fix `data/kv_cache/ablation_results.json:80` malformed JSON | `jq . ablation_results.json` succeeds | 30 min |
| C4.6 | Update `Cargo.toml`: pin FajarQuant to crate version `0.1.0` for citation | Cargo workspace clean | 30 min |

**Gate:** `bash paper/reproduce.sh` regenerates every paper number on a fresh checkout (with cached Gemma 4 E2B).

### Phase C Success Criteria

```
✅ Multi-model validation       → 4 models tested, FajarQuant wins documented
✅ Performance benchmarks       → wall-clock vs TurboQuant published
✅ Paper compiles               → fits venue page limit
✅ Supplementary materials      → linked
✅ Reproducibility              → reproduce.sh runs end-to-end
✅ 6 demos                      → including hierarchical
✅ Per-fn rustdoc               → complete
```

**Phase C target: 75% → 100% production + paper submission-ready.**

---

## 6. Phase D — Stretch Goal: Kernel LLM Phase 3+ (Optional)

> **Goal:** Move FajarOS kernel from Phase 1-2 (algorithms) to Phase 3+ (full LLM pipeline) per `docs/FAJARQUANT_KERNEL_PLAN.md`.
> **Duration:** Defer to V27 unless time permits in V26
> **Risk:** High (LLVM O2 fragility, scope creep)

### D1: If V26 has slack, attempt Phase 3 (Model Loader)

| # | Task | Verification | Est. |
|---|------|-------------|------|
| D1.1 | Extend `kernel/compute/model_loader.fj` for SmolLM-360M (32 layers, 960 dim) | `model-load nvme 0` reads correct header | 4 h |
| D1.2 | Frame allocator: contiguous region for 90 MB embedding table | `frame_alloc_contiguous(N)` works | 3 h |
| D1.3 | Lazy layer load: load layer N on first attention call | Memory usage stays under 100 MB | 4 h |

**Defer to V27:** Phase 4 (BPE tokenizer in pure @kernel), Phase 5 (full transformer), Phase 6 (autoregressive), Phase 7 (ML scheduler), Phase 8 (edge AI pipeline).

---

## 7. Timeline (Revised)

```
WEEK 1  ──  Phase A (Fajar Lang Polish)
            └─ 25 hours: fmt, unwrap audit, module wiring, doc truth
            └─ Fajar Lang: 95% → 100% ✅

WEEK 2  ──  Phase B Part 1 (FajarOS Critical Bugs + CI)
            └─ B1: fork(), process exit (10 h)
            └─ B2: CI + sentry tests (12 h)

WEEK 3  ──  Phase B Part 2 (VFS + Security + LLM Decision)
            └─ B3: VFS write (19 h)
            └─ B4: SMEP/SMAP/CPUID (18 h)
            └─ B5: Decision + execution (12 h SmolLM-360M)
            └─ FajarOS: 80% → 95% ✅

WEEK 4  ──  Phase C Part 1 (Multi-Model Validation)
            └─ C1: Mistral + Llama + Qwen extraction + comparison (32 h)

WEEK 5  ──  Phase C Part 2 (Perf + Paper Polish)
            └─ C2: latency/throughput/memory benchmarks (19 h)
            └─ C3: paper polish (15 h)
            └─ C4: reproducibility (11 h)
            └─ FajarQuant: 75% → 100% ✅

WEEK 6  ──  V26 "Final" Release
            └─ Paper submission to MLSys 2027 (or workshop)
            └─ FajarOS v1.0 release notes
            └─ Fajar Lang v1.0 stable
            └─ All three products: ≥95% production
```

**Total effort:** ~185 hours over 6 weeks (assumes 1 dev, 30 h/week).

---

## 8. Success Criteria (V26 Final)

| Product | V25 v5.0 | V26 Target | V26 Stretch |
|---------|----------|------------|-------------|
| **Fajar Lang** | 95% | **100%** | — |
| **FajarOS** | 65% | **95%** | 100% (SmolLM-360M coherent answers) |
| **FajarQuant** | algorithm done, paper draft | **100% + paper submitted** | accepted at MLSys 2027 |

### V26 "Done" Definition

```
✅ cargo test --lib                        → 0 failures, 0 flakes
✅ cargo clippy --lib -- -D warnings       → 0 warnings
✅ cargo fmt --check                       → exit 0
✅ cargo clippy -- -D unwrap_used          → ≤30 instances
✅ Module count                            → 52 [x], 0 [f]
✅ FajarOS CI (QEMU test-all)              → ≥18/20 kernel tests pass
✅ FajarOS LLM E2E test                    → `ask hello` produces ≥10 tokens
✅ FajarOS fork() + waitpid()              → standard POSIX semantics
✅ FajarOS VFS                             → write roundtrip on FAT32 + RamFS
✅ FajarOS security                        → SMEP + SMAP enabled, CPUID gated
✅ FajarQuant multi-model                  → 4 models benchmarked
✅ FajarQuant performance                  → wall-clock numbers published
✅ FajarQuant paper                        → fits venue page limit, supplementary linked
✅ FajarQuant reproducibility              → reproduce.sh runs end-to-end
✅ All three products                      → release notes + version tags
```

---

## 9. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **LLVM O2 wild-pointer regression** when changing FajarOS hot path | High | Critical | Sentry test in CI (B2.3); never modify hot path without sentry green |
| **SmolLM-360M tensor pool extension** breaks v5/v6 backward compat | Medium | High | Keep 128-dim slots untouched, add 1024-dim slots separately |
| **Mistral 7B / Llama 2 7B download/license issues** | Medium | High | Use unsloth mirrors (proven for Gemma 3 270M); fall back to open models |
| **Multi-model validation reveals FajarQuant doesn't generalize** | Low | Critical | Document honest tradeoffs; reframe as "structured low-rank specialist" |
| **MLSys 2027 deadline conflict** | Medium | Medium | Have NeurIPS workshop as backup |
| **Process exit cleanup introduces deadlock** | Medium | High | Test under stress (100 fork/exit); add timeout to waitpid() |
| **SMEP enable triggers undetected U/S=1 mapping** | Medium | Critical | Audit B4.1 must be exhaustive before B4.2; staged rollout |
| **C1 GPU compute time exceeds budget** | Low | Medium | Use spot instances; priority-queue extraction by model importance |
| **Paper venue rejection** | Medium | Low | Workshop track as backup; arXiv preprint regardless |

---

## 10. Audit Methodology (V26 Standard)

```
RULE 1: Every "[x]" must have a verifiable command in the right column.
RULE 2: Numbers in CLAUDE.md and MEMORY.md must match `find` / `grep` / `cargo test` output.
RULE 3: Memory files older than 7 days are flagged stale; verify before citing.
RULE 4: When auditing, use Bash to run the actual command, not Read to assume.
RULE 5: Multi-agent audits in parallel get cross-checked; single-source claims are verified manually.

V26 audit corrections from V25:
- ".unwrap() count 4062" was inflated — actual production count is 174 (rest in #[cfg(test)] modules)
- "FajarOS LLM commands missing" was wrong — they exist but use byte-level dispatch (volatile_read_u8)
- "FajarOS 41K LOC" was kernel-only — total with shell + drivers + fs + services is 47,821
- "11,395 tests" in CLAUDE.md drift — actual lib tests = 7,580

V26 audit confirmed corrections:
- @kernel transitive heap taint: now ENFORCED (V17 critical bug fixed)
- All V17 critical bugs: HashMap, JIT strings, AOT linking, native crash, tensor + — ALL FIXED
- LLVM backend: production-grade with 30+ enhancements + 4 recent string display fixes
```

---

## 11. Commit Convention (V26 Sessions)

```
Format: <type>(<scope>): <description>

V26 scopes (in addition to V25):
  v26-a1, v26-a2, v26-a3, v26-a4   — Fajar Lang Polish phases
  v26-b1, v26-b2, v26-b3, v26-b4, v26-b5  — FajarOS Hardening phases
  v26-c1, v26-c2, v26-c3, v26-c4   — FajarQuant Multi-Model phases
  v26-d1                             — Stretch goals

Examples:
  fix(v26-a1): cargo fmt LLVM AVX2 i64 SIMD diffs
  feat(v26-b1): fork() returns child PID from scheduler
  feat(v26-c1): Mistral 7B KV cache extraction script
  docs(v26-c3): paper supplementary materials
```

---

## 12. Execution Order & Dependencies

```
Phase A ── independent ── can start anytime
Phase B ── independent ── can start anytime
Phase C ── independent ── can start anytime

Within Phase A:  A1 → A2 → A3 → A4 (sequential)
Within Phase B:  B1 + B2 (parallel) → B3 → B4 → B5 (decision then exec)
Within Phase C:  C1 (4 models in parallel) → C2 (perf) → C3 (paper) → C4 (polish)

Optimal schedule: A + B run weeks 1-3 in parallel (different products), C runs weeks 4-5.
```

---

## 13. Post-V26 Outlook

### V27 "Convergence" — Tentative Scope

- **FajarOS Phase 3-8** of FAJARQUANT_KERNEL_PLAN (full kernel LLM pipeline)
- **Gemma 3 270M port** (6 norms/layer, dual RoPE, sliding window)
- **FajarQuant benchmark suite** (more models, more bit widths, more datasets)
- **Fajar Lang stdlib expansion** based on FajarOS use cases
- **Commercial release**: PrimeCore.id distribution, Radxa Q6A image, install.sh

### Long-term (V28+)

- Self-hosting: Fajar Lang compiles itself (Stage 0 bootstrap)
- FajarOS on real hardware: Lenovo Legion Pro, Radxa Q6A, RPi 5
- FajarQuant in production at PrimeCore.id internal tools
- Paper accepted at MLSys 2027 or NeurIPS 2026

---

*V26 "Final" Production Plan v1.0 — 2026-04-11*
*All three products: ≥95% production by end of Week 5.*
*Audit standard: hands-on verification, no document trust.*
*Predecessor: V25 v5.0 "Production" — partial completion, this plan finishes the job.*
