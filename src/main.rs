pub mod controllers;

use controllers::{
    c_persons::{get_person, get_persons, post_person},
    c_posts::{get_post, get_posts, post_post},
};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use surrealdb::{engine::any::connect, opt::auth::Root};
use surrealdb_migrations::MigrationRunner;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let db = connect("ws:127.0.0.1:52000").await.unwrap();

    // Signin as a namespace, database, or root user
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await
    .unwrap();

    // Select a specific namespace / database
    db.use_ns("test").use_db("novabyte.blog").await.unwrap();

    // Apply all migrations
    MigrationRunner::new(&db)
        .up()
        .await
        .expect("Failed to apply migrations");

    // build our application
    let app = init_api().await;

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 52001));
    println!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn init_api() -> Router {
    // configre cors
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]); // <- needed for `content-type: application/json`

    Router::new()
        .route("/persons", post(post_person))
        .route("/persons", get(get_persons))
        .route("/persons/:person_id", get(get_person))
        .route("/posts", post(post_post))
        .route("/posts", get(get_posts))
        .route("/posts/:post_id", get(get_post))
        .layer(cors)
}
