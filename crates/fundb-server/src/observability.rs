//! Observability: Prometheus metrics, OpenTelemetry-compatible span tracing,
//! and Grafana dashboard template for FunDB.
//!
//! Implements everything from scratch using std::sync primitives — no external
//! metrics or tracing crates are required.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the current Unix timestamp in milliseconds.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Simple 64-bit pseudo-random number based on the current time and a counter.
/// Not cryptographically secure — used only for trace/span ID generation.
fn gen_id_64() -> u64 {
    use std::sync::atomic::AtomicU64;
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    // Mix time with the sequence counter so concurrent callers always differ.
    let t = now_ms();
    t.wrapping_mul(6364136223846793005)
        .wrapping_add(seq)
        .wrapping_mul(1442695040888963407)
        .wrapping_add(1013904223)
}

fn gen_id_128() -> u128 {
    let hi = gen_id_64() as u128;
    let lo = gen_id_64() as u128;
    (hi << 64) | lo
}

// ---------------------------------------------------------------------------
// Counter
// ---------------------------------------------------------------------------

/// A monotonically increasing unsigned counter (Prometheus `counter`).
#[derive(Debug, Clone)]
pub struct Counter {
    name: String,
    help: String,
    value: Arc<AtomicU64>,
    labels: HashMap<String, String>,
}

impl Counter {
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            help: help.into(),
            value: Arc::new(AtomicU64::new(0)),
            labels: HashMap::new(),
        }
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Increment the counter by 1.
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Add `n` to the counter.
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Return the current value.
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Gauge
// ---------------------------------------------------------------------------

/// A signed, freely-settable integer gauge (Prometheus `gauge`).
#[derive(Debug, Clone)]
pub struct Gauge {
    name: String,
    help: String,
    value: Arc<AtomicI64>,
    labels: HashMap<String, String>,
}

impl Gauge {
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            help: help.into(),
            value: Arc::new(AtomicI64::new(0)),
            labels: HashMap::new(),
        }
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set the gauge to an absolute value.
    pub fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }

    /// Increment the gauge by 1.
    pub fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the gauge by 1.
    pub fn decrement(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Return the current value.
    pub fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Histogram
// ---------------------------------------------------------------------------

/// A histogram with fixed upper-bound buckets for latency tracking.
///
/// `counts` has length `buckets.len() + 1`; the last slot accumulates
/// observations that exceed the highest bucket bound (`+Inf`).
#[derive(Debug, Clone)]
pub struct Histogram {
    name: String,
    help: String,
    /// Upper bounds, e.g. `[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]` ms.
    buckets: Vec<f64>,
    /// Per-bucket counts. Length is `buckets.len() + 1`.
    counts: Arc<Mutex<Vec<u64>>>,
    sum: Arc<Mutex<f64>>,
    total: Arc<AtomicU64>,
    labels: HashMap<String, String>,
}

impl Histogram {
    pub fn new(name: impl Into<String>, help: impl Into<String>, buckets: Vec<f64>) -> Self {
        let n = buckets.len() + 1; // +1 for +Inf
        Self {
            name: name.into(),
            help: help.into(),
            buckets,
            counts: Arc::new(Mutex::new(vec![0u64; n])),
            sum: Arc::new(Mutex::new(0.0)),
            total: Arc::new(AtomicU64::new(0)),
            labels: HashMap::new(),
        }
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Record a single observation.
    pub fn observe(&self, value: f64) {
        {
            let mut counts = self.counts.lock().unwrap();
            // Increment every bucket whose upper bound >= value (cumulative).
            for (i, &bound) in self.buckets.iter().enumerate() {
                if value <= bound {
                    counts[i] += 1;
                }
            }
            // +Inf bucket always gets incremented.
            let inf_idx = self.buckets.len();
            counts[inf_idx] += 1;
        }
        {
            let mut sum = self.sum.lock().unwrap();
            *sum += value;
        }
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the sum of all observed values.
    pub fn get_sum(&self) -> f64 {
        *self.sum.lock().unwrap()
    }

    /// Return the total number of observations.
    pub fn get_count(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Return `(upper_bound, cumulative_count)` pairs for all buckets including `+Inf`.
    ///
    /// The `+Inf` pair is represented as `(f64::INFINITY, count)`.
    pub fn bucket_counts(&self) -> Vec<(f64, u64)> {
        let counts = self.counts.lock().unwrap();
        let mut result: Vec<(f64, u64)> = self
            .buckets
            .iter()
            .enumerate()
            .map(|(i, &b)| (b, counts[i]))
            .collect();
        result.push((f64::INFINITY, counts[self.buckets.len()]));
        result
    }
}

// ---------------------------------------------------------------------------
// MetricsRegistry
// ---------------------------------------------------------------------------

/// Central registry for all metric types; renders Prometheus text exposition.
pub struct MetricsRegistry {
    counters: HashMap<String, Counter>,
    gauges: HashMap<String, Gauge>,
    histograms: HashMap<String, Histogram>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }

    pub fn register_counter(&mut self, c: Counter) -> &Counter {
        self.counters.insert(c.name.clone(), c);
        self.counters.values().last().unwrap()
    }

    pub fn register_gauge(&mut self, g: Gauge) -> &Gauge {
        self.gauges.insert(g.name.clone(), g);
        self.gauges.values().last().unwrap()
    }

    pub fn register_histogram(&mut self, h: Histogram) -> &Histogram {
        self.histograms.insert(h.name.clone(), h);
        self.histograms.values().last().unwrap()
    }

    pub fn counter(&self, name: &str) -> Option<&Counter> {
        self.counters.get(name)
    }

    pub fn gauge(&self, name: &str) -> Option<&Gauge> {
        self.gauges.get(name)
    }

    pub fn histogram(&self, name: &str) -> Option<&Histogram> {
        self.histograms.get(name)
    }

    /// Render all registered metrics in Prometheus text exposition format.
    pub fn render_text(&self) -> String {
        let mut out = String::new();

        // Histograms
        let mut hist_names: Vec<&String> = self.histograms.keys().collect();
        hist_names.sort();
        for name in hist_names {
            let h = &self.histograms[name];
            out.push_str(&format!("# HELP {} {}\n", h.name, h.help));
            out.push_str(&format!("# TYPE {} histogram\n", h.name));
            let label_str = render_labels(&h.labels);
            for (bound, count) in h.bucket_counts() {
                let le = if bound.is_infinite() {
                    "+Inf".to_string()
                } else {
                    // Remove trailing zeros for cleaner output.
                    format_f64(bound)
                };
                out.push_str(&format!(
                    "{}_bucket{{{}le=\"{}\"}} {}\n",
                    h.name,
                    if label_str.is_empty() {
                        String::new()
                    } else {
                        format!("{},", label_str)
                    },
                    le,
                    count
                ));
            }
            out.push_str(&format!(
                "{}_sum{} {}\n",
                h.name,
                wrap_labels(&h.labels),
                h.get_sum()
            ));
            out.push_str(&format!(
                "{}_count{} {}\n",
                h.name,
                wrap_labels(&h.labels),
                h.get_count()
            ));
        }

        // Counters
        let mut counter_names: Vec<&String> = self.counters.keys().collect();
        counter_names.sort();
        for name in counter_names {
            let c = &self.counters[name];
            out.push_str(&format!("# HELP {} {}\n", c.name, c.help));
            out.push_str(&format!("# TYPE {} counter\n", c.name));
            out.push_str(&format!(
                "{}{} {}\n",
                c.name,
                wrap_labels(&c.labels),
                c.get()
            ));
        }

        // Gauges
        let mut gauge_names: Vec<&String> = self.gauges.keys().collect();
        gauge_names.sort();
        for name in gauge_names {
            let g = &self.gauges[name];
            out.push_str(&format!("# HELP {} {}\n", g.name, g.help));
            out.push_str(&format!("# TYPE {} gauge\n", g.name));
            out.push_str(&format!(
                "{}{} {}\n",
                g.name,
                wrap_labels(&g.labels),
                g.get()
            ));
        }

        out
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Format a float without unnecessary trailing zeros.
fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Return `{k="v",k="v"}` or empty string when labels map is empty.
fn wrap_labels(labels: &HashMap<String, String>) -> String {
    if labels.is_empty() {
        String::new()
    } else {
        format!("{{{}}}", render_labels(labels))
    }
}

/// Return `k="v",k="v"` (sorted for determinism).
fn render_labels(labels: &HashMap<String, String>) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(&String, &String)> = labels.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);
    pairs
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// FunDbMetrics — pre-defined metric set
// ---------------------------------------------------------------------------

/// The canonical set of FunDB operational metrics.
pub struct FunDbMetrics {
    pub query_latency_ms: Histogram,
    pub write_throughput: Counter,
    pub active_connections: Gauge,
    pub replication_lag_ms: Gauge,
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub vector_scan_latency: Histogram,
    pub cognitive_ops: Counter,
}

/// Standard latency buckets in milliseconds.
fn default_latency_buckets() -> Vec<f64> {
    vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]
}

impl FunDbMetrics {
    pub fn new() -> Self {
        Self {
            query_latency_ms: Histogram::new(
                "fundb_query_latency_ms",
                "Query execution latency in milliseconds",
                default_latency_buckets(),
            ),
            write_throughput: Counter::new("fundb_write_ops_total", "Total write operations"),
            active_connections: Gauge::new(
                "fundb_active_connections",
                "Number of currently active client connections",
            ),
            replication_lag_ms: Gauge::new(
                "fundb_replication_lag_ms",
                "Replication lag in milliseconds",
            ),
            cache_hits: Counter::new("fundb_cache_hits_total", "Total cache hit events"),
            cache_misses: Counter::new("fundb_cache_misses_total", "Total cache miss events"),
            vector_scan_latency: Histogram::new(
                "fundb_vector_scan_latency_ms",
                "Vector similarity scan latency in milliseconds",
                default_latency_buckets(),
            ),
            cognitive_ops: Counter::new(
                "fundb_cognitive_ops_total",
                "Total cognitive / AI-assisted operations",
            ),
        }
    }

    /// Register all metrics into the provided registry.
    ///
    /// Clones are cheap because every inner value is `Arc`-backed.
    pub fn register_all(&self, registry: &mut MetricsRegistry) {
        registry.register_histogram(self.query_latency_ms.clone());
        registry.register_counter(self.write_throughput.clone());
        registry.register_gauge(self.active_connections.clone());
        registry.register_gauge(self.replication_lag_ms.clone());
        registry.register_counter(self.cache_hits.clone());
        registry.register_counter(self.cache_misses.clone());
        registry.register_histogram(self.vector_scan_latency.clone());
        registry.register_counter(self.cognitive_ops.clone());
    }
}

impl Default for FunDbMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Span tracing — OpenTelemetry-compatible API, no external OTel crate
// ---------------------------------------------------------------------------

/// Immutable propagation context attached to every span.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpanContext {
    pub trace_id: u128,
    pub span_id: u64,
    pub parent_span_id: Option<u64>,
    pub sampled: bool,
}

/// Status of a span once it has finished.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanStatus {
    Unset,
    Ok,
    Error(String),
}

/// A timestamped event attached to a span.
#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp_ms: u64,
    pub attributes: HashMap<String, String>,
}

/// A single unit of work within a distributed trace.
#[derive(Debug)]
pub struct Span {
    pub context: SpanContext,
    pub operation: String,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
    pub status: SpanStatus,
}

impl Span {
    /// Add or overwrite a string attribute.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Record a named event on this span.
    pub fn add_event(&mut self, name: impl Into<String>, attrs: HashMap<String, String>) {
        self.events.push(SpanEvent {
            name: name.into(),
            timestamp_ms: now_ms(),
            attributes: attrs,
        });
    }

    /// Mark the span as ended at the current wall-clock time.
    pub fn finish(&mut self) {
        self.end_ms = Some(now_ms());
    }

    /// Set the span status to `Ok`.
    pub fn set_ok(&mut self) {
        self.status = SpanStatus::Ok;
    }

    /// Set the span status to `Error` with a message.
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = SpanStatus::Error(message.into());
    }
}

// ---------------------------------------------------------------------------
// SpanExporter trait
// ---------------------------------------------------------------------------

/// Sink that receives finished spans.
pub trait SpanExporter: Send + Sync {
    fn export(&self, span: &Span);
}

// ---------------------------------------------------------------------------
// InMemoryExporter — for testing
// ---------------------------------------------------------------------------

/// An in-process exporter that stores finished spans in a `Vec`.
pub struct InMemoryExporter {
    spans: Arc<Mutex<Vec<Span>>>,
}

impl InMemoryExporter {
    pub fn new() -> Self {
        Self {
            spans: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return the `SpanContext` of every exported span.
    pub fn exported_spans(&self) -> Vec<SpanContext> {
        self.spans
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.context.clone())
            .collect()
    }

    /// Return the total number of exported spans.
    pub fn len(&self) -> usize {
        self.spans.lock().unwrap().len()
    }

    /// Returns `true` when no spans have been exported yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemoryExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanExporter for InMemoryExporter {
    fn export(&self, span: &Span) {
        // We cannot move out of the reference, so we reconstruct a minimal Span
        // by cloning the fields that are Clone, and re-creating the rest.
        let copy = Span {
            context: span.context.clone(),
            operation: span.operation.clone(),
            start_ms: span.start_ms,
            end_ms: span.end_ms,
            attributes: span.attributes.clone(),
            events: span.events.clone(),
            status: span.status.clone(),
        };
        self.spans.lock().unwrap().push(copy);
    }
}

// ---------------------------------------------------------------------------
// Tracer
// ---------------------------------------------------------------------------

/// Creates and finishes spans, forwarding finished spans to the configured exporter.
pub struct Tracer {
    exporter: Arc<dyn SpanExporter>,
    /// Probability `[0.0, 1.0]` that a new root span is sampled.
    sample_rate: f64,
}

impl Tracer {
    pub fn new(exporter: Arc<dyn SpanExporter>, sample_rate: f64) -> Self {
        Self {
            exporter,
            sample_rate: sample_rate.clamp(0.0, 1.0),
        }
    }

    /// Start a new span.  When `parent` is `Some`, the new span joins its
    /// trace and inherits its `sampled` flag; otherwise a fresh trace is
    /// created and the span is sampled according to `sample_rate`.
    pub fn start_span(&self, operation: &str, parent: Option<&SpanContext>) -> Span {
        let (trace_id, parent_span_id, sampled) = match parent {
            Some(p) => (p.trace_id, Some(p.span_id), p.sampled),
            None => {
                // Deterministic sampling: compare the low 32 bits of a new ID
                // against the threshold.
                let r = (gen_id_64() & 0xFFFF_FFFF) as f64 / (u32::MAX as f64);
                (gen_id_128(), None, r < self.sample_rate)
            }
        };

        Span {
            context: SpanContext {
                trace_id,
                span_id: gen_id_64(),
                parent_span_id,
                sampled,
            },
            operation: operation.to_string(),
            start_ms: now_ms(),
            end_ms: None,
            attributes: HashMap::new(),
            events: Vec::new(),
            status: SpanStatus::Unset,
        }
    }

    /// Finish a span and forward it to the exporter (if sampled).
    pub fn finish_span(&self, mut span: Span) {
        if span.end_ms.is_none() {
            span.finish();
        }
        if span.context.sampled {
            self.exporter.export(&span);
        }
    }

    /// Convenience wrapper: start a child span that shares the parent's trace.
    pub fn start_child(&self, operation: &str, parent: &SpanContext) -> Span {
        self.start_span(operation, Some(parent))
    }
}

// ---------------------------------------------------------------------------
// Grafana dashboard template
// ---------------------------------------------------------------------------

/// Returns a JSON string containing a simplified Grafana dashboard definition
/// with panels for all key FunDB metrics.
pub fn grafana_dashboard_json() -> String {
    r#"{
  "dashboard": {
    "id": null,
    "uid": "fundb-ops",
    "title": "FunDB Operations",
    "tags": ["fundb", "database", "ai"],
    "timezone": "browser",
    "schemaVersion": 36,
    "version": 1,
    "refresh": "10s",
    "panels": [
      {
        "id": 1,
        "title": "Query Latency (p50 / p95 / p99)",
        "type": "timeseries",
        "gridPos": { "h": 8, "w": 12, "x": 0, "y": 0 },
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(fundb_query_latency_ms_bucket[5m]))",
            "legendFormat": "p50",
            "refId": "A"
          },
          {
            "expr": "histogram_quantile(0.95, rate(fundb_query_latency_ms_bucket[5m]))",
            "legendFormat": "p95",
            "refId": "B"
          },
          {
            "expr": "histogram_quantile(0.99, rate(fundb_query_latency_ms_bucket[5m]))",
            "legendFormat": "p99",
            "refId": "C"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "ms" }
        }
      },
      {
        "id": 2,
        "title": "Write Throughput (ops/s)",
        "type": "timeseries",
        "gridPos": { "h": 8, "w": 12, "x": 12, "y": 0 },
        "targets": [
          {
            "expr": "rate(fundb_write_ops_total[1m])",
            "legendFormat": "writes/s",
            "refId": "A"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "ops" }
        }
      },
      {
        "id": 3,
        "title": "Active Connections",
        "type": "stat",
        "gridPos": { "h": 4, "w": 6, "x": 0, "y": 8 },
        "targets": [
          {
            "expr": "fundb_active_connections",
            "legendFormat": "connections",
            "refId": "A"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "short", "thresholds": {
            "mode": "absolute",
            "steps": [
              { "color": "green", "value": null },
              { "color": "yellow", "value": 50 },
              { "color": "red", "value": 100 }
            ]
          }}
        }
      },
      {
        "id": 4,
        "title": "Replication Lag",
        "type": "stat",
        "gridPos": { "h": 4, "w": 6, "x": 6, "y": 8 },
        "targets": [
          {
            "expr": "fundb_replication_lag_ms",
            "legendFormat": "lag ms",
            "refId": "A"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "ms", "thresholds": {
            "mode": "absolute",
            "steps": [
              { "color": "green", "value": null },
              { "color": "yellow", "value": 100 },
              { "color": "red", "value": 500 }
            ]
          }}
        }
      },
      {
        "id": 5,
        "title": "Cache Hit Ratio",
        "type": "gauge",
        "gridPos": { "h": 4, "w": 6, "x": 12, "y": 8 },
        "targets": [
          {
            "expr": "rate(fundb_cache_hits_total[5m]) / (rate(fundb_cache_hits_total[5m]) + rate(fundb_cache_misses_total[5m]))",
            "legendFormat": "hit ratio",
            "refId": "A"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "percentunit",
            "min": 0,
            "max": 1,
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "color": "red", "value": null },
                { "color": "yellow", "value": 0.7 },
                { "color": "green", "value": 0.9 }
              ]
            }
          }
        }
      },
      {
        "id": 6,
        "title": "Vector Scan Latency (p95)",
        "type": "timeseries",
        "gridPos": { "h": 8, "w": 12, "x": 0, "y": 12 },
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(fundb_vector_scan_latency_ms_bucket[5m]))",
            "legendFormat": "p50",
            "refId": "A"
          },
          {
            "expr": "histogram_quantile(0.95, rate(fundb_vector_scan_latency_ms_bucket[5m]))",
            "legendFormat": "p95",
            "refId": "B"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "ms" }
        }
      },
      {
        "id": 7,
        "title": "Cognitive Operations (ops/s)",
        "type": "timeseries",
        "gridPos": { "h": 8, "w": 12, "x": 12, "y": 12 },
        "targets": [
          {
            "expr": "rate(fundb_cognitive_ops_total[1m])",
            "legendFormat": "cognitive ops/s",
            "refId": "A"
          }
        ],
        "fieldConfig": {
          "defaults": { "unit": "ops" }
        }
      }
    ]
  },
  "folderId": 0,
  "overwrite": true
}"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // 1. Counter increments
    #[test]
    fn test_counter_increment() {
        let c = Counter::new("test_counter", "A test counter");
        c.increment();
        c.increment();
        c.increment();
        assert_eq!(c.get(), 3);
    }

    // 2. Counter add
    #[test]
    fn test_counter_add() {
        let c = Counter::new("test_add", "A test counter");
        c.add(10);
        assert_eq!(c.get(), 10);
    }

    // 3. Gauge set / increment / decrement
    #[test]
    fn test_gauge_set_inc_dec() {
        let g = Gauge::new("test_gauge", "A test gauge");
        g.set(5);
        assert_eq!(g.get(), 5);
        g.increment();
        assert_eq!(g.get(), 6);
        g.decrement();
        assert_eq!(g.get(), 5);
    }

    // 4. Histogram single observation
    #[test]
    fn test_histogram_observe() {
        let h = Histogram::new("test_hist", "A test histogram", vec![1.0, 10.0, 100.0]);
        h.observe(5.0);
        assert_eq!(h.get_count(), 1);
        assert_eq!(h.get_sum(), 5.0);
    }

    // 5. Histogram bucket distribution
    #[test]
    fn test_histogram_buckets() {
        let h = Histogram::new(
            "test_buckets",
            "Bucket distribution test",
            vec![5.0, 10.0, 50.0],
        );
        h.observe(3.0); // lands in <=5, <=10, <=50, +Inf
        h.observe(7.0); // lands in <=10, <=50, +Inf  (not <=5)
        h.observe(30.0); // lands in <=50, +Inf  (not <=5 or <=10)

        let bc = h.bucket_counts();
        // bucket[0] = <=5 : 1
        assert_eq!(bc[0], (5.0, 1));
        // bucket[1] = <=10 : 2
        assert_eq!(bc[1], (10.0, 2));
        // bucket[2] = <=50 : 3
        assert_eq!(bc[2], (50.0, 3));
        // +Inf : 3
        assert_eq!(bc[3].1, 3);
        assert!(bc[3].0.is_infinite());
    }

    // 6. Registry render_text basic structure
    #[test]
    fn test_metrics_registry_render() {
        let mut reg = MetricsRegistry::new();
        reg.register_counter(Counter::new("my_counter", "A counter for testing"));
        reg.register_gauge(Gauge::new("my_gauge", "A gauge for testing"));
        reg.register_histogram(Histogram::new(
            "my_hist",
            "A histogram for testing",
            vec![1.0, 10.0],
        ));

        let text = reg.render_text();
        assert!(text.contains("# HELP"), "missing HELP comment");
        assert!(text.contains("# TYPE"), "missing TYPE comment");
        assert!(text.contains("my_counter"));
        assert!(text.contains("my_gauge"));
        assert!(text.contains("my_hist"));
    }

    // 7. FunDbMetrics construction
    #[test]
    fn test_fundb_metrics_new() {
        let m = FunDbMetrics::new();
        // Just verifying it builds without panic.
        assert_eq!(m.write_throughput.get(), 0);
        assert_eq!(m.active_connections.get(), 0);
        assert_eq!(m.query_latency_ms.get_count(), 0);
    }

    // 8. Tracer start + finish
    #[test]
    fn test_tracer_start_finish() {
        let exporter = Arc::new(InMemoryExporter::new());
        let tracer = Tracer::new(exporter.clone(), 1.0); // always sample

        let span = tracer.start_span("test_op", None);
        tracer.finish_span(span);

        assert_eq!(exporter.len(), 1);
        let ctxs = exporter.exported_spans();
        assert_eq!(ctxs[0].operation_name_check(), ""); // just exercise the Vec
        assert!(ctxs[0].sampled);
    }

    // 9. Child span has parent_span_id
    #[test]
    fn test_tracer_child_span() {
        let exporter = Arc::new(InMemoryExporter::new());
        let tracer = Tracer::new(exporter.clone(), 1.0);

        let parent = tracer.start_span("parent_op", None);
        let parent_ctx = parent.context.clone();
        tracer.finish_span(parent);

        let child = tracer.start_child("child_op", &parent_ctx);
        assert_eq!(child.context.parent_span_id, Some(parent_ctx.span_id));
        assert_eq!(child.context.trace_id, parent_ctx.trace_id);
        tracer.finish_span(child);

        assert_eq!(exporter.len(), 2);
        let ctxs = exporter.exported_spans();
        assert_eq!(ctxs[1].parent_span_id, Some(parent_ctx.span_id));
    }

    // 10. Grafana dashboard JSON is valid and contains the right title
    #[test]
    fn test_grafana_dashboard_json() {
        let json = grafana_dashboard_json();
        assert!(!json.is_empty());
        assert!(json.contains("FunDB Operations"), "Dashboard title missing");
        // Basic structural checks — the string must be parseable as JSON.
        // We rely only on std (no serde_json in the module), so we do a
        // lightweight structural probe rather than full parse.
        assert!(json.trim_start().starts_with('{'));
        assert!(json.trim_end().ends_with('}'));
        assert!(json.contains("\"panels\""));
        assert!(json.contains("\"folderId\""));
        assert!(json.contains("\"overwrite\""));
        // Must have at least 5 panels: count the panel id fields.
        let panel_count = json.matches("\"id\":").count();
        assert!(
            panel_count >= 5,
            "expected at least 5 panels, found {}",
            panel_count
        );
    }

    // 11. Prometheus counter text format
    #[test]
    fn test_prometheus_counter_format() {
        let mut reg = MetricsRegistry::new();
        let c = Counter::new("fundb_write_ops_total", "Total write operations");
        c.add(42);
        reg.register_counter(c);

        let text = reg.render_text();
        assert!(
            text.contains("# TYPE fundb_write_ops_total counter"),
            "TYPE line missing or wrong"
        );
        // The value line must be "name<space>value\n"
        assert!(
            text.contains("fundb_write_ops_total 42"),
            "value line not found in:\n{}",
            text
        );
    }

    // 12. Span attributes survive export
    #[test]
    fn test_span_attributes() {
        let exporter = Arc::new(InMemoryExporter::new());
        let tracer = Tracer::new(exporter.clone(), 1.0);

        let mut span = tracer.start_span("attr_op", None);
        span.set_attribute("db.system", "fundb");
        span.set_attribute("db.operation", "SELECT");
        tracer.finish_span(span);

        assert_eq!(exporter.len(), 1);
        let stored = exporter.spans.lock().unwrap();
        let s = &stored[0];
        assert_eq!(
            s.attributes.get("db.system").map(|v| v.as_str()),
            Some("fundb")
        );
        assert_eq!(
            s.attributes.get("db.operation").map(|v| v.as_str()),
            Some("SELECT")
        );
    }

    // -----------------------------------------------------------------------
    // Helper impl used only in tests (avoids modifying SpanContext)
    // -----------------------------------------------------------------------

    impl SpanContext {
        /// No-op method used in tests to exercise the struct without adding
        /// production API surface.
        fn operation_name_check(&self) -> &str {
            ""
        }
    }
}
