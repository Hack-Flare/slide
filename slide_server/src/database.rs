use serde::{Deserialize, Serialize};
use std::io;
use surrealdb::Surreal;
use surrealdb::engine::local::Mem;

const MIGRATIONS: &str = include_str!("../../database/migrations/0001_node_identity.surql");

#[derive(Clone, Deserialize, Serialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub node_name: String,
}

pub async fn open(node_name: &str) -> io::Result<NodeIdentity> {
    let client = Surreal::new::<Mem>(()).await.map_err(database_error)?;
    client
        .use_ns("slide")
        .use_db("slide")
        .await
        .map_err(database_error)?;
    client.query(MIGRATIONS).await.map_err(database_error)?;

    let identity = match client
        .select::<Option<NodeIdentity>>(("node_identity", "singleton"))
        .await
        .map_err(database_error)?
    {
        Some(identity) => identity,
        None => {
            let identity = NodeIdentity {
                node_id: uuid::Uuid::new_v4().to_string(),
                node_name: node_name.to_owned(),
            };
            client
                .create::<Option<NodeIdentity>>(("node_identity", "singleton"))
                .content(identity.clone())
                .await
                .map_err(database_error)?;
            identity
        }
    };

    Ok(identity)
}

fn database_error(error: impl std::error::Error) -> io::Error {
    io::Error::other(format!("database error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn identity_is_process_local_without_persistence() {
        let first_identity = open("first-name").await.expect("database opens");

        let second_identity = open("second-name").await.expect("database reopens");

        assert_ne!(first_identity.node_id, second_identity.node_id);
        assert_eq!(first_identity.node_name, "first-name");
        assert_eq!(second_identity.node_name, "second-name");
    }
}
