use crate::db::nova_db::NovaQuery;
use crate::utils::thing_from_string;
use surrealdb::types::RecordId;

#[derive(Debug, Clone)]
pub struct MetaRepo {
    pub select_meta_string: String,
}

pub fn select_meta_string() -> String {
    r#"
        meta,
        (
            SELECT
                fn::string_id(id) as id,
                fn::string_id(created_by) as created_by,
                modified_on,
                (IF !type::is_none(modified_by) THEN fn::string_id(modified_by) END) as modified_by,
                deleted_on,
                (IF !type::is_none(deleted_by) THEN fn::string_id(deleted_by) END) as deleted_by,
                *
            FROM ONLY meta
            WHERE id = $parent.meta
            LIMIT 1
        ) as meta
    "#
    .to_string()
}

impl MetaRepo {
    pub fn new() -> Self {
        Self {
            select_meta_string: select_meta_string(),
        }
    }

    /// SQL snippet: create a new meta record and store its id in `meta_var`.
    ///
    /// Requires `$created_by` to be bound in the surrounding query.
    pub fn sql_create_meta(&self, meta_var: &str) -> String {
        format!(
            r#"
            LET {meta_var} = meta:ulid();
            CREATE {meta_var}
            SET
                created_by = $created_by,
                created_on = time::now(),
                modified_by = NONE,
                modified_on = NONE,
                deleted_by = NONE,
                deleted_on = NONE;
            "#
        )
    }

    /// SQL snippet: resolve `$rec_id` from `table` via `resolve_where` — a
    /// `WHERE` clause that must match at most one row and, critically, is
    /// the *only* place ownership/existence is checked — then apply
    /// `set_clause` to it and stamp its meta as touched by `meta_touch_by`.
    ///
    /// Every following statement in the surrounding query (a trailing
    /// `SELECT ... WHERE id = $rec_id`, say) is scoped by construction: it
    /// only ever sees the record `resolve_where` matched. There is no
    /// separate `WHERE person = $person` on a later statement for a query
    /// author to forget, which is how an update's meta-stamp and its
    /// caller-facing `SELECT` previously ended up touching and returning
    /// records that didn't belong to the caller — the `resolve_where` guard
    /// exists exactly once and covers everything downstream of it.
    ///
    /// Wrapped in `IF $rec_id != NONE` because `UPDATE NONE` is a runtime
    /// error in SurrealDB, not a no-op: when `resolve_where` matches
    /// nothing, `$rec_id` is `NONE`, both `UPDATE`s are skipped, and the
    /// trailing `SELECT` cleanly returns nothing instead of the query
    /// erroring out.
    ///
    /// The caller must bind whatever variables `resolve_where`, `set_clause`,
    /// and `meta_touch_by` reference.
    pub fn sql_scoped_update(
        &self,
        table: &str,
        resolve_where: &str,
        set_clause: &str,
        meta_touch_by: &str,
    ) -> String {
        format!(
            r#"
            LET $rec_id = (SELECT id FROM ONLY {table} WHERE {resolve_where} LIMIT 1).id;

            IF $rec_id != NONE {{
                UPDATE $rec_id SET {set_clause};

                UPDATE (SELECT meta FROM ONLY $rec_id LIMIT 1).meta
                SET modified_on = time::now(), modified_by = {meta_touch_by};
            }};
            "#
        )
    }

    /// Standalone query: select a Meta record by id.
    pub fn query_select_meta(&self, meta_id: &str) -> NovaQuery {
        let sql = r#"
            SELECT
                fn::string_id(id) as id,
                fn::string_id(created_by) as created_by,
                modified_on,
                deleted_on,
                *
            FROM ONLY meta
            WHERE id = $id
            LIMIT 1;
        "#;
        NovaQuery::new(sql).bind("id", thing_from_string(meta_id))
    }

    pub fn record_id(id: &str) -> RecordId {
        thing_from_string(id)
    }
}
