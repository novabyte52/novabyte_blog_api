use futures::future::join_all;
use rand::seq::SliceRandom;
use tracing::{info, instrument};

use crate::db::nova_db::{NovaDB, NovaQuery, NovaResponse};
use crate::db::SurrealDBConnection;
use crate::models::meta::IdContainer;
use crate::models::post::{DraftPostArgs, Post, PostHydrated, PostVersion};
use crate::repos::r_posts::PostsRepo;
use crate::utils::thing_from_string;

#[derive(Debug, Clone)]
pub struct PostsService {
    repo: PostsRepo,
    conn: SurrealDBConnection,
}

impl PostsService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: PostsRepo::new(),
            conn,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_post(&self, post_id: String) -> Post {
        info!("s: get post");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_post(&post_id))
            .await
            .expect("db query failed");

        resp.take_one::<Post>(0).expect("post not found")
    }

    #[instrument(skip(self))]
    pub async fn get_posts(&self) -> Vec<PostHydrated> {
        info!("s: get posts");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_posts())
            .await
            .expect("db query failed");

        resp.take_vec::<PostHydrated>(0).unwrap_or_default()
    }

    #[instrument(skip(self))]
    pub async fn get_post_drafts(&self, post_id: String) -> Vec<PostVersion> {
        info!("s: get post drafts");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_post_drafts(&post_id))
            .await
            .expect("db query failed");

        resp.take_vec::<PostVersion>(0)
            .expect("select drafts failed")
    }

    #[instrument(skip(self))]
    pub async fn get_draft(&self, draft_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_draft(&draft_id))
            .await
            .expect("db query failed");

        resp.take_one::<PostVersion>(0).expect("draft not found")
    }

    /// Create a new draft for a post.
    ///
    /// If `draft.id` is `Some`, adds a new draft to an existing post (no transaction needed).
    /// If `draft.id` is `None`, creates a new post + draft atomically in a transaction.
    #[instrument(skip(self))]
    pub async fn create_draft(&self, draft: DraftPostArgs, author_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        // Case A: existing post — just add a new draft version.
        if let Some(post_id) = draft.id {
            info!(
                "post id exists on draft! adding draft to post: {:#?}",
                &post_id
            );

            let q = self
                .repo
                .query_create_draft()
                .bind("person_id", thing_from_string(&author_id))
                .bind("post_id", thing_from_string(&post_id))
                .bind("title", draft.title)
                .bind("markdown", draft.markdown)
                .bind("published", draft.published)
                .bind("image", draft.image);

            let mut resp = db.exec(q).await.expect("db query failed");

            // Statement indices in query_create_draft (LET counted in SurrealDB v3):
            //   0: LET $drafted_id
            //   1: LET $meta_id
            //   2: IF post-not-found check (NONE or throws)
            //   3: RELATE (draft relation)
            //   4: SELECT drafted with meta join
            return resp
                .take_one::<PostVersion>(4)
                .expect("draft create failed");
        }

        // Case B: no post — create post + meta + draft atomically.
        let sql = format!(
            r#"
            {}
            LET $post_id = post:ulid();
            CREATE $post_id SET meta = $meta_id;

            LET $drafted_id = drafted:ulid();

            RELATE $person_id->drafted->$post_id
                SET
                    id = $drafted_id,
                    title = $title,
                    markdown = $markdown,
                    published = $published,
                    at = time::now(),
                    image = $image,
                    visits = 0,
                    meta = $meta_id;

            SELECT
                fn::string_id(id) as draft_id,
                fn::string_id(in) as author,
                fn::string_id(out) as id,
                at,
                title,
                markdown,
                published,
                image,
                visits,
                {}
            FROM ONLY drafted
            WHERE id = $drafted_id
            LIMIT 1;
            "#,
            self.repo.meta.sql_create_meta("$meta_id"),
            self.repo.meta.select_meta_string
        );

        let q = NovaQuery::new(sql)
            .bind("created_by", thing_from_string(&author_id))
            .bind("person_id", thing_from_string(&author_id))
            .bind("title", draft.title)
            .bind("markdown", draft.markdown)
            .bind("published", draft.published)
            .bind("image", draft.image);

        let tx = db.begin().await.expect("tx start failed");
        let mut resp: NovaResponse = tx
            .query(&q.sql)
            .bind(q.args)
            .await
            .expect("create post+draft failed")
            .into();
        tx.commit().await.expect("tx commit failed");

        // Statement indices (LET counted in SurrealDB v3):
        //   0: LET $meta_id
        //   1: CREATE meta
        //   2: LET $post_id
        //   3: CREATE post
        //   4: LET $drafted_id
        //   5: RELATE (draft relation)
        //   6: SELECT drafted with meta join
        resp.take_one::<PostVersion>(6)
            .expect("draft create failed")
    }

    /// Gets all current draft versions of posts that are not published.
    #[instrument(skip(self))]
    pub async fn get_drafted_posts(&self) -> Vec<PostVersion> {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_unpublished_post_ids())
            .await
            .expect("db query failed");

        // Statement indices (LET counted in SurrealDB v3):
        //   0: LET $published
        //   1: LET $unpublished
        //   2: RETURN array::distinct($unpublished)
        let ids: Vec<IdContainer> = resp.take_vec(2).unwrap_or_default();

        let unpublished_post_ids: Vec<String> = ids.into_iter().map(|c| c.id).collect();

        join_all(
            unpublished_post_ids
                .into_iter()
                .map(|p| self.get_current_draft(p)),
        )
        .await
    }

    /// Gets the most recent unpublished draft for the given post id.
    #[instrument(skip(self))]
    pub async fn get_current_draft(&self, post_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_current_draft(&post_id))
            .await
            .expect("db query failed");

        resp.take_one::<PostVersion>(0)
            .expect("current draft not found")
    }

    /// Publish a draft: unpublishes all other drafts for that post, then publishes this one.
    #[instrument(skip(self))]
    pub async fn publish_draft(&self, draft_id: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        // Single transactional query:
        // 1. Find the post for this draft
        // 2. Unpublish all drafts for that post
        // 3. Publish the target draft
        let q = NovaQuery::new(
            r#"
            LET $post_id = (SELECT out FROM ONLY drafted WHERE id = $draft_id LIMIT 1).out;
            UPDATE drafted SET published = false WHERE out = $post_id;
            UPDATE $draft_id SET published = true;
            RETURN true;
            "#,
        )
        .bind("draft_id", thing_from_string(&draft_id));

        let tx = db.begin().await.expect("tx start failed");
        let mut resp: NovaResponse = tx
            .query(&q.sql)
            .bind(q.args)
            .await
            .expect("publish draft failed")
            .into();
        tx.commit().await.expect("tx commit failed");

        // Statement indices (LET counted in SurrealDB v3):
        //   0: LET $post_id
        //   1: UPDATE drafted (unpublish all)
        //   2: UPDATE $draft_id (publish)
        //   3: RETURN true
        resp.take_one::<bool>(3).unwrap_or(false)
    }

    /// Gets all published post versions.
    #[instrument(skip(self))]
    pub async fn get_published_posts(&self) -> Vec<PostVersion> {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_select_published_posts())
            .await
            .expect("db query failed");

        resp.take_vec::<PostVersion>(0).unwrap_or_default()
    }

    /// Unpublish the draft with the given draft id.
    #[instrument(skip(self))]
    pub async fn unpublish_post(&self, draft_id: String) -> bool {
        info!("s: unpublish post");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");

        let mut resp = db
            .exec(self.repo.query_unpublish_draft(&draft_id))
            .await
            .expect("db query failed");

        // Statement indices: 0=UPDATE, 1=SELECT draft with meta join
        resp.take_opt::<PostVersion>(1)
            .map(|o| o.is_some())
            .unwrap_or(false)
    }

    #[instrument(skip(self))]
    pub async fn get_random_post(&self) -> PostVersion {
        let published_posts = self.get_published_posts().await;

        published_posts
            .choose(&mut rand::thread_rng())
            .expect("unable to choose random published post.")
            .to_owned()
    }
}
