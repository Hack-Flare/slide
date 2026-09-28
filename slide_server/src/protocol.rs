use std::io::{self, BufRead, Read};
use std::time::Instant;

use serde::Serialize;

use crate::authorization::{AuthorizationPolicy, Permission};
use crate::identity::AuthenticatedClient;

const MAX_HEADER_LINE_BYTES: u64 = 8 * 1024;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_HEADER_COUNT: usize = 64;
const SLIDE_CONNECTION_TOKEN: &str = "connect-me-please";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[u16] = &[1];

pub enum ReadRequest {
    Accepted(Request),
    NotSlide,
    UnsupportedVersion,
}

pub struct Request {
    pub method: Method,
    pub target: Target,
    pub protocol_version: u16,
    pub request_id: Option<String>,
}

pub enum Method {
    Get,
    Other,
}

pub enum Target {
    Health,
    Status,
    Other,
}

pub fn read_request(reader: &mut impl BufRead) -> io::Result<ReadRequest> {
    let mut request_line = String::new();
    let request_line_bytes = read_line_with_limit(reader, &mut request_line)?;

    if request_line_bytes == 0 {
        return Ok(ReadRequest::NotSlide);
    }

    let mut header_bytes = request_line_bytes;
    let mut header_count = 0;
    let mut slide_handshake = false;
    let mut client_versions = Vec::new();
    let mut request_id = None;

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
        if let Some((name, value)) = header_line.split_once(':')
            && name.trim().eq_ignore_ascii_case("slide-versions")
        {
            for version in value.split(',') {
                client_versions.push(version.trim().parse::<u16>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid Slide-Versions header")
                })?);
            }
        }
        if let Some((name, value)) = header_line.split_once(':')
            && name.trim().eq_ignore_ascii_case("slide-request-id")
        {
            let value = value.trim();
            if value.is_empty()
                || value.len() > 128
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid Slide-Request-Id header",
                ));
            }
            request_id = Some(value.to_owned());
        }
    }

    if !slide_handshake {
        return Ok(ReadRequest::NotSlide);
    }

    let Some(&protocol_version) = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|version| client_versions.contains(version))
    else {
        return Ok(ReadRequest::UnsupportedVersion);
    };

    Ok(ReadRequest::Accepted(Request::parse(
        &request_line,
        protocol_version,
        request_id,
    )?))
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
    pub node_id: String,
    pub node_name: String,
    pub started_at: Instant,
}

impl Request {
    fn parse(
        request_line: &str,
        protocol_version: u16,
        request_id: Option<String>,
    ) -> io::Result<Self> {
        let mut parts = request_line.split_whitespace();
        let method = match parts.next() {
            Some("GET") => Method::Get,
            Some(_) => Method::Other,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "request is missing a method",
                ));
            }
        };
        let target = match parts.next() {
            Some("/health") => Target::Health,
            Some("/status") => Target::Status,
            Some(_) => Target::Other,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "request is missing a target",
                ));
            }
        };

        Ok(Self {
            method,
            target,
            protocol_version,
            request_id,
        })
    }
}

pub fn response_for_request(
    request: &Request,
    state: &NodeState,
    client: &AuthenticatedClient,
    policy: &AuthorizationPolicy,
) -> io::Result<String> {
    match (&request.method, &request.target) {
        (Method::Get, Target::Health) => http_response(
            request,
            200,
            ResponseBody::Health {
                status: "ok".to_owned(),
            },
        ),
        (Method::Get, Target::Status) if policy.allows(client, Permission::ReadStatus) => {
            http_response(
                request,
                200,
                ResponseBody::Status {
                    node_id: state.node_id.clone(),
                    node_name: state.node_name.clone(),
                    uptime_seconds: state.started_at.elapsed().as_secs(),
                    client_certificate_fingerprint: client.certificate_fingerprint.clone(),
                },
            )
        }
        (Method::Get, Target::Status) => http_response(
            request,
            403,
            ResponseBody::Error {
                error: ProtocolError::new(ErrorCode::Forbidden, "client is not authorized"),
            },
        ),
        (Method::Get, Target::Other) => http_response(
            request,
            404,
            ResponseBody::Error {
                error: ProtocolError::new(ErrorCode::NotFound, "request target was not found"),
            },
        ),
        (Method::Other, _) => http_response(
            request,
            405,
            ResponseBody::Error {
                error: ProtocolError::new(ErrorCode::MethodNotAllowed, "method is not allowed"),
            },
        ),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
enum ResponseBody {
    Health {
        status: String,
    },
    Status {
        node_id: String,
        node_name: String,
        uptime_seconds: u64,
        client_certificate_fingerprint: String,
    },
    Error {
        error: ProtocolError,
    },
}

#[derive(Serialize)]
struct ProtocolError {
    code: ErrorCode,
    message: String,
}

impl ProtocolError {
    fn new(code: ErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.to_owned(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    Forbidden,
    MethodNotAllowed,
    NotFound,
    UnsupportedVersion,
}

pub fn unsupported_version_response() -> io::Result<String> {
    let supported_versions = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let body = serde_json::to_string(&ResponseBody::Error {
        error: ProtocolError::new(
            ErrorCode::UnsupportedVersion,
            "no mutually supported protocol version",
        ),
    })
    .map_err(io::Error::other)?;

    Ok(format!(
        "HTTP/1.1 426 Upgrade Required\r\nSlide-Versions: {supported_versions}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn http_response(request: &Request, status: u16, body: ResponseBody) -> io::Result<String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        403 => "Forbidden",
        _ => "Unknown",
    };

    let body = serde_json::to_string(&body).map_err(io::Error::other)?;
    let request_id = request
        .request_id
        .as_deref()
        .map(|request_id| format!("Slide-Request-Id: {request_id}\r\n"))
        .unwrap_or_default();

    Ok(format!(
        "HTTP/1.1 {status} {reason}\r\nSlide-Version: {}\r\n{request_id}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        request.protocol_version,
        body.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> NodeState {
        NodeState {
            node_id: "test-id".to_owned(),
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

    fn request(method: Method, target: Target) -> Request {
        Request {
            method,
            target,
            protocol_version: 1,
            request_id: None,
        }
    }

    #[test]
    fn slide_handshake_is_required() {
        let mut request = std::io::Cursor::new(
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: connect-me-please\r\nSlide-Versions: 1\r\n\r\n",
        );

        let request = read_request(&mut request).expect("valid request");

        assert!(matches!(request, ReadRequest::Accepted(_)));
    }

    #[test]
    fn ordinary_http_is_not_a_slide_request() {
        let mut request = std::io::Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");

        assert!(matches!(
            read_request(&mut request).expect("valid request"),
            ReadRequest::NotSlide
        ));
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let mut request = std::io::Cursor::new(
            b"GET /health HTTP/1.1\r\nConnection: connect-me-please\r\nSlide-Versions: 2\r\n\r\n",
        );

        assert!(matches!(
            read_request(&mut request).expect("valid request"),
            ReadRequest::UnsupportedVersion
        ));
    }

    #[test]
    fn health_endpoint_returns_ok() {
        let response = response_for_request(
            &request(Method::Get, Target::Health),
            &state(),
            &client(),
            &policy(),
        )
        .expect("response serializes");

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with("{\"type\":\"health\",\"status\":\"ok\"}"));
    }

    #[test]
    fn status_endpoint_includes_node_name() {
        let response = response_for_request(
            &request(Method::Get, Target::Status),
            &state(),
            &client(),
            &policy(),
        )
        .expect("response serializes");

        assert!(response.contains("\"type\":\"status\""));
        assert!(response.contains("\"node_id\":\"test-id\""));
        assert!(response.contains("\"node_name\":\"test-node\""));
        assert!(response.contains("\"uptime_seconds\":"));
        assert!(response.contains("\"client_certificate_fingerprint\":\"test-fingerprint\""));
    }

    #[test]
    fn unknown_path_returns_not_found() {
        let response = response_for_request(
            &request(Method::Get, Target::Other),
            &state(),
            &client(),
            &policy(),
        )
        .expect("response serializes");

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn unauthorized_status_returns_forbidden() {
        let client = AuthenticatedClient {
            certificate_fingerprint: "unknown-client".to_owned(),
        };
        let response = response_for_request(
            &request(Method::Get, Target::Status),
            &state(),
            &client,
            &policy(),
        )
        .expect("response serializes");

        assert!(response.starts_with("HTTP/1.1 403 Forbidden"));
    }

    #[test]
    fn request_id_is_returned_in_response() {
        let request = Request {
            method: Method::Get,
            target: Target::Health,
            protocol_version: 1,
            request_id: Some("request-123".to_owned()),
        };
        let response = response_for_request(&request, &state(), &client(), &policy())
            .expect("response serializes");

        assert!(response.contains("Slide-Request-Id: request-123\r\n"));
    }
}
