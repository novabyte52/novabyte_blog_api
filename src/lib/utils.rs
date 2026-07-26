use std::str::FromStr;

use surrealdb::types::{RecordId, RecordIdKey};
use tracing::{debug, instrument};
use ulid::Ulid;

/// Creates a [`RecordId`] from a `"table:ulid"` string.
///
/// Panics if the string does not contain exactly one `':'`, or if the part
/// after the colon is not a valid ULID.
#[instrument]
pub fn thing_from_string(thing_string: &str) -> RecordId {
    debug!("converting {} into RecordId", thing_string);
    let split = thing_string.split(":");

    if split.clone().count() != 2 {
        panic!("A RecordId must contain exactly 2 parts separated by a ':'");
    }

    let thing_parts: Vec<&str> = split.collect();

    let ulid = match Ulid::from_str(thing_parts[1]) {
        Ok(u) => u,
        Err(e) => panic!("The second part of a RecordId should be a ULID: {:#?}", e),
    };

    RecordId::new(thing_parts[0], ulid.to_string())
}

/// Converts a [`RecordId`] back to a `"table:key"` string for use with [`thing_from_string`].
///
/// Panics if the key is not a String variant (all ids in this project use ULID string keys).
pub fn record_id_to_string(id: RecordId) -> String {
    match id.key {
        RecordIdKey::String(s) => format!("{}:{}", id.table, s),
        RecordIdKey::Number(n) => format!("{}:{}", id.table, n),
        other => panic!("unexpected RecordIdKey variant: {:?}", other),
    }
}
