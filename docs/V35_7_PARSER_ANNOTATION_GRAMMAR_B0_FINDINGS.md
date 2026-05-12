# V35.7 Self-Host Parser Annotation Grammar — B0 Findings (Option B pre-flight)

> **Phase:** v35.7.x Option B — self-host parser annotation grammar.
> **Audit date:** 2026-05-12 (EOS-30 session, post-B-δ).
> **Plan Hygiene §6.8 R1:** This B0 sub-phase precedes any code work.

## §1. Scope

The EOS-28 resume protocol (`memory/project_resume_lanjut_protocol.md` §2.B)
flagged the self-host parser as architectural blocker:

> Discovered as architectural blocker in A.4 (v35.6.0 session).
> `stdlib/parser_ast.fj` doesn't recognize `@unsafe`/`@kernel`/etc at top-level
> fn declarations. This is why we couldn't annotate stdlib chain modules
> (parser_ast/codegen/codegen_driver) — the self-host parser would fail to parse
> them.

This B0 hands-on verifies the gap and proposes a minimal-scope fix.

## §2. Verified facts (commit refs + line numbers, all on main @ HEAD `5ba17b66`)

### 2.1 Top-level dispatcher: `stdlib/parser_ast.fj:1273` `parse_to_ast`

```
pub fn parse_to_ast(src: str) -> [str]:
    while p < n:
        p = skip_ws(src, p)
        // 1. Optional `pub` modifier (Phase 17.0)
        let after_pub = expect_str(src, p, "pub")
        if after_pub > 0 { p = skip_ws(src, after_pub) }
        // 2. Dispatch on keyword: struct → parse_struct_ast
        //                         enum   → parse_enum_ast
        //                         const  → parse_const_ast
        //                         (else) → parse_fn_ast
```

**Gap:** between step 1 (`pub` skip) and step 2 (keyword dispatch), there is
no handling for `@<name>` tokens. So source `@safe fn foo() { ... }` flows into
`parse_fn_ast`, which at line 1086 calls `expect_str(src, p0, "fn")`,
fails (because `@safe` is at `p0`, not `fn`), and returns `ERR_NO_FN`.

### 2.2 Downstream consumers don't see annotations

The AST emitted by `parse_fn_ast` is the token sequence:
```
BEGIN_FN <name> BEGIN_PARAMS <pname1> <ptype1> ... END_PARAMS RET_TYPE <type> BEGIN_BODY ... END_BODY END_FN
```

No `ANNOTATION` token. Codegen consumers (`stdlib/codegen_driver.fj:1092, 1131-1135,
1362-1422`) iterate this AST and emit C functions; they have no annotation-aware
branches. So **whether the self-host parser preserves or discards `@`-info,
codegen output is identical**.

### 2.3 Rust parser's reference (`src/parser/mod.rs:747-917`)

The authoritative parser has 24+ annotation tokens (`AtKernel`, `AtDevice`, `AtSafe`,
`AtUnsafe`, `AtFfi`, `AtPanicHandler`, `AtEntry`, `AtNaked`, `AtInterrupt`, `AtMessage`,
`AtTest`, `AtShouldPanic`, `AtIgnore`, `AtDerive`, `AtPure`, `AtInline`, `AtCold`,
`AtNoInline`, `AtNoMangle`, `AtNoStd`, `AtReprC`, `AtReprPacked`, `AtSimd`, `AtSection`,
`AtNpu`, `AtGpu`, `AtApp`, `AtHost`).

Some take parameters: `@device("net")`, `@section(".data")`, `@derive(Debug, Clone)`,
`@simd(8)`. The parameter form is `(<value>)` where value is a paren-balanced
expression.

### 2.4 Confirmation: no stdlib *.fj file currently has top-level `@`-annotations

Grep `^@(kernel|device|safe|unsafe|ffi|naked|inline|interrupt|test|...)\b` against
`stdlib/*.fj`:

```bash
grep -nE "^@[a-z]" stdlib/*.fj
# (no output expected — verified)
```

This is consistent with v35.6.0 A.4 finding: stdlib chain modules can't be
annotated until the self-host parser learns to skip annotations.

## §3. Decision matrix

### D1.A — Skip-only (RECOMMENDED for Phase 1)

Add a single while-loop at `parse_to_ast` after `pub`-skip, before keyword
dispatch. Consume `@<word>` plus optional paren-balanced `(...)`. Don't add
anything to AST. Codegen sees identical AST as before.

**Effort:** ~30-45min. ~15 LOC in parser_ast.fj. Single commit.

**Pros:**
- Smallest scope.
- Codegen unchanged → Phase17 byte-equality risk minimal (only chain
  processing of the new parser_ast.fj lines).
- Unblocks Phase 2: annotate stdlib chain modules (separate later commit).

**Cons:**
- No semantic awareness — the self-host parser doesn't distinguish `@kernel`
  from `@safe`. Any future stdlib-level analysis (e.g., context-isolation
  checks running in the fj-source analyzer.fj) would still need extension.
  Currently the fj-source analyzer.fj doesn't do context checking either;
  the Rust analyzer is authoritative.

### D1.B — Parse-into-AST (preserve annotation through chain)

Add `ANNOTATION <name> <param-or-empty>` token to AST. Update codegen_driver
to recognize and pass through (or ignore). Optional: future analyzer.fj could
consume.

**Effort:** ~2-3h. ~30-50 LOC across parser_ast.fj + codegen_driver.fj + maybe
analyzer.fj.

**Pros:**
- AST-faithful — annotations preserved as data.
- Forward-compatible with stdlib-level context-isolation checks.

**Cons:**
- Larger scope. More Phase17 risk (codegen output changes if any consumer
  pattern-matches on AST length or position).
- Premature — fj-source analyzer.fj doesn't currently do context-isolation.

### D1.C — Skip-but-record (compromise)

Skip annotations like D1.A but store them in a side-channel (e.g.,
`fn_annotations: [str]` map keyed by fn name) inside the parser_ast.fj state
without putting them in the main AST. Available for future analyzer.fj passes.

**Effort:** ~1-2h. ~25-35 LOC.

**Pros:**
- Data preserved without changing main AST structure.

**Cons:**
- Adds state to a stateless dispatcher → ergonomic regression.
- Premature unless fj-source analyzer.fj is being written this session.

## §4. Stage 2 byte-equality risk assessment (CRITICAL)

Phase17 test (`tests/selfhost_phase17_self_compile.rs`) verifies internal
self-consistency:
- Stage1 (interpreter running chain on stdlib `.fj` files) → emit_C_1
- Stage2 (Stage1-native-binary running chain on stdlib `.fj` files) → emit_C_2
- Assertion: emit_C_1 byte-equal emit_C_2

The chain processes parser_ast.fj as one of its INPUTS. So if parser_ast.fj
grows, both Stage1 and Stage2 see the larger source. The risk is divergence:
does the new code execute identically under both the Rust interpreter (Stage1)
and the Stage1-native-binary (Stage2)?

**For D1.A specifically:** the new logic uses only constructs already exercised
in stdlib chain — `while`, `if`, `str_byte_at`, `read_word`, `skip_ws`, integer
compare, integer add. All have known-stable Stage1/Stage2 semantics. **Risk: LOW.**

**For D1.B:** introduces new AST token `ANNOTATION` that codegen_driver must
handle. The codegen path is longer → more chance for Stage1/Stage2 divergence.
**Risk: MEDIUM.** Must run phase17 byte-equality before push.

**For D1.C:** same as D1.A plus side-channel state. **Risk: LOW-MEDIUM** depending
on how the side-channel is wired.

**Mitigation common to all three:** pre-push hook already runs
`selfhost_phase17_self_compile` (verified during R1 push earlier this session).

## §5. Recommendation

**Adopt D1.A (skip-only) for v35.7.0 Phase 1.** Defer D1.B/D1.C to v36.x if
ever needed.

Rationale:
1. **Closes the architectural blocker.** Stdlib chain modules can now be
   annotated without breaking the self-host parser.
2. **Smallest scope** — single commit, ~15 LOC, ~30-45min.
3. **Codegen unchanged** — no AST shape change, no consumer update.
4. **Phase 2 unlock for free** — once parser accepts annotations, future
   commits can annotate stdlib chain modules with appropriate contexts.
5. **D1.B/D1.C are premature** because the fj-source analyzer.fj doesn't
   currently do context-isolation checking. Adding annotation-data to the AST
   serves no consumer today.

## §6. Stage-2 risk for the recommended path

D1.A risk: LOW.

The new code in parser_ast.fj is ~15 LOC of while-loop + 3 if-conditions +
known-stable builtins. No new chain features used.

Pre-push hook will catch any unexpected divergence by running
`selfhost_phase17_self_compile` (4-test suite).

## §7. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — see §8)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — Phase 1 ship adds regression test)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all locations verified via grep on HEAD)
[ ] Effort variance tagged in commit message               (Rule 5 — at commit time)
[ ] Decisions are committed files                          (Rule 6 — decision doc still TBD)
[x] Public-artifact drift swept                            (Rule 7 — done in R4 earlier this session)
[x] Multi-repo state checked                               (Rule 8 — R1 done earlier this session)
```

## §8. Verification commands for chosen path (D1.A)

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Before:
cat > /tmp/annotated.fj <<'EOF'
@safe fn foo() -> i64 { 42 }
fn main() -> i64 { foo() }
EOF
cargo run -- run /tmp/annotated.fj    # interpreter — should print 42 (works via Rust parser)

# Self-host chain parser test (currently fails per A.4 of v35.6.0):
cargo run --release -- selfhost-compile /tmp/annotated.fj 2>&1 | head -5
# expect (pre-D1.A): some ERR_NO_FN-flavored error from parser_ast.fj
# expect (post-D1.A): clean compile, identical to non-annotated equivalent

# Regression: phase17 byte-equality preserved
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# expect: 4 passed (Stage 2 == Stage 1 md5)

# Regression: stage1_full self-host suite still green
cargo test --release --test selfhost_stage1_full
# expect: 86 passed
```

## §9. Implementation sketch (D1.A, for the post-B0 commit)

In `stdlib/parser_ast.fj:1273-1315`, between line 1284 (`pub`-skip end) and
line 1287 (keyword dispatch start), insert:

```fj
// v35.7 D1.A: skip optional @-annotations on top-level decls.
// Each annotation is `@<word>` optionally followed by paren-balanced
// `(<args>)`. The self-host parser ignores annotation semantics — the
// Rust analyzer running before this chain is authoritative on context
// safety. This skip-only path unblocks Phase 2 annotation of stdlib
// chain modules without changing the emitted AST shape.
while p < n && str_byte_at(src, p) == 64 {  // '@'
    p = p + 1
    let word_end = read_word(src, p)
    if word_end == p { break }
    p = word_end
    p = skip_ws(src, p)
    if p < n && str_byte_at(src, p) == 40 {  // '('
        let mut depth = 1
        let mut q = p + 1
        while q < n && depth > 0 {
            let cc = str_byte_at(src, q)
            if cc == 40 { depth = depth + 1 }
            else if cc == 41 { depth = depth - 1 }
            q = q + 1
        }
        p = q
    }
    p = skip_ws(src, p)
}
```

Also re-run the same skip if it should be allowed BEFORE `pub` (the Rust
parser accepts both `@safe pub fn` and `pub @safe fn`). Decision needed —
audit which form is canonical in the codebase.

## §10. Open decisions for the user

| Decision | Default | Alternative |
|---|---|---|
| D1.A vs D1.B vs D1.C | **D1.A** (skip-only) | D1.B preserves annotations in AST; D1.C side-channel |
| Annotation position vs `pub` | Both (`@safe pub fn` AND `pub @safe fn` accepted) | Just one form, matching Rust parser convention |
| Phase 2 timing (annotate stdlib chain modules) | **Defer** to v35.8 / v36 after D1.A bake-in | Bundle Phase 2 with D1.A this session |
| Ship as v35.7.0 minor release | No tag (per session pattern; consistent with skip-tag for B-δ) | Tag if user wants release visibility |

## §11. Source artifacts (audit trail)

- This file: `docs/V35_7_PARSER_ANNOTATION_GRAMMAR_B0_FINDINGS.md`
- Decision file (to write after user picks): `docs/decisions/2026-05-12-parser-annotation-grammar-shape.md`
- Pred-context: `docs/KERNEL_MODE_PHASE_A_B0_FINDINGS.md` §A.4 detour explanation
- Resume protocol: `memory/project_resume_lanjut_protocol.md` §2.B
- B-δ companion (closed Option A): `docs/V35_6_LAYER_RECONCILIATION_B0_FINDINGS.md`

---

*B0 written 2026-05-12 EOS-30 session. ~30min actual. All locations verified live
via grep on HEAD `5ba17b66` (post-B-δ ship). Implementation deferred per
"first step only" rule until user confirms D1.A choice and Phase 2 timing.*
