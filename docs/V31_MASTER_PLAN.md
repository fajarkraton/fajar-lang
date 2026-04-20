# V31 Master Plan — Precision Debug + Fajar Lang Fix + FajarQuant Phase D

**Date prepared:** 2026-04-20
**Author:** Muhamad Fajar Putranto
**Upstream:** V30 Tracks 1+2+3+4 all SHIPPED (2026-04-20). V30.GEMMA3 Path D
  deferred `M7 coherent-generation` to V31 R3 via ranked hypothesis set.
  V30.TRACK4 surfaced a latent `ext2_create` bug. Both are V31 items.
**Status:** DRAFT — Pre-Flight / Rule-1 deliverable for the V31 program.

---

## 0. Executive Summary

V31 is a **three-track parallel program** covering ~10-14 weeks of solo
focused work. Tracks are prioritized by cost-to-informativeness:

| Track | Name | Effort | Role |
|---|---|---|---|
| **A** | V31.R3 Precision Debug | 1.5-2 days | **First run — cheap, high-info** |
| **B** | V31.FAJARLANG Compiler Fix | 1-1.5 weeks | **Prereq for C clean-path** |
| **C** | FajarQuant Phase D (IntLLM) | 6-8 weeks focused | **Main research deliverable** |
| **D** | V31 Carry-over bugs (ext2) | 0.5-1 week | Surfaced by V30.T4 |

Critical path: A runs first (may collapse C entirely into "not needed").
B and C.P0 (lit sweep) run in parallel after A closes. C.P5 (kernel
integration) depends on B.P1 (LLVM fix).

**Milestone targets:**

- **M8 "Precision closed"** — A done (H1-H4 results committed).
  If positive → coherent generation with existing Gemma 3 → optional
  Phase D becomes research-only, not blocker.
- **M9 "Fajar Lang clean"** — B done. 100% Fajar Lang kernel (no C
  bypass required for new ops). Regression gate for codegen.
- **M10 "IntLLM proof-of-concept"** — C.P4 done. Tiny custom
  integer-attention model produces coherent output on FajarOS,
  no pad-collapse.
- **M11 "FajarQuant v0.4 paper"** — C.P7 done. Submittable paper.
- **M12 "ext2 write path"** — D done. Latent bug closed; V30.T4
  gate expanded to 10 invariants (add the write roundtrip).

---

## 1. Context from V30 (what we know)

### 1.1 Pad-collapse evidence (V30.GEMMA3)

Observed in kernel:
- Gemma 3 1B 4-bit: token 107 (`\n`) repeated
- Gemma 3 1B 8-bit: token 106 (`<end_of_turn>`) immediate
- SmolLM-135M v5 retest: pad byte 0x00 × 64

Root cause is **model-level numerical precision**, not:
- Quant bit width (fails at 4, 8 bits identically)
- Tokenizer (fixed and verified bit-exact v2 .fjt)
- LLVM codegen for vecmat (C-bypass gives 0-ULP vs Python sim)
- Model size (observed on both 1B and 135M)

Per V30.GEMMA3.P10_FOUNDATION.md §3, four ranked hypotheses:
1. **H1** Cumulative RMSNorm scaling (104 norms per forward)
2. **H2** `c_exp_approx` softmax saturation
3. **H3** Bhaskara RoPE 0.16% shared error (LUT ×10000 would be 0.01%)
4. **H4** Final LayerNorm gamma loading byte-offset bug

### 1.2 Fajar Lang compiler gap

V30 Track 3 P3.6 found LLVM O2 miscompiles `km_vecmat_packed_v8` for
large loops (7.9M ops FFN gate). Quarantined via C bypass. Memory says
*"Fajar Lang codegen fix deferred to V31."* As of V30.GEMMA3, 10
hot-path functions are in C (~2,000 LOC duplicate).

### 1.3 Infrastructure available

- V30.SIM Python bit-exact simulator at `~/Documents/fajarquant/tools/kernel_sim/`
- FJTRACE per-op JSONL capture in kernel (`make test-fjtrace-capture`)
- `diff.py` three-way divergence tool (kernel · sim · HF reference)
- .fjm v7 + v8 group-wise quant format with full GQA/RoPE/SWA fields
- 262K BPE tokenizer export pipeline (`scripts/export_tokenizer.py`)
- Regression harness pattern (`test-security-triple-regression`,
  `test-gemma3-e2e`, `test-fs-roundtrip`)

---

## 2. Track A — V31.R3 Precision Debug

**Repo:** fajaros-x86
**Effort:** 1.5-2 days (8-12h focused)
**Runs FIRST.** Cheapest experiment, highest information value — may
collapse Track C's urgency or change its arch direction.

### 2.1 Phase A.P0 — Pre-Flight

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P0.1 | Re-run `make test-gemma3-e2e` on HEAD, confirm 5-invariant PASS still green | exit 0 | 0.1h |
| A.P0.2 | Re-capture 1-layer FJTRACE JSONL baseline | ≥27 records | 0.2h |
| A.P0.3 | Commit `docs/V31_R3_FINDINGS.md` P0 section | file in git | 0.1h |

**Gate:** baseline reproducible, infrastructure green.

### 2.2 Phase A.P1 — H3 RoPE LUT (QUICK-WIN CANDIDATE, priority)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P1.1 | Implement `rope_cos/sin` LUT ×10000 fixed-point (130 KB table) in Fajar Lang + C mirror in `vecmat_v8.c` | Table-populated boot marker | 1.0h |
| A.P1.2 | Re-run `ask hello`, capture output | Token IDs in log | 0.25h |
| A.P1.3 | Compare decoded output to V30 baseline | Diff first 10 IDs | 0.25h |
| A.P1.4 | Commit findings + decision (continue to H1/H2/H4 or pause — coherent?) | `V31_R3_H3_DECISION.md` in git | 0.5h |

**Gate (mechanical):** decision doc committed. If coherent output →
**M8 "Precision closed"** reached early. If still pad-collapse →
continue A.P2.

### 2.3 Phase A.P2 — H1 Cumulative RMSNorm instrumentation

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P2.1 | Add FJTRACE emit points for min/max/nnz per-layer RMSNorm output | 26 extra JSONL records per token | 0.5h |
| A.P2.2 | Run with 9-token prompt, capture all 26 layers | JSONL has magnitude stats | 0.25h |
| A.P2.3 | Plot magnitude drift (Python), compare to HF reference | Plot committed | 0.5h |
| A.P2.4 | If drift monotonic or oscillatory → propose variance precision fix | `V31_R3_H1_DECISION.md` | 1h |

### 2.4 Phase A.P3 — H2 Softmax saturation

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P3.1 | Dump post-softmax attention distribution layer-0 pos-0 | ≤256 values logged | 0.5h |
| A.P3.2 | Compare against PyTorch `F.softmax` output for same inputs | Diff values | 0.5h |
| A.P3.3 | If saturation confirmed → propose extended-range `c_exp_approx` | `V31_R3_H2_DECISION.md` | 1.5h |

### 2.5 Phase A.P4 — H4 Final LayerNorm gamma byte-compare

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P4.1 | Dump `final_rmsnorm` input bytes at pos 0 via FJTRACE | Bytes in JSONL | 0.25h |
| A.P4.2 | Byte-compare with HF reference forward | Match/mismatch report | 0.25h |
| A.P4.3 | If mismatch → trace gamma-load path, fix byte offset | `V31_R3_H4_DECISION.md` | 1h |

### 2.6 Phase A.P5 — Close-out

| # | Task | Verification | Est |
|---|------|--------------|-----|
| A.P5.1 | Update `V30_GEMMA3_P10_FOUNDATION.md` with R3 results | commit | 0.25h |
| A.P5.2 | Update memory + CLAUDE.md §3 | commit | 0.25h |
| A.P5.3 | If any hypothesis closed precision → **GitHub Release v3.8.0 "Precision"** | release live | 0.5h |

**Total Track A: 8-12h** (+25% = 10-15h)
**Gate:** four decision docs committed + summary in FOUNDATION.md.

---

## 3. Track B — V31.FAJARLANG Compiler Fix

**Repo:** fajar-lang (codegen changes), fajaros-x86 (rollback C bypass)
**Effort:** 1-1.5 weeks (5-8 days focused)
**Runs in parallel** with Track A.P2+ and Track C.P0+.

### 3.1 Phase B.P0 — Pre-Flight

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P0.1 | Reproduce LLVM O2 miscompile minimally (isolate `km_vecmat_packed_v8` loop into ~100-LOC test) | Minimal case diverges kernel-vs-sim | 0.5h |
| B.P0.2 | Capture pre-opt LLVM IR via `FJ_EMIT_IR=1` (already shipped feature) | .ll file committed | 0.2h |
| B.P0.3 | Commit `docs/V31_FAJARLANG_P0_FINDINGS.md` with minimal repro + IR excerpt | file in git | 0.3h |

### 3.2 Phase B.P1 — LLVM O2 Miscompile Bisect

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P1.1 | Run `opt -print-after-all` on minimal case, diff pre/post each pass | Divergent pass identified | 1-2d |
| B.P1.2 | Quarantine divergent pass via `disable-pass` or function attribute | Minimal case matches sim | 0.5d |
| B.P1.3 | Rebuild full kernel, run `make test-gemma3-e2e` + `test-fjtrace-capture` | All green, bit-exact retained | 0.5d |
| B.P1.4 | If unsolvable at pass level → file LLVM upstream bug; keep C bypass as quarantine | Bug URL + rationale | 1d |

### 3.3 Phase B.P2 — `@no_vectorize` Attribute

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P2.1 | Add `@no_vectorize` to lexer ANNOTATIONS table (follow V29.P1 pattern) | Lexer meta-test passes | 0.3d |
| B.P2.2 | Codegen mapping: `@no_vectorize` → LLVM function attribute `"no-implicit-float"` + `"disable-tail-calls"` + `"no-builtins"` + explicit loop metadata | Test function with attribute compiles | 0.7d |
| B.P2.3 | Apply to known-risky kernel functions (`km_vecmat_packed_v8`, `tok_encode_bpe`, others identified in B.P1) | AVX instruction audit via `objdump -d` | 0.5d |

### 3.4 Phase B.P3 — `i128` Codegen Audit

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P3.1 | Verify `i128` arithmetic: add/sub/mul/div + bit ops correct on x86_64 | Test passes | 0.3d |
| B.P3.2 | Verify `(i128 × i128) → i128` for Phase D matmul intermediates (overflow prevention) | Test passes | 0.2d |

### 3.5 Phase B.P4 — C Bypass Rollback (optional, post-B.P1)

Condition: B.P1 fully fixes the codegen bug.

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P4.1 | Port ONE C-bypass op back to Fajar Lang (`km_add_raw`, simplest) | kernel test green, bit-exact retained | 0.5d |
| B.P4.2 | Validate on `test-gemma3-e2e` + `test-fs-roundtrip` | No regression | 0.3d |
| B.P4.3 | Decision: continue porting (9 more) or keep current C bypass for stability | `V31_FAJARLANG_P4_DECISION.md` | 0.2d |

### 3.6 Phase B.P5 — Regression + Doc Sync

| # | Task | Verification | Est |
|---|------|--------------|-----|
| B.P5.1 | Add `test-codegen-parity` Makefile target: vecmat kernel output matches sim 0 ULP | green | 0.5d |
| B.P5.2 | CHANGELOG fajar-lang + CLAUDE.md §3 row | commit | 0.3d |
| B.P5.3 | GitHub Release fajar-lang | tag live | 0.2d |

**Total Track B: 5-8 days** (+30% research budget = 6.5-10.5 days)
**Gate:** `test-codegen-parity` green + C-bypass rollback decision.

---

## 4. Track C — FajarQuant Phase D (IntLLM)

**Repo:** fajarquant (primary arch + paper), fajar-lang (new kernel ops),
  fajaros-x86 (integration + validation)
**Effort:** 6-8 weeks focused
**Runs after** Track A closes (precision hypothesis informs design)
and **in parallel** with Track B (kernel integration depends on B.P1).

**Research focus:** *"Transformer-class sequence model designed natively
for fixed-point i64 kernel arithmetic, without softmax, RoPE, or
FP-precision-sensitive ops."* Novel contribution targeting MLSys /
NeurIPS MLSys Workshop / TinyML Summit.

### 4.1 Phase C.P0 — Literature Sweep (Rule §6.9 R2)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P0.1 | Survey ≥10 papers (2024-2025): BitNet, RWKV-7, Mamba-2, Retentive Net, PowerAttention, Hedgehog, integer-native variants | Landscape table in `FJQ_PHASE_D_P0_FINDINGS.md` | 3d |
| C.P0.2 | Identify softmax-free attention patterns: linear kernels, state-space, integer attention | ≥3 candidate families | 1d |
| C.P0.3 | Identify integer-friendly positional encoding alternatives (ALiBi, integer Rotary, no-pos state-space) | ≥2 candidates | 0.5d |
| C.P0.4 | Position IntLLM against landscape: what's genuinely novel? | 1-paragraph positioning in findings | 0.5d |

**Gate:** committed findings doc + positioning statement.

### 4.2 Phase C.P1 — Arch Design + Paper Outline

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P1.1 | Choose arch family (likely RWKV-7 integer variant or linear-attention + ALiBi integer) | Design decision in `FJQ_PHASE_D_ARCH.md` | 2d |
| C.P1.2 | Specify integer ops inventory: matmul, RMSNorm-int, bitshift-activation, aggregate op | Ops table with dtype + overflow bounds | 1d |
| C.P1.3 | Paper outline (MLSys template): intro + related work + method + experiments + ablations | `FJQ_PHASE_D_PAPER_OUTLINE.md` | 1d |
| C.P1.4 | Target params + budget: d_model ∈ {256, 384, 512}, L ∈ {6, 12, 18}, vocab 49K → 10-50M total | Config matrix | 0.5d |

### 4.3 Phase C.P2 — PyTorch Reference Implementation

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P2.1 | Implement integer attention (or state-space alternative) in PyTorch | Forward pass matches paper math | 3-4d |
| C.P2.2 | Implement integer RMSNorm, activation, position encoding per C.P1.2 | Unit tests pass | 2d |
| C.P2.3 | Put-together forward pass — loss decreasing on tiny synthetic corpus | `loss < 2.0` after 1h QAT on synthetic | 2d |
| C.P2.4 | Quantization-aware training (QAT) harness: train FP32, quantize to i64 via FajarQuant | Output shift ≤10% after quant | 2d |

### 4.4 Phase C.P3 — Dataset Pipeline

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P3.1 | Tokenizer: reuse SmolLM's 49K vocab OR build custom 32K BPE | .fjt file + self-test | 1-2d |
| C.P3.2 | Corpus: 1-5B tokens (OpenWebText / RedPajama-v2 subset / Wikipedia + books) | ≥1B tokens staged | 2d |
| C.P3.3 | Streaming loader + shard management | Loader benchmark ≥10K tok/s | 1d |

### 4.5 Phase C.P4 — Training Runs (RTX 4090 16 GB)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P4.1 | Initial config pass: d_model=384 L=12, ~25M params, 1 epoch on 1B tokens | 1 day GPU wall-clock | 1d GPU |
| C.P4.2 | HP tuning: LR schedule, batch size, weight decay — 3-5 iterations | Val PPL improving | 3-5d GPU |
| C.P4.3 | Final QAT phase: fine-tune with integer simulation | Val PPL drop ≤5% vs FP | 1d GPU |
| C.P4.4 | Checkpoint → .fjm v8 export | File loads in FajarOS | 0.5d |

**Gate (Rule §6.9 R1):** Val PPL ≤ SmolLM-135M baseline on same corpus.

### 4.6 Phase C.P5 — Kernel Integration

**Depends on:** Track B.P1 complete (write Phase D ops in Fajar Lang,
not C bypass).

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P5.1 | Implement integer attention as `@kernel @no_vectorize` function | Compiles, links | 2d |
| C.P5.2 | Implement position encoding path (ALiBi / int-RoPE / none) | Test vector matches PyTorch int reference | 1d |
| C.P5.3 | Wire into `tfm_layer_stream` as new path conditional on new model_type | Model loads, 26-layer forward runs | 2d |
| C.P5.4 | Update `.fjm` parser for Phase D header fields | Loads without error | 1d |
| C.P5.5 | Model-load → embed-load → tok-load → ask workflow green | Serial log clean | 1d |

### 4.7 Phase C.P6 — Validation (Rule §6.9 R1, R3)

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P6.1 | Canonical benchmark: PPL on Wikitext-103 + OpenWebText held-out | Single-source benchmark table | 2d |
| C.P6.2 | Comparative benchmark: **full-feature SmolLM-135M** + TinyLlama 1.1B | Both beat by ≥1 PPL OR honestly lose | 2d |
| C.P6.3 | Coherent-output qualitative check: 10 prompts × 64 tokens | No pad-collapse, sensible output | 1d |
| C.P6.4 | FajarOS E2E: boot + model-load + ask → coherent | `test-intllm-e2e` Makefile gate | 1d |

**Gate (Rule §6.9 R1):** Canonical protocol used; baseline parity kept.

### 4.8 Phase C.P7 — Paper + Release

| # | Task | Verification | Est |
|---|------|--------------|-----|
| C.P7.1 | Paper draft: method + algorithm + pseudocode | 10-page MLSys template | 3d |
| C.P7.2 | Ablation runs: no-position, no-QAT, fp32-attention baseline | Ablation table complete | 3-5d |
| C.P7.3 | `reproduce.sh` with --verify/--smoke/--full modes (mirror fajarquant v0.3.1 pattern) | 4-mode script | 1d |
| C.P7.4 | `verify_paper_tables.py --strict` (Rule §6.9 R7 mechanical gate) | exit 0 before commit | 0.5d |
| C.P7.5 | FajarQuant v0.4.0 GitHub Release + arXiv upload | release live + arXiv ID | 1d |

**Total Track C: 6-8 weeks** (+30% research budget = 8-10.5 weeks)
**Gate:** M10 + M11 (coherent E2E + submittable paper).

---

## 5. Track D — ext2 Write-Path Fix (carry-over from V30.T4)

**Repo:** fajaros-x86
**Effort:** 0.5-1 week
**Low priority** — not blocking C. Runs parallel when slot available.

### 5.1 Phase D.P0 — Pre-Flight

| # | Task | Verification | Est |
|---|------|--------------|-----|
| D.P0.1 | Reproduce `ext2_create rc=-1` via `test-fs-roundtrip` | repro confirmed | 0.1d |
| D.P0.2 | Read `fs/ext2_ops.fj:ext2_create` + inode/bitmap alloc path | Code walked, hypothesis formed | 0.5d |

### 5.2 Phase D.P1 — Fix

| # | Task | Verification | Est |
|---|------|--------------|-----|
| D.P1.1 | Fix identified bug (likely `ext2_inode_alloc` bitmap offset OR `ext2_block_alloc`) | In-kernel roundtrip PASS | 1-2d |
| D.P1.2 | Extend `test-fs-roundtrip` ext2 branch to 5 PASS invariants (add write + readback) | gate green | 0.3d |

### 5.3 Phase D.P2 — Regression + Release

| # | Task | Verification | Est |
|---|------|--------------|-----|
| D.P2.1 | Remove NOTE line from `test-fs-roundtrip` | log clean | 0.1d |
| D.P2.2 | CHANGELOG fajaros-x86 v3.7.1 patch release | commit + tag + Release | 0.5d |

**Total Track D: 2-3.5 days**

---

## 6. Dependency Graph

```
                           ┌──────────────────┐
                           │  A.P0 pre-flight │
                           └────────┬─────────┘
                                    │
                     ┌──────────────┼──────────────┐
                     ▼              ▼              ▼
             ┌────────────┐  ┌────────────┐  ┌────────────┐
             │  A.P1 H3   │  │  B.P0      │  │  C.P0 lit  │
             │  RoPE LUT  │  │  pre-flight│  │  sweep     │
             └─────┬──────┘  └─────┬──────┘  └─────┬──────┘
                   │               │               │
          ┌────────┴────────┐      │               │
          ▼                 ▼      ▼               ▼
    (coherent?)       (still col-│ B.P1 LLVM    C.P1 arch
    M8 EARLY           lapse)    │ bisect        design
          │                 │    │               │
          │                 ▼    ▼               ▼
          │        A.P2+A.P3+A.P4    B.P2-P5       C.P2 impl
          │        (H1/H2/H4)         │               │
          │                 │         │               ▼
          │                 ▼         │         C.P3 data
          │              M8 FULL      │               │
          │                           │               ▼
          │                           │         C.P4 train
          │                           │               │
          └────────────┐              ▼               ▼
                       │            M9          ┌────────────┐
                       ▼                         │  depends  │
               (Phase D urgency)◄────────────────┤  on B.P1  │
                       │                         └─────┬─────┘
                       ▼                               ▼
                     (if M8 reached                C.P5 kernel
                      and Phase D is               integration
                      research-only,                    │
                      continue C)                       ▼
                       │                         C.P6 validate
                       │                               │
                       │                               ▼
                       │                         C.P7 paper
                       │                               │
                       │                               ▼
                       └──────────────────────────►  M10, M11
```

**Critical path:** A.P1 → B.P1 → C.P5 → C.P7. If A.P1 closes precision,
B.P1 becomes optional for Phase D (can stay on C bypass).

---

## 7. Timeline (realistic, solo)

| Week | Activities |
|---|---|
| 1 | A.P0+A.P1 (2h) · A.P2-P4 if needed (1 day) · B.P0 (1 day) · C.P0 lit sweep (3-5 days parallel) |
| 2 | B.P1 (LLVM bisect, 2-3 days) · C.P1 arch design (3-5 days parallel) · A.P5 close-out |
| 3 | B.P2-P4 (2-3 days) · C.P2 PyTorch impl start (3-4 days) · M8 + M9 ship |
| 4 | C.P2 finish · C.P3 dataset pipeline (2-3 days parallel) · B.P5 regression |
| 5 | C.P4 initial training run (1 day GPU) · C.P4 HP tune start · D.P0 ext2 investigation |
| 6 | C.P4 HP iterations (3-5 days GPU) · D.P1+D.P2 ext2 fix + release |
| 7 | C.P4 final QAT · C.P5 kernel integration start (2-3 days) |
| 8 | C.P5 finish · C.P6 validation (2-3 days) |
| 9 | C.P7 paper draft + ablations (3-5 days) |
| 10 | C.P7 paper polish + reproduce.sh + verify + arXiv upload · M10 + M11 ship |

**Best case:** 8 weeks (if H3 closes precision and Phase D runs clean).
**Realistic:** 10-12 weeks.
**Worst case:** 14 weeks (if H1/H2/H4 investigations compound and
Phase D training needs 2-3 full retraining cycles).

---

## 8. Surprise Budget Tracking (Rule §6.8 R5)

Per-phase budgets:

| Phase | Default +25% | Higher for |
|---|---|---|
| A.P1 H3 | +25% | — |
| A.P2-P4 (H1/H2/H4) | +30% | Research hypotheses |
| B.P1 LLVM bisect | +40% | LLVM internals, high uncertainty |
| B.P2-P4 | +25% | — |
| C.P0 lit sweep | +25% | — |
| C.P1-P2 arch + impl | +30% | Novel design |
| C.P4 training | +40% | Training convergence is unpredictable |
| C.P5 integration | +30% | New kernel ops |
| C.P6-P7 | +25% | — |
| D.P1 ext2 fix | +30% | Unknown bug depth |

Commit variance tracking per Rule §6.8 R5:
`[actual Xh, est Yh, +Z%]` in commit footer.

---

## 9. Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | H3 RoPE LUT fix does NOT close pad-collapse | MEDIUM | LOW | A.P2-P4 cover remaining hypotheses; total +4-6h |
| 2 | NO hypothesis closes precision | LOW | HIGH | Path C proceeds as full research project, not "luxury" |
| 3 | LLVM O2 miscompile is a known LLVM bug with no upstream fix | MEDIUM | MEDIUM | Keep C bypass, ship `@no_vectorize` as workaround — existing pattern works |
| 4 | Custom integer attention fails to converge in training | MEDIUM | HIGH | Fall back to proven baseline (Mamba-int, RWKV-int variants) before full retraining |
| 5 | Training compute runs over budget (>2 weeks GPU) | MEDIUM | MEDIUM | Reduce model size to 10-20M params; accept worse PPL |
| 6 | Canonical benchmark puts IntLLM >5% PPL behind SmolLM | MEDIUM | PAPER-CRITICAL | Rule §6.9 R1: honest loss ships as ablation study + positioning, not claim |
| 7 | Tokenizer choice (custom vs SmolLM reuse) causes rework | LOW | MEDIUM | Decide in C.P3.1 first; reuse if feasible |
| 8 | Multi-track context-switching reduces focus | HIGH | MEDIUM | Sequential within-week scheduling; avoid daily context switches |
| 9 | V31 scope creeps (e.g., Gemma 3 270M fallback) | MEDIUM | HIGH | This plan is the scope — anything not in it requires a new plan doc |

---

## 10. Decision Gates (Rule §6.8 R6)

Mechanical checkpoints — each a committed file that blocks downstream:

| Gate | File | Blocks | Decision |
|------|------|--------|----------|
| G1 | `V31_R3_H3_DECISION.md` | A.P2+ | Continue to H1/H2/H4 or early M8 |
| G2 | `V31_R3_CLOSE.md` | B/C kernel-side priority | Precision closed? |
| G3 | `V31_FAJARLANG_P1_DECISION.md` | B.P2+ | Fix landed upstream, quarantined, or filed? |
| G4 | `V31_FAJARLANG_P4_DECISION.md` | C bypass rollback scope | Port all 10 back or keep current? |
| G5 | `FJQ_PHASE_D_P0_FINDINGS.md` | C.P1 | Arch family chosen, positioning defined |
| G6 | `FJQ_PHASE_D_P4_DECISION.md` | C.P5 | Training converged? Go or pivot? |
| G7 | `FJQ_PHASE_D_P6_DECISION.md` | C.P7 | Benchmark results — submit paper or publish ablation? |
| G8 | `FJQ_PHASE_D_P7_PREPUB.md` | v0.4.0 release | Rule §6.9 R7 prepub audit passed? |

---

## 11. Prevention Layers Shipped (Rule §6.8 R3)

Per phase, at least one regression mechanism:

| Phase | Prevention artifact |
|---|---|
| A.P5 | `test-gemma3-precision-gate` (optional, if H3 closes — bit-exact layer-0 against sim) |
| B.P5 | `test-codegen-parity` — kernel vecmat vs sim 0 ULP, blocks Fajar Lang regressions |
| C.P6 | `test-intllm-e2e` Makefile gate — model-load + coherent-output check |
| D.P2 | Expand `test-fs-roundtrip` ext2 branch: 4 → 5 invariants (+ write roundtrip) |

---

## 12. Success Criteria (per milestone)

### M8 Precision closed (end of Track A)
- [ ] 4 decision docs committed (H1-H4)
- [ ] At least one hypothesis definitively closed (fix + regression test)
- [ ] `ask hello` produces non-pad output on HEAD (if feasible)
- [ ] V31.R3 summary section in `V30_GEMMA3_P10_FOUNDATION.md`

### M9 Fajar Lang clean (end of Track B)
- [ ] LLVM O2 miscompile fixed OR upstream-filed-and-quarantined
- [ ] `@no_vectorize` attribute lands in lexer + codegen
- [ ] `test-codegen-parity` green
- [ ] GitHub Release fajar-lang v28.0.0 (first major since V27.5)
- [ ] Decision doc on C-bypass rollback scope

### M10 IntLLM proof-of-concept (end of C.P5)
- [ ] Training run converges (loss ≤ baseline)
- [ ] 10-50M param checkpoint exported as `.fjm v8` (or new v9)
- [ ] Loads on FajarOS, full forward completes without crash
- [ ] Produces non-pad-collapse output
- [ ] `test-intllm-e2e` green

### M11 FajarQuant v0.4 paper (end of C.P7)
- [ ] Paper draft (10-page MLSys + 8-page arXiv)
- [ ] Ablation table complete (≥4 variants)
- [ ] `verify_paper_tables.py --strict` exit 0
- [ ] `reproduce.sh --smoke` + `--full` both green
- [ ] arXiv uploaded
- [ ] FajarQuant v0.4.0 GitHub Release

### M12 ext2 write closed (end of Track D)
- [ ] `ext2-write` works on freshly-mkfs'd disk
- [ ] `test-fs-roundtrip` ext2 branch 5 PASS (up from 4)
- [ ] fajaros-x86 v3.7.1 patch release

---

## 13. Multi-repo state check (Rule §6.8 R8)

Pre-V31-execution baseline (2026-04-20):

```
Fajar Lang : ahead=0, dirty=3 (CLAUDE.md M + 3 untracked examples)
fajaros-x86: ahead=0, dirty=0 (clean)
fajarquant : ahead=0, dirty=1 (diag_gate_proj.py untracked)
```

Before starting execution (before A.P0):
- Commit the dirty working-tree items to avoid drift
- Final `git status -sb` across all three repos = clean

---

## 14. Self-check (§6.8 Plan Hygiene 8/8)

- [x] Pre-flight audit (P0) exists for each Phase? — A.P0, B.P0, C.P0, D.P0
- [x] Every task has a runnable verification command?
- [x] At least one prevention mechanism per Track? — see §11
- [x] Agent-produced numbers cross-checked? — effort estimates from V30 actual data (Track 3+4)
- [x] Effort variance tagged in commit messages? — per §8 budget table
- [x] Decisions are committed files? — 8 gates G1-G8 in §10
- [x] Internal doc fixes audited for public-artifact drift? — CHANGELOG + GitHub Release scheduled in M-milestones
- [x] Multi-repo state check run before starting? — §13

**8/8 = ship.**

---

## 15. Research Integrity self-check (§6.9 — for Track C)

- [x] R1 Canonical protocol — C.P6.1 uses Wikitext-103 + OWT hold-out
- [x] R2 Literature review — C.P0 surveys ≥10 papers
- [x] R3 Baseline parity — C.P6.2 uses full-feature SmolLM + TinyLlama
- [x] R4 Calibrated not per-chunk — QAT uses full-corpus calibration in C.P4.3
- [x] R5 Outlier handling — inherited from FajarQuant v3.1 (sparse top-K preserved)
- [x] R6 Algorithmic validation precedes paper — C.P6 runs before C.P7 draft
- [x] R7 Prepub audit gate — G8 `verify_paper_tables.py --strict`

**7/7 = publishable when closed.**

---

## 16. Open questions (to resolve before execution)

1. **Tokenizer reuse vs custom (C.P3.1)** — saves 2 days + leverages
   HF BPE tooling if we reuse SmolLM's 49K vocab. Custom gives us
   a truly-from-scratch claim but adds cost. Default: reuse.

2. **Arch family final choice (C.P1.1)** — RWKV-7 integer variant
   vs linear-attention + ALiBi vs Mamba-int. C.P0 lit sweep
   outputs the decision. Preferences should be recorded in this
   plan once sweep completes.

3. **Training corpus (C.P3.2)** — OWT subset is fast to stage;
   RedPajama-v2 is better quality. Default: start OWT 1B tokens,
   rerun on RP-v2 for final publishable numbers.

4. **Paper venue** — MLSys 2027 call-for-papers, NeurIPS MLSys
   Workshop 2026, or arXiv+workshop combo. Decide before C.P7.1.

5. **When to push C→FajarOS integration** — before or after
   paper submission? Default: push integration in C.P5 as the
   *validation of arch claim*, paper submission in C.P7.5.

---

## 17. Commit after plan approval

After this plan is reviewed + agreed:

1. Commit this file to fajar-lang main
2. Update `CLAUDE.md` §3 with V31 program row
3. Update `MEMORY.md` Current Status to point at this plan
4. Start execution at A.P0

---

*Plan status: DRAFT ready for review. Recommendation: execute
 A.P0+A.P1 first (≤3h), checkpoint, then launch B.P0 + C.P0 in
 parallel. M8 is reachable within 1 week, unblocks all downstream
 decisions.*
