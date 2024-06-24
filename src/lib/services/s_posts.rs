use futures::future::join_all;
use itertools::Itertools;
use surrealdb::sql::Thing;

use crate::models::post::{DraftPostArgs, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

async fn create_post(created_by: Thing) -> Post {
    println!("s: create post");
    PostsRepo::new().await.insert_post(created_by).await
}

pub async fn get_post(post_id: Thing) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

pub async fn draft_post(draft: DraftPostArgs, author: Thing) -> bool {
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
            post.id as Thing,
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
        .collect::<Vec<Thing>>();

    join_all(unique_draft_ids.clone().into_iter().map(|p| async {
        return get_current_draft(p).await;
    }))
    .await
}

pub async fn get_current_draft(post_id: Thing) -> PostVersion {
    PostsRepo::new().await.select_current_draft(post_id).await
}

pub async fn publish_new_draft(draft: DraftPostArgs, author: Thing) -> bool {
    // TODO: make sure there are no other published drafts
    // for this post before publishing this one
    let repo = PostsRepo::new().await;

    repo.writer.begin_tran().await;

    if let Some(post_id) = draft.id {
        let post = get_post(post_id).await;

        repo.publish_new_draft(post.id as Thing, draft.title, draft.markdown, author)
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

pub async fn publish_draft(draft_id: Thing) -> bool {
    // TODO: make sure there are no other published drafts
    // for this post before publishing this one
    PostsRepo::new().await.publish_draft(draft_id).await;
    true
}

/// Gets all current published versions of any post that is published (visible)
pub async fn get_published_posts() -> Vec<PostVersion> {
    PostsRepo::new().await.select_published_posts().await
}
