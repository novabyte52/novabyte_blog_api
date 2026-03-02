use super::SurrealDBConnection;

use serde::de::{DeserializeOwned, Error};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::fmt;

use surrealdb::engine::any::{connect, Any};
use surrealdb::error::{Api, Db};
use surrealdb::opt::auth::Database;
use surrealdb::sql::{Array as SqlArray, Object as SqlObject, Strand, Thing, Value as SqlValue};
use surrealdb::{Error as DbError, Surreal};

use tracing::{instrument, trace};

/// A single statement plus optional "debug label" to help you reason about indices.
#[derive(Debug, Clone)]
pub struct DbOp {
    pub label: &'static str,
    pub sql: String,
}

impl DbOp {
    pub fn new(label: &'static str, sql: impl Into<String>) -> Self {
        Self {
            label,
            sql: sql.into(),
        }
    }
}

/// A composable query “program”:
/// - a list of statements (ops)
/// - a single binding map shared by the whole request
#[derive(Debug, Clone, Default)]
pub struct DbProgram {
    ops: Vec<DbOp>,
    binds: BTreeMap<String, SqlValue>,
}

impl DbProgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn op(mut self, op: DbOp) -> Self {
        self.ops.push(op);
        self
    }

    pub fn extend(mut self, other: DbProgram) -> Self {
        self.ops.extend(other.ops);
        for (k, v) in other.binds {
            self.binds.insert(k, v);
        }
        self
    }

    pub fn bind_json(mut self, key: impl Into<String>, value: JsonValue) -> Self {
        self.binds.insert(key.into(), json_to_sql_value(value));
        self
    }

    pub fn bind_value(mut self, key: impl Into<String>, value: SqlValue) -> Self {
        self.binds.insert(key.into(), value);
        self
    }

    pub fn bind_thing(mut self, key: impl Into<String>, thing: Thing) -> Self {
        self.binds.insert(key.into(), SqlValue::Thing(thing));
        self
    }

    pub fn bind<V: Serialize>(mut self, key: impl Into<String>, value: V) -> Result<Self, DbError> {
        let json = serde_json::to_value(value)
            .map_err(|e| DbError::Db(Db::unreachable(format!("Invalid bindings: {e}"))))?;

        self.binds.insert(key.into(), json_to_sql_value(json));
        Ok(self)
    }

    pub fn bind_serde<A: Serialize>(mut self, args: A) -> Result<Self, DbError> {
        let json = serde_json::to_value(args)
            .map_err(|e| DbError::Db(Db::unreachable(format!("Invalid bindings: {e}"))))?;

        let obj = json.as_object().ok_or_else(|| {
            DbError::Db(Db::unreachable(
                "bind_serde expects a serializable object/map",
            ))
        })?;

        for (k, v) in obj {
            self.binds.insert(k.clone(), json_to_sql_value(v.clone()));
        }
        Ok(self)
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    pub fn labels(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.ops.iter().map(|o| o.label)
    }
}

impl fmt::Display for DbProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 1) Bindings -> LET statements (stable order for diffing)
        let mut keys: Vec<_> = self.binds.keys().collect();
        keys.sort();

        if !keys.is_empty() {
            writeln!(f, "-- bindings")?;
            for k in keys {
                let v = self.binds.get(k).unwrap();
                writeln!(f, "LET ${} = {};", k, sql_literal(v))?;
            }
            writeln!(f)?;
        }

        // 2) Ops -> labeled blocks
        for (i, op) in self.ops.iter().enumerate() {
            writeln!(f, "-- [{:02}] {}", i, op.label)?;
            writeln!(f, "{}", ensure_trailing_semicolon(op.sql.trim()))?;
            writeln!(f)?;
        }

        Ok(())
    }
}

fn json_to_sql_value(v: JsonValue) -> SqlValue {
    match v {
        JsonValue::Null => SqlValue::None,
        JsonValue::Bool(b) => SqlValue::Bool(b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::from(i)
            } else if let Some(u) = n.as_u64() {
                SqlValue::from(u)
            } else if let Some(f) = n.as_f64() {
                SqlValue::from(f)
            } else {
                SqlValue::None
            }
        }
        JsonValue::String(s) => SqlValue::Strand(Strand::from(s)),
        JsonValue::Array(arr) => {
            let items = arr.into_iter().map(json_to_sql_value).collect::<Vec<_>>();
            SqlValue::Array(SqlArray::from(items))
        }
        JsonValue::Object(map) => {
            // ✅ IMPORTANT: detect Thing-like objects and convert to SqlValue::Thing
            // Thing serializes like: {"tb":"person","id":{"String":"..."}}
            let candidate = JsonValue::Object(map.clone());
            if let Ok(thing) = serde_json::from_value::<Thing>(candidate) {
                return SqlValue::Thing(thing);
            }

            // fallback: regular object
            let mut obj = SqlObject::default();
            for (k, v) in map {
                obj.insert(k, json_to_sql_value(v));
            }
            SqlValue::Object(obj)
        }
    }
}

fn sql_literal(v: &SqlValue) -> String {
    match v {
        SqlValue::None => "NONE".to_string(),
        SqlValue::Null => "NONE".to_string(), // keep Surrealist-friendly
        SqlValue::Bool(b) => b.to_string(),
        SqlValue::Number(n) => n.to_string(),
        SqlValue::Strand(s) => format!("{:?}", s.as_str()),
        SqlValue::Thing(t) => t.to_string(), // prints like person:01...
        // For objects/arrays and anything else, fallback to Surreal's canonical string
        other => other.to_string(),
    }
}

/// Ensure the SQL ends with a trailing semicolon.
/// If it already ends with ';' (after trimming), leave it alone.
fn ensure_trailing_semicolon(sql: &str) -> String {
    let s = sql.trim_end();
    if s.is_empty() {
        return ";".to_string();
    }
    if s.ends_with(';') {
        s.to_string()
    } else {
        format!("{};", s)
    }
}

/// Wraps a SurrealDB response and provides typed extraction.
#[derive(Debug)]
pub struct NovaResponse {
    inner: surrealdb::Response,
}

impl NovaResponse {
    /// Take an Optional single result from statement `idx`.
    /// Works with `usize` selector in SurrealDB 2.0.4.
    pub fn take_opt<T: DeserializeOwned>(&mut self, idx: usize) -> Result<Option<T>, DbError> {
        self.inner.take::<Option<T>>(idx)
    }

    /// Take a Vec result from statement `idx`.
    /// Works with `usize` selector in SurrealDB 2.0.4.
    pub fn take_vec<T: DeserializeOwned>(&mut self, idx: usize) -> Result<Vec<T>, DbError> {
        self.inner.take::<Vec<T>>(idx)
    }

    /// Take a single result (expects exactly one), otherwise errors.
    /// This is your ergonomic replacement for `take::<T>(idx)`.
    pub fn take_one<T: DeserializeOwned>(&mut self, idx: usize) -> Result<T, DbError> {
        match self.inner.take::<Option<T>>(idx)? {
            Some(v) => Ok(v),
            None => Err(DbError::Db(Db::unreachable(format!(
                "Expected a result at index {idx}, got NONE"
            )))),
        }
    }

    /// If you sometimes RETURN arrays and want the first item.
    pub fn take_first<T: DeserializeOwned>(&mut self, idx: usize) -> Result<T, DbError> {
        let v = self.inner.take::<Vec<T>>(idx)?;
        v.into_iter().next().ok_or_else(|| {
            DbError::Db(Db::unreachable(format!(
                "Expected non-empty array at index {idx}, got empty"
            )))
        })
    }

    pub fn into_inner(self) -> surrealdb::Response {
        self.inner
    }
}

#[derive(Debug, Clone)]
pub struct NovaDB {
    db: Surreal<Any>,
}

impl NovaDB {
    #[instrument]
    pub async fn new(conn: &SurrealDBConnection) -> Result<Self, DbError> {
        let SurrealDBConnection {
            address,
            username,
            password,
            namespace,
            database,
        } = conn;

        let db = connect(address).await?;
        db.signin(Database {
            username,
            password,
            namespace,
            database,
        })
        .await?;

        Ok(Self { db })
    }

    pub fn executor(&self) -> DbExecutor<'_> {
        DbExecutor { db: self }
    }
}

/// Executes DbPrograms as one Surreal request (optionally wrapped in a transaction).
pub struct DbExecutor<'db> {
    db: &'db NovaDB,
}

impl<'db> DbExecutor<'db> {
    /// Execute as a plain batch (single request).
    #[instrument(skip(self, program))]
    pub async fn run(&self, program: DbProgram) -> Result<NovaResponse, DbError> {
        self.run_inner(program, false).await
    }

    /// Execute wrapped as:
    /// BEGIN TRANSACTION;
    ///   ...program ops...
    /// COMMIT TRANSACTION;
    ///
    /// All in one request (the only supported manual transaction style).
    #[instrument(skip(self, program))]
    pub async fn run_tx(&self, program: DbProgram) -> Result<NovaResponse, DbError> {
        self.run_inner(program, true).await
    }

    async fn run_inner(
        &self,
        program: DbProgram,
        transactional: bool,
    ) -> Result<NovaResponse, DbError> {
        if program.is_empty() {
            return Err(DbError::Api(Api::missing_field("DbProgram has 0 ops")));
        }

        trace!("running program => {}", &program);

        let mut q = if transactional {
            self.db.db.query("BEGIN TRANSACTION;")
        } else {
            let first = &program.ops[0].sql;
            self.db.db.query(first)
        };

        let start_idx = if transactional { 0 } else { 1 };

        // When transactional, we add ops after BEGIN.
        // When non-transactional, the first op is already set as the base query.
        for (i, op) in program.ops.iter().enumerate() {
            if !transactional && i == 0 {
                continue;
            }
            q = q.query(op.sql.clone());
        }

        if transactional {
            q = q.query("COMMIT TRANSACTION;");
        }

        if !program.binds.is_empty() {
            let binds = program.binds.clone();
            q = q.bind(binds);
        }

        trace!(
            "Executing DbProgram: transactional={}, ops={}, labels={:?}",
            transactional,
            program.op_count(),
            program.labels().collect::<Vec<_>>()
        );

        let resp = q.await?;

        // NOTE:
        // Surreal can return Ok(Response) even when internal statements errored,
        // and those errors surface on `take(idx)`.
        //
        // Indices:
        // - if transactional: idx 0 = BEGIN, then ops map to idx 1..=N, idx N+1 = COMMIT
        // - if not transactional: ops map to idx 0..=N-1
        //
        // We don’t auto-check here because sometimes you WANT partial inspection.
        // Your services can enforce with `take()` calls (fail fast).
        let _ = start_idx;

        Ok(NovaResponse { inner: resp })
    }
}
