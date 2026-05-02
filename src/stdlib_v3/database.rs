//! SQLite database module — real rusqlite integration.
//!
//! Provides `db_open`, `db_execute`, `db_query`, `db_close` for Fajar Lang programs.
//! Connections are tracked by integer handles in the interpreter.

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

/// Global handle counter for database connections.
static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

/// Manages open SQLite connections by integer handle.
pub struct DbManager {
    connections: HashMap<i64, Connection>,
}

impl Default for DbManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DbManager {
    /// Create a new empty database manager.
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Open a SQLite database file (or ":memory:" for in-memory).
    /// Returns a unique handle ID.
    pub fn open(&mut self, path: &str) -> Result<i64, String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("db_open: failed to open '{}': {}", path, e))?;
        let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
        self.connections.insert(handle, conn);
        Ok(handle)
    }

    /// Execute a non-query SQL statement (INSERT, UPDATE, DELETE, CREATE TABLE, etc.).
    /// `params` are bound positionally (?1, ?2, ...).
    /// Returns the number of rows affected.
    pub fn execute(&self, handle: i64, sql: &str, params: &[DbParam]) -> Result<i64, String> {
        let conn = self
            .connections
            .get(&handle)
            .ok_or_else(|| format!("db_execute: invalid handle {}", handle))?;
        let rusqlite_params: Vec<Box<dyn rusqlite::types::ToSql>> = params
            .iter()
            .map(|p| -> Box<dyn rusqlite::types::ToSql> {
                match p {
                    DbParam::Int(v) => Box::new(*v),
                    DbParam::Float(v) => Box::new(*v),
                    DbParam::Text(v) => Box::new(v.clone()),
                    DbParam::Null => Box::new(rusqlite::types::Null),
                }
            })
            .collect();
        let refs: Vec<&dyn rusqlite::types::ToSql> = rusqlite_params.iter().map(|b| &**b).collect();
        let changed = conn
            .execute(sql, refs.as_slice())
            .map_err(|e| format!("db_execute: {}", e))?;
        Ok(changed as i64)
    }

    /// Execute a query and return rows as Vec of column-name → value maps.
    pub fn query(
        &self,
        handle: i64,
        sql: &str,
        params: &[DbParam],
    ) -> Result<Vec<HashMap<String, DbValue>>, String> {
        let conn = self
            .connections
            .get(&handle)
            .ok_or_else(|| format!("db_query: invalid handle {}", handle))?;
        let rusqlite_params: Vec<Box<dyn rusqlite::types::ToSql>> = params
            .iter()
            .map(|p| -> Box<dyn rusqlite::types::ToSql> {
                match p {
                    DbParam::Int(v) => Box::new(*v),
                    DbParam::Float(v) => Box::new(*v),
                    DbParam::Text(v) => Box::new(v.clone()),
                    DbParam::Null => Box::new(rusqlite::types::Null),
                }
            })
            .collect();
        let refs: Vec<&dyn rusqlite::types::ToSql> = rusqlite_params.iter().map(|b| &**b).collect();
        let mut stmt = conn.prepare(sql).map_err(|e| format!("db_query: {}", e))?;

        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                let mut map = HashMap::new();
                for (i, name) in col_names.iter().enumerate() {
                    let val = match row.get_ref(i) {
                        Ok(rusqlite::types::ValueRef::Null) => DbValue::Null,
                        Ok(rusqlite::types::ValueRef::Integer(v)) => DbValue::Int(v),
                        Ok(rusqlite::types::ValueRef::Real(v)) => DbValue::Float(v),
                        Ok(rusqlite::types::ValueRef::Text(v)) => {
                            DbValue::Text(String::from_utf8_lossy(v).to_string())
                        }
                        Ok(rusqlite::types::ValueRef::Blob(v)) => {
                            DbValue::Text(format!("<blob {} bytes>", v.len()))
                        }
                        Err(_) => DbValue::Null,
                    };
                    map.insert(name.clone(), val);
                }
                Ok(map)
            })
            .map_err(|e| format!("db_query: {}", e))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| format!("db_query row: {}", e))?);
        }
        Ok(result)
    }

    /// Close a database connection by handle.
    pub fn close(&mut self, handle: i64) -> Result<(), String> {
        self.connections
            .remove(&handle)
            .ok_or_else(|| format!("db_close: invalid handle {}", handle))?;
        // Connection is dropped here, which closes the SQLite connection.
        Ok(())
    }

    /// Check if a handle is valid (for testing).
    pub fn is_open(&self, handle: i64) -> bool {
        self.connections.contains_key(&handle)
    }

    /// Begin a transaction on the given connection.
    pub fn begin(&self, handle: i64) -> Result<(), String> {
        let conn = self
            .connections
            .get(&handle)
            .ok_or_else(|| format!("db_begin: invalid handle {}", handle))?;
        conn.execute_batch("BEGIN TRANSACTION")
            .map_err(|e| format!("db_begin: {}", e))
    }

    /// Commit the current transaction on the given connection.
    pub fn commit(&self, handle: i64) -> Result<(), String> {
        let conn = self
            .connections
            .get(&handle)
            .ok_or_else(|| format!("db_commit: invalid handle {}", handle))?;
        conn.execute_batch("COMMIT")
            .map_err(|e| format!("db_commit: {}", e))
    }

    /// Rollback the current transaction on the given connection.
    pub fn rollback(&self, handle: i64) -> Result<(), String> {
        let conn = self
            .connections
            .get(&handle)
            .ok_or_else(|| format!("db_rollback: invalid handle {}", handle))?;
        conn.execute_batch("ROLLBACK")
            .map_err(|e| format!("db_rollback: {}", e))
    }
}

/// Parameter types for SQL binding (positional `?1`, `?2`, ...).
#[derive(Debug, Clone)]
pub enum DbParam {
    /// 64-bit integer parameter.
    Int(i64),
    /// 64-bit float parameter.
    Float(f64),
    /// UTF-8 text parameter.
    Text(String),
    /// SQL NULL parameter.
    Null,
}

/// Return value types from SQLite query results.
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    /// 64-bit integer value.
    Int(i64),
    /// 64-bit float value.
    Float(f64),
    /// UTF-8 text value.
    Text(String),
    /// SQL NULL value.
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_open_close_memory() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        assert!(mgr.is_open(h));
        mgr.close(h).unwrap();
        assert!(!mgr.is_open(h));
    }

    #[test]
    fn db_close_invalid_handle() {
        let mut mgr = DbManager::new();
        let err = mgr.close(9999).unwrap_err();
        assert!(err.contains("invalid handle"));
    }

    #[test]
    fn db_create_table_and_insert() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(
            h,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
            &[],
        )
        .unwrap();
        let changed = mgr
            .execute(
                h,
                "INSERT INTO users (name, age) VALUES (?1, ?2)",
                &[DbParam::Text("Fajar".into()), DbParam::Int(30)],
            )
            .unwrap();
        assert_eq!(changed, 1);
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_query_returns_rows() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(
            h,
            "CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT)",
            &[],
        )
        .unwrap();
        mgr.execute(
            h,
            "INSERT INTO items (label) VALUES (?1)",
            &[DbParam::Text("alpha".into())],
        )
        .unwrap();
        mgr.execute(
            h,
            "INSERT INTO items (label) VALUES (?1)",
            &[DbParam::Text("beta".into())],
        )
        .unwrap();

        let rows = mgr
            .query(h, "SELECT id, label FROM items ORDER BY id", &[])
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("label"), Some(&DbValue::Text("alpha".into())));
        assert_eq!(rows[1].get("label"), Some(&DbValue::Text("beta".into())));
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_parameterized_query() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(h, "CREATE TABLE kv (key TEXT, val REAL)", &[])
            .unwrap();
        mgr.execute(
            h,
            "INSERT INTO kv VALUES (?1, ?2)",
            &[DbParam::Text("pi".into()), DbParam::Float(1.25)],
        )
        .unwrap();
        mgr.execute(
            h,
            "INSERT INTO kv VALUES (?1, ?2)",
            &[DbParam::Text("e".into()), DbParam::Float(2.72)],
        )
        .unwrap();

        let rows = mgr
            .query(
                h,
                "SELECT val FROM kv WHERE key = ?1",
                &[DbParam::Text("pi".into())],
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("val"), Some(&DbValue::Float(1.25)));
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_invalid_sql_returns_error() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        let err = mgr.execute(h, "NOT VALID SQL", &[]).unwrap_err();
        assert!(err.contains("db_execute:"));
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_query_on_invalid_handle() {
        let mgr = DbManager::new();
        let err = mgr.query(999, "SELECT 1", &[]).unwrap_err();
        assert!(err.contains("invalid handle"));
    }

    #[test]
    fn db_null_param_and_value() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(h, "CREATE TABLE t (a TEXT)", &[]).unwrap();
        mgr.execute(h, "INSERT INTO t VALUES (?1)", &[DbParam::Null])
            .unwrap();

        let rows = mgr.query(h, "SELECT a FROM t", &[]).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("a"), Some(&DbValue::Null));
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_persistence_with_tempfile() {
        let dir = std::env::temp_dir();
        let path = dir.join("fj_test_db_persist.sqlite");
        let path_str = path.to_string_lossy().to_string();

        // Write
        {
            let mut mgr = DbManager::new();
            let h = mgr.open(&path_str).unwrap();
            mgr.execute(h, "CREATE TABLE persist (val INTEGER)", &[])
                .unwrap();
            mgr.execute(h, "INSERT INTO persist VALUES (42)", &[])
                .unwrap();
            mgr.close(h).unwrap();
        }

        // Reopen and read
        {
            let mut mgr = DbManager::new();
            let h = mgr.open(&path_str).unwrap();
            let rows = mgr.query(h, "SELECT val FROM persist", &[]).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].get("val"), Some(&DbValue::Int(42)));
            mgr.close(h).unwrap();
        }

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Interpreter integration tests (eval_source → db_* builtins)
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn builtin_db_open_close_roundtrip() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "db open/close failed: {:?}", result);
    }

    #[test]
    fn builtin_db_create_insert_query() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
            db_execute(db, "INSERT INTO users (name) VALUES (?1)", ["Fajar"])
            db_execute(db, "INSERT INTO users (name) VALUES (?1)", ["Budi"])
            let rows = db_query(db, "SELECT id, name FROM users ORDER BY id")
            println(len(rows))
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "db CRUD failed: {:?}", result);
        assert_eq!(interp.get_output(), vec!["2"]);
    }

    #[test]
    fn builtin_db_parameterized_query() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "CREATE TABLE kv (key TEXT, val TEXT)")
            db_execute(db, "INSERT INTO kv VALUES (?1, ?2)", ["alpha", "10"])
            db_execute(db, "INSERT INTO kv VALUES (?1, ?2)", ["beta", "20"])
            let rows = db_query(db, "SELECT val FROM kv WHERE key = ?1", ["beta"])
            println(len(rows))
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "parameterized query failed: {:?}", result);
        assert_eq!(interp.get_output(), vec!["1"]);
    }

    #[test]
    fn builtin_db_invalid_sql_error() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "NOT VALID SQL")
        "#,
        );
        assert!(result.is_err(), "invalid SQL should error");
    }

    #[test]
    fn builtin_db_close_invalid_handle_error() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            db_close(99999)
        "#,
        );
        assert!(result.is_err(), "close invalid handle should error");
    }

    #[test]
    fn builtin_db_multiple_tables() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "CREATE TABLE t1 (a INTEGER)")
            db_execute(db, "CREATE TABLE t2 (b TEXT)")
            db_execute(db, "INSERT INTO t1 VALUES (1)")
            db_execute(db, "INSERT INTO t1 VALUES (2)")
            db_execute(db, "INSERT INTO t2 VALUES (?1)", ["hello"])
            let r1 = db_query(db, "SELECT a FROM t1")
            let r2 = db_query(db, "SELECT b FROM t2")
            println(len(r1))
            println(len(r2))
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "multiple tables failed: {:?}", result);
        assert_eq!(interp.get_output(), vec!["2", "1"]);
    }

    // Transaction tests
    #[test]
    fn db_transaction_commit() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(h, "CREATE TABLE t (v INTEGER)", &[]).unwrap();
        mgr.begin(h).unwrap();
        mgr.execute(h, "INSERT INTO t VALUES (1)", &[]).unwrap();
        mgr.execute(h, "INSERT INTO t VALUES (2)", &[]).unwrap();
        mgr.commit(h).unwrap();
        let rows = mgr.query(h, "SELECT v FROM t ORDER BY v", &[]).unwrap();
        assert_eq!(rows.len(), 2);
        mgr.close(h).unwrap();
    }

    #[test]
    fn db_transaction_rollback() {
        let mut mgr = DbManager::new();
        let h = mgr.open(":memory:").unwrap();
        mgr.execute(h, "CREATE TABLE t (v INTEGER)", &[]).unwrap();
        mgr.execute(h, "INSERT INTO t VALUES (1)", &[]).unwrap();
        mgr.begin(h).unwrap();
        mgr.execute(h, "INSERT INTO t VALUES (2)", &[]).unwrap();
        mgr.rollback(h).unwrap();
        let rows = mgr.query(h, "SELECT v FROM t", &[]).unwrap();
        assert_eq!(rows.len(), 1); // Only the pre-transaction row
        mgr.close(h).unwrap();
    }

    #[test]
    fn builtin_db_transaction_commit() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "CREATE TABLE t (v INTEGER)")
            db_begin(db)
            db_execute(db, "INSERT INTO t VALUES (10)")
            db_execute(db, "INSERT INTO t VALUES (20)")
            db_commit(db)
            let rows = db_query(db, "SELECT v FROM t")
            println(len(rows))
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "transaction commit failed: {:?}", result);
        assert_eq!(interp.get_output(), vec!["2"]);
    }

    #[test]
    fn builtin_db_transaction_rollback() {
        let mut interp = crate::interpreter::Interpreter::new_capturing();
        let result = interp.eval_source(
            r#"
            let db = db_open(":memory:")
            db_execute(db, "CREATE TABLE t (v INTEGER)")
            db_execute(db, "INSERT INTO t VALUES (1)")
            db_begin(db)
            db_execute(db, "INSERT INTO t VALUES (2)")
            db_rollback(db)
            let rows = db_query(db, "SELECT v FROM t")
            println(len(rows))
            db_close(db)
        "#,
        );
        assert!(result.is_ok(), "transaction rollback failed: {:?}", result);
        assert_eq!(interp.get_output(), vec!["1"]);
    }
}
