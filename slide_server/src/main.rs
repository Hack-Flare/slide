use std::env;
use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use rustls::{ServerConnection, StreamOwned};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use slide_server::authorization::AuthorizationPolicy;
use slide_server::config::NodeConfig;
use slide_server::identity::from_connection;
use slide_server::node_state::NodeStateRepository;
use slide_server::protocol::{
    NodeState, ReadRequest, read_request, response_for_request, unsupported_version_response,
};
use slide_server::tls;

fn main() -> io::Result<()> {
    let config = NodeConfig::from_args(env::args().skip(1))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let repository = Arc::new(runtime.block_on(NodeStateRepository::open(&config.node_name))?);
    let runtime_handle = runtime.handle().clone();
    let tls_config = tls::load_server_config(
        &config.certificate_path,
        &config.private_key_path,
        &config.client_ca_path,
    )?;
    let listener = TcpListener::bind(&config.listen_address)?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let state = Arc::new(NodeState {
        started_at: std::time::Instant::now(),
    });
    let authorization = Arc::new(AuthorizationPolicy::new(config.status_readers));
    let shutdown = Arc::new(AtomicBool::new(false));
    install_shutdown_handler(Arc::clone(&shutdown))?;
    let mut connections = Vec::new();

    println!(
        "slided node '{}' listening with mutual TLS on {address}",
        config.node_name
    );

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _peer_address)) => {
                let tls_config = Arc::clone(&tls_config);
                let state = Arc::clone(&state);
                let authorization = Arc::clone(&authorization);
                let repository = Arc::clone(&repository);
                let runtime_handle = runtime_handle.clone();

                let connection = std::thread::spawn(move || {
                    if let Err(error) = handle_connection(
                        stream,
                        &tls_config,
                        &state,
                        &authorization,
                        &repository,
                        &runtime_handle,
                    ) {
                        eprintln!("failed to handle connection: {error}");
                    }
                });
                connections.push(connection);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => eprintln!("failed to accept connection: {error}"),
        }
    }

    eprintln!(
        "shutdown requested, draining {} connections",
        connections.len()
    );
    drain_connections(connections);
    eprintln!("shutdown complete");

    Ok(())
}

fn install_shutdown_handler(shutdown: Arc<AtomicBool>) -> io::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            shutdown.store(true, Ordering::Relaxed);
        }
    });

    Ok(())
}

fn drain_connections(connections: Vec<JoinHandle<()>>) {
    for connection in connections {
        if connection.join().is_err() {
            eprintln!("connection thread panicked during shutdown");
        }
    }
}

fn handle_connection(
    stream: TcpStream,
    tls_config: &Arc<rustls::ServerConfig>,
    state: &Arc<NodeState>,
    authorization: &Arc<AuthorizationPolicy>,
    repository: &Arc<NodeStateRepository>,
    runtime_handle: &tokio::runtime::Handle,
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
    let request = match read_request(&mut reader)? {
        ReadRequest::Accepted(request) => request,
        ReadRequest::NotSlide => return Ok(()),
        ReadRequest::UnsupportedVersion => {
            stream.write_all(unsupported_version_response()?.as_bytes())?;
            return stream.flush();
        }
    };
    let identity = runtime_handle.block_on(repository.node_identity())?;
    let response = response_for_request(&request, state, &identity, &client, authorization)?;
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
