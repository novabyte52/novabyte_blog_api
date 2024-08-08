use futures::future::join_all;
use itertools::Itertools;

use crate::db::nova_db::{get_tran_connection, NovaDB};
use crate::models::post::{DraftPostArgs, PostHydrated, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

/// Create a new post.
async fn create_post(created_by: String, tran_conn: Option<&NovaDB>) -> Post {
    println!("s: create post");

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
pub async fn get_post(post_id: String) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

/// Get all posts.
pub async fn get_posts() -> Vec<PostHydrated> {
    println!("s: get posts");
    PostsRepo::new().await.select_posts().await
}

/// Get all drafts for the given post id.
pub async fn get_post_drafts(post_id: String) -> Vec<PostVersion> {
    println!("s: get post drafts");
    PostsRepo::new().await.select_post_drafts(post_id).await
}

/*
TODO: rename to create_draft.
and i should probably pass the optional post_id as an argument
instead of obscuring it in the DraftPostArgs object
*/
/// Create a new draft for a post.
///
/// If the id (a post id in this case) is not present create a new post
/// and then create a new draft for that post.
pub async fn draft_post(draft: DraftPostArgs, author: String) -> bool {
    println!("s: draft post {:#?}", draft.clone());
    let repo = PostsRepo::new().await;

    // if a post id exists create a new draft for the post and return
    if let Some(post_id) = draft.id {
        println!(
            "post id exists on draft! adding draft to post: {:#?}",
            &post_id
        );
        let post = get_post(post_id).await;

        repo.draft_post(
            post.id as String,
            draft.title,
            draft.markdown,
            author.clone(),
            draft.published,
        )
        .await;
        return true;
    };

    // no post id so create a new post
    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    let new_post = create_post(author.clone(), Some(&tran_conn)).await;

    repo.draft_post(
        new_post.id,
        draft.title,
        draft.markdown,
        author,
        draft.published,
    )
    .await;

    tran_conn.commit_tran().await;
    return true;
}

// TODO: rename to get_drafts
/// Gets all current draft versions of any post that is not currently published
pub async fn get_drafted_posts() -> Vec<PostVersion> {
    let all_drafts = PostsRepo::new().await.select_drafted_posts().await;

    let unique_draft_ids = all_drafts
        .into_iter()
        .map(|p| p.id)
        .unique_by(|id| id.clone())
        .collect::<Vec<String>>();

    join_all(unique_draft_ids.clone().into_iter().map(|p| async {
        return get_current_draft(p).await;
    }))
    .await
}

/// Gets the most recent draft for the given post id.
pub async fn get_current_draft(post_id: String) -> PostVersion {
    PostsRepo::new().await.select_current_draft(post_id).await
}

// TODO: rename to create_published_draft
/// Create a new draft that is published for a post.
///
/// If the id (a post id in this case) is not present create a new post
/// and then create a new draft for that post that is published.
pub async fn publish_new_draft(draft: DraftPostArgs, author: String) -> bool {
    // TODO: make sure there are no other published drafts
    // for this post before publishing this one
    let repo = PostsRepo::new().await;

    // if a post id exists publish the new draft and return
    if let Some(post_id) = draft.id {
        let post = get_post(post_id).await;

        repo.publish_new_draft(post.id, draft.title, draft.markdown, author)
            .await;
        return true;
    };

    // no post id exists so create a post and then publish the new draft
    let tran_conn = get_tran_connection().await;
    tran_conn.begin_tran().await;

    let new_post = create_post(author.clone(), Some(&tran_conn)).await;

    repo.publish_new_draft(new_post.id, draft.title, draft.markdown, author)
        .await;

    tran_conn.commit_tran().await;
    return true;
}

/// Publish an already existing draft by passing the draft id.
pub async fn publish_draft(draft_id: String) -> bool {
    let repo = PostsRepo::new().await;

    let post_id = repo.select_post_id_for_draft_id(&draft_id).await;

    /*
    unpublishing all drafts first to ensure there are never two published drafts for a post.
    i doubt the amount of drafts per post will ever go beyond at most 50 so this should be fine.
    */
    if repo.unpublish_drafts_for_post_id(post_id).await {
        repo.publish_draft(draft_id).await;
        return true;
    }

    false
}

// TODO: rename to get_published_drafts
/// Gets all current published versions of any post that has a published draft.
pub async fn get_published_posts() -> Vec<PostVersion> {
    PostsRepo::new().await.select_published_posts().await
}

// TODO: rename to unpublish_draft
/// Unpublish the draft of the given draft id.
pub async fn unpublish_post(draft_id: String) -> bool {
    println!("s: unpublish post");
    PostsRepo::new().await.unpublish_draft(draft_id).await;
    true
}
