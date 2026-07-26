# SurrealDB Rust SDK Fetch Best Practices

## Overview
This guide covers best practices and patterns for fetching records using the SurrealDB Rust SDK (3.x), based on official documentation.

## Key Concepts

### FETCH vs SELECT with .* operator
- `FETCH` provides cleaner syntax for joining related records compared to `.*` operator
- For arrays, `.*.*` is needed with the dot operator, while `FETCH` handles both single records and arrays uniformly
- Use `FETCH` when you need full record details instead of just record IDs

## Best Practices

### 1. Use FETCH for Related Records
```surrealql
-- Instead of:
SELECT *, teacher.*, students.*.* FROM classroom;

-- Use:
SELECT * FROM classroom FETCH teacher, students;
```

### 2. Flexible Typing with Resource
When you don't need to deserialize immediately, use `Resource` to return a `Value`:
```rust
db.create(Resource::from("student"))
    .content(student_data)
    .await?;
```

### 3. Query Pattern for Fetching with Related Data
```rust
// Use query() with FETCH clause
let mut results = db.query(format!("SELECT * FROM student FETCH classes")).await?;

// Extract results with proper deserialization type
let students: Vec<StudentWithClasses> = results.take(0)?;
```

### 4. Deserialization Types
- For fetching data: derive `Deserialize` (or `SurrealValue`)
- For inserting data: derive `Serialize`
- Use separate structs for input vs output when needed

```rust
#[derive(Debug, Serialize)]
struct StudentInput { /* fields for creation */ }

#[derive(Debug, Deserialize)]
struct StudentOutput { /* fields including related records */ }
```

### 5. Record ID Handling
- Use `RecordId::from((table, key))` for creating record IDs in 3.x
- The `RecordId::new()` constructor may have different signatures between versions

## Gotchas

1. **Version Differences**: SurrealDB 3.x has API changes from 2.x
   - Check `Resource` usage and `RecordId` construction
   - Verify query result extraction methods

2. **Type Safety**: When using `Value` type, you'll need manual deserialization later

3. **Query Results**: Always check the correct index when extracting results from multi-statement queries

4. **Nullable Fields**: Use `Option<T>` for fields that might be null or absent

## Example Pattern

```rust
use surrealdb::{
    engine::remote::ws::Ws,
    opt::{auth::Root, Resource},
    sql::Datetime,
    RecordId, Surreal,
};

#[derive(Debug, Deserialize)]
struct RelatedRecord {
    id: RecordId,
    name: String,
}

#[derive(Debug, Deserialize)]
struct MainRecord {
    id: RecordId,
    name: String,
    related: Vec<RelatedRecord>,
}

async fn fetch_with_related(db: &Surreal<Ws>) -> surrealdb::Result<Vec<MainRecord>> {
    let mut results = db.query("SELECT * FROM main FETCH related").await?;
    results.take(0)
}
```
