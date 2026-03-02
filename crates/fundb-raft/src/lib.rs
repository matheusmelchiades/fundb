// fundb-raft — Raft consensus implementation (STORY-8-1)

pub mod raft;

pub use raft::{
    // RPC messages
    AppendEntriesRequest,
    AppendEntriesResponse,
    EntryType,
    HardState,
    InstallSnapshotRequest,
    InstallSnapshotResponse,
    // Data types
    LogEntry,
    MemoryRaftStorage,
    // Type aliases
    NodeId,
    PeerAddr,
    // Error
    RaftError,
    // Node
    RaftNode,
    RaftRole,
    // Storage
    RaftStorage,
    RequestVoteRequest,
    RequestVoteResponse,
    Snapshot,
};
