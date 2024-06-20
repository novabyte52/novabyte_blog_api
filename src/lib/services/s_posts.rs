use surrealdb::sql::Thing;

use crate::models::post::{DraftPostArgs, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

async fn create_post(created_by: Thing) -> Post {
    println!("s: create post");
    // TODO: create post record
    PostsRepo::new().await.insert_post(created_by).await
}

pub async fn get_post(post_id: Thing) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

// pub async fn get_posts() -> Vec<Post> {
//     println!("s: get posts");
//     PostsRepo::new().await.select_posts().await
// }

// pub async fn author_post(person_id: Thing, post_id: Thing) -> bool {
//     let repo = PostsRepo::new().await;

//     let authors = repo.get_post_authors().await;

//     if authors.into_iter().any(|a| a.r#in == person_id) {
//         return true;
//     }

//     repo.author_post(person_id, post_id).await
// }

pub async fn draft_post(draft: DraftPostArgs, author: Thing) -> bool {
    let repo = PostsRepo::new().await;

    repo.writer.begin_tran().await;

    // TODO: check if post already exists
    if let Some(post_id) = draft.id {
        // TODO: update the existing post markdown with new post markdown
        let post = get_post(post_id).await;

        // TODO: relate the post with new markdown to the author using the
        // drafted edge. store the markdown in the edge record to preserve
        // history.
        repo.draft_post(
            post.id as Thing,
            draft.title,
            draft.markdown,
            author.clone(),
        )
        .await;
        repo.writer.commit_tran().await;
        return true;
    };

    // TODO: create a new post
    let new_post = create_post(author.clone()).await;

    // TODO: relate the new post to the author using the drafted edge
    repo.draft_post(new_post.id, draft.title, draft.markdown, author)
        .await;

    repo.writer.commit_tran().await;
    return true;
}

pub async fn publish_post(draft: DraftPostArgs, author: Thing) -> bool {
    let repo = PostsRepo::new().await;

    repo.writer.begin_tran().await;

    // TODO: check if post already exists
    if let Some(post_id) = draft.id {
        // TODO: update the existing post markdown with new post markdown
        let post = get_post(post_id).await;

        // TODO: relate the post with new markdown to the author using the
        // drafted edge. store the markdown in the edge record to preserve
        // history.
        repo.publish_post(post.id as Thing, draft.title, draft.markdown, author)
            .await;
        repo.writer.commit_tran().await;
        return true;
    };

    // TODO: create a new post
    let new_post = create_post(author.clone()).await;

    // TODO: relate the new post to the author using the drafted edge
    repo.publish_post(new_post.id, draft.title, draft.markdown, author)
        .await;

    repo.writer.commit_tran().await;
    return true;
}

pub async fn get_published_posts() -> Vec<Post> {
    println!("s: get published posts");
    PostsRepo::new().await.select_published_posts().await
}

// pub async fn get_current_versions() -> Vec<Post> {
//     println!("s: get current post versions");
//     // PostsRepo::new().await.select_current_versions().await
// }

/// Get the current version of a specific post.
pub async fn get_current_version(post_id: Thing) -> PostVersion {
    println!("s: get current post version");
    let post = get_post(post_id).await;
    let content = PostsRepo::new()
        .await
        .get_current_content(post.id.clone())
        .await;

    PostVersion {
        id: post.id,
        title: content.title,
        markdown: content.markdown,
        author: content.author,
        meta: post.meta,
    }
}
