use slide_server::config::{DEV_LISTEN_ADDRESS, NodeConfig};

#[test]
fn dev_mode_uses_dev_listen_address() {
    let args = [
        "--dev",
        "--cert",
        "server.crt",
        "--key",
        "server.key",
        "--client-ca",
        "ca.crt",
    ]
    .into_iter()
    .map(str::to_owned);
    let config = NodeConfig::from_args(args).expect("valid arguments");

    assert_eq!(config.listen_address, DEV_LISTEN_ADDRESS);
}

#[test]
fn tls_paths_are_required() {
    let error = NodeConfig::from_args(std::iter::empty()).expect_err("TLS paths are required");

    assert!(error.to_string().contains("--cert"));
}
