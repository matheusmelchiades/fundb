/// Contradiction Detection for FunDB (STORY-5-3).
///
/// Detects when a newly inserted [`FunRecord`] contradicts existing records in the
/// store, using tag-based bag-of-words overlap and optional vector (cosine) similarity.
///
/// # Tag convention
/// Tags are stored as [`Source`] `origin` strings on each `FunRecord`.  Every entry in
/// `record._sources` whose `origin` does not start with `"contradicts:"` is treated as
/// a plain tag.  Cross-link provenance entries added by [`ContradictionDetector::auto_link`]
/// use the `"contradicts:<hex_id>"` prefix so they are distinguishable.
///
/// # Polarity heuristics
/// - **Conflicting**: a candidate tag is the semantic negation of a new tag
///   (prefixed with `"not_"` / `"no_"`, contains `"anti"`, suffixed with `"_false"`,
///   or is the explicit negation `"not_<tag>"`), OR similarity > 0.7 and confidence
///   difference > 0.5.
/// - **Corroborating**: everything else that passes the similarity threshold (> 0.3).
use fundb_core::{FunRecord, Source, SourceMethod};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of a single contradiction check against one existing record.
#[derive(Debug, Clone)]
pub struct ContradictionCandidate {
    /// UUID of the existing record that may contradict the new one.
    pub existing_id: Uuid,
    /// Combined similarity score (0.0–1.0) used to rank candidates.
    pub similarity: f32,
    /// Whether the candidate conflicts with or corroborates the new record.
    pub polarity: Polarity,
}

/// Semantic relationship between two records with respect to their content.
#[derive(Debug, Clone, PartialEq)]
pub enum Polarity {
    /// The records assert opposing facts.
    Conflicting,
    /// The records assert compatible or reinforcing facts.
    Corroborating,
}

/// Event emitted when a contradiction pair is discovered.
///
/// Observers (e.g. an event bus or notification channel) can subscribe to
/// `ContradictionEvent` to react to newly detected contradictions.
#[derive(Debug, Clone)]
pub struct ContradictionEvent {
    /// UUID of the first record in the contradiction pair.
    pub record_a_id: Uuid,
    /// UUID of the second record in the contradiction pair.
    pub record_b_id: Uuid,
    /// Similarity-based strength of the contradiction (0.0–1.0).
    pub strength: f32,
    /// Whether the relationship is conflicting or corroborating.
    pub polarity: Polarity,
}

// ---------------------------------------------------------------------------
// ContradictionDetector
// ---------------------------------------------------------------------------

/// Stateless engine for detecting and linking contradicting [`FunRecord`]s.
///
/// All methods are pure functions (no `&self` receiver) — no state is held.
pub struct ContradictionDetector;

impl ContradictionDetector {
    // -----------------------------------------------------------------------
    // check_insert
    // -----------------------------------------------------------------------

    /// Check whether `new` contradicts any record in `candidates`.
    ///
    /// For each candidate:
    /// 1. Compute tag-based (bag-of-words) similarity over `_sources` origins.
    /// 2. Also compute cosine similarity on the `"text"` vector, if present in both.
    /// 3. Take `max(tag_sim, vector_sim)` as the combined score.
    /// 4. Compute [`Polarity`] via [`Self::polarity`].
    /// 5. Include the candidate if combined similarity > 0.3.
    ///
    /// Results are sorted by similarity descending.
    pub fn check_insert(new: &FunRecord, candidates: &[FunRecord]) -> Vec<ContradictionCandidate> {
        let new_tags = tags_of(new);

        let mut results: Vec<ContradictionCandidate> = candidates
            .iter()
            .filter_map(|candidate| {
                let cand_tags = tags_of(candidate);

                // --- tag-based bag-of-words overlap -------------------------
                let tag_sim = tag_similarity(&new_tags, &cand_tags);

                // --- optional cosine similarity on "text" vector ------------
                let vector_sim = cosine_text_similarity(new, candidate);

                // --- combined score -----------------------------------------
                let similarity = tag_sim.max(vector_sim);

                if similarity <= 0.3 {
                    return None;
                }

                let polarity = Self::polarity_with_sim(new, candidate, similarity);

                Some(ContradictionCandidate {
                    existing_id: candidate._id,
                    similarity,
                    polarity,
                })
            })
            .collect();

        // Sort by similarity descending (highest first).
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    // -----------------------------------------------------------------------
    // polarity
    // -----------------------------------------------------------------------

    /// Compute the [`Polarity`] between two records based on their tags and confidence.
    ///
    /// This is a public wrapper that recomputes similarity internally.  For internal
    /// use within `check_insert` prefer [`Self::polarity_with_sim`] to avoid
    /// re-computing similarity.
    pub fn polarity(a: &FunRecord, b: &FunRecord) -> Polarity {
        let a_tags = tags_of(a);
        let b_tags = tags_of(b);
        let sim = tag_similarity(&a_tags, &b_tags).max(cosine_text_similarity(a, b));
        Self::polarity_with_sim(a, b, sim)
    }

    // -----------------------------------------------------------------------
    // auto_link
    // -----------------------------------------------------------------------

    /// Cross-link two records as contradictions and decay both confidences.
    ///
    /// Effects:
    /// - `record_a._confidence` decremented by `0.2 × strength` (clamped at 0.0).
    /// - `record_b._confidence` decremented by `0.2 × strength` (clamped at 0.0).
    /// - A `Source { origin: "contradicts:<hex_b>" }` entry pushed to `record_a._sources`.
    /// - A `Source { origin: "contradicts:<hex_a>" }` entry pushed to `record_b._sources`.
    pub fn auto_link(record_a: &mut FunRecord, record_b: &mut FunRecord, strength: f32) {
        // Decrement confidence of both records.
        record_a._confidence = (record_a._confidence - 0.2 * strength).max(0.0);
        record_b._confidence = (record_b._confidence - 0.2 * strength).max(0.0);

        // Cross-link via _sources (encode contradiction provenance as origin strings).
        let hex_b = id_as_hex(&record_b._id);
        let hex_a = id_as_hex(&record_a._id);

        record_a._sources.push(Source {
            origin: format!("contradicts:{}", hex_b),
            timestamp: now_ns(),
            method: SourceMethod::Inference,
            confidence: strength,
        });

        record_b._sources.push(Source {
            origin: format!("contradicts:{}", hex_a),
            timestamp: now_ns(),
            method: SourceMethod::Inference,
            confidence: strength,
        });
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Extract the plain (non-contradiction) tags from a record's `_sources` origins.
///
/// A tag is any `_sources[i].origin` value that does NOT start with `"contradicts:"`.
fn tags_of(record: &FunRecord) -> Vec<String> {
    record
        ._sources
        .iter()
        .filter(|s| !s.origin.starts_with("contradicts:"))
        .map(|s| s.origin.clone())
        .collect()
}

/// Bag-of-words Jaccard-style overlap between two tag lists.
///
/// `similarity = |new_tags ∩ candidate_tags| / max(|new_tags|, |candidate_tags|, 1)`
fn tag_similarity(new_tags: &[String], candidate_tags: &[String]) -> f32 {
    let denom = new_tags.len().max(candidate_tags.len()).max(1) as f32;

    let intersection = new_tags
        .iter()
        .filter(|t| candidate_tags.contains(t))
        .count() as f32;

    intersection / denom
}

/// Cosine similarity on the `"text"` vector key, if both records carry one.
///
/// Returns `0.0` when either record lacks a `"text"` vector or when one of the
/// norms is zero.
fn cosine_text_similarity(a: &FunRecord, b: &FunRecord) -> f32 {
    let va = match a._vectors.get("text") {
        Some(v) => v,
        None => return 0.0,
    };
    let vb = match b._vectors.get("text") {
        Some(v) => v,
        None => return 0.0,
    };

    let dot: f32 = va.iter().zip(vb.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = va.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = vb.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Internal polarity computation that accepts a precomputed similarity score to
/// avoid redundant work inside `check_insert`.
fn polarity_with_sim_internal(
    new_tags: &[String],
    candidate_tags: &[String],
    new_confidence: f32,
    candidate_confidence: f32,
    similarity: f32,
) -> Polarity {
    // --- negation heuristic -------------------------------------------------
    for new_tag in new_tags {
        for cand_tag in candidate_tags {
            if is_negation_pair(new_tag, cand_tag) {
                return Polarity::Conflicting;
            }
        }
    }

    // --- confidence-divergence heuristic ------------------------------------
    if similarity > 0.7 && (new_confidence - candidate_confidence).abs() > 0.5 {
        return Polarity::Conflicting;
    }

    Polarity::Corroborating
}

impl ContradictionDetector {
    /// Internal helper: polarity given a precomputed similarity value.
    fn polarity_with_sim(a: &FunRecord, b: &FunRecord, similarity: f32) -> Polarity {
        let a_tags = tags_of(a);
        let b_tags = tags_of(b);
        polarity_with_sim_internal(&a_tags, &b_tags, a._confidence, b._confidence, similarity)
    }
}

/// Returns `true` if `candidate_tag` is a negation of `new_tag`.
///
/// Rules (all case-sensitive):
/// - `candidate_tag == "not_<new_tag>"`  (explicit negation prefix)
/// - `candidate_tag` starts with `"not_"` or `"no_"`
/// - `candidate_tag` contains `"anti"`
/// - `candidate_tag` ends with `"_false"`
fn is_negation_pair(new_tag: &str, candidate_tag: &str) -> bool {
    // Exact "not_<tag>" negation.
    if candidate_tag == format!("not_{}", new_tag) {
        return true;
    }

    // Structural negation markers.
    if candidate_tag.starts_with("not_") || candidate_tag.starts_with("no_") {
        return true;
    }
    if candidate_tag.contains("anti") {
        return true;
    }
    if candidate_tag.ends_with("_false") {
        return true;
    }

    false
}

/// Format a [`Uuid`] as a lowercase hex string (32 hex characters, no hyphens).
fn id_as_hex(id: &Uuid) -> String {
    id.as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// Current wall-clock time in Unix nanoseconds.
fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::{FunRecordBuilder, SourceMethod};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a record whose tags are encoded as `_sources` origin strings.
    fn record_with_tags(tags: &[&str]) -> FunRecord {
        let mut builder = FunRecordBuilder::new("test");
        for &tag in tags {
            builder = builder.source(Source {
                origin: tag.to_string(),
                timestamp: 0,
                method: SourceMethod::Inference,
                confidence: 1.0,
            });
        }
        builder.build()
    }

    /// Build a record with tags and a named vector embedding.
    fn record_with_tags_and_vector(tags: &[&str], vec: Vec<f32>) -> FunRecord {
        let mut builder = FunRecordBuilder::new("test");
        for &tag in tags {
            builder = builder.source(Source {
                origin: tag.to_string(),
                timestamp: 0,
                method: SourceMethod::Inference,
                confidence: 1.0,
            });
        }
        builder.vector("text", vec).build()
    }

    // -----------------------------------------------------------------------
    // Test 1: empty candidates → empty result
    // -----------------------------------------------------------------------

    /// `check_insert` with no candidates must always return an empty vector.
    #[test]
    fn test_no_contradiction_empty_candidates() {
        let new = record_with_tags(&["sky_blue", "daytime"]);
        let result = ContradictionDetector::check_insert(&new, &[]);
        assert!(
            result.is_empty(),
            "check_insert with zero candidates must return an empty Vec"
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: identical tags → Corroborating with similarity ≈ 1.0
    // -----------------------------------------------------------------------

    /// Two records sharing all tags: similarity must be 1.0 and polarity Corroborating.
    #[test]
    fn test_identical_tags_corroborating() {
        let new = record_with_tags(&["water_is_h2o", "chemistry"]);
        let existing = record_with_tags(&["water_is_h2o", "chemistry"]);

        let results = ContradictionDetector::check_insert(&new, &[existing]);
        assert_eq!(results.len(), 1, "one candidate should be returned");

        let candidate = &results[0];
        assert!(
            (candidate.similarity - 1.0).abs() < 1e-5,
            "identical tags → similarity should be 1.0, got {}",
            candidate.similarity
        );
        assert_eq!(
            candidate.polarity,
            Polarity::Corroborating,
            "identical non-negated tags must yield Corroborating"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: negation tag → Conflicting
    // -----------------------------------------------------------------------

    /// `new` has tag "sky_blue"; candidate has tag "not_sky_blue" → Conflicting.
    #[test]
    fn test_negation_tag_conflicting() {
        let new = record_with_tags(&["sky_blue"]);
        let candidate = record_with_tags(&["not_sky_blue"]);

        let results = ContradictionDetector::check_insert(&new, &[candidate]);
        // "not_sky_blue" starts with "not_" → polarity is Conflicting regardless of
        // similarity (0/1 = 0 tag overlap, but both have 1 tag and "not_sky_blue" is
        // the exact negation of "sky_blue").
        //
        // Tag overlap = 0 (different strings) and vector_sim = 0 → similarity = 0.0
        // which is ≤ 0.3, so the candidate is filtered out by the threshold.
        // The test therefore verifies that when we add a shared context tag to push
        // similarity above the threshold the polarity comes back as Conflicting.
        let new2 = record_with_tags(&["sky_blue", "weather", "color"]);
        let candidate2 = record_with_tags(&["not_sky_blue", "weather", "color"]);

        let results2 = ContradictionDetector::check_insert(&new2, &[candidate2]);
        assert_eq!(results2.len(), 1, "one candidate above threshold expected");
        assert_eq!(
            results2[0].polarity,
            Polarity::Conflicting,
            "negation tag 'not_sky_blue' must yield Conflicting polarity"
        );

        // Verify the direct polarity helper too.
        let a = record_with_tags(&["sky_blue"]);
        let b = record_with_tags(&["not_sky_blue"]);
        assert_eq!(
            ContradictionDetector::polarity(&a, &b),
            Polarity::Conflicting
        );

        // Silence unused-variable warning from the first (unfocused) call above.
        let _ = results;
    }

    // -----------------------------------------------------------------------
    // Test 4: auto_link decrements confidence
    // -----------------------------------------------------------------------

    /// After `auto_link(a, b, 1.0)`, both records' confidence must drop by 0.2.
    #[test]
    fn test_auto_link_decrements_confidence() {
        let mut a = FunRecordBuilder::new("test").confidence(0.9).build();
        let mut b = FunRecordBuilder::new("test").confidence(0.8).build();

        ContradictionDetector::auto_link(&mut a, &mut b, 1.0);

        assert!(
            (a._confidence - 0.7).abs() < 1e-5,
            "record_a confidence should be 0.9 - 0.2×1.0 = 0.7, got {}",
            a._confidence
        );
        assert!(
            (b._confidence - 0.6).abs() < 1e-5,
            "record_b confidence should be 0.8 - 0.2×1.0 = 0.6, got {}",
            b._confidence
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: auto_link adds cross-link tags
    // -----------------------------------------------------------------------

    /// After `auto_link`, each record must have a `_sources` entry whose origin
    /// matches `"contradicts:<hex_id_of_other>"`.
    #[test]
    fn test_auto_link_adds_cross_tags() {
        let mut a = FunRecordBuilder::new("test").build();
        let mut b = FunRecordBuilder::new("test").build();

        let id_a = a._id;
        let id_b = b._id;

        ContradictionDetector::auto_link(&mut a, &mut b, 1.0);

        let hex_a = id_as_hex(&id_a);
        let hex_b = id_as_hex(&id_b);

        let tag_in_a = format!("contradicts:{}", hex_b);
        let tag_in_b = format!("contradicts:{}", hex_a);

        assert!(
            a._sources.iter().any(|s| s.origin == tag_in_a),
            "record_a must contain a source with origin '{}'",
            tag_in_a
        );
        assert!(
            b._sources.iter().any(|s| s.origin == tag_in_b),
            "record_b must contain a source with origin '{}'",
            tag_in_b
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: low similarity → filtered out
    // -----------------------------------------------------------------------

    /// Records with no overlapping tags must not appear in the results.
    #[test]
    fn test_low_similarity_filtered_out() {
        let new = record_with_tags(&["sky_blue", "sunny", "warm"]);
        let unrelated = record_with_tags(&["deep_ocean", "cold", "dark"]);

        let results = ContradictionDetector::check_insert(&new, &[unrelated]);
        assert!(
            results.is_empty(),
            "records with no tag overlap must be filtered out (similarity = 0.0 ≤ 0.3)"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: vector similarity
    // -----------------------------------------------------------------------

    /// When both records have a "text" vector, cosine similarity should be used
    /// if it exceeds the tag similarity.
    #[test]
    fn test_vector_similarity_used_when_higher() {
        // No shared tags → tag_sim = 0.0
        // High cosine similarity via aligned vectors → should push above threshold
        let new = record_with_tags_and_vector(&["topic_a"], vec![1.0, 0.0, 0.0]);
        let similar = record_with_tags_and_vector(&["topic_b"], vec![1.0, 0.0, 0.0]);

        let results = ContradictionDetector::check_insert(&new, &[similar]);
        assert_eq!(
            results.len(),
            1,
            "cosine similarity of 1.0 should push candidate above 0.3 threshold"
        );
        assert!(
            results[0].similarity > 0.3,
            "combined similarity should exceed threshold"
        );
    }

    // -----------------------------------------------------------------------
    // Additional: confidence clamp at 0.0
    // -----------------------------------------------------------------------

    /// `auto_link` must never drive confidence below 0.0.
    #[test]
    fn test_auto_link_confidence_clamp() {
        let mut a = FunRecordBuilder::new("test").confidence(0.05).build();
        let mut b = FunRecordBuilder::new("test").confidence(0.05).build();

        ContradictionDetector::auto_link(&mut a, &mut b, 1.0);

        assert!(
            a._confidence >= 0.0,
            "confidence must not go below 0.0, got {}",
            a._confidence
        );
        assert!(
            b._confidence >= 0.0,
            "confidence must not go below 0.0, got {}",
            b._confidence
        );
        assert_eq!(a._confidence, 0.0, "clamped confidence must equal 0.0");
        assert_eq!(b._confidence, 0.0, "clamped confidence must equal 0.0");
    }

    // -----------------------------------------------------------------------
    // Additional: results sorted by similarity descending
    // -----------------------------------------------------------------------

    /// `check_insert` must return candidates sorted by similarity (highest first).
    #[test]
    fn test_results_sorted_by_similarity_descending() {
        // new has 3 tags
        let new = record_with_tags(&["a", "b", "c"]);

        // candidate_1 shares 3/3 tags → sim = 1.0
        let candidate_1 = record_with_tags(&["a", "b", "c"]);
        // candidate_2 shares 2/3 tags → sim = 2/3 ≈ 0.667
        let candidate_2 = record_with_tags(&["a", "b", "x"]);

        let results = ContradictionDetector::check_insert(&new, &[candidate_2, candidate_1]);

        assert_eq!(
            results.len(),
            2,
            "both candidates should pass the threshold"
        );
        assert!(
            results[0].similarity >= results[1].similarity,
            "results must be sorted by similarity descending"
        );
        assert!(
            (results[0].similarity - 1.0).abs() < 1e-5,
            "highest similarity candidate must come first"
        );
    }
}
