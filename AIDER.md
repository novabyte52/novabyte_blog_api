# novabyte.blog api

this is the api for novabyte.blog, an open source blog platform powered on the backend by rust and surrealdb.

## major dependencies

- _axum_
  - crate used for routing and general api functionality
- _surrealdb_
  - crate supplied by surrealdb to work with it programatically
- _surrealkit_
  - crate used for migrating surrealdb data

## conventions

- write idomatic rust
- write semantic code
- architect for code reuse
- use the _tracing_ crate macros for logging, never add print macros
  - use appropriate log levels when logging

## notes for ai assistants

- ask before attempting to commit any code
- ask before adding dependencies to the project
- we do not have any code testing at the moment
