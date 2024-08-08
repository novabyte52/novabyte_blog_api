use std::str::FromStr;

use surrealdb::sql::Thing;
use ulid::Ulid;

/// Creates a Thing from a borrowed String
///
/// It will panic if the thing_string does not yield exactly 2 parts
/// when split using a ':'.
///
/// It will panic if the second part of the thing_string isn't a ULID
pub fn thing_from_string(thing_string: &String) -> Thing {
    let split = thing_string.split(":");

    if split.clone().count() != 2 {
        panic!("A Thing must contain exactly 2 parts separated by a ':'");
    }

    let thing_parts: Vec<&str> = split.collect();

    // TODO: need to get a list of all table prefixes to check against

    let ulid = match Ulid::from_str(thing_parts[1]) {
        Ok(u) => u,
        Err(e) => panic!("The second part of a Thing should be a ULID: {:#?}", e),
    };

    Thing::from((String::from(thing_parts[0]), ulid.to_string()))
}
