# V26 Phase A2 — Production `.unwrap()` Inventory

> **Date:** 2026-04-11
> **Phase:** V26 A2.1 (inventory)
> **Tool:** `scripts/audit_unwrap.py`
> **Source CSV:** `audit/unwrap_inventory.csv`

---

## Headline

| Source | Claim | Verified | Notes |
|---|---|---|---|
| V17 audit (2026-04-03) | 43 production unwraps | Methodology unclear | — |
| V26 audit agent (initial) | **4,062** production unwraps | **WRONG** | Counted entire files including `#[cfg(test)] mod tests {}` blocks |
| V26 manual (this session, naive) | **174** | **STILL WRONG** | Counted files where the entire file is test-only via `#[cfg(test)] mod foo;` in parent |
| V26 audit agent (this session, no comment filter) | **20** | **STILL WRONG** | Doc comments + string literals counted as code |
| **V26 final (this script, all filters)** | **3** | **CORRECT** | Real production unwraps requiring action |

**Real count: 3 production `.unwrap()` calls in 2 files.**

V26 plan target was ≤30. **We are already at 3, before fixing anything.**

---

## Methodology

`scripts/audit_unwrap.py` walks `src/` and applies three exclusions:

1. **File-level test gating** — skip files declared as `#[cfg(test)] mod foo;`
   in their parent mod file. (Catches `src/codegen/cranelift/tests.rs` which
   alone has 154 `.unwrap()` calls but is entirely test code.)

2. **Inline test modules** — skip code inside `#[cfg(test)] mod tests { ... }`
   blocks within otherwise-production files.

3. **False positive filters:**
   - Doc comments (`///`, `//!`) and regular comments (`//`)
   - `.unwrap()` appearing inside string literals (security/hardening modules
     define rules that *describe* the antipattern in a string)

Total false positives filtered:
- 154 (test-only file `cranelift/tests.rs`)
- 8 in doc comments (lexer, parser, analyzer)
- 9 in string literals / pattern definitions (security, hardening)

---

## The Three Real Production Unwraps

| # | File | Line | Function | Snippet |
|---|---|---|---|---|
| 1 | `compiler/incremental/rebuild_bench.rs` | 334 | `bench_parallel_speedup` | `let _levels_1 = topological_levels(&units).unwrap();` |
| 2 | `compiler/incremental/rebuild_bench.rs` | 338 | `bench_parallel_speedup` | `let levels_8 = topological_levels(&units).unwrap();` |
| 3 | `distributed/dist_bench.rs` | 415 | `is_linear_scaling` | `let last = self.points.last().unwrap();` |

### Categorization (V26 A2.2 preview)

| # | File | Category | Justification |
|---|---|---|---|
| 1, 2 | `rebuild_bench.rs:334,338` | **`infallible-by-construction`** | `topological_levels` on a hand-built valid `units: Vec<CompileUnit>` from `generate_project()` cannot fail. Should be `.expect("topological sort of valid project graph cannot fail")` for clarity. |
| 3 | `dist_bench.rs:415` | **`infallible-by-construction`** | `is_linear_scaling` is called only after `self.points.len() >= 2` check, so `last()` is `Some`. Should restructure or use `.expect("linearity check requires ≥2 points")`. |

All 3 are in `_bench` files — code paths that exist for benchmarking, not core
runtime. Even so, replacing them with `.expect("...")` (with rationale) or
restructuring to remove the unwrap is straightforward.

---

## Phase A2 Effort Re-Estimate

Original V26 plan estimate (assumed 174 production unwraps):
- A2.1 inventory: 1h
- A2.2 categorize: 2h
- A2.3 hot files (top 5): 4h
- A2.4 remaining: 8h
- A2.5 clippy lint: 1h
- **Total: 16 hours**

Revised estimate (3 production unwraps, all infallible-by-construction):
- A2.1 inventory: ✅ DONE
- A2.2 categorize: ✅ DONE (all 3 are `infallible-by-construction`)
- A2.3 fix all 3 with `.expect("...")`: 30 min
- A2.4 — N/A (no remaining)
- A2.5 add `clippy::unwrap_used` lint at crate root: 1h
- **Total: 1.5 hours**

V26 Phase A2 will likely close in a single session.

---

## Why The Headlines Were Wrong

**V26 audit agent (4,062):**
The agent ran `grep -r ".unwrap()" src/ | wc -l` without filtering test
modules. 96% of those hits were inside `#[cfg(test)] mod tests {}` blocks.

**V26 manual naive (174):**
My initial script split at `#[cfg(test)]\nmod tests {` (inline boundary)
but didn't recognize files declared as `#[cfg(test)] mod foo;` in their
parent. `cranelift/tests.rs` alone is 154 `.unwrap()`s, entirely test code.

**V26 agent without comment filter (20):**
Doc comments demonstrating `.unwrap()` usage in API examples (`/// let
tokens = tokenize(...).unwrap()`) were counted as production code. So
were string literal patterns in `hardening.rs`/`security.rs` that *define*
the rule to detect `.unwrap()` in user code.

**V26 final (3):**
Filter: file-level cfg(test) + inline cfg(test) + comment lines +
string literal positions.

---

## Lesson for Future Audits

Counting `.unwrap()` requires **all three** exclusions:
1. Test-only files (declared via parent's `#[cfg(test)] mod foo;`)
2. Inline `#[cfg(test)]` test modules
3. Comments and string literals

Skipping any one inflates the count by ~10x. The agent's 4,062 was 1,353x
the truth.

This finding will inform A4 (doc truth update) — CLAUDE.md should not
say "0 .unwrap() in production" when the rule was already nearly satisfied
(3/several thousand lines). The right metric is "0 *unjustified* unwraps"
verified by `clippy::unwrap_used` with explicit `#[allow]` for the
infallible-by-construction cases.

---

*V26 A2.1 inventory complete — 2026-04-11*
*Tool: scripts/audit_unwrap.py | Output: audit/unwrap_inventory.csv*
*Next: A2.2 (categorize — already done above) → A2.3 (fix) → A2.5 (lint)*
