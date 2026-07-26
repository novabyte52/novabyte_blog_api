use std::collections::HashMap;

use surrealdb::types::RecordId;
use time::OffsetDateTime;

use crate::db::nova_db::NovaQuery;
use crate::models::person::SignUpState;
use crate::models::token::{Token, TokenRecord};
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

#[derive(Debug, Clone)]
pub struct PersonsRepo {
    meta: MetaRepo,
}

impl PersonsRepo {
    pub fn new() -> Self {
        Self {
            meta: MetaRepo::new(),
        }
    }

    /// Query: check whether email is unique (returns bool).
    pub fn query_is_unique_email(&self, email: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            IF string::is::email($email) {
                LET $count = (
                    SELECT count(email)
                    FROM ONLY person
                    WHERE email = $email
                    LIMIT 1
                ).count;

                RETURN $count IS NONE;
            } ELSE {
                RETURN false;
            };
            "#,
        )
        .bind("email", email)
    }

    /// Query: check whether username is unique (returns bool).
    pub fn query_is_unique_username(&self, username: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            LET $count = (
                SELECT count(username)
                FROM ONLY person
                WHERE username = $username
                LIMIT 1
            ).count;

            RETURN $count IS NONE;
            "#,
        )
        .bind("username", username)
    }

    /// Query: select person by id (returns Person).
    pub fn query_select_person(&self, person_id: &str) -> NovaQuery {
        let sql = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM ONLY person WHERE id = $id LIMIT 1;",
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("id", thing_from_string(person_id))
    }

    /// Query: select person by email (returns Person).
    pub fn query_select_person_by_email(&self, email: &str) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(id) as id,
                username,
                email,
                is_admin,
                {}
            FROM ONLY person
            WHERE email = $email
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("email", email)
    }

    /// Query: select person pass_hash by email (returns row with pass_hash field).
    pub fn query_select_person_hash_by_email(&self, email: &str) -> NovaQuery {
        NovaQuery::new("SELECT pass_hash FROM ONLY person WHERE email = $email LIMIT 1;")
            .bind("email", email)
    }

    /// Query: select all persons (returns Vec<Person>).
    pub fn query_select_persons(&self) -> NovaQuery {
        let sql = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM person;",
            self.meta.select_meta_string
        );
        NovaQuery::new(sql)
    }

    /// Query: create a new person + meta (run in a transaction).
    /// Multi-statement: creates meta, creates person, returns Person.
    pub fn query_insert_person(&self, new_person: SignUpState, created_by: &str) -> NovaQuery {
        let pass_hash = new_person
            .pass_hash
            .clone()
            .expect("Can't create user without pass_hash");

        let sql = format!(
            r#"
            {}
            LET $person_id = person:ulid();

            CREATE $person_id
            SET
                email = $email,
                username = $username,
                pass_hash = $pass_hash,
                is_admin = false,
                meta = $meta_id;

            SELECT
                fn::string_id(id) as id,
                *,
                {}
            FROM ONLY person
            WHERE id = $person_id
            LIMIT 1;
            "#,
            self.meta.sql_create_meta("$meta_id"),
            self.meta.select_meta_string
        );

        NovaQuery::new(sql)
            .bind("created_by", thing_from_string(created_by))
            .bind("email", new_person.email)
            .bind("username", new_person.username)
            .bind("pass_hash", pass_hash)
    }

    /// Query: select token record by id (returns Token).
    pub fn query_select_token_record(&self, token_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(id) as id,
                fn::string_id(person) as person,
                {}
            FROM ONLY nb_token
            WHERE id = $id
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("id", thing_from_string(token_id))
    }

    /// Query: create token record + meta (run in a transaction).
    /// Multi-statement: creates meta, creates token, returns token RecordId.
    pub fn query_insert_token_record(&self, person_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            LET $token_id = nb_token:ulid();

            CREATE $token_id
            SET
                person = $person,
                meta = $meta_id;

            RETURN fn::string_id($token_id);
            "#,
            self.meta.sql_create_meta("$meta_id")
        );
        NovaQuery::new(sql)
            .bind("created_by", thing_from_string(person_id))
            .bind("person", thing_from_string(person_id))
    }

    /// Query: set signed token string (returns Token).
    pub fn query_set_signed_token(&self, token_id: &str, signed_token: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            UPDATE $token_id
            SET signed_token = $signed_token;

            SELECT * FROM ONLY nb_token WHERE id = $token_id LIMIT 1;
            "#,
        )
        .bind("token_id", thing_from_string(token_id))
        .bind("signed_token", signed_token)
    }

    /// Query: soft-delete token via meta.deleted_on (returns true).
    pub fn query_soft_delete_token_record(&self, token_id: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            LET $meta_id = (SELECT meta FROM ONLY nb_token WHERE id = $token_id LIMIT 1).meta;
            UPDATE $meta_id SET deleted_on = time::now();
            RETURN true;
            "#,
        )
        .bind("token_id", thing_from_string(token_id))
    }

    /// Query: soft-delete all sessions for a person via meta.deleted_on (returns true).
    pub fn query_delete_all_sessions_for_person(&self, person_id: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            UPDATE meta
            SET
                deleted_on = time::now(),
                deleted_by = $person_id
            WHERE id IN (SELECT meta FROM nb_token WHERE person = $person_id).meta
            RETURN true;
            "#,
        )
        .bind("person_id", thing_from_string(person_id))
    }

    // ---- helpers ----

    pub fn extract_pass_hash(row: Option<HashMap<String, String>>) -> String {
        match row.and_then(|m| m.get("pass_hash").cloned()) {
            Some(h) => h,
            None => panic!("No person hash found"),
        }
    }

    pub fn make_token_record(
        token: Token,
        created_by: RecordId,
        created_on: OffsetDateTime,
        deleted_on: Option<OffsetDateTime>,
        meta: RecordId,
    ) -> TokenRecord {
        TokenRecord {
            id: token.id.to_string(),
            person: token.person.to_string(),
            created_by,
            created_on,
            deleted_on,
            meta,
        }
    }
}
