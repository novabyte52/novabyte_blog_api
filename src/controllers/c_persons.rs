use nb_lib::{models::person::PostPerson, services::s_persons};

use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ulid::Ulid;

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
