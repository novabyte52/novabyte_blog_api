use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::meta::Meta;

// TODO: should either split out these models and make sure i remove any unneeded ones
#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub meta: Meta<()>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostHydrated {
    pub id: String,
    pub working_title: String,
    pub meta: Meta<()>,
}

// TODO: rename id to post_id and draft_id to id
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostVersion {
    pub id: String,
    pub draft_id: String,
    pub title: String,
    pub markdown: String,
    pub author: String,
    pub published: Option<bool>,
    pub at: DateTime<Utc>,
    pub meta: Meta<()>,
}

#[derive(Debug, Deserialize)]
pub struct Drafted {
    pub id: String,
    pub r#in: String,
    pub r#out: String,
    pub markdown: String,
    pub at: DateTime<Utc>,
    pub meta: Meta<()>,
}

// === draft models === //

#[derive(Debug, Deserialize, Clone)]
pub struct DraftPostArgs {
    pub id: Option<String>,
    pub title: String,
    pub markdown: String,
    pub published: bool,
}
