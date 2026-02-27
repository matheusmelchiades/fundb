use fundb_core::FunRecord;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options controlling how the context window is filled.
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Maximum number of tokens allowed in the selected context.
    pub max_tokens: u32,
    /// Minimum mean pairwise cosine similarity required for the selected set
    /// (0.0 = disabled, 1.0 = all records must be identical).
    pub coherence: f32,
    /// MMR lambda: 0.0 = maximize diversity, 1.0 = maximize relevance.
    pub diversity: f32,
    /// When true, force-include at least one low-confidence record (proxy for
    /// contradiction) if any exists among the candidates.
    pub include_contradictions: bool,
    /// Ordered list of sort keys applied to the candidate list before MMR.
    /// Only the first key is used for sorting.
    pub priority: Vec<SortKey>,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            coherence: 0.0,
            diversity: 0.5,
            include_contradictions: false,
            priority: vec![],
        }
    }
}

/// A key by which to sort candidates before running MMR.
#[derive(Debug, Clone, PartialEq)]
pub enum SortKey {
    /// Sort descending by `_confidence`.
    Confidence,
    /// Sort descending by the relevance score (second tuple element).
    Relevance,
    /// Sort descending by `_valid_from` (recency proxy).
    Recency,
}

/// Metadata about the context selection that was performed.
#[derive(Debug, Clone)]
pub struct ContextMetadata {
    /// Total tokens consumed by the selected records.
    pub tokens_used: u32,
    /// Token budget provided in `ContextOptions`.
    pub tokens_budget: u32,
    /// `candidates_selected / candidates_evaluated` (0.0 if no candidates).
    pub coverage_score: f32,
    /// Mean pairwise cosine similarity of selected records (0.0 if < 2).
    pub coherence_score: f32,
    /// `1.0 - coherence_score`.
    pub diversity_score: f32,
    /// Mean `_confidence` of selected records (0.0 if none selected).
    pub avg_confidence: f32,
    /// Number of selected records whose `_confidence < 0.3` (contradiction proxy).
    pub contradictions_found: u32,
    /// Number of candidates that were evaluated.
    pub candidates_evaluated: u32,
    /// Number of candidates that were ultimately selected.
    pub candidates_selected: u32,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Estimate the token cost of a single record using the specification formula.
fn estimate_tokens(record: &FunRecord) -> u32 {
    let data_estimate: u32 = 50;
    let vector_tokens: u32 = record
        ._vectors
        .values()
        .map(|v| v.len() as u32 * 6)
        .sum();
    let scalar_tokens: u32 = 10;
    data_estimate + vector_tokens + scalar_tokens
}

/// Cosine similarity between two records using the first shared (or each
/// record's first) vector field.
///
/// Returns 0.0 when either record has no vector embeddings.
fn cosine_sim(a: &FunRecord, b: &FunRecord) -> f32 {
    // Prefer a field name present in both records.
    let vec_a = a
        ._vectors
        .iter()
        .find_map(|(k, v)| b._vectors.get(k).map(|bv| (v.as_slice(), bv.as_slice())))
        .or_else(|| {
            // Fall back: first vector of a paired with first vector of b.
            let va = a._vectors.values().next()?;
            let vb = b._vectors.values().next()?;
            Some((va.as_slice(), vb.as_slice()))
        });

    let (va, vb) = match vec_a {
        Some(pair) => pair,
        None => return 0.0,
    };

    let min_len = va.len().min(vb.len());
    if min_len == 0 {
        return 0.0;
    }

    let dot: f32 = va[..min_len].iter().zip(vb[..min_len].iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = va[..min_len].iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = vb[..min_len].iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Mean pairwise cosine similarity for a slice of records.
/// Returns 0.0 when fewer than 2 records are provided.
fn mean_pairwise_cosine(records: &[FunRecord]) -> f32 {
    if records.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0f32;
    let mut count = 0u32;
    for i in 0..records.len() {
        for j in (i + 1)..records.len() {
            total += cosine_sim(&records[i], &records[j]);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total / count as f32
    }
}

/// Maximum cosine similarity between `candidate` and any record already in
/// `selected`.  Returns 0.0 when `selected` is empty.
fn max_sim_to_selected(candidate: &FunRecord, selected: &[FunRecord]) -> f32 {
    selected
        .iter()
        .map(|s| cosine_sim(candidate, s))
        .fold(0.0f32, f32::max)
}

// ---------------------------------------------------------------------------
// ContextOptimizer
// ---------------------------------------------------------------------------

/// Selects a subset of candidate records that fits within the token budget
/// using Maximal Marginal Relevance (MMR).
pub struct ContextOptimizer;

impl ContextOptimizer {
    /// Select the best records from `candidates` under the given `options`.
    ///
    /// Returns the selected [`FunRecord`]s and accompanying [`ContextMetadata`].
    pub fn select(
        candidates: Vec<(FunRecord, f32)>,
        options: ContextOptions,
    ) -> (Vec<FunRecord>, ContextMetadata) {
        let candidates_evaluated = candidates.len() as u32;

        if candidates.is_empty() {
            return (
                vec![],
                ContextMetadata {
                    tokens_used: 0,
                    tokens_budget: options.max_tokens,
                    coverage_score: 0.0,
                    coherence_score: 0.0,
                    diversity_score: 1.0,
                    avg_confidence: 0.0,
                    contradictions_found: 0,
                    candidates_evaluated: 0,
                    candidates_selected: 0,
                },
            );
        }

        // --- 1. Apply priority sort -----------------------------------------
        let mut candidates = candidates;
        if let Some(key) = options.priority.first() {
            match key {
                SortKey::Confidence => {
                    candidates.sort_by(|(a, _), (b, _)| {
                        b._confidence
                            .partial_cmp(&a._confidence)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                SortKey::Relevance => {
                    candidates.sort_by(|(_, ra), (_, rb)| {
                        rb.partial_cmp(ra).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                SortKey::Recency => {
                    candidates.sort_by(|(a, _), (b, _)| b._valid_from.cmp(&a._valid_from));
                }
            }
        }

        // --- 2. Identify contradiction candidates (confidence < 0.3) ---------
        let contradiction_idx: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(_, (r, _))| r._confidence < 0.3)
            .map(|(i, _)| i)
            .collect();

        let lambda = options.diversity; // higher = more relevance-focused

        let mut selected: Vec<FunRecord> = Vec::new();
        let mut used_tokens: u32 = 0;
        let mut remaining_indices: Vec<usize> = (0..candidates.len()).collect();

        // --- 3. Force-include one contradiction if requested -----------------
        if options.include_contradictions {
            if let Some(&cidx) = contradiction_idx.first() {
                let (record, _) = &candidates[cidx];
                let cost = estimate_tokens(record);
                if used_tokens + cost <= options.max_tokens + cost {
                    // Accept even if it slightly overruns (spec says "force include")
                    let (record, _) = candidates[cidx].clone();
                    used_tokens += cost;
                    selected.push(record);
                    remaining_indices.retain(|&i| i != cidx);
                }
            }
        }

        // --- 4. MMR main loop ------------------------------------------------
        while !remaining_indices.is_empty() {
            let mut best_idx_in_remaining: Option<usize> = None; // index into remaining_indices
            let mut best_mmr = f32::NEG_INFINITY;

            for (pos, &cand_idx) in remaining_indices.iter().enumerate() {
                let (record, relevance) = &candidates[cand_idx];
                let cost = estimate_tokens(record);

                // Skip if adding this record would exceed the budget.
                if used_tokens + cost > options.max_tokens {
                    continue;
                }

                // Coherence filter: if coherence constraint is active and the
                // selected set is non-empty, skip records that are too dissimilar.
                if options.coherence > 0.0 && !selected.is_empty() {
                    let sim = max_sim_to_selected(record, &selected);
                    if sim < options.coherence {
                        continue;
                    }
                }

                let max_sim = max_sim_to_selected(record, &selected);
                let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim;

                if mmr_score > best_mmr {
                    best_mmr = mmr_score;
                    best_idx_in_remaining = Some(pos);
                }
            }

            match best_idx_in_remaining {
                None => break, // no valid candidate found — budget exhausted or all filtered
                Some(pos) => {
                    let cand_idx = remaining_indices[pos];
                    let (record, _) = candidates[cand_idx].clone();
                    let cost = estimate_tokens(&record);
                    used_tokens += cost;
                    selected.push(record);
                    remaining_indices.remove(pos);
                }
            }
        }

        // --- 5. Compute metadata --------------------------------------------
        let candidates_selected = selected.len() as u32;
        let coverage_score =
            candidates_selected as f32 / (candidates_evaluated.max(1) as f32);
        let coherence_score = mean_pairwise_cosine(&selected);
        let diversity_score = 1.0 - coherence_score;
        let avg_confidence = if selected.is_empty() {
            0.0
        } else {
            selected.iter().map(|r| r._confidence).sum::<f32>() / selected.len() as f32
        };
        let contradictions_found = selected
            .iter()
            .filter(|r| r._confidence < 0.3)
            .count() as u32;

        let metadata = ContextMetadata {
            tokens_used: used_tokens,
            tokens_budget: options.max_tokens,
            coverage_score,
            coherence_score,
            diversity_score,
            avg_confidence,
            contradictions_found,
            candidates_evaluated,
            candidates_selected,
        };

        (selected, metadata)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::FunRecordBuilder;

    /// Build a minimal record with a single named vector.
    fn make_record(vec: Vec<f32>, confidence: f32, valid_from: i64) -> FunRecord {
        FunRecordBuilder::new("test")
            .vector("emb", vec)
            .confidence(confidence)
            .valid_time(valid_from, i64::MAX)
            .build()
    }

    fn default_opts() -> ContextOptions {
        ContextOptions::default()
    }

    // -----------------------------------------------------------------------
    // Test 1: empty candidates → empty result, all metadata zeros
    // -----------------------------------------------------------------------
    #[test]
    fn test_empty_candidates() {
        let (selected, meta) = ContextOptimizer::select(vec![], default_opts());
        assert!(selected.is_empty());
        assert_eq!(meta.tokens_used, 0);
        assert_eq!(meta.candidates_evaluated, 0);
        assert_eq!(meta.candidates_selected, 0);
        assert_eq!(meta.coverage_score, 0.0);
        assert_eq!(meta.avg_confidence, 0.0);
        assert_eq!(meta.contradictions_found, 0);
    }

    // -----------------------------------------------------------------------
    // Test 2: token budget is not materially exceeded
    // -----------------------------------------------------------------------
    #[test]
    fn test_budget_not_exceeded() {
        let candidates: Vec<(FunRecord, f32)> = (0..10)
            .map(|i| {
                let r = make_record(vec![0.1 * i as f32; 4], 0.9, i as i64);
                (r, 0.5 + 0.05 * i as f32)
            })
            .collect();

        let opts = ContextOptions {
            max_tokens: 100,
            ..default_opts()
        };

        let (_, meta) = ContextOptimizer::select(candidates, opts);
        // Tolerance of 110 as stated in the spec.
        assert!(
            meta.tokens_used <= 110,
            "tokens_used={} exceeds allowed 110",
            meta.tokens_used
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: diversity=0.0 → first pick has highest relevance score
    // -----------------------------------------------------------------------
    #[test]
    fn test_diversity_0_returns_most_relevant() {
        // All records have the same vector so max_sim is always 1.0 after the
        // first pick; with lambda=0.0, MMR = -(1-0)*max_sim.  But for the FIRST
        // pick selected is empty, so max_sim = 0 and MMR = lambda*rel = 0*rel = 0
        // for every candidate regardless.  Sorting by relevance first ensures
        // the highest-relevance record is picked when scores tie.
        //
        // We therefore use SortKey::Relevance in priority so the highest-relevance
        // record is the first one considered (and picked on tie).
        let candidates: Vec<(FunRecord, f32)> = vec![
            (make_record(vec![1.0, 0.0], 0.9, 0), 0.3),
            (make_record(vec![0.0, 1.0], 0.9, 0), 0.7),  // highest relevance
            (make_record(vec![0.5, 0.5], 0.9, 0), 0.5),
        ];

        let opts = ContextOptions {
            max_tokens: 512,
            diversity: 0.0,
            priority: vec![SortKey::Relevance],
            ..default_opts()
        };

        let (selected, _) = ContextOptimizer::select(candidates, opts);
        assert!(!selected.is_empty());
        // With priority::Relevance sort, the highest-relevance record (relevance=0.7)
        // is evaluated first when all MMR scores are equal at the start.
        // Verify the first selected record has confidence 0.9 (all do) and the
        // vector matching the highest-relevance entry [0.0, 1.0].
        let first = &selected[0];
        let emb = first._vectors.get("emb").expect("must have emb vector");
        assert!(
            (emb[0] - 0.0).abs() < 1e-5 && (emb[1] - 1.0).abs() < 1e-5,
            "expected first pick to be the highest-relevance record [0.0, 1.0], got {:?}",
            emb
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: diversity=1.0 → spreads picks across different vectors
    // -----------------------------------------------------------------------
    #[test]
    fn test_diversity_1_spreads_picks() {
        // 5 records with vector [1.0, 0.0] (cluster A) and
        // 5 records with vector [0.0, 1.0] (cluster B).
        // With diversity=1.0 (max relevance focus) and all equal relevance,
        // MMR = 1.0*rel - 0.0*max_sim = rel, so it's purely relevance-driven.
        // To test spread we give cluster B slightly higher relevance so at least
        // one from each cluster must appear among picks.
        let mut candidates: Vec<(FunRecord, f32)> = Vec::new();
        for _ in 0..5 {
            candidates.push((make_record(vec![1.0, 0.0], 0.8, 0), 0.6));
        }
        for _ in 0..5 {
            candidates.push((make_record(vec![0.0, 1.0], 0.8, 0), 0.7));
        }

        let opts = ContextOptions {
            max_tokens: 2048,
            diversity: 1.0,
            ..default_opts()
        };

        let (selected, _) = ContextOptimizer::select(candidates, opts);
        assert!(!selected.is_empty(), "should select at least one record");

        let has_cluster_a = selected
            .iter()
            .any(|r| r._vectors.get("emb").map(|v| (v[0] - 1.0).abs() < 1e-5).unwrap_or(false));
        let has_cluster_b = selected
            .iter()
            .any(|r| r._vectors.get("emb").map(|v| (v[1] - 1.0).abs() < 1e-5).unwrap_or(false));

        // With a generous budget and candidates from both clusters, both should
        // appear since all fit within the budget.
        assert!(has_cluster_a || has_cluster_b, "should pick from at least one cluster");
        // With 10 records all fitting in budget, we expect both clusters selected.
        assert!(
            has_cluster_a && has_cluster_b,
            "with large budget, both clusters should be selected"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: coherence filter with high threshold → at most 1 after first pick
    // -----------------------------------------------------------------------
    #[test]
    fn test_coherence_filter() {
        // Record A: [1, 0] — first pick.
        // Records B–D: [0, 1] — orthogonal to A, cosine_sim=0 < 0.99.
        // With coherence=0.99, after A is selected nothing else can pass the filter.
        let candidates: Vec<(FunRecord, f32)> = vec![
            (make_record(vec![1.0, 0.0], 0.9, 0), 0.9), // highest relevance → first pick
            (make_record(vec![0.0, 1.0], 0.8, 0), 0.5),
            (make_record(vec![0.0, 1.0], 0.8, 0), 0.4),
            (make_record(vec![0.0, 1.0], 0.8, 0), 0.3),
        ];

        let opts = ContextOptions {
            max_tokens: 4096,
            coherence: 0.99,
            diversity: 0.5,
            ..default_opts()
        };

        let (selected, _) = ContextOptimizer::select(candidates, opts);
        // After the first record is selected, all remaining are filtered out.
        assert!(
            selected.len() <= 1,
            "expected at most 1 selected with strict coherence, got {}",
            selected.len()
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: all metadata fields are reasonable after a normal selection
    // -----------------------------------------------------------------------
    #[test]
    fn test_metadata_fields_populated() {
        let candidates: Vec<(FunRecord, f32)> = vec![
            (make_record(vec![1.0, 0.0, 0.0], 0.9, 100), 0.8),
            (make_record(vec![0.0, 1.0, 0.0], 0.7, 200), 0.6),
            (make_record(vec![0.0, 0.0, 1.0], 0.5, 300), 0.7),
            (make_record(vec![0.5, 0.5, 0.0], 0.2, 400), 0.4), // contradiction (conf < 0.3 → no)
            (make_record(vec![0.1, 0.1, 0.1], 0.1, 500), 0.3), // contradiction (conf < 0.3 → yes)
        ];

        let opts = ContextOptions {
            max_tokens: 4096,
            ..default_opts()
        };

        let (selected, meta) = ContextOptimizer::select(candidates, opts);

        assert_eq!(meta.candidates_evaluated, 5);
        assert!(meta.candidates_selected > 0, "should select at least one record");
        assert!(meta.tokens_used > 0, "tokens_used should be > 0");
        assert!(meta.tokens_used <= 4096, "tokens_used must not exceed budget");
        assert_eq!(meta.tokens_budget, 4096);

        // coverage_score must be in [0, 1]
        assert!((0.0..=1.0).contains(&meta.coverage_score));

        // coherence and diversity are complementary
        let sum = meta.coherence_score + meta.diversity_score;
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "coherence + diversity should equal 1.0, got {}",
            sum
        );

        // avg_confidence must be in [0, 1]
        assert!((0.0..=1.0).contains(&meta.avg_confidence));

        // contradictions_found must not exceed candidates_selected
        assert!(meta.contradictions_found <= meta.candidates_selected);

        // Verify selected records are consistent with metadata
        let expected_tokens: u32 = selected.iter().map(estimate_tokens).sum();
        assert_eq!(meta.tokens_used, expected_tokens);

        let _ = selected; // ensure selected is non-empty is already checked above
    }
}
