use std::collections::HashMap;

use surrealdb::sql::Thing;
use tracing::error;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::person::{InsertPersonArgs, Person, SelectPersonArgs, SignUpState};
use crate::models::token::{InsertTokenArgs, Token, TokenRecord};
use crate::repos::r_meta::MetaRepo;
use crate::utils::thing_from_string;

pub struct PersonsRepo {
    reader: NovaDB,
    _writer: NovaDB,
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
            _writer: writer,
            meta: MetaRepo::new().await,
        }
    }

    pub async fn insert_person(
        &self,
        new_person: SignUpState,
        created_by: String,
        tran_conn: &NovaDB,
    ) -> Person {
        println!("r: insert person - {:#?}", created_by);

        let pass_hash = match new_person.pass_hash {
            Some(ph) => ph,
            None => panic!("Can't create user without the password hash!"),
        };

        let meta = self
            .meta
            .insert_meta(InsertMetaArgs { created_by }, Some(tran_conn))
            .await;

        let create_user = tran_conn.query_single_with_args::<Person, InsertPersonArgs>(
            r#"
                    CREATE 
                        person:ulid()
                    SET 
                        email = $email,
                        username = $username,
                        pass_hash = $pass_hash,
                        is_admin = false,
                        meta = $meta;
                "#,
            InsertPersonArgs {
                email: new_person.email,
                username: new_person.username,
                pass_hash,
                meta: thing_from_string(&meta.id),
            },
        );

        match create_user.await {
            Some(p) => p,
            None => panic!("No person returned, potential issue creating person"),
        }
    }

    pub async fn select_person(&self, person_id: String) -> Option<Person> {
        println!("r: select persons: {}", person_id);

        self.reader.query_single_with_args(
            "SELECT fn::string_id(id) as id, *, fn::string_id(meta) as meta FROM person WHERE id = $id",
            SelectPersonArgs { id: thing_from_string(&person_id) },
        ).await
    }

    pub async fn select_person_by_email(&self, email: String) -> Option<Person> {
        println!("r: select person by email | {:#?}", &email);

        self.reader
            .query_single_with_args::<Person, (String, String)>(
                r#"
                    SELECT
                        fn::string_id(id) as id,
                        username,
                        email,
                        is_admin,
                        fn::string_id(meta) as meta
                    FROM person WHERE email = $email"#,
                (String::from("email"), email),
            )
            .await
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
            Some(h) => match h.get("pass_hash") {
                Some(h) => h.to_string(),
                None => panic!("No person hash found in map"),
            },
            None => panic!("No person hash record found"),
        }
    }

    pub async fn select_persons(&self) -> Vec<Person> {
        println!("r: select posts");

        let query = self.reader.query_many(
            "SELECT fn::string_id(id) as id, *, fn::string_id(meta) as meta FROM person",
        );

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Error selecting persons: {}", e),
        }
    }

    pub async fn select_token_record(&self, token_id: String) -> Token {
        println!("token_id: {:#?}", token_id);

        let token_query = format!(
            r#"
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(person) as person,
                    {}
                FROM nb_token
                WHERE id = $id;
            "#,
            self.meta.select_meta_string.clone()
        );

        let query = self.reader.query_single_with_args::<Token, (&str, Thing)>(
            token_query.as_str(),
            ("id", thing_from_string(&token_id)),
        );

        match query.await {
            Some(t) => t,
            None => panic!("No token found for token_id: {}", token_id),
        }
    }

    /*
    TODO: things like the below sort of confuse me...
    creating a token takes multiple queries and then getting the proper
    return takes some more. I think some of this can be pulled into the
    service... but what?

    maybe a service function called get_token_record which will handle
    building the token record out of the various parts.
    */
    pub async fn insert_token_record(&self, person_id: String, tran_conn: &NovaDB) -> TokenRecord {
        let meta = self
            .meta
            .insert_meta(
                InsertMetaArgs {
                    created_by: person_id.clone(),
                },
                Some(tran_conn),
            )
            .await;

        let query = tran_conn.query_single_with_args_specify_result::<Thing, InsertTokenArgs>(
            r#"
                    LET $token_id = nb_token:ulid();
                    CREATE
                        $token_id
                    SET 
                        person = $person,
                        meta = $meta;
                    
                    RETURN $token_id;
                "#,
            InsertTokenArgs {
                person: thing_from_string(&person_id),
                meta: thing_from_string(&meta.id),
            },
            2,
        );

        let token_thing = match query.await {
            Some(t) => t,
            None => {
                tran_conn.cancel_tran().await;
                error!("Unable to get token id, cancelling transaction");
                panic!();
            }
        };

        let token = self.select_token_record(token_thing.to_string()).await;

        let meta = match self.meta.select_meta(&token.meta.id).await {
            Some(m) => m,
            None => panic!("Meta not found!"),
        };

        TokenRecord {
            id: token.id.to_string(),
            person: token.person.to_string(),
            created_by: thing_from_string(&meta.created_by),
            created_on: meta.created_on,
            deleted_on: meta.deleted_on,
            meta: thing_from_string(&meta.id),
        }
    }

    pub async fn soft_delete_token_record(&self, token_id: &String) {
        let query = self.reader.query_none_with_args(
            r#"
                LET $meta_id = (SELECT meta FROM nb_token WHERE id = $token_id);
                UPDATE $meta_id.meta SET deleted_on = time::now();
            "#,
            ("token_id", thing_from_string(token_id)),
        );

        query.await
    }
}
