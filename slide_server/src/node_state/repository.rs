use std::io;

use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::database;

use super::NodeIdentity;
use super::membership::{ClusterMember, NodeRole};

pub struct NodeStateRepository {
    client: Surreal<Db>,
}

impl NodeStateRepository {
    pub async fn open(node_name: &str) -> io::Result<Self> {
        let repository = Self {
            client: database::open().await?,
        };
        repository.ensure_identity(node_name).await?;
        let identity = repository.node_identity().await?;
        let role = if repository.members().await?.is_empty() {
            NodeRole::Primary
        } else {
            NodeRole::Follower
        };
        repository
            .register(&identity.node_id, &identity.node_name, role)
            .await?;

        Ok(repository)
    }

    pub async fn node_identity(&self) -> io::Result<NodeIdentity> {
        self.client
            .select::<Option<NodeIdentity>>(("node_identity", "singleton"))
            .await
            .map_err(database_error)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "node identity is missing"))
    }

    pub async fn join(&self, node_id: &str, node_name: &str) -> io::Result<ClusterMember> {
        self.register(node_id, node_name, NodeRole::Follower).await
    }

    async fn register(
        &self,
        node_id: &str,
        node_name: &str,
        role: NodeRole,
    ) -> io::Result<ClusterMember> {
        if let Some(member) = self
            .client
            .select::<Option<ClusterMember>>(("cluster_member", node_id))
            .await
            .map_err(database_error)?
        {
            return Ok(member);
        }

        let member = ClusterMember {
            node_id: node_id.to_owned(),
            node_name: node_name.to_owned(),
            role,
            voting: true,
        };
        self.client
            .create::<Option<ClusterMember>>(("cluster_member", node_id))
            .content(member.clone())
            .await
            .map_err(database_error)?;

        Ok(member)
    }

    pub async fn members(&self) -> io::Result<Vec<ClusterMember>> {
        self.client
            .select::<Vec<ClusterMember>>("cluster_member")
            .await
            .map_err(database_error)
    }

    pub async fn member(&self, node_id: &str) -> io::Result<ClusterMember> {
        self.client
            .select::<Option<ClusterMember>>(("cluster_member", node_id))
            .await
            .map_err(database_error)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cluster member is missing"))
    }

    pub async fn leave(&self, _node_id: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "leaving a fleet is disabled",
        ))
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
