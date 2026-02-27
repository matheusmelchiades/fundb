// crates/fundb-indexes/src/confidence.rs
//
// STORY-3-5: Confidence Index
//
// A lightweight index over per-record confidence scores (0.0–1.0).
// Supports:
//   - Insert / update of (UUID, confidence) pairs
//   - Range queries returning all UUIDs whose confidence falls in [min, max]
//   - 100-bucket histogram for confidence distribution analysis

use uuid::Uuid;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

/// An in-memory index mapping record UUIDs to their confidence scores.
///
/// Confidence values must be in the range `[0.0, 1.0]`. The index stores
/// them as a flat `Vec<(Uuid, f32)>` for simplicity; all query operations
/// are linear scans, which is suitable for moderate collection sizes and
/// can be upgraded to a sorted structure later.
pub struct ConfidenceIndex {
    entries: Vec<(Uuid, f32)>,
}

impl ConfidenceIndex {
    /// Create a new, empty `ConfidenceIndex`.
    pub fn new() -> Self {
        ConfidenceIndex {
            entries: Vec::new(),
        }
    }

    /// Insert a `(id, confidence)` pair.
    ///
    /// Duplicate ids are allowed (each insert appends a new entry). Use
    /// [`update`] to change an existing record's confidence in place.
    pub fn insert(&mut self, id: Uuid, confidence: f32) -> Result<()> {
        self.entries.push((id, confidence));
        Ok(())
    }

    /// Return all record UUIDs whose confidence is in the closed interval
    /// `[min, max]`.
    pub fn range(&self, min: f32, max: f32) -> Vec<Uuid> {
        self.entries
            .iter()
            .filter(|(_, c)| *c >= min && *c <= max)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return a 100-bucket histogram of confidence values.
    ///
    /// Bucket `i` counts records with confidence in the half-open interval
    /// `[i/100.0, (i+1)/100.0)`. The last bucket (i = 99) also captures
    /// the value 1.0 (clamped to index 99).
    pub fn histogram(&self) -> [u32; 100] {
        let mut buckets = [0u32; 100];
        for (_, confidence) in &self.entries {
            let idx = (*confidence * 100.0).floor() as usize;
            let idx = idx.min(99); // clamp to handle confidence == 1.0
            buckets[idx] += 1;
        }
        buckets
    }

    /// Update the confidence value for the first entry whose UUID matches `id`.
    ///
    /// If no entry with `id` exists this is a no-op and returns `Ok(())`.
    pub fn update(&mut self, id: Uuid, new_confidence: f32) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|(eid, _)| *eid == id) {
            entry.1 = new_confidence;
        }
        Ok(())
    }

    /// Return the total number of entries in the index.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for ConfidenceIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn uid(seed: u128) -> Uuid {
        Uuid::from_u128(seed)
    }

    // -----------------------------------------------------------------------
    // 1. insert_range — 10 records [0.0..0.9], range(0.5, 1.0) returns 5
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_range() {
        let mut idx = ConfidenceIndex::new();

        // Insert records with confidences 0.0, 0.1, 0.2, ..., 0.9.
        for i in 0..10u128 {
            idx.insert(uid(i), i as f32 * 0.1).unwrap();
        }

        assert_eq!(idx.len(), 10);

        // range(0.5, 1.0) should return records with confidence 0.5, 0.6, 0.7, 0.8, 0.9.
        let result = idx.range(0.5, 1.0);
        assert_eq!(result.len(), 5, "range(0.5, 1.0) should return 5 records; got {:?}", result);

        // Verify the returned UUIDs correspond to seeds 5..9.
        let expected_seeds: std::collections::HashSet<u128> = (5..10).collect();
        let result_seeds: std::collections::HashSet<u128> = result
            .iter()
            .map(|id| id.as_u128())
            .collect();
        assert_eq!(result_seeds, expected_seeds, "wrong UUIDs returned");
    }

    // -----------------------------------------------------------------------
    // 2. histogram — 100 records each in a distinct bucket
    // -----------------------------------------------------------------------
    #[test]
    fn test_histogram() {
        let mut idx = ConfidenceIndex::new();

        // Insert 100 records with confidence i/100.0 for i in 0..100.
        for i in 0..100u128 {
            idx.insert(uid(i), i as f32 / 100.0).unwrap();
        }

        let hist = idx.histogram();

        for (i, &count) in hist.iter().enumerate() {
            assert_eq!(
                count, 1,
                "bucket {} should have exactly 1 entry, got {}",
                i, count
            );
        }
    }

    // -----------------------------------------------------------------------
    // 3. update — change confidence from 0.3 to 0.8, then range(0.7, 1.0) finds it
    // -----------------------------------------------------------------------
    #[test]
    fn test_update() {
        let mut idx = ConfidenceIndex::new();

        let x = uid(42);
        idx.insert(x, 0.3).unwrap();

        // Should NOT appear in the high range before update.
        assert!(
            idx.range(0.7, 1.0).is_empty(),
            "record should not appear in range(0.7, 1.0) before update"
        );

        // Update to 0.8.
        idx.update(x, 0.8).unwrap();

        let result = idx.range(0.7, 1.0);
        assert_eq!(result.len(), 1, "record should appear in range(0.7, 1.0) after update");
        assert_eq!(result[0], x, "returned UUID should be X");
    }

    // -----------------------------------------------------------------------
    // 4. len tracks insertions
    // -----------------------------------------------------------------------
    #[test]
    fn test_len() {
        let mut idx = ConfidenceIndex::new();
        assert_eq!(idx.len(), 0);

        idx.insert(uid(1), 0.5).unwrap();
        idx.insert(uid(2), 0.8).unwrap();
        assert_eq!(idx.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 5. histogram clamps confidence == 1.0 to bucket 99
    // -----------------------------------------------------------------------
    #[test]
    fn test_histogram_clamps_one() {
        let mut idx = ConfidenceIndex::new();
        idx.insert(uid(1), 1.0).unwrap();

        let hist = idx.histogram();
        assert_eq!(hist[99], 1, "confidence 1.0 should land in bucket 99");
    }
}
