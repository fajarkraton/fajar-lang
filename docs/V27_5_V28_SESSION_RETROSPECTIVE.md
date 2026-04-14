# V27.5 + V28 Foundation — Session Retrospective

**Date:** 2026-04-14
**Duration:** ~7h actual
**Estimated:** ~200h
**Variance:** -97%

## Executive Summary

Single session shipped Fajar Lang **v27.5.0** (Compiler Prep) + FajarOS **v3.3.0** (V28 Foundation). Deep hands-on audits revealed that the planning documents systematically overestimated remaining work by 25-50×, because prior sessions had already implemented most features while plans were stale.

**Core finding:** Reading code before writing plans prevents 97% of wasted effort.

## Shipped Releases

| Product | From | To | Gap Closed |
|---------|------|-----|-----------|
| Fajar Lang | v27.0.0 | **v27.5.0** | Compiler prep for V28-V33 |
| FajarOS | v3.2.0 | **v3.3.0** | V28 tensor pool + audit |
| FajarQuant | v0.3.0 | v0.3.0 | (no change) |

All 3 repos synced at 0 unpushed commits.

## V27.5 "Compiler Prep" — 7 Phases

All phases done in 5.6h actual vs 196h estimated.

| Phase | Task | Commit | Actual |
|-------|------|--------|--------|
| P0 | Pre-flight audit (12 checks) | `b22dd34` | 0.3h |
| P1.1 | Kernel tensor 16→128 | `f8fabaa` | 0.2h |
| P1.2 | AI scheduler builtins | `4959f47` | 0.5h |
| P1.3a | @interrupt AOT wiring | `3a4a803` | 0.7h |
| P1.3b | x86_64 ISR wrapper | `478ca16` | 0.5h |
| P1.4 | VESA framebuffer MMIO | `5e15701` | 0.4h |
| P2 | IPC service stub generator | `c3a9663` | 0.5h |
| P3 | @app + @host annotations | `6efef49` | 0.4h |
| P4.1 | Refinement param checking | `ab3dd2b` | 0.5h |
| P4.2 | Cap<T> capability type | `f972061` | 0.8h |
| P5 | Integration tests + CI gate | `e8eb02f` | 0.5h |

## V28 Foundation — 2 Sub-Phases

| Phase | Task | Commit | Actual |
|-------|------|--------|--------|
| V28.0 | Pre-flight audit | `6507c4f` | 0.3h |
| V28.1 | Gemma tensor pool 1280-dim | `b5aa70e` | 0.5h |

## The Audit Correction Pattern

Six of ten initially-reported Fajar Lang gaps were already implemented:

| Reported Gap | Reality | Evidence |
|-------------|---------|----------|
| Result<T,E> + ? operator | ✅ Complete + 5 tests | `builtins.rs:9233` |
| Module file resolver | ✅ Complete | `builtins.rs:9526` |
| Borrow checker | ✅ Complete (lite) | `borrow_lite.rs` 1,253 LOC |
| Effects system | ✅ Complete + 8 error codes | `effects.rs` 2,306 LOC |
| Incremental compilation | ✅ Complete | `incremental/` 9,377 LOC |
| Async codegen | ✅ Complete (eager by design) | `compile/expr.rs:360` |

Similarly for FajarOS V28:
- ml_scheduler.fj is V28.4 equivalent
- services/display/ (2,047 LOC) is V28.3 equivalent  
- transformer.fj already has GQA + dual-theta RoPE + sliding window
- tokenizer.fj already supports 262K vocab
- model_loader.fj supports v1-v6 formats

## Effort Estimation Anti-Pattern

Plan documents consistently estimated effort based on "scope of feature" not "what's actually missing." The audit corrections:

| Feature | Plan est | Actual work needed |
|---------|----------|-------------------|
| @app annotation | 8h | 0.2h (5 lexer/parser points) |
| @host annotation | 12h | 0.2h (same pattern) |
| Refinement params | 32h | 0.5h (extracted helper + 1 call site) |
| Cap<T> | 40h | 0.8h (Value variant + 3 builtins) |
| AI scheduler builtins | 16h | 0.5h (2 builtins in existing pattern) |
| IPC stubs | 24h | 0.5h (metadata generator) |
| Gemma 3 1B port | 160h | **~1 week** (export script + test) |

## V28.1 Remaining Work

The "massive" V28.1 sprint is actually **1 week of focused work**:

1. `huggingface-cli download google/gemma-3-1b-pt` (~2 GB)
2. Copy `scripts/export_smollm_v6.py` → `export_gemma3_v7.py`
3. Update header format (add rope_theta_global, sliding_window, n_kv_heads fields)
4. Add v7 branch in `mdl_parse_header` (15-line change)
5. Run export → .fjm at 4-bit (~460 MB)
6. Write to disk.img, boot QEMU, test `ask "what is 2+2"`
7. Benchmark perplexity on WikiText-2

Reference: `fajaros-x86/scripts/README_GEMMA3_EXPORT.md`
Next-steps checklist: `fajaros-x86/docs/V28_1_NEXT_STEPS.md`

## User Actions Still Required

These cannot be automated:

1. **ORCID registration** — orcid.org, 2 min, free
2. **Crates.io publish** — `cargo login` + `cargo publish` (needs API token)
3. **arXiv upload** — register account + upload PDF
4. **Gemma 3 license** — accept HF license for model download

## Key Files Produced/Modified This Session

### Fajar Lang (12 commits)
- `src/runtime/os/ai_kernel.rs` — MAX_KERNEL_TENSOR_DIM 16→128
- `src/interpreter/eval/builtins.rs` — AI scheduler + Cap builtins + FB ext
- `src/codegen/cranelift/mod.rs` — AOT ISR wrapper emission
- `src/codegen/linker.rs` — x86_64 wrapper + target dispatcher
- `src/codegen/cranelift/runtime_bare.rs` — fb_set_base, fb_scroll
- `src/codegen/ipc_stub.rs` — NEW — service stub generator
- `src/lexer/token.rs` — AtApp, AtHost tokens
- `src/parser/mod.rs` — annotation parser additions
- `src/interpreter/value.rs` — Value::Cap variant
- `src/interpreter/eval/mod.rs` — check_refinement helper, param checks
- `src/selfhost/bootstrap_v2.rs` — @host file I/O subset
- `tests/v27_5_compiler_prep.rs` — NEW — 16 E2E integration tests
- `.github/workflows/ci.yml` — v27_5_regression job

### FajarOS (5 commits)
- `kernel/compute/kmatrix.fj` — KM_GEMMA pool (80 KB at 0xB70000)
- `tests/kernel_tests.fj` — test_gemma_pool_alloc (TEST_TOTAL 25→26)
- `docs/MEMORY_MAP.md` — updated Gemma region
- `docs/V28_B0_FINDINGS.md` — NEW — pre-flight audit
- `docs/V28_STATUS.md` — NEW — scope revision
- `docs/V28_1_NEXT_STEPS.md` — NEW — sprint checklist
- `scripts/README_GEMMA3_EXPORT.md` — NEW — export design

## Lessons for Future Sessions

1. **Audit before planning.** Read code with grep + file reads before committing to effort estimates. Plans drift 10-100× from reality.

2. **The 5-point annotation pattern is cheap.** Adding new @annotations takes <30 minutes following the established pattern (lexer token, Display impl, ANNOTATIONS map, display test, parser match list + name mapping).

3. **Feature flags and cfg patterns enable broad test coverage quickly.** 12 feature flag tests in 1.5h vs the planned 8h.

4. **Runtime enforcement beats compile-time for prototype phases.** Cap<T> linear semantics via runtime `Arc<Mutex<Option<Value>>>` works today; full compile-time linearity can be added later when the type system work is warranted.

5. **FajarOS is more mature than compiler plans assume.** Infrastructure for GUI, networking, ML, and attention primitives is broadly in place. The productive work is identifying what's genuinely missing, not rebuilding what exists.

## Plan Hygiene Self-Check (§6.8)

```
[x] Pre-flight audits ran before each phase                      (Rule 1)
[x] Every task verified with runnable commands                    (Rule 2)
[x] Prevention layer added (v27_5_regression CI job)              (Rule 3)
[x] Agent-produced numbers cross-checked with hands-on Bash      (Rule 4)
[x] Effort variance tagged in every commit message               (Rule 5)
[x] Decisions committed as files (V28_*_FINDINGS.md, V28_STATUS) (Rule 6)
[x] Public artifact sync (GitHub releases + CHANGELOG drafts)    (Rule 7)
[x] Multi-repo state check at start + end of session             (Rule 8)
```

## Closing

Session ends at genuine natural checkpoint. Remaining work requires either:
- User action (ORCID/crates.io/arXiv)
- Model weight download (Gemma 3 1B sprint)

No artificial "continue" moves possible without those inputs. Work is durable, discoverable, and reproducible. Future V28.1 session can resume with zero context loss from the committed docs.
