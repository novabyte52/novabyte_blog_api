// use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::meta::Meta;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostVersion {
    pub id: String,
    pub draft_id: String,
    pub title: String,
    pub markdown: String,
    pub author: String,
    pub published: Option<bool>,
    #[serde(with = "time::serde::iso8601")]
    pub at: OffsetDateTime,
    pub image: String,
    pub visits: u128,
    pub meta: Meta<()>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DraftPostArgs {
    pub id: Option<String>,
    pub title: String,
    pub markdown: String,
    pub published: bool,
    pub image: String,
}
