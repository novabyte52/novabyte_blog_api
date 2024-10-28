use surrealdb::sql::Thing;
use tracing::{error, info, instrument};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::{InsertMetaArgs, Meta};
use crate::utils::thing_from_string;

#[derive(Debug, Clone)]
pub struct MetaRepo {
    reader: NovaDB,
    writer: NovaDB,
    pub select_meta_string: String,
}

impl MetaRepo {
    #[instrument]
    pub async fn new(conn: &SurrealDBConnection) -> Self {
        let reader = NovaDB::new(conn).await;

        let writer = NovaDB::new(conn).await;

        Self {
            reader,
            writer,
            // TODO: will need to update this eventually to also make the updated_by
            // and deleted_by properties into strings
            select_meta_string: r#"
                # potential meta selection string
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
            .to_string(),
        }
    }

    // TODO: create some helper functions for easy updating of meta info
    // - i.e. set_updated_at, set_updated_by, set_deleted_on, etc.
    // will probably create a meta service to transfer them to eventually
    pub async fn select_meta_string() -> String {
        r#"
            # potential meta selection string
            meta,
            (
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(created_by) as created_by,
                    modified_on,
                    deleted_on,
                    *
                FROM ONLY meta
                WHERE id = $parent.meta
                LIMIT 1
            ) as meta
        "#
        .to_string()
    }

    // TODO: probably should return Results from the repo layer instead of panicking
    #[instrument(skip(self, tran_conn))]
    pub async fn insert_meta(
        &self,
        new_meta: InsertMetaArgs,
        tran_conn: Option<&NovaDB>,
    ) -> Meta<()> {
        let conn = match tran_conn {
            Some(t) => t,
            None => &self.writer,
        };

        let created_by = thing_from_string(&new_meta.created_by);

        let query = conn.query_single_with_args_specify_result::<Meta<()>, (&str, Thing)>(
            r#"
                    LET $meta_id = meta:ulid();

                    CREATE
                        $meta_id
                    SET
                        created_by = $created_by;
                    
                    SELECT
                        fn::string_id(id) as id,
                        fn::string_id(created_by) as created_by,
                        modified_on,
                        deleted_on,
                        *
                    FROM meta
                    WHERE id = $meta_id;
                "#,
            ("created_by", created_by),
            2,
        );

        let response = match query.await {
            Ok(r) => r,
            Err(e) => {
                println!("=== error creating meta ===");
                if tran_conn.is_some() {
                    conn.cancel_tran().await;
                    info!("transaction canceled")
                }
                error!("Error creating meta: {:#?}", e);
                panic!("Error creating meta: {:#?}", e);
            }
        };

        match response {
            Some(m) => m,
            None => {
                if tran_conn.is_some() {
                    conn.cancel_tran().await;
                    info!("transaction canceled")
                }
                info!("=== ERROR ===");
                error!("No meta returned, potential issue creating meta");
                panic!("No meta returned, potential issue creating meta")
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn select_meta(&self, meta_id: &String) -> Option<Meta<()>> {
        info!("r: select meta: {}", meta_id);

        let meta_thing = thing_from_string(meta_id);

        let query = self
            .reader
            .query_single_with_args::<Meta<()>, (&str, Thing)>(
                r#"
                    SELECT
                        fn::string_id(id) as id,
                        fn::string_id(created_by) as created_by,
                        modified_on,
                        deleted_on,
                        *
                    FROM meta
                    WHERE id = $id
                "#,
                ("id", meta_thing),
            );

        match query.await {
            Ok(r) => r,
            Err(e) => {
                error!("Error selecting meta: {:#?}", e);
                panic!()
            }
        }
    }
}
