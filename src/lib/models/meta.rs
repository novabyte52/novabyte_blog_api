use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize)]
pub struct InsertMetaArgs {
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta<T> {
    pub id: String,
    pub created_by: String,
    pub modified_by: Option<String>,
    pub deleted_by: Option<String>,
    pub data: Option<T>,

    #[serde(with = "time::serde::iso8601")]
    pub created_on: OffsetDateTime,

    #[serde(with = "time::serde::iso8601::option")]
    pub modified_on: Option<OffsetDateTime>,

    #[serde(with = "time::serde::iso8601::option")]
    pub deleted_on: Option<OffsetDateTime>,
}

#[derive(Deserialize)]
pub struct IdContainer {
    pub id: String,
}
