use std::env;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::extract::CookieJar;
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::{JWTClaims, NoCustomClaims},
};
use nb_lib::{
    models::{custom_claims::CustomClaims, person::Person},
    services::s_persons::{self, get_person},
};
use time::OffsetDateTime;
use tracing::{debug, instrument, trace};

use crate::{
    constants::NB_REFRESH_KEY,
    errors::{NovaWebError, NovaWebErrorId},
};

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
pub async fn require_authentication(mut req: Request, next: Next) -> impl IntoResponse {
    let auth_header = match get_authorization_header(&req) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                NovaWebError {
                    id: NovaWebErrorId::MissingAuthHeader,
                    message: "Missing authorization header.".into(),
                },
            ));
        }
    };

    // verify token against secret key
    let claims = match verify_token(&auth_header) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                NovaWebError {
                    id: NovaWebErrorId::UnverifiableToken,
                    message: "Unable to verify token.".into(),
                },
            ));
        }
    };

    let exp = claims.expires_at.expect("no expiration detected");
    let secs = exp.as_secs() as i64;
    let now = OffsetDateTime::now_utc().unix_timestamp();

    if now - secs > 0 {
        return Err((
            StatusCode::UNAUTHORIZED,
            NovaWebError {
                id: NovaWebErrorId::TokenExpired,
                message: "Token expired.".into(),
            },
        ));
    }

    let person_id = claims.subject.expect("Unable to find subject claim");

    if let Some(current_person) = get_person(person_id).await {
        // insert the current user into a request extension so the handler can extract it
        req.extensions_mut().insert(current_person);

        Ok(next.run(req).await)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            NovaWebError {
                id: NovaWebErrorId::NotFound,
                message: "Unable to find subject of token.".into(),
            },
        ))
    }
}

#[instrument(skip(jar, req, next))]
pub async fn require_refresh_token(
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    debug!("request uri: {}", req.uri().path());
    debug!("path is logout = {}", req.uri().path() == "/persons/logout");
    debug!(
        "path is refresh = {}",
        req.uri().path() == "/persons/refresh"
    );

    // TODO: HIGH: do some validation checks on the refresh token regardless of path

    // TODO: take care of these magic strings
    if req.uri().path() != "/persons/logout" && req.uri().path() != "/persons/refresh" {
        trace!("path is not logout and is not refresh");
        return Ok(next.run(req).await);
    };

    if let Some(cookie) = jar.get(NB_REFRESH_KEY) {
        let refresh_token = String::from(cookie.value_trimmed());

        // verify token against secret key
        let claims = match verify_refresh_token(&refresh_token) {
            Ok(t) => t,
            Err(e) => {
                println!("{}", e);
                return Err((
                    StatusCode::UNAUTHORIZED,
                    NovaWebError {
                        id: NovaWebErrorId::UnverifiableToken,
                        message: "Unable to verify token.".into(),
                    },
                ));
            }
        };

        let refresh_id = claims.subject.expect("Unable to find subject claim.");

        let refresh = s_persons::get_token_record(refresh_id).await;

        if let Some(current_person) = get_person(refresh.person).await {
            // insert the current user into a request extension so the handler can extract it
            req.extensions_mut().insert(current_person);

            return Ok(next.run(req).await);
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                NovaWebError {
                    id: NovaWebErrorId::NotFound,
                    message: "Unable to find subject of token.".into(),
                },
            ));
        }
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            NovaWebError {
                id: NovaWebErrorId::MissingRefreshToken,
                message: "Missing refresh token.".into(),
            },
        ))
    }
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
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    key.verify_token::<CustomClaims>(&token, None)
}

fn verify_refresh_token(token: &String) -> Result<JWTClaims<NoCustomClaims>, jwt_simple::Error> {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    key.verify_token(&token, None)
}
