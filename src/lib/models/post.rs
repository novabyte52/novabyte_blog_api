use super::{base::Meta, person::Person};

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub meta: Meta<()>,
    pub markdown: String, // the text content of a post will be written with Markdown
    pub author: Person,
}

#[derive(Debug, Serialize)]
pub struct SelectPostArgs {
    pub id: Thing,
}
