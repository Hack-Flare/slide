use std::time::Instant;

pub struct NodeState {
    pub node_name: String,
    pub started_at: Instant,
}

pub fn response_for_request(request_line: &str, state: &NodeState) -> String {
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
        state.node_name,
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
            node_name: "test-node".to_owned(),
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
}
