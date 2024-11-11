use std::collections::HashMap;

use surrealdb::sql::Thing;
use tracing::{error, info, instrument};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::person::{
    InsertPersonArgs, Person, PersonCheck, PersonCheckResponse, SelectPersonArgs, SignUpState,
};
use crate::models::token::{InsertTokenArgs, SetSignedTokenArgs, Token, TokenRecord};
use crate::repos::r_meta::MetaRepo;
use crate::utils::thing_from_string;

#[derive(Debug, Clone)]
pub struct PersonsRepo {
    reader: NovaDB,
    writer: NovaDB,
    meta: MetaRepo,
}

impl PersonsRepo {
    #[instrument]
    pub async fn new(conn: &SurrealDBConnection) -> Self {
        let reader = NovaDB::new(conn).await;

        let writer = NovaDB::new(conn).await;

        Self {
            reader,
            writer,
            meta: MetaRepo::new(conn).await,
        }
    }

    #[instrument(skip(self))]
    pub async fn is_unique_email<'b>(&self, email: &'b String) -> bool {
        let query = self.reader.query_single_with_args::<bool, (&str, String)>(
            r#"
                    IF string::is::email($email) {    
                        LET $count = (
                            SELECT
                                count(email)
                            FROM ONLY person
                            WHERE email = $email
                            LIMIT 1
                        ).count;
                        
                        RETURN $count IS NONE;
                    } ELSE {
                        RETURN false;
                    };
                "#,
            ("email", email.clone()),
        );

        let response = match query.await {
            Ok(c) => c,
            Err(e) => {
                error!("Error checking validity of email: {:#?}", e);
                panic!();
            }
        };

        match response {
            Some(c) => c,
            None => false,
        }
    }

    #[instrument(skip(self))]
    pub async fn is_unique_username(&self, username: &String) -> bool {
        let query = self
            .reader
            .query_single_with_args_specify_result::<bool, (&str, String)>(
                r#" 
                    LET $count = (
                        SELECT
                            count(username)
                        FROM ONLY person
                        WHERE username = $username
                        LIMIT 1
                    ).count;
                    
                    RETURN $count IS NONE;
                "#,
                ("username", username.clone()),
                1,
            );

        let response = match query.await {
            Ok(c) => c,
            Err(e) => {
                error!("Error checking validity of username: {:#?}", e);
                panic!();
            }
        };

        match response {
            Some(c) => c,
            None => false,
        }
    }

    #[instrument(skip(self, tran_conn))]
    pub async fn insert_person(
        &self,
        new_person: SignUpState,
        created_by: &str,
        tran_conn: &NovaDB,
    ) -> Person {
        let pass_hash = match new_person.pass_hash {
            Some(ph) => ph,
            None => panic!("Can't create user without the password hash!"),
        };

        let validity = self
            .is_person_unique(PersonCheck {
                email: Some(new_person.email.clone()),
                username: Some(new_person.username.clone()),
            })
            .await;

        if !validity.email || !validity.username {
            error!("email or username invalid");
            panic!();
        }

        let meta = self
            .meta
            .insert_meta(
                InsertMetaArgs {
                    created_by: created_by.into(),
                },
                Some(tran_conn),
            )
            .await;

        let create_user_query = format!(
            r#"
                LET $person_id = person:ulid();

                CREATE
                    $person_id
                SET 
                    email = $email,
                    username = $username,
                    pass_hash = $pass_hash,
                    is_admin = false,
                    meta = $meta;
                
                SELECT
                    fn::string_id(id) as id,
                    *,
                    {}
                FROM person
                WHERE id = $person_id;
            "#,
            &self.meta.select_meta_string
        );

        let create_user = tran_conn
            .query_single_with_args_specify_result::<Person, InsertPersonArgs>(
                &create_user_query,
                InsertPersonArgs {
                    email: new_person.email,
                    username: new_person.username,
                    pass_hash,
                    meta: thing_from_string(&meta.id),
                },
                2,
            );

        let response = match create_user.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error creating person: {:#?}", e);
                panic!()
            }
        };

        match response {
            Some(p) => p,
            None => panic!("No person returned, potential issue creating person"),
        }
    }

    pub async fn is_person_unique(&self, check: PersonCheck) -> PersonCheckResponse {
        let mut check_response = PersonCheckResponse {
            email: false,
            username: false,
        };

        if let Some(email) = &check.email {
            check_response.email = self.is_unique_email(email).await;
        };

        if let Some(username) = &check.username {
            check_response.username = self.is_unique_username(username).await;
        };

        check_response
    }

    #[instrument(skip(self))]
    pub async fn select_person(&self, person_id: String) -> Option<Person> {
        info!("r: select persons: {}", person_id);

        let select_person_query = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM person WHERE id = $id",
            &self.meta.select_meta_string
        );

        let query = self.reader.query_single_with_args(
            &select_person_query,
            SelectPersonArgs {
                id: thing_from_string(&person_id),
            },
        );

        let response = match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error selecting person: {:#?}", e);
                panic!()
            }
        };

        match response {
            Some(p) => p,
            None => panic!(),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_person_by_email(&self, email: String) -> Option<Person> {
        info!("r: select person by email | {:#?}", &email);

        let select_person_query = format!(
            r#"
                SELECT
                    fn::string_id(id) as id,
                    username,
                    email,
                    is_admin,
                    {}
                FROM person WHERE email = $email
            "#,
            &self.meta.select_meta_string
        );

        let query = self
            .reader
            .query_single_with_args::<Person, (String, String)>(
                &select_person_query,
                (String::from("email"), email),
            );

        match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error selecting person by email: {:#?}", e);
                panic!()
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn select_person_hash_by_email(&self, email: String) -> String {
        info!("r: select person hash by email");

        let query = self
            .reader
            .query_single_with_args::<HashMap<String, String>, (String, String)>(
                "SELECT pass_hash FROM person WHERE email = $email",
                (String::from("email"), email),
            );

        let response = match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error selecting hash with email: {:#?}", e);
                panic!()
            }
        };

        match response {
            Some(h) => match h.get("pass_hash") {
                Some(h) => h.to_string(),
                None => panic!("No person hash found in map"),
            },
            None => panic!("No person hash record found"),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_persons(&self) -> Vec<Person> {
        info!("r: select posts");

        let select_persons_query = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM person",
            &self.meta.select_meta_string
        );

        let query = self.reader.query_many(&select_persons_query);

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Error selecting persons: {}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_token_record(&self, token_id: String) -> Token {
        info!("token_id: {:#?}", token_id);

        let token_query = format!(
            r#"
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(person) as person,
                    {}
                FROM nb_token
                WHERE id = $id;
            "#,
            &self.meta.select_meta_string
        );

        let query = self.reader.query_single_with_args::<Token, (&str, Thing)>(
            token_query.as_str(),
            ("id", thing_from_string(&token_id)),
        );

        let response = match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Errpr selecting refresh token record: {:#?}", e);
                panic!()
            }
        };

        match response {
            Some(t) => t,
            None => panic!("No token found for token_id: {}", token_id),
        }
    }

    #[instrument(skip(self, tran_conn))]
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

        let response = match query.await {
            Ok(r) => r,
            Err(e) => {
                tran_conn.cancel_tran().await;
                error!("Unable to get token id, cancelling transaction: {:#?}", e);
                panic!();
            }
        };

        let token_thing = match response {
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

    #[instrument(skip(self))]
    pub async fn set_signed_token(&self, token_id: String, signed_token: String) -> bool {
        let query = self
            .writer
            .query_single_with_args_specify_result::<Token, SetSignedTokenArgs>(
                r#"
                    UPDATE $token_id
                    SET
                        signed_token = $signed_token;
                    
                    SELECT * FROM nb_token WHERE token_id = $token_id FETCH meta;
                "#,
                SetSignedTokenArgs {
                    token_id: thing_from_string(&token_id),
                    signed_token,
                },
                1,
            );

        match query.await {
            Ok(_t) => true,
            Err(e) => {
                error!("Unable to set token for {}: {}", token_id, e);
                return false;
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn soft_delete_token_record(&self, token_id: &String) -> bool {
        let query = self.writer.query_none_with_args(
            r#"
                LET $meta_id = (SELECT meta FROM nb_token WHERE id = $token_id);
                UPDATE $meta_id.meta SET deleted_on = time::now();
            "#,
            ("token_id", thing_from_string(token_id)),
        );

        match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error deleting token: {:#?}", e);
                panic!()
            }
        }
    }

    #[instrument(skip(self, tran_conn))]
    pub async fn delete_all_sessions_for_person(
        &self,
        person_id: String,
        tran_conn: Option<&NovaDB>,
    ) -> bool {
        let conn = match tran_conn {
            Some(t) => t,
            None => &self.writer,
        };

        let query = conn.query_none_with_args(
            r#"
                UPDATE meta
                SET
                    deleted_on = time::now(),
                    deleted_by = $person_id
                WHERE id IN (SELECT meta FROM nb_token WHERE person = $person_id).meta;
            "#,
            ("person_id", thing_from_string(&person_id)),
        );

        match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error deleting all tokens for person: {:#?}", e);
                panic!()
            }
        }
    }
}
