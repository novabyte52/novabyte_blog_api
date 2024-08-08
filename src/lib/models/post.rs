use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use super::meta::Meta;

// TODO: should either split out these models and make sure i remove an unneeded ones

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub meta: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostHydrated {
    pub id: String,
    pub created_by: String,
    pub created_on: DateTime<Utc>,
    pub working_title: String,
    pub meta: Meta<()>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostVersion {
    pub id: String,
    pub draft_id: String,
    pub title: String,
    pub markdown: String,
    pub author: String,
    pub published: Option<bool>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostContent {
    pub title: String,
    pub markdown: String,
    pub author: String,
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
    pub post_id: String,
}

// === draft models === //

#[derive(Debug, Deserialize)]
pub struct Drafted {
    pub id: String,
    pub r#in: String,
    pub r#out: String,
    pub markdown: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct Published {
    pub id: String,
    pub r#in: String,
    pub r#out: String,
    pub markdown: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DraftPostArgs {
    pub id: Option<String>,
    pub title: String,
    pub markdown: String,
    pub published: bool,
}

// === author models === //

#[derive(Debug, Deserialize)]
pub struct NewAuthored {
    pub r#in: String,
    pub r#out: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Authored {
    pub id: String,
    pub r#in: String,
    pub r#out: String,
    pub at: DateTime<Utc>,
}
