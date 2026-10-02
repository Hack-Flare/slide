use rustls::pki_types::CertificateDer;
use slide_server::identity::AuthenticatedClient;

#[test]
fn fingerprint_is_lowercase_sha256() {
    let identity = AuthenticatedClient::from_certificate(&CertificateDer::from(vec![1, 2, 3]));

    assert_eq!(
        identity.certificate_fingerprint,
        "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
    );
}
