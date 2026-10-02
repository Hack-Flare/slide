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
