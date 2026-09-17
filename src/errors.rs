use std::{
    convert::Infallible,
    fmt::{Debug, Display},
};

use axum::http::StatusCode;
use axum::response::{IntoResponse, IntoResponseParts};
use axum::Json;
use nb_lib::errors::NovaError;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub enum NovaWebErrorContext {
    Authentication,
    Refresh,
    Finance,
}

#[derive(Debug, Serialize, Clone)]
pub enum NovaWebErrorId {
    NotAdmin,
    MissingAuthHeader,
    UnverifiableToken,
    TokenExpired,
    NotFound,
    MissingRefreshToken,
    /// The year has no tax profile yet — the UI should prompt for filing
    /// status, state, and city rather than showing an error.
    MissingTaxProfile,
    /// The year's rates haven't been seeded or configured yet.
    MissingTaxRules,
    InvalidTaxRules,
    BadRequest,
    DatabaseError,
    CalculationFailed,
}

impl Display for NovaWebErrorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:#?}", self))
    }
}

impl IntoResponseParts for NovaWebErrorId {
    type Error = Infallible;

    fn into_response_parts(
        self,
        mut res: axum::response::ResponseParts,
    ) -> Result<axum::response::ResponseParts, Self::Error> {
        res.extensions_mut().insert(self.to_string());
        Ok(res)
    }
}

#[derive(Debug, Serialize)]
pub struct NovaWebError {
    pub id: NovaWebErrorId,
    pub message: String,
    pub context: Option<NovaWebErrorContext>,
}

impl IntoResponse for NovaWebError {
    fn into_response(self) -> axum::response::Response {
        (self.id, self.message).into_response()
    }
}

/// Shorthand for the error half of a finance controller's return type.
pub type WebErr = (StatusCode, Json<NovaWebError>);

/// Maps a service-layer error onto an HTTP response.
///
/// The two "missing configuration" cases deliberately return 409 rather than
/// 404 or 500: the request was well-formed and the resource exists in
/// principle, the account just hasn't finished setup. That distinction is what
/// lets the finance UI show a setup prompt instead of an error toast.
pub fn web_err(e: NovaError) -> WebErr {
    let message = e.to_string();
    {
        let (status, id) = match e {
            NovaError::MissingTaxProfile(_) => {
                (StatusCode::CONFLICT, NovaWebErrorId::MissingTaxProfile)
            }
            NovaError::MissingTaxRules(_) => {
                (StatusCode::CONFLICT, NovaWebErrorId::MissingTaxRules)
            }
            NovaError::InvalidTaxRules(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                NovaWebErrorId::InvalidTaxRules,
            ),
            NovaError::NotFound(_) => (StatusCode::NOT_FOUND, NovaWebErrorId::NotFound),
            NovaError::Calc(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                NovaWebErrorId::CalculationFailed,
            ),
            NovaError::Db(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                NovaWebErrorId::DatabaseError,
            ),
        };

        (
            status,
            Json(NovaWebError {
                id,
                message,
                context: Some(NovaWebErrorContext::Finance),
            }),
        )
    }
}

pub fn bad_request(message: impl Into<String>) -> WebErr {
    (
        StatusCode::BAD_REQUEST,
        Json(NovaWebError {
            id: NovaWebErrorId::BadRequest,
            message: message.into(),
            context: Some(NovaWebErrorContext::Finance),
        }),
    )
}
