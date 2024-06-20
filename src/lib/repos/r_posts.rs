use serde::Serialize;
use surrealdb::sql::Thing;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::meta::InsertMetaArgs;
use crate::models::post::{CreatePostArgs, Drafted, Post, PostContent, Published, SelectPostArgs};

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

        let args = CreatePostArgs { meta: meta.id };

        let query = self.writer.query_single_with_args::<Post, CreatePostArgs>(
            r#"
                    CREATE 
                        post:ulid()
                    SET
                        meta = $meta;
                "#,
            args,
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

        let query = self.reader.query_single_with_args(
            "SELECT * FROM person WHERE id = $id",
            SelectPostArgs { id: post_id },
        );

        match query.await {
            Ok(r) => match r {
                Some(r) => r,
                None => panic!("no post returned"),
            },
            Err(e) => panic!("Nothing found!: {:#?}", e),
        }
    }

    // pub async fn select_posts(&self) -> Vec<Post> {
    //     println!("r: select posts");
    //     let query = self.reader.query_many("SELECT * FROM post");

    //     match query.await {
    //         Ok(p) => p,
    //         Err(e) => panic!("error selecting posts: {:#?}", e),
    //     }
    // }

    // pub async fn author_post(&self, person_id: Thing, post_id: Thing) -> bool {
    //     println!("r: author post");
    //     let query = self
    //         .writer
    //         .query_single_with_args::<bool, ((&str, Thing), (&str, Thing))>(
    //             "RELATE $personId->authored->$postId",
    //             (("personId", person_id), ("postId", post_id)),
    //         );

    //     match query.await {
    //         Ok(o) => match o {
    //             Some(p) => p,
    //             None => panic!("Nothing returned when authoring post..."),
    //         },
    //         Err(e) => panic!("error authoring post: {:#?}", e),
    //     };

    //     return true;
    // }

    // pub async fn get_post_authors(&self) -> Vec<Authored> {
    //     println!("r: get post authors");
    //     let query = self
    //         .reader
    //         .query_single("SELECT * FROM post WHERE <-authored;");

    //     match query.await {
    //         Ok(o) => match o {
    //             Some(p) => p,
    //             None => panic!("Nothing returned when getting post authors..."),
    //         },
    //         Err(e) => panic!("error getting post authors: {:#?}", e),
    //     }
    // }

    pub async fn draft_post(
        &self,
        post_id: Thing,
        title: String,
        markdown: String,
        person_id: Thing,
    ) -> Drafted {
        println!("r: draft post");
        let query = self.reader.query_single_with_args::<Drafted, DraftedArgs>(
            r#"
                RELATE $person_id->drafted->$post_id
                    SET
                        title = $title,
                        markdown = $markdown,
                        on = time::now();
            "#,
            DraftedArgs {
                person_id,
                post_id,
                title,
                markdown,
            },
        );

        match query.await {
            Ok(p) => match p {
                Some(p) => p,
                None => panic!("Nothing returned when drafting post..."),
            },
            Err(e) => panic!("error selecting drafted posts: {:#?}", e),
        }
    }

    pub async fn select_drafted_posts(&self) -> Vec<Post> {
        println!("r: select drafted posts");
        let query = self
            .reader
            .query_many("SELECT * FROM post WHERE <-drafted;");

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting drafted posts: {:#?}", e),
        }
    }

    pub async fn publish_post(
        &self,
        post_id: Thing,
        title: String,
        markdown: String,
        person_id: Thing,
    ) -> Published {
        println!("r: publish post");
        let query = self.reader.query_single_with_args(
            r#"
                RELATE $person_id->published->$post_id
                    SET
                        title = $title,
                        markdown = $markdown,
                        on = time::now();
            "#,
            DraftedArgs {
                person_id,
                post_id,
                title,
                markdown,
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

    pub async fn select_published_posts(&self) -> Vec<Post> {
        println!("r: select published posts");
        let query = self
            .reader
            .query_many("SELECT * FROM post WHERE <-published;");

        match query.await {
            Ok(p) => p,
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }

    pub async fn get_current_content(&self, post_id: Thing) -> PostContent {
        println!("r: select post content");
        let query = self.reader.query_single::<PostContent>(
            r#"
                    SELECT
                        in as author,
                        title,
                        markdown,
                        on
                    FROM drafted
                    WHERE out = $post_id
                    ORDER BY on DESC
                    LIMIT 1
                    FETCH in;
                "#,
        );

        match query.await {
            Ok(o) => match o {
                Some(p) => p,
                None => panic!("no post content found for given id {:#?}", post_id),
            },
            Err(e) => panic!("error selecting published posts: {:#?}", e),
        }
    }
}
