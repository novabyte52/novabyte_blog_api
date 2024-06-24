pub mod nova_db;

pub struct SurrealDBConnection<'a> {
    pub address: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub namespace: &'a str,
    pub database: &'a str,
}
