use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    middleware::from_fn,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use surrealdb::{engine::any::connect, opt::auth::Root};
use surrealdb_migrations::MigrationRunner;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

pub mod controllers;
pub mod middleware;

use controllers::{
    c_persons::{get_person, get_persons, login_person, refresh_token, signup_person},
    c_posts::{draft_post, publish_post},
};
use middleware::require_authentication;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let db = connect("ws:127.0.0.1:52000")
        .await
        .expect("Unable to connect to database. Is it running?");

    // Signin as a namespace, database, or root user
    db.signin(Root {
        username: "root",
        password: "root",
    })
    .await
    .expect("Unable to login to database. Review credentials.");

    // Select a specific namespace / database
    db.use_ns("test")
        .use_db("novabyte.blog")
        .await
        .expect("Unable to access specified namespace or database.");

    // Apply all migrations
    MigrationRunner::new(&db)
        .up()
        .await
        .expect("Failed to apply migrations");

    // Remove all migrations
    // MigrationRunner::new(&db)
    //     .down("0")
    //     .await
    //     .expect("Failed to revert migrations");

    let migrations_applied = MigrationRunner::new(&db)
        .list()
        .await
        .expect("no applied migrations");
    println!("applied migrations: {:#?}", migrations_applied);

    // build our application
    let app = init_api().await;

    // run our app
    serve(app, 52001).await;
}

async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn init_api() -> Router {
    // configre cors
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_credentials(true);

    Router::new()
        .route("/persons", get(get_persons))
        .route("/persons/:person_id", get(get_person))
        // .route("/posts", post(post_post))
        // .route("/posts/:post_id", get(get_post))
        .route("/posts/draft", post(draft_post))
        .route("/posts/publish", post(publish_post))
        .route_layer(from_fn(require_authentication))
        // .route("/posts", get(get_posts))
        .route("/persons/signup", post(signup_person))
        .route("/persons/login", post(login_person))
        .route("/persons/refresh", get(refresh_token))
        .layer(cors)
}
