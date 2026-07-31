# SurrealDB 3.x Transactions - Best Practices & Patterns

## Overview

SurrealDB 3.x introduces a native Rust `Transaction` type for client-side transaction management. This is different from SurrealDB 2.x where transactions were managed via SQL `BEGIN`/`COMMIT` statements.

## Key Changes in 3.x

1. **Client-Side Transactions**: Use `db.begin().await?` to get a `Transaction` handle
2. **Explicit Commit/Cancel**: Must explicitly call `.commit()` or `.cancel()`
3. **Per-Statement Error Handling**: Check for per-statement errors with `.take_errors()` or use `.check()`

## Best Practices

### 1. Use Client-Side Transactions When Possible

Prefer the `Transaction` API over manual SurrealQL transactions:

```rust
// ✅ Preferred in 3.x
let tx = db.begin().await?;
tx.query("CREATE person SET name = 'John'").await?;
tx.commit().await?;

// ❌ Legacy approach (still works but less idiomatic)
db.query("BEGIN; CREATE person SET name = 'John'; COMMIT;").await?;
```

### 2. Always Handle Per-Statement Errors

In 3.x, the outer `Result` only indicates if the request succeeded, not if all statements executed successfully:

```rust
let tx = db.begin().await?;

match tx.query(sql).await {
    Ok(mut response) => {
        let errors = response.take_errors();
        if !errors.is_empty() {
            // Handle per-statement errors
            return tx.cancel().await;
        }
        tx.commit().await?;
    }
    Err(e) => {
        // Request-level error - transaction is unusable
        return tx.cancel().await;
    }
}
```

### 3. Use `check()` for Strict Error Handling

If you want automatic failure on any error:

```rust
let tx = db.begin().await?;
tx.query(sql).await?.check()?; // Fails on first per-statement error
tx.commit().await?;
```

### 4. Transaction Scope Management

The `Transaction` type implements `Deref<Target = Surreal>` so you can use all the same methods:

```rust
let tx = db.begin().await?;

// All these work on Transaction:
tx.set("var", value).await?;
tx.create("person").content(json!({"name": "John"})).await?;
tx.query("RETURN 1").await?;
```

### 5. Cleanup Pattern

Always ensure transactions are either committed or cancelled:

```rust
fn run_in_transaction<F>(db: Surreal<Any>, f: F) -> impl Future<Output = Result<Surreal<Any>, surrealdb::Error>>
where
    F: FnOnce(Transaction) -> impl Future<Output = Result<(), surrealdb::Error>> + Send,
{
    async move {
        let tx = db.begin().await?;
        
        match f(tx).await {
            Ok(_) => tx.commit().await,
            Err(_) => tx.cancel().await,
        }
    }
}
```

## Migration from 2.x

### Before (2.x):
```rust
db.query("
    BEGIN;
    CREATE person SET name = 'John';
    COMMIT;
").await?;
```

### After (3.x):
```rust
let tx = db.begin().await?;
tx.create("person").content(json!({"name": "John"})).await?;
tx.commit().await?;
```

## Gotchas

1. **Transaction is not `Surreal`**: It's a separate type, though it derefs to `Surreal`
2. **Auto-commit on drop**: Transactions do NOT auto-commit on drop - you must explicitly call `commit()` or `cancel()`
3. **Error propagation**: A failed statement doesn't automatically cancel the transaction; you must handle errors and call `cancel()`

## Recommended Patterns

### Pattern 1: Transaction with Error Reporting
```rust
pub async fn run_with_transaction<F, T>(
    db: &Surreal<Any>,
    operation: F,
) -> Result<T, surrealdb::Error>
where
    F: FnOnce(Transaction) -> impl Future<Output = Result<(), surrealdb::Error>> + Send,
{
    let tx = db.begin().await?;
    let result = operation(tx.clone()).await?;
    
    // Check for any per-statement errors before committing
    if !tx.take_errors().is_empty() {
        tx.cancel().await?;
        return Err(surrealdb::Error::Api("Per-statement errors occurred".into()));
    }
    
    tx.commit().await?;
    Ok(result)
}
```

### Pattern 2: Reusable Transaction Helper
```rust
pub async fn execute_tx<F, T>(db: Surreal<Any>, sql: &str, f: F) -> Result<Surreal<Any>, surrealdb::Error>
where
    F: FnOnce(Transaction) -> impl Future<Output = Result<(), surrealdb::Error>> + Send,
{
    let tx = db.begin().await?;
    
    match f(tx.clone()).await {
        Ok(_) => {
            if !tx.take_errors().is_empty() {
                tx.cancel().await
            } else {
                tx.commit().await
            }
        }
        Err(_) => tx.cancel().await,
    }
}
```

## References

- [SurrealDB 3.x Rust Transactions Docs](https://surrealdb.com/docs/reference/rust/concepts/transaction)
- `Transaction` type: https://docs.rs/surrealdb/3.0.5/surrealdb/method/struct.Transaction.html
- `commit()`: https://docs.rs/surrealdb/3.0.5/surrealdb/method/struct.Transaction.html#method.commit
- `cancel()`: https://docs.rs/surrealdb/3.0.5/surrealdb/method/struct.Transaction.html#method.cancel
- `check()`: https://docs.rs/surrealdb/3.0.5/surrealdb/struct.IndexedResults.html#method.check
- `take_errors()`: https://docs.rs/surrealdb/3.0.5/surrealdb/struct.IndexedResults.html#method.take_errors
