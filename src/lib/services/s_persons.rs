use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use tracing::{info, instrument};

use crate::{
    constants::SYSTEM_ID,
    db::nova_db::get_tran_connection,
    models::{
        person::{LogInCreds, Person, PersonCheck, PersonCheckResponse, SignUpState},
        token::{Token, TokenRecord},
    },
    repos::r_persons::PersonsRepo,
};

#[instrument]
pub async fn check_person_validity(check: PersonCheck) -> PersonCheckResponse {
    let mut check_response = PersonCheckResponse {
        email: false,
        username: false,
    };

    let repo = PersonsRepo::new().await;

    if let Some(email) = &check.email {
        check_response.email = repo.is_unique_email(email).await;
    }

    if let Some(username) = &check.username {
        check_response.username = repo.is_unique_username(username).await;
    }

    check_response
}

#[instrument]
pub async fn sign_up(mut sign_up_state: SignUpState) -> Person {
    let argon2 = Argon2::default();

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(sign_up_state.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    sign_up_state.pass_hash = Some(password_hash);

    PersonsRepo::new()
        .await
        .insert_person(sign_up_state, SYSTEM_ID, &get_tran_connection().await)
        .await
}

#[instrument]
pub async fn log_in_with_creds(creds: LogInCreds) -> Person {
    info!("s: log in");
    let pass_hash = PersonsRepo::new()
        .await
        .select_person_hash_by_email(creds.email.clone())
        .await;

    let parsed_hash = PasswordHash::new(&pass_hash).unwrap();
    let matches = Argon2::default()
        .verify_password(creds.password.as_bytes(), &parsed_hash)
        .is_ok();

    if !matches {
        panic!("passwords don't match!");
    }

    match PersonsRepo::new()
        .await
        .select_person_by_email(creds.email)
        .await
    {
        Some(p) => p,
        None => panic!("No person found for that email"),
    }
}

#[instrument]
pub async fn create_refresh_token(person_id: String) -> TokenRecord {
    let tran_conn = get_tran_connection().await;

    let repo = PersonsRepo::new().await;

    // make sure all previous tokens are invalidated before issuing a new one
    let success = repo
        .delete_all_sessions_for_person(person_id.clone(), Some(&tran_conn))
        .await;

    if success {
        repo.insert_token_record(person_id, &tran_conn).await
    } else {
        panic!("Unable to invalidate previous sessions for user {}. Unable to issue new refresh token.", person_id)
    }
}

#[instrument]
pub async fn logout(person: Person) {
    PersonsRepo::new()
        .await
        .delete_all_sessions_for_person(person.id, None)
        .await;
}

#[instrument]
pub async fn get_token_record(token_id: String) -> Token {
    PersonsRepo::new().await.select_token_record(token_id).await
}

#[instrument]
pub async fn soft_delete_token_record(token_id: String) {
    PersonsRepo::new()
        .await
        .soft_delete_token_record(&token_id)
        .await;
}

#[instrument]
pub async fn get_person(person_id: String) -> Option<Person> {
    info!("s: get person");
    PersonsRepo::new().await.select_person(person_id).await
}

#[instrument]
pub async fn get_persons() -> Vec<Person> {
    info!("s: get persons");
    let foo = PersonsRepo::new().await.select_persons().await;
    info!("s: {:#?}", foo);
    foo
}
