use std::{env, time::Duration};

use crate::middleware::thing_from_string;
use axum::{
    extract::{rejection::PathRejection, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use chrono::{Days, Utc};
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::{Claims, NoCustomClaims},
    reexports::coarsetime::Duration as JwtDuration,
};
use nb_lib::{
    models::{
        custom_claims::CustomClaims,
        person::{LogInCreds, Person, SignUpCreds, SignUpState, TokenResponse},
    },
    services::s_persons,
};
use surrealdb::sql::Thing;
use time::OffsetDateTime;

pub async fn signup_person(Json(creds): Json<SignUpCreds>) -> impl IntoResponse {
    let new_person = s_persons::sign_up(SignUpState {
        username: creds.username,
        email: creds.email,
        password: creds.password,
        pass_hash: None,
    })
    .await;

    Json(new_person)
}

pub async fn login_person(jar: CookieJar, Json(creds): Json<LogInCreds>) -> impl IntoResponse {
    let person = s_persons::log_in_with_creds(creds).await;

    // TODO: insert token record
    let refresh = s_persons::create_refresh_token(person.id.clone()).await;
    let refresh_token = generate_refresh_token(&refresh.id);

    let now = OffsetDateTime::now_utc();
    let jar = jar.add(
        Cookie::build(("nbRefresh", refresh_token.clone()))
            .expires(now + Duration::from_secs(60 * 60 * 24))
            .secure(false)
            .http_only(true),
    );

    (
        jar,
        Json(TokenResponse {
            token: generate_token(person),
        }),
    )
}

pub async fn refresh_token(jar: CookieJar) -> impl IntoResponse {
    let nb_refresh = if let Some(cookie) = jar.get("nbRefresh") {
        cookie.clone().into_owned()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing refresh token.".to_string(),
        ));
    };

    let refresh_token = nb_refresh.value();

    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    let claims = match key.verify_token::<NoCustomClaims>(&refresh_token, None) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::FORBIDDEN, e.to_string())),
    };

    let sub = match claims.subject {
        Some(s) => s,
        None => panic!("Unable to find subject claim"),
    };

    let token_id = match thing_from_string(sub) {
        Ok(id) => id,
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    };

    // TODO: need to soft delete the token record at some point
    let token = s_persons::get_token_record(token_id).await;
    s_persons::soft_delete_token_record(token.id).await;
    let current_person = if let Some(person) = s_persons::get_person(token.person.clone()).await {
        person
    } else {
        return Err((
            StatusCode::NOT_FOUND,
            "Could not find person for refresh token.".into(),
        ));
    };

    // TODO: check if the record is too old (need to define an amount of time refresh is valid)
    let expiry_duration = Utc::now().checked_sub_days(Days::new(2)).expect("");

    // TODO: if record is too old, reject
    if expiry_duration > token.meta.created_on {
        return Err((StatusCode::UNAUTHORIZED, "Refresh token retired.".into()));
    }

    let refresh = s_persons::create_refresh_token(current_person.id.clone()).await;
    let refresh_token = generate_refresh_token(&refresh.id);

    let jar = jar.remove(nb_refresh);
    let jar = jar.add(
        Cookie::build(("nbRefresh", refresh_token.clone()))
            .http_only(true)
            .expires(OffsetDateTime::now_utc() + Duration::from_secs(60 * 60 * 24))
            .same_site(SameSite::None),
    );

    // TODO: if record is fine, generate_token(current_person)
    Ok((
        jar,
        Json(TokenResponse {
            token: generate_token(current_person),
        }),
    ))
}

pub async fn get_person(person_id: Result<Path<Thing>, PathRejection>) -> impl IntoResponse {
    println!("c: get person");

    let id = match person_id {
        Ok(id) => id,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    println!("c: person id - {:#?}", id);

    let generated_id = s_persons::get_person(id.0).await;
    Ok(Json(generated_id))
}

pub async fn get_persons() -> impl IntoResponse {
    println!("c: get persons");

    let persons = s_persons::get_persons().await;

    Json(persons)
}

fn generate_token(person: Person) -> String {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");

    let key = HS256Key::from_bytes(secret.as_bytes());

    let custom_claims = CustomClaims { is_admin: true };

    let claims = Claims::with_custom_claims(custom_claims, JwtDuration::from_mins(1)) //JwtDuration::from_hours(1)
        .with_subject(person.id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}

fn generate_refresh_token(refresh_id: &Thing) -> String {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");

    let key = HS256Key::from_bytes(secret.as_bytes());

    let claims = Claims::create(JwtDuration::from_hours(24)).with_subject(refresh_id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}
