// use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use time::OffsetDateTime;

use super::meta::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    pub person: String,
    pub signed_token: Option<String>,
    pub meta: Meta<()>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRecord {
    pub created_by: RecordId,

    #[serde(with = "time::serde::iso8601")]
    pub created_on: OffsetDateTime, // DateTime<Utc>,

    #[serde(with = "time::serde::iso8601::option")]
    pub deleted_on: Option<OffsetDateTime>, // Option<DateTime<Utc>>,

    pub id: String,
    pub person: String,
    pub meta: RecordId,
}

#[derive(Debug, Serialize)]
pub struct SetSignedTokenArgs {
    pub token_id: RecordId,
    pub signed_token: String,
}

#[derive(Debug, Serialize)]
pub struct InsertTokenArgs {
    pub person: RecordId,
    pub meta: RecordId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BareToken {
    pub id: RecordId,
    pub person: RecordId,
    pub meta: RecordId,
}

#[derive(Debug, Serialize)]
pub struct SelectTokenArgs {
    pub id: String,
}
