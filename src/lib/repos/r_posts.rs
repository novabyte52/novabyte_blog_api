use serde::Serialize;
use surrealdb::sql::Thing;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::post::{CreatePostArgs, Drafted, Post, PostVersion, Published, SelectPostArgs};

use super::r_meta::MetaRepo;

// TODO: writer is ONLY public for transactions
// i need to find a better way to access the transaction stuff
// also, transaction stuff is odd cause i imagine the transaction
// only lasts through the same instance and connection, meaning
// that if i start a transaction on the writer and call reader
// functions those reader functions probably won't be accounted
// for in the transaction. though, i probably do only need
// transaction functionality for the writer?
pub struct PostsRepo {
    reader: NovaDB,
    pub writer: NovaDB,
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

    pub async fn insert_post(&self, created_by: Thing) -> Post {
        println!("r: insert post");

        let meta = self
            .meta
            .insert_meta(InsertMetaArgs {
                created_by: created_by.clone(),
            })
            .await;

        let query = self.writer.query_single_with_args::<Post, CreatePostArgs>(
            r#"
                    CREATE 
                        post:ulid()
                    SET
                        meta = $meta;
                "#,
            CreatePostArgs { meta: meta.id },
        );

        match query.await {
            Ok(o) => match o {
                Some(p) => p,
                None => panic!("No post created"),
            },
            Err(e) => panic!("Error creating post: {:#?}", e),
        }
    }

    pub async fn select_post(&self, post_id: Thing) -> Post {
        println!("r: select post: {}", post_id);

        let query = self
            .reader
            .query_single_with_args("SELECT * FROM $post_id;", SelectPostArgs { post_id });

        match query.await {
            Ok(r) => match r {
                Some(r) => r,
                None => panic!("no post returned"),
            },
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    pub async fn draft_post(
        &self,
        post_id: Thing,
        title: String,
        markdown: String,
        person_id: Thing,
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
                person_id,
                post_id,
                title,
                markdown,
                published,
            },
        );

        match query.await {
            Ok(p) => match p {
                Some(p) => p,
                None => panic!("Nothing returned when drafting post..."),
            },
            Err(e) => panic!("error drafting post: {:#?}", e),
        }
    }

    pub async fn select_drafted_posts(&self) -> Vec<PostVersion> {
        println!("r: select drafted posts");
        let query = self.reader.query_many(
            r#"
                SELECT
                    out as id,
                    id as draft_id,
                    title,
                    markdown,
                    at,
                    in as author,
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

    pub async fn select_current_draft(&self, post_id: Thing) -> PostVersion {
        let query = self.reader.query_single_with_args(
            r#"
            SELECT
                out as id,
                id as draft_id,
                title,
                markdown,
                at,
                in as author,
                published
            FROM drafted
            WHERE out = $post_id
                AND published = false
            ORDER BY at DESC
            LIMIT 1;
        "#,
            SelectPostArgs {
                post_id: post_id.clone(),
            },
        );

        match query.await {
            Ok(o) => match o {
                Some(p) => p,
                None => panic!("Current draft version not found for {:#?}", post_id),
            },
            Err(e) => panic!("error selecting drafted post version: {:#?}", e),
        }
    }

    pub async fn publish_new_draft(
        &self,
        post_id: Thing,
        title: String,
        markdown: String,
        person_id: Thing,
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
                person_id,
                post_id,
                title,
                markdown,
                published: true,
            },
        );

        match query.await {
            Ok(p) => match p {
                Some(p) => p,
                None => panic!("Nothing returned when publishing post..."),
            },
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }

    pub async fn publish_draft(&self, draft_id: Thing) -> Published {
        println!("r: publish post");
        let query = self
            .reader
            .query_single_with_args("UPDATE $draft_id SET published = true;", draft_id);

        match query.await {
            Ok(p) => match p {
                Some(p) => p,
                None => panic!("Nothing returned when publishing post..."),
            },
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }

    pub async fn select_published_posts(&self) -> Vec<PostVersion> {
        println!("r: select published posts");
        let query = self.reader.query_many(
            r#"
                SELECT
                    out as id,
                    id as draft_id,
                    title,
                    markdown,
                    at,
                    in as author,
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
}
