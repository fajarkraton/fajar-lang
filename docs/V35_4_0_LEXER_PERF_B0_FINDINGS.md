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

---

*V35_4_0_LEXER_PERF_B0_FINDINGS — written 2026-05-09. Surfaces
cascade scope ~3× larger than v35.3.2 B0 projected (252 char-
literal compares + 3 helper fns + ~119 substring sites = ~400 edit
points). 3 sub-options for execution. Recommendation: Sub-A1
phased per-file (~3-4h) for resilience.*
