use nb_lib::models::{person::Person, post::DraftPostArgs};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use tracing::{info, instrument};

use crate::middleware::NbBlogServices;

#[instrument]
pub async fn handle_get_random_post(State(services): State<NbBlogServices>) -> impl IntoResponse {
    let post = services.posts.get_random_post().await;
    Json(post)
}

#[instrument]
pub async fn get_posts(State(services): State<NbBlogServices>) -> impl IntoResponse + 'static {
    let posts = services.posts.get_posts().await;
    Json(posts)
}

#[instrument]
pub async fn get_post_drafts(
    State(services): State<NbBlogServices>,
    Path(post_id): Path<String>,
) -> impl IntoResponse {
    Json(services.posts.get_post_drafts(post_id).await)
}

/// GET endpoint to handle getting a draft based on the draft_id passed in the request url.
/// Sends the retrieved draft in the response body.
#[instrument]
pub async fn get_draft(
    State(services): State<NbBlogServices>,
    Path(draft_id): Path<String>,
) -> impl IntoResponse {
    let draft = services.posts.get_draft(draft_id).await;

    Json(draft)
}

/// POST endpoint to handle the creation of a draft.
/// Sends the newly created draft in the response body.
#[instrument]
pub async fn handle_create_draft(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    draft_post: Json<DraftPostArgs>,
) -> impl IntoResponse {
    let new_draft = services
        .posts
        .create_draft(draft_post.0.clone(), current_person.id.clone())
        .await;

    Json(new_draft)
}

#[instrument]
pub async fn get_drafted_posts(State(services): State<NbBlogServices>) -> impl IntoResponse {
    info!("c: get drafted posts");

    let posts = services.posts.get_drafted_posts().await;

    Json(posts)
}

#[instrument]
pub async fn get_current_draft() -> impl IntoResponse {}

#[instrument]
pub async fn publish_draft(
    State(services): State<NbBlogServices>,
    Path(draft_id): Path<String>,
) -> impl IntoResponse {
    services.posts.publish_draft(draft_id).await;

    StatusCode::NO_CONTENT
}

#[instrument(skip(services))]
pub async fn get_published_posts(State(services): State<NbBlogServices>) -> impl IntoResponse {
    let posts = services.posts.get_published_posts().await;

    Json(posts)
}

#[instrument]
pub async fn unpublish_post(
    State(services): State<NbBlogServices>,
    Path(draft_id): Path<String>,
) -> impl IntoResponse {
    services.posts.unpublish_post(draft_id).await;

    StatusCode::NO_CONTENT
}
