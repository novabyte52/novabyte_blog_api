use serde::Serialize;
use surrealdb::sql::{Id, Thing};
use ulid::Ulid;

use crate::db::nova_db::NovaDB;
use crate::db::SurrealDBConnection;
use crate::models::person::Person;

// TODO: should maybe have a reader and writer per repo
// for now, this will just do both
async fn get_db_client() -> NovaDB {
    NovaDB::new(SurrealDBConnection {
        address: "127.0.0.1:52000",
        username: "root",
        password: "root",
        namespace: "test",
        database: "novabyte.blog",
    })
    .await
}

pub async fn insert_post() -> Person {
    println!("insert post");
    // Perform a custom advanced query
    let user = get_db_client()
        .await
        .query_single::<Person>(
            r#"
                CREATE 
                    person
                SET 
                name = 'Generated 01'
            "#,
        )
        .await;

    match user {
        Some(p) => {
            println!("{:#?}", p);
            p
        }
        _ => {
            panic!("nothing found!");
        }
    }
}

#[derive(Debug, Serialize)]
struct SelectPostArgs {
    id: Thing,
}

pub async fn select_post(post_id: Ulid) -> Person {
    println!("r: select post: {}", post_id);

    let db_client = get_db_client().await;
    let query = db_client
        .novadb
        .query("SELECT * FROM person WHERE id = $id")
        .bind(SelectPostArgs {
            id: Thing {
                tb: String::from("person"),
                id: Id::String(String::from("01HJ4T9031ZWV6N8XM17Z9XV9C")),
            },
        });

    let mut response = match query.await {
        Ok(r) => r,
        Err(e) => panic!("{:#?}", e),
    };

    let user = response.take(0).unwrap();

    match user {
        Some(p) => {
            println!("{:#?}", p);
            p
        }
        _ => {
            panic!("nothing found!");
        }
    }
}

pub async fn select_posts() -> Vec<Person> {
    println!("r: select posts");
    let cl = get_db_client().await;
    let mut user = cl.novadb.query("SELECT * FROM person").await.unwrap();

    println!("{:#?}", user);

    let foo: Vec<Person> = match user.take(0) {
        Ok(u) => {
            println!("r: posts Ok - {:#?}", u);
            u
        }
        Err(e) => panic!("{:#?}", e),
    };

    println!("r: returning posts - {:#?}", foo);

    foo
}
