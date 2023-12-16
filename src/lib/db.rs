pub mod nova_db;
use nova_db::NovaDB;

use crate::models::user::User;

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
    let user = db.query_single::<User>("SELECT * FROM user").await;

    match user {
        Some(p) => println!("{:#?}", p),
        _ => println!("nothing found")
    }
}
