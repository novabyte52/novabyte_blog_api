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
                (IF !type::is::none(modified_by) THEN fn::string_id(modified_by) END) as modified_by,
                deleted_on,
                (IF !type::is::none(deleted_by) THEN fn::string_id(deleted_by) END) as deleted_by,
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
