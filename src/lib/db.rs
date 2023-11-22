use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use tokio::time::sleep;
use tokio_postgres::NoTls;
use uuid::Uuid;

use crate::models::{base::MetaData, post::Post};

pub async fn insert_post() -> Uuid {
    println!("insert post");
    // Connect to the database.
    // let (client, connection) = match tokio_postgres::connect(POSTGRES_CONN_STRING, NoTls).await {
    //     Ok(r) => r,
    //     Err(err) => panic!("Error connecting to database: {:?}", err),
    // };

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    // tokio::spawn(async move {
    //     if let Err(e) = connection.await {
    //         eprintln!("connection error: {}", e);
    //     }
    // });

    // Now we can execute a simple statement that just returns its parameter.
    // let results = client.query("SELECT $1::TEXT", &[&"hello world"]).await;

    // let rows = match results {
    //     Ok(r) => r,
    //     Err(err) => panic!("Error getting rows: {:?}", err),
    // };

    // let result: &str = rows[0].get(0);
    // println!("query result: {}", result);
    // return String::from(result);
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
