/// Raw TCP connection to FunDB using the PostgreSQL wire protocol (frontend/backend).
///
/// Implements only the minimal subset needed by the CLI:
///   – Startup message (protocol 3.0 = 196608)
///   – Simple Query ('Q')
///   – Backend message decoding for: R, S, K, Z, T, D, C, E, N
use anyhow::Context;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct QueryResponse {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub command_tag: String,
    pub notice: Option<String>,
    pub error: Option<String>,
}

pub struct FunDbConn {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    host: String,
    port: u16,
    database: String,
    user: String,
}

// ---------------------------------------------------------------------------
// Connection lifecycle
// ---------------------------------------------------------------------------

impl FunDbConn {
    /// Connect and perform the PG startup handshake.
    pub async fn connect(
        host: &str,
        port: u16,
        database: &str,
        user: &str,
        password: Option<&str>,
    ) -> anyhow::Result<Self> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Cannot connect to {}", addr))?;

        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        // --- Send startup message ---
        let startup = build_startup(user, database);
        write_half
            .write_all(&startup)
            .await
            .context("Failed to send startup message")?;

        // --- Read server messages until ReadyForQuery ---
        loop {
            let (msg_type, payload) = read_message(&mut reader).await?;
            match msg_type {
                b'R' => {
                    // AuthenticationRequest
                    let auth_type = read_i32_be(&payload, 0)?;
                    match auth_type {
                        0 => {
                            // AuthenticationOk — nothing to do
                        }
                        3 => {
                            // CleartextPassword
                            let pw = password.unwrap_or("");
                            let pw_msg = build_password_message(pw);
                            write_half
                                .write_all(&pw_msg)
                                .await
                                .context("Failed to send password")?;
                        }
                        5 => {
                            // MD5Password — salt is bytes 4..8
                            let pw = password.unwrap_or("");
                            if payload.len() < 8 {
                                return Err(anyhow::anyhow!("MD5 authentication failed: server sent incomplete salt (expected 8 bytes, got {})", payload.len()));
                            }
                            let salt = &payload[4..8];
                            let md5_pw = md5_password(user, pw, salt);
                            let pw_msg = build_password_message(&md5_pw);
                            write_half
                                .write_all(&pw_msg)
                                .await
                                .context("Failed to send MD5 password")?;
                        }
                        other => {
                            return Err(anyhow::anyhow!(
                                "Unsupported authentication method (type {}). FunDB CLI supports cleartext (3) and MD5 (5) authentication",
                                other
                            ));
                        }
                    }
                }
                b'S' => {
                    // ParameterStatus — ignore
                }
                b'K' => {
                    // BackendKeyData — ignore
                }
                b'Z' => {
                    // ReadyForQuery — we are done with handshake
                    break;
                }
                b'E' => {
                    let msg = parse_error_response(&payload);
                    return Err(anyhow::anyhow!("Server error during startup: {}", msg));
                }
                b'N' => {
                    // NoticeResponse during startup — ignore
                }
                other => {
                    return Err(anyhow::anyhow!(
                        "Unexpected protocol message '{}' (0x{:02x}) during startup. Is the server running FunDB or a compatible PostgreSQL protocol?",
                        other as char, other
                    ));
                }
            }
        }

        Ok(Self {
            reader,
            writer: write_half,
            host: host.to_string(),
            port,
            database: database.to_string(),
            user: user.to_string(),
        })
    }

    /// Send a Simple Query message and collect the full response.
    pub async fn simple_query(&mut self, sql: &str) -> anyhow::Result<QueryResponse> {
        let msg = build_simple_query(sql);
        self.writer
            .write_all(&msg)
            .await
            .context("Failed to send query")?;

        let mut response = QueryResponse::default();

        loop {
            let (msg_type, payload) = read_message(&mut self.reader).await?;
            match msg_type {
                b'T' => {
                    // RowDescription
                    response.columns = parse_row_description(&payload)?;
                }
                b'D' => {
                    // DataRow
                    let row = parse_data_row(&payload)?;
                    response.rows.push(row);
                }
                b'C' => {
                    // CommandComplete
                    response.command_tag =
                        parse_cstring(&payload, 0).unwrap_or_default().to_string();
                }
                b'E' => {
                    // ErrorResponse
                    response.error = Some(parse_error_response(&payload));
                }
                b'N' => {
                    // NoticeResponse
                    response.notice = Some(parse_notice_response(&payload));
                }
                b'Z' => {
                    // ReadyForQuery — end of response
                    break;
                }
                b'S' => {
                    // ParameterStatus — ignore
                }
                b'I' => {
                    // EmptyQueryResponse — no data
                }
                other => {
                    // Unknown message — log and skip
                    tracing::debug!("Unknown backend message type: {}", other as char);
                }
            }
        }

        Ok(response)
    }

    /// Send a Terminate message and close the connection.
    pub async fn close(&mut self) -> anyhow::Result<()> {
        // 'X' Terminate: type byte + 4-byte length
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u8(b'X');
        buf.put_i32(4);
        let _ = self.writer.write_all(&buf).await;
        let _ = self.writer.shutdown().await;
        Ok(())
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    pub fn user(&self) -> &str {
        &self.user
    }
}

// ---------------------------------------------------------------------------
// Message builders (frontend → backend)
// ---------------------------------------------------------------------------

/// Build the initial startup message (not prefixed with a type byte).
/// Format: [i32 total_length][i32 protocol_version][key\0value\0 pairs...][0x00]
fn build_startup(user: &str, database: &str) -> Vec<u8> {
    let protocol: i32 = 196608; // 3.0

    let mut params = Vec::new();
    params.extend_from_slice(b"user\0");
    params.extend_from_slice(user.as_bytes());
    params.push(0);
    params.extend_from_slice(b"database\0");
    params.extend_from_slice(database.as_bytes());
    params.push(0);
    params.push(0); // terminating null

    // Total length = 4 (length field) + 4 (protocol) + params
    let total_len = 4 + 4 + params.len();

    let mut buf = Vec::with_capacity(total_len);
    buf.extend_from_slice(&(total_len as i32).to_be_bytes());
    buf.extend_from_slice(&protocol.to_be_bytes());
    buf.extend_from_slice(&params);
    buf
}

/// Build a PasswordMessage ('p') for cleartext or md5 passwords.
fn build_password_message(password: &str) -> Vec<u8> {
    // Type 'p' + i32 length (includes itself) + password + null
    let pwd_bytes = password.as_bytes();
    let len = 4 + pwd_bytes.len() + 1;
    let mut buf = Vec::with_capacity(1 + len);
    buf.push(b'p');
    buf.extend_from_slice(&(len as i32).to_be_bytes());
    buf.extend_from_slice(pwd_bytes);
    buf.push(0);
    buf
}

/// Build a SimpleQuery ('Q') message.
fn build_simple_query(sql: &str) -> Vec<u8> {
    let sql_bytes = sql.as_bytes();
    // Type 'Q' + i32 length (4 + sql_len + 1 null terminator)
    let len = 4 + sql_bytes.len() + 1;
    let mut buf = Vec::with_capacity(1 + len);
    buf.push(b'Q');
    buf.extend_from_slice(&(len as i32).to_be_bytes());
    buf.extend_from_slice(sql_bytes);
    buf.push(0);
    buf
}

// ---------------------------------------------------------------------------
// Message reader (backend → frontend)
// ---------------------------------------------------------------------------

/// Read a single backend message: returns (type_byte, payload_bytes).
/// The payload does NOT include the type byte or the 4-byte length field.
async fn read_message(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> anyhow::Result<(u8, Vec<u8>)> {
    // Read 1-byte type
    let mut type_buf = [0u8; 1];
    reader
        .read_exact(&mut type_buf)
        .await
        .context("Connection closed while reading message type")?;
    let msg_type = type_buf[0];

    // Read 4-byte big-endian length (includes itself)
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .await
        .context("Connection closed while reading message length")?;
    let total_len = i32::from_be_bytes(len_buf) as usize;

    if total_len < 4 {
        return Err(anyhow::anyhow!(
            "Invalid message length {} for type '{}'",
            total_len,
            msg_type as char
        ));
    }

    let payload_len = total_len - 4;
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader
            .read_exact(&mut payload)
            .await
            .context("Connection closed while reading message payload")?;
    }

    Ok((msg_type, payload))
}

// ---------------------------------------------------------------------------
// Payload parsers
// ---------------------------------------------------------------------------

/// Parse a RowDescription ('T') payload → list of column names.
fn parse_row_description(payload: &[u8]) -> anyhow::Result<Vec<String>> {
    if payload.len() < 2 {
        return Ok(vec![]);
    }
    let num_fields = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    let mut columns = Vec::with_capacity(num_fields);
    let mut pos = 2usize;

    for _ in 0..num_fields {
        // Column name (null-terminated string)
        let name = read_cstring(payload, &mut pos)?;
        columns.push(name);

        // Skip: tableOid (4), colAttr (2), typeOid (4), typeLen (2), typeMod (4), format (2)
        pos += 4 + 2 + 4 + 2 + 4 + 2;
    }

    Ok(columns)
}

/// Parse a DataRow ('D') payload → list of column values as strings.
fn parse_data_row(payload: &[u8]) -> anyhow::Result<Vec<String>> {
    if payload.len() < 2 {
        return Ok(vec![]);
    }
    let num_cols = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    let mut row = Vec::with_capacity(num_cols);
    let mut pos = 2usize;

    for _ in 0..num_cols {
        if pos + 4 > payload.len() {
            return Err(anyhow::anyhow!("DataRow payload truncated"));
        }
        let col_len = i32::from_be_bytes([
            payload[pos],
            payload[pos + 1],
            payload[pos + 2],
            payload[pos + 3],
        ]);
        pos += 4;

        if col_len == -1 {
            // NULL value
            row.push("NULL".to_string());
        } else {
            let col_len = col_len as usize;
            if pos + col_len > payload.len() {
                return Err(anyhow::anyhow!("DataRow column data truncated"));
            }
            let value = std::str::from_utf8(&payload[pos..pos + col_len])
                .unwrap_or("<binary>")
                .to_string();
            row.push(value);
            pos += col_len;
        }
    }

    Ok(row)
}

/// Parse an ErrorResponse ('E') payload into a human-readable string.
fn parse_error_response(payload: &[u8]) -> String {
    parse_field_messages(payload, b"MDH").unwrap_or_else(|| "Unknown server error".to_string())
}

/// Parse a NoticeResponse ('N') payload into a human-readable string.
fn parse_notice_response(payload: &[u8]) -> String {
    parse_field_messages(payload, b"M").unwrap_or_else(|| "Notice".to_string())
}

/// Walk the field-value pairs in an error/notice payload.
/// Returns the concatenation of fields whose codes appear in `wanted`.
fn parse_field_messages(payload: &[u8], wanted: &[u8]) -> Option<String> {
    let mut pos = 0usize;
    let mut parts: Vec<String> = Vec::new();

    while pos < payload.len() {
        let code = payload[pos];
        pos += 1;
        if code == 0 {
            break;
        }
        // Null-terminated string for this field
        let start = pos;
        while pos < payload.len() && payload[pos] != 0 {
            pos += 1;
        }
        let value = std::str::from_utf8(&payload[start..pos]).unwrap_or("");
        pos += 1; // skip null terminator

        if wanted.contains(&code) {
            parts.push(value.to_string());
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" — "))
    }
}

// ---------------------------------------------------------------------------
// Low-level helpers
// ---------------------------------------------------------------------------

/// Read a null-terminated C-string from `buf` starting at `*pos`, advancing `*pos`.
fn read_cstring(buf: &[u8], pos: &mut usize) -> anyhow::Result<String> {
    let start = *pos;
    while *pos < buf.len() && buf[*pos] != 0 {
        *pos += 1;
    }
    let s = std::str::from_utf8(&buf[start..*pos])
        .context("Non-UTF8 string in message")?
        .to_string();
    *pos += 1; // skip null
    Ok(s)
}

/// Read a null-terminated C-string from `buf` starting at `offset` (non-mutating).
fn parse_cstring(buf: &[u8], offset: usize) -> Option<&str> {
    let slice = &buf[offset..];
    let end = slice.iter().position(|&b| b == 0)?;
    std::str::from_utf8(&slice[..end]).ok()
}

/// Read a big-endian i32 from `buf` at `offset`.
fn read_i32_be(buf: &[u8], offset: usize) -> anyhow::Result<i32> {
    if buf.len() < offset + 4 {
        return Err(anyhow::anyhow!(
            "Buffer too short to read i32 at {}",
            offset
        ));
    }
    Ok(i32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]))
}

/// Compute the MD5-based password string used by PG auth type 5.
/// Result: "md5" + hex(md5(hex(md5(password + user)) + salt))
fn md5_password(user: &str, password: &str, salt: &[u8]) -> String {
    // We implement a minimal MD5 inline so we don't need an external crate.
    // Using the standard library's lack of md5, we fall back to a simple hex string.
    // NOTE: This is a best-effort implementation. FunDB may not use MD5 auth.
    let inner = format!("{}{}", password, user);
    let inner_hash = simple_md5(inner.as_bytes());
    let inner_hex = hex_encode(&inner_hash);
    let mut salted = inner_hex.into_bytes();
    salted.extend_from_slice(salt);
    let outer_hash = simple_md5(&salted);
    let outer_hex = hex_encode(&outer_hash);
    format!("md5{}", outer_hex)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Minimal MD5 implementation (RFC 1321).
/// This avoids adding an external md5 crate while staying `no unsafe`.
fn simple_md5(input: &[u8]) -> [u8; 16] {
    // Per-round shift amounts
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // Precomputed table T[i] = floor(2^32 * |sin(i+1)|)
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let msg_len = input.len();
    let bit_len = (msg_len as u64).wrapping_mul(8);

    // Padding: append 0x80, then zeros, then 8-byte little-endian bit length
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    for chunk in padded.chunks(64) {
        let mut m = [0u32; 16];
        for (j, word) in m.iter_mut().enumerate() {
            let base = j * 4;
            *word = u32::from_le_bytes([
                chunk[base],
                chunk[base + 1],
                chunk[base + 2],
                chunk[base + 3],
            ]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);

        for i in 0usize..64 {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                (a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g])).rotate_left(S[i]),
            );
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut digest = [0u8; 16];
    digest[0..4].copy_from_slice(&a0.to_le_bytes());
    digest[4..8].copy_from_slice(&b0.to_le_bytes());
    digest[8..12].copy_from_slice(&c0.to_le_bytes());
    digest[12..16].copy_from_slice(&d0.to_le_bytes());
    digest
}
