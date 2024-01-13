use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: Thing,
    pub username: String,
    pub email: String,
    pass_hash: String,
    pub meta: Thing,
}

pub struct Creds {
    pub username: String,
    pub password: String
}

#[derive(Debug, Deserialize)]
pub struct PostPerson {
    pub username: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct InsertPersonArgs {
    pub username: String,
    pub email: String,
    pub meta: Thing,
}
