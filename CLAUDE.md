# CLAUDE.md — Fajar Lang Master Reference

> Auto-loaded by Claude Code on every session. This is the **single source of truth** for all development decisions. Read this FIRST before any action.

---

## 1. Project Identity

- **Project:** Fajar Lang (`fj`) — A statically-typed systems programming language for embedded ML + OS integration
- **File extension:** `.fj`
- **Author:** Fajar (TaxPrime / PrimeCore.id)
- **Model:** Claude Opus 4.6 exclusively
- **Stack:** Rust (interpreter/compiler), ndarray (tensor backend), miette (error display), Cranelift (native codegen — v1.0)
- **Binary name:** `fj`

**Vision:** *"Bahasa terbaik untuk embedded ML + OS integration — the only language where an OS kernel and a neural network can share the same codebase, type system, and compiler, with safety guarantees that no existing language provides."*

**Design Principles:**
1. **Explicitness over magic** — no hidden allocation or hidden cost
2. **Dual-context safety** — @kernel disables heap+tensor; @device disables raw pointers. Compiler enforces isolation
3. **Rust-inspired but simpler** — ownership lite without lifetime annotations
4. **Native tensor types** — Tensor is a first-class citizen in the type system, shape checked at compile time

**Target Audience:** Embedded AI engineers (drone, robot, IoT), OS research teams (AI-integrated kernels), Safety-critical ML systems (automotive, aerospace, medical)

---

## 2. Mandatory Session Protocol

Every session: **READ** `CLAUDE.md` + `docs/HONEST_AUDIT_V33.md` → **ORIENT**
on what user wants vs what's real → **ACT** per TDD workflow (§8) → **VERIFY**
`cargo test --lib && cargo clippy -- -D warnings && cargo fmt -- --check` →
**UPDATE** task to `[x]` only if E2E works (use `[f]` for framework-only).

### Completion Status (v33.5.0, 2026-05-05 — Stage-1-Full Self-Hosting)

**54 modules: 54 [x] / 0 [sim] / 0 [f] / 0 [s].** Zero framework, zero stubs.
Every public mod has a callable surface from `.fj` or `fj` CLI. 39 CLI
subcommands, all production. **FAJAR_LANG_PERFECTION_PLAN P0-P9 closed
engineering-side; 22/25 work-items PASS**. **FAJAROS_100PCT_FJ_PLAN
TERMINAL COMPLETE** (2026-05-05): 9/9 fj-lang LLVM compiler gaps closed
+ ZERO non-fj LOC in fajaros-x86 kernel build path. All 4 C mailbox
functions migrated to pure fj; vecmat_v8.c (585 LOC) deleted.
**FAJARQUANT_RUST_TO_FJ_PLAN closed** (2026-05-05): 7 algorithm modules
(~2,649 LOC Rust) ported to `stdlib/fajarquant.fj` (986 LOC, 39 fj fns);
70+ I/O pairs verified bit-exact at full f64 precision; 1 fj-lang core
change (analyzer wrapping_*); cumulative ~115min vs 10-17d budget (-99%).

> **Source of truth (FAJAROS_100PCT_FJ_PLAN):** `docs/FAJAROS_100PCT_FJ_PHASE_{0..7,4D,6_6}_FINDINGS.md` series. v33.2.0 tag (`b49e197b`) caps the plan.
>
> **Source of truth (perfection plan, predecessor):** `docs/HONEST_AUDIT_V33.md` — exit scorecard for all 25 perfection-plan work-items, written 2026-05-03. Predecessor audits: `HONEST_AUDIT_V32.md` (V32 re-audit), `HONEST_STATUS_V26.md` (V26 module status), `HONEST_AUDIT_V26.md` (V26 baseline), `HONEST_AUDIT_V17.md` (historical). Historical V13-V15 "100% production" claims were inflated 40-55% per V17 re-audit; V26 closed the remaining gap; V33 added ~280 tests + 5 audit scripts; v33.1.x/v33.2.0 closed the 9 fj-lang→OS compiler gaps surfaced by FAJAROS_100PCT.

**Core compiler (v1.0 → v0.5):** ALL COMPLETE — 506 + 739 + 40 + 80 + 130 tasks across
lexer, parser, analyzer, Cranelift, ML runtime, concurrency, OS runtime, generic enums,
RAII, async, test framework, iterators, f-strings.

**V06-V26 history:** see §3 Version History table or `CHANGELOG.md` (root) for
detailed entries.

**V17 critical bugs (9):** ALL FIXED. See `docs/HONEST_AUDIT_V17.md` §4 for the list.

### Key Documents

- **`docs/FAJAROS_100PCT_FJ_PLAN.md`** — 9-phase plan, **TERMINAL COMPLETE 2026-05-05**. ZERO non-fj LOC in fajaros kernel; 9/9 compiler gaps closed.
- **`docs/FAJAROS_100PCT_FJ_PHASE_{0..7,4D,6_6}_FINDINGS.md` series** — per-phase findings docs (closure proofs)
- **`docs/HONEST_AUDIT_V33.md`** — perfection-plan exit scorecard (2026-05-03, predecessor)
- **`docs/FAJAR_LANG_PERFECTION_PLAN.md`** — 25-item / 10-phase plan (CLOSED engineering-side)
- **`docs/HONEST_AUDIT_V32.md`** — V32 deep re-audit (2026-05-02)
- **`docs/TUTORIAL.md`** — 10-chapter language tutorial (P6.E3)
- `docs/HONEST_STATUS_V26.md` — V26 per-module status snapshot (still authoritative for module classification)
- `docs/HONEST_AUDIT_V26.md` / `V17.md` — historical audit trail
- `docs/V1_RULES.md` — coding conventions (mostly subsumed by §6 below)
- See §18 for full document index.

---

## 3. Current Status

### Core Compiler (v1.0-v0.5): ALL COMPLETE
- v1.0: 506 tasks (lexer, parser, analyzer, Cranelift, ML runtime) ✅
- v0.2: Codegen type system ✅ | v0.3: 739 tasks (concurrency, GPU, ML, self-hosting) ✅
- v0.4: 40 tasks (generic enums, RAII, async) ✅ | v0.5: 80 tasks (test framework, iterators, f-strings) ✅

### Current Totals (v33.2.0 "FAJAROS_100PCT TERMINAL COMPLETE", 2026-05-05)

```
Tests:     8,974 lib + 2,498+ integ (in 60+ test files) + 14 doc + 1 ignored
           ≈ 11,486+ total | 0 failures, 0 flakes
           Stress: 5/5 consecutive runs at `cargo test --lib -- --test-threads=64`
           LLVM: 162+ tests pass under --features llvm,native
           NEW since v33.0.0 (8967): 7 (atomics + @naked + @no_mangle +
           @no_vectorize stack + at_naked_emits/regular_fn + at_no_mangle/default
           + at_no_vectorize_stacks_with_kernel)
LOC:       ~449,000 lines of Rust (391+ files in src/)
Examples:  243 .fj programs + 6 multi-file real-project folders
           Binary: 18 MB release | MSRV: Rust 1.87
Modules:   42 lib.rs pub mods | 54 [x], 0 [sim], 0 [f], 0 [s] (54 logical)
           Source of truth: docs/HONEST_AUDIT_V33.md (P0-P9 closed; no demotions)
           V26 Phase A3 closed all 5 framework + 2 stub modules. 0 remaining.
CLI:       39 subcommands declared in src/main.rs, all production
CI:        7 GitHub Actions workflows (CI, Embedded, Docs, Release, nightly,
           release.yml, fuzz job) — release.yml triggers on v*.*.* tags
Feature Flags: websocket, mqtt, ble, gui, https, native (Cranelift), llvm, registry, cuda, wasm, playground-wasm
Quality:   0 clippy warnings (all 20 features) | 0 production .unwrap()
           0 rustdoc warnings (incl. --document-private-items)
           0 fmt diffs | 0 test failures | 0 flakes
           95.79% pub-item doc coverage (scripts/check_doc_coverage.sh)
           100% stdlib_v3 doc coverage (scripts/check_stdlib_docs.sh)
           0 error-code coverage gap (135 cataloged, 125 covered, 12 forward-compat;
             scripts/audit_error_codes.py --strict gap=0)
FajarQuant: extracted to standalone repo `fajarkraton/fajarquant` (V26 A4)
            wire-up via Cargo path dep + re-export shim, 16 integ tests pass
Threading: Real std::thread actors + Arc<Mutex> throughout interpreter
GPU:       RTX 4090 CUDA (9 PTX kernels, tiled matmul, async streams, 3x speedup)
Hooks:     Pre-commit rejects fmt drift (scripts/git-hooks/pre-commit, V26 A1.2)
Tags:      v32.1.0 (P0-P6 milestone, tag-only) +
           v33.0.0 (Perfection-Complete, GitHub Release w/ 5 binaries) +
           v33.1.0 (FAJAROS_100PCT partial — 8/9 gaps + Phase 6.6 12/17 stubs) +
           v33.1.1 (inline asm dialect fix + $ escape discovery) +
           v33.2.0 (FAJAROS_100PCT TERMINAL COMPLETE — 9/9 gaps,
                    vecmat_v8.c deleted, ZERO non-fj LOC in fajaros kernel)

Labeling: [x] = production (tested, works E2E)
          [sim] = simulated — NONE REMAINING (all upgraded to [x] in V21)
          [f] = framework (code exists, not callable from .fj)
          [s] = stub (near-empty placeholder)

Numbers verified by runnable commands as of 2026-05-05 (v33.2.0 sync).
Drift history → `docs/FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md` series + `docs/HONEST_AUDIT_V33.md`.
```

### Version History (V18 → v33.2.0)

> **Detailed entries:** `CHANGELOG.md` (root) — has V20.8 → v33.2.0 with full
> Added/Changed/Fixed/Removed/Stats sections. V18-V20 history lives in
> git log (`git log --oneline --grep="V1[89]\|V20"`).

| Version | Date | Highlight |
|---|---|---|
| **v33.2.0** "FAJAROS_100PCT TERMINAL COMPLETE" | 2026-05-05 | 9/9 fj-lang LLVM compiler gaps closed (G-A/B/C/G/H/I/K/M/N). vecmat_v8.c (585 LOC) DELETED in fajaros-x86; ZERO non-fj LOC in kernel build path. All 4 C mailbox fns migrated to pure fj. G-M closure: --code-model kernel implies LLVM `noredzone`. Source: `docs/FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md`. |
| **v33.1.0 / v33.1.1** "FAJAROS_100PCT partial + dialect fixes" | 2026-05-05 | 8/9 compiler gaps closed; Phase 6.6 12/17 fajaros runtime stubs migrated to @naked fn. Inline asm dialect fix + `$ → $$` escape pattern documented. |
| **V33 / v33.0.0** "Perfection-Plan Complete" | 2026-05-03 | PERFECTION_PLAN P0-P9 all closed engineering-side. 22/25 PASS; 3 await founder external action (F1/F3/A1, regression-gated). Adds: ~280 tests, 5 audit scripts, 13 docs (incl. HONEST_AUDIT_V33, TUTORIAL.md, CRATES_IO_PUBLISH_PLAN), 3 example folders, 2 benchmarks. v33.0.0 GitHub Release LIVE w/ 5 binaries. ~14h vs 218-336h estimate (-95%). Source: `docs/HONEST_AUDIT_V33.md`. |
| **v32.1.0** "P0-P6 milestone" | 2026-05-03 | Tag-only; release.yml failed on `llvm_compile_float_literal` stale assertion (fixed in v33.0.0). Use v33.0.0 for binaries. |
| **V32** "Audit Complete" | 2026-05-02 | HONEST_AUDIT_V32 deep re-audit (commits `ecd265a2..96843ab7`). 0 module demotions; 7626 lib + 2498 integ + 14 doc tests all green; 5 gaps surfaced (G1 LLVM O2 deferred opportunistic; G2/G3/G4/G5 closed via F1-F4 followup). FAJAR_LANG_PERFECTION_PLAN v1.0 enumerates remaining 25 work-items across 10 phases — closed in V33. |
| **V30.TRACK4 / V30.GEMMA3** | 2026-04-20 | FajarOS Nova v3.7.0 ext2/FAT32 disk harness + Gemma 3 1B 12 phases audit-PASS. Surfaces §6.10 rule. |
| **V29.P3 / V29.P1** | 2026-04-16 | NX triple closure (security_enable_smep/smap + nx_enforce); @noinline+@inline+@cold lexer entries closing silent-build-failure class. |
| **V27.5 / V27 / V26** | 2026-04-11..14 | Compiler prep (AI scheduler builtins, @interrupt, refinement params, Cap<T>); Hardened (0 doc warnings); V26 "Final" (80/80 stress, 0 unwraps, §6.7 rule). |

> V18-V25 + detailed V26-V30 entries: full detail in `CHANGELOG.md` + `git log --oneline --grep="V[12][0-9]"`.

### FajarOS (two platforms)
- **FajarOS v3.0 "Surya"** (ARM64): Verified on Radxa Dragon Q6A. 65+ commands.
- **FajarOS Nova** (x86_64): **100% Fajar Lang as of 2026-05-05** (FAJAROS_100PCT_FJ_PLAN TERMINAL COMPLETE; ZERO non-fj LOC in kernel build path). V26 LLM E2E (SmolLM-135M v5/v6) + V31 IntLLM Phase D + V30.GEMMA3 Gemma 3 1B in-kernel paths, 14 LLM shell commands. Boot to `nova>` reliably in QEMU. 5/5 gemma3-e2e PASS gates verified at every Phase-closure commit. Source: fajaros-x86 commit `541db09` (Phase 4.D-G + vecmat_v8.c deletion).

### FajarQuant (separate repo since 2026-04-11)
- **`fajarkraton/fajarquant`** (standalone) — extracted from `src/runtime/ml/fajarquant/` + `turboquant.rs` in V26 Phase A4 split. Algorithm + paper + data + reproducibility scripts now live there.
- fajar-lang depends via Cargo path/git dep + thin re-export shim in `src/runtime/ml/{fajarquant/mod.rs, turboquant.rs}` — zero changes to `interpreter/eval/builtins.rs` call sites.
- 29 unit tests moved with the algorithm (now in fajarquant repo). 16 integration tests stay in `tests/fajarquant_*.rs` to verify the wire-up.
- **All Phase C work** (multi-model validation, perf benchmarks, paper polish) happens in the new repo. See `docs/V26_PRODUCTION_PLAN.md` v1.2.

---

## 4. Architecture Overview

> **Full architecture:** `docs/ARCHITECTURE.md` — module contracts, data flow, dependency graph.

### 4.1 Compilation Pipeline (one-line summary)

`source.fj → lexer → parser → analyzer → {interpreter | vm | cranelift | llvm} → {os runtime | ml runtime}`

- **Lexer** (`src/lexer/`): `&str → Vec<Token>` (LE001-LE008)
- **Parser** (`src/parser/`): `Vec<Token> → Program` (recursive descent + Pratt, 19 levels)
- **Analyzer** (`src/analyzer/`): `&Program → Result<(), Vec<SemanticError>>` (types, scope, @kernel/@device contexts)
- **Backends:** tree-walking interpreter, bytecode VM (45 opcodes), Cranelift (embedded), LLVM (production w/ 30 enhancements)

### 4.2 Top-Level Types

```rust
enum FjError { Lex, Parse, Semantic, Runtime }
enum Value { Null, Int, Float, Bool, Char, Str, Array, Tuple, Tensor,
             Map, Struct, Enum, Function, BuiltinFn, Pointer, Optimizer, Layer }
```

### 4.3 Dependency Direction (STRICT)

`main → interpreter → analyzer → parser → lexer` ; `interpreter → runtime/{os,ml}` ; `main → codegen`. **Forbidden:** any upward dep, parser → interpreter, runtime/os ↔ runtime/ml, any cycle.

### 4.4 Key Architectural Details

- `eval_source()` runs full pipeline; REPL uses `analyze_with_known()` for prior names
- Warnings (SE009 UnusedVariable, SE010 UnreachableCode) do NOT block execution
- `EvalError::Control` is boxed (avoids large_enum_variant clippy warning)
- `loss` is a keyword — cannot use as variable name
- `parse_int`/`parse_float` return `Value::Enum { Ok/Err }`, not RuntimeError

---

## 5. Language Essentials (Quick Reference)

### 5.1 Keywords

```
Control:      if else match while for in return break continue loop
Declarations: let mut fn struct enum impl trait type const
Types:        bool i8-i128 u8-u128 isize usize f32 f64 str char void never
ML:           tensor grad loss layer model
OS:           ptr addr page region irq syscall
Module:       use mod pub extern as
Literals:     true false null
Annotations:  @kernel @device @safe @unsafe @ffi
```

### 5.2 Operator Precedence
19 levels (lowest→highest): Assignment → Pipeline(`|>`) → Logic(`||`,`&&`) → Bitwise → Equality → Comparison → Range → Shift → Add → Mul → Power(`**`) → Cast(`as`) → Unary → Try(`?`) → Postfix(`.`,`()`,`[]`) → Primary. Full table: `docs/GRAMMAR_REFERENCE.md`.

### 5.3 Context Annotations (Unique Feature)

```
@unsafe --> Full access to all features
@kernel --> OS primitives, no heap, no tensor
@device --> Tensor ops, no raw pointer, no IRQ
@safe   --> Default; no hardware, no raw pointer, no direct tensor (safest subset)
```

| Operation | @safe | @kernel | @device | @unsafe |
|-----------|-------|---------|---------|---------|
| `let x = 42` | OK | OK | OK | OK |
| `String::new()` | OK | ERROR KE001 | OK | OK |
| `zeros(3,4)` / `relu()` | ERROR | ERROR KE002 | OK | OK |
| `alloc!(4096)` | ERROR | OK | ERROR DE002 | OK |
| `*mut T` dereference | ERROR | OK | ERROR DE001 | OK |
| `irq_register!()` | ERROR | OK | ERROR DE002 | OK |
| Call `@device` function | OK | ERROR KE003 | OK | OK |
| Call `@kernel` function | OK | OK | ERROR DE002 | OK |

### 5.4 Fajar Lang Syntax Samples

```fajar
// Variables
let x: i32 = 42
let mut counter = 0
const MAX: usize = 1024

// Functions
fn add(a: i32, b: i32) -> i32 { a + b }

// Structs & Enums
struct Point { x: f64, y: f64 }
enum Shape { Circle(f64), Rect(f64, f64) }

// Control flow (expressions!)
let max = if a > b { a } else { b }
let label = match x { 0 => "zero", _ => "other" }

// Pipeline operator
5 |> double |> add_one  // = add_one(double(5))

// Error handling
let val = risky_fn()?   // propagate with ?

// Cross-domain bridge pattern
@kernel fn read_sensor() -> [f32; 4] { ... }
@device fn infer(x: Tensor) -> Tensor { ... }
@safe fn bridge() -> Action {
    let raw = read_sensor()
    let result = infer(Tensor::from_slice(raw))
    Action::from_prediction(result)
}
```

---

## 6. Coding Rules (Non-Negotiable)

> **Full production rules:** `docs/V1_RULES.md`

### 6.1 Core Principles

```
CORRECTNESS > SAFETY > USABILITY > PERFORMANCE
"If it compiles in Fajar Lang, it's safe to deploy on hardware."
```

1. **CORRECTNESS** first — no undefined behavior, no incorrect results
2. **EXPLICIT** over implicit — no hidden behavior
3. **ERRORS are values** — never panic in library code
4. **TESTS before implementation** — TDD always
5. **SMALL functions** — max 50 lines per function
6. **ONE concern per module** — strict single responsibility

### 6.2 Rust Code Style

```
Types/Traits/Enums:  PascalCase     -> TokenKind, FjError
Functions/vars/mods: snake_case     -> tokenize(), token_count
Constants/statics:   SCREAMING_CASE -> MAX_RECURSION_DEPTH
Lifetimes:           short lowercase -> 'src, 'a, 'ctx
Type params:         PascalCase      -> T, U
Error codes:         PREFIX + NUMBER -> SE004, KE001, CE003
```

### 6.3 Error Handling Rules

- **NEVER** use `.unwrap()` in `src/` — only allowed in `tests/` and `benches/`
- **NEVER** `panic!()` in library code — return `Result` or `Option`
- **ALLOWED:** `.expect("reason")` with meaningful message in `main.rs` only
- **USE** `thiserror` for all error types
- **COLLECT** all errors, don't stop at first — show all at once
- **ALL** errors must have error codes and source spans

### 6.4 Safety Rules

- **ZERO** `unsafe {}` blocks outside `src/codegen/` and `src/runtime/os/`
- Every `unsafe` block MUST have `// SAFETY:` comment
- No raw pointer dereference outside `@kernel`/`@unsafe` context

### 6.5 Code Review Checklist (Before Marking Task Done)

- [ ] No `.unwrap()` in `src/` (only in tests)
- [ ] No `unsafe` without `// SAFETY:` comment
- [ ] No wall-clock `assert!(elapsed < threshold)` in unit tests (see §6.7)
- [ ] All `pub` items have doc comments
- [ ] `cargo test` — all pass
- [ ] `cargo test --lib -- --test-threads=64` — passes 5x in a row (stress test)
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo fmt` — formatted
- [ ] New functions have at least 1 test
- [ ] Task file updated

### 6.6 Documentation Integrity Rules (Non-Negotiable)

These rules exist because of GAP_ANALYSIS_V2 + HONEST_AUDIT_V32 findings. They prevent inflated claims.

1. **[x] means END-TO-END working.** A task is only [x] if a user can actually USE the feature. Type definitions with passing unit tests are `[f]` (framework), not `[x]`.

2. **Every task needs a verification method.** "Verify: send HTTP request and receive response" not "Verify: unit test passes".

3. **No inflated statistics.** Documentation must match actual code capability. Reference HONEST_AUDIT_V32.md (latest) or HONEST_AUDIT_V26.md / GAP_ANALYSIS_V2.md (historical) for accurate LOC/status.

4. **No stub plans.** Every option in a plan must have full task tables. No `*(placeholder)*` lines.

5. **Audit before building.** Before creating new plans, verify previous plan claims are backed by real code.

6. **Distinguish real vs framework.** When a module has type definitions but no external integration (no networking, no FFI, no solver calls), document it honestly as "framework — needs X integration".

### 6.7 Test Hygiene Rules (No Wall-Clock Assertions in Unit Tests)

> **Reason:** V26 A1.3: 14 tests asserting `elapsed < threshold` on
> microsecond work flaked ~20% under `--test-threads=64` (commit `13aa9e3`).

1. **NEVER** `assert!(elapsed < N_ms)` in unit tests for microsecond-scale
   work. Wall-clock timing is unreliable under parallel load.
2. **DO** put perf regression detection in **criterion benchmarks**, not unit tests.
3. **IF** a unit test must check timing, set threshold **≥10x** expected value,
   or use a noise-floor pattern treating sub-ms differences as passing.
4. **CI safeguard:** `flake-stress` job runs `--test-threads=64` 5x per push.
5. Antipattern: `assert!(start.elapsed() < Duration::from_millis(50))` on µs work.
   Acceptable: same with `500` (10x) for jitter immunity.

### 6.8 Plan Hygiene Rules (No Inflated Estimates, No Skipped Decisions)

> **Reason:** V26 surfaced 8 systemic distortion patterns. Examples:
> "174 unwraps" was actually 3 (58× inflation). "1 flaky test" was 14.
> fajaros-x86 had 40 unpushed commits for 5 days. Full evidence:
> `docs/V26_PRODUCTION_PLAN.md` §10.5.

When writing or reviewing any plan, audit, or status doc:

1. **Pre-flight audit mandatory.** Every Phase starts with B0/C0/D0
   subphase that hands-on verifies baseline via runnable commands and
   produces `docs/V26_<phase>_FINDINGS.md`. Downstream blocked until committed.
2. **Verification columns must be runnable commands.** Literal command
   whose output can be checked — not "test passes"/"feature works".
3. **Prevention layer per phase.** Every class-of-bugs fix ships a
   pre-commit hook, CI job, or CLAUDE.md rule. The patch alone is not the deliverable.
4. **Multi-agent audit cross-check.** Numbers from parallel sub-agents
   must be manually re-verified with `Bash` before commit.
5. **Surprise budget +25% min, tracked per commit.** Default +25%,
   high-uncertainty +30%. Tag variance: `feat(v26-b1): X [actual 3h, est 2h, +50%]`.
6. **Decision gates must be mechanical.** Decisions produce a committed
   file that pre-commit/commit-msg hooks check, mechanically blocking downstream.
7. **Public-facing artifact sync.** When fixing CLAUDE.md/status/plan,
   audit README badges, git tags, GitHub Releases, project description same session.
8. **Multi-repo state check.** Before cross-repo sessions, run `git status -sb`
   AND `git rev-list --count origin/main..main` for all local repos.

**Self-check before any plan/audit commit:**
```
[ ] Pre-flight audit (B0/C0/D0) exists for the Phase?           (Rule 1)
[ ] Every task has a runnable verification command?             (Rule 2)
[ ] At least one prevention mechanism added (hook/CI/rule)?     (Rule 3)
[ ] Agent-produced numbers cross-checked with Bash?             (Rule 4)
[ ] Effort variance tagged in commit message?                   (Rule 5)
[ ] Decisions are committed files, not prose paragraphs?        (Rule 6)
[ ] Internal doc fixes audited for public-artifact drift?       (Rule 7)
[ ] Multi-repo state check run before starting work?            (Rule 8)
```
Eight NO answers = revert. Eight YES answers = ship.

### 6.9 Research Integrity Rules (Algorithm Validation)

> **Reason:** V26 Phase C1.6 surfaced a chain of failures that nearly shipped
> incorrect FajarQuant paper claims. The original PPL claims (FQ 80.1 vs TQ
> 117.1 vs KIVI 231.9 at 2-bit) were generated by a custom prefix+target
> post-hoc cache mutation protocol. When switched to canonical R-α.1 model
> surgery (matching KIVI/KVQuant/SKVQ literally), FajarQuant LOSES to
> TurboQuant by 5.6× on the same model. The original "win" was a protocol
> artifact, not an algorithmic advantage. Worse: my benchmark TurboQuant was
> a "naive TQ" missing the published method's outlier handling, so even the
> head-to-head was unfair in TurboQuant's disfavor. Full evidence:
> `docs/V26_C1_6_PATH_B_PLAN.md` "Why this plan exists", commits `c9b2ff5`
> → `3015545` → R-α.1 smoke test. Companion memory:
> `memory/feedback_research_integrity.md`.

When designing/evaluating any algorithm whose results appear in a paper,
README, or publishable artifact:

1. **No paper claim without canonical-protocol benchmark.** Quantitative
   claims require the canonical protocol from ≥2 reference papers. Custom
   "convenience" protocols introduce invisible bias. If FP16 baseline is
   implausible vs literature, the protocol is broken — fix before measuring.
2. **Literature review precedes algorithm design.** Sweep ≥8-10 papers
   (24mo) before code edits. Synthesize landscape table first.
3. **Baseline parity — port full features, not naive versions.** Port ALL
   published features (outlier handling, calibration, grids). Document
   unported features explicitly. Better to skip a baseline than strawman it.
4. **Calibrated > per-chunk for data-driven decompositions.** Calibrate
   PCA/SVD/rotations/scales ONCE on representative data, reuse. Per-chunk
   recomputation accumulates noise.
5. **Outlier handling non-negotiable for LLM quantization.** Require top-K
   preservation, per-coord adaptive bits, OR rotation (Hadamard/learned).
   Ablate with/without to quantify.
6. **Algorithmic validation precedes paper validation.** Results-section
   text is LAST. Methodology/related-work can be earlier; claims cannot.
7. **Pre-publication audit gate (mechanical).** `verify_paper_tables.py
   --strict` as pre-commit hook + required CI check. Audit README/blog/
   release notes for claims not in the verify script.

**Self-check before publishing any algorithm-claim artifact:**
```
[ ] Canonical protocol identified from ≥2 reference papers?       (R1)
[ ] Literature review covers ≥8 recent papers in the area?         (R2)
[ ] All baselines ported with their full feature set?              (R3)
[ ] Data-driven decompositions calibrated, not per-chunk?          (R4)
[ ] Explicit outlier handling included in the method?              (R5)
[ ] All quantitative paper claims backed by canonical benchmarks?  (R6)
[ ] verify_paper_tables.py --strict exit 0 before publishing?      (R7)
```
Seven YES = publish. Any NO = block.

### 6.10 Filesystem Roundtrip Coverage Rule

> **Reason:** V30 Track 4 surfaced `ext2_create` returning -1 on freshly-mkfs'd
> disk — latent ≥2 releases because no regression harness exercised the write
> path E2E. Code-path audits alone miss invariants that only fail under disk I/O.

When adding/modifying any on-disk FS write path in the kernel:

1. **Must have a Makefile regression target** following the
   `test-fs-roundtrip` pattern (shell QEMU + grep invariants on serial log).
   Refs: `test-security-triple-regression`, `test-gemma3-e2e`, `test-fs-roundtrip`.
2. **Attach test disk with `-boot order=d`** to force CDROM boot. Otherwise
   QEMU boots a disk whose `0x55 0xAA` signature triple-faults before any serial.
3. **Prefer in-kernel `mkfs`+`mount`+write** over host-built images when layout
   is custom. The honest roundtrip is what the kernel does, not the host.
4. **Surface pre-existing bugs via NOTE lines**, not hidden. A silently-passing
   gate despite known-broken path is worse than no gate.

**Self-check before marking an FS write task `[x]`:**
```
[ ] Makefile regression target exists and is green?              (R1)
[ ] Test disk attached with -boot order=d?                        (R2)
[ ] Either kernel-owned mkfs or bytes-identical host layout?      (R3)
[ ] Known-broken paths surfaced as NOTE, not hidden?              (R4)
```
Four YES = ship. Any NO = block.

### 6.11 Training Script Interruption-Safety Rule

> **Reason:** V31 Phase D Base c.1 hang on 2026-04-22: training ran 1h42m,
> then laptop hit battery-low → OS suspend → dead HF CDN TCP sockets → urllib3
> blocked forever on socket-read → process stayed in State=R with 0 step
> progress for **8.5 hours** before user noticed. No intermediate checkpoint
> had been saved, no watchdog, no read timeout — whole run lost, restart
> from step 0. Full forensics: `memory/feedback_hf_streaming_hang.md`.

When adding/modifying any production training script (anywhere that
loops `for batch in stream: loss = model(...); loss.backward(); opt.step()`):

1. **Must save intermediate checkpoints.** `ckpt_every` wired into the
   step loop, not just a one-shot save at the end. Atomic write (.tmp +
   `os.replace`) so SIGKILL mid-save cannot leave partial files. Rotate
   `keep_last_n_ckpts` to cap disk usage.
2. **Must support `--resume <path>` and `--resume-auto`.** Resume loads
   model + optimizer + LR-scheduler state bit-exactly (unit test:
   pre-save vs post-resume loss within 1e-4). LambdaLR / OneCycleLR /
   etc. all have `state_dict()` — use it. Step counter continues from
   true total, not relative.
3. **Must arm a step-idle watchdog.** A daemon thread that SIGTERMs
   the main process if the step counter doesn't advance for N seconds
   (default 1800 = 30 min). Skip during warmup (before first touch).
   Single-shot: fire once, then exit. External orchestrator owns restart
   via `--resume-auto`.
4. **Must set per-chunk read timeouts on streaming data sources.**
   `HF_DATASETS_DOWNLOAD_TIMEOUT=60` + `HF_HUB_DOWNLOAD_TIMEOUT=60` at
   module import (via `os.environ.setdefault` so external overrides
   still win). Wrap the iterator in a retry loop that rebuilds on
   transient network exceptions (socket.timeout, ConnectionError,
   requests/urllib3 read-timeout variants). Seed offset by attempt
   number so retries see a different shuffle order.
5. **Must have a `test-*-watchdog` Makefile regression.** The gate
   exercises at minimum: watchdog real-thread fire, default on_fire
   SIGTERM delivery, ckpt rotation, --resume bit-exact load, retry_iter
   on transient exception. Pre-push hook runs it when training code
   changes.

Reference implementation: `fajarquant/python/phase_d/{intllm/train.py,
intllm/data.py, scripts/train_*.py}` V31.C.P6.1–P6.5. Make target:
`make test-train-watchdog`.

**Self-check before marking a production training task `[x]`:**
```
[ ] ckpt_every wired into step loop, atomic write, rotation?       (R1)
[ ] --resume / --resume-auto with bit-exact state restoration?     (R2)
[ ] StepWatchdog armed with sensible default (e.g. 30 min)?        (R3)
[ ] HF read timeouts + retry_iter on transient network errors?     (R4)
[ ] test-*-watchdog Makefile gate green + pre-push hooked?         (R5)
```
Five YES = ship. Any NO = block. A training script that loses hours
of GPU time to a single interruption is NOT production-ready.

---

## 7. Error Code System

```
Format: [PREFIX][NUMBER]

LE = Lex Error        (LE001-LE008)     --  8 tokenization problems
PE = Parse Error      (PE001-PE010)     -- 10 syntax problems
SE = Semantic Error   (SE001-SE016)     -- 16 type/scope problems
KE = Kernel Error     (KE001-KE004)     --  4 @kernel context violations
DE = Device Error     (DE001-DE003)     --  3 @device context violations
TE = Tensor Error     (TE001-TE009)     --  9 shape/type problems
RE = Runtime Error    (RE001-RE008)     --  8 execution problems
ME = Memory Error     (ME001-ME010)     -- 10 ownership/borrow problems
CE = Codegen Error    (CE001-CE010)     -- 10 native compilation problems

Total: 78 error codes across 9 categories (verified by grep on docs/ERROR_CODES.md)
```

Key errors:
- **SE004** TypeMismatch | **KE001** HeapAllocInKernel | **KE002** TensorInKernel
- **DE001** RawPointerInDevice | **ME001** UseAfterMove | **RE003** StackOverflow

> **Full catalog:** `docs/ERROR_CODES.md`

---

## 8. TDD Workflow (Per Task)

> **Full workflow:** `docs/V1_WORKFLOW.md`

```
+-- 1. THINK   -> Read task from V1_TASKS.md
|               -> Check V1_SKILLS.md for implementation patterns
|
+-- 2. DESIGN  -> Write PUBLIC INTERFACE first (fn signatures, structs, enums)
|
+-- 3. TEST    -> Write tests BEFORE implementation (RED phase)
|
+-- 4. IMPL    -> Write MINIMAL code to make tests pass (GREEN phase)
|
+-- 5. VERIFY  -> cargo test && cargo clippy -- -D warnings && cargo fmt
|
+-- 6. UPDATE  -> Mark task [x] in V1_TASKS.md, move to next task
```

### Quality Gates

**Per-Task:** All tests pass, no unwrap in src, pub items documented, clippy clean
**Per-Sprint:** No regressions, benchmarks compared, at least 1 new example
**Per-Milestone:** Full suite passes, all examples run, cargo doc compiles, release notes

---

## 9. Testing Strategy

### 9.1 Test Suite: ~10,138 tests (7,626 lib + 2,498 integ in 55 files + 14 doc + 1 ignored)

> Numbers re-verified 2026-05-02 via `cargo test --lib`, `cargo test --test '*'`,
> `cargo test --doc` per HONEST_AUDIT_V32 §2. Stress test (V26 A1.4) runs
> `cargo test --lib -- --test-threads=64 × 5` per push (5/5 PASS audit-day).

### 9.2 Test Naming Convention

```rust
// Pattern: <what>_<when>_<expected>
fn lexer_produces_int_token_for_decimal_literal() { ... }
fn s1_1_eval_source_runs_analyzer() { ... }
```

### 9.3 Coverage Targets (v1.0)

| Component | Minimum | Target |
|-----------|---------|--------|
| Lexer | 95% | 100% |
| Parser | 90% | 100% |
| Analyzer | 90% | 95% |
| Codegen | 85% | 95% |
| Interpreter | 85% | 95% |
| Runtime | 80% | 90% |
| Overall | 85% | 90% |

---

## 10. Git & Contributing

> **Full guide:** `docs/CONTRIBUTING.md`. CHANGELOG: root `CHANGELOG.md`.

- **Branches:** `main` (stable, tagged) | `feat/XXX` | `fix/XXX` | `release/vX.Y`
- **Commits:** `<type>(<scope>): <desc>` — types: feat/fix/test/refactor/docs/perf/ci/chore; scopes: lexer/parser/analyzer/interp/runtime/vm/codegen/cli/stdlib + V26 phase scopes (`v26-a1`..`v26-c4`)
- **Milestones:** v0.2-v1.0 (6 monthly Cranelift+ML+ownership) ✅ DONE; v0.3 "Dominion", v0.4 "Sovereignty", v0.5 "Apex" ✅ DONE

---

## 11. Standard Library Overview

> **Full API:** `docs/STDLIB_SPEC.md`. Discover via REPL `:help` or grep `src/interpreter/builtins.rs`.

Modules: `std::{io,collections,string,math,convert}` + `os::{memory,irq,syscall,io}` + `nn::{tensor,ops,activation,loss,layer,autograd,optim,metrics}`. Built-in globals: `Some/None/Ok/Err` constructors; `print/println/len/type_of/assert/assert_eq/panic/todo/dbg`; `PI/E` constants.

---

## 12. Security Model Summary

**Philosophy:** "Security by Construction" -- if it compiles, it's safe.

| Pillar | Mechanism | Enforcement |
|--------|-----------|-------------|
| Memory Safety | No use-after-free, no null deref, no buffer overflow | Compiler (ownership + borrow) |
| Context Isolation | @kernel != @device, no heap in kernel, no tensor in kernel | Compiler (context analyzer) |
| Type Safety | PhysAddr != VirtAddr, tensor shape check, no implicit cast | Compiler (type checker) |

Key features: ownership lite (no lifetime annotations), borrow rules (many &T OR one &mut T), null safety (Option<T>), no implicit type conversions, exhaustive match, integer overflow checking.

---

## 13. Performance Targets

Priority: **CORRECTNESS > SAFETY > PERFORMANCE**. Per-component v0.1→v1.0 targets in `docs/STDLIB_SPEC.md` + `benches/`. Binary size <10MB. Native fibonacci(30) <50ms.

---

## 14. Cargo.toml Dependencies

> **Source of truth:** `Cargo.toml` itself. Keys: `thiserror` (errors), `miette` (display),
> `clap` (CLI), `ndarray` (tensors), `tokio` + `tower-lsp` (LSP), `cranelift-*` + `inkwell` (codegen,
> feature-gated), `criterion` (benches), `proptest` (property tests).

---

## 15. Key Design Decisions

Interpreter: tree-walking + bytecode VM. Codegen: Cranelift (embedded) + LLVM (production). Tensors: ndarray. Errors: collect-all + miette display. Env: `Rc<RefCell<>>` for closures. Parser: Pratt (19 levels). Generics: monomorphization. Borrow: NLL-like without lifetimes. Full table: see git history.

---

## 16. Quick Commands

```bash
# Build & test (mandatory before commit)
cargo build [--release]
cargo test --lib                                 # 7,626 lib tests
cargo test --lib -- --test-threads=64            # stress (V26 §6.7 rule)
cargo clippy --lib -- -D warnings                # MUST pass
cargo fmt -- --check                             # MUST pass

# Run Fajar Lang programs
cargo run -- run examples/hello.fj               # default (interpreter)
cargo run -- run --vm file.fj                    # bytecode VM
cargo run -- check file.fj                       # type-check only
cargo run -- repl                                # interactive REPL
cargo run -- dump-tokens|dump-ast file.fj        # debug

# Project lifecycle
cargo run -- new <name> | build | fmt | lsp | doc | demo | watch

# Feature flags (cargo run --features X -- run file.fj)
#   websocket | mqtt | ble | https | gui | native (Cranelift) | llvm | cuda
```

---

## 17. Repository Structure

`src/`: lexer, parser, analyzer, interpreter, vm, codegen/{cranelift,llvm}, runtime/{os,ml}, gpu_codegen, dependent, verify, lsp, package, distributed, wasi_p2, ffi_v2, formatter, selfhost, const_*, gui (gated). **Glob discovery preferred** — use `Glob "src/**/mod.rs"` rather than reading this map. Companion dirs: `tests/` (46 files), `examples/` (231 .fj), `docs/` (157), `benches/`, `fuzz/`, `audit/`, `scripts/`, `.github/workflows/`.

---

## 18. Document Quick-Reference Index

| When You Need... | Read This |
|---|---|
| **FAJAROS_100PCT_FJ_PLAN (TERMINAL COMPLETE 2026-05-05)** | **`docs/FAJAROS_100PCT_FJ_PLAN.md`** + per-phase findings `docs/FAJAROS_100PCT_FJ_PHASE_{0..7,4D,6_6}_FINDINGS.md`. 9/9 fj-lang LLVM compiler gaps closed (G-A through G-N); ZERO non-fj LOC in fajaros kernel build path. v33.2.0 caps the plan. |
| **Latest audit (V33) — perfection plan** | **`docs/HONEST_AUDIT_V33.md`** — exit scorecard for all 25 perfection-plan work-items (2026-05-03) |
| **Current per-module status** | **`docs/HONEST_STATUS_V26.md`** — V26 module classification (still authoritative; V33 added no demotions) |
| **Perfection plan (CLOSED engineering-side)** | **`docs/FAJAR_LANG_PERFECTION_PLAN.md`** — 25-item / 10-phase plan; per-phase findings in `docs/FAJAR_LANG_PERFECTION_PHASE_{1..8}_FINDINGS.md` |
| **Tutorial / book** | **`docs/TUTORIAL.md`** — 10 chapters basics → robot control loop (P6.E3) |
| **crates.io publish unblock** | `docs/CRATES_IO_PUBLISH_PLAN.md` — F3 closure sequence |
| **LLVM O2 miscompile filing draft** | `docs/LLVM_O2_VECMAT_MISCOMPILE_REPRO.md` — A1 paste-ready upstream filing |
| **Audit predecessor (V32)** | `docs/HONEST_AUDIT_V32.md` — deep re-audit 2026-05-02 |
| **V26 audit trail** | `docs/HONEST_AUDIT_V26.md` / `docs/V26_PRODUCTION_PLAN.md` — V26 baseline + plan |
| **Version history (V18-V33)** | **`CHANGELOG.md`** (root) — full Added/Changed/Fixed/Stats per version |
| **FajarQuant standalone** | **`~/Documents/fajarquant/`** — extracted 2026-04-11. Algorithms, paper, data, scripts. fajar-lang depends via path/git Cargo dep + re-export shim |
| **FajarQuant Phase E** (bilingual ID+EN ternary kernel-context LLM, Tier 1+2; Tier 3 → Phase F) | Plan: `~/Documents/fajarquant/docs/FJQ_PHASE_E_BILINGUAL_KERNEL_PRODUCTION_PLAN.md` v1.9. State: E0+E1+E2.0+E2.4+E2.1 CLOSED. Two honest NEGATIVE results (E2.4 balanced_calib, E2.1 Hadamard) demoted to F.5/F.6. Bilingual corpus v1.0 = 25.67 B tokens 60:40 ID:EN. Findings + decision docs in `~/Documents/fajarquant/docs/FJQ_PHASE_E_*`. Full per-sub-phase detail in `MEMORY.md`. |
| **Honest codebase audit (older)** | `docs/HONEST_AUDIT_V17.md` (V17 baseline) |
| **Coding rules** | CLAUDE.md §6 (V1_RULES.md is archived in docs/archive/) |
| **Completed core tasks** | `docs/V05_PLAN.md` + `docs/V04_PLAN.md` + `docs/V03_TASKS.md` + `docs/V1_TASKS.md` |
| **Implementation plans** | `docs/NEXT_IMPLEMENTATION_PLAN_V{2-8}.md` — all with detailed task tables |
| Language syntax, keywords, types | `docs/FAJAR_LANG_SPEC.md` |
| Formal EBNF grammar | `docs/GRAMMAR_REFERENCE.md` |
| Component contracts, data flow | `docs/ARCHITECTURE.md` |
| Error code catalog | `docs/ERROR_CODES.md` |
| Standard library API | `docs/STDLIB_SPEC.md` |
| Security model | `docs/SECURITY.md` |
| Example programs | `docs/EXAMPLES.md` |
| Git workflow | `docs/CONTRIBUTING.md` |
| OS plans | `docs/V30_PLAN.md` + `docs/COMPILER_ENHANCEMENT_PLAN.md` |
| **FajarOS 100% Fajar Lang plan** | **`docs/FAJAROS_100PCT_FJ_PLAN.md`** — 9-phase migration: remove all .S/.c/.cpp from kernel build path + close 3 fj-lang gaps (LLVM atomics, @naked, @no_mangle); ~21-32d realistic |

---

## 19. Troubleshooting Quick Reference

| Problem | Solution |
|---------|----------|
| Linker not found | `sudo apt-get install build-essential` |
| Test timeout / infinite loop | MAX_RECURSION_DEPTH = 64 (debug) / 1024 (release) |
| Random test failures | Each test must create fresh `Interpreter::new()` |
| Gradient mismatch | Use epsilon `1e-4`, not exact equality |
| Slow compilation | `cargo check` (no codegen) |
| Claude forgot context | "Read FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md + MEMORY.md resume protocol" |

---

*CLAUDE.md Version: 33.2 (**v33.2.0 FAJAROS_100PCT TERMINAL COMPLETE 2026-05-05**: 9/9 fj-lang LLVM compiler gaps closed; ZERO non-fj LOC in fajaros-x86 kernel build path; vecmat_v8.c DELETED + all 4 mailbox fns migrated to pure fj). Quality gates: 8974 lib tests, 0 clippy / 0 fmt / 0 unwrap warnings. Tags v33.{0.0, 1.0, 1.1, 2.0} LIVE on GitHub Releases. Source of truth: `docs/FAJAROS_100PCT_FJ_PHASE_*_FINDINGS.md` + `docs/HONEST_AUDIT_V33.md`. Detail → `CHANGELOG.md` + `MEMORY.md`. Active rules: §6.1–§6.11.*
*Last Updated: 2026-05-05*
