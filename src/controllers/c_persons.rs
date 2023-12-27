use nb_lib::services::s_posts;

use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ulid::Ulid;

pub async fn post_person() -> impl IntoResponse {
    println!("c: create post");
    let foo = s_posts::create_post().await;
    Json(foo)
}

pub async fn get_person(person_id: Result<Path<Ulid>, PathRejection>) -> impl IntoResponse {
    println!("c: get person");

    let id = match person_id {
        Ok(id) => id,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    println!("c: person id - {:#?}", id);

    let generated_id = s_persons::get_person(id.0.to_owned()).await;
    Ok(Json(generated_id))
}

pub async fn get_persons() -> impl IntoResponse {
    println!("c: get persons");

    let persons = s_persons::get_persons().await;

    println!("c: {:#?}", persons);

    Json(persons)
}
