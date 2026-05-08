---
phase: TQ12.2 SQLite — B0 pre-flight audit (2026-05-08)
status: B0 CLOSED — TQ12.2 is 90% done; ~15-30min closure work remaining
purpose: empirical verification of "in progress" status before any code work; pending memory `pending_tq12_2_sqlite.md` is stale (claimed file not created; actually 18KB exists with 19 passing tests)
---

# TQ12.2 SQLite — B0 Pre-Flight Audit Findings

> The pending memory `pending_tq12_2_sqlite.md` (created 2026-03-27)
> claimed `src/stdlib_v3/database.rs` had not been created. **Reality
> at 2026-05-08: file exists (18,492 bytes), 19 tests pass, builtins
> dispatched in interpreter. The remaining work is much smaller than
> the pending notes suggested.**

## §1 — Headline numbers

| Probe | Number | Significance |
|---|---|---|
| `Cargo.toml rusqlite` dep | ✅ `rusqlite = { version = "0.34", features = ["bundled"] }` | Already in place |
| `src/stdlib_v3/database.rs` LOC | **18,492 bytes** | NOT a stub — full implementation |
| `pub mod database;` in `src/stdlib_v3/mod.rs` | ✅ exists | Module wired |
| Database functions in `database.rs` | **6 pub** (open, execute, query, close, is_open, begin, commit, rollback) | Plus `DbManager` struct + `DbParam` / `DbValue` enums |
| Tests in `database.rs` | **19 PASS** (test_db_* + builtin_db_* + transaction_*) | All green per `cargo test --lib stdlib_v3::database` |
| Builtin dispatch in `src/interpreter/eval/builtins.rs` | ✅ 7 builtins (db_open / execute / query / close / begin / commit / rollback) at L3444-3464 | Wired |
| Builtin name list in `src/interpreter/eval/mod.rs` | ✅ all 7 names listed at L1810-1816 | Interpreter knows them |
| Analyzer name+sig table in `src/analyzer/type_check/register.rs` | ⚠️ **only 3 of 7** registered (L275-277) | **THIS IS THE BUG** |

## §2 — The bug

The analyzer's `register.rs` has registered:
- `db_open(*) -> i64`
- `db_execute(*) -> i64`
- `db_query(*) -> *`

But MISSING:
- `db_close(handle: i64) -> void`
- `db_begin(handle: i64) -> void`
- `db_commit(handle: i64) -> void`
- `db_rollback(handle: i64) -> void`

User-facing impact: any `.fj` source that calls `db_close(handle)` (or
the transaction primitives) is rejected by the analyzer with:

```
SE001: undefined variable 'db_close' — did you mean 'ws_close'?
```

…even though the interpreter would have happily executed it. Verified
empirically:

```fj
fn main() {
    let db = db_open(":memory:")
    let _ = db_execute(db, "CREATE TABLE t (n INTEGER)", [])
    let _ = db_execute(db, "INSERT INTO t VALUES (42)", [])
    let rows = db_query(db, "SELECT n FROM t", [])
    println(to_string(len(rows)))
    db_close(db)              // ← analyzer rejects with SE001
}
```

`cargo run -- run /tmp/db_smoke.fj` exits with SE001 on the `db_close`
line.

## §3 — Why this slipped through

1. The 19 tests in `database.rs` exercise the Rust API directly
   (`DbManager::open` / `execute` / `query` / `close`), bypassing the
   analyzer entirely. They're unit tests for the module, not
   integration tests through the .fj source pipeline.
2. The `builtin_db_*` tests in `database.rs` simulate the builtin call
   path via interpreter internals (Value::BuiltinFn) without going
   through the `analyzer::analyze` step.
3. No `.fj` example or integration test exercises the full path
   `parse → analyze → eval` for `db_close` etc. If there were, this
   would have surfaced months ago.
4. The pending memory `pending_tq12_2_sqlite.md` was never updated
   when the bulk of the work landed. The file was timestamped 2026-03-27
   but the implementation appears to have landed shortly after (modtimes
   on database.rs, builtins.rs sections all 2026-05-02..03 era).

## §4 — Closure plan (~15-30min)

Replace the original "~2-4h, 7-step plan" with this much smaller
sequence:

| Step | What | Effort |
|---|---|---|
| **1** | Add 4 entries to `src/analyzer/type_check/register.rs` after L277 (db_query line): `("db_close", vec![Type::I64], Type::Void)`, `("db_begin", vec![Type::I64], Type::Void)`, `("db_commit", vec![Type::I64], Type::Void)`, `("db_rollback", vec![Type::I64], Type::Void)` | ~5min |
| **2** | Add an integration test that exercises full pipeline: write `.fj` source using all 7 db builtins → `analyze` → `eval` → assert correct row count. File: `tests/stdlib_v3_database_integration.rs` (NEW) or extend existing `tests/integration.rs`. | ~10-15min |
| **3** | Smoke-test: re-run `/tmp/db_smoke.fj` from §2 above → expect "1\n" output (the row count). | ~2min |
| **4** | Update `pending_tq12_2_sqlite.md` memory to "CLOSED — see TQ12_2_SQLITE_B0_FINDINGS.md". | ~2min |
| **5** | Commit: `feat(stdlib_v3): TQ12.2 close — register db_close/begin/commit/rollback in analyzer name table + integration test` | ~3min |
| **Optional Z** | Push + tag v35.2.1 (patch bump for bugfix) + GitHub Release. Decision deferred to user. | ~5min |
| **Total** | | **~15-30min** |

No B1/B2/etc — this is a single-commit closure.

## §5 — Risks (per CLAUDE.md §6.8)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Adding analyzer entries breaks lib tests | LOW | Low | The 4 entries mirror existing patterns (similar to `tcp_close` at L1312); types match interpreter handler signatures | n/a |
| Integration test surfaces another gap | LOW | Low | If a test reveals more missing wires, scope creeps; STOP and re-evaluate. Single-step commit makes rollback trivial. | Single commit |
| Stage 2 byte-equality / Stage1_full / Phase17 regression | NONE | NONE | Changes are confined to Rust analyzer + tests; do not touch any `stdlib/*.fj` source nor codegen. | n/a |

## §6 — Decision gate (per CLAUDE.md §6.8 R6)

B0 closed → ready for **single-step closure commit** (~15-30min).

After TQ12.2 closes, the original "TQ12.2 SQLite finish (~2-4h)" line
in next-session pickup recommendations should be removed. The
`pending_tq12_2_sqlite.md` memory should be archived or deleted.

The next-default work item after TQ12.2 closure is unrelated. Per
MEMORY.md "Pending work" the candidates remain: language fix #2
(`len()` → i64), language fix #3 (stack overflow on large programs),
crypto CQ1.3/CQ1.4, or template hardware tasks (TQ12.4-12.6 need Q6A
online).

---

*TQ12_2_SQLITE_B0_FINDINGS — written 2026-05-08. Surfaces that
TQ12.2 SQLite is 90% done; the pending memory was stale by ~6 weeks.
Real remaining work: register 4 builtins (db_close, db_begin,
db_commit, db_rollback) in `src/analyzer/type_check/register.rs` +
add 1 integration test through the full parse→analyze→eval pipeline.
Estimated ~15-30min closure vs original ~2-4h pending estimate.*
