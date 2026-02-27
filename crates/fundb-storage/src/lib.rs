// fundb-storage — in-memory + on-disk storage engine for FunDB.

use fundb_core::Timestamp;

/// Log Sequence Number — monotonically increasing, uniquely identifies a WAL entry.
pub type Lsn = u64;

/// A read/write transaction token.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub txn_id:      u64,
    pub snapshot_ts: Timestamp,
}

// Storage modules — per CONTRIBUTING.md, STORY-2-4 (this story) adds all pub mod declarations.
pub mod memtable;
pub mod wal;
pub mod sstable;
pub mod block_cache;
pub mod mvcc;

pub use memtable::{ImmutableMemTable, MemTable};
pub use wal::{Wal, WalEntry};
pub use sstable::{SstableReader, SstableWriter, merge_sstables};
pub use block_cache::{BlockCache, CacheKey, SharedBlockCache, new_shared_cache};
pub use mvcc::MvccStore;
