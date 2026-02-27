//! RecordBatch — a columnar batch of FunRecord rows.
//!
//! Physical operators produce and consume `RecordBatch` values.  The batch
//! keeps keys and records in parallel `Vec`s so that callers can iterate or
//! zip them cheaply.

use fundb_core::{FunRecord, RecordKey};
use serde::{Deserialize, Serialize};

/// A columnar batch of records returned by physical operators.
///
/// Keys and records are stored in parallel `Vec`s in insertion order.
/// `keys[i]` is the `RecordKey` for `records[i]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordBatch {
    /// Row keys in insertion order.
    pub keys: Vec<RecordKey>,
    /// Corresponding records (parallel to `keys`).
    pub records: Vec<FunRecord>,
}

impl RecordBatch {
    /// Create an empty batch.
    pub fn new() -> Self {
        RecordBatch::default()
    }

    /// Append a single `(key, record)` pair to the batch.
    pub fn push(&mut self, key: RecordKey, record: FunRecord) {
        self.keys.push(key);
        self.records.push(record);
    }

    /// Number of rows in the batch.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the batch contains no rows.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Extend the batch from an iterator of `(RecordKey, FunRecord)` pairs.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = (RecordKey, FunRecord)>) {
        for (key, record) in iter {
            self.push(key, record);
        }
    }

    /// Merge two batches together, returning a new batch with `self` rows
    /// followed by `other` rows.
    pub fn merge(mut self, other: RecordBatch) -> RecordBatch {
        self.keys.extend(other.keys);
        self.records.extend(other.records);
        self
    }

    /// Truncate the batch to at most `n` rows, dropping any excess rows.
    pub fn truncate(&mut self, n: usize) {
        self.keys.truncate(n);
        self.records.truncate(n);
    }
}
