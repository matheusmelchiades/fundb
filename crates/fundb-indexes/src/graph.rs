// crates/fundb-indexes/src/graph.rs
//
// STORY-3-3: Graph SPO (Subject–Predicate–Object) index.
// Supports multi-hop traversal with BFS and DFS path-finding,
// enabling "who influenced whom" queries over the knowledge graph.

use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// A directed, weighted edge between two nodes in the knowledge graph.
///
/// Corresponds to the Subject–Predicate–Object triple model where:
/// - `subject`   = the source node UUID
/// - `predicate` = the relationship label (e.g. "follows", "cites", "influenced")
/// - `object`    = the target node UUID
/// - `confidence` = edge reliability score (0.0–1.0)
/// - `props`     = MessagePack-encoded extra properties (schema-on-read)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub subject: Uuid,
    pub predicate: String,
    pub object: Uuid,
    pub confidence: f32,
    pub props: Vec<u8>, // MessagePack-encoded extra properties
}

/// In-memory bidirectional graph index keyed by Subject–Predicate–Object triples.
///
/// Maintains two separate indexes:
/// - `outgoing`: maps each subject node to all edges leaving it (forward index).
/// - `incoming`: maps each object node to all edges pointing into it (reverse index).
///
/// Both indexes are kept in sync on every mutating operation, enabling O(degree)
/// forward *and* backward traversal without a full scan.
pub struct GraphIndex {
    /// Forward index: subject UUID → all edges leaving that node.
    outgoing: HashMap<Uuid, Vec<Edge>>,
    /// Reverse index: object UUID → all edges pointing into that node.
    incoming: HashMap<Uuid, Vec<Edge>>,
}

impl GraphIndex {
    /// Create a new, empty `GraphIndex`.
    pub fn new() -> Self {
        GraphIndex {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
        }
    }

    /// Insert a directed edge `s --[predicate]--> o` into the index.
    ///
    /// Duplicate edges (same subject, predicate, object triple) are allowed;
    /// they will appear as distinct entries (e.g. with different confidence values).
    ///
    /// # Arguments
    /// * `s`          – UUID of the source (subject) node.
    /// * `predicate`  – Relationship label string.
    /// * `o`          – UUID of the target (object) node.
    /// * `confidence` – Reliability score in range [0.0, 1.0].
    /// * `props`      – MessagePack-encoded extra properties blob.
    pub fn insert_edge(
        &mut self,
        s: Uuid,
        predicate: &str,
        o: Uuid,
        confidence: f32,
        props: Vec<u8>,
    ) -> Result<()> {
        let edge = Edge {
            subject: s,
            predicate: predicate.to_string(),
            object: o,
            confidence,
            props,
        };

        self.outgoing.entry(s).or_default().push(edge.clone());
        self.incoming.entry(o).or_default().push(edge);

        Ok(())
    }

    /// Delete all edges matching the `(s, predicate, o)` triple.
    ///
    /// Removes from both the forward (`outgoing`) and reverse (`incoming`) indexes.
    /// If no such edge exists this is a no-op (returns `Ok(())`).
    pub fn delete_edge(&mut self, s: Uuid, predicate: &str, o: Uuid) -> Result<()> {
        if let Some(edges) = self.outgoing.get_mut(&s) {
            edges.retain(|e| !(e.subject == s && e.predicate == predicate && e.object == o));
        }

        if let Some(edges) = self.incoming.get_mut(&o) {
            edges.retain(|e| !(e.subject == s && e.predicate == predicate && e.object == o));
        }

        Ok(())
    }

    /// Return all edges leaving `subject`.
    ///
    /// If `predicate` is `Some(p)`, only edges with `edge.predicate == p` are
    /// included. If `predicate` is `None`, all outgoing edges are returned.
    pub fn outgoing(&self, subject: Uuid, predicate: Option<&str>) -> Vec<Edge> {
        match self.outgoing.get(&subject) {
            None => vec![],
            Some(edges) => match predicate {
                None => edges.clone(),
                Some(p) => edges.iter().filter(|e| e.predicate == p).cloned().collect(),
            },
        }
    }

    /// Return all edges pointing into `object`.
    ///
    /// If `predicate` is `Some(p)`, only edges with `edge.predicate == p` are
    /// included. If `predicate` is `None`, all incoming edges are returned.
    pub fn incoming(&self, object: Uuid, predicate: Option<&str>) -> Vec<Edge> {
        match self.incoming.get(&object) {
            None => vec![],
            Some(edges) => match predicate {
                None => edges.clone(),
                Some(p) => edges.iter().filter(|e| e.predicate == p).cloned().collect(),
            },
        }
    }

    /// Breadth-first traversal from `start` following edges labelled `predicate`.
    ///
    /// Returns `(node, depth, path_strength)` for every reachable node
    /// (excluding `start` itself). `path_strength` is the product of all
    /// edge confidences along the traversal path. When multiple paths lead
    /// to the same node, the one with the highest confidence product is kept
    /// (guaranteed by BFS visiting lower-depth paths first when the graph is
    /// uniformly weighted, but note that in general the *first* path found
    /// wins because of the `visited` guard — this matches the specified algorithm).
    ///
    /// Cycles are handled by the `visited` set: once a node has been dequeued
    /// it will never be enqueued again, so traversal always terminates.
    ///
    /// # Arguments
    /// * `start`     – UUID of the start node.
    /// * `predicate` – Only follow edges with this label.
    /// * `max_depth` – Maximum hop count from `start`.
    pub fn bfs(&self, start: Uuid, predicate: &str, max_depth: u32) -> Vec<(Uuid, u32, f32)> {
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut queue: VecDeque<(Uuid, u32, f32)> = VecDeque::new();
        let mut result: Vec<(Uuid, u32, f32)> = Vec::new();

        queue.push_back((start, 0, 1.0_f32));

        while let Some((node, depth, strength)) = queue.pop_front() {
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node);

            if node != start {
                result.push((node, depth, strength));
            }

            if depth < max_depth {
                if let Some(edges) = self.outgoing.get(&node) {
                    for edge in edges.iter().filter(|e| e.predicate == predicate) {
                        if !visited.contains(&edge.object) {
                            queue.push_back((edge.object, depth + 1, strength * edge.confidence));
                        }
                    }
                }
            }
        }

        result
    }

    /// Find all simple paths (no repeated nodes) from `from` to `to`.
    ///
    /// Uses depth-first search with an explicit backtracking stack. Only paths
    /// of at most `max_depth` hops are considered. Paths using *any* predicate
    /// are included (predicate-agnostic traversal).
    ///
    /// Each returned path is a `Vec<Uuid>` starting at `from` and ending at
    /// `to`, inclusive.
    ///
    /// # Arguments
    /// * `from`      – UUID of the start node.
    /// * `to`        – UUID of the target node.
    /// * `max_depth` – Maximum number of edges in any returned path.
    pub fn path(&self, from: Uuid, to: Uuid, max_depth: u32) -> Vec<Vec<Uuid>> {
        let mut results: Vec<Vec<Uuid>> = Vec::new();
        let mut path: Vec<Uuid> = vec![from];
        let mut visited: HashSet<Uuid> = HashSet::new();
        visited.insert(from);

        Self::dfs(
            from,
            to,
            &mut path,
            &mut visited,
            0,
            max_depth,
            &self.outgoing,
            &mut results,
        );

        results
    }

    /// Recursive DFS helper used by [`path`].
    #[allow(clippy::too_many_arguments)]
    fn dfs(
        current: Uuid,
        target: Uuid,
        path: &mut Vec<Uuid>,
        visited: &mut HashSet<Uuid>,
        depth: u32,
        max_depth: u32,
        outgoing: &HashMap<Uuid, Vec<Edge>>,
        results: &mut Vec<Vec<Uuid>>,
    ) {
        if current == target {
            results.push(path.clone());
            return;
        }

        if depth >= max_depth {
            return;
        }

        if let Some(edges) = outgoing.get(&current) {
            for edge in edges {
                if !visited.contains(&edge.object) {
                    path.push(edge.object);
                    visited.insert(edge.object);

                    Self::dfs(
                        edge.object,
                        target,
                        path,
                        visited,
                        depth + 1,
                        max_depth,
                        outgoing,
                        results,
                    );

                    path.pop();
                    visited.remove(&edge.object);
                }
            }
        }
    }

    /// Return the total number of edges stored in the forward index.
    pub fn edge_count(&self) -> usize {
        self.outgoing.values().map(|v| v.len()).sum()
    }
}

impl Default for GraphIndex {
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

    // Convenience helpers — fixed deterministic UUIDs for all tests.
    fn a() -> Uuid {
        Uuid::from_u128(1)
    }
    fn b() -> Uuid {
        Uuid::from_u128(2)
    }
    fn c() -> Uuid {
        Uuid::from_u128(3)
    }
    fn d() -> Uuid {
        Uuid::from_u128(4)
    }

    // -----------------------------------------------------------------------
    // 1. Insert and outgoing filter
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_and_outgoing() {
        let mut g = GraphIndex::new();

        g.insert_edge(a(), "follows", b(), 1.0, vec![]).unwrap();
        g.insert_edge(a(), "follows", c(), 1.0, vec![]).unwrap();
        g.insert_edge(a(), "likes", d(), 1.0, vec![]).unwrap();

        let follows = g.outgoing(a(), Some("follows"));
        assert_eq!(follows.len(), 2, "should find 2 'follows' edges");

        let all = g.outgoing(a(), None);
        assert_eq!(all.len(), 3, "should find 3 total outgoing edges");
    }

    // -----------------------------------------------------------------------
    // 2. Incoming index
    // -----------------------------------------------------------------------
    #[test]
    fn test_incoming() {
        let mut g = GraphIndex::new();

        g.insert_edge(a(), "cites", b(), 1.0, vec![]).unwrap();
        g.insert_edge(c(), "cites", b(), 1.0, vec![]).unwrap();

        let cites = g.incoming(b(), Some("cites"));
        assert_eq!(cites.len(), 2, "should find 2 'cites' edges pointing to B");
    }

    // -----------------------------------------------------------------------
    // 3. Delete edge
    // -----------------------------------------------------------------------
    #[test]
    fn test_delete_edge() {
        let mut g = GraphIndex::new();

        g.insert_edge(a(), "follows", b(), 1.0, vec![]).unwrap();
        g.insert_edge(a(), "follows", c(), 1.0, vec![]).unwrap();

        assert_eq!(g.edge_count(), 2);

        g.delete_edge(a(), "follows", b()).unwrap();

        assert_eq!(
            g.edge_count(),
            1,
            "edge_count should decrement after deletion"
        );

        let remaining = g.outgoing(a(), Some("follows"));
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].object, c(), "only the A->C edge should remain");

        // Verify the reverse index is also cleaned up.
        let inc = g.incoming(b(), Some("follows"));
        assert!(
            inc.is_empty(),
            "incoming index for B should be empty after deletion"
        );
    }

    // -----------------------------------------------------------------------
    // 4. BFS linear chain
    // -----------------------------------------------------------------------
    #[test]
    fn test_bfs_linear() {
        let mut g = GraphIndex::new();

        // Chain: A → B → C → D (predicate "next")
        g.insert_edge(a(), "next", b(), 1.0, vec![]).unwrap();
        g.insert_edge(b(), "next", c(), 1.0, vec![]).unwrap();
        g.insert_edge(c(), "next", d(), 1.0, vec![]).unwrap();

        let reachable = g.bfs(a(), "next", 5);

        // Must reach B, C, D — excluding start node A.
        assert_eq!(reachable.len(), 3, "should reach exactly B, C, D");

        // Build a lookup map for easy assertion.
        let map: HashMap<Uuid, (u32, f32)> = reachable
            .into_iter()
            .map(|(node, depth, strength)| (node, (depth, strength)))
            .collect();

        assert_eq!(map[&b()].0, 1, "B should be at depth 1");
        assert_eq!(map[&c()].0, 2, "C should be at depth 2");
        assert_eq!(map[&d()].0, 3, "D should be at depth 3");
    }

    // -----------------------------------------------------------------------
    // 5. BFS with cycle terminates
    // -----------------------------------------------------------------------
    #[test]
    fn test_bfs_cycle_terminates() {
        let mut g = GraphIndex::new();

        // Cycle: A → B → C → A
        g.insert_edge(a(), "next", b(), 1.0, vec![]).unwrap();
        g.insert_edge(b(), "next", c(), 1.0, vec![]).unwrap();
        g.insert_edge(c(), "next", a(), 1.0, vec![]).unwrap();

        // max_depth=100 would loop forever without cycle detection.
        let reachable = g.bfs(a(), "next", 100);

        // Must have terminated and found exactly B and C.
        assert_eq!(
            reachable.len(),
            2,
            "should find exactly B and C despite cycle"
        );

        let nodes: HashSet<Uuid> = reachable.iter().map(|(n, _, _)| *n).collect();
        assert!(nodes.contains(&b()), "B must be reachable");
        assert!(nodes.contains(&c()), "C must be reachable");
        assert!(
            !nodes.contains(&a()),
            "start node A must not appear in results"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Path DFS — simple graph with two routes
    // -----------------------------------------------------------------------
    #[test]
    fn test_path_simple() {
        let mut g = GraphIndex::new();

        // Graph: A → B → C  and  A → C
        g.insert_edge(a(), "rel", b(), 1.0, vec![]).unwrap();
        g.insert_edge(b(), "rel", c(), 1.0, vec![]).unwrap();
        g.insert_edge(a(), "rel", c(), 1.0, vec![]).unwrap();

        let paths = g.path(a(), c(), 3);

        assert_eq!(
            paths.len(),
            2,
            "should find exactly 2 simple paths from A to C"
        );

        // Both expected paths.
        let expected_long: Vec<Uuid> = vec![a(), b(), c()];
        let expected_short: Vec<Uuid> = vec![a(), c()];

        assert!(
            paths.contains(&expected_long),
            "path [A,B,C] must be present; got {:?}",
            paths
        );
        assert!(
            paths.contains(&expected_short),
            "path [A,C] must be present; got {:?}",
            paths
        );
    }

    // -----------------------------------------------------------------------
    // 7. BFS path_strength — product of confidences along the path
    // -----------------------------------------------------------------------
    #[test]
    fn test_path_strength() {
        let mut g = GraphIndex::new();

        // A --0.8--> B --0.5--> C  (predicate "edge")
        g.insert_edge(a(), "edge", b(), 0.8, vec![]).unwrap();
        g.insert_edge(b(), "edge", c(), 0.5, vec![]).unwrap();

        let reachable = g.bfs(a(), "edge", 2);

        let map: HashMap<Uuid, (u32, f32)> = reachable
            .into_iter()
            .map(|(node, depth, strength)| (node, (depth, strength)))
            .collect();

        // Path to B: confidence = 0.8
        let (depth_b, strength_b) = map[&b()];
        assert_eq!(depth_b, 1);
        assert!(
            (strength_b - 0.8_f32).abs() < 1e-6,
            "strength to B should be 0.8, got {}",
            strength_b
        );

        // Path to C: confidence = 0.8 * 0.5 = 0.4
        let (depth_c, strength_c) = map[&c()];
        assert_eq!(depth_c, 2);
        assert!(
            (strength_c - 0.4_f32).abs() < 1e-6,
            "strength to C should be 0.4 (0.8 * 0.5), got {}",
            strength_c
        );
    }
}
