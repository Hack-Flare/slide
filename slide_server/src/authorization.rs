use std::collections::HashSet;
use std::io;

use crate::identity::AuthenticatedClient;

#[derive(Clone, Copy)]
pub enum Permission {
    ReadStatus,
}

pub struct AuthorizationPolicy {
    status_readers: HashSet<String>,
}

impl AuthorizationPolicy {
    pub fn new(status_readers: impl IntoIterator<Item = String>) -> Self {
        Self {
            status_readers: status_readers.into_iter().collect(),
        }
    }

    pub fn allows(&self, client: &AuthenticatedClient, permission: Permission) -> bool {
        match permission {
            Permission::ReadStatus => self
                .status_readers
                .contains(&client.certificate_fingerprint),
        }
    }
}

pub fn validate_fingerprint(fingerprint: &str) -> io::Result<()> {
    if fingerprint.len() != 64 || !fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "client fingerprint must be 64 hexadecimal characters",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
