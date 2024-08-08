use nb_lib::{
    models::{person::Person, post::DraftPostArgs},
    services::s_posts,
};

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};

pub async fn get_posts() -> impl IntoResponse {
    println!("c: get post");
    let posts = s_posts::get_posts().await;
    Json(posts)
}

pub async fn get_post_drafts(Path(post_id): Path<String>) -> impl IntoResponse {
    println!("c: get {:#?} drafts", &post_id);

    Json(s_posts::get_post_drafts(post_id).await)
}

pub async fn draft_post(
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    println!("c: draft post {:#?}", draft_post.clone());

    s_posts::draft_post(draft_post.0.clone(), current_person.id.clone()).await;

    StatusCode::NO_CONTENT
}

pub async fn get_drafted_posts() -> impl IntoResponse {
    println!("c: get drafted posts");

    let posts = s_posts::get_drafted_posts().await;

    Json(posts)
}

pub async fn get_current_draft() -> impl IntoResponse {}

pub async fn publish_draft(Path(draft_id): Path<String>) -> impl IntoResponse {
    println!("c: publish post");
    s_posts::publish_draft(draft_id).await;

    StatusCode::NO_CONTENT
}

pub async fn get_published_posts() -> impl IntoResponse {
    println!("c: get published posts");

    let posts = s_posts::get_published_posts().await;

    Json(posts)
}

pub async fn unpublish_post(Path(draft_id): Path<String>) -> impl IntoResponse {
    println!("c: unpublish post");
    s_posts::unpublish_post(draft_id).await;

    StatusCode::NO_CONTENT
}
