use crate::db::nova_db::NovaQuery;
use crate::models::tax_info::{TaxRules, TaxYear, UpsertTaxProfileArgs};
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

/// Query builders for tax configuration: the per-person, per-year
/// [`TaxProfile`](crate::models::tax_info::TaxProfile) and the shared
/// per-year [`TaxRules`] snapshot.
#[derive(Debug, Clone)]
pub struct TaxRepo {
    meta: MetaRepo,
}

impl Default for TaxRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl TaxRepo {
    pub fn new() -> Self {
        Self {
            meta: MetaRepo::new(),
        }
    }

    // ---- tax profile -----------------------------------------------

    const PROFILE_FIELDS: &'static str = r#"
        year,
        filing_status,
        state,
        city,
        prefer_itemized,
        deductions,
        prior_year_total_tax,
        prior_year_agi,
    "#;

    fn select_profile(&self) -> String {
        format!(
            r#"
            fn::string_id(id) as id,
            fn::string_id(person) as person,
            {}
            {}
            "#,
            Self::PROFILE_FIELDS,
            self.meta.select_meta_string
        )
    }

    pub fn query_select_tax_profile(&self, person_id: &str, year: TaxYear) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT {}
            FROM ONLY tax_profile
            WHERE person = $person AND year = $year AND meta.deleted_on IS NONE
            LIMIT 1;
            "#,
            self.select_profile()
        );
        NovaQuery::new(sql)
            .bind("person", thing_from_string(person_id))
            .bind_json("year", &year)
    }

    /// Statement indices: 0 `LET $meta_id`, 1 `CREATE meta`,
    /// 2 `LET $rec_id`, 3 `CREATE`, 4 `SELECT`.
    pub fn query_insert_tax_profile(
        &self,
        person_id: &str,
        year: TaxYear,
        args: &UpsertTaxProfileArgs,
    ) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            LET $rec_id = tax_profile:ulid();

            CREATE $rec_id
            SET
                person = $person,
                year = $year,
                filing_status = $filing_status,
                state = $state,
                city = $city,
                prefer_itemized = $prefer_itemized,
                deductions = $deductions,
                prior_year_total_tax = $prior_year_total_tax,
                prior_year_agi = $prior_year_agi,
                meta = $meta_id;

            SELECT {} FROM ONLY tax_profile WHERE id = $rec_id LIMIT 1;
            "#,
            self.meta.sql_create_meta("$meta_id"),
            self.select_profile()
        );
        self.bind_profile(NovaQuery::new(sql), person_id, year, args)
            .bind("created_by", thing_from_string(person_id))
    }

    pub fn query_update_tax_profile(
        &self,
        person_id: &str,
        year: TaxYear,
        args: &UpsertTaxProfileArgs,
    ) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            SELECT {} FROM ONLY tax_profile WHERE id = $rec_id LIMIT 1;
            "#,
            self.meta.sql_scoped_update(
                "tax_profile",
                "person = $person AND year = $year",
                "filing_status = $filing_status, state = $state, city = $city, \
                 prefer_itemized = $prefer_itemized, deductions = $deductions, \
                 prior_year_total_tax = $prior_year_total_tax, \
                 prior_year_agi = $prior_year_agi",
                "$person",
            ),
            self.select_profile()
        );
        self.bind_profile(NovaQuery::new(sql), person_id, year, args)
    }

    fn bind_profile(
        &self,
        q: NovaQuery,
        person_id: &str,
        year: TaxYear,
        args: &UpsertTaxProfileArgs,
    ) -> NovaQuery {
        q.bind("person", thing_from_string(person_id))
            .bind_json("year", &year)
            .bind_json("filing_status", &args.filing_status)
            .bind_json("state", &args.state)
            .bind_json("city", &args.city)
            .bind("prefer_itemized", args.prefer_itemized)
            .bind_json("deductions", &args.deductions)
            .bind_json("prior_year_total_tax", &args.prior_year_total_tax)
            .bind_json("prior_year_agi", &args.prior_year_agi)
    }

    // ---- tax rules -------------------------------------------------
    //
    // One row per year, holding the whole rule snapshot as a nested object.
    // Rules are shared across people — they describe the law, not the person —
    // so unlike everything else here they are not scoped by `person`.

    pub fn query_select_tax_rules(&self, year: TaxYear) -> NovaQuery {
        NovaQuery::new(
            r#"
            SELECT rules
            FROM ONLY tax_rules
            WHERE year = $year AND meta.deleted_on IS NONE
            LIMIT 1;
            "#,
        )
        .bind_json("year", &year)
    }

    /// Years that have a configured rule set, so the UI can offer a year picker
    /// and flag the ones still needing to be seeded.
    pub fn query_select_rule_years(&self) -> NovaQuery {
        NovaQuery::new(
            "SELECT VALUE year FROM tax_rules WHERE meta.deleted_on IS NONE ORDER BY year DESC;",
        )
    }

    pub fn query_insert_tax_rules(&self, created_by: &str, rules: &TaxRules) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            LET $rec_id = tax_rules:ulid();

            CREATE $rec_id
            SET
                year = $year,
                rules = $rules,
                meta = $meta_id;

            SELECT rules FROM ONLY tax_rules WHERE id = $rec_id LIMIT 1;
            "#,
            self.meta.sql_create_meta("$meta_id")
        );
        NovaQuery::new(sql)
            .bind("created_by", thing_from_string(created_by))
            .bind_json("year", &rules.year)
            .bind_json("rules", rules)
    }

    pub fn query_update_tax_rules(&self, modified_by: &str, rules: &TaxRules) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            SELECT rules FROM ONLY tax_rules WHERE id = $rec_id LIMIT 1;
            "#,
            self.meta.sql_scoped_update(
                "tax_rules",
                "year = $year",
                "rules = $rules",
                "$modified_by"
            )
        );
        NovaQuery::new(sql)
            .bind("modified_by", thing_from_string(modified_by))
            .bind_json("year", &rules.year)
            .bind_json("rules", rules)
    }
}
