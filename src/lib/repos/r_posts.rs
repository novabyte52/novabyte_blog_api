use serde::Serialize;
use surrealdb::sql::Thing;
use tracing::{error, info};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::post::{CreatePostArgs, Drafted, Post, PostHydrated, PostVersion, Published};
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

pub struct PostsRepo {
    reader: NovaDB,
    writer: NovaDB,
    meta: MetaRepo,
}

#[derive(Debug, Serialize)]
struct DraftedArgs {
    person_id: Thing,
    post_id: Thing,
    title: String,
    markdown: String,
    published: bool,
}

impl PostsRepo {
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
            meta: MetaRepo::new().await,
        }
    }

    pub async fn insert_post(&self, created_by: String, tran_conn: &NovaDB) -> Post {
        println!("r: insert post");

        let meta = self
            .meta
            .insert_meta(
                InsertMetaArgs {
                    created_by: created_by.clone(),
                },
                Some(tran_conn),
            )
            .await;

        let query = self
            .writer
            .query_single_with_args_specify_result::<Post, CreatePostArgs>(
                r#"
                    LET $post_id = post:ulid();

                    CREATE 
                        $post_id
                    SET
                        meta = $meta;
                    
                    SELECT * FROM post WHERE id = $post_id;
                "#,
                CreatePostArgs {
                    meta: thing_from_string(&meta.id),
                },
                2,
            );

        match query.await {
            Some(p) => p,
            None => {
                tran_conn.cancel_tran().await;
                error!("No post returned after creation, cancelling transaction");
                panic!();
            }
        }
    }

    pub async fn select_post(&self, post_id: String) -> Post {
        println!("r: select post: {}", post_id);

        let query = self.reader.query_single_with_args(
            "SELECT fn::string_id(id) as id, fn::string_id(meta) as meta FROM $post_id;",
            ("post_id", thing_from_string(&post_id)),
        );

        match query.await {
            Some(r) => r,
            None => panic!("no post returned"),
        }
    }

    pub async fn select_posts(&self) -> Vec<PostHydrated> {
        println!("r: select posts");

        let formatted_query = format!(
            r#"
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(meta.created_by) as created_by,
                    meta.created_on as created_on,
                    array::first(
                        (
                            SELECT
                            at,
                            title
                            FROM drafted
                            WHERE out = $parent.id
                            ORDER BY at DESC
                            LIMIT 1
                        ).title
                    ) as working_title,
                    {}
                FROM post
                ORDER BY created_on DESC;
            "#,
            self.meta.select_meta_string.clone()
        );

        let query = self.reader.query_many(&formatted_query);

        match query.await {
            Ok(r) => r,
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    pub async fn select_post_drafts(&self, post_id: String) -> Vec<PostVersion> {
        println!("r: select post drafts");

        let query = self.reader.query_many_with_args(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published
                FROM drafted
                WHERE out = $post_id
                ORDER BY at DESC;
            "#,
            ("post_id", thing_from_string(&post_id)),
        );

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    pub async fn draft_post(
        &self,
        post_id: String,
        title: String,
        markdown: String,
        person_id: String,
        published: bool,
    ) -> Drafted {
        println!("r: draft post");
        let query = self.reader.query_single_with_args::<Drafted, DraftedArgs>(
            r#"
                RELATE $person_id->drafted->$post_id
                    SET
                        id = drafted:ulid(),
                        title = $title,
                        markdown = $markdown,
                        published = $published,
                        at = time::now();
            "#,
            DraftedArgs {
                person_id: thing_from_string(&person_id),
                post_id: thing_from_string(&post_id),
                title,
                markdown,
                published,
            },
        );

        match query.await {
            Some(p) => p,
            None => panic!("Nothing returned when drafting post..."),
        }
    }

    pub async fn select_drafted_posts(&self) -> Vec<PostVersion> {
        println!("r: select drafted posts");
        let query = self.reader.query_many(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published
                FROM drafted
                WHERE published = false
                ORDER BY at DESC;
            "#,
        );

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting drafted posts: {:#?}", e),
        }
    }

    pub async fn select_current_draft(&self, post_id: String) -> PostVersion {
        let query = self.reader.query_single_with_args(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published
            FROM drafted
            WHERE out = $post_id
                AND published = false
            ORDER BY at DESC
            LIMIT 1;
        "#,
            ("post_id", thing_from_string(&post_id)),
        );

        match query.await {
            Some(p) => p,
            None => panic!("Current draft version not found for {:#?}", post_id),
        }
    }

    pub async fn publish_new_draft(
        &self,
        post_id: String,
        title: String,
        markdown: String,
        person_id: String,
    ) -> Published {
        println!("r: publish post");
        let query = self.reader.query_single_with_args(
            r#"
                RELATE $person_id->drafted:ulid()->$post_id
                    SET
                        title = $title,
                        markdown = $markdown,
                        published = true,
                        at = time::now();
            "#,
            DraftedArgs {
                person_id: thing_from_string(&person_id),
                post_id: thing_from_string(&post_id),
                title,
                markdown,
                published: true,
            },
        );

        match query.await {
            Some(p) => p,
            None => panic!("Nothing returned when publishing post..."),
        }
    }

    pub async fn publish_draft(&self, draft_id: String) -> Published {
        println!("r: publish post");
        let query = self.reader.query_single_with_args(
            "UPDATE $draft_id SET published = true;",
            ("draft_id", thing_from_string(&draft_id)),
        );

        match query.await {
            Some(p) => p,
            None => panic!("Nothing returned when publishing post..."),
        }
    }

    pub async fn unpublish_draft(&self, draft_id: String) -> Published {
        println!("r: unpublish draft");
        let query = self.reader.query_single_with_args(
            "UPDATE $draft_id SET published = false;",
            ("draft_id", thing_from_string(&draft_id)),
        );

        match query.await {
            Some(p) => p,
            None => panic!("Nothing returned when unpublishing draft..."),
        }
    }

    pub async fn select_published_posts(&self) -> Vec<PostVersion> {
        println!("r: select published posts");
        let query = self.reader.query_many(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published
                FROM drafted
                WHERE published = true
                ORDER BY at DESC;
            "#,
        );

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }

    pub async fn unpublish_drafts_for_post_id(&self, post_id: String) -> bool {
        info!("r: unpublish_drafts_for_posT_id");
        let query = self.writer.query_none_with_args(
            r#"
            UPDATE drafted
            SET published = false
            WHERE out = $post_id;"#,
            ("post_id", thing_from_string(&post_id)),
        );

        query.await;
        true
    }

    pub async fn select_post_id_for_draft_id(&self, draft_id: &String) -> String {
        info!("r: select_post_id_for_draft_id");
        let query = self.reader.query_single_with_args(
            "SELECT fn::string_id(out) FROM drafted WHERE id = $draft_id",
            ("draft_id", thing_from_string(draft_id)),
        );

        match query.await {
            Some(id) => id,
            None => panic!("No post id found for draft: {:#?}", draft_id),
        }
    }
}
