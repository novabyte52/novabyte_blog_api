use serde::{Serialize, Deserialize};

use super::base::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub meta: Meta<()>,
    pub username: String,
    pub email: String,
}
