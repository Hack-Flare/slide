use std::io;

use surrealdb::Surreal;
use surrealdb::engine::local::Db;

use crate::database;

use super::NodeIdentity;
use super::election::{ElectionState, has_quorum};
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
        repository.ensure_election_state().await?;
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

    pub async fn election_state(&self) -> io::Result<ElectionState> {
        self.client
            .select::<Option<ElectionState>>(("election_state", "singleton"))
            .await
            .map_err(database_error)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "election state is missing"))
    }

    pub async fn request_vote(&self, candidate_id: &str, term: u64) -> io::Result<bool> {
        let candidate = self.member(candidate_id).await?;
        if !candidate.voting {
            return Ok(false);
        }

        let mut state = self.election_state().await?;
        if term < state.current_term {
            return Ok(false);
        }
        if term > state.current_term {
            state.current_term = term;
            state.voted_for = None;
            state.leader_id = None;
        }
        if state
            .voted_for
            .as_deref()
            .is_some_and(|voted_for| voted_for != candidate_id)
        {
            return Ok(false);
        }

        state.voted_for = Some(candidate_id.to_owned());
        self.save_election_state(&state).await?;
        Ok(true)
    }

    pub async fn become_primary(
        &self,
        candidate_id: &str,
        term: u64,
        votes: usize,
    ) -> io::Result<bool> {
        let members = self.members().await?;
        let voting_members = members.iter().filter(|member| member.voting).count();
        if !members.iter().any(|member| member.node_id == candidate_id)
            || !has_quorum(voting_members, votes)
        {
            return Ok(false);
        }

        let mut state = self.election_state().await?;
        if term != state.current_term {
            return Ok(false);
        }
        state.leader_id = Some(candidate_id.to_owned());
        state.voted_for = Some(candidate_id.to_owned());
        self.save_election_state(&state).await?;
        self.set_member_role(candidate_id, NodeRole::Primary)
            .await?;
        Ok(true)
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

    async fn ensure_election_state(&self) -> io::Result<()> {
        if self.election_state().await.is_ok() {
            return Ok(());
        }

        self.client
            .create::<Option<ElectionState>>(("election_state", "singleton"))
            .content(ElectionState {
                current_term: 0,
                voted_for: None,
                leader_id: None,
            })
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn save_election_state(&self, state: &ElectionState) -> io::Result<()> {
        self.client
            .update::<Option<ElectionState>>(("election_state", "singleton"))
            .content(state.clone())
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn set_member_role(&self, node_id: &str, role: NodeRole) -> io::Result<()> {
        let mut member = self.member(node_id).await?;
        member.role = role;
        self.client
            .update::<Option<ClusterMember>>(("cluster_member", node_id))
            .content(member)
            .await
            .map_err(database_error)?;
        Ok(())
    }
}

fn database_error(error: impl std::error::Error) -> io::Error {
    io::Error::other(format!("database error: {error}"))
}
