pub mod nova_db;

use serde::{Serialize, Deserialize};

use crate::db::nova_db::NovaDB;

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,
}

pub struct SurrealDBConnection<'a> {
    address: &'a str,
    username: &'a str,
    password: &'a str,
    namespace: &'a str,
    database: &'a str,
}

pub async fn get_something() {
    println!("insert post");

    let db = NovaDB::new(SurrealDBConnection {
        address: "127.0.0.1:52000",
        username: "root",
        password: "root",
        namespace: "test",
        database: "novabyte.blog",
    })
    .await;

    // Perform a custom advanced query
    let person = db.query_single::<Person>("SELECT * FROM person").await;

    match person {
        Some(p) => println!("{:#?}", p),
        _ => println!("nothing found")
    }
}
