//! STORY-7-2: REST API HTTP/1.1 server using raw tokio TCP.
//!
//! Provides the following endpoints:
//!   GET  /health         — health check
//!   POST /query          — execute a FunQL query stub
//!   POST /understand     — semantic interface stub
//!   POST /causal/trace   — causal path trace stub

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// A minimal HTTP/1.1 server that listens on a TCP address and dispatches
/// incoming requests to the REST handlers.
pub struct HttpServer {
    addr: String,
}

impl HttpServer {
    /// Create a new `HttpServer` that will bind to `addr`.
    pub fn new(addr: impl Into<String>) -> Self {
        Self { addr: addr.into() }
    }

    /// Bind and start accepting connections.  Each connection is handled in its
    /// own tokio task so the server can serve multiple clients concurrently.
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("HTTP REST API listening on {}", self.addr);
        loop {
            let (stream, peer_addr) = listener.accept().await?;
            tracing::debug!("HTTP connection from {}", peer_addr);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream).await {
                    tracing::error!("HTTP connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Low-level HTTP/1.1 connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(stream: TcpStream) -> Result<()> {
    let mut reader = BufReader::new(stream);

    // --- Parse request line: "METHOD /path HTTP/1.1\r\n" -------------------
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // --- Read and parse headers until the blank line -----------------------
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).await?;
        let header = header.trim().to_string();
        if header.is_empty() {
            break;
        }
        if header.to_lowercase().starts_with("content-length:") {
            content_length = header
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
        }
    }

    // --- Read body ---------------------------------------------------------
    let body = if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).await?;
        String::from_utf8_lossy(&buf).into_owned()
    } else {
        String::new()
    };

    // --- Route and obtain a JSON response body -----------------------------
    let response_body = route(&method, &path, &body).await;

    // --- Write HTTP/1.1 response -------------------------------------------
    let mut stream = reader.into_inner();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

async fn route(method: &str, path: &str, body: &str) -> String {
    match (method, path) {
        ("GET", "/health") => health_handler(),
        ("POST", "/query") => query_handler(body).await,
        ("POST", "/understand") => understand_handler(body).await,
        ("POST", "/causal/trace") => causal_trace_handler(body).await,
        _ => not_found_response(),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn health_handler() -> String {
    r#"{"status":"ok","version":"0.1.0"}"#.to_string()
}

async fn query_handler(body: &str) -> String {
    let query = extract_json_string_field(body, "query").unwrap_or_else(|| "SELECT 1".to_string());
    format!(
        r#"{{"columns":[],"rows":[],"command_tag":"OK","query":"{}"}}"#,
        query.replace('"', "\\\"")
    )
}

async fn understand_handler(body: &str) -> String {
    let intent = extract_json_string_field(body, "intent").unwrap_or_else(|| "unknown".to_string());
    format!(
        r#"{{"result_type":"confident","confidence":0.7,"plan_description":"Scan{{collection:\"{}\"}}" }}"#,
        intent.replace('"', "\\\"")
    )
}

async fn causal_trace_handler(body: &str) -> String {
    let from = extract_json_string_field(body, "from").unwrap_or_default();
    let to = extract_json_string_field(body, "to").unwrap_or_default();
    format!(
        r#"{{"paths":[],"path_count":0,"from":"{}","to":"{}"}}"#,
        from, to
    )
}

fn not_found_response() -> String {
    r#"{"error":"not found"}"#.to_string()
}

// ---------------------------------------------------------------------------
// JSON string-field extractor (no external deps)
// ---------------------------------------------------------------------------

/// Extract the string value of a JSON field by scanning for `"key":"value"`.
///
/// This is a deliberately simple, allocation-light parser intended for the
/// well-structured request bodies expected by this REST API.  It does **not**
/// handle escaped quotes inside values.
fn extract_json_string_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = json[start..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_handler_returns_ok() {
        let resp = health_handler();
        assert!(resp.contains("\"status\":\"ok\""));
        assert!(resp.contains("\"version\":\"0.1.0\""));
    }

    #[test]
    fn test_not_found_response_contains_error() {
        let resp = not_found_response();
        assert!(resp.contains("\"error\""));
    }

    #[test]
    fn test_extract_json_string_field_basic() {
        let json = r#"{"query":"SELECT * FROM users","limit":10}"#;
        assert_eq!(
            extract_json_string_field(json, "query").as_deref(),
            Some("SELECT * FROM users")
        );
    }

    #[test]
    fn test_extract_json_string_field_missing() {
        assert_eq!(extract_json_string_field("{}", "query"), None);
    }

    #[test]
    fn test_extract_json_string_field_second_key() {
        let json = r#"{"from":"node_a","to":"node_b","max_depth":5}"#;
        assert_eq!(
            extract_json_string_field(json, "from").as_deref(),
            Some("node_a")
        );
        assert_eq!(
            extract_json_string_field(json, "to").as_deref(),
            Some("node_b")
        );
    }

    #[test]
    fn test_extract_json_string_field_non_string_value() {
        // "limit" is a number, not a quoted string — should return None.
        let json = r#"{"limit":10}"#;
        assert_eq!(extract_json_string_field(json, "limit"), None);
    }

    #[tokio::test]
    async fn test_route_health() {
        let resp = route("GET", "/health", "").await;
        assert!(resp.contains("ok"));
        assert!(resp.contains("version"));
    }

    #[tokio::test]
    async fn test_route_query() {
        let body = r#"{"query":"SELECT 1"}"#;
        let resp = route("POST", "/query", body).await;
        assert!(resp.contains("command_tag"));
        assert!(resp.contains("SELECT 1"));
    }

    #[tokio::test]
    async fn test_route_query_default_when_missing() {
        let resp = route("POST", "/query", "{}").await;
        assert!(resp.contains("command_tag"));
        // Default query "SELECT 1" should appear.
        assert!(resp.contains("SELECT 1"));
    }

    #[tokio::test]
    async fn test_route_understand() {
        let body = r#"{"intent":"find documents","collection":"docs"}"#;
        let resp = route("POST", "/understand", body).await;
        assert!(resp.contains("result_type"));
        assert!(resp.contains("confidence"));
        assert!(resp.contains("plan_description"));
    }

    #[tokio::test]
    async fn test_route_understand_default_intent() {
        let resp = route("POST", "/understand", "{}").await;
        assert!(resp.contains("result_type"));
        assert!(resp.contains("unknown"));
    }

    #[tokio::test]
    async fn test_route_causal_trace() {
        let body = r#"{"from":"A","to":"B","max_depth":5}"#;
        let resp = route("POST", "/causal/trace", body).await;
        assert!(resp.contains("path_count"));
        assert!(resp.contains("\"from\":\"A\""));
        assert!(resp.contains("\"to\":\"B\""));
    }

    #[tokio::test]
    async fn test_route_causal_trace_empty_fields() {
        let resp = route("POST", "/causal/trace", "{}").await;
        assert!(resp.contains("path_count"));
        assert!(resp.contains("paths"));
    }

    #[tokio::test]
    async fn test_route_not_found() {
        let resp = route("GET", "/nonexistent", "").await;
        assert!(resp.contains("error"));
    }

    #[tokio::test]
    async fn test_route_post_unknown_path() {
        let resp = route("POST", "/unknown", "{}").await;
        assert!(resp.contains("error"));
    }

    #[tokio::test]
    async fn test_route_wrong_method_health() {
        // POST /health is not a registered route.
        let resp = route("POST", "/health", "").await;
        assert!(resp.contains("error"));
    }
}
