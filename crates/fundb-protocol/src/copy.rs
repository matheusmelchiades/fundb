//! PostgreSQL COPY FROM STDIN bulk-loading protocol for FunDB.
//!
//! COPY FROM STDIN allows a client to stream rows directly into a table without
//! going through individual INSERT statements.  This module implements:
//!
//! - Server-side response messages (`CopyInResponse`, `CopyDone`, `CopyFail`).
//! - Client-side message parsing (`CopyData`, `CopyDone`, `CopyFail`).
//! - Text-format row parsing (tab-delimited, `\N` = NULL).
//!
//! ## Protocol flow
//!
//! ```text
//! Server  ──► CopyInResponse        (server is ready to receive data)
//! Client  ──► CopyData × N          (row data in batches)
//! Client  ──► CopyDone              (all data sent)
//! Server  ──► CommandComplete       (e.g. "COPY 42")
//! Server  ──► ReadyForQuery
//! ```
//!
//! If the client encounters an error it sends `CopyFail` instead of `CopyDone`,
//! and the server responds with `ErrorResponse`.
//!
//! Protocol reference:
//! <https://www.postgresql.org/docs/current/protocol-flow.html#PROTOCOL-COPY>

use bytes::{BufMut, BytesMut};

// ──────────────────────────────────────────────────────────────────────────────
// Server → Client messages
// ──────────────────────────────────────────────────────────────────────────────

/// Encode a `CopyInResponse` message signalling that the server is ready to
/// receive COPY data.
///
/// We always use text format (overall format = 0) with all columns in text
/// format (per-column format code = 0).
///
/// Wire format:
/// ```text
/// 'G'  i32(length)  i8(overall_format)  i16(n_cols)  [i16(format_code) × n_cols]
/// ```
pub fn encode_copy_in_response(buf: &mut BytesMut, column_count: i16) {
    // Body: 1 (overall_format) + 2 (n_cols) + 2 * column_count (format codes)
    let body_len: i32 = 1 + 2 + 2 * column_count as i32;
    // length field includes itself (4 bytes) plus body
    let len: i32 = 4 + body_len;

    buf.put_u8(b'G');
    buf.put_i32(len);
    buf.put_i8(0); // overall format: 0 = text
    buf.put_i16(column_count);
    for _ in 0..column_count {
        buf.put_i16(0); // per-column format code: 0 = text
    }
}

/// Encode a `CopyDone` message from the server indicating the copy is complete.
///
/// Wire format:
/// ```text
/// 'c'  i32(4)
/// ```
pub fn encode_copy_done(buf: &mut BytesMut) {
    buf.put_u8(b'c');
    buf.put_i32(4);
}

/// Encode a `CopyFail` message from the server rejecting a copy operation.
///
/// Wire format:
/// ```text
/// 'f'  i32(length)  message\0
/// ```
pub fn encode_copy_fail(buf: &mut BytesMut, message: &str) {
    // length = 4 (length field) + message bytes + 1 (null terminator)
    let len: i32 = 4 + message.len() as i32 + 1;
    buf.put_u8(b'f');
    buf.put_i32(len);
    buf.put_slice(message.as_bytes());
    buf.put_u8(0); // null terminator
}

// ──────────────────────────────────────────────────────────────────────────────
// Text-format row parsing
// ──────────────────────────────────────────────────────────────────────────────

/// Parse a single COPY text-format row.
///
/// PostgreSQL text format uses:
/// - Tab (`\t`) as the column delimiter.
/// - `\N` (backslash-N) to represent SQL NULL.
/// - `\\` for a literal backslash.
/// - `\n` for a literal newline within a field value.
/// - `\t` for a literal tab within a field value.
///
/// Returns a vector of `Option<String>` — one per column.  `None` means NULL.
pub fn parse_copy_text_row(line: &str) -> Vec<Option<String>> {
    line.split('\t')
        .map(|field| {
            if field == "\\N" {
                None
            } else {
                Some(unescape_copy_text(field))
            }
        })
        .collect()
}

/// Unescape a single field value from PostgreSQL COPY text format.
fn unescape_copy_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => {
                    // Unknown escape: preserve both backslash and the character.
                    out.push('\\');
                    out.push(other);
                }
                None => {
                    // Trailing backslash: preserve as-is.
                    out.push('\\');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────────
// Client → Server messages
// ──────────────────────────────────────────────────────────────────────────────

/// Messages sent by the client during a COPY FROM STDIN operation.
#[derive(Debug, Clone, PartialEq)]
pub enum CopyClientMessage {
    /// `'d'` — a chunk of COPY data (may span multiple rows).
    CopyData(Vec<u8>),
    /// `'c'` — client signals that all data has been sent successfully.
    CopyDone,
    /// `'f'` — client signals an error, aborting the copy.
    CopyFail(String),
}

/// Parse a single COPY client message given its type byte and body.
///
/// Returns `None` for any type byte that is not part of the COPY sub-protocol.
pub fn parse_copy_client_message(type_byte: u8, body: &[u8]) -> Option<CopyClientMessage> {
    match type_byte {
        // CopyData: raw bytes (may be partial rows)
        b'd' => Some(CopyClientMessage::CopyData(body.to_vec())),

        // CopyDone: no body
        b'c' => Some(CopyClientMessage::CopyDone),

        // CopyFail: null-terminated error message
        b'f' => {
            let msg = String::from_utf8_lossy(body)
                .trim_end_matches('\0')
                .to_string();
            Some(CopyClientMessage::CopyFail(msg))
        }

        _ => None,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helper: read big-endian i32 at offset ────────────────────────────────

    fn read_i32_at(buf: &[u8], offset: usize) -> i32 {
        i32::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    }

    // ── test 1: CopyInResponse has 'G' at position 0 ─────────────────────────

    #[test]
    fn test_encode_copy_in_response() {
        let mut buf = BytesMut::new();
        encode_copy_in_response(&mut buf, 3);

        // Type byte
        assert_eq!(buf[0], b'G', "expected type byte 'G'");

        // length = 4 + 1 + 2 + 2*3 = 13
        let len = read_i32_at(&buf, 1);
        assert_eq!(len, 13, "unexpected length");

        // overall format = 0 (text)
        assert_eq!(buf[5], 0, "expected overall format = 0");

        // n_cols = 3
        let n_cols = i16::from_be_bytes([buf[6], buf[7]]);
        assert_eq!(n_cols, 3, "expected n_cols = 3");

        // All column format codes should be 0
        for i in 0..3usize {
            let fmt = i16::from_be_bytes([buf[8 + i * 2], buf[9 + i * 2]]);
            assert_eq!(fmt, 0, "expected column format code 0 at index {}", i);
        }
    }

    // ── test 2: CopyDone produces 'c' + 4 ────────────────────────────────────

    #[test]
    fn test_encode_copy_done() {
        let mut buf = BytesMut::new();
        encode_copy_done(&mut buf);

        assert_eq!(&buf[..], &[b'c', 0, 0, 0, 4], "CopyDone encoding mismatch");
    }

    // ── test 3: parse_copy_text_row with no NULLs ────────────────────────────

    #[test]
    fn test_parse_copy_text_row_no_nulls() {
        let row = parse_copy_text_row("alice\t30\tNY");
        assert_eq!(
            row,
            vec![
                Some("alice".to_string()),
                Some("30".to_string()),
                Some("NY".to_string()),
            ]
        );
    }

    // ── test 4: parse_copy_text_row with NULL field ───────────────────────────

    #[test]
    fn test_parse_copy_text_row_with_null() {
        let row = parse_copy_text_row("alice\t\\N\tNY");
        assert_eq!(
            row,
            vec![
                Some("alice".to_string()),
                None,
                Some("NY".to_string()),
            ]
        );
    }

    // ── test 5: parse_copy_client_message CopyData ───────────────────────────

    #[test]
    fn test_parse_copy_client_message_data() {
        let data = b"alice\t30\tNY\n".as_slice();
        let msg = parse_copy_client_message(b'd', data);
        assert_eq!(
            msg,
            Some(CopyClientMessage::CopyData(data.to_vec())),
            "expected CopyData"
        );
    }

    // ── test 6: parse_copy_client_message CopyDone ───────────────────────────

    #[test]
    fn test_parse_copy_client_message_done() {
        let msg = parse_copy_client_message(b'c', &[]);
        assert_eq!(msg, Some(CopyClientMessage::CopyDone), "expected CopyDone");
    }

    // ── extra: parse_copy_text_row escaping ──────────────────────────────────

    #[test]
    fn test_parse_copy_text_row_escapes() {
        // \\ should become \, \n should become newline, \t should become tab
        let row = parse_copy_text_row("a\\\\b\tc\\nd\te\\tf");
        assert_eq!(row[0], Some("a\\b".to_string()));
        assert_eq!(row[1], Some("c\nd".to_string()));
        assert_eq!(row[2], Some("e\tf".to_string()));
    }

    // ── extra: encode_copy_fail encodes message correctly ────────────────────

    #[test]
    fn test_encode_copy_fail() {
        let mut buf = BytesMut::new();
        encode_copy_fail(&mut buf, "client error");

        assert_eq!(buf[0], b'f', "expected type byte 'f'");

        // length = 4 + len("client error") + 1 = 17
        let len = read_i32_at(&buf, 1);
        assert_eq!(len, 17, "unexpected length");

        // message bytes
        let msg = std::str::from_utf8(&buf[5..5 + 12]).expect("UTF-8");
        assert_eq!(msg, "client error");

        // null terminator
        assert_eq!(buf[17], 0, "expected null terminator");
    }
}
