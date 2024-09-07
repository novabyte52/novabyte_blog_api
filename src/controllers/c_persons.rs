use std::env;

use axum::{extract::Path, http::StatusCode, response::IntoResponse, Extension, Json};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
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
use time::{Duration, OffsetDateTime};
use tracing::{info, instrument};

use crate::constants::{NB_JWT_DURATION, NB_REFRESH_DURATION, NB_REFRESH_KEY, NB_SECRET_KEY};

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

    let refresh_duration = env::var(NB_REFRESH_DURATION)
        .expect(format!("cannot find {}", NB_REFRESH_DURATION).as_str())
        .parse::<i64>()
        .expect(format!("unable to parse {} into i64", NB_REFRESH_DURATION).as_str());

    let jar = jar.add(
        Cookie::build((NB_REFRESH_KEY, refresh_token.clone()))
            .path("/")
            .expires(OffsetDateTime::now_utc() + Duration::days(refresh_duration))
            .http_only(true)
            .secure(false)
            .same_site(SameSite::None),
    );

    (
        jar,
        Json(LoginResponse {
            person: person.clone(),
            token: generate_token(person),
        }),
    )
}

pub async fn logout_person(jar: CookieJar, person: Extension<Person>) -> impl IntoResponse {
    let jar = jar.remove(Cookie::from(NB_REFRESH_KEY));

    s_persons::logout(person.0).await;

    (jar, Json(true))
}

#[instrument]
pub async fn refresh_token(jar: CookieJar) -> impl IntoResponse {
    let nb_refresh = if let Some(cookie) = jar.get(NB_REFRESH_KEY) {
        cookie.clone().into_owned()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing refresh token.".to_string(),
        ));
    };

    let refresh_token = nb_refresh.value();

    let secret = env::var(NB_SECRET_KEY).expect(format!("cannot find {}", NB_SECRET_KEY).as_str());
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

    let expiry_duration = OffsetDateTime::now_utc()
        .checked_sub(Duration::days(2))
        .expect("Unable to compute expiry duration");

    if expiry_duration > token.meta.created_on {
        return Err((StatusCode::UNAUTHORIZED, "Refresh token retired.".into()));
    }

    let refresh = s_persons::create_refresh_token(current_person.id.clone()).await;
    let refresh_token = generate_refresh_token(&refresh.id);

    let jar = jar.remove(nb_refresh);

    let refresh_duration = env::var(NB_REFRESH_DURATION)
        .expect(format!("cannot find {}", NB_REFRESH_DURATION).as_str())
        .parse::<i64>()
        .expect(format!("unable to parse {} into i64", NB_REFRESH_DURATION).as_str());

    let jar = jar.add(
        Cookie::build((NB_REFRESH_KEY, refresh_token.clone()))
            .http_only(true)
            .expires(OffsetDateTime::now_utc() + Duration::days(refresh_duration))
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
pub async fn handle_get_person(
    current_person: Extension<Person>,
    Path(person_id): Path<String>,
) -> impl IntoResponse {
    if person_id != current_person.id && !current_person.is_admin {
        return Err((
            StatusCode::UNAUTHORIZED,
            "You are unauthorized to view other users info.".to_string(),
        ));
    }

    if let Some(person) = s_persons::get_person(person_id.clone()).await {
        return Ok(Json(person));
    };

    Err((
        StatusCode::NOT_FOUND,
        format!("Unable to find person with id: {}", person_id),
    ))
}

#[instrument]
pub async fn get_persons() -> impl IntoResponse {
    info!("c: get persons");

    let persons = s_persons::get_persons().await;

    Json(persons)
}

#[instrument]
fn generate_token(person: Person) -> String {
    let secret = env::var(NB_SECRET_KEY).expect(format!("cannot find {}", NB_SECRET_KEY).as_str());
    let jwt_duration =
        env::var(NB_JWT_DURATION).expect(format!("cannot find {}", NB_JWT_DURATION).as_str());

    let key = HS256Key::from_bytes(secret.as_bytes());

    let custom_claims = CustomClaims {
        is_admin: person.is_admin,
    };

    let claims = Claims::with_custom_claims(
        custom_claims,
        JwtDuration::from_mins(jwt_duration.parse::<u64>().unwrap()),
    )
    .with_subject(person.id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}

#[instrument]
fn generate_refresh_token(refresh_id: &String) -> String {
    let secret = env::var(NB_SECRET_KEY).expect(format!("cannot find {}", NB_SECRET_KEY).as_str());
    let refresh_duration = env::var(NB_REFRESH_DURATION)
        .expect(format!("cannot find {}", NB_REFRESH_DURATION).as_str());

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
