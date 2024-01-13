use super::{meta::Meta, person::Person};

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub title: String,
    pub markdown: String,
    pub author: Person,
    pub meta: Meta<()>,
}

#[derive(Debug, Serialize)]
pub struct SelectPostArgs {
    pub id: Thing,
}
