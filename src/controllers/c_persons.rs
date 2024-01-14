use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use nb_lib::{
    models::person::{Creds, SignUpState},
    services::s_persons,
};
use ulid::Ulid;

// was named sign_up() but to stick with conventions for now it will be post_person
pub async fn post_person(Json(creds): Json<Creds>) -> impl IntoResponse {
    let new_person = s_persons::sign_up(SignUpState {
        username: creds.username,
        email: creds.email,
        password: creds.password,
        pass_hash: None,
    })
    .await;

    Json(new_person)
}

pub async fn login(Json(_creds): Json<Creds>) -> impl IntoResponse {
    // TODO: create a way to fetch persons based on their login credentials
    // let person = get_person(creds.password).await;

    // TODO: replace this parsed_hash var with the above persons pass_hash
    // let parsed_hash = PasswordHash::new(&password_hash).unwrap();
    // assert!(Argon2::default().verify_password(creds.password.as_bytes(), &parsed_hash).is_ok());
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
