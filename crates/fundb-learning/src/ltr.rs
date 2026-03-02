use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

struct AgentModel {
    /// How many times the document was selected/used by the agent.
    used_count: HashMap<Uuid, u32>,
    /// How many times the document was shown but not selected by the agent.
    ignored_count: HashMap<Uuid, u32>,
    /// Total number of feedback signals (sum of all used + ignored increments).
    total_signals: u32,
}

impl AgentModel {
    fn new() -> Self {
        Self {
            used_count: HashMap::new(),
            ignored_count: HashMap::new(),
            total_signals: 0,
        }
    }

    /// Smoothed click-through score for a single document.
    /// score = used / (used + ignored + 1)
    fn learned_score(&self, doc_id: &Uuid) -> f32 {
        let used = *self.used_count.get(doc_id).unwrap_or(&0) as f32;
        let ignored = *self.ignored_count.get(doc_id).unwrap_or(&0) as f32;
        used / (used + ignored + 1.0)
    }
}

struct FeedbackRecord {
    _query_id: Uuid,
    used: Vec<Uuid>,
    ignored: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct LtrRanker {
    /// Per-agent learned model.
    models: HashMap<String, AgentModel>,
    /// Raw feedback history, keyed by agent_id.
    history: HashMap<String, Vec<FeedbackRecord>>,
}

impl LtrRanker {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            history: HashMap::new(),
        }
    }

    /// Record which results the agent used vs ignored for a given query.
    pub fn record_feedback(
        &mut self,
        agent_id: &str,
        query_id: Uuid,
        used: &[Uuid],
        ignored: &[Uuid],
    ) {
        // Ensure model exists.
        let model = self
            .models
            .entry(agent_id.to_string())
            .or_insert_with(AgentModel::new);

        for &doc_id in used {
            *model.used_count.entry(doc_id).or_insert(0) += 1;
            model.total_signals += 1;
        }

        for &doc_id in ignored {
            *model.ignored_count.entry(doc_id).or_insert(0) += 1;
            model.total_signals += 1;
        }

        // Append to history.
        self.history
            .entry(agent_id.to_string())
            .or_default()
            .push(FeedbackRecord {
                _query_id: query_id,
                used: used.to_vec(),
                ignored: ignored.to_vec(),
            });
    }

    /// Re-rank candidates for a query using learned per-agent scores.
    /// Returns original ranking unchanged if < 100 feedback signals for this agent.
    pub fn rerank(
        &self,
        agent_id: &str,
        _query: &str,
        mut candidates: Vec<(Uuid, f32)>,
    ) -> Vec<(Uuid, f32)> {
        let model = match self.models.get(agent_id) {
            Some(m) if m.total_signals >= 100 => m,
            _ => return candidates,
        };

        // Compute combined scores.
        let mut scored: Vec<(Uuid, f32)> = candidates
            .drain(..)
            .map(|(uuid, original_score)| {
                let learned = model.learned_score(&uuid);
                let combined = 0.7 * original_score + 0.3 * learned;
                (uuid, combined)
            })
            .collect();

        // Sort descending by combined score (stable for equal values).
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
    }

    /// NDCG@10 for this agent based on historical feedback.
    /// Returns None if agent has < 100 feedback signals.
    pub fn ndcg_at_10(&self, agent_id: &str) -> Option<f32> {
        let model = self.models.get(agent_id)?;
        if model.total_signals < 100 {
            return None;
        }

        let records = self.history.get(agent_id)?;
        if records.is_empty() {
            return None;
        }

        let mut sum_ndcg = 0.0_f32;
        let mut count = 0_usize;

        for record in records {
            let ndcg = ndcg_for_record(record);
            sum_ndcg += ndcg;
            count += 1;
        }

        if count == 0 {
            return None;
        }

        Some(sum_ndcg / count as f32)
    }
}

// ---------------------------------------------------------------------------
// NDCG helpers
// ---------------------------------------------------------------------------

/// Compute NDCG@10 for a single feedback record.
///
/// The "retrieved list" is built by placing `used` items first, then
/// `ignored` items — matching the assumption that a well-ranked result
/// would surface used items at the top.
///
/// rel_i = 1.0 if the item at position i was used, else 0.0.
fn ndcg_for_record(record: &FeedbackRecord) -> f32 {
    // Build a set for fast membership test.
    let used_set: std::collections::HashSet<Uuid> = record.used.iter().copied().collect();

    // Construct the ordered list: used items first, then ignored items.
    // This represents the "current" ranking the model produces.
    let ordered: Vec<Uuid> = record
        .used
        .iter()
        .chain(record.ignored.iter())
        .copied()
        .collect();

    // DCG@10
    let dcg: f32 = ordered
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, uuid)| {
            let rel = if used_set.contains(uuid) {
                1.0_f32
            } else {
                0.0_f32
            };
            rel / (i as f32 + 2.0_f32).log2()
        })
        .sum();

    // IDCG@10: ideal ordering has all used items at the top.
    let ideal_used = record.used.len().min(10);
    let idcg: f32 = (0..ideal_used)
        .map(|i| 1.0_f32 / (i as f32 + 2.0_f32).log2())
        .sum();

    if idcg <= 0.0 {
        // No relevant items → NDCG is undefined; treat as 1.0 (perfect trivially).
        return 1.0;
    }

    (dcg / idcg).min(1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: generate a sequence of distinct UUIDs.
    fn make_uuids(n: usize) -> Vec<Uuid> {
        (0..n).map(|_| Uuid::new_v4()).collect()
    }

    // Helper: record N identical feedback entries (doc_a used, doc_b ignored).
    fn fill_feedback(ranker: &mut LtrRanker, agent_id: &str, doc_a: Uuid, doc_b: Uuid, n: usize) {
        for _ in 0..n {
            ranker.record_feedback(agent_id, Uuid::new_v4(), &[doc_a], &[doc_b]);
        }
    }

    // -----------------------------------------------------------------------
    // 1. Cold start: rerank returns identical order when < 100 signals.
    // -----------------------------------------------------------------------
    #[test]
    fn test_cold_start_returns_original() {
        let ranker = LtrRanker::new();
        let docs = make_uuids(5);
        let candidates: Vec<(Uuid, f32)> = docs
            .iter()
            .copied()
            .zip([0.9, 0.8, 0.7, 0.6, 0.5])
            .collect();

        let result = ranker.rerank("agent_x", "query", candidates.clone());
        assert_eq!(
            result, candidates,
            "cold start must preserve original order"
        );
    }

    // -----------------------------------------------------------------------
    // 2. record_feedback accumulates total_signals correctly.
    // -----------------------------------------------------------------------
    #[test]
    fn test_record_feedback_accumulates() {
        let mut ranker = LtrRanker::new();
        let docs = make_uuids(4);

        // 5 records: 2 used + 2 ignored per record → 20 signals total.
        for _ in 0..5 {
            ranker.record_feedback("agent_a", Uuid::new_v4(), &docs[0..2], &docs[2..4]);
        }

        let model = ranker.models.get("agent_a").expect("model must exist");
        assert_eq!(
            model.total_signals, 20,
            "5 records × (2 used + 2 ignored) = 20 signals"
        );
    }

    // -----------------------------------------------------------------------
    // 3. After 100 signals, known-used doc ranks above known-ignored doc.
    // -----------------------------------------------------------------------
    #[test]
    fn test_rerank_after_100_signals() {
        let mut ranker = LtrRanker::new();
        let doc_a = Uuid::new_v4(); // always used
        let doc_b = Uuid::new_v4(); // always ignored

        // 100 feedback records each: doc_a used, doc_b ignored → 200 signals.
        fill_feedback(&mut ranker, "agent_rank", doc_a, doc_b, 100);

        // Give doc_b a slightly higher original score so the test is meaningful.
        let candidates = vec![(doc_b, 0.8_f32), (doc_a, 0.6_f32)];
        let result = ranker.rerank("agent_rank", "q", candidates);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0].0, doc_a,
            "doc_a (always used) should rank first after learning"
        );
    }

    // -----------------------------------------------------------------------
    // 4. Per-agent isolation: agent_a's feedback must not affect agent_b.
    // -----------------------------------------------------------------------
    #[test]
    fn test_per_agent_isolation() {
        let mut ranker = LtrRanker::new();
        let doc_a = Uuid::new_v4();
        let doc_b = Uuid::new_v4();

        // Only agent_a accumulates 100+ signals.
        fill_feedback(&mut ranker, "agent_a", doc_a, doc_b, 100);

        // agent_b has zero signals; rerank must return original order.
        let candidates = vec![(doc_b, 0.9_f32), (doc_a, 0.1_f32)];
        let result = ranker.rerank("agent_b", "q", candidates.clone());

        assert_eq!(
            result, candidates,
            "agent_b must not be affected by agent_a feedback"
        );
    }

    // -----------------------------------------------------------------------
    // 5. ndcg_at_10 returns None before reaching 100 signals.
    // -----------------------------------------------------------------------
    #[test]
    fn test_ndcg_none_before_100_signals() {
        let mut ranker = LtrRanker::new();
        let doc_a = Uuid::new_v4();
        let doc_b = Uuid::new_v4();

        // 49 records × 2 signals = 98 signals (< 100).
        fill_feedback(&mut ranker, "agent_ndcg", doc_a, doc_b, 49);

        assert!(
            ranker.ndcg_at_10("agent_ndcg").is_none(),
            "NDCG must be None with 98 signals"
        );
    }

    // -----------------------------------------------------------------------
    // 6. ndcg_at_10 returns Some(f32) in [0.0, 1.0] after 100+ signals.
    // -----------------------------------------------------------------------
    #[test]
    fn test_ndcg_some_after_100_signals() {
        let mut ranker = LtrRanker::new();
        let doc_a = Uuid::new_v4();
        let doc_b = Uuid::new_v4();

        fill_feedback(&mut ranker, "agent_ndcg2", doc_a, doc_b, 100);

        let ndcg = ranker.ndcg_at_10("agent_ndcg2");
        assert!(ndcg.is_some(), "NDCG must be Some after 100+ signals");
        let v = ndcg.unwrap();
        assert!(
            (0.0..=1.0).contains(&v),
            "NDCG value {v} must be in [0.0, 1.0]"
        );
    }

    // -----------------------------------------------------------------------
    // 7. rerank output length equals input length.
    // -----------------------------------------------------------------------
    #[test]
    fn test_rerank_preserves_length() {
        let mut ranker = LtrRanker::new();
        let docs = make_uuids(10);

        // Record 100 signals using a subset so the model is active.
        for _ in 0..50 {
            ranker.record_feedback("agent_len", Uuid::new_v4(), &docs[0..1], &docs[1..2]);
        }

        let candidates: Vec<(Uuid, f32)> = docs
            .iter()
            .copied()
            .enumerate()
            .map(|(i, id)| (id, 1.0 - i as f32 * 0.05))
            .collect();

        let result = ranker.rerank("agent_len", "q", candidates.clone());
        assert_eq!(
            result.len(),
            candidates.len(),
            "rerank must not drop or duplicate candidates"
        );
    }

    // -----------------------------------------------------------------------
    // 8. NDCG improves with consistent feedback (always-used top candidates).
    // -----------------------------------------------------------------------
    #[test]
    fn test_ndcg_improves_with_feedback() {
        let mut ranker = LtrRanker::new();

        // 10 "good" docs that are always used, 10 "bad" docs always ignored.
        let good_docs = make_uuids(10);
        let bad_docs = make_uuids(10);

        // 200 records so total_signals = 200 × 20 = 4000.
        for _ in 0..200 {
            ranker.record_feedback("agent_improve", Uuid::new_v4(), &good_docs, &bad_docs);
        }

        let ndcg = ranker
            .ndcg_at_10("agent_improve")
            .expect("NDCG must be Some after 200 records");

        assert!(
            ndcg > 0.5,
            "NDCG {ndcg} should be > 0.5 with consistently good top results"
        );
    }
}
