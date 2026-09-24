use std::collections::BTreeMap;

use super::SurrealDBConnection;

use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

use surrealdb::engine::any::{connect, Any};
use surrealdb::method::Transaction;
use surrealdb::opt::auth::Database;
use surrealdb::types::{Array, Object, SurrealValue, Value as SurrealVal};
use surrealdb::{Error as DbError, IndexedResults, Surreal};

use tracing::instrument;

/// Wraps a SurrealDB [`IndexedResults`] response and provides typed extraction helpers.
#[derive(Debug)]
pub struct NovaResponse {
    inner: IndexedResults,
}

impl From<IndexedResults> for NovaResponse {
    fn from(resp: IndexedResults) -> Self {
        Self { inner: resp }
    }
}

impl NovaResponse {
    /// Extract `Option<T>` from result at `idx`, converting through JSON for serde compatibility.
    pub fn take_opt<T: DeserializeOwned>(&mut self, idx: usize) -> Result<Option<T>, DbError> {
        match self.inner.take::<Option<SurrealVal>>(idx)? {
            None => Ok(None),
            Some(v @ (SurrealVal::None | SurrealVal::Null)) => {
                let _ = v;
                Ok(None)
            }
            Some(v) => serde_json::from_value::<T>(v.into_json_value())
                .map(Some)
                .map_err(|e| DbError::thrown(e.to_string())),
        }
    }

    /// Extract `Vec<T>` from result at `idx`, converting through JSON for serde compatibility.
    pub fn take_vec<T: DeserializeOwned>(&mut self, idx: usize) -> Result<Vec<T>, DbError> {
        let vals = self.inner.take::<Vec<SurrealVal>>(idx)?;
        vals.into_iter()
            .map(|v| {
                serde_json::from_value::<T>(v.into_json_value())
                    .map_err(|e| DbError::thrown(e.to_string()))
            })
            .collect()
    }

    /// Extract exactly one `T` from result at `idx`, or error if absent.
    pub fn take_one<T: DeserializeOwned>(&mut self, idx: usize) -> Result<T, DbError> {
        match self.inner.take::<Option<SurrealVal>>(idx)? {
            Some(v) if !matches!(v, SurrealVal::None | SurrealVal::Null) => {
                serde_json::from_value::<T>(v.into_json_value())
                    .map_err(|e| DbError::thrown(e.to_string()))
            }
            _ => Err(DbError::thrown(format!(
                "Expected a result at index {idx}, got NONE"
            ))),
        }
    }

    /// Extract the first element of a `Vec<T>` from result at `idx`, or error if empty.
    pub fn take_first<T: DeserializeOwned>(&mut self, idx: usize) -> Result<T, DbError> {
        let vals = self.inner.take::<Vec<SurrealVal>>(idx)?;
        match vals.into_iter().next() {
            Some(v) => serde_json::from_value::<T>(v.into_json_value())
                .map_err(|e| DbError::thrown(e.to_string())),
            None => Err(DbError::thrown(format!(
                "Expected non-empty array at index {idx}, got empty"
            ))),
        }
    }

    pub fn into_inner(self) -> IndexedResults {
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
            username: username.to_owned(),
            password: password.to_owned(),
            namespace: namespace.to_owned(),
            database: database.to_owned(),
        })
        .await?;

        Ok(Self { db })
    }

    /// Execute a SQL query and return a typed response wrapper.
    #[instrument(skip(self))]
    pub async fn query(&self, sql: &str) -> Result<NovaResponse, DbError> {
        Ok(self.db.query(sql).await?.into())
    }

    /// Execute a SQL query with named variable bindings.
    ///
    /// Pass a `serde_json::json!({ "var": value, ... })` object — each key maps to a
    /// `$var` placeholder in the SQL.
    ///
    /// ```rust,ignore
    /// db.query_with(
    ///     "SELECT * FROM person WHERE id = $id",
    ///     json!({ "id": person_id }),
    /// ).await?;
    /// ```
    #[instrument(skip(self, args))]
    pub async fn query_with(&self, sql: &str, args: JsonValue) -> Result<NovaResponse, DbError> {
        Ok(self.db.query(sql).bind(args).await?.into())
    }

    /// Begin a transaction. Use the returned [`Transaction`] to execute statements,
    /// then call `.commit()` or `.cancel()` to close it.
    ///
    /// `Transaction` derefs to `Surreal<Any>`, so `.query()` and all the usual typed
    /// methods work on it. Wrap responses in [`NovaResponse`] for the typed extraction helpers:
    ///
    /// ```rust,ignore
    /// let tx = db.begin().await?;
    /// let mut resp: NovaResponse = tx.query(sql).await?.into();
    /// let result = resp.take_one::<MyType>(0)?;
    /// tx.commit().await?;
    /// ```
    #[instrument(skip(self))]
    pub async fn begin(&self) -> Result<Transaction<Any>, DbError> {
        self.db.clone().begin().await
    }

    /// Execute a [`NovaQuery`] with typed variable bindings.
    #[instrument(skip(self, q))]
    pub async fn exec(&self, q: NovaQuery) -> Result<NovaResponse, DbError> {
        Ok(self.db.query(&q.sql).bind(q.args).await?.into())
    }

    /// Execute a single-statement `SELECT ...` query and extract the whole
    /// result as `Vec<T>` in one call — the "select a list of records"
    /// shape repeated across `list_income`/`list_expenses`/`list_payments`
    /// and their equivalents in the estimate path, without each call site
    /// writing out `exec` + `take_vec::<T>(0)` by hand.
    #[instrument(skip(self, q))]
    pub async fn exec_vec<T: DeserializeOwned>(&self, q: NovaQuery) -> Result<Vec<T>, DbError> {
        self.exec(q).await?.take_vec::<T>(0)
    }
}

/// A SQL query with typed variable bindings ready for execution via [`NovaDB::exec`].
///
/// Build one with `NovaQuery::new(sql)` then chain `.bind(key, val)` calls.
/// Any type implementing [`SurrealValue`] can be bound — including [`RecordId`],
/// `String`, `&str`, `bool`, numeric types, etc.
///
/// [`RecordId`]: surrealdb::types::RecordId
#[derive(Debug)]
pub struct NovaQuery {
    pub sql: String,
    pub args: BTreeMap<String, SurrealVal>,
}

impl NovaQuery {
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            args: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, key: &str, val: impl SurrealValue) -> Self {
        self.args.insert(key.to_string(), val.into_value());
        self
    }

    /// Bind any `Serialize` value by routing it through `serde_json`.
    ///
    /// Use this for nested structures — enums, sub-objects, arrays — that have
    /// no direct [`SurrealValue`] impl. They land in SurrealDB as native
    /// objects and arrays, so the same serde definition reads them back out
    /// through [`NovaResponse`], which also converts via JSON.
    ///
    /// Panics if the value cannot be serialized, which for the model types
    /// here means a bug rather than bad input.
    pub fn bind_json(self, key: &str, val: &impl serde::Serialize) -> Self {
        let json = serde_json::to_value(val).expect("model value must serialize to JSON");
        self.bind(key, json_to_surreal(json))
    }
}

/// Convert JSON into a SurrealDB value, mapping `null` to `NONE`.
///
/// This mapping is the whole reason the conversion isn't left to the blanket
/// `SurrealValue` impl for `serde_json::Value`. Rust models spell "absent" as
/// `Option::None`, which serde writes as JSON `null` — but SurrealDB treats
/// `NULL` and `NONE` as different values, and a field declared
/// `TYPE option<string>` rejects `NULL`. Without this, every insert carrying
/// an empty optional field fails its type assertion.
pub fn json_to_surreal(value: JsonValue) -> SurrealVal {
    match value {
        JsonValue::Null => SurrealVal::None,
        JsonValue::Bool(b) => SurrealVal::Bool(b),
        JsonValue::String(s) => SurrealVal::String(s),
        JsonValue::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into_value(),
            (None, Some(f)) => f.into_value(),
            // A number serde_json can represent but neither i64 nor f64 can
            // hold exactly (a u64 above i64::MAX). Keep the exact text rather
            // than silently rounding it.
            (None, None) => SurrealVal::String(n.to_string()),
        },
        JsonValue::Array(a) => {
            SurrealVal::Array(a.into_iter().map(json_to_surreal).collect::<Array>())
        }
        JsonValue::Object(o) => {
            let mut obj = Object::new();
            for (k, v) in o {
                obj.insert(k, json_to_surreal(v));
            }
            SurrealVal::Object(obj)
        }
    }
}
