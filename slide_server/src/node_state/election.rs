use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct ElectionState {
    pub current_term: u64,
    pub voted_for: Option<String>,
    pub leader_id: Option<String>,
}

pub fn has_quorum(voting_members: usize, votes: usize) -> bool {
    votes >= super::quorum_size(voting_members)
}
