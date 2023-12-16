use std::fmt::Debug;

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta<T> {
    pub id: Uuid,
    pub created_by: Uuid,
    pub created_on: DateTime<Utc>,
    pub modified_by: Uuid,
    pub modified_on: DateTime<Utc>,
    pub deleted_by: Uuid,
    pub deleted_on: DateTime<Utc>,
    pub data: Option<T>,
}
