use std::{
    env,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use axum::{
    extract::{Request, State},
    http::{header, HeaderName, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use jwt_simple::{
    algorithms::{HS256Key, MACLike},
    claims::JWTClaims,
    common::VerificationOptions,
    prelude::Duration,
};
use nb_lib::{
    models::{custom_claims::CustomClaims, person::Person},
    services::{s_persons::PersonsService, s_posts::PostsService},
};
use tower::{layer::util::Stack, ServiceBuilder};
use tower_http::request_id::{
    MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer,
};
use tracing::{debug, error, instrument, warn};

use crate::{
    constants::NB_SECRET_KEY,
    errors::{NovaWebError, NovaWebErrorContext, NovaWebErrorId},
};

#[derive(Debug, Clone)]
pub struct NbBlogServices {
    pub posts: PostsService,
    pub persons: PersonsService,
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

#[instrument(skip(services, req, next))]
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
                StatusCode::UNAUTHORIZED,
                Json(NovaWebError {
                    id: NovaWebErrorId::MissingAuthHeader,
                    message: "Missing authorization header.".into(),
                    context: Some(NovaWebErrorContext::Authentication),
                }),
            ));
        }
    };

    if !auth_header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(NovaWebError {
                id: NovaWebErrorId::UnverifiableToken,
                message: "Authorization header malformed".into(),
                context: Some(NovaWebErrorContext::Authentication),
            }),
        ));
    }

    let token = match auth_header.split(" ").nth(1) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(NovaWebError {
                    id: NovaWebErrorId::UnverifiableToken,
                    message: "Bearer token malformed".into(),
                    context: Some(NovaWebErrorContext::Authentication),
                }),
            ))
        }
    };

    debug!("token value: {}", token);

    // verify token against secret key
    let claims = match verify_token(&String::from(token)) {
        Ok(t) => t,
        Err(e) => {
            error!("{:#?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(NovaWebError {
                    id: NovaWebErrorId::UnverifiableToken,
                    message: format!("Unable to verify token: {}", e.to_string()),
                    context: Some(NovaWebErrorContext::Authentication),
                }),
            ));
        }
    };

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

#[instrument(skip(req))]
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

#[instrument(skip(token))]
fn verify_token(token: &String) -> Result<JWTClaims<CustomClaims>, jwt_simple::Error> {
    let secret = env::var(NB_SECRET_KEY).expect("cannot find NOVA_SECRET");
    let key = HS256Key::from_bytes(secret.as_bytes());

    key.verify_token::<CustomClaims>(
        &token,
        Some(VerificationOptions {
            time_tolerance: Some(Duration::from_mins(0)),
            ..Default::default()
        }),
    )
}

// A `MakeRequestId` that increments an atomic counter
#[derive(Clone, Default)]
pub struct MyMakeRequestId {
    counter: Arc<AtomicU64>,
}

impl MakeRequestId for MyMakeRequestId {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let request_id = self
            .counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string()
            .parse()
            .unwrap();

        Some(RequestId::new(request_id))
    }
}

pub fn get_request_id_service() -> ServiceBuilder<
    Stack<
        PropagateRequestIdLayer,
        Stack<SetRequestIdLayer<MyMakeRequestId>, tower::layer::util::Identity>,
    >,
> {
    let x_request_id = HeaderName::from_static("x-request-id");

    ServiceBuilder::new()
        // set `x-request-id` header on all requests
        .layer(SetRequestIdLayer::new(
            x_request_id.clone(),
            MyMakeRequestId::default(),
        ))
        // propagate `x-request-id` headers from request to response
        .layer(PropagateRequestIdLayer::new(x_request_id))
}
