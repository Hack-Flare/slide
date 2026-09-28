use std::io;

use rustls::pki_types::CertificateDer;
use sha2::{Digest, Sha256};

pub struct AuthenticatedClient {
    pub certificate_fingerprint: String,
}

impl AuthenticatedClient {
    pub fn from_certificate(certificate: &CertificateDer<'_>) -> Self {
        let fingerprint = Sha256::digest(certificate.as_ref());
        let certificate_fingerprint = fingerprint
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        Self {
            certificate_fingerprint,
        }
    }
}

pub fn from_connection(connection: &rustls::ServerConnection) -> io::Result<AuthenticatedClient> {
    let certificate = connection
        .peer_certificates()
        .and_then(|certificates| certificates.first())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "TLS connection has no authenticated client certificate",
            )
        })?;

    Ok(AuthenticatedClient::from_certificate(certificate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_lowercase_sha256() {
        let identity = AuthenticatedClient::from_certificate(&CertificateDer::from(vec![1, 2, 3]));

        assert_eq!(
            identity.certificate_fingerprint,
            "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        );
    }
}
