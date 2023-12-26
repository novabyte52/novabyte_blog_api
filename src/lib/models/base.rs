use std::fmt::Debug;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use ulid::Ulid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta<T> {
    pub id: Thing,
    pub created_by: Ulid,
    pub created_on: DateTime<Utc>,
    pub modified_by: Ulid,
    pub modified_on: DateTime<Utc>,
    pub deleted_by: Ulid,
    pub deleted_on: DateTime<Utc>,
    pub data: Option<T>,
}
