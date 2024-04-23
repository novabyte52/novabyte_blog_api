use nb_lib::{
    models::{person::Person, post::PostPostArgs},
    services::s_posts,
};

use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use ulid::Ulid;

pub async fn post_post(
    current_person: Extension<Person>,
    new_post: Json<PostPostArgs>,
) -> impl IntoResponse {
    println!("c: create post");
    s_posts::create_post(new_post.0, current_person.id.clone()).await;
}

pub async fn get_post(post_id: Result<Path<Ulid>, PathRejection>) -> impl IntoResponse {
    println!("c: get post");

    let id = match post_id {
        Ok(id) => id,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    println!("c: post id - {:#?}", id);

    let generated_id = s_posts::get_post(id.0.to_owned()).await;
    Ok(Json(generated_id))
}

pub async fn get_posts() -> impl IntoResponse {
    println!("c: get posts");

    let posts = s_posts::get_posts().await;

    Json(posts)
}
