use nb_lib::{
    models::{person::Person, post::DraftPostArgs},
    services::s_posts,
};

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use surrealdb::sql::Thing;

// pub async fn post_post(
//     current_person: Extension<Person>,
//     new_post: Json<PostPostArgs>,
// ) -> impl IntoResponse {
//     println!("c: create post");
//     s_posts::create_post(new_post.0, current_person.id.clone()).await;
// }

// pub async fn get_post(post_id: Result<Path<Thing>, PathRejection>) -> impl IntoResponse {
//     println!("c: get post");

//     let id = match post_id {
//         Ok(id) => id,
//         Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
//     };

//     println!("c: post id - {:#?}", id);

//     let generated_id = s_posts::get_post(id.0).await;
//     Ok(Json(generated_id))
// }

// pub async fn get_posts() -> impl IntoResponse {
//     println!("c: get posts");

//     let posts = s_posts::get_posts().await;

//     Json(posts)
// }

pub async fn get_published_posts() -> impl IntoResponse {
    Json(s_posts::get_published_posts().await)
}

pub async fn get_current_version(post_id: Thing) -> impl IntoResponse {
    // TODO: get most recent version of a post, i.e. most recent drafted or published
}

pub async fn get_current_versions() -> impl IntoResponse {
    // TODO: get most recent versions of every post, i.e. most recent drafted or published
}

pub async fn draft_post(
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    println!("c: draft post");
    s_posts::draft_post(draft_post.0, current_person.id.clone()).await;

    StatusCode::NO_CONTENT
}

// pub async fn get_drafted_posts() -> impl IntoResponse {
//     println!("c: get drafted posts");

//     let posts = s_posts::get_published_posts().await;

//     Json(posts)
// }

pub async fn publish_post(
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    println!("c: publish post");
    s_posts::publish_post(draft_post.0, current_person.id.clone()).await;

    StatusCode::NO_CONTENT
}
