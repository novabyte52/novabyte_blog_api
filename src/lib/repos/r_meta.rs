use surrealdb::sql::Thing;
use tracing::{error, info, instrument};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::{InsertMetaArgs, Meta};
use crate::utils::thing_from_string;

#[derive(Debug)]
pub struct MetaRepo {
    reader: NovaDB,
    writer: NovaDB,
    pub select_meta_string: String,
}

impl MetaRepo {
    #[instrument]
    pub async fn new() -> Self {
        let reader = NovaDB::new(SurrealDBConnection {
            address: "127.0.0.1:52000",
            username: "root",
            password: "root",
            namespace: "test",
            database: "novabyte.blog",
        })
        .await;

        let writer = NovaDB::new(SurrealDBConnection {
            address: "127.0.0.1:52000",
            username: "root",
            password: "root",
            namespace: "test",
            database: "novabyte.blog",
        })
        .await;

        Self {
            reader,
            writer,
            select_meta_string: r#"
                # potential meta selection string
                meta,
                (
                    SELECT
                        fn::string_id(id) as id,
                        fn::string_id(created_by) as created_by,
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

    #[instrument]
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
                    LET $ulid_id = meta:ulid();

                    CREATE
                        $ulid_id
                    SET
                        created_by = $created_by;
                    
                    SELECT
                        fn::string_id(id) as id,
                        fn::string_id(created_by) as created_by,
                        *
                    FROM meta
                    WHERE id = $ulid_id;
                "#,
            ("created_by", created_by),
            2,
        );

        match query.await {
            Some(m) => m,
            None => {
                if tran_conn.is_some() {
                    conn.cancel_tran().await;
                    info!("transaction canceled")
                }
                error!("No meta returned, potential issue creating meta");
                panic!();
            }
        }
    }

    #[instrument]
    pub async fn select_meta(&self, meta_id: &String) -> Option<Meta<()>> {
        info!("r: select meta: {}", meta_id);

        let meta_thing = thing_from_string(meta_id);

        self.reader
            .query_single_with_args::<Meta<()>, (&str, Thing)>(
                "SELECT fn::string_id(id) as id, fn::string_id(created_by) as created_by, * FROM meta WHERE id = $id",
                ("id", meta_thing),
            )
            .await
    }
}
