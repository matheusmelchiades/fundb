// fundb-storage — shared types used across all storage stories.
//
// Per CONTRIBUTING.md merge order, STORY-2-4 (mvcc.rs) is last and will
// add `pub mod` declarations for all four storage modules.

use fundb_core::Timestamp;

/// Log Sequence Number — monotonically increasing, uniquely identifies a WAL entry.
pub type Lsn = u64;

/// A read/write transaction token.
///
/// Created by `MvccStore::begin_txn()`.  Encapsulates the snapshot timestamp
/// used for snapshot-isolation reads, and the transaction ID used to tag WAL entries.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Monotonically increasing transaction identifier.
    pub txn_id:      u64,
    /// System timestamp at which this transaction began (used for snapshot reads).
    pub snapshot_ts: Timestamp,
}
