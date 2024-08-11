use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    middleware::from_fn,
    routing::{delete, get, post},
    Router,
};
use std::net::SocketAddr;
use surrealdb::{engine::any::connect, opt::auth::Root};
use surrealdb_migrations::MigrationRunner;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{event, info, instrument, Level};
use tracing_subscriber;
pub mod controllers;
pub mod middleware;

use controllers::{
    c_persons::{get_person, get_persons, login_person, refresh_token, signup_person},
    c_posts::{
        draft_post, get_drafted_posts, get_post_drafts, get_posts, get_published_posts,
        publish_draft, unpublish_post,
    },
};
use middleware::require_authentication;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();
    info!("tracing subscriber initialized");
    event!(Level::INFO, answer = 42, "Testing debug");

    connect_to_db().await;

    // build our application
    let app = init_api().await;

    // run our app
    serve(app, 52001).await;
}

#[instrument]
async fn connect_to_db() {
    let addr = "ws:127.0.0.1:52000"; // ENV

    info!("connecting to: {:#?}", &addr);
    let db = connect(addr)
        .await
        .expect("Unable to connect to database. Is it running?");

    // Signin as a namespace, database, or root user
    let user = "root"; // ENV
    let pswd = "root"; // ENV
    info!("sgning in as: {:#?} with password: {:#?}", &user, &pswd);
    db.signin(Root {
        username: user,
        password: pswd,
    })
    .await
    .expect("Unable to login to database. Review credentials.");

    // Select a specific namespace / database
    let ns = "test"; // ENV
    let db_name = "novabyte.blog"; // ENV
    info!("ns: {:#?} | db: {:#?}", &ns, &db);
    db.use_ns(ns)
        .use_db(db_name)
        .await
        .expect("Unable to access specified namespace or database.");

    // Apply all migrations
    info!("applying migrations...");
    MigrationRunner::new(&db)
        .up()
        .await
        .expect("Failed to apply migrations");

    // TODO: may want to add an endpoint in to rollback migrations at some point
    // or even just a series of endpoints to manage the db

    let migrations_applied = MigrationRunner::new(&db)
        .list()
        .await
        .expect("no applied migrations");
    event!(Level::INFO, "applied migrations: {:#?}", migrations_applied);
    info!("end");
}

#[instrument]
async fn init_api() -> Router {
    // configre cors
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_credentials(true);

    Router::new()
        // persons routes
        .route("/persons", get(get_persons))
        .route("/persons/:person_id", get(get_person))
        // posts routes
        .route("/posts", get(get_posts))
        .route("/posts/drafts", get(get_drafted_posts))
        .route("/posts/drafts", post(draft_post)) // ?publish=bool
        .route("/posts/drafts/:draft_id/publish", post(publish_draft))
        .route("/posts/drafts/:draft_id/publish", delete(unpublish_post))
        .route("/posts/:post_id/drafts", get(get_post_drafts))
        // authentication layer (all previous routes require authentication to access)
        .route_layer(from_fn(require_authentication))
        // all routes after are public and can be accessed by ANYONE
        // public persons routes
        .route("/persons/login", post(login_person))
        .route("/persons/signup", post(signup_person))
        .route("/persons/refresh", get(refresh_token))
        // public posts routes
        .route("/posts/published", get(get_published_posts))
        // all routes should stay behind the CORS layer
        .layer(cors)
}

#[instrument(skip(app))]
async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
