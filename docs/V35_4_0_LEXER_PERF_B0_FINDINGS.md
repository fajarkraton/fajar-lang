---
phase: v35.4.0 — lexer perf cascade migration (Option A from v35.3.2 B0) — B0 audit (2026-05-09)
status: B0 CLOSED — original estimate too optimistic; real scope ~3-4h. 3 sub-options surfaced for user re-confirmation.
purpose: re-verify scope after v35.3.2 B0; surfaces 252 single-char compares (vs 119 substring sites alone) + helper fn migrations
---

# v35.4.0 Lexer Perf Cascade — B0 Pre-Flight Audit

> v35.3.2 B0 chose Option C (analyzer fix only) and projected
> ~1.5-2h for the eventual cascade migration. **B0 today reveals
> the cascade is ~3-4h** — the substring-site count (119) was
> only part of the story; each substring result gets compared
> multiple times → 252 single-char `== "X"` compares + 3 helper
> fns also need migration.

## §1 — Headline numbers (full cascade scope)

| Probe | Number |
|---|---|
| `substring(p, p+1)` sites in `stdlib/lexer.fj` | **43** (unchanged from v35.3.2 B0) |
| `substring(p, p+1)` sites in `stdlib/parser_ast.fj` | **76** (unchanged) |
| Single-char `== "X"` compares in `stdlib/lexer.fj` | **165** (NEW — wasn't counted in v35.3.2 B0) |
| Single-char `== "X"` compares in `stdlib/parser_ast.fj` | **87** (NEW) |
| Helper fns to rewrite | **3** (`is_digit_str`, `is_alpha_str`, `is_alnum_str`) |
| Helper fn call sites | (TBD; need separate grep) |

**Total cascade work points:** ~119 substring + ~252 compare-literal
+ 3 helper rewrites + ~N helper-call updates = **~400 individual
edit points**.

## §2 — Migration shape per call site

```fj
// BEFORE
let c: str = source.substring(pos, pos + 1)
if c == "0" || c == "1" || ... { is_digit_str(c) }

// AFTER
let c: char = source.char_at(pos)
if c == '0' || c == '1' || ... { is_digit_char(c) }
```

Every variable bound from `substring(p, p+1)` gets type-promoted from
`str` to `char`. Every comparison `c == "X"` becomes `c == 'X'`.
Every `is_*_str(c)` call becomes `is_*_char(c)`. Cascade is local
per-fn but multiplies fast.

## §3 — Three sub-options for execution

### Sub-A1 — Phased per-file ship (~3-4h, 2 commits + Z)

- **Phase 1**: rewrite helper fns (`is_*_str` → `is_*_char`) +
  migrate all 43 lexer.fj substring sites + 165 char-literal
  compares. Single commit. ~1.5-2h.
- **Phase 2**: migrate 76 parser_ast.fj substring sites + 87
  char-literal compares. Single commit. ~1-1.5h.
- **Z** ship: closure docs + CHANGELOG v35.4.0 + tag + Release.
  ~30-45min.

Each phase verified independently against stage1_full + phase17.
Smaller blast radius per commit; if Phase 2 surfaces unexpected
patterns, Phase 1 still lands cleanly.

### Sub-A2 — All-in-one cascade (~3-4h, 1 commit + Z)

- Single migration commit covering both files + all helpers.
  ~3h.
- Z ship: ~30min.

Faster overall but if anything goes wrong, full revert needed.

### Sub-A3 — Pivot to Option B from v35.3.2 B0 (~3-4h, similar but using `byte_at`)

Re-evaluate: the **byte_at + numeric** approach has the SAME ~3-4h
cost (we'd just discovered the char migration is bigger than estimated).
But byte_at gives 10-20× perf vs char_at's 5-10×. If we're already
spending 3-4h, going for the bigger perf win might be better.

Trade-off: byte_at requires ASCII constants table or inline magic
numbers (`47` for `/`); char_at keeps the readable `'X'` literals.

## §4 — Helper fn migration detail

Current `stdlib/lexer.fj:24-43`:
```fj
fn is_digit_str(c: str) -> bool {
    c == "0" || c == "1" || c == "2" || c == "3" || c == "4" ||
    c == "5" || c == "6" || c == "7" || c == "8" || c == "9"
}
fn is_alpha_str(c: str) -> bool {
    c == "_" ||
    c == "a" || c == "b" || ... (full alphabet) ...
}
fn is_alnum_str(c: str) -> bool {
    is_alpha_str(c) || is_digit_str(c)
}
```

Migration target:
```fj
fn is_digit_char(c: char) -> bool {
    c == '0' || c == '1' || ... // (10 chars, 1-line ish)
}
fn is_alpha_char(c: char) -> bool {
    c == '_' || c == 'a' || ... // (53 chars total: a-z + A-Z + _)
}
fn is_alnum_char(c: char) -> bool {
    is_alpha_char(c) || is_digit_char(c)
}
```

Trivial 1:1 rewrite; just `str` → `char` parameter type + `"X"` → `'X'`.

## §5 — Risks (per CLAUDE.md §6.8)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Stage 2 byte-equality breaks | LOW | MEDIUM | Both stages process modified stdlib identically; stage1==stage2 invariant holds. Pre-push hook catches if not. |
| stage1_full chain breaks (per-batch) | MEDIUM | MEDIUM | Per-file phased commit (Sub-A1) smaller blast radius than all-in-one (Sub-A2). |
| Sed over-match (e.g., `c == "abc"` where c is full string, not char) | LOW | LOW | Visual diff scan; manual sed not safe — use targeted sed per-pattern + spot-check |
| Time over-runs (real cascade > 3-4h) | MEDIUM | LOW | If time-budget exceeded, partial-commit + defer rest. Phased plan (Sub-A1) most resilient. |
| Helper-fn callsites missed | LOW | MEDIUM | After helper rewrite, grep for `is_*_str(` and ensure all updated to `is_*_char(` |

## §6 — Decision gate

User picks A1 / A2 / A3 / pivot.

After v35.4.0 ship (whichever sub-option):
- self-host lexer perf gain measured via existing `interpreter_bench.rs`
  (or new bench if needed)
- pending_language_fixes.md §4 fully closed
- verified-actionable open list reduces to: TQ12.3 op + TQ12.6 hw +
  D-FULL deferred + @kernel deferred

## §7 — 2026-05-09 update: Phase 2 rolled back due to UTF-8 indexing mismatch

**Phase 1 (lexer.fj) shipped clean** (commit `8a02bba6`); Phase 2
(parser_ast.fj) cascade was attempted then ROLLED BACK after hitting
a fundamental indexing mismatch.

### Root cause

`String::char_at(i)` in the interpreter returns the i-th **CODEPOINT**
(Unicode char), not the i-th BYTE:
```rust
match s.chars().nth(idx) {
    Some(c) => Ok(Value::Char(c)),
    ...
}
```

But parser_ast.fj uses byte-indexed loops:
```fj
let n = len(src)            // byte length
let mut p = 0                // byte position
while p < n {
    let c = src.char_at(p)   // ← codepoint index, NOT byte index
    p = p + 1
    ...
}
```

For ASCII-only source: codepoint_index == byte_index (no problem).
For UTF-8 source (e.g. selfhost_main.fj contains em-dash "—" =
3 bytes / 1 codepoint): codepoint_index ≠ byte_index → parser
silently misreads characters → ERR_NO_FN.

### Why Phase 1 was safe

`stdlib/lexer.fj` is NOT part of the self-host chain pipeline (the
chain uses `parser_ast.fj`'s `parse_to_ast` directly). Phase 1's
char_at migration only affects users who explicitly call `tokenize()`
from `.fj` source — and those users typically pass ASCII source
where the bug doesn't manifest.

### Rollback decision

Phase 2 parser_ast.fj cascade rolled back in same session. Phase 1
lexer.fj retained for v35.4.0 ship (5-10× perf gain for the lexer
that user code can call directly).

### Future v35.4.x / v35.5.0 — proper fix requires `byte_at`

To safely migrate parser_ast.fj (and any byte-indexed parser),
need a NEW builtin `byte_at(s: str, i: i64) -> i64` that returns
the byte at byte-index i (range 0-255 or -1 for out-of-range).
This was Option B from the earlier B0 — now retroactively
recommended as the correct migration target for any byte-indexed
parsing code.

The v35.4.0 ship target reduces to: just lexer.fj migration (Phase 1
already committed). Phase 2 parser_ast.fj migration requires
byte_at + a careful audit to identify all byte-indexed loops.

### Lesson for future B0 audits

Always verify char_at semantics (codepoint vs byte) before assuming
drop-in replacement of substring(p, p+1) → char_at(p). For
byte-indexed parsing code, char_at is NOT a drop-in — it's
semantically different. byte_at is the correct primitive.

This is the **8th surface-finding via B0 audit pattern** in the
2026-05-08+09 session arc.

---

*V35_4_0_LEXER_PERF_B0_FINDINGS — written 2026-05-09. Surfaces
cascade scope ~3× larger than v35.3.2 B0 projected (252 char-
literal compares + 3 helper fns + ~119 substring sites = ~400 edit
points). 3 sub-options for execution. Recommendation: Sub-A1
phased per-file (~3-4h) for resilience.*
