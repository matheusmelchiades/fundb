//! Consistent-hash shard manager for FunDB cluster.
//!
//! Uses FNV-1a 64-bit hashing (implemented inline) to distribute
//! shards across physical nodes using a virtual-node ring.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public type aliases
// ---------------------------------------------------------------------------

pub type NodeId = u64;
pub type ShardId = u32;

// ---------------------------------------------------------------------------
// FNV-1a 64-bit — implemented inline, no external crate
// ---------------------------------------------------------------------------

const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

#[inline]
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// Describes a shard that has been reassigned from one node to another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShardMigration {
    pub shard_id: ShardId,
    pub from_node: NodeId,
    pub to_node: NodeId,
}

/// A replication group for a single shard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationGroup {
    pub shard_id: ShardId,
    pub leader: NodeId,
    pub followers: Vec<NodeId>,
    pub replication_factor: u8,
}

// ---------------------------------------------------------------------------
// Internal ring entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RingEntry {
    hash: u64,
    node_id: NodeId,
    shard_id: ShardId,
}

// ---------------------------------------------------------------------------
// ShardMap
// ---------------------------------------------------------------------------

/// Consistent-hash ring + shard-to-node routing table.
///
/// # Overview
/// * Each physical node gets `virtual_nodes` entries spread around a u64 ring.
/// * A `shard_id` is the `ShardId` stored in the ring entry at the successor
///   position for a given key hash.
/// * The mapping `shard_id → node_id` is maintained in a separate `HashMap`
///   so look-ups are O(1).
/// * Collection → shard sets are tracked lazily whenever `shard_for` is called.
pub struct ShardMap {
    /// Number of virtual nodes (ring replicas) per physical node.
    virtual_nodes: u32,
    /// Sorted ring entries.
    ring: Vec<RingEntry>,
    /// node_id → SocketAddr.
    node_addrs: HashMap<NodeId, SocketAddr>,
    /// shard_id → node_id (current owner).
    shard_owners: HashMap<ShardId, NodeId>,
    /// collection name → set of shard IDs that have been used by this collection.
    collection_shards: HashMap<String, HashSet<ShardId>>,
    /// Replication factor (default 1 = leader only).
    replication_factor: u8,
    /// Monotonically-increasing counter used to generate unique shard IDs.
    next_shard_id: ShardId,
}

impl ShardMap {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new, empty `ShardMap`.
    ///
    /// * `virtual_nodes` — number of ring entries per physical node (e.g. 150).
    pub fn new(virtual_nodes: u32) -> Self {
        ShardMap {
            virtual_nodes,
            ring: Vec::new(),
            node_addrs: HashMap::new(),
            shard_owners: HashMap::new(),
            collection_shards: HashMap::new(),
            replication_factor: 1,
            next_shard_id: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Routing
    // -----------------------------------------------------------------------

    /// Return the shard responsible for `(collection, record_id)`.
    ///
    /// The routing key is `"{collection}/{record_id_bytes}"` hashed with FNV-1a.
    /// When no nodes are present the method returns shard 0 (stable default).
    pub fn shard_for(&mut self, collection: &str, record_id: &Uuid) -> ShardId {
        if self.ring.is_empty() {
            return 0;
        }

        let key = self.routing_key(collection, record_id);
        let hash = fnv1a_64(key.as_bytes());
        let shard = self.successor_shard(hash);

        // Track which shards this collection has used.
        self.collection_shards
            .entry(collection.to_owned())
            .or_default()
            .insert(shard);

        shard
    }

    /// Return all shard IDs that have been used by `collection`.
    pub fn shards_for_collection(&self, collection: &str) -> Vec<ShardId> {
        self.collection_shards
            .get(collection)
            .map(|s| {
                let mut v: Vec<ShardId> = s.iter().copied().collect();
                v.sort_unstable();
                v
            })
            .unwrap_or_default()
    }

    /// Return the `SocketAddr` of the node that currently owns `shard`.
    ///
    /// # Panics
    /// Panics if the shard has no known owner *or* the owner's address is not
    /// registered.
    pub fn leader_addr(&self, shard: ShardId) -> SocketAddr {
        let node_id = self
            .shard_owners
            .get(&shard)
            .copied()
            .expect("shard has no known owner");
        *self
            .node_addrs
            .get(&node_id)
            .expect("node address not registered")
    }

    /// Return the `NodeId` that currently owns `shard`, or `None`.
    pub fn node_for_shard(&self, shard: ShardId) -> Option<NodeId> {
        self.shard_owners.get(&shard).copied()
    }

    /// Return all (NodeId, SocketAddr) pairs in insertion order.
    pub fn all_nodes(&self) -> Vec<(NodeId, SocketAddr)> {
        let mut nodes: Vec<(NodeId, SocketAddr)> = self
            .node_addrs
            .iter()
            .map(|(&id, &addr)| (id, addr))
            .collect();
        nodes.sort_unstable_by_key(|&(id, _)| id);
        nodes
    }

    // -----------------------------------------------------------------------
    // Topology changes
    // -----------------------------------------------------------------------

    /// Add a physical node to the ring.
    ///
    /// Inserts `virtual_nodes` ring entries for the node, then recomputes
    /// shard ownership.  Returns the list of shards that have migrated from
    /// their previous owner to the new node.
    pub fn add_node(&mut self, node: NodeId, addr: SocketAddr) -> Vec<ShardMigration> {
        if self.node_addrs.contains_key(&node) {
            return Vec::new();
        }

        self.node_addrs.insert(node, addr);

        // Add virtual-node ring entries.
        for replica in 0..self.virtual_nodes {
            let key = format!("{}#{}", node, replica);
            let hash = fnv1a_64(key.as_bytes());
            let shard_id = self.next_shard_id;
            self.next_shard_id += 1;

            self.ring.push(RingEntry {
                hash,
                node_id: node,
                shard_id,
            });
        }

        // Keep the ring sorted by hash.
        self.ring.sort_unstable_by_key(|e| e.hash);

        // Recompute ownership and collect migrations.
        self.recompute_ownership(Some(node))
    }

    /// Remove a physical node from the ring.
    ///
    /// All ring entries for the node are removed and affected shards are
    /// redistributed to their new successors.  Returns the migration list.
    pub fn remove_node(&mut self, node: NodeId) -> Vec<ShardMigration> {
        if !self.node_addrs.contains_key(&node) {
            return Vec::new();
        }

        // Capture shards currently owned by this node so we can build migrations.
        let owned_shards: HashSet<ShardId> = self
            .shard_owners
            .iter()
            .filter_map(|(&shard, &owner)| if owner == node { Some(shard) } else { None })
            .collect();

        // Remove ring entries for the node.
        self.ring.retain(|e| e.node_id != node);
        self.node_addrs.remove(&node);

        if self.ring.is_empty() {
            // No nodes remain — clear ownership.
            self.shard_owners.clear();
            return owned_shards
                .into_iter()
                .map(|shard_id| ShardMigration {
                    shard_id,
                    from_node: node,
                    to_node: 0,
                })
                .collect();
        }

        // Reassign each affected shard to its new successor.
        let mut migrations = Vec::new();
        for shard_id in owned_shards {
            // Find the ring entry for this shard to get its hash.
            if let Some(entry_hash) = self.ring.iter().find(|e| e.shard_id == shard_id)
                .map(|e| e.hash)
            {
                // The shard's ring entry is already for another node (node was removed).
                let new_owner = self
                    .ring
                    .iter()
                    .find(|e| e.hash >= entry_hash && e.node_id != node)
                    .or_else(|| self.ring.iter().find(|e| e.node_id != node))
                    .map(|e| e.node_id);

                if let Some(new_node) = new_owner {
                    self.shard_owners.insert(shard_id, new_node);
                    migrations.push(ShardMigration {
                        shard_id,
                        from_node: node,
                        to_node: new_node,
                    });
                }
            } else {
                // The shard_id is not currently in the ring (it was a virtual-node
                // entry for the removed node). Reassign via successor search.
                // Use hash of shard_id itself as a stable key.
                let hash = fnv1a_64(&shard_id.to_le_bytes());
                let new_owner = self.ring
                    .iter()
                    .find(|e| e.hash >= hash)
                    .or_else(|| self.ring.first())
                    .map(|e| e.node_id);

                if let Some(new_node) = new_owner {
                    self.shard_owners.insert(shard_id, new_node);
                    migrations.push(ShardMigration {
                        shard_id,
                        from_node: node,
                        to_node: new_node,
                    });
                }
            }
        }

        migrations
    }

    // -----------------------------------------------------------------------
    // Replication
    // -----------------------------------------------------------------------

    /// Return the replication group for a shard.
    ///
    /// The leader is the primary node for the shard.  Followers are the
    /// next `replication_factor - 1` distinct physical nodes on the ring
    /// after the leader's ring position.
    pub fn replication_group(&self, shard: ShardId) -> Option<ReplicationGroup> {
        let leader = *self.shard_owners.get(&shard)?;

        let mut followers: Vec<NodeId> = Vec::new();
        let rf = self.replication_factor as usize;

        if rf > 1 && self.ring.len() > 1 {
            // Find any ring entry that belongs to the leader so we can start
            // walking the ring from there.
            let start_pos = self
                .ring
                .iter()
                .position(|e| e.node_id == leader)
                .unwrap_or(0);

            let total = self.ring.len();
            let mut seen: HashSet<NodeId> = HashSet::new();
            seen.insert(leader);

            let mut idx = (start_pos + 1) % total;
            while followers.len() < rf - 1 && idx != start_pos {
                let candidate = self.ring[idx].node_id;
                if !seen.contains(&candidate) {
                    seen.insert(candidate);
                    followers.push(candidate);
                }
                idx = (idx + 1) % total;
                if idx == start_pos {
                    break;
                }
            }
        }

        Some(ReplicationGroup {
            shard_id: shard,
            leader,
            followers,
            replication_factor: self.replication_factor,
        })
    }

    /// Set the replication factor used when building `ReplicationGroup`s.
    pub fn set_replication_factor(&mut self, factor: u8) {
        self.replication_factor = factor;
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Build the routing key string for a `(collection, record_id)` pair.
    #[inline]
    fn routing_key(&self, collection: &str, record_id: &Uuid) -> String {
        format!("{}/{}", collection, record_id.as_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>())
    }

    /// Walk the sorted ring and return the `shard_id` of the first entry
    /// whose hash is `>= key_hash` (with wrap-around to index 0).
    fn successor_shard(&self, key_hash: u64) -> ShardId {
        // Binary search for the first entry with hash >= key_hash.
        let pos = self.ring.partition_point(|e| e.hash < key_hash);
        let entry = if pos < self.ring.len() {
            &self.ring[pos]
        } else {
            &self.ring[0] // wrap-around
        };
        entry.shard_id
    }

    /// After a topology change, recompute which node owns each ring entry's
    /// shard and return the list of migrations.
    ///
    /// * `new_node` — when `Some`, only shards whose successor changed *to*
    ///   `new_node` are emitted as migrations (the "stolen" shards).
    ///   When `None` (node removal), the caller handles migrations directly.
    fn recompute_ownership(&mut self, new_node: Option<NodeId>) -> Vec<ShardMigration> {
        let mut migrations = Vec::new();

        // For each ring entry the owning node IS the node_id stored in the entry
        // (each virtual node "owns" its shard directly).
        for i in 0..self.ring.len() {
            let shard_id = self.ring[i].shard_id;
            let new_owner = self.ring[i].node_id;

            match self.shard_owners.get(&shard_id).copied() {
                Some(old_owner) if old_owner != new_owner => {
                    // The shard changed owner.
                    if new_node.map_or(true, |n| new_owner == n) {
                        migrations.push(ShardMigration {
                            shard_id,
                            from_node: old_owner,
                            to_node: new_owner,
                        });
                    }
                    self.shard_owners.insert(shard_id, new_owner);
                }
                None => {
                    // First time this shard is being assigned — no migration needed.
                    self.shard_owners.insert(shard_id, new_owner);
                }
                _ => {} // owner unchanged
            }
        }

        migrations
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    // 1. Empty map — shard_for returns 0 by default.
    #[test]
    fn test_new_empty_map() {
        let mut m = ShardMap::new(150);
        let id = Uuid::from_u128(0xdeadbeef);
        assert_eq!(m.shard_for("col", &id), 0);
    }

    // 2. Single node — add_node returns no migrations (no previous owner).
    #[test]
    fn test_add_single_node() {
        let mut m = ShardMap::new(150);
        let migrations = m.add_node(1, addr(9001));
        // All entries are brand-new; no previous owner → no migration events.
        assert!(
            migrations.is_empty(),
            "expected no migrations for first node, got {:?}",
            migrations
        );
    }

    // 3. Two nodes — records split across both.
    #[test]
    fn test_add_two_nodes() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));

        let mut seen: HashSet<NodeId> = HashSet::new();
        for i in 0u64..200 {
            // Spread sequential i across the full 128-bit UUID space so that
            // consecutive keys produce diverse FNV routing hashes.
            let n = (i as u128)
                .wrapping_mul(6364136223846793005u128)
                .wrapping_add(1442695040888963407u128);
            let id = Uuid::from_u128(n);
            let shard = m.shard_for("col", &id);
            if let Some(owner) = m.node_for_shard(shard) {
                seen.insert(owner);
            }
        }
        assert!(
            seen.contains(&1) && seen.contains(&2),
            "expected records on both nodes, got {:?}",
            seen
        );
    }

    // 4. Consistent-hash stability — adding a third node re-maps ≤ 40 % of keys.
    #[test]
    fn test_consistent_hash_stability() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));

        const N: usize = 1000;
        let ids: Vec<Uuid> = (0..N as u128).map(Uuid::from_u128).collect();

        // Record baseline owner for each id.
        let before: Vec<NodeId> = ids
            .iter()
            .map(|id| {
                let s = m.shard_for("stability", id);
                m.node_for_shard(s).unwrap_or(0)
            })
            .collect();

        m.add_node(3, addr(9003));

        let moved = ids
            .iter()
            .zip(before.iter())
            .filter(|(id, &old_owner)| {
                let s = m.shard_for("stability", id);
                m.node_for_shard(s).unwrap_or(0) != old_owner
            })
            .count();

        let pct = moved as f64 / N as f64;
        assert!(
            pct <= 0.40,
            "too many keys re-mapped: {}/{} ({:.1}%)",
            moved,
            N,
            pct * 100.0
        );
    }

    // 5. Remove node — affected shards migrate to remaining nodes.
    #[test]
    fn test_remove_node() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));
        m.add_node(3, addr(9003));

        let migrations = m.remove_node(2);

        // All migrations must be FROM node 2.
        for mig in &migrations {
            assert_eq!(mig.from_node, 2, "expected from_node=2, got {:?}", mig);
            assert!(
                mig.to_node == 1 || mig.to_node == 3,
                "expected to_node in {{1,3}}, got {:?}",
                mig
            );
        }

        // Node 2 must no longer appear as an owner.
        for (_, &owner) in m.shard_owners.iter() {
            assert_ne!(owner, 2, "node 2 still owns shards after removal");
        }
    }

    // 6. shard_for is deterministic.
    #[test]
    fn test_shard_for_deterministic() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));

        let id = Uuid::from_u128(0xdeadbeef_cafe_babe_0000_111122223333);
        let s1 = m.shard_for("orders", &id);
        let s2 = m.shard_for("orders", &id);
        let s3 = m.shard_for("orders", &id);
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    // 7. shards_for_collection returns non-empty after routing.
    #[test]
    fn test_shards_for_collection() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));

        // Route 50 records to "inventory".
        for i in 0u128..50 {
            m.shard_for("inventory", &Uuid::from_u128(i));
        }

        let shards = m.shards_for_collection("inventory");
        assert!(!shards.is_empty(), "expected non-empty shard set for 'inventory'");

        // Unknown collection should return empty.
        assert!(m.shards_for_collection("unknown").is_empty());
    }

    // 8. leader_addr returns the registered SocketAddr.
    #[test]
    fn test_leader_addr() {
        let mut m = ShardMap::new(150);
        let expected = addr(9999);
        m.add_node(42, expected);

        // Route a record to get a real shard.
        let shard = m.shard_for("test", &Uuid::from_u128(1));
        let got = m.leader_addr(shard);
        assert_eq!(got, expected);
    }

    // 9. Replication group has 1 leader + (factor-1) followers.
    #[test]
    fn test_replication_group() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));
        m.add_node(3, addr(9003));
        m.set_replication_factor(3);

        let shard = m.shard_for("replicated", &Uuid::from_u128(0xABCD));
        let group = m.replication_group(shard).expect("replication group missing");

        assert_eq!(group.replication_factor, 3);
        assert_eq!(group.shard_id, shard);
        // Leader + followers must be distinct.
        let mut all = vec![group.leader];
        all.extend_from_slice(&group.followers);
        let unique: HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len(), "duplicate nodes in replication group");
        assert_eq!(all.len(), 3, "expected 3 members in replication group");
    }

    // 10. all_nodes returns all added nodes.
    #[test]
    fn test_all_nodes() {
        let mut m = ShardMap::new(150);
        m.add_node(1, addr(9001));
        m.add_node(2, addr(9002));
        m.add_node(3, addr(9003));

        let nodes = m.all_nodes();
        assert_eq!(nodes.len(), 3);
        let ids: Vec<NodeId> = nodes.iter().map(|&(id, _)| id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
    }
}
