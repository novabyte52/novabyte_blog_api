use super::SurrealDBConnection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
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

    pub async fn query_single<T: DeserializeOwned>(
        &self,
        query: &str,
    ) -> Result<Option<T>, surrealdb::Error> {
        let query = self.novadb.query(query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }

    pub async fn query_single_with_args<T: DeserializeOwned, A: Serialize + Debug>(
        &self,
        query: &str,
        args: A,
    ) -> Result<Option<T>, surrealdb::Error> {
        let query = self.novadb.query(query).bind(args);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }

    pub async fn query_many<T: DeserializeOwned>(
        &self,
        query: &str,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let query = self.novadb.query(query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }

    pub async fn query_many_with_args<T: DeserializeOwned, A: Serialize + Debug>(
        &self,
        query: &str,
        args: A,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let query = self.novadb.query(query).bind(args);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }
}
