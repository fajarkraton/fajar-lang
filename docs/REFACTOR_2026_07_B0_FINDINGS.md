# REFACTOR_2026_07 — B0 Pre-Flight Survey (Refactoring Candidates)

> **Date:** 2026-07-30
> **Scope:** read-only survey of `src/` at HEAD `f381b23` (branch
> `claude/fajar-lang-refactor-yptxut`, == `origin/main`) to map and
> prioritize refactoring candidates before any code change.
> **Protocol:** CLAUDE.md §6.8 R1 (B0 pre-flight) + R2 (every claim backed
> by a runnable command) + R4 (numbers hand-verified, not agent-reported).
> **No code was modified in this phase.**

## Baseline gates (must be preserved by every refactor step)

```
cargo fmt -- --check                 PASS
cargo clippy --lib -- -D warnings    exit 0
cargo test --lib                     6,616 PASS / 0 FAIL   (12.7s)
```

Additional pins that any codegen-touching refactor must keep green:
`selfhost_phase17_self_compile` 4/4 byte-equality, `selfhost_stage1_full`
91/91, `context_safety_tests` 149/149 (all inside `cargo test --tests`).

## Measurement notes

- Function lengths are **approximate** (brace-depth scanner; string and
  char literals stripped before counting). A first pass without the
  char-literal fix produced two false giants
  (`format_asm_constraint_impl`, `compute_document_links` — actually ~45
  and ~20 lines); the corrected scanner was re-run and spot-checked.
- All other numbers are exact `grep`/`wc` outputs.

---

## Findings

### R1 — `codegen/cranelift/mod.rs`: two ~98%-identical 4K-line functions (P0)

The file (13,503 lines) contains **two parallel compiler impls** —
`impl CraneliftCompiler` (JIT, line 426) and `impl ObjectCompiler` (AOT,
line 6666) — each with its own `declare_runtime_functions`:

| Copy | Lines | Length |
|---|---|---|
| `CraneliftCompiler::declare_runtime_functions` | 543..4724 | 4,182 |
| `ObjectCompiler::declare_runtime_functions` | 8302..11882 | 3,581 |

Whitespace-normalized overlap: **3,516 of 3,581 lines identical** (~98%).

```
# evidence
grep -n "fn declare_runtime_functions" src/codegen/cranelift/mod.rs
sed -n '543,4724p'    src/codegen/cranelift/mod.rs | sed 's/^[[:space:]]*//' | sort > /tmp/drf1
sed -n '8302,11882p'  src/codegen/cranelift/mod.rs | sed 's/^[[:space:]]*//' | sort > /tmp/drf2
comm -12 /tmp/drf1 /tmp/drf2 | wc -l     # 3516
```

Each registration is ~15 lines of `Signature::new` + `declare_function` +
`functions.insert` boilerplate. Both `JITModule` and `ObjectModule`
implement the `cranelift_module::Module` trait, so the whole surface can
collapse to **one declarative `(builtin_name, symbol, param_types, ret)`
table + one generic `fn declare_all<M: Module>(...)` loop**, preserving
declaration order exactly (order affects symbol IDs → keep it byte-stable
for the phase17 byte-equality pin).

The same JIT/AOT twinning repeats for `compile_program` (554 + 722 lines)
and `define_function` (615 + 569 lines).

**Impact:** ~−6..7K LOC in one file; kills the "builtin added to JIT but
not AOT" drift class. **Risk:** medium (byte-equality pins; mitigated by
order-preserving table). **Effort:** 1-2 sessions, mechanical.

### R2 — `main.rs` is 6,420 lines with 59 `cmd_*` handlers inline (P0)

```
wc -l src/main.rs                          # 6420
grep -cE '^(pub )?fn cmd_' src/main.rs     # 59
```

Worst handlers: `cmd_build_native` (main.rs:3658, 634 lines),
`cmd_build_llvm` (509), `cmd_verify` (327). (The B0 scanner initially
reported `cmd_debug_replay` as 2,023 lines — corrected during Phase 2
execution: the item-level re-measure shows 53 lines; the scanner ran past
a string containing braces. Same correction class as the two §Measurement
notes false giants.) Extraction to a `src/cli/`
module tree (one file per command family, `main.rs` keeps clap defs +
dispatch) is pure code motion. CLAUDE.md §6.3 allows `.expect()` only in
`main.rs` — moved code must be audited for that during extraction.

**Impact:** navigability; `main.rs` becomes a thin entry point.
**Risk:** low (no unit tests live in main.rs; 38 subcommands smoke-covered
by integ tests). **Effort:** ~1 session, mechanical.

### R3 — `codegen/cranelift/tests.rs`: 17,687-line single test file (P1)

```
wc -l src/codegen/cranelift/tests.rs            # 17687
grep -c 'fn ' src/codegen/cranelift/tests.rs    # ~2431 fns
```

One `#[cfg(test)] mod tests;` file holding 2,431 test fns. Split into a
`tests/` submodule dir (`mod.rs` + per-area files: arithmetic, closures,
strings, arrays, methods, bare-metal, ...). Zero production risk; pure
test-code motion — test count must stay identical before/after
(`cargo test --lib codegen::cranelift:: | tail -1`).

Similar inline test mods worth moving to sibling `#[path]` files while
touching the parent file anyway: `llvm/mod.rs` (5,269 test LOC from line
9308), `interpreter/eval/mod.rs` (5,702 test LOC from line 3214),
`analyzer/type_check/mod.rs` (3,871 test LOC from line 2156).

### R4 — giant flat dispatchers (P1, split-only, no logic change)

Corrected long-function census: **103 fns >150 lines, 38 >300, 25 >500,
11 >1000** (approx). §6.1 says max 50 — the tail is dominated by flat
match dispatchers, which are low-harm but block navigation:

| Function | Location | ~Lines |
|---|---|---|
| `call_builtin` (367 string arms) | interpreter/eval/builtins.rs:22 | 3,714 |
| `generate_x86_64_startup` | codegen/linker.rs:1418 | 2,322 |
| `compile_method_call` | codegen/cranelift/compile/method.rs:24 | 2,321 |
| `compile_call` | codegen/cranelift/compile/call.rs:88 | 1,990 |
| `register_builtins` (declarative vec) | analyzer/type_check/register.rs:11 | 1,964 |
| `compile_builtin_call` (64 arms) | codegen/llvm/mod.rs:893 | 1,332 |
| `declare_bare_metal_runtime` | codegen/cranelift/mod.rs:6805 | 1,296 |
| `eval_method_call` | interpreter/eval/methods.rs:16 | 1,095 |
| `generate_aarch64_startup` | codegen/linker.rs:324 | 1,069 |

Proposal: split by domain into submodule fns (`call_builtin` →
`builtins/{string,math,array,map,io,os,ml}.rs` with a thin dispatcher),
NOT arm-by-arm rewrites. `register_builtins` is already a declarative
table — acceptable as data; lowest priority.

### R5 — four parallel builtin surfaces, no single source of truth (P2 — design phase, not mechanical)

| Surface | Measure | Command |
|---|---|---|
| interpreter dispatch | 367 string match arms | `sed -n '22,3736p' src/interpreter/eval/builtins.rs \| grep -cE '^\s+"[a-z_0-9]+"...=>'` |
| analyzer type table | ~178 top-level signatures | `sed -n '11,1974p' src/analyzer/type_check/register.rs \| grep -cE '^\s+"[A-Za-z_0-9:.]+",\s*$'` |
| cranelift runtime | 418 `extern "C" fn` | `grep -cE '^pub (unsafe )?extern "C" fn' src/codegen/cranelift/runtime_fns.rs` |
| LLVM lowering | 64 match arms | `sed -n '893,2225p' src/codegen/llvm/mod.rs \| grep -cE '..."=>'` |

Adding one builtin today means editing up to 4 places that can silently
drift (historical precedent: `char_at` needed a separate analyzer fix in
v35.3.x). A unified registry is the *right* long-term fix but is an
architectural change touching every backend — needs its own plan +
decision doc, NOT part of this mechanical arc. R1's declarative table is
a deliberate first step in this direction.

### R6 — dead weight is small but real (P2)

```
58× #[allow(dead_code)] | 13× allow(clippy::too_many_arguments)
13× allow(unused_unsafe) | 8× allow(missing_docs) | 6× allow(deprecated)
worst files: lsp/server.rs (25 allows), interpreter/eval/mod.rs (16),
codegen/cranelift/runtime_bare.rs (12)
```

`allow(unused_unsafe)` ×13 deserves an audit pass (either the block needs
no unsafe → remove both, or it does → why is the lint firing?). TODO/FIXME
census: 14 hits, of which 10 are the lint plugin's own test fixtures
(plugin/mod.rs detects TODO comments) — **real comment debt ≈ 3 sites**.

### R7 — dependency direction §4.3 is CLEAN in production (good news)

Full `crate::<top_module>` matrix built; the only forbidden-direction hits
are **test-only**:

- `analyzer → interpreter`: 3 refs in `src/analyzer/effects.rs`
  (2257/2280/2302), all inside `#[cfg(test)]` (mod starts line 1563).
- `interpreter → lsp/lsp_v3`: all refs inside `interpreter/eval/mod.rs`
  test mod (starts line 3214).
- `runtime/os ↔ runtime/ml`: **zero** cross-refs.

```
grep -rn "crate::interpreter" src/analyzer/     # 3 hits, all in test mod
grep -rn "crate::runtime::ml" src/runtime/os/   # empty
grep -rn "crate::runtime::os" src/runtime/ml/   # empty
```

No architectural repair needed; nothing to do here.

### R8 — `.clone()` density hotspots (P3 — measure-first)

3,066 `.clone()` calls in src/. Top: `analyzer/type_check/register.rs`
(328 — table construction, benign), `interpreter/eval/builtins.rs` (249),
`codegen/cranelift/mod.rs` (213). No action proposed without a profile
showing it matters (CORRECTNESS > SAFETY > USABILITY > PERFORMANCE);
recorded for a future perf pass only.

---

## Proposed phasing (each phase independently shippable, gates after each)

| Phase | Content | Est. | Risk |
|---|---|---|---|
| **1** | R1: dedup `declare_runtime_functions` via order-preserving table + generic `M: Module` fn; then split cranelift/mod.rs into `jit.rs`/`object.rs` | 1-2 sessions | medium (byte-equality pins) |
| **2** | R2: extract `src/cli/` from main.rs | 1 session | low |
| **3** | R3: split cranelift/tests.rs + relocate giant inline test mods | 1 session | ~zero |
| **4** | R4: domain-split `call_builtin` + the two `compile_*_call` giants | 1-2 sessions | low-medium |
| **5** | R6: allow-audit (esp. 13× unused_unsafe) | 0.5 session | low |
| later | R5 unified builtin registry — separate plan + decision doc | — | high |

Per-phase gate (mandatory): `cargo test --lib` == 6,616 · clippy/fmt clean ·
`cargo test --tests` unchanged pass count · phase17 4/4 for any codegen touch.

## Execution record

### Phase 1 — EXECUTED 2026-07-30 (same session as B0) [actual ~1h, est 1-2 sessions, −60%]

R1 closed via `src/codegen/cranelift/runtime_hosted.rs`:
`declare_hosted_runtime<M: Module>(module, functions, jit_extras, profiling)`
— body is the JIT declaration text with the four JIT-only groups (str_rev;
waker; thread pool→stream; SIMD+ONNX) behind `jit_extras` and the profiler
pair behind `profiling`. Both `declare_runtime_functions` methods are now
thin wrappers keeping their original `no_std`/`user_mode` prologs.

**Equivalence proof (pre-merge):** the AOT fj_rt symbol stream (293) was
shown to be an exact ordered subsequence of the JIT stream (385) with 0
AOT-only symbols; post-merge, the comment-stripped string-literal streams
of both simulated variants reproduce the original bodies exactly
(JIT 804/804, AOT 620/620 literals, order-identical).

**Gates (all green):**

```
cargo test --lib                                   6,616 PASS (== baseline)
cargo test --lib -- --test-threads=64  (×5)        5/5 PASS
cargo test --lib --features native                 7,792 PASS (baseline 7,791 + 1 new drift test)
cargo test --features native --lib codegen::cranelift  1,130 (baseline 1,129 + 1)
cargo clippy --lib | --features native | --tests --features native   all clean
cargo fmt -- --check                               PASS
python3 scripts/audit_unwrap.py                    0 production unwraps
python3 scripts/audit_unsafe.py --strict           PASS
```

**LOC:** cranelift/mod.rs 13,503 → 5,772; new runtime_hosted.rs ~3,990
(incl. drift test). **Net ≈ −3,740 LOC**, duplication class eliminated.

**Prevention layer (§6.8 R3):** single source of truth by construction +
`runtime_hosted::tests::aot_runtime_declarations_are_subset_of_jit` pins
AOT ⊆ JIT and asserts the JIT-only groups never leak into AOT objects.

## Self-check (CLAUDE.md §6.8)

| Rule | Status |
|---|---|
| R1 pre-flight audit | YES — this document |
| R2 runnable verification | YES — every finding carries its command |
| R3 prevention layer | Deferred to execution phases (e.g. drift test asserting JIT/AOT registration parity ships with Phase 1) |
| R4 numbers cross-checked | YES — survey ran inline; anomalies re-measured by hand (two false giants corrected, test-context violations reclassified) |
| R5 surprise budget | Applied at execution-phase estimates (+25%) |
| R6 mechanical decisions | Phase list above; decision doc to be committed when a phase is chosen |
| R7 public-artifact sync | n/a (no public claims changed) |
| R8 multi-repo check | n/a (single-repo, read-only phase) |
