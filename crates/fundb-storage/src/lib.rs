// fundb-storage — in-memory + on-disk storage engine for FunDB.

use fundb_core::Timestamp;

/// Log Sequence Number — monotonically increasing, uniquely identifies a WAL entry.
pub type Lsn = u64;

/// A read/write transaction token.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub txn_id: u64,
    pub snapshot_ts: Timestamp,
}

// Storage modules — per CONTRIBUTING.md, STORY-2-4 (this story) adds all pub mod declarations.
pub mod block_cache;
pub mod compaction;
pub mod lsm;
pub mod memtable;
pub mod mvcc;
pub mod sstable;
pub mod wal;

pub use block_cache::{new_shared_cache, BlockCache, CacheKey, SharedBlockCache};
pub use compaction::CompactionPolicy;
pub use lsm::LsmTree;
pub use memtable::{ImmutableMemTable, MemTable};
pub use mvcc::MvccStore;
pub use sstable::{merge_sstables, SstableReader, SstableWriter};
pub use wal::{Wal, WalEntry};
