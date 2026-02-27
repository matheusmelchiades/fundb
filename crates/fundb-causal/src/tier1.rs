// crates/fundb-causal/src/tier1.rs
//
// STORY-5-4: Causal Engine Tier 1 — Explicit Causality
//
// Provides a cycle-safe DAG of CausalEdges with:
//   - DFS-based path tracing with strength and stability filters
//   - BFS forward (effects_of) and backward (causes_of) traversal
//   - Mermaid, DOT, and JSON visualisation

use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use thiserror::Error;
use fundb_core::{CausalEdge, CausalType, CausalOrigin, StabilityStatus};

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum CausalError {
    #[error("inserting edge would create a cycle: {from} -> {to}")]
    Cycle { from: Uuid, to: Uuid },
    #[error("node not found: {0}")]
    NodeNotFound(Uuid),
}

// ---------------------------------------------------------------------------
// Public option / format types
// ---------------------------------------------------------------------------

/// Options that control path tracing in [`CausalEngine::trace`].
pub struct TraceOptions {
    /// Maximum number of hops allowed in a single path.
    pub max_depth: u32,
    /// Edges with `strength < min_strength` are excluded.
    pub min_strength: f32,
    /// When `Some(s)`, edges with `stability_score < s` are excluded.
    pub min_stability: Option<f32>,
}

/// Output format for [`CausalEngine::visualize`].
pub enum VisFormat {
    Json,
    Mermaid,
    Dot,
}

// ---------------------------------------------------------------------------
// CausalPath
// ---------------------------------------------------------------------------

/// A single simple path through the causal DAG.
#[derive(Debug, Clone)]
pub struct CausalPath {
    /// Ordered node UUIDs from source to target (inclusive).
    pub nodes: Vec<Uuid>,
    /// Ordered edges traversed (parallel to consecutive node pairs).
    pub edges: Vec<CausalEdge>,
    /// Product of all edge strengths along the path (0.0–1.0).
    pub total_strength: f32,
    /// Minimum `stability_score` across all edges (`1.0` when none carry a score).
    pub min_stability: f32,
}

// ---------------------------------------------------------------------------
// CausalEngine
// ---------------------------------------------------------------------------

/// Tier-1 causal engine: stores an in-memory DAG of explicitly declared
/// [`CausalEdge`]s and supports path tracing, reachability queries, and
/// text-format visualisation.
pub struct CausalEngine {
    /// Forward index: source_id → outgoing edges.
    adjacency: HashMap<Uuid, Vec<CausalEdge>>,
    /// Backward index: target_id → incoming edges (full edge stored for
    /// strength look-up during `causes_of`).
    reverse: HashMap<Uuid, Vec<CausalEdge>>,
}

impl CausalEngine {
    /// Create an empty [`CausalEngine`].
    pub fn new() -> Self {
        CausalEngine {
            adjacency: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Mutation
    // -----------------------------------------------------------------------

    /// Insert a directed causal edge into the engine.
    ///
    /// # Errors
    /// Returns [`CausalError::Cycle`] if the edge would create a directed cycle.
    pub fn insert_edge(&mut self, edge: CausalEdge) -> Result<(), CausalError> {
        // Cycle check: if edge.target_id can already reach edge.source_id,
        // adding source_id → target_id would close a cycle.
        if self.can_reach(edge.target_id, edge.source_id) {
            return Err(CausalError::Cycle {
                from: edge.source_id,
                to: edge.target_id,
            });
        }

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

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Find all simple paths from `from` to `to`, subject to the constraints
    /// in `opts`.
    pub fn trace(&self, from: Uuid, to: Uuid, opts: TraceOptions) -> Vec<CausalPath> {
        let mut results: Vec<CausalPath> = Vec::new();
        let mut path_nodes: Vec<Uuid> = vec![from];
        let mut path_edges: Vec<CausalEdge> = Vec::new();

        self.dfs_trace(
            from,
            to,
            &opts,
            0,
            1.0_f32,
            1.0_f32,
            &mut path_nodes,
            &mut path_edges,
            &mut results,
        );

        results
    }

    /// Return all nodes reachable (forward) from `event`, up to `max_depth`
    /// hops, along with the cumulative product of edge strengths along the
    /// strongest path to each node.
    pub fn effects_of(&self, event: Uuid, max_depth: u32) -> Vec<(Uuid, f32)> {
        Self::bfs_reachable(event, max_depth, &self.adjacency, true)
    }

    /// Return all nodes that transitively cause `event`, up to `max_depth`
    /// hops backward, along with the cumulative edge-strength product.
    pub fn causes_of(&self, event: Uuid, max_depth: u32) -> Vec<(Uuid, f32)> {
        Self::bfs_reachable(event, max_depth, &self.reverse, false)
    }

    /// Render a slice of [`CausalPath`]s in the requested text format.
    pub fn visualize(&self, paths: &[CausalPath], format: VisFormat) -> String {
        match format {
            VisFormat::Json => Self::vis_json(paths),
            VisFormat::Mermaid => Self::vis_mermaid(paths),
            VisFormat::Dot => Self::vis_dot(paths),
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers — cycle detection
    // -----------------------------------------------------------------------

    /// DFS reachability check: can we reach `target` starting from `from`?
    fn can_reach(&self, from: Uuid, target: Uuid) -> bool {
        let mut visited: HashSet<Uuid> = HashSet::new();
        self.can_reach_inner(from, target, &mut visited)
    }

    fn can_reach_inner(&self, from: Uuid, target: Uuid, visited: &mut HashSet<Uuid>) -> bool {
        if from == target {
            return true;
        }
        if !visited.insert(from) {
            return false;
        }
        let empty: Vec<CausalEdge> = Vec::new();
        for edge in self.adjacency.get(&from).unwrap_or(&empty) {
            if self.can_reach_inner(edge.target_id, target, visited) {
                return true;
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Private helpers — DFS path tracing
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn dfs_trace(
        &self,
        current: Uuid,
        target: Uuid,
        opts: &TraceOptions,
        depth: u32,
        path_strength: f32,
        path_min_stability: f32,
        path_nodes: &mut Vec<Uuid>,
        path_edges: &mut Vec<CausalEdge>,
        results: &mut Vec<CausalPath>,
    ) {
        // If we arrived at the target (and have taken at least one step), record path.
        if current == target && depth > 0 {
            results.push(CausalPath {
                nodes: path_nodes.clone(),
                edges: path_edges.clone(),
                total_strength: path_strength,
                min_stability: path_min_stability,
            });
            return;
        }

        if depth >= opts.max_depth {
            return;
        }

        let empty: Vec<CausalEdge> = Vec::new();
        for edge in self.adjacency.get(&current).unwrap_or(&empty) {
            // Strength filter.
            if edge.strength < opts.min_strength {
                continue;
            }

            // Stability filter.
            if let Some(min_stab) = opts.min_stability {
                let score = edge.stability_score.unwrap_or(1.0);
                if score < min_stab {
                    continue;
                }
            }

            // Simple-path guard: do not revisit nodes already on the path.
            if path_nodes.contains(&edge.target_id) {
                continue;
            }

            let new_strength = path_strength * edge.strength;
            let edge_stability = edge.stability_score.unwrap_or(1.0);
            let new_min_stability = path_min_stability.min(edge_stability);

            path_nodes.push(edge.target_id);
            path_edges.push(edge.clone());

            self.dfs_trace(
                edge.target_id,
                target,
                opts,
                depth + 1,
                new_strength,
                new_min_stability,
                path_nodes,
                path_edges,
                results,
            );

            path_nodes.pop();
            path_edges.pop();
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers — BFS reachability (effects / causes)
    // -----------------------------------------------------------------------

    /// Generic BFS over either the forward or reverse adjacency map.
    ///
    /// When `forward` is `true` the map is keyed by `source_id` and each
    /// entry's `target_id` is the neighbour.  When `false` the map is keyed by
    /// `target_id` and each entry's `source_id` is the neighbour (backward
    /// traversal via the reverse index).
    fn bfs_reachable(
        start: Uuid,
        max_depth: u32,
        adj: &HashMap<Uuid, Vec<CausalEdge>>,
        forward: bool,
    ) -> Vec<(Uuid, f32)> {
        // best: node → highest cumulative strength seen so far.
        let mut best: HashMap<Uuid, f32> = HashMap::new();

        // Queue: (node, cumulative_strength, depth).
        let mut queue: VecDeque<(Uuid, f32, u32)> = VecDeque::new();
        queue.push_back((start, 1.0_f32, 0));

        while let Some((node, strength, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let empty: Vec<CausalEdge> = Vec::new();
            for edge in adj.get(&node).unwrap_or(&empty) {
                let neighbour = if forward { edge.target_id } else { edge.source_id };
                let new_strength = strength * edge.strength;

                let entry = best.entry(neighbour).or_insert(f32::NEG_INFINITY);
                if new_strength > *entry {
                    *entry = new_strength;
                    queue.push_back((neighbour, new_strength, depth + 1));
                }
            }
        }

        best.into_iter().collect()
    }

    // -----------------------------------------------------------------------
    // Private helpers — visualisation
    // -----------------------------------------------------------------------

    fn vis_json(paths: &[CausalPath]) -> String {
        let mut out = String::from("{\"paths\":[");

        for (i, path) in paths.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('{');

            // nodes array
            out.push_str("\"nodes\":[");
            for (j, node) in path.nodes.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push('"');
                out.push_str(&node.to_string());
                out.push('"');
            }
            out.push(']');

            // total_strength
            out.push_str(",\"total_strength\":");
            out.push_str(&format!("{:.6}", path.total_strength));

            // min_stability
            out.push_str(",\"min_stability\":");
            out.push_str(&format!("{:.6}", path.min_stability));

            out.push('}');
        }

        out.push_str("]}");
        out
    }

    fn vis_mermaid(paths: &[CausalPath]) -> String {
        let mut out = String::from("graph LR\n");
        let mut seen_edges: HashSet<(Uuid, Uuid)> = HashSet::new();

        for path in paths {
            for edge in &path.edges {
                let key = (edge.source_id, edge.target_id);
                if seen_edges.insert(key) {
                    // Use first 8 characters of the UUID as a short label.
                    let src_short = &edge.source_id.to_string()[..8];
                    let tgt_short = &edge.target_id.to_string()[..8];
                    // Mermaid node ids must not contain hyphens, so strip them.
                    let src_id = src_short.replace('-', "_");
                    let tgt_id = tgt_short.replace('-', "_");
                    out.push_str(&format!(
                        "  {}[{}] --> {}[{}]\n",
                        src_id, src_short, tgt_id, tgt_short
                    ));
                }
            }
        }

        out
    }

    fn vis_dot(paths: &[CausalPath]) -> String {
        let mut out = String::from("digraph G {\n");
        let mut seen_edges: HashSet<(Uuid, Uuid)> = HashSet::new();

        for path in paths {
            for edge in &path.edges {
                let key = (edge.source_id, edge.target_id);
                if seen_edges.insert(key) {
                    out.push_str(&format!(
                        "  \"{}\" -> \"{}\";\n",
                        edge.source_id, edge.target_id
                    ));
                }
            }
        }

        out.push('}');
        out
    }
}

impl Default for CausalEngine {
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

    // Helper node UUIDs.
    fn a() -> Uuid { Uuid::from_u128(0x_A000_0000_0000_0000_0000_0000_0000_0001) }
    fn b() -> Uuid { Uuid::from_u128(0x_B000_0000_0000_0000_0000_0000_0000_0002) }
    fn c() -> Uuid { Uuid::from_u128(0x_C000_0000_0000_0000_0000_0000_0000_0003) }
    fn d() -> Uuid { Uuid::from_u128(0x_D000_0000_0000_0000_0000_0000_0000_0004) }

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

    fn make_edge_with_stability(
        source: Uuid,
        target: Uuid,
        strength: f32,
        stability: f32,
    ) -> CausalEdge {
        CausalEdge {
            source_id:        source,
            target_id:        target,
            relation:         CausalType::Caused,
            strength,
            mechanism:        None,
            origin:           CausalOrigin::UserDeclared,
            confidence:       strength,
            stability_score:  Some(stability),
            stability_status: StabilityStatus::Stable,
            direction_status: DirectionStatus::Confirmed,
            discovery_algo:   None,
        }
    }

    fn default_opts(max_depth: u32) -> TraceOptions {
        TraceOptions {
            max_depth,
            min_strength: 0.0,
            min_stability: None,
        }
    }

    // -----------------------------------------------------------------------
    // 1. Simple chain: A→B→C; trace(A, C) returns one path with 3 nodes.
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_simple_chain() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        engine.insert_edge(make_edge(b(), c(), 0.9)).unwrap();

        let paths = engine.trace(a(), c(), default_opts(5));

        assert_eq!(paths.len(), 1, "expected exactly one path A→B→C");
        let p = &paths[0];
        assert_eq!(p.nodes, vec![a(), b(), c()]);
        assert_eq!(p.edges.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 2. Cycle detection: A→B, B→C, then C→A must return Err(Cycle).
    // -----------------------------------------------------------------------
    #[test]
    fn test_cycle_detection() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        engine.insert_edge(make_edge(b(), c(), 0.9)).unwrap();

        let result = engine.insert_edge(make_edge(c(), a(), 0.9));

        assert!(
            matches!(result, Err(CausalError::Cycle { .. })),
            "expected CausalError::Cycle, got {:?}", result
        );
    }

    // -----------------------------------------------------------------------
    // 3. Strength product: A→B(0.9), B→C(0.7) ⟹ total_strength ≈ 0.63.
    // -----------------------------------------------------------------------
    #[test]
    fn test_trace_strength_product() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        engine.insert_edge(make_edge(b(), c(), 0.7)).unwrap();

        let paths = engine.trace(a(), c(), default_opts(5));

        assert_eq!(paths.len(), 1);
        let delta = (paths[0].total_strength - 0.63_f32).abs();
        assert!(
            delta < 1e-5,
            "expected total_strength ≈ 0.63, got {}",
            paths[0].total_strength
        );
    }

    // -----------------------------------------------------------------------
    // 4. min_strength filter: A→B(0.5), A→C(0.9); only A→C path survives.
    // -----------------------------------------------------------------------
    #[test]
    fn test_min_strength_filter() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.5)).unwrap();
        engine.insert_edge(make_edge(a(), c(), 0.9)).unwrap();

        let opts = TraceOptions {
            max_depth: 5,
            min_strength: 0.8,
            min_stability: None,
        };

        // Trace A→B (weak edge, should be filtered out).
        let paths_ab = engine.trace(a(), b(), TraceOptions { max_depth: 5, min_strength: 0.8, min_stability: None });
        assert!(paths_ab.is_empty(), "A→B(0.5) should be filtered out by min_strength=0.8");

        // Trace A→C (strong edge, should pass).
        let paths_ac = engine.trace(a(), c(), opts);
        assert_eq!(paths_ac.len(), 1, "A→C(0.9) should pass min_strength=0.8");
    }

    // -----------------------------------------------------------------------
    // 5. min_stability filter: edges with stability_score below threshold filtered.
    // -----------------------------------------------------------------------
    #[test]
    fn test_min_stability_filter() {
        let mut engine = CausalEngine::new();
        // High-stability edge A→B.
        engine.insert_edge(make_edge_with_stability(a(), b(), 0.9, 0.9)).unwrap();
        // Low-stability edge A→C.
        engine.insert_edge(make_edge_with_stability(a(), c(), 0.9, 0.3)).unwrap();

        let opts = TraceOptions {
            max_depth: 5,
            min_strength: 0.0,
            min_stability: Some(0.6),
        };

        let paths_ab = engine.trace(a(), b(), TraceOptions { max_depth: 5, min_strength: 0.0, min_stability: Some(0.6) });
        assert_eq!(paths_ab.len(), 1, "A→B with stability 0.9 should pass");

        let paths_ac = engine.trace(a(), c(), opts);
        assert!(paths_ac.is_empty(), "A→C with stability 0.3 should be filtered by min_stability=0.6");
    }

    // -----------------------------------------------------------------------
    // 6. effects_of BFS: A→B, A→C, B→D; effects_of(A, 3) contains B, C, D.
    // -----------------------------------------------------------------------
    #[test]
    fn test_effects_of_bfs() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        engine.insert_edge(make_edge(a(), c(), 0.7)).unwrap();
        engine.insert_edge(make_edge(b(), d(), 0.5)).unwrap();

        let effects: HashMap<Uuid, f32> = engine.effects_of(a(), 3).into_iter().collect();

        assert!(effects.contains_key(&b()), "B should be an effect of A");
        assert!(effects.contains_key(&c()), "C should be an effect of A");
        assert!(effects.contains_key(&d()), "D should be a transitive effect of A via B");
    }

    // -----------------------------------------------------------------------
    // 7. causes_of BFS: same graph; causes_of(D, 3) contains B and A.
    // -----------------------------------------------------------------------
    #[test]
    fn test_causes_of_bfs() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();
        engine.insert_edge(make_edge(a(), c(), 0.7)).unwrap();
        engine.insert_edge(make_edge(b(), d(), 0.5)).unwrap();

        let causes: HashMap<Uuid, f32> = engine.causes_of(d(), 3).into_iter().collect();

        assert!(causes.contains_key(&b()), "B should be a cause of D");
        assert!(causes.contains_key(&a()), "A should be a transitive cause of D via B");
    }

    // -----------------------------------------------------------------------
    // 8. Mermaid visualisation: single A→B path must contain header and arrow.
    // -----------------------------------------------------------------------
    #[test]
    fn test_visualize_mermaid() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.9)).unwrap();

        let paths = engine.trace(a(), b(), default_opts(5));
        assert_eq!(paths.len(), 1);

        let output = engine.visualize(&paths, VisFormat::Mermaid);

        assert!(
            output.contains("graph LR"),
            "Mermaid output must start with 'graph LR', got: {}",
            output
        );
        assert!(
            output.contains("-->"),
            "Mermaid output must contain '-->', got: {}",
            output
        );
    }

    // -----------------------------------------------------------------------
    // 9. Separate components must not trigger cycle detection.
    // -----------------------------------------------------------------------
    #[test]
    fn test_no_cycle_for_separate_components() {
        let mut engine = CausalEngine::new();
        engine.insert_edge(make_edge(a(), b(), 0.8)).unwrap();
        // C and D are in a completely separate component.
        let result = engine.insert_edge(make_edge(c(), d(), 0.8));
        assert!(
            result.is_ok(),
            "inserting an edge in a separate component must not produce a cycle error"
        );
    }
}
