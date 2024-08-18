use futures::future::join_all;
use tracing::{info, instrument};
// use itertools::Itertools;

use crate::db::nova_db::{get_tran_connection, NovaDB};
use crate::models::post::{DraftPostArgs, PostHydrated, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

/// Create a new post.
#[instrument]
async fn create_post(created_by: String, tran_conn: Option<&NovaDB>) -> Post {
    info!("s: create post");

    if let Some(conn) = tran_conn {
        return PostsRepo::new().await.insert_post(created_by, conn).await;
    };

    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    PostsRepo::new()
        .await
        .insert_post(created_by, &tran_conn)
        .await
}

/// Get a post.
#[instrument]
pub async fn get_post(post_id: String) -> Post {
    info!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

/// Get all posts.
#[instrument]
pub async fn get_posts() -> Vec<PostHydrated> {
    info!("s: get posts");
    PostsRepo::new().await.select_posts().await
}

/// Get all drafts for the given post id.
#[instrument]
pub async fn get_post_drafts(post_id: String) -> Vec<PostVersion> {
    info!("s: get post drafts");
    PostsRepo::new().await.select_post_drafts(post_id).await
}

#[instrument]
pub async fn get_draft(draft_id: String) -> PostVersion {
    PostsRepo::new().await.select_draft(&draft_id).await
}

/*
TODO: i should probably pass the optional post_id as an argument
instead of obscuring it in the DraftPostArgs object
*/
/// Create a new draft for a post.
///
/// If the id (a post id in this case) is not present create a new post
/// and then create a new draft for that post.
#[instrument]
pub async fn create_draft(draft: DraftPostArgs, author_id: String) -> PostVersion {
    let repo = PostsRepo::new().await;

    // if a post id exists create a new draft for the post and return
    if let Some(post_id) = draft.id {
        info!(
            "post id exists on draft! adding draft to post: {:#?}",
            &post_id
        );
        let post = get_post(post_id).await;

        return repo
            .create_draft(
                post.id as String,
                draft.title,
                draft.markdown,
                author_id.clone(),
                draft.published,
                None,
            )
            .await;
    };

    // no post id so create a new post
    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    let new_post = create_post(author_id.clone(), Some(&tran_conn)).await;

    let new_draft = repo
        .create_draft(
            new_post.id,
            draft.title,
            draft.markdown,
            author_id,
            draft.published,
            Some(&tran_conn),
        )
        .await;

    tran_conn.commit_tran().await;

    new_draft
}

// TODO: rename to get_drafts
/// Gets all current draft versions of any post that is not currently published
#[instrument]
pub async fn get_drafted_posts() -> Vec<PostVersion> {
    let unpublished_post_ids = PostsRepo::new().await.select_unpublished_post_ids().await;

    join_all(unpublished_post_ids.into_iter().map(|p| {
        return get_current_draft(p);
    }))
    .await
}

/// Gets the most recent draft for the given post id.
#[instrument]
pub async fn get_current_draft(post_id: String) -> PostVersion {
    PostsRepo::new().await.select_current_draft(post_id).await
}

// TODO: rename to create_published_draft
/// Create a new draft that is published for a post.
///
/// If the id (a post id in this case) is not present create a new post
/// and then create a new draft for that post that is published.
#[instrument]
pub async fn publish_new_draft(draft: DraftPostArgs, author: String) -> bool {
    // TODO: make sure there are no other published drafts
    // for this post before publishing this one
    let repo = PostsRepo::new().await;

    // if a post id exists publish the new draft and return
    if let Some(post_id) = draft.id {
        let post = get_post(post_id).await;

        repo.publish_new_draft(post.id, draft.title, draft.markdown, author, None)
            .await;
        return true;
    };

    // no post id exists so create a post and then publish the new draft
    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    let new_post = create_post(author.clone(), Some(&tran_conn)).await;

    repo.publish_new_draft(
        new_post.id,
        draft.title,
        draft.markdown,
        author,
        Some(&tran_conn),
    )
    .await;

    tran_conn.commit_tran().await;
    return true;
}

/// Publish an already existing draft by passing the draft id.
#[instrument]
pub async fn publish_draft(draft_id: String) -> bool {
    let repo = PostsRepo::new().await;

    let post_id = repo.select_post_id_for_draft_id(&draft_id).await;

    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    /*
    unpublishing all drafts first to ensure there are never two published drafts for a post.
    i doubt the amount of drafts per post will ever go beyond at most 50 so this should be fine.
    */
    if repo.unpublish_drafts_for_post_id(post_id, &tran_conn).await {
        repo.publish_draft(draft_id, &tran_conn).await;
        return true;
    }

    false
}

// TODO: rename to get_published_drafts
/// Gets all current published versions of any post that has a published draft.
#[instrument]
pub async fn get_published_posts() -> Vec<PostVersion> {
    PostsRepo::new().await.select_published_posts().await
}

// TODO: rename to unpublish_draft
/// Unpublish the draft of the given draft id.
#[instrument]
pub async fn unpublish_post(draft_id: String) -> bool {
    info!("s: unpublish post");
    PostsRepo::new().await.unpublish_draft(draft_id).await;
    true
}
