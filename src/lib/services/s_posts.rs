use ulid::Ulid;

use crate::{models::post::Post, repos::r_posts::PostsRepo};
use crate::services::s_persons::get_person;

pub async fn create_post(new_post: Post) -> Post {
    println!("s: create post");
    PostsRepo::new().await
        .insert_post(get_person(new_post.author.id.id).await).await
}

pub async fn get_post(post_id: Ulid) -> Post {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

pub async fn get_posts() -> Vec<Post> {
    println!("s: get posts");
    PostsRepo::new().await.select_posts().await
}
