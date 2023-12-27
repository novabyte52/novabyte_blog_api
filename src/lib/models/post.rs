use super::base::Meta;

use serde::Serialize;
use surrealdb::sql::Thing;

#[derive(Serialize)]
pub struct Post {
    pub meta: Meta<()>,
    pub markdown: String, // the text content of a post will be written with Markdown
}

#[derive(Debug, Serialize)]
pub struct SelectPostArgs {
    pub id: Thing,
}
