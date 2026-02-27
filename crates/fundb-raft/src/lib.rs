// fundb-raft — Raft consensus implementation (STORY-8-1)

pub mod raft;

pub use raft::{
    // Type aliases
    NodeId,
    PeerAddr,
    // Error
    RaftError,
    // Data types
    LogEntry,
    EntryType,
    HardState,
    Snapshot,
    RaftRole,
    // RPC messages
    AppendEntriesRequest,
    AppendEntriesResponse,
    RequestVoteRequest,
    RequestVoteResponse,
    InstallSnapshotRequest,
    InstallSnapshotResponse,
    // Storage
    RaftStorage,
    MemoryRaftStorage,
    // Node
    RaftNode,
};
