use bytes::{Bytes, BytesMut, BufMut};
use anyhow::Result;
use crate::record::FunRecord;
use crate::codec::{Encode, Decode};

// ---------------------------------------------------------------------------
// BloomFilter
// ---------------------------------------------------------------------------

/// A space-efficient probabilistic membership data structure backed by xxhash-rust xxh3.
///
/// Uses double-hashing to simulate `k` independent hash functions with only two
/// underlying hash calls (`h1 = xxh3_64(key)`, `h2 = xxh3_64_with_seed(key, h1)`).
pub struct BloomFilter {
    /// Bit-array stored as 64-bit words.
    pub bits: Vec<u64>,
    /// Number of hash functions applied per element.
    pub k: u8,
}

impl BloomFilter {
    /// Create a new BloomFilter sized for `capacity` items at false-positive rate `fpr`.
    ///
    /// Optimal m (bits):  `m = -n * ln(fpr) / ln(2)^2`
    /// Optimal k (hashes): `k = m/n * ln(2)`
    pub fn new(capacity: usize, fpr: f64) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        assert!((0.0..1.0).contains(&fpr), "fpr must be in (0, 1)");

        let n = capacity as f64;
        // Number of bits.
        let m = (-(n * fpr.ln()) / (2_f64.ln() * 2_f64.ln())).ceil() as usize;
        // Round up to a multiple of 64.
        let m_base = m.max(64);
        let m_rounded = (m_base + 63) & !63;
        let words = m_rounded / 64;

        // Optimal k: (m/n) * ln(2), minimum 1.
        let k = ((m_rounded as f64 / n) * 2_f64.ln())
            .round()
            .max(1.0) as u8;

        Self {
            bits: vec![0u64; words],
            k,
        }
    }

    /// Number of bits in the filter.
    #[inline]
    fn m(&self) -> usize {
        self.bits.len() * 64
    }

    /// Compute the `i`-th bit position for `key` using double hashing.
    #[inline]
    fn bit_index(&self, key: &[u8], i: u64) -> usize {
        use xxhash_rust::xxh3::{xxh3_64, xxh3_64_with_seed};
        let h1 = xxh3_64(key);
        let h2 = xxh3_64_with_seed(key, h1);
        let combined = h1.wrapping_add(i.wrapping_mul(h2));
        (combined as usize) % self.m()
    }

    /// Insert `key` into the filter.
    pub fn insert(&mut self, key: &[u8]) {
        for i in 0..self.k as u64 {
            let bit = self.bit_index(key, i);
            self.bits[bit / 64] |= 1u64 << (bit % 64);
        }
    }

    /// Test whether `key` is (probably) in the filter.
    ///
    /// Returns `false` if the key is definitely absent; `true` if probably present.
    pub fn contains(&self, key: &[u8]) -> bool {
        for i in 0..self.k as u64 {
            let bit = self.bit_index(key, i);
            if self.bits[bit / 64] & (1u64 << (bit % 64)) == 0 {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// PageHeader
// ---------------------------------------------------------------------------

/// Header for an SSTable column-page group (ARCHITECTURE.md §4.1).
///
/// Holds the key range, record count, bloom filter for point-lookup skipping,
/// and a confidence histogram for OQ-9 analytics.
pub struct PageHeader {
    /// Smallest key (encoded `_id` bytes) in the page.
    pub min_key: Bytes,
    /// Largest key (encoded `_id` bytes) in the page.
    pub max_key: Bytes,
    /// Number of records stored in the page.
    pub row_count: u32,
    /// Bloom filter keyed on raw `_id` UUID bytes for fast negative lookups.
    pub bloom_filter: BloomFilter,
    /// OQ-9: confidence histogram; bucket `i` counts records with
    /// `_confidence ∈ [i/100, (i+1)/100)`.  Bucket 99 is inclusive of 1.0.
    pub confidence_histogram: [u32; 100],
}

// ---------------------------------------------------------------------------
// PageEncoder
// ---------------------------------------------------------------------------

/// Encodes a slice of [`FunRecord`]s into a columnar binary page.
///
/// Wire layout:
/// ```text
/// [ u32 record_count ]
/// [ u32 msgpack_section_len ]
/// [ msgpack_section_len bytes: concatenated msgpack records ]
/// [ record_count * 4 bytes: f32 confidence values in the same order ]
/// ```
///
/// The confidence column is separated so that range/quantile scans can skip
/// the heavier msgpack payload.
pub struct PageEncoder;

impl PageEncoder {
    /// Encode `records` into a [`PageHeader`] and the raw page [`Bytes`].
    ///
    /// Returns an error if any record fails to encode.
    pub fn encode(records: &[FunRecord]) -> Result<(PageHeader, Bytes)> {
        if records.is_empty() {
            return Err(anyhow::anyhow!("cannot encode an empty record slice"));
        }

        // ----------------------------------------------------------------
        // Build msgpack section + extract confidence values and keys.
        // ----------------------------------------------------------------
        let mut msgpack_buf = Vec::new();
        let mut confidences: Vec<f32> = Vec::with_capacity(records.len());
        let mut min_key: Option<Bytes> = None;
        let mut max_key: Option<Bytes> = None;

        // Bloom filter: 1% FPR sized for record count.
        let mut bloom = BloomFilter::new(records.len().max(1), 0.01);
        let mut histogram = [0u32; 100];

        for record in records {
            let encoded = record.encode()?;
            msgpack_buf.extend_from_slice(&encoded);

            confidences.push(record._confidence);

            // Key is the raw UUID bytes.
            let key_bytes: Bytes = Bytes::copy_from_slice(record._id.as_bytes());
            bloom.insert(&key_bytes);

            if min_key.is_none() || key_bytes < *min_key.as_ref().unwrap() {
                min_key = Some(key_bytes.clone());
            }
            if max_key.is_none() || key_bytes > *max_key.as_ref().unwrap() {
                max_key = Some(key_bytes.clone());
            }

            // Confidence histogram bucket: clamp to [0, 99].
            let bucket = ((record._confidence * 100.0) as usize).min(99);
            histogram[bucket] += 1;
        }

        let msgpack_len = msgpack_buf.len() as u32;
        let record_count = records.len() as u32;

        // ----------------------------------------------------------------
        // Assemble the page binary layout.
        // ----------------------------------------------------------------
        // 4 (record_count) + 4 (msgpack_len) + msgpack + confidences*4
        let total = 4 + 4 + msgpack_buf.len() + confidences.len() * 4;
        let mut buf = BytesMut::with_capacity(total);

        buf.put_u32_le(record_count);
        buf.put_u32_le(msgpack_len);
        buf.put_slice(&msgpack_buf);
        for c in &confidences {
            buf.put_f32_le(*c);
        }

        let page_bytes = buf.freeze();

        let header = PageHeader {
            min_key: min_key.unwrap(),
            max_key: max_key.unwrap(),
            row_count: record_count,
            bloom_filter: bloom,
            confidence_histogram: histogram,
        };

        Ok((header, page_bytes))
    }
}

// ---------------------------------------------------------------------------
// PageDecoder
// ---------------------------------------------------------------------------

/// Decodes a columnar page produced by [`PageEncoder`] back into [`FunRecord`]s.
pub struct PageDecoder;

impl PageDecoder {
    /// Decode `data` using `header` metadata and return the reconstructed records.
    ///
    /// Each record's `_confidence` is reattached from the confidence column
    /// after the msgpack payload is decoded.
    pub fn decode(header: &PageHeader, data: &Bytes) -> Result<Vec<FunRecord>> {
        if data.len() < 8 {
            return Err(anyhow::anyhow!("page data too short: {} bytes", data.len()));
        }

        // ----------------------------------------------------------------
        // Read framing.
        // ----------------------------------------------------------------
        let record_count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let msgpack_len  = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;

        // Validate lengths.
        let expected_min = 8 + msgpack_len + record_count * 4;
        if data.len() < expected_min {
            return Err(anyhow::anyhow!(
                "page data too short: expected at least {} bytes, got {}",
                expected_min,
                data.len()
            ));
        }

        if record_count as u32 != header.row_count {
            return Err(anyhow::anyhow!(
                "header row_count ({}) != encoded record_count ({})",
                header.row_count,
                record_count
            ));
        }

        let msgpack_section = &data[8..8 + msgpack_len];
        let conf_section    = &data[8 + msgpack_len..8 + msgpack_len + record_count * 4];

        // ----------------------------------------------------------------
        // Decode confidence values.
        // ----------------------------------------------------------------
        let mut confidences = Vec::with_capacity(record_count);
        for i in 0..record_count {
            let off = i * 4;
            let c = f32::from_le_bytes(conf_section[off..off + 4].try_into().unwrap());
            confidences.push(c);
        }

        // ----------------------------------------------------------------
        // Decode msgpack records.
        //
        // rmp-serde doesn't expose a streaming cursor directly, so we use
        // rmpv + rmp_serde::from_read on a std::io::Cursor to walk the
        // concatenated msgpack frames one at a time.
        // ----------------------------------------------------------------
        let mut records = Vec::with_capacity(record_count);
        let mut cursor = std::io::Cursor::new(msgpack_section);

        for i in 0..record_count {
            let record: FunRecord = rmp_serde::from_read(&mut cursor)
                .map_err(|e| anyhow::anyhow!("decode record {}: {}", i, e))?;
            records.push(record);
        }

        if records.len() != record_count {
            return Err(anyhow::anyhow!(
                "decoded {} records, expected {}",
                records.len(),
                record_count
            ));
        }

        // ----------------------------------------------------------------
        // Reattach confidence values.
        // ----------------------------------------------------------------
        for (record, conf) in records.iter_mut().zip(confidences.iter()) {
            record._confidence = *conf;
        }

        Ok(records)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::FunRecordBuilder;

    // -----------------------------------------------------------------------
    // BloomFilter
    // -----------------------------------------------------------------------

    #[test]
    fn test_bloom_insert_contains() {
        let mut bf = BloomFilter::new(100, 0.01);
        let key = b"hello_world";
        assert!(!bf.contains(key), "key must not be present before insert");
        bf.insert(key);
        assert!(bf.contains(key), "key must be present after insert");
    }

    #[test]
    fn test_bloom_false_positive_rate() {
        // Insert 1000 keys, then test 1000 non-inserted keys.
        // FPR must be < 2%.
        let n = 1000usize;
        let mut bf = BloomFilter::new(n, 0.01);

        // Insert keys: "key_0" .. "key_999"
        for i in 0..n {
            let key = format!("key_{}", i);
            bf.insert(key.as_bytes());
        }

        // All inserted keys must be found (no false negatives).
        for i in 0..n {
            let key = format!("key_{}", i);
            assert!(bf.contains(key.as_bytes()), "false negative for key_{}", i);
        }

        // Test non-inserted keys: "nkey_0" .. "nkey_999"
        let mut false_positives = 0usize;
        for i in 0..n {
            let key = format!("nkey_{}", i);
            if bf.contains(key.as_bytes()) {
                false_positives += 1;
            }
        }

        let fpr = false_positives as f64 / n as f64;
        assert!(
            fpr < 0.02,
            "false positive rate {} >= 2% ({} / {})",
            fpr,
            false_positives,
            n
        );
    }

    // -----------------------------------------------------------------------
    // PageEncoder / PageDecoder roundtrip
    // -----------------------------------------------------------------------

    fn make_record(conf: f32) -> FunRecord {
        FunRecordBuilder::new("pages").confidence(conf).build()
    }

    #[test]
    fn test_page_roundtrip_three_records() {
        let r1 = make_record(0.9);
        let r2 = make_record(0.5);
        let r3 = make_record(0.1);
        let records = vec![r1, r2, r3];

        let (header, page) = PageEncoder::encode(&records).expect("encode must succeed");
        assert_eq!(header.row_count, 3);

        let decoded = PageDecoder::decode(&header, &page).expect("decode must succeed");
        assert_eq!(decoded.len(), 3, "must decode 3 records");

        for (orig, dec) in records.iter().zip(decoded.iter()) {
            // Verify identity.
            assert_eq!(orig._id, dec._id, "record ID mismatch");
            assert_eq!(orig._collection, dec._collection);
            // Confidence is reattached from the column.
            assert!((orig._confidence - dec._confidence).abs() < 1e-6,
                "confidence mismatch: orig={} dec={}", orig._confidence, dec._confidence);
        }
    }

    #[test]
    fn test_page_roundtrip_with_payload() {
        let records: Vec<FunRecord> = (0..10)
            .map(|i| {
                FunRecordBuilder::new("test")
                    .data(vec![i as u8; 64])
                    .vector("emb", vec![i as f32 * 0.1; 32])
                    .confidence(i as f32 * 0.1)
                    .build()
            })
            .collect();

        let (header, page) = PageEncoder::encode(&records).expect("encode");
        let decoded = PageDecoder::decode(&header, &page).expect("decode");

        assert_eq!(decoded.len(), records.len());
        for (orig, dec) in records.iter().zip(decoded.iter()) {
            assert_eq!(orig._id, dec._id);
            assert_eq!(orig.data, dec.data);
            assert_eq!(orig._vectors, dec._vectors);
        }
    }

    // -----------------------------------------------------------------------
    // Bloom filter integration in PageEncoder
    // -----------------------------------------------------------------------

    #[test]
    fn test_page_bloom_filter_contains_all_ids() {
        let records: Vec<FunRecord> = (0..20).map(|_| make_record(0.5)).collect();
        let (header, _page) = PageEncoder::encode(&records).expect("encode");

        for r in &records {
            let key = r._id.as_bytes().as_slice();
            assert!(
                header.bloom_filter.contains(key),
                "bloom filter must contain record id {}",
                r._id
            );
        }
    }

    // -----------------------------------------------------------------------
    // Confidence histogram
    // -----------------------------------------------------------------------

    #[test]
    fn test_confidence_histogram_bucket_counts() {
        // Create 100 records: record i has confidence = i as f32 / 100.0
        // So record 0 → 0.00, record 1 → 0.01, ..., record 99 → 0.99.
        // Each bucket i should contain exactly 1 record.
        let records: Vec<FunRecord> = (0u32..100)
            .map(|i| {
                FunRecordBuilder::new("hist")
                    .confidence(i as f32 / 100.0)
                    .build()
            })
            .collect();

        let (header, _page) = PageEncoder::encode(&records).expect("encode");

        for bucket in 0..100 {
            assert_eq!(
                header.confidence_histogram[bucket],
                1,
                "bucket {} must contain exactly 1 record, got {}",
                bucket,
                header.confidence_histogram[bucket]
            );
        }
    }

    #[test]
    fn test_confidence_histogram_full_confidence() {
        // 10 records all at confidence=1.0 → all land in bucket 99.
        let records: Vec<FunRecord> = (0..10).map(|_| make_record(1.0)).collect();
        let (header, _page) = PageEncoder::encode(&records).expect("encode");
        assert_eq!(header.confidence_histogram[99], 10);
        for bucket in 0..99 {
            assert_eq!(header.confidence_histogram[bucket], 0, "bucket {} should be 0", bucket);
        }
    }

    // -----------------------------------------------------------------------
    // Key range
    // -----------------------------------------------------------------------

    #[test]
    fn test_page_key_range() {
        let records: Vec<FunRecord> = (0..5).map(|_| make_record(0.5)).collect();
        let (header, _page) = PageEncoder::encode(&records).expect("encode");

        // min_key <= max_key
        assert!(
            header.min_key <= header.max_key,
            "min_key must be <= max_key"
        );

        // Every record ID must be within [min_key, max_key].
        for r in &records {
            let key = Bytes::copy_from_slice(r._id.as_bytes());
            assert!(key >= header.min_key, "key below min_key");
            assert!(key <= header.max_key, "key above max_key");
        }
    }
}
