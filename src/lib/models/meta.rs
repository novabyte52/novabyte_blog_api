use std::fmt::Debug;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct InsertMetaArgs {
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta<T> {
    pub id: String,
    pub created_by: String,
    pub created_on: DateTime<Utc>,
    pub modified_by: Option<String>,
    pub modified_on: Option<DateTime<Utc>>,
    pub deleted_by: Option<String>,
    pub deleted_on: Option<DateTime<Utc>>,
    pub data: Option<T>,
}

#[derive(Deserialize)]
pub struct IdContainer {
    pub id: String,
}
