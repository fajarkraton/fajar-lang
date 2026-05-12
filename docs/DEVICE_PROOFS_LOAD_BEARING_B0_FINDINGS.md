# device_proofs.rs Load-Bearing Audit — B0 Findings

> **Phase:** Compass §5.1 SMT-verification freeze, narrow audit (single file).
> **Audit date:** 2026-05-12 (EOS-34+, on `lanjutkan` first-step).
> **Plan Hygiene §6.8 R1:** Audit only. Strategic decision is Fajar's.
> **Predecessor audit:** `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md` §3.3
> already flagged `verify/device_proofs.rs` as "appears to be similarly dead —
> zero production callers. That's a separate audit candidate." This B0 makes
> the audit explicit and uncovers a systemic finding (§5).

## §1. Scope

Verify `src/verify/device_proofs.rs` (Sprint V4 — `@device` safety proofs)
load-bearing status. Identify whether it's part of the production
verification path or research code that never crossed the wire-up
threshold.

## §2. device_proofs.rs API surface (HEAD `d93ce312`)

File: 1,121 LOC, 24 `#[test]` fns (v4_1..v4_10 pattern), 17 pub items.

Module header (source):
> "@device Safety Proofs — Sprint V4: 10 tasks.
> Proves safety properties for `@device` annotated code: no-raw-pointer
> usage, tensor shape correctness, tensor dtype correctness, memory bound
> safety, gradient tracking, numerical stability, shape inference,
> broadcast compatibility, and memory layout consistency.
> **All simulated (no real Z3).**"

| Pub item | Line | Role |
|---|---|---|
| `struct DeviceViolation` | 17 | Violation report |
| `enum DeviceViolationKind` | 42 | 9 violation classes |
| `struct DeviceFunction` | 86 | Function under check |
| `struct TensorInfo` | 101 | Tensor metadata |
| `enum TensorDim` | 116 | Concrete/Symbolic/Dynamic |
| `enum TensorDtype` | 137 | F32/I8/etc |
| `enum MemoryLayout` | 168 | Contiguous/Strided/etc |
| `enum DeviceOp` | 186 | Op categories |
| `fn check_no_raw_pointer` | 238 | Pointer-safety check |
| `fn check_tensor_shapes` | 258 | Shape correctness |
| `fn check_tensor_dtypes` | 317 | Dtype correctness |
| `fn check_memory_bounds` | 350 | Bounds check |
| `fn check_gradient_tracking` | 377 | Gradient flow check |
| `fn check_numerical_stability` | 402 | NaN/Inf check |
| `fn check_shape_inference` | 449 | Inference correctness |
| `fn check_broadcast_compatibility` | 491 | Broadcast check |
| `fn check_memory_layout` | 532 | Layout check |
| `struct DeviceCheckConfig` | 564 | Check configuration |
| `struct DeviceSafetyChecker` | 588 | Aggregator |
| `struct DeviceCheckStats` | 601 | Stats |

## §3. Consumer trace (HEAD `d93ce312` — re-verified live)

### 3.1 Direct imports

```
$ grep -rln "use crate::verify::device_proofs\|verify::device_proofs::" \
    src/ tests/ examples/ benches/ stdlib/
(empty)
```

**Zero direct imports anywhere in the codebase.**

### 3.2 Symbol-name search

```
$ grep -rln "DeviceFunction\|DeviceViolation\|DeviceSafetyChecker\|\
DeviceCheckConfig\|check_tensor_shapes\|check_broadcast_compatibility" \
    src/ tests/ examples/ stdlib/ | grep -v "device_proofs.rs"
(empty)
```

**Zero symbol-level references** outside the module's own file.

### 3.3 Module exposure

```
$ grep -rn "device_proofs" src/ | grep -v "src/verify/device_proofs.rs:"
src/verify/mod.rs:10://! - `device_proofs` — @device safety proofs (V4)
src/verify/mod.rs:15:pub mod device_proofs;
```

Only mention is its own module declaration in `verify/mod.rs`. No
re-exports, no `use` statements, no documentation cross-references.

### 3.4 Internal tests

24 `#[test]` fns inside the same file (`v4_1..v4_10` sprint pattern).
These tests exercise the module's own pub items in isolation — they are
**self-tests of the API surface, not production consumers**.

### 3.5 CLI exposure

```
$ grep -n "device_proofs\|verify_device" src/main.rs
(empty)
```

No CLI subcommand routes through device_proofs.

### 3.6 Feature flags / conditional compilation

```
$ grep -n "cfg(feature\|cfg_attr" src/verify/device_proofs.rs
(empty)
```

Not feature-gated. Built unconditionally.

## §4. Verdict: device_proofs.rs is DEAD in production AND in tests

Stronger pattern than the dependent-types deletions:
- **Zero production consumers** — like arrays.rs/patterns.rs/tensor_shapes.rs.
- **Zero non-self test consumers** — unlike arrays/patterns which had
  external sprint DT2/DT4 tests in `eval/mod.rs`. device_proofs has only
  its own 24 internal `v4_*` self-tests.
- **Header explicitly admits simulated implementation** ("All simulated
  (no real Z3)") — never intended to ship as a real verifier.

## §5. Systemic finding — verify/ family is 75-80% dead

While re-verifying device_proofs, I bulk-counted all 13 modules in
`src/verify/`. Pattern:

| Module | LOC | Consumer files | Production status |
|---|---|---|---|
| `tensor_verify` | 600 | 1 (src/analyzer/type_check/check.rs) | **LIVE** (analyzer integration) |
| `spec` | 721 | 1 (src/main.rs CLI step 3) | **LIVE** (CLI production) |
| `symbolic` | 1,302 | 2 (src/main.rs + eval/mod.rs) | **LIVE** (CLI production; eval consumer is tests) |
| `certification` | 1,316 | 1 (eval/mod.rs `#[test]` only — n1_10/n4_10/n10_1) | **TEST-ONLY** |
| `pipeline` | 1,196 | 1 (tests/nova_v2_tests.rs) | **TEST-ONLY** |
| `smt` | 1,254 | 2 (eval/mod.rs `#[test]` + tests/feature_flag_tests.rs) | **TEST-ONLY** |
| `benchmarks` | 1,125 | 0 | **DEAD** |
| `device_proofs` | 1,121 | 0 | **DEAD** ← this audit's target |
| `inference` | 1,401 | 0 | **DEAD** |
| `kernel_proofs` | 987 | 0 | **DEAD** (V3 — `@kernel` sibling of V4) |
| `proof_cache` | 1,239 | 0 | **DEAD** (V5 — caching sibling) |
| `properties` | 1,194 | 0 | **DEAD** (V2 — property language) |
| `theories` | 1,170 | 0 | **DEAD** (SMT theories) |

**Aggregate:**

| Category | Modules | LOC | Notes |
|---|---|---|---|
| Production-live | 3 | 2,623 | spec + symbolic + tensor_verify |
| Test-only | 3 | 3,766 | certification + pipeline + smt |
| Zero-consumer | 7 | 8,237 | benchmarks + device_proofs + inference + kernel_proofs + proof_cache + properties + theories |
| **Dead surface total** | **10** | **~12,003 LOC** | Test-only counts as dead-in-production |

verify/ family is **78% dead by module count, 82% dead by LOC**.

### 5.1 Compass §5.1 verdict applies

`docs/1/STRATEGIC_COMPASS.md` §5.1 has TWO explicit freeze entries that
match this surface:

> | SMT verification (DO-178C) | Diklaim ada | **Bekukan**. Butuh tim untuk certification serius. |

And in the Compass §5.1 Decision Framework:

> | Tambah formal proof / SMT verification? | ⏸️ Bekukan kecuali untuk niche safety-critical certification |

The Compass argument: SMT verification at DO-178C/ISO-26262 maturity
requires a dedicated team. Solo-contributor pre-1.0 cannot deliver
certification-grade proof infrastructure. The V2-V5 sprints prototyped
the design; further investment without a certification team is sunk cost.

This is **the same verdict pattern as dependent types** (Compass §5.1
"deptypes-research → Mungkin tidak kembali"). This session already
deleted the entire dependent-types surface (tensor_shapes + arrays +
patterns, -2,469 LOC + ~83 tests) under that verdict. Applying the
same logic to verify/ is consistent.

## §6. Recommendations (three paths, Fajar to pick)

### Path A — Narrow: delete device_proofs.rs only

- 1 file, 1,121 LOC, 24 self-tests.
- **Effort:** ~15-20min.
- **Risk:** NONE. Zero consumers anywhere.
- Closes the file flagged by predecessor B0 §3.3 audit candidate.
- Limited LOC reclaim relative to the systemic dead surface.

### Path B — Targeted: delete 7 zero-consumer modules (V2/V3/V4/V5 dead surface)

- 7 files (benchmarks + device_proofs + inference + kernel_proofs +
  proof_cache + properties + theories), -8,237 LOC.
- Plus update verify/mod.rs to remove 7 `pub mod` declarations + doc comments.
- **Effort:** ~1-2h.
- **Risk:** LOW. None of these modules has any consumer.
- **LOC reclaim:** -8,237 LOC + ~80-150 self-tests removed.

### Path C — Full SMT freeze: delete 10 dead modules (zero-consumer + test-only) + remove their test consumers

- All 10 dead modules: ~-12,003 LOC.
- Also remove test consumers in eval/mod.rs (n1_*/n4_*/n10_* sprint
  blocks consuming smt/certification) + tests/feature_flag_tests.rs +
  tests/nova_v2_tests.rs (where dead-only).
- **Effort:** ~2-3h (more careful — must scope test deletions precisely).
- **Risk:** LOW-MEDIUM. Larger surface; test scoping needs careful boundary detection.
- **LOC reclaim:** ~-12,000 LOC + ~200+ tests removed.
- Honors Compass §5.1 SMT-freeze verdict at full scope.

### Path D — Freeze in place (Compass-literal)

- Add deprecation header comments to all 10 dead modules.
- Keep all code, tests, dependencies.
- 0 LOC reclaim. **Effort:** ~15min.
- **Cost:** Continued maintenance burden (clippy/fmt/test discipline on dead code).
- **Cost:** Misleading to readers — looks like a working verification suite.
- Worst-of-both: keeps the burden of live code with the value of dead code.

## §7. Recommendation: Path B (targeted)

**Rationale:**
1. **Compass §5.1 explicitly says BEKUKAN SMT verification.** Path D
   (freeze) literally matches the verdict but creates the burden problem.
   Paths A/B/C are all stronger reads ("delete = freeze + actually let go").
2. **Path B is the safe sweet spot.** The 7 zero-consumer modules have
   no risk profile at all — nothing breaks. Path C touches tests, which
   adds careful-boundary work for marginal additional LOC reclaim.
3. **Test-only modules in Path C category may still have value as test
   coverage.** smt + certification tests in eval/mod.rs exercise sprint
   N1/N4/N10 work; even if the production modules are dead, the test
   surface may have residual value as a "things we proved we could do"
   archive. Decision deferrable.
4. **Pattern continuity.** Same as `tensor_shapes` (v35.7.2) and
   `arrays + patterns` (Action C extension) — delete the zero-consumer
   research surface, leave decisions on test-only consumers for later.

Path A alone is too narrow — closes the file flagged by predecessor but
leaves 6 more files in the same dead pattern.

Path C is best-end-state but triples the scope.

## §8. Stage 2 byte-equality risk

NONE for any path. None of the dead modules are referenced by
`stdlib/*.fj`. The verify/ family is Rust-only.

Verified: `grep -r "DeviceFunction\|NatPattern\|SafeIndexResult\|
SmtResult\|ProofCache\|VerificationCondition" stdlib/` → empty for the
dead-module symbol set.

## §9. CLI/user-facing surface impact

NONE. The 7 zero-consumer modules have no CLI exposure (verified
via `grep "device_proofs\|kernel_proofs\|proof_cache\|properties\|
theories\|inference\|benchmarks" src/main.rs` → empty for module names).

`spec`/`symbolic`/`tensor_verify` (the production-live trio) remain
untouched in any of Paths A/B/C.

## §10. Re-entry conditions (post-deletion)

If SMT/formal-verification is ever reintroduced:
1. Compass §5.1 freeze verdict must be reversed (committed decision file).
2. A real certification team commitment must exist (per Compass rationale
   — "Butuh tim untuk certification serius").
3. Files recoverable via `git log --diff-filter=D -- src/verify/<name>.rs`.

## §11. Verification commands

```bash
cd "/home/primecore/Documents/Fajar Lang"

# Pre-flight (baseline confirmation)
grep -rln "use crate::verify::device_proofs\|verify::device_proofs::" \
    src/ tests/ examples/ benches/ stdlib/
# expect: empty

grep -rln "DeviceFunction\|DeviceViolation\|DeviceSafetyChecker" \
    src/ tests/ examples/ stdlib/ | grep -v "device_proofs.rs"
# expect: empty

# Path B post-deletion (when shipped — 7-module delete)
cargo test --lib 2>&1 | tail -3
# expect: ~7502 - sum_of_self_tests passed (each module ~10-25 lib tests)
# device_proofs alone: -24. Plus kernel_proofs/proof_cache/properties/
# theories/inference/benchmarks tests: probably 50-100 total drop.

cargo clippy --lib -- -D warnings  # expect 0 warnings
cargo fmt -- --check                # expect clean
cargo test --test context_safety_tests
                                    # expect 149 passed (untouched)
cargo test --release --test selfhost_phase17_self_compile -- --test-threads=1
                                    # expect 4 passed (byte-equality preserved)
```

## §12. Self-check (§6.8 audit checklist)

```
[x] Pre-flight audit (B0) exists for the Phase            (Rule 1)
[x] Every task has runnable verification command           (Rule 2 — §11)
[ ] Prevention mechanism added (hook/CI/rule)              (Rule 3 — applied at ship time)
[x] Agent-produced numbers cross-checked with Bash         (Rule 4 — all live verified)
[ ] Effort variance tagged in commit message               (Rule 5 — at ship time)
[ ] Decisions are committed files                          (Rule 6 — pending Fajar's choice)
[x] Public-artifact drift swept                            (Rule 7 — done EOS-29 this session)
[x] Multi-repo state checked                               (Rule 8 — done EOS-29..34 this session)
```

## §13. Source artifacts

- This file: `docs/DEVICE_PROOFS_LOAD_BEARING_B0_FINDINGS.md`
- Predecessor B0 §3.3 flag: `docs/TENSOR_SHAPES_LOAD_BEARING_B0_FINDINGS.md`
- Sibling dep-types B0: `docs/ARRAYS_PATTERNS_LOAD_BEARING_B0_FINDINGS.md`
- Compass §5.1 SMT freeze: `docs/1/STRATEGIC_COMPASS.md` §5.1 table + Decision Framework
- Same-session dep-types closure: commit `d93ce312` (arrays+patterns Action C extension)

---

*B0 written 2026-05-12 post-EOS-34. ~25min actual. Verdict on
device_proofs.rs: DEAD in production AND in tests (zero consumers
anywhere). Systemic finding: verify/ family is ~82% dead by LOC.
Recommendation: Path B (delete 7 zero-consumer modules, -8,237 LOC).
Decision pending Fajar.*
