use super::SurrealDBConnection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::usize;
use surrealdb::engine::any::{connect, Any};
use surrealdb::Error;
use surrealdb::{opt::auth::Root, Surreal};
use tracing::{info, instrument};

#[instrument(skip(conn))]
pub async fn get_tran_connection(conn: &SurrealDBConnection) -> NovaDB {
    NovaDB::new(conn).await
}

#[derive(Debug, Clone)]
pub struct NovaDB {
    pub novadb: Surreal<Any>,
}

impl NovaDB {
    #[instrument]
    pub async fn new(conn: &SurrealDBConnection) -> Self {
        let SurrealDBConnection {
            address,
            username,
            password,
            namespace,
            database,
        } = conn;

        let db = connect(address).await.unwrap();
        db.signin(Root { username, password }).await.unwrap();
        db.use_ns(namespace).use_db(database).await.unwrap();

        Self { novadb: db }
    }

    #[instrument(skip(self))]
    pub async fn begin_tran(&self) {
        let query = self.novadb.query("BEGIN TRANSACTION;");

        match query.await {
            Ok(r) => info!("Begin tran response: {:#?}", r),
            Err(e) => panic!("Error beginning tran: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn commit_tran(&self) {
        let query = self.novadb.query("COMMIT TRANSACTION;");

        match query.await {
            Ok(r) => info!("Commit tran response: {:#?}", r),
            Err(e) => panic!("Error committing tran: {:#?}", e),
        }
    }

    #[instrument(skip(self))]
    pub async fn cancel_tran(&self) {
        let query = self.novadb.query("CANCEL TRANSACTION;");

        match query.await {
            Ok(r) => info!("Cancel tran response: {:#?}", r),
            Err(e) => panic!("Error canceling tran: {:#?}", e),
        }
    }

    #[instrument(skip(self, query))]
    pub async fn query_none_with_args<A: Serialize + Debug + 'static>(
        &self,
        query: &str,
        args: A,
    ) -> Result<bool, Error> {
        let query = self.novadb.query(query).bind(args);

        match query.await {
            Ok(_r) => Ok(true),
            Err(e) => Err(e), // panic!("Query Error: {}", e),
        }
    }

    #[instrument(skip(self, query))]
    pub async fn query_single<T: DeserializeOwned>(
        &self,
        query: &str,
    ) -> Result<Option<T>, surrealdb::Error> {
        let query = self.novadb.query(query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => return Err(e), // panic!("Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }

    #[instrument(skip(self, query))]
    pub async fn query_single_with_args<T: DeserializeOwned, A: Serialize + Debug + 'static>(
        &self,
        query: &str,
        args: A,
    ) -> Result<Option<T>, Error> {
        let query = self.novadb.query(query).bind(args);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => return Err(e), // panic!("DB Query Error: {:#?}", e),
        };

        match response.take(0) {
            Ok(p) => Ok(p),
            Err(e) => Err(e), // panic!("DB Response error: {:#?}", e),
        }
    }

    #[instrument(skip(self, query))]
    pub async fn query_single_with_args_specify_result<
        T: DeserializeOwned,
        A: Serialize + Debug + 'static,
    >(
        &self,
        query: &str,
        args: A,
        result_idx: i8,
    ) -> Result<Option<T>, Error> {
        let query = self.novadb.query(query).bind(args);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => return Err(e), // panic!("Query Error: {:#?}", e),
        };

        match response.take::<Option<T>>(result_idx as usize) {
            Ok(o) => Ok(o),
            Err(e) => Err(e), // panic!("DB Response error: {:#?}", e),
        }
    }

    #[instrument(skip(self, query))]
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

    #[instrument(skip(self, query))]
    pub async fn query_many_with_args<T: DeserializeOwned, A: Serialize + Debug + 'static>(
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

    #[instrument(skip(self, query))]
    pub async fn query_many_specify_result<T: DeserializeOwned>(
        &self,
        query: &str,
        result_idx: i8,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let query = self.novadb.query(query);

        let mut response = match query.await {
            Ok(r) => r,
            Err(e) => panic!("Query Error: {:#?}", e),
        };

        match response.take::<Vec<T>>(result_idx as usize) {
            Ok(p) => Ok(p),
            Err(e) => Err(e),
        }
    }
}
