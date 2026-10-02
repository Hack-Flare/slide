use slide_server::database::NodeStateRepository;

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
