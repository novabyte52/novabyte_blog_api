use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use nb_lib::models::{
    finance::{
        ExpenseRecord, FinanceSummary, IncomeRecord, TaxPayment, UpsertExpenseArgs,
        UpsertIncomeArgs, UpsertPaymentArgs,
    },
    person::Person,
};
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::instrument;

use crate::{
    errors::{web_err, WebErr},
    middleware::NbBlogServices,
};

use nb_lib::models::tax_info::TaxYear;

/// Every finance endpoint is year-scoped. Defaulting to the current calendar
/// year keeps the common case a bare `GET` with no query string.
#[derive(Debug, Deserialize)]
pub struct YearQuery {
    pub year: Option<TaxYear>,
}

impl YearQuery {
    pub fn resolve(&self) -> TaxYear {
        self.year
            .unwrap_or_else(|| TaxYear(OffsetDateTime::now_utc().year() as u16))
    }
}

// ---- income ---------------------------------------------------------

#[instrument(skip(services))]
pub async fn get_income(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Query(q): Query<YearQuery>,
) -> Result<Json<Vec<IncomeRecord>>, WebErr> {
    services
        .finance
        .list_income(&current_person.id, q.resolve())
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn create_income(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Json(args): Json<UpsertIncomeArgs>,
) -> Result<Json<IncomeRecord>, WebErr> {
    services
        .finance
        .create_income(&current_person.id, args)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn update_income(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(record_id): Path<String>,
    Json(args): Json<UpsertIncomeArgs>,
) -> Result<Json<IncomeRecord>, WebErr> {
    services
        .finance
        .update_income(&current_person.id, &record_id, args)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn delete_income(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(record_id): Path<String>,
) -> Result<Json<bool>, WebErr> {
    services
        .finance
        .delete_income(&current_person.id, &record_id)
        .await
        .map(|_| Json(true))
        .map_err(web_err)
}

// ---- expenses -------------------------------------------------------

#[instrument(skip(services))]
pub async fn get_expenses(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Query(q): Query<YearQuery>,
) -> Result<Json<Vec<ExpenseRecord>>, WebErr> {
    services
        .finance
        .list_expenses(&current_person.id, q.resolve())
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn create_expense(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Json(args): Json<UpsertExpenseArgs>,
) -> Result<Json<ExpenseRecord>, WebErr> {
    services
        .finance
        .create_expense(&current_person.id, args)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn update_expense(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(record_id): Path<String>,
    Json(args): Json<UpsertExpenseArgs>,
) -> Result<Json<ExpenseRecord>, WebErr> {
    services
        .finance
        .update_expense(&current_person.id, &record_id, args)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn delete_expense(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(record_id): Path<String>,
) -> Result<Json<bool>, WebErr> {
    services
        .finance
        .delete_expense(&current_person.id, &record_id)
        .await
        .map(|_| Json(true))
        .map_err(web_err)
}

// ---- payments -------------------------------------------------------

#[instrument(skip(services))]
pub async fn get_payments(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Query(q): Query<YearQuery>,
) -> Result<Json<Vec<TaxPayment>>, WebErr> {
    services
        .finance
        .list_payments(&current_person.id, q.resolve())
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn create_payment(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Json(args): Json<UpsertPaymentArgs>,
) -> Result<Json<TaxPayment>, WebErr> {
    services
        .finance
        .create_payment(&current_person.id, args)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn delete_payment(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(record_id): Path<String>,
) -> Result<Json<bool>, WebErr> {
    services
        .finance
        .delete_payment(&current_person.id, &record_id)
        .await
        .map(|_| Json(true))
        .map_err(web_err)
}

// ---- dashboard ------------------------------------------------------

#[instrument(skip(services))]
pub async fn get_summary(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Query(q): Query<YearQuery>,
) -> Result<Json<FinanceSummary>, WebErr> {
    services
        .finance
        .get_summary(&current_person.id, q.resolve())
        .await
        .map(Json)
        .map_err(web_err)
}
