pub mod config;
pub mod controllers;

use controllers::c_posts::{get_post, post_post};

use axum::{
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application
    let app = init_api().await;

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn init_api() -> Router {
    // configre cors
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin("http://localhost:9000".parse::<HeaderValue>().unwrap());

    return Router::new()
        // .layer(ServiceBuilder::new().layer(cors))
        // .route("/posts", post(post_post))
        .route("/posts/:post_id", get(get_post))
        .route_layer(ServiceBuilder::new().layer(cors));
}

// consider this for db migration https://docs.rs/refinery/latest/refinery/
