use std::{collections::HashMap, env};

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::Cookie;
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::{JWTClaims, NoCustomClaims},
};
use nb_lib::{
    db::SurrealDBConnection,
    models::{custom_claims::CustomClaims, person::Person},
    services::{s_persons::PersonsService, s_posts::PostsService},
};
use time::OffsetDateTime;
use tracing::{debug, instrument, trace, warn};

use crate::{
    constants::{
        NB_DB_ADDRESS, NB_DB_NAME, NB_DB_NAMESPACE, NB_DB_PSWD, NB_DB_USER, NB_REFRESH_KEY,
        NB_SECRET_KEY,
    },
    errors::{NovaWebError, NovaWebErrorContext, NovaWebErrorId},
    utils::get_env,
};

#[derive(Debug, Clone)]
pub struct NbBlogServices {
    pub posts: PostsService,
    pub persons: PersonsService,
}

#[instrument(skip(req, next))]
pub async fn init_services(mut req: Request, next: Next) -> impl IntoResponse {
    let addr = get_env::<String>(NB_DB_ADDRESS);
    let user = get_env::<String>(NB_DB_USER);
    let pass = get_env::<String>(NB_DB_PSWD);
    let namespace = get_env::<String>(NB_DB_NAMESPACE);
    let db = get_env::<String>(NB_DB_NAME);

    let conn = SurrealDBConnection {
        address: addr,
        username: user,
        password: pass,
        namespace: namespace,
        database: db,
    };

    let services = NbBlogServices {
        posts: PostsService::new(conn.clone()).await,
        persons: PersonsService::new(conn.clone()).await,
    };

    req.extensions_mut().insert(services);

    next.run(req).await
}

#[instrument(skip(req, next))]
pub async fn is_admin(req: Request, next: Next) -> impl IntoResponse {
    if let Some(person) = req.extensions().get::<Person>() {
        if person.is_admin {
            return Ok(next.run(req).await);
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "You are not authorized to access this endpoint.",
    ))
}

#[instrument(skip(req, next))]
pub async fn require_authentication(
    State(services): State<NbBlogServices>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    let auth_header = match get_authorization_header(&req) {
        Ok(t) => t,
        Err(e) => {
            println!("{:#?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(NovaWebError {
                    id: NovaWebErrorId::MissingAuthHeader,
                    message: "Missing authorization header.".into(),
                    context: Some(NovaWebErrorContext::Authentication),
                }),
            ));
        }
    };

    // verify token against secret key
    let claims = match verify_token(&auth_header) {
        Ok(t) => t,
        Err(e) => {
            println!("{:#?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(NovaWebError {
                    id: NovaWebErrorId::UnverifiableToken,
                    message: "Unable to verify token.".into(),
                    context: Some(NovaWebErrorContext::Authentication),
                }),
            ));
        }
    };

    let exp = claims.expires_at.expect("no expiration detected");
    let secs = exp.as_secs() as i64;
    let now = OffsetDateTime::now_utc().unix_timestamp();

    if now - secs > 0 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(NovaWebError {
                id: NovaWebErrorId::TokenExpired,
                message: "Token expired.".into(),
                context: Some(NovaWebErrorContext::Authentication),
            }),
        ));
    }

    let person_id = claims.subject.expect("Unable to find subject claim");

    if let Some(current_person) = services.persons.get_person(person_id).await {
        // insert the current user into a request extension so the handler can extract it
        req.extensions_mut().insert(current_person);

        Ok(next.run(req).await)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(NovaWebError {
                id: NovaWebErrorId::NotFound,
                message: "Unable to find subject of token.".into(),
                context: Some(NovaWebErrorContext::Authentication),
            }),
        ))
    }
}

#[instrument(skip(services, req, next))]
pub async fn require_refresh_token(
    State(services): State<NbBlogServices>,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    if req.uri().path() != "/persons/logout" && req.uri().path() != "/persons/refresh" {
        trace!("path is not logout and is not refresh");
        return Ok(next.run(req).await);
    };

    if let Some(jar) = extract_jar(&req) {
        if let Some(refresh_token) = jar.get(NB_REFRESH_KEY) {
            println!("nb refresh cookie: {}", &refresh_token);

            // verify token against secret key
            let claims = match verify_refresh_token(&refresh_token) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{:#?}", e);

                    if format!("{}", e) == "Token has expired" {
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            NovaWebError {
                                id: NovaWebErrorId::UnverifiableToken,
                                message: "Refresh token expired.".into(),
                                context: Some(NovaWebErrorContext::Refresh),
                            },
                        ));
                    }

                    return Err((
                        StatusCode::UNAUTHORIZED,
                        NovaWebError {
                            id: NovaWebErrorId::UnverifiableToken,
                            message: "Unable to verify token.".into(),
                            context: Some(NovaWebErrorContext::Refresh),
                        },
                    ));
                }
            };

            let refresh_id = claims.subject.expect("Unable to find subject claim.");

            let refresh = services.persons.get_token_record(refresh_id).await;

            if let Some(current_person) = services.persons.get_person(refresh.person).await {
                // insert the current user into a request extension so the handler can extract it
                req.extensions_mut().insert(current_person);

                return Ok(next.run(req).await);
            } else {
                return Err((
                    StatusCode::NOT_FOUND,
                    NovaWebError {
                        id: NovaWebErrorId::NotFound,
                        message: "Unable to find subject of token.".into(),
                        context: Some(NovaWebErrorContext::Refresh),
                    },
                ));
            }
        }
    }

    Err((
        StatusCode::BAD_REQUEST,
        NovaWebError {
            id: NovaWebErrorId::MissingRefreshToken,
            message: "Missing refresh token.".into(),
            context: Some(NovaWebErrorContext::Refresh),
        },
    ))
}

fn extract_jar(req: &Request) -> Option<HashMap<String, String>> {
    let cookie_header = match req.headers().get("cookie") {
        Some(h) => h,
        None => {
            warn!("Unable to find cookie header.");
            return None;
        }
    };

    let raw_jar = cookie_header
        .to_str()
        .expect("Unable to convert cookie header to string.");

    Some(
        Cookie::split_parse_encoded(raw_jar).fold(HashMap::new(), |mut acc, c| {
            let bar = c.unwrap();
            acc.insert(bar.name().to_string(), bar.value().to_string());
            acc
        }),
    )
}

fn get_authorization_header(req: &Request) -> Result<String, String> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        Ok(auth_header.into())
    } else {
        Err("Issue extracting Authorization header.".into())
    }
}

fn verify_token(token: &String) -> Result<JWTClaims<CustomClaims>, jwt_simple::Error> {
    let secret = env::var(NB_SECRET_KEY).expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    key.verify_token::<CustomClaims>(&token, None)
}

fn verify_refresh_token(token: &String) -> Result<JWTClaims<NoCustomClaims>, jwt_simple::Error> {
    let secret = env::var(NB_SECRET_KEY).expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    key.verify_token(&token, None)
}
