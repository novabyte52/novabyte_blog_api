use futures::future::join_all;
use itertools::Itertools;

use crate::models::post::{DraftPostArgs, PostHydrated, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

async fn create_post(created_by: String) -> Post {
    println!("s: create post");
    PostsRepo::new().await.insert_post(created_by).await
}

pub async fn get_post(post_id: String) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

pub async fn get_posts() -> Vec<PostHydrated> {
    println!("s: get posts");
    PostsRepo::new().await.select_posts().await
}

pub async fn get_post_drafts(post_id: String) -> Vec<PostVersion> {
    println!("s: get post drafts");
    PostsRepo::new().await.select_post_drafts(post_id).await
}

pub async fn draft_post(draft: DraftPostArgs, author: String) -> bool {
    println!("s: draft post {:#?}", draft.clone());
    let repo = PostsRepo::new().await;

    repo.writer.begin_tran().await;

    if let Some(post_id) = draft.id {
        println!(
            "post id exists on draft! adding draft to post: {:#?}",
            post_id.clone()
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
        repo.writer.commit_tran().await;
        return true;
    };

    let new_post = create_post(author.clone()).await;

    repo.draft_post(
        new_post.id,
        draft.title,
        draft.markdown,
        author,
        draft.published,
    )
    .await;

    repo.writer.commit_tran().await;
    return true;
}

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

pub async fn get_current_draft(post_id: String) -> PostVersion {
    PostsRepo::new().await.select_current_draft(post_id).await
}

pub async fn publish_new_draft(draft: DraftPostArgs, author: String) -> bool {
    // TODO: make sure there are no other published drafts
    // for this post before publishing this one
    let repo = PostsRepo::new().await;

    repo.writer.begin_tran().await;

    if let Some(post_id) = draft.id {
        let post = get_post(post_id).await;

        repo.publish_new_draft(post.id, draft.title, draft.markdown, author)
            .await;
        repo.writer.commit_tran().await;
        return true;
    };

    let new_post = create_post(author.clone()).await;

    repo.publish_new_draft(new_post.id, draft.title, draft.markdown, author)
        .await;

    repo.writer.commit_tran().await;
    return true;
}

pub async fn publish_draft(draft_id: String) -> bool {
    let repo = PostsRepo::new().await;

    // TODO: get post_id using draft_id
    let post_id = repo.select_post_id_for_draft_id(&draft_id).await;

    if repo.unpublish_drafts_for_post_id(post_id).await {
        repo.publish_draft(draft_id).await;
        return true;
    }

    false
}

/// Gets all current published versions of any post that is published (visible)
pub async fn get_published_posts() -> Vec<PostVersion> {
    PostsRepo::new().await.select_published_posts().await
}

pub async fn unpublish_post(draft_id: String) -> bool {
    println!("s: unpublish post");
    PostsRepo::new().await.unpublish_draft(draft_id).await;
    true
}
