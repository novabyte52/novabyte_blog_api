use ulid::Ulid;

use crate::{models::person::Person, repos::r_posts};

pub async fn create_post() -> Person {
    println!("s: create post");
    return r_posts::insert_post().await;
}

pub async fn get_post(post_id: Ulid) -> Person {
    println!("s: get post");

    return r_posts::select_post(post_id).await;
}

pub async fn get_posts() -> Vec<Person> {
    println!("s: get posts");
    let foo = r_posts::select_posts().await;
    println!("s: {:#?}", foo);
    foo
}
