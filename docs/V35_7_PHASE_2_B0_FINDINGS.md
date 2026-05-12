# V35.7 Phase 2 — B0 Findings (annotate stdlib chain modules)

> **Phase:** v35.7.x Option B Phase 2 — annotate stdlib chain modules.
> **Audit date:** 2026-05-12 (EOS-30, post-D1.A ship).
> **Plan Hygiene §6.8 R1:** This B0 sub-phase precedes any code work.

## §1. Scope

D1.A (v35.7 Phase 1, commit `6f3cca65`) added skip-only `@`-annotation
support to `stdlib/parser_ast.fj`. Phase 2 was deferred from the same
session under the "first step only" rule.

The protocol's Phase 2 description:

> Phase 2 (annotate stdlib chain modules with `@safe`, `@unsafe`, etc.)
> NOT bundled per "first step only" rule. Opens as separate v35.7.1/v35.8 work.

This B0 audits the actual value of Phase 2 by:
1. Inventorying every `pub fn` in stdlib chain modules.
2. Categorizing each as a candidate for `@safe` / `@unsafe` / other.
3. Honestly assessing whether the annotation pass is worth shipping.

## §2. Verified inventory (HEAD `6f3cca65`)

### 2.1 Public function counts per chain module

| Module | pub fn count | Role |
|---|---|---|
| `stdlib/lexer.fj` | 7 | Tokenization (byte iteration over source) |
| `stdlib/parser_ast.fj` | 10 | AST construction (token sequence emission) |
| `stdlib/codegen.fj` | 29 | C-string emission helpers + state mgmt |
| `stdlib/codegen_driver.fj` | 17 | AST → C orchestration |
| `stdlib/analyzer.fj` | 9 | Type checking, dup-fn detection |
| `stdlib/ast.fj` | 5 | AST shape helpers |
| **Total** | **77** | |

### 2.2 Hardware / raw-pointer / asm operations in chain modules

Grep for `volatile_*`, `port_*`, `irq_*`, `page_*`, `mem_alloc`,
`mem_read*`, `mem_write*`, `cpuid_*`, `asm `, `x86_*`:

```bash
$ for f in stdlib/{lexer,parser_ast,codegen,codegen_driver,analyzer,ast}.fj; do
    grep -nE "(volatile_|port_|irq_|page_|mem_alloc|mem_read|mem_write|cpuid_|asm |x86_)" "$f"
done
# (no output)
```

**ZERO matches.** None of the chain modules touch hardware, raw memory,
inline asm, or IRQ. They are all pure data manipulation (byte iteration,
string concat, array growth, integer arithmetic).

### 2.3 Carved-out @safe-permitted builtins used in chain modules

The chain modules DO use builtins that were carved out of `safe_blocked_builtins`
in v35.6.0 (commit `9d5528c3`):
- `str_byte_at`, `str_len` (carved in v35.6.0 A.4 — pure-functional byte access)
- `tensor_workload_hint` (carved in v35.6.0 audit batch 2)
- `cap_new`, `cap_unwrap`, `cap_is_valid` (carved in v35.6.0 audit batch 2)

These are all `@safe`-permitted in the v35.6.0 analyzer, so they don't
require `@unsafe` or `@kernel` annotations on the callers.

### 2.4 Candidate categorization

| Category | Count | Examples |
|---|---|---|
| `@safe` (current default) | **77** | All chain pub fns |
| `@unsafe` (raw byte/pointer access requiring explicit acknowledgement) | **0** | none surface |
| `@kernel` (hw-touching) | **0** | none |
| `@device` (tensor-touching) | **0** | none |
| Other annotation (`@inline`, `@cold`, `@pure`, etc.) | **possibly some** | `op_prec`, `is_digit_byte`, `is_alpha_byte` could plausibly be `@pure` or `@inline`, but the chain modules currently don't request these and the Rust analyzer doesn't enforce them |

## §3. Marginal-value assessment (honesty-upfront)

Phase 2 as originally scoped means **adding explicit `@safe` to 77 pub fns**.
This is unambiguously low-value:

| Claim | Reality |
|---|---|
| "Adds safety enforcement" | NO. `@safe` is already the default since v35.6.0. Annotating doesn't change analyzer behavior. |
| "Documents intent" | WEAK. `@safe` being the default means absence-of-annotation already documents `@safe`. Adding the marker is noise, not signal. |
| "Enables future analysis" | NO. The fj-source `stdlib/analyzer.fj` doesn't do context-isolation; the Rust analyzer is sole authority and doesn't differentiate explicit-`@safe` from implicit. |
| "Smoke-tests D1.A on real stdlib" | WEAK. Phase17 byte-equality already exercises the chain on real stdlib (4/4 PASS post-D1.A). D1.A's correctness has been verified end-to-end. |
| "Dogfoods the language's own annotation feature" | WEAK. The dogfooding value is the annotation parser working (D1.A, already shipped) — not the annotation being present. |

If `@unsafe` candidates had surfaced (a fn doing raw-pointer arithmetic
in the chain that should warn callers), Phase 2 would have real value:
explicit `@unsafe` would tell callers "this is the dangerous bit." But
since the chain modules are uniformly safe, there's nothing to flag.

## §4. Stage 2 byte-equality risk if Phase 2 ships anyway

If we shipped Phase 2 despite low value, the risk model is identical to
D1.A's: parser_ast.fj's `skip_at_annotations` would consume the new
`@safe` tokens; emit_program would produce identical C. Phase17 4/4 risk:
LOW (the same risk that empirically passed for D1.A's parser_ast.fj edit).

So technical risk is not the blocker. **Value is the blocker.**

## §5. Recommendation: SKIP Phase 2

Mark Option B fully CLOSED at Phase 1. No code changes for Phase 2.

Rationale:
1. **77 cosmetic `@safe` insertions are noise.** The codebase reads
   cleaner without them.
2. **No re-entry condition has fired.** The D1.A Phase 1 decision doc
   listed 3 re-entry conditions for Phase 2:
   - fj-source analyzer.fj begins context-isolation: NO.
   - A stdlib-level lint needs annotation visibility: NO.
   - Phase 2 requires per-annotation codegen dispatch: NO.
   None of these are true today.
3. **Engineering capacity is finite.** The ~30-60min to ship Phase 2 +
   the cognitive overhead of reviewing 77 cosmetic changes is better
   spent elsewhere (e.g., the v36.x B-γ refactor, fajarquant Phase E,
   or genuine new feature work).

### Alternative path if user wants Phase 2 anyway

If the user still wants Phase 2 (e.g., for repository-aesthetic
consistency), the smallest defensible scope is:
- Annotate ONLY the chain entry points: `parse_to_ast`, `emit_program`,
  `analyze` (in analyzer.fj). 3 fns instead of 77.
- This shows the annotation feature "in use" at the top of the chain
  without touching internal helpers.
- ~10min. Phase17 risk identical to D1.A (already verified LOW).

## §6. Re-entry conditions (v36.x or beyond)

Reopen Phase 2 if any of these emerge:

1. **fj-source `stdlib/analyzer.fj` gains context-isolation checking.**
   Then explicit annotations become required input data, not just
   documentation.
2. **A new chain module touches raw memory or asm** (e.g., a future
   bare-metal optimizer pass). That module's pub fns would need
   explicit `@unsafe` or `@kernel`.
3. **A linter or doc-tool consumes annotations as machine-readable
   contracts** for stdlib API stability or capability tracking.
4. **B-Phase-1 (D1.A) needs a stress test under heavy real-world
   usage** that current phase17 doesn't cover.

## §7. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase           (Rule 1)
[x] Every task has runnable verification command          (Rule 2 — §2 + §4)
[x] Prevention mechanism added (hook/CI/rule)             (Rule 3 — this findings doc IS the prevention, by formalizing the SKIP rationale)
[x] Agent-produced numbers cross-checked with Bash        (Rule 4 — all counts verified live via grep on HEAD)
[ ] Effort variance tagged in commit message              (Rule 5 — at commit time)
[x] Decisions are committed files                         (Rule 6 — this file commits the SKIP decision)
[x] Public-artifact drift swept                            (Rule 7 — done in R4 earlier this session)
[x] Multi-repo state checked                               (Rule 8 — R1 done earlier this session)
```

## §8. Verification commands

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Verify pub fn counts
for f in stdlib/{lexer,parser_ast,codegen,codegen_driver,analyzer,ast}.fj; do
    echo "$f: $(grep -c '^pub fn ' "$f")"
done

# Verify no hardware ops in chain modules
grep -rnE "(volatile_|port_|irq_|page_|mem_alloc|mem_read|mem_write|cpuid_|asm |x86_)" \
    stdlib/{lexer,parser_ast,codegen,codegen_driver,analyzer,ast}.fj
# expect: empty output

# Verify D1.A still green (skip-only annotations work end-to-end)
cargo test --release --test selfhost_stage1_full full_p8
cargo test --release --test selfhost_stage1_full full_p9
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: D1.A regression tests + phase17 all PASS
```

## §9. Source artifacts (audit trail)

- This file: `docs/V35_7_PHASE_2_B0_FINDINGS.md`
- Decision (committed alongside this B0): SKIP Phase 2. No separate
  decision file needed — this findings doc is the decision.
- Phase 1 closure (D1.A): `docs/decisions/2026-05-12-parser-annotation-grammar-shape.md`
- Phase 1 B0: `docs/V35_7_PARSER_ANNOTATION_GRAMMAR_B0_FINDINGS.md`
- Resume protocol: `memory/project_resume_lanjut_protocol.md` §2.B

---

*B0 written 2026-05-12 EOS-30 session. ~15min actual. The audit completed
quickly because the inventory surfaced unambiguously: 77 pub fns, zero
hw/unsafe operations. Recommendation: SKIP Phase 2 entirely. Option B
fully CLOSED at Phase 1.*
