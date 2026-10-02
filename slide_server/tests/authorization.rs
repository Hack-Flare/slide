use slide_server::authorization::{AuthorizationPolicy, Permission, validate_fingerprint};
use slide_server::identity::AuthenticatedClient;

const FINGERPRINT: &str = "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81";

fn client(fingerprint: &str) -> AuthenticatedClient {
    AuthenticatedClient {
        certificate_fingerprint: fingerprint.to_owned(),
    }
}

#[test]
fn allowed_client_can_read_status() {
    let policy = AuthorizationPolicy::new([FINGERPRINT.to_owned()]);

    assert!(policy.allows(&client(FINGERPRINT), Permission::ReadStatus));
}

#[test]
fn unknown_client_cannot_read_status() {
    let policy = AuthorizationPolicy::new(std::iter::empty());

    assert!(!policy.allows(&client(FINGERPRINT), Permission::ReadStatus));
}

#[test]
fn fingerprints_must_be_sha256_length_hex() {
    assert!(validate_fingerprint(FINGERPRINT).is_ok());
    assert!(validate_fingerprint("not-a-fingerprint").is_err());
}
