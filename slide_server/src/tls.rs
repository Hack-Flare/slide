use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};

pub fn load_server_config(
    certificate_path: &Path,
    private_key_path: &Path,
    client_ca_path: &Path,
) -> io::Result<Arc<ServerConfig>> {
    let certificates = load_certificates(certificate_path)?;
    let private_key = load_private_key(private_key_path)?;
    let client_ca = load_certificates(client_ca_path)?;
    let mut roots = RootCertStore::empty();

    for certificate in client_ca {
        roots.add(certificate).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid client CA certificate: {error}"),
            )
        })?;
    }

    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid client certificate verifier: {error}"),
            )
        })?;

    ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(certificates, private_key)
        .map(Arc::new)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid server certificate or private key: {error}"),
            )
        })
}

fn load_certificates(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn load_private_key(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    rustls_pemfile::private_key(&mut BufReader::new(file))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key found"))
}
