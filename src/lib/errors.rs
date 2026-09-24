use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::models::tax_info::TaxYear;
use crate::services::tax_calc::TaxCalcError;

/// Errors the finance and tax services can return.
///
/// The blog services predate this and `panic!` on failure. The finance
/// endpoints can't: "no profile configured for this year" and "rules not
/// seeded yet" are ordinary states the UI has to render a prompt for, not
/// crashes.
#[derive(Debug)]
pub enum NovaError {
    Db(String),
    NotFound(String),
    MissingTaxProfile(TaxYear),
    MissingTaxRules(TaxYear),
    InvalidTaxRules(String),
    Calc(TaxCalcError),
}

impl Display for NovaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            NovaError::Db(m) => write!(f, "database error: {m}"),
            NovaError::NotFound(m) => write!(f, "not found: {m}"),
            NovaError::MissingTaxProfile(y) => write!(
                f,
                "no tax profile configured for {}. Set filing status, state, and city first.",
                y.0
            ),
            NovaError::MissingTaxRules(y) => write!(
                f,
                "no tax rules configured for {}. Seed the year's rates first.",
                y.0
            ),
            NovaError::InvalidTaxRules(m) => write!(f, "stored tax rules are invalid: {m}"),
            NovaError::Calc(e) => write!(f, "calculation failed: {e}"),
        }
    }
}

impl std::error::Error for NovaError {}

impl From<surrealdb::Error> for NovaError {
    fn from(e: surrealdb::Error) -> Self {
        NovaError::Db(e.to_string())
    }
}

impl From<TaxCalcError> for NovaError {
    fn from(e: TaxCalcError) -> Self {
        NovaError::Calc(e)
    }
}
