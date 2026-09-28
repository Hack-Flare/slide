mod authorization;
mod config;
mod identity;
mod protocol;
mod tls;

use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use rustls::{ServerConnection, StreamOwned};

use crate::authorization::AuthorizationPolicy;
use crate::config::NodeConfig;
use crate::identity::from_connection;
use crate::protocol::{NodeState, response_for_request};

const MAX_REQUEST_LINE_BYTES: u64 = 8 * 1024;

fn main() -> io::Result<()> {
    let config = NodeConfig::from_args(env::args().skip(1))?;
    let tls_config = tls::load_server_config(
        &config.certificate_path,
        &config.private_key_path,
        &config.client_ca_path,
    )?;
    let listener = TcpListener::bind(&config.listen_address)?;
    let address = listener.local_addr()?;
    let state = Arc::new(NodeState {
        node_name: config.node_name,
        started_at: std::time::Instant::now(),
    });
    let authorization = Arc::new(AuthorizationPolicy::new(config.status_readers));

    println!(
        "slided node '{}' listening with mutual TLS on {address}",
        state.node_name
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tls_config = Arc::clone(&tls_config);
                let state = Arc::clone(&state);
                let authorization = Arc::clone(&authorization);

                std::thread::spawn(move || {
                    if let Err(error) =
                        handle_connection(stream, &tls_config, &state, &authorization)
                    {
                        eprintln!("failed to handle connection: {error}");
                    }
                });
            }
            Err(error) => eprintln!("failed to accept connection: {error}"),
        }
    }

    Ok(())
}

fn handle_connection(
    stream: TcpStream,
    tls_config: &Arc<rustls::ServerConfig>,
    state: &Arc<NodeState>,
    authorization: &Arc<AuthorizationPolicy>,
) -> io::Result<()> {
    let connection = ServerConnection::new(tls_config.clone())
        .map_err(|error| io::Error::other(format!("failed to create TLS connection: {error}")))?;
    let mut stream = StreamOwned::new(connection, stream);
    stream
        .get_mut()
        .set_read_timeout(Some(Duration::from_secs(5)))?;

    stream.conn.complete_io(&mut stream.sock)?;
    let client = from_connection(&stream.conn)?;
    let (request_line, request_line_too_large) = {
        let mut request_line = String::new();
        let bytes_read = BufReader::new(&mut stream)
            .take(MAX_REQUEST_LINE_BYTES + 1)
            .read_line(&mut request_line)?;
        (request_line, bytes_read > MAX_REQUEST_LINE_BYTES as usize)
    };
    let response = if request_line_too_large {
        response_for_request("", state, &client, authorization)
    } else {
        response_for_request(&request_line, state, &client, authorization)
    };
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
