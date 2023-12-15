pub mod nova_db;

use serde::Serialize;

use crate::db::nova_db::NovaDB;

#[derive(Debug, Serialize)]
struct Person<'a> {
    name: &'a str,
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

    let NovaDB { novadb } = NovaDB::new(SurrealDBConnection {
        address: "127.0.0.1:52000",
        username: "root",
        password: "root",
        namespace: "test",
        database: "novabyte.blog",
    })
    .await;

    // Perform a custom advanced query
    let groups = novadb
        .query("SELECT * FROM type::table($table)")
        .bind(("table", "person"))
        .await
        .unwrap();
    dbg!(groups);
}
