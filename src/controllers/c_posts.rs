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

/// GET endpoint to handle getting a draft based on the draft_id passed in the request url.
/// Sends the retrieved draft in the response body.
#[instrument]
pub async fn get_draft(Path(draft_id): Path<String>) -> impl IntoResponse {
    let draft = s_posts::get_draft(draft_id).await;

    Json(draft)
}

/// POST endpoint to handle the creation of a draft.
/// Sends the newly created draft in the response body.
#[instrument]
pub async fn handle_create_draft(
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    let new_draft = s_posts::create_draft(draft_post.0.clone(), current_person.id.clone()).await;

    Json(new_draft)
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
