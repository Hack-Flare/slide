use std::io::Cursor;
use std::time::Instant;

use slide_server::authorization::AuthorizationPolicy;
use slide_server::identity::AuthenticatedClient;
use slide_server::node_state::{NodeIdentity, NodeRole};
use slide_server::protocol::{
    Method, NodeState, ReadRequest, Request, Target, read_request, response_for_request,
};

fn state() -> NodeState {
    NodeState {
        started_at: Instant::now(),
    }
}

fn identity() -> NodeIdentity {
    NodeIdentity {
        node_id: "test-id".to_owned(),
        node_name: "test-node".to_owned(),
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
        join: None,
    }
}

#[test]
fn slide_handshake_is_required() {
    let mut request = Cursor::new(
        b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: connect-me-please\r\nSlide-Versions: 1\r\n\r\n",
    );

    let request = read_request(&mut request).expect("valid request");

    assert!(matches!(request, ReadRequest::Accepted(_)));
}

#[test]
fn ordinary_http_is_not_a_slide_request() {
    let mut request = Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");

    assert!(matches!(
        read_request(&mut request).expect("valid request"),
        ReadRequest::NotSlide
    ));
}

#[test]
fn unsupported_version_is_rejected() {
    let mut request = Cursor::new(
        b"GET /health HTTP/1.1\r\nConnection: connect-me-please\r\nSlide-Versions: 2\r\n\r\n",
    );

    assert!(matches!(
        read_request(&mut request).expect("valid request"),
        ReadRequest::UnsupportedVersion
    ));
}

#[test]
fn join_request_decodes_typed_body() {
    let mut request = Cursor::new(
        b"POST /cluster/join HTTP/1.1\r\nConnection: connect-me-please\r\nSlide-Versions: 1\r\nContent-Length: 51\r\n\r\n{\"node_id\":\"joining-id\",\"node_name\":\"joining-node\"}",
    );

    let ReadRequest::Accepted(request) = read_request(&mut request).expect("valid request") else {
        panic!("join request was not accepted");
    };
    let join = request.join.expect("join body");

    assert_eq!(join.node_id, "joining-id");
    assert_eq!(join.node_name, "joining-node");
}

#[test]
fn health_endpoint_returns_ok() {
    let response = response_for_request(
        &request(Method::Get, Target::Health),
        &state(),
        &identity(),
        NodeRole::Follower,
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
        &identity(),
        NodeRole::Follower,
        &client(),
        &policy(),
    )
    .expect("response serializes");

    assert!(response.contains("\"type\":\"status\""));
    assert!(response.contains("\"node_id\":\"test-id\""));
    assert!(response.contains("\"node_name\":\"test-node\""));
    assert!(response.contains("\"role\":\"follower\""));
    assert!(response.contains("\"uptime_seconds\":"));
    assert!(response.contains("\"client_certificate_fingerprint\":\"test-fingerprint\""));
}

#[test]
fn unknown_path_returns_not_found() {
    let response = response_for_request(
        &request(Method::Get, Target::Other),
        &state(),
        &identity(),
        NodeRole::Follower,
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
        &identity(),
        NodeRole::Follower,
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
        join: None,
    };
    let response = response_for_request(
        &request,
        &state(),
        &identity(),
        NodeRole::Follower,
        &client(),
        &policy(),
    )
    .expect("response serializes");

    assert!(response.contains("Slide-Request-Id: request-123\r\n"));
}
