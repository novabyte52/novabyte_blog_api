use std::fmt::Debug;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize)]
pub struct InsertMetaArgs {
    pub created_by: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta<T> {
    pub id: Thing,
    pub created_by: Thing,
    pub created_on: DateTime<Utc>,
    pub modified_by: Option<Thing>,
    pub modified_on: Option<DateTime<Utc>>,
    pub deleted_by: Option<Thing>,
    pub deleted_on: Option<DateTime<Utc>>,
    pub data: Option<T>,
}
