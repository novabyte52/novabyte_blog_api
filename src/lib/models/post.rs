use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

// TODO: should either split out these models and make sure i remove an unneeded ones

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: Thing,
    pub meta: Thing,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostVersion {
    pub id: Thing,
    pub draft_id: Thing,
    pub title: String,
    pub markdown: String,
    pub author: Thing,
    pub published: Option<bool>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostContent {
    pub title: String,
    pub markdown: String,
    pub author: Thing,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostPostArgs {
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePostArgs {
    pub meta: Thing,
}

#[derive(Debug, Serialize)]
pub struct SelectPostArgs {
    pub post_id: Thing,
}

// === draft models === //

#[derive(Debug, Deserialize)]
pub struct Drafted {
    pub id: Thing,
    pub r#in: Thing,
    pub r#out: Thing,
    pub markdown: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct Published {
    pub id: Thing,
    pub r#in: Thing,
    pub r#out: Thing,
    pub markdown: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DraftPostArgs {
    pub id: Option<Thing>,
    pub title: String,
    pub markdown: String,
    pub published: bool,
}

// === author models === //

#[derive(Debug, Deserialize)]
pub struct NewAuthored {
    pub r#in: Thing,
    pub r#out: Thing,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Authored {
    pub id: Thing,
    pub r#in: Thing,
    pub r#out: Thing,
    pub at: DateTime<Utc>,
}
