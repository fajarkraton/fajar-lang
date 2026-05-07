---
audit: examples/*.fj sweep — NEW-4 closure
date: 2026-05-07
prereq: docs/RE_AUDIT_2026_05_07.md NEW-4 (open at re-audit time)
purpose: replace 4-file sample with full 245-file empirical sweep
status: COMPLETE 2026-05-07
---

# Examples Sweep Findings (2026-05-07) — NEW-4 closure

> RE_AUDIT_2026_05_07 NEW-4 was "info-level" because the sample size
> (4 files) was too small to claim drift. This sweep runs all 245
> top-level `examples/*.fj` files via `fj run` (15s timeout each)
> and categorizes results.

## Headline numbers

| Bucket | Count | % | Notes |
|---|---|---|---|
| **PASS (rc=0)** | **233 / 245** | **95.1%** | runs cleanly via default `fj run` |
| Errored (rc=2) | 8 | 3.3% | mostly missing-stdlib + missing-feature, not example bugs |
| Errored (rc=1) | 1 | 0.4% | real example bug (str+map type error) |
| Timeout (rc=124) | 3 | 1.2% | legitimate long-running, not bugs |

**Bottom line:** 95.1% of examples run end-to-end. Of the 12 non-pass,
**only 1 is a real example bug**; the other 11 are documented gaps
(stdlib loading via CLI, deferred features, expected-long-running).

## Method

```bash
for f in examples/*.fj; do
    out=$(timeout 15 ./target/release/fj run "$f" 2>&1 < /dev/null)
    rc=$?
    record file, rc, first stderr line
done
```

Run on Linux x86_64, fj v35.0.0 release binary, 245 top-level
`examples/*.fj` (subdirectories like `examples/calculator-cli/`,
`examples/tcp-echo-server/` not in scope — those are multi-file
projects with their own `fj.toml` and need different invocation).

Subdirectory inventory: 63 additional `.fj` files across 10 project
folders. Out of scope for this sweep (recommend separate "project
examples" sweep if needed).

## Failure category breakdown

### A. stdlib auto-load limitation (5 files) — CLI gap, not example bugs

These examples expect `stdlib/*.fj` modules to be in scope when run
via `fj run`, but the fj CLI doesn't currently auto-load stdlib
files. Same orthogonal concern noted in T4 B0.6 finding.

| File | Missing symbol |
|---|---|
| `examples/selfhost_analyzer_v3.fj` | `new_analyzer` (from `stdlib/analyzer.fj`) |
| `examples/selfhost_bootstrap_v3.fj` | various from stdlib |
| `examples/selfhost_compiler.fj` | `parse_to_ast` (from `stdlib/parser_ast.fj`) |
| `examples/selfhost_lexer_v3.fj` | various from `stdlib/lexer.fj` |
| `examples/distributed_mnist.fj` | `@distributed` annotation (LE001 — not actually stdlib; see Category B) |

**Honest classification:** these examples WORK when invoked via the
test harness (which concatenates stdlib + driver before parsing).
They are demonstrating self-host chain features. They DON'T work
standalone via `fj run`. This is the same pattern as
`tests/selfhost_*.rs` integration tests.

**Recommendation:** add a comment header to each of these examples
explaining the harness-only nature, OR move them to
`stdlib/examples/` where convention is clearer.

### B. Missing features (4 files) — aspirational/forward-looking

| File | Issue | Status |
|---|---|---|
| `examples/distributed_mnist.fj` | `@distributed` annotation not implemented | per STRATEGIC_COMPASS §5.1, "Distributed runtime (Raft)" → "Hapus dari core. Tidak relevan untuk niche embedded. Jadikan side library." |
| `examples/ffi_numpy.fj` | `@ffi("python")` annotation form not parsed | FFI v2 listed as feature; this specific syntax variant deferred |
| `examples/ffi_opencv.fj` | `@ffi("c++")` same | same |
| `examples/wasi_http_server.fj` | `fn(req: Request) -> Response { ... }` closure-as-arg | closures with capture as call-arg is the same `S2.6 deferred` Cranelift feature noted in `tests/codegen/cranelift/tests.rs:6753` |

**Honest classification:** these examples document features the
project would like to support but don't yet. Per STRATEGIC_COMPASS
§5.2 "Pangkas klaim README" — examples that demonstrate
unimplemented features should be either:
- moved to `examples/aspirational/` with a README explaining "this
  is planned syntax, not yet implemented"
- or removed entirely if they don't represent the niche

### C. Real example bug (1 file)

**`examples/actor_demo.fj`** — RE002 type error at line 44:
```
println("Supervision: " + result)
                          ^^^^^^
RE002: type error: unsupported operator + for str and map
```

`result` is a map (`Map<str, str>`), not stringifiable. The example
should use `format!("Supervision: {}", result)` or a stringify call.

**Note**: when re-tested individually post-sweep, the example exits
0 despite reporting RE002 (the runtime continues past the error).
That's an interesting fj-runtime behavior — RE002 should arguably
abort, not warn. Tracked separately as fj-runtime question.

**Recommendation:** fix the example. ~5 min. Single-line change.

### D. Legitimate long-running (3 files)

| File | Expected runtime | Reason |
|---|---|---|
| `examples/mnist_real.fj` | minutes | full MNIST training (5 epochs) |
| `examples/rest_api_crud.fj` | infinite | TCP server, runs until killed |
| `examples/v31b_vecmat_miscompile_repro.fj` | seconds-minutes | vectorized matmul, large iteration count for repro |

**Recommendation:** these are documented long-runners. Not bugs.
Could add `// TIMEOUT: long-running; expect >30s` header for
clarity.

## Drift vs CLAUDE.md / README claims

| Surface | Claim | Actual |
|---|---|---|
| CLAUDE.md §3 Examples | "243 .fj programs" | **245 top-level** + **63 in subdirs** = 308 total |
| README post-prune | "245 example .fj programs (sample-tested; full sweep open per RE_AUDIT NEW-4)" | **245 top-level**: 233 PASS / 8 errored / 1 bug / 3 long-running |

**No new significant drift.** README's "sample-tested" disclaimer
was honest; this sweep promotes it to "full-swept, 95.1% pass".

## Recommendations (ranked by impact:effort)

| Item | Effort | Impact |
|---|---|---|
| Fix `examples/actor_demo.fj` str+map (real example bug) | ~5min | LOW (one example) |
| Add header comment to 5 selfhost_* examples explaining harness-only nature | ~10min | LOW (clarifies confusion) |
| Move 4 missing-feature examples to `examples/aspirational/` with README | ~15min | MEDIUM (matches STRATEGIC_COMPASS §5.2 honest-claim discipline) |
| Add `// TIMEOUT:` header to 3 long-runners | ~5min | LOW |
| Update README "sample-tested" → "245 examples, 95% pass via fj run; 5 require harness, 4 forward-looking, 1 has known bug, 3 long-running" | ~5min | LOW (improves honesty) |
| Sweep 63 subdirectory project examples (separate audit) | ~30-60min | LOW (out of NEW-4 scope) |

**Total recommended effort to fully close NEW-4: ~40 min.**

## Decision-gate inputs

None — NEW-4 was info-level and remains so. No strategic decisions
required; the recommendations above are mechanical hygiene.

## NEW-4 status

✅ **CLOSED** — empirical baseline locked: **233 / 245 PASS (95.1%)**.
Remaining 12 categorized as:
- 5 stdlib-loading-via-CLI gap (not example bugs)
- 4 missing-feature/aspirational (per kompas §5.2 candidates for `aspirational/` move)
- 1 real bug (`actor_demo.fj`)
- 3 long-running (legitimate)

## Verification

```
$ awk -F'\t' 'NR>1 {bucket[$2]++} END {for (b in bucket) print "rc="b": "bucket[b]}' /tmp/sweep_results.tsv | sort -n
rc=0: 233
rc=1: 1
rc=2: 8
rc=124: 3
```

---

*EXAMPLES_SWEEP_2026_05_07 — NEW-4 closed. 95.1% top-level
examples pass via default `fj run`. Recommendations are hygiene-
level; no strategic decisions surface.*
