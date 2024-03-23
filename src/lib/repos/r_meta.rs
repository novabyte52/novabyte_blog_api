use surrealdb::sql::Thing;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::{InsertMetaArgs, Meta};

pub struct MetaRepo {
    reader: NovaDB,
    writer: NovaDB,
}

impl MetaRepo {
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

        Self { reader, writer }
    }

    pub async fn insert_meta(&self, new_meta: InsertMetaArgs) -> Meta<()> {
        println!("r: insert meta - {:#?}", new_meta);

        let create_meta = self
            .writer
            .query_single_with_args::<Meta<()>, InsertMetaArgs>(
                r#"
                    CREATE
                        meta:ulid()
                    SET
                        created_by = $created_by
                "#,
                new_meta,
            );

        let response = match create_meta.await {
            Ok(m) => m,
            Err(e) => panic!("Meta creation failed: {:#?}", e),
        };

        match response {
            Some(m) => m,
            None => panic!("No meta returned, potential issue creating meta for person"),
        }
    }

    pub async fn select_meta(&self, meta_id: Thing) -> Option<Meta<()>> {
        println!("r: select meta: {}", meta_id);

        let query = self
            .reader
            .query_single_with_args::<Meta<()>, (String, Thing)>(
                "SELECT * FROM meta WHERE id = $id",
                ("id".into(), meta_id),
            );

        match query.await {
            Ok(r) => r,
            Err(e) => panic!("Meta object not found: {:#?}", e),
        }
    }
}
