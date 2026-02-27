// crates/fundb-indexes/src/causal_dag.rs
//
// STORY-3-5: Causal DAG Index
//
// Indexes directed causal edges between FunRecords. Supports:
//   - Cycle-safe insertion via DFS reachability check
//   - Multi-hop path enumeration with strength filtering
//   - Forward effects() and backward causes() traversal
//   - Hot-path closure caching via materialize_closure()

use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use fundb_core::record::CausalEdge;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A sequence of causal edges from one node to another, with accumulated
/// strength (product of all edge strengths along the path).
#[derive(Debug, Clone)]
pub struct CausalPath {
    /// Ordered list of node UUIDs from source to target (inclusive).
    pub nodes:          Vec<Uuid>,
    /// Ordered list of causal edges traversed along the path.
    pub edges:          Vec<CausalEdge>,
    /// Product of all edge strengths along the path (0.0–1.0).
    pub total_strength: f32,
}

/// Errors that can be returned by [`CausalDagIndex`] operations.
#[derive(Debug, thiserror::Error)]
pub enum CausalError {
    /// Inserting the edge would create a directed cycle in the DAG.
    #[error("cycle detected at node {detected_at}")]
    Cycle { detected_at: Uuid },
    /// Edge strength is outside the valid range [0.0, 1.0].
    #[error("invalid strength: must be in [0, 1]")]
    InvalidStrength,
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

/// In-memory index of directed causal edges forming a DAG.
///
/// Maintains three structures:
/// - `adjacency`:     forward index (source → outgoing edges).
/// - `reverse`:       backward index (target → incoming edges) for `causes()`.
/// - `closure_cache`: pre-computed path lists for hot (from, to) pairs.
pub struct CausalDagIndex {
    adjacency:     HashMap<Uuid, Vec<CausalEdge>>,
    reverse:       HashMap<Uuid, Vec<CausalEdge>>,
    closure_cache: HashMap<(Uuid, Uuid), Vec<CausalPath>>,
}

impl CausalDagIndex {
    /// Create a new, empty `CausalDagIndex`.
    pub fn new() -> Self {
        CausalDagIndex {
            adjacency:     HashMap::new(),
            reverse:       HashMap::new(),
            closure_cache: HashMap::new(),
        }
    }

    /// Insert a directed causal edge into the index.
    ///
    /// # Errors
    /// - [`CausalError::InvalidStrength`] if `edge.strength` is not in `[0.0, 1.0]`.
    /// - [`CausalError::Cycle`] if inserting the edge would create a directed cycle.
    pub fn insert_edge(&mut self, edge: CausalEdge) -> Result<(), CausalError> {
        // 1. Validate strength.
        if !(0.0..=1.0).contains(&edge.strength) {
            return Err(CausalError::InvalidStrength);
        }

        // 2. Cycle detection: if `edge.target_id` can already reach `edge.source_id`,
        //    adding source_id → target_id would create a cycle.
        let mut visited = HashSet::new();
        if self.can_reach(edge.target_id, edge.source_id, &mut visited) {
            return Err(CausalError::Cycle { detected_at: edge.source_id });
        }

        // 3. Insert into both indexes.
        self.adjacency
            .entry(edge.source_id)
            .or_default()
            .push(edge.clone());

        self.reverse
            .entry(edge.target_id)
            .or_default()
            .push(edge);

        Ok(())
    }

    /// Find all simple paths from `from` to `to` with at most `max_depth` hops
    /// and a minimum cumulative strength of `min_strength`.
    ///
    /// Checks the closure cache first; if a cached result exists it is returned
    /// immediately (cache stores all paths with min_strength = 0.0, so a cached
    /// result is filtered here to honour `min_strength`).
    pub fn paths(&self, from: Uuid, to: Uuid, max_depth: u32, min_strength: f32) -> Vec<CausalPath> {
        // Check cache (cache stores full paths without strength filtering).
        if let Some(cached) = self.closure_cache.get(&(from, to)) {
            return cached
                .iter()
                .filter(|p| p.total_strength >= min_strength)
                .cloned()
                .collect();
        }

        // DFS path search.
        let mut results: Vec<CausalPath> = Vec::new();
        let mut path_nodes: Vec<Uuid>      = vec![from];
        let mut path_edges: Vec<CausalEdge> = Vec::new();

        Self::dfs_paths(
            from,
            to,
            &mut path_nodes,
            &mut path_edges,
            1.0_f32,
            max_depth,
            0,
            min_strength,
            &self.adjacency,
            &mut results,
        );

        results
    }

    /// Return all nodes reachable (directly or transitively) from `from`,
    /// following forward edges up to `max_depth` hops.
    ///
    /// Each entry is `(node_uuid, cumulative_strength)` where cumulative
    /// strength is the product of edge strengths along the path. When a node
    /// is reachable via multiple paths, the highest-strength path is kept.
    pub fn effects(&self, from: Uuid, max_depth: u32) -> Vec<(Uuid, f32)> {
        Self::traverse_reachable(from, max_depth, &self.adjacency, true)
    }

    /// Return all nodes that can transitively reach `of` (i.e. ancestors),
    /// following reverse edges up to `max_depth` hops.
    ///
    /// Each entry is `(node_uuid, cumulative_strength)`.
    pub fn causes(&self, of: Uuid, max_depth: u32) -> Vec<(Uuid, f32)> {
        Self::traverse_reachable(of, max_depth, &self.reverse, false)
    }

    /// Pre-compute and cache all paths between `from` and `to` (with
    /// max_depth=10, min_strength=0.0). Subsequent calls to `paths(from, to,
    /// …)` will be served from the cache.
    pub fn materialize_closure(&mut self, from: Uuid, to: Uuid) {
        // Compute uncached paths using the full depth / no strength filter.
        let mut results: Vec<CausalPath> = Vec::new();
        let mut path_nodes: Vec<Uuid>       = vec![from];
        let mut path_edges: Vec<CausalEdge>  = Vec::new();

        Self::dfs_paths(
            from,
            to,
            &mut path_nodes,
            &mut path_edges,
            1.0_f32,
            10,
            0,
            0.0,
            &self.adjacency,
            &mut results,
        );

        self.closure_cache.insert((from, to), results);
    }

    /// Return the total number of causal edges stored in the index.
    pub fn len(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// DFS reachability check: can we reach `target` starting from `from`?
    fn can_reach(&self, from: Uuid, target: Uuid, visited: &mut HashSet<Uuid>) -> bool {
        if from == target {
            return true;
        }
        if visited.contains(&from) {
            return false;
        }
        visited.insert(from);
        let empty: Vec<CausalEdge> = Vec::new();
        for edge in self.adjacency.get(&from).unwrap_or(&empty) {
            if self.can_reach(edge.target_id, target, visited) {
                return true;
            }
        }
        false
    }

    /// Recursive DFS path finder (backtracking, simple-path only).
    #[allow(clippy::too_many_arguments)]
    fn dfs_paths(
        current:     Uuid,
        target:      Uuid,
        path_nodes:  &mut Vec<Uuid>,
        path_edges:  &mut Vec<CausalEdge>,
        strength:    f32,
        max_depth:   u32,
        depth:       u32,
        min_strength: f32,
        adjacency:   &HashMap<Uuid, Vec<CausalEdge>>,
        results:     &mut Vec<CausalPath>,
    ) {
        if current == target && depth > 0 {
            results.push(CausalPath {
                nodes:          path_nodes.clone(),
                edges:          path_edges.clone(),
                total_strength: strength,
            });
            return;
        }

        if depth >= max_depth {
            return;
        }

        if let Some(edges) = adjacency.get(&current) {
            for edge in edges {
                let new_strength = strength * edge.strength;

                // Prune weak paths and avoid revisiting nodes (simple paths only).
                if new_strength >= min_strength && !path_nodes.contains(&edge.target_id) {
                    path_nodes.push(edge.target_id);
                    path_edges.push(edge.clone());

                    Self::dfs_paths(
                        edge.target_id,
                        target,
                        path_nodes,
                        path_edges,
                        new_strength,
                        max_depth,
                        depth + 1,
                        min_strength,
                        adjacency,
                        results,
                    );

                    path_nodes.pop();
                    path_edges.pop();
                }
            }
        }
    }

    /// BFS/DFS traversal over either the forward or reverse adjacency map,
    /// accumulating cumulative strength (product of edge strengths).
    ///
    /// When multiple paths lead to the same node the highest-strength entry
    /// wins (we update if a better path is found).
    ///
    /// `_forward` is unused at the call site but kept for future extension.
    fn traverse_reachable(
        start:     Uuid,
        max_depth: u32,
        adj:       &HashMap<Uuid, Vec<CausalEdge>>,
        _forward:  bool,
    ) -> Vec<(Uuid, f32)> {
        // Map: node → best cumulative strength seen so far.
        let mut best: HashMap<Uuid, f32> = HashMap::new();

        // Stack: (current_node, depth, cumulative_strength)
        let mut stack: Vec<(Uuid, u32, f32)> = vec![(start, 0, 1.0_f32)];

        while let Some((node, depth, strength)) = stack.pop() {
            if depth >= max_depth {
                continue;
            }

            if let Some(edges) = adj.get(&node) {
                for edge in edges {
                    // For forward traversal the neighbour is target_id.
                    // For reverse traversal the adjacency map is keyed by target_id,
                    // so edges contain the original CausalEdge — neighbour is source_id.
                    let neighbour = if _forward { edge.target_id } else { edge.source_id };
                    let new_strength = strength * edge.strength;

                    let entry = best.entry(neighbour).or_insert(f32::NEG_INFINITY);
                    if new_strength > *entry {
                        *entry = new_strength;
                        stack.push((neighbour, depth + 1, new_strength));
                    }
                }
            }
        }

        best.into_iter().collect()
    }
}

impl Default for CausalDagIndex {
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
    use fundb_core::{CausalType, CausalOrigin, StabilityStatus, DirectionStatus};

    // Helper UUIDs.
    fn a() -> Uuid { Uuid::from_u128(1) }
    fn b() -> Uuid { Uuid::from_u128(2) }
    fn c() -> Uuid { Uuid::from_u128(3) }
    fn d() -> Uuid { Uuid::from_u128(4) }

    fn make_edge(source: Uuid, target: Uuid, strength: f32) -> CausalEdge {
        CausalEdge {
            source_id:        source,
            target_id:        target,
            relation:         CausalType::Caused,
            strength,
            mechanism:        None,
            origin:           CausalOrigin::UserDeclared,
            confidence:       strength,
            stability_score:  None,
            stability_status: StabilityStatus::NotApplicable,
            direction_status: DirectionStatus::Confirmed,
            discovery_algo:   None,
        }
    }

    // -----------------------------------------------------------------------
    // 1. insert_and_paths — A→B→C, strength 0.8 each
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_and_paths() {
        let mut idx = CausalDagIndex::new();

        idx.insert_edge(make_edge(a(), b(), 0.8)).unwrap();
        idx.insert_edge(make_edge(b(), c(), 0.8)).unwrap();

        let paths = idx.paths(a(), c(), 3, 0.0);

        assert_eq!(paths.len(), 1, "should find exactly one path A→B→C");

        let p = &paths[0];
        assert_eq!(p.nodes, vec![a(), b(), c()]);
        assert_eq!(p.edges.len(), 2);
        assert!(
            (p.total_strength - 0.64_f32).abs() < 1e-5,
            "total_strength should be 0.8*0.8 = 0.64, got {}",
            p.total_strength
        );
    }

    // -----------------------------------------------------------------------
    // 2. cycle_detection — A→B→C, then C→A should fail
    // -----------------------------------------------------------------------
    #[test]
    fn test_cycle_detection() {
        let mut idx = CausalDagIndex::new();

        idx.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        idx.insert_edge(make_edge(b(), c(), 0.9)).unwrap();

        let result = idx.insert_edge(make_edge(c(), a(), 0.9));

        match result {
            Err(CausalError::Cycle { detected_at }) => {
                assert_eq!(detected_at, a(), "cycle should be detected at node A");
            }
            other => panic!("expected CausalError::Cycle, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // 3. effects — A→B(0.9), A→C(0.7), B→D(0.5)
    // -----------------------------------------------------------------------
    #[test]
    fn test_effects() {
        let mut idx = CausalDagIndex::new();

        idx.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        idx.insert_edge(make_edge(a(), c(), 0.7)).unwrap();
        idx.insert_edge(make_edge(b(), d(), 0.5)).unwrap();

        let mut effects = idx.effects(a(), 2);
        // Sort for deterministic comparison.
        effects.sort_by(|x, y| x.0.cmp(&y.0));

        let map: HashMap<Uuid, f32> = effects.into_iter().collect();

        assert!(map.contains_key(&b()), "B should be an effect of A");
        assert!(map.contains_key(&c()), "C should be an effect of A");
        assert!(map.contains_key(&d()), "D should be an effect of A via B");

        assert!((map[&b()] - 0.9_f32).abs() < 1e-5, "strength to B should be 0.9, got {}", map[&b()]);
        assert!((map[&c()] - 0.7_f32).abs() < 1e-5, "strength to C should be 0.7, got {}", map[&c()]);
        assert!((map[&d()] - 0.45_f32).abs() < 1e-5, "strength to D should be 0.9*0.5=0.45, got {}", map[&d()]);
    }

    // -----------------------------------------------------------------------
    // 4. causes — same graph; causes(D, 2) → B(0.5), A(0.45)
    // -----------------------------------------------------------------------
    #[test]
    fn test_causes() {
        let mut idx = CausalDagIndex::new();

        idx.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        idx.insert_edge(make_edge(a(), c(), 0.7)).unwrap();
        idx.insert_edge(make_edge(b(), d(), 0.5)).unwrap();

        let causes = idx.causes(d(), 2);
        let map: HashMap<Uuid, f32> = causes.into_iter().collect();

        assert!(map.contains_key(&b()), "B should be a cause of D");
        assert!(map.contains_key(&a()), "A should be a cause of D (via B)");

        assert!((map[&b()] - 0.5_f32).abs() < 1e-5,  "strength from B to D should be 0.5, got {}", map[&b()]);
        assert!((map[&a()] - 0.45_f32).abs() < 1e-5, "strength from A to D via B should be 0.45, got {}", map[&a()]);
    }

    // -----------------------------------------------------------------------
    // 5. closure cache — materialize then re-query should return same result
    // -----------------------------------------------------------------------
    #[test]
    fn test_closure_cache() {
        let mut idx = CausalDagIndex::new();

        idx.insert_edge(make_edge(a(), b(), 0.8)).unwrap();
        idx.insert_edge(make_edge(b(), c(), 0.8)).unwrap();

        // Pre-compute the closure.
        idx.materialize_closure(a(), c());

        // Verify the cache contains the entry.
        assert!(
            idx.closure_cache.contains_key(&(a(), c())),
            "closure_cache should contain the (A, C) entry after materialize_closure"
        );

        let cached = &idx.closure_cache[&(a(), c())];
        assert_eq!(cached.len(), 1, "cached result should contain one path A→B→C");
        assert!((cached[0].total_strength - 0.64_f32).abs() < 1e-5);

        // A subsequent paths() call should hit the cache and return the same answer.
        let paths = idx.paths(a(), c(), 3, 0.0);
        assert_eq!(paths.len(), 1);
        assert!((paths[0].total_strength - 0.64_f32).abs() < 1e-5);
    }

    // -----------------------------------------------------------------------
    // 6. len() counts edges correctly
    // -----------------------------------------------------------------------
    #[test]
    fn test_len() {
        let mut idx = CausalDagIndex::new();

        assert_eq!(idx.len(), 0);

        idx.insert_edge(make_edge(a(), b(), 0.5)).unwrap();
        idx.insert_edge(make_edge(b(), c(), 0.5)).unwrap();

        assert_eq!(idx.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 7. Invalid strength is rejected
    // -----------------------------------------------------------------------
    #[test]
    fn test_invalid_strength() {
        let mut idx = CausalDagIndex::new();

        let result = idx.insert_edge(make_edge(a(), b(), 1.5));
        assert!(
            matches!(result, Err(CausalError::InvalidStrength)),
            "strength > 1.0 should return InvalidStrength"
        );

        let result = idx.insert_edge(make_edge(a(), b(), -0.1));
        assert!(
            matches!(result, Err(CausalError::InvalidStrength)),
            "negative strength should return InvalidStrength"
        );
    }
}
