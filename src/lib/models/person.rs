use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: Thing,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub meta: Thing,
}

#[derive(Debug, Deserialize)]
pub struct LogInCreds {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SignUpCreds {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct PostPerson {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SignUpState {
    pub username: String,
    pub email: String,
    pub password: String,
    pub pass_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InsertPersonArgs {
    pub username: String,
    pub email: String,
    pub pass_hash: String,
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct SelectPersonArgs {
    pub id: Thing,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub person: Person,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub token: String,
}
