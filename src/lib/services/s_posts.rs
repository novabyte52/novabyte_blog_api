use crate::{models::post::Post, repos::r_posts};

use uuid::Uuid;

pub async fn create_post() -> Uuid {
    println!("s: create post");
    return r_posts::insert_post().await;
}

pub async fn get_post(post_id: Uuid) -> Post {
    println!("s: get post");
    return r_posts::select_post(post_id).await;
}

pub async fn get_posts() -> Vec<u32> {
    println!("s: get posts");
    return r_posts::select_posts().await;
}
