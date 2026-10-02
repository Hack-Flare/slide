use std::io::{self, BufRead, Read};
use std::time::Instant;

use serde::Serialize;

use crate::authorization::{AuthorizationPolicy, Permission};
use crate::identity::AuthenticatedClient;
use crate::node_state::{NodeIdentity, NodeRole};

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
    identity: &NodeIdentity,
    role: NodeRole,
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
                    node_id: identity.node_id.clone(),
                    node_name: identity.node_name.clone(),
                    role: role.as_str().to_owned(),
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
        role: String,
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
