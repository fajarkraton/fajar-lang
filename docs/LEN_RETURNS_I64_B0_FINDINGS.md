---
phase: Language fix #2 — `len()` returns i64 / cleanup `to_int(len(...))` workaround — B0 audit (2026-05-08)
status: B0 CLOSED — `len()` already returns i64 correctly; ~30-45min mechanical cleanup of 109 workaround sites remaining
purpose: empirical verification of pending memory `pending_language_fixes.md` claim "len() returns usize, not i64"; reality at 2026-05-08 is that len() already returns i64 (memory ~6 weeks stale; the actual remaining work is clean-up of 109 redundant `to_int(len(...))` wrappers in stdlib)
---

# Language Fix #2 — `len()` returns i64 — B0 Pre-Flight Audit Findings

> The pending memory `pending_language_fixes.md` (created earlier
> 2026 era) claimed `len()` returns usize and recommended fixing it.
> **Reality at 2026-05-08:** `len()` already returns `Value::Int(_ as
> i64)` at all 9 call sites in interpreter. The workaround
> `to_int(len(...))` in stdlib is now a no-op wrapper — semantically
> safe but redundant. Remaining work: mechanical cleanup of 109 sites.

## §1 — Headline numbers

| Probe | Number | Significance |
|---|---|---|
| `len()` impl: returns `Value::Int(_ as i64)` | ✅ at 9 call sites in `src/interpreter/eval/builtins.rs` (L62-65) + `src/interpreter/eval/methods.rs` (L209, L465, L469, L779, L904, L1879) | NOT usize — already i64 |
| `len()` analyzer signature | ✅ registered in `src/analyzer/type_check/register.rs:28` | Wired |
| Smoke: `while i < len(v)` works in `.fj` | ✅ verified in `/tmp/len_smoke.fj` | No to_int needed |
| Smoke: `to_int(len(v)) == len(v)` | ✅ both equal 3, marked "equal" | Round-trip safe |
| `to_int(len(...))` workaround sites in stdlib/*.fj | **109 total** | Mechanical cleanup target |
| Distribution by file | codegen_driver.fj 51 + parser_ast.fj 30 + analyzer.fj 6 + codegen.fj 6 + parser.fj 5 + lexer.fj 4 + ast.fj 4 + compiler.fj 3 | codegen_driver dominates |

## §2 — The pending-memory error

`pending_language_fixes.md` §2 says:

> ### 2. len() returns usize, not i64
> `while i < len(arr)` fails — need `while i < to_int(len(arr))`
> Workaround: explicit `to_int()` conversion
> Impact: Every loop over arrays needs extra conversion
> Priority: MEDIUM (annoying but has workaround)

**This is incorrect today.** Empirical evidence:

```fj
fn main() {
    let v: [i64] = [1, 2, 3]
    let n: i64 = len(v)        // works — len returns i64
    let mut i = 0
    while i < len(v) {          // works — no to_int needed
        println(to_string(v[i]))
        i = i + 1
    }
}
```

`cargo run -- run /tmp/len_smoke.fj` exits cleanly with output `3 1 2 3`.

The workaround was needed in an older era (likely v32 or earlier);
sometime between that era and 2026-05-08, `len()` was migrated to
return `Value::Int(_ as i64)` at all interpreter sites. The pending
memory was never updated.

## §3 — Cleanup scope

109 sites in stdlib/*.fj currently use `to_int(len(...))` defensively.
Each such site is a no-op wrapper today:

```fj
// Before (redundant):
while i < to_int(len(items)) { ... }

// After (cleaner, semantically equivalent):
while i < len(items) { ... }
```

Distribution:
- `stdlib/codegen_driver.fj` — 51 sites (47% of total)
- `stdlib/parser_ast.fj` — 30 sites (28%)
- `stdlib/analyzer.fj` — 6 sites
- `stdlib/codegen.fj` — 6 sites
- `stdlib/parser.fj` — 5 sites
- `stdlib/lexer.fj` — 4 sites
- `stdlib/ast.fj` — 4 sites
- `stdlib/compiler.fj` — 3 sites
- Other stdlib files (nn, drivers, os, hal, fajarquant, core, selfhost_main): 0

## §4 — Cleanup plan (~30-45min)

| Step | What | Effort |
|---|---|---|
| **1** | Mechanical sed: `s|to_int(len(\([^)]*\)))|len(\1)|g` across all `stdlib/*.fj`. Note: regex must match `to_int(len(X))` where X has no nested parens. Sites with nested expressions inside `len(...)` would need manual review (estimated 0-5 such sites based on B0.5 sample). | ~10min |
| **2** | Visual scan of diff for false-positive matches (e.g. `to_int(len_other(...))` shouldn't match because the regex requires `len(` exactly) | ~5min |
| **3** | Run all gates: `cargo test --lib` (7,629), `cargo test --release --test selfhost_stage1_full` (86), `cargo test --release --test selfhost_phase17_self_compile` (4 — Stage 2 byte-equality MUST hold since both stages process modified source identically) | ~3-5min lib + ~110s phase17 |
| **4** | Update `pending_language_fixes.md` §2 from "len() returns usize" to "len() returns i64 ✅; cleanup landed in commit X (2026-05-08)" | ~3min |
| **5** | Commit single-step closure | ~3min |
| **Optional Z** | Small patch release v35.2.2 — same pattern as v35.2.1 (CHANGELOG entry + tag + GitHub Release). Worth it because the cleanup makes stdlib substantially cleaner (109 fewer wrapper calls). | ~10-15min |
| **Total** | | **~30-45min** + optional ~15min ship |

## §5 — Risks (per CLAUDE.md §6.8)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Sed regex over-matches and corrupts source | LOW | Medium | Visual diff scan in step 2; revert is `git checkout stdlib/` |
| Stage 2 byte-equality breaks | LOW | Medium | Both Stage 1 and Stage 2 process the SAME modified source → emit identical output. Different from prior md5, but stage1==stage2 invariant holds. Pre-push hook will catch if not. |
| stage1_full chain breaks (analyzer or codegen barfs on bare `len()` somewhere) | LOW | High | Smoke evidence (B0.6) shows bare `len()` works in `while i < len(v)` context. If a corner case breaks, sed is reversible. |
| FajarOS-x86 multi-repo breaks | NONE | n/a | No `to_int(len(...))` patterns are FajarOS-specific; cleanup is local to fajar-lang stdlib. |
| Self-host source readability regresses | NONE | n/a | Removing redundant wrappers IMPROVES readability, by definition. |

## §6 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → ready for **single-step mechanical cleanup commit**
(~30-45min, optionally followed by ~15min v35.2.2 patch ship).

The cleanup is purely additive in spirit (removes noise without
changing semantics). Same risk profile as the v35.2.1 patch (which
also touched no codegen and preserved all self-host gates).

After this cleanup, `pending_language_fixes.md` §2 closes. §3 (stack
overflow on large self-host programs) and §4 (self-hosted lexer perf
24x slower than Rust) remain open. §1 was already CLOSED previously.

---

*LEN_RETURNS_I64_B0_FINDINGS — written 2026-05-08. Surfaces that
language fix #2 is much smaller than the pending memory suggested:
`len()` already returns i64 correctly across all 9 interpreter sites;
the only remaining work is mechanical sed cleanup of 109 redundant
`to_int(len(...))` wrappers in stdlib (predominantly codegen_driver.fj
+ parser_ast.fj). Estimated ~30-45min cleanup + optional ~15min
v35.2.2 patch ship.*
