---
phase: D-FULL Option B Phase Plan (full strict — all non-Copy types affine in default mode)
plan: docs/D_FULL_CASCADE_B0_FINDINGS.md §6 Option B (user-authorized 2026-05-10)
status: B0 PHASE 1 COMPLETE 2026-05-10 (audit only; no source-tree changes shipped)
prereq: v35.4.1 + E3/E4/E5/E1.5 already shipped (v35.2.0)
purpose: deeper empirical audit + 5-phase plan for Option B; user authorization required between each phase
---

# D-FULL Option B — Full-Strict Cascade Phase Plan

> User authorized Option B 2026-05-10. This is the deeper B0/Phase-1
> audit (CLAUDE.md §6.8 R1: mandatory pre-flight per phase). Empirical
> probe + revert; tree clean at HEAD `32f83e3c`.

## §0 — TL;DR

**Empirical full-strict cascade at v35.4.1 HEAD:**

| Metric | Value |
|---|---|
| Unique stdlib move sites firing | **12** (consistent across full_p1/full_p57/full_p86) |
| Unique moved variables | **8**: `vars`, `raw_name`, `r`, `declared_type`, `op`, `method`, `arr_name`, `struct_name` |
| Distribution | 1 in `parser_ast.fj` + 11 in `codegen_driver.fj` |
| Pre-Phase-2 lib tests breaking | 4 of 5 (line 3391/3419/3578/3712 in `mod.rs`) |
| **E2 method-receiver consume** | **NOT WIRED** — design decision pending |

**Effort revision (vs scope-doc §6 Option B):**

| Sub-phase | Original estimate | Revised | Notes |
|---|---|---|---|
| Phase 2: Wire E2 + define `self`-kind | ~2-3h | **~3-4h** | Requires design decision on method-call ownership |
| Phase 3: Cascade `.clone()` insertions | ~6-9h | **~3-5h** | Only 12 base sites, fewer than feared |
| Phase 4: Update lib tests | ~30min | **~30min** | 4 tests |
| Phase 5: Closure + CHANGELOG | ~30min | **~45min** | E2 design choice needs documentation |
| **Total** | ~9-13h | **~7-10h** | Inside the original 6-12h window |

**Recommendation:** before committing to Phase 2 (E2 wiring), the
user should make an explicit design decision about method-call
ownership semantics for `[T]` and `str`. See §3.

## §1 — Empirical map of the 12 stdlib sites

Verified across 3 tests (full_p1, full_p57, full_p86) — all fail at
the same 12 byte offsets, confirming these are stdlib-resident
bootstrap-blocking sites.

| # | Variable | Type | Source file:line | Pattern | Suggested fix |
|---|---|---|---|---|---|
| 1 | `op` | str | `parser_ast.fj:737` | `let a = rhs.ast.push("BINOP").push(op)` then `op` consumed by `.push(op)`; later use re-fires | `.clone()` on `op` at `.push(op)` |
| 2 | `op` | str | `codegen_driver.fj:134` | `let op = ast[pos + 1]` then 2 fn-arg consumes | `.clone()` at first consume |
| 3 | `arr_name` | str | `codegen_driver.fj:176-184` | `let arr_name = …; let arr_type = lookup_var_type_in_table(vars, arr_name)` then later use | `.clone()` at line 184 |
| 4 | `method` | str | `codegen_driver.fj:197-212` | `let method = ast[p2 + 1]; … if method == "push"` after fn-arg consume | `.clone()` at first consume |
| 5 | `raw_name` | str | `codegen_driver.fj:373-376` | `let raw_name = ast[pos + 1]; let mut name = raw_name` then later use | `.clone()` at line 376 |
| 6 | `declared_type` | str | `codegen_driver.fj:688` | `if declared_type != "" { c_type = map_type_ctx(declared_type, cg) }` — used 2× in same expr | `.clone()` at fn-arg site |
| 7 | `r` | str | `codegen_driver.fj:695` | `if r != "" && is_struct_name(cg, r) { r }` — used 3× | `.clone()` at first consume + maybe extract local |
| 8 | `declared_type` | str | `codegen_driver.fj:924` | duplicate of #6 in different fn body | `.clone()` |
| 9 | `r` | str | `codegen_driver.fj:946` | duplicate of #7 in different fn body | `.clone()` + maybe extract local |
| 10 | `vars` | [str] | `codegen_driver.fj:~860-870` (byte 123231) | `let er = parse_expr_emit(ast, vars, p_after_name, cg)` after prior consume | `.clone()` at first consume |
| 11 | `vars` | [str] | `codegen_driver.fj:~895-915` (byte 126091) | 4 consecutive `lookup_var_type_in_table(vars, …)` in BEGIN_FIELD/INDEX path | `.clone()` chain or extract local snapshot |
| 12 | `vars` | [str] | `codegen_driver.fj:~970` (byte 131320) | `let r_end = parse_expr_emit(ast, vars, …)` after prior consume | `.clone()` |

**Note:** byte offsets reflect the materialized chain source (~145K
bytes). Source-line mapping verified by grepping the patterns
directly in stdlib files.

## §2 — Pre-Phase-2 lib tests (5 docs, 4 break)

Same as scope-doc §3. Located in `src/analyzer/type_check/mod.rs`:

| Line | Test name | Fate under full-strict flip |
|---|---|---|
| 3391 | `move_type_use_after_move_detected` | **BREAKS** — expects `is_ok()`, would get SE024 |
| 3406 | `move_type_ok_when_not_used_after` | **STILL PASSES** — `let t = s; println(t)` no reuse |
| 3419 | `fn_call_moves_move_type_arg` | **BREAKS** — expects `is_ok()`, would get SE024 |
| 3578 | `match_enum_destructure_moves_subject` | **BREAKS** — expects `is_ok()`, would get SE024 |
| 3712 | `move_while_immutably_borrowed_me003` | **BREAKS** — expects `is_ok()`, would get ME003 |

Update strategy: flip 4 tests from `assert!(check(src).is_ok())` to
asserting the diagnostic appears in errors.

## §3 — E2 method-receiver consume — DESIGN DECISION REQUIRED

E2 is currently NOT wired. With full-strict default mode, this is a
**semantic gap**: `arr.push(x)` doesn't consume `arr`, but
`push(arr, x)` (free fn) does. This inconsistency means full strict
isn't actually self-consistent without E2.

**Three E2 design choices:**

### E2.A — Method takes `self` (consume)
```fj
let new_arr = arr.push(x)   // arr is moved, must use new_arr
```
- Forces chain-grow `arr = arr.push(x)` pattern; breaks
  `arr.push(x); arr.push(y)` style.
- ~5-10 additional cascade sites to insert `.clone()` or chain.

### E2.B — Method takes `&mut self` (mutable borrow)
```fj
arr.push(x)   // mutates arr in place; returns &mut self for chaining
arr.push(y)   // OK: arr still owned
```
- Matches Rust idiom for `Vec::push`.
- Requires changing the codegen of method calls (returns `&mut [T]`
  vs new `[T]`).
- Compatible with current chain-grow assignment `args = args.push(x)`
  if `.push()` returns `self` after mutation.

### E2.C — Method takes `&self` (immutable borrow)
```fj
let new_arr = arr.push(x)   // arr unchanged; new_arr is a fresh array
```
- Matches the current Fajar codegen (allocator copies + appends).
- Receiver never consumed. **Equivalent to NOT wiring E2.**
- Means full-strict default doesn't track method-call consumption at
  all — but since `&self` borrows are also untracked, this is
  internally consistent.

**Recommendation:** **E2.C** is the cheapest path. It matches the
current allocator behavior (`_fj_arr_clone` + push semantics) and
requires no codegen changes. Free-fn arg consumption still tracks via
the existing `mark_moved` at fn-arg sites; method receivers simply
borrow. This is internally consistent: a method call is a `&self`
borrow, a free fn call is a move.

If user wants stronger semantics later (e.g., for `into_iter`-style
consumers), individual methods can opt into `self`-takes via an
attribute like `@consume fn`. Out of scope for Option B.

**With E2.C decision: skip Phase 2 entirely.** Estimated effort drops
to **~4-6h** (just Phases 3+4+5).

## §4 — 5-Phase plan (with E2 decision pending)

Per CLAUDE.md §6.8 R1, each phase has a B0-style audit and runnable
verification. User authorization required between each phase.

### Phase 1 — B0 audit (THIS DOC) ✅ COMPLETE

- ✅ Empirical site map (§1)
- ✅ Pre-Phase-2 lib test list (§2)
- ✅ E2 design options (§3)
- ✅ Phase plan (this section)
- Effort actual: ~45min

### Phase 2 — E2 method-receiver consume (CONDITIONAL on §3 decision)

**Skip if user picks E2.C** (recommended).

If user picks E2.A or E2.B:
- Define `self`-kind in fn signature parser
- Wire `mark_moved` (E2.A) or borrow-tracking (E2.B) at MethodCall
  check site
- Add unit tests for method-call consumption tracking
- Re-run full-strict probe to count *new* cascade sites surfaced
- Estimated: 3-4h

### Phase 3 — Cascade `.clone()` insertions

12 base sites + N from Phase 2 (0 if E2.C). Each insertion:
- Edit one line in stdlib/{parser_ast,codegen_driver}.fj
- Re-run `cargo test --release --test selfhost_stage1_full` smoke
- Verify Stage 2 byte-equality preserved (`phase17_self_compile`)
- Commit per logical group (e.g., "v36.x.0 Phase 3a: 4 sites in
  parser_ast.fj")

Estimated: 3-5h. Possibly faster with batched commits.

### Phase 4 — Update pre-Phase-2 lib tests

4 tests in `mod.rs` (per §2). Flip from `is_ok()` to assertion on
diagnostic. Estimated: ~30min.

### Phase 5 — Flip default + closure ship

Single edit:
```diff
-pub fn is_copy_type(_ty: &Type) -> bool { true }
+pub fn is_copy_type(_ty: &Type) -> bool { is_copy_type_strict(_ty) }
```

Plus:
- CHANGELOG entry: "v35.5.0 (or v36.0.0) — FJARR_LEAK Phase 2 D-FULL
  full strict default-on. Closes Compass §4.4 'sebagai default'."
- CLAUDE.md §3 update
- Findings doc closure
- GitHub Release

Estimated: ~45min.

**Total realistic effort (with E2.C decision): ~5-6h.**
**Total realistic effort (with E2.A/B decision): ~8-10h.**

## §5 — Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| New cascade sites surface after each `.clone()` insertion | High | Re-run stage1_full per insertion; pre-push hook gates phase17 |
| Phase 17 byte-equality breaks (Stage 2 self-compile diverges) | Low-Medium | Affine wire is analyzer-only; codegen unchanged. But `.clone()` insertions in stdlib alter the materialized chain, which Stage 2 must reproduce. Verify byte-identical at every commit. |
| Performance regression from `.clone()` allocations in chain | Low (small) | Each `.clone()` allocates one [T] copy via arena. Chain bootstrap currently ~38s interp / 0.66s native; expected impact <5%. Benchmark per phase. |
| E2 design decision (§3) is wrong | Medium | E2.C is reversible (just don't wire E2); E2.A/B require codegen changes that are harder to unwind. **Lean E2.C unless user has explicit reason for stronger.** |
| Lib test cascade (more than 4 tests fail) | Low | Empirical scan said 4. Verified by source-grep on `Copy now\|Rc-based`. |

## §6 — Self-check (per CLAUDE.md §6.8)

```
[x] Pre-flight audit (B0/C0/D0) exists?                          (R1: this doc IS Phase 1 B0)
[x] Every action in §4 has a runnable verification command?      (R2: all gates listed by name)
[x] Prevention mechanism specified?                               (R3: pre-push hook gates phase17 + stage1_full per phase)
[x] Agent-produced numbers cross-checked with Bash?               (R4: 12 sites verified via source-grep)
[x] Effort variance tagged?                                       (R5: §0 table compares original vs revised)
[x] Decisions are committed files?                                (R6: this doc + §3 E2 choice)
[x] No public-artifact drift in this audit?                       (R7: no shipped changes)
[x] Multi-repo state check?                                       (R8: only fajar-lang touched; clean)
```

## §7 — Verification commands

To reproduce the empirical numbers:

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Apply full-strict probe (REVERT BEFORE COMMIT)
# Edit src/analyzer/borrow_lite.rs:588 from `true` to `is_copy_type_strict(_ty)`

# Count unique stdlib sites (consistent across all tests)
for t in full_p1 full_p57 full_p86; do
  echo "=== $t ==="
  cargo test --release --test selfhost_stage1_full $t -- --nocapture --test-threads=1 2>&1 \
    | grep -oE "moved at byte [0-9]+" | sort -u | wc -l
done
# Expected: 12 12 12

# List unique moved variables
cargo test --release --test selfhost_stage1_full full_p57 -- --nocapture --test-threads=1 2>&1 \
  | grep -oE "(SE024|ME001): use of moved (variable|\`\[T\]\` array) '[^']+'" | sort -u

# Source-grep verification of 12 sites (from §1 table)
grep -n "rhs.ast.push(\"BINOP\").push(op)" stdlib/parser_ast.fj
grep -n 'let mut name = raw_name\|if declared_type != ""\|if r != ""' stdlib/codegen_driver.fj

# Always revert + verify GREEN
git checkout src/analyzer/borrow_lite.rs
cargo test --release --test selfhost_stage1_full -- --test-threads=1 2>&1 | tail -3
# Expected: 86 passed; 0 failed
```

## §8 — Cumulative state at Phase 1 close

| Aggregate | At v35.4.1 HEAD (probe reverted) |
|---|---|
| `cargo test --release --test selfhost_stage1_full` | **86/86 PASS @ ~13.2s** |
| `cargo test --release --test selfhost_phase17_self_compile` | 4/4 PASS (Stage 2 byte-equality preserved through audit) |
| Working tree | clean (`git diff src/analyzer/borrow_lite.rs` empty) |
| Phase 1 ship-status | 0 commits (audit only); this doc is the deliverable |
| Untracked | `docs/D_FULL_CASCADE_B0_FINDINGS.md` (prior B0) + `docs/D_FULL_OPTION_B_PHASE_PLAN.md` (this doc) + `docs/1/` |

## §9 — Decision gate for next session

**Two questions for user before Phase 2:**

1. **E2 design** (§3): pick E2.A (consume), E2.B (&mut self), or
   E2.C (&self / skip). Recommendation: **E2.C**.
2. **Phase ordering**: with E2.C → straight to Phase 3 cascade (~3-5h
   in one session). With E2.A/B → Phase 2 first (~3-4h), then re-run
   audit, then Phase 3.

Per `feedback_lanjutkan_rekomendasi.md`: STOP after this Phase 1
ship. Phase 2 (or Phase 3 if E2.C) requires explicit user
authorization with answers to the two questions above.

---

*D-FULL Option B Phase Plan — written 2026-05-10. Phase 1 (deeper
B0 audit) complete: 12 stdlib sites mapped, 4 of 5 lib tests
identified as breaking, E2 design options surfaced. Total realistic
effort revised to ~5-6h (E2.C) or ~8-10h (E2.A/B), within original
6-12h window. Probe reverted, 86/86 stage1_full GREEN. User
authorization required to proceed to Phase 2.*
