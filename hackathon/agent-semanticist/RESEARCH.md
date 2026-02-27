# Agent Semanticist -- Research Notes

**Focus:** Mechanism Detection + Semantic Confounder Search for FunDB's Causal Reasoning Engine
**Sub-problems addressed:** SP2 (Automatic Confounder Search) and supporting SP1 (Signal Fusion via mechanism plausibility)

---

## 1. The Semantic Gap in Causal Inference

Statistical causal inference (Granger tests, temporal precedence, co-occurrence) answers "do A and B co-vary in a way consistent with causation?" but cannot answer "is there a plausible story for *how* A could cause B?" This is the **mechanism gap**.

Conversely, semantic analysis can identify plausible mechanisms but cannot quantify effect sizes or rule out confounders statistically. The two approaches are complementary -- and FunDB is uniquely positioned to fuse them because it stores embeddings, text, and graph edges in a single engine.

This research covers what NLP and embedding-based techniques exist for:
1. Detecting whether a plausible causal mechanism connects two events
2. Searching for confounders that share semantic context with both cause and effect
3. Distinguishing causal from merely correlational semantic relationships

---

## 2. Causal Relation Extraction from Text

### 2.1 Explicit Causal Markers

The simplest approach: find linguistic markers of causation in text.

**Causal connectives and patterns:**
- "X caused Y", "X led to Y", "X resulted in Y"
- "because of X, Y happened", "Y due to X"
- "X triggered Y", "X induced Y", "X contributed to Y"
- "as a consequence of X", "Y is a side effect of X"

**Relevant work:**
- SemEval shared tasks on causal relation extraction (2007-2020) established benchmarks
- Girju et al. (2002) -- early pattern-based causal extraction using lexical pairs
- Mirza & Tonelli (2014) -- temporal and causal relation extraction from text
- Li & Mao (2019) -- knowledge-oriented causal relation extraction from text using NLI formulations

**Limitations for FunDB:**
- Event descriptions in a database are typically short metadata, not full prose. Explicit causal markers may be absent.
- This approach only works when the *text itself* states a causal relationship, not when the causation must be inferred across records.

### 2.2 Implicit Causal Relation Detection

When text does not explicitly state causation, we need models that can infer it.

**Approaches:**
- **Natural Language Inference (NLI):** Given premise A and hypothesis "A caused B", does the text entail, contradict, or remain neutral about causation? Models like RoBERTa fine-tuned on MNLI can do this at ~90% accuracy on in-domain data.
- **Causal Commonsense Reasoning:** COMET (Bosselut et al., 2019) -- a generative model trained on ATOMIC that can generate plausible causal chains. Given "deployed new tokenizer", COMET can generate potential effects like "text processing errors", "encoding failures", etc.
- **CausalBERT (Veitch et al., 2020):** Adjustments to BERT-style models to separate correlation from causation in text embeddings by controlling for confounding context.

**Applicability to FunDB:**
- NLI is directly usable at query time. Given two FunRecords with text fields, we can formulate an NLI query: "Does [event A description] entail that [event B description] would follow?"
- COMET-style generation is heavier but could be run as a background enrichment process.

### 2.3 Large Language Models for Causal Reasoning

Modern LLMs (GPT-4 class, Claude class) have strong causal reasoning capabilities.

**Key findings:**
- Kiciman et al. (2023) -- "Causal Reasoning and Large Language Models" showed LLMs can match or exceed specialized models on causal discovery benchmarks
- Jin et al. (2023) -- "CLadder: Assessing Causal Reasoning in Language Models" demonstrated that LLMs can handle all three levels of Pearl's causal hierarchy (associational, interventional, counterfactual) in text
- Zevcic et al. (2023) -- showed that while LLMs have strong causal *verbalization*, they sometimes confuse correlation with causation in novel domains

**Relevance for FunDB:**
- FunDB already has an embedded LLM in its Semantic Interface (Tier 3 intent parser, ~500MB ONNX model)
- This can be repurposed for mechanism plausibility scoring
- However, LLM calls are the most expensive option (~100ms). We need a tiered approach that only invokes the LLM when cheaper methods are insufficient.

---

## 3. Embeddings for Causal vs. Correlational Relationships

### 3.1 Standard Sentence Embeddings

General-purpose embeddings (text-embedding-3-small, BGE, E5) capture semantic similarity, not causal direction.

**Key problem:** cos_sim("rain causes wet streets", "wet streets cause rain") is very high. Standard embeddings are symmetric -- they cannot distinguish cause from effect.

**This is a fundamental limitation:** Cosine similarity measures topical relatedness, not causal directionality. Two events with high embedding similarity may be:
- Causally related (deploy -> errors)
- Effects of a common cause (confounder!)
- Coincidentally similar in topic but unrelated

### 3.2 Knowledge Graph Embeddings

Knowledge graph embedding methods can capture directed relationships:

- **TransE (Bordes et al., 2013):** Models relations as translations in embedding space. If (h, r, t) is a true triple, then h + r ~ t. The relation vector r captures directionality.
- **RotatE (Sun et al., 2019):** Models relations as rotations in complex space. Can capture symmetry, antisymmetry, inversion, and composition patterns.
- **CompGCN (Vashishth et al., 2020):** Graph neural network that jointly embeds entities and relations, supporting multiple relation types.

**Applicability to FunDB:**
- FunDB already stores `_edges` (graph adjacency) and `_caused_by` / `_effects` (causal edges) on FunRecords
- The FunGraph index stores SPO (Subject-Predicate-Object) triples
- We could train TransE or RotatE embeddings over the existing causal edge graph to learn a "causes" relation vector
- Then, for a new (A, B) pair, we test whether A + r_causes ~ B in embedding space

**Critical insight:** We do NOT need to train these from scratch per query. The "causes" relation vector can be learned from the existing corpus of explicit causal edges (`_caused_by` with relation=CAUSED), and then applied as a zero-shot test for new pairs.

### 3.3 Causal Embedding Spaces

Recent work on learning causal-aware embeddings:

- **CausalRep (Feder et al., 2022):** Modifies representation learning to separate causal from spurious features using counterfactual data augmentation
- **Causal Direction in Embedding Spaces:** Research by Mikolov et al. showed word2vec captures some relational directions (king - man + woman = queen). This principle extends to causal relations: if we can identify the "causes" direction in embedding space, we can project onto it.
- **Asymmetric Distance Functions:** Instead of cosine similarity (symmetric), use a learned asymmetric distance d(A, B) != d(B, A) that captures cause-to-effect directionality.

### 3.4 Practical Embedding Approach for FunDB

Given FunDB's constraints (query-time latency, no per-query training), the most practical approach combines:

1. **Standard embeddings** for topical relatedness (already stored as `_vectors` in FunRecord)
2. **A learned causal direction vector** derived from existing `_caused_by` edges
3. **Asymmetric scoring** that combines similarity with directional projection

This avoids training new models per query while still capturing causal directionality.

---

## 4. Mechanism Detection Techniques

### 4.1 What is a "Mechanism"?

A mechanism is an intermediate process or pathway by which cause A produces effect B. In database terms, it means there exist intermediate concepts, entities, or events that form a plausible chain from A to B.

**Example:**
- A: "Deployed tokenizer v2"
- B: "Error rate spiked in Asian markets"
- Mechanism: tokenizer v2 -> changed Unicode handling -> CJK characters processed incorrectly -> errors in Asian market text

### 4.2 Semantic Bridge Detection

The core technique for mechanism detection: find records in FunDB that are semantically close to *both* A and B, forming a "bridge" between them.

**Algorithm sketch:**
1. Embed A and B (already done -- they are FunRecords with `_vectors`)
2. Search for records whose embedding is within threshold of both A and B
3. These bridge records represent potential mechanisms
4. Score bridges by: proximity to both A and B, temporal plausibility (bridge event between A and B in time), and confidence

**This is essentially a "semantic intersection" query** -- records in the embedding space region between A and B.

**Relevant research:**
- Camacho-Collados et al. (2019) -- semantic relatedness as a bridge concept in knowledge graphs
- Das et al. (2019) -- chains of reasoning over knowledge graphs using embedding similarity

### 4.3 Path-Based Mechanism Detection

Use FunDB's graph index to find paths from A to B through existing edges:

1. Direct causal edges: A --caused--> B (already known, trivial)
2. Indirect paths: A --edge--> X --edge--> B (mechanism through X)
3. Semantic + graph hybrid: A ~similar_to~ X --caused--> Y ~similar_to~ B

The hybrid approach is most powerful: it uses embedding similarity to "jump" between semantically related records and graph edges to traverse known relationships.

### 4.4 Textual Entailment Chains

Use NLI to build entailment chains:
1. Does text(A) entail text(M)? (cause plausibly leads to mechanism)
2. Does text(M) entail text(B)? (mechanism plausibly leads to effect)
3. If both entailments hold with reasonable confidence, M is a plausible mechanism

**This can be implemented using a cross-encoder NLI model** (e.g., DeBERTa fine-tuned on MNLI, ~400MB). The model takes (premise, hypothesis) pairs and outputs entailment/contradiction/neutral probabilities.

### 4.5 Domain-Specific Mechanism Libraries

For known domains (software deployments, infrastructure, ML pipelines), common mechanism templates exist:

- "code change" -> "behavior change" -> "metric change"
- "config change" -> "resource allocation change" -> "performance change"
- "data pipeline change" -> "data quality change" -> "model accuracy change"

These can be stored as templates in FunDB and matched against via embedding similarity. This provides rapid, high-confidence mechanism detection for known patterns.

---

## 5. Semantic Confounder Discovery

### 5.1 What Makes a Confounder Semantically Detectable?

A confounder Z satisfies:
- Z is a plausible cause of A
- Z is a plausible cause of B
- Z explains the observed correlation between A and B

**Semantic signals for confounders:**
- Z's embedding is close to both A's and B's embeddings
- Z's text co-occurs with or is mentioned alongside both A and B in documents
- Z temporally precedes both A and B
- Z shares graph edges with both A and B

### 5.2 The Embedding Triangle Test

For a candidate confounder Z:
- sim(Z, A) should be high (Z is related to A)
- sim(Z, B) should be high (Z is related to B)
- The "triangle" formed by A, B, Z in embedding space should be acute (Z is centrally located between A and B, not off to one side)

This geometric test in embedding space can rapidly filter candidate confounders from the entire database.

### 5.3 Temporal Confounder Filtering

A true confounder must temporally precede both A and B. FunDB's bitemporal indexing enables efficient filtering:
- Z._valid_from < A._valid_from
- Z._valid_from < B._valid_from

This eliminates many false positive confounders that are semantically related but temporally impossible.

### 5.4 Document Co-occurrence Mining

FunDB stores full documents (`data` field, MessagePack-encoded). We can mine for:
- Terms or entities that appear in documents related to both A and B
- Shared context windows: if a single document mentions concepts related to both A and B along with a third concept Z, then Z is a confounder candidate

**Implementation:** Use FunDB's inverted index (FunText) to find terms shared between documents related to A and documents related to B. Terms with high TF-IDF in both contexts are confounder candidates.

### 5.5 Graph-Based Confounder Search

Using FunDB's graph index:
- Find nodes Z that have edges to both A and B (or to records similar to A and B)
- Common ancestors in the causal DAG are confounder candidates
- Shared graph neighbors (records that link to both A and B) indicate shared context

### 5.6 Prior Art in Automated Confounder Discovery

- **The "Deconfounder" (Wang & Blei, 2019):** Uses factor models to infer latent confounders from observed data. Controversial (see D'Amour 2019 criticism) but the idea of using latent structure to find confounders is relevant.
- **Causal Discovery Algorithms (PC, FCI, GES):** These constraint-based and score-based algorithms search for causal structures, including confounders, from observational data. They are compute-heavy but produce principled results.
- **Text-based Confounding Adjustment (Roberts et al., 2020):** Uses text as a high-dimensional proxy for confounders. The idea: if you can represent the confounding context as text, then conditioning on text embeddings approximates conditioning on the confounder.

**Key insight for FunDB:** FunDB already stores text, embeddings, graph edges, and time-series for every record. This is an extraordinarily rich substrate for confounder search -- far richer than what is available in typical statistical settings where confounders must be manually hypothesized.

---

## 6. Key Limitations of Semantic Approaches

### 6.1 Semantic Similarity is Not Causation

High embedding similarity between A and B means they are *about the same topic*, not that one causes the other. This is the fundamental limitation. Semantic methods can identify *candidates* for causal relationships and mechanisms, but cannot confirm causation alone.

### 6.2 Embedding Space Geometry is Domain-Dependent

The "causal direction" in embedding space may not be consistent across domains. A causal direction vector learned from software deployment incidents may not transfer to financial market events. The system must be domain-aware or learn domain-specific causal geometry.

### 6.3 Text Descriptions May Be Incomplete or Misleading

FunRecords contain human- or agent-written text. This text may:
- Omit important contextual information
- Use domain-specific jargon that general embeddings do not capture well
- Contain post-hoc rationalizations (the mechanism described may not be the true mechanism)

### 6.4 Confounders Can Be Latent

Some confounders have no representation in the database at all. If no record describes "Q4 holiday season" as an event, semantic search cannot find it as a confounder. The system must be honest about this limitation and report when confounder search coverage is low.

### 6.5 Computational Cost of Exhaustive Search

Searching the entire database for confounders is O(N) in the number of records. With millions of records, even ANN search has nontrivial cost. The system needs aggressive candidate pruning strategies.

### 6.6 False Mechanism Detection

A semantic bridge between A and B does not guarantee a true mechanism. The bridge may be:
- A coincidental semantic intermediate (related to both A and B by topic, not by mechanism)
- An effect rather than a mechanism (C is caused by A, and is similar to B, but does not mediate A -> B)
- A linguistic artifact (the descriptions happen to share vocabulary)

---

## 7. Integration Points with FunDB Architecture

### 7.1 Data Already Available

From the FunRecord structure, we have:
- `_vectors`: Named embedding fields for semantic search
- `_edges`: Graph adjacency for path finding
- `_caused_by` / `_effects`: Explicit causal edges with strength and mechanism text
- `_confidence`: Trust scores for weighting
- `_sources`: Provenance for source reliability
- `data`: Full document payload for text analysis
- `_valid_from` / `_valid_to`: Temporal ordering for precedence checks

### 7.2 Indexes We Can Leverage

- **HNSW (FunVector):** ANN search for finding semantically similar records
- **FunGraph (SPO):** Path queries for mechanism chains
- **FunCausal (DAG):** Existing causal edge traversal
- **FunText (Inverted):** Full-text search for co-occurrence mining
- **FunTemporal:** Efficient temporal range filtering
- **FunConfidence:** Confidence-based pruning

### 7.3 Query Engine Integration

FunQL already supports:
- `TRACE CAUSALITY` syntax for causal path queries
- `_vector() <->` for similarity search
- `TRAVERSE` for graph traversal
- `AS OF` for temporal queries
- `_confidence` filtering

Our mechanism detection and confounder search algorithms need to compose these existing primitives, not introduce entirely new query execution operators.

---

## 8. References

1. Girju, R. (2002). Text Mining for Causal Relations. FLAIRS Conference.
2. Mirza, P., & Tonelli, S. (2014). Analysis of temporal relations and causal chains. COLING.
3. Bosselut, A., et al. (2019). COMET: Commonsense Transformers for Knowledge Graph Construction. ACL.
4. Bordes, A., et al. (2013). Translating Embeddings for Modeling Multi-relational Data. NeurIPS.
5. Sun, Z., et al. (2019). RotatE: Knowledge Graph Embedding by Relational Rotation in Complex Space. ICLR.
6. Veitch, V., et al. (2020). Adapting Text Embeddings for Causal Inference. UAI.
7. Kiciman, E., et al. (2023). Causal Reasoning and Large Language Models: Opening a New Frontier. arXiv.
8. Jin, Z., et al. (2023). CLadder: Assessing Causal Reasoning in Language Models. NeurIPS.
9. Wang, Y., & Blei, D. (2019). The Blessings of Multiple Causes. JASA.
10. Roberts, M., et al. (2020). Adjusting for Confounding with Text Matching. AJPS.
11. Feder, A., et al. (2022). Causal Inference in Natural Language Processing: Estimation, Prediction, Interpretation. TACL.
12. Li, Z., & Mao, X. (2019). Knowledge-oriented Causal Relation Extraction from Natural Language Text. arXiv.
13. Pearl, J. (2009). Causality: Models, Reasoning, and Inference. Cambridge University Press.
14. D'Amour, A. (2019). On Multi-Cause Approaches to Causal Inference. arXiv.
