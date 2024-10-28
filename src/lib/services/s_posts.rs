use futures::future::join_all;
use rand::seq::SliceRandom;
use tracing::{info, instrument};
// use itertools::Itertools;

use crate::db::nova_db::{get_tran_connection, NovaDB};
use crate::db::SurrealDBConnection;
use crate::models::post::{DraftPostArgs, PostHydrated, PostVersion};
use crate::{models::post::Post, repos::r_posts::PostsRepo};

#[derive(Debug, Clone)]
pub struct PostsService {
    repo: PostsRepo,
    conn: SurrealDBConnection,
}

impl PostsService {
    pub async fn new(conn: SurrealDBConnection) -> Self {
        Self {
            repo: PostsRepo::new(&conn).await,
            conn,
        }
    }

    /// Create a new post.
    #[instrument(skip(self))]
    async fn create_post(&self, created_by: String, tran_conn: Option<&NovaDB>) -> Post {
        info!("s: create post");

        if let Some(conn) = tran_conn {
            return self.repo.insert_post(created_by, conn).await;
        };

        let tran_conn = get_tran_connection(&self.conn).await;
        tran_conn.begin_tran().await;

        self.repo.insert_post(created_by, &tran_conn).await
    }

    /// Get a post.
    #[instrument(skip(self))]
    pub async fn get_post(&self, post_id: String) -> Post {
        info!("s: get post");

        self.repo.select_post(post_id).await
    }

    /// Get all posts.
    #[instrument(skip(self))]
    pub async fn get_posts(&self) -> Vec<PostHydrated> {
        info!("s: get posts");
        self.repo.select_posts().await
    }

    /// Get all drafts for the given post id.
    #[instrument(skip(self))]
    pub async fn get_post_drafts(&self, post_id: String) -> Vec<PostVersion> {
        info!("s: get post drafts");
        self.repo.select_post_drafts(post_id).await
    }

    #[instrument(skip(self))]
    pub async fn get_draft(&self, draft_id: String) -> PostVersion {
        self.repo.select_draft(&draft_id).await
    }

    /*
    TODO: i should probably pass the optional post_id as an argument
    instead of obscuring it in the DraftPostArgs object
    */
    /// Create a new draft for a post.
    ///
    /// If the id (a post id in this case) is not present create a new post
    /// and then create a new draft for that post.
    #[instrument(skip(self))]
    pub async fn create_draft(&self, draft: DraftPostArgs, author_id: String) -> PostVersion {
        // if a post id exists create a new draft for the post and return
        if let Some(post_id) = draft.id {
            info!(
                "post id exists on draft! adding draft to post: {:#?}",
                &post_id
            );
            let post = self.get_post(post_id).await;

            return self
                .repo
                .create_draft(
                    post.id as String,
                    draft.title,
                    draft.markdown,
                    author_id.clone(),
                    draft.published,
                    draft.image,
                    None,
                )
                .await;
        };

        // no post id so create a new post
        let tran_conn = get_tran_connection(&self.conn).await;
        tran_conn.begin_tran().await;

        let new_post = self.create_post(author_id.clone(), Some(&tran_conn)).await;

        let new_draft = self
            .repo
            .create_draft(
                new_post.id,
                draft.title,
                draft.markdown,
                author_id,
                draft.published,
                draft.image,
                Some(&tran_conn),
            )
            .await;

        tran_conn.commit_tran().await;

        new_draft
    }

    // TODO: rename to get_drafts
    /// Gets all current draft versions of any post that is not currently published
    #[instrument(skip(self))]
    pub async fn get_drafted_posts(&self) -> Vec<PostVersion> {
        let unpublished_post_ids = self.repo.select_unpublished_post_ids().await;

        join_all(unpublished_post_ids.into_iter().map(|p| {
            return self.get_current_draft(p);
        }))
        .await
    }

    /// Gets the most recent draft for the given post id.
    #[instrument(skip(self))]
    pub async fn get_current_draft(&self, post_id: String) -> PostVersion {
        self.repo.select_current_draft(post_id).await
    }

    /// Publish an already existing draft by passing the draft id.
    #[instrument(skip(self))]
    pub async fn publish_draft(&self, draft_id: String) -> bool {
        let post_id = self.repo.select_post_id_for_draft_id(&draft_id).await;

        let tran_conn = get_tran_connection(&self.conn).await;
        tran_conn.begin_tran().await;

        /*
        unpublishing all drafts first to ensure there are never two published drafts for a post.
        i doubt the amount of drafts per post will ever go beyond at most 50 so this should be fine.
        */
        if self
            .repo
            .unpublish_drafts_for_post_id(post_id, &tran_conn)
            .await
        {
            self.repo.publish_draft(draft_id, &tran_conn).await;
            return true;
        }

        false
    }

    // TODO: rename to get_published_drafts
    /// Gets all current published versions of any post that has a published draft.
    #[instrument(skip(self))]
    pub async fn get_published_posts(&self) -> Vec<PostVersion> {
        self.repo.select_published_posts().await
    }

    // TODO: rename to unpublish_draft
    /// Unpublish the draft of the given draft id.
    #[instrument(skip(self))]
    pub async fn unpublish_post(&self, draft_id: String) -> bool {
        info!("s: unpublish post");
        self.repo.unpublish_draft(draft_id).await;
        true
    }

    #[instrument(skip(self))]
    pub async fn get_random_post(&self) -> PostVersion {
        let published_posts = self.repo.select_published_posts().await;

        published_posts
            .choose(&mut rand::thread_rng())
            .expect("unable to choose random published post.")
            .to_owned()
    }
}
