// use itertools::Itertools;
use serde::Serialize;
use surrealdb::sql::Thing;
use tracing::{error, info, instrument};

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::{IdContainer, InsertMetaArgs};
use crate::models::post::{Drafted, Post, PostHydrated, PostVersion};
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

#[derive(Debug)]
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
            meta: MetaRepo::new().await,
        }
    }

    #[instrument(skip(self))]
    pub async fn insert_post(&self, created_by: String, tran_conn: &NovaDB) -> Post {
        info!("r: insert post");

        let meta = self
            .meta
            .insert_meta(
                InsertMetaArgs {
                    created_by: created_by.clone(),
                },
                Some(tran_conn),
            )
            .await;

        let create_post_query = format!(
            r#"
                LET $post_id = post:ulid();

                CREATE 
                    $post_id
                SET
                    meta = $meta;
                
                SELECT
                    fn::string_id(id) as id,
                    {}
                FROM post WHERE id = $post_id;
            "#,
            &self.meta.select_meta_string
        );

        let query = self
            .writer
            .query_single_with_args_specify_result::<Post, (&str, Thing)>(
                &create_post_query,
                ("meta", thing_from_string(&meta.id)),
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

    #[instrument(skip(self))]
    pub async fn select_post(&self, post_id: String) -> Post {
        info!("r: select post: {}", post_id);

        let select_post_query = format!(
            "SELECT fn::string_id(id) as id, {} FROM $post_id;",
            &self.meta.select_meta_string
        );

        let query = self
            .reader
            .query_single_with_args(&select_post_query, ("post_id", thing_from_string(&post_id)));

        match query.await {
            Some(r) => r,
            None => panic!("no post returned"),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_posts(&self) -> Vec<PostHydrated> {
        info!("r: select posts");

        let select_posts = format!(
            r#"
                SELECT
                    fn::string_id(id) as id,
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
                    meta.created_on,
                    {}
                FROM post
                ORDER BY meta.created_on DESC;
            "#,
            &self.meta.select_meta_string
        );

        let query = self.reader.query_many(&select_posts);

        match query.await {
            Ok(r) => r,
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_draft(&self, draft_id: &String) -> PostVersion {
        let select_draft = format!(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published,
                    {}
                FROM drafted
                WHERE id = $draft_id
                ORDER BY at DESC;
            "#,
            &self.meta.select_meta_string
        );

        let query = self
            .reader
            .query_single_with_args(&select_draft, ("draft_id", thing_from_string(draft_id)));

        match query.await {
            Some(p) => p,
            None => panic!("No draft found for {}!", draft_id),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_post_drafts(&self, post_id: String) -> Vec<PostVersion> {
        info!("r: select post drafts");

        let select_drafts = format!(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published,
                    {}
                FROM drafted
                WHERE out = $post_id
                ORDER BY at DESC;
            "#,
            &self.meta.select_meta_string
        );

        let query = self
            .reader
            .query_many_with_args(&select_drafts, ("post_id", thing_from_string(&post_id)));

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn create_draft(
        &self,
        post_id: String,
        title: String,
        markdown: String,
        person_id: String,
        published: bool,
        tran_conn: Option<&NovaDB>,
    ) -> PostVersion {
        let conn = match tran_conn {
            Some(t) => t,
            None => &self.writer,
        };

        let drafted_query = format!(
            r#"
                LET $drafted_id = drafted:ulid();
                LET $meta_id = SELECT meta FROM post WHERE id = $post_id;

                RELATE $person_id->drafted->$post_id
                    SET
                        id = $drafted_id,
                        title = $title,
                        markdown = $markdown,
                        published = $published,
                        at = time::now(),
                        meta = $meta_id;
                
                SELECT
                    fn::string_id(id) as draft_id,
                    fn::string_id(in) as author,
                    fn::string_id(out) as id,
                    *,
                    {}
                FROM drafted
                WHERE id = $drafted_id;
            "#,
            &self.meta.select_meta_string
        );

        let query = conn.query_single_with_args_specify_result::<PostVersion, DraftedArgs>(
            &drafted_query,
            DraftedArgs {
                person_id: thing_from_string(&person_id),
                post_id: thing_from_string(&post_id),
                title,
                markdown,
                published,
            },
            2,
        );

        match query.await {
            Some(p) => p,
            None => {
                if tran_conn.is_some() {
                    conn.cancel_tran().await;
                    info!("Cancelling transaction")
                }
                error!("Issue creating draft for {}", post_id);
                panic!();
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn select_drafted_posts(&self) -> Vec<PostVersion> {
        info!("r: select drafted posts");

        let select_drafts = format!(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published,
                    {}
                FROM drafted
                WHERE published = false
                ORDER BY at DESC;
            "#,
            &self.meta.select_meta_string
        );

        let query = self.reader.query_many(&select_drafts);

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting drafted posts: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_current_draft(&self, post_id: String) -> PostVersion {
        let select_draft = format!(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published,
                    {}
                FROM drafted
                WHERE out = $post_id
                    AND published = false
                ORDER BY at DESC
                LIMIT 1;
            "#,
            &self.meta.select_meta_string
        );

        let query = self
            .reader
            .query_single_with_args(&select_draft, ("post_id", thing_from_string(&post_id)));

        match query.await {
            Some(p) => p,
            None => panic!("Current draft version not found for {:#?}", post_id),
        }
    }

    #[instrument(skip(self))]
    pub async fn publish_new_draft(
        &self,
        post_id: String,
        title: String,
        markdown: String,
        person_id: String,
        tran_conn: Option<&NovaDB>,
    ) -> Drafted {
        info!("r: publish post");
        let conn = match tran_conn {
            Some(t) => t,
            None => &self.writer,
        };

        let drafted_query = format!(
            r#"
                LET $drafted_id = drafted:ulid();
                LET $meta_id = SELECT meta FROM post WHERE id = $post_id;

                RELATE $person_id->drafted->$post_id
                    SET
                        id = $drafted_id,
                        title = $title,
                        markdown = $markdown,
                        published = true,
                        at = time::now(),
                        meta = $meta_id;
                
                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(in) as in,
                    fn::string_id(out) as out,
                    *,
                    {}
                FROM drafted
                WHERE id = $drafted_id;
            "#,
            &self.meta.select_meta_string
        );

        let query = conn.query_single_with_args(
            &drafted_query,
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
            None => {
                if tran_conn.is_some() {
                    conn.cancel_tran().await;
                    info!("Cancelling transaction")
                }
                error!("Issue creating published draft for {}", post_id);
                panic!();
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn publish_draft(&self, draft_id: String, tran_conn: &NovaDB) -> Drafted {
        info!("r: publish post");

        let drafted_query = format!(
            r#"
                UPDATE $draft_id SET published = true;

                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(in) as in,
                    fn::string_id(out) as out,
                    *,
                    {}
                FROM drafted
                WHERE id = $draft_id;
            "#,
            &self.meta.select_meta_string
        );

        let query = tran_conn.query_single_with_args_specify_result(
            &drafted_query,
            ("draft_id", thing_from_string(&draft_id)),
            1,
        );

        match query.await {
            Some(p) => p,
            None => {
                tran_conn.cancel_tran().await;
                error!("Error publishing {}, cancelling transaction", draft_id);
                panic!();
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn unpublish_draft(&self, draft_id: String) -> Drafted {
        info!("r: unpublish draft");

        let drafted_query = format!(
            r#"
                UPDATE $draft_id SET published = false;

                SELECT
                    fn::string_id(id) as id,
                    fn::string_id(in) as in,
                    fn::string_id(out) as out,
                    *,
                    {}
                FROM drafted
                WHERE id = $draft_id;
            "#,
            &self.meta.select_meta_string
        );

        let query = self.reader.query_single_with_args_specify_result(
            &drafted_query,
            ("draft_id", thing_from_string(&draft_id)),
            1,
        );

        match query.await {
            Some(p) => p,
            None => panic!("Nothing returned when unpublishing draft..."),
        }
    }

    #[instrument(skip(self))]
    pub async fn select_published_posts(&self) -> Vec<PostVersion> {
        info!("r: select published posts");

        let select_published = format!(
            r#"
                SELECT
                    fn::string_id(out) as id,
                    fn::string_id(id) as draft_id,
                    title,
                    markdown,
                    at,
                    fn::string_id(in) as author,
                    published,
                    {}
                FROM drafted
                WHERE published = true
                ORDER BY at DESC;
            "#,
            &self.meta.select_meta_string
        );

        let query = self.reader.query_many(&select_published);

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn unpublish_drafts_for_post_id(&self, post_id: String, tran_conn: &NovaDB) -> bool {
        info!("r: unpublish_drafts_for_posT_id");
        let query = tran_conn.query_none_with_args(
            r#"
                UPDATE drafted
                    SET published = false
                WHERE out = $post_id;
            "#,
            ("post_id", thing_from_string(&post_id)),
        );

        query.await;
        true
    }

    #[instrument(skip(self))]
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

    #[instrument(skip(self))]
    pub async fn select_unpublished_post_ids(&self) -> Vec<String> {
        let query = self.reader.query_many_specify_result(
            r#"
            LET $published = SELECT out FROM drafted WHERE published = true;
            
            LET $unpublished = SELECT fn::string_id(out) as id FROM drafted WHERE out NOT IN $published.out;
            
            RETURN array::distinct($unpublished);
        "#,
            2,
        );

        let result: Vec<IdContainer> = match query.await {
            Ok(ids) => ids,
            Err(e) => panic!("Error selecting unpublished post ids: {:#?}", e),
        };

        result.into_iter().map(|c| c.id).collect()
    }
}
