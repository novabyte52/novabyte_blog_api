use include_dir::{include_dir, Dir};
use serde::Deserialize;
use tracing::{info, instrument};

use crate::constants::INSERT_SELECT_IDX;
use crate::db::nova_db::{NovaDB, NovaResponse};
use crate::db::SurrealDBConnection;
use crate::errors::NovaError;
use crate::models::finance::{
    ExpenseRecord, IncomeRecord, ProjectionMethod, Quarter, QuarterlyEstimate, TaxPayment,
};
use crate::models::tax_info::{TaxProfile, TaxRules, TaxYear, UpsertTaxProfileArgs};
use crate::repos::r_finance::FinanceRepo;
use crate::repos::r_tax::TaxRepo;

use super::tax_calc::{estimate, EstimateInputs};

/// Committed default rate tables, one JSON file per year, embedded in the
/// binary so seeding works the same in Docker as it does locally.
static SEEDS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/lib/db/seeds");

#[derive(Debug, Deserialize)]
struct RulesRow {
    rules: TaxRules,
}

#[derive(Debug, Clone)]
pub struct TaxService {
    repo: TaxRepo,
    finance_repo: FinanceRepo,
    conn: SurrealDBConnection,
}

impl TaxService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: TaxRepo::new(),
            finance_repo: FinanceRepo::new(),
            conn,
        }
    }

    async fn db(&self) -> Result<NovaDB, NovaError> {
        Ok(NovaDB::new(&self.conn).await?)
    }

    // ---- tax profile -----------------------------------------------

    async fn fetch_profile(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Option<TaxProfile>, NovaError> {
        let mut resp = db
            .exec(self.repo.query_select_tax_profile(person_id, year))
            .await?;
        Ok(resp.take_opt::<TaxProfile>(0)?)
    }

    #[instrument(skip(self))]
    pub async fn get_profile(
        &self,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Option<TaxProfile>, NovaError> {
        let db = self.db().await?;
        self.fetch_profile(&db, person_id, year).await
    }

    async fn insert_profile(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
        args: &UpsertTaxProfileArgs,
    ) -> Result<TaxProfile, NovaError> {
        let q = self.repo.query_insert_tax_profile(person_id, year, args);
        let tx = db.begin().await?;
        let mut resp: NovaResponse = tx.query(&q.sql).bind(q.args).await?.into();
        tx.commit().await?;
        Ok(resp.take_one::<TaxProfile>(INSERT_SELECT_IDX)?)
    }

    async fn update_profile(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
        args: &UpsertTaxProfileArgs,
    ) -> Result<TaxProfile, NovaError> {
        let mut resp = db
            .exec(self.repo.query_update_tax_profile(person_id, year, args))
            .await?;
        // 0 LET $rec_id, 1 IF (scoped UPDATE + meta touch), 2 SELECT
        resp.take_opt::<TaxProfile>(2)?
            .ok_or(NovaError::MissingTaxProfile(year))
    }

    /// Create or replace this person's profile for the year.
    ///
    /// The exists-check and the branch it picks are not atomic with each
    /// other, so two concurrent upserts for a brand-new person+year can both
    /// see "doesn't exist" and both attempt to insert; the
    /// `tax_profile_person_year` UNIQUE index then rejects the loser's
    /// `CREATE`. Rather than treat that as a hard failure, the insert
    /// failure path re-checks whether the row exists now and, if so, falls
    /// back to updating it — completing the upsert the caller actually
    /// asked for instead of surfacing a raw database conflict. This costs
    /// nothing extra on the common, non-racing path.
    #[instrument(skip(self))]
    pub async fn upsert_profile(
        &self,
        person_id: &str,
        year: TaxYear,
        args: UpsertTaxProfileArgs,
    ) -> Result<TaxProfile, NovaError> {
        let db = self.db().await?;
        let exists = self.get_profile(person_id, year).await?.is_some();

        if exists {
            return self.update_profile(&db, person_id, year, &args).await;
        }

        match self.insert_profile(&db, person_id, year, &args).await {
            Ok(profile) => Ok(profile),
            Err(insert_err) => match self.update_profile(&db, person_id, year, &args).await {
                Ok(profile) => Ok(profile),
                // Genuinely doesn't exist — the insert failed for some
                // other reason, so that's the error worth reporting.
                Err(NovaError::MissingTaxProfile(_)) => Err(insert_err),
                Err(other) => Err(other),
            },
        }
    }

    // ---- tax rules -------------------------------------------------

    async fn fetch_rules(&self, db: &NovaDB, year: TaxYear) -> Result<Option<TaxRules>, NovaError> {
        let mut resp = db.exec(self.repo.query_select_tax_rules(year)).await?;
        Ok(resp.take_opt::<RulesRow>(0)?.map(|r| r.rules))
    }

    #[instrument(skip(self))]
    pub async fn get_rules(&self, year: TaxYear) -> Result<Option<TaxRules>, NovaError> {
        let db = self.db().await?;
        self.fetch_rules(&db, year).await
    }

    #[instrument(skip(self))]
    pub async fn list_rule_years(&self) -> Result<Vec<TaxYear>, NovaError> {
        let db = self.db().await?;
        let mut resp = db.exec(self.repo.query_select_rule_years()).await?;
        Ok(resp.take_vec::<TaxYear>(0)?)
    }

    async fn insert_rules(
        &self,
        db: &NovaDB,
        person_id: &str,
        rules: &TaxRules,
    ) -> Result<TaxRules, NovaError> {
        let q = self.repo.query_insert_tax_rules(person_id, rules);
        let tx = db.begin().await?;
        let mut resp: NovaResponse = tx.query(&q.sql).bind(q.args).await?.into();
        tx.commit().await?;
        Ok(resp.take_one::<RulesRow>(INSERT_SELECT_IDX)?.rules)
    }

    async fn update_rules(
        &self,
        db: &NovaDB,
        person_id: &str,
        rules: &TaxRules,
    ) -> Result<TaxRules, NovaError> {
        let mut resp = db
            .exec(self.repo.query_update_tax_rules(person_id, rules))
            .await?;
        // 0 LET $rec_id, 1 IF (scoped UPDATE + meta touch), 2 SELECT
        resp.take_opt::<RulesRow>(2)?
            .map(|r| r.rules)
            .ok_or(NovaError::MissingTaxRules(rules.year))
    }

    /// Create or replace the shared rate table for a year. See
    /// [`upsert_profile`](Self::upsert_profile) for why the insert path
    /// falls back to an update on failure instead of trusting the earlier
    /// exists-check alone — the same non-atomicity applies here against the
    /// `tax_rules` `UNIQUE(year)` index.
    #[instrument(skip(self, rules))]
    pub async fn upsert_rules(
        &self,
        person_id: &str,
        rules: TaxRules,
    ) -> Result<TaxRules, NovaError> {
        let db = self.db().await?;
        let exists = self.get_rules(rules.year).await?.is_some();

        if exists {
            return self.update_rules(&db, person_id, &rules).await;
        }

        match self.insert_rules(&db, person_id, &rules).await {
            Ok(rules) => Ok(rules),
            Err(insert_err) => match self.update_rules(&db, person_id, &rules).await {
                Ok(rules) => Ok(rules),
                Err(NovaError::MissingTaxRules(_)) => Err(insert_err),
                Err(other) => Err(other),
            },
        }
    }

    /// Load the committed defaults for a year into the database.
    ///
    /// Idempotent in the sense that it can be re-run, but it *overwrites* —
    /// re-seeding a year discards any rate edits made through the UI. The
    /// endpoint that calls this should make that clear.
    #[instrument(skip(self))]
    pub async fn seed_year(&self, person_id: &str, year: TaxYear) -> Result<TaxRules, NovaError> {
        info!("s: seeding tax rules for {}", year.0);

        let filename = format!("tax_rules_{}.json", year.0);
        let file = SEEDS
            .get_file(&filename)
            .ok_or(NovaError::MissingTaxRules(year))?;

        let raw = file
            .contents_utf8()
            .ok_or_else(|| NovaError::InvalidTaxRules(format!("{filename} is not valid UTF-8")))?;

        let rules: TaxRules = serde_json::from_str(raw)
            .map_err(|e| NovaError::InvalidTaxRules(format!("{filename}: {e}")))?;

        if rules.year != year {
            return Err(NovaError::InvalidTaxRules(format!(
                "{filename} declares year {} but was requested for {}",
                rules.year.0, year.0
            )));
        }

        self.upsert_rules(person_id, rules).await
    }

    /// Years shipped as committed seed data, whether or not they've been
    /// loaded into the database yet.
    pub fn available_seed_years(&self) -> Vec<TaxYear> {
        let mut years: Vec<TaxYear> = SEEDS
            .files()
            .filter_map(|f| {
                f.path()
                    .file_stem()?
                    .to_str()?
                    .strip_prefix("tax_rules_")?
                    .parse::<u16>()
                    .ok()
                    .map(TaxYear)
            })
            .collect();
        years.sort_by_key(|y| std::cmp::Reverse(y.0));
        years
    }

    // ---- the estimate ----------------------------------------------

    async fn fetch_ledger_income(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<IncomeRecord>, NovaError> {
        Ok(db
            .exec_vec(
                self.finance_repo
                    .query_select_income_for_year(person_id, year),
            )
            .await?)
    }

    async fn fetch_ledger_expenses(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<ExpenseRecord>, NovaError> {
        Ok(db
            .exec_vec(
                self.finance_repo
                    .query_select_expenses_for_year(person_id, year),
            )
            .await?)
    }

    async fn fetch_ledger_payments(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<TaxPayment>, NovaError> {
        Ok(db
            .exec_vec(
                self.finance_repo
                    .query_select_payments_for_year(person_id, year),
            )
            .await?)
    }

    /// The headline feature: what to pay for this quarter.
    ///
    /// The profile, rules, and three ledger reads are all independent, so
    /// they share one connection and run concurrently rather than paying
    /// for five sequential connect+signin+query round trips.
    #[instrument(skip(self))]
    pub async fn estimate_quarter(
        &self,
        person_id: &str,
        year: TaxYear,
        quarter: Quarter,
        method: ProjectionMethod,
    ) -> Result<QuarterlyEstimate, NovaError> {
        let db = self.db().await?;

        let (profile, rules, income, expenses, payments) = tokio::try_join!(
            self.fetch_profile(&db, person_id, year),
            self.fetch_rules(&db, year),
            self.fetch_ledger_income(&db, person_id, year),
            self.fetch_ledger_expenses(&db, person_id, year),
            self.fetch_ledger_payments(&db, person_id, year),
        )?;

        let profile = profile.ok_or(NovaError::MissingTaxProfile(year))?;
        let rules = rules.ok_or(NovaError::MissingTaxRules(year))?;

        Ok(estimate(EstimateInputs {
            profile: &profile,
            rules: &rules,
            income: &income,
            expenses: &expenses,
            payments: &payments,
            quarter,
            method,
        })?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed rate tables are only useful if they actually parse. A typo
    /// in the seed JSON would otherwise surface as a runtime 500 the first time
    /// someone seeds a year.
    #[test]
    fn every_committed_seed_file_parses() {
        let files: Vec<_> = SEEDS.files().collect();
        assert!(!files.is_empty(), "expected at least one seed file");

        for file in files {
            let name = file.path().display().to_string();
            let raw = file.contents_utf8().expect("seed file must be UTF-8");
            let rules: TaxRules =
                serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name} failed to parse: {e}"));

            let expected: u16 = file
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("tax_rules_"))
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| panic!("{name} is not named tax_rules_<year>.json"));

            assert_eq!(rules.year.0, expected, "{name} declares the wrong year");

            for status in ["single", "mfj", "mfs", "hoh"] {
                assert!(
                    rules.federal.brackets.contains_key(status),
                    "{name} is missing federal brackets for {status}"
                );
                assert!(
                    rules.federal.standard_deduction.contains_key(status),
                    "{name} is missing a standard deduction for {status}"
                );
            }
        }
    }
}
