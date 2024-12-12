use std::env;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    CookieJar,
};
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::{Claims, NoCustomClaims},
    common::VerificationOptions,
    reexports::coarsetime::Duration as JwtDuration,
};
use nb_lib::models::{
    custom_claims::CustomClaims,
    person::{
        LogInCreds, LoginResponse, Person, PersonCheck, RefreshResponse, SignUpCreds, SignUpState,
    },
};
use time::{Duration, OffsetDateTime};
use tracing::{error, info, instrument, warn};

use crate::{
    constants::{NB_JWT_DURATION, NB_REFRESH_DURATION, NB_REFRESH_KEY, NB_SECRET_KEY},
    errors::{NovaWebError, NovaWebErrorContext, NovaWebErrorId},
    middleware::NbBlogServices,
};

#[instrument(skip(services))]
pub async fn handle_check_person_validity(
    State(services): State<NbBlogServices>,
    Query(person_check): Query<PersonCheck>,
) -> impl IntoResponse {
    Json(services.persons.check_person_validity(person_check).await)
}

#[instrument(skip(services))]
pub async fn signup_person(
    State(services): State<NbBlogServices>,
    Json(creds): Json<SignUpCreds>,
) -> impl IntoResponse {
    let new_person = services
        .persons
        .sign_up(SignUpState {
            username: creds.username,
            email: creds.email,
            password: creds.password,
            pass_hash: None,
        })
        .await;

    Json(new_person)
}

/// Attempt to log in a person with the provided credentials (email & password)
#[instrument(skip(jar, services))]
pub async fn login_person(
    State(services): State<NbBlogServices>,
    jar: CookieJar,
    Json(creds): Json<LogInCreds>,
) -> impl IntoResponse {
    // attempt to log the person in using their credentials
    let person = services.persons.log_in_with_creds(creds).await;

    // create the db record for the refresh token (our session record)
    let refresh = services
        .persons
        .create_refresh_token(person.id.clone())
        .await;

    // generate a signed refresh token using the id of the session record
    let refresh_token = generate_refresh_token(&refresh.id);

    // store the signed token in the session record for lookup purposes
    let _success = services
        .persons
        .set_signed_token(refresh.id, refresh_token.clone())
        .await;

    // add the refresh token as an http-only cookie
    let jar = jar.add(generate_refresh_cookie(Some(refresh_token)));

    // return the modified cookie jar, the person who logged in and their jwt (authentication token)
    (
        jar,
        Json(LoginResponse {
            person: person.clone(),
            token: generate_token(person),
        }),
    )
}

#[instrument(skip(services))]
pub async fn logout_person(
    State(services): State<NbBlogServices>,
    jar: CookieJar,
    current_person: Extension<Person>,
    person_id: String
) -> impl IntoResponse {
    if current_person.id != person_id {
        return Err((StatusCode::FORBIDDEN, jar, Json(false)))
    }

    services.persons.logout_by_id(person_id).await;

    Ok((StatusCode::OK, remove_refresh_cookie(jar), Json(true)))
}

#[instrument(skip(services, jar))]
pub async fn refresh_token(
    State(services): State<NbBlogServices>,
    jar: CookieJar,
) -> impl IntoResponse {
    let nb_refresh = if let Some(cookie) = jar.get(NB_REFRESH_KEY) {
        cookie.clone().into_owned()
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            jar,
            Json(NovaWebError {
                id: NovaWebErrorId::MissingAuthHeader,
                message: "Missing refresh cookie.".into(),
                context: Some(NovaWebErrorContext::Refresh),
            }),
        ));
    };

    let refresh_token = nb_refresh.value();

    let secret = env::var(NB_SECRET_KEY).expect(format!("cannot find {}", NB_SECRET_KEY).as_str());
    let key = HS256Key::from_bytes(secret.as_bytes());

    let claims = match key.verify_token::<NoCustomClaims>(
        &refresh_token,
        Some(VerificationOptions {
            time_tolerance: Some(JwtDuration::from_mins(0)),
            ..Default::default()
        }),
    ) {
        Ok(c) => c,
        Err(e) => {
            error!("{:#?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                remove_refresh_cookie(jar),
                Json(NovaWebError {
                    id: NovaWebErrorId::UnverifiableToken,
                    message: e.to_string(),
                    context: Some(NovaWebErrorContext::Refresh),
                }),
            ));
        }
    };

    let sub = match claims.subject {
        Some(s) => s,
        None => panic!("Unable to find subject claim"),
    };

    let token_id = sub;

    let token = services.persons.get_token_record(token_id).await;
    services.persons.soft_delete_token_record(token.id).await;
    let current_person =
        if let Some(person) = services.persons.get_person(token.person.clone()).await {
            person
        } else {
            // DEBT: its hard to imagine a scenario where the id of the person in the token
            // is missing or incorrect, but there could be some better error handling here.
            return Err((
                StatusCode::NOT_FOUND,
                jar,
                Json(NovaWebError {
                    id: NovaWebErrorId::NotFound,
                    message: "Unable to find person from refresh token.".into(),
                    context: Some(NovaWebErrorContext::Refresh),
                }),
            ));
        };

    let refresh = services
        .persons
        .create_refresh_token(current_person.id.clone())
        .await;
    let refresh_token = generate_refresh_token(&refresh.id);

    Ok((
        StatusCode::OK,
        jar.add(generate_refresh_cookie(Some(refresh_token))),
        Json(RefreshResponse {
            token: generate_token(current_person),
        }),
    ))
}

#[instrument(skip(services))]
pub async fn handle_get_person(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(person_id): Path<String>,
) -> impl IntoResponse {
    if person_id != current_person.id && !current_person.is_admin {
        return Err((
            StatusCode::UNAUTHORIZED,
            "You are unauthorized to view other users info.".to_string(),
        ));
    }

    if let Some(person) = services.persons.get_person(person_id.clone()).await {
        return Ok(Json(person));
    };

    Err((
        StatusCode::NOT_FOUND,
        format!("Unable to find person with id: {}", person_id),
    ))
}

#[instrument(skip(services))]
pub async fn get_persons(State(services): State<NbBlogServices>) -> impl IntoResponse {
    info!("c: get persons");

    let persons = services.persons.get_persons().await;

    Json(persons)
}

#[instrument]
fn generate_refresh_cookie<'a>(refresh_token: Option<String>) -> Cookie<'a> {
    let refresh_duration = env::var(NB_REFRESH_DURATION)
        .expect(format!("cannot find {}", NB_REFRESH_DURATION).as_str())
        .parse::<i64>()
        .expect(format!("unable to parse {} into i64", NB_REFRESH_DURATION).as_str());

    // DEBT: may want to make a specific path for logout and refresh to more granularly control the cookie 
    let cookie_path = "/api/persons";
    let valid_until = OffsetDateTime::now_utc() + Duration::days(refresh_duration);

    // TODO: make this env key a constant
    let is_secure = env::var("USE_TLS")
        .expect("Unable to find USE_TLS env var")
        .parse()
        .expect("USE_TLS env var is not a bool and it should be");

    if let Some(refresh_token) = refresh_token {
        Cookie::build((NB_REFRESH_KEY, refresh_token))
            .path(cookie_path)
            .expires(valid_until)
            .http_only(true)
            .secure(is_secure)
            // .domain("novabyte.blog")
            // .domain("localhost")
            .same_site(SameSite::Lax)
            .into()
    } else {
        Cookie::build(NB_REFRESH_KEY)
            .path(cookie_path)
            .expires(valid_until)
            .http_only(true)
            .secure(is_secure)
            // .domain("novabyte.blog")
            // .domain("localhost")
            .same_site(SameSite::Lax)
            .into()
    }
}

#[instrument]
fn remove_refresh_cookie(jar: CookieJar) -> CookieJar {
    jar.remove(generate_refresh_cookie(None))
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

    let claims = Claims::create(JwtDuration::from_mins(
        refresh_duration.parse::<u64>().unwrap(),
    ))
    .with_subject(refresh_id);

    match key.authenticate(claims) {
        Ok(t) => t,
        Err(e) => panic!("token failed: {}", e),
    }
}
