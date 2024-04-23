use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: Thing,
    pub title: String,
    pub markdown: String,
    pub author: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostPostArgs {
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePostArgs {
    pub title: String,
    pub markdown: String,
    pub author: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct SelectPostArgs {
    pub id: Thing,
}
