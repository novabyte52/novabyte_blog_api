use surrealdb::sql::Thing;
use ulid::Ulid;

use crate::models::post::PostPostArgs;
use crate::{models::post::Post, repos::r_posts::PostsRepo};

pub async fn create_post(new_post: PostPostArgs, author: Thing) {
    println!("s: create post");
    PostsRepo::new()
        .await
        .insert_post(new_post.title, new_post.markdown, author)
        .await
}

pub async fn get_post(post_id: Ulid) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

pub async fn get_posts() -> Vec<Post> {
    println!("s: get posts");
    PostsRepo::new().await.select_posts().await
}
