pub mod nova_db;

#[derive(Debug)]
pub struct SurrealDBConnection<'a> {
    pub address: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub namespace: &'a str,
    pub database: &'a str,
}
