use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::base::Meta;

#[derive(Debug, Deserialize)]
pub struct PostPerson {
    pub username: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct InsertPersonArgs {
    pub username: String,
    pub email: String,
    pub created_by: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: Thing,
    pub username: String,
    pub email: String,
    pub meta: Meta<()>,
}
