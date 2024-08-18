use std::env;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::JWTClaims,
};
use nb_lib::{
    models::{custom_claims::CustomClaims, person::Person},
    services::s_persons::get_person,
};
use tracing::instrument;

#[instrument(skip(req, next))]
pub async fn is_admin(req: Request, next: Next) -> Result<Response, (StatusCode, String)> {
    if let Some(person) = req.extensions().get::<Person>() {
        if person.is_admin {
            return Ok(next.run(req).await);
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "You are not authorized to access this endpoint.".into(),
    ))
}

#[instrument(skip(req, next))]
pub async fn require_authentication(
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let auth_header = match get_authorization_header(&req) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                "Missing Authorization header.".into(),
            ));
        }
    };

    // verify token against secret key
    let claims = match verify_token(&auth_header) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", e);
            return Err((StatusCode::UNAUTHORIZED, "Cannot verify token.".into()));
        }
    };

    let exp = claims.expires_at.expect("no expiration detected");
    let secs = exp.as_secs() as i64;
    let now = Utc::now().timestamp();

    if now - secs > 0 {
        return Err((StatusCode::UNAUTHORIZED, "Token expired".into()));
    }

    let sub = match claims.subject {
        Some(s) => s,
        None => panic!("Unable to find subject claim"),
    };

    let person_id = sub;

    if let Some(current_person) = get_person(person_id).await {
        // insert the current user into a request extension so the handler can extract it
        req.extensions_mut().insert(current_person);

        Ok(next.run(req).await)
    } else {
        Err((StatusCode::NOT_FOUND, "Cannot find person.".into()))
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
