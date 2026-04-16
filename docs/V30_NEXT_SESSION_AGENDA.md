# V30 Next Session Agenda — Plan Drafting + Skill Creation

**Date prepared:** 2026-04-16 (end of V29.P2 session)
**Purpose:** handoff document so the next session starts with the
4 open tracks fully enumerated, the plan-drafting pattern pinned,
and the skill-creation scope scoped.

**🟢 STATUS UPDATE 2026-04-16 (end of V29.P3 + V29.P3.P6 session):**
**Track 1 ✅ COMPLETE — V26 B4.2 security triple 3/3 SHIPPED.**
17 commits across fajaros-x86 + fajar-lang + GitHub Release v3.5.0
"Security Triple" published. 3 tracks remain (2, 3, 4). Full rollup
in §Track 1 below + `CLAUDE.md` §3 V29.P3 + V29.P3.P6 rows.

## Context for Next Session

Today's session shipped V29.P1 (compiler enhancement, 4 phases) +
V29.P2 (SMEP + VFS_TESTS, 8 steps) across fajaros-x86 + fajar-lang.
13-layer prevention chain in place. 35/35 kernel tests pass.
Multi-repo clean, everything pushed. See `V29_P1_COMPILER_ENHANCEMENT_PLAN.md`
for the plan-doc pattern that has been working well.

**Next session's goal: draft comprehensive plan files (one per open
track) AND create powerful Claude Code skills that automate the
repeatable parts of each track.**

## 4 Open Tracks — Plans to Draft

Each track gets its own plan file in the established V29.P1 pattern
(self-check 8/8 Plan Hygiene rules, phased approach with runnable
verification, surprise budget tracking, prevention layers, decision
gates). Estimated plan-drafting time per track: ~30 min.

### Track 1: V29.P3.SMAP — ✅ COMPLETE (2026-04-16)

**Entry doc:** `fajaros-x86/docs/V29_P2_SMEP_STEP4_BISECT.md` (historical)
**Status:** V26 B4.2 SMEP+SMAP+NX all enabled; GitHub Release v3.5.0
published 2026-04-16.

**Outcome summary:**
- V29.P3 (SMAP closure): H2 matched (non-leaf USER at PML4[0]+PDPT[0]
  via SMAP AND-chain). Fix: `690124b` extended
  `strip_user_from_kernel_identity()`. SMEP+SMAP shipped in `f2dd682`.
  8 commits, 2.4h (-9% vs budget).
- V29.P3.P6 (NX closure, same session): H3 matched via P0 static
  analysis alone (P1 walker skipped, saved 0.55h). Root cause:
  kernel `.text` spans 0x101000-0x248297 (PD[0] AND PD[1]); loop
  `pd_idx = 1` wrongly NX-marked PD[1] → silent triple-fault
  (no EXC marker — handler page also NX). Fix `540743b`:
  single-line `pd_idx = 1 → pd_idx = 2` in `security.fj:236`.
  5 commits, 1.48h (-35% vs original budget).

**Plans + docs shipped:**
- `V29_P3_SMAP_PLAN.md`, `V29_P3_SMAP_FINDINGS.md`, `V29_P3_SMAP_DECISION.md`
- `V29_P3_P6_NX_PLAN.md`, `V29_P3_P6_NX_FINDINGS.md`, `V29_P3_P6_NX_DECISION.md`
- CHANGELOG v3.5.0 "Security Triple" narrative
- CLAUDE.md §3 V29.P3 + V29.P3.P6 rows
- GitHub Release: https://github.com/fajarkraton/fajaros-x86/releases/tag/v3.5.0

**Prevention shipped:** `make test-security-triple-regression` — 6-invariant
Makefile gate (PTE_LEAKS=0, PTE_LEAKS_FULL=0, no PLKNL, no EXC/PANIC,
nova>, NX_ENFORCED=0x800). `test-smap-regression` retained as alias.

**Skill created + matured:** `fajaros-bisect` at `~/.claude/skills/`
— battle-tested on 4 real configs during P0 execution. 2 sed bugs
found + fixed during first use (delimiter conflict + empty-pattern
group reference). `--also-comment <regex>` flag added for bidirectional
toggles.

**Remaining non-blocking** (tracked as `TODO(P3, V30+)` inline):
- `protect_kernel_data()` dead code cleanup
- Dynamic `__kernel_end` symbol (current headroom: 1.72 MB before
  `pd_idx = 2` needs re-evaluation)

**Wall clock total:** ~3.9h across both sub-tracks, 17 commits
pushed (13 code/plan/doc in fajaros-x86 + 2 CLAUDE.md in fajar-lang
+ this status update + GitHub Release).

### Track 2: V28.1 Gemma 3 1B Full Sprint

**Entry doc:** `fajaros-x86/docs/V28_1_NEXT_STEPS.md`
**Goal:** port Gemma 3 1B with complete architecture support (GQA,
dual-theta RoPE, sliding window, 262K vocab, 32K context) producing
coherent output (not just stable multilingual tokens).

**Plan should cover (4-week / 160h estimate):**
- Week 1: weights export + disk image (release note: `disk_v8.img`
  already exists from earlier work; may just need updated export)
- Week 2: GQA (4 Q : 1 KV) + dual-theta RoPE (local 10K, global 1M)
- Week 3: sliding window attention (512-token local) + 262K vocab
  lookup + 32K KV cache
- Week 4: numerical validation layer-by-layer vs HF reference +
  perplexity benchmark + `ask` coherence gate

**Plan hygiene angle:** this is a big-bet phase. Must include
EXPLICIT surprise budget +40% (high uncertainty on GQA at 2-bit
+ RoPE numerical drift + KV cache sizing) and mid-sprint decision
gates (after Week 2: keep going OR fall back to gemma-3-270m;
after Week 3: ship as research artifact even if coherence not
hit).

**Skill candidates:**
- `hf-model-export` — driver for HuggingFace → .fjm v8 export
  pipeline. Parametrize model ID, output path, quantization mode.
  Currently lives in `fajaros-x86/scripts/export_gemma3_v8.py` but
  each run takes manual config.
- `kernel-infer-bench` — boot QEMU + NVMe disk + run `ask <prompt>`
  + capture token count + time + EXC:13 state. Comparable runs
  across kernel revisions for A/B testing the GQA/RoPE work.

### Track 3: V8 Coherence Gap — Python Reference Simulator

**Entry doc:** `fajaros-x86/docs/V28_2_CLOSED_PARTIAL.md` + this
session's `V28_5_RETEST.md`
**Goal:** build a Python reference simulator that mirrors kernel
integer math exactly, run on Gemma 3 layer 0, find the arithmetic
divergence that produces pad=0 argmax.

**Plan should cover:**
- P0 pre-flight: document exact kernel hot-path sequence
  (km_vecmat_packed_v8 → rmsnorm → per-head scaling → argmax)
- P1 scaffold Python simulator with same int16/int32 truncation
  semantics
- P2 run both on single layer with known input, emit per-step
  intermediate values
- P3 diff analysis — first step where Python and kernel disagree
  is the bug
- P4 fix kernel (or algorithm)
- P5 retest V28.5 multilingual scenario with fix

**Online research needed:** cross-check with published 4-bit
quantization implementations (KIVI, Quarot, AWQ) for their
integer-math conventions. Particularly around per-head scaling,
accumulator width, and rounding policy. The fajarquant paper
review will also have clues.

**Skill candidates:**
- `algo-numerical-diff` — given two implementations (kernel log +
  Python log) of the same algorithm, diff intermediate values and
  report first divergence + tolerance analysis. Reusable beyond
  this track.
- `paper-claim-verify` — automates "run reference impl, compare to
  paper-claimed values, report delta". Would help FajarQuant paper
  claims too.

### Track 4: ext2 + FAT32 Write Tests — Disk Harness

**Entry doc:** `V29.P2.VFS_TESTS` commit message (scope pin:
"ext2/FAT32 tests need disk-backed mount which isn't set up at
boot-test time")
**Goal:** build test harness that creates pre-populated disk
images, mounts them in QEMU, runs ext2/FAT32 write tests as part
of `make test-smep-regression` (or similar gate).

**Plan should cover:**
- P0 survey existing disk.img handling in Makefile
  (run-nvme target + `$(QEMU_NVME)` variable)
- P1 test disk image builder script (build ext2 + FAT32 images
  with known content, store in `build/test-disks/`)
- P2 extend `test-smep-regression` target (or add
  `test-fs-roundtrip`) to attach the test disk
- P3 kernel tests: `test_ext2_write_roundtrip`,
  `test_fat32_write_roundtrip`, `test_disk_mount_unmount`
- P4 CI wiring

**Estimated effort:** 4-6h.

**Skill candidate:** `disk-image-builder` — generates minimal
ext2 / FAT32 / RamFS-format disk images with known files + known
content. Driven by a manifest file. Useful for ANY filesystem
test, not just this track.

## Skills to Create (Claude Code-side)

Skills live in `~/.claude/skills/<name>/` with a SKILL.md
metadata file and any supporting scripts/config. They are
project-agnostic but can be scoped to specific projects.

### Proposed skills (ranked by reusability)

| Name | Scope | Useful for |
|------|-------|-----------|
| `fajaros-bisect` | kernel flag toggle + boot-grep | Track 1 + future CR4/MSR work |
| `kernel-infer-bench` | QEMU + NVMe + infer + capture | Track 2 + any LLM-kernel benchmark |
| `algo-numerical-diff` | dual-impl step diff + first-divergence | Track 3 + FajarQuant paper review |
| `disk-image-builder` | manifest → ext2/FAT32/RamFS images | Track 4 + future fs work |
| `hf-model-export` | HF → .fjm quantized export | Track 2 + future model ports |
| `paper-claim-verify` | paper numbers vs runtime impl diff | FajarQuant + future ML papers |

**Skill format pattern to use** (from observed `/loop`, `/schedule`,
`/claude-api` builtins):
```
~/.claude/skills/<name>/
├── SKILL.md          # metadata: when to invoke, params, guardrails
├── run.sh            # main executable (or .py / .rs)
└── README.md         # user-facing doc
```

Pre-work for next session: the session should start by loading
CLAUDE.md skill list + this agenda, then ask user which of the 4
tracks + which of the 6 skills are in-scope for that session's
time budget. Skills should be created AS NEEDED while executing
tracks, not all up-front.

## Online Research Triggers (per CLAUDE.md §6.9 Rule 2)

Tracks that clearly need online research:

- **Track 3 (V8 Coherence):** literature sweep on 4-bit KV cache
  quantization integer-math conventions (KIVI, Quarot, AWQ, SKVQ).
  Minimum 8-10 recent papers per Rule 2. Position FajarQuant's
  per-head approach explicitly.
- **Track 1 (SMAP double fault):** Intel SDM Volume 3A §§ 4.6
  (Access Rights) + 4.6.1.1 (SMAP), Linux kernel's SMAP enable
  sequence (arch/x86/kernel/cpu/common.c), any Intel errata
  affecting QEMU -cpu host on i9-14900HX.
- **Track 2 (Gemma 3 port):** HuggingFace Gemma 3 architecture doc,
  Google's gemma.cpp reference for GQA + dual-theta RoPE + sliding
  window implementation detail.
- **Track 4 (disk harness):** no online research needed;
  straightforward tooling work.

## Next Session Opening Checklist

When the next session starts:

1. Load CLAUDE.md (auto) + this agenda file
2. Run `git status` in all 3 repos to confirm clean state
3. Ask user: which track(s) + how much time budget?
4. Draft plan file for chosen track(s) following V29.P1 pattern
5. Create skills as needed during execution, not pre-emptively

## Effort Estimates for Plan Drafting

| Deliverable | Est |
|-------------|----:|
| V29.P3.SMAP plan file | 0.4h |
| V28.1 Gemma 3 sprint plan file | 0.6h (bigger scope) |
| V8 Coherence Python simulator plan file | 0.5h |
| Track 4 disk harness plan file | 0.3h |
| Skill: `fajaros-bisect` + docs | 0.5h |
| Skill: `kernel-infer-bench` + docs | 0.5h |
| Skill: `algo-numerical-diff` + docs | 0.7h |
| Skill: `disk-image-builder` + docs | 0.5h |
| Skill: `hf-model-export` + docs | 0.5h |
| Skill: `paper-claim-verify` + docs | 0.6h |
| **Total if everything** | **5.1h** |

Realistic next-session scope: 2 plan files + 1-2 skills = ~2h.
Skills that directly unblock tracks should be prioritized.

## Quality Gates for Plans (per V29.P1 pattern)

Every plan file must self-check all 8 Plan Hygiene rules:

- [ ] 1. Pre-flight audit mandatory (P0 sub-phase)
- [ ] 2. Verification commands runnable (literal shell commands)
- [ ] 3. Prevention layer per phase (hook/CI/rule)
- [ ] 4. Multi-agent cross-check mandatory for numerical claims
- [ ] 5. Surprise budget +25% minimum, tracked per commit
- [ ] 6. Decision gates are committed files
- [ ] 7. Public-facing artifact sync on doc fixes
- [ ] 8. Multi-repo state check before any phase starts

Plans that don't pass self-check get rejected at write time.

## Sign-off

Drafted by **Claude Opus 4.6** on **2026-04-16** as the final
deliverable of the V29.P2 session. Next session resumes from this
file as the entry point.
