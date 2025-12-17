use std::collections::HashMap;

use surrealdb::sql::Thing;
use time::OffsetDateTime;

use crate::db::nova_db::{DbOp, DbProgram};
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

    /// Program: check whether email is unique (returns bool).
    pub fn program_is_unique_email(&self, email: &str) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "person.unique_email",
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
            ))
            .bind("email", email)
            .expect("bind email")
    }

    /// Program: check whether username is unique (returns bool).
    pub fn program_is_unique_username(&self, username: &str) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "person.unique_username",
                r#"
                LET $count = (
                    SELECT count(username)
                    FROM ONLY person
                    WHERE username = $username
                    LIMIT 1
                ).count;

                RETURN $count IS NONE;
                "#,
            ))
            .bind("username", username)
            .expect("bind username")
    }

    /// Program: select person by id (returns Person).
    pub fn program_select_person(&self, person_id: &String) -> DbProgram {
        let q = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM ONLY person WHERE id = $id LIMIT 1;",
            self.meta.select_meta_string
        );

        DbProgram::new()
            .op(DbOp::new("person.select_by_id", q))
            .bind_thing("id", thing_from_string(person_id))
    }

    /// Program: select person by email (returns Person).
    pub fn program_select_person_by_email(&self, email: &str) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("person.select_by_email", q))
            .bind("email", email)
            .expect("bind email")
    }

    /// Program: select person hash by email (returns pass_hash string inside a map).
    pub fn program_select_person_hash_by_email(&self, email: &str) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "person.select_pass_hash",
                r#"SELECT pass_hash FROM ONLY person WHERE email = $email LIMIT 1;"#,
            ))
            .bind("email", email)
            .expect("bind email")
    }

    /// Program: select all persons (returns Vec<Person>).
    pub fn program_select_persons(&self) -> DbProgram {
        let q = format!(
            "SELECT fn::string_id(id) as id, *, {} FROM person;",
            self.meta.select_meta_string
        );
        DbProgram::new().op(DbOp::new("person.select_all", q))
    }

    /// Program: create a new person + meta (recommended to run in a transaction).
    /// Returns Person as last op (RETURN).
    pub fn program_insert_person(&self, new_person: SignUpState, created_by: &String) -> DbProgram {
        let pass_hash = new_person
            .pass_hash
            .clone()
            .expect("Can't create user without pass_hash");

        let create_user = format!(
            r#"
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
            self.meta.select_meta_string
        );

        DbProgram::new()
            .op(self.meta.op_create_meta("$meta_id"))
            .op(DbOp::new("person.create", create_user))
            .bind_thing("created_by", thing_from_string(created_by))
            .bind("email", new_person.email)
            .expect("bind email")
            .bind("username", new_person.username)
            .expect("bind username")
            .bind("pass_hash", pass_hash)
            .expect("bind pass_hash")
    }

    /// Program: select token record (returns Token).
    pub fn program_select_token_record(&self, token_id: &String) -> DbProgram {
        let token_query = format!(
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

        DbProgram::new()
            .op(DbOp::new("token.select", token_query))
            .bind_thing("id", thing_from_string(token_id))
    }

    /// Program: insert token record (meta + token) and RETURN the token id (Thing).
    pub fn program_insert_token_record(&self, person_id: &String) -> DbProgram {
        DbProgram::new()
            .op(self.meta.op_create_meta("$meta_id"))
            .op(DbOp::new(
                "token.create",
                r#"
                LET $token_id = nb_token:ulid();

                CREATE $token_id
                SET
                    person = $person,
                    meta = $meta_id;

                RETURN $token_id;
                "#,
            ))
            .bind_thing("created_by", thing_from_string(person_id))
            .bind_thing("person", thing_from_string(person_id))
    }

    /// Program: set signed token (returns Token).
    pub fn program_set_signed_token(&self, token_id: &String, signed_token: &str) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "token.set_signed",
                r#"
                UPDATE $token_id
                SET signed_token = $signed_token;

                SELECT * FROM ONLY nb_token WHERE id = $token_id LIMIT 1;
                "#,
            ))
            .bind_thing("token_id", thing_from_string(token_id))
            .bind("signed_token", signed_token)
            .expect("bind signed_token")
    }

    /// Program: soft-delete token record via meta.deleted_on (returns true).
    pub fn program_soft_delete_token_record(&self, token_id: &String) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "token.soft_delete",
                r#"
                LET $meta_id = (SELECT meta FROM ONLY nb_token WHERE id = $token_id LIMIT 1).meta;
                UPDATE $meta_id SET deleted_on = time::now();
                RETURN true;
                "#,
            ))
            .bind_thing("token_id", thing_from_string(token_id))
    }

    /// Program: delete all sessions for person (updates meta records) returns true.
    pub fn program_delete_all_sessions_for_person(&self, person_id: &String) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "token.delete_all_sessions_for_person",
                r#"
                UPDATE meta
                SET
                    deleted_on = time::now(),
                    deleted_by = $person_id
                WHERE id IN (SELECT meta FROM nb_token WHERE person = $person_id).meta
                RETURN true;
                "#,
            ))
            .bind_thing("person_id", thing_from_string(person_id))
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
        created_by: Thing,
        created_on: OffsetDateTime,
        deleted_on: Option<OffsetDateTime>,
        meta: Thing,
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
