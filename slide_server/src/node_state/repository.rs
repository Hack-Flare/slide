use std::io;

use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::database;

use super::NodeIdentity;

pub struct NodeStateRepository {
    client: Surreal<Db>,
}

impl NodeStateRepository {
    pub async fn open(node_name: &str) -> io::Result<Self> {
        let repository = Self {
            client: database::open().await?,
        };
        repository.ensure_identity(node_name).await?;

        Ok(repository)
    }

    pub async fn node_identity(&self) -> io::Result<NodeIdentity> {
        self.client
            .select::<Option<NodeIdentity>>(("node_identity", "singleton"))
            .await
            .map_err(database_error)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "node identity is missing"))
    }

    async fn ensure_identity(&self, node_name: &str) -> io::Result<()> {
        if self.node_identity().await.is_ok() {
            return Ok(());
        }

        let identity = NodeIdentity {
            node_id: uuid::Uuid::new_v4().to_string(),
            node_name: node_name.to_owned(),
        };
        self.client
            .create::<Option<NodeIdentity>>(("node_identity", "singleton"))
            .content(identity)
            .await
            .map_err(database_error)?;

        Ok(())
    }
}

fn database_error(error: impl std::error::Error) -> io::Error {
    io::Error::other(format!("database error: {error}"))
}
