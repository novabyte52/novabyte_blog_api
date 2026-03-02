use serde::Serialize;

use super::r_meta::MetaRepo;
use crate::db::nova_db::{DbOp, DbProgram};
use crate::utils::thing_from_string;

#[derive(Debug, Clone)]
pub struct PostsRepo {
    pub meta: MetaRepo,
}

#[derive(Debug, Serialize)]
pub struct DraftedArgs {
    // pub person_id: Thing,
    // pub post_id: Thing,
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

    /// Program: create a post + meta and RETURN Post (recommended to run in a transaction).
    pub fn program_insert_post(&self, created_by: String) -> DbProgram {
        let create_post_query = format!(
            r#"
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
            self.meta.select_meta_string
        );

        DbProgram::new()
            .op(self.meta.op_create_meta("$meta_id"))
            .op(DbOp::new("post.create", create_post_query))
            .bind_thing("created_by", thing_from_string(&created_by))
    }

    /// Program: select a post (returns Post).
    pub fn program_select_post(&self, post_id: &String) -> DbProgram {
        let select_post_query = format!(
            "SELECT fn::string_id(id) as id, {} FROM ONLY $post_id LIMIT 1;",
            self.meta.select_meta_string
        );

        DbProgram::new()
            .op(DbOp::new("post.select", select_post_query))
            .bind_thing("post_id", thing_from_string(post_id))
    }

    /// Program: select posts (returns Vec<PostHydrated>).
    pub fn program_select_posts(&self) -> DbProgram {
        let select_posts = format!(
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

        DbProgram::new().op(DbOp::new("post.select_all", select_posts))
    }

    /// Program: select draft by draft_id (returns PostVersion).
    pub fn program_select_draft(&self, draft_id: &String) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("draft.select", q))
            .bind_thing("draft_id", thing_from_string(draft_id))
    }

    /// Program: select drafts for a post (returns Vec<PostVersion>).
    pub fn program_select_post_drafts(&self, post_id: &String) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("draft.select_for_post", q))
            .bind_thing("post_id", thing_from_string(post_id))
    }

    /// Program: create a draft version for an existing post and RETURN PostVersion.
    ///
    /// IMPORTANT: this uses the post's meta id so draft.meta matches post.meta.
    pub fn program_create_draft(&self) -> DbProgram {
        let q = format!(
            r#"
            LET $drafted_id = drafted:ulid();
            LET $meta_id = (SELECT meta FROM ONLY post WHERE id = $post_id LIMIT 1).meta;

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

        DbProgram::new().op(DbOp::new("draft.create", q))
    }

    /// Program: publish a draft (returns PostVersion).
    pub fn program_publish_draft(&self, draft_id: String) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("draft.publish", q))
            .bind_thing("draft_id", thing_from_string(&draft_id))
    }

    /// Program: unpublish a draft (returns PostVersion).
    pub fn program_unpublish_draft(&self, draft_id: &String) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("draft.unpublish", q))
            .bind_thing("draft_id", thing_from_string(draft_id))
    }

    /// Program: select drafted posts (published=false) returns Vec<PostVersion>.
    pub fn program_select_drafted_posts(&self) -> DbProgram {
        let q = format!(
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
        DbProgram::new().op(DbOp::new("draft.select_unpublished", q))
    }

    /// Program: select current draft (published=false) for post returns PostVersion.
    pub fn program_select_current_draft(&self, post_id: &String) -> DbProgram {
        let q = format!(
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

        DbProgram::new()
            .op(DbOp::new("draft.select_current", q))
            .bind_thing("post_id", thing_from_string(post_id))
    }

    /// Program: select published posts returns Vec<PostVersion>.
    pub fn program_select_published_posts(&self) -> DbProgram {
        let q = format!(
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
        DbProgram::new().op(DbOp::new("draft.select_published", q))
    }

    /// Program: unpublish all drafts for post returns true.
    pub fn program_unpublish_drafts_for_post_id(&self, post_id: String) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "draft.unpublish_all_for_post",
                r#"
                UPDATE drafted SET published = false WHERE out = $post_id
                RETURN true;
                "#,
            ))
            .bind_thing("post_id", thing_from_string(&post_id))
    }

    /// Program: select post id for draft id returns String id.
    pub fn program_select_post_id_for_draft_id(&self, draft_id: String) -> DbProgram {
        DbProgram::new()
            .op(DbOp::new(
                "draft.select_post_id",
                r#"
                SELECT fn::string_id(out) as id FROM ONLY drafted WHERE id = $draft_id LIMIT 1;
                "#,
            ))
            .bind_thing("draft_id", thing_from_string(&draft_id))
    }

    /// Program: select unpublished post ids returns Vec<IdContainer> (service can map to Vec<String>).
    pub fn program_select_unpublished_post_ids(&self) -> DbProgram {
        DbProgram::new().op(DbOp::new(
            "post.select_unpublished_ids",
            r#"
            LET $published = SELECT out FROM drafted WHERE published = true;

            LET $unpublished = SELECT fn::string_id(out) as id FROM drafted WHERE out NOT IN $published.out;

            RETURN array::distinct($unpublished);
            "#,
        ))
    }
}
