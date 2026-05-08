---
phase: v35.3.2 — language fix #4 lexer perf — B0 audit (2026-05-09)
status: B0 CLOSED — char_at exists but analyzer-mistyped; true perf needs `byte_at` + cascade migration; 3 scope options surfaced
purpose: pre-flight verification of "lexer 24x slower than Rust" pending claim — surfaces analyzer-interpreter type-mismatch + larger-than-expected cascade scope (119 sites vs 41 in original memory)
---

# v35.3.2 Language Fix #4 — Lexer Perf — B0 Findings

> Pending memory said: "char_at builtin needed; substring(pos, pos+1)
> is slow." **B0 reveals** char_at IS implemented (interpreter returns
> Value::Char correctly) but the analyzer mistypes it as Str. AND the
> cascade scope is 119 sites, not 41. True perf gain needs a different
> primitive (`byte_at` returning i64) + cascade migration.

## §1 — Headline numbers

| Probe | Number |
|---|---|
| `char_at` impl in interpreter | ✅ exists (`src/interpreter/eval/methods.rs:420`) |
| `char_at` returns | `Value::Char` (correct, no alloc) |
| `char_at` analyzer type | ❌ **`Type::Str`** (`src/analyzer/type_check/check.rs:2773`) — INCORRECT |
| `substring(pos, pos+1)` sites in `stdlib/lexer.fj` | **43** (memory said 41) |
| `substring(pos, pos+1)` sites in `stdlib/parser_ast.fj` | **76** (NEW — wasn't in original count) |
| **Total cascade target** | **119 sites** (~3× the memory estimate) |
| Existing `byte_at` builtin | ❌ NOT FOUND (the actually-fastest primitive) |

## §2 — The analyzer-interpreter mismatch

`src/analyzer/type_check/check.rs:2773`:
```rust
(Type::Str, "substring" | "char_at") => Type::Str,
```

The analyzer thinks `s.char_at(i)` returns Str. The interpreter
returns Char. So:
- `let c = s.char_at(0)` works at runtime (gets a Char)
- `if c == 'h' { ... }` analyzer error: "expected str, found char"
  (analyzer thinks c is Str; the char literal is Char)

This means **char_at is currently unusable for char-literal
comparisons**, defeating its perf benefit (callers must wrap with
`to_string(...)` which loses the Char-vs-Str-allocation win).

## §3 — Three perf-tier options

### Option A — Fix analyzer typing, migrate stdlib to char literals (~1.5-2h)

- Fix analyzer: change `char_at` return type to `Type::Char`
- Migrate 119 sites in stdlib: `s.substring(p, p+1) == "X"` → `s.char_at(p) == 'X'`
- Update helpers: `is_digit_str(s: str)` → `is_digit_char(c: char)`
- **Stage 2 byte-equality risk**: stdlib changes propagate through
  chain → both stages emit identical new C → invariant holds
- Perf gain: char_at avoids String alloc → ~5-10× speedup (Char is just u32)

### Option B — Add `byte_at` + migrate stdlib to numeric compare (~2-3h)

- Add `byte_at(s: str, i: i64) -> i64` builtin (returns byte 0-255)
- Migrate 119 sites: `s.substring(p, p+1) == "X"` → `byte_at(s, p) == 47` (47 = '/')
- Update helpers to use byte ops: `is_digit_byte(b: i64) -> bool` etc
- **Stage 2 byte-equality**: same as A — stdlib change → both stages emit identical new C
- Perf gain: ~10-20× speedup (no allocation, integer compare; no Char wrapping)
- Trade-off: less readable (47 vs '/'); needs ASCII constants throughout lexer

### Option C — Fix analyzer typing only, no stdlib migration (~30min)

- Just fix the analyzer mistype
- char_at usable in user code with char literals
- Stdlib unchanged → self-host lexer perf unchanged
- Defers cascade migration to future ship
- Smallest commit; ships v35.3.2 patch as "char_at type fix"

### Option D — Pivot — defer perf work, surface findings only (~5min)

- Document the analyzer mistype + cascade scope in B0 findings
- Don't ship code change
- Per `feedback_perfection_over_time.md`: this is genuinely LOW
  priority per pending memory ("proof-of-concept achieved,
  optimization is future work")
- Still updates `pending_language_fixes.md` §4 with accurate scope
  estimate for whoever picks it up later

## §4 — Recommendation

**Option C (fix analyzer typing only, ~30min, ship v35.3.2 patch).**

Reasoning:
- Smallest concrete fix; closes the analyzer bug
- char_at becomes correctly usable from `.fj` source (with char
  literal compare)
- Doesn't commit to the larger cascade migration
- Future v35.3.3 / v35.4.0 can land Option A or B with full design pass
  on byte vs char + helper-fn redesign + ASCII constant table
- Perf claim "24× slower than Rust" is the worst-case baseline;
  fixing the analyzer alone unlocks the perf win for new code
  even if stdlib doesn't migrate yet

**Tradeoff:** Self-host lexer perf doesn't improve in v35.3.2.
Stdlib migration is honest deferred work.

## §5 — Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Existing callers depending on `Char-as-Str` confusion break | LOW | LOW | Stdlib doesn't use char_at currently (verified — grep returns 0 usage); no extant code affected |
| Stage 2 byte-equality breaks (Option C) | NONE | n/a | Pure analyzer-only change; no stdlib/codegen touched |
| Stage 2 byte-equality breaks (Options A/B) | LOW | LOW | Both stages process modified stdlib → emit identical new output; stage1==stage2 invariant holds |
| Cascade-migration over-fire (Options A/B) | MEDIUM | MEDIUM | Per CQ1.4/v35.3.0 cascade history pattern; manageable with phased commits |

## §6 — Decision gate

User picks A / B / C / D.

After v35.3.2 ship (whichever option), `pending_language_fixes.md` §4
either CLOSED (Options A/B) or partially-addressed (Option C — the
analyzer fix shipped, cascade migration still future work).

---

*V35_3_2_LEXER_PERF_B0_FINDINGS — written 2026-05-09. Surfaces
char_at analyzer-interpreter mismatch + 119-site cascade scope
(3× memory estimate). Recommendation: Option C minimal analyzer-
fix as v35.3.2 patch (~30min). Options A (char migration ~1.5-2h)
and B (byte migration ~2-3h) deferred for future ships with full
design pass.*
