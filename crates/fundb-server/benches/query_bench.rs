// query_bench.rs — Query execution benchmarks for FunDB
//
// Performance targets being validated (ARCHITECTURE.md §19):
// REQ-PERF-001: write throughput >= 100K records/sec
// REQ-PERF-002: point lookup p99 <= 1ms
// REQ-PERF-003: ANN top-10 p99 <= 10ms (384-dim, 1M vectors)
// REQ-PERF-004: scan 1M records/sec
// REQ-PERF-005: cognitive confidence propagation <= 0.1ms per chain link

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fundb_core::{FunRecord, FunRecordBuilder, RecordKey};
use fundb_storage::MemTable;

// ---------------------------------------------------------------------------
// Inline helpers
// ---------------------------------------------------------------------------

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Propagate confidence along a chain of f32 values using multiplicative decay.
/// Each link multiplies the running confidence by the link's own confidence.
fn propagate_confidence_chain(chain: &[f32]) -> f32 {
    chain.iter().fold(1.0_f32, |acc, &c| acc * c)
}

/// Propagate confidence along a chain using Dempster-Shafer-style combination:
/// combined = a * b / (a * b + (1-a) * (1-b))
fn propagate_confidence_ds(chain: &[f32]) -> f32 {
    chain.iter().skip(1).fold(chain[0], |acc, &c| {
        let num = acc * c;
        let denom = num + (1.0 - acc) * (1.0 - c);
        if denom == 0.0 { 0.0 } else { num / denom }
    })
}

// ---------------------------------------------------------------------------
// Record key construction helper
// ---------------------------------------------------------------------------

fn make_key(collection: &str, idx: usize) -> RecordKey {
    let mut id = [0u8; 16];
    let bytes = idx.to_le_bytes();
    id[..8].copy_from_slice(&bytes);
    RecordKey {
        collection: collection.to_string(),
        id,
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// REQ-PERF-001 (partial): Measures the overhead of constructing a single FunRecord.
fn bench_record_build(c: &mut Criterion) {
    c.bench_function("record_build", |b| {
        b.iter(|| {
            black_box(FunRecordBuilder::new("events").confidence(0.9).build())
        })
    });
}

/// REQ-PERF-001: Write throughput — insert N records into a MemTable.
/// Throughput is reported as Elements/sec so criterion can compute records/sec.
fn bench_memtable_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("memtable_insert");

    for n in [100_u64, 1_000, 10_000] {
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::new("n", n), &n, |b, &n| {
            b.iter(|| {
                let mem = MemTable::new();
                for i in 0..n as usize {
                    let key = make_key("events", i);
                    let record = FunRecordBuilder::new("events").build();
                    mem.insert(key, record).expect("insert must not fail");
                }
                black_box(mem.size_bytes())
            })
        });
    }

    group.finish();
}

/// REQ-PERF-004: Scan throughput — scan over records in the "events" collection
/// using a full-range query (min..=max key for that collection).
fn bench_memtable_scan(c: &mut Criterion) {
    // Pre-fill the memtable with 10 000 records in the "events" collection.
    let mem = MemTable::new();
    let n: usize = 10_000;
    for i in 0..n {
        let key = make_key("events", i);
        let record = FunRecordBuilder::new("events").build();
        mem.insert(key, record).expect("insert must not fail");
    }

    // Build the range bounds that span the entire "events" collection.
    let from_key = RecordKey { collection: "events".to_string(), id: [0u8; 16] };
    let to_key   = RecordKey { collection: "events".to_string(), id: [255u8; 16] };

    let mut group = c.benchmark_group("query");
    group.throughput(Throughput::Elements(n as u64));
    group.bench_function("memtable_scan_10000", |b| {
        b.iter(|| {
            let results = mem.range(black_box(&from_key), black_box(&to_key));
            black_box(results.len())
        })
    });
    group.finish();
}

/// REQ-PERF-003: Compute cosine similarity between two 384-dimensional vectors.
fn bench_vector_cosine_similarity(c: &mut Criterion) {
    // Build two deterministic 384-dim vectors.
    let vec_a: Vec<f32> = (0..384).map(|i| (i as f32) / 384.0).collect();
    let vec_b: Vec<f32> = (0..384).map(|i| 1.0 - (i as f32) / 384.0).collect();

    let mut group = c.benchmark_group("query");
    group.bench_function("cosine_similarity_384d", |b| {
        b.iter(|| {
            black_box(cosine_similarity(
                black_box(&vec_a),
                black_box(&vec_b),
            ))
        })
    });
    group.finish();
}

/// REQ-PERF-005: Cognitive confidence propagation along a 100-element chain.
/// Two propagation strategies are benchmarked: multiplicative and Dempster-Shafer.
fn bench_confidence_propagation(c: &mut Criterion) {
    // Build a 100-element chain with confidence values in [0.7, 1.0].
    let chain: Vec<f32> = (0..100)
        .map(|i| 0.7_f32 + 0.003 * (i as f32))
        .collect();

    let mut group = c.benchmark_group("query");

    group.bench_function("confidence_propagation_multiplicative_100", |b| {
        b.iter(|| black_box(propagate_confidence_chain(black_box(&chain))))
    });

    group.bench_function("confidence_propagation_dempster_shafer_100", |b| {
        b.iter(|| black_box(propagate_confidence_ds(black_box(&chain))))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion wiring
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_record_build,
    bench_memtable_insert,
    bench_memtable_scan,
    bench_vector_cosine_similarity,
    bench_confidence_propagation,
);
criterion_main!(benches);
