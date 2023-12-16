use super::base::Meta;

use serde::Serialize;

#[derive(Serialize)]
pub struct Post {
    pub meta: Meta<()>,
    pub markdown: String, // the text content of a post will be written with Markdown
}
