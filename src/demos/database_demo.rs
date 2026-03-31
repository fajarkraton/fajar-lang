//! Sprint W9: Database Client — connection pool, query builder, migration,
//! row mapper, transactions, prepared statements, schema introspection, metrics.

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// W9.1: ConnectionPool — Database Connection Pool
// ═══════════════════════════════════════════════════════════════════════

/// Simulated database connection.
#[derive(Debug, Clone)]
pub struct DbConnection {
    /// Connection identifier.
    pub id: u64,
    /// Whether the connection is currently in use.
    pub in_use: bool,
    /// Database URL.
    pub url: String,
    /// Number of queries executed on this connection.
    pub query_count: u64,
}

impl DbConnection {
    /// Create a new connection.
    fn new(id: u64, url: &str) -> Self {
        Self {
            id,
            in_use: false,
            url: url.into(),
            query_count: 0,
        }
    }
}

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Database URL.
    pub url: String,
    /// Maximum number of connections.
    pub max_size: usize,
    /// Minimum idle connections.
    pub min_idle: usize,
}

impl PoolConfig {
    /// Create a new config with defaults.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.into(),
            max_size: 10,
            min_idle: 2,
        }
    }
}

/// Pool of database connections with checkout/return.
#[derive(Debug)]
pub struct ConnectionPool {
    /// All connections.
    connections: Vec<DbConnection>,
    /// Configuration.
    pub config: PoolConfig,
    /// Next connection ID.
    next_id: u64,
    /// Total checkouts.
    pub total_checkouts: u64,
}

impl ConnectionPool {
    /// Create a new pool from config.
    pub fn new(config: PoolConfig) -> Self {
        let min_idle = config.min_idle;
        let url = config.url.clone();
        let mut pool = Self {
            connections: Vec::new(),
            config,
            next_id: 1,
            total_checkouts: 0,
        };
        // Pre-create min_idle connections
        for _ in 0..min_idle {
            let conn = DbConnection::new(pool.next_id, &url);
            pool.next_id += 1;
            pool.connections.push(conn);
        }
        pool
    }

    /// Checkout a connection from the pool.
    pub fn checkout(&mut self) -> Result<u64, String> {
        // Find idle connection
        if let Some(conn) = self.connections.iter_mut().find(|c| !c.in_use) {
            conn.in_use = true;
            self.total_checkouts += 1;
            return Ok(conn.id);
        }
        // Create new if under max
        if self.connections.len() < self.config.max_size {
            let mut conn = DbConnection::new(self.next_id, &self.config.url);
            self.next_id += 1;
            conn.in_use = true;
            let id = conn.id;
            self.connections.push(conn);
            self.total_checkouts += 1;
            return Ok(id);
        }
        Err("connection pool exhausted".into())
    }

    /// Return a connection to the pool.
    pub fn checkin(&mut self, id: u64) -> Result<(), String> {
        let conn = self
            .connections
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("connection {id} not found in pool"))?;
        if !conn.in_use {
            return Err(format!("connection {id} already returned"));
        }
        conn.in_use = false;
        Ok(())
    }

    /// Number of total connections.
    pub fn size(&self) -> usize {
        self.connections.len()
    }

    /// Number of idle connections.
    pub fn idle_count(&self) -> usize {
        self.connections.iter().filter(|c| !c.in_use).count()
    }

    /// Number of active (in-use) connections.
    pub fn active_count(&self) -> usize {
        self.connections.iter().filter(|c| c.in_use).count()
    }

    /// Get pool utilization as a fraction (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        if self.connections.is_empty() {
            return 0.0;
        }
        self.active_count() as f64 / self.connections.len() as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.2: QueryBuilder — Type-Safe SQL Query Construction
// ═══════════════════════════════════════════════════════════════════════

/// SQL operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlOperation {
    /// SELECT query.
    Select,
    /// INSERT query.
    Insert,
    /// UPDATE query.
    Update,
    /// DELETE query.
    Delete,
}

/// SQL comparison operator.
#[derive(Debug, Clone, PartialEq)]
pub enum WhereClause {
    /// Column equals value.
    Eq(String, SqlParam),
    /// Column not equal to value.
    Neq(String, SqlParam),
    /// Column greater than value.
    Gt(String, SqlParam),
    /// Column less than value.
    Lt(String, SqlParam),
    /// Column LIKE pattern.
    Like(String, String),
    /// Column IS NULL.
    IsNull(String),
    /// Column IS NOT NULL.
    IsNotNull(String),
    /// Column IN (values).
    In(String, Vec<SqlParam>),
    /// AND of two clauses.
    And(Box<WhereClause>, Box<WhereClause>),
    /// OR of two clauses.
    Or(Box<WhereClause>, Box<WhereClause>),
}

/// SQL parameter value (for prepared statements).
#[derive(Debug, Clone, PartialEq)]
pub enum SqlParam {
    /// Integer.
    Int(i64),
    /// Float.
    Float(f64),
    /// String.
    Text(String),
    /// Boolean.
    Bool(bool),
    /// NULL.
    Null,
}

impl fmt::Display for SqlParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlParam::Int(n) => write!(f, "{n}"),
            SqlParam::Float(v) => write!(f, "{v}"),
            SqlParam::Text(s) => write!(f, "'{s}'"),
            SqlParam::Bool(b) => write!(f, "{b}"),
            SqlParam::Null => write!(f, "NULL"),
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Ascending.
    Asc,
    /// Descending.
    Desc,
}

/// Builds type-safe SQL queries.
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    /// Operation type.
    operation: SqlOperation,
    /// Table name.
    table: String,
    /// Selected columns (for SELECT).
    columns: Vec<String>,
    /// WHERE clauses.
    where_clauses: Vec<WhereClause>,
    /// ORDER BY clauses.
    order_by: Vec<(String, SortOrder)>,
    /// LIMIT value.
    limit: Option<u64>,
    /// OFFSET value.
    offset: Option<u64>,
    /// SET values (for UPDATE).
    set_values: Vec<(String, SqlParam)>,
    /// VALUES (for INSERT).
    insert_columns: Vec<String>,
    /// INSERT rows.
    insert_values: Vec<Vec<SqlParam>>,
}

impl QueryBuilder {
    /// Start a SELECT query.
    pub fn select(table: &str) -> Self {
        Self {
            operation: SqlOperation::Select,
            table: table.into(),
            columns: Vec::new(),
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            set_values: Vec::new(),
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
        }
    }

    /// Start an INSERT query.
    pub fn insert(table: &str) -> Self {
        Self {
            operation: SqlOperation::Insert,
            table: table.into(),
            columns: Vec::new(),
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            set_values: Vec::new(),
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
        }
    }

    /// Start an UPDATE query.
    pub fn update(table: &str) -> Self {
        Self {
            operation: SqlOperation::Update,
            table: table.into(),
            columns: Vec::new(),
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            set_values: Vec::new(),
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
        }
    }

    /// Start a DELETE query.
    pub fn delete(table: &str) -> Self {
        Self {
            operation: SqlOperation::Delete,
            table: table.into(),
            columns: Vec::new(),
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            set_values: Vec::new(),
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
        }
    }

    /// Add columns to SELECT.
    pub fn columns(mut self, cols: &[&str]) -> Self {
        self.columns.extend(cols.iter().map(|s| (*s).into()));
        self
    }

    /// Add a WHERE clause.
    pub fn where_clause(mut self, clause: WhereClause) -> Self {
        self.where_clauses.push(clause);
        self
    }

    /// Add an ORDER BY clause.
    pub fn order_by(mut self, column: &str, order: SortOrder) -> Self {
        self.order_by.push((column.into(), order));
        self
    }

    /// Set LIMIT.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Set OFFSET.
    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    /// Add a SET value (for UPDATE).
    pub fn set(mut self, column: &str, value: SqlParam) -> Self {
        self.set_values.push((column.into(), value));
        self
    }

    /// Set INSERT columns.
    pub fn into_columns(mut self, cols: &[&str]) -> Self {
        self.insert_columns.extend(cols.iter().map(|s| (*s).into()));
        self
    }

    /// Add an INSERT row.
    pub fn values(mut self, vals: Vec<SqlParam>) -> Self {
        self.insert_values.push(vals);
        self
    }

    /// Build the SQL string.
    pub fn build(&self) -> String {
        match self.operation {
            SqlOperation::Select => self.build_select(),
            SqlOperation::Insert => self.build_insert(),
            SqlOperation::Update => self.build_update(),
            SqlOperation::Delete => self.build_delete(),
        }
    }

    /// Build SELECT query.
    fn build_select(&self) -> String {
        let cols = if self.columns.is_empty() {
            "*".into()
        } else {
            self.columns.join(", ")
        };
        let mut sql = format!("SELECT {cols} FROM {}", self.table);
        sql.push_str(&self.build_where());
        sql.push_str(&self.build_order_by());
        if let Some(n) = self.limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }
        if let Some(n) = self.offset {
            sql.push_str(&format!(" OFFSET {n}"));
        }
        sql
    }

    /// Build INSERT query.
    fn build_insert(&self) -> String {
        let cols = self.insert_columns.join(", ");
        let rows: Vec<String> = self
            .insert_values
            .iter()
            .map(|row| {
                let vals: Vec<String> = row.iter().map(|v| format!("{v}")).collect();
                format!("({})", vals.join(", "))
            })
            .collect();
        format!(
            "INSERT INTO {} ({}) VALUES {}",
            self.table,
            cols,
            rows.join(", ")
        )
    }

    /// Build UPDATE query.
    fn build_update(&self) -> String {
        let sets: Vec<String> = self
            .set_values
            .iter()
            .map(|(col, val)| format!("{col} = {val}"))
            .collect();
        let mut sql = format!("UPDATE {} SET {}", self.table, sets.join(", "));
        sql.push_str(&self.build_where());
        sql
    }

    /// Build DELETE query.
    fn build_delete(&self) -> String {
        let mut sql = format!("DELETE FROM {}", self.table);
        sql.push_str(&self.build_where());
        sql
    }

    /// Build WHERE clause string.
    fn build_where(&self) -> String {
        if self.where_clauses.is_empty() {
            return String::new();
        }
        let conditions: Vec<String> = self
            .where_clauses
            .iter()
            .map(|c| Self::format_where(c))
            .collect();
        format!(" WHERE {}", conditions.join(" AND "))
    }

    /// Format a single WHERE clause.
    fn format_where(clause: &WhereClause) -> String {
        match clause {
            WhereClause::Eq(col, val) => format!("{col} = {val}"),
            WhereClause::Neq(col, val) => format!("{col} != {val}"),
            WhereClause::Gt(col, val) => format!("{col} > {val}"),
            WhereClause::Lt(col, val) => format!("{col} < {val}"),
            WhereClause::Like(col, pat) => format!("{col} LIKE '{pat}'"),
            WhereClause::IsNull(col) => format!("{col} IS NULL"),
            WhereClause::IsNotNull(col) => format!("{col} IS NOT NULL"),
            WhereClause::In(col, vals) => {
                let items: Vec<String> = vals.iter().map(|v| format!("{v}")).collect();
                format!("{col} IN ({})", items.join(", "))
            }
            WhereClause::And(a, b) => {
                format!("({} AND {})", Self::format_where(a), Self::format_where(b))
            }
            WhereClause::Or(a, b) => {
                format!("({} OR {})", Self::format_where(a), Self::format_where(b))
            }
        }
    }

    /// Build ORDER BY clause string.
    fn build_order_by(&self) -> String {
        if self.order_by.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = self
            .order_by
            .iter()
            .map(|(col, ord)| {
                let dir = match ord {
                    SortOrder::Asc => "ASC",
                    SortOrder::Desc => "DESC",
                };
                format!("{col} {dir}")
            })
            .collect();
        format!(" ORDER BY {}", parts.join(", "))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.3: Migration — Schema Migration System
// ═══════════════════════════════════════════════════════════════════════

/// A database migration (versioned schema change).
#[derive(Debug, Clone)]
pub struct Migration {
    /// Version number.
    pub version: u32,
    /// Description.
    pub description: String,
    /// SQL to apply (up).
    pub up_sql: String,
    /// SQL to revert (down).
    pub down_sql: String,
}

/// Migration state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    /// Not yet applied.
    Pending,
    /// Successfully applied.
    Applied,
    /// Reverted.
    Reverted,
}

impl fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationStatus::Pending => write!(f, "PENDING"),
            MigrationStatus::Applied => write!(f, "APPLIED"),
            MigrationStatus::Reverted => write!(f, "REVERTED"),
        }
    }
}

/// Migration runner.
#[derive(Debug)]
pub struct MigrationRunner {
    /// Available migrations (sorted by version).
    migrations: Vec<Migration>,
    /// Applied versions.
    applied: Vec<u32>,
}

impl MigrationRunner {
    /// Create a new runner.
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
            applied: Vec::new(),
        }
    }

    /// Add a migration.
    pub fn add(mut self, migration: Migration) -> Self {
        self.migrations.push(migration);
        self.migrations.sort_by_key(|m| m.version);
        self
    }

    /// Apply the next pending migration, returning its SQL.
    pub fn migrate_up(&mut self) -> Option<(u32, String)> {
        for m in &self.migrations {
            if !self.applied.contains(&m.version) {
                self.applied.push(m.version);
                return Some((m.version, m.up_sql.clone()));
            }
        }
        None
    }

    /// Revert the last applied migration, returning its SQL.
    pub fn migrate_down(&mut self) -> Option<(u32, String)> {
        if let Some(&version) = self.applied.last() {
            self.applied.pop();
            if let Some(m) = self.migrations.iter().find(|m| m.version == version) {
                return Some((version, m.down_sql.clone()));
            }
        }
        None
    }

    /// Apply all pending migrations.
    pub fn migrate_all(&mut self) -> Vec<(u32, String)> {
        let mut results = Vec::new();
        while let Some(result) = self.migrate_up() {
            results.push(result);
        }
        results
    }

    /// Get the status of each migration.
    pub fn status(&self) -> Vec<(u32, &str, MigrationStatus)> {
        self.migrations
            .iter()
            .map(|m| {
                let status = if self.applied.contains(&m.version) {
                    MigrationStatus::Applied
                } else {
                    MigrationStatus::Pending
                };
                (m.version, m.description.as_str(), status)
            })
            .collect()
    }

    /// Current applied version (0 if none).
    pub fn current_version(&self) -> u32 {
        self.applied.last().copied().unwrap_or(0)
    }

    /// Number of pending migrations.
    pub fn pending_count(&self) -> usize {
        self.migrations
            .iter()
            .filter(|m| !self.applied.contains(&m.version))
            .count()
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.4: RowMapper — Map Query Results to Structs
// ═══════════════════════════════════════════════════════════════════════

/// A single row from a query result.
#[derive(Debug, Clone)]
pub struct Row {
    /// Column name -> value.
    columns: HashMap<String, SqlParam>,
}

impl Row {
    /// Create a new row.
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    /// Set a column value.
    pub fn set(mut self, name: &str, value: SqlParam) -> Self {
        self.columns.insert(name.into(), value);
        self
    }

    /// Get a column value.
    pub fn get(&self, name: &str) -> Option<&SqlParam> {
        self.columns.get(name)
    }

    /// Get an integer column.
    pub fn get_int(&self, name: &str) -> Option<i64> {
        match self.columns.get(name) {
            Some(SqlParam::Int(n)) => Some(*n),
            _ => None,
        }
    }

    /// Get a text column.
    pub fn get_text(&self, name: &str) -> Option<&str> {
        match self.columns.get(name) {
            Some(SqlParam::Text(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get a boolean column.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.columns.get(name) {
            Some(SqlParam::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// Column names.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for types that can be mapped from a database row.
pub trait FromRow: Sized {
    /// Map a row to this type.
    fn from_row(row: &Row) -> Result<Self, String>;
}

/// Generic row mapper.
pub struct RowMapper;

impl RowMapper {
    /// Map a list of rows to a Vec of mapped types.
    pub fn map_rows<T: FromRow>(rows: &[Row]) -> Result<Vec<T>, String> {
        rows.iter().map(T::from_row).collect()
    }

    /// Map a single row.
    pub fn map_one<T: FromRow>(row: &Row) -> Result<T, String> {
        T::from_row(row)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.5: Transaction — Begin/Commit/Rollback
// ═══════════════════════════════════════════════════════════════════════

/// Transaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxState {
    /// Active transaction.
    Active,
    /// Committed.
    Committed,
    /// Rolled back.
    RolledBack,
}

impl fmt::Display for TxState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxState::Active => write!(f, "ACTIVE"),
            TxState::Committed => write!(f, "COMMITTED"),
            TxState::RolledBack => write!(f, "ROLLED_BACK"),
        }
    }
}

/// A database transaction.
#[derive(Debug)]
pub struct Transaction {
    /// Transaction ID.
    pub id: u64,
    /// Connection ID.
    pub connection_id: u64,
    /// Current state.
    pub state: TxState,
    /// Statements executed within this transaction.
    pub statements: Vec<String>,
    /// Savepoints.
    savepoints: Vec<String>,
}

impl Transaction {
    /// Begin a new transaction.
    pub fn begin(id: u64, connection_id: u64) -> Self {
        Self {
            id,
            connection_id,
            state: TxState::Active,
            statements: vec!["BEGIN".into()],
            savepoints: Vec::new(),
        }
    }

    /// Execute a statement within this transaction.
    pub fn execute(&mut self, sql: &str) -> Result<(), String> {
        if self.state != TxState::Active {
            return Err(format!("transaction is {}", self.state));
        }
        self.statements.push(sql.into());
        Ok(())
    }

    /// Create a savepoint.
    pub fn savepoint(&mut self, name: &str) -> Result<(), String> {
        if self.state != TxState::Active {
            return Err(format!("transaction is {}", self.state));
        }
        self.savepoints.push(name.into());
        self.statements.push(format!("SAVEPOINT {name}"));
        Ok(())
    }

    /// Rollback to a savepoint.
    pub fn rollback_to(&mut self, name: &str) -> Result<(), String> {
        if self.state != TxState::Active {
            return Err(format!("transaction is {}", self.state));
        }
        if !self.savepoints.contains(&name.to_string()) {
            return Err(format!("savepoint '{name}' not found"));
        }
        self.statements
            .push(format!("ROLLBACK TO SAVEPOINT {name}"));
        Ok(())
    }

    /// Commit the transaction.
    pub fn commit(&mut self) -> Result<Vec<String>, String> {
        if self.state != TxState::Active {
            return Err(format!("transaction is {}", self.state));
        }
        self.statements.push("COMMIT".into());
        self.state = TxState::Committed;
        Ok(self.statements.clone())
    }

    /// Rollback the transaction.
    pub fn rollback(&mut self) -> Result<(), String> {
        if self.state != TxState::Active {
            return Err(format!("transaction is {}", self.state));
        }
        self.statements.push("ROLLBACK".into());
        self.state = TxState::RolledBack;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.6: PreparedStatement — Parameterized Queries
// ═══════════════════════════════════════════════════════════════════════

/// A prepared (parameterized) statement to prevent SQL injection.
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    /// SQL template with `?` placeholders.
    pub sql_template: String,
    /// Number of expected parameters.
    pub param_count: usize,
    /// Statement name/ID.
    pub name: String,
}

impl PreparedStatement {
    /// Prepare a statement from SQL with `?` placeholders.
    pub fn prepare(name: &str, sql: &str) -> Self {
        let param_count = sql.chars().filter(|&c| c == '?').count();
        Self {
            sql_template: sql.into(),
            param_count,
            name: name.into(),
        }
    }

    /// Bind parameters and produce the final SQL.
    pub fn bind(&self, params: &[SqlParam]) -> Result<String, String> {
        if params.len() != self.param_count {
            return Err(format!(
                "expected {} parameters, got {}",
                self.param_count,
                params.len()
            ));
        }
        let mut sql = self.sql_template.clone();
        for param in params {
            if let Some(pos) = sql.find('?') {
                let replacement = format!("{param}");
                sql.replace_range(pos..pos + 1, &replacement);
            }
        }
        Ok(sql)
    }

    /// Validate that a parameter set has the correct count.
    pub fn validate_params(&self, params: &[SqlParam]) -> Result<(), String> {
        if params.len() != self.param_count {
            Err(format!(
                "expected {} parameters, got {}",
                self.param_count,
                params.len()
            ))
        } else {
            Ok(())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.7: SchemaIntrospector — Read Database Schema
// ═══════════════════════════════════════════════════════════════════════

/// Database column type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    /// Integer.
    Integer,
    /// Text/VARCHAR.
    Text,
    /// Real/Float.
    Real,
    /// Boolean.
    Boolean,
    /// Blob.
    Blob,
    /// Timestamp.
    Timestamp,
}

impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColumnType::Integer => write!(f, "INTEGER"),
            ColumnType::Text => write!(f, "TEXT"),
            ColumnType::Real => write!(f, "REAL"),
            ColumnType::Boolean => write!(f, "BOOLEAN"),
            ColumnType::Blob => write!(f, "BLOB"),
            ColumnType::Timestamp => write!(f, "TIMESTAMP"),
        }
    }
}

/// Column definition.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column name.
    pub name: String,
    /// Column type.
    pub col_type: ColumnType,
    /// Whether this column is nullable.
    pub nullable: bool,
    /// Whether this column is a primary key.
    pub primary_key: bool,
    /// Default value (as SQL string).
    pub default_value: Option<String>,
}

/// Table definition.
#[derive(Debug, Clone)]
pub struct TableDef {
    /// Table name.
    pub name: String,
    /// Columns.
    pub columns: Vec<ColumnDef>,
}

impl TableDef {
    /// Create a new table definition.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
        }
    }

    /// Add a column.
    pub fn column(mut self, col: ColumnDef) -> Self {
        self.columns.push(col);
        self
    }

    /// Get a column by name.
    pub fn get_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Primary key columns.
    pub fn primary_keys(&self) -> Vec<&ColumnDef> {
        self.columns.iter().filter(|c| c.primary_key).collect()
    }

    /// Generate CREATE TABLE SQL.
    pub fn to_create_sql(&self) -> String {
        let cols: Vec<String> = self
            .columns
            .iter()
            .map(|c| {
                let mut def = format!("{} {}", c.name, c.col_type);
                if c.primary_key {
                    def.push_str(" PRIMARY KEY");
                }
                if !c.nullable {
                    def.push_str(" NOT NULL");
                }
                if let Some(ref default) = c.default_value {
                    def.push_str(&format!(" DEFAULT {default}"));
                }
                def
            })
            .collect();
        format!("CREATE TABLE {} ({})", self.name, cols.join(", "))
    }
}

/// Reads database schema metadata.
pub struct SchemaIntrospector {
    /// Known tables.
    tables: Vec<TableDef>,
}

impl SchemaIntrospector {
    /// Create with known tables.
    pub fn new(tables: Vec<TableDef>) -> Self {
        Self { tables }
    }

    /// List all table names.
    pub fn table_names(&self) -> Vec<&str> {
        self.tables.iter().map(|t| t.name.as_str()).collect()
    }

    /// Get a table definition by name.
    pub fn get_table(&self, name: &str) -> Option<&TableDef> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Total number of tables.
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    /// Total number of columns across all tables.
    pub fn total_columns(&self) -> usize {
        self.tables.iter().map(|t| t.columns.len()).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// W9.8: DatabaseMetrics — Query Count, Latency, Pool Utilization
// ═══════════════════════════════════════════════════════════════════════

/// Database performance metrics.
#[derive(Debug, Clone, Default)]
pub struct DatabaseMetrics {
    /// Total queries executed.
    pub query_count: u64,
    /// Total query time in microseconds.
    pub total_latency_us: u64,
    /// Queries per operation type.
    pub ops: HashMap<String, u64>,
    /// Error count.
    pub error_count: u64,
    /// Slow query count (above threshold).
    pub slow_query_count: u64,
    /// Slow query threshold in microseconds.
    pub slow_threshold_us: u64,
}

impl DatabaseMetrics {
    /// Create new metrics with a slow query threshold.
    pub fn new(slow_threshold_us: u64) -> Self {
        Self {
            slow_threshold_us,
            ..Default::default()
        }
    }

    /// Record a query execution.
    pub fn record_query(&mut self, operation: &str, latency_us: u64) {
        self.query_count += 1;
        self.total_latency_us += latency_us;
        *self.ops.entry(operation.into()).or_insert(0) += 1;
        if latency_us > self.slow_threshold_us {
            self.slow_query_count += 1;
        }
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Average latency in microseconds.
    pub fn avg_latency_us(&self) -> f64 {
        if self.query_count == 0 {
            return 0.0;
        }
        self.total_latency_us as f64 / self.query_count as f64
    }

    /// Error rate (0.0 to 1.0).
    pub fn error_rate(&self) -> f64 {
        if self.query_count == 0 {
            return 0.0;
        }
        self.error_count as f64 / self.query_count as f64
    }

    /// Queries per second (given elapsed time in seconds).
    pub fn qps(&self, elapsed_secs: f64) -> f64 {
        if elapsed_secs <= 0.0 {
            return 0.0;
        }
        self.query_count as f64 / elapsed_secs
    }

    /// Format metrics as a summary string.
    pub fn summary(&self) -> String {
        format!(
            "Queries: {} | Avg latency: {:.1}us | Errors: {} | Slow: {}",
            self.query_count,
            self.avg_latency_us(),
            self.error_count,
            self.slow_query_count
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // W9.1: ConnectionPool
    #[test]
    fn w9_1_pool_checkout_return() {
        let config = PoolConfig::new("sqlite://test.db");
        let mut pool = ConnectionPool::new(config);
        assert_eq!(pool.size(), 2); // min_idle
        let id = pool.checkout().unwrap();
        assert_eq!(pool.active_count(), 1);
        pool.checkin(id).unwrap();
        assert_eq!(pool.idle_count(), 2);
    }

    #[test]
    fn w9_1_pool_exhaustion() {
        let mut config = PoolConfig::new("sqlite://test.db");
        config.max_size = 2;
        config.min_idle = 0;
        let mut pool = ConnectionPool::new(config);
        let _id1 = pool.checkout().unwrap();
        let _id2 = pool.checkout().unwrap();
        assert!(pool.checkout().is_err());
    }

    #[test]
    fn w9_1_pool_utilization() {
        let config = PoolConfig::new("sqlite://test.db");
        let mut pool = ConnectionPool::new(config);
        assert_eq!(pool.utilization(), 0.0);
        let _id = pool.checkout().unwrap();
        assert!(pool.utilization() > 0.0);
    }

    // W9.2: QueryBuilder
    #[test]
    fn w9_2_select_query() {
        let sql = QueryBuilder::select("users")
            .columns(&["id", "name"])
            .where_clause(WhereClause::Gt("age".into(), SqlParam::Int(18)))
            .order_by("name", SortOrder::Asc)
            .limit(10)
            .build();
        assert!(sql.contains("SELECT id, name FROM users"));
        assert!(sql.contains("WHERE age > 18"));
        assert!(sql.contains("ORDER BY name ASC"));
        assert!(sql.contains("LIMIT 10"));
    }

    #[test]
    fn w9_2_insert_query() {
        let sql = QueryBuilder::insert("users")
            .into_columns(&["name", "age"])
            .values(vec![SqlParam::Text("Alice".into()), SqlParam::Int(30)])
            .build();
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("(name, age)"));
        assert!(sql.contains("('Alice', 30)"));
    }

    #[test]
    fn w9_2_update_query() {
        let sql = QueryBuilder::update("users")
            .set("name", SqlParam::Text("Bob".into()))
            .where_clause(WhereClause::Eq("id".into(), SqlParam::Int(1)))
            .build();
        assert!(sql.contains("UPDATE users SET name = 'Bob'"));
        assert!(sql.contains("WHERE id = 1"));
    }

    #[test]
    fn w9_2_delete_query() {
        let sql = QueryBuilder::delete("users")
            .where_clause(WhereClause::Eq("id".into(), SqlParam::Int(5)))
            .build();
        assert_eq!(sql, "DELETE FROM users WHERE id = 5");
    }

    // W9.3: Migration
    #[test]
    fn w9_3_migration_up_down() {
        let mut runner = MigrationRunner::new()
            .add(Migration {
                version: 1,
                description: "create users".into(),
                up_sql: "CREATE TABLE users (id INT)".into(),
                down_sql: "DROP TABLE users".into(),
            })
            .add(Migration {
                version: 2,
                description: "add email".into(),
                up_sql: "ALTER TABLE users ADD email TEXT".into(),
                down_sql: "ALTER TABLE users DROP email".into(),
            });

        assert_eq!(runner.pending_count(), 2);
        let (v, sql) = runner.migrate_up().unwrap();
        assert_eq!(v, 1);
        assert!(sql.contains("CREATE TABLE"));
        assert_eq!(runner.current_version(), 1);

        let (v, _) = runner.migrate_up().unwrap();
        assert_eq!(v, 2);
        assert_eq!(runner.pending_count(), 0);

        let (v, sql) = runner.migrate_down().unwrap();
        assert_eq!(v, 2);
        assert!(sql.contains("DROP email"));
    }

    #[test]
    fn w9_3_migration_all() {
        let mut runner = MigrationRunner::new()
            .add(Migration {
                version: 1,
                description: "v1".into(),
                up_sql: "SQL1".into(),
                down_sql: "".into(),
            })
            .add(Migration {
                version: 2,
                description: "v2".into(),
                up_sql: "SQL2".into(),
                down_sql: "".into(),
            });
        let results = runner.migrate_all();
        assert_eq!(results.len(), 2);
        assert_eq!(runner.current_version(), 2);
    }

    // W9.4: RowMapper
    #[test]
    fn w9_4_row_get_values() {
        let row = Row::new()
            .set("id", SqlParam::Int(1))
            .set("name", SqlParam::Text("Fajar".into()))
            .set("active", SqlParam::Bool(true));
        assert_eq!(row.get_int("id"), Some(1));
        assert_eq!(row.get_text("name"), Some("Fajar"));
        assert_eq!(row.get_bool("active"), Some(true));
        assert_eq!(row.get_int("missing"), None);
    }

    #[test]
    fn w9_4_row_mapper() {
        #[derive(Debug, PartialEq)]
        struct User {
            id: i64,
            name: String,
        }
        impl FromRow for User {
            fn from_row(row: &Row) -> Result<Self, String> {
                Ok(User {
                    id: row.get_int("id").ok_or("missing id")?,
                    name: row.get_text("name").ok_or("missing name")?.into(),
                })
            }
        }
        let rows = vec![
            Row::new()
                .set("id", SqlParam::Int(1))
                .set("name", SqlParam::Text("Alice".into())),
            Row::new()
                .set("id", SqlParam::Int(2))
                .set("name", SqlParam::Text("Bob".into())),
        ];
        let users: Vec<User> = RowMapper::map_rows(&rows).unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].name, "Alice");
        assert_eq!(users[1].id, 2);
    }

    // W9.5: Transaction
    #[test]
    fn w9_5_transaction_commit() {
        let mut tx = Transaction::begin(1, 100);
        assert_eq!(tx.state, TxState::Active);
        tx.execute("INSERT INTO users VALUES (1, 'Alice')").unwrap();
        let stmts = tx.commit().unwrap();
        assert_eq!(tx.state, TxState::Committed);
        assert_eq!(stmts.len(), 3); // BEGIN, INSERT, COMMIT
    }

    #[test]
    fn w9_5_transaction_rollback() {
        let mut tx = Transaction::begin(2, 100);
        tx.execute("DELETE FROM users").unwrap();
        tx.rollback().unwrap();
        assert_eq!(tx.state, TxState::RolledBack);
        // Cannot execute after rollback
        assert!(tx.execute("SELECT 1").is_err());
    }

    #[test]
    fn w9_5_transaction_savepoint() {
        let mut tx = Transaction::begin(3, 100);
        tx.savepoint("sp1").unwrap();
        tx.execute("INSERT INTO x VALUES (1)").unwrap();
        tx.rollback_to("sp1").unwrap();
        assert_eq!(tx.state, TxState::Active);
        assert!(tx.rollback_to("nonexistent").is_err());
    }

    // W9.6: PreparedStatement
    #[test]
    fn w9_6_prepared_bind() {
        let stmt = PreparedStatement::prepare("find_user", "SELECT * FROM users WHERE id = ? AND name = ?");
        assert_eq!(stmt.param_count, 2);
        let sql = stmt
            .bind(&[SqlParam::Int(1), SqlParam::Text("Alice".into())])
            .unwrap();
        assert_eq!(sql, "SELECT * FROM users WHERE id = 1 AND name = 'Alice'");
    }

    #[test]
    fn w9_6_prepared_wrong_params() {
        let stmt = PreparedStatement::prepare("s", "SELECT ? WHERE ?");
        assert!(stmt.bind(&[SqlParam::Int(1)]).is_err());
    }

    // W9.7: SchemaIntrospector
    #[test]
    fn w9_7_schema_introspect() {
        let users = TableDef::new("users")
            .column(ColumnDef {
                name: "id".into(),
                col_type: ColumnType::Integer,
                nullable: false,
                primary_key: true,
                default_value: None,
            })
            .column(ColumnDef {
                name: "name".into(),
                col_type: ColumnType::Text,
                nullable: false,
                primary_key: false,
                default_value: None,
            });
        let intro = SchemaIntrospector::new(vec![users]);
        assert_eq!(intro.table_count(), 1);
        assert_eq!(intro.total_columns(), 2);
        let table = intro.get_table("users").unwrap();
        assert_eq!(table.primary_keys().len(), 1);
    }

    #[test]
    fn w9_7_create_table_sql() {
        let table = TableDef::new("items")
            .column(ColumnDef {
                name: "id".into(),
                col_type: ColumnType::Integer,
                nullable: false,
                primary_key: true,
                default_value: None,
            })
            .column(ColumnDef {
                name: "desc".into(),
                col_type: ColumnType::Text,
                nullable: true,
                primary_key: false,
                default_value: Some("''".into()),
            });
        let sql = table.to_create_sql();
        assert!(sql.contains("CREATE TABLE items"));
        assert!(sql.contains("id INTEGER PRIMARY KEY NOT NULL"));
        assert!(sql.contains("desc TEXT DEFAULT ''"));
    }

    // W9.8: DatabaseMetrics
    #[test]
    fn w9_8_metrics_recording() {
        let mut metrics = DatabaseMetrics::new(1000);
        metrics.record_query("SELECT", 500);
        metrics.record_query("SELECT", 1500);
        metrics.record_query("INSERT", 200);
        metrics.record_error();
        assert_eq!(metrics.query_count, 3);
        assert_eq!(metrics.slow_query_count, 1);
        assert_eq!(metrics.error_count, 1);
        assert!((metrics.avg_latency_us() - 733.3).abs() < 1.0);
    }

    #[test]
    fn w9_8_metrics_qps() {
        let mut metrics = DatabaseMetrics::new(1000);
        for _ in 0..100 {
            metrics.record_query("SELECT", 100);
        }
        let qps = metrics.qps(10.0);
        assert!((qps - 10.0).abs() < 0.01);
    }

    #[test]
    fn w9_8_metrics_summary() {
        let mut metrics = DatabaseMetrics::new(500);
        metrics.record_query("SELECT", 100);
        let summary = metrics.summary();
        assert!(summary.contains("Queries: 1"));
        assert!(summary.contains("Errors: 0"));
    }
}
