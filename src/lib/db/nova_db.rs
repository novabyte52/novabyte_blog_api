use super::{Person, SurrealDBConnection};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::{opt::auth::Root, Surreal};

pub struct NovaDB {
    pub novadb: Surreal<Client>,
}

impl NovaDB {
    pub async fn new(conn: SurrealDBConnection<'_>) -> Self {
        let SurrealDBConnection {
            address,
            username,
            password,
            namespace,
            database,
        } = conn;
        let db = Surreal::new::<Ws>(address).await.unwrap();

        db.signin(Root { username, password }).await.unwrap();

        db.use_ns(namespace).use_db(database).await.unwrap();

        Self { novadb: db }
    }

    pub async fn query_single<T: DeserializeOwned>(&self, query: &str) -> Option<T> {
        let response = &mut self
            .novadb
            .query(query)
            .bind(("table", "person"))
            .await
            // TODO: error handling
            .unwrap();

        // sometimes rust makes me deeply unhappy :) - old evan
        // right until you learn it was your fault... it's understandable, but still annoying
        let bar: Option<T> = response.take(0).unwrap();

        return bar;
    }

    pub async fn query_many<T: DeserializeOwned>(&self, query: &str) -> Vec<T> {
        let response = &mut self
            .novadb
            .query(query)
            .bind(("table", "person"))
            .await
            // TODO: error handling
            .unwrap();

        let foo: Vec<T> = response.take::<Vec<T>>(0).unwrap();

        return foo;
    }

    pub async fn docsEx01(&self) {
        let mut response = self
            .novadb
            // Get `john`'s details
            .query("SELECT * FROM user:john")
            // List all users whose first name is John
            .query("SELECT * FROM user WHERE name.first = 'John'")
            // Get John's address
            .query("SELECT address FROM user:john")
            // Get all users' addresses
            .query("SELECT address FROM user")
            .await
            .unwrap();

        // Get the first (and only) user from the first query
        let user: Option<User> = response.take(0).unwrap();

        // Get all users from the second query
        let users: Vec<User> = response.take(1).unwrap();

        // Retrieve John's address without making a special struct for it
        let address: Option<String> = response.take((2, "address")).unwrap();

        // Get all users' addresses
        let addresses: Vec<String> = response.take((3, "address")).unwrap();

        // You can continue taking more fields on the same response
        // object when extracting individual fields
        let mut response = self.novadb.query("SELECT * FROM user").await;

        println!("{:#?}", user.unwrap());
        println!("{:#?}", users);
        println!("{:#?}", address.unwrap());
        println!("{:#?}", addresses);
        // Since the query we want to access is at index 0, we can use
        // a shortcut instead of `response.take((0, "field"))`
        // let ids: Vec<String> = response.take("id")?;
        // let names: Vec<String> = response.take("name")?;
        // let addresses: Vec<String> = response.take("address")?;
    }
}

#[derive(Debug, Deserialize)]
struct User {
    id: String,
    balance: String,
}
