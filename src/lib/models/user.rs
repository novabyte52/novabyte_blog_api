use serde::{Deserialize, Serialize};

use super::base::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub meta: Meta<()>,
    pub username: String,
    pub email: String,
}
