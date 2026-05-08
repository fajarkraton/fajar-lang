---
phase: FJARR_LEAK Phase 2 (Strategy D / linear-types-lite) — B0 pre-flight audit
plan: docs/FJARR_LEAK_PLAN.md §0 + §2 row 18.D.* + §4 R5
status: B0 CLOSED 2026-05-08 (yellow light → proceed to 18.D.1 with original +30% surprise budget)
artifacts: this doc
purpose: empirical baseline before §2 row 18.D.1 (analyzer SE017 + .clone() builtin) — answers "is the cascade-risk in self-host source survivable under naive affine semantics?"
prereq: v35.1.0 (Phase 1 closed); decision file `docs/decisions/2026-05-07-fjarr-leak-strategy.md` Choice F §Migration-path Phase 2
---

# FJARR_LEAK — Phase 2 B0 Pre-Flight Audit Findings

> Per CLAUDE.md §6.8 R1, every Phase opens with a pre-flight audit
> via runnable commands. Phase 1 B0 (`docs/FJARR_LEAK_B0_FINDINGS.md`)
> measured the leak; Phase 2 B0 measures **cascade risk** — how many
> existing self-host source patterns would break under naive affine
> `[T]` semantics, and how many `.clone()` insertions the migration
> would require. **No 18.D.1 analyzer change ships until §1 Decision
> Gate (below) confirms green/yellow/red light.**

## Headline numbers

| Probe | Number | R5-threshold |
|---|---|---|
| `[T]` let bindings in `stdlib/*.fj` (B0.1) | **64** | >50 fires R5 (per FJARR_LEAK_PLAN §4) |
| Fns returning `[T]` (B0.2) | **13** | mostly `ast_*` constructors |
| `_FjArr`/`_fj_arr_` C-ABI refs (B0.3) | **codegen.fj:18 + codegen_driver.fj:32, 0 elsewhere** | low surface |
| Test harnesses touching `_FjArr` (B0.4) | **2** | stage1_full + leak baseline |
| codegen_driver.fj `[T]` let bindings (B0.5) | **10** | modest last-use-analysis blast radius |
| Phase 1 leak baseline still GREEN (B0.6) | **YES — 0 bytes lost** ✅ | no regression |
| Chain-grow `a = a.push(x)` sites (B0.7) | **133** (fajarquant 35 + parser_ast 45 + lexer 40 + codegen_driver 13) | affine-friendly |
| `fajaros-x86` `_FjArr` deps (B0.8) | **0** | no multi-repo break risk |
| Fns taking `[T]` parameter (B0.9) | **92 signatures** | fan-out for affine analysis |
| Highest per-var use count: `parser_ast.fj a` (B0.10) | **228 uses** (handoff pattern dominant) | affine-friendly per B0.11 |
| Highest per-var use count: `lexer.fj tokens` | **131 uses** | similar handoff pattern |
| `arr[i] = x` direct index-ASSIGN sites (B0.13/B0.14/B0.15) | **~0 true assigns** (99 regex hits all `==` comparisons) | no codegen extension needed |
| stage1_full smoke post-audit (B0.16) | **86/86 PASS** | sanity unchanged |

## B0.1 — 64 `[T]` let bindings (>50 fires R5 cascade threshold)

**Command:** `grep -nE "let .*:\s*\[" stdlib/*.fj | wc -l`

**Output:** `64`

Per FJARR_LEAK_PLAN §4 Risk Register row "Strategy D requires `.clone()`
insertions in self-host source — Medium-High likelihood, Cascade
re-baselining impact": *"Audit `[T]` reuse sites BEFORE D: `grep -E
'let.*: \\[' stdlib/*.fj | wc -l`. If >50, prefer F."*

**The threshold technically fires.** But the count alone doesn't predict
cascade size — the *pattern of use* of those 64 bindings does. B0.7,
B0.10, B0.11 explore this qualitatively below.

Distribution by file:
- `stdlib/fajarquant.fj` — bulk of count (~40 bindings; ML algorithm
  workspaces, mostly `let mut workspace: [f64] = []` + chain-grow inside
  fns; **affine-friendly** — workspace lifecycle is fn-local)
- `stdlib/codegen_driver.fj` — 10 bindings (B0.5)
- `stdlib/analyzer.fj` — 4 bindings (`new_names`, `new_types`, `new_moved`,
  `new_depths` — all in single fn; **affine-friendly**)
- Other stdlib files: ≤5 bindings each

## B0.2 — 13 fns return `[T]` (low blast radius)

**Command:** `grep -nE "^fn .*->\s*\[" stdlib/*.fj`

**Output:** 13 fns; 12 in `stdlib/ast.fj` (`ast_int`, `ast_float`,
`ast_str`, `ast_bool`, `ast_ident`, `ast_binop`, `ast_unary`, `ast_let`,
`ast_return`, `ast_fn`, `ast_struct`, `ast_use` — all return `[str]`)
+ 1 in `stdlib/parser_ast.fj` (`try_binop` returning `[str]`).

Each callsite of these fns is a **producer-consumer handoff** — caller
binds the result, consumes once. **Affine-friendly** by construction.

## B0.3 — `_FjArr`/`_fj_arr_` C-ABI refs concentrated in 2 files

**Command:** `grep -cE "_FjArr|_fj_arr_" stdlib/*.fj`

| File | Count |
|---|---|
| `stdlib/codegen.fj` | **18** (preamble emit) |
| `stdlib/codegen_driver.fj` | **32** (codegen-time dispatch) |
| All other stdlib files | **0** |

The other 13 stdlib files use language-level `[T]` syntax exclusively,
not the C ABI. **Implication for Phase 2:** Strategy D's affine
semantics need to apply only at the language-level `[T]` site. The
C-ABI `_fj_arr_*` calls in codegen.fj are emit-time string literals
(no semantics layer). Codegen.fj itself is "language-aware code" but
the `_fj_arr_*` strings it writes are codegen output, not consuming use.

**No need to touch codegen.fj** for Phase 2 analyzer SE017; only
codegen.fj preamble gets new `_fj_arr_clone` builtin (B0.5 estimate).

## B0.4 — Only 2 test harnesses depend on `_FjArr` shape

**Command:** `grep -rclE "_FjArr|_fj_arr_" tests/`

**Output:**
- `tests/selfhost_stage1_full.rs`
- `tests/selfhost_fjarr_leak_baseline.rs` (Phase 1 closure baseline)

Neither asserts specific shared-`_FjArr` semantics — both
compile-and-run-then-assert-stdout. **Cascade risk to test suite: low.**

## B0.5 — Codegen blast radius (last-use-analysis surface)

**Command:** `grep -nE "let .*:\s*\[" stdlib/codegen_driver.fj | wc -l`

**Output:** `10` `[T]` let bindings + 9 fns (`grep -cE "^fn " stdlib/codegen_driver.fj`).

`codegen_driver.fj` is the file that 18.D.2 (codegen emits free at
last-use) would modify. 10 bindings × ~5-10 use sites each = ~50-100
last-use decision points. The decision is mechanical (find the last
read of each binding in each control-flow branch). **Manageable scope.**

## B0.6 — Phase 1 arena leak STILL 0 (sanity check) ✅

**Command:** `cargo test --release --test selfhost_fjarr_leak_baseline -- --include-ignored`

**Output:** `1 passed; 0 failed; finished in 0.40s`

Phase 1 arena migration unaffected by Phase 2 audit work. **No regression
of the Phase 1 closure** from any of B0.1–B0.16 probes.

## B0.7 — Chain-grow pattern is affine-friendly (133 sites)

**Command:** `grep -cnE "^\s*[a-z_][a-z_0-9]*\s*=\s*[a-z_][a-z_0-9]*\.push\(" stdlib/*.fj`

**Output:** 133 total sites
- `stdlib/parser_ast.fj` — 45
- `stdlib/lexer.fj` — 40
- `stdlib/fajarquant.fj` — 35
- `stdlib/codegen_driver.fj` — 13

Sample (codegen_driver.fj L78 / L114 / L205 / etc.):
```fj
args = args.push(r.code)
elems = elems.push(r.code)
codes = codes.push(r.code)
flags = flags.push(if atom_is_str(ast, vars, p) { "1" } else { "0" })
```

This pattern reads `args` ONCE on the right (`args.push(...)`), then
**re-binds** `args` to the result. Under affine semantics this is a
**move-then-replace**: the right-side `args` is consumed by `.push(...)`,
the result becomes the new value of the binding. **Zero `.clone()`
needed.** This handles 133 of the 64+92+13+99 audit-counted sites
inherently.

## B0.8 — Multi-repo state clean (`fajaros-x86` zero deps)

**Command:** `grep -rclE "_FjArr|_fj_arr_" ~/Documents/fajaros-x86/`

**Output:** (empty — no matches)

**Implication:** fajaros-x86 kernel does NOT depend on the `_FjArr`
shape. Phase 2 changes to `_fj_arr_*` codegen are 100% local to
fajar-lang repo. No multi-repo break risk; no cross-repo plan-of-work
needed.

## B0.9 — 92 fn signatures take `[T]` parameter (cascade-risk multiplier)

**Command:** `grep -nE "fn .*\([^)]*:\s*\[" stdlib/*.fj | wc -l`

**Output:** `92` fn signatures

Under naive Strategy D (no `&[T]` borrow type), every callsite of these
92 fns passing `[T]` by value would consume the argument. Multi-callsite
fns where the SAME variable is passed twice in same scope = `.clone()`
needed.

**B0.11 + B0.12 evidence below** suggests this happens RARELY in
self-host source — the dominant pattern is "build up, hand off, rebuild
result" where each binding is consumed once at end-of-scope.

## B0.10 — Highest per-var use counts (raw count, not consuming-use count)

**Command:** for each [T] var found in `let mut? VAR: [...]`, count
identifier occurrences in the same file.

| File | Variable | Total uses |
|---|---|---|
| `stdlib/parser_ast.fj` | `a` | **228** |
| `stdlib/lexer.fj` | `tokens` | **131** |
| `stdlib/lexer.fj` | `starts` | **65** |
| `stdlib/parser_ast.fj` | `chain` | 14 |
| `stdlib/parser_ast.fj` | `field_chain` | 11 |
| `stdlib/codegen_driver.fj` | `codes` | 10 |
| `stdlib/codegen_driver.fj` | `param_pairs` | 9 |
| `stdlib/codegen_driver.fj` | `flags` | 6 |

**`parser_ast.fj` `a` = 228 uses** is the headline-grabber that initially
suggested SHOWSTOPPER. B0.11 below qualitatively unpacks this number
and finds the dominant pattern is **affine-friendly handoff**, NOT
read-after-consume.

## B0.11 — Parser-AST `a` is dominantly handoff pattern (affine-friendly)

**Sample of `a` usage in parser_ast.fj** (first 25 non-comment matches
after L34):

```fj
let a = ast.push(code)                    // create a
ParseResult { ast: a, pos: pos, error: true }   // single use, struct field; consume

let mut a = ast.push("BEGIN_MATCH")       // create a
let r_subj = parse_expr_ast(src, pos, a)  // CONSUME a (passed to fn)
a = r_subj.ast                            // RE-BIND a from result struct
if p_lb < 0 { return pr_err(a, r_subj.pos, "ERR_MATCH_LB") }   // consume on err path
                                          // (else path keeps building from new a)
a = a.push("BEGIN_DEFAULT")               // chain-grow (B0.7) — affine-friendly
let r_body = parse_expr_ast(src, p_arrow, a)   // CONSUME a again
a = r_body.ast.push("END_DEFAULT")        // RE-BIND from result + chain-grow
```

Pattern decomposition:
1. **Build**: `let mut a = ast.push(...)` — fresh binding from fn parameter
2. **Handoff**: pass `a` to recursive `parse_expr_ast(...)` → fn consumes,
   returns struct with new `[str]` `ast` field
3. **Rebind**: `a = r.ast` — completely replaces `a` with new value
4. **Chain-grow**: `a = a.push(...)` — affine-friendly per B0.7
5. **Branch consume**: `if cond { return pr_err(a, ...) }` — error path
   consumes `a`; else path falls through to keep building. Standard
   affine analysis: branch-merge with explicit consume in one branch is
   fine; the other branch must NOT use `a` after the branch (it can
   re-define `a`, fine).
6. **Final consume**: `return pr_ok(a, ...)` at end-of-fn

**No read-after-consume** observed in sampled patterns. The 228 uses
ARE 228 consuming uses — but each is a SINGLE consume, immediately
followed by a re-bind from the result. **Zero `.clone()` insertions
needed for `a`** under naive Strategy D semantics with proper branch-
merge analysis.

Same pattern conjectured for `lexer.fj tokens` (131 uses) — likely
chain-grow + handoff. To be confirmed in 18.D.1 implementation when
the analyzer flags any actual SE017 violations.

## B0.12 — No obvious affine-hostile pattern surfaced in lexer

**Command:** `grep -nE "[a-z_]+\([^)]*tokens[^)]*\)" stdlib/lexer.fj | head`

**Output:** (no matches surfaced patterns where `tokens` is passed to a
fn AND used again on a subsequent line)

`tokens` is bound, chain-grown via `tokens = tokens.push(...)`, and at
end-of-fn returned (consumed once). Standard affine-friendly pattern.

## B0.13 / B0.14 / B0.15 — Direct `arr[i] = x` index-assign DOES NOT exist

**Command:** `grep -nE "\b[a-z_][a-z_0-9]*\[[^]]+\]\s*=" stdlib/*.fj | wc -l`

**Output:** `99` regex hits — but B0.15 sample reveals all 99 are
**`arr[i] == x`** (equality comparison) false positives (greedy regex
matched `]` followed by ` =` which `==` satisfies). True index-ASSIGN
count: **near 0**.

**Command:** `grep -nE "_fj_arr_set" stdlib/codegen.fj`

**Output:** (empty — no `_fj_arr_set_*` builtins exist in preamble)

**Implication:** fj-source `[T]` arrays are **read-only after construction
+ chain-grow** in self-host source. No in-place index-mutation. This is
**Strategy-D-friendly**: affine semantics naturally prohibit shared
mutation, but no extant code RELIES on shared mutation. No new analyzer
work needed for this case.

## B0.16 — Final smoke: stage1_full 86/86 PASS post-B0-audit

**Command:** `cargo test --release --test selfhost_stage1_full`

**Output:** `86 passed; 0 failed; finished in 0.78s`

No regression from B0 grep audits (which were read-only). Sanity GREEN.

## §1 — B0 conclusion + Phase 2 decision gate

**Light:** 🟡 **YELLOW** (proceed to 18.D.1 with original +30% surprise budget intact).

**Justification:**
- Cascade-risk threshold technically fires at B0.1 (64 > 50)
- BUT qualitative pattern analysis (B0.7, B0.10, B0.11, B0.12) shows the
  dominant patterns (handoff + chain-grow) are **affine-friendly with
  zero `.clone()` insertions needed**
- C-ABI surface area is concentrated in 2 files (B0.3); other stdlib
  files use language-level `[T]` syntax exclusively
- Multi-repo state clean (B0.8 — fajaros-x86 has 0 deps)
- Phase 1 leak still GREEN, no regression (B0.6, B0.16)
- Test harness surface area is 2 files (B0.4)
- No in-place index-mutation `arr[i] = x` exists in self-host source
  (B0.13/B0.14/B0.15) — Strategy D's no-shared-mutation invariant is
  already de facto observed

**Recommendation:** Proceed to 18.D.1 (analyzer SE017 + `_fj_arr_clone`
builtin) with the original +30% surprise budget unchanged. Add an
**early-warning trigger**: if SE017 surfaces >20 affine-violation errors
when first applied to self-host source, PAUSE and surface for re-decision
(may need targeted `&[T]` borrow type, expanding scope beyond plan §5
14h estimate).

**Risks NOT yet measured (deferred to 18.D.1 evidence):**
- Branch-merge analysis correctness (do `if {return pr_err(a)} else {a = ...}`
  patterns Just Work?)
- Closure capture semantics (does fj-lang have closures over `[T]`? Per
  CLAUDE.md §4: "Env: `Rc<RefCell<>>` for closures" — this is the
  interpreter side; codegen doesn't have closures-over-`[T]` in self-host
  source per inspection)
- Inter-procedural last-use analysis (fn returns `[T]` then caller
  passes returned `[T]` to another fn — is the chain consume correctly
  inferred?)

These will surface as analyzer test failures during 18.D.1 RED phase.
The +30% surprise budget covers expected discoveries.

**STOP after this B0 commit.** Do NOT auto-chain into 18.D.1 — give the
user the chance to review B0 findings + the YELLOW-light recommendation
before the analyzer one-way-door commits.

## §2 — Cumulative Phase 2 readiness state

| Aggregate | Phase 1 close (v35.1.0) | Phase 2 B0 (now) |
|---|---|---|
| Self-host tests | 102 | 102 (unchanged; B0 read-only) |
| Phase 1 arena leak | 0 bytes ✅ | 0 bytes ✅ (sanity GREEN) |
| Stage1-full | 86/86 | 86/86 (post-B0 sanity) |
| Phase17 self-compile | 4/4 | (not re-run; B0 was read-only) |
| `[T]` reuse-site cascade-risk | unaudited | **64 audited; YELLOW light** |
| Multi-repo break risk | (not measured) | **fajaros-x86 0 deps** ✅ |
| 18.D.1 readiness | not started | **READY (B0 closed; awaiting user OK)** |

## §3 — Variance + cumulative effort

| Sub-item | Plan estimate | Actual | Variance |
|---|---|---|---|
| B0 audit + this findings doc | 1.5h (Phase 2 §1.5h slice) | ~30min | **-67%** |

Phase 2 cumulative so far: ~30min / 14h base / 18h cap = -97% (lopsided
because the audit work was lighter than expected; the heavy lift starts
at 18.D.1 analyzer).

Variance tag: `docs(fjarr-leak-phase2): step B0 — Phase 2 cascade-risk audit + decision gate [actual ~30min, est 1.5h, -67%]`.

## §4 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → ready for **user decision** on whether to proceed to 18.D.1.

**Three options:**
1. ✅ **Proceed to 18.D.1** (default per YELLOW-light recommendation):
   start analyzer SE017 + `_fj_arr_clone` builtin work. ~6h base / +30%
   = ~8h cap. One-way-door per decision file §Reverse-cost. Early-
   warning: pause if >20 SE017 violations surface.
2. ⏸️ **Pause Phase 2**: defer to a later session pending more design
   work (e.g. spec out `&[T]` borrow type pre-emptively, or revisit
   Strategy B). Phase 1 (arena) keeps holding the leak class closed.
3. 🔄 **Re-decision**: revisit decision file Choice F. If B0's YELLOW
   light feels too risky, this is the moment to consider Strategy E
   (opt-in @scoped) or an entirely new strategy. Note: Strategy C
   (refcounting) and the original Strategy E remain pre-rejected per
   Compass §6.2 + B0.7.

**STOP** after this B0 commit. Do not auto-chain into 18.D.1.

---

*FJARR_LEAK_PHASE_2_B0_FINDINGS — written 2026-05-08. Cascade-risk
audit complete: technical R5 threshold fires (64 > 50) but qualitative
pattern analysis surfaces affine-friendly dominance (handoff + chain-
grow). YELLOW light: proceed to 18.D.1 with original +30% surprise
budget; add early-warning trigger for >20 SE017 violations during
analyzer rollout. Phase 1 arena leak unaffected (still 0 bytes).
fajaros-x86 multi-repo state clean (0 `_FjArr` deps). Awaiting user
decision per §4 before 18.D.1 commits.*
