use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use tracing::{info, instrument};

use crate::{
    constants::SYSTEM_ID,
    db::{
        nova_db::{DbProgram, NovaDB},
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
        let exec = db.executor();

        // This was previously a repo method doing multiple awaits.
        // Under Pattern A: execute the programs you need.
        let mut resp_email = if let Some(email) = check.email.as_deref() {
            let program = self.repo.program_is_unique_email(email);
            let mut r = exec.run(program).await.expect("db query failed");
            Some(r.take_one::<bool>(0).unwrap_or(false))
        } else {
            None
        };

        let mut resp_user = if let Some(username) = check.username.as_deref() {
            let program = self.repo.program_is_unique_username(username);
            let mut r = exec.run(program).await.expect("db query failed");
            Some(r.take_one::<bool>(0).unwrap_or(false))
        } else {
            None
        };

        PersonCheckResponse {
            email: resp_email.take().unwrap_or(false),
            username: resp_user.take().unwrap_or(false),
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
        let exec = db.executor();

        // Create person is a program that returns Person (recommended run_tx)
        let program = self
            .repo
            .program_insert_person(sign_up_state, &String::from(SYSTEM_ID));
        let mut resp = exec.run_tx(program).await.expect("tx failed");

        // tx indices: 0 BEGIN, 1 meta.create, 2 person.create(return), 3 COMMIT
        resp.take_one::<Person>(2).expect("insert person failed")
    }

    #[instrument(skip(self))]
    pub async fn log_in_with_creds(&self, creds: LogInCreds) -> Person {
        info!("s: log in");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        // 1) fetch pass hash
        let program = self.repo.program_select_person_hash_by_email(&creds.email);
        let mut resp = exec.run(program).await.expect("db query failed");

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

        // 2) fetch person
        let program = self.repo.program_select_person_by_email(&creds.email);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_one::<Person>(0)
            .expect("No person found for that email")
    }

    #[instrument(skip(self))]
    pub async fn create_refresh_token(&self, person_id: String) -> TokenRecord {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        // Transaction:
        // - invalidate all sessions for person
        // - create meta + token and return token_id
        // - (optional) return hydrated TokenRecord (we'll do this in Rust using additional selects)
        //
        // You *can* do full hydration in one query too, but keeping it simple/semantic here.

        let program = DbProgram::new()
            .extend(self.repo.program_delete_all_sessions_for_person(&person_id))
            .extend(self.repo.program_insert_token_record(&person_id));

        let mut resp = exec.run_tx(program).await.expect("tx failed");

        info!("refresh token response: {:#?}", &resp);

        // Need the token_id Thing returned by program_insert_token_record.
        // Indices depend on composition; easiest rule:
        // - tx adds BEGIN at 0
        // - first program ops occupy 1..k
        // - second program's RETURN is after that
        //
        // We’ll just compute it: delete_all_sessions has 1 op, insert_token_record has 2 ops (meta.create + RETURN token_id)
        // In tx: BEGIN(0), delete_all(1), meta.create(2), token.create/return(3), COMMIT(4)
        let token_thing: surrealdb::sql::Thing = resp.take_one(0).expect("token id not returned");

        // Now select token record (read) and build TokenRecord (with meta lookup).
        // You can also move this into a single program later if you want.

        let mut resp_token = exec
            .run(
                self.repo
                    .program_select_token_record(&token_thing.to_string()),
            )
            .await
            .expect("select token failed");

        let token: Token = resp_token.take_one(0).expect("token not found");

        // If you still want Meta<> hydration like before, do it with MetaRepo program,
        // or simplify TokenRecord to not require meta read. Here we keep your old behavior:
        let meta_repo = crate::repos::r_meta::MetaRepo::new();
        let mut resp_meta = exec
            .run(meta_repo.program_select_meta(&token.meta.id))
            .await
            .expect("select meta failed");

        let meta: crate::models::meta::Meta<()> = resp_meta.take_one(0).expect("meta not found");

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
    pub async fn logout(&self, person: Person) {
        self.logout_by_id(person.id).await;
    }

    #[instrument(skip(self))]
    pub async fn logout_by_id(&self, person_id: String) {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_delete_all_sessions_for_person(&person_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        // program returns true
        let _ = resp.take_one::<bool>(0).unwrap_or(true);
    }

    #[instrument(skip(self))]
    pub async fn get_token_record(&self, token_id: String) -> Token {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_token_record(&token_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_one::<Token>(0).expect("token not found")
    }

    #[instrument(skip(self, signed_token))]
    pub async fn set_signed_token(&self, token_id: String, signed_token: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_set_signed_token(&token_id, &signed_token);
        let mut resp = exec.run(program).await.expect("db query failed");

        // if it returns Token, we just confirm it exists
        resp.take_opt::<Token>(0)
            .map(|o| o.is_some())
            .unwrap_or(false)
    }

    #[instrument(skip(self))]
    pub async fn soft_delete_token_record(&self, token_id: String) {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_soft_delete_token_record(&token_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        let _ = resp.take_one::<bool>(0).unwrap_or(true);
    }

    #[instrument(skip(self))]
    pub async fn get_person(&self, person_id: String) -> Option<Person> {
        info!("s: get person");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_person(&person_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        info!("get_person response: {:#?}", &resp);

        resp.take_opt::<Person>(0).unwrap_or(None)
    }

    #[instrument(skip(self))]
    pub async fn get_persons(&self) -> Vec<Person> {
        info!("s: get persons");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_persons();
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_vec::<Person>(0).unwrap_or_default()
    }

    #[instrument(skip(self))]
    pub async fn invalidate_refresh(&self, person_id: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_delete_all_sessions_for_person(&person_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_one::<bool>(0).unwrap_or(false)
    }
}
