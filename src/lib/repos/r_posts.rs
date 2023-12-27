use surrealdb::sql::{Id, Thing};
use ulid::Ulid;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::person::Person;
use crate::models::post::SelectPostArgs;

pub struct PostsRepo {
    reader: NovaDB,
    writer: NovaDB,
}

impl PostsRepo {
    pub async fn new() -> Self {
        let reader = NovaDB::new(SurrealDBConnection {
            address: "127.0.0.1:52000",
            username: "root",
            password: "root",
            namespace: "test",
            database: "novabyte.blog",
        })
        .await;

        let writer = NovaDB::new(SurrealDBConnection {
            address: "127.0.0.1:52000",
            username: "root",
            password: "root",
            namespace: "test",
            database: "novabyte.blog",
        })
        .await;

        Self { reader, writer }
    }

    pub async fn insert_post(&self) -> Person {
        println!("insert post");
        // Perform a custom advanced query
        let user = self
            .writer
            .query_single::<Person>(
                r#"
                    CREATE 
                        person
                    SET 
                    name = 'Generated 01'
                "#,
            )
            .await;

        match user {
            Some(p) => {
                println!("{:#?}", p);
                p
            }
            _ => {
                panic!("nothing found!");
            }
        }
    }

    pub async fn select_post(&self, post_id: Ulid) -> Person {
        println!("r: select post: {}", post_id);

        let query = self.reader.query_single_with_args(
            "SELECT * FROM person WHERE id = $id",
            SelectPostArgs {
                id: Thing {
                    tb: String::from("person"),
                    id: Id::String(String::from("01HJ4T9031ZWV6N8XM17Z9XV9C")),
                },
            },
        );

        match query.await {
            Some(r) => r,
            None => panic!("Nothing found!"),
        }
    }

    pub async fn select_posts(&self) -> Vec<Person> {
        println!("r: select posts");
        self.reader.query_many("SELECT * FROM person").await
    }
}
