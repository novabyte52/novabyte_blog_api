use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CustomClaims {
    pub name: String,
    pub is_admin: bool,
}
