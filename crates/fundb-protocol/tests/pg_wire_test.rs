//! Integration-style tests for the PostgreSQL wire protocol encoding / decoding.

use bytes::{Bytes, BytesMut};
use fundb_protocol::pg_wire::{parse_startup_params, BackendMessage, FieldDescription};

// ──────────────────────────────────────────────────────────────────────────────
// Startup message parsing
// ──────────────────────────────────────────────────────────────────────────────

/// Build a raw startup-message body (everything after the 4-byte length and
/// 4-byte protocol version) for the given key-value pairs.
fn make_startup_body(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (k, v) in pairs {
        body.extend_from_slice(k.as_bytes());
        body.push(0);
        body.extend_from_slice(v.as_bytes());
        body.push(0);
    }
    body.push(0); // terminator
    body
}

#[test]
fn test_startup_message_parse_basic() {
    let body = make_startup_body(&[("user", "fun"), ("database", "fundb")]);
    let params = parse_startup_params(&body);
    assert_eq!(params.get("user").map(String::as_str), Some("fun"));
    assert_eq!(params.get("database").map(String::as_str), Some("fundb"));
}

#[test]
fn test_startup_message_parse_extra_params() {
    let body = make_startup_body(&[
        ("user", "alice"),
        ("database", "testdb"),
        ("application_name", "psql"),
        ("client_encoding", "UTF8"),
    ]);
    let params = parse_startup_params(&body);
    assert_eq!(params.get("user").map(String::as_str), Some("alice"));
    assert_eq!(params.get("database").map(String::as_str), Some("testdb"));
    assert_eq!(
        params.get("application_name").map(String::as_str),
        Some("psql")
    );
    assert_eq!(
        params.get("client_encoding").map(String::as_str),
        Some("UTF8")
    );
}

#[test]
fn test_startup_message_parse_empty() {
    // Just the terminator byte.
    let params = parse_startup_params(&[0u8]);
    assert!(params.is_empty());
}

// ──────────────────────────────────────────────────────────────────────────────
// Backend message encoding
// ──────────────────────────────────────────────────────────────────────────────

fn encode(msg: BackendMessage) -> Bytes {
    msg.to_bytes()
}

#[test]
fn test_authentication_ok_encode() {
    let b = encode(BackendMessage::AuthenticationOk);
    assert_eq!(&b[..], &[b'R', 0, 0, 0, 8, 0, 0, 0, 0]);
}

#[test]
fn test_ready_for_query_idle_encode() {
    let b = encode(BackendMessage::ReadyForQuery { status: b'I' });
    // 'Z'  length=5  'I'
    assert_eq!(&b[..], &[b'Z', 0, 0, 0, 5, b'I']);
}

#[test]
fn test_ready_for_query_length() {
    let b = encode(BackendMessage::ReadyForQuery { status: b'I' });
    assert_eq!(b.len(), 6); // 1 type + 4 length + 1 status
}

#[test]
fn test_empty_query_response_encode() {
    let b = encode(BackendMessage::EmptyQueryResponse);
    // 'I'  length=4
    assert_eq!(&b[..], &[b'I', 0, 0, 0, 4]);
}

#[test]
fn test_command_complete_encode() {
    let b = encode(BackendMessage::CommandComplete {
        tag: "SELECT 1".into(),
    });
    // 'C'  length = 4 + 8 + 1 = 13  "SELECT 1\0"
    assert_eq!(b[0], b'C');
    let length = i32::from_be_bytes(b[1..5].try_into().unwrap());
    assert_eq!(length, 4 + 8 + 1); // 4 (length field) + "SELECT 1" + NUL
    assert_eq!(&b[5..13], b"SELECT 1");
    assert_eq!(b[13], 0);
}

#[test]
fn test_error_response_encode() {
    let b = encode(BackendMessage::ErrorResponse {
        severity: "ERROR".into(),
        code: "42601".into(),
        message: "syntax error".into(),
    });
    assert_eq!(b[0], b'E');
    // Body should contain severity field ('S'), code field ('C'), message ('M').
    let body = &b[5..]; // skip type + length
    assert!(body.contains(&b'S'));
    assert!(body.contains(&b'C'));
    assert!(body.contains(&b'M'));
    // Last byte should be the extra NUL terminator.
    assert_eq!(*body.last().unwrap(), 0);
}

#[test]
fn test_backend_key_data_encode() {
    let b = encode(BackendMessage::BackendKeyData {
        pid: 42,
        secret_key: 99,
    });
    // 'K'  length=12  pid(i32)  secret(i32)
    assert_eq!(b[0], b'K');
    let length = i32::from_be_bytes(b[1..5].try_into().unwrap());
    assert_eq!(length, 12);
    let pid = i32::from_be_bytes(b[5..9].try_into().unwrap());
    assert_eq!(pid, 42);
    let secret = i32::from_be_bytes(b[9..13].try_into().unwrap());
    assert_eq!(secret, 99);
}

#[test]
fn test_row_description_encode() {
    let b = encode(BackendMessage::RowDescription {
        fields: vec![FieldDescription {
            name: "?column?".into(),
            type_oid: 23,
        }],
    });
    assert_eq!(b[0], b'T');
    // After type + length: field count (2 bytes) = 1
    let field_count = i16::from_be_bytes(b[5..7].try_into().unwrap());
    assert_eq!(field_count, 1);
    // Field name starts at byte 7 and ends at the NUL.
    let name_end = b[7..].iter().position(|&x| x == 0).unwrap();
    let name = std::str::from_utf8(&b[7..7 + name_end]).unwrap();
    assert_eq!(name, "?column?");
}

#[test]
fn test_data_row_encode_with_value() {
    let b = encode(BackendMessage::DataRow {
        values: vec![Some("1".into())],
    });
    assert_eq!(b[0], b'D');
    // Field count = 1
    let field_count = i16::from_be_bytes(b[5..7].try_into().unwrap());
    assert_eq!(field_count, 1);
    // Value length = 1
    let val_len = i32::from_be_bytes(b[7..11].try_into().unwrap());
    assert_eq!(val_len, 1);
    assert_eq!(b[11], b'1');
}

#[test]
fn test_data_row_encode_null_value() {
    let b = encode(BackendMessage::DataRow { values: vec![None] });
    assert_eq!(b[0], b'D');
    let field_count = i16::from_be_bytes(b[5..7].try_into().unwrap());
    assert_eq!(field_count, 1);
    // NULL is represented by -1
    let val_len = i32::from_be_bytes(b[7..11].try_into().unwrap());
    assert_eq!(val_len, -1);
}

#[test]
fn test_data_row_encode_multiple_values() {
    let b = encode(BackendMessage::DataRow {
        values: vec![Some("hello".into()), None, Some("world".into())],
    });
    assert_eq!(b[0], b'D');
    let field_count = i16::from_be_bytes(b[5..7].try_into().unwrap());
    assert_eq!(field_count, 3);
}

// ──────────────────────────────────────────────────────────────────────────────
// Multi-message sequences
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_select_one_sequence_encodes() {
    // Simulate what the server sends for "SELECT 1".
    let mut buf = BytesMut::new();
    BackendMessage::AuthenticationOk.encode(&mut buf);
    BackendMessage::BackendKeyData {
        pid: 1,
        secret_key: 0,
    }
    .encode(&mut buf);
    BackendMessage::ReadyForQuery { status: b'I' }.encode(&mut buf);
    BackendMessage::RowDescription {
        fields: vec![FieldDescription {
            name: "?column?".into(),
            type_oid: 23,
        }],
    }
    .encode(&mut buf);
    BackendMessage::DataRow {
        values: vec![Some("1".into())],
    }
    .encode(&mut buf);
    BackendMessage::CommandComplete {
        tag: "SELECT 1".into(),
    }
    .encode(&mut buf);
    BackendMessage::ReadyForQuery { status: b'I' }.encode(&mut buf);

    // Validate the sequence starts with the correct message type bytes.
    let bytes = buf.freeze();
    assert_eq!(bytes[0], b'R'); // AuthenticationOk
                                // Scan for 'Z' (ReadyForQuery) after the first ReadyForQuery.
    assert!(bytes.contains(&b'Z'));
}
