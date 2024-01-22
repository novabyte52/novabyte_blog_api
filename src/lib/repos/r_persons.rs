use std::collections::HashMap;

use surrealdb::sql::{Id, Thing};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::person::{InsertPersonArgs, Person, SignUpState};
use crate::models::post::SelectPostArgs;
use crate::repos::r_meta::MetaRepo;

pub struct PersonsRepo {
    reader: NovaDB,
    writer: NovaDB,
    meta: MetaRepo,
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

        Self {
            reader,
            writer,
            meta: MetaRepo::new().await,
        }
    }

    pub async fn insert_person(&self, new_person: SignUpState, created_by: Thing) -> Person {
        println!("r: insert person - {:#?}", created_by);

        let pass_hash = match new_person.pass_hash {
            Some(ph) => ph,
            None => panic!("Can't create user without the password hash!"),
        };

        let meta = self.meta.insert_meta(InsertMetaArgs { created_by }).await;

        let create_user = self
            .writer
            .query_single_with_args::<Person, InsertPersonArgs>(
                r#"
                    CREATE 
                        person:ulid()
                    SET 
                        email = $email,
                        username = $username,
                        pass_hash = $pass_hash,
                        meta = $meta;
                "#,
                InsertPersonArgs {
                    email: new_person.email,
                    username: new_person.username,
                    pass_hash,
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

    pub async fn select_person_by_email(&self, email: String) -> Option<Person> {
        println!("r: select person by email");

        let query = self
            .reader
            .query_single_with_args::<Person, (String, String)>(
                "SELECT * FROM person WHERE email = $email",
                (String::from("email"), email),
            );

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Error selecting persons: {:#?}", e),
        }
    }

    pub async fn select_person_hash_by_email(&self, email: String) -> String {
        println!("r: select person hash by email");

        let query = self
            .reader
            .query_single_with_args::<HashMap<String, String>, (String, String)>(
                "SELECT pass_hash FROM person WHERE email = $email",
                (String::from("email"), email),
            );

        match query.await {
            Ok(r) => match r {
                Some(h) => match h.get("pass_hash") {
                    Some(h) => h.to_string(),
                    None => panic!("No person hash found in map"),
                },
                None => panic!("No person hash record found"),
            },
            Err(e) => panic!("Error selecting persons: {:#?}", e),
        }
    }

    pub async fn select_persons(&self) -> Vec<Person> {
        println!("r: select posts");

        let query = self.reader.query_many("SELECT * FROM person");

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Error selecting persons: {:#?}", e),
        }
    }
}
