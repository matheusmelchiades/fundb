//! PostgreSQL extended query protocol implementation for FunDB.
//!
//! The extended query protocol splits query execution into discrete steps:
//! Parse → Bind → Describe → Execute → Sync.  This allows prepared statements
//! and parameter binding (preventing SQL-injection at the protocol level).
//!
//! Protocol reference:
//! <https://www.postgresql.org/docs/current/protocol-flow.html#PROTOCOL-FLOW-EXT-QUERY>

use std::collections::HashMap;

use bytes::{BufMut, BytesMut};

// ──────────────────────────────────────────────────────────────────────────────
// Domain types
// ──────────────────────────────────────────────────────────────────────────────

/// A named prepared statement.
///
/// Created by a `Parse` message.  The unnamed statement (`name == ""`) is
/// always overwritten on each new `Parse`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedStatement {
    /// Statement name; `""` means the unnamed statement.
    pub name: String,
    /// The SQL text as sent by the client.
    pub query: String,
    /// PostgreSQL type OIDs declared by the client for each parameter.
    /// Zero means "infer the type from context".
    pub param_types: Vec<i32>,
}

/// A bound portal ready for execution.
///
/// Created by a `Bind` message that pairs a prepared statement with concrete
/// parameter values.
#[derive(Debug, Clone, PartialEq)]
pub struct Portal {
    /// Portal name; `""` means the unnamed portal.
    pub name: String,
    /// Name of the prepared statement this portal was bound from.
    pub statement_name: String,
    /// Bound parameter values: `None` = SQL NULL, `Some(bytes)` = encoded value.
    pub params: Vec<Option<Vec<u8>>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Frontend message types (client → server)
// ──────────────────────────────────────────────────────────────────────────────

/// Extended query messages sent by the frontend (client).
#[derive(Debug, Clone, PartialEq)]
pub enum ExtendedFrontendMessage {
    /// `'P'` — parse a SQL statement and give it a name.
    Parse {
        name: String,
        query: String,
        param_types: Vec<i32>,
    },
    /// `'B'` — bind parameters to a prepared statement, creating a portal.
    Bind {
        portal: String,
        statement: String,
        params: Vec<Option<Vec<u8>>>,
    },
    /// `'D'` — describe a prepared statement or portal.
    Describe { kind: DescribeKind, name: String },
    /// `'E'` — execute a portal, optionally limiting the number of rows.
    Execute { portal: String, max_rows: i32 },
    /// `'S'` — sync: end of an extended query cycle, triggers `ReadyForQuery`.
    Sync,
    /// `'H'` — flush: send any buffered output to the client.
    Flush,
}

/// Discriminates what a `Describe` message targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescribeKind {
    /// Describe a prepared statement (`'S'`).
    Statement,
    /// Describe a portal (`'P'`).
    Portal,
}

// ──────────────────────────────────────────────────────────────────────────────
// Backend message types (server → client) — extended protocol additions
// ──────────────────────────────────────────────────────────────────────────────

/// Backend messages specific to the extended query protocol.
///
/// The simple-protocol messages (RowDescription, DataRow, CommandComplete,
/// ErrorResponse, ReadyForQuery) are reused from [`crate::pg_wire::BackendMessage`]
/// and are not repeated here.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtendedBackendMessage {
    /// `'1'` — parse completed successfully.
    ParseComplete,
    /// `'2'` — bind completed successfully.
    BindComplete,
    /// `'n'` — the statement/portal has no result columns.
    NoData,
    /// `'t'` — parameter type OIDs for a prepared statement.
    ParameterDescription { type_oids: Vec<i32> },
    /// `'s'` — a row-limited Execute suspended before completion.
    PortalSuspended,
}

impl ExtendedBackendMessage {
    /// Encode this message into `buf` in PostgreSQL wire format.
    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            // ParseComplete:  '1'  length=4
            ExtendedBackendMessage::ParseComplete => {
                buf.put_u8(b'1');
                buf.put_i32(4);
            }

            // BindComplete:  '2'  length=4
            ExtendedBackendMessage::BindComplete => {
                buf.put_u8(b'2');
                buf.put_i32(4);
            }

            // NoData:  'n'  length=4
            ExtendedBackendMessage::NoData => {
                buf.put_u8(b'n');
                buf.put_i32(4);
            }

            // ParameterDescription:  't'  length  n_params(i16)  [type_oid(i32) × n]
            ExtendedBackendMessage::ParameterDescription { type_oids } => {
                let n = type_oids.len() as i16;
                // length = 4 (length field) + 2 (n_params i16) + 4 * n (OIDs)
                let len = 4i32 + 2 + 4 * (type_oids.len() as i32);
                buf.put_u8(b't');
                buf.put_i32(len);
                buf.put_i16(n);
                for oid in type_oids {
                    buf.put_i32(*oid);
                }
            }

            // PortalSuspended:  's'  length=4
            ExtendedBackendMessage::PortalSuspended => {
                buf.put_u8(b's');
                buf.put_i32(4);
            }
        }
    }

    /// Convenience: encode into a fresh [`bytes::Bytes`].
    pub fn to_bytes(&self) -> bytes::Bytes {
        let mut buf = BytesMut::new();
        self.encode(&mut buf);
        buf.freeze()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Parsing helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Read a null-terminated C-string from the front of `src`, advancing `src`.
fn read_cstring(src: &mut &[u8]) -> Option<String> {
    let nul = src.iter().position(|&b| b == 0)?;
    let s = String::from_utf8_lossy(&src[..nul]).into_owned();
    *src = &src[nul + 1..];
    Some(s)
}

/// Read a big-endian `i16` from the front of `src`, advancing `src`.
fn read_i16(src: &mut &[u8]) -> Option<i16> {
    if src.len() < 2 {
        return None;
    }
    let v = i16::from_be_bytes([src[0], src[1]]);
    *src = &src[2..];
    Some(v)
}

/// Read a big-endian `i32` from the front of `src`, advancing `src`.
fn read_i32(src: &mut &[u8]) -> Option<i32> {
    if src.len() < 4 {
        return None;
    }
    let v = i32::from_be_bytes([src[0], src[1], src[2], src[3]]);
    *src = &src[4..];
    Some(v)
}

/// Parse one extended-protocol frontend message given its type byte and body.
///
/// `body` is the raw bytes following the 4-byte length field (i.e. everything
/// in the message except the type byte and the length itself).
///
/// Returns `Err` with a description if the body is malformed.
pub fn parse_extended_message(
    type_byte: u8,
    body: &[u8],
) -> Result<ExtendedFrontendMessage, String> {
    let mut src: &[u8] = body;

    match type_byte {
        // Parse ('P'):  name\0  query\0  n_params(i16)  [type_oid(i32) × n]
        b'P' => {
            let name = read_cstring(&mut src)
                .ok_or_else(|| "Parse: missing statement name".to_string())?;
            let query =
                read_cstring(&mut src).ok_or_else(|| "Parse: missing query string".to_string())?;
            let n_params =
                read_i16(&mut src).ok_or_else(|| "Parse: missing param_count".to_string())?;
            let mut param_types = Vec::with_capacity(n_params as usize);
            for i in 0..n_params {
                let oid = read_i32(&mut src)
                    .ok_or_else(|| format!("Parse: missing type OID for param {}", i))?;
                param_types.push(oid);
            }
            Ok(ExtendedFrontendMessage::Parse {
                name,
                query,
                param_types,
            })
        }

        // Bind ('B'):
        //   portal\0  statement\0
        //   param_format_count(i16)  [param_format(i16) × n]   -- format codes (ignored here)
        //   param_count(i16)         [param_len(i32) + param_data × n]
        //   result_format_count(i16) [result_format(i16) × n]  -- (ignored here)
        b'B' => {
            let portal = read_cstring(&mut src)
                .ok_or_else(|| "Bind: missing portal name".to_string())?;
            let statement = read_cstring(&mut src)
                .ok_or_else(|| "Bind: missing statement name".to_string())?;

            // Skip param format codes.
            let fmt_count =
                read_i16(&mut src).ok_or_else(|| "Bind: missing param_format_count".to_string())?;
            for i in 0..fmt_count {
                read_i16(&mut src)
                    .ok_or_else(|| format!("Bind: missing param format code {}", i))?;
            }

            // Read parameter values.
            let param_count =
                read_i16(&mut src).ok_or_else(|| "Bind: missing param_count".to_string())?;
            let mut params: Vec<Option<Vec<u8>>> = Vec::with_capacity(param_count as usize);
            for i in 0..param_count {
                let len = read_i32(&mut src)
                    .ok_or_else(|| format!("Bind: missing length for param {}", i))?;
                if len == -1 {
                    // SQL NULL
                    params.push(None);
                } else if len < 0 {
                    return Err(format!("Bind: invalid param length {} for param {}", len, i));
                } else {
                    let n = len as usize;
                    if src.len() < n {
                        return Err(format!(
                            "Bind: not enough bytes for param {}; need {}, have {}",
                            i,
                            n,
                            src.len()
                        ));
                    }
                    params.push(Some(src[..n].to_vec()));
                    src = &src[n..];
                }
            }

            // Skip result format codes (consume but do not store).
            if let Some(rfmt_count) = read_i16(&mut src) {
                for _ in 0..rfmt_count {
                    let _ = read_i16(&mut src);
                }
            }

            Ok(ExtendedFrontendMessage::Bind {
                portal,
                statement,
                params,
            })
        }

        // Describe ('D'):  kind(u8: 'S' or 'P')  name\0
        b'D' => {
            if src.is_empty() {
                return Err("Describe: empty body".to_string());
            }
            let kind_byte = src[0];
            src = &src[1..];
            let kind = match kind_byte {
                b'S' => DescribeKind::Statement,
                b'P' => DescribeKind::Portal,
                other => {
                    return Err(format!(
                        "Describe: unknown kind byte 0x{:02x}",
                        other
                    ))
                }
            };
            let name = read_cstring(&mut src)
                .ok_or_else(|| "Describe: missing name".to_string())?;
            Ok(ExtendedFrontendMessage::Describe { kind, name })
        }

        // Execute ('E'):  portal\0  max_rows(i32)
        b'E' => {
            let portal = read_cstring(&mut src)
                .ok_or_else(|| "Execute: missing portal name".to_string())?;
            let max_rows =
                read_i32(&mut src).ok_or_else(|| "Execute: missing max_rows".to_string())?;
            Ok(ExtendedFrontendMessage::Execute { portal, max_rows })
        }

        // Sync ('S'): no body
        b'S' => Ok(ExtendedFrontendMessage::Sync),

        // Flush ('H'): no body
        b'H' => Ok(ExtendedFrontendMessage::Flush),

        other => Err(format!(
            "unknown extended message type: 0x{:02x}",
            other
        )),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Session state
// ──────────────────────────────────────────────────────────────────────────────

/// Stateful extended-query session layer.
///
/// Holds all prepared statements and portals for a single connection.  One
/// instance should be created per accepted connection and mutated as messages
/// arrive.
pub struct ExtendedQuerySession {
    /// All prepared statements stored for this connection, keyed by name.
    pub statements: HashMap<String, PreparedStatement>,
    /// All portals stored for this connection, keyed by name.
    pub portals: HashMap<String, Portal>,
}

impl ExtendedQuerySession {
    /// Create a new, empty session.
    pub fn new() -> Self {
        Self {
            statements: HashMap::new(),
            portals: HashMap::new(),
        }
    }

    /// Handle a `Parse` message: store the prepared statement and return
    /// [`ExtendedBackendMessage::ParseComplete`] on success, or an empty vec if
    /// the message is not a `Parse`.
    pub fn handle_parse(&mut self, msg: ExtendedFrontendMessage) -> Vec<ExtendedBackendMessage> {
        if let ExtendedFrontendMessage::Parse {
            name,
            query,
            param_types,
        } = msg
        {
            self.statements.insert(
                name.clone(),
                PreparedStatement {
                    name,
                    query,
                    param_types,
                },
            );
            vec![ExtendedBackendMessage::ParseComplete]
        } else {
            vec![]
        }
    }

    /// Handle a `Bind` message: look up the named prepared statement, create a
    /// portal from it, and return [`ExtendedBackendMessage::BindComplete`].
    ///
    /// Returns an empty vec if the message is not a `Bind` or the referenced
    /// statement does not exist.
    pub fn handle_bind(&mut self, msg: ExtendedFrontendMessage) -> Vec<ExtendedBackendMessage> {
        if let ExtendedFrontendMessage::Bind {
            portal,
            statement,
            params,
        } = msg
        {
            if self.statements.contains_key(&statement) {
                self.portals.insert(
                    portal.clone(),
                    Portal {
                        name: portal,
                        statement_name: statement,
                        params,
                    },
                );
                vec![ExtendedBackendMessage::BindComplete]
            } else {
                // Statement not found — callers should send an ErrorResponse.
                vec![]
            }
        } else {
            vec![]
        }
    }

    /// Handle a `Describe` message.
    ///
    /// - `DescribeKind::Statement` → returns [`ExtendedBackendMessage::ParameterDescription`]
    ///   with the declared OIDs from the prepared statement, followed by
    ///   [`ExtendedBackendMessage::NoData`] (we do not know the row shape yet).
    /// - `DescribeKind::Portal` → returns [`ExtendedBackendMessage::NoData`] as a
    ///   placeholder (row description would require execution).
    pub fn handle_describe(&mut self, msg: ExtendedFrontendMessage) -> Vec<ExtendedBackendMessage> {
        if let ExtendedFrontendMessage::Describe { kind, name } = msg {
            match kind {
                DescribeKind::Statement => {
                    if let Some(stmt) = self.statements.get(&name) {
                        vec![
                            ExtendedBackendMessage::ParameterDescription {
                                type_oids: stmt.param_types.clone(),
                            },
                            ExtendedBackendMessage::NoData,
                        ]
                    } else {
                        vec![ExtendedBackendMessage::NoData]
                    }
                }
                DescribeKind::Portal => vec![ExtendedBackendMessage::NoData],
            }
        } else {
            vec![]
        }
    }

    /// Handle an `Execute` message.
    ///
    /// Looks up the named portal and returns the SQL string that should be
    /// executed by the query engine, together with any pre-execution response
    /// messages (currently none).
    ///
    /// Returns `(None, vec![])` if the portal does not exist or the message is
    /// not an `Execute`.
    pub fn handle_execute(
        &self,
        msg: &ExtendedFrontendMessage,
    ) -> (Option<String>, Vec<ExtendedBackendMessage>) {
        if let ExtendedFrontendMessage::Execute { portal, .. } = msg {
            if let Some(p) = self.portals.get(portal.as_str()) {
                if let Some(stmt) = self.statements.get(&p.statement_name) {
                    return (Some(stmt.query.clone()), vec![]);
                }
            }
            (None, vec![])
        } else {
            (None, vec![])
        }
    }
}

impl Default for ExtendedQuerySession {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Build a raw Parse ('P') message body for testing.
    fn build_parse_body(name: &str, query: &str, param_types: &[i32]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(query.as_bytes());
        body.push(0);
        let n = param_types.len() as i16;
        body.extend_from_slice(&n.to_be_bytes());
        for &oid in param_types {
            body.extend_from_slice(&oid.to_be_bytes());
        }
        body
    }

    /// Build a raw Bind ('B') message body with zero params.
    fn build_bind_body_no_params(portal: &str, statement: &str) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(portal.as_bytes());
        body.push(0);
        body.extend_from_slice(statement.as_bytes());
        body.push(0);
        // param_format_count = 0
        body.extend_from_slice(&0i16.to_be_bytes());
        // param_count = 0
        body.extend_from_slice(&0i16.to_be_bytes());
        // result_format_count = 0
        body.extend_from_slice(&0i16.to_be_bytes());
        body
    }

    // ── test 1: parse a 'P' message ──────────────────────────────────────────

    #[test]
    fn test_parse_parse_message() {
        let body = build_parse_body("my_stmt", "SELECT $1", &[23]);
        let msg = parse_extended_message(b'P', &body).expect("should parse");
        match msg {
            ExtendedFrontendMessage::Parse {
                name,
                query,
                param_types,
            } => {
                assert_eq!(name, "my_stmt");
                assert_eq!(query, "SELECT $1");
                assert_eq!(param_types, vec![23]);
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // ── test 2: parse a 'B' message with zero params ─────────────────────────

    #[test]
    fn test_parse_bind_message() {
        let body = build_bind_body_no_params("my_portal", "my_stmt");
        let msg = parse_extended_message(b'B', &body).expect("should parse");
        match msg {
            ExtendedFrontendMessage::Bind {
                portal,
                statement,
                params,
            } => {
                assert_eq!(portal, "my_portal");
                assert_eq!(statement, "my_stmt");
                assert!(params.is_empty());
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // ── test 3: parse a 'S' (Sync) message ───────────────────────────────────

    #[test]
    fn test_parse_sync_message() {
        let msg = parse_extended_message(b'S', &[]).expect("should parse");
        assert_eq!(msg, ExtendedFrontendMessage::Sync);
    }

    // ── test 4: full extended session cycle ──────────────────────────────────

    #[test]
    fn test_extended_session_parse_bind_execute() {
        let mut session = ExtendedQuerySession::new();

        // Parse step
        let parse_body = build_parse_body("", "SELECT $1", &[23]);
        let parse_msg = parse_extended_message(b'P', &parse_body).unwrap();
        let parse_responses = session.handle_parse(parse_msg);
        assert_eq!(parse_responses, vec![ExtendedBackendMessage::ParseComplete]);

        // Bind step — supply one text parameter "1"
        let mut bind_body = Vec::new();
        bind_body.extend_from_slice(b"\0"); // unnamed portal
        bind_body.extend_from_slice(b"\0"); // unnamed statement
        bind_body.extend_from_slice(&0i16.to_be_bytes()); // param_format_count = 0
        bind_body.extend_from_slice(&1i16.to_be_bytes()); // param_count = 1
        let param_bytes = b"1";
        bind_body.extend_from_slice(&(param_bytes.len() as i32).to_be_bytes());
        bind_body.extend_from_slice(param_bytes);
        bind_body.extend_from_slice(&0i16.to_be_bytes()); // result_format_count = 0

        let bind_msg = parse_extended_message(b'B', &bind_body).unwrap();
        let bind_responses = session.handle_bind(bind_msg);
        assert_eq!(bind_responses, vec![ExtendedBackendMessage::BindComplete]);

        // Execute step
        let mut exec_body = Vec::new();
        exec_body.extend_from_slice(b"\0"); // unnamed portal
        exec_body.extend_from_slice(&0i32.to_be_bytes()); // max_rows = 0 (all rows)

        let exec_msg = parse_extended_message(b'E', &exec_body).unwrap();
        let (query, pre_msgs) = session.handle_execute(&exec_msg);

        assert_eq!(query.as_deref(), Some("SELECT $1"));
        assert!(pre_msgs.is_empty());
    }

    // ── test 5: backend message encoding ─────────────────────────────────────

    #[test]
    fn test_extended_backend_message_encode() {
        // ParseComplete must encode as [b'1', 0, 0, 0, 4].
        let mut buf = BytesMut::new();
        ExtendedBackendMessage::ParseComplete.encode(&mut buf);
        let bytes = buf.freeze();
        assert_eq!(&bytes[..], &[b'1', 0, 0, 0, 4]);

        // BindComplete: [b'2', 0, 0, 0, 4]
        let mut buf = BytesMut::new();
        ExtendedBackendMessage::BindComplete.encode(&mut buf);
        assert_eq!(&buf[..], &[b'2', 0, 0, 0, 4]);

        // NoData: [b'n', 0, 0, 0, 4]
        let mut buf = BytesMut::new();
        ExtendedBackendMessage::NoData.encode(&mut buf);
        assert_eq!(&buf[..], &[b'n', 0, 0, 0, 4]);

        // PortalSuspended: [b's', 0, 0, 0, 4]
        let mut buf = BytesMut::new();
        ExtendedBackendMessage::PortalSuspended.encode(&mut buf);
        assert_eq!(&buf[..], &[b's', 0, 0, 0, 4]);

        // ParameterDescription with two OIDs (23, 25):
        //   't'  length=4+2+8=14  n=2  23  25
        let mut buf = BytesMut::new();
        ExtendedBackendMessage::ParameterDescription {
            type_oids: vec![23, 25],
        }
        .encode(&mut buf);
        assert_eq!(buf[0], b't');
        // length field = 14
        let len = i32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
        assert_eq!(len, 14);
        // n_params = 2
        let n = i16::from_be_bytes([buf[5], buf[6]]);
        assert_eq!(n, 2);
    }
}
