use futures::future::join_all;
use rand::seq::SliceRandom;
use tracing::{info, instrument};

use crate::db::nova_db::NovaDB;
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
        let exec = db.executor();

        let program = self.repo.program_select_post(&post_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        // single RETURN at stmt 0
        resp.take_one::<Post>(0).expect("post not found")
    }

    #[instrument(skip(self))]
    pub async fn get_posts(&self) -> Vec<PostHydrated> {
        info!("s: get posts");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_posts();
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_vec::<PostHydrated>(0)
            .expect("select posts failed")
    }

    #[instrument(skip(self))]
    pub async fn get_post_drafts(&self, post_id: String) -> Vec<PostVersion> {
        info!("s: get post drafts");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_post_drafts(&post_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_vec::<PostVersion>(0)
            .expect("select drafts failed")
    }

    #[instrument(skip(self))]
    pub async fn get_draft(&self, draft_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_draft(&draft_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_one::<PostVersion>(0).expect("draft not found")
    }

    /// Create a new draft for a post.
    ///
    /// If draft.id is None, create:
    ///   (post + meta) AND (draft) in one transaction request.
    #[instrument(skip(self))]
    pub async fn create_draft(&self, draft: DraftPostArgs, author_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        // Case A: existing post_id — just add draft (non-tx is fine)
        if let Some(post_id) = draft.id {
            info!(
                "post id exists on draft! adding draft to post: {:#?}",
                &post_id
            );

            let program = self
                .repo
                .program_create_draft()
                .bind_thing("person_id", thing_from_string(&author_id))
                .bind_thing("post_id", thing_from_string(&post_id))
                .bind("title", draft.title)
                .expect("binding title")
                .bind("markdown", draft.markdown)
                .expect("binding markdown")
                .bind("published", draft.published)
                .expect("binding published")
                .bind("image", draft.image)
                .expect("binding image");

            let mut resp = exec.run(program).await.expect("db query failed");

            return resp
                .take_one::<PostVersion>(3)
                .expect("draft create failed");
        }

        // Case B: no post id — create post and draft together, atomically.
        //
        // We intentionally do this in ONE request with BEGIN/COMMIT.
        // We also keep post_id in a Surreal variable and reuse it.
        //
        // NOTE: repo.program_insert_post() currently RETURNs a Post; that’s fine,
        // but for drafting we need the *post id* as a Thing variable.
        // So we do a tiny composed program here with an op that creates $post_id,
        // then draft uses $post_id, then we RETURN the draft.
        //
        // (This avoids "create post, then parse id in Rust, then send another query".)

        let create_post_and_draft = {
            // You can keep this inline or move it into a repo helper later.
            use crate::db::nova_db::{DbOp, DbProgram};

            let create_post_op = DbOp::new(
                "post.create_for_draft",
                r#"
                LET $post_id = post:ulid();

                // Create meta + post. We'll mirror what program_insert_post does but keep $post_id as Thing.
                LET $meta_id = meta:ulid();
                CREATE $meta_id
                SET
                    created_by = $created_by,
                    created_on = time::now(),
                    modified_by = NONE,
                    modified_on = NONE,
                    deleted_by = NONE,
                    deleted_on = NONE;

                CREATE $post_id
                SET meta = $meta_id;
                "#,
            );

            let create_draft_op = DbOp::new(
                "draft.create_for_new_post",
                format!(
                    r#"
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

                    RETURN (
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
                        FROM drafted
                        WHERE id = $drafted_id
                        LIMIT 1
                    )[0];
                    "#,
                    self.repo.meta.select_meta_string // see helper below
                ),
            );

            DbProgram::new()
                .op(create_post_op)
                .op(create_draft_op)
                .bind_thing("created_by", thing_from_string(&author_id))
                .bind_thing("person_id", thing_from_string(&author_id))
                .bind_serde(serde_json::json!({
                    "title": draft.title,
                    "markdown": draft.markdown,
                    "published": draft.published,
                    "image": draft.image
                }))
                .expect("bind")
        };

        let mut resp = exec.run_tx(create_post_and_draft).await.expect("tx failed");

        // transactional indices:
        // 0 = BEGIN, 1 = post.create_for_draft, 2 = draft.create_for_new_post, 3 = COMMIT
        resp.take_one::<PostVersion>(2)
            .expect("draft create failed")
    }

    /// Gets all current draft versions of any post that is not currently published
    #[instrument(skip(self))]
    pub async fn get_drafted_posts(&self) -> Vec<PostVersion> {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        // returns Vec<IdContainer> as last result
        let program = self.repo.program_select_unpublished_post_ids();
        let mut resp = exec.run(program).await.expect("db query failed");

        let ids: Vec<IdContainer> = resp.take_vec(2).expect("id query failed");
        let unpublished_post_ids: Vec<String> = ids.into_iter().map(|c| c.id).collect();

        join_all(
            unpublished_post_ids
                .into_iter()
                .map(|p| self.get_current_draft(p)),
        )
        .await
    }

    /// Gets the most recent draft for the given post id.
    #[instrument(skip(self))]
    pub async fn get_current_draft(&self, post_id: String) -> PostVersion {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_current_draft(&post_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_one::<PostVersion>(0)
            .expect("current draft not found")
    }

    /// Publish an already existing draft by passing the draft id.
    #[instrument(skip(self))]
    pub async fn publish_draft(&self, draft_id: String) -> bool {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        // Build one transactional program:
        // - find post_id for draft
        // - unpublish all drafts for that post
        // - publish the target draft
        use crate::db::nova_db::{DbOp, DbProgram};

        let program = DbProgram::new()
            .op(DbOp::new(
                "draft.select_post_id",
                r#"
                LET $post_id = (SELECT out FROM ONLY drafted WHERE id = $draft_id LIMIT 1).out;
                "#,
            ))
            .op(DbOp::new(
                "draft.unpublish_all",
                r#"
                UPDATE drafted SET published = false WHERE out = $post_id;
                "#,
            ))
            .op(DbOp::new(
                "draft.publish",
                r#"
                UPDATE $draft_id SET published = true;
                RETURN true;
                "#,
            ))
            .bind_thing("draft_id", thing_from_string(&draft_id));

        let mut resp = exec.run_tx(program).await.expect("tx failed");

        // tx indices: 0 BEGIN, 1 select_post_id, 2 unpublish_all, 3 publish(return true), 4 COMMIT
        resp.take_one::<bool>(3).unwrap_or(false)
    }

    /// Gets all current published versions of any post that has a published draft.
    #[instrument(skip(self))]
    pub async fn get_published_posts(&self) -> Vec<PostVersion> {
        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_select_published_posts();
        let mut resp = exec.run(program).await.expect("db query failed");

        resp.take_vec::<PostVersion>(0)
            .expect("select published failed")
    }

    /// Unpublish the draft of the given draft id.
    #[instrument(skip(self))]
    pub async fn unpublish_post(&self, draft_id: String) -> bool {
        info!("s: unpublish post");

        let db = NovaDB::new(&self.conn).await.expect("db connect failed");
        let exec = db.executor();

        let program = self.repo.program_unpublish_draft(&draft_id);
        let mut resp = exec.run(program).await.expect("db query failed");

        let _ = resp.take_one::<PostVersion>(1).expect("unpublish failed");
        true
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
