
use serde::{Serialize};
use surrealdb::engine::remote::ws::{Ws, Client};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

#[derive(Debug, Serialize)]
struct Person<'a> {
    name: &'a str,
}

struct SurrealDB {
    db: Surreal<Client>,
}

impl SurrealDB {
    pub async fn new(address: &str, username: &str, password: &str, namespace: &str, database: &str) -> Self {
        let db = Surreal::new::<Ws>(address).await.unwrap();

        db.signin(Root {
            username,
            password,
        })
        .await.unwrap();

        db.use_ns(namespace).use_db(database).await.unwrap();

        Self {
            db
        }
    }
}

pub async fn get_something() {
    println!("insert post");
    
    let SurrealDB { db } = SurrealDB::new(
        "127.0.0.1:52000",
        "root",
        "root",
        "test",
        "novabyte.blog")
    .await;

    // Perform a custom advanced query
    let groups = db
        .query("SELECT * FROM type::table($table)")
        .bind(("table", "person"))
        .await.unwrap();
    dbg!(groups);
}
