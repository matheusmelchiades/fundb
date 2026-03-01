//! PostgreSQL wire protocol implementation for FunDB.
//!
//! Implements the frontend/backend message framing so that `psql` and
//! PostgreSQL-compatible clients can connect and execute queries.
//!
//! Protocol reference:
//! <https://www.postgresql.org/docs/current/protocol.html>

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, warn};

// ──────────────────────────────────────────────────────────────────────────────
// Public data types
// ──────────────────────────────────────────────────────────────────────────────

/// Description of a single result-set column.
#[derive(Debug, Clone)]
pub struct FieldDescription {
    /// Column name as shown by the client.
    pub name: String,
    /// PostgreSQL type OID.  23 = int4, 25 = text, 701 = float8.
    pub type_oid: i32,
}

/// Successful result returned by a [`QueryHandler`].
pub struct QueryResult {
    pub columns: Vec<FieldDescription>,
    pub rows: Vec<Vec<Option<String>>>,
    /// Human-readable command tag, e.g. "SELECT 1" or "INSERT 0 1".
    pub command_tag: String,
}

/// Error returned by a [`QueryHandler`].
pub struct QueryError {
    /// PostgreSQL error code (SQLSTATE), e.g. "42601" for syntax error.
    pub code: String,
    pub message: String,
}

/// Per-connection context forwarded to the query handler.
pub struct ConnContext {
    pub user: String,
    pub database: String,
    pub tenant_id: u32,
    pub agent_id: Option<String>,
}

/// Trait implemented by the engine to handle incoming SQL queries.
///
/// Marked with `#[async_trait]` so it can be stored as `Arc<dyn QueryHandler>`.
#[async_trait]
pub trait QueryHandler: Send + Sync {
    async fn execute(&self, query: &str, ctx: &ConnContext) -> Result<QueryResult, QueryError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Wire-level message types
// ──────────────────────────────────────────────────────────────────────────────

/// Messages sent **from** the client **to** the server.
#[derive(Debug)]
pub enum FrontendMessage {
    /// Initial startup packet (no type byte, unlike all other messages).
    StartupMessage {
        protocol_version: i32,
        params: HashMap<String, String>,
    },
    /// Simple query (`Q` / 0x51).
    Query(String),
    /// Client requests graceful shutdown (`X` / 0x58).
    Terminate,
}

/// Messages sent **from** the server **to** the client.
#[derive(Debug)]
pub enum BackendMessage {
    /// Authentication succeeded (`R` / 0x52).
    AuthenticationOk,
    /// Backend process ID and secret key (`K` / 0x4B).
    BackendKeyData { pid: i32, secret_key: i32 },
    /// Ready for a new query cycle (`Z` / 0x5A).
    ReadyForQuery {
        /// Transaction status indicator: `b'I'` = idle.
        status: u8,
    },
    /// Result-set column metadata (`T` / 0x54).
    RowDescription { fields: Vec<FieldDescription> },
    /// A single result row (`D` / 0x44).
    DataRow { values: Vec<Option<String>> },
    /// End of a query result (`C` / 0x43).
    CommandComplete { tag: String },
    /// An error condition (`E` / 0x45).
    ErrorResponse {
        severity: String,
        code: String,
        message: String,
    },
    /// Response to an empty query string (`I` / 0x49).
    EmptyQueryResponse,
}

// ──────────────────────────────────────────────────────────────────────────────
// Encoding helpers
// ──────────────────────────────────────────────────────────────────────────────

impl BackendMessage {
    /// Encode the message into `buf`, ready to be sent over the wire.
    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            // AuthenticationOk:  'R'  length=8  auth_type=0
            BackendMessage::AuthenticationOk => {
                buf.put_u8(b'R');
                buf.put_i32(8);
                buf.put_i32(0);
            }

            // BackendKeyData:  'K'  length=12  pid  secret_key
            BackendMessage::BackendKeyData { pid, secret_key } => {
                buf.put_u8(b'K');
                buf.put_i32(12);
                buf.put_i32(*pid);
                buf.put_i32(*secret_key);
            }

            // ReadyForQuery:  'Z'  length=5  status
            BackendMessage::ReadyForQuery { status } => {
                buf.put_u8(b'Z');
                buf.put_i32(5);
                buf.put_u8(*status);
            }

            // RowDescription:  'T'  length  field_count  [per-field ...]
            //
            // Per field:
            //   name \0
            //   table_oid   int32  (0)
            //   col_attr    int16  (0)
            //   type_oid    int32
            //   type_size   int16  (-1 = variable)
            //   type_mod    int32  (-1)
            //   format      int16  (0 = text)
            BackendMessage::RowDescription { fields } => {
                let mut body = BytesMut::new();
                body.put_i16(fields.len() as i16);
                for f in fields {
                    body.put_slice(f.name.as_bytes());
                    body.put_u8(0); // null terminator
                    body.put_i32(0); // table OID
                    body.put_i16(0); // column attribute number
                    body.put_i32(f.type_oid);
                    body.put_i16(-1); // data type size (variable)
                    body.put_i32(-1); // type modifier
                    body.put_i16(0); // format code (text)
                }
                buf.put_u8(b'T');
                buf.put_i32(4 + body.len() as i32);
                buf.put(body);
            }

            // DataRow:  'D'  length  field_count  [per-value ...]
            //
            // Per value:
            //   -1 (int32) = NULL
            //   n  (int32) followed by n bytes of text
            BackendMessage::DataRow { values } => {
                let mut body = BytesMut::new();
                body.put_i16(values.len() as i16);
                for v in values {
                    match v {
                        None => body.put_i32(-1),
                        Some(s) => {
                            body.put_i32(s.len() as i32);
                            body.put_slice(s.as_bytes());
                        }
                    }
                }
                buf.put_u8(b'D');
                buf.put_i32(4 + body.len() as i32);
                buf.put(body);
            }

            // CommandComplete:  'C'  length  tag \0
            BackendMessage::CommandComplete { tag } => {
                buf.put_u8(b'C');
                buf.put_i32(4 + tag.len() as i32 + 1);
                buf.put_slice(tag.as_bytes());
                buf.put_u8(0);
            }

            // ErrorResponse:  'E'  length  fields  \0
            //
            // Each field: field_type (u8) + value \0
            //   'S' = severity, 'C' = code, 'M' = message
            BackendMessage::ErrorResponse {
                severity,
                code,
                message,
            } => {
                let mut body = BytesMut::new();
                body.put_u8(b'S');
                body.put_slice(severity.as_bytes());
                body.put_u8(0);
                body.put_u8(b'C');
                body.put_slice(code.as_bytes());
                body.put_u8(0);
                body.put_u8(b'M');
                body.put_slice(message.as_bytes());
                body.put_u8(0);
                body.put_u8(0); // terminator
                buf.put_u8(b'E');
                buf.put_i32(4 + body.len() as i32);
                buf.put(body);
            }

            // EmptyQueryResponse:  'I'  length=4
            BackendMessage::EmptyQueryResponse => {
                buf.put_u8(b'I');
                buf.put_i32(4);
            }
        }
    }

    /// Convenience: encode into a fresh [`Bytes`].
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();
        self.encode(&mut buf);
        buf.freeze()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Parsing helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Read null-terminated strings from `src`.
fn read_cstring(src: &mut &[u8]) -> Option<String> {
    let nul = src.iter().position(|&b| b == 0)?;
    let s = String::from_utf8_lossy(&src[..nul]).into_owned();
    *src = &src[nul + 1..];
    Some(s)
}

/// Parse a `StartupMessage` from the raw body bytes (after the 8-byte header).
///
/// The body is a sequence of key-value pairs, each null-terminated, followed
/// by an extra null byte.
pub fn parse_startup_params(body: &[u8]) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let mut src: &[u8] = body;
    loop {
        match read_cstring(&mut src) {
            None => break,
            Some(key) if key.is_empty() => break,
            Some(key) => {
                if let Some(value) = read_cstring(&mut src) {
                    params.insert(key, value);
                } else {
                    break;
                }
            }
        }
    }
    params
}

// ──────────────────────────────────────────────────────────────────────────────
// Connection handler
// ──────────────────────────────────────────────────────────────────────────────

/// SSL request magic bytes (8-byte payload, no type byte).
const SSL_REQUEST_CODE: i32 = 80877103; // 0x04D2162F

/// Handles a single client connection for its entire lifetime.
pub struct PgConnection {
    stream: TcpStream,
}

impl PgConnection {
    /// Create a new connection from an accepted [`TcpStream`].
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    // ── low-level I/O ────────────────────────────────────────────────────────

    /// Send one backend message.
    async fn send(&mut self, msg: BackendMessage) -> Result<()> {
        let bytes = msg.to_bytes();
        self.stream.write_all(&bytes).await?;
        Ok(())
    }

    /// Flush all buffered writes.
    async fn flush(&mut self) -> Result<()> {
        self.stream.flush().await?;
        Ok(())
    }

    /// Read exactly `n` bytes into a fresh `Vec`.
    async fn read_exact_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf).await?;
        Ok(buf)
    }

    // ── startup phase ────────────────────────────────────────────────────────

    /// Read and handle the startup phase (SSL negotiation + StartupMessage).
    ///
    /// Returns the parsed connection parameters on success.
    async fn startup(&mut self) -> Result<HashMap<String, String>> {
        loop {
            // Read 4-byte length prefix.
            let len_bytes = self.read_exact_bytes(4).await?;
            let total_len = i32::from_be_bytes(len_bytes[..4].try_into().unwrap());
            if total_len < 4 {
                return Err(anyhow!("invalid startup message: declared length {} is less than the 4-byte minimum. Ensure the client uses PostgreSQL protocol v3.0", total_len));
            }
            let payload_len = (total_len - 4) as usize;
            let payload = self.read_exact_bytes(payload_len).await?;

            // Read the protocol version / request code (next 4 bytes).
            if payload.len() < 4 {
                return Err(anyhow!("startup payload too short: expected at least 4 bytes for protocol version, got {}", payload.len()));
            }
            let code = i32::from_be_bytes(payload[..4].try_into().unwrap());

            if code == SSL_REQUEST_CODE {
                // Decline SSL — respond with single byte 'N'.
                debug!("SSL request received, responding with N");
                self.stream.write_all(b"N").await?;
                self.flush().await?;
                // Next iteration will read the real StartupMessage.
                continue;
            }

            // Normal startup message: payload[0..4] = protocol version,
            // remainder = NUL-terminated key=value pairs.
            let params = parse_startup_params(&payload[4..]);
            debug!(?params, "startup params received");
            return Ok(params);
        }
    }

    // ── query loop ───────────────────────────────────────────────────────────

    /// Read one frontend message (post-startup).
    async fn read_frontend_message(&mut self) -> Result<FrontendMessage> {
        // Each message: type (1 byte) + length (4 bytes, includes itself).
        let type_byte = self.read_exact_bytes(1).await?;
        let len_bytes = self.read_exact_bytes(4).await?;
        let total_len = i32::from_be_bytes(len_bytes[..4].try_into().unwrap());
        if total_len < 4 {
            return Err(anyhow!("invalid message length {}: must be at least 4 bytes", total_len));
        }
        let body_len = (total_len - 4) as usize;
        let body = self.read_exact_bytes(body_len).await?;

        match type_byte[0] {
            b'Q' => {
                // Query: null-terminated string.
                let query = if body.ends_with(&[0]) {
                    String::from_utf8_lossy(&body[..body.len() - 1]).into_owned()
                } else {
                    String::from_utf8_lossy(&body).into_owned()
                };
                Ok(FrontendMessage::Query(query))
            }
            b'X' => Ok(FrontendMessage::Terminate),
            other => {
                warn!("unknown frontend message type: 0x{:02x}, closing connection", other);
                Err(anyhow!("unsupported frontend message type: 0x{:02x} ('{}' as char). Supported: Q (Query), X (Terminate)", other, other as char))
            }
        }
    }

    // ── public entry point ───────────────────────────────────────────────────

    /// Run this connection to completion.
    ///
    /// Performs startup handshake then drives the query loop until the client
    /// disconnects or sends a `Terminate` message.
    pub async fn run(mut self, handler: Arc<dyn QueryHandler>) -> Result<()> {
        // ── startup ──────────────────────────────────────────────────────────
        let params = match self.startup().await {
            Ok(p) => p,
            Err(e) => {
                warn!("startup failed: {}", e);
                return Err(e);
            }
        };

        // Send AuthenticationOk → BackendKeyData → ReadyForQuery.
        self.send(BackendMessage::AuthenticationOk).await?;
        self.send(BackendMessage::BackendKeyData {
            pid: 1,
            secret_key: 0,
        })
        .await?;
        self.send(BackendMessage::ReadyForQuery { status: b'I' })
            .await?;
        self.flush().await?;

        let ctx = ConnContext {
            user: params.get("user").cloned().unwrap_or_default(),
            database: params.get("database").cloned().unwrap_or_default(),
            tenant_id: 0,
            agent_id: None,
        };

        // ── query loop ───────────────────────────────────────────────────────
        loop {
            let msg = match self.read_frontend_message().await {
                Ok(m) => m,
                Err(e) => {
                    // Treat EOF/connection-reset as clean disconnect.
                    debug!("connection closed or error reading message: {}", e);
                    return Ok(());
                }
            };

            match msg {
                FrontendMessage::Terminate => {
                    debug!("client sent Terminate");
                    return Ok(());
                }

                FrontendMessage::Query(ref query) => {
                    let trimmed = query.trim();
                    if trimmed.is_empty() {
                        // Empty query: respond with EmptyQueryResponse + ReadyForQuery.
                        self.send(BackendMessage::EmptyQueryResponse).await?;
                    } else {
                        match handler.execute(trimmed, &ctx).await {
                            Ok(result) => {
                                // Send RowDescription only if there are columns.
                                if !result.columns.is_empty() {
                                    self.send(BackendMessage::RowDescription {
                                        fields: result.columns.clone(),
                                    })
                                    .await?;
                                    for row in &result.rows {
                                        self.send(BackendMessage::DataRow {
                                            values: row.clone(),
                                        })
                                        .await?;
                                    }
                                }
                                // CommandComplete always sent.
                                self.send(BackendMessage::CommandComplete {
                                    tag: result.command_tag,
                                })
                                .await?;
                            }
                            Err(qe) => {
                                self.send(BackendMessage::ErrorResponse {
                                    severity: "ERROR".into(),
                                    code: qe.code,
                                    message: qe.message,
                                })
                                .await?;
                            }
                        }
                    }
                    // Always send ReadyForQuery to signal we are ready for the next command.
                    self.send(BackendMessage::ReadyForQuery { status: b'I' })
                        .await?;
                }

                // StartupMessage should not arrive after the startup phase.
                FrontendMessage::StartupMessage { .. } => {
                    warn!("unexpected StartupMessage in query loop");
                }
            }

            self.flush().await?;
        }
    }
}
