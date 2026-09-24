use tracing::{info, instrument};

use crate::constants::INSERT_SELECT_IDX;
use crate::db::nova_db::{NovaDB, NovaResponse};
use crate::db::SurrealDBConnection;
use crate::errors::NovaError;
use crate::models::finance::{
    ExpenseRecord, FinanceSummary, IncomeKind, IncomeRecord, TaxPayment, UpsertExpenseArgs,
    UpsertIncomeArgs, UpsertPaymentArgs, YtdTotals,
};
use crate::models::tax_info::{Money, TaxYear};
use crate::repos::r_finance::FinanceRepo;
use crate::repos::r_tax::TaxRepo;

/// CRUD over the personal-finance ledger. Every method is scoped to the
/// `person_id` of the caller — records are never shared between accounts.
#[derive(Debug, Clone)]
pub struct FinanceService {
    repo: FinanceRepo,
    tax_repo: TaxRepo,
    conn: SurrealDBConnection,
}

impl FinanceService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: FinanceRepo::new(),
            tax_repo: TaxRepo::new(),
            conn,
        }
    }

    async fn db(&self) -> Result<NovaDB, NovaError> {
        Ok(NovaDB::new(&self.conn).await?)
    }

    // ---- income ----------------------------------------------------

    /// Shares `db` with the caller rather than opening its own connection —
    /// [`get_summary`](Self::get_summary) runs this concurrently alongside
    /// several other independent reads on one connection instead of each
    /// paying for its own `connect` + `signin` round trip.
    async fn fetch_income(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<IncomeRecord>, NovaError> {
        let mut resp = db
            .exec(self.repo.query_select_income_for_year(person_id, year))
            .await?;
        Ok(resp.take_vec::<IncomeRecord>(0)?)
    }

    #[instrument(skip(self))]
    pub async fn list_income(
        &self,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<IncomeRecord>, NovaError> {
        let db = self.db().await?;
        self.fetch_income(&db, person_id, year).await
    }

    #[instrument(skip(self))]
    pub async fn create_income(
        &self,
        person_id: &str,
        args: UpsertIncomeArgs,
    ) -> Result<IncomeRecord, NovaError> {
        info!("s: create income record");
        let db = self.db().await?;
        let q = self.repo.query_insert_income(person_id, &args);

        let tx = db.begin().await?;
        let mut resp: NovaResponse = tx.query(&q.sql).bind(q.args).await?.into();
        tx.commit().await?;

        Ok(resp.take_one::<IncomeRecord>(INSERT_SELECT_IDX)?)
    }

    #[instrument(skip(self))]
    pub async fn update_income(
        &self,
        person_id: &str,
        record_id: &str,
        args: UpsertIncomeArgs,
    ) -> Result<IncomeRecord, NovaError> {
        let db = self.db().await?;
        let mut resp = db
            .exec(self.repo.query_update_income(person_id, record_id, &args))
            .await?;
        // 0 LET $rec_id, 1 IF (scoped UPDATE + meta touch), 2 SELECT.
        // $rec_id resolves to NONE (and the IF is skipped) when record_id
        // doesn't exist or doesn't belong to this person, so a missing vs.
        // not-yours record both cleanly fall through to NotFound here.
        resp.take_opt::<IncomeRecord>(2)?
            .ok_or_else(|| NovaError::NotFound(format!("income record {record_id}")))
    }

    #[instrument(skip(self))]
    pub async fn delete_income(&self, person_id: &str, record_id: &str) -> Result<(), NovaError> {
        let db = self.db().await?;
        let mut resp = db
            .exec(self.repo.query_soft_delete_income(person_id, record_id))
            .await?;
        // 0 LET $meta_id, 1 IF (scoped UPDATE), 2 RETURN — the RETURN is the
        // one true signal of whether a row was actually deleted; a record
        // that's missing or belongs to someone else resolves $meta_id to
        // NONE and RETURNs false here rather than silently reporting success.
        if resp.take_one::<bool>(2)? {
            Ok(())
        } else {
            Err(NovaError::NotFound(format!("income record {record_id}")))
        }
    }

    // ---- expenses --------------------------------------------------

    async fn fetch_expenses(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<ExpenseRecord>, NovaError> {
        let mut resp = db
            .exec(self.repo.query_select_expenses_for_year(person_id, year))
            .await?;
        Ok(resp.take_vec::<ExpenseRecord>(0)?)
    }

    #[instrument(skip(self))]
    pub async fn list_expenses(
        &self,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<ExpenseRecord>, NovaError> {
        let db = self.db().await?;
        self.fetch_expenses(&db, person_id, year).await
    }

    #[instrument(skip(self))]
    pub async fn create_expense(
        &self,
        person_id: &str,
        args: UpsertExpenseArgs,
    ) -> Result<ExpenseRecord, NovaError> {
        let db = self.db().await?;
        let q = self.repo.query_insert_expense(person_id, &args);

        let tx = db.begin().await?;
        let mut resp: NovaResponse = tx.query(&q.sql).bind(q.args).await?.into();
        tx.commit().await?;

        Ok(resp.take_one::<ExpenseRecord>(INSERT_SELECT_IDX)?)
    }

    #[instrument(skip(self))]
    pub async fn update_expense(
        &self,
        person_id: &str,
        record_id: &str,
        args: UpsertExpenseArgs,
    ) -> Result<ExpenseRecord, NovaError> {
        let db = self.db().await?;
        let mut resp = db
            .exec(self.repo.query_update_expense(person_id, record_id, &args))
            .await?;
        // 0 LET $rec_id, 1 IF (scoped UPDATE + meta touch), 2 SELECT — see
        // update_income's comment above for why a missing/not-yours record
        // both land here as a clean NotFound.
        resp.take_opt::<ExpenseRecord>(2)?
            .ok_or_else(|| NovaError::NotFound(format!("expense record {record_id}")))
    }

    #[instrument(skip(self))]
    pub async fn delete_expense(&self, person_id: &str, record_id: &str) -> Result<(), NovaError> {
        let db = self.db().await?;
        let mut resp = db
            .exec(self.repo.query_soft_delete_expense(person_id, record_id))
            .await?;
        if resp.take_one::<bool>(2)? {
            Ok(())
        } else {
            Err(NovaError::NotFound(format!("expense record {record_id}")))
        }
    }

    // ---- payments --------------------------------------------------

    async fn fetch_payments(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<TaxPayment>, NovaError> {
        let mut resp = db
            .exec(self.repo.query_select_payments_for_year(person_id, year))
            .await?;
        Ok(resp.take_vec::<TaxPayment>(0)?)
    }

    #[instrument(skip(self))]
    pub async fn list_payments(
        &self,
        person_id: &str,
        year: TaxYear,
    ) -> Result<Vec<TaxPayment>, NovaError> {
        let db = self.db().await?;
        self.fetch_payments(&db, person_id, year).await
    }

    #[instrument(skip(self))]
    pub async fn create_payment(
        &self,
        person_id: &str,
        args: UpsertPaymentArgs,
    ) -> Result<TaxPayment, NovaError> {
        let db = self.db().await?;
        let q = self.repo.query_insert_payment(person_id, &args);

        let tx = db.begin().await?;
        let mut resp: NovaResponse = tx.query(&q.sql).bind(q.args).await?.into();
        tx.commit().await?;

        Ok(resp.take_one::<TaxPayment>(INSERT_SELECT_IDX)?)
    }

    #[instrument(skip(self))]
    pub async fn delete_payment(&self, person_id: &str, record_id: &str) -> Result<(), NovaError> {
        let db = self.db().await?;
        let mut resp = db
            .exec(self.repo.query_soft_delete_payment(person_id, record_id))
            .await?;
        if resp.take_one::<bool>(2)? {
            Ok(())
        } else {
            Err(NovaError::NotFound(format!("payment record {record_id}")))
        }
    }

    // ---- dashboard -------------------------------------------------

    async fn fetch_has_tax_profile(
        &self,
        db: &NovaDB,
        person_id: &str,
        year: TaxYear,
    ) -> Result<bool, NovaError> {
        let mut resp = db
            .exec(self.tax_repo.query_select_tax_profile(person_id, year))
            .await?;
        Ok(resp.take_opt::<serde_json::Value>(0)?.is_some())
    }

    async fn fetch_has_tax_rules(&self, db: &NovaDB, year: TaxYear) -> Result<bool, NovaError> {
        let mut resp = db.exec(self.tax_repo.query_select_tax_rules(year)).await?;
        Ok(resp.take_opt::<serde_json::Value>(0)?.is_some())
    }

    /// Whole-year rollup. Deliberately does not require tax rules or a
    /// profile — it reports whether those exist so the dashboard can render
    /// numbers and a "finish setup" prompt at the same time.
    #[instrument(skip(self))]
    pub async fn get_summary(
        &self,
        person_id: &str,
        year: TaxYear,
    ) -> Result<FinanceSummary, NovaError> {
        // Five independent reads, sharing one connection and running
        // concurrently instead of paying for five separate
        // connect+signin+query round trips in sequence.
        let db = self.db().await?;
        let (income, expenses, payments, has_tax_profile, has_tax_rules) = tokio::try_join!(
            self.fetch_income(&db, person_id, year),
            self.fetch_expenses(&db, person_id, year),
            self.fetch_payments(&db, person_id, year),
            self.fetch_has_tax_profile(&db, person_id, year),
            self.fetch_has_tax_rules(&db, year),
        )?;

        let wages: Money = income
            .iter()
            .filter(|r| r.kind == IncomeKind::Wage)
            .map(|r| r.amount)
            .sum();
        let se_gross: Money = income
            .iter()
            .filter(|r| r.kind == IncomeKind::SelfEmploymentProfit)
            .map(|r| r.amount)
            .sum();
        let se_deductible_expenses: Money = expenses
            .iter()
            .filter(|e| e.deductible)
            .map(|e| e.amount)
            .sum();
        let non_deductible_expenses: Money = expenses
            .iter()
            .filter(|e| !e.deductible)
            .map(|e| e.amount)
            .sum();

        let withholding_to_date: Money = income
            .iter()
            .filter_map(|r| r.withholding.as_ref().map(|w| w.total()))
            .sum();
        let payments_made: Money = payments.iter().map(|p| p.amount).sum();

        Ok(FinanceSummary {
            year,
            ytd: YtdTotals {
                wages,
                se_gross,
                se_deductible_expenses,
                se_net: se_gross - se_deductible_expenses,
                non_deductible_expenses,
            },
            withholding_to_date,
            payments_made,
            income_count: income.len(),
            expense_count: expenses.len(),
            has_tax_profile,
            has_tax_rules,
        })
    }
}
