pub const SYSTEM_ID: &str = "person:01J72MQD8NS5NBYVTVKWHRT18D";

/// Statement index of the `SELECT` that returns the created row in every
/// "meta preamble + record `LET`/`CREATE` + `SELECT`" insert query in this
/// codebase (see [`crate::repos::r_meta::MetaRepo::sql_create_meta`]).
/// SurrealDB v3 counts `LET` statements, so the shape is:
/// 0 `LET $meta_id`, 1 `CREATE meta`, 2 `LET $rec_id`, 3 `CREATE`, 4 `SELECT`.
///
/// Centralized here instead of redefined per service so the two places that
/// used to each hardcode `4` can't drift out of sync if that shape ever
/// changes.
pub const INSERT_SELECT_IDX: usize = 4;
