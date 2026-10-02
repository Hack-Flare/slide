use std::io;

use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem};

const MIGRATIONS: &str = include_str!("../../database/migrations/0001_node_identity.surql");

pub async fn open() -> io::Result<Surreal<Db>> {
    let client = Surreal::new::<Mem>(()).await.map_err(database_error)?;
    client
        .use_ns("slide")
        .use_db("slide")
        .await
        .map_err(database_error)?;
    client.query(MIGRATIONS).await.map_err(database_error)?;

    Ok(client)
}

fn database_error(error: impl std::error::Error) -> io::Error {
    io::Error::other(format!("database error: {error}"))
}
