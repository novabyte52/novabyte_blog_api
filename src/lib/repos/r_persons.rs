use serde::Serialize;
use surrealdb::sql::{Id, Thing};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::base::Meta;
use crate::models::person::{InsertPersonArgs, Person, PostPerson};
use crate::models::post::SelectPostArgs;

pub struct PersonsRepo {
    reader: NovaDB,
    writer: NovaDB,
}

#[derive(Debug, Serialize)]
struct InsertMetaArgs {
    created_by: Thing,
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
        println!("r: insert person - {:#?}", created_by);

        let create_meta = self.writer
            .query_single_with_args::<Meta<()>, InsertMetaArgs>(
                r#"
                    CREATE
                        meta:ulid()
                    SET
                        created_by = $created_by
                "#,
                InsertMetaArgs {
                    created_by: created_by.clone(),
                },
            );

        let meta = match create_meta.await {
            Ok(m) => match m {
                Some(m) => m,
                None => panic!("No meta returned, potential issue creating meta for person"),
            },
            Err(e) => panic!("Meta creation failed: {:#?}", e),
        };

        // println!("created meta id: {:#?}", meta);

        let create_user = self
            .writer
            .query_single_with_args::<Person, InsertPersonArgs>(
                r#"
                    CREATE 
                        person:ulid()
                    SET 
                        email = $email,
                        username = $username,
                        meta = $meta;
                "#,
                InsertPersonArgs {
                    email: new_person.email,
                    username: new_person.username,
                    meta: meta.id,
                },
            );

        match create_user.await {
            Ok(p) => match p {
                Some(p) => p,
                None => panic!("No person returned, potential issue creating person"),
            },
            Err(e) => panic!("Error creating user!: {:#?}", e),
        }
    }

    pub async fn select_person(&self, person_id: Id) -> Option<Person> {
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
            Ok(r) => r,
            Err(e) => panic!("Nothing found! {:#?}", e),
        }
    }

    pub async fn select_persons(&self) -> Vec<Person> {
        println!("r: select posts");
        // self.reader.query_many("SELECT * FROM person").await

        let query = self.reader.query_many("SELECT * FROM person");

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Error selecting persons: {:#?}", e),
        }
    }
}
