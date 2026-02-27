# Agent Semanticist -- Proposal

**Two Algorithms for FunDB's Causal Reasoning Engine:**
1. **MechanismDetect** -- Validate whether a plausible causal mechanism exists between two events
2. **ConfounderSearch** -- Systematically find potential confounders that could explain an observed correlation

Both algorithms are designed to operate at query-time using data and indexes already present in FunDB.

---

## Part A: MechanismDetect Algorithm

### A.1 Problem Statement

Given two FunRecords:
- **A** (suspected cause): has embedding `A.vec`, text description `A.text`, timestamp `A.time`, graph edges `A.edges`
- **B** (suspected effect): has embedding `B.vec`, text description `B.text`, timestamp `B.time`, graph edges `B.edges`

Produce:
- `mechanism_score`: float in [0.0, 1.0] representing plausibility of a causal mechanism from A to B
- `mechanism_evidence`: list of intermediate records, paths, or textual explanations supporting the mechanism
- `mechanism_type`: one of DIRECT, BRIDGED, CHAIN, INFERRED, NONE

### A.2 Algorithm Overview

MechanismDetect operates in three tiers, each progressively more expensive. It stops as soon as it reaches sufficient confidence, following FunDB's existing tiered architecture (like the Semantic Interface's Tier 1/2/3 approach).

```
MechanismDetect(A, B):

  // ---- Tier 1: Direct Evidence (< 1ms) ----
  // Check existing causal edges in the graph

  direct_edges = FunCausal.query_path(A._id, B._id, max_depth=3)

  IF direct_edges.found:
    RETURN {
      score:    product(edge.strength for edge in direct_edges.path),
      evidence: direct_edges.path,
      type:     DIRECT
    }

  // ---- Tier 2: Semantic Bridge Search (< 10ms) ----
  // Find records whose embeddings are close to both A and B

  midpoint = (A.vec + B.vec) / 2
  radius   = ||A.vec - B.vec|| / 2 * expansion_factor  // expansion_factor = 1.5

  bridges = HNSW.search(
    center    = midpoint,
    radius    = radius,
    limit     = 50,
    filter    = { _valid_from < B._valid_from }  // bridge must exist before effect
  )

  scored_bridges = []
  FOR bridge IN bridges:
    sim_a = cosine_sim(bridge.vec, A.vec)
    sim_b = cosine_sim(bridge.vec, B.vec)

    // Bridge must be related to both A and B, not just one
    bridge_score = harmonic_mean(sim_a, sim_b)

    // Temporal bonus: bridge between A and B in time is stronger evidence
    temporal_bonus = 1.0
    IF A._valid_from < bridge._valid_from < B._valid_from:
      temporal_bonus = 1.2

    // Confidence weight
    confidence_weight = bridge._confidence

    scored_bridges.append({
      record:  bridge,
      score:   bridge_score * temporal_bonus * confidence_weight,
      sim_a:   sim_a,
      sim_b:   sim_b
    })

  scored_bridges.sort_by(score, DESC)
  top_bridges = scored_bridges[:5]

  IF top_bridges[0].score > BRIDGE_THRESHOLD:  // BRIDGE_THRESHOLD = 0.55
    RETURN {
      score:    min(top_bridges[0].score, 0.85),  // cap: bridge alone is not proof
      evidence: top_bridges,
      type:     BRIDGED
    }

  // ---- Tier 2b: Graph-Augmented Bridge (< 20ms) ----
  // Use graph edges to find paths through bridge records

  FOR bridge IN top_bridges[:10]:
    path_a_to_bridge = FunGraph.shortest_path(A._id, bridge.record._id, max_depth=2)
    path_bridge_to_b = FunGraph.shortest_path(bridge.record._id, B._id, max_depth=2)

    IF path_a_to_bridge AND path_bridge_to_b:
      chain_strength = compute_chain_strength(path_a_to_bridge, path_bridge_to_b)
      RETURN {
        score:    chain_strength,
        evidence: { path: path_a_to_bridge + path_bridge_to_b, bridge: bridge },
        type:     CHAIN
      }

  // ---- Tier 3: NLI-Based Mechanism Inference (< 100ms) ----
  // Use textual entailment to check if A's description plausibly leads to B's

  // Formulate NLI query
  premise    = A.text
  hypothesis = "This could lead to: " + B.text

  nli_result = NLI_model.predict(premise, hypothesis)
  // nli_result = { entailment: 0.X, contradiction: 0.Y, neutral: 0.Z }

  IF nli_result.entailment > 0.6:
    // Also check reverse: does B entail A? If so, this might be common cause, not mechanism
    reverse_nli = NLI_model.predict(B.text, "This could lead to: " + A.text)

    // Asymmetry bonus: strong A->B but weak B->A suggests directional causation
    asymmetry = nli_result.entailment - reverse_nli.entailment

    direction_score = nli_result.entailment * (1 + max(0, asymmetry))

    RETURN {
      score:    min(direction_score, 0.75),  // cap: NLI alone is weaker evidence
      evidence: {
        nli_forward:  nli_result,
        nli_reverse:  reverse_nli,
        asymmetry:    asymmetry
      },
      type:     INFERRED
    }

  // ---- No mechanism found ----
  RETURN {
    score:    max(0, nli_result.entailment - 0.3),  // small residual from NLI
    evidence: { exhaustive_search: true, best_bridge: top_bridges[0] if any },
    type:     NONE
  }
```

### A.3 The Midpoint Search Technique

The core geometric insight: if A caused B through some mechanism M, then M's embedding should lie in the semantic region *between* A and B in embedding space.

```
Embedding Space Visualization:

    A --------[midpoint]-------- B
    ^              ^              ^
  "deployed     "unicode       "errors in
   tokenizer"   handling"      Asian markets"

   M should be near the midpoint, with high similarity to BOTH A and B
```

We search a hypersphere centered at the midpoint of A and B, with radius equal to half the distance between them (expanded by a factor to allow for imperfect linearity in embedding space).

**Why harmonic mean for bridge scoring:** We want M to be related to *both* A and B. If sim(M, A) = 0.95 but sim(M, B) = 0.1, then M is just a record similar to A, not a bridge. The harmonic mean penalizes such asymmetry: harmonic_mean(0.95, 0.1) = 0.19, while arithmetic_mean(0.95, 0.1) = 0.525.

### A.4 NLI Model Specification

For the Tier 3 NLI check, we use a cross-encoder model:
- **Model:** DeBERTa-v3-base fine-tuned on MNLI + SNLI + ANLI (~400MB ONNX)
- **Input:** (premise, hypothesis) pair, max 512 tokens combined
- **Output:** Three probabilities: P(entailment), P(contradiction), P(neutral)
- **Latency:** ~50ms on CPU with ONNX Runtime, ~10ms on GPU

This model already fits FunDB's architecture: the Semantic Interface's Tier 3 uses a ~500MB ONNX model. The NLI model can share the same inference infrastructure.

### A.5 Confidence Scoring for Mechanisms

The `mechanism_score` is not just a similarity metric -- it is calibrated to reflect actual plausibility:

| Score Range | Interpretation | Typical Evidence |
|-------------|---------------|------------------|
| 0.85 - 1.00 | Strong mechanism | Direct causal edges in graph with high strength |
| 0.65 - 0.85 | Plausible mechanism | Good semantic bridges + temporal ordering |
| 0.40 - 0.65 | Weak mechanism | NLI entailment or distant semantic bridges |
| 0.20 - 0.40 | Speculative | Some topical similarity but no clear path |
| 0.00 - 0.20 | No mechanism | No evidence of connection |

**Score caps by evidence tier:**
- Tier 1 (direct edges): no cap (can reach 1.0)
- Tier 2 (semantic bridges): capped at 0.85 (strong but not conclusive)
- Tier 3 (NLI inference): capped at 0.75 (suggestive but weakest tier)

This ensures the system is honest about its uncertainty: finding a semantic bridge is good evidence but not proof.

### A.6 Computational Complexity

| Tier | Operations | Latency (expected) | When triggered |
|------|-----------|-------------------|----------------|
| 1 | DAG index lookup | < 1ms | Always (first check) |
| 2 | 1 HNSW search + scoring | < 10ms | If Tier 1 finds nothing |
| 2b | Up to 10 graph path queries | < 20ms | If bridges found but no strong standalone bridge |
| 3 | 1-2 NLI model forward passes | < 100ms | If Tiers 1-2 insufficient |

**Total worst case:** ~130ms (all tiers triggered, no early exit)
**Typical case:** ~10ms (most pairs either have direct edges or find good bridges)

---

## Part B: ConfounderSearch Algorithm

### B.1 Problem Statement

Given a suspected causal pair (A, B), find records Z in the database that could be confounders -- common causes that explain the observed correlation between A and B without A actually causing B.

Produce:
- `confounders`: list of candidate confounders, each with:
  - `record`: the FunRecord
  - `confounder_score`: float in [0.0, 1.0]
  - `evidence`: why this is a plausible confounder
- `coverage_score`: how thoroughly the database was searched (0.0 = barely searched, 1.0 = exhaustive)
- `search_metadata`: what strategies were used, how many candidates evaluated

### B.2 Algorithm Overview

ConfounderSearch uses four parallel search strategies, then fuses results.

```
ConfounderSearch(A, B):

  // ---- Strategy 1: Embedding Triangle Search ----
  // Find records whose embeddings are close to both A and B

  candidates_1 = HNSW.search(
    center = A.vec,
    radius = CONFOUNDER_RADIUS,  // 0.6 cosine distance
    limit  = 100,
    filter = {
      _valid_from < A._valid_from,   // must precede cause
      _valid_from < B._valid_from,   // must precede effect
      _id != A._id AND _id != B._id
    }
  )

  scored_1 = []
  FOR z IN candidates_1:
    sim_za = cosine_sim(z.vec, A.vec)
    sim_zb = cosine_sim(z.vec, B.vec)

    // A good confounder is related to both A and B
    triangle_score = harmonic_mean(sim_za, sim_zb)

    // Penalize records that are much closer to one than the other
    balance = 1 - abs(sim_za - sim_zb)

    scored_1.append({
      record:          z,
      confounder_score: triangle_score * balance * z._confidence,
      strategy:        "embedding_triangle",
      evidence:        { sim_to_cause: sim_za, sim_to_effect: sim_zb, balance: balance }
    })

  // ---- Strategy 2: Common Ancestor Search ----
  // Find records that have graph edges pointing to both A and B (or their neighbors)

  ancestors_a = FunGraph.reverse_traverse(A._id, depth=2, edge_types=["caused", "influenced", "related_to"])
  ancestors_b = FunGraph.reverse_traverse(B._id, depth=2, edge_types=["caused", "influenced", "related_to"])

  common_ancestors = intersection(ancestors_a, ancestors_b)

  scored_2 = []
  FOR z IN common_ancestors:
    // Strength of path from Z to A and Z to B
    strength_za = path_strength(z, A)  // product of edge strengths
    strength_zb = path_strength(z, B)

    scored_2.append({
      record:          z,
      confounder_score: harmonic_mean(strength_za, strength_zb),
      strategy:        "common_ancestor",
      evidence:        { path_to_cause: path(z, A), path_to_effect: path(z, B) }
    })

  // ---- Strategy 3: Co-occurrence Text Mining ----
  // Find terms/concepts that appear in documents related to both A and B

  // Get documents semantically related to A
  docs_a = HNSW.search(center=A.vec, radius=0.4, limit=50)
  // Get documents semantically related to B
  docs_b = HNSW.search(center=B.vec, radius=0.4, limit=50)

  // Extract salient terms from each set using TF-IDF
  terms_a = extract_salient_terms(docs_a, top_k=100)
  terms_b = extract_salient_terms(docs_b, top_k=100)

  // Shared terms are confounder candidates
  shared_terms = intersection(terms_a, terms_b)

  scored_3 = []
  FOR term IN shared_terms:
    // Find records that are about this term
    term_records = FunText.search(term, limit=10, filter={ _valid_from < min(A._valid_from, B._valid_from) })

    FOR z IN term_records:
      scored_3.append({
        record:          z,
        confounder_score: tfidf_score(term, docs_a) * tfidf_score(term, docs_b) * z._confidence,
        strategy:        "text_cooccurrence",
        evidence:        { shared_term: term, tfidf_in_cause_context: ..., tfidf_in_effect_context: ... }
      })

  // ---- Strategy 4: Temporal Context Search ----
  // Find significant events in the time window preceding both A and B

  time_window_start = min(A._valid_from, B._valid_from) - LOOKBACK_WINDOW  // default: 30 days
  time_window_end   = min(A._valid_from, B._valid_from)

  preceding_events = FunTemporal.range_scan(
    from = time_window_start,
    to   = time_window_end,
    filter = { _confidence > 0.5 }  // only consider reasonably confident records
  )

  scored_4 = []
  FOR z IN preceding_events:
    sim_za = cosine_sim(z.vec, A.vec)
    sim_zb = cosine_sim(z.vec, B.vec)

    IF sim_za > 0.3 AND sim_zb > 0.3:  // minimum relatedness threshold
      recency_weight = exponential_decay(
        distance = min(A._valid_from, B._valid_from) - z._valid_from,
        half_life = LOOKBACK_WINDOW / 3
      )

      scored_4.append({
        record:          z,
        confounder_score: harmonic_mean(sim_za, sim_zb) * recency_weight * z._confidence,
        strategy:        "temporal_context",
        evidence:        { time_before_cause: A._valid_from - z._valid_from, ... }
      })

  // ---- Fusion: Combine all strategies ----

  all_candidates = merge_and_deduplicate(scored_1, scored_2, scored_3, scored_4)

  // Records found by multiple strategies get a boost
  FOR z IN all_candidates:
    strategies_found = count_strategies(z)
    z.confounder_score *= (1 + 0.2 * (strategies_found - 1))
    // 1 strategy: 1.0x, 2 strategies: 1.2x, 3: 1.4x, 4: 1.6x

  // Sort by score, return top candidates
  all_candidates.sort_by(confounder_score, DESC)

  // Compute coverage score
  total_records_in_timewindow = FunTemporal.count(time_window_start, time_window_end)
  records_evaluated = len(union(candidates_1, preceding_events, ...))
  coverage = min(1.0, records_evaluated / total_records_in_timewindow)

  RETURN {
    confounders:    all_candidates[:20],
    coverage_score: coverage,
    search_metadata: {
      strategies_used:   ["embedding_triangle", "common_ancestor", "text_cooccurrence", "temporal_context"],
      candidates_evaluated: len(all_candidates),
      records_scanned:     records_evaluated,
      total_records:       total_records_in_timewindow
    }
  }
```

### B.3 The Four Strategies Explained

**Strategy 1 -- Embedding Triangle:** The geometric approach. In embedding space, a confounder Z should be in the "overlap zone" of A's and B's semantic neighborhoods. This is the fastest strategy (single HNSW search from A's vector, then cosine similarity checks against B).

**Strategy 2 -- Common Ancestor:** The graph approach. If Z has known edges (causal or relational) to both A and B, it is a structural confounder candidate. This leverages FunDB's FunGraph and FunCausal indexes.

**Strategy 3 -- Text Co-occurrence:** The linguistic approach. If a concept appears prominently in documents related to A *and* documents related to B, it is a shared contextual factor. Example: "Q4 holiday season" appearing in both marketing spend reports and revenue reports.

**Strategy 4 -- Temporal Context:** The chronological approach. Significant events preceding both A and B in time are natural confounder candidates. Example: a market crash that preceded both a marketing budget cut (A) and a revenue drop (B).

### B.4 Multi-Strategy Fusion

The key insight: a candidate found by *multiple independent strategies* is much more likely to be a true confounder.

- Found only by embedding similarity? Might just be topically related.
- Found by embedding similarity AND as a graph ancestor? Much stronger.
- Found by all four strategies? Almost certainly relevant.

The 20% boost per additional strategy is conservative. In practice, convergence across multiple strategies is the strongest signal.

### B.5 Confounder Scoring

| Score Range | Interpretation | Action |
|-------------|---------------|--------|
| 0.70 - 1.00 | Strong confounder candidate | Must be accounted for before claiming causation |
| 0.45 - 0.70 | Moderate confounder candidate | Should be investigated |
| 0.25 - 0.45 | Weak confounder candidate | Possible but unlikely to fully explain correlation |
| 0.00 - 0.25 | Unlikely confounder | Probably not relevant |

### B.6 Coverage Score: Honest About Search Completeness

The `coverage_score` answers: "How much of the potentially relevant data did we actually examine?"

- coverage = 1.0: Every record in the relevant time window was evaluated (exhaustive search)
- coverage = 0.5: We examined about half the potentially relevant records
- coverage < 0.2: Large portions of the database were not searched; low confidence in confounder exclusion

This is critical for the Statistician agent's signal fusion: if coverage is low, the "no confounders found" result should be weighted accordingly.

### B.7 Computational Complexity

| Strategy | Operations | Latency (expected) |
|----------|-----------|-------------------|
| 1. Embedding Triangle | 1 HNSW search + N cosine sims | ~10ms |
| 2. Common Ancestor | 2 reverse graph traversals + intersection | ~5ms |
| 3. Text Co-occurrence | 2 HNSW searches + TF-IDF extraction + text search | ~30ms |
| 4. Temporal Context | 1 temporal range scan + N cosine sims | ~15ms |
| Fusion | Merge + dedup + scoring | ~2ms |

**Total: ~62ms** with all four strategies running (parallelizable to ~35ms since strategies 1-4 are independent).

---

## Part C: FunQL Integration

### C.1 New Query Syntax

```sql
-- Mechanism detection between two records
SELECT mechanism_score, mechanism_type, mechanism_evidence
FROM DETECT MECHANISM
  FROM 'deploy-v2.3.1' TO 'error-spike-2025-02-15'
  MAX_TIERS 3               -- how deep to search (1=fast, 3=thorough)
  MIN_SCORE 0.3;            -- early return threshold

-- Confounder search for a suspected causal pair
SELECT confounder_record, confounder_score, strategy, evidence
FROM SEARCH CONFOUNDERS
  FOR PAIR ('marketing-spend-increase', 'revenue-q4')
  STRATEGIES ['embedding_triangle', 'common_ancestor', 'text_cooccurrence', 'temporal_context']
  LOOKBACK '90 days'
  MIN_SCORE 0.3
  LIMIT 20;

-- Combined: mechanism + confounder in one query
SELECT *
FROM ASSESS CAUSALITY
  FROM 'event-a' TO 'event-b'
  WITH mechanism_detection  = ON (max_tiers: 3)
  WITH confounder_search    = ON (lookback: '90 days', strategies: ALL)
  RETURN mechanism, confounders, net_causal_score;
```

### C.2 Net Causal Score

When used in the combined `ASSESS CAUSALITY` query, the semantic layer contributes to a net causal score:

```
semantic_signal = {
  mechanism_support:   mechanism_score,          // [0, 1] -- does a mechanism exist?
  confounder_penalty:  max(confounder_scores),   // [0, 1] -- strongest confounder found
  confounder_coverage: coverage_score,           // [0, 1] -- how thoroughly we searched
}

// Net semantic contribution to causal assessment:
semantic_causal_score = mechanism_support * (1 - confounder_penalty * confounder_coverage)
```

**Interpretation:**
- High mechanism + low confounders = strong semantic support for causation
- High mechanism + high confounders = mechanism exists but could be explained by confounder
- Low mechanism + any confounders = weak semantic support
- Any result + low coverage = uncertain (flagged in output)

This score is designed to be consumed by the Statistician agent's signal fusion framework, where it will be combined with temporal, statistical, and experimental evidence.

### C.3 Integration with Existing Operators

MechanismDetect and ConfounderSearch are implemented as new **physical operators** in the query planner:

```
Physical Plan Additions:

  MechanismScan:
    Input:  two record references (source, target)
    Output: mechanism_score, mechanism_type, mechanism_evidence
    Uses:   FunCausal index, HNSW index, FunGraph index, NLI model

  ConfounderScan:
    Input:  two record references (cause, effect)
    Output: confounder candidates with scores
    Uses:   HNSW index, FunGraph index, FunText index, FunTemporal index
```

These operators compose with existing operators:

```sql
-- Find all events that BOTH have a mechanism to the error spike
-- AND pass statistical Granger test
SELECT e.*, m.mechanism_score, g.p_value
FROM events e
JOIN (DETECT MECHANISM FROM e._id TO 'error-spike' MAX_TIERS 2) m
JOIN granger_test(e.timeseries, error_spike.timeseries) g
WHERE m.mechanism_score > 0.5
  AND g.p_value < 0.05
ORDER BY m.mechanism_score * (1 - g.p_value) DESC;
```

### C.4 Index Requirements

No new index types are needed. The algorithms use:
- **FunCausal**: Already exists -- DAG index for causal edges
- **FunVector (HNSW)**: Already exists -- for semantic bridge and triangle searches
- **FunGraph**: Already exists -- for path finding and ancestor search
- **FunText**: Already exists -- for term co-occurrence mining
- **FunTemporal**: Already exists -- for time-windowed searches
- **FunConfidence**: Already exists -- for filtering low-confidence candidates

### C.5 Optional Enhancement: Causal Direction Vector

As a background process (not query-time), FunDB can learn a "causes" direction vector from existing causal edges:

```
Background Process: LearnCausalDirection

Every N hours (configurable):
  1. Collect all (A, B) pairs where A._caused_by includes B with relation=CAUSED and strength > 0.7
  2. Compute direction vectors: d_i = B.vec - A.vec (effect minus cause)
  3. Average direction: r_causes = mean(d_i)  // normalize to unit vector
  4. Store r_causes as a system parameter

At query time:
  directional_score = cosine_sim(B.vec - A.vec, r_causes)
  // High score = the A->B direction aligns with known causal directions
  // This is an additional signal for MechanismDetect
```

This is essentially a simplified TransE: learning that "cause + r = effect" in embedding space. It requires zero per-query training and adds a single dot product to the scoring pipeline.

---

## Part D: Optimization Strategies

### D.1 Caching

**Mechanism cache:** Cache MechanismDetect results for (A._id, B._id) pairs. Cache invalidation when either record is updated or new edges are added to the causal graph.

**Bridge cache:** Frequently appearing bridge records (records that are bridges for many A-B pairs) can be pre-indexed. These are "hub" concepts in the semantic space.

**Causal direction vector:** Computed in background, cached in memory. Updated periodically, not per query.

### D.2 Parallelism

The four ConfounderSearch strategies are independent and can run in parallel. On a multi-core system, total latency is max(strategy latencies) rather than sum.

MechanismDetect tiers are sequential by design (each tier is tried only if the previous one was insufficient), but within Tier 2, the bridge scoring loop can be parallelized using SIMD batch cosine similarity operations, leveraging FunDB's existing SIMD acceleration for vector distance computation.

### D.3 Early Termination

Both algorithms support early termination:
- MechanismDetect stops at the first tier that exceeds the threshold
- ConfounderSearch can be configured with a `MIN_SCORE` that prunes low-scoring candidates early

### D.4 Approximate Search

For very large databases, the HNSW searches in Strategies 1 and 4 already use approximate nearest neighbor search. The trade-off is configurable via `ef_search`: higher values give better recall but slower search.

For ConfounderSearch, recall is more important than for standard retrieval (missing a confounder is more costly than returning a false positive), so we recommend `ef_search = 200` (higher than the default 50-100 for standard queries).

### D.5 Batch Operation

When assessing causality for multiple pairs (e.g., "find all possible causes of event B"), batch the HNSW searches:

```sql
-- Batch mechanism detection: what could have caused this error?
SELECT e.*, m.mechanism_score
FROM events e
CROSS APPLY (DETECT MECHANISM FROM e._id TO 'error-spike' MAX_TIERS 2) m
WHERE e._valid_from < 'error-spike'._valid_from
  AND e._collection = 'deployments'
ORDER BY m.mechanism_score DESC
LIMIT 10;
```

The query planner can recognize this pattern and batch HNSW searches, sharing the midpoint computation across candidates.

---

## Part E: Honest Limitations

### E.1 What This Cannot Do

1. **Prove causation:** Semantic evidence is supportive, never conclusive. A high mechanism score means "there is a plausible story for how A causes B," not "A definitely caused B."

2. **Find truly latent confounders:** If a confounder has no representation in the database (no record, no mention in any document), semantic search cannot find it. The coverage score helps communicate this limitation.

3. **Handle domain shift:** The causal direction vector and NLI model are general-purpose. They may perform poorly in highly specialized domains where causal language differs from common usage.

4. **Replace domain expertise:** A human expert who understands the system being modeled will always be able to identify mechanisms and confounders that semantic analysis misses.

### E.2 How We Communicate Uncertainty

Every output includes:
- **Confidence-calibrated scores** with explicit caps per evidence tier
- **Coverage metrics** showing how thoroughly the search space was explored
- **Evidence type labels** (DIRECT vs BRIDGED vs CHAIN vs INFERRED vs NONE) so consumers know the quality of evidence
- **Multi-strategy convergence indicators** showing whether multiple independent strategies agree

### E.3 Integration with Other Agents' Work

The semantic signals from this proposal are designed as **inputs** to the Statistician agent's signal fusion framework:

- `mechanism_score` contributes to the "mechanism plausibility" factor
- `confounder_candidates` feed into the statistical confounder adjustment process
- `coverage_score` modulates the weight given to confounder exclusion

The Historian agent can use mechanism bridges as starting points for finding natural experiments: if semantic search finds a bridge concept M between A and B, the Historian can look for cases where M varied naturally to test the mechanism.

The Architect agent needs to implement the `MechanismScan` and `ConfounderScan` physical operators in the query engine, with the parallelism and caching strategies described above.

The Skeptic agent should stress-test the score calibration, especially the caps and thresholds, against adversarial examples (known non-causal correlations that happen to have high semantic similarity).
