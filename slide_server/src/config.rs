use std::io;
use std::path::PathBuf;

pub const DEFAULT_LISTEN_ADDRESS: &str = "0.0.0.0:1";
pub const DEV_LISTEN_ADDRESS: &str = "127.0.0.1:10000";

#[derive(Debug)]
pub struct NodeConfig {
    pub listen_address: String,
    pub node_name: String,
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
    pub client_ca_path: PathBuf,
    pub status_readers: Vec<String>,
}

impl NodeConfig {
    pub fn from_args(args: impl Iterator<Item = String>) -> io::Result<Self> {
        let mut listen_address = None;
        let mut node_name = "local".to_owned();
        let mut certificate_path = None;
        let mut private_key_path = None;
        let mut client_ca_path = None;
        let mut status_readers = Vec::new();
        let mut dev_mode = false;
        let mut args = args.peekable();

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--dev" => dev_mode = true,
                "--listen" => listen_address = Some(next_argument(&mut args, "--listen")?),
                "--node-name" => node_name = next_argument(&mut args, "--node-name")?,
                "--cert" => certificate_path = Some(next_path(&mut args, "--cert")?),
                "--key" => private_key_path = Some(next_path(&mut args, "--key")?),
                "--client-ca" => client_ca_path = Some(next_path(&mut args, "--client-ca")?),
                "--allow-client" => {
                    let fingerprint = next_argument(&mut args, "--allow-client")?;
                    crate::authorization::validate_fingerprint(&fingerprint)?;
                    status_readers.push(fingerprint);
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown argument: {argument}"),
                    ));
                }
            }
        }

        if node_name.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "node name cannot be empty",
            ));
        }

        Ok(Self {
            listen_address: listen_address.unwrap_or_else(|| {
                if dev_mode {
                    DEV_LISTEN_ADDRESS.to_owned()
                } else {
                    DEFAULT_LISTEN_ADDRESS.to_owned()
                }
            }),
            node_name,
            certificate_path: required_path(certificate_path, "--cert")?,
            private_key_path: required_path(private_key_path, "--key")?,
            client_ca_path: required_path(client_ca_path, "--client-ca")?,
            status_readers,
        })
    }
}

fn next_argument(args: &mut impl Iterator<Item = String>, flag: &str) -> io::Result<String> {
    args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} requires a value"),
        )
    })
}

fn next_path(args: &mut impl Iterator<Item = String>, flag: &str) -> io::Result<PathBuf> {
    Ok(PathBuf::from(next_argument(args, flag)?))
}

fn required_path(path: Option<PathBuf>, flag: &str) -> io::Result<PathBuf> {
    path.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} is required for mutual TLS"),
        )
    })
}

fn print_help() {
    println!(
        "Usage: slide_server --cert PATH --key PATH --client-ca PATH [OPTIONS]\n\n\
         Defaults to listening on {DEFAULT_LISTEN_ADDRESS} as node 'local'.\n\
         --dev listens on {DEV_LISTEN_ADDRESS}.\n\n\
         TLS options:\n\
           --cert PATH       Server certificate chain in PEM format\n\
           --key PATH        Server private key in PEM format\n\
           --client-ca PATH  CA certificate for verifying client certificates\n\
         Authorization options:\n\
           --allow-client FINGERPRINT  Grant status read access"
    );
}
