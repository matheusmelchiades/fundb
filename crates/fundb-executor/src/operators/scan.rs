//! Scan operators — full-collection scan and vector-similarity scan.

use std::sync::Arc;

use anyhow::Result;
use fundb_core::RecordKey;
use fundb_storage::LsmTree;

use crate::batch::RecordBatch;

// ---------------------------------------------------------------------------
// ScanOperator
// ---------------------------------------------------------------------------

/// Physical operator that performs a full range scan over a single collection.
///
/// It issues a key-range scan on the underlying [`LsmTree`], covering all
/// record keys whose `collection` field equals `self.collection`.
pub struct ScanOperator {
    pub collection: String,
    pub lsm: Arc<LsmTree>,
}

impl ScanOperator {
    /// Construct a new `ScanOperator` for the given collection.
    pub fn new(collection: String, lsm: Arc<LsmTree>) -> Self {
        ScanOperator { collection, lsm }
    }

    /// Execute the scan and return all records in the collection.
    pub async fn execute(&self) -> Result<RecordBatch> {
        // Build inclusive key range that covers the entire collection.
        // The lower bound has the minimum possible id ([0u8; 16]).
        // The upper bound uses a collection name with the ASCII DEL character
        // (0x7F) appended, which sorts after every valid collection-prefixed key,
        // giving an upper bound that is inclusive of all records whose collection
        // field equals `self.collection`.
        let from = RecordKey {
            collection: self.collection.clone(),
            id: [0u8; 16],
        };
        let to = RecordKey {
            collection: format!("{}\x7f", self.collection),
            id: [0xffu8; 16],
        };

        let pairs = self.lsm.scan(&self.collection, &from, &to).await?;

        let mut batch = RecordBatch::new();
        batch.extend(pairs);
        Ok(batch)
    }
}

// ---------------------------------------------------------------------------
// VectorScanOperator
// ---------------------------------------------------------------------------

/// Physical operator that performs an approximate-nearest-neighbour scan over
/// a named vector field stored inside each `FunRecord`.
///
/// Because FunDB's `FunRecord` stores named embeddings in
/// `_vectors: HashMap<String, Vec<f32>>`, this operator:
///
/// 1. Performs a full collection scan via [`ScanOperator`].
/// 2. For each record, retrieves the vector stored under `vector_field` from
///    `record._vectors`.
/// 3. Computes the cosine distance between that vector and `self.query`.
/// 4. Keeps records whose cosine distance is strictly less than `self.threshold`.
pub struct VectorScanOperator {
    pub collection: String,
    pub vector_field: String,
    pub query: Vec<f32>,
    pub threshold: f32,
    pub lsm: Arc<LsmTree>,
}

impl VectorScanOperator {
    /// Construct a new `VectorScanOperator`.
    pub fn new(
        collection: String,
        vector_field: String,
        query: Vec<f32>,
        threshold: f32,
        lsm: Arc<LsmTree>,
    ) -> Self {
        VectorScanOperator {
            collection,
            vector_field,
            query,
            threshold,
            lsm,
        }
    }

    /// Execute the vector scan, returning records whose named vector embedding
    /// is within `threshold` cosine distance of `self.query`.
    pub async fn execute(&self) -> Result<RecordBatch> {
        // Step 1: full collection scan.
        let full = ScanOperator::new(self.collection.clone(), Arc::clone(&self.lsm))
            .execute()
            .await?;

        // Step 2: filter by cosine distance.
        let mut result = RecordBatch::new();
        for (key, record) in full.keys.into_iter().zip(full.records.into_iter()) {
            // Look up the named vector field.
            let vec = match record._vectors.get(&self.vector_field) {
                Some(v) => v,
                None => continue, // field absent — skip this record
            };

            let dist = cosine_distance(&self.query, vec);
            if dist < self.threshold {
                result.push(key, record);
            }
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Cosine distance helper
// ---------------------------------------------------------------------------

/// Compute the cosine distance between two vectors.
///
/// Cosine distance = `1.0 - dot(a, b) / (|a| * |b|)`.
///
/// Returns `1.0` (maximum distance) when either vector has zero norm or the
/// vectors have different lengths.
fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }

    1.0 - dot / (norm_a * norm_b)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_distance_identical() {
        let v = vec![1.0f32, 0.0, 0.0];
        // Identical vectors → distance 0.
        let d = cosine_distance(&v, &v);
        assert!((d - 0.0).abs() < 1e-5, "expected ~0, got {}", d);
    }

    #[test]
    fn test_cosine_distance_orthogonal() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        let d = cosine_distance(&a, &b);
        assert!((d - 1.0).abs() < 1e-5, "expected ~1, got {}", d);
    }

    #[test]
    fn test_cosine_distance_zero_vector() {
        let a = vec![0.0f32, 0.0];
        let b = vec![1.0f32, 0.0];
        assert_eq!(cosine_distance(&a, &b), 1.0);
    }

    #[test]
    fn test_cosine_distance_length_mismatch() {
        let a = vec![1.0f32];
        let b = vec![1.0f32, 0.0];
        assert_eq!(cosine_distance(&a, &b), 1.0);
    }
}
