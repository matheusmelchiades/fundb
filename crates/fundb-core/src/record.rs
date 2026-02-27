use std::collections::HashMap;
use uuid::Uuid;
use crate::types::{
    CausalOrigin, CausalType, DirectionStatus, SourceMethod, StabilityStatus, Timestamp,
};

// ---------------------------------------------------------------------------
// Sub-structs
// ---------------------------------------------------------------------------

/// A directed graph edge stored inside a FunRecord.
///
/// Corresponds to `_edges: []Edge{label, target, props}` in ARCHITECTURE.md §4.3.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    /// Relationship label, e.g. `"follows"`, `"cites"`, `"belongs_to"`.
    pub label:      String,
    /// UUID of the target FunRecord.
    pub target:     Uuid,
    /// MessagePack-encoded edge properties (schema-on-read).
    pub props:      Vec<u8>,
    /// Confidence that this edge is correct (0.0–1.0).
    pub confidence: f32,
}

/// A single time-series data point stored inside a FunRecord.
///
/// Corresponds to `_timeseries: []Sample{ts, value}` in ARCHITECTURE.md §4.3.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    /// Timestamp of the measurement (Unix nanoseconds).
    pub ts:    Timestamp,
    /// Scalar measurement value.
    pub value: f64,
}

/// One entry in the provenance chain of a FunRecord.
///
/// Corresponds to `_sources: []Source` in ARCHITECTURE.md §4.3.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Source {
    /// Human-readable origin identifier, e.g. `"model:gpt-4"`, `"sensor:temp-01"`.
    pub origin:     String,
    /// When this source produced or asserted the fact (Unix nanoseconds).
    pub timestamp:  Timestamp,
    /// How this information was generated.
    pub method:     SourceMethod,
    /// Trust score assigned to this particular source (0.0–1.0).
    pub confidence: f32,
}

/// A weak reference to another FunRecord, with a scalar association strength.
///
/// Used in `_supports` and `_contradicts` fields of FunRecord.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Ref {
    /// UUID of the referenced FunRecord.
    pub id:       Uuid,
    /// Strength of the support or contradiction relationship (0.0–1.0).
    pub strength: f32,
}

/// A directed causal edge connecting two FunRecords.
///
/// Incorporates all OQ resolutions from `causal-design-decisions.md`:
/// - Decision 2 (OQ-6): `origin` + `confidence` for hallucination protection.
/// - Decision 3 (OQ-7): `stability_score` + `stability_status` for stationarity.
/// - Decision 5 (OQ-10): `direction_status` + `discovery_algo` for ensemble discovery.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CausalEdge {
    /// UUID of the cause record (source of the causal arrow).
    pub source_id:        Uuid,
    /// UUID of the effect record (target of the causal arrow).
    pub target_id:        Uuid,
    /// Semantic classification of the causal relationship.
    pub relation:         CausalType,
    /// Effect size / magnitude of the causal influence (0.0–1.0).
    pub strength:         f32,
    /// Optional human-readable description of the causal mechanism.
    pub mechanism:        Option<String>,

    // --- OQ-6: Origin tracking + confidence ceiling (Decision 2) -----------
    /// How this edge was discovered or declared.
    pub origin:           CausalOrigin,
    /// Epistemic confidence that this edge represents real causation (0.0–1.0).
    /// Subject to confidence ceilings per `CausalOrigin` variant.
    pub confidence:       f32,

    // --- OQ-7: Stationarity metadata (Decision 3) --------------------------
    /// Rolling-window p-value stability metric (0.0–1.0).
    /// `None` when the records are not time-series data.
    pub stability_score:  Option<f32>,
    /// Qualitative stationarity classification derived from `stability_score`.
    pub stability_status: StabilityStatus,

    // --- OQ-10: Ensemble discovery metadata (Decision 5) -------------------
    /// Whether the discovery algorithms agreed on the direction of this edge.
    pub direction_status: DirectionStatus,
    /// Which algorithm (or ensemble) produced this edge.
    /// Examples: `"ensemble"`, `"pc"`, `"notears"`, `"granger"`.
    pub discovery_algo:   Option<String>,
}

// ---------------------------------------------------------------------------
// FunRecord — the core unit of knowledge
// ---------------------------------------------------------------------------

/// The fundamental storage and knowledge unit of FunDB.
///
/// A single FunRecord can simultaneously act as:
/// - A **document** (via `data`, MessagePack-encoded)
/// - A **vector embedding holder** (via `_vectors`)
/// - A **graph node** (via `_edges`)
/// - A **time-series container** (via `_timeseries`)
/// - A **knowledge fact with confidence and provenance** (cognitive metadata fields)
/// - A **causal participant** (via `_caused_by` / `_effects`)
///
/// All fields follow the spec in ARCHITECTURE.md §4.3 plus OQ resolutions from
/// `causal-design-decisions.md`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunRecord {
    // --- Core Identity -----------------------------------------------------
    /// Unique record identifier. Always UUID v7 (time-ordered).
    pub _id:          Uuid,
    /// Logical collection (namespace) this record belongs to.
    pub _collection:  String,
    /// Tenant identifier for multi-tenancy isolation.
    pub _tenant:      u32,

    // --- Temporal Envelope (bitemporal MVCC) -------------------------------
    /// System time: when this version was physically written (Unix nanoseconds).
    pub _sys_from:    Timestamp,
    /// System time: when this version was superseded. `i64::MAX` for the current version.
    pub _sys_to:      Timestamp,
    /// Valid time: earliest application-layer validity (Unix nanoseconds).
    pub _valid_from:  Timestamp,
    /// Valid time: end of application-layer validity (Unix nanoseconds).
    pub _valid_to:    Timestamp,

    // --- Knowledge Payload ------------------------------------------------
    /// MessagePack-encoded document payload (schema-on-read).
    pub data:         Vec<u8>,

    // --- Typed Extensions -------------------------------------------------
    /// Named vector embeddings, e.g. `{ "content_embedding": [...] }`.
    pub _vectors:     HashMap<String, Vec<f32>>,
    /// Graph adjacency list — outbound edges from this record.
    pub _edges:       Vec<Edge>,
    /// Time-series data points embedded in this record.
    pub _timeseries:  Vec<Sample>,

    // --- Cognitive Metadata -----------------------------------------------
    /// Aggregate trust score for this fact (0.0–1.0). Defaults to `1.0`.
    pub _confidence:  f32,
    /// Provenance chain — ordered list of sources that contributed to this fact.
    pub _sources:     Vec<Source>,
    /// Other records that corroborate this fact.
    pub _supports:    Vec<Ref>,
    /// Other records that contradict this fact.
    pub _contradicts: Vec<Ref>,
    /// Incoming causal edges — what caused or influenced this record.
    pub _caused_by:   Vec<CausalEdge>,
    /// Outgoing causal edges — what this record caused or influenced.
    pub _effects:     Vec<CausalEdge>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::FunRecordBuilder;
    use crate::id::new_record_id;
    use crate::types::{
        CausalOrigin, CausalType, DirectionStatus, SourceMethod, StabilityStatus,
    };

    /// Test 1: Builder defaults — _sys_to == i64::MAX, _confidence == 1.0
    #[test]
    fn test_funrecord_builder_defaults() {
        let record = FunRecordBuilder::new("test_collection").build();

        assert_eq!(
            record._sys_to,
            i64::MAX,
            "_sys_to must be i64::MAX for the current (live) version"
        );
        assert!(
            (record._confidence - 1.0_f32).abs() < f32::EPSILON,
            "_confidence must default to 1.0, got {}",
            record._confidence
        );
        assert_eq!(record._collection, "test_collection");
        assert_eq!(record._tenant, 0, "default tenant must be 0");
        assert!(record.data.is_empty(), "default data payload must be empty");
        assert!(record._vectors.is_empty());
        assert!(record._edges.is_empty());
        assert!(record._timeseries.is_empty());
        assert!(record._sources.is_empty());
        assert!(record._supports.is_empty());
        assert!(record._contradicts.is_empty());
        assert!(record._caused_by.is_empty());
        assert!(record._effects.is_empty());
        // _sys_from must be a recent timestamp (not zero)
        assert!(record._sys_from > 0, "_sys_from must be set to current time");
    }

    /// Test 2: UUID v7 ordering — 1000 sequential IDs must sort in generation order.
    #[test]
    fn test_uuid_v7_ordering() {
        let count = 1_000;
        let ids: Vec<uuid::Uuid> = (0..count).map(|_| new_record_id()).collect();

        let mut sorted = ids.clone();
        sorted.sort_by_key(|u| *u.as_bytes());

        assert_eq!(
            ids, sorted,
            "UUID v7 IDs must be lexicographically ordered by generation time"
        );
    }

    /// Test 3: CausalEdge stores all OQ-resolved fields correctly.
    #[test]
    fn test_causal_edge_fields() {
        let source = new_record_id();
        let target = new_record_id();

        let edge = CausalEdge {
            source_id:        source,
            target_id:        target,
            relation:         CausalType::Caused,
            strength:         0.80,
            mechanism:        Some("deployed new model → error rate increased".to_string()),
            // OQ-6 fields
            origin:           CausalOrigin::Granger { p_value: 0.03, lag: 2 },
            confidence:       0.75,
            // OQ-7 fields
            stability_score:  Some(0.65),
            stability_status: StabilityStatus::Stable,
            // OQ-10 fields
            direction_status: DirectionStatus::Confirmed,
            discovery_algo:   Some("ensemble".to_string()),
        };

        assert_eq!(edge.source_id, source);
        assert_eq!(edge.target_id, target);
        assert_eq!(edge.relation, CausalType::Caused);
        assert!((edge.strength - 0.80).abs() < 1e-6);
        assert_eq!(
            edge.mechanism.as_deref(),
            Some("deployed new model → error rate increased")
        );

        // OQ-6
        match &edge.origin {
            CausalOrigin::Granger { p_value, lag } => {
                assert!((p_value - 0.03).abs() < 1e-10);
                assert_eq!(*lag, 2);
            }
            other => panic!("expected Granger origin, got {:?}", other),
        }
        assert!((edge.confidence - 0.75).abs() < 1e-6);

        // OQ-7
        assert!((edge.stability_score.unwrap() - 0.65).abs() < 1e-6);
        assert_eq!(edge.stability_status, StabilityStatus::Stable);

        // OQ-10
        assert_eq!(edge.direction_status, DirectionStatus::Confirmed);
        assert_eq!(edge.discovery_algo.as_deref(), Some("ensemble"));
    }

    /// Test 4: Build a record with every field populated and verify nothing is
    /// unexpectedly empty or default when explicitly set.
    #[test]
    fn test_funrecord_all_fields() {
        let causal_source = new_record_id();
        let causal_target = new_record_id();
        let edge_target   = new_record_id();

        let source = Source {
            origin:     "model:gpt-4o".to_string(),
            timestamp:  1_700_000_000_000_000_000_i64,
            method:     SourceMethod::Inference,
            confidence: 0.85,
        };

        let edge = Edge {
            label:      "cites".to_string(),
            target:     edge_target,
            props:      vec![0x80], // empty msgpack map
            confidence: 0.90,
        };

        let causal_edge = CausalEdge {
            source_id:        causal_source,
            target_id:        causal_target,
            relation:         CausalType::Influenced,
            strength:         0.55,
            mechanism:        Some("indirect feedback".to_string()),
            origin:           CausalOrigin::LlmValidated {
                model:           "claude-3-opus".to_string(),
                coherence_score: 0.58,
            },
            confidence:       0.60,
            stability_score:  None,
            stability_status: StabilityStatus::NotApplicable,
            direction_status: DirectionStatus::DirectionUncertain,
            discovery_algo:   Some("pc".to_string()),
        };

        let record = FunRecordBuilder::new("research_papers")
            .tenant(42)
            .data(vec![0x81, 0xa5, 0x74, 0x69, 0x74, 0x6c, 0x65, 0xa3, 0x66, 0x6f, 0x6f])
            .vector("content_embedding", vec![0.1, 0.2, 0.3])
            .edge(edge.clone())
            .confidence(0.88)
            .source(source.clone())
            .valid_time(
                1_700_000_000_000_000_000_i64,
                1_800_000_000_000_000_000_i64,
            )
            .caused_by(causal_edge.clone())
            .build();

        // Core identity
        assert_eq!(record._collection, "research_papers");
        assert_eq!(record._tenant, 42);
        // UUID must be a valid v7
        assert_eq!(record._id.get_version(), Some(uuid::Version::SortRand));

        // Temporal envelope
        assert_eq!(record._sys_to, i64::MAX);
        assert!(record._sys_from > 0);
        assert_eq!(record._valid_from, 1_700_000_000_000_000_000_i64);
        assert_eq!(record._valid_to,   1_800_000_000_000_000_000_i64);

        // Payload
        assert!(!record.data.is_empty());

        // Vectors
        let emb = record._vectors.get("content_embedding").expect("vector must exist");
        assert_eq!(emb.len(), 3);
        assert!((emb[0] - 0.1).abs() < 1e-6);

        // Edges
        assert_eq!(record._edges.len(), 1);
        assert_eq!(record._edges[0].label, "cites");
        assert_eq!(record._edges[0].target, edge_target);

        // Confidence
        assert!((record._confidence - 0.88).abs() < 1e-6);

        // Sources
        assert_eq!(record._sources.len(), 1);
        assert_eq!(record._sources[0].origin, "model:gpt-4o");
        assert_eq!(record._sources[0].method, SourceMethod::Inference);

        // Causal edges
        assert_eq!(record._caused_by.len(), 1);
        let ce = &record._caused_by[0];
        assert_eq!(ce.source_id, causal_source);
        assert_eq!(ce.target_id, causal_target);
        assert_eq!(ce.relation, CausalType::Influenced);
        assert!((ce.confidence - 0.60).abs() < 1e-6);
        assert_eq!(ce.stability_status, StabilityStatus::NotApplicable);
        assert_eq!(ce.direction_status, DirectionStatus::DirectionUncertain);
        assert_eq!(ce.discovery_algo.as_deref(), Some("pc"));

        // Unused fields default to empty
        assert!(record._supports.is_empty());
        assert!(record._contradicts.is_empty());
        assert!(record._effects.is_empty());
        assert!(record._timeseries.is_empty());
    }
}
