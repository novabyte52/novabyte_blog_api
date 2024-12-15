use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use tracing::{info, instrument};

use crate::{
    constants::SYSTEM_ID,
    db::{nova_db::get_tran_connection, SurrealDBConnection},
    models::{
        person::{LogInCreds, Person, PersonCheck, PersonCheckResponse, SignUpState},
        token::{Token, TokenRecord},
    },
    repos::r_persons::PersonsRepo,
};

#[derive(Debug, Clone)]
pub struct PersonsService {
    repo: PersonsRepo,
    conn: SurrealDBConnection,
}

impl PersonsService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: PersonsRepo::new(&conn).await,
            conn,
        }
    }

    #[instrument(skip(self))]
    pub async fn check_person_validity(&self, check: PersonCheck) -> PersonCheckResponse {
        self.repo.is_person_unique(check).await
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

        self.repo
            .insert_person(
                sign_up_state,
                SYSTEM_ID,
                &get_tran_connection(&self.conn).await,
            )
            .await
    }

    #[instrument(skip(self))]
    pub async fn log_in_with_creds(&self, creds: LogInCreds) -> Person {
        info!("s: log in");
        let pass_hash = self
            .repo
            .select_person_hash_by_email(creds.email.clone())
            .await;

        let parsed_hash = PasswordHash::new(&pass_hash).unwrap();
        let matches = Argon2::default()
            .verify_password(creds.password.as_bytes(), &parsed_hash)
            .is_ok();

        if !matches {
            panic!("passwords don't match!");
        }

        match self.repo.select_person_by_email(creds.email).await {
            Some(p) => p,
            None => panic!("No person found for that email"),
        }
    }

    #[instrument(skip(self))]
    pub async fn create_refresh_token(&self, person_id: String) -> TokenRecord {
        let tran_conn = get_tran_connection(&self.conn).await;

        // make sure all previous tokens are invalidated before issuing a new one
        let success = self
            .repo
            .delete_all_sessions_for_person(person_id.clone(), Some(&tran_conn))
            .await;

        if success {
            self.repo.insert_token_record(person_id, &tran_conn).await
        } else {
            panic!(
                "Unable to invalidate previous sessions for {}. Unable to issue new refresh token.",
                person_id
            )
        }
    }

    #[instrument(skip(self))]
    pub async fn logout(&self, person: Person) {
        self.repo
            .delete_all_sessions_for_person(person.id, None)
            .await;
    }

    #[instrument(skip(self))]
    pub async fn logout_by_id(&self, person_id: String) {
        self.repo
            .delete_all_sessions_for_person(person_id, None)
            .await;
    }

    #[instrument(skip(self))]
    pub async fn get_token_record(&self, token_id: String) -> Token {
        self.repo.select_token_record(token_id).await
    }

    #[instrument(skip(self, signed_token))]
    pub async fn set_signed_token(&self, token_id: String, signed_token: String) -> bool {
        self.repo.set_signed_token(token_id, signed_token).await
    }

    #[instrument(skip(self))]
    pub async fn soft_delete_token_record(&self, token_id: String) {
        self.repo.soft_delete_token_record(&token_id).await;
    }

    #[instrument(skip(self))]
    pub async fn get_person(&self, person_id: String) -> Option<Person> {
        info!("s: get person");
        self.repo.select_person(person_id).await
    }

    #[instrument(skip(self))]
    pub async fn get_persons(&self) -> Vec<Person> {
        info!("s: get persons");
        let foo = self.repo.select_persons().await;
        info!("s: {:#?}", foo);
        foo
    }

    #[instrument(skip(self))]
    pub async fn invalidate_refresh(&self, person_id: String) -> bool {
        self.repo
            .delete_all_sessions_for_person(person_id, None)
            .await
    }
}
