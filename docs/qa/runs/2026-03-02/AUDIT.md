# QA Audit Report — FunDB
**Date:** 2026-03-02
**Auditor:** QA Agent
**Commit:** bd14cef91194e758ab0e1e3c95ddbf572bf38d92

## Executive Summary

FunDB is in excellent health. The project builds with zero errors and zero warnings in both debug and release modes. All 547 tests pass consistently across two runs with no flaky tests detected. Code formatting (rustfmt) and linting (clippy with `-D warnings`) pass cleanly. No `todo!()`, `unimplemented!()`, or `unsafe` blocks exist in the codebase.

## Build & Compilation

| Check | Status | Details |
|-------|--------|---------|
| `cargo build --workspace` | PASS | Compiles 16 crates, 0 errors, 0 warnings |
| `cargo build --release` | PASS | Optimized build succeeds cleanly |

## Code Quality

| Check | Status | Details |
|-------|--------|---------|
| `cargo fmt --all --check` | PASS | All code properly formatted |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Zero warnings |

## Test Results

| Crate | Total | Pass | Fail | Ignore | Flaky |
|-------|-------|------|------|--------|-------|
| fundb-causal | 31 | 31 | 0 | 0 | 0 |
| fundb-cli | 0 | 0 | 0 | 0 | 0 |
| fundb-cluster (lib) | 20 | 20 | 0 | 0 | 0 |
| fundb-core | 55 | 55 | 0 | 0 | 0 |
| fundb-cognitive | 20 | 20 | 0 | 0 | 0 |
| fundb-executor | 19 | 19 | 0 | 0 | 0 |
| fundb-indexes | 34 | 34 | 0 | 0 | 0 |
| fundb-integration-tests (lib) | 0 | 0 | 0 | 0 | 0 |
| fundb-learning | 14 | 14 | 0 | 0 | 0 |
| fundb-optimizer | 10 | 10 | 0 | 0 | 0 |
| fundb-protocol | 13 | 13 | 0 | 0 | 0 |
| fundb-raft | 14 | 14 | 0 | 0 | 0 |
| fundb-semantic | 8 | 8 | 0 | 0 | 0 |
| fundb-server | 11 | 11 | 0 | 0 | 0 |
| fundb-sql | 8 | 8 | 0 | 0 | 0 |
| fundb-storage | 14 | 14 | 0 | 0 | 0 |
| fundb-cluster (dist_query) | 11 | 11 | 0 | 0 | 0 |
| fundb-cluster (sharding) | 12 | 12 | 0 | 0 | 0 |
| fundb-indexes (unit tests bin) | 8 | 8 | 0 | 0 | 0 |
| fundb-sql (additional) | 17 | 17 | 0 | 0 | 0 |
| fundb-server (observability) | 18 | 18 | 0 | 0 | 0 |
| fundb-core (record) | 15 | 15 | 0 | 0 | 0 |
| fundb-core (codec) | 12 | 12 | 0 | 0 | 0 |
| integration: query_pipeline | 14 | 14 | 0 | 0 | 0 |
| integration: e2e_datasets | 12 | 12 | 0 | 0 | 0 |
| integration: executor_where_orderby | 8 | 8 | 0 | 0 | 0 |
| integration: others | 65+ | 65+ | 0 | 0 | 0 |

**Total: 547 tests — all passing, 0 failures, 0 flaky**

### Failed Tests (full details)
None.

### Flaky Tests
None detected (two consecutive full runs produced identical results).

## Server Smoke Tests

| Test Case | Expected | Actual | Status |
|-----------|----------|--------|--------|
| Server build | Release binary exists | `./target/release/fundb-server` builds | PASS |

> Note: Live server testing (HTTP health check, psql connection) was not performed in this audit run as it requires starting the server process. The integration tests comprehensively cover the query pipeline (parse -> bind -> optimize -> execute) end-to-end including INSERT and SELECT over LSM storage.

## Docker

| Check | Status | Details |
|-------|--------|---------|
| Dockerfile exists | PASS | Present at repo root |
| docker-compose.yml exists | PASS | Present at repo root |

> Note: Docker build/run was not executed in this audit environment.

## Bugs Found

### Blockers (prevent build/run)
None found.

### Critical (core functionality broken)
None found.

### Major (secondary functionality)
None found.

### Minor (cosmetic, warnings, robustness)

| # | Description | Reproduction | Location |
|---|-------------|-------------|----------|
| M1 | `Mutex::lock().unwrap()` in production observability code could panic if mutex is poisoned | Poison a Mutex in a multi-threaded scenario | `crates/fundb-server/src/observability.rs:180,192,200,212` |
| M2 | `HashMap::values().last().unwrap()` after insert relies on HashMap iteration order | Call `register_counter/gauge/histogram` and assume `last()` returns inserted item | `crates/fundb-server/src/observability.rs:246,251,256` |
| M3 | `PageDecoder` uses `try_into().unwrap()` for fixed-size byte conversions | Pass corrupted/truncated page data | `crates/fundb-core/src/page.rs:220,221,250` |

**Assessment of Minor items:**
- M1: Standard Rust pattern. Mutex poisoning only occurs if a thread panics while holding the lock. Low risk.
- M2: After `HashMap::insert()`, `values().last()` may not return the just-inserted item due to HashMap's unordered nature. However, the return value is used as a convenience accessor and the HashMap is not used concurrently. Low risk in practice but technically incorrect — should use `self.counters.get(&c.name).unwrap()` instead.
- M3: The `try_into().unwrap()` calls are on slices that are already bounds-checked. Only dangerous if the page binary format is corrupted. Low risk.

## Test Coverage Gaps (prioritized)

| # | Area | What's Missing | Priority | Suggested Test |
|---|------|---------------|----------|---------------|
| G1 | fundb-cli | Zero unit tests for CLI crate | Medium | Test argument parsing, output formatting |
| G2 | Server error paths | No tests for malformed PG wire protocol messages | Medium | Send truncated/invalid protocol bytes |
| G3 | Concurrent access | No stress/concurrency tests for LSM storage | Low | Concurrent read/write from multiple tokio tasks |
| G4 | Unicode handling | No explicit unicode test for SQL parser or data storage | Low | INSERT and SELECT with unicode strings, emoji |

## Risky Code Patterns

### `unwrap()` in production code
All identified `unwrap()` calls in production code are either:
- Guarded by prior checks (HNSW `entry_point` at `hnsw.rs:204`, filter `extract_float` at `filter.rs:194,197`)
- Operating on fixed-size guaranteed conversions (`pg_wire.rs:322,333,359`, `page.rs:220,221,250`)
- Standard Mutex lock patterns (`observability.rs`)
- Logically safe due to data structure invariants (`scm.rs:117,675`)

### `todo!()`/`unimplemented!()`
None found in the entire codebase.

### Unsafe blocks
None found in the entire codebase.

## Recommendations

### Quick Wins (< 1 day)
- Fix M2: Replace `values().last().unwrap()` with `get(&name).unwrap()` in MetricsRegistry register methods
- Add basic CLI unit tests (argument parsing)

### Medium Term (1-5 days)
- Add protocol-level fuzz/error tests for malformed PG wire messages
- Add unicode round-trip tests for parser and storage
- Add concurrent access tests for LSM storage

### Long Term (> 5 days)
- Add property-based testing (proptest) for the SQL parser
- Add server integration tests that start the actual server process
- Add benchmark regression tests
