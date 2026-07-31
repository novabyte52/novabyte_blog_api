use serde::Serialize;

use crate::db::nova_db::NovaQuery;
use crate::utils::thing_from_string;

use super::r_meta::MetaRepo;

#[derive(Debug, Clone)]
pub struct PostsRepo {
    pub meta: MetaRepo,
}

#[derive(Debug, Serialize)]
pub struct DraftedArgs {
    pub title: String,
    pub markdown: String,
    pub published: bool,
    pub image: String,
}

impl PostsRepo {
    pub fn new() -> Self {
        Self {
            meta: MetaRepo::new(),
        }
    }

    /// Query: create a post + meta (run in a transaction).
    /// Multi-statement: creates meta, creates post, returns Post.
    pub fn query_insert_post(&self, created_by: &str) -> NovaQuery {
        let sql = format!(
            r#"
            {}
            LET $post_id = post:ulid();

            CREATE $post_id
            SET meta = $meta_id;

            SELECT
                fn::string_id(id) as id,
                {}
            FROM ONLY post
            WHERE id = $post_id
            LIMIT 1;
            "#,
            self.meta.sql_create_meta("$meta_id"),
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("created_by", thing_from_string(created_by))
    }

    /// Query: select a post by id (returns Post).
    pub fn query_select_post(&self, post_id: &str) -> NovaQuery {
        let sql = format!(
            "SELECT fn::string_id(id) as id, {} FROM ONLY $post_id LIMIT 1;",
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("post_id", thing_from_string(post_id))
    }

    /// Query: select all posts (returns Vec<PostHydrated>).
    pub fn query_select_posts(&self) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(id) as id,
                array::first(
                    (
                        SELECT at, title
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
            self.meta.select_meta_string
        );
        NovaQuery::new(sql)
    }

    /// Query: select draft by draft_id (returns PostVersion).
    pub fn query_select_draft(&self, draft_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published,
                image,
                visits,
                {}
            FROM drafted
            WHERE id = $draft_id
            ORDER BY at DESC
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("draft_id", thing_from_string(draft_id))
    }

    /// Query: select drafts for a post (returns Vec<PostVersion>).
    pub fn query_select_post_drafts(&self, post_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published,
                image,
                visits,
                {}
            FROM drafted
            WHERE out = $post_id
            ORDER BY at DESC;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("post_id", thing_from_string(post_id))
    }

    /// Query: create a draft version for an existing post (returns PostVersion).
    ///
    /// Uses the post's existing meta id so draft.meta matches post.meta.
    pub fn query_create_draft(&self) -> NovaQuery {
        let sql = format!(
            r#"
            LET $drafted_id = drafted:ulid();
            LET $meta_id = (SELECT meta FROM ONLY post WHERE id = $post_id LIMIT 1).meta;
            IF $meta_id IS NONE {{ THROW "Post not found: " + fn::string_id($post_id) }};

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
            self.meta.select_meta_string
        );
        NovaQuery::new(sql)
    }

    /// Query: publish a draft (returns PostVersion).
    pub fn query_publish_draft(&self, draft_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            UPDATE $draft_id SET published = true;

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
            WHERE id = $draft_id
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("draft_id", thing_from_string(draft_id))
    }

    /// Query: unpublish a draft (returns PostVersion).
    pub fn query_unpublish_draft(&self, draft_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            UPDATE $draft_id SET published = false;

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
            WHERE id = $draft_id
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("draft_id", thing_from_string(draft_id))
    }

    /// Query: select drafted posts (published=false) returns Vec<PostVersion>.
    pub fn query_select_drafted_posts(&self) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published,
                image,
                visits,
                {}
            FROM drafted
            WHERE published = false
            ORDER BY at DESC;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql)
    }

    /// Query: select current draft (published=false) for post (returns PostVersion).
    pub fn query_select_current_draft(&self, post_id: &str) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published,
                image,
                visits,
                {}
            FROM drafted
            WHERE out = $post_id
                AND published = false
            ORDER BY at DESC
            LIMIT 1;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql).bind("post_id", thing_from_string(post_id))
    }

    /// Query: select published posts (returns Vec<PostVersion>).
    pub fn query_select_published_posts(&self) -> NovaQuery {
        let sql = format!(
            r#"
            SELECT
                fn::string_id(out) as id,
                fn::string_id(id) as draft_id,
                title,
                markdown,
                at,
                fn::string_id(in) as author,
                published,
                image,
                visits,
                {}
            FROM drafted
            WHERE published = true
            ORDER BY at DESC;
            "#,
            self.meta.select_meta_string
        );
        NovaQuery::new(sql)
    }

    /// Query: unpublish all drafts for a post (returns true).
    pub fn query_unpublish_drafts_for_post_id(&self, post_id: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            UPDATE drafted SET published = false WHERE out = $post_id
            RETURN true;
            "#,
        )
        .bind("post_id", thing_from_string(post_id))
    }

    /// Query: select post id for a draft id (returns row with id field).
    pub fn query_select_post_id_for_draft_id(&self, draft_id: &str) -> NovaQuery {
        NovaQuery::new(
            r#"
            SELECT fn::string_id(out) as id FROM ONLY drafted WHERE id = $draft_id LIMIT 1;
            "#,
        )
        .bind("draft_id", thing_from_string(draft_id))
    }

    /// Query: select unpublished post ids (returns Vec<IdContainer>).
    pub fn query_select_unpublished_post_ids(&self) -> NovaQuery {
        NovaQuery::new(
            r#"
            LET $published = SELECT out FROM drafted WHERE published = true;

            LET $unpublished = SELECT fn::string_id(out) as id FROM drafted WHERE out NOT IN $published.out;

            RETURN array::distinct($unpublished);
            "#,
        )
    }
}
