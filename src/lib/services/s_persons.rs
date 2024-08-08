use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};

use crate::{
    constants::Constants,
    models::{
        person::{LogInCreds, Person, SignUpState},
        token::{Token, TokenRecord},
    },
    repos::r_persons::PersonsRepo,
};

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
        .insert_person(sign_up_state, Constants::system_thing())
        .await
}

pub async fn log_in_with_creds(creds: LogInCreds) -> Person {
    println!("s: log in");
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

pub async fn create_refresh_token(person_id: String) -> TokenRecord {
    PersonsRepo::new()
        .await
        .insert_token_record(person_id)
        .await
}

pub async fn get_token_record(token_id: String) -> Token {
    PersonsRepo::new().await.select_token_record(token_id).await
}

pub async fn soft_delete_token_record(token_id: String) {
    PersonsRepo::new()
        .await
        .soft_delete_token_record(&token_id)
        .await
}

pub async fn get_person(person_id: String) -> Option<Person> {
    println!("s: get person");
    PersonsRepo::new().await.select_person(person_id).await
}

pub async fn get_persons() -> Vec<Person> {
    println!("s: get persons");
    let foo = PersonsRepo::new().await.select_persons().await;
    println!("s: {:#?}", foo);
    foo
}
