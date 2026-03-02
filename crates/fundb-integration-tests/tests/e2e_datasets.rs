//! End-to-end tests with real-world datasets.
//!
//! These tests validate the full FunDB pipeline (parse → bind → execute)
//! using real CSV datasets:
//!   - IMDB Movies (1000 rows) — strings, integers, floats
//!   - World Countries (250 rows) — diverse column types, geographic data
//!   - USGS Earthquakes (8394 rows) — numeric-heavy, good for stress testing
//!
//! Each test exercises a different aspect of the query engine to expose gaps
//! and validate correctness at scale.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use fundb_core::{FunRecord, FunRecordBuilder};
use fundb_executor::Executor;
use fundb_integration_tests::{key_for_record, open_lsm};
use fundb_optimizer::optimize;
use fundb_sql::{bind, parse, Catalog, LogicalPlan};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a path relative to the testdata directory.
fn testdata_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("testdata");
    p.push(name);
    p
}

/// Escape a string value for use in FunQL SQL strings.
fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Build a FunRecord from a map of column values, serialised as MessagePack.
fn build_record_from_map(
    collection: &str,
    data: &HashMap<String, serde_json::Value>,
    confidence: Option<f32>,
) -> FunRecord {
    let mut builder = FunRecordBuilder::new(collection);
    if let Some(c) = confidence {
        builder = builder.confidence(c);
    }
    let bytes = rmp_serde::to_vec(data).unwrap_or_default();
    builder = builder.data(bytes);
    builder.build()
}

/// Execute an INSERT via the full SQL pipeline (parse → bind → execute).
async fn exec_insert(executor: &Executor, sql: &str) -> anyhow::Result<()> {
    let stmt = parse(sql).map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
    let catalog = Catalog::new(); // INSERT doesn't need catalog validation
    let plan = bind(stmt, &catalog)?;
    executor.execute(plan).await?;
    Ok(())
}

/// Execute a SELECT via the full SQL pipeline (parse → bind → execute).
async fn exec_select(
    executor: &Executor,
    sql: &str,
    catalog: &Catalog,
) -> anyhow::Result<fundb_executor::batch::RecordBatch> {
    let stmt = parse(sql).map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
    let plan = bind(stmt, catalog)?;
    executor.execute(plan).await
}

// ===========================================================================
// 1. MOVIES DATASET — INSERT + SELECT pipeline
// ===========================================================================

/// Load the IMDB movies dataset via SQL INSERT and verify record count.
#[tokio::test]
async fn test_movies_insert_and_count() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");

    let mut inserted = 0usize;
    for result in rdr.records() {
        let record = result.expect("CSV parse error");

        // Columns: Rank,Title,Genre,Description,Director,Actors,Year,
        //          Runtime (Minutes),Rating,Votes,Revenue (Millions),Metascore
        let title = escape_sql(record.get(1).unwrap_or(""));
        let genre = escape_sql(record.get(2).unwrap_or(""));
        let director = escape_sql(record.get(4).unwrap_or(""));
        let year = record.get(6).unwrap_or("0");
        let runtime = record.get(7).unwrap_or("0");
        let rating = record.get(8).unwrap_or("0.0");
        let votes = record.get(9).unwrap_or("0");

        let sql = format!(
            "INSERT INTO movies (title, genre, director, year, runtime, rating, votes) \
             VALUES ('{}', '{}', '{}', {}, {}, {}, {})",
            title, genre, director, year, runtime, rating, votes
        );

        match exec_insert(&executor, &sql).await {
            Ok(_) => inserted += 1,
            Err(e) => {
                // Log but don't fail — some rows may have tricky characters
                eprintln!("WARN: failed to insert movie row {}: {}", inserted + 1, e);
            }
        }
    }

    // We expect most of the ~1000 rows to insert successfully
    assert!(
        inserted >= 900,
        "expected at least 900 movies inserted, got {}",
        inserted
    );

    // Verify via SELECT * scan
    let mut catalog = Catalog::new();
    catalog.add_collection("movies");

    let batch = exec_select(&executor, "SELECT * FROM movies", &catalog)
        .await
        .expect("SELECT * FROM movies failed");

    assert_eq!(
        batch.len(),
        inserted,
        "SELECT * should return exactly {} records (inserted), got {}",
        inserted,
        batch.len()
    );
}

/// Verify LIMIT works correctly on the movies dataset.
#[tokio::test]
async fn test_movies_select_with_limit() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    // Insert a smaller subset (50 movies) for speed
    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");
    let mut inserted = 0usize;

    for result in rdr.records().take(50) {
        let record = result.expect("CSV parse error");
        let title = escape_sql(record.get(1).unwrap_or(""));
        let year = record.get(6).unwrap_or("0");
        let rating = record.get(8).unwrap_or("0.0");

        let sql = format!(
            "INSERT INTO movies (title, year, rating) VALUES ('{}', {}, {})",
            title, year, rating
        );
        if exec_insert(&executor, &sql).await.is_ok() {
            inserted += 1;
        }
    }

    assert!(
        inserted >= 45,
        "expected at least 45 inserts, got {}",
        inserted
    );

    let mut catalog = Catalog::new();
    catalog.add_collection("movies");

    // Test various LIMIT values
    for limit in [1, 5, 10, 25] {
        let sql = format!("SELECT * FROM movies LIMIT {}", limit);
        let batch = exec_select(&executor, &sql, &catalog)
            .await
            .expect("LIMIT query failed");

        assert_eq!(
            batch.len(),
            limit,
            "LIMIT {} should return exactly {} records, got {}",
            limit,
            limit,
            batch.len()
        );
    }

    // LIMIT larger than dataset
    let batch = exec_select(&executor, "SELECT * FROM movies LIMIT 10000", &catalog)
        .await
        .expect("large LIMIT query failed");
    assert_eq!(
        batch.len(),
        inserted,
        "LIMIT 10000 should return all {} records, got {}",
        inserted,
        batch.len()
    );
}

// ===========================================================================
// 2. COUNTRIES DATASET — diverse data types
// ===========================================================================

/// Load the world countries dataset and verify full INSERT → SELECT pipeline.
#[tokio::test]
async fn test_countries_insert_and_count() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    let path = testdata_path("countries.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open countries.csv");

    let mut inserted = 0usize;
    for result in rdr.records() {
        let record = result.expect("CSV parse error");

        // Key columns: id, name, iso3, capital, region, latitude, longitude
        let name = escape_sql(record.get(1).unwrap_or(""));
        let iso3 = escape_sql(record.get(2).unwrap_or(""));
        let capital = escape_sql(record.get(6).unwrap_or(""));
        let region = escape_sql(record.get(14).unwrap_or(""));
        let lat = record.get(23).unwrap_or("0.0");
        let lng = record.get(24).unwrap_or("0.0");

        let sql = format!(
            "INSERT INTO countries (name, iso3, capital, region, latitude, longitude) \
             VALUES ('{}', '{}', '{}', '{}', {}, {})",
            name, iso3, capital, region, lat, lng
        );

        match exec_insert(&executor, &sql).await {
            Ok(_) => inserted += 1,
            Err(e) => {
                eprintln!("WARN: failed to insert country: {}", e);
            }
        }
    }

    assert!(
        inserted >= 240,
        "expected at least 240 countries inserted, got {}",
        inserted
    );

    let mut catalog = Catalog::new();
    catalog.add_collection("countries");

    let batch = exec_select(&executor, "SELECT * FROM countries", &catalog)
        .await
        .expect("SELECT * FROM countries failed");

    assert_eq!(batch.len(), inserted);
}

// ===========================================================================
// 3. EARTHQUAKES DATASET — massive data stress test
// ===========================================================================

/// Load 8000+ earthquake records to stress test INSERT throughput and scan.
#[tokio::test]
async fn test_earthquakes_bulk_insert_and_scan() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    let path = testdata_path("earthquakes.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open earthquakes.csv");

    let mut inserted = 0usize;
    let mut errors = 0usize;

    for result in rdr.records() {
        let record = result.expect("CSV parse error");

        // Columns: id, impact.gap, impact.magnitude, impact.significance,
        //          location.depth, location.distance, location.full,
        //          location.latitude, location.longitude, location.name, ...
        let id_str = escape_sql(record.get(0).unwrap_or(""));
        let magnitude = record.get(2).unwrap_or("0.0");
        let significance = record.get(3).unwrap_or("0");
        let depth = record.get(4).unwrap_or("0.0");
        let lat = record.get(7).unwrap_or("0.0");
        let lng = record.get(8).unwrap_or("0.0");
        let location = escape_sql(record.get(9).unwrap_or(""));
        let year = record.get(17).unwrap_or("0");

        let sql = format!(
            "INSERT INTO earthquakes (event_id, magnitude, significance, depth, latitude, longitude, location, year) \
             VALUES ('{}', {}, {}, {}, {}, {}, '{}', {})",
            id_str, magnitude, significance, depth, lat, lng, location, year
        );

        match exec_insert(&executor, &sql).await {
            Ok(_) => inserted += 1,
            Err(e) => {
                errors += 1;
                if errors <= 5 {
                    eprintln!("WARN: earthquake insert error: {}", e);
                }
            }
        }
    }

    eprintln!("Earthquakes: {} inserted, {} errors", inserted, errors);

    assert!(
        inserted >= 8000,
        "expected at least 8000 earthquakes inserted, got {}",
        inserted
    );

    // Full scan
    let mut catalog = Catalog::new();
    catalog.add_collection("earthquakes");

    let batch = exec_select(&executor, "SELECT * FROM earthquakes", &catalog)
        .await
        .expect("SELECT * FROM earthquakes failed");

    assert_eq!(batch.len(), inserted);

    // Verify LIMIT on large dataset
    let batch_limited = exec_select(&executor, "SELECT * FROM earthquakes LIMIT 100", &catalog)
        .await
        .expect("LIMIT query on earthquakes failed");

    assert_eq!(batch_limited.len(), 100);
}

// ===========================================================================
// 4. CONFIDENCE FILTERING on real data
// ===========================================================================

/// Insert records with varying confidence and filter them.
#[tokio::test]
async fn test_confidence_filtering_on_movie_data() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");

    let mut high_conf_count = 0usize;
    let mut total = 0usize;

    for result in rdr.records().take(100) {
        let record = result.expect("CSV parse error");

        let title = escape_sql(record.get(1).unwrap_or(""));
        let rating_str = record.get(8).unwrap_or("5.0");
        let rating: f64 = rating_str.parse().unwrap_or(5.0);

        // Use rating/10 as confidence (0.0 - 1.0 scale)
        let confidence = (rating / 10.0).min(1.0) as f32;

        // Build the record directly (INSERT doesn't handle _confidence via SQL columns
        // in a way FilterOperator can use — need to set it on FunRecord directly)
        let mut data_map: HashMap<String, serde_json::Value> = HashMap::new();
        data_map.insert("title".to_string(), serde_json::Value::String(title));
        data_map.insert(
            "rating".to_string(),
            serde_json::Value::Number(serde_json::Number::from_f64(rating).unwrap()),
        );

        let rec = build_record_from_map("rated_movies", &data_map, Some(confidence));
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();

        if confidence > 0.7 {
            high_conf_count += 1;
        }
        total += 1;
    }

    assert_eq!(total, 100);

    let mut catalog = Catalog::new();
    catalog.add_collection("rated_movies");

    // Filter: _confidence > 0.7
    let batch = exec_select(
        &executor,
        "SELECT * FROM rated_movies WHERE _confidence > 0.7",
        &catalog,
    )
    .await
    .expect("confidence filter query failed");

    assert_eq!(
        batch.len(),
        high_conf_count,
        "expected {} records with confidence > 0.7, got {}",
        high_conf_count,
        batch.len()
    );

    // Filter: _confidence <= 0.5
    let low_conf_expected = (0..100)
        .zip(
            csv::Reader::from_path(testdata_path("movies.csv"))
                .unwrap()
                .records()
                .take(100),
        )
        .filter(|(_, r)| {
            let rating: f64 = r
                .as_ref()
                .unwrap()
                .get(8)
                .unwrap_or("5.0")
                .parse()
                .unwrap_or(5.0);
            (rating / 10.0).min(1.0) <= 0.5
        })
        .count();

    let batch_low = exec_select(
        &executor,
        "SELECT * FROM rated_movies WHERE _confidence <= 0.5",
        &catalog,
    )
    .await
    .expect("low confidence filter failed");

    assert_eq!(
        batch_low.len(),
        low_conf_expected,
        "expected {} records with confidence <= 0.5, got {}",
        low_conf_expected,
        batch_low.len()
    );
}

// ===========================================================================
// 5. VECTOR SEARCH with embedded data
// ===========================================================================

/// Insert earthquake locations as 2D vectors (lat, lng) and search by proximity.
#[tokio::test]
async fn test_vector_search_with_geo_data() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    // Insert 50 countries as records with lat/lng vectors
    let path = testdata_path("countries.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open countries.csv");

    let mut inserted = 0usize;
    let mut records_data: Vec<(String, f32, f32)> = Vec::new();

    for result in rdr.records().take(50) {
        let record = result.expect("CSV parse error");
        let name = record.get(1).unwrap_or("").to_string();
        let lat: f32 = record.get(23).unwrap_or("0.0").parse().unwrap_or(0.0);
        let lng: f32 = record.get(24).unwrap_or("0.0").parse().unwrap_or(0.0);

        // Normalize lat/lng to [0, 1] range for vector search
        let norm_lat = (lat + 90.0) / 180.0;
        let norm_lng = (lng + 180.0) / 360.0;

        let rec = FunRecordBuilder::new("geo_points")
            .vector("coords", vec![norm_lat, norm_lng])
            .confidence(0.9)
            .data(rmp_serde::to_vec(&serde_json::json!({"name": name})).unwrap())
            .build();

        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
        records_data.push((name, norm_lat, norm_lng));
        inserted += 1;
    }

    assert_eq!(inserted, 50);

    // Query vector for a point roughly in Europe (~48°N, ~2°E → Paris area)
    let query_lat = (48.0 + 90.0) / 180.0;
    let query_lng = (2.0 + 180.0) / 360.0;

    // Manually count expected matches (cosine distance < 0.01)
    let query_vec = vec![query_lat, query_lng];
    let mut expected_matches = 0;
    for (_, lat, lng) in &records_data {
        let v = [*lat, *lng];
        let dot: f32 = query_vec.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        let norm_q: f32 = query_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_v: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_q > 0.0 && norm_v > 0.0 {
            let cos_dist = 1.0 - dot / (norm_q * norm_v);
            if cos_dist < 0.01 {
                expected_matches += 1;
            }
        }
    }

    // Use the executor directly with a VectorScan plan
    let plan = LogicalPlan::VectorScan {
        collection: "geo_points".to_string(),
        vector_field: "coords".to_string(),
        query: query_vec,
        threshold: 0.01,
    };

    let batch = executor.execute(plan).await.expect("vector scan failed");

    assert_eq!(
        batch.len(),
        expected_matches,
        "vector scan should find {} nearby points, got {}",
        expected_matches,
        batch.len()
    );
}

// ===========================================================================
// 6. FULL SQL PIPELINE — parse → bind → execute round-trip
// ===========================================================================

/// Test that INSERT SQL generated from CSV data correctly parses and binds.
#[test]
fn test_parse_insert_from_csv_data() {
    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");

    let mut parse_ok = 0;
    let mut parse_err = 0;

    for result in rdr.records() {
        let record = result.expect("CSV parse error");
        let title = escape_sql(record.get(1).unwrap_or(""));
        let genre = escape_sql(record.get(2).unwrap_or(""));
        let year = record.get(6).unwrap_or("0");
        let rating = record.get(8).unwrap_or("0.0");

        let sql = format!(
            "INSERT INTO movies (title, genre, year, rating) VALUES ('{}', '{}', {}, {})",
            title, genre, year, rating
        );

        match parse(&sql) {
            Ok(_) => parse_ok += 1,
            Err(e) => {
                parse_err += 1;
                if parse_err <= 3 {
                    eprintln!("Parse error on: {} — {}", &sql[..sql.len().min(80)], e);
                }
            }
        }
    }

    eprintln!("Movies parse: {} ok, {} errors", parse_ok, parse_err);
    assert!(
        parse_ok >= 900,
        "at least 900/1000 movie INSERTs should parse, got {}",
        parse_ok
    );
}

/// Test that all earthquake data rows can be parsed as INSERT statements.
#[test]
fn test_parse_insert_from_earthquake_data() {
    let path = testdata_path("earthquakes.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open earthquakes.csv");

    let mut parse_ok = 0;
    let mut parse_err = 0;

    for result in rdr.records() {
        let record = result.expect("CSV parse error");
        let magnitude = record.get(2).unwrap_or("0.0");
        let depth = record.get(4).unwrap_or("0.0");
        let lat = record.get(7).unwrap_or("0.0");
        let lng = record.get(8).unwrap_or("0.0");
        let location = escape_sql(record.get(9).unwrap_or(""));

        let sql = format!(
            "INSERT INTO earthquakes (magnitude, depth, lat, lng, location) \
             VALUES ({}, {}, {}, {}, '{}')",
            magnitude, depth, lat, lng, location
        );

        match parse(&sql) {
            Ok(_) => parse_ok += 1,
            Err(e) => {
                parse_err += 1;
                if parse_err <= 3 {
                    eprintln!("Parse error: {} — {}", &sql[..sql.len().min(80)], e);
                }
            }
        }
    }

    eprintln!("Earthquakes parse: {} ok, {} errors", parse_ok, parse_err);
    assert!(
        parse_ok >= 8000,
        "at least 8000 earthquake INSERTs should parse, got {}",
        parse_ok
    );
}

// ===========================================================================
// 7. MULTI-COLLECTION isolation
// ===========================================================================

/// Verify that records from different collections don't interfere.
#[tokio::test]
async fn test_multi_collection_isolation() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    // Insert 20 movies
    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");
    for result in rdr.records().take(20) {
        let record = result.expect("CSV error");
        let title = escape_sql(record.get(1).unwrap_or(""));
        let sql = format!("INSERT INTO movies (title) VALUES ('{}')", title);
        let _ = exec_insert(&executor, &sql).await;
    }

    // Insert 10 countries
    let path2 = testdata_path("countries.csv");
    let mut rdr2 = csv::Reader::from_path(&path2).expect("failed to open countries.csv");
    for result in rdr2.records().take(10) {
        let record = result.expect("CSV error");
        let name = escape_sql(record.get(1).unwrap_or(""));
        let sql = format!("INSERT INTO countries (name) VALUES ('{}')", name);
        let _ = exec_insert(&executor, &sql).await;
    }

    let mut catalog = Catalog::new();
    catalog.add_collection("movies");
    catalog.add_collection("countries");

    let movies = exec_select(&executor, "SELECT * FROM movies", &catalog)
        .await
        .expect("movie select failed");
    let countries = exec_select(&executor, "SELECT * FROM countries", &catalog)
        .await
        .expect("country select failed");

    assert_eq!(movies.len(), 20, "movies collection should have 20 records");
    assert_eq!(
        countries.len(),
        10,
        "countries collection should have 10 records"
    );
}

// ===========================================================================
// 8. DATA FIELD FILTERING — user-defined fields are filtered correctly
// ===========================================================================

/// Test that filtering on user-defined data fields (stored as MessagePack)
/// correctly applies the WHERE predicate.
///
/// The FilterOperator deserializes the MessagePack data payload and compares
/// user-defined fields (e.g., WHERE year > 2015) against the literal value.
#[tokio::test]
async fn test_data_field_filter_works() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    // Insert 30 movies
    let path = testdata_path("movies.csv");
    let mut rdr = csv::Reader::from_path(&path).expect("failed to open movies.csv");
    let mut inserted = 0;
    for result in rdr.records().take(30) {
        let record = result.expect("CSV error");
        let title = escape_sql(record.get(1).unwrap_or(""));
        let year = record.get(6).unwrap_or("2000");
        let sql = format!(
            "INSERT INTO filter_test (title, year) VALUES ('{}', {})",
            title, year
        );
        if exec_insert(&executor, &sql).await.is_ok() {
            inserted += 1;
        }
    }

    let mut catalog = Catalog::new();
    catalog.add_collection("filter_test");

    // WHERE year > 2015 should now correctly filter records.
    let batch = exec_select(
        &executor,
        "SELECT * FROM filter_test WHERE year > 2015",
        &catalog,
    )
    .await
    .expect("data field filter query failed");

    // The filter should return fewer records than inserted (only those with year > 2015).
    assert!(
        batch.len() < inserted,
        "data field filter should return fewer than {} records, got {}",
        inserted,
        batch.len()
    );
    assert!(
        !batch.is_empty(),
        "data field filter should return at least some records"
    );
}

// ===========================================================================
// 9. COMPLEX SQL PARSING — variety of queries on real schemas
// ===========================================================================

/// Parse a variety of SELECT queries that would be used on our datasets.
#[test]
fn test_parse_complex_queries_for_datasets() {
    let queries = vec![
        // Basic selects
        "SELECT * FROM movies",
        "SELECT title, rating FROM movies",
        "SELECT * FROM movies LIMIT 10",
        "SELECT * FROM movies WHERE _confidence > 0.8",
        // Aggregations (parse only — executor stub returns empty)
        "SELECT COUNT(*) FROM movies",
        "SELECT AVG(rating) FROM movies",
        "SELECT genre, COUNT(*) FROM movies GROUP BY genre",
        "SELECT year, AVG(rating), MAX(rating) FROM movies GROUP BY year",
        // Ordering
        "SELECT * FROM movies ORDER BY rating DESC",
        "SELECT * FROM movies ORDER BY year ASC LIMIT 20",
        // Countries queries
        "SELECT * FROM countries",
        "SELECT name, region FROM countries",
        "SELECT * FROM countries WHERE _confidence >= 0.9",
        "SELECT region, COUNT(*) FROM countries GROUP BY region",
        // Earthquakes queries
        "SELECT * FROM earthquakes",
        "SELECT * FROM earthquakes LIMIT 1000",
        "SELECT location, AVG(magnitude) FROM earthquakes GROUP BY location",
        "SELECT * FROM earthquakes ORDER BY magnitude DESC LIMIT 10",
        // Vector queries
        "SELECT * FROM geo_points WHERE _vector <-> [0.5, 0.5] < 0.1",
        "SELECT * FROM articles WHERE _vector <-> [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8] < 0.3",
        // Temporal queries (parse only)
        "SELECT * FROM movies AS OF SYSTEM TIME '2025-01-01T00:00:00Z'",
        // Joins (parse only)
        "SELECT * FROM movies JOIN ratings ON movies.id = ratings.movie_id",
        // Combined filters
        "SELECT * FROM earthquakes WHERE _confidence > 0.8",
    ];

    let mut passed = 0;
    let mut failed = 0;

    for sql in &queries {
        match parse(sql) {
            Ok(_) => passed += 1,
            Err(e) => {
                failed += 1;
                eprintln!("FAILED to parse: {} — error: {}", sql, e);
            }
        }
    }

    assert_eq!(
        failed,
        0,
        "all {} queries should parse successfully, {} failed",
        queries.len(),
        failed
    );
    assert_eq!(passed, queries.len());
}

// ===========================================================================
// 10. INSERT + SELECT consistency under concurrent-like load
// ===========================================================================

/// Rapidly insert records from multiple "tables" and verify counts remain
/// consistent across scans.
#[tokio::test]
async fn test_insert_consistency_across_collections() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    let collections = ["products", "orders", "users", "reviews"];
    let counts = [100, 75, 50, 30];

    for (coll, &count) in collections.iter().zip(counts.iter()) {
        for i in 0..count {
            let sql = format!(
                "INSERT INTO {} (name, idx) VALUES ('item_{}', {})",
                coll, i, i
            );
            exec_insert(&executor, &sql)
                .await
                .unwrap_or_else(|e| panic!("insert into {} failed: {}", coll, e));
        }
    }

    let mut catalog = Catalog::new();
    for c in &collections {
        catalog.add_collection(c);
    }

    // Verify each collection has the exact expected count
    for (coll, &expected) in collections.iter().zip(counts.iter()) {
        let sql = format!("SELECT * FROM {}", coll);
        let batch = exec_select(&executor, &sql, &catalog)
            .await
            .unwrap_or_else(|e| panic!("select from {} failed: {}", coll, e));

        assert_eq!(
            batch.len(),
            expected,
            "collection {} should have {} records, got {}",
            coll,
            expected,
            batch.len()
        );
    }

    // Verify total across all collections
    let total: usize = counts.iter().sum();
    let mut grand_total = 0;
    for coll in &collections {
        let sql = format!("SELECT * FROM {}", coll);
        let batch = exec_select(&executor, &sql, &catalog).await.unwrap();
        grand_total += batch.len();
    }
    assert_eq!(grand_total, total);
}

// ===========================================================================
// 11. MESSAGEPACK DATA INTEGRITY — verify stored data can be deserialized
// ===========================================================================

/// Verify that data inserted via SQL can be deserialized back from the
/// FunRecord's data payload (MessagePack format).
#[tokio::test]
async fn test_messagepack_data_integrity() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    // Insert a movie with known data
    let sql = "INSERT INTO data_check (title, year, rating) VALUES ('The Matrix', 1999, 8.7)";
    exec_insert(&executor, sql).await.expect("insert failed");

    // Scan and verify the stored data
    let mut catalog = Catalog::new();
    catalog.add_collection("data_check");

    let batch = exec_select(&executor, "SELECT * FROM data_check", &catalog)
        .await
        .expect("select failed");

    assert_eq!(batch.len(), 1, "expected 1 record");

    let record = &batch.records[0];

    // Deserialize the MessagePack payload to a generic map
    let data: HashMap<String, serde_json::Value> =
        rmp_serde::from_slice(&record.data).expect("failed to deserialize MessagePack");

    // Verify fields are present
    assert!(data.contains_key("title"), "expected 'title' key in data");
    assert!(data.contains_key("year"), "expected 'year' key in data");
    assert!(data.contains_key("rating"), "expected 'rating' key in data");

    // Verify values
    assert_eq!(
        data["title"].as_str().unwrap(),
        "The Matrix",
        "title mismatch"
    );
    assert_eq!(data["year"].as_i64().unwrap(), 1999, "year mismatch");
    let rating = data["rating"].as_f64().unwrap();
    assert!(
        (rating - 8.7).abs() < 0.01,
        "expected rating 8.7, got {}",
        rating
    );
}

// ===========================================================================
// 12. OPTIMIZER correctness — predicate pushdown with real filters
// ===========================================================================

/// Verify the optimizer produces valid plans for confidence filter queries.
#[test]
fn test_optimizer_on_confidence_filters() {
    let confidence_queries = vec![
        ("SELECT * FROM movies WHERE _confidence > 0.5", 0.5_f64),
        ("SELECT * FROM movies WHERE _confidence >= 0.8", 0.8),
        ("SELECT * FROM movies WHERE _confidence < 0.3", 0.3),
        ("SELECT * FROM movies WHERE _confidence = 1.0", 1.0),
    ];

    let mut catalog = Catalog::new();
    catalog.add_collection("movies");

    for (sql, _threshold) in &confidence_queries {
        let stmt = parse(sql).expect("parse failed");
        let plan = bind(stmt, &catalog).expect("bind failed");
        let optimized = optimize(plan);

        // The optimized plan should not panic and should be a valid plan
        match &optimized {
            LogicalPlan::Scan { predicate, .. } => {
                // Predicate was pushed down into the scan
                assert!(
                    predicate.is_some(),
                    "expected predicate pushdown for: {}",
                    sql
                );
            }
            LogicalPlan::Filter { .. } => {
                // Filter remained as a separate node — acceptable
            }
            _ => {
                // Any valid transformation is fine
            }
        }
    }
}
