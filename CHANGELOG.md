# Changelog

All notable changes to Fajar Lang are documented here.

## [Unreleased] — 2026-05-03 FAJAR_LANG_PERFECTION_PLAN P4 + P5 closed

### Added

**P4 — Soundness probes** (~4.5h actual vs 30-50h plan estimate, -85%
under). Three sub-items:

- **C1 polonius soundness probes** (commit `8d9a3768`) — 16 tests in
  `tests/polonius_property_tests.rs`. 11 deterministic scenario probes
  (many `&T` allowed, solo `&mut T` allowed, dangling-ref detection,
  loop-CFG termination, killed-loan propagation, reborrow via subset,
  disjoint loans, etc.) + 5 proptest properties (termination,
  monotonic invalidation, determinism, no-loans-no-errors, killed-
  loans-silenced). PASS criterion ≥10 → +60% over.
- **C2 error-code coverage** (commits `cdc99219..4d3ad435`, 6 commits) —
  103 tests in `tests/error_code_coverage.rs` covering 125 of 135
  cataloged codes; 12 forward-compat per §6.6 R6 (catalog-only or
  declared-but-never-emitted variants documented honestly with routing
  fallback). Catalog reconciliation: `docs/ERROR_CODES.md` 91 → 135 codes;
  PE/SE/TE/DE descriptions corrected to match source. New audit script
  `scripts/audit_error_codes.py --strict` exits 0 with gap=0 (CI-gated
  in `.github/workflows/ci.yml`).
- **C3 fuzz +3 targets** (commit `cb6d7ce2`) — `fuzz_codegen`,
  `fuzz_borrow`, `fuzz_async` registered in `fuzz/Cargo.toml`; CI runs
  each at 60s in the `fuzz` job. Stable-Rust canary
  (`tests/fuzz_target_canary.rs`, 6 tests) catches API drift without
  needing nightly + cargo-fuzz.

Findings: `docs/FAJAR_LANG_PERFECTION_PHASE_4_FINDINGS.md`.

**P5 — LSP + IDE quality** (~1.5h actual vs 24-32h plan estimate, -94%
under). Three sub-items:

- **D1 5 editor packages** (commit `def30dc5`) — 10 tests in
  `tests/editor_packages.rs` validating helix/jetbrains/neovim/vscode/
  zed configs parse + reference `fj lsp` invocation + declare `.fj` file
  extension. Plus `lsp::run_lsp` pub-surface check + main.rs `Command::Lsp`
  dispatch regression gate. Honest scope: true E2E editor testing
  requires graphical env beyond CI; tests validate launch pre-conditions.
- **D2 lsp_v3 semantic tokens** (commit `f57f7992`) — 41 tests in
  `tests/lsp_v3_semantic_tokens.rs` covering all 24 `SemanticTokenType`
  variants + 8 `SemanticTokenModifier` variants + 4 meta-checks +
  5 delta-encoding correctness tests. PASS ≥1 test per token kind.
  Honest finding: pre-flight count was 25; actual 24 (corrected).
- **D3 error display polish** (commit `9ebd6baf`) — 18 tests in
  `tests/error_display_golden.rs` verifying miette render quality
  (code + filename + source excerpt + help) across LE/PE/SE/KE/DE/RE
  layers. Substring-invariant rather than byte-exact goldens (more
  stable across miette upgrades + theme settings). Honest finding:
  RuntimeError variants don't carry spans, so RE renders are sparse;
  `from_runtime_error_with_span` exists for future tightening.

Findings: `docs/FAJAR_LANG_PERFECTION_PHASE_5_FINDINGS.md`.

### Stats

- 11 commits across P4+P5
- ~200 new tests (16 + 103 + 6 + 41 + 18 + 10 + 6 = 200)
- 0 production code changes (test-only / docs-only)
- Cumulative perfection-plan progress: **P0+P1+P2+P3+P4+P5 closed**
  (6 of 10 phases). Remaining: P6 examples+docs, P7 distribution,
  P8 LLVM O2 miscompile, P9 synthesis.

### Quality gates (all green at session end)

```
cargo test --lib --release -- --test-threads=64       7626 PASS / 0 FAIL
cargo test --release --test error_code_coverage        103 PASS / 0 FAIL
cargo test --release --test polonius_property_tests     16 PASS / 0 FAIL
cargo test --release --test fuzz_target_canary           6 PASS / 0 FAIL
cargo test --release --test lsp_v3_semantic_tokens      41 PASS / 0 FAIL
cargo test --release --test error_display_golden        18 PASS / 0 FAIL
cargo test --release --test editor_packages             10 PASS / 0 FAIL
cargo clippy --tests --release -- -D warnings           exit 0
cargo fmt -- --check                                     exit 0
python3 scripts/audit_error_codes.py --strict           exit 0; gap=0
```

---

## [V32-AUDIT-COMPLETE] — 2026-05-02 V32 audit + 4-fix follow-up

### Changed

**HONEST_AUDIT_V32 deep re-audit** (commits `ecd265a2..5c08f511`):
6-phase deep re-audit of Fajar Lang post-V26 (V27/V27.5/V28.5/V29.P1-P3/
V30/V30.SIM/V30.GEMMA3/V31.B.P2/V31.C/V31.D/V31.4 cycle, ~3 weeks).
Verdict: **No demotions.** Module classification holds at 54 [x] / 0 [f]
/ 0 [s]. All quality gates green: 7,626 lib + 2,498 integ + 14 doc tests
(0 fail, 0 flake), 0 clippy/fmt/unwrap/doc warnings.

V27.5 -97% effort variance DEBUNKED — the work is real with 16 dedicated
E2E tests in `tests/v27_5_compiler_prep.rs`. 5 gaps surfaced (1 retracted,
4 actionable, 1 deferred), all residual or doc-drift, none critical-path.

Documents added:
- `docs/HONEST_AUDIT_V32_PLAN.md` (audit plan v1.0)
- `docs/HONEST_AUDIT_V32.md` (audit findings v1.0)
- `docs/HONEST_AUDIT_V32_PHASE_{1,2,3,4,5}_FINDINGS.md` (per-phase intermediate)
- `docs/HONEST_AUDIT_V32_FOLLOWUP_PLAN.md` (4-fix plan v1.0)

**V32 audit follow-up: 4 of 5 surfaced gaps closed** (commits
`bc0f7020..3f4aaeea`). Total ~90 min vs plan 145 min = -38%, under cap.

- F1 (G5 numerical drift): synced CLAUDE.md §3 + §9.1 to hand-verified
  actuals — lib tests 7,611 → 7,626; integ 2,553 → 2,498 in 52 → 55
  files; examples 238 → 243; binary 14 → 18 MB; CLI 23 → 39 subcommands.
- F2 (G4 TE001-TE009): RETRACTED. Initial Phase 5 finding was based on
  incomplete grep scoped to a single file; wider grep found 7 actual
  TE variants (TE001 + TE004-009) and docs/ERROR_CODES.md catalogs all
  9 (TE001-TE009). CLAUDE.md §7 was correct against the catalog. No edit
  needed; mistake documented in audit doc + Phase 5 findings for honesty.
- F3 (G3 call_main TypeError): added 3 unit tests to `tests/eval_tests.rs`
  exercising V27.0 fix (rejects non-Function `main` with `RuntimeError::TypeError`).
  All 3 PASS.
- F4 (G2 @interrupt codegen): added 2 unit tests to
  `src/codegen/llvm/mod.rs` `#[cfg(test)] mod tests` (gated on
  `--features llvm`) verifying that `@interrupt fn` produces LLVM IR
  with `naked` + `noinline` attributes + `.text.interrupt` section.
  Both PASS. Pre-flight pivot to "Approach 1a" (codegen-API direct test)
  worked because no FJ_EMIT_IR test infrastructure existed in tests/.

Item 5 (G1 LLVM O2 miscompile root-cause fix or upstream filing,
~5-8 days) remains OPPORTUNISTIC. Currently quarantined via 3 layers:
`@no_vectorize` workaround + gcc C bypass for kernel vecmat + Phase D
MatMul-Free architecture choice. M9 "Fajar Lang clean" milestone open.

---

## [27.5.0] — 2026-04-14 "Compiler Prep" (back-filled 2026-05-02 from GitHub Release)

> Deep audit found 6/10 reported gaps were already implemented. 4 real gaps + 7 enhancements addressed in 5.6h actual vs 196h estimated (-97% — variance debunked in HONEST_AUDIT_V32 §4: leverage of pre-existing infra + estimate inflation, work is real with 16 dedicated E2E tests).

### Added (V28-V33 prep)

- **`MAX_KERNEL_TENSOR_DIM`** raised 16 → 128 (Gemma 3 head_dim=256 unblocked)
- **AI scheduler builtins:** `tensor_workload_hint(rows, cols)`, `schedule_ai_task(id, priority, deadline)`
- **`@interrupt` ISR wrappers** — ARM64 + x86_64 + target dispatcher, wired to AOT pipeline (codegen at `src/codegen/llvm/mod.rs:3312-3325` adds `naked + noinline + .text.interrupt` section; E2E test added in V32 follow-up F4)
- **VESA framebuffer extensions:** `fb_set_base(addr)`, `fb_scroll(lines)` + full MMIO stack
- **IPC service stubs:** `ServiceStub::from_service_def()` generates dispatch fn names, sequential message IDs, client proxy names, ID constants
- **`@app`** annotation (GUI application entry point, V30 Desktop)
- **`@host`** annotation (Stage 1 self-hosting compiler context, V31)
- **Refinement predicates** extended from let-binding to function parameters
- **`Cap<T>`** capability type with linear semantics: `cap_new`, `cap_unwrap`, `cap_is_valid`

### Quality & Prevention

- **`tests/v27_5_compiler_prep.rs`** — 16 E2E integration tests covering AI scheduler, framebuffer, @app/@host, refinement params, Cap<T>, cross-feature integration
- **`v27_5_regression` CI job** runs on every push (`.github/workflows/ci.yml`)
- Version sync check added to pre-commit hook

### Stats

- 7,623 lib tests + 16 V27.5 integration = ~10,200 total tests
- 0 failures, 0 clippy warnings, 0 fmt diffs
- All 12 feature flags tested

---

## [27.0.0] — 2026-04-13 "Hardened" (back-filled 2026-05-02 from GitHub Release)

> Deep re-audit found 5 gaps. All closed with prevention layers.

### Added

- **12 feature flag integration tests** in `tests/feature_flag_tests.rs` (22 actual `#[test]` fns gated on `#[cfg(feature = "...")]` for websocket, mqtt, ble, gui, https, cuda, smt, cpp-ffi, python-ffi, gpu, tls, playground-wasm)
- **`scripts/check_version_sync.sh`** — Cargo.toml ↔ CLAUDE.md major-version sync check (V27 A4 prevention layer)

### Changed

- **`call_main()`** rejects non-Function `main` with `RuntimeError::TypeError` (was silent `Null`); test coverage added in V32 follow-up F3
- **Cargo.toml version** 24.0.0 → 27.0.0; CLAUDE.md banner V27.0

### Fixed

- **10 cargo doc broken intra-doc links** — bracket escaping, HTML tag wrapping; `cargo doc` now emits 0 warnings

### Stats

- 7,611 lib + 2,553 integ + 14 doc = ~10,179 tests
- 238 examples | 54 modules | ~448K LOC
- 12 feature flags with integration tests

---

## [26.3.0] — 2026-04-13 "V26 Final" (back-filled 2026-05-02 from GitHub Release)

> All three V26 phases complete. Phase A 100%, Phase B 100%, Phase C ~95%.

### Added

- **12 v3 tensor ops as interpreter builtins** for FajarQuant v3 profiler: `var_axis`, `std_axis`, `kurtosis`, `svd_ratio`, `select`, `per_channel_quant`, `residual_quant`, `asymmetric_quant`, `abs_max`, `topk`, `skewness`, `channel_cv`
- **`docs/V26_FAJARQUANT_V3_PLAN.md`** — committed FajarQuant v3 plan

### Changed

- **CLAUDE.md** synced to v25.1 with verified numbers (7,611 tests, 238 examples)

### Stats

- 7,611 lib tests + 2,374 integ + 14 doc ≈ 10,000 total
- 238 examples | 54 modules (0 framework, 0 stubs)
- ~446K LOC Rust across 394 source files
- 80/80 stress runs at `--test-threads=64`

### Companion Releases

- [FajarOS v3.1.0](https://github.com/fajarkraton/fajaros-x86/releases/tag/v3.1.0) — Security hardened
- [FajarQuant v0.3.0](https://github.com/fajarkraton/fajarquant/releases/tag/v0.3.0-fajarquant-v3.1) — Adaptive per-head selection

---

## [31.0.0] — 2026-04-23 "Phase D + Track B"

> 8-day catch-up consolidating V28-V31 across compiler + OS + quant. Last
> CHANGELOG entry was v26.2.0 (2026-04-13); this entry retains the bulk
> V28-V31 changes. v26.3.0, v27.0.0, v27.5.0 entries above are back-filled
> 2026-05-02 from their GitHub Release pages (per FAJAR_LANG_PERFECTION_PLAN
> P1.A5).

### Added

**Compiler attrs (V29.P1, V31.B.P2):**
- **`@noinline`+`@inline`+`@cold` lexer** (V29.P1) — lexer recognition closes silent-build-failure class. 5-layer prevention chain: lexer + codegen test + Makefile ELF-gate + pre-commit hook + install-hooks script.
- **`@no_vectorize` codegen attribute** (V31.B.P2) — lexer + parser + codegen E2E. IR + disasm verified. Forces scalar codegen for kernels whose vectorization triggers downstream issues (e.g. V31 R3 pad-collapse).
- **`FJ_EMIT_IR` env var** — dumps pre-optimization LLVM IR to stderr, enabling root-cause investigation of optimizer-induced bugs without rebuilding with verbose flags.

**CLAUDE.md rules (V30.TRACK4, V31.C):**
- **§6.10 Filesystem Roundtrip Coverage Rule** — surfaced by V30 Track 4. Any kernel FS write path needs a Makefile regression target with QEMU `-boot order=d` for CDROM boot, in-kernel mkfs+mount+write over host-built images, and pre-existing bugs surfaced as NOTE lines. 4-YES self-check.
- **§6.11 Training Script Interruption-Safety Rule** — surfaced by FajarQuant c.1 hang (laptop suspend → dead HF sockets → 8.5h wasted GPU). Codifies Track B 5-layer defence as cross-repo rule. 5-YES self-check.

**Earlier compiler additions (v27.5.0 "Compiler Prep", v27.0.0 "Hardened", v26.3.0 "V26 Final" — covered en bloc here):**
- AI scheduler builtins (`tensor_workload_hint(rows,cols)`, `schedule_ai_task(id,priority,deadline)`) — V27.5.
- `@interrupt` ISR wrappers (ARM64 + x86_64 + target dispatcher) wired to AOT pipeline — V27.5.
- `@app` (GUI app entry) + `@host` (Stage 1 self-hosting) annotations — V27.5.
- `Cap<T>` linear/affine capability type with `cap_new`/`cap_unwrap`/`cap_is_valid` — V27.5.
- Refinement predicates extended from let-binding to function parameters — V27.5.
- `fb_set_base(addr)` + `fb_scroll(lines)` VESA framebuffer extensions + full MMIO stack — V27.5.
- IPC service stub generator (`ServiceStub::from_service_def()`) — V27.5.
- `MAX_KERNEL_TENSOR_DIM` 16 → 128 (Gemma 3 head_dim=256) — V27.5.
- `tests/v27_5_compiler_prep.rs` 16 E2E integration tests + `v27_5_regression` CI job — V27.5.
- `tests/feature_flag_tests.rs` 12 untested feature flag tests — V27.0.
- `scripts/check_version_sync.sh` (V27 A4 prevention layer for §6.8 Rule 3) — V27.0.
- Phase B + C completion per `docs/V26_PRODUCTION_PLAN.md` — V26.3.

### Changed

- **Cargo.toml version** 27.5.0 → 31.0.0 (matches CLAUDE.md major bump for `scripts/check_version_sync.sh` CI gate).
- **CLAUDE.md banner** Version `27.5+V29.P1+V30.GEMMA3+V30.TRACK4+V31.C.TRACKB` → `31.0+V31.C.TRACKB`; Last Updated 2026-04-22 → 2026-04-23.
- **README.md** Release/Tests/FajarOS/FajarQuant badges + Project Stats Release+Tests+FajarOS Nova rows + Production status row + new V28-V31 additions row + Release History new top entry.
- **GitHub repo metadata** — 5 new topics added (`cuda`, `llvm`, `quantization`, `risc-v`, `wasm`); 12 → 17 total.
- **`Cargo.toml` description** kept at v27.5 baseline phrasing (still accurate for v31.0.0; not regenerated).
- **`call_main()`** rejects non-Function main with TypeError (was silent Null) — V27.0.
- **10 cargo doc warnings → 0** — V27.0.

### Fixed

**FajarOS Nova security triple (V29.P2, V29.P3, V29.P3.P6):**
- **SMEP re-enabled** (V29.P2) — closed V28.1 U-bit leak. 35/35 kernel tests.
- **SMAP re-enabled** (V29.P3) — V26 B4.2 SMAP CLOSED. Fix: extend `strip_user_from_kernel_identity()` to strip USER from non-leaf PML4[0]+PDPT[0]. Gate: `make test-smap-regression`.
- **NX triple closure** (V29.P3.P6) — V26 B4.2 security triple 3/3 (SMEP+SMAP+NX) COMPLETE. Fix: `pd_idx=1→2` in `security.fj:236` (kernel `.text` straddles PD[0]+PD[1]). Gate: `make test-security-triple-regression` 6-invariant.

**FajarOS Nova FS write (V30.TRACK4 + V31.D Track D, fajaros-x86 commit `c2d6be7`):**
- **`ext2_create` returning -1 on freshly-mkfs'd disk** — root inode missing BLOCK0 allocation. 3 `cmd_mkfs_ext2` bugs + 1 UI bug closed. `make test-fs-roundtrip` 11/11 invariants PASS.
- **Silent QEMU triple-fault** — `-boot order=d` forces CDROM boot, otherwise QEMU boots a disk whose `0x55 0xAA` signature triple-faults before any serial output.

### Stats

```
Compiler:        0 production .unwrap() | 0 clippy warnings | 0 fmt drift
                 0 doc warnings | CI gates green at every push since v27.5.0
                 Modules: 54 [x] / 0 [f] / 0 [s] (no regression from v26.1.0-phase-a)
                 Cargo.toml: 31.0.0 | CLAUDE.md banner: 31.0+V31.C.TRACKB

FajarOS Nova:    v3.4.0 → v3.7.0 ("FS Roundtrip")
                 108K LOC | 183 .fj files | 35 kernel tests
                 SMEP+SMAP+NX security triple closed | ASLR
                 VFS write: RamFS + FAT32 + ext2
                 14 LLM shell commands | SmolLM-135M v5/v6 E2E
                 Gemma 3 1B foundation audit-complete (Path D, 12 phases PASS)
                 Gates green:
                   test-security-triple-regression (6-invariant)
                   test-fs-roundtrip (11/11 invariants after V31.D fix)
                   test-gemma3-{e2e,kernel-path} (0 crashes)
                 Boots reliably to nova> in QEMU

FajarQuant:      Phase D IntLLM (separate repo fajarkraton/fajarquant)
                 Custom MatMul-Free LLM (HGRNBitForCausalLM + ternary BitLinear)
                 Mini v2: val_loss 4.38 (PPL 80.0)
                 Base c.1 PASS: val_loss 3.9903 (PPL 54.1)
                                by 0.21 nat margin (3× wider than c.2's 0.071)
                                Chinchilla-optimal 21.16 tok/p
                                8h03m wall-clock on RTX 4090 Laptop
                 Track B 5+1 layers (V31.C.P6.1-P6.6):
                   ckpt_every (atomic + rotation)
                   --resume / --resume-auto (bit-exact state restore)
                   StepWatchdog (SIGTERM if step idle > 1800s)
                   HF timeout + retry_iter
                   test-train-watchdog Makefile gate (24 tests + signal delivery)
                   nohup line-buffering hardening
                 Medium training: in flight at v31 cut (~17.8h ETA, 91K steps × 16,384 tok)

GitHub:          5 new topics: cuda, llvm, quantization, risc-v, wasm (12 → 17)
                 Release v27.5.0 → v31.0.0 (Latest)
                 Tag v31.0.0 → commit 6650545 on main
```

### Notes (intermediate tags not back-filled)

This entry covers v26.3.0 (2026-04-13 "V26 Final"), v27.0.0 (2026-04-13 "Hardened"), v27.5.0 (2026-04-14 "Compiler Prep") collectively rather than as separate CHANGELOG entries. Granular detail for those tags lives in their GitHub Release pages:

- https://github.com/fajarkraton/fajar-lang/releases/tag/v26.3.0
- https://github.com/fajarkraton/fajar-lang/releases/tag/v27.0.0
- https://github.com/fajarkraton/fajar-lang/releases/tag/v27.5.0

Granular back-fill into CHANGELOG.md is a deferred follow-up (no functional gap; release pages cover the same content).

---

## [26.2.0] — 2026-04-13 "FajarQuant v2.12" (C1.6 Path B complete)

### Added
- **Native `Quantized<T, BITS>` type** — first-class quantized tensor in the type system with `Value::Quantized` + `Type::Quantized` (B5.L1)
- **SE023 QuantizedNotDequantized** — compiler error when Quantized used where Tensor expected, forces explicit `dequantize()` (B5.L1.2)
- **`hadamard()` + `hadamard_inverse()` builtins** — Fast Walsh-Hadamard Transform O(D log D), power-of-2 check (B5.L2)
- **`hadamard_avx2()` AVX2 SIMD** — 1.9-2.0x speedup over scalar at D>=128, `_mm256` butterfly intrinsics (B5.L2.2)
- **`load_calibration()` / `save_calibration()` / `verify_orthogonal()`** — calibration data pipeline with orthogonality check (B5.L3)
- **`hadamard_quantize()` fused kernel** — single-pass Hadamard+quantize, 1.6x speedup, AVX2 (B5.L5)
- **`matmul_quantized()`** — dequantize + matmul with auto NK/KN layout detection and shape validation (B5.L6)
- **`QuantizedKVCache`** — `kv_cache_create/update/get_keys/get_values/len/size_bytes` with overflow detection (B5.L7)
- **20+ new builtins** wired E2E from `.fj` programs
- **Criterion benchmark** `benches/hadamard_simd.rs` — scalar vs AVX2 vs fused pipeline
- **4 new examples:** `quantized_tensor.fj`, `hadamard_demo.fj`, `calibrated_rotation.fj`, `fajarquant_v2_device.fj`, `fajarquant_v2_selfhost.fj`, `stack_kv_cache.fj`
- **5 new integration test files** (44 tests): `quant_type_safety.rs`, `calibrated_rotation_orthogonal.rs`, `fajarquant_v2_device.rs`, `quant_matmul_shape.rs`, `stack_kv_cache.rs`

### Changed
- **`Type::Quantized` compatibility** — `bits=0` is polymorphic, bare `Quantized` resolves in type checker
- **`resolve_type`** maps `"Quantized"` like `"Tensor"` in analyzer
- **FajarQuant paper** reframed: "Cross-Architecture KV Cache Quantization: Why No Single Method Wins"
- **Paper PPL table** replaced with 3-model × 5-method canonical R-alpha.1 data (28 claims verified)
- **Related Work** expanded from 5 to 13 entries (8 new: KVQuant, SKVQ, SpinQuant, FlatQuant, RotateKV, KVTC, KVLinC, AsymKV)
- **`verify_paper_tables.py`** rewritten for reframed paper — 28/28 claims PASS

### Stats
```
Tests:     7,572 lib + 2,374+44 integ + 14 doc ≈ 10,004 total
LOC:       ~449,000 Rust (src/) + 3,300 new for B5
Examples:  237 .fj (was 231, +6 new)
Benchmarks: hadamard_simd (7 configs: scalar/avx2/fused × 6 dimensions)
Native vs Python: 5.0x faster (28ms vs 142ms)
```

## [26.1.0-phase-a] — 2026-04-11 "Final" (Phase A complete)

### Added
- **Pre-commit hook** (`scripts/git-hooks/pre-commit`) — rejects fmt drift via two-layer check (`cargo fmt --check` + per-file `rustfmt --check --edition 2024` for orphan files). Installer at `scripts/install-git-hooks.sh`.
- **CI flake-stress job** (`.github/workflows/ci.yml`) — runs `cargo test --lib -- --test-threads=64 × 5` per push to catch wall-clock timing flakes.
- **CLAUDE.md §6.7 Test Hygiene Rules** — formal antipattern rejection for `assert!(elapsed < N_ms)` on simulated/microsecond-scale work.
- **`scripts/audit_unwrap.py`** — three-layer false-positive filter for accurate production `.unwrap()` accounting.
- **`audit/A2_unwrap_inventory.md`** + `audit/unwrap_inventory.csv` — full audit trail showing prior counts inflated 1,353× (4,062 → 174 → 20 → real 3).
- **3 new builtins** wiring previously-framework `const_*` modules:
  - `const_serialize(value)` — wraps `serialize_const()`, returns `.rodata`-ready byte serialization (A3.1)
  - `const_eval_nat(expr, bindings)` — wraps `parse_nat_expr` + `eval_nat`, evaluates Nat expressions like `"N+1"` (A3.2)
  - `const_trait_list()`, `const_trait_implements(type, trait)`, `const_trait_resolve(type, trait, method)` — query the `ConstTraitRegistry` of 5 built-in const traits + ~70 numeric impls (A3.3)
- **Parser fix:** `parse_trait_method` accepts optional `const`/`comptime` before `fn`. `trait Foo { const fn bar() -> i64 { 42 } }` now parses (was PE002).
- **3 new demos:** `examples/const_alloc_demo.fj`, `const_generics_demo.fj`, `const_traits_demo.fj`
- **18 new V26 builtin tests** in `tests/v20_builtin_tests.rs` (`v26_a3_*`)
- **`docs/V26_PRODUCTION_PLAN.md`** — 6-week roadmap with 4 phases (A: Fajar Lang, B: FajarOS, C: FajarQuant, D: stretch)
- **`docs/HONEST_AUDIT_V26.md`** — verified state with audit-correction tables
- **`docs/HONEST_STATUS_V26.md`** — per-module status replacing V20.5

### Changed
- **`measure_incremental_overhead()`** — added 1 ms noise floor + asymmetric jitter handling (`.abs_diff()`)
- **14 wall-clock test thresholds** bumped 10× across `validation.rs`, `rebuild_bench.rs`, `lsp/server.rs`, `codegen/cranelift/tests.rs`. Targets preserved in comments.
- **`i10_10_report_display`** rewritten as hermetic test using fixture `IncrementalValidationReport`
- **`#![cfg_attr(not(test), deny(clippy::unwrap_used))]`** added to `src/lib.rs` — production builds machine-enforce zero unwraps
- **3 production `.unwrap()` calls** replaced with `.expect("rationale")` documenting infallibility
- **CLAUDE.md** — comprehensive numbers refresh: tests 11,395 → 9,969 (verified), examples 285 → 231, error codes 71 → 78, modules 56 → 54 (54 [x], 0 [f], 0 [s])

### Fixed
- **6 fmt diffs** in `src/codegen/llvm/mod.rs` from V24 AVX2 i64 SIMD commit (author skipped `cargo fmt`)
- **Test flake `i10_10_report_display`** — investigation revealed 14 vulnerable tests across 4 files all sharing root cause: wall-clock timing assertions on microsecond-scale simulated work. Pre-fix flake rate ~20% per full run; post-fix 0% across **80 consecutive runs at `--test-threads=64`**
- **Hook edition mismatch** — `rustfmt --check` defaulted to edition 2015, conflicting with project's edition 2024. Hook now extracts edition from `Cargo.toml`

### Removed
- Stale references to `demos/` and `generators_v12` modules in CLAUDE.md and HONEST_STATUS docs (modules already deleted in V20.8)

### Stats
- 7,581 lib tests + 2,374 integ + 14 doc = ~9,969 total | **0 failures, 0 flakes**
- **80/80 consecutive `--test-threads=64` runs** (was ~20% flake rate pre-fix)
- 0 production `.unwrap()` (was claimed 4,062, real was 3, all replaced)
- 0 fmt diffs, 0 clippy warnings
- **54 [x] / 0 [sim] / 0 [f] / 0 [s] modules — zero framework, zero stubs**
- 231 examples (was 228; +3 V26 const_*+gui demos)
- **Fajar Lang at 100% production per V26 Phase A goals**

---

## [25.1.0] — 2026-04-07 "Production Plan + Initial Fixes"

### Added
- **`docs/V25_PRODUCTION_PLAN.md`** v5.0 — 5-week roadmap targeting commercial release. Updated through 4 rounds of hands-on re-audit, fixing 10 false alarms.
- **HashMap auto-create** — `map_insert(null, "k", v)` now creates an empty map (commit `30ef65b`)
- **K8s deploy target** — `fj deploy --target k8s` generates Kubernetes manifests (was not wired)
- **WGSL CodebookDot compute shader** — fixes `--features gpu` build (was E0004)
- **FajarQuant Phase C complete** — real KV cache extraction from Gemma 4 E2B (50 prompts), 3-way comparison vs KIVI + TurboQuant
- **FajarQuant ablation study (C4)** — PCA rotation isolated 4-6% MSE improvement, fused attention 524,288× memory reduction, hierarchical 48.7% bit savings @ 10K context
- **FajarQuant paper finalized** — 5-page LaTeX with 6 tables of real Gemma 4 E2B data, 7 references, Theorem 3 with formal proof
- **`docs/FAJARQUANT_KERNEL_PLAN.md`** — 8-phase roadmap to kernel-native LLM inference

### Changed
- **LLVM release JIT** — `lto = true` → `false` in `Cargo.toml`. LTO was stripping MCJIT symbols
- **LLVM `println` segfault fixed** — runtime functions gated behind `#[cfg(feature = "native")]`
- **f-string codegen** — `Expr::FString` now handled in LLVM backend
- **String concat `a + b`** — `compile_binop` checks struct-type before `into_int_value()`
- **Real Gemma 4 E2B perplexity** (FajarQuant): wins at 2-bit (80.14 ppl) and 3-bit (75.65 ppl); TurboQuant wins at 4-bit (92.84 ppl) — design tradeoff documented

### Fixed
- **`@kernel` transitive heap taint** (commit `849943d`) — V17's CRITICAL bug. Analyzer now blocks indirect heap allocation through function calls. KE001 fires correctly.
- **LLVM string global name collision** (`3e5bae0`) — each literal gets a unique name
- **LLVM null-terminated string globals** (`b14f136`) — fixes serial output display in bare-metal
- **AOT linker symbols** — `.weak` symbols, `read_cr2`, `irq_disable`, `XSETBV` in `sse_enable` (`69a4439`)
- **Paper table overflow** (`48549da`)

### Stats
- ~7,581 lib tests | 0 failures
- LLVM backend production-grade with 30 enhancements + 4 string-display fixes
- @kernel/@device enforcement WORKING (was V17's "CRITICAL not enforced at all")

---

## [24.0.0] — 2026-04-06 "Quantum"

### Added
- **CUDA GPU compute on RTX 4090** (Phase 7 complete):
  - Real `cuModuleLoadData` → `cuModuleGetFunction` → `cuLaunchKernel` pipeline
  - **9 PTX kernels:** tiled matmul (16×16 shared mem), vector add/sub/mul/div, relu, sigmoid, softmax, codebook_dot
  - Device cache (`OnceLock`), kernel cache, async CUDA stream pipeline
  - `gpu_matmul`/`add`/`relu`/`sigmoid` builtins → CUDA first, CPU fallback
  - **~3× speedup at 1024×1024 matmul** on RTX 4090 (measured)
- **FajarQuant Phase 5-7** wired into interpreter:
  - Phase 5: 8 `@kernel`/`@device` safety tests
  - Phase 6: Paper benchmarks with real numbers
  - Phase 7: GPU codebook dot product on RTX 4090 via PTX
- **AVX2 SIMD + AES-NI builtins** (LLVM backend only, Phase 3.6+3.7):
  - 6 LLVM-only builtins via inline asm: `avx2_dot_f32`, `avx2_add_f32`, `avx2_mul_f32`, `avx2_relu_f32`, `aesni_encrypt_block`, `aesni_decrypt_block`
  - Memory-based XMM/YMM operands (no vector type changes needed)
  - Interpreter returns clear error directing user to `--backend llvm`
- **PTX sm_89 (Ada Lovelace)** support + BF16/FP8 types
- **GPU benchmark example** — RTX 4090 detection + matmul

### Stats
- ~7,572 lib tests | 0 failures
- ~446K LOC | claim 285 examples (real 231 verified later in V26)

---

## [23.0.0] — 2026-04-06 "Boot"

### Added
- **FajarOS boots to shell** — 61 init stages, `nova>` prompt, 90/90 commands pass
- **Ring 3 user mode** — SYSCALL/SYSRET + user pages, `x86_64-user` target, `_start` wrapper, `SYS_EXIT=0`
- **NVMe full I/O** — controller + identify + I/O queues, `INTMS=0x7FFFFFFF` (mask hardware interrupts)
- **GUI compositor** — 14 modules initialized, framebuffer mapped from Multiboot2

### Fixed (16 bugs)
- **LLVM asm constraint ordering** (`fcb66c4`) — outputs before inputs (`"=r,r"` not `"r,=r"`), fixes BSF/POPCNT
- **InOut asm operands** (`f76bf2e`) — tied output + input constraints
- **Entry block alloca helper** — stable stack allocations for arrays
- **CR4.OSXSAVE** in `sse_enable` (`0044f13`) — required for VEX-encoded BMI2 instructions
- **Exception handler `__isr_common`** — correct vector offset (+32), proper digit print
- **Page fault `__isr_14`** — CS offset +24 (was +16, reading RIP instead of CS)
- **PIC IRQ handlers** (vectors 34-47) — send EOI and return
- **LAPIC spurious handler** (vector 255) — silent `iretq`
- **`iretq_to_user`** — segment selectors + kernel RSP save, uses CALL not inline asm
- **User-mode `_start`** — removes privileged I/O from Ring 3 println runtime
- **Frame allocator** — hardware BSF/POPCNT via inline asm (was software fallback)
- **VGA cursor state** moved 0x6FA00 → 0x6FB10 (was inside history buffer overlap)
- **ACPI table page mapping** — `nproc`/`acpi`/`lspci` work now
- **GUI framebuffer** — map Multiboot2 FB pages, dynamic front buffer address
- **`cprint_decimal`** — divisor-based (avoids stack array codegen issue)

### Stats
- 7,572 compiler lib tests pass | 90 FajarOS shell commands pass
- FajarOS: 1.02 MB ELF, NVMe 64 MB, 4 PCI devices, 1 ACPI CPU, GUI FB mapped

---

## [22.0.0] — 2026-04-06 "Hardened"

### Added (30 LLVM Enhancements across 5 batches)
- **Batch E1-E5 (Hardening):** universal builtin override, asm constraint parser, silent error audit, type coercion, pre-link verification
- **Batch F1-F7 (Correctness):** match guards all patterns, enum payload extraction, method dispatch, string/float/bool patterns
- **Batch G1-G6 (Features):** float pow/rem, deref/ref operators, nested field access, bool/ptr casts, closure captures, indirect calls
- **Batch H1-H6 (Completeness):** `Stmt::Item`, `yield`, `tuple.0` access, range/struct/tuple/array/binding patterns in match
- **Batch I1-I6 (Final gaps):** chained field assign, int power, float range patterns, better diagnostics
- **23 new LLVM E2E tests** (was 15)

### Fixed
- 4 codegen bugs found by testing (bool cast, implicit return coercion, closure builder, var-as-fn-ptr)
- DCE preserves `kernel_main` + `@kernel`-annotated functions (was eliminated as dead code)
- Actor API: `actor_spawn` returns Map, `actor_send` returns handler result (synchronous dispatch)
- Cranelift I/O error logging + benchmark stack overflow
- 24 false pre-link warnings eliminated

### Stats
- ~7,573 lib tests, 0 failures | **38 LLVM E2E tests** (was 15)
- **0 codegen errors in bare-metal compilation** (was 690)
- FajarOS: 1.02 MB ELF, boots to shell, 90/90 commands

---

## [21.0.0] — 2026-04-04 "Production"

### Added
- **Real threaded actors** — `actor_spawn`/`send`/`supervise` use `std::thread` + `mpsc` channels (was simulated)
- **2 new actor builtins:** `actor_stop`, `actor_status`
- **6 actor integration tests** + updated demo for real threads
- **5 [sim] → [x] upgrades:** actors, accelerate, pipeline, diffusion, rl_agent
- **Real UNet diffusion model** — forward, train, sample (was random output)
- **Real DQN reinforcement agent** + CartPole physics environment
- **LLVM JIT** — `fj run --backend llvm` works for full Fajar Lang programs
- **LLVM AOT runtime library** — `fj build --backend llvm` produces working ELF
- **5 LLVM E2E integration tests** (initial set)
- **FajarQuant LaTeX paper** — 4-page PDF with 11 references, 6 tables, 4 theorems

### Changed
- **`Rc<RefCell>` → `Arc<Mutex>` migration** complete throughout interpreter (env + iterators)
- **Iterative parent chain traversal** in environment lookup
- **`RUST_MIN_STACK = 16 MB`** for tests (was 8 MB)
- **PIC enabled in AOT compiler** (eliminates TEXTREL warnings, ASLR-compatible)
- **`const_alloc` upgraded** [sim] → [x] — creates correct `ConstAllocation`; `.rodata` lowering deferred
- **5 [sim] modules relabeled to [x]** after V21 wiring

### Removed (dead code cleanup, V20.8 + V21)
- `src/rtos/` — 8 K LOC framework with zero CLI integration
- `src/iot/` — 5 K LOC framework
- `src/rt_pipeline/`, `src/package_v2/`, `src/lsp_v2/`, `src/stdlib/` — 13.4 K LOC dead modules total
- Generated artifacts (`output.ptx`, `output.spv`, `docs/api/*.html`) added to `.gitignore`

### Fixed
- 4 last `.unwrap()` calls in production code (V21 baseline; V26 audit later found 3 more, all fixed)
- 4 pre-existing integration test failures
- JIT match→variable→println string length tracking
- 7 examples: `usize` → `i64` (205 → 212 passing, 94.6%)

### Stats
- 7,581 lib tests | 0 failures
- **48 [x] / 0 [sim] / 5 [f] / 3 [s]** — zero simulated builtins
- ~459 K LOC

---

## [20.8.0] — 2026-04-04 "Perfection"

### Added
- **FajarQuant**: Complete vector quantization system (7 phases, ~4,700 LOC)
  - TurboQuant baseline: Lloyd-Max quantizer, Algorithm 1 & 2
  - Innovation 1: PCA-based adaptive rotation (49-86% MSE improvement)
  - Innovation 2: Fused quantized attention (zero-copy codebook compute)
  - Innovation 3: Hierarchical multi-resolution bit allocation
  - Paper outline: `docs/FAJARQUANT_PAPER_OUTLINE.md`
- **Native JIT**: `fj run --jit` compiles hot functions via Cranelift (76x speedup on fib(30))
- **GPU Discovery**: `gpu_discover()` detects NVIDIA GPUs via CUDA Driver API
- **12 New Tensor/Scalar Ops**: sign, argmin, norm, dot, exp_tensor, log_tensor, sqrt_tensor, abs_tensor, clamp_tensor, where_tensor, exp, gamma
- **String Free Functions**: split, trim, contains, starts_with, ends_with, replace
- **read_file_text**: Convenience builtin returning string directly
- **RuntimeError Source Spans**: Division-by-zero, index OOB, undefined var now show file:line
- **Plugin CLI**: `fj plugin list`, `fj plugin load <path.so>`
- **Strict Mode**: `fj run --strict` rejects simulated builtins
- 31 V20 builtin tests, 20 tensor op tests, 22 FajarQuant tests, 8 safety tests, 8 E2E tests

### Changed
- **Tensor Display**: Now shows actual values (NumPy-like format), not just shape
- **matmul**: Auto-reshapes 1D tensors (dot product for vectors)
- **accelerate()**: Uses real CUDA GPU detection (detected RTX 4090, 9728 cores)
- **rl_agent_step**: Normalized -0.0 → 0.0

### Fixed
- `fj build` env var handling: wrapped std::env::set_var in unsafe{} (Rust >= 1.83)
- 2 registry_cli test failures (stale SQLite cleanup)
- `accelerate()` + `actor_send()`: replaced error-swallowing unwrap_or with ? propagation

### Removed
- 20,512 LOC dead code: src/demos/ (16,257), generators_v12.rs (372), ml/data.rs (236), 6 dead const_* modules (3,644)

### Stats
- 7,999 lib tests (0 failures) + 2,400+ integration tests
- ~459K LOC (down from 479K)
- 131/131 audit tests pass (100%)
- 42 [x] production + 5 [sim] simulated + 5 [f] framework + 3 [s] stub
- FajarQuant: 49-86% MSE improvement over TurboQuant
- JIT: 76x speedup on fib(30) with --features native

## [12.6.0] — 2026-04-02 "Infinity"

### Added
- **Effect Composition**: `effect Combined = IO + State` syntax in parser, analyzer, interpreter
- **Effect Row Polymorphism**: `with IO, ..r` open row variable syntax
- **Effect Statistics**: `fj run --effect-stats` prints runtime effect usage
- **AST-Driven GPU Codegen**: `fj build --target <spirv|ptx|metal|hlsl> input.fj`
- **GPU Workgroup Size**: `@gpu(workgroup=256)` annotation with shared memory support
- **Refinement Types**: `{ x: i32 | x > 0 }` with runtime predicate checking
- **Pi Types**: `Pi(n: usize) -> [f64; n]` dependent function type syntax
- **Sigma Types**: `Sigma(n: usize, [f64; n])` dependent pair type syntax
- **Async Registry Server**: tokio-based HTTP with CORS, HMAC-SHA256 signing
- **Rate Limiting**: Token bucket rate limiter for registry API
- **API Key Auth**: Registry publish authentication
- **Search Ranking**: Relevance-ranked package search (exact > prefix > substring > description)
- **Predictive LSP Completions**: Context-aware suggestions (let=, fn(, @annotation)
- **Code Lens Resolve**: LSP code_lens_resolve handler wired to tower-lsp
- **Boot Verification**: `fj verify --verbose` analyzes kernel boot patterns
- **Driver Interface Check**: Struct conformance verification for driver-like types
- **FFI Library Detection**: `fj hw-info` shows OpenCV, PostgreSQL, Python, PyTorch, QEMU availability
- **QEMU Boot Test**: Multiboot kernel boots in QEMU, serial output verified
- **OpenCV FFI Test**: Real C → OpenCV 4.6.0 image processing verified
- 8 new example programs (effect, GPU, refinement, Pi/Sigma, MNIST, kernel)

### Changed
- GPU codegen reads .fj source files instead of hardcoded kernels
- Registry server uses tokio::net::TcpListener (was std::net)
- Package signing uses HMAC-SHA256 via sha2 crate (was DefaultHasher)
- Effect declarations registered in analyzer first pass (was second pass)

### Stats
- 8,478 tests (0 failures)
- ~486K LOC (442 Rust files)
- 218 example .fj programs
- V14: 500/500 tasks complete
- V15: 98/120 tasks complete

## [12.5.0] — 2026-04-02

### Added
- V16 Horizon features: MNIST builtins, full pipeline, tutorials
- SPIR-V + PTX codegen via `fj build --target spirv/ptx`

## [12.4.0] — 2026-03-31

### Added
- V16 Horizon 97% production: 8,102 tests

## [12.3.0] — 2026-03-30

### Added
- V16 Horizon complete: 8,096 tests, 47 .fj programs
