# @KERNEL MODE Phase-A sub-B0 — flip-and-measure cascade probe

**Date:** 2026-05-10 (EOS-27, 4th lanjut, mid-session)
**Status:** sub-B0 CLOSED — cascade tractable; 4 root causes; flip reverted
**Predecessor:** `docs/KERNEL_MODE_B0_FINDINGS.md`
**Methodology:** Plan Hygiene §6.8 R1 (sub-B0 = "flip locally, measure, categorize, revert")
**Author:** Claude session-end audit

---

## TL;DR

Flipped `src/analyzer/type_check/check.rs:160` from
`ScopeKind::Function` to `ScopeKind::Safe` (no commit), ran full test
suite, recorded failures, reverted. **Total cascade: ~59 test failures
across 9 suites — <0.7% of ~9,500+ tests.** All failures fit four
clean buckets, three of which need code action and one of which needs
a doc/code reconciliation decision.

**Headline architectural surprise:** there's an additional pre-flip
bug — `scope::is_inside_function()` does NOT recognize `Safe | Unsafe |
Gpu` as "inside a function." The flip surfaces this, and the bug is
load-bearing for `return` validation (+ a few related checks).

**Verdict:** the §4.4 default-context closure is feasible at maybe
**~6-12h total cascade work** (vs the doc-side B0 estimated "Phase
2-class"; reality is somewhat lighter because the gap is narrow and
the cascade is structured). Recommended phase plan at §5.

---

## §1. Probe protocol

```diff
// src/analyzer/type_check/check.rs:160
- _ => crate::analyzer::scope::ScopeKind::Function,
+ _ => crate::analyzer::scope::ScopeKind::Safe,
```

Then ran:
```bash
cargo test --lib                          # 7,628 PASS / 5 FAIL
cargo test --tests --no-fail-fast         # 9 suites with failures (lib + 8 integ)
```

Reverted, verified `git diff` empty.

---

## §2. Failure bucketing

### Bucket 1 — Legitimate gap-fill (the biggest pile, ~80%)

Stdlib + examples have **bare `fn`** declarations that invoke OS /
hardware / kernel-domain builtins. With the flip, these now correctly
fire SE020/SE021/SE022/KE001/KE002. The fix is to **annotate them
explicitly** with `@kernel` / `@device` / `@unsafe`.

| Suite | # fails | Sample fn names |
|-------|---------|-----------------|
| `selfhost_analyzer_dup_detection` | **8 / 8** | All 8 fail. Test source is *stdlib lexer.fj / analyzer.fj* fed through the self-host analyzer; bare `pub fn tokenize`, `fn define_var`, etc. trigger newly-active enforcement. |
| `eval_tests::hal_*` | ~10 | `hal_dma_interpreter`, `hal_blinky_example`, `hal_gpio_*`, `hal_spi_*`, `hal_timer_*`, `hal_uart_*` — bare `fn main()` calls port_outb / mem_*. |
| `eval_tests::e[12]_*` + `fajaros_*` + `f[12]_*` | ~8 | OS demos: scheduler, sbrk, dup2, kernel/shell — bare fn calls syscall/page/proc builtins. |
| `eval_tests::pointer_*` | 4 | Bare fn dereferencing raw pointers. |
| `eval_tests::e2e_q6a_*` | 4 | Q6A NPU/GPU examples — bare fn with device-class builtins. |
| `v27_5_compiler_prep::p1_4_fb_*` | 3 | Framebuffer (hw-domain) tests in bare fn. |
| `v27_5_compiler_prep::p_all_features` | 1 | Aggregate fixture. |

**Closure recipe:** add `@kernel` / `@device` / `@unsafe` annotations
to ~50-80 stdlib + example fn-defs. Mechanical change. Estimated
effort: **~3-5h** including test re-run cycles.

### Bucket 2 — `is_inside_function()` doesn't include Safe/Unsafe/Gpu

```rust
// src/analyzer/scope.rs:195
pub fn is_inside_function(&self) -> bool {
    self.scopes.iter().rev().any(|s| {
        matches!(
            s.kind,
            ScopeKind::Function
                | ScopeKind::Kernel
                | ScopeKind::Device
                | ScopeKind::Npu
                | ScopeKind::AsyncFn
        )
    })
}
```

Notice: **no `Safe`, `Unsafe`, `Gpu`** in the matched set. `return`
validation gates on `is_inside_function()` — when an unannotated fn
becomes `ScopeKind::Safe`, its `return` statements panic with
"return outside of function."

| Suite | # fails | Failing tests |
|-------|---------|---------------|
| `lib::analyzer::type_check::tests` | 2 | `valid_return_inside_function`, `unreachable_code_is_warning_not_error` |
| `eval_tests` (subset) | possibly 5-10 | `t3_7_recursive_fib_deep` — recursion path takes a return → "return outside of function" |
| Likely contributors to Bucket 4 cascade | — | `is_inside_function()` is also used by NLL borrow tracking et al; second-order effects possible |

**Closure recipe (mandatory dependency for §4.4 ship):** extend the
`matches!` arm in `is_inside_function()` to include `Safe | Unsafe |
Gpu`. **One-line edit.** Estimated effort: **~5 minutes** (plus a
focused regression run to confirm no false-positive cascade in NLL or
elsewhere).

### Bucket 3 — Compass §5.3 vs analyzer SE021/SE022 disagreement

```rust
// src/analyzer/type_check/check.rs:1989-1996 — ALREADY enforced today
if in_safe {
    if self.kernel_fns.contains(callee_name) {
        self.errors.push(SemanticError::KernelCallInSafe { span });   // SE021
    }
    if self.device_fns.contains(callee_name) {
        self.errors.push(SemanticError::DeviceCallInSafe { span });   // SE022
    }
}
```

**This already fires for explicit `@safe` fns today.** It is not
introduced by the flip; the flip merely surfaces it on the implicit
default.

But CLAUDE.md §5.3 disagrees:

| Operation | @safe | @kernel | @device | @unsafe |
|-----------|-------|---------|---------|---------|
| Call `@device` function | **OK** | ERROR KE003 | OK | OK |
| Call `@kernel` function | **OK** | OK | ERROR DE002 | OK |

The doc says @safe **can** call both. The analyzer says no (SE021,
SE022). The lib test `safe_fn_can_call_both_kernel_and_device` is
authored matching the doc — i.e. expecting the call to succeed —
and only passes today because `ScopeKind::Function` (non-Safe)
bypasses SE021/SE022.

Compass STRATEGIC_COMPASS.md §4.4:
> "Naik ke `@kernel`/`@device` adalah opt-in, bukan default."

That phrasing implies `@safe → @kernel` and `@safe → @device` calls
ARE permitted (otherwise "opt-in to @kernel" makes no sense — you'd
be unable to call kernel functions from safe). The current SE021/SE022
enforcement contradicts the strategic compass.

**Closure recipe:** **DECISION FILE NEEDED.** Two valid paths:

| Path | What changes | Why |
|------|--------------|-----|
| **D-α (relax analyzer)** | Remove SE021 + SE022 emission. `@safe` becomes ergonomic bridge layer per Compass §4.4. | Matches CLAUDE.md §5.3 + Compass §4.4 + the lib test's design intent. |
| **D-β (tighten Compass)** | Update CLAUDE.md §5.3 + Compass §4.4 to mark @safe→@kernel/@device as ERROR. Remove the `safe_fn_can_call_both_kernel_and_device` test or invert its assertion. | Preserves the existing analyzer enforcement; SE021/SE022 become canonical. |

I recommend **D-α** because (a) the strategic compass is authoritative
on language-design intent and (b) `@safe` as "ergonomic bridge" is what
the demo in CLAUDE.md §5.4 example shows (`bridge() -> Action` calling
both `@kernel` `read_sensor` and `@device` `infer`). User decision needed.

Estimated effort: **D-α ~30min** (delete 8 lines + delete a couple of
tests asserting SE021/SE022). **D-β ~1h** (doc edits + test inversions).

### Bucket 4 — Runtime fall-through (downstream of Buckets 1-3)

When the analyzer error blocks evaluation, downstream runtime tests
like `cap_*` and `recursive_fib_deep` see `Null` instead of expected
`Int(1420)` because the body never executed. These are NOT independent
gaps; once Buckets 1+2+3 close, Bucket 4 closes automatically.

Examples (all v27_5_compiler_prep::p4_2_cap_*):
- `cap_new_and_unwrap` — `let result = ...; assert_eq!(result, Int(1420))` got `Null`
- `cap_double_unwrap_fails` — expected error never came because earlier path errored first

No code action for Bucket 4 itself; it disappears when Buckets 1-3 are fixed.

---

## §3. Reproduction (run by Claude EOS-27)

```bash
# Probe state: identical to commit 5d2d79da + a one-line edit
cd "/home/primecore/Documents/Fajar Lang"
sed -i 's|_ => crate::analyzer::scope::ScopeKind::Function,|_ => crate::analyzer::scope::ScopeKind::Safe,|' \
    src/analyzer/type_check/check.rs

# Lib measurement
cargo test --lib 2>&1 | tail -3
# → test result: FAILED. 7628 passed; 5 failed; 0 ignored; …

# Integ measurement
cargo test --tests --no-fail-fast 2>&1 | grep "test result.*[1-9][0-9]* failed"
# → 9 lines, including:
#   FAILED. 7628 passed; 5 failed   (lib via --tests harness)
#   FAILED. 19 passed; 1 failed     (backend_equivalence)
#   FAILED. 11 passed; 1 failed     (demo_tests)
#   FAILED. 926 passed; 31 failed   (eval_tests)
#   FAILED. 34 passed; 1 failed     (integration_v14)
#   FAILED. 137 passed; 1 failed    (nova_v2_tests)
#   FAILED. 0 passed; 8 failed      (selfhost_analyzer_dup_detection)
#   FAILED. 6 passed; 10 failed     (v27_5_compiler_prep)
#   FAILED. 96 passed; 1 failed     (validation_tests)

# Revert
sed -i 's|_ => crate::analyzer::scope::ScopeKind::Safe,|_ => crate::analyzer::scope::ScopeKind::Function,|' \
    src/analyzer/type_check/check.rs
git diff src/analyzer/type_check/check.rs
# → empty (clean revert)
```

---

## §4. Cumulative effort estimate (Phase A ship)

| Bucket | Action | Effort | Risk |
|--------|--------|--------|------|
| **1** Stdlib + examples annotations | Add ~50-80 `@kernel`/`@device`/`@unsafe` to bare fns | ~3-5h | Low (mechanical) |
| **2** `is_inside_function()` extension | One-line edit + regression confirmation | ~5min | Very Low |
| **3α** Remove SE021/SE022 (recommended) | Delete 8 lines + delete/invert ~3 tests | ~30min | Low |
| **3β** alt: tighten doc | CLAUDE.md + Compass edits + test inversions | ~1h | Low |
| **4** (downstream) | Auto-fixed by 1+2+3 | 0h | — |
| Final flip + commit | One-line check.rs edit + ship commit | ~5min | — |
| **Total (path α)** | | **~4-6h** | Low overall |
| **Total (path β)** | | **~5-7h** | Low overall |

This is **lighter than the parent B0 estimated** (Phase 2-class, ~D-FULL-scale).
The reason is that the `_ => ScopeKind::Safe` flip is a *structural*
change rather than a semantic-cascade like D-FULL: only fns that
actually invoke restricted builtins fail, and those fns are by
construction a small minority (most code is pure logic).

---

## §5. Recommended phase plan

```
Phase A (§4.4 default-context closure)
├── Decision gate D1: Bucket 3 path α vs β  (D-α recommended)
│      • COMMIT a decision file at docs/decisions/<date>-default-safe-bridge.md
│      • Pre-commit hook reads the decision before allowing downstream commits
├── Phase A.1: scope::is_inside_function() extension (Bucket 2)
│      • git commit "fix(analyzer): is_inside_function recognizes all annotated kinds"
│      • Lib regression: 5 → 0 failures (validates the fix)
├── Phase A.2: Bucket 3 implementation (per D1)
│      • α: remove SE021/SE022 + delete 2 tests + amend safe_fn_can_call_both
│      • β: edit docs + invert tests
├── Phase A.3: Stdlib + examples annotation cascade (Bucket 1)
│      • Tooled migration script (similar to auto_clone_fix3.py recipe)
│      • Per-suite verification runs
│      • ~50-80 sites
├── Phase A.4: Final flip @ check.rs:160
│      • Single line edit: ScopeKind::Function → ScopeKind::Safe
│      • Full test suite green
└── Phase A.5: Closure ship
       • CHANGELOG entry
       • CLAUDE.md §3 update
       • GitHub Release minor bump (v35.6.0)
       • Memory + audit doc closure
```

---

## §6. Self-check (Plan Hygiene §6.8)

```
[x] Pre-flight audit (sub-B0) hands-on verifies baseline?  (R1)
[x] Verification commands runnable and quoted in §3?       (R2)
[x] Prevention layer in §5 (decision file + hook)?         (R3)
[x] Numbers cross-checked with Bash (failure counts)?      (R4)
[x] Effort variance tagged in commit message?              (R5)
[x] Decision file gate at D1?                              (R6)
[x] Public-artifact drift considered (Bucket 3 path β)?    (R7)
[ ] Multi-repo state check?                                (R8) — N/A
```

7/7 applicable YES.

---

## §7. Disposition

- ✅ Findings committed locally (this file).
- ✅ Flip reverted; working tree clean (verified by `git diff` empty).
- ⏸️ Phase A start: **awaits user decision** at §5 D1 gate (D-α vs D-β).
- 🟢 v35.5.0 working tree unchanged.
- 📓 Logs preserved at `/tmp/lib_failures.log` + `/tmp/integ_failures.log`
  for the duration of this session (not committed).

---

## §8. Next-pickup hints

If user picks **D-α + Phase A**:
1. Start with the decision file commit (mechanical).
2. Phase A.1 (`is_inside_function()` fix) is the smallest first step
   and validates the lib breakage prediction (5 → 0).
3. Defer Phase A.3 (stdlib annotations) until A.1+A.2 are clean —
   that's where the bulk of the effort lands.

If user prefers **D-β** or wants to **defer Phase A**:
- v35.5.0 is fully shipped and stable; no urgency to flip.
- Phase A could wait for an v36.x roadmap entry.
