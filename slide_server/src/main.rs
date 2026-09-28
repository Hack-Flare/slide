mod authorization;
mod config;
mod identity;
mod protocol;
mod tls;

use std::env;
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use rustls::{ServerConnection, StreamOwned};

use crate::authorization::AuthorizationPolicy;
use crate::config::NodeConfig;
use crate::identity::from_connection;
use crate::protocol::{NodeState, read_request, response_for_request};

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
    let mut reader = BufReader::new(&mut stream);
    let Some(request_line) = read_request(&mut reader)? else {
        return Ok(());
    };
    let response = response_for_request(&request_line, state, &client, authorization);
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
