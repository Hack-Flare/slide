use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Follower,
    Candidate,
    Primary,
}

impl NodeRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Follower => "follower",
            Self::Candidate => "candidate",
            Self::Primary => "primary",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ClusterMember {
    pub node_id: String,
    pub node_name: String,
    pub role: NodeRole,
    pub voting: bool,
}

pub fn quorum_size(voting_members: usize) -> usize {
    voting_members / 2 + 1
}
