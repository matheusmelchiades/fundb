use crate::record::FunRecord;
use anyhow::Result;
use bytes::Bytes;

// ---------------------------------------------------------------------------
// Core encode / decode traits
// ---------------------------------------------------------------------------

/// Serialize a value to a [`Bytes`] buffer.
pub trait Encode {
    fn encode(&self) -> Result<Bytes>;
}

/// Deserialize a value from a byte slice.
pub trait Decode: Sized {
    fn decode(bytes: &[u8]) -> Result<Self>;
}

// ---------------------------------------------------------------------------
// FunRecord  — MessagePack  (rmp-serde)
// ---------------------------------------------------------------------------

impl Encode for FunRecord {
    fn encode(&self) -> Result<Bytes> {
        let vec = rmp_serde::to_vec_named(self)
            .map_err(|e| anyhow::anyhow!("msgpack encode error: {}", e))?;
        Ok(Bytes::from(vec))
    }
}

impl Decode for FunRecord {
    fn decode(bytes: &[u8]) -> Result<Self> {
        let record: FunRecord = rmp_serde::from_slice(bytes)
            .map_err(|e| anyhow::anyhow!("msgpack decode error: {}", e))?;
        Ok(record)
    }
}

// ---------------------------------------------------------------------------
// Zstd-compressed variants
// ---------------------------------------------------------------------------

/// Encode a [`FunRecord`] to MessagePack and then compress with zstd (level 3).
pub fn encode_compressed(record: &FunRecord) -> Result<Bytes> {
    let raw = record.encode()?;
    let compressed = zstd::encode_all(raw.as_ref(), 3)
        .map_err(|e| anyhow::anyhow!("zstd encode error: {}", e))?;
    Ok(Bytes::from(compressed))
}

/// Decompress a zstd-compressed blob and decode a [`FunRecord`] from the result.
pub fn decode_compressed(bytes: &[u8]) -> Result<FunRecord> {
    let decompressed =
        zstd::decode_all(bytes).map_err(|e| anyhow::anyhow!("zstd decode error: {}", e))?;
    FunRecord::decode(&decompressed)
}

// ---------------------------------------------------------------------------
// Token count estimation  (OQ-5)
// ---------------------------------------------------------------------------

/// Estimate the number of language-model tokens represented by a [`FunRecord`].
///
/// Formula (per OQ-5 resolution):
/// ```text
/// max(byte_length / 4, vector_dims * 6 + scalar_fields * 3)
/// ```
/// where:
/// - `byte_length`   = `record.data.len()`
/// - `vector_dims`   = sum of lengths of all named embedding vectors
/// - `scalar_fields` = 10 (fixed estimate for the cognitive metadata fields)
pub fn estimate_tokens(record: &FunRecord) -> u32 {
    let byte_length = record.data.len();

    let vector_dims: usize = record._vectors.values().map(|v| v.len()).sum();

    // Fixed approximation for scalar cognitive fields per OQ-5 resolution.
    let scalar_fields: usize = 10;

    let a = byte_length / 4;
    let b = vector_dims * 6 + scalar_fields * 3;

    std::cmp::max(a, b) as u32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::FunRecordBuilder;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a minimal record with no data or vectors.
    fn minimal_record() -> FunRecord {
        FunRecordBuilder::new("test").build()
    }

    /// Build a record with `data_len` bytes of data payload and `vector_dims`
    /// dimensions in a single embedding named "v".
    fn record_with(data_len: usize, vector_dims: usize) -> FunRecord {
        let data = vec![0xAB_u8; data_len];
        let vecs: Vec<f32> = (0..vector_dims).map(|i| i as f32 * 0.001).collect();
        FunRecordBuilder::new("test")
            .data(data)
            .vector("v", vecs)
            .build()
    }

    // -----------------------------------------------------------------------
    // Encode / Decode roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_decode_roundtrip() {
        let record = minimal_record();
        let encoded = record.encode().expect("encode must succeed");
        let decoded = FunRecord::decode(&encoded).expect("decode must succeed");
        // Re-encode the decoded value; bytes must be identical.
        let re_encoded = decoded.encode().expect("re-encode must succeed");
        assert_eq!(
            encoded, re_encoded,
            "encode(decode(encode(r))) must equal encode(r)"
        );
    }

    #[test]
    fn test_encode_decode_roundtrip_with_payload() {
        let record = record_with(128, 64);
        let encoded = record.encode().expect("encode must succeed");
        let decoded = FunRecord::decode(&encoded).expect("decode must succeed");
        let re_encoded = decoded.encode().expect("re-encode must succeed");
        assert_eq!(encoded, re_encoded);
    }

    // -----------------------------------------------------------------------
    // 20 manually constructed varied records — encode/decode/re-encode
    // -----------------------------------------------------------------------

    #[test]
    fn test_varied_records_roundtrip() {
        let test_cases: Vec<FunRecord> = vec![
            // 1 – empty record
            FunRecordBuilder::new("col_a").build(),
            // 2 – with data only
            FunRecordBuilder::new("col_b")
                .data(vec![0x01, 0x02, 0x03])
                .build(),
            // 3 – with small vector
            FunRecordBuilder::new("col_c")
                .vector("emb", vec![1.0, 2.0])
                .build(),
            // 4 – data + vector
            record_with(32, 16),
            // 5 – larger data
            record_with(256, 0),
            // 6 – larger vector
            record_with(0, 512),
            // 7 – high confidence
            FunRecordBuilder::new("col_d").confidence(0.99).build(),
            // 8 – low confidence
            FunRecordBuilder::new("col_e").confidence(0.01).build(),
            // 9 – different tenant
            FunRecordBuilder::new("col_f").tenant(7).build(),
            // 10 – tenant + data + vector
            FunRecordBuilder::new("col_g")
                .tenant(42)
                .data(vec![0xFF; 64])
                .vector("x", vec![0.5; 8])
                .build(),
            // 11 – multiple vectors
            {
                let mut r = FunRecordBuilder::new("col_h")
                    .vector("a", vec![0.1; 4])
                    .vector("b", vec![0.2; 4])
                    .build();
                r._vectors.insert("c".to_string(), vec![0.3; 4]);
                r
            },
            // 12 – with edge
            {
                use crate::id::new_record_id;
                use crate::record::Edge;
                let e = Edge {
                    label: "follows".to_string(),
                    target: new_record_id(),
                    props: vec![0x80],
                    confidence: 0.75,
                };
                FunRecordBuilder::new("col_i").edge(e).build()
            },
            // 13 – with source
            {
                use crate::record::Source;
                use crate::types::SourceMethod;
                let s = Source {
                    origin: "model:gpt-4".to_string(),
                    timestamp: 1_700_000_000_000_000_000,
                    method: SourceMethod::Inference,
                    confidence: 0.9,
                };
                FunRecordBuilder::new("col_j").source(s).build()
            },
            // 14 – with valid time
            FunRecordBuilder::new("col_k")
                .valid_time(1_000_000_000, 2_000_000_000)
                .build(),
            // 15 – max data
            record_with(1024, 0),
            // 16 – max vector dims
            record_with(0, 1024),
            // 17 – data + many vector dims
            record_with(512, 512),
            // 18 – supports ref
            {
                use crate::id::new_record_id;
                use crate::record::Ref;
                let mut r = FunRecordBuilder::new("col_l").build();
                r._supports.push(Ref {
                    id: new_record_id(),
                    strength: 0.5,
                });
                r
            },
            // 19 – contradicts ref
            {
                use crate::id::new_record_id;
                use crate::record::Ref;
                let mut r = FunRecordBuilder::new("col_m").build();
                r._contradicts.push(Ref {
                    id: new_record_id(),
                    strength: 0.3,
                });
                r
            },
            // 20 – all non-causal fields set
            {
                use crate::id::new_record_id;
                use crate::record::{Edge, Ref, Source};
                use crate::types::SourceMethod;
                let e = Edge {
                    label: "cites".to_string(),
                    target: new_record_id(),
                    props: vec![0x80],
                    confidence: 0.8,
                };
                let s = Source {
                    origin: "sensor:01".to_string(),
                    timestamp: 0,
                    method: SourceMethod::Observation,
                    confidence: 1.0,
                };
                let mut r = FunRecordBuilder::new("col_n")
                    .tenant(99)
                    .data(vec![1, 2, 3, 4])
                    .vector("emb", vec![0.1, 0.2, 0.3])
                    .confidence(0.77)
                    .edge(e)
                    .source(s)
                    .valid_time(0, i64::MAX)
                    .build();
                r._supports.push(Ref {
                    id: new_record_id(),
                    strength: 0.6,
                });
                r._contradicts.push(Ref {
                    id: new_record_id(),
                    strength: 0.2,
                });
                r
            },
        ];

        for (i, record) in test_cases.iter().enumerate() {
            let encoded = record
                .encode()
                .unwrap_or_else(|e| panic!("case {}: encode failed: {}", i + 1, e));
            let decoded = FunRecord::decode(&encoded)
                .unwrap_or_else(|e| panic!("case {}: decode failed: {}", i + 1, e));
            let re_encoded = decoded
                .encode()
                .unwrap_or_else(|e| panic!("case {}: re-encode failed: {}", i + 1, e));
            assert_eq!(
                encoded,
                re_encoded,
                "case {}: encode(decode(encode(r))) != encode(r)",
                i + 1
            );
        }
    }

    // -----------------------------------------------------------------------
    // Compressed roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_compressed_roundtrip() {
        let record = record_with(512, 128);
        let compressed = encode_compressed(&record).expect("compress must succeed");
        let decoded = decode_compressed(&compressed).expect("decompress must succeed");
        // Verify content matches by re-encoding
        let original_bytes = record.encode().unwrap();
        let decoded_bytes = decoded.encode().unwrap();
        assert_eq!(original_bytes, decoded_bytes);
    }

    #[test]
    fn test_compressed_is_smaller_for_large_records() {
        // Large repetitive data compresses well.
        let record = record_with(4096, 0);
        let raw = record.encode().expect("encode");
        let compressed = encode_compressed(&record).expect("compress");
        assert!(
            compressed.len() < raw.len(),
            "compressed ({} B) should be smaller than raw ({} B)",
            compressed.len(),
            raw.len()
        );
    }

    // -----------------------------------------------------------------------
    // estimate_tokens — 3 explicit examples
    // -----------------------------------------------------------------------

    #[test]
    fn test_estimate_tokens_no_data_no_vectors() {
        // data_len=0, vector_dims=0
        // a = 0/4 = 0
        // b = 0*6 + 10*3 = 30
        // max(0, 30) = 30
        let record = FunRecordBuilder::new("t").build();
        assert_eq!(estimate_tokens(&record), 30);
    }

    #[test]
    fn test_estimate_tokens_data_dominates() {
        // data_len=1200, vector_dims=0
        // a = 1200/4 = 300
        // b = 0*6 + 10*3 = 30
        // max(300, 30) = 300
        let record = FunRecordBuilder::new("t").data(vec![0xAB; 1200]).build();
        assert_eq!(estimate_tokens(&record), 300);
    }

    #[test]
    fn test_estimate_tokens_vector_dominates() {
        // data_len=100, vector_dims=200
        // a = 100/4 = 25
        // b = 200*6 + 10*3 = 1200 + 30 = 1230
        // max(25, 1230) = 1230
        let record = FunRecordBuilder::new("t")
            .data(vec![0x00; 100])
            .vector("emb", vec![0.0_f32; 200])
            .build();
        assert_eq!(estimate_tokens(&record), 1230);
    }
}
