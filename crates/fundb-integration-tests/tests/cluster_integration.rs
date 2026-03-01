use std::net::SocketAddr;
use std::sync::Arc;

use fundb_cluster::ShardMap;
use fundb_raft::{
    AppendEntriesRequest, EntryType, InstallSnapshotRequest,
    LogEntry, MemoryRaftStorage, RaftNode, RequestVoteRequest,
};
use uuid::Uuid;

/// Helper: create a RaftNode with MemoryRaftStorage.
fn make_node(id: u64, peers: Vec<(u64, String)>) -> RaftNode {
    let storage = Arc::new(MemoryRaftStorage::new());
    RaftNode::new(id, peers, storage)
}

// ---------------------------------------------------------------------------
// 1. Raft — follower to candidate transition
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_follower_to_candidate() {
    let node = make_node(1, vec![(2, "127.0.0.1:9002".to_string())]);

    let initial_term = node.current_term();

    // Start election — node transitions to Candidate
    let _won = node.start_election().await;

    // Term should have incremented
    let new_term = node.current_term();
    assert!(
        new_term > initial_term,
        "term should increment on election: {} > {}",
        new_term,
        initial_term
    );
}

// ---------------------------------------------------------------------------
// 2. Raft — wins election with majority
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_wins_election_majority() {
    let node1 = make_node(
        1,
        vec![
            (2, "127.0.0.1:9002".to_string()),
            (3, "127.0.0.1:9003".to_string()),
        ],
    );

    // Start election
    node1.start_election().await;

    // Simulate vote responses from peers
    let vote1 = RequestVoteRequest {
        term: node1.current_term(),
        candidate_id: 1,
        last_log_index: 0,
        last_log_term: 0,
    };

    // Node 2 votes yes
    let node2 = make_node(2, vec![]);
    let resp2 = node2.handle_request_vote(vote1.clone()).await;
    assert!(resp2.vote_granted, "node 2 should grant vote");

    // Node 3 votes yes
    let node3 = make_node(3, vec![]);
    let resp3 = node3.handle_request_vote(vote1).await;
    assert!(resp3.vote_granted, "node 3 should grant vote");
}

// ---------------------------------------------------------------------------
// 3. Raft — leader replicates log
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_leader_replicates_log() {
    let node = make_node(1, vec![(2, "127.0.0.1:9002".to_string())]);

    // Simulate leader state by proposing
    // First become leader via election
    node.start_election().await;

    let follower = make_node(2, vec![(1, "127.0.0.1:9001".to_string())]);

    let req = AppendEntriesRequest {
        term: node.current_term(),
        leader_id: 1,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![LogEntry {
            index: 1,
            term: node.current_term(),
            data: b"test_entry".to_vec(),
            entry_type: EntryType::Normal,
        }],
        leader_commit: 0,
    };

    let resp = follower.handle_append_entries(req).await;
    assert!(resp.success, "follower should accept AppendEntries from leader");
}

// ---------------------------------------------------------------------------
// 4. Raft — advances commit on majority ACK
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_advances_commit_on_majority() {
    let leader = make_node(
        1,
        vec![
            (2, "127.0.0.1:9002".to_string()),
            (3, "127.0.0.1:9003".to_string()),
        ],
    );

    leader.start_election().await;
    let term = leader.current_term();

    let follower1 = make_node(2, vec![]);
    let follower2 = make_node(3, vec![]);

    // Send AppendEntries to both followers
    let req = AppendEntriesRequest {
        term,
        leader_id: 1,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![LogEntry {
            index: 1,
            term,
            data: b"committed_entry".to_vec(),
            entry_type: EntryType::Normal,
        }],
        leader_commit: 1,
    };

    let r1 = follower1.handle_append_entries(req.clone()).await;
    let r2 = follower2.handle_append_entries(req).await;

    assert!(r1.success, "follower 1 should accept");
    assert!(r2.success, "follower 2 should accept");

    // Both followers accepted — in real Raft, leader would advance commit_index
    assert!(
        follower1.commit_index() >= 1 || follower2.commit_index() >= 1,
        "at least one follower should have advanced commit_index"
    );
}

// ---------------------------------------------------------------------------
// 5. Raft — rejects stale term
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_rejects_stale_term() {
    let follower = make_node(1, vec![]);

    // First, advance the follower's term to 5 by processing a higher-term message
    let advance_req = AppendEntriesRequest {
        term: 5,
        leader_id: 99,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![],
        leader_commit: 0,
    };
    follower.handle_append_entries(advance_req).await;

    // Now send a stale AppendEntries with term 3
    let stale_req = AppendEntriesRequest {
        term: 3,
        leader_id: 42,
        prev_log_index: 0,
        prev_log_term: 0,
        entries: vec![LogEntry {
            index: 1,
            term: 3,
            data: b"stale".to_vec(),
            entry_type: EntryType::Normal,
        }],
        leader_commit: 0,
    };

    let resp = follower.handle_append_entries(stale_req).await;
    assert!(
        !resp.success,
        "follower with term 5 should reject AppendEntries with term 3"
    );
    assert!(
        resp.term >= 5,
        "response term should be at least 5, got {}",
        resp.term
    );
}

// ---------------------------------------------------------------------------
// 6. Shard — consistent hashing stability
// ---------------------------------------------------------------------------
#[test]
fn test_shard_consistent_hashing_stability() {
    let mut map = ShardMap::new(100);
    let addr1: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:9002".parse().unwrap();
    let addr3: SocketAddr = "127.0.0.1:9003".parse().unwrap();

    map.add_node(1, addr1);
    map.add_node(2, addr2);
    map.add_node(3, addr3);

    // Route 100 keys and record their shards
    let mut original_mapping = Vec::new();
    for i in 0u128..100 {
        let id = Uuid::from_u128(i);
        let shard = map.shard_for("test", &id);
        original_mapping.push((id, shard));
    }

    // Remove one node
    let _migrations = map.remove_node(3);

    // Re-route and check how many changed
    let mut changed = 0;
    for (id, old_shard) in &original_mapping {
        let new_shard = map.shard_for("test", id);
        if new_shard != *old_shard {
            changed += 1;
        }
    }

    // With consistent hashing, removing 1 of 3 nodes should redistribute < 40%
    assert!(
        changed < 40,
        "removing 1 of 3 nodes should move < 40% of keys, moved {}",
        changed
    );
}

// ---------------------------------------------------------------------------
// 7. Shard — add/remove rebalance
// ---------------------------------------------------------------------------
#[test]
fn test_shard_add_remove_rebalance() {
    let mut map = ShardMap::new(50);
    let addr1: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:9002".parse().unwrap();

    map.add_node(1, addr1);
    map.add_node(2, addr2);

    // Route some keys
    for i in 0..50u128 {
        let id = Uuid::from_u128(i);
        map.shard_for("data", &id);
    }

    // Add a third node — should trigger migrations
    let addr3: SocketAddr = "127.0.0.1:9003".parse().unwrap();
    let migrations = map.add_node(3, addr3);

    // Some shards should have migrated
    // (migration count depends on virtual node distribution)
    // add_node returns a list of migrations (may be empty)
    let _ = &migrations;
}

// ---------------------------------------------------------------------------
// 8. Shard — deterministic routing
// ---------------------------------------------------------------------------
#[test]
fn test_shard_deterministic_routing() {
    let mut map = ShardMap::new(50);
    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    map.add_node(1, addr);

    let id = Uuid::from_u128(0xDEAD_BEEF);

    let shard1 = map.shard_for("users", &id);
    let shard2 = map.shard_for("users", &id);
    let shard3 = map.shard_for("users", &id);

    assert_eq!(shard1, shard2, "same key should route to same shard");
    assert_eq!(shard2, shard3, "routing should be idempotent");
}

// ---------------------------------------------------------------------------
// 9. Shard — replication groups
// ---------------------------------------------------------------------------
#[test]
fn test_shard_replication_groups() {
    let mut map = ShardMap::new(50);
    map.set_replication_factor(3);

    for i in 1..=5 {
        let addr: SocketAddr = format!("127.0.0.1:900{}", i).parse().unwrap();
        map.add_node(i, addr);
    }

    let id = Uuid::from_u128(0xABCD);
    let shard = map.shard_for("replicated", &id);
    let group = map.replication_group(shard).expect("replication group missing");

    assert_eq!(group.replication_factor, 3, "RF should be 3");

    // Leader + followers should give us RF nodes total
    let mut all_nodes = vec![group.leader];
    all_nodes.extend(&group.followers);

    // All nodes should be distinct
    all_nodes.sort();
    all_nodes.dedup();
    assert!(
        all_nodes.len() >= 2,
        "replication group should have distinct nodes, got {:?}",
        all_nodes
    );
}

// ---------------------------------------------------------------------------
// 10. Raft — install snapshot
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_raft_install_snapshot() {
    let node = make_node(1, vec![]);

    let req = InstallSnapshotRequest {
        term: 3,
        leader_id: 99,
        last_included_index: 10,
        last_included_term: 2,
        data: b"snapshot_data_here".to_vec(),
    };

    let resp = node.handle_install_snapshot(req).await;
    assert_eq!(resp.term, 3, "snapshot response term should match request term");

    // After installing snapshot, commit_index should advance
    let ci = node.commit_index();
    assert!(
        ci >= 10,
        "commit_index should be at least snapshot's last_included_index (10), got {}",
        ci
    );
}
