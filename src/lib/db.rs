pub mod nova_db;

#[derive(Debug, Clone)]
pub struct SurrealDBConnection {
    pub address: String,
    pub username: String,
    pub password: String,
    pub namespace: String,
    pub database: String,
}
