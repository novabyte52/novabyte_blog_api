use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use tracing::{info, instrument};

use crate::{
    constants::SYSTEM_ID,
    db::{
        nova_db::{NovaDB, NovaResponse},
        SurrealDBConnection,
    },
    models::{
        person::{LogInCreds, Person, PersonCheck, PersonCheckResponse, SignUpState},
        token::{Token, TokenRecord},
    },
    repos::r_persons::PersonsRepo,
    utils::thing_from_string,
};

#[derive(Debug, Clone)]
pub struct PersonsService {
    repo: PersonsRepo,
    conn: SurrealDBConnection,
}

impl PersonsService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: PersonsRepo::new(),
            conn,
        }
    }

    #[instrument(skip(self))]
    pub async fn check_person_validity(&self, check: PersonCheck) -> PersonCheckResponse {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let resp_email = if let Some(email) = check.email.as_deref() {
            let mut r = db
                .exec(self.repo.query_is_unique_email(email))
                .await
                .expect("db query failed");
            Some(r.take_one::<bool>(0).unwrap_or(false))
        } else {
            None
        };

        let resp_user = if let Some(username) = check.username.as_deref() {
            let mut r = db
                .exec(self.repo.query_is_unique_username(username))
                .await
                .expect("db query failed");
            Some(r.take_one::<bool>(0).unwrap_or(false))
        } else {
            None
        };

        PersonCheckResponse {
            email: resp_email.unwrap_or(false),
            username: resp_user.unwrap_or(false),
        }
    }

    #[instrument(skip(self))]
    pub async fn sign_up(&self, mut sign_up_state: SignUpState) -> Person {
        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = argon2
            .hash_password(sign_up_state.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        sign_up_state.pass_hash = Some(password_hash);

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let q = self.repo.query_insert_person(sign_up_state, SYSTEM_ID);

        let tx = db.begin().await.expect("tx start failed");
        let mut resp: NovaResponse = tx
            .query(&q.sql)
            .bind(q.args)
            .await
            .expect("insert person failed")
            .into();
        tx.commit().await.expect("tx commit failed");

        // Statement indices (LET not counted, only CREATE/SELECT):
        //   0: CREATE meta
        //   1: CREATE person
        //   2: SELECT person with meta join
        resp.take_one::<Person>(2).expect("insert person failed")
    }

    #[instrument(skip(self))]
    pub async fn log_in_with_creds(&self, creds: LogInCreds) -> Person {
        info!("s: log in");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_person_hash_by_email(&creds.email))
            .await
            .expect("db query failed");

        let pass_hash_row = resp
            .take_opt::<std::collections::HashMap<String, String>>(0)
            .expect("hash lookup failed");

        let pass_hash = PersonsRepo::extract_pass_hash(pass_hash_row);

        let parsed_hash = PasswordHash::new(&pass_hash).unwrap();
        let matches = Argon2::default()
            .verify_password(creds.password.as_bytes(), &parsed_hash)
            .is_ok();

        if !matches {
            panic!("passwords don't match!");
        }

        let mut resp2 = db
            .exec(self.repo.query_select_person_by_email(&creds.email))
            .await
            .expect("db query failed");

        resp2
            .take_one::<Person>(0)
            .expect("No person found for that email")
    }

    #[instrument(skip(self))]
    pub async fn create_refresh_token(&self, person_id: String) -> TokenRecord {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let tx = db.begin().await.expect("tx start failed");

        // Delete all existing sessions for this person
        let q_del = self.repo.query_delete_all_sessions_for_person(&person_id);
        let _ = tx
            .query(&q_del.sql)
            .bind(q_del.args)
            .await
            .expect("delete sessions failed");

        // Create new meta + token record, return the token RecordId
        let q_ins = self.repo.query_insert_token_record(&person_id);
        let mut resp_ins: NovaResponse = tx
            .query(&q_ins.sql)
            .bind(q_ins.args)
            .await
            .expect("insert token failed")
            .into();

        tx.commit().await.expect("tx commit failed");

        // Statement indices in query_insert_token_record:
        //   0: CREATE meta
        //   1: CREATE token
        //   2: RETURN fn::string_id($token_id) → String
        let token_id_str = resp_ins
            .take_one::<String>(2)
            .expect("token id not returned");

        // Select the token record with meta join
        let mut resp_token = db
            .exec(self.repo.query_select_token_record(&token_id_str))
            .await
            .expect("select token failed");

        let token: Token = resp_token.take_one(0).expect("token not found");

        // token.meta is already fully joined via select_meta_string in query_select_token_record
        TokenRecord {
            id: token.id,
            person: token.person,
            created_by: thing_from_string(&token.meta.created_by),
            created_on: token.meta.created_on,
            deleted_on: token.meta.deleted_on,
            meta: thing_from_string(&token.meta.id),
        }
    }

    #[instrument(skip(self))]
    pub async fn logout(&self, person: Person) {
        self.logout_by_id(person.id).await;
    }

    #[instrument(skip(self))]
    pub async fn logout_by_id(&self, person_id: String) {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_delete_all_sessions_for_person(&person_id))
            .await
            .expect("db query failed");

        let _ = resp.take_one::<bool>(0).unwrap_or(true);
    }

    #[instrument(skip(self))]
    pub async fn get_token_record(&self, token_id: String) -> Token {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_token_record(&token_id))
            .await
            .expect("db query failed");

        resp.take_one::<Token>(0).expect("token not found")
    }

    #[instrument(skip(self, signed_token))]
    pub async fn set_signed_token(&self, token_id: String, signed_token: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_set_signed_token(&token_id, &signed_token))
            .await
            .expect("db query failed");

        // Statement indices: 0=UPDATE token, 1=SELECT token
        resp.take_opt::<Token>(0)
            .map(|o| o.is_some())
            .unwrap_or(false)
    }

    #[instrument(skip(self))]
    pub async fn soft_delete_token_record(&self, token_id: String) {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let _ = db
            .exec(self.repo.query_soft_delete_token_record(&token_id))
            .await
            .expect("db query failed");
    }

    #[instrument(skip(self))]
    pub async fn get_person(&self, person_id: String) -> Option<Person> {
        info!("s: get person");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_person(&person_id))
            .await
            .expect("db query failed");

        info!("get_person response: {:#?}", &resp);

        resp.take_opt::<Person>(0).unwrap_or(None)
    }

    #[instrument(skip(self))]
    pub async fn get_persons(&self) -> Vec<Person> {
        info!("s: get persons");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_persons())
            .await
            .expect("db query failed");

        resp.take_vec::<Person>(0).unwrap_or_default()
    }

    #[instrument(skip(self))]
    pub async fn invalidate_refresh(&self, person_id: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_delete_all_sessions_for_person(&person_id))
            .await
            .expect("db query failed");

        resp.take_one::<bool>(0).unwrap_or(false)
    }
}
