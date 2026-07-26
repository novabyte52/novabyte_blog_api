• Prefer Resource + Value over raw structs when schema is unstable or dynamic (e.g., during early development).  
 Use db.select(Resource::from("person")).await? → returns Value, which supports .to_sql()/.to_sql_pretty().  
 Use db.select(Resource::from("person")).await? → returns Value, which supports .to_sql()/.to_sql_pretty().

db.create(Resource::from("post")).content(json!({ "title": "Hi" })).await?;

• For strict typing, derive Serialize + Deserialize (not SurrealValue) and use direct methods like db.select::<\_,
Vec<Post>>("post").await?.  
 • Avoid .query() for structured results—it returns a verbose Response wrapper. Prefer typed methods or  
 Resource::from(...).  
 • When deserializing to structs, extra fields are silently dropped (not an error). Use Value if you need all  
 data.  
 • Use Value::to_sql() for debugging—it matches Surrealist/CLI output and handles nested types cleanly.  
 • For bindings in custom SQL, prefer DbProgram.bind_serde(json!({ ... })) or .bind(...) with serializable values
(your current pattern is solid).  
 • SurrealDB 3.x removed SurrealValue derive—use standard Serialize/Deserialize. Your code using  
 serde_json::to_value, json_to_sql_value, etc., aligns well.  
 • When working with IDs, use Thing (e.g., person:ulid()) and convert via your thing_from_string helper. SurrealDB
3.x still uses RecordId internally, but Thing remains compatible.  
 • For optional/nullable fields, prefer Option<T> in structs or handle Value::None explicitly when using Value.  
 • Transactions: Use DbExecutor::run_tx(...) as you already do—no changes needed for 3.x.
