use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use nb_lib::{models::person::{PostPerson, Creds}, services::s_persons};

use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ulid::Ulid;

pub async fn sign_up(Json(creds): Json<Creds>) -> impl IntoResponse {
    let argon2 = Argon2::default();
        
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2.hash_password(creds.password.as_bytes(), &salt).unwrap().to_string();
    
    // TODO: store new person
}

pub async fn login(Json(creds): Json<Creds>) -> impl IntoResponse {
    // TODO: create a way to fetch persons based on their login credentials
    let person = get_person(creds.password).await;

    // TODO: replace this parsed_hash var with the above persons pass_hash
    let parsed_hash = PasswordHash::new(&password_hash).unwrap();
    assert!(Argon2::default().verify_password(creds.password.as_bytes(), &parsed_hash).is_ok());
}

pub async fn post_person(Json(new_person): Json<PostPerson>) -> impl IntoResponse {
    println!("c: create persons - {:#?}", new_person);
    let foo = s_persons::create_person(new_person).await;
    Json(foo)
}

pub async fn get_person(person_id: Result<Path<Ulid>, PathRejection>) -> impl IntoResponse {
    println!("c: get person");

    let id = match person_id {
        Ok(id) => id,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    println!("c: person id - {:#?}", id);

    let generated_id = s_persons::get_person(id.0.to_string().into()).await;
    Ok(Json(generated_id))
}

pub async fn get_persons() -> impl IntoResponse {
    println!("c: get persons");

    let persons = s_persons::get_persons().await;

    println!("c: {:#?}", persons);

    Json(persons)
}
