use ulid::Ulid;

use crate::{
    models::person::Person,
    repos::r_posts::PostsRepo,
};

pub async fn create_post() -> Person {
    println!("s: create post");
    PostsRepo::new().await.insert_post().await
}

pub async fn get_post(post_id: Ulid) -> Person {
    println!("s: get post");

    PostsRepo::new().await.select_post(post_id).await
}

pub async fn get_posts() -> Vec<Person> {
    println!("s: get posts");
    PostsRepo::new().await.select_posts().await
}
