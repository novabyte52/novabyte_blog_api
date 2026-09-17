use crate::db::nova_db::NovaQuery;
use crate::models::finance::{UpsertExpenseArgs, UpsertIncomeArgs, UpsertPaymentArgs};
use crate::models::tax_info::TaxYear;
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

/// Query builders for the personal-finance ledger: income, expenses, and
/// estimated payments already made.
///
/// Every query is scoped to a single `person` — these records are private
/// bookkeeping, not shared blog content.
#[derive(Debug, Clone)]
pub struct FinanceRepo {
    meta: MetaRepo,
}

/// `income_record`/`expense_record` scope a year by matching a `YYYY-`
/// prefix on the stored `date` string, while `tax_payment` matches an
/// explicit numeric `year` field. Deliberately different, not an oversight:
/// income and expense rows have no `year` column of their own — the date
/// they were incurred is their only temporal anchor, so a prefix match on
/// it is the only option without adding a redundant column that would need
/// to be kept in sync with `date` on every write. A payment, by contrast,
/// belongs to a `(year, quarter)` estimated-tax period that need not equal
/// the calendar year of the day it was actually paid (an estimate can be
/// mailed early or late), so `year` has to be its own stored field rather
/// than derived from `date` — and once it's a real field, matching it
/// directly is both correct and cheaper than a string-prefix scan.
fn year_prefix(year: TaxYear) -> String {
    format!("{}-", year.0)
}

impl Default for FinanceRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl FinanceRepo {
    pub fn new() -> Self {
        Self {
            meta: MetaRepo::new(),
        }
    }

    fn select_fields(&self, extra: &str) -> String {
        format!(
            r#"
            fn::string_id(id) as id,
            fn::string_id(person) as person,
            {extra}
            {}
            "#,
            self.meta.select_meta_string
        )
    }

    /// Builds an "insert a new record + meta" query: the shared meta
    /// preamble, `LET $rec_id = <table>:ulid()`, a `CREATE` setting
    /// `person` plus `set_clause`, and a trailing `SELECT` of
    /// `select_fields` so the caller can read the created row back. Every
    /// insert in this repo (income, expense, payment) has this exact shape
    /// and differs only in table name and fields.
    fn sql_insert(&self, table: &str, set_clause: &str, select_fields: &str) -> String {
        format!(
            r#"
            {}
            LET $rec_id = {table}:ulid();

            CREATE $rec_id
            SET
                person = $person,
                {set_clause},
                meta = $meta_id;

            SELECT {select_fields} FROM ONLY {table} WHERE id = $rec_id LIMIT 1;
            "#,
            self.meta.sql_create_meta("$meta_id")
        )
    }

    /// [`MetaRepo::sql_scoped_update`] plus the trailing `SELECT` every
    /// update in this repo appends to read the updated row back.
    fn sql_scoped_update_and_select(
        &self,
        table: &str,
        resolve_where: &str,
        set_clause: &str,
        meta_touch_by: &str,
        select_fields: &str,
    ) -> String {
        format!(
            "{}\nSELECT {select_fields} FROM ONLY {table} WHERE id = $rec_id LIMIT 1;",
            self.meta
                .sql_scoped_update(table, resolve_where, set_clause, meta_touch_by)
        )
    }

    /// Builds a "this person's not-deleted records for a year, newest
    /// first" query. `year_where` supplies the year-matching technique —
    /// see [`year_prefix`] for why income/expense and payment use different
    /// ones.
    fn sql_select_for_year(&self, table: &str, year_where: &str, select_fields: &str) -> String {
        format!(
            r#"
            SELECT {select_fields}
            FROM {table}
            WHERE person = $person
                AND {year_where}
                AND meta.deleted_on IS NONE
            ORDER BY date DESC;
            "#
        )
    }

    // ---- income ----------------------------------------------------

    const INCOME_FIELDS: &'static str = "kind, payer, amount, date, withholding, category, note,";
    const INCOME_SET: &'static str = "kind = $kind, payer = $payer, amount = $amount, \
        date = $date, withholding = $withholding, category = $category, note = $note";

    fn bind_income_args(q: NovaQuery, args: &UpsertIncomeArgs) -> NovaQuery {
        q.bind_json("kind", &args.kind)
            .bind_json("payer", &args.payer)
            .bind_json("amount", &args.amount)
            .bind_json("date", &DateArg(args.date))
            .bind_json("withholding", &args.withholding)
            .bind_json("category", &args.category)
            .bind_json("note", &args.note)
    }

    /// Create an income record + meta. Run inside a transaction.
    ///
    /// Statement indices (SurrealDB v3 counts `LET`):
    /// 0 `LET $meta_id`, 1 `CREATE meta`, 2 `LET $rec_id`, 3 `CREATE`, 4 `SELECT`.
    pub fn query_insert_income(&self, person_id: &str, args: &UpsertIncomeArgs) -> NovaQuery {
        let sql = self.sql_insert(
            "income_record",
            Self::INCOME_SET,
            &self.select_fields(Self::INCOME_FIELDS),
        );
        let q = NovaQuery::new(sql)
            .bind("created_by", thing_from_string(person_id))
            .bind("person", thing_from_string(person_id));
        Self::bind_income_args(q, args)
    }

    pub fn query_update_income(
        &self,
        person_id: &str,
        record_id: &str,
        args: &UpsertIncomeArgs,
    ) -> NovaQuery {
        let sql = self.sql_scoped_update_and_select(
            "income_record",
            "id = $rec_id_in AND person = $person",
            Self::INCOME_SET,
            "$person",
            &self.select_fields(Self::INCOME_FIELDS),
        );
        let q = NovaQuery::new(sql)
            .bind("rec_id_in", thing_from_string(record_id))
            .bind("person", thing_from_string(person_id));
        Self::bind_income_args(q, args)
    }

    pub fn query_select_income_for_year(&self, person_id: &str, year: TaxYear) -> NovaQuery {
        let sql = self.sql_select_for_year(
            "income_record",
            "string::starts_with(date, $year)",
            &self.select_fields(Self::INCOME_FIELDS),
        );
        NovaQuery::new(sql)
            .bind("person", thing_from_string(person_id))
            .bind("year", year_prefix(year))
    }

    pub fn query_soft_delete_income(&self, person_id: &str, record_id: &str) -> NovaQuery {
        self.soft_delete("income_record", person_id, record_id)
    }

    // ---- expenses --------------------------------------------------

    const EXPENSE_FIELDS: &'static str = "amount, date, category, deductible, note,";
    const EXPENSE_SET: &'static str = "amount = $amount, date = $date, category = $category, \
        deductible = $deductible, note = $note";

    fn bind_expense_args(q: NovaQuery, args: &UpsertExpenseArgs) -> NovaQuery {
        q.bind_json("amount", &args.amount)
            .bind_json("date", &DateArg(args.date))
            .bind_json("category", &args.category)
            .bind("deductible", args.deductible)
            .bind_json("note", &args.note)
    }

    pub fn query_insert_expense(&self, person_id: &str, args: &UpsertExpenseArgs) -> NovaQuery {
        let sql = self.sql_insert(
            "expense_record",
            Self::EXPENSE_SET,
            &self.select_fields(Self::EXPENSE_FIELDS),
        );
        let q = NovaQuery::new(sql)
            .bind("created_by", thing_from_string(person_id))
            .bind("person", thing_from_string(person_id));
        Self::bind_expense_args(q, args)
    }

    pub fn query_update_expense(
        &self,
        person_id: &str,
        record_id: &str,
        args: &UpsertExpenseArgs,
    ) -> NovaQuery {
        let sql = self.sql_scoped_update_and_select(
            "expense_record",
            "id = $rec_id_in AND person = $person",
            Self::EXPENSE_SET,
            "$person",
            &self.select_fields(Self::EXPENSE_FIELDS),
        );
        let q = NovaQuery::new(sql)
            .bind("rec_id_in", thing_from_string(record_id))
            .bind("person", thing_from_string(person_id));
        Self::bind_expense_args(q, args)
    }

    pub fn query_select_expenses_for_year(&self, person_id: &str, year: TaxYear) -> NovaQuery {
        let sql = self.sql_select_for_year(
            "expense_record",
            "string::starts_with(date, $year)",
            &self.select_fields(Self::EXPENSE_FIELDS),
        );
        NovaQuery::new(sql)
            .bind("person", thing_from_string(person_id))
            .bind("year", year_prefix(year))
    }

    pub fn query_soft_delete_expense(&self, person_id: &str, record_id: &str) -> NovaQuery {
        self.soft_delete("expense_record", person_id, record_id)
    }

    // ---- payments --------------------------------------------------

    const PAYMENT_FIELDS: &'static str = "year, quarter, jurisdiction, amount, date, note,";

    pub fn query_insert_payment(&self, person_id: &str, args: &UpsertPaymentArgs) -> NovaQuery {
        let sql = self.sql_insert(
            "tax_payment",
            "year = $year, quarter = $quarter, jurisdiction = $jurisdiction, \
             amount = $amount, date = $date, note = $note",
            &self.select_fields(Self::PAYMENT_FIELDS),
        );

        NovaQuery::new(sql)
            .bind("created_by", thing_from_string(person_id))
            .bind("person", thing_from_string(person_id))
            .bind_json("year", &args.year)
            .bind_json("quarter", &args.quarter)
            .bind_json("jurisdiction", &args.jurisdiction)
            .bind_json("amount", &args.amount)
            .bind_json("date", &DateArg(args.date))
            .bind_json("note", &args.note)
    }

    pub fn query_select_payments_for_year(&self, person_id: &str, year: TaxYear) -> NovaQuery {
        let sql = self.sql_select_for_year(
            "tax_payment",
            "year = $year",
            &self.select_fields(Self::PAYMENT_FIELDS),
        );
        NovaQuery::new(sql)
            .bind("person", thing_from_string(person_id))
            .bind_json("year", &year)
    }

    pub fn query_soft_delete_payment(&self, person_id: &str, record_id: &str) -> NovaQuery {
        self.soft_delete("tax_payment", person_id, record_id)
    }

    // ---- shared ----------------------------------------------------

    /// Soft delete via `meta.deleted_on`, matching how sessions are retired in
    /// [`PersonsRepo`](crate::repos::r_persons::PersonsRepo). The `person`
    /// guard means one admin can never delete another's records.
    ///
    /// Guarded by `IF $meta_id != NONE` for the same reason as
    /// [`MetaRepo::sql_scoped_update`](super::r_meta::MetaRepo::sql_scoped_update):
    /// `UPDATE NONE` is a runtime error in SurrealDB, not a no-op, so an
    /// unguarded `UPDATE $meta_id` on a record that doesn't exist or isn't
    /// the caller's would throw — and because nothing here ever calls
    /// `.take()` on that specific statement, the caller previously never saw
    /// it. The trailing `RETURN` is the one canonical signal of whether a row
    /// was actually deleted; callers must read it, not assume success.
    fn soft_delete(&self, table: &str, person_id: &str, record_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            LET $meta_id = (
                SELECT meta FROM ONLY {table}
                WHERE id = $rec_id AND person = $person AND meta.deleted_on IS NONE
                LIMIT 1
            ).meta;

            IF $meta_id != NONE {{
                UPDATE $meta_id SET deleted_on = time::now(), deleted_by = $person;
            }};

            RETURN $meta_id IS NOT NONE;
            "#
        );
        NovaQuery::new(sql)
            .bind("rec_id", thing_from_string(record_id))
            .bind("person", thing_from_string(person_id))
    }
}

/// Wrapper that serializes a bare [`time::Date`] as `YYYY-MM-DD` for binding.
///
/// `Date`'s own `Serialize` impl is not the format this project stores, and
/// `#[serde(with = ...)]` can't be applied to a naked binding argument.
struct DateArg(time::Date);

impl serde::Serialize for DateArg {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        crate::models::date_iso::serialize(&self.0, s)
    }
}
