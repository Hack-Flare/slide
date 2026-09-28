use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const DEFAULT_LISTEN_ADDRESS: &str = "0.0.0.0:1";
const DEV_LISTEN_ADDRESS: &str = "127.0.0.1:10000";

struct NodeConfig {
    listen_address: String,
    node_name: String,
}

struct NodeState {
    config: NodeConfig,
    started_at: Instant,
}

fn main() -> io::Result<()> {
    let config = NodeConfig::from_args(env::args().skip(1))?;
    let listener = TcpListener::bind(&config.listen_address)?;
    let address = listener.local_addr()?;
    let state = NodeState {
        config,
        started_at: Instant::now(),
    };

    println!(
        "slided node '{}' listening on {address}",
        state.config.node_name
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &state) {
                    eprintln!("failed to handle connection: {error}");
                }
            }
            Err(error) => eprintln!("failed to accept connection: {error}"),
        }
    }

    Ok(())
}

impl NodeConfig {
    fn from_args(args: impl Iterator<Item = String>) -> io::Result<Self> {
        let mut listen_address = None;
        let mut node_name = "local".to_owned();
        let mut dev_mode = false;
        let mut args = args.peekable();

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--dev" => dev_mode = true,
                "--listen" => {
                    listen_address = Some(next_argument(&mut args, "--listen")?);
                }
                "--node-name" => {
                    node_name = next_argument(&mut args, "--node-name")?;
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

fn print_help() {
    println!(
        "Usage: slide_server [--dev] [--listen ADDRESS] [--node-name NAME]\n\n\
         Defaults to listening on {DEFAULT_LISTEN_ADDRESS} as node 'local'.\n\
         --dev listens on {DEV_LISTEN_ADDRESS}."
    );
}

fn handle_connection(mut stream: TcpStream, state: &NodeState) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut request_line = String::new();
    BufReader::new(&mut stream).read_line(&mut request_line)?;
    let response = response_for_request(&request_line, state);
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn response_for_request(request_line: &str, state: &NodeState) -> String {
    let mut parts = request_line.split_whitespace();
    let method = parts.next();
    let path = parts.next();

    match (method, path) {
        (Some("GET"), Some("/health")) => http_response(200, "{\"status\":\"ok\"}"),
        (Some("GET"), Some("/status")) => http_response(200, &status_body(state)),
        (Some("GET"), Some(_)) => http_response(404, "{\"error\":\"not found\"}"),
        (Some(_), Some(_)) => http_response(405, "{\"error\":\"method not allowed\"}"),
        _ => http_response(400, "{\"error\":\"bad request\"}"),
    }
}

fn status_body(state: &NodeState) -> String {
    format!(
        "{{\"node_name\":\"{}\",\"uptime_seconds\":{}}}",
        state.config.node_name,
        state.started_at.elapsed().as_secs()
    )
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Unknown",
    };

    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> NodeState {
        NodeState {
            config: NodeConfig {
                listen_address: DEFAULT_LISTEN_ADDRESS.to_owned(),
                node_name: "test-node".to_owned(),
            },
            started_at: Instant::now(),
        }
    }

    #[test]
    fn health_endpoint_returns_ok() {
        let response = response_for_request("GET /health HTTP/1.1\r\n", &state());

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with("{\"status\":\"ok\"}"));
    }

    #[test]
    fn status_endpoint_includes_node_name() {
        let response = response_for_request("GET /status HTTP/1.1\r\n", &state());

        assert!(response.contains("\"node_name\":\"test-node\""));
        assert!(response.contains("\"uptime_seconds\":"));
    }

    #[test]
    fn unknown_path_returns_not_found() {
        let response = response_for_request("GET /missing HTTP/1.1\r\n", &state());

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn config_accepts_dev_options() {
        let config = NodeConfig::from_args(
            [
                "--dev".to_owned(),
                "--listen".to_owned(),
                "127.0.0.1:9000".to_owned(),
                "--node-name".to_owned(),
                "dev-node".to_owned(),
            ]
            .into_iter(),
        )
        .expect("valid arguments");

        assert_eq!(config.listen_address, "127.0.0.1:9000");
        assert_eq!(config.node_name, "dev-node");
    }

    #[test]
    fn dev_mode_uses_dev_listen_address() {
        let config =
            NodeConfig::from_args(["--dev".to_owned()].into_iter()).expect("valid arguments");

        assert_eq!(config.listen_address, DEV_LISTEN_ADDRESS);
    }
}
