use slide_server::node_state::{NodeRole, NodeStateRepository, quorum_size};

#[tokio::test(flavor = "current_thread")]
async fn repository_creates_and_reads_identity() {
    let repository = NodeStateRepository::open("test-node")
        .await
        .expect("database opens");
    let identity = repository.node_identity().await.expect("identity exists");

    assert!(!identity.node_id.is_empty());
    assert_eq!(identity.node_name, "test-node");
}

#[tokio::test(flavor = "current_thread")]
async fn separate_memory_databases_have_separate_identities() {
    let first = NodeStateRepository::open("first-name")
        .await
        .expect("first database opens")
        .node_identity()
        .await
        .expect("first identity exists");
    let second = NodeStateRepository::open("second-name")
        .await
        .expect("second database opens")
        .node_identity()
        .await
        .expect("second identity exists");

    assert_ne!(first.node_id, second.node_id);
    assert_eq!(first.node_name, "first-name");
    assert_eq!(second.node_name, "second-name");
}

#[tokio::test(flavor = "current_thread")]
async fn joining_adds_a_voting_follower() {
    let repository = NodeStateRepository::open("local")
        .await
        .expect("database opens");
    let members = repository.members().await.expect("members are readable");

    assert_eq!(members.len(), 1);
    assert!(matches!(members[0].role, NodeRole::Follower));
    assert!(members[0].voting);
}

#[tokio::test(flavor = "current_thread")]
async fn leaving_is_disabled() {
    let repository = NodeStateRepository::open("local")
        .await
        .expect("database opens");

    let error = repository
        .leave("node-id")
        .await
        .expect_err("leave is disabled");

    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn quorum_requires_a_strict_majority() {
    assert_eq!(quorum_size(1), 1);
    assert_eq!(quorum_size(2), 2);
    assert_eq!(quorum_size(3), 2);
    assert_eq!(quorum_size(4), 3);
}
