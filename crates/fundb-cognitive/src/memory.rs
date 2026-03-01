use anyhow::{anyhow, Result};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryType {
    Semantic,
    Episodic,
    Procedural,
}

pub struct RememberOptions {
    pub importance: f32,
    pub memory_type: MemoryType,
    pub decay_rate: f32,
}

pub struct RecallWeights {
    pub semantic: f32,
    pub recency: f32,
    pub importance: f32,
    // must sum ~1.0 — caller's responsibility
}

pub struct RecallComponents {
    pub semantic_score: f32,
    pub recency_score: f32,
    pub importance_score: f32,
}

pub struct MemoryResult {
    pub memory_id: Uuid,
    pub content: String,
    pub score: f32,
    pub components: RecallComponents,
}

// ---------------------------------------------------------------------------
// Internal storage model
// ---------------------------------------------------------------------------

struct MemoryEntry {
    id: Uuid,
    agent_id: String,
    content: String,
    embedding: Vec<f32>,
    importance: f32,
    memory_type: MemoryType,
    decay_rate: f32,
    created_at: i64,
    last_accessed: i64,
    access_count: u32,
    confidence: f32,
    tombstone: bool,
}

pub struct AgentMemory {
    entries: Vec<MemoryEntry>,
    clock: i64,
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Polynomial hash used for bag-of-words bucketing.
fn djb2_hash(word: &str) -> u64 {
    word.bytes()
        .fold(5381u64, |acc, b| acc.wrapping_mul(33).wrapping_add(b as u64))
}

/// Split `text` on whitespace and common punctuation, lowercase, skip empties.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Produce a 64-dimensional bag-of-words unit vector for `text`.
fn bag_of_words_embed(text: &str) -> Vec<f32> {
    const DIM: usize = 64;
    let mut vec = vec![0.0_f32; DIM];

    for word in tokenize(text) {
        let bucket = (djb2_hash(&word) % DIM as u64) as usize;
        vec[bucket] += 1.0;
    }

    // Normalize to unit vector.
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }

    vec
}

/// Cosine similarity between two slices (shorter one is zero-padded).
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().max(b.len());
    let dot: f32 = (0..len)
        .map(|i| {
            let av = a.get(i).copied().unwrap_or(0.0);
            let bv = b.get(i).copied().unwrap_or(0.0);
            av * bv
        })
        .sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na * nb)).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------------------
// AgentMemory implementation
// ---------------------------------------------------------------------------

impl AgentMemory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            clock: 0,
        }
    }

    /// Store a new memory for `agent_id` and return its UUID.
    pub fn remember(
        &mut self,
        agent_id: &str,
        content: &str,
        opts: RememberOptions,
    ) -> Result<Uuid> {
        let embedding = bag_of_words_embed(content);
        let id = Uuid::now_v7();
        let now = self.clock;

        let entry = MemoryEntry {
            id,
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            embedding,
            importance: opts.importance,
            memory_type: opts.memory_type,
            decay_rate: opts.decay_rate,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            confidence: opts.importance.min(1.0),
            tombstone: false,
        };

        self.clock += 1;
        self.entries.push(entry);

        Ok(id)
    }

    /// Retrieve the top-`top_k` memories for `agent_id` that best match `query`.
    pub fn recall(
        &self,
        agent_id: &str,
        query: &str,
        weights: RecallWeights,
        top_k: usize,
    ) -> Vec<MemoryResult> {
        let query_embedding = bag_of_words_embed(query);

        let mut scored: Vec<(usize, f32, RecallComponents)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.tombstone && e.agent_id == agent_id)
            .map(|(idx, e)| {
                let semantic_score = cosine_sim(&query_embedding, &e.embedding);
                let recency_score =
                    1.0 / (1.0 + (self.clock - e.created_at) as f32);
                let importance_score = e.importance;

                let total = weights.semantic * semantic_score
                    + weights.recency * recency_score
                    + weights.importance * importance_score;

                (
                    idx,
                    total,
                    RecallComponents {
                        semantic_score,
                        recency_score,
                        importance_score,
                    },
                )
            })
            .collect();

        // Sort descending by total score, then take top_k.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        scored
            .into_iter()
            .map(|(idx, score, components)| {
                let e = &self.entries[idx];
                MemoryResult {
                    memory_id: e.id,
                    content: e.content.clone(),
                    score,
                    components,
                }
            })
            .collect()
    }

    /// Mark the memory with `memory_id` as deleted (tombstone).
    pub fn forget(&mut self, memory_id: Uuid) -> Result<()> {
        match self.entries.iter_mut().find(|e| e.id == memory_id) {
            Some(e) => {
                e.tombstone = true;
                Ok(())
            }
            None => Err(anyhow!("memory not found: {}", memory_id)),
        }
    }

    /// Merge near-duplicate memories for `agent_id` (cosine similarity > 0.9).
    ///
    /// For each similar pair the entry with the *lower* importance is tombstoned.
    /// Returns the number of entries tombstoned.
    pub fn consolidate(&mut self, agent_id: &str) -> Result<u32> {
        // Collect indices of live entries belonging to this agent.
        let candidates: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.tombstone && e.agent_id == agent_id)
            .map(|(i, _)| i)
            .collect();

        let mut tombstoned: u32 = 0;

        // O(n²) pairwise comparison — acceptable for an in-memory store.
        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                let idx_a = candidates[i];
                let idx_b = candidates[j];

                // Skip if either has already been tombstoned in this pass.
                if self.entries[idx_a].tombstone || self.entries[idx_b].tombstone {
                    continue;
                }

                let sim =
                    cosine_sim(&self.entries[idx_a].embedding, &self.entries[idx_b].embedding);

                if sim > 0.9 {
                    // Keep the higher-importance entry; tombstone the other.
                    let keep_a =
                        self.entries[idx_a].importance >= self.entries[idx_b].importance;
                    let drop_idx = if keep_a { idx_b } else { idx_a };
                    self.entries[drop_idx].tombstone = true;
                    tombstoned += 1;
                }
            }
        }

        Ok(tombstoned)
    }

    /// Apply one step of confidence decay to every live memory.
    pub fn decay_all(&mut self) -> Result<()> {
        for entry in self.entries.iter_mut().filter(|e| !e.tombstone) {
            entry.confidence = (entry.confidence - entry.decay_rate * 0.01).max(0.0);
        }
        Ok(())
    }
}

impl Default for AgentMemory {
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

    fn default_opts(importance: f32) -> RememberOptions {
        RememberOptions {
            importance,
            memory_type: MemoryType::Semantic,
            decay_rate: 0.1,
        }
    }

    fn semantic_weights() -> RecallWeights {
        RecallWeights {
            semantic: 1.0,
            recency: 0.0,
            importance: 0.0,
        }
    }

    fn balanced_weights() -> RecallWeights {
        RecallWeights {
            semantic: 0.33,
            recency: 0.34,
            importance: 0.33,
        }
    }

    // -----------------------------------------------------------------------

    #[test]
    fn test_remember_and_recall_basic() {
        let mut mem = AgentMemory::new();
        mem.remember("agent1", "the sky is blue", default_opts(0.5))
            .unwrap();

        let results = mem.recall("agent1", "sky color", semantic_weights(), 5);
        assert!(!results.is_empty(), "recall should return at least one result");
        assert!(
            results[0].score > 0.0,
            "score should be positive, got {}",
            results[0].score
        );
    }

    #[test]
    fn test_recall_returns_correct_agent() {
        let mut mem = AgentMemory::new();
        mem.remember("agent_a", "rivers flow to the sea", default_opts(0.5))
            .unwrap();
        mem.remember("agent_b", "mountains are tall and cold", default_opts(0.5))
            .unwrap();

        let results = mem.recall("agent_a", "flow", semantic_weights(), 10);
        for r in &results {
            // Content from agent_b must not appear.
            assert!(
                !r.content.contains("mountains"),
                "agent_b memory leaked into agent_a recall"
            );
        }

        let results_b = mem.recall("agent_b", "mountains", semantic_weights(), 10);
        for r in &results_b {
            assert!(
                !r.content.contains("rivers"),
                "agent_a memory leaked into agent_b recall"
            );
        }
    }

    #[test]
    fn test_forget_removes_from_recall() {
        let mut mem = AgentMemory::new();
        let id = mem
            .remember("agent1", "the ocean is vast", default_opts(0.5))
            .unwrap();

        // Confirm it is recalled before forgetting.
        let before = mem.recall("agent1", "ocean", semantic_weights(), 5);
        assert!(!before.is_empty());

        mem.forget(id).unwrap();

        let after = mem.recall("agent1", "ocean", semantic_weights(), 5);
        assert!(
            after.iter().all(|r| r.memory_id != id),
            "forgotten memory should not appear in recall"
        );
    }

    #[test]
    fn test_consolidate_merges_duplicates() {
        let mut mem = AgentMemory::new();
        // Two memories with identical content → cosine similarity == 1.0 > 0.9.
        mem.remember("agent1", "the quick brown fox", default_opts(0.8))
            .unwrap();
        mem.remember("agent1", "the quick brown fox", default_opts(0.6))
            .unwrap();

        let merged = mem.consolidate("agent1").unwrap();
        assert_eq!(merged, 1, "exactly one duplicate should be tombstoned");

        let results = mem.recall("agent1", "quick fox", semantic_weights(), 10);
        assert_eq!(
            results.len(),
            1,
            "only one memory should survive after consolidation"
        );
        // The survivor should be the more important one.
        assert!(
            (results[0].components.importance_score - 0.8).abs() < 1e-5,
            "higher-importance memory should survive"
        );
    }

    #[test]
    fn test_decay_reduces_confidence() {
        let mut mem = AgentMemory::new();
        mem.remember("agent1", "some content", default_opts(0.8))
            .unwrap();

        let initial_confidence = mem.entries[0].confidence;
        mem.decay_all().unwrap();
        let after_confidence = mem.entries[0].confidence;

        assert!(
            after_confidence < initial_confidence,
            "confidence should decrease after decay_all: {} -> {}",
            initial_confidence,
            after_confidence
        );
    }

    #[test]
    fn test_recall_weights_semantic_dominant() {
        let mut mem = AgentMemory::new();
        // High semantic match for "ocean".
        mem.remember("agent1", "the ocean is wide and deep", default_opts(0.3))
            .unwrap();
        // Lower semantic match but higher importance — should NOT win with semantic-only weights.
        mem.remember("agent1", "the forest is green", default_opts(0.9))
            .unwrap();

        let results = mem.recall(
            "agent1",
            "ocean water",
            RecallWeights {
                semantic: 1.0,
                recency: 0.0,
                importance: 0.0,
            },
            2,
        );

        assert_eq!(results.len(), 2);
        assert!(
            results[0].content.contains("ocean"),
            "most semantically similar memory should rank first"
        );
    }

    #[test]
    fn test_recall_weights_importance_dominant() {
        let mut mem = AgentMemory::new();
        mem.remember("agent1", "low importance fact", default_opts(0.1))
            .unwrap();
        mem.remember("agent1", "high importance fact", default_opts(0.95))
            .unwrap();

        let results = mem.recall(
            "agent1",
            "fact",
            RecallWeights {
                semantic: 0.0,
                recency: 0.0,
                importance: 1.0,
            },
            2,
        );

        assert_eq!(results.len(), 2);
        assert!(
            results[0].content.contains("high importance"),
            "highest-importance memory should rank first, got: {}",
            results[0].content
        );
    }

    #[test]
    fn test_recall_top_k_limit() {
        let mut mem = AgentMemory::new();
        for i in 0..10 {
            mem.remember("agent1", &format!("memory number {}", i), default_opts(0.5))
                .unwrap();
        }

        let results = mem.recall("agent1", "memory", balanced_weights(), 5);
        assert_eq!(results.len(), 5, "recall should return exactly top_k results");
    }
}
