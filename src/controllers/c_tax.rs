use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use nb_lib::models::{
    finance::{ProjectionMethod, Quarter, QuarterlyEstimate},
    person::Person,
    tax_info::{TaxProfile, TaxRules, TaxYear, UpsertTaxProfileArgs},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::instrument;

use crate::{
    errors::{bad_request, web_err, WebErr},
    middleware::NbBlogServices,
};

// ---- tax profile ----------------------------------------------------

#[instrument(skip(services))]
pub async fn get_tax_profile(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(year): Path<TaxYear>,
) -> Result<Json<Option<TaxProfile>>, WebErr> {
    services
        .tax
        .get_profile(&current_person.id, year)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services))]
pub async fn put_tax_profile(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(year): Path<TaxYear>,
    Json(args): Json<UpsertTaxProfileArgs>,
) -> Result<Json<TaxProfile>, WebErr> {
    services
        .tax
        .upsert_profile(&current_person.id, year, args)
        .await
        .map(Json)
        .map_err(web_err)
}

// ---- tax rules ------------------------------------------------------

/// Which years have rules loaded, and which the server ships defaults for but
/// hasn't loaded yet. Drives the year picker and the "seed this year" prompt.
#[derive(Debug, Serialize)]
pub struct RuleYears {
    pub configured: Vec<TaxYear>,
    pub available_to_seed: Vec<TaxYear>,
}

#[instrument(skip(services))]
pub async fn get_rule_years(
    State(services): State<NbBlogServices>,
) -> Result<Json<RuleYears>, WebErr> {
    let configured = services.tax.list_rule_years().await.map_err(web_err)?;
    let available_to_seed = services
        .tax
        .available_seed_years()
        .into_iter()
        .filter(|y| !configured.contains(y))
        .collect();

    Ok(Json(RuleYears {
        configured,
        available_to_seed,
    }))
}

#[instrument(skip(services))]
pub async fn get_tax_rules(
    State(services): State<NbBlogServices>,
    Path(year): Path<TaxYear>,
) -> Result<Json<Option<TaxRules>>, WebErr> {
    services
        .tax
        .get_rules(year)
        .await
        .map(Json)
        .map_err(web_err)
}

#[instrument(skip(services, rules))]
pub async fn put_tax_rules(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(year): Path<TaxYear>,
    Json(rules): Json<TaxRules>,
) -> Result<Json<TaxRules>, WebErr> {
    if rules.year != year {
        return Err(bad_request(format!(
            "body declares year {} but the path says {}",
            rules.year.0, year.0
        )));
    }

    services
        .tax
        .upsert_rules(&current_person.id, rules)
        .await
        .map(Json)
        .map_err(web_err)
}

/// Load the committed default rates for a year.
///
/// This **overwrites** any rates already configured for that year — it's the
/// "start over from the shipped defaults" action, not a merge.
#[instrument(skip(services))]
pub async fn seed_tax_rules(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Path(year): Path<TaxYear>,
) -> Result<Json<TaxRules>, WebErr> {
    services
        .tax
        .seed_year(&current_person.id, year)
        .await
        .map(Json)
        .map_err(web_err)
}

// ---- the estimate ---------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EstimateQuery {
    pub year: Option<TaxYear>,
    /// 1–4. Defaults to the quarter the current date falls in.
    pub quarter: Option<u8>,
    /// `annualized` (default) or `ytd`.
    pub method: Option<String>,
}

/// Which estimated-tax period today falls in. Note these are the IRS payment
/// periods, which are *not* equal calendar quarters — Q2 is two months long
/// and Q4 is four.
fn current_quarter(now: OffsetDateTime) -> Quarter {
    match now.month() as u8 {
        1..=3 => Quarter::Q1,
        4..=5 => Quarter::Q2,
        6..=8 => Quarter::Q3,
        _ => Quarter::Q4,
    }
}

#[instrument(skip(services))]
pub async fn get_estimate(
    State(services): State<NbBlogServices>,
    current_person: Extension<Person>,
    Query(q): Query<EstimateQuery>,
) -> Result<Json<QuarterlyEstimate>, WebErr> {
    let now = OffsetDateTime::now_utc();
    let year = q.year.unwrap_or(TaxYear(now.year() as u16));

    let quarter = match q.quarter {
        Some(n) => Quarter::from_ordinal(n)
            .ok_or_else(|| bad_request(format!("quarter must be 1-4, got {n}")))?,
        None => current_quarter(now),
    };

    let method = match q.method.as_deref() {
        None | Some("annualized") => ProjectionMethod::Annualized,
        Some("ytd") => ProjectionMethod::YtdAsFinal,
        Some(other) => {
            return Err(bad_request(format!(
                "method must be 'annualized' or 'ytd', got '{other}'"
            )))
        }
    };

    services
        .tax
        .estimate_quarter(&current_person.id, year, quarter, method)
        .await
        .map(Json)
        .map_err(web_err)
}
