use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::meta::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    pub person: String,
    pub meta: Meta<()>,
}

// TODO: don't like having this varient for only one property difference
// maybe the better solution is to use an Option type...
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenRecord {
    pub created_by: Thing,
    pub created_on: DateTime<Utc>,
    pub deleted_on: Option<DateTime<Utc>>,
    pub id: String,
    pub person: String,
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct InsertTokenArgs {
    pub person: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BareToken {
    pub id: Thing,
    pub person: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct SelectTokenArgs {
    pub id: String,
}
