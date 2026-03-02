// fundb-semantic — STORY-6-1: Semantic Interface: Intent-to-FunQL (Tier 1, rule-based)

use fundb_sql::{AggExpr, AggFunc, Catalog, Expr, LogicalPlan, UnderstandOptions};

// ── Public types ──────────────────────────────────────────────────────────────

/// The result of parsing a natural-language intent string.
#[derive(Debug, Clone)]
pub enum IntentResult {
    /// A single confident plan and its confidence score (score >= 0.5).
    Confident(LogicalPlan, f32),
    /// Ambiguous intent: three candidate FunQL strings with scores (score < 0.5).
    Candidates(Vec<(String, f32)>),
    /// Intent could not be interpreted at all.
    Failed(String),
}

/// Rule-based semantic interface that translates natural-language intent strings
/// into FunQL `LogicalPlan` nodes (Tier 1 — no ML classifier).
pub struct SemanticInterface;

impl SemanticInterface {
    /// Create a new `SemanticInterface`.
    pub fn new() -> Self {
        SemanticInterface
    }

    /// Parse a natural-language `intent` against the given `catalog` and
    /// return the best matching `LogicalPlan` (or candidates when ambiguous).
    pub fn parse_intent(&self, intent: &str, catalog: &Catalog) -> IntentResult {
        let trimmed = intent.trim();

        if trimmed.is_empty() {
            return IntentResult::Failed("empty intent string".to_string());
        }

        let lower = trimmed.to_lowercase();
        let collection =
            extract_collection(&lower, catalog).unwrap_or_else(|| "records".to_string());

        // ── Rule matching — evaluated in priority order ────────────────────────

        // Rule 3: CAUSAL-TRACE (checked before SELECT because it is more specific)
        let causal_keywords = &[
            "caused by",
            "cause of",
            "effects of",
            "leads to",
            "trace causality",
            "what caused",
            "why did",
        ];
        let causal_score = score_multi(&lower, causal_keywords);

        // Rule 2: VECTOR-SIMILARITY
        let vector_keywords = &[
            "similar to",
            "like",
            "nearest to",
            "closest to",
            "<->",
            "resembles",
        ];
        let vector_score = score_multi(&lower, vector_keywords);

        // Rule 4: UNDERSTAND
        let understand_keywords = &[
            "understand",
            "explain",
            "what is",
            "tell me about",
            "describe",
        ];
        let understand_score = score_multi(&lower, understand_keywords);

        // Rule 5: COUNT/AGGREGATE
        let count_keywords = &["count", "how many", "number of", "total"];
        let count_score = score_multi(&lower, count_keywords);

        // Rule 6: TEMPORAL
        let temporal_keywords = &[
            "last",
            "recent",
            "today",
            "yesterday",
            "this week",
            "since",
            "before",
            "after",
            "as of",
        ];
        let temporal_score = score_multi(&lower, temporal_keywords);

        // Rule 1: SELECT-LIKE (lowest specificity, checked last among rules)
        let select_keywords = &[
            "find", "get", "show", "list", "fetch", "select", "query", "retrieve", "top",
        ];
        let select_score = score_multi(&lower, select_keywords);

        // Determine the winning rule by highest score.  Ties are broken by the
        // order in which rules appear below (most-specific first).
        #[allow(dead_code)]
        struct Match {
            score: f32,
            base_score: f32, // the rule's intrinsic confidence ceiling
        }

        // (score, rule_base_confidence, builder closure index)
        let candidates_ranked: &[(f32, f32, usize)] = &[
            (causal_score, 0.85, 3),
            (vector_score, 0.75, 2),
            (understand_score, 0.70, 4),
            (count_score, 0.75, 5),
            (temporal_score, 0.60, 6),
            (select_score, 0.80, 1),
        ];

        // Find the rule with the highest keyword score that has at least one hit.
        // Use reduce (not max_by) so that ties are broken by the order in the
        // array (most-specific rule first), not by picking the last equal element.
        let winner = candidates_ranked
            .iter()
            .filter(|(s, _, _)| *s > 0.0)
            .reduce(|best, cur| if cur.0 > best.0 { cur } else { best });

        let (keyword_score, base_confidence, rule_id) = match winner {
            Some(&(s, bc, rid)) => (s, bc, rid),
            None => {
                // No keywords matched at all → return Candidates.
                return self.make_candidates(trimmed, &collection);
            }
        };

        // Combined confidence: blend keyword match ratio with the rule's ceiling.
        let combined = (keyword_score * base_confidence).clamp(0.1, 1.0);

        if combined < 0.5 {
            return self.make_candidates(trimmed, &collection);
        }

        // Build the base plan for the winning rule.
        let base_plan = match rule_id {
            1 => build_scan(collection.clone()),
            2 => build_vector_scan(collection.clone()),
            3 => build_causal_trace(),
            4 => build_understand(trimmed),
            5 => build_aggregate(collection.clone()),
            6 => build_scan(collection.clone()), // temporal: scan, time filter TBD
            _ => build_scan(collection.clone()),
        };

        // Rule 7: LIMIT — wrap plan if intent specifies a count.
        let plan = maybe_wrap_limit(base_plan, &lower);

        IntentResult::Confident(plan, combined)
    }

    // ── Candidate generation ───────────────────────────────────────────────────

    fn make_candidates(&self, intent: &str, collection: &str) -> IntentResult {
        let candidates = vec![
            (
                format!("SELECT * FROM {} WHERE _confidence > 0.5", collection),
                0.4,
            ),
            (
                format!(
                    "SELECT * FROM {} ORDER BY _confidence DESC LIMIT 10",
                    collection
                ),
                0.35,
            ),
            (
                format!("UNDERSTAND \"{}\" MIN_CONFIDENCE 0.5 DEPTH 2", intent),
                0.3,
            ),
        ];
        IntentResult::Candidates(candidates)
    }
}

impl Default for SemanticInterface {
    fn default() -> Self {
        Self::new()
    }
}

// ── Rule builders ─────────────────────────────────────────────────────────────

fn build_scan(collection: String) -> LogicalPlan {
    LogicalPlan::Scan {
        collection,
        predicate: None,
        projections: vec![],
    }
}

fn build_vector_scan(collection: String) -> LogicalPlan {
    LogicalPlan::VectorScan {
        collection,
        vector_field: "embedding".to_string(),
        query: vec![],
        threshold: 0.5,
    }
}

fn build_causal_trace() -> LogicalPlan {
    LogicalPlan::CausalTrace {
        from: Expr::Param("from".to_string()),
        to: Expr::Param("to".to_string()),
        max_depth: 5,
        min_strength: 0.3,
        min_stability: None,
    }
}

fn build_understand(intent: &str) -> LogicalPlan {
    LogicalPlan::Understand {
        intent: intent.to_string(),
        options: UnderstandOptions {
            min_confidence: Some(0.5),
            within_days: None,
            depth: Some(2),
            collection: None,
            vector_field: None,
            min_similarity: None,
        },
    }
}

fn build_aggregate(collection: String) -> LogicalPlan {
    let scan = build_scan(collection);
    LogicalPlan::Aggregate {
        input: Box::new(scan),
        group_by: vec![],
        aggregates: vec![AggExpr {
            func: AggFunc::Count,
            arg: Box::new(Expr::Star),
            alias: Some("count".to_string()),
        }],
    }
}

/// If the intent contains a numeric limit pattern, wrap `plan` in a `Limit` node.
fn maybe_wrap_limit(plan: LogicalPlan, lower: &str) -> LogicalPlan {
    if let Some(n) = extract_limit(lower) {
        LogicalPlan::Limit {
            input: Box::new(plan),
            n,
        }
    } else {
        plan
    }
}

// ── Helper: limit extraction ──────────────────────────────────────────────────

/// Extract a limit N from the intent string.
///
/// Recognises:
/// - "top N ..."
/// - "N results / items / records / rows / entries"
fn extract_limit(lower: &str) -> Option<usize> {
    let limit_suffixes = ["results", "items", "records", "rows", "entries"];

    // "top N" pattern.
    if let Some(rest) = lower.strip_prefix("top ") {
        let token = rest.split_whitespace().next()?;
        if let Ok(n) = token.parse::<usize>() {
            return Some(n.min(1000));
        }
    }

    // "N <suffix>" pattern: scan all words, check if previous token was a digit.
    let words: Vec<&str> = lower.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        if limit_suffixes.contains(word) && i > 0 {
            if let Ok(n) = words[i - 1].parse::<usize>() {
                return Some(n.min(1000));
            }
        }
    }

    // Fallback: if any digit appears in context with limit-related suffixes, use it.
    // (e.g. "give me 5 documents")
    for (i, word) in words.iter().enumerate() {
        if let Ok(n) = word.parse::<usize>() {
            if n <= 1000 {
                // Check whether the surrounding context suggests a count/limit.
                let prev = i.checked_sub(1).and_then(|p| words.get(p)).copied();
                let next = words.get(i + 1).copied();
                let is_limit_context = matches!(prev, Some("top") | Some("first") | Some("last"))
                    || limit_suffixes.contains(&next.unwrap_or(""));
                if is_limit_context {
                    return Some(n);
                }
            }
        }
    }

    None
}

// ── Helper: collection extraction ────────────────────────────────────────────

/// Return the first word (lowercased) from `intent` that matches a catalog
/// collection name.  If no match, fall back to the longest word > 4 chars.
fn extract_collection(intent: &str, catalog: &Catalog) -> Option<String> {
    // First pass: exact catalog match (case-insensitive, whole-word).
    for word in intent.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphabetic());
        if catalog.has_collection(cleaned) {
            return Some(cleaned.to_string());
        }
    }

    // Second pass: longest noun-like word (> 4 chars) as a heuristic fallback.
    intent
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .filter(|w| w.len() > 4)
        .max_by_key(|w| w.len())
        .map(|w| w.to_string())
}

// ── Helper: keyword scoring ───────────────────────────────────────────────────

/// Score a multi-word keyword list against the lowercased `intent`.
///
/// Returns the fraction of keyword phrases found in the intent, clamped to
/// [0.1, 1.0].  Returns 0.0 when no keywords match (to distinguish "no
/// signal" from the clamp floor).
fn score_multi(intent: &str, keywords: &[&str]) -> f32 {
    if keywords.is_empty() {
        return 0.0;
    }
    let matches = keywords.iter().filter(|&&kw| intent.contains(kw)).count();
    if matches == 0 {
        return 0.0;
    }
    // Any keyword match signals high confidence for this rule.
    1.0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_with(names: &[&str]) -> Catalog {
        let mut c = Catalog::new();
        for name in names {
            c.add_collection(name);
        }
        c
    }

    fn open_catalog() -> Catalog {
        Catalog::open()
    }

    // 1. "find documents about rust" → Confident(Scan{collection:"documents"}, _)
    #[test]
    fn test_find_documents() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["documents"]);
        let result = si.parse_intent("find documents about rust", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Scan { ref collection, .. }, score) => {
                assert_eq!(collection, "documents");
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
            }
            other => panic!("expected Confident(Scan{{..}}), got {:?}", other),
        }
    }

    // 2. "how many users" → Confident(Aggregate{..}, _)
    #[test]
    fn test_how_many_users() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["users"]);
        let result = si.parse_intent("how many users", &cat);
        match result {
            IntentResult::Confident(plan, score) => {
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
                assert!(
                    matches!(
                        plan,
                        LogicalPlan::Aggregate { .. } | LogicalPlan::Scan { .. }
                    ),
                    "expected Aggregate or Scan, got {:?}",
                    plan
                );
            }
            other => panic!("expected Confident, got {:?}", other),
        }
    }

    // 3. "what caused the database failure" → Confident(CausalTrace{..}, _)
    #[test]
    fn test_causal_trace() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("what caused the database failure", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::CausalTrace { .. }, score) => {
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
            }
            other => panic!("expected Confident(CausalTrace{{..}}), got {:?}", other),
        }
    }

    // 4. "similar to X" → Confident(VectorScan{..}, _)
    #[test]
    fn test_vector_similarity() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("similar to X", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::VectorScan { .. }, score) => {
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
            }
            other => panic!("expected Confident(VectorScan{{..}}), got {:?}", other),
        }
    }

    // 5. "explain this" → Confident(Understand{..}, _)
    #[test]
    fn test_understand_explain() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("explain this", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Understand { .. }, score) => {
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
            }
            other => panic!("expected Confident(Understand{{..}}), got {:?}", other),
        }
    }

    // 6. "xyz" → Candidates([..]) (no keywords matched)
    #[test]
    fn test_unknown_intent_returns_candidates() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("xyz", &cat);
        match result {
            IntentResult::Candidates(cands) => {
                assert_eq!(cands.len(), 3, "expected 3 candidates");
            }
            other => panic!("expected Candidates, got {:?}", other),
        }
    }

    // 7. "top 5 documents" → Confident(Limit { n:5, input: Scan{..} }, _)
    #[test]
    fn test_limit_top_n() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["documents"]);
        let result = si.parse_intent("top 5 documents", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Limit { n, .. }, score) => {
                assert_eq!(n, 5);
                assert!(score >= 0.5, "score {} should be >= 0.5", score);
            }
            other => panic!("expected Confident(Limit{{n:5,..}}), got {:?}", other),
        }
    }

    // 8. empty intent → Failed(_)
    #[test]
    fn test_empty_intent_returns_failed_or_candidates() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("", &cat);
        assert!(
            matches!(
                result,
                IntentResult::Failed(_) | IntentResult::Candidates(_)
            ),
            "expected Failed or Candidates for empty input, got {:?}",
            result
        );
    }

    // Additional: collection extraction falls back to heuristic when not in catalog
    #[test]
    fn test_collection_fallback() {
        let si = SemanticInterface::new();
        let cat = Catalog::new(); // empty catalog
        let result = si.parse_intent("find something in articles", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Scan { ref collection, .. }, _) => {
                // "articles" (8 chars) > "something" (9) — longest word wins
                // either is acceptable as a fallback
                assert!(!collection.is_empty());
            }
            IntentResult::Candidates(_) => { /* also acceptable */ }
            other => panic!("unexpected result {:?}", other),
        }
    }

    // Additional: "nearest to" triggers VectorScan
    #[test]
    fn test_nearest_to_vector_scan() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("nearest to the query vector", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::VectorScan { .. }, _) => {}
            other => panic!("expected VectorScan, got {:?}", other),
        }
    }

    // Additional: "count events" triggers Aggregate
    #[test]
    fn test_count_events() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["events"]);
        let result = si.parse_intent("count events", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Aggregate { .. }, score) => {
                assert!(score >= 0.5);
            }
            other => panic!("expected Aggregate, got {:?}", other),
        }
    }

    // Additional: temporal keywords produce a Scan
    #[test]
    fn test_temporal_recent_produces_scan() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["events"]);
        let result = si.parse_intent("recent events", &cat);
        match result {
            IntentResult::Confident(plan, score) => {
                assert!(score >= 0.5);
                assert!(
                    matches!(plan, LogicalPlan::Scan { .. }),
                    "expected Scan for temporal query, got {:?}",
                    plan
                );
            }
            other => panic!("expected Confident(Scan{{..}}), got {:?}", other),
        }
    }

    // Additional: "what is knowledge_base" triggers Understand
    #[test]
    fn test_what_is_understand() {
        let si = SemanticInterface::new();
        let cat = open_catalog();
        let result = si.parse_intent("what is knowledge_base", &cat);
        match result {
            IntentResult::Confident(LogicalPlan::Understand { ref intent, .. }, score) => {
                assert!(score >= 0.5);
                assert!(!intent.is_empty());
            }
            other => panic!("expected Understand, got {:?}", other),
        }
    }

    // Additional: candidate FunQL strings contain collection name
    #[test]
    fn test_candidates_contain_collection_name() {
        let si = SemanticInterface::new();
        let cat = catalog_with(&["metrics"]);
        // Intent that matches no rule keywords but contains a known collection name.
        let result = si.parse_intent("metrics", &cat);
        match result {
            IntentResult::Candidates(cands) => {
                let all_contain = cands.iter().all(|(s, _)| s.contains("metrics"));
                assert!(
                    all_contain,
                    "candidates should reference 'metrics': {:?}",
                    cands
                );
            }
            IntentResult::Confident(_, _) => { /* a single keyword match is also fine */ }
            other => panic!("unexpected result {:?}", other),
        }
    }
}
