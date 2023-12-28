use surrealdb::sql::{Id, Thing};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::person::{InsertPersonArgs, Person, PostPerson};
use crate::models::post::SelectPostArgs;

pub struct PersonsRepo {
    reader: NovaDB,
    writer: NovaDB,
}

impl PersonsRepo {
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

    pub async fn insert_person(&self, new_person: PostPerson, created_by: Thing) -> Person {
        println!("r: insert post - {:#?}", created_by);
        // Perform a custom advanced query
        let user = self
            .writer
            .query_single_with_args::<Person, InsertPersonArgs>(
                r#"
                    CREATE 
                        person:ulid()
                    SET 
                        email = $email,
                        username = $username,
                        meta = {
                            created_by: $created_by,
                            created_on: time::now(),
                            modified_by: NULL,
                            modified_on: NULL,
                            deleted_by: NULL,
                            deleted_on: NULL,
                        }
                "#,
                InsertPersonArgs {
                    email: new_person.email,
                    username: new_person.username,
                    created_by,
                },
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

    pub async fn select_person(&self, person_id: Id) -> Person {
        println!("r: select post: {}", person_id);

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

    pub async fn select_persons(&self) -> Vec<Person> {
        println!("r: select posts");
        self.reader.query_many("SELECT * FROM person").await
    }
}
