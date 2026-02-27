// crates/fundb-indexes/src/temporal.rs
//
// STORY-3-4: Temporal Interval Index
//
// Enables point and range queries over half-open intervals [valid_from, valid_to),
// supporting ML dataset reproducibility by reconstructing which records were
// "true" during any given time window.

use uuid::Uuid;
use fundb_core::Timestamp;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Internal representation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Interval {
    id:         Uuid,
    valid_from: Timestamp,
    valid_to:   Timestamp,
}

// ---------------------------------------------------------------------------
// Public index type
// ---------------------------------------------------------------------------

/// A temporal index over half-open intervals `[valid_from, valid_to)`.
///
/// Uses a simple linear scan for correctness. Can be replaced with an
/// interval tree or segment tree later for O(log n + k) query performance.
pub struct TemporalIndex {
    intervals: Vec<Interval>,
}

impl TemporalIndex {
    /// Create an empty `TemporalIndex`.
    pub fn new() -> Self {
        Self {
            intervals: Vec::new(),
        }
    }

    /// Insert a new interval `[valid_from, valid_to)` associated with `id`.
    ///
    /// `valid_to = i64::MAX` represents an open-ended ("currently valid") record.
    pub fn insert(&mut self, id: Uuid, valid_from: Timestamp, valid_to: Timestamp) -> Result<()> {
        self.intervals.push(Interval { id, valid_from, valid_to });
        Ok(())
    }

    /// Return all record ids whose interval contains the point `at`.
    ///
    /// An interval `[valid_from, valid_to)` contains `at` iff:
    /// `valid_from <= at && at < valid_to`.
    pub fn point_query(&self, at: Timestamp) -> Vec<Uuid> {
        self.intervals
            .iter()
            .filter(|iv| iv.valid_from <= at && at < iv.valid_to)
            .map(|iv| iv.id)
            .collect()
    }

    /// Return all record ids whose interval overlaps with the half-open window
    /// `[from, to)`.
    ///
    /// Two half-open intervals `[a, b)` and `[c, d)` overlap iff:
    /// `a < d && c < b`.
    ///
    /// Applied here: interval `[valid_from, valid_to)` overlaps `[from, to)` iff:
    /// `valid_from < to && valid_to > from`.
    pub fn range_query(&self, from: Timestamp, to: Timestamp) -> Vec<Uuid> {
        self.intervals
            .iter()
            .filter(|iv| iv.valid_from < to && iv.valid_to > from)
            .map(|iv| iv.id)
            .collect()
    }

    /// Remove all intervals associated with `id`.
    ///
    /// Returns `Ok(())` whether or not any intervals were found.
    pub fn delete(&mut self, id: Uuid) -> Result<()> {
        self.intervals.retain(|iv| iv.id != id);
        Ok(())
    }

    /// Return the number of intervals currently stored in the index.
    pub fn len(&self) -> usize {
        self.intervals.len()
    }
}

impl Default for TemporalIndex {
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

    // Helper: create a deterministic but unique Uuid from a u128 seed.
    fn uid(seed: u128) -> Uuid {
        Uuid::from_u128(seed)
    }

    /// 1. Basic point queries with three non-overlapping / partially-overlapping intervals.
    #[test]
    fn test_point_query_basic() {
        let id_a = uid(1);
        let id_b = uid(2);
        let id_c = uid(3);

        let mut idx = TemporalIndex::new();
        idx.insert(id_a, 100, 200).unwrap(); // [100, 200)
        idx.insert(id_b, 150, 300).unwrap(); // [150, 300)
        idx.insert(id_c,  50, 100).unwrap(); // [50,  100)

        // point_query(150) -> A and B (150 is in [100,200) and [150,300))
        let mut result = idx.point_query(150);
        result.sort();
        let mut expected = vec![id_a, id_b];
        expected.sort();
        assert_eq!(result, expected, "point_query(150) should return A and B");

        // point_query(99) -> C only (99 is in [50,100))
        let result = idx.point_query(99);
        assert_eq!(result, vec![id_c], "point_query(99) should return C only");

        // point_query(200) -> B only (200 is NOT in [100,200) because the interval is half-open)
        let result = idx.point_query(200);
        assert_eq!(result, vec![id_b], "point_query(200) should return B only");
    }

    /// 2. Range query overlap semantics with three intervals.
    #[test]
    fn test_range_query_overlap() {
        let id_a = uid(10);
        let id_b = uid(11);
        let id_c = uid(12);

        let mut idx = TemporalIndex::new();
        idx.insert(id_a,   0, 100).unwrap(); // [0,   100)
        idx.insert(id_b,  50, 150).unwrap(); // [50,  150)
        idx.insert(id_c, 200, 300).unwrap(); // [200, 300)

        // range_query(80, 210) should overlap all three:
        //   A: 0 < 210 && 100 > 80   -> true
        //   B: 50 < 210 && 150 > 80  -> true
        //   C: 200 < 210 && 300 > 80 -> true
        let mut result = idx.range_query(80, 210);
        result.sort();
        let mut expected = vec![id_a, id_b, id_c];
        expected.sort();
        assert_eq!(result, expected, "range_query(80, 210) should return A, B, and C");
    }

    /// 3. Open-ended interval (valid_to = i64::MAX) represents a currently-active record.
    #[test]
    fn test_open_ended_interval() {
        let id_a = uid(20);

        let mut idx = TemporalIndex::new();
        idx.insert(id_a, 100, i64::MAX).unwrap(); // [100, ∞)

        // Any point >= 100 should be included.
        let result = idx.point_query(999_999_999);
        assert!(result.contains(&id_a), "open-ended interval should include far-future point");

        // A point before valid_from should not be included.
        let result = idx.point_query(99);
        assert!(!result.contains(&id_a), "open-ended interval should not include point before valid_from");
    }

    /// 4. Delete removes the interval so it no longer appears in queries.
    #[test]
    fn test_delete() {
        let id_a = uid(30);
        let id_b = uid(31);
        let id_c = uid(32);

        let mut idx = TemporalIndex::new();
        idx.insert(id_a, 0, 500).unwrap();
        idx.insert(id_b, 0, 500).unwrap();
        idx.insert(id_c, 0, 500).unwrap();

        assert_eq!(idx.len(), 3);

        idx.delete(id_b).unwrap();

        assert_eq!(idx.len(), 2, "len() should be 2 after deleting one interval");

        // point_query at a time covered by all three original intervals
        let result = idx.point_query(250);
        assert!(!result.contains(&id_b), "deleted id should not appear in point_query");
        assert!(result.contains(&id_a), "remaining id A should still appear");
        assert!(result.contains(&id_c), "remaining id C should still appear");
    }

    /// 5. Adjacent intervals do not overlap — [0, 100) and [100, 200) share no points.
    #[test]
    fn test_no_overlap() {
        let id_a = uid(40);

        let mut idx = TemporalIndex::new();
        idx.insert(id_a, 0, 100).unwrap(); // [0, 100)

        // range_query(100, 200): A requires valid_from < 200 (0 < 200 ✓)
        // AND valid_to > 100 (100 > 100 ✗) -> no overlap.
        let result = idx.range_query(100, 200);
        assert!(result.is_empty(), "range_query(100, 200) should return empty for interval [0, 100)");
    }
}
