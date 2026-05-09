---
phase: v35.4.1 — byte_at + parser_ast.fj cascade Phase 2 — B0 audit (2026-05-09)
status: B0 CLOSED — 3 surface-findings; original "need NEW byte_at builtin" assumption WRONG
purpose: re-verify Phase 2 cascade target after v35.4.0 rollback; reveal str_byte_at already exists; surface chain-codegen wiring gap
---

# v35.4.1 byte_at + parser_ast.fj cascade — B0 Pre-Flight Audit

> v35.4.0 deferred Phase 2 parser_ast.fj cascade with the assumption
> that a NEW `byte_at(s, i: i64) -> i64` builtin was needed. **B0
> today reveals that builtin already exists as `str_byte_at` and works
> correctly for both ASCII and UTF-8 source.** The real gap is in
> the self-host chain codegen, not the language runtime.

## §1 — Surface-finding 1: `str_byte_at` already exists ✅

**Locations:**
- Analyzer: `src/analyzer/type_check/register.rs:422` —
  `("str_byte_at", vec![Type::Unknown, Type::I64], Type::I64)`
- Interpreter: `src/interpreter/eval/builtins.rs:2869-2879` —
  returns `Value::Int(s.as_bytes()[i] as i64)`
- LLVM codegen: `src/codegen/llvm/mod.rs:838, 2149, 7729` — has runtime
  function `fj_rt_bare_str_byte_at`

**Empirical test:**
```fj
let s: str = "abc/—xy"
len(s)                      // → 9 (em-dash = 3 bytes)
str_byte_at(s, 0)           // → 97 ('a')
str_byte_at(s, 3)           // → 47 ('/')
str_byte_at(s, 4)           // → 226 (UTF-8 first byte of —)
str_byte_at(s, 7)           // → 120 ('x')
```
All values correct.

## §2 — Surface-finding 2: `len(s)` returns BYTE length ✅

**Empirical:** `len("—")` returns 3 (the byte length), not 1 (codepoint
count). So all byte-indexed loops in `stdlib/parser_ast.fj` of the
form `while p < len(src)` are already correct — the loop bound and
the position are both byte-indexed. They just need a byte-indexed
ACCESSOR (which is `str_byte_at`).

This contradicts yesterday's hypothesis that "len() also needs to
become byte_len()" — len IS byte_len.

## §3 — Surface-finding 3: chain codegen does NOT know `str_byte_at` ⚠️

```bash
grep -n "str_byte_at" stdlib/codegen.fj   # → 0 results
```

The bootstrap chain (`fjc` Stage 1+2) emits C from a fixed set of
known builtins. `str_byte_at` is not in that set. So:
- ✅ A `.fj` program using `str_byte_at` compiled via production
  LLVM (`cargo run -- compile --features llvm,native`) works.
- ❌ `stdlib/parser_ast.fj` USING `str_byte_at` would NOT survive
  `phase17_stage2_native_triple_test` because `fjc` can't emit it.

This is the same wiring problem as historic builtins (`len`, `to_string`,
`substring` etc) when first added to the chain. The fix is mechanical
(add an emit branch in `stdlib/codegen.fj`), not architectural.

## §4 — Headline numbers (Phase 2 scope)

| Probe | Number |
|---|---|
| `substring(p, p+1)` sites in `stdlib/parser_ast.fj` | **76** |
| Single-char `== "X"` and `!= "X"` compares | **98** |
| Multi-char `== "abc"` compares to PRESERVE (str-not-byte) | TBD; need careful filter |
| Helper fns to add (e.g. `is_digit_byte`, `is_alpha_byte`) | **~3** in parser_ast.fj if any; may already exist as char-helpers from Phase 1 |

## §5 — Migration shape (corrected)

```fj
// BEFORE (current parser_ast.fj — works on ASCII via byte-luck)
let c: str = src.substring(p, p + 1)
if c == "/" { ... }
if c == "*" { ... }

// AFTER (byte-correct + 10-20× perf)
let b: i64 = str_byte_at(src, p)
if b == 47 { ... }  // '/'
if b == 42 { ... }  // '*'
```

**ASCII constants table** (could be inlined as magic numbers OR
defined as fj `const` table for readability — pick one):

```
'\n' = 10   ' ' = 32   '!' = 33   '"' = 34   '#' = 35
'$'  = 36   '%' = 37   '&' = 38   '\'' = 39  '(' = 40
')'  = 41   '*' = 42   '+' = 43   ',' = 44   '-' = 45
'.'  = 46   '/' = 47   '0'-'9' = 48-57
':'  = 58   ';' = 59   '<' = 60   '=' = 61   '>' = 62
'?'  = 63   '@' = 64   'A'-'Z' = 65-90
'['  = 91   '\\' = 92  ']' = 93   '^' = 94   '_' = 95
'`'  = 96   'a'-'z' = 97-122
'{'  = 123  '|' = 124  '}' = 125  '~' = 126
```

## §6 — Three sub-options for execution

### Sub-A — Wire str_byte_at into chain codegen + parser_ast cascade (~2-3h)

- **Phase A** (~30-60min): Add `str_byte_at` emit branch in
  `stdlib/codegen.fj`. Verify chain still compiles itself
  (`phase17_stage2` byte-equality preserved).
- **Phase B** (~1-1.5h): Migrate 76 substring + 98 compares in
  parser_ast.fj. Verify chain still works.
- **Z** ship: closure docs + CHANGELOG v35.4.1 + tag + Release.
  ~30min.

**Pros:** Closes pending_language_fixes.md §4 fully. Real perf gain
in the most-hit hot path. Closes the v35.4.0 deferral honestly.
**Cons:** chain codegen edits are higher-blast-radius than stdlib edits.

### Sub-B — Skip Phase 2; close §4 as "Phase 1 sufficient" (~15min)

- Update pending_language_fixes.md §4 to mark closed.
- No new code; no ship cycle.
- Phase 1 (v35.4.0) already gave 5-10× speedup for direct lexer
  callers; parser_ast.fj is in the chain (only invoked at
  bootstrap time, not at user-program runtime), so its perf
  matters less.

**Pros:** Zero blast radius; no chain codegen risk.
**Cons:** Leaves the deferral unresolved; pattern of "B0 surface
finds something already-done" continues.

### Sub-C — Phase A only; defer Phase B (~45-90min)

- Wire `str_byte_at` into chain codegen now (so it's available
  for any future use), but DON'T cascade parser_ast.fj.
- Document the new builtin in stdlib reference.
- Ship as v35.4.1 (small minor or patch).

**Pros:** Removes the architectural gap (chain compiler now full
parity with prod compiler on this builtin). Future Phase B becomes
mechanical. Low blast radius.
**Cons:** Doesn't deliver perf gain by itself; Phase B still pending.

## §7 — Risks (per CLAUDE.md §6.8)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Stage 2 byte-equality breaks (Sub-A or C) | MEDIUM | MEDIUM | Wire via canonical pattern (mirror existing `len` or `substring` emit branch); verify phase17_stage2 each commit |
| Sed over-match on multi-char compares (Sub-A only) | MEDIUM | MEDIUM | Use targeted sed with anchored single-char regex; spot-check diff manually |
| Chain codegen edit cascades to other builtins | LOW | HIGH | Single-builtin edit; mirror nearest existing pattern; revert ready |
| str_byte_at signature differs from prod LLVM expectation | LOW | MEDIUM | Verify signature `(str, i64) -> i64` matches LLVM `fj_rt_bare_str_byte_at` |
| Magic numbers harder to read than `'X'` chars | LOW | LOW | Use explicit comments at first compare site of each new char; or define const-table fn |

## §8 — Recommendation

**Sub-A (full close)** — ~2-3h to fully close pending_language_fixes.md
§4 with real perf gain in the hot path. Pattern from session arc
shows phased per-file ships are reliable; same pattern fits here
(Phase A codegen wiring + Phase B parser_ast migration).

If user prefers lower risk, **Sub-C** (Phase A only) closes the
architectural gap and leaves Phase B as a separate trivial follow-up.

If user prefers fastest close, **Sub-B** (declare done) is also
defensible — Phase 1 v35.4.0 already gave the 5-10× perf gain on
the path users actually call (`tokenize()` from .fj source);
parser_ast.fj is bootstrap-time only.

## §9 — Decision gate

User picks A / B / C.

After v35.4.1 ship (whichever sub-option):
- pending_language_fixes.md §4 fully closed (or marked deferred-honestly)
- Verified-actionable open list reduces accordingly

## §10 — Lesson for future B0 audits

**9th surface-finding via B0 audit pattern** in 2026-05-08+09 session arc:
"the missing primitive is already implemented; only the chain wiring
is missing". Pattern: when a builtin is needed for stdlib code, check
THREE places — analyzer, interpreter, AND chain-codegen (`stdlib/codegen.fj`).
Production LLVM coverage is necessary but not sufficient.

---

*V35_4_1_BYTE_AT_B0_FINDINGS — written 2026-05-09. Surfaces
str_byte_at already exists in 3 of 4 needed locations; only chain
codegen wiring missing. 3 sub-options A/B/C surfaced.
Recommendation: Sub-A for full close.*
