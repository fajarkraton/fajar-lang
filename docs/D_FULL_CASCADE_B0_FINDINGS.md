---
phase: D-FULL cascade B0 (scope-doc only, no implementation)
plan: docs/FJARR_LEAK_PLAN.md §2 row 18.D + docs/FJARR_LEAK_PHASE_2_18D1_2_OVERFIRE_FINDINGS.md §5 Option A
status: B0 SCOPE COMPLETE 2026-05-10 (audit only; no source-tree changes shipped)
prereq: v35.4.1 (parser_ast.fj str_byte_at cascade closed) + E3/E4/E5/E1.5 already shipped in v35.2.0
purpose: empirical re-measurement of D-FULL cascade scope at HEAD; the prior over-fire estimate (30-60 .clone() insertions, 4-8h) predates v35.4.1 and the E3+E1.5 ships, so the landscape may have changed
---

# D-FULL Cascade B0 — Scope-Doc

> Goal: **scope-doc only**. No implementation, no shipped commits.
> Determines whether the v35.2.0 D-LITE opt-in (`--strict-ownership`)
> can be flipped to default-on for Compass §4.4 "@safe sebagai default"
> satisfaction, and at what cost. Empirical probe + revert; current
> tree is back to 86/86 stage1_full GREEN.

## §0 — TL;DR

**Headline finding:** D-FULL **arrays-only** flip at v35.4.1 HEAD
fires SE024 at **3 unique stdlib sites** + breaks **4 of 5** "arrays
are Copy" lib tests. This is **~10× smaller** than the over-fire
doc's pre-v35.4.1 estimate of 30-60 cascade sites. **Realistic
effort: ~1-2h** (vs prior 4-8h estimate).

**But:** "D-FULL" has been ambiguous. Two interpretations:

| Interpretation | Empirical fires per test | Unique stdlib sites | Realistic effort |
|---|---|---|---|
| **Arrays-only** (matches FJARR_LEAK Phase 2 framing) | 10 SE024 | **3** | ~1-2h |
| **Full strict** (all non-Copy types affine) | 5 SE024 + 23 ME001 | **44** (across 7 vars) | ~6-12h |

**Recommendation:** if user wants Compass §4.4 default-on safety
specifically for the `_FjArr` leak class, **arrays-only D-FULL is now
small enough to ship as a single ~2h commit**. If user wants full
strict semantics (all non-Copy affine including `str`/struct), that's
still a multi-session project.

**Plus a known gap:** E2 method-receiver consume (`arr.method(x)`
doesn't mark `arr` moved) is still **NOT wired**. Adding E2 would
likely re-introduce some cascade. Whether this matters depends on
the goal — for `_FjArr` leak closure specifically, fn-arg + let
+ match consume tracking is sufficient.

## §1 — Current SE024 wire state at v35.4.1

Verified against `src/analyzer/type_check/check.rs` + `mod.rs` +
`borrow_lite.rs` at HEAD `32f83e3c`.

| Component | Location | State |
|---|---|---|
| `is_copy_type` (default mode) | `borrow_lite.rs:588-592` | All types Copy (delegates to `true`) |
| `is_copy_type_strict` | `borrow_lite.rs:603-650` | Primitives + `&T` Copy; rest Move |
| `TypeChecker::is_copy` dispatch | `mod.rs:1971-1977` | Switches by `self.strict_ownership` flag |
| `--strict-ownership` CLI plumbing | `main.rs:62/87/431/462/474/476` | Wired end-to-end |
| `check_ident` SE024/ME001 split | `check.rs:1208-1226` | `was_array` matches `Type::Array(_)` → SE024; else ME001 |
| `let y = x` consume | `check.rs:683-705` | `mark_moved` if `!is_copy` |
| fn-arg consume | `check.rs:1685-1697` | `mark_moved` if `!is_copy` (and not `&self`) |
| match-subject consume | `check.rs:2300-2309` | `mark_moved` if has destructure pattern |
| **method-receiver consume (E2)** | nowhere | **NOT WIRED** ⚠️ |
| E3 branch-merge w/ terminator | `check.rs:check_if` + `borrow_lite.rs:138-237` | ✅ Shipped v35.2.0 |
| E1.5 `MoveTracker::reset()` | `borrow_lite.rs` | ✅ Shipped v35.2.0 |
| E4 `.clone()` recognition | check.rs MethodCall path | ✅ Shipped v35.2.0 |
| E5 `_fj_arr_clone` preamble | runtime | ✅ Shipped v35.2.0 |

## §2 — Empirical probe results (run + reverted)

Probe method: temporarily edit `src/analyzer/borrow_lite.rs:588-592`
to flip `is_copy_type` default behavior, run `cargo test --release
--test selfhost_stage1_full`, capture SE024/ME001 emissions, revert.

### §2.1 — Probe A: Full strict (`is_copy_type` → `is_copy_type_strict`)

```diff
-pub fn is_copy_type(_ty: &Type) -> bool { true }
+pub fn is_copy_type(_ty: &Type) -> bool { is_copy_type_strict(_ty) }
```

Result on `cargo test --release --test selfhost_stage1_full
full_p57 -- --nocapture`:

| Metric | Value |
|---|---|
| stage1_full pass rate | **0/86** |
| SE024 emissions | 5 |
| ME001 emissions | 23 |
| Unique move sites (byte offsets) | **44** |
| Unique moved variables | 7: `arr_name`, `declared_type`, `method`, `op`, `r`, `raw_name`, `struct_name` (mostly `str`, one struct) |

**Interpretation:** flipping the entire ownership model to strict
in default mode breaks heavily because `str`/`struct`/etc. become
affine simultaneously. This is **not** what FJARR_LEAK Phase 2 is
about — it's about closing the `_FjArr` realloc-leak class. Full
strict is a separate (much larger) project.

### §2.2 — Probe B: Arrays-only (`!matches!(ty, Type::Array(_))`)

```diff
-pub fn is_copy_type(_ty: &Type) -> bool { true }
+pub fn is_copy_type(_ty: &Type) -> bool { !matches!(_ty, Type::Array(_)) }
```

Result on `cargo test --release --test selfhost_stage1_full
full_p57 -- --nocapture`:

| Metric | Value |
|---|---|
| stage1_full pass rate | **0/86** (same 3 stdlib sites fire on every test) |
| SE024 emissions | **10** (per test; chain runs same source) |
| ME001 emissions | 0 |
| Unique move sites (byte offsets) | **3** (`123231`, `126091`, `131320`) |
| Unique moved variables | **1**: `vars: [str]` |

**Cross-test consistency check:** ran `full_p1` + `full_p57` +
`full_p86` separately; all three failed at the **same 3 byte
offsets**. Confirms these are stdlib-resident sites in
`stdlib/codegen_driver.fj`, not test fixtures.

### §2.3 — Identified consume sites (codegen_driver.fj)

Source-grep against `stdlib/codegen_driver.fj` shows **44 raw
consume sites** of `vars` total (25 `parse_expr_emit(ast, vars, …)`
+ 19 `lookup_var_type_in_table(vars, …)`). Of these, only **3 fire
SE024** in arrays-only mode at v35.4.1 — meaning E3 branch-merge
with terminator awareness + E1.5 reset across scopes already handle
the other ~41 cases.

The 3 sites that DO fire all share the same anti-pattern: same
fn body uses `vars` ≥2 times in a non-branching, non-terminating
flow path:

| Site | Line in materialized full_p57.fj | Pattern |
|---|---|---|
| Site A (byte 123231) | 2928 | `let er = parse_expr_emit(ast, vars, p_after_name, cg)` after a prior consume in same fn |
| Site B (byte 126091) | 2966+2972+2979+2980 | 4 consecutive `lookup_var_type_in_table(vars, …)` calls in BEGIN_FIELD/INDEX path |
| Site C (byte 131320) | 3037 | `let r_end = parse_expr_emit(ast, vars, r_start.pos + 1, cg)` after `r_start = parse_expr_emit(…)` consume |

**Fix shape per site:** insert `.clone()` at the prior consume site.
Each insertion is ~5 chars. 3 insertions total. ~5 minutes mechanical
work.

## §3 — Pre-Phase-2 lib tests (5 documented; 4 actually break)

Located in `src/analyzer/type_check/mod.rs`. Each comment says
"arrays are now Copy (Rc-based runtime semantics)". Of the 5 only
**4 actually break** under D-FULL arrays-only flip:

| # | Line | Test name | Pattern | Fate under D-FULL arrays-only |
|---|---|---|---|---|
| 1 | 3391 | `move_type_use_after_move_detected` | `let b = a; len(a)` (a: [i64]) | **BREAKS** — expect SE024 |
| 2 | 3406 | `move_type_ok_when_not_used_after` | `let t = s; println(t)` (s: str, no reuse) | **STILL PASSES** — str not affected, no reuse |
| 3 | 3419 | `fn_call_moves_move_type_arg` | `consume(a); len(a)` (a: [i64]) | **BREAKS** — expect SE024 |
| 4 | 3578 | `match_enum_destructure_moves_subject` | match x with destructure; `len(x)` after (x: [i64]) | **BREAKS** — expect SE024 |
| 5 | 3712 | `move_while_immutably_borrowed_me003` | `let r = &a; consume(a); println(r)` (a: [i64]) | **BREAKS** — expect ME003 |

**Update strategy per test:** rewrite to assert the *new* contract.
For tests 1/3/4 → flip from `is_ok()` to assert SE024 in errors.
For test 5 → assert ME003. For test 2 → no change needed.

Total: **4 tests** to update. Effort: ~20-30min (mechanical).

## §4 — Cumulative effort estimate (D-FULL arrays-only at v35.4.1)

| Step | Effort | Notes |
|---|---|---|
| Flip `is_copy_type` to arrays-only | 5min | One-line edit + comment |
| Insert 3 `.clone()` calls in `stdlib/codegen_driver.fj` | 15-30min | Mechanical; verify each with stage1_full smoke |
| Update 4 lib tests in `mod.rs` | 20-30min | Flip is_ok → assert SE024/ME003 in errors |
| Re-run full gate suite | 30min | `cargo test --lib` + `cargo test --test selfhost_stage1_full` + `cargo test --release --test selfhost_phase17_self_compile` + `cargo test --test selfhost_se024_emissions` (the 11 SE024 tests should now fire by default) |
| Update SE024 emission tests (some are `#[ignore]`d) | 15min | Un-ignore the default-mode SE024 tests since flag no longer needed |
| Write closure findings + CHANGELOG entry | 30min | v35.4.2 or v35.5.0 ship |
| **Total** | **~1.75-2.5h** | (was 4-8h in over-fire doc; recovered ~3h via E3+E1.5 already shipped + parser_ast.fj migration) |

## §5 — What D-FULL arrays-only does NOT cover

For honest scope:

1. **`str` use-after-move tracking** — strings are Copy in default
   mode under arrays-only D-FULL. The 23 ME001 emissions in Probe A
   would not fire. If user wants `str` affinity (e.g., for ownership
   transfer in @kernel paths), that's a separate project.

2. **E2 method-receiver consume** — `arr.method(...)` does NOT mark
   `arr` moved. Means chain-grow `args = args.push(x)` works without
   `.clone()`, BUT also means receiver consumption escapes the affine
   tracking. For closing the `_FjArr` realloc-leak class, this is fine
   (the leak was about the realloc path, which is now arena-backed
   per Phase 1 and doesn't depend on receiver tracking). For full
   Compass §4.4 default-on safety, E2 is missing.

3. **Branch-merge edge cases beyond E3** — E3 ships with terminator
   awareness (return/break/continue). Other diverging expressions
   (e.g., `panic!()`, `loop {}` infinite loops, `?`-operator early
   return) may still over-fire in pathological cases. None observed
   in stdlib at v35.4.1 — possibly because stdlib code is conservative
   about these patterns.

4. **Compass §4.4 "ALL @safe types are affine"** — that's full strict,
   not arrays-only. Arrays-only is a partial step toward §4.4 that
   addresses the specific FJARR_LEAK Phase 2 motivation.

## §6 — Decision points for next session

**Single decision: which interpretation of "D-FULL" does user want?**

### Option A — D-FULL arrays-only (~2h, this audit shows it's now feasible)

Ship a v35.4.2 (or v35.5.0) commit that:
1. Flips `is_copy_type` to delegate to a new `is_copy_type_default`
   that returns `!matches!(ty, Type::Array(_))`. Keeps `--strict-
   ownership` flag operational for str/struct affinity in production
   builds.
2. Inserts 3 `.clone()` calls in `stdlib/codegen_driver.fj`.
3. Updates 4 lib tests in `mod.rs:3390+`.
4. Un-ignores the SE024 emission tests that previously required
   `--strict-ownership`.
5. CHANGELOG: "FJARR_LEAK Phase 2 D-FULL — arrays-only affine
   default-on. Closes Compass §4.4 partial; full strict still gated
   behind `--strict-ownership`."

**Risk:** low. The cascade is empirically tiny. Pre-push hook gates
phase17 + stage1_full, catching any regression in 1 commit cycle.

### Option B — D-FULL full strict (~6-12h, multi-session)

Flip `is_copy_type` to `is_copy_type_strict` directly. Cascade work
in stdlib for `str`/struct/etc. would be ~44+ insertions across
codegen_driver.fj + parser_ast.fj + others. Plus E2 method-receiver
consume would need to be wired (~2-3h) to make the affine semantics
self-consistent. Plus more lib tests would need contract updates.

**Risk:** medium-high. Each cascade insertion may surface new sites
once prior ones are fixed (per the over-fire doc's E7 observation).
Multi-session, multi-commit project.

### Option C — Defer indefinitely; v35.2.0 D-LITE remains the safety wire

Accept that production builds opt-in via `--strict-ownership` and
default builds remain Copy. Compass §4.4 satisfaction parked until
@kernel mode lands (post-v36.x). Phase 1 arena (88 bytes/array → 0)
remains the leak ceiling.

**Trade-off:** unchanged from over-fire doc §5 Option C. The user
already chose this implicitly when they accepted the v35.2.0 D-LITE
ship.

### Recommendation

**Option A is the cheap win.** This audit's most valuable finding is
that the cascade collapsed from 30-60 sites to **3** between when the
over-fire doc was written and v35.4.1 HEAD. The `parser_ast.fj`
migration in v35.4.1 + E3+E4+E5+E1.5 in v35.2.0 did the heavy lifting
incidentally.

If the user previously rejected Option A as "too expensive at 4-8h",
the empirical re-measurement (~2h) may flip that calculus.

## §7 — Self-check (per CLAUDE.md §6.8)

```
[x] Pre-flight audit (B0/C0/D0) exists for this work?            (R1: this doc IS the B0)
[x] Every action in §4 has a runnable verification command?      (R2: all gates listed by name)
[x] Prevention mechanism specified?                               (R3: pre-push hook gates phase17 + stage1_full)
[x] Agent-produced numbers cross-checked with Bash?               (R4: all numbers come from cargo test runs above)
[x] Effort variance tagged?                                       (R5: prior estimate vs new in §4)
[x] Decisions are committed files?                                (R6: this doc IS the decision artifact; user picks A/B/C)
[x] No public-artifact drift?                                     (R7: no shipped changes in this audit)
[x] Multi-repo state check?                                       (R8: only fajar-lang touched; clean)
```

## §8 — Probe verification commands

To reproduce the empirical numbers in §2:

```bash
# Probe A (full strict)
cd "/home/primecore/Documents/Fajar Lang"
sed -i 's|^pub fn is_copy_type(_ty: &Type) -> bool {$|pub fn is_copy_type(_ty: \&Type) -> bool { is_copy_type_strict(_ty) } /*|' src/analyzer/borrow_lite.rs
# (manual cleanup needed — easier to use Edit tool as in this audit)
cargo test --release --test selfhost_stage1_full full_p57 -- --nocapture --test-threads=1 2>&1 \
  | grep -oE "(SE024|ME001): use of moved variable '[^']+'" | sort | uniq -c

# Probe B (arrays-only)
# Edit borrow_lite.rs:588-592 to: !matches!(_ty, Type::Array(_))
cargo test --release --test selfhost_stage1_full full_p57 -- --nocapture --test-threads=1 2>&1 \
  | grep -oE "moved at byte [0-9]+" | sort -u

# Always revert before commit
git diff src/analyzer/borrow_lite.rs   # should be empty
cargo test --release --test selfhost_stage1_full -- --test-threads=1 2>&1 | tail -3
# expected: 86 passed; 0 failed
```

## §9 — Cumulative state at audit close

| Aggregate | At v35.4.1 HEAD (probe reverted) |
|---|---|
| `cargo test --lib` | 7,629+ PASS (verified during probe; reverted to clean) |
| `cargo test --release --test selfhost_stage1_full` | **86/86 PASS @ ~13.4s** |
| `cargo test --release --test selfhost_phase17_self_compile` | 4/4 PASS (Stage 2 byte-equality preserved through audit) |
| Working tree | clean (`git diff src/analyzer/borrow_lite.rs` empty after revert) |
| Untracked | `docs/1/STRATEGIC_COMPASS.md` (pre-existing; ignore per resume protocol) |
| `--strict-ownership` flag | still operational (D-LITE shipped v35.2.0) |
| Probe ship-status | 0 commits; this is a scope-doc, not implementation |

---

*D-FULL Cascade B0 — written 2026-05-10. Empirical re-measurement
shows the cascade collapsed from prior 30-60 estimate to **3** unique
stdlib sites (arrays-only) at v35.4.1 HEAD, driven by E3/E4/E5/E1.5
already shipped in v35.2.0 + parser_ast.fj migration in v35.4.1.
Realistic effort revised from 4-8h to **~2h** for arrays-only D-FULL.
Three decision options surfaced (A/B/C); user authorization required
before implementation. Probe fully reverted; gates GREEN; tree clean.*
