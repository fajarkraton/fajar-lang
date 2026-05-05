---
phase: 0 — Pre-flight audit for fj-lang self-hosting plan
status: AUDIT COMPLETE 2026-05-05; recommend Stage-1-Subset path (not full self-host) as first achievable milestone
budget: 0.5d audit + ~5-15d realistic plan delivery
actual: ~30min Claude time
artifacts:
  - This findings doc
  - Inventory of existing self-host infrastructure
prereq: discussion of "what 100% Fajar Lang means" (FajarOS done, FajarQuant done, training infeasible, compiler self-host feasible)
---

# fj-lang Self-Hosting — Phase 0 Findings

> **Phase 0 of self-host plan.** Audits existing self-host infrastructure
> in fj-lang, identifies what's done vs stub, calibrates realistic scope.
> **Outcome: existing "self-host" is theatre — Stage 1/2 are Rust
> simulations, not actual fj-source compilers. Pragmatic path forward
> is Stage-1-Subset milestone, not full self-host.**

## 0.1 — What "self-host" means

A compiler is self-hosted when it can compile its own source code. The
classic 3-stage validation:

- **Stage 0**: bootstrap compiler in some other language (here: Rust)
- **Stage 1**: Stage-0 compiles the self-hosted compiler source → Stage-1 binary
- **Stage 2**: Stage-1 compiles the self-hosted compiler source → Stage-2 binary
- **Triple test**: Stage 1 binary == Stage 2 binary (byte-identical or behavior-identical)

When `Stage 1 ≡ Stage 2`, the fixed point proves self-hosting.

Languages that achieved this milestone: Rust (2010), Go (2015),
TypeScript (2022), Zig (2024). Marketing impact: HIGH — proves
language can express its own complexity.

## 0.2 — Current state of fj-lang self-host infrastructure

### What's REAL

| Path | LOC | Status |
|---|---|---|
| `src/lexer/` | ~2K | ✅ Production Rust lexer |
| `src/parser/` | ~13.7K | ✅ Production Rust parser (recursive descent + 19-level Pratt) |
| `src/analyzer/` | ~24.2K | ✅ Production Rust type checker / context analyzer |
| `src/codegen/cranelift+llvm` | ~96.6K | ✅ Production codegen backends |
| **Total Rust compiler core** | **~135K LOC** | **Production** |

This is the fj-lang compiler that exists and works today.

### What's PARTIAL (skeleton/scaffolding)

| Path | LOC | Status |
|---|---|---|
| `stdlib/lexer.fj` | 513 (4 fns) | ⚠️ Skeleton — token kind constants, basic char predicates, no actual tokenize fn |
| `stdlib/parser.fj` | 784 (26 fns) | ⚠️ Partial — some statement parsers, no full Pratt expr |
| `stdlib/analyzer.fj` | 432 (11 fns) | ⚠️ Partial — basic type table, no full type inference |
| `stdlib/codegen.fj` | 321 | ⚠️ Partial — IR sketch |
| **Total fj-source self-host** | **~2,050 LOC** | **~2-3% of Rust core** |

### What's THEATRE (Rust simulation, NOT real self-host)

| Path | LOC | What it actually is |
|---|---|---|
| `src/selfhost/bootstrap.rs` | 562 | Bootstrap data structures (`Stage`, `StageResult`) + format reporters — does NOT spawn fj compilations |
| `src/selfhost/bootstrap_v2.rs` | 1066 | Stage1Compiler/SubsetDefinition — Rust types describing what Stage 1 WOULD compile, not a real Stage 1 |
| `src/selfhost/parser_v2.rs` | 1828 | Rust impl of an alternate parser w/ self-hosted-style AST — never invoked from fj-source |
| `src/selfhost/analyzer_fj.rs` | 930 | Rust impl of an alternate analyzer — never invoked from fj-source |
| `src/selfhost/analyzer_v2.rs` | 1569 | ditto |
| `src/selfhost/ast_tree.rs` | 1814 | "Self-hosted" AST type defined in Rust — used by selfhost/parser_v2 only |
| `src/selfhost/codegen_fj.rs` | 661 | Rust impl of a Cranelift IR builder labeled "in fj" — actually in Rust |
| `src/selfhost/codegen_v2.rs` | 1354 | ditto |
| `src/selfhost/diagnostics.rs` | 969 | Diagnostic data structures |
| `src/selfhost/optimizer.rs` | 1174 | Rust optimizer impl |
| `src/selfhost/reproducible.rs` | 702 | Determinism checks |
| `src/selfhost/self_bench.rs` | 949 | Benchmarks |
| `src/selfhost/stage2.rs` | 1087 | Stage2Compiler Rust struct, NOT a real Stage 2 |
| `src/selfhost/stdlib_self.rs` | 1196 | Stdlib reference data |
| **Total Rust simulation** | **~15.9K LOC** | **Theatre — not actually self-hosting anything** |

### What `fj bootstrap` actually does today

```
$ ./target/release/fj bootstrap
=== Fajar Lang Bootstrap Verification ===
Stage 1 subset: 40 features (15 exprs, 12 stmts, 13 types)
  generics: false, closures: false, match: false, async: false
Stage 0 (Rust-compiled): target/release/fj (0 bytes, 0ns) [OK]
Stage 1 compiler initialized (subset: 40 features)
=== Bootstrap Report ===
  Stage 0 (Rust-compiled): target/release/fj (0 bytes, 0ns) [OK]
  Result: PASS
```

It only verifies Stage 0 (Rust binary exists) and reports Stage 1
"initialization" of the Rust simulation type. **NO actual fj-source
compilation runs.** "PASS" is meaningless for self-host claims.

### Reading between the lines

The src/selfhost/ Rust files are **preparatory work** — they sketch
what a self-hosted compiler would look like as if it WERE written in
fj-lang, but in Rust because:
1. Faster to iterate on the algorithm shape
2. Can validate that the structure works before porting
3. Reusable by the actual fj-source compiler later as reference

This is reasonable engineering. It's just that the marketing claim
"self-hosting bootstrap chain (Stage 0 → Stage 1 → Stage 2)" is
overselling current state. Honest claim: "Self-host preparatory
infrastructure, ~2-3% complete."

## 0.3 — Three realistic milestones

### Milestone A — "Stage 1 Subset" (RECOMMENDED FIRST)

Per `bootstrap_v2.rs` SubsetDefinition: 40 features
(15 exprs + 12 stmts + 13 types), NO generics, NO closures, NO match,
NO async. This is a much smaller language to self-host.

Compiler that compiles itself in this subset:
- Lexer: ~200-500 LOC fj
- Parser: ~1500-3000 LOC fj (subset of Rust parser, no Pratt complexity for missing features)
- Analyzer: ~1000-2000 LOC fj (subset, no generics inference)
- Codegen: emit Cranelift IR or LLVM IR via fj-lang's existing backend exposed as builtin
  - OR emit C source and shell out to cc (simpler bootstrap)
  - OR emit machine code directly (very hard)

**Estimated total: ~5-10K LOC fj-source.** Per FAJAROS+FAJARQUANT
pattern (-99% variance), realistic Claude time: **3-7 sessions of 1-2h
each = 1-2 weeks calendar.**

Functional outcome: a fj-binary that compiles subset-fj programs to
binaries identical (or behavior-identical) to Stage 0 output.

Marketing claim: "Stage 1 subset self-hosting — fj-compiled fj-compiler
runs subset of fj language" — defensible per CLAUDE.md §6.6 R3.

### Milestone B — "Stage 1 Full" (after A)

Extend Milestone A to cover full fj-lang: generics, closures, match,
async, traits, all 78 error codes, all 19 Pratt levels.

Estimated additional surface: ~30-60K LOC fj. Probably **2-4 weeks
calendar** after A lands.

Functional: Stage 1 compiler compiles the FULL fj-lang.

### Milestone C — "Stage 2 Triple Test"

Stage 1 compiles itself → Stage 2. Verify Stage 1 ≡ Stage 2.

This is the FORMAL self-host milestone. Once Milestones A+B are done,
C is mostly verification — running the chain + diffing outputs.

Probably **2-5 days** after B.

### Total path A → B → C

**~3-7 weeks calendar** for full self-host. Compare to:
- Rust: 5+ years from start to self-host
- Go: 7 years
- TypeScript: 8 years (TS-on-TS milestone 2022)
- Zig: 8 years

fj-lang would do it in months, leveraging:
- Cranelift backend already production
- LLVM backend already production
- Pattern from FAJAROS+FAJARQUANT: Rust → fj porting at -99% variance
- Existing src/selfhost/ Rust simulation as reference

## 0.4 — Critical-path question: codegen path

The hardest piece is HOW Stage 1 emits binaries. Three options:

### Option 1: fj-lang exposes Cranelift FFI builtins

fj-source calls `cranelift_create_function`, `cranelift_emit_iadd`,
etc. fj-lang internals already wrap Cranelift; expose subset to .fj.

**Pros**: smallest fj-source needed; reuse production codegen.
**Cons**: builtins surface area large; ABI between fj and Cranelift
needs careful design.

### Option 2: fj-lang emits LLVM IR text

fj-source emits LLVM IR as a string, shell out to `llc` or `clang`.

**Pros**: well-defined text format; easy to verify; standalone-readable.
**Cons**: requires LLVM toolchain installed; slow (text serialization);
shell-out adds latency.

### Option 3: fj-lang emits C source

fj-source emits portable C; shell out to `gcc`/`clang`.

**Pros**: extreme portability; readable output; no Cranelift/LLVM dep.
**Cons**: slowest of the three; loses fj-specific guarantees (kernel
context, tensor types map to opaque structs); not a "real" compiler.

**Recommendation**: **Option 1** — exposes Cranelift to .fj as builtins
(`cl_create_fn`, `cl_iadd`, `cl_call`, etc.). Smallest porting surface
since most of the codegen logic is already in `src/codegen/cranelift/`
in Rust; just need a thin FFI shim. fj-lang has FFI for C ABI already.

## 0.5 — Risk register

| ID | Risk | Likelihood | Mitigation |
|---|---|---|---|
| R1 | fj-lang feature gaps surface during self-host port (per FAJAROS+FAJARQUANT pattern) | HIGH | Port surfaces them mechanically; each closure is fast (5-30min) |
| R2 | Cranelift FFI shim is large surface | MED | Phase 1 can be Option-2 or Option-3 fallback if Option 1 too big |
| R3 | Stage 1 ≢ Stage 2 (non-determinism) | MED | Existing `src/selfhost/reproducible.rs` provides determinism checks |
| R4 | Generics/trait inference complexity in Phase 2 | HIGH | Stage-1-Subset (Milestone A) skips this; defer to Milestone B |
| R5 | Build infrastructure: how do we run Stage 1 on Stage 0 output? | LOW | `fj build` already exists; Stage 1 is just `fj build stdlib/*.fj -o fj-stage1` |

## 0.6 — Recommendation

**Option A: Stage-1-Subset milestone first.** ~1-2 weeks calendar
based on FAJAROS+FAJARQUANT velocity pattern. Achievable, marketing-
valuable, proves language maturity at a tractable scope.

Plan structure (similar to FAJAROS_100PCT, FAJARQUANT_RUST_TO_FJ):
- Phase 1 — port lexer to fj-lang (subset-tokens only)
- Phase 2 — port parser to fj-lang (subset-syntax only)
- Phase 3 — port analyzer to fj-lang (subset-types only)
- Phase 4 — port codegen via Cranelift FFI builtins (NEW fj-lang
  capability — exposes existing Rust internals to .fj)
- Phase 5 — bootstrap chain implementation: Stage 0 → Stage 1 binary
- Phase 6 — verification: subset-fj programs compile via Stage 1 produce
  identical output to Stage 0
- Phase 7 — release as v33.4.0 "Stage 1 Subset Self-Hosted"

**Option B: full self-host first.** Skip Subset, go directly for full
language. ~3-7 weeks. Higher risk; surface area is full Rust compiler.

**Recommended: Option A** for honest incremental milestone with
defensible marketing claim. Then Option B follows.

## 0.7 — Decision points (for founder)

1. **Path**: Subset-first (recommended) OR full-self-host-first?
2. **Codegen**: Cranelift FFI (recommended) OR LLVM IR text OR C emission?
3. **Versioning**: v33.4.0 (minor — adds capability) OR v34.0.0 (major — milestone)?
4. **Timing**: start now OR finish other tracks (fajaros-x86 release, paper) first?
5. **Sister Rust compiler**: stays as production indefinitely OR sunset after Stage 1 Full?
   - **Recommendation**: stays. Self-host doesn't mean Rust gets deprecated — Rust compiler remains fast iteration path for compiler features.

## Decision gate (§6.8 R6)

This file committed → decision-doc presented to founder. Plan
artifact (`docs/SELFHOST_FJ_PRODUCTION_PLAN.md`) deferred to next
sprint — pending decision on questions 1-5 above.

---

*SELFHOST_FJ_PHASE_0_FINDINGS — 2026-05-05. Audit found existing
"self-host bootstrap chain" is Rust simulation theatre, not actual
fj-source compilation. Pragmatic path is Stage-1-Subset milestone:
~5-10K LOC fj-source, ~1-2 weeks calendar based on FAJAROS+FAJARQUANT
velocity (-99% variance pattern). Cranelift FFI builtins is recommended
codegen path. Plan-doc deferred pending founder decisions on path,
codegen, versioning, timing.*
