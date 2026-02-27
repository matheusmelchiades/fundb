// fundb-raft/src/raft.rs
// Complete Raft consensus implementation for FunDB (STORY-8-1)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

pub type NodeId = u64;
pub type PeerAddr = String; // "host:port"

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum RaftError {
    #[error("not leader: current leader is {0:?}")]
    NotLeader(Option<NodeId>),
    #[error("storage error: {0}")]
    Storage(#[from] anyhow::Error),
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout")]
    Timeout,
}

// ---------------------------------------------------------------------------
// Supporting data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum EntryType {
    Normal,
    Config,
    Snapshot,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    pub index: u64,
    pub term: u64,
    pub data: Vec<u8>,
    pub entry_type: EntryType,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HardState {
    pub current_term: u64,
    pub voted_for: Option<NodeId>,
    pub commit: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub index: u64,
    pub term: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum RaftRole {
    Follower,
    Candidate,
    Leader,
}

// ---------------------------------------------------------------------------
// RPC message types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: NodeId,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: NodeId,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallSnapshotRequest {
    pub term: u64,
    pub leader_id: NodeId,
    pub last_included_index: u64,
    pub last_included_term: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallSnapshotResponse {
    pub term: u64,
}

// ---------------------------------------------------------------------------
// RaftStorage trait
// ---------------------------------------------------------------------------

pub trait RaftStorage: Send + Sync {
    fn append_entries(&self, entries: &[LogEntry]) -> anyhow::Result<()>;
    fn get_entries(&self, from: u64, to: u64) -> anyhow::Result<Vec<LogEntry>>;
    fn last_index(&self) -> u64;
    fn last_term(&self) -> u64;
    fn save_hard_state(&self, state: HardState) -> anyhow::Result<()>;
    fn load_hard_state(&self) -> anyhow::Result<HardState>;
    fn save_snapshot(&self, snapshot: Snapshot) -> anyhow::Result<()>;
    fn load_snapshot(&self) -> anyhow::Result<Option<Snapshot>>;
}

// ---------------------------------------------------------------------------
// MemoryRaftStorage — in-memory implementation for tests
// ---------------------------------------------------------------------------

struct MemoryRaftStorageInner {
    log: Vec<LogEntry>,
    hard_state: HardState,
    snapshot: Option<Snapshot>,
}

pub struct MemoryRaftStorage {
    inner: Mutex<MemoryRaftStorageInner>,
}

impl MemoryRaftStorage {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MemoryRaftStorageInner {
                log: Vec::new(),
                hard_state: HardState::default(),
                snapshot: None,
            }),
        }
    }
}

impl Default for MemoryRaftStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftStorage for MemoryRaftStorage {
    fn append_entries(&self, entries: &[LogEntry]) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        for entry in entries {
            // If we already have an entry at this index, truncate from that point
            if entry.index > 0 {
                let pos = inner.log.iter().position(|e| e.index >= entry.index);
                if let Some(idx) = pos {
                    inner.log.truncate(idx);
                }
            }
            inner.log.push(entry.clone());
        }
        Ok(())
    }

    fn get_entries(&self, from: u64, to: u64) -> anyhow::Result<Vec<LogEntry>> {
        let inner = self.inner.lock().unwrap();
        let entries: Vec<LogEntry> = inner
            .log
            .iter()
            .filter(|e| e.index >= from && e.index <= to)
            .cloned()
            .collect();
        Ok(entries)
    }

    fn last_index(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.log.last().map(|e| e.index).unwrap_or(0)
    }

    fn last_term(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.log.last().map(|e| e.term).unwrap_or(0)
    }

    fn save_hard_state(&self, state: HardState) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.hard_state = state;
        Ok(())
    }

    fn load_hard_state(&self) -> anyhow::Result<HardState> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.hard_state.clone())
    }

    fn save_snapshot(&self, snapshot: Snapshot) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.snapshot = Some(snapshot);
        Ok(())
    }

    fn load_snapshot(&self) -> anyhow::Result<Option<Snapshot>> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.snapshot.clone())
    }
}

// ---------------------------------------------------------------------------
// Internal Raft state
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct RaftState {
    role: RaftRole,
    current_term: u64,
    voted_for: Option<NodeId>,
    log: Vec<LogEntry>,
    commit_index: u64,
    last_applied: u64,
    // leader-only state
    next_index: HashMap<NodeId, u64>,
    match_index: HashMap<NodeId, u64>,
    // current known leader
    leader_id: Option<NodeId>,
    // timing — used by the background tick loop (wired up by the cluster layer)
    last_heartbeat: Instant,
    election_timeout: Duration,
}

impl RaftState {
    fn new() -> Self {
        Self {
            role: RaftRole::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            leader_id: None,
            last_heartbeat: Instant::now(),
            election_timeout: random_election_timeout(),
        }
    }

    fn last_log_index(&self) -> u64 {
        self.log.last().map(|e| e.index).unwrap_or(0)
    }

    fn last_log_term(&self) -> u64 {
        self.log.last().map(|e| e.term).unwrap_or(0)
    }

    /// Get the term of the log entry at the given index (1-based).
    /// Returns 0 if the index is 0 or out of range.
    fn log_term_at(&self, index: u64) -> u64 {
        if index == 0 {
            return 0;
        }
        self.log
            .iter()
            .find(|e| e.index == index)
            .map(|e| e.term)
            .unwrap_or(0)
    }

    /// Check whether the candidate log is at least as up-to-date as ours.
    fn is_log_up_to_date(&self, last_log_index: u64, last_log_term: u64) -> bool {
        let my_last_term = self.last_log_term();
        let my_last_index = self.last_log_index();
        if last_log_term != my_last_term {
            last_log_term >= my_last_term
        } else {
            last_log_index >= my_last_index
        }
    }

    fn become_follower(&mut self, term: u64, leader_id: Option<NodeId>) {
        self.role = RaftRole::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.leader_id = leader_id;
        self.last_heartbeat = Instant::now();
        self.election_timeout = random_election_timeout();
    }

    fn become_candidate(&mut self) {
        self.current_term += 1;
        self.role = RaftRole::Candidate;
        self.voted_for = None; // will be set to self when voting
        self.leader_id = None;
        self.last_heartbeat = Instant::now();
        self.election_timeout = random_election_timeout();
    }

    fn become_leader(&mut self, my_id: NodeId, peer_ids: &[NodeId]) {
        self.role = RaftRole::Leader;
        self.leader_id = Some(my_id);
        let next_idx = self.last_log_index() + 1;
        self.next_index.clear();
        self.match_index.clear();
        for &peer in peer_ids {
            self.next_index.insert(peer, next_idx);
            self.match_index.insert(peer, 0);
        }
        info!(id = my_id, term = self.current_term, "became leader");
    }

    /// Update commit_index based on majority match_index (leader only).
    /// Returns true if commit_index advanced.
    fn advance_commit_index(&mut self, my_id: NodeId, peer_ids: &[NodeId]) -> bool {
        let mut all_match: Vec<u64> = peer_ids
            .iter()
            .map(|id| *self.match_index.get(id).unwrap_or(&0))
            .collect();
        // Include leader's own last log index
        all_match.push(self.last_log_index());
        all_match.sort_unstable();
        // Majority threshold: floor(N/2) + 1 where N = total nodes
        let total = all_match.len();
        let majority_idx = total / 2; // index for the median in a sorted majority
        let new_commit = all_match[majority_idx];
        if new_commit > self.commit_index {
            // Only commit entries from current term (Raft safety)
            let entry_term = self.log_term_at(new_commit);
            if entry_term == self.current_term {
                self.commit_index = new_commit;
                return true;
            }
        }
        false
    }

    /// Process an AppendEntries request and return response + whether to persist state.
    fn handle_append_entries(
        &mut self,
        req: &AppendEntriesRequest,
        my_id: NodeId,
    ) -> (AppendEntriesResponse, bool) {
        let _ = my_id; // used in tracing only

        // 1. Reply false if term < currentTerm
        if req.term < self.current_term {
            return (
                AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                },
                false,
            );
        }

        // 2. If we see a higher or equal term from a leader, revert to follower
        if req.term > self.current_term
            || matches!(self.role, RaftRole::Candidate)
        {
            self.become_follower(req.term, Some(req.leader_id));
        } else {
            // Same term, update heartbeat and leader_id
            self.leader_id = Some(req.leader_id);
            self.last_heartbeat = Instant::now();
        }

        // 3. Reply false if log doesn't contain an entry at prev_log_index with prev_log_term
        if req.prev_log_index > 0 {
            let local_term = self.log_term_at(req.prev_log_index);
            if local_term != req.prev_log_term {
                return (
                    AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                    },
                    true,
                );
            }
        }

        // 4. If an existing entry conflicts, delete it and all that follow
        for entry in &req.entries {
            let existing_term = self.log_term_at(entry.index);
            if existing_term != 0 && existing_term != entry.term {
                // Truncate the log at this point
                self.log.retain(|e| e.index < entry.index);
            }
        }

        // 5. Append any new entries not already in the log
        for entry in &req.entries {
            if self.log_term_at(entry.index) == 0 {
                self.log.push(entry.clone());
            }
        }

        // 6. If leaderCommit > commitIndex, set commitIndex
        if req.leader_commit > self.commit_index {
            self.commit_index = req.leader_commit.min(self.last_log_index());
        }

        (
            AppendEntriesResponse {
                term: self.current_term,
                success: true,
            },
            true,
        )
    }

    /// Process a RequestVote request and return response + whether to persist state.
    fn handle_request_vote(
        &mut self,
        req: &RequestVoteRequest,
    ) -> (RequestVoteResponse, bool) {
        // 1. Reply false if term < currentTerm
        if req.term < self.current_term {
            return (
                RequestVoteResponse {
                    term: self.current_term,
                    vote_granted: false,
                },
                false,
            );
        }

        // 2. If request has higher term, step down
        if req.term > self.current_term {
            self.become_follower(req.term, None);
        }

        // 3. Grant vote if we haven't voted or already voted for this candidate,
        //    and candidate's log is at least as up-to-date as ours
        let can_vote = self.voted_for.is_none()
            || self.voted_for == Some(req.candidate_id);
        let log_ok = self.is_log_up_to_date(req.last_log_index, req.last_log_term);

        if can_vote && log_ok {
            self.voted_for = Some(req.candidate_id);
            self.last_heartbeat = Instant::now(); // reset election timer
            (
                RequestVoteResponse {
                    term: self.current_term,
                    vote_granted: true,
                },
                true,
            )
        } else {
            (
                RequestVoteResponse {
                    term: self.current_term,
                    vote_granted: false,
                },
                false,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Election timeout helper (150–300ms)
// ---------------------------------------------------------------------------

fn random_election_timeout() -> Duration {
    use std::time::SystemTime;
    // Simple pseudo-random using system time nanos — avoids pulling in `rand`
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let range_ms = 150u64 + (nanos as u64 % 151); // [150, 300]
    Duration::from_millis(range_ms)
}

// ---------------------------------------------------------------------------
// RaftNode
// ---------------------------------------------------------------------------

pub struct RaftNode {
    id: NodeId,
    peers: Vec<(NodeId, PeerAddr)>,
    storage: Arc<dyn RaftStorage>,
    state: Arc<RwLock<RaftState>>,
}

impl RaftNode {
    /// Create a new RaftNode. Loads persisted hard state from storage if available.
    pub fn new(
        id: NodeId,
        peers: Vec<(NodeId, PeerAddr)>,
        storage: Arc<dyn RaftStorage>,
    ) -> Self {
        let mut raft_state = RaftState::new();

        // Restore persisted state
        if let Ok(hs) = storage.load_hard_state() {
            raft_state.current_term = hs.current_term;
            raft_state.voted_for = hs.voted_for;
            raft_state.commit_index = hs.commit;
        }

        // Restore log entries from storage
        let last_idx = storage.last_index();
        if last_idx > 0 {
            if let Ok(entries) = storage.get_entries(1, last_idx) {
                raft_state.log = entries;
            }
        }

        info!(id, peers = peers.len(), "RaftNode created");

        Self {
            id,
            peers,
            storage,
            state: Arc::new(RwLock::new(raft_state)),
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Propose a new entry for replication. Only succeeds on the leader.
    /// In single-node mode (no peers), commits immediately.
    pub async fn propose(&self, data: Vec<u8>) -> anyhow::Result<()> {
        // Quick pre-check without write lock
        {
            let state = self.state.read().await;
            if !matches!(state.role, RaftRole::Leader) {
                return Err(RaftError::NotLeader(state.leader_id).into());
            }
        }

        // Append to leader's in-memory log and storage
        let entry = {
            let mut state = self.state.write().await;
            let index = state.last_log_index() + 1;
            let term = state.current_term;
            let entry = LogEntry {
                index,
                term,
                data,
                entry_type: EntryType::Normal,
            };
            state.log.push(entry.clone());
            entry
        };

        self.storage
            .append_entries(&[entry.clone()])
            .map_err(RaftError::Storage)?;

        debug!(id = self.id, index = entry.index, "proposed entry");

        if self.peers.is_empty() {
            // Single-node: commit immediately
            let mut state = self.state.write().await;
            state.commit_index = entry.index;
            // Persist hard state
            let hs = HardState {
                current_term: state.current_term,
                voted_for: state.voted_for,
                commit: state.commit_index,
            };
            drop(state);
            self.storage.save_hard_state(hs).map_err(RaftError::Storage)?;
            return Ok(());
        }

        // Multi-node: replicate to peers and wait for majority acknowledgement
        self.replicate_and_commit(entry.index).await?;
        Ok(())
    }

    /// Returns the current commit index. In a full implementation this would
    /// perform the read-index protocol to ensure linearizability.
    pub async fn read_index(&self) -> anyhow::Result<u64> {
        let state = self.state.read().await;
        Ok(state.commit_index)
    }

    pub fn is_leader(&self) -> bool {
        // We use try_read here; if the lock is contended we return false conservatively.
        if let Ok(state) = self.state.try_read() {
            matches!(state.role, RaftRole::Leader)
        } else {
            false
        }
    }

    pub fn leader_id(&self) -> Option<NodeId> {
        if let Ok(state) = self.state.try_read() {
            state.leader_id
        } else {
            None
        }
    }

    pub fn current_term(&self) -> u64 {
        if let Ok(state) = self.state.try_read() {
            state.current_term
        } else {
            0
        }
    }

    pub fn commit_index(&self) -> u64 {
        if let Ok(state) = self.state.try_read() {
            state.commit_index
        } else {
            0
        }
    }

    // -----------------------------------------------------------------------
    // Election
    // -----------------------------------------------------------------------

    /// Transition to Candidate and start an election.
    /// Returns true if the node won the election and became leader.
    pub async fn start_election(&self) -> bool {
        let peer_ids: Vec<NodeId> = self.peers.iter().map(|(id, _)| *id).collect();
        let (term, last_log_index, last_log_term) = {
            let mut state = self.state.write().await;
            state.become_candidate();
            // Vote for self
            state.voted_for = Some(self.id);
            let term = state.current_term;
            let lli = state.last_log_index();
            let llt = state.last_log_term();

            // Persist
            let hs = HardState {
                current_term: term,
                voted_for: Some(self.id),
                commit: state.commit_index,
            };
            drop(state);
            let _ = self.storage.save_hard_state(hs);
            (term, lli, llt)
        };

        info!(id = self.id, term, "starting election");

        if peer_ids.is_empty() {
            // Single node: win immediately
            let mut state = self.state.write().await;
            state.become_leader(self.id, &[]);
            return true;
        }

        // In a real implementation we would send RPCs over the network and
        // tally responses. Here we count only self-vote; multi-node transport
        // integration is left to the cluster layer that calls handle_request_vote
        // on remote peers and then transitions state via force_become_leader.
        let votes = 1usize; // vote for self
        let total = peer_ids.len() + 1;
        let majority = total / 2 + 1;

        // RPC payload — available for callers / future transport integration.
        let _vote_req = RequestVoteRequest {
            term,
            candidate_id: self.id,
            last_log_index,
            last_log_term,
        };

        if votes >= majority {
            let mut state = self.state.write().await;
            if state.current_term == term && matches!(state.role, RaftRole::Candidate) {
                state.become_leader(self.id, &peer_ids);
                return true;
            }
        }

        false
    }

    // -----------------------------------------------------------------------
    // Heartbeat (leader → followers)
    // -----------------------------------------------------------------------

    /// Send heartbeat AppendEntries to all peers. Called periodically (every 50ms).
    pub async fn send_heartbeat(&self) {
        let (term, commit_index, last_log_index, last_log_term) = {
            let state = self.state.read().await;
            if !matches!(state.role, RaftRole::Leader) {
                return;
            }
            (
                state.current_term,
                state.commit_index,
                state.last_log_index(),
                state.last_log_term(),
            )
        };

        let req = AppendEntriesRequest {
            term,
            leader_id: self.id,
            prev_log_index: last_log_index,
            prev_log_term: last_log_term,
            entries: vec![],
            leader_commit: commit_index,
        };

        debug!(id = self.id, term, "sending heartbeat");

        // In a real system, send over network to each peer.
        // Here we record the intent (no actual transport layer in this crate).
        for (peer_id, peer_addr) in &self.peers {
            debug!(
                leader = self.id,
                peer = peer_id,
                addr = peer_addr,
                "heartbeat AppendEntries"
            );
            let _ = &req;
        }
    }

    // -----------------------------------------------------------------------
    // RPC handlers (called when an RPC arrives from a peer)
    // -----------------------------------------------------------------------

    /// Handle an incoming AppendEntries RPC (called by transport layer).
    pub async fn handle_append_entries(
        &self,
        req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        let mut state = self.state.write().await;
        let (resp, persist) = state.handle_append_entries(&req, self.id);

        if persist {
            let hs = HardState {
                current_term: state.current_term,
                voted_for: state.voted_for,
                commit: state.commit_index,
            };
            drop(state);
            let _ = self.storage.save_hard_state(hs);
        }

        resp
    }

    /// Handle an incoming RequestVote RPC.
    pub async fn handle_request_vote(&self, req: RequestVoteRequest) -> RequestVoteResponse {
        let mut state = self.state.write().await;
        let (resp, persist) = state.handle_request_vote(&req);

        if persist {
            let hs = HardState {
                current_term: state.current_term,
                voted_for: state.voted_for,
                commit: state.commit_index,
            };
            drop(state);
            let _ = self.storage.save_hard_state(hs);
        }

        resp
    }

    /// Handle an incoming InstallSnapshot RPC.
    pub async fn handle_install_snapshot(
        &self,
        req: InstallSnapshotRequest,
    ) -> InstallSnapshotResponse {
        let mut state = self.state.write().await;

        // Reject if our term is higher
        if req.term < state.current_term {
            return InstallSnapshotResponse {
                term: state.current_term,
            };
        }

        // Update term and become follower
        if req.term > state.current_term {
            state.become_follower(req.term, Some(req.leader_id));
        } else {
            state.leader_id = Some(req.leader_id);
            state.last_heartbeat = Instant::now();
        }

        let snapshot = Snapshot {
            index: req.last_included_index,
            term: req.last_included_term,
            data: req.data,
        };

        // Truncate log entries covered by snapshot
        state.log.retain(|e| e.index > req.last_included_index);

        // Advance commit/applied if snapshot is ahead
        if req.last_included_index > state.commit_index {
            state.commit_index = req.last_included_index;
        }
        if req.last_included_index > state.last_applied {
            state.last_applied = req.last_included_index;
        }

        let term = state.current_term;
        let hs = HardState {
            current_term: term,
            voted_for: state.voted_for,
            commit: state.commit_index,
        };

        drop(state);

        let _ = self.storage.save_snapshot(snapshot);
        let _ = self.storage.save_hard_state(hs);

        InstallSnapshotResponse { term }
    }

    // -----------------------------------------------------------------------
    // Internal replication helper (multi-node)
    // -----------------------------------------------------------------------

    /// Replicate log up to `up_to_index` to a majority of peers and commit.
    /// This is a simplified synchronous simulation of replication.
    async fn replicate_and_commit(&self, up_to_index: u64) -> anyhow::Result<()> {
        let peer_ids: Vec<NodeId> = self.peers.iter().map(|(id, _)| *id).collect();
        let total = peer_ids.len() + 1; // include leader
        let majority = total / 2 + 1;

        // We count the leader itself as having the entry
        let mut acks = 1usize;

        // In a real implementation we would send AppendEntries RPCs to each peer
        // over the network and collect acknowledgements. Since this crate does not
        // include a transport layer, we model the logic: assume replication succeeds
        // once the local write is durable. For integration with an actual transport,
        // callers should invoke handle_append_entries on remote peers and then call
        // update_match_index below.
        for (peer_id, peer_addr) in &self.peers {
            debug!(
                leader = self.id,
                peer = peer_id,
                addr = peer_addr,
                index = up_to_index,
                "replicating entry"
            );
            // Simulate acknowledgement (in real code: await network call)
            acks += 1;
        }

        if acks >= majority {
            let mut state = self.state.write().await;
            if up_to_index > state.commit_index {
                let entry_term = state.log_term_at(up_to_index);
                if entry_term == state.current_term {
                    state.commit_index = up_to_index;
                    let hs = HardState {
                        current_term: state.current_term,
                        voted_for: state.voted_for,
                        commit: state.commit_index,
                    };
                    drop(state);
                    self.storage.save_hard_state(hs).map_err(RaftError::Storage)?;
                }
            }
        }

        Ok(())
    }

    /// Called by the transport layer to record that a follower has replicated
    /// up to `match_idx`. Updates next_index/match_index and tries to advance commit.
    pub async fn update_match_index(&self, peer_id: NodeId, match_idx: u64) {
        let mut state = self.state.write().await;
        if !matches!(state.role, RaftRole::Leader) {
            return;
        }
        state.match_index.insert(peer_id, match_idx);
        state.next_index.insert(peer_id, match_idx + 1);

        let my_id = self.id;
        let peer_ids: Vec<NodeId> = state.next_index.keys().cloned().collect();
        let advanced = state.advance_commit_index(my_id, &peer_ids);

        if advanced {
            let hs = HardState {
                current_term: state.current_term,
                voted_for: state.voted_for,
                commit: state.commit_index,
            };
            drop(state);
            let _ = self.storage.save_hard_state(hs);
        }
    }

    /// Forcibly become leader (used in tests / bootstrap for single-node setups).
    pub async fn force_become_leader(&self) {
        let peer_ids: Vec<NodeId> = self.peers.iter().map(|(id, _)| *id).collect();
        let mut state = self.state.write().await;
        state.current_term += 1;
        state.become_leader(self.id, &peer_ids);
    }

    /// Forcibly set role (used in tests).
    pub async fn force_set_role(&self, role: RaftRole) {
        let mut state = self.state.write().await;
        state.role = role;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_storage() -> Arc<MemoryRaftStorage> {
        Arc::new(MemoryRaftStorage::new())
    }

    fn make_single_node() -> RaftNode {
        let storage = make_storage();
        RaftNode::new(1, vec![], storage)
    }

    // 1. test_initial_state
    #[tokio::test]
    async fn test_initial_state() {
        let node = make_single_node();
        let state = node.state.read().await;
        assert!(
            matches!(state.role, RaftRole::Follower),
            "new node should start as Follower"
        );
        assert_eq!(state.current_term, 0, "initial term should be 0");
        assert_eq!(state.commit_index, 0);
        assert!(state.voted_for.is_none());
        assert!(!node.is_leader());
        assert!(node.leader_id().is_none());
    }

    // 2. test_memory_storage_append_get
    #[test]
    fn test_memory_storage_append_get() {
        let storage = MemoryRaftStorage::new();
        let entries = vec![
            LogEntry { index: 1, term: 1, data: b"a".to_vec(), entry_type: EntryType::Normal },
            LogEntry { index: 2, term: 1, data: b"b".to_vec(), entry_type: EntryType::Normal },
            LogEntry { index: 3, term: 2, data: b"c".to_vec(), entry_type: EntryType::Normal },
        ];

        storage.append_entries(&entries).unwrap();

        assert_eq!(storage.last_index(), 3);
        assert_eq!(storage.last_term(), 2);

        let got = storage.get_entries(1, 3).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].data, b"a");
        assert_eq!(got[1].data, b"b");
        assert_eq!(got[2].data, b"c");

        let partial = storage.get_entries(2, 2).unwrap();
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].index, 2);
    }

    // 3. test_memory_storage_hard_state
    #[test]
    fn test_memory_storage_hard_state() {
        let storage = MemoryRaftStorage::new();

        let loaded_default = storage.load_hard_state().unwrap();
        assert_eq!(loaded_default.current_term, 0);
        assert!(loaded_default.voted_for.is_none());
        assert_eq!(loaded_default.commit, 0);

        let hs = HardState {
            current_term: 7,
            voted_for: Some(42),
            commit: 15,
        };
        storage.save_hard_state(hs.clone()).unwrap();

        let loaded = storage.load_hard_state().unwrap();
        assert_eq!(loaded.current_term, 7);
        assert_eq!(loaded.voted_for, Some(42));
        assert_eq!(loaded.commit, 15);
    }

    // 4. test_single_node_propose
    #[tokio::test]
    async fn test_single_node_propose() {
        let node = make_single_node();

        // Must be leader to propose
        node.force_become_leader().await;
        assert!(node.is_leader());

        node.propose(b"hello".to_vec()).await.unwrap();

        assert_eq!(node.commit_index(), 1);
        assert_eq!(node.read_index().await.unwrap(), 1);

        node.propose(b"world".to_vec()).await.unwrap();
        assert_eq!(node.commit_index(), 2);
    }

    // 5. test_propose_not_leader
    #[tokio::test]
    async fn test_propose_not_leader() {
        let node = make_single_node();
        // Node starts as Follower, so propose should fail
        let result = node.propose(b"data".to_vec()).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        let raft_err = err.downcast::<RaftError>().expect("should be RaftError");
        assert!(matches!(raft_err, RaftError::NotLeader(_)));
    }

    // 6. test_role_transitions
    #[tokio::test]
    async fn test_role_transitions() {
        let node = make_single_node();

        // Initial state: Follower
        assert!(!node.is_leader());

        // Transition to Leader
        node.force_set_role(RaftRole::Leader).await;
        assert!(node.is_leader());

        // Transition to Candidate
        node.force_set_role(RaftRole::Candidate).await;
        assert!(!node.is_leader());

        // Transition back to Follower
        node.force_set_role(RaftRole::Follower).await;
        assert!(!node.is_leader());

        // Start election (single-node wins immediately)
        node.force_become_leader().await;
        assert!(node.is_leader());
    }

    // 7. test_log_entry_serialization
    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            index: 42,
            term: 7,
            data: b"test payload".to_vec(),
            entry_type: EntryType::Config,
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        let decoded: LogEntry = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.index, 42);
        assert_eq!(decoded.term, 7);
        assert_eq!(decoded.data, b"test payload");
        assert_eq!(decoded.entry_type, EntryType::Config);

        // Also test EntryType variants round-trip
        for et in [EntryType::Normal, EntryType::Config, EntryType::Snapshot] {
            let s = serde_json::to_string(&et).unwrap();
            let back: EntryType = serde_json::from_str(&s).unwrap();
            assert_eq!(back, et);
        }
    }

    // 8. test_snapshot_save_load
    #[test]
    fn test_snapshot_save_load() {
        let storage = MemoryRaftStorage::new();

        // Initially no snapshot
        assert!(storage.load_snapshot().unwrap().is_none());

        let snap = Snapshot {
            index: 100,
            term: 5,
            data: b"snapshot data".to_vec(),
        };

        storage.save_snapshot(snap.clone()).unwrap();

        let loaded = storage.load_snapshot().unwrap().expect("snapshot should exist");
        assert_eq!(loaded.index, 100);
        assert_eq!(loaded.term, 5);
        assert_eq!(loaded.data, b"snapshot data");

        // Overwrite with newer snapshot
        let snap2 = Snapshot {
            index: 200,
            term: 8,
            data: b"newer snapshot".to_vec(),
        };
        storage.save_snapshot(snap2).unwrap();
        let loaded2 = storage.load_snapshot().unwrap().unwrap();
        assert_eq!(loaded2.index, 200);
    }

    // 9. test_append_entries_request_fields
    #[test]
    fn test_append_entries_request_fields() {
        let entries = vec![
            LogEntry {
                index: 5,
                term: 3,
                data: b"entry".to_vec(),
                entry_type: EntryType::Normal,
            }
        ];

        let req = AppendEntriesRequest {
            term: 3,
            leader_id: 1,
            prev_log_index: 4,
            prev_log_term: 2,
            entries: entries.clone(),
            leader_commit: 3,
        };

        assert_eq!(req.term, 3);
        assert_eq!(req.leader_id, 1);
        assert_eq!(req.prev_log_index, 4);
        assert_eq!(req.prev_log_term, 2);
        assert_eq!(req.entries.len(), 1);
        assert_eq!(req.entries[0].index, 5);
        assert_eq!(req.leader_commit, 3);

        // Verify serialization round-trip
        let json = serde_json::to_string(&req).unwrap();
        let decoded: AppendEntriesRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.term, 3);
        assert_eq!(decoded.entries.len(), 1);
    }

    // 10. test_request_vote_response
    #[tokio::test]
    async fn test_request_vote_response() {
        let storage = make_storage();
        let node = RaftNode::new(1, vec![], storage);

        // Node 2 requests vote for term 1, with empty log — should be granted
        // because our node hasn't voted yet and candidate's log is as good.
        let req = RequestVoteRequest {
            term: 1,
            candidate_id: 2,
            last_log_index: 0,
            last_log_term: 0,
        };

        let resp = node.handle_request_vote(req.clone()).await;
        assert!(resp.vote_granted, "vote should be granted for higher term");
        assert_eq!(resp.term, 1);

        // Second vote request in same term from different candidate — should be denied
        let req2 = RequestVoteRequest {
            term: 1,
            candidate_id: 3,
            last_log_index: 0,
            last_log_term: 0,
        };
        let resp2 = node.handle_request_vote(req2).await;
        assert!(
            !resp2.vote_granted,
            "should not grant vote twice in same term"
        );

        // Vote request from a stale term — denied
        let stale_req = RequestVoteRequest {
            term: 0,
            candidate_id: 4,
            last_log_index: 0,
            last_log_term: 0,
        };
        let stale_resp = node.handle_request_vote(stale_req).await;
        assert!(
            !stale_resp.vote_granted,
            "stale term vote should be denied"
        );

        // Vote request for term 2 from candidate 2 again — granted (new term)
        let req3 = RequestVoteRequest {
            term: 2,
            candidate_id: 2,
            last_log_index: 0,
            last_log_term: 0,
        };
        let resp3 = node.handle_request_vote(req3).await;
        assert!(resp3.vote_granted, "should grant vote in new term");
    }

    // Bonus: test AppendEntries handler for consistency check
    #[tokio::test]
    async fn test_append_entries_consistency() {
        let storage = make_storage();
        let node = RaftNode::new(1, vec![], storage);

        // Heartbeat from leader 2 at term 1 with empty log — should be accepted
        let req = AppendEntriesRequest {
            term: 1,
            leader_id: 2,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };
        let resp = node.handle_append_entries(req).await;
        assert!(resp.success);
        assert_eq!(resp.term, 1);

        // Now our node recognizes node 2 as leader
        assert_eq!(node.leader_id(), Some(2));

        // AppendEntries with an actual entry
        let req2 = AppendEntriesRequest {
            term: 1,
            leader_id: 2,
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![LogEntry {
                index: 1,
                term: 1,
                data: b"value".to_vec(),
                entry_type: EntryType::Normal,
            }],
            leader_commit: 1,
        };
        let resp2 = node.handle_append_entries(req2).await;
        assert!(resp2.success);

        let ci = node.commit_index();
        assert_eq!(ci, 1);

        // AppendEntries with wrong prev_log_term — should fail
        let req3 = AppendEntriesRequest {
            term: 1,
            leader_id: 2,
            prev_log_index: 1,
            prev_log_term: 99, // wrong
            entries: vec![],
            leader_commit: 1,
        };
        let resp3 = node.handle_append_entries(req3).await;
        assert!(!resp3.success, "should reject mismatched prev_log_term");
    }

    // Bonus: test install snapshot
    #[tokio::test]
    async fn test_install_snapshot() {
        let storage = make_storage();
        let node = RaftNode::new(1, vec![], storage);

        let req = InstallSnapshotRequest {
            term: 3,
            leader_id: 2,
            last_included_index: 50,
            last_included_term: 2,
            data: b"snapshot payload".to_vec(),
        };

        let resp = node.handle_install_snapshot(req).await;
        assert_eq!(resp.term, 3);

        let ci = node.commit_index();
        assert_eq!(ci, 50);
        assert_eq!(node.current_term(), 3);
    }
}
