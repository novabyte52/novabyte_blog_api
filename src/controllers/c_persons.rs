use std::{env, time::Duration};

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
        person::{LogInCreds, LoginResponse, Person, RefreshResponse, SignUpCreds, SignUpState},
    },
    services::s_persons,
};
use time::OffsetDateTime;
use tracing::{info, instrument};

#[instrument]
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

#[instrument]
pub async fn login_person(jar: CookieJar, Json(creds): Json<LogInCreds>) -> impl IntoResponse {
    let person = s_persons::log_in_with_creds(creds).await;

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
        Json(LoginResponse {
            person: person.clone(),
            token: generate_token(person),
        }),
    )
}

#[instrument]
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

    let token_id = sub;

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

    let expiry_duration = Utc::now().checked_sub_days(Days::new(2)).expect("");

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

    Ok((
        jar,
        Json(RefreshResponse {
            token: generate_token(current_person),
        }),
    ))
}

#[instrument]
pub async fn get_person(person_id: Result<Path<String>, PathRejection>) -> impl IntoResponse {
    info!("c: get person");
    info!("c: {:#?}", &person_id);

    let thing_param = match person_id {
        Ok(p) => p,
        Err(err) => return Err((StatusCode::BAD_REQUEST, err.to_string())),
    };

    let thing = thing_param.0.clone();

    info!("c: person thingParam - {:#?}", thing_param.0);

    let generated_id = s_persons::get_person(thing).await;
    Ok(Json(generated_id))
}

#[instrument]
pub async fn get_persons() -> impl IntoResponse {
    info!("c: get persons");

    let persons = s_persons::get_persons().await;

    Json(persons)
}

#[instrument]
fn generate_token(person: Person) -> String {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let jwt_duration = env::var("JWT_DURATION_MINUTES").expect("cannot find JWT_DURATION_MINUTES");

    let key = HS256Key::from_bytes(secret.as_bytes());

    // TODO: need to only set is_admin to true if i'm the person
    let custom_claims = CustomClaims { is_admin: true };

    let claims = Claims::with_custom_claims(
        custom_claims,
        JwtDuration::from_mins(jwt_duration.parse::<u64>().unwrap()),
    ) //JwtDuration::from_hours(1)
    .with_subject(person.id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}

#[instrument] // TODO: need to invalidate any other refresh tokens associated with this person
fn generate_refresh_token(refresh_id: &String) -> String {
    let secret = env::var("NOVA_SECRET").expect("cannot find NOVA_SECRET");
    let refresh_duration = env::var("REFRESH_DURATION_DAYS").expect("cannot find NOVA_SECRET");

    let key = HS256Key::from_bytes(secret.as_bytes());

    let claims = Claims::create(JwtDuration::from_hours(
        refresh_duration.parse::<u64>().unwrap(),
    ))
    .with_subject(refresh_id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}
