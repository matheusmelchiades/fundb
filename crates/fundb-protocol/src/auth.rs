//! SCRAM-SHA-256 authentication scaffolding for FunDB.
//!
//! This module implements the structural message framing for SASL/SCRAM-SHA-256
//! authentication as defined in RFC 5802 and the PostgreSQL wire protocol.
//!
//! The actual cryptographic proof verification (HMAC-SHA-256, PBKDF2 key
//! stretching) is left as a stub that always succeeds, making this suitable for
//! integration tests and future extension with a real crypto library such as
//! `ring` or `hmac`/`sha2`.
//!
//! ## Authentication flow
//!
//! ```text
//! Server  ──► AuthenticationSASL ("SCRAM-SHA-256")
//! Client  ──► SASLInitialResponse (client-first-message)
//! Server  ──► AuthenticationSASLContinue (server-first-message: nonce + salt + iter)
//! Client  ──► SASLResponse (client-final-message with proof)
//! Server  ──► AuthenticationSASLFinal (server-final-message: verifier)
//! Server  ──► AuthenticationOk
//! ```
//!
//! Protocol reference:
//! <https://www.postgresql.org/docs/current/sasl-authentication.html>

use bytes::{BufMut, BytesMut};

// ──────────────────────────────────────────────────────────────────────────────
// Server → Client messages
// ──────────────────────────────────────────────────────────────────────────────

/// Encode an `AuthenticationSASL` message requesting SCRAM-SHA-256.
///
/// Wire format:
/// ```text
/// 'R'  i32(length)  i32(10)  "SCRAM-SHA-256\0\0"
/// ```
/// The double null at the end terminates both the mechanism name and the list
/// of offered mechanism names.
pub fn encode_authentication_sasl(buf: &mut BytesMut) {
    let mechanism: &[u8] = b"SCRAM-SHA-256\0\0";
    // length = 4 (length field) + 4 (auth_type) + mechanism bytes
    let len: i32 = 4 + 4 + mechanism.len() as i32;
    buf.put_u8(b'R');
    buf.put_i32(len);
    buf.put_i32(10); // SASL auth type
    buf.put_slice(mechanism);
}

/// Encode an `AuthenticationSASLContinue` message with the server-first-message.
///
/// The server-first-message follows RFC 5802 §3:
/// ```text
/// r=<combined-nonce>,s=<base64-salt>,i=<iteration-count>
/// ```
/// We use a hard-coded salt (`AAAAAAAAAAAAAAAA` in base64, 12 zero bytes) and
/// 4096 iterations as required by the PostgreSQL specification.
///
/// Wire format:
/// ```text
/// 'R'  i32(length)  i32(11)  <SASL-data>
/// ```
pub fn encode_authentication_sasl_continue(buf: &mut BytesMut, server_nonce: &str) {
    let data = format!("r={},s=AAAAAAAAAAAAAAAA,i=4096", server_nonce);
    // length = 4 (length field) + 4 (auth_type) + data bytes
    let len: i32 = 4 + 4 + data.len() as i32;
    buf.put_u8(b'R');
    buf.put_i32(len);
    buf.put_i32(11); // SASL continue
    buf.put_slice(data.as_bytes());
}

/// Encode an `AuthenticationSASLFinal` message with the server-final-message.
///
/// The server-final-message is:
/// ```text
/// v=<base64-server-signature>
/// ```
///
/// Wire format:
/// ```text
/// 'R'  i32(length)  i32(12)  <SASL-data>
/// ```
pub fn encode_authentication_sasl_final(buf: &mut BytesMut, verifier: &str) {
    let data = format!("v={}", verifier);
    // length = 4 (length field) + 4 (auth_type) + data bytes
    let len: i32 = 4 + 4 + data.len() as i32;
    buf.put_u8(b'R');
    buf.put_i32(len);
    buf.put_i32(12); // SASL final
    buf.put_slice(data.as_bytes());
}

// ──────────────────────────────────────────────────────────────────────────────
// Utility
// ──────────────────────────────────────────────────────────────────────────────

/// Generate a server nonce to be sent in the server-first-message.
///
/// In production this must be cryptographically random (e.g. via
/// `rand::thread_rng().fill_bytes()`).  For determinism in tests we return a
/// fixed 29-character alphanumeric string that satisfies the RFC 5802 printable
/// ASCII requirement.
pub fn generate_server_nonce() -> String {
    // Real implementation: use rand::thread_rng() + base64-encode 24 random bytes.
    "xyHkG9YUTFVRZzabc123defghi456".to_string()
}

/// Stub SCRAM-SHA-256 response verifier.
///
/// A real implementation would:
/// 1. Extract the client nonce, channel binding, and proof from `client_response`.
/// 2. Derive the StoredKey and ServerKey from the stored salted password using
///    PBKDF2-HMAC-SHA-256.
/// 3. Verify the ClientProof with HMAC-SHA-256.
///
/// This stub always returns `true` to allow structural integration tests to
/// pass without a crypto dependency.
pub fn verify_scram_response(_client_response: &[u8], _server_nonce: &str) -> bool {
    true // stub: accept all responses
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

    // ── test 1: AuthenticationSASL ───────────────────────────────────────────

    #[test]
    fn test_encode_authentication_sasl() {
        let mut buf = BytesMut::new();
        encode_authentication_sasl(&mut buf);

        // First byte must be 'R' (0x52).
        assert_eq!(buf[0], b'R', "expected type byte 'R'");

        // auth_type at bytes 5..9 must be 10.
        let auth_type = read_i32_at(&buf, 5);
        assert_eq!(auth_type, 10, "expected auth_type = 10 for SASL");

        // Mechanism name must be present in the payload.
        let payload = &buf[9..];
        assert!(
            payload.starts_with(b"SCRAM-SHA-256"),
            "mechanism name not found in payload"
        );
    }

    // ── test 2: AuthenticationSASLContinue ───────────────────────────────────

    #[test]
    fn test_encode_authentication_sasl_continue() {
        let nonce = "testNonce42";
        let mut buf = BytesMut::new();
        encode_authentication_sasl_continue(&mut buf, nonce);

        assert_eq!(buf[0], b'R', "expected type byte 'R'");

        let auth_type = read_i32_at(&buf, 5);
        assert_eq!(auth_type, 11, "expected auth_type = 11 for SASLContinue");

        // The SASL data should start after the 9-byte header (type + length + auth_type).
        let sasl_data = std::str::from_utf8(&buf[9..]).expect("UTF-8");
        assert!(
            sasl_data.starts_with(&format!("r={}", nonce)),
            "server-first-message must begin with r=<nonce>"
        );
        assert!(sasl_data.contains(",s="), "server-first-message must contain salt");
        assert!(sasl_data.contains(",i="), "server-first-message must contain iteration count");
    }

    // ── test 3: AuthenticationSASLFinal ─────────────────────────────────────

    #[test]
    fn test_encode_authentication_sasl_final() {
        let verifier = "someBase64EncodedVerifier==";
        let mut buf = BytesMut::new();
        encode_authentication_sasl_final(&mut buf, verifier);

        assert_eq!(buf[0], b'R', "expected type byte 'R'");

        let auth_type = read_i32_at(&buf, 5);
        assert_eq!(auth_type, 12, "expected auth_type = 12 for SASLFinal");

        let sasl_data = std::str::from_utf8(&buf[9..]).expect("UTF-8");
        assert_eq!(
            sasl_data,
            &format!("v={}", verifier),
            "server-final-message mismatch"
        );
    }

    // ── test 4: generate_server_nonce returns non-empty string ───────────────

    #[test]
    fn test_generate_server_nonce() {
        let nonce = generate_server_nonce();
        assert!(!nonce.is_empty(), "server nonce must not be empty");
        // RFC 5802: nonce must not contain a comma.
        assert!(!nonce.contains(','), "server nonce must not contain a comma");
    }

    // ── test 5: verify_scram_response stub always returns true ───────────────

    #[test]
    fn test_verify_scram_response_stub() {
        assert!(
            verify_scram_response(b"some client proof data", "serverNonce"),
            "stub verifier must always return true"
        );
        assert!(
            verify_scram_response(b"", ""),
            "stub verifier must return true even for empty input"
        );
    }
}
