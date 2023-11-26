use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use tokio::time::sleep;
use uuid::Uuid;

use crate::models::{base::MetaData, post::Post};
use crate::db::get_something;

pub async fn insert_post() -> Uuid {
    println!("insert post");
    get_something().await;
    return Uuid::new_v4();
}

pub async fn select_post(post_id: Uuid) -> Post {
    println!("s: select post: {}", post_id);
    sleep(Duration::from_secs(3)).await;
    return Post {
        post_id: Uuid::new_v4(),
        markdown: String::from("# Heading 1"),
        metadata: MetaData {
            created_by: Uuid::new_v4(),
            created_on: DateTime::from_naive_utc_and_offset(
                NaiveDateTime::from_timestamp_micros(1662921288000000).unwrap(),
                Utc,
            ),
            modified_by: Uuid::new_v4(),
            modified_on: DateTime::from_naive_utc_and_offset(
                NaiveDateTime::from_timestamp_micros(1662921288000000).unwrap(),
                Utc,
            ),
            deleted_by: Uuid::new_v4(),
            deleted_on: DateTime::from_naive_utc_and_offset(
                NaiveDateTime::from_timestamp_micros(1662921288000000).unwrap(),
                Utc,
            ),
        },
    };
}

pub async fn select_posts() -> Vec<u32> {
    println!("s: select posts");
    return vec![5, 2, 7, 8, 13];
}
