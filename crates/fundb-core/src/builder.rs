use std::collections::BTreeMap;
use crate::id::new_record_id;
use crate::record::{CausalEdge, Edge, FunRecord, Sample, Source};
use crate::types::Timestamp;

/// Fluent builder for [`FunRecord`].
///
/// Required fields:
/// - `collection` — supplied to [`FunRecordBuilder::new`].
///
/// Defaults:
/// - `_id`         — generated via [`new_record_id`] (UUID v7) on [`build`].
/// - `_tenant`     — `0`
/// - `_sys_from`   — current time (Unix nanoseconds) on [`build`].
/// - `_sys_to`     — `i64::MAX` (current version marker).
/// - `_valid_from` — `0`
/// - `_valid_to`   — `i64::MAX`
/// - `_confidence` — `1.0`
/// - All collection fields (`_vectors`, `_edges`, …) — empty.
///
/// # Example
/// ```
/// use fundb_core::FunRecordBuilder;
/// let record = FunRecordBuilder::new("my_collection")
///     .tenant(1)
///     .confidence(0.9)
///     .build();
/// assert_eq!(record._collection, "my_collection");
/// assert_eq!(record._sys_to, i64::MAX);
/// ```
#[derive(Debug)]
pub struct FunRecordBuilder {
    collection:  String,
    tenant:      u32,
    data:        Vec<u8>,
    vectors:     BTreeMap<String, Vec<f32>>,
    edges:       Vec<Edge>,
    timeseries:  Vec<Sample>,
    confidence:  f32,
    sources:     Vec<Source>,
    supports:    Vec<crate::record::Ref>,
    contradicts: Vec<crate::record::Ref>,
    caused_by:   Vec<CausalEdge>,
    effects:     Vec<CausalEdge>,
    valid_from:  Timestamp,
    valid_to:    Timestamp,
}

impl FunRecordBuilder {
    /// Create a new builder for the given collection name.
    pub fn new(collection: impl Into<String>) -> Self {
        Self {
            collection:  collection.into(),
            tenant:      0,
            data:        Vec::new(),
            vectors:     BTreeMap::new(),
            edges:       Vec::new(),
            timeseries:  Vec::new(),
            confidence:  1.0,
            sources:     Vec::new(),
            supports:    Vec::new(),
            contradicts: Vec::new(),
            caused_by:   Vec::new(),
            effects:     Vec::new(),
            valid_from:  0,
            valid_to:    i64::MAX,
        }
    }

    /// Set the tenant identifier.
    pub fn tenant(mut self, id: u32) -> Self {
        self.tenant = id;
        self
    }

    /// Set the raw MessagePack-encoded document payload.
    pub fn data(mut self, payload: Vec<u8>) -> Self {
        self.data = payload;
        self
    }

    /// Add (or overwrite) a named vector embedding.
    pub fn vector(mut self, name: impl Into<String>, vec: Vec<f32>) -> Self {
        self.vectors.insert(name.into(), vec);
        self
    }

    /// Append a graph edge.
    pub fn edge(mut self, edge: Edge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Append a time-series sample.
    pub fn sample(mut self, sample: Sample) -> Self {
        self.timeseries.push(sample);
        self
    }

    /// Set the aggregate confidence score (0.0–1.0).
    pub fn confidence(mut self, c: f32) -> Self {
        self.confidence = c;
        self
    }

    /// Append a provenance source.
    pub fn source(mut self, s: Source) -> Self {
        self.sources.push(s);
        self
    }

    /// Append a supporting reference.
    pub fn supports(mut self, r: crate::record::Ref) -> Self {
        self.supports.push(r);
        self
    }

    /// Append a contradicting reference.
    pub fn contradicts(mut self, r: crate::record::Ref) -> Self {
        self.contradicts.push(r);
        self
    }

    /// Set the application-layer validity window.
    pub fn valid_time(mut self, from: Timestamp, to: Timestamp) -> Self {
        self.valid_from = from;
        self.valid_to   = to;
        self
    }

    /// Append an incoming causal edge (what caused this record).
    pub fn caused_by(mut self, edge: CausalEdge) -> Self {
        self.caused_by.push(edge);
        self
    }

    /// Append an outgoing causal edge (what this record caused).
    pub fn effect(mut self, edge: CausalEdge) -> Self {
        self.effects.push(edge);
        self
    }

    /// Consume the builder and produce a fully-initialized [`FunRecord`].
    ///
    /// - `_id` is generated as UUID v7 (time-ordered).
    /// - `_sys_from` is set to the current monotonic wall-clock time in Unix nanoseconds.
    /// - `_sys_to` is set to `i64::MAX` (this is the live/current version).
    pub fn build(self) -> FunRecord {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time must be after UNIX epoch")
            .as_nanos() as i64;

        FunRecord {
            _id:          new_record_id(),
            _collection:  self.collection,
            _tenant:      self.tenant,
            _sys_from:    now_ns,
            _sys_to:      i64::MAX,
            _valid_from:  self.valid_from,
            _valid_to:    self.valid_to,
            data:         self.data,
            _vectors:     self.vectors,
            _edges:       self.edges,
            _timeseries:  self.timeseries,
            _confidence:  self.confidence,
            _sources:     self.sources,
            _supports:    self.supports,
            _contradicts: self.contradicts,
            _caused_by:   self.caused_by,
            _effects:     self.effects,
        }
    }
}
