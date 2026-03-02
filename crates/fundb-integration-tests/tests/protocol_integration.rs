use async_trait::async_trait;
use fundb_protocol::pg_wire::{
    parse_startup_params, BackendMessage, ConnContext, FieldDescription, QueryError, QueryHandler,
    QueryResult,
};

// ---------------------------------------------------------------------------
// StubHandler — a simple test implementation of QueryHandler
// ---------------------------------------------------------------------------
struct StubHandler;

#[async_trait]
impl QueryHandler for StubHandler {
    async fn execute(&self, query: &str, _ctx: &ConnContext) -> Result<QueryResult, QueryError> {
        let trimmed = query.trim().to_uppercase();

        if trimmed == "SELECT 1" {
            Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "?column?".to_string(),
                    type_oid: 23, // int4
                }],
                rows: vec![vec![Some("1".to_string())]],
                command_tag: "SELECT 1".to_string(),
            })
        } else if trimmed.starts_with("SELECT VERSION") {
            Ok(QueryResult {
                columns: vec![FieldDescription {
                    name: "version".to_string(),
                    type_oid: 25, // text
                }],
                rows: vec![vec![Some("FunDB 0.1.0".to_string())]],
                command_tag: "SELECT 1".to_string(),
            })
        } else {
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                command_tag: "SELECT 0".to_string(),
            })
        }
    }
}

fn test_ctx() -> ConnContext {
    ConnContext {
        user: "test_user".to_string(),
        database: "fundb".to_string(),
        tenant_id: 1,
        agent_id: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Startup → Auth → Ready sequence
// ---------------------------------------------------------------------------
#[test]
fn test_startup_auth_ready_sequence() {
    // Encode startup params
    let body = make_startup_body(&[("user", "fun"), ("database", "fundb")]);
    let params = parse_startup_params(&body);
    assert_eq!(params.get("user").map(String::as_str), Some("fun"));
    assert_eq!(params.get("database").map(String::as_str), Some("fundb"));

    // Server would respond with AuthOk + KeyData + ReadyForQuery
    let auth_ok = BackendMessage::AuthenticationOk.to_bytes();
    assert!(!auth_ok.is_empty(), "AuthenticationOk should produce bytes");

    let key_data = BackendMessage::BackendKeyData {
        pid: 1234,
        secret_key: 5678,
    }
    .to_bytes();
    assert!(!key_data.is_empty(), "BackendKeyData should produce bytes");

    let ready = BackendMessage::ReadyForQuery { status: b'I' }.to_bytes();
    assert!(!ready.is_empty(), "ReadyForQuery should produce bytes");

    // AuthOk starts with 'R'
    assert_eq!(auth_ok[0], b'R');
    // KeyData starts with 'K'
    assert_eq!(key_data[0], b'K');
    // ReadyForQuery starts with 'Z'
    assert_eq!(ready[0], b'Z');
}

// ---------------------------------------------------------------------------
// 2. StubHandler — SELECT 1
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_stub_handler_select_one() {
    let handler = StubHandler;
    let ctx = test_ctx();

    let result = handler
        .execute("SELECT 1", &ctx)
        .await
        .ok()
        .expect("query execution failed");
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], Some("1".to_string()));
}

// ---------------------------------------------------------------------------
// 3. StubHandler — VERSION
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_stub_handler_version() {
    let handler = StubHandler;
    let ctx = test_ctx();

    let result = handler
        .execute("SELECT VERSION()", &ctx)
        .await
        .ok()
        .expect("query execution failed");
    assert_eq!(result.rows.len(), 1);
    assert!(
        result.rows[0][0].as_ref().unwrap().contains("FunDB"),
        "version should contain 'FunDB'"
    );
}

// ---------------------------------------------------------------------------
// 4. StubHandler — unknown query returns empty
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_stub_handler_unknown_returns_empty() {
    let handler = StubHandler;
    let ctx = test_ctx();

    let result = handler
        .execute("EXPLAIN ANALYZE SELECT * FROM magic", &ctx)
        .await
        .ok()
        .expect("query execution failed");
    assert!(
        result.rows.is_empty(),
        "unknown query should return empty result set"
    );
}

// ---------------------------------------------------------------------------
// 5. ErrorResponse fields
// ---------------------------------------------------------------------------
#[test]
fn test_error_response_fields() {
    let err = BackendMessage::ErrorResponse {
        severity: "ERROR".to_string(),
        code: "42601".to_string(),
        message: "syntax error at or near \"GIBBERISH\"".to_string(),
    };

    let bytes = err.to_bytes();
    assert!(!bytes.is_empty(), "ErrorResponse should produce bytes");
    assert_eq!(bytes[0], b'E', "ErrorResponse should start with 'E'");
}

// ---------------------------------------------------------------------------
// 6. RowDescription — multiple columns
// ---------------------------------------------------------------------------
#[test]
fn test_row_description_multiple_columns() {
    let fields: Vec<FieldDescription> = (0..5)
        .map(|i| FieldDescription {
            name: format!("col_{}", i),
            type_oid: 25, // text
        })
        .collect();

    let msg = BackendMessage::RowDescription { fields };
    let bytes = msg.to_bytes();
    assert!(!bytes.is_empty());
    assert_eq!(bytes[0], b'T', "RowDescription should start with 'T'");

    // Parse the field count from the body (after type byte + length)
    let field_count = i16::from_be_bytes([bytes[5], bytes[6]]);
    assert_eq!(field_count, 5, "should encode 5 columns");
}

// ---------------------------------------------------------------------------
// 7. DataRow — mixed null values
// ---------------------------------------------------------------------------
#[test]
fn test_data_row_mixed_null_values() {
    let row = BackendMessage::DataRow {
        values: vec![
            Some("hello".to_string()),
            None,
            Some("world".to_string()),
            None,
        ],
    };

    let bytes = row.to_bytes();
    assert!(!bytes.is_empty());
    assert_eq!(bytes[0], b'D', "DataRow should start with 'D'");

    // Field count should be 4
    let field_count = i16::from_be_bytes([bytes[5], bytes[6]]);
    assert_eq!(field_count, 4, "should encode 4 fields");
}

// ---------------------------------------------------------------------------
// 8. Full session sequence
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_full_session_sequence() {
    let handler = StubHandler;
    let ctx = test_ctx();

    // 1. Startup params
    let body = make_startup_body(&[("user", "fun"), ("database", "fundb")]);
    let params = parse_startup_params(&body);
    assert_eq!(params.get("user").map(String::as_str), Some("fun"));

    // 2. Auth OK
    let auth = BackendMessage::AuthenticationOk.to_bytes();
    assert_eq!(auth[0], b'R');

    // 3. Ready for query
    let ready = BackendMessage::ReadyForQuery { status: b'I' }.to_bytes();
    assert_eq!(ready[0], b'Z');

    // 4. Query → result
    let result = handler
        .execute("SELECT 1", &ctx)
        .await
        .ok()
        .expect("query execution failed");

    // 5. Encode RowDescription
    let row_desc = BackendMessage::RowDescription {
        fields: result.columns,
    }
    .to_bytes();
    assert_eq!(row_desc[0], b'T');

    // 6. Encode DataRows
    for row in &result.rows {
        let data_row = BackendMessage::DataRow {
            values: row.clone(),
        }
        .to_bytes();
        assert_eq!(data_row[0], b'D');
    }

    // 7. CommandComplete
    let complete = BackendMessage::CommandComplete {
        tag: result.command_tag,
    }
    .to_bytes();
    assert_eq!(complete[0], b'C');

    // 8. Ready for next query
    let ready2 = BackendMessage::ReadyForQuery { status: b'I' }.to_bytes();
    assert_eq!(ready2[0], b'Z');
}

// ---------------------------------------------------------------------------
// Helper: build startup message body
// ---------------------------------------------------------------------------
fn make_startup_body(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (k, v) in pairs {
        body.extend_from_slice(k.as_bytes());
        body.push(0);
        body.extend_from_slice(v.as_bytes());
        body.push(0);
    }
    body.push(0);
    body
}
