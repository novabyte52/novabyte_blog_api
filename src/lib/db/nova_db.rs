use super::SurrealDBConnection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::usize;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::{opt::auth::Root, Surreal};
use tracing::{event, instrument, Level};

pub async fn get_tran_connection() -> NovaDB {
    NovaDB::new(SurrealDBConnection {
        address: "127.0.0.1:52000",
        username: "root",
        password: "root",
        namespace: "test",
        database: "novabyte.blog",
    })
    .await
}

#[derive(Debug)]
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

    pub async fn begin_tran(&self) {
        let query = self.novadb.query("BEGIN TRANSACTION;");

        match query.await {
            Ok(r) => println!("Begin tran response: {:#?}", r),
            Err(e) => panic!("Error beginning tran: {:#?}", e),
        }
    }

    pub async fn commit_tran(&self) {
        let query = self.novadb.query("COMMIT TRANSACTION;");

        match query.await {
            Ok(r) => println!("Commit tran response: {:#?}", r),
            Err(e) => panic!("Error committing tran: {:#?}", e),
        }
    }

    pub async fn cancel_tran(&self) {
        let query = self.novadb.query("CANCEL TRANSACTION;");

        match query.await {
            Ok(r) => println!("Cancel tran response: {:#?}", r),
            Err(e) => panic!("Error canceling tran: {:#?}", e),
        }
    }

    pub async fn query_none_with_args<A: Serialize + Debug>(&self, query: &str, args: A) {
        let query = self.novadb.query(query).bind(args);

        match query.await {
            Ok(r) => println!("Query response: {:#?}", r),
            Err(e) => panic!("Query Error: {}", e),
        }
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

    #[instrument]
    pub async fn query_single_with_args<T: DeserializeOwned, A: Serialize + Debug>(
        &self,
        query: &str,
        args: A,
    ) -> Option<T> {
        let query = self.novadb.query(query).bind(args);

        event!(Level::INFO, "built query: {query:#?}", query = &query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("DB Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => p,
            Err(e) => panic!("DB Response error: {:#?}", e),
        }
    }

    #[instrument]
    pub async fn query_single_with_args_specify_result<
        T: DeserializeOwned,
        A: Serialize + Debug,
    >(
        &self,
        query: &str,
        args: A,
        result_idx: i8,
    ) -> Option<T> {
        let query = self.novadb.query(query).bind(args);

        event!(Level::INFO, "built query: {query:#?}", query = &query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        println!("Response: {:#?}", &response);

        match response.take::<Option<T>>(result_idx as usize) {
            Ok(o) => o,
            Err(e) => panic!("DB Response error: {:#?}", e),
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
