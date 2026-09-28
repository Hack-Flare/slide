use std::io::{self, BufRead, Read};
use std::time::Instant;

use crate::authorization::{AuthorizationPolicy, Permission};
use crate::identity::AuthenticatedClient;

const MAX_HEADER_LINE_BYTES: u64 = 8 * 1024;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_HEADER_COUNT: usize = 64;
const SLIDE_CONNECTION_TOKEN: &str = "connect-me-please";

pub fn read_request(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut request_line = String::new();
    let request_line_bytes = read_line_with_limit(reader, &mut request_line)?;

    if request_line_bytes == 0 {
        return Ok(None);
    }

    let mut header_bytes = request_line_bytes;
    let mut header_count = 0;
    let mut slide_handshake = false;

    loop {
        let mut header_line = String::new();
        let line_bytes = read_line_with_limit(reader, &mut header_line)?;

        if line_bytes == 0 || header_line == "\r\n" || header_line == "\n" {
            break;
        }

        header_count += 1;
        header_bytes += line_bytes;
        if header_count > MAX_HEADER_COUNT || header_bytes > MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "request headers are too large",
            ));
        }

        if let Some((name, value)) = header_line.split_once(':')
            && name.trim().eq_ignore_ascii_case("connection")
        {
            slide_handshake = value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case(SLIDE_CONNECTION_TOKEN));
        }
    }

    if slide_handshake {
        Ok(Some(request_line))
    } else {
        Ok(None)
    }
}

fn read_line_with_limit(reader: &mut impl BufRead, line: &mut String) -> io::Result<usize> {
    let bytes_read = reader.take(MAX_HEADER_LINE_BYTES + 1).read_line(line)?;

    if bytes_read > MAX_HEADER_LINE_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "request header line is too large",
        ));
    }

    Ok(bytes_read)
}

pub struct NodeState {
    pub node_name: String,
    pub started_at: Instant,
}

pub fn response_for_request(
    request_line: &str,
    state: &NodeState,
    client: &AuthenticatedClient,
    policy: &AuthorizationPolicy,
) -> String {
    let mut parts = request_line.split_whitespace();
    let method = parts.next();
    let path = parts.next();

    match (method, path) {
        (Some("GET"), Some("/health")) => http_response(200, "{\"status\":\"ok\"}"),
        (Some("GET"), Some("/status")) if policy.allows(client, Permission::ReadStatus) => {
            http_response(200, &status_body(state, client))
        }
        (Some("GET"), Some("/status")) => http_response(403, "{\"error\":\"forbidden\"}"),
        (Some("GET"), Some(_)) => http_response(404, "{\"error\":\"not found\"}"),
        (Some(_), Some(_)) => http_response(405, "{\"error\":\"method not allowed\"}"),
        _ => http_response(400, "{\"error\":\"bad request\"}"),
    }
}

fn status_body(state: &NodeState, client: &AuthenticatedClient) -> String {
    format!(
        "{{\"node_name\":\"{}\",\"uptime_seconds\":{},\"client_certificate_fingerprint\":\"{}\"}}",
        state.node_name,
        state.started_at.elapsed().as_secs(),
        client.certificate_fingerprint
    )
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        403 => "Forbidden",
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
            node_name: "test-node".to_owned(),
            started_at: Instant::now(),
        }
    }

    fn client() -> AuthenticatedClient {
        AuthenticatedClient {
            certificate_fingerprint: "test-fingerprint".to_owned(),
        }
    }

    fn policy() -> AuthorizationPolicy {
        AuthorizationPolicy::new(["test-fingerprint".to_owned()])
    }

    #[test]
    fn slide_handshake_is_required() {
        let mut request = std::io::Cursor::new(
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: connect-me-please\r\n\r\n",
        );

        let request_line = read_request(&mut request).expect("valid request");

        assert_eq!(request_line.as_deref(), Some("GET /health HTTP/1.1\r\n"));
    }

    #[test]
    fn ordinary_http_is_not_a_slide_request() {
        let mut request = std::io::Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");

        assert!(read_request(&mut request).expect("valid request").is_none());
    }

    #[test]
    fn health_endpoint_returns_ok() {
        let response =
            response_for_request("GET /health HTTP/1.1\r\n", &state(), &client(), &policy());

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with("{\"status\":\"ok\"}"));
    }

    #[test]
    fn status_endpoint_includes_node_name() {
        let response =
            response_for_request("GET /status HTTP/1.1\r\n", &state(), &client(), &policy());

        assert!(response.contains("\"node_name\":\"test-node\""));
        assert!(response.contains("\"uptime_seconds\":"));
        assert!(response.contains("\"client_certificate_fingerprint\":\"test-fingerprint\""));
    }

    #[test]
    fn unknown_path_returns_not_found() {
        let response =
            response_for_request("GET /missing HTTP/1.1\r\n", &state(), &client(), &policy());

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn unauthorized_status_returns_forbidden() {
        let client = AuthenticatedClient {
            certificate_fingerprint: "unknown-client".to_owned(),
        };
        let response =
            response_for_request("GET /status HTTP/1.1\r\n", &state(), &client, &policy());

        assert!(response.starts_with("HTTP/1.1 403 Forbidden"));
    }
}
