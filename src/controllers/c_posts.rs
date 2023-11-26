use nb_lib::services::s_posts;

use axum::{extract::Path, Json};
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn post_post() -> Uuid {
    println!("c: create post");
    return s_posts::create_post().await;
}

// will probably use a guid crate for IDs or something
pub async fn get_post(Path(post_id): Path<Uuid>) -> Json<Value> {
    s_posts::create_post().await;
    println!("c: get post");
    let generated_id = s_posts::get_post(post_id).await;
    return Json(json!({ "id": generated_id }));
}

pub async fn get_posts() -> Vec<u32> {
    println!("c: get posts");
    return s_posts::get_posts().await;
}
