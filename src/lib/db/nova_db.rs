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

    pub async fn query_single<T: DeserializeOwned>(&self, query: &str) -> Option<T> {
        let response = &mut self
            .novadb
            .query(query)
            .await
            // TODO: error handling
            .unwrap();

        response.take(0).unwrap()
    }

    pub async fn query_single_with_args<T: DeserializeOwned, A: Serialize + Debug>(
        &self,
        query: &str,
        args: A,
    ) -> Option<T> {
        println!("args to bind {:#?}", args);
        let query = self.novadb.query(query).bind(args);

        println!("query {:#?}", query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("{}", e),
        };

        response.take(0).unwrap()
    }

    pub async fn query_many<T: DeserializeOwned, A: Serialize>(
        &self,
        query: &str,
        args: A,
    ) -> Vec<T> {
        let response = &mut self
            .novadb
            .query(query)
            .bind(args)
            .await
            // TODO: error handling
            .unwrap();

        let result: Vec<T> = response.take::<Vec<T>>(0).unwrap();

        return result;
    }
}
