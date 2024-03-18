use std::env;

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jwt_simple::algorithms::{HS256Key, MACLike};
use nb_lib::{models::custom_claims::CustomClaims, services::s_persons::get_person};
use surrealdb::sql::{Id, Thing};

pub mod persons_middleware;

pub async fn require_authentication(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_header = if let Ok(auth_header) = get_authorization_header(&req) {
        auth_header
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    // verify token against secret key
    // verify_token() -> Claims
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    let claims = key
        .verify_token::<CustomClaims>(&auth_header, None)
        .expect("Could not verify token.");

    println!("claims: {:#?}", claims.custom);
    // verify_token() -> Claims

    // parse_sub() -> Thing
    let sub = claims.subject.expect("");
    let thing_parts: Vec<&str> = sub.split(":").collect();
    let thing = Thing {
        id: Id::from(thing_parts[1]),
        tb: String::from(thing_parts[0]),
    };
    // parse_sub() -> Thing

    if let Some(current_user) = get_person(thing).await {
        // insert the current user into a request extension so the handler can
        // extract it
        req.extensions_mut().insert(current_user);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

fn get_authorization_header(req: &Request) -> Result<String, String> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if let Some(auth_header) = auth_header {
        Ok(String::from(auth_header))
    } else {
        Err(String::from("Issue extracting Authorization header."))
    }
}
