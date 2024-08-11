use nb_lib::{
    models::{person::Person, post::DraftPostArgs},
    services::s_posts,
};

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use tracing::{info, instrument};

#[instrument]
pub async fn get_posts() -> impl IntoResponse {
    info!("c: get post");
    let posts = s_posts::get_posts().await;
    Json(posts)
}

#[instrument]
pub async fn get_post_drafts(Path(post_id): Path<String>) -> impl IntoResponse {
    info!("c: get {:#?} drafts", &post_id);

    Json(s_posts::get_post_drafts(post_id).await)
}

#[instrument]
pub async fn draft_post(
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    info!("c: draft post {:#?}", draft_post.clone());

    s_posts::draft_post(draft_post.0.clone(), current_person.id.clone()).await;

    StatusCode::NO_CONTENT
}

#[instrument]
pub async fn get_drafted_posts() -> impl IntoResponse {
    info!("c: get drafted posts");

    let posts = s_posts::get_drafted_posts().await;

    Json(posts)
}

#[instrument]
pub async fn get_current_draft() -> impl IntoResponse {}

#[instrument]
pub async fn publish_draft(Path(draft_id): Path<String>) -> impl IntoResponse {
    info!("c: publish post");
    s_posts::publish_draft(draft_id).await;

    StatusCode::NO_CONTENT
}

#[instrument]
pub async fn get_published_posts() -> impl IntoResponse {
    info!("c: get published posts");

    let posts = s_posts::get_published_posts().await;

    Json(posts)
}

#[instrument]
pub async fn unpublish_post(Path(draft_id): Path<String>) -> impl IntoResponse {
    info!("c: unpublish post");
    s_posts::unpublish_post(draft_id).await;

    StatusCode::NO_CONTENT
}
