use std::{
    convert::Infallible,
    fmt::{Debug, Display},
};

use axum::response::{IntoResponse, IntoResponseParts};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub enum NovaWebErrorId {
    NotAdmin,
    MissingAuthHeader,
    UnverifiableToken,
    TokenExpired,
    NotFound,
    MissingRefreshToken,
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
}

impl IntoResponse for NovaWebError {
    fn into_response(self) -> axum::response::Response {
        (self.id, self.message).into_response()
    }
}
