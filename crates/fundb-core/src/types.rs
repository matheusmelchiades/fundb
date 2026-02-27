/// Unix nanoseconds timestamp.
pub type Timestamp = i64;

/// Composite key for a record: collection namespace + record UUID bytes.
/// Implements lexicographic ordering so B+Tree scans stay within a collection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct RecordKey {
    pub collection: String,
    pub id:         [u8; 16],   // UUID bytes — UUID v7 keeps time-ordered sort
}

// ---------------------------------------------------------------------------
// Causal relationship classification
// ---------------------------------------------------------------------------

/// The semantic type of a directed causal edge between two records.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CausalType {
    /// Direct, mechanistic causation: A produced B.
    Caused,
    /// Partial or modulating influence: A increased/decreased the likelihood of B.
    Influenced,
    /// Statistical correlation without established direction.
    Correlated,
    /// Temporal precedence only: A happened before B.
    Preceded,
}

/// How the causal edge was discovered or asserted.
///
/// Implements Decision 2 (OQ-6) confidence ceilings:
/// - `UserDeclared`        → max initial confidence 1.00
/// - `Granger`             → max initial confidence 0.85
/// - `TemporalPrecedence`  → max initial confidence 0.75
/// - `LlmValidated`        → max initial confidence 0.60
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CausalOrigin {
    /// Human or upstream system explicitly declared this edge as causal.
    UserDeclared,

    /// Edge was validated by a Granger causality test.
    Granger {
        /// p-value from the Granger F-test (lower is more significant).
        p_value: f64,
        /// Optimal lag in time steps that produced the lowest p-value.
        lag:     u32,
    },

    /// Edge inferred purely from temporal ordering and correlation.
    TemporalPrecedence {
        /// Pearson or Spearman correlation coefficient at the optimal lag.
        correlation: f64,
    },

    /// Edge proposed by an LLM and passed the Three-Layer Shield (Decision 2).
    LlmValidated {
        /// Model identifier, e.g. "gpt-4o", "claude-3-opus".
        model:            String,
        /// Counterfactual coherence score returned by the coherence check (0.0–1.0).
        coherence_score:  f32,
    },
}

// ---------------------------------------------------------------------------
// Stationarity / stability metadata  (Decision 3, OQ-7)
// ---------------------------------------------------------------------------

/// Stability classification derived from rolling-window Granger p-value variance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StabilityStatus {
    /// `stability_score >= 0.6` — edge holds consistently across time windows.
    Stable,
    /// `stability_score < 0.4` — edge is present but unreliable.
    Unstable,
    /// `0.4 <= stability_score < 0.6` — edge is tentatively stable.
    Provisional,
    /// Record is not a time-series; stationarity analysis is not applicable.
    NotApplicable,
}

// ---------------------------------------------------------------------------
// Ensemble discovery metadata  (Decision 5, OQ-10)
// ---------------------------------------------------------------------------

/// Direction confidence from ensemble causal discovery (PC + NOTEARS + Granger).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DirectionStatus {
    /// All algorithms that produced an output agreed on the direction X→Y.
    Confirmed,
    /// Algorithms disagreed on direction; edge persisted with reduced confidence.
    DirectionUncertain,
}

// ---------------------------------------------------------------------------
// Agent memory classification
// ---------------------------------------------------------------------------

/// Cognitive memory type — mirrors human memory research taxonomy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MemoryType {
    /// Event-specific, time-stamped recollection ("what happened at T").
    Episodic,
    /// General conceptual knowledge ("what X is").
    Semantic,
    /// How-to knowledge; skills and procedures.
    Procedural,
}

// ---------------------------------------------------------------------------
// Provenance / source method
// ---------------------------------------------------------------------------

/// How a source record was generated or collected.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SourceMethod {
    /// Derived by a model or rule engine.
    Inference,
    /// Measured directly from a sensor or external event.
    Observation,
    /// Computed by aggregating other records.
    Aggregation,
    /// Bulk-imported from an external data source.
    Import,
    /// Hand-labeled or entered by a human annotator.
    ManualAnnotation,
}
