use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::meta::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub id: Thing,
    pub person: Thing,
    pub meta: Meta<()>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BareToken {
    pub id: Thing,
    pub person: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InsertTokenArgs {
    pub person: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct SelectTokenArgs {
    pub id: Thing,
}
