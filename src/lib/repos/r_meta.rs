use crate::utils::thing_from_string;
use serde_json::json;
use surrealdb::sql::Thing;

use crate::db::nova_db::{DbOp, DbProgram};

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

    /// Create a new meta record and store its Thing in a variable you choose.
    ///
    /// Example: meta.op_create_meta("$meta_id", "created_by")
    pub fn op_create_meta(&self, meta_var: &str) -> DbOp {
        DbOp::new(
            "meta.create",
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
            ),
        )
    }

    /// Select a Meta<()> by id var (Thing var) and RETURN it.
    pub fn op_return_meta_by_var(&self, meta_var: &str) -> DbOp {
        DbOp::new(
            "meta.return",
            format!(
                r#"
                RETURN (
                    SELECT
                        fn::string_id(id) as id,
                        fn::string_id(created_by) as created_by,
                        modified_on,
                        deleted_on,
                        *
                    FROM ONLY meta
                    WHERE id = {meta_var}
                    LIMIT 1
                )[0];
                "#
            ),
        )
    }

    /// Convenience: a standalone “select meta by id” program (for reads)
    pub fn program_select_meta(&self, meta_id: &String) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "meta.select",
                r#"
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(created_by) as created_by,
                    modified_on,
                    deleted_on,
                    *
                FROM ONLY meta
                WHERE id = $id
                LIMIT 1;
                "#,
            ))
            .bind_serde(json!({ "id": thing_from_string(meta_id) }))
            .expect("binding meta_id should be serializable")
    }

    /// Helper for callers that want to pre-bind `created_by` as a Thing.
    pub fn bind_created_by(program: DbProgram, created_by: String, bind_name: &str) -> DbProgram {
        program
            .bind_serde(json!({ bind_name: thing_from_string(&created_by) }))
            .expect("binding created_by should be serializable")
    }

    /// If you need the Thing for created_by as a parameter type.
    pub fn created_by_thing(created_by: String) -> Thing {
        thing_from_string(&created_by)
    }
}
