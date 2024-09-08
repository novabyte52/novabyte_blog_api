use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    middleware::from_fn,
    routing::{delete, get, post},
    Router,
};
use constants::{NB_DB_ADDRESS, NB_DB_NAME, NB_DB_NAMESPACE, NB_DB_PSWD, NB_DB_USER};
use std::net::SocketAddr;
use surrealdb::{engine::any::connect, opt::auth::Root};
use surrealdb_migrations::MigrationRunner;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{debug, info, instrument, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use utils::get_env;

pub mod constants;
pub mod controllers;
pub mod errors;
pub mod middleware;
pub mod utils;

use controllers::{
    c_persons::{
        get_persons, handle_get_person, login_person, logout_person, refresh_token, signup_person,
    },
    c_posts::{
        get_draft, get_drafted_posts, get_post_drafts, get_posts, get_published_posts,
        handle_create_draft, publish_draft, unpublish_post,
    },
};
use middleware::{is_admin, require_authentication, require_refresh_token};

#[instrument]
#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    trace!("tracing subscriber initialized");

    connect_to_db().await;

    // build our application
    let app = init_api().await;

    // run our app
    serve(app, 52001).await;
}

#[instrument]
async fn connect_to_db() {
    let addr: String = get_env(NB_DB_ADDRESS);

    debug!("connecting to: {:#?}", &addr);
    let db = connect(addr)
        .await
        .expect("Unable to connect to database. Is it running?");

    // Signin as a namespace, database, or root user
    let user: String = get_env(NB_DB_USER);
    let pswd: String = get_env(NB_DB_PSWD);

    debug!("signing in as: {:#?} with password: {:#?}", &user, &pswd);

    db.signin(Root {
        username: user.as_str(),
        password: pswd.as_str(),
    })
    .await
    .expect("Unable to login to database. Review credentials.");

    // Select a specific namespace / database
    let ns: String = get_env(NB_DB_NAMESPACE);
    let db_name: String = get_env(NB_DB_NAME);
    debug!("ns: {:#?} | db: {:#?}", &ns, &db_name);

    db.use_ns(ns)
        .use_db(db_name)
        .await
        .expect("Unable to access specified namespace or database.");

    // Apply all migrations
    info!("applying migrations");
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

    debug!("applied migrations: {:#?}", migrations_applied);
}

#[instrument]
async fn init_api() -> Router {
    // configre cors
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:9000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_credentials(true);

    Router::new()
        // admin persons routes
        .route("/persons", get(get_persons))
        //
        // admin posts routes
        .route("/posts", get(get_posts))
        .route("/posts/drafts", get(get_drafted_posts))
        .route("/posts/drafts", post(handle_create_draft)) // ?publish=bool
        .route("/posts/drafts/:draft_id", get(get_draft))
        .route("/posts/drafts/:draft_id/publish", post(publish_draft))
        .route("/posts/drafts/:draft_id/publish", delete(unpublish_post))
        .route("/posts/:post_id/drafts", get(get_post_drafts))
        //
        // ^^ admin layer ^^
        .route_layer(from_fn(is_admin))
        //
        // eventual endpoints for profiles, comments, etc. will go in between the authorization check and the admin check
        .route("/persons/:person_id", get(handle_get_person))
        //
        // ^^ authentication layer ^^
        .route_layer(from_fn(require_authentication))
        //
        .route("/persons/logout", delete(logout_person))
        .route("/persons/refresh", get(refresh_token))
        //
        // ^^ refresh token layer ^^
        .route_layer(from_fn(require_refresh_token))
        //
        // anonymous public persons routes
        .route("/persons/login", post(login_person))
        .route("/persons/signup", post(signup_person))
        //
        // anonymous public posts routes
        .route("/posts/published", get(get_published_posts))
        // ^^ anonymous routes ^^
        //
        // ^^ CORS layer ^^
        .layer(cors)
}

#[instrument(skip(app))]
async fn serve(app: Router, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
