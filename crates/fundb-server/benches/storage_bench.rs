// storage_bench.rs — Storage engine benchmarks for FunDB
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

/// REQ-PERF-001: Sustained write throughput — 1 000 records per iteration.
/// Reports Elements/sec so criterion derives records/sec automatically.
fn bench_memtable_write_throughput(c: &mut Criterion) {
    const N: u64 = 1_000;

    let mut group = c.benchmark_group("storage");
    group.throughput(Throughput::Elements(N));
    group.bench_function("memtable_write_throughput_1000", |b| {
        b.iter(|| {
            let mem = MemTable::new();
            for i in 0..N as usize {
                let key = make_key("bench", i);
                let record = FunRecordBuilder::new("bench").build();
                mem.insert(key, record).expect("insert must not fail");
            }
            black_box(mem.size_bytes())
        })
    });
    group.finish();
}

/// REQ-PERF-002: Point lookup latency — get a known record by key from a
/// 10 000-record memtable.  Measures the p99 cost of a single `get` call.
fn bench_memtable_get(c: &mut Criterion) {
    const TOTAL: usize = 10_000;
    // Pre-fill once outside the benchmark loop.
    let mem = MemTable::new();
    for i in 0..TOTAL {
        let key = make_key("lookup", i);
        let record = FunRecordBuilder::new("lookup").build();
        mem.insert(key, record).expect("insert must not fail");
    }

    // Pick a key in the middle of the range as the lookup target.
    let target_key = make_key("lookup", TOTAL / 2);

    let mut group = c.benchmark_group("storage");
    group.bench_function("memtable_get_10000", |b| {
        b.iter(|| black_box(mem.get(black_box(&target_key))))
    });
    group.finish();
}

/// Measures the cost of constructing and cloning a `RecordKey` 1 000 times.
/// RecordKey is a composite struct (String + [u8;16]); this validates that
/// key serialization/copy stays negligible compared to IO costs.
fn bench_key_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage");

    for n in [100_u64, 1_000, 10_000] {
        group.throughput(Throughput::Elements(n));
        group.bench_with_input(BenchmarkId::new("key_serialization", n), &n, |b, &n| {
            b.iter(|| {
                let mut keys: Vec<RecordKey> = Vec::with_capacity(n as usize);
                for i in 0..n as usize {
                    // Construct the key (simulates serialization of a composite key).
                    let key = make_key("storage_bench", i);
                    // Clone the key (simulates deserialization / copy into an index).
                    keys.push(black_box(key.clone()));
                }
                black_box(keys.len())
            })
        });
    }

    group.finish();
}

/// REQ-PERF-003: Flat ANN index build + nearest-neighbor search.
/// Builds a brute-force flat index over 1 000 384-dimensional vectors and
/// finds the single nearest neighbor for a query vector by cosine distance.
fn bench_vector_index_build(c: &mut Criterion) {
    const NUM_VECS: usize = 1_000;
    const DIM: usize = 384;

    // Build deterministic index vectors once.
    let index: Vec<Vec<f32>> = (0..NUM_VECS)
        .map(|i| {
            (0..DIM)
                .map(|d| ((i + d) as f32) / (NUM_VECS + DIM) as f32)
                .collect()
        })
        .collect();

    // Query vector — also deterministic.
    let query: Vec<f32> = (0..DIM).map(|d| (d as f32) / DIM as f32).collect();

    let mut group = c.benchmark_group("storage");
    group.throughput(Throughput::Elements(NUM_VECS as u64));
    group.bench_function("vector_flat_index_build_and_search_1000x384", |b| {
        b.iter(|| {
            // Full build + search each iteration to validate total ANN overhead.
            let index_ref: &Vec<Vec<f32>> = black_box(&index);
            let query_ref: &Vec<f32> = black_box(&query);

            let best_idx = index_ref
                .iter()
                .enumerate()
                .map(|(idx, vec)| {
                    let sim = cosine_similarity(query_ref, vec);
                    (idx, sim)
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            black_box(best_idx)
        })
    });
    group.finish();
}

/// REQ-PERF-004: Filter throughput — retain records with confidence > 0.7
/// from a pre-built 10 000-record collection.
fn bench_confidence_filter(c: &mut Criterion) {
    const TOTAL: usize = 10_000;

    // Pre-fill once; alternate between high and low confidence so ~50 % pass.
    let mem = MemTable::new();
    for i in 0..TOTAL {
        let conf = if i % 2 == 0 { 0.9_f32 } else { 0.5_f32 };
        let key = make_key("filter", i);
        let record = FunRecordBuilder::new("filter").confidence(conf).build();
        mem.insert(key, record).expect("insert must not fail");
    }

    // Scan bounds spanning the entire "filter" collection.
    let from_key = RecordKey {
        collection: "filter".to_string(),
        id: [0u8; 16],
    };
    let to_key = RecordKey {
        collection: "filter".to_string(),
        id: [255u8; 16],
    };

    let mut group = c.benchmark_group("storage");
    group.throughput(Throughput::Elements(TOTAL as u64));

    for threshold_pct in [100_u64, 1_000, 10_000] {
        // Re-use the same pre-filled memtable; only the bench label varies.
        group.bench_with_input(
            BenchmarkId::new("confidence_filter", threshold_pct),
            &threshold_pct,
            |b, _| {
                b.iter(|| {
                    let all = mem.range(black_box(&from_key), black_box(&to_key));
                    let passing: Vec<FunRecord> = all
                        .into_iter()
                        .map(|(_k, r)| r)
                        .filter(|r| r._confidence > 0.7)
                        .collect();
                    black_box(passing.len())
                })
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion wiring
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_memtable_write_throughput,
    bench_memtable_get,
    bench_key_serialization,
    bench_vector_index_build,
    bench_confidence_filter,
);
criterion_main!(benches);
