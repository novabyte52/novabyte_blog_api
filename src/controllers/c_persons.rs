use std::env;

use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::Claims,
    reexports::coarsetime::Duration,
};
use nb_lib::{
    models::{
        custom_claims::CustomClaims,
        person::{LogInCreds, Person, SignUpCreds, SignUpState},
    },
    services::s_persons,
};
use surrealdb::sql::Thing;

pub async fn signup_person(Json(creds): Json<SignUpCreds>) -> impl IntoResponse {
    let new_person = s_persons::sign_up(SignUpState {
        username: creds.username,
        email: creds.email,
        password: creds.password,
        pass_hash: None,
    })
    .await;

    Json(new_person)
}

pub async fn login_person(Json(creds): Json<LogInCreds>) -> impl IntoResponse {
    let person = s_persons::log_in_with_creds(creds).await;

    generate_token(person)
}

pub async fn get_person(person_id: Result<Path<Thing>, PathRejection>) -> impl IntoResponse {
    println!("c: get person");

    let id = match person_id {
        Ok(id) => id,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    println!("c: person id - {:#?}", id);

    let generated_id = s_persons::get_person(id.0).await;
    Ok(Json(generated_id))
}

pub async fn get_persons(Extension(current_user): Extension<Person>) -> impl IntoResponse {
    println!("c: get persons");
    println!("current user: {:?}", current_user);

    let persons = s_persons::get_persons().await;

    println!("c: {:#?}", persons);

    Json(persons)
}

fn generate_token(person: Person) -> String {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");

    let key = HS256Key::from_bytes(secret.as_bytes());

    let custom_claims = CustomClaims {
        name: person.email,
        is_admin: true,
    };

    let claims = Claims::with_custom_claims(custom_claims, Duration::from_hours(1))
        .with_subject(person.id.id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}
