use super::base::MetaData;

use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Post {
    pub post_id: Uuid,
    pub markdown: String, // the text content of a post will be written with Markdown
    pub metadata: MetaData,
}
