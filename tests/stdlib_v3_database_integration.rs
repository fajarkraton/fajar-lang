//! TQ12.2 SQLite — full pipeline integration tests.
//!
//! These tests exercise the COMPLETE path `parse → analyze → eval`
//! for all 7 db builtins (db_open, db_execute, db_query, db_close,
//! db_begin, db_commit, db_rollback). They cover the gap that
//! `src/stdlib_v3/database.rs` unit tests miss: those tests call the
//! Rust API directly and bypass the analyzer's name-resolution step.
//!
//! Closure of TQ12.2 per `docs/TQ12_2_SQLITE_B0_FINDINGS.md` §4 step 2.
//! Without this suite, the 4 missing builtins (db_close, db_begin,
//! db_commit, db_rollback) in `src/analyzer/type_check/register.rs`
//! would not be caught — analyzer rejects them with SE001 "undefined
//! variable".

use fajar_lang::interpreter::Interpreter;

fn run(src: &str) -> Result<(), String> {
    let mut interp = Interpreter::new();
    interp
        .eval_source(src)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn db_open_execute_query_close_full_pipeline() {
    // Smoke test: covers the complete flow that B0 §2 verified breaks
    // without the register.rs additions. This MUST analyze + run cleanly
    // — no SE001 on db_close.
    run(r#"
fn main() {
    let db = db_open(":memory:")
    let _ = db_execute(db, "CREATE TABLE t (n INTEGER)", [])
    let _ = db_execute(db, "INSERT INTO t VALUES (42)", [])
    let rows = db_query(db, "SELECT n FROM t", [])
    println(to_string(len(rows)))
    db_close(db)
}
"#)
    .expect("full pipeline (open/execute/query/close) should succeed");
}

#[test]
fn db_transaction_commit_full_pipeline() {
    // Exercise db_begin + db_commit through the analyzer.
    run(r#"
fn main() {
    let db = db_open(":memory:")
    let _ = db_execute(db, "CREATE TABLE t (n INTEGER)", [])
    db_begin(db)
    let _ = db_execute(db, "INSERT INTO t VALUES (1)", [])
    let _ = db_execute(db, "INSERT INTO t VALUES (2)", [])
    db_commit(db)
    let rows = db_query(db, "SELECT n FROM t", [])
    println(to_string(len(rows)))
    db_close(db)
}
"#)
    .expect("transaction commit through analyzer should succeed");
}

#[test]
fn db_transaction_rollback_full_pipeline() {
    // Exercise db_begin + db_rollback through the analyzer.
    run(r#"
fn main() {
    let db = db_open(":memory:")
    let _ = db_execute(db, "CREATE TABLE t (n INTEGER)", [])
    let _ = db_execute(db, "INSERT INTO t VALUES (1)", [])
    db_begin(db)
    let _ = db_execute(db, "INSERT INTO t VALUES (99)", [])
    db_rollback(db)
    let rows = db_query(db, "SELECT n FROM t", [])
    println(to_string(len(rows)))
    db_close(db)
}
"#)
    .expect("transaction rollback through analyzer should succeed");
}
