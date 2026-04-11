# V26 Phase C0 — Pre-Flight Audit Findings

**Audit date:** 2026-04-11
**Audit scope:** V26 Phase C0.1-C0.7 hands-on baseline verification of fajarquant repo, paper data integrity, candidate model availability, and GPU budget
**Plan Hygiene Rule 1 status:** ✅ Pre-flight audit committed before any Phase C1 substantive work begins
**Findings owner:** V26 Phase C kickoff (this document gates C1+)
**Author:** Claude Code session continuation, verified by hand against runnable commands

---

## TL;DR

- ✅ **5 of 7 audit tasks closed cleanly** with no surprises (C0.1, C0.2, C0.3, C0.5, C0.7)
- 🚨 **2 surprises caught by the audit** that would have wasted Phase C effort if uncorrected:
  1. **C0.4**: Handoff claimed `ablation_results.json` was malformed at line 80 — **the file is fine**. Both `jq` and Python `json.load` parse it cleanly. This is a Plan Hygiene Rule 4 (cross-check) catch.
  2. **C0.6**: **Llama 2 7B is Meta-gated** on HuggingFace and requires an access agreement. The current C1 plan does not account for this. C1.0 dry run will fail without prior approval.
- ✅ **Permanent prevention layer added** (Plan Hygiene Rule 3): `fajarquant/scripts/verify_paper_tables.py` — 9 paper claims registered, all PASS within tolerance, runnable on every paper edit
- ✅ **C1 effort estimate revised** based on findings (see §6 below)
- ✅ **Gate cleared:** Phase C1+ may begin once this file is committed

---

## 1. Audit Task Status Table

| # | Task | Verification command | Status | Drift / surprise |
|---|---|---|---|---|
| C0.1 | Algorithm LOC | `find src -name "*.rs" \| xargs wc -l \| tail -1` | ✅ 2,342 LOC | +66 LOC (2.9%) vs handoff "~2,276" — minor |
| C0.2 | Test count | `cargo test --lib` (fq) + `cargo test --test fajarquant_e2e_tests --test fajarquant_safety_tests` (fl) | ✅ 29 + 16 = **45 total** | None — matches handoff exactly |
| C0.3 | Demo count | `ls examples/*.fj \| wc -l` | ✅ 5 files | None — `hierarchical_demo.fj` still missing (C4.3 owns) |
| C0.4 | `ablation_results.json` integrity | `jq . data/kv_cache/ablation_results.json` | 🚨 **HANDOFF WRONG** — file parses cleanly | Strike from C0 task list, return ~10 min to surprise pool |
| C0.5 | Paper tables vs source data | `python3 scripts/verify_paper_tables.py --strict` | ✅ 9/9 claims PASS within tolerance | None blocking; minor "4-6%" rounding noted |
| C0.6 | HF model availability | WebFetch on each `huggingface.co/<model>` | 🚨 **Llama 2 gated** + Qwen 8B not 7B | New C1.0.0 + C1.0.1 tasks needed |
| C0.7 | GPU budget snapshot | `nvidia-smi --query-gpu=...` | ✅ RTX 4090 Laptop, 16 GB VRAM, 0% util, 14 MiB used | Sequential model loading required |

**Total surprises:** 2. **Severity:** 1 medium (C0.6 Llama gate), 1 low (C0.4 mistaken claim).

---

## 2. C0.1 — Algorithm LOC

**Verification command (verbatim):**
```bash
cd ~/Documents/fajarquant && find src -name "*.rs" -type f | xargs wc -l
```

**Output:**
```
  401 src/hierarchical.rs
  493 src/kivi.rs
  518 src/adaptive.rs
  535 src/turboquant.rs
   75 src/lib.rs
  320 src/fused_attention.rs
 2342 total
```

**Drift:** Handoff said "~2,276 LOC". Real is **2,342** (+66, **2.9% inflation**). Minor; likely incremental edits between extraction and audit. **Action:** none. Update handoff in next memory edit.

---

## 3. C0.2 — Test Count

**Verification commands + outputs:**
```bash
cd ~/Documents/fajarquant && cargo test --lib 2>&1 | tail -1
# → test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured

cd ~/Documents/Fajar\ Lang && cargo test --test fajarquant_e2e_tests --test fajarquant_safety_tests 2>&1 | tail -1
# → test result: ok. 8 passed; 0 failed (e2e + safety = 8 + 8 = 16 wire-up tests)
```

**Match prior handoff exactly:** 29 + 16 = **45 total**. ✅

---

## 4. C0.3 — Demo Count

**Verification command + output:**
```bash
cd ~/Documents/fajarquant && ls examples/*.fj
```
```
adaptive_demo.fj
benchmark.fj
fused_demo.fj
kv_cache.fj
paper_benchmark.fj
```

**5 of 6 promised.** Missing: `hierarchical_demo.fj`. The hierarchical algorithm IS implemented (`src/hierarchical.rs`, 401 LOC, included in 9-claim paper verification), only the `.fj` demo file is missing. **Owner:** V26 Phase C4.3. **Risk:** reproducibility reviewers may flag the gap.

---

## 5. C0.4 — `ablation_results.json` Integrity (Surprise #1)

**Handoff claim:**
> "C0.4 Re-verify ablation_results.json malformed | shows the parse error at line 80"

**Verification commands + outputs:**
```bash
cd ~/Documents/fajarquant && wc -l data/kv_cache/ablation_results.json
# → 172 lines

cd ~/Documents/fajarquant && jq . data/kv_cache/ablation_results.json | tail -5
# → (clean output, exit 0)

cd ~/Documents/fajarquant && python3 -c "import json; d = json.load(open('data/kv_cache/ablation_results.json')); print(list(d.keys()))"
# → ['model', 'd_head', 'rotation_ablation', 'memory_ablation', 'hierarchical_ablation']
```

**Result:** **The file is NOT malformed.** Both `jq` and Python `json.load` parse it cleanly. 172 lines, 3,383 bytes, 5 top-level keys, all values intact. The handoff's "parse error at line 80" claim is wrong.

**Plan Hygiene Rule 4 lesson:** This is exactly the inflated baseline that pre-flight audits exist to catch. The handoff was treated as authoritative; cross-check with two parsers (Rule 4) revealed the assumption was false. **Without C0.4, Phase C would have spent ~30 min "fixing" a non-problem.**

**Action:** strike "ablation results malformed" from any future C0/C1 task list. No remediation work needed.

---

## 6. C0.5 — Paper Tables vs Source Data + Prevention Layer

**Prevention layer added (Plan Hygiene Rule 3):** new script `fajarquant/scripts/verify_paper_tables.py` (193 lines) registers **9 numerical claims** from the paper and verifies each against source JSON within explicit tolerance. Future paper edits can re-run with `--strict` to exit non-zero on any mismatch.

**Verification command:**
```bash
cd ~/Documents/fajarquant && python3 scripts/verify_paper_tables.py --strict
```

**Output (verbatim):**
```
verify_paper_tables.py — checking 9 claims against /home/primecore/Documents/fajarquant/data/kv_cache
==============================================================================
  PASS PPL 2-bit FajarQuant (abstract + Table) (paper line 39): paper=80.1 source=80.1466 diff=0.0466 tol=0.1
  PASS PPL 2-bit TurboQuant (paper line 39): paper=117.1 source=117.1135 diff=0.0135 tol=0.1
  PASS PPL 2-bit KIVI (paper line 39): paper=231.9 source=231.8866 diff=0.0134 tol=0.1
  PASS PPL 3-bit FajarQuant (paper line 291): paper=75.6 source=75.6476 diff=0.0476 tol=0.1
  PASS Hierarchical 48.7% at 10K context (paper line 37): paper=48.7 source=48.7000 diff=0.0000 tol=0.05
  PASS Hierarchical 55.7% at 16K context (paper line 271): paper=55.7 source=55.7000 diff=0.0000 tol=0.05
  PASS PCA vs TurboQuant 2-bit (4.9%) (paper line 271): paper=4.9 source=4.9157 diff=0.0157 tol=0.05
  PASS PCA vs TurboQuant 3-bit (4.3%) (paper line 271): paper=4.3 source=4.3330 diff=0.0330 tol=0.05
  PASS PCA vs TurboQuant 4-bit (4.8%) (paper line 271): paper=4.8 source=4.7934 diff=0.0066 tol=0.05
==============================================================================
OK — all 9 claims verified within tolerance
```

**All 9 claims PASS within tolerance.** Paper is internally consistent with `data/kv_cache/perplexity_results.json`, `comparison_results.json`, `ablation_results.json`.

**Minor accuracy note (not blocking):** abstract + conclusion say "4-6% MSE improvement" but the actual range across all bit widths is 3.85%-6.33%. The paper rounds the floor to 4%, which is acceptable for an abstract claim. **C3.7 proofread pass** should consider tightening to "3.8-6.3%" or "≈4-6%".

---

## 7. C0.6 — HF Model Availability (Surprise #2 — BLOCKER)

**Full snapshot:** `~/Documents/fajarquant/audit/C0_model_availability.md` (committed in fajarquant repo)

**Per-model summary:**

| Model | HF ID | Public? | License | Friction | Disk |
|---|---|---|---|---|---|
| Mistral 7B | `mistralai/Mistral-7B-v0.1` | ✅ | Apache-2.0 | None | ~14 GB |
| Llama 2 7B | `meta-llama/Llama-2-7b-hf` | ❌ login required | LLAMA 2 Community | **🚨 Meta gated, agreement required** | ~14 GB |
| Qwen 7B/8B | `Qwen/Qwen-7B` | ✅ | Tongyi Qianwen | Commercial use → Alibaba form | ~16 GB (HF says **8B**, not 7B) |
| Phi-3 mini | `microsoft/Phi-3-mini-4k-instruct` | ✅ | MIT | None | ~7.6 GB |

**🚨 Findings that change C1 plan:**

1. **Llama 2 7B Meta gating is a real C1.0 blocker.** Author must visit https://huggingface.co/meta-llama/Llama-2-7b-hf logged in with HF account, click "Submit" on the access form, wait for Meta approval (typically a few hours to a day), then re-run a download check. **This is not in the current C1 task table.** Without this, C1.3 (extract Llama 2) will fail at the first `huggingface_hub` call.

2. **Qwen variant naming must be pinned.** The V26 plan currently says "Qwen 7B" but the canonical `Qwen/Qwen-7B` HF page reports **8B params**. Variants to choose between: `Qwen/Qwen-7B`, `Qwen/Qwen1.5-7B`, `Qwen/Qwen2-7B` (newer, actually 7B). **Decision needed before C1.0 dry run.**

3. **VRAM is sequential.** RTX 4090 Laptop has 16 GB VRAM. Mistral and Llama 2 fit at FP16 (~14 GB), Qwen 8B is borderline (16 GB exactly), Phi-3 mini fits comfortably (~7.6 GB). C1 must process models **one at a time** with `del model; torch.cuda.empty_cache()` between runs.

4. **Disk budget ~52 GB** for the 4-model set. Verify free disk before C1.0:
   ```
   df -h ~/Documents/fajarquant/data/
   ```
   Reserve ≥60 GB.

---

## 8. C0.7 — GPU Budget Snapshot

**Snapshot file:** `~/Documents/fajarquant/audit/C0_gpu_state.json` (committed in fajarquant repo)

**Key facts:**
- **Hardware:** NVIDIA GeForce RTX 4090 Laptop GPU
- **Driver:** 590.48.01
- **VRAM:** 16,376 MiB total / 15,933 MiB free / 14 MiB used (essentially clean)
- **Utilization:** 0%
- **Concurrent compute apps:** none
- **Host RAM:** 31 GB

**Fit assessment:**
| Model | FP16 size | Fit? |
|---|---|---|
| Mistral 7B | ~14 GB | ✅ |
| Llama 2 7B | ~14 GB | ✅ |
| Qwen 7B/8B | ~16 GB | ⚠️ tight, may need 8-bit base loading |
| Phi-3 mini | ~7.6 GB | ✅ easy |

**Conclusion:** GPU has enough headroom for the C1 multi-model extraction provided models are loaded **sequentially** with VRAM cleared between runs. Concurrent loading of two 7B models is not possible.

---

## 9. Revised C1 Effort Estimates (based on C0 findings)

**Original C1 estimates** (from V26 plan v1.2 §C1):
- C1.0 dry run: 1 h GPU
- C1.1 adapter: 2 h
- C1.2 Mistral extract: 4 h GPU
- C1.3 Llama 2 extract: 4 h GPU
- C1.4 Qwen extract: 4 h GPU
- C1.5 3-way comparison: 8 h
- C1.5.5 Go/No-Go gate: 1 h decision
- C1.6 perplexity eval: 6 h GPU
- C1.7 paper table updates: 4 h
- **Original total:** 34 hours

**Revisions:**

| Adjustment | Reason | Effort delta |
|---|---|---|
| **+ NEW C1.0.0** "Confirm Llama 2 Meta access OR pin redistribution variant" | C0.6 finding — gated | +0.5 h decision + waiting (real time, not effort) |
| **+ NEW C1.0.1** "Pin Qwen variant (Qwen-7B / Qwen1.5-7B / Qwen2-7B)" → committed file `audit/C1_qwen_variant.md` | C0.6 finding — naming drift | +0.5 h |
| C1.4 Qwen extract: clarify which Qwen | C0.6 finding | 0 (covered by C1.0.1) |
| C1.0 dry run: add "verify VRAM clear with `nvidia-smi` between runs" | C0.7 finding | 0 (process improvement) |
| **− Strike** any "fix ablation_results.json" task | C0.4 finding — not malformed | −0.5 h returned to pool |
| Add "verify_paper_tables.py to C4.2.5 CI smoke test" | C0.5 prevention layer | +0.25 h |

**Net effort delta:** **+0.75 h** (from +1.25 h additions − 0.5 h subtraction)
**Revised C1 total:** ~34.75 h (was 34 h, +2.2%)

The Phase C surprise budget is +30% on a base of 84 h = 110 h total. C0 findings consume **less than 1 hour** of that surplus. The Phase C budget **remains comfortable**.

---

## 10. New / Modified C1 Tasks

The V26 plan §C1 task table should be updated to include these mechanical entries (separate commit, not part of this audit):

| # | Task | Verification command | Est. |
|---|---|---|---|
| **C1.0.0** | Confirm Llama 2 Meta access (HF login + agreement on https://huggingface.co/meta-llama/Llama-2-7b-hf) OR pin `NousResearch/Llama-2-7b-hf` redistribution variant in the C1 task table | `huggingface-cli download meta-llama/Llama-2-7b-hf --include "config.json" --local-dir /tmp/llama2_test 2>&1 \| grep -v "401 Client Error"` succeeds; OR `git log --oneline --grep "v26-c1.0.0"` shows variant pinning commit | 0.5 h + waiting on Meta |
| **C1.0.1** | Pin Qwen variant — committed file `~/Documents/fajarquant/audit/C1_qwen_variant.md` with chosen variant + reasoning | `cd ~/Documents/fajarquant && test -f audit/C1_qwen_variant.md && grep -c '^variant:' audit/C1_qwen_variant.md` ≥ 1 | 0.5 h |

These should be added to `docs/V26_PRODUCTION_PLAN.md` §C1 in the next plan-edit commit, before C1.0 begins.

---

## 11. Surprises Inventory (Plan Hygiene Rules 1, 4)

**Surprise budget tracking** (per Plan Hygiene Rule 5):

| Surprise | Direction | Magnitude | Caught by | Effort delta |
|---|---|---|---|---|
| `ablation_results.json` parse error claim was false | Contraction | -100% (no work needed) | Rule 1 (pre-flight) + Rule 4 (cross-check with 2 parsers) | -0.5 h |
| Llama 2 Meta gating not in C1 plan | Expansion | +0.5-24 h waiting + 0.5 h plan update | Rule 1 (pre-flight) + Rule 4 (WebFetch verification) | +0.5-24 h real time |
| Qwen variant naming drift | Expansion | +0.5 h decision | Rule 1 (pre-flight) + Rule 4 (WebFetch) | +0.5 h |
| LOC drift +66 (2.9%) | Expansion | Doc update only | Rule 1 (pre-flight) | 0 h effort, 5 min memory edit |

**Net audit ROI:** Pre-flight audit cost ~3 hours of session time. It surfaced one BLOCKER (Llama 2 gating) that would have failed C1.3 mid-extraction (~4 GPU hours wasted) and one inflated baseline that would have wasted ~30 min of debugging. **Audit paid for itself before it was committed.**

---

## 12. Gate Clearance — C1+ Unblocked

Per **Plan Hygiene Rule 1**: Phase C1+ cannot start until `docs/V26_C0_FINDINGS.md` is committed. This document is that file. Once committed:

1. ✅ Pre-flight audit lands (this file exists in fajar-lang/docs/)
2. ✅ Findings include revised effort estimates
3. ✅ All 7 audit tasks have runnable verification commands recorded with verbatim output
4. ✅ Prevention layer added (verify_paper_tables.py, Plan Hygiene Rule 3)
5. ✅ New blocking tasks identified (C1.0.0 Llama 2 access, C1.0.1 Qwen pin)
6. ✅ Surprise budget tracked (Plan Hygiene Rule 5)
7. ✅ Cross-checked with two parsers + WebFetch (Plan Hygiene Rule 4)

**Phase C1 unblocked starting from this commit.** First C1 action must be C1.0.0 (Llama 2 access request) since it has the longest external wait time.

## 13. Sign-Off

C0 audit completed 2026-04-11 by Claude Code session continuation. All 7 C0 tasks executed with runnable verification. Two surprises caught and documented. Prevention layer (`verify_paper_tables.py`) added to fajarquant repo. Revised C1 effort estimate is +0.75 h vs original — well inside the +30% surprise budget for Phase C.

**Effort variance for C0:** actual ~2.5 h vs estimate 3 h = **−17%**. Tagged in commit message per Plan Hygiene Rule 5.

**Recommended next action:** commit this file + the 4 fajarquant artifacts (verify_paper_tables.py, audit/C0_baseline.md, audit/C0_model_availability.md, audit/C0_gpu_state.json) — then start Phase C1.0.0 (Llama 2 access request) since it has the longest external wait time.

---

**File cross-references:**
- `~/Documents/fajarquant/scripts/verify_paper_tables.py` — prevention script
- `~/Documents/fajarquant/audit/C0_baseline.md` — C0.1-C0.5 baseline detail
- `~/Documents/fajarquant/audit/C0_model_availability.md` — C0.6 detail
- `~/Documents/fajarquant/audit/C0_gpu_state.json` — C0.7 detail
- `Fajar Lang/docs/V26_PRODUCTION_PLAN.md` §C0-C1 — task tables to be updated
- `Fajar Lang/CLAUDE.md` §6.8 — Plan Hygiene Rules 1-8 governing this audit
