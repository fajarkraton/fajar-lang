---
phase: Language fix #3 — stack overflow on large self-hosted programs — B0 audit (2026-05-08)
status: B0 CLOSED — fix #3 ALREADY CLOSED via SQ11.7 (16 MB thread stack at src/main.rs:409); no work needed; pending memory `pending_language_fixes.md` §3 was 43 days stale
purpose: empirical verification of the third stale-memory pending item in today's session — same pattern as TQ12.2 (B0 commit `8b53749e`, ~6 weeks stale) and `len()` returns i64 (B0 commit `91d76eed`, ~43 days stale)
---

# Language Fix #3 — Stack Overflow — B0 Pre-Flight Audit Findings

> The pending memory `pending_language_fixes.md` §3 (created earlier
> 2026 era) claimed: *"Combining all stdlib .fj files (3,076 lines)
> causes stack overflow on 500+ statement programs. Needed for SQ11.6
> (stage 2 bootstrap) and SQ11.7 (stack depth fix)."* **Reality at
> 2026-05-08: BOTH SQ11.6 and SQ11.7 are CLOSED.** No work needed.

## §1 — Headline numbers

| Probe | Number | Significance |
|---|---|---|
| `phase17_stage2_native_triple_test` (= SQ11.6 stage-2 bootstrap) | ✅ **PASS** 4/4 @ ~100s | Self-compile chain runs cleanly |
| Self-host stdlib LOC compiled by chain | **3,406 LOC** (codegen 682 + parser_ast 1295 + codegen_driver 1410 + selfhost_main 19) | Exceeds memory's "3,076 lines" threshold |
| Stack-size config in `src/main.rs:409` | **16 MB** (`std::thread::Builder::new().stack_size(16 * 1024 * 1024)`) | Comment explicitly cites "SQ11.7: Increase thread stack size to 16MB" |
| 700-statement smoke test | ✅ **PASS** (sum = 244,650 = 0+1+...+699 ✓) | Exceeds memory's "500+ statement programs" claim |

## §2 — Empirical proof

```bash
# SQ11.6 stage-2 bootstrap
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
# → 4/4 PASS @ ~100s. Compiles 3,406 LOC of stdlib through the chain.

# 700-statement program smoke
python3 -c "print('fn main() {'); print('    let mut sum: i64 = 0'); \
  [print(f'    sum = sum + {i}') for i in range(700)]; \
  print('    println(to_string(sum))'); print('}')" > /tmp/big_smoke.fj
cargo run -- run /tmp/big_smoke.fj
# → 244650  (Gauss formula: 699*700/2)
# Plugin warning about 704-line function is harmless; analyzer accepts.
```

## §3 — Existing fix (SQ11.7)

`src/main.rs:405-415`:
```rust
fn main() -> ExitCode {
    // SQ11.7: Increase thread stack size to 16MB for deeply recursive
    // programs (self-hosted compiler tokenizing large files).
    let stack_size = 16 * 1024 * 1024; // 16 MB
    let builder = std::thread::Builder::new().stack_size(stack_size);
    let handler = builder
        .spawn(main_inner)
        .expect("failed to spawn main thread with larger stack");
    // ...
}
```

This is exactly the "increase thread stack size" option named in the
pending memory. Fix #3 is closed-by-implementation; the pending
memory was never updated.

## §4 — Adjacent finding: language fix #4 IS genuinely still open

While we're here, quick-check on the next pending item:

| Probe | Result |
|---|---|
| `char_at` builtin in `src/interpreter/eval/builtins.rs` or `src/analyzer/type_check/register.rs` | **NOT FOUND** (empty grep) |
| `substring(pos, pos+1)` sites in `stdlib/lexer.fj` | **41 sites** still using the slow allocating pattern |

Language fix #4 ("self-hosted lexer 24x slower than Rust") IS
genuinely still open. The pending memory accurately describes the
state. This is the **only** language-fixes pending item that is
actually pending — §1, §2, §3 all surfaced as already-done in today's
session.

## §5 — Meta-pattern: stale pending memories

This is the **3rd stale-memory finding today** in a single session:

| # | Pending memory | Stale by | Reality |
|---|---|---|---|
| 1 | `pending_tq12_2_sqlite.md` (TQ12.2 SQLite) | ~6 weeks | 90% done; only analyzer name table missing 4 builtins (15-30min closure → shipped v35.2.1) |
| 2 | `pending_language_fixes.md` §2 (`len()` returns usize) | ~43 days | Already i64; only mechanical cleanup of 109 wrappers needed (~10min cleanup → shipped v35.2.2) |
| 3 | `pending_language_fixes.md` §3 (stack overflow) | ~43 days | Already CLOSED via SQ11.7 16MB stack; phase17 stage-2 bootstrap PASS |

**Pattern:** every "MEDIUM/HIGH priority" pending item from the older
2026 era I've audited today turned out to be either fully closed or
90% done. The work-tracking via `pending_*.md` memories has drifted
substantially from code reality.

**Implication for next-session protocol:** the `feedback_*.md` memory
"always B0-audit before assuming pending work is large" rule (added
implicitly via this session's findings) should be promoted to a
hard rule. Future "lanjutkan" should default to a quick B0 verification
of any pending-memory claim before allocating work-budget.

**Possible proactive next step:** sweep ALL `pending_*.md` files,
B0-audit each, mark CLOSED ones, refresh the genuinely-open ones.
Estimated 1-2h sweep would prevent more wasted estimation cycles.

## §6 — Closure plan (~5min, single-commit)

| Step | What | Effort |
|---|---|---|
| **1** | Update `pending_language_fixes.md` §3 from "MEDIUM priority — Needed for SQ11.6/SQ11.7" to "CLOSED 2026-05-08" with empirical evidence + source-of-truth pointer to this doc | ~3min |
| **2** | Commit this B0 doc + memory update | ~2min |
| **NO ship** | No code change → no v35.2.3 patch needed. Pure docs/memory hygiene. | — |
| **Total** | | **~5min** |

## §7 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → fix #3 closure is single-commit (B0 doc + memory update).
After this commit, `pending_language_fixes.md` has only §4 (lexer
perf, LOW priority) remaining as genuinely-pending.

Three reasonable next-steps for user decision:

1. **Tackle language fix #4 (genuinely open)** — ~2-4h effort, LOW
   priority per pending memory. Would add `char_at(s, i)` builtin +
   migrate 41 `substring(pos, pos+1)` sites in stdlib/lexer.fj.
   Improves self-host lexer perf (currently 24x slower than Rust);
   "proof-of-concept achieved" per memory note, so this is
   nice-to-have not load-bearing.
2. **Proactive memory audit sweep** — ~1-2h pass through ALL
   `pending_*.md` + `project_*.md` memories, B0-audit each
   "pending" or "in progress" claim, mark CLOSED ones. Prevents
   more wasted estimation cycles like today's 3 stale findings.
3. **Switch tracks** — crypto CQ1.3/CQ1.4, D-FULL cascade, @kernel
   mode, or template TQ12.4-12.6 (Q6A hardware needed). Or
   something else entirely.

---

*STACK_OVERFLOW_B0_FINDINGS — written 2026-05-08. Surfaces that
language fix #3 (stack overflow on large self-hosted programs) is
CLOSED via SQ11.7 16MB thread stack + phase17 stage-2 bootstrap
PASS @ 3,406 LOC. Pending memory was 43 days stale. Adjacent finding:
fix #4 (lexer perf) IS genuinely open; `char_at` builtin missing,
41 `substring(pos, pos+1)` sites in lexer.fj. Meta-pattern: 3rd
stale-memory finding today; proactive memory audit sweep proposed
as a possible next-step to prevent more wasted estimation cycles.*
