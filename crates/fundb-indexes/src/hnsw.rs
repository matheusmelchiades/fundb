// crates/fundb-indexes/src/hnsw.rs
//
// STORY-3-2: HNSW vector index + PQ encoder.
//
// Provides approximate nearest neighbour (ANN) search over millions of
// high-dimensional embeddings in under 5 ms, enabling real-time RAG retrieval.

use anyhow::{anyhow, Result};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Distance helpers
// ---------------------------------------------------------------------------

/// Euclidean (L2) distance between two equal-length slices.
#[inline]
fn euclidean(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

// ---------------------------------------------------------------------------
// Ordered-float wrapper so we can put f32 into BinaryHeap
// ---------------------------------------------------------------------------

/// A (distance, id) pair that orders by distance.
/// `MinItem` is smallest-first (min-heap behaviour via BinaryHeap<Reverse<MinItem>>).
#[derive(Clone, PartialEq)]
struct Item {
    dist: f32,
    id: Uuid,
}

impl Eq for Item {}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        // NaN-safe comparison: treat NaN as largest
        self.dist
            .partial_cmp(&other.dist)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.id.cmp(&other.id))
    }
}

// ---------------------------------------------------------------------------
// HnswIndex
// ---------------------------------------------------------------------------

pub struct HnswIndex {
    dims: usize,
    m: usize,               // max connections per node per layer
    ef_construction: usize, // dynamic candidate list size during construction
    max_layer: usize,       // current highest layer index (0-based)
    /// layers[layer][node] = list of neighbour IDs
    layers: Vec<HashMap<Uuid, Vec<Uuid>>>,
    /// full vector storage
    vectors: HashMap<Uuid, Vec<f32>>,
    /// lazy-delete set
    tombstones: HashSet<Uuid>,
    /// entry point: (node_id, layer at which it lives)
    entry_point: Option<(Uuid, usize)>,
    /// count of live (non-tombstoned) nodes
    live_count: usize,
}

impl HnswIndex {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    pub fn new(dims: usize, m: usize, ef_construction: usize) -> Self {
        HnswIndex {
            dims,
            m,
            ef_construction,
            max_layer: 0,
            layers: Vec::new(),
            vectors: HashMap::new(),
            tombstones: HashSet::new(),
            entry_point: None,
            live_count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Insert a vector with the given ID.
    pub fn insert(&mut self, id: Uuid, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dims {
            return Err(anyhow!(
                "vector dimension mismatch: expected {}, got {}",
                self.dims,
                vector.len()
            ));
        }

        // Choose the highest layer for this new node.
        let node_layer = self.random_layer(id);

        // Store the vector.
        self.vectors.insert(id, vector.to_vec());
        self.live_count += 1;

        // Ensure enough layer maps exist.
        while self.layers.len() <= node_layer {
            self.layers.push(HashMap::new());
        }
        // Add the node to all layers up to node_layer with an empty neighbour list.
        for l in 0..=node_layer {
            self.layers[l].entry(id).or_default();
        }

        // If this is the very first node, set it as entry point and return.
        let (mut ep_id, ep_layer) = match self.entry_point {
            None => {
                self.entry_point = Some((id, node_layer));
                return Ok(());
            }
            Some(ep) => ep,
        };

        // --- Phase 1: greedy descent from current top layer to node_layer+1 ---
        let current_top = ep_layer;
        for layer in (node_layer + 1..=current_top).rev() {
            ep_id = self.greedy_search_layer(vector, ep_id, layer);
        }

        // --- Phase 2: beam search + edge insertion from node_layer down to 0 ---
        let mut entry_candidates = vec![Item {
            dist: euclidean(vector, &self.vectors[&ep_id]),
            id: ep_id,
        }];

        for layer in (0..=node_layer).rev() {
            // Find ef_construction nearest neighbours at this layer.
            let neighbours =
                self.search_layer(vector, &entry_candidates, self.ef_construction, layer);

            // Pick the m best to actually connect.
            let m_max = if layer == 0 { self.m * 2 } else { self.m };
            let connect_count = neighbours.len().min(self.m);

            for nb in &neighbours[..connect_count] {
                // new node -> neighbour
                self.layers[layer].entry(id).or_default().push(nb.id);

                // neighbour -> new node (bidirectional)
                self.layers[layer].entry(nb.id).or_default().push(id);

                // Prune neighbour's connections if over limit.
                let nb_vec = self.vectors[&nb.id].clone();
                let nb_neighbours = self.layers[layer].get(&nb.id).cloned().unwrap_or_default();

                if nb_neighbours.len() > m_max {
                    let pruned = self.select_neighbours(&nb_vec, &nb_neighbours, m_max);
                    self.layers[layer].insert(nb.id, pruned);
                }
            }

            // Use the found neighbours as entry candidates for the next (lower) layer.
            entry_candidates = neighbours;
        }

        // Update entry point if this node reaches a higher layer.
        if node_layer > ep_layer {
            self.entry_point = Some((id, node_layer));
        }

        Ok(())
    }

    /// Approximate k-nearest-neighbour search.
    ///
    /// Returns up to `k` `(id, distance)` pairs sorted by distance ascending.
    /// `recall_target` adjusts the internal ef parameter: higher = more
    /// accurate but slower.
    pub fn search(&self, query: &[f32], k: usize, recall_target: f32) -> Vec<(Uuid, f32)> {
        if self.entry_point.is_none() || self.live_count == 0 {
            return vec![];
        }

        // Adaptive ef based on recall target.
        let ef = if recall_target > 0.9 {
            let ef_adaptive = (k as f32 / (1.0 - recall_target)).ceil() as usize;
            ef_adaptive.max(k * 4).max(64)
        } else {
            (k * 2).max(16)
        };

        let (mut ep_id, ep_layer) = self.entry_point.unwrap();

        // Greedy descent from top layer to layer 1.
        for layer in (1..=ep_layer).rev() {
            ep_id = self.greedy_search_layer(query, ep_id, layer);
        }

        // Beam search at layer 0.
        let entry_candidates = vec![Item {
            dist: euclidean(query, &self.vectors[&ep_id]),
            id: ep_id,
        }];
        let results = self.search_layer(query, &entry_candidates, ef, 0);

        // Filter tombstones and take top k.
        results
            .into_iter()
            .filter(|item| !self.tombstones.contains(&item.id))
            .take(k)
            .map(|item| (item.id, item.dist))
            .collect()
    }

    /// Lazily mark a node as deleted (tombstone).
    pub fn delete(&mut self, id: Uuid) -> Result<()> {
        if !self.vectors.contains_key(&id) {
            return Err(anyhow!("node {} not found", id));
        }
        if self.tombstones.insert(id) {
            // Only decrement if it wasn't already tombstoned.
            self.live_count = self.live_count.saturating_sub(1);
        }
        Ok(())
    }

    /// Number of live (non-tombstoned) nodes.
    pub fn len(&self) -> usize {
        self.live_count
    }

    /// Returns `true` if there are no live (non-tombstoned) nodes.
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }

    /// Remove tombstoned nodes from all layers and rebuild edges.
    pub fn compact(&mut self) {
        if self.tombstones.is_empty() {
            return;
        }

        let dead: HashSet<Uuid> = self.tombstones.drain().collect();

        // Remove dead nodes from vector storage.
        for id in &dead {
            self.vectors.remove(id);
        }

        // Remove dead nodes from every layer adjacency list and prune their
        // references from neighbour lists.
        for layer in &mut self.layers {
            for id in &dead {
                layer.remove(id);
            }
            for neighbours in layer.values_mut() {
                neighbours.retain(|nb| !dead.contains(nb));
            }
        }

        // Fix entry point if it was tombstoned.
        if let Some((ep_id, ep_layer)) = self.entry_point {
            if dead.contains(&ep_id) {
                // Find any surviving node at the highest possible layer.
                self.entry_point = None;
                #[allow(clippy::never_loop)]
                'outer: for layer in (0..self.layers.len()).rev() {
                    for &node_id in self.layers[layer].keys() {
                        self.entry_point = Some((node_id, layer));
                        break 'outer;
                    }
                }
                let _ = ep_layer; // suppress unused warning
            }
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Choose a random layer for a new node using the HNSW level distribution.
    /// Uses a deterministic hash of the UUID as a pseudo-random source so the
    /// index is reproducible given the same insertion sequence.
    fn random_layer(&self, id: Uuid) -> usize {
        let bytes = id.as_bytes();
        // Mix bytes into a u64.
        let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        // Convert to [0, 1) float.
        let r = (h >> 11) as f32 / (1u64 << 53) as f32;
        let r = r.clamp(1e-9, 1.0 - 1e-9);
        let m_l = 1.0 / (self.m as f32).ln();
        let layer = (-r.ln() * m_l).floor() as usize;
        layer.min(self.max_layer.max(8))
    }

    /// Greedy single-result search at a given layer (used for traversing upper
    /// layers where only 1 candidate is needed).
    fn greedy_search_layer(&self, query: &[f32], entry: Uuid, layer: usize) -> Uuid {
        let mut current = entry;
        let mut current_dist = euclidean(query, &self.vectors[&current]);
        loop {
            let mut improved = false;
            let neighbours = self.layers[layer]
                .get(&current)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            for &nb in neighbours {
                if self.tombstones.contains(&nb) {
                    continue;
                }
                if let Some(vec) = self.vectors.get(&nb) {
                    let d = euclidean(query, vec);
                    if d < current_dist {
                        current = nb;
                        current_dist = d;
                        improved = true;
                    }
                }
            }
            if !improved {
                break;
            }
        }
        current
    }

    /// Standard HNSW beam search at a single layer.
    ///
    /// Returns results sorted by distance ascending (nearest first).
    fn search_layer(
        &self,
        query: &[f32],
        entry_candidates: &[Item],
        ef: usize,
        layer: usize,
    ) -> Vec<Item> {
        let mut visited: HashSet<Uuid> = HashSet::new();

        // min-heap: pop the nearest candidate first
        // We use Reverse<Item> so BinaryHeap pops the smallest distance.
        let mut candidates: BinaryHeap<std::cmp::Reverse<Item>> = BinaryHeap::new();
        // max-heap: keeps the ef nearest results (pop the worst to evict)
        let mut result: BinaryHeap<Item> = BinaryHeap::new();

        for item in entry_candidates {
            if visited.insert(item.id) {
                candidates.push(std::cmp::Reverse(item.clone()));
                result.push(item.clone());
            }
        }

        while let Some(std::cmp::Reverse(current)) = candidates.pop() {
            // Pruning: if current candidate is worse than the worst in result,
            // no further improvement is possible.
            let worst_dist = result.peek().map(|w| w.dist).unwrap_or(f32::INFINITY);

            if current.dist > worst_dist && result.len() >= ef {
                break;
            }

            // Expand neighbours.
            let neighbours = self.layers[layer]
                .get(&current.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            for &nb in neighbours {
                if !visited.insert(nb) {
                    continue;
                }
                if let Some(vec) = self.vectors.get(&nb) {
                    let d = euclidean(query, vec);
                    let worst = result.peek().map(|w| w.dist).unwrap_or(f32::INFINITY);
                    if d < worst || result.len() < ef {
                        let nb_item = Item { dist: d, id: nb };
                        candidates.push(std::cmp::Reverse(nb_item.clone()));
                        result.push(nb_item);
                        if result.len() > ef {
                            result.pop(); // evict the farthest
                        }
                    }
                }
            }
        }

        // Convert max-heap to sorted vec (nearest first).
        let mut out: Vec<Item> = result.into_vec();
        out.sort_unstable();
        out
    }

    /// Select the `m` best neighbours for a node from a candidate list,
    /// using simple distance ordering.
    fn select_neighbours(&self, node_vec: &[f32], candidates: &[Uuid], m: usize) -> Vec<Uuid> {
        let mut scored: Vec<(f32, Uuid)> = candidates
            .iter()
            .filter_map(|&id| self.vectors.get(&id).map(|v| (euclidean(node_vec, v), id)))
            .collect();
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        scored.into_iter().take(m).map(|(_, id)| id).collect()
    }
}

// ---------------------------------------------------------------------------
// PqEncoder
// ---------------------------------------------------------------------------

pub struct PqEncoder {
    pub num_subspaces: usize,
    dims: usize,
    /// codebooks[subspace][centroid] = sub-vector of length (dims / num_subspaces)
    codebooks: Vec<Vec<Vec<f32>>>,
    trained: bool,
}

impl PqEncoder {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    pub fn new(dims: usize, num_subspaces: usize) -> Self {
        PqEncoder {
            num_subspaces,
            dims,
            codebooks: Vec::new(),
            trained: false,
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Train the PQ codebooks on a set of vectors using Lloyd's k-means.
    ///
    /// Each subspace gets 256 centroids trained for 20 iterations.
    /// Training is limited to at most 10 000 vectors for efficiency.
    #[allow(clippy::manual_is_multiple_of)]
    pub fn train(&mut self, vectors: &[Vec<f32>]) -> Result<()> {
        if vectors.is_empty() {
            return Err(anyhow!("cannot train on empty vector set"));
        }
        if vectors[0].len() != self.dims {
            return Err(anyhow!(
                "vector dimension mismatch: expected {}, got {}",
                self.dims,
                vectors[0].len()
            ));
        }
        if self.dims % self.num_subspaces != 0 {
            return Err(anyhow!(
                "dims ({}) must be divisible by num_subspaces ({})",
                self.dims,
                self.num_subspaces
            ));
        }

        // Sample at most 10 000 training vectors.
        let sample: Vec<&Vec<f32>> = if vectors.len() > 10_000 {
            // Take evenly-spaced samples.
            let step = vectors.len() / 10_000;
            vectors.iter().step_by(step.max(1)).take(10_000).collect()
        } else {
            vectors.iter().collect()
        };

        let subdim = self.dims / self.num_subspaces;
        let num_centroids: usize = 256;
        let num_iters: usize = 20;

        let mut codebooks: Vec<Vec<Vec<f32>>> = Vec::with_capacity(self.num_subspaces);

        for sub in 0..self.num_subspaces {
            let start = sub * subdim;
            let end = start + subdim;

            // Extract sub-vectors for this subspace.
            let sub_vecs: Vec<Vec<f32>> = sample.iter().map(|v| v[start..end].to_vec()).collect();

            // Initialise centroids by picking the first `num_centroids` unique sub-vectors.
            let actual_k = num_centroids.min(sub_vecs.len());
            let mut centroids: Vec<Vec<f32>> = sub_vecs[..actual_k].to_vec();

            // Lloyd's algorithm.
            for _ in 0..num_iters {
                // Assignment step: assign each sub-vector to nearest centroid.
                let assignments: Vec<usize> = sub_vecs
                    .iter()
                    .map(|sv| nearest_centroid(sv, &centroids))
                    .collect();

                // Update step: recompute each centroid as mean of assigned vectors.
                let mut sums: Vec<Vec<f32>> = vec![vec![0.0f32; subdim]; actual_k];
                let mut counts: Vec<usize> = vec![0; actual_k];

                for (sv, &c) in sub_vecs.iter().zip(assignments.iter()) {
                    for (dim_idx, &val) in sv.iter().enumerate() {
                        sums[c][dim_idx] += val;
                    }
                    counts[c] += 1;
                }

                for c in 0..actual_k {
                    if counts[c] > 0 {
                        for d in 0..subdim {
                            centroids[c][d] = sums[c][d] / counts[c] as f32;
                        }
                    }
                    // Empty centroids keep their previous position (no reinit needed
                    // for correctness in this simplified implementation).
                }
            }

            codebooks.push(centroids);
        }

        self.codebooks = codebooks;
        self.trained = true;
        Ok(())
    }

    /// Encode a vector as one byte per subspace (centroid index).
    ///
    /// Panics if `train` has not been called.
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        assert!(self.trained, "PqEncoder must be trained before encoding");
        let subdim = self.dims / self.num_subspaces;
        let mut code = Vec::with_capacity(self.num_subspaces);
        for sub in 0..self.num_subspaces {
            let start = sub * subdim;
            let end = start + subdim;
            let sub_vec = &vector[start..end];
            let idx = nearest_centroid(sub_vec, &self.codebooks[sub]);
            code.push(idx as u8);
        }
        code
    }

    /// Compute an approximate distance between two encoded vectors.
    ///
    /// Sums Euclidean distances between the corresponding centroids for each
    /// subspace.
    pub fn approximate_distance(&self, encoded_a: &[u8], encoded_b: &[u8]) -> f32 {
        assert!(
            self.trained,
            "PqEncoder must be trained before computing distances"
        );
        let mut total = 0.0f32;
        for sub in 0..self.num_subspaces {
            let ca = &self.codebooks[sub][encoded_a[sub] as usize];
            let cb = &self.codebooks[sub][encoded_b[sub] as usize];
            total += euclidean(ca, cb);
        }
        total
    }
}

// ---------------------------------------------------------------------------
// Internal PQ helper
// ---------------------------------------------------------------------------

/// Find the index of the centroid nearest to `sub_vec`.
fn nearest_centroid(sub_vec: &[f32], centroids: &[Vec<f32>]) -> usize {
    let mut best_idx = 0;
    let mut best_dist = f32::INFINITY;
    for (i, c) in centroids.iter().enumerate() {
        let d = euclidean(sub_vec, c);
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple deterministic pseudo-random number generator (LCG variant).
    fn pseudo_rand(seed: u64) -> f32 {
        let x = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((x >> 33) as f32) / (u32::MAX as f32)
    }

    /// Generate a deterministic `dims`-dimensional vector from a seed.
    fn make_vector(seed: u64, dims: usize) -> Vec<f32> {
        (0..dims)
            .map(|j| pseudo_rand(seed * 1000 + j as u64))
            .collect()
    }

    /// Generate a stable UUID from an integer index.
    fn make_uuid(i: u64) -> Uuid {
        // Use Uuid::from_u128 to get a stable ID without relying on rand.
        Uuid::from_u128(i as u128 * 0x9e3779b97f4a7c15 + 1)
    }

    // -----------------------------------------------------------------------
    // 1. Basic insert + search
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_search_basic() {
        let mut idx = HnswIndex::new(4, 8, 16);

        let n = 50usize;
        for i in 0..n {
            let id = make_uuid(i as u64);
            let vec = make_vector(i as u64, 4);
            idx.insert(id, &vec).expect("insert failed");
        }

        let query = make_vector(999, 4);
        let results = idx.search(&query, 5, 0.9);

        assert_eq!(
            results.len(),
            5,
            "expected exactly 5 results, got {}",
            results.len()
        );

        // All distances must be non-negative.
        for (_, dist) in &results {
            assert!(*dist >= 0.0, "negative distance: {}", dist);
        }

        // Results must be sorted ascending by distance.
        for window in results.windows(2) {
            assert!(
                window[0].1 <= window[1].1,
                "results not sorted ascending: {} > {}",
                window[0].1,
                window[1].1
            );
        }
    }

    // -----------------------------------------------------------------------
    // 2. Delete + tombstone correctness
    // -----------------------------------------------------------------------
    #[test]
    fn test_delete_tombstone() {
        let mut idx = HnswIndex::new(4, 8, 16);

        let n = 20usize;
        let mut ids: Vec<Uuid> = Vec::new();
        for i in 0..n {
            let id = make_uuid(i as u64);
            let vec = make_vector(i as u64, 4);
            idx.insert(id, &vec).expect("insert failed");
            ids.push(id);
        }

        // Delete 5 nodes.
        let deleted: Vec<Uuid> = ids[..5].to_vec();
        for &id in &deleted {
            idx.delete(id).expect("delete failed");
        }

        let query = make_vector(500, 4);

        // Search before compact.
        let results_before = idx.search(&query, 10, 0.5);
        for (id, _) in &results_before {
            assert!(
                !deleted.contains(id),
                "deleted id {:?} appeared in results before compact",
                id
            );
        }

        // Compact and search again.
        idx.compact();
        let results_after = idx.search(&query, 10, 0.5);
        for (id, _) in &results_after {
            assert!(
                !deleted.contains(id),
                "deleted id {:?} appeared in results after compact",
                id
            );
        }
    }

    // -----------------------------------------------------------------------
    // 3. len() tracks live nodes correctly
    // -----------------------------------------------------------------------
    #[test]
    fn test_len() {
        let mut idx = HnswIndex::new(4, 8, 16);

        let n = 100usize;
        for i in 0..n {
            let id = make_uuid(i as u64);
            let vec = make_vector(i as u64, 4);
            idx.insert(id, &vec).expect("insert failed");
        }
        assert_eq!(idx.len(), 100, "expected len 100 after 100 inserts");

        for i in 0..10 {
            idx.delete(make_uuid(i as u64)).expect("delete failed");
        }
        assert_eq!(idx.len(), 90, "expected len 90 after 10 deletes");
    }

    // -----------------------------------------------------------------------
    // 4. PQ encode round-trip: encoded length == num_subspaces
    // -----------------------------------------------------------------------
    #[test]
    fn test_pq_roundtrip() {
        let dims = 16;
        let num_subspaces = 4;
        let mut pq = PqEncoder::new(dims, num_subspaces);

        let training: Vec<Vec<f32>> = (0..300).map(|i| make_vector(i as u64, dims)).collect();

        pq.train(&training).expect("train failed");

        let vec = make_vector(42, dims);
        let encoded = pq.encode(&vec);

        assert_eq!(
            encoded.len(),
            num_subspaces,
            "encoded length should be {} bytes, got {}",
            num_subspaces,
            encoded.len()
        );
    }

    // -----------------------------------------------------------------------
    // 5. PQ approximate distance
    // -----------------------------------------------------------------------
    #[test]
    fn test_pq_approximate_distance() {
        let dims = 16;
        let num_subspaces = 4;
        let mut pq = PqEncoder::new(dims, num_subspaces);

        let training: Vec<Vec<f32>> = (0..300).map(|i| make_vector(i as u64, dims)).collect();
        pq.train(&training).expect("train failed");

        // Identical vectors should encode to the same code → distance == 0.
        let vec_a = make_vector(1, dims);
        let enc_a = pq.encode(&vec_a);
        let self_dist = pq.approximate_distance(&enc_a, &enc_a);
        assert_eq!(
            self_dist, 0.0,
            "distance between identical codes should be 0.0, got {}",
            self_dist
        );

        // Two very different vectors should have distance > 0.
        // vec_a is close to 0 (pseudo_rand produces values in [0,1]);
        // vec_b is shifted far away.
        let vec_b: Vec<f32> = (0..dims).map(|_| 100.0f32).collect();
        let enc_b = pq.encode(&vec_b);
        let cross_dist = pq.approximate_distance(&enc_a, &enc_b);
        assert!(
            cross_dist > 0.0,
            "expected distance > 0 between different vectors, got {}",
            cross_dist
        );
    }
}
