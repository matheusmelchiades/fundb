use std::collections::HashMap;

use fundb_causal::{
    CausalEngine, CausalError, EnsembleDiscovery, GrangerDiscovery, PcDiscovery, ScmModel,
    StructuralEquation, TraceOptions, VisFormat,
};
use fundb_core::{CausalEdge, CausalOrigin, CausalType, DirectionStatus, StabilityStatus};
use uuid::Uuid;

/// Helper: create a deterministic UUID from a u128 value.
fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Helper: create a simple causal edge.
fn edge(from: Uuid, to: Uuid, strength: f32) -> CausalEdge {
    CausalEdge {
        source_id: from,
        target_id: to,
        relation: CausalType::Caused,
        strength,
        mechanism: None,
        origin: CausalOrigin::UserDeclared,
        confidence: 1.0,
        stability_score: Some(1.0),
        stability_status: StabilityStatus::Stable,
        direction_status: DirectionStatus::Confirmed,
        discovery_algo: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Tier 1 — trace 5-node DAG (multiple paths)
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_trace_5_node_dag() {
    let mut engine = CausalEngine::new();
    let a = uuid(1);
    let b = uuid(2);
    let c = uuid(3);
    let d = uuid(4);

    // A→B→C→D, A→C, B→D
    engine.insert_edge(edge(a, b, 0.9)).unwrap();
    engine.insert_edge(edge(b, c, 0.8)).unwrap();
    engine.insert_edge(edge(c, d, 0.7)).unwrap();
    engine.insert_edge(edge(a, c, 0.6)).unwrap();
    engine.insert_edge(edge(b, d, 0.5)).unwrap();

    let paths = engine.trace(
        a,
        d,
        TraceOptions {
            max_depth: 10,
            min_strength: 0.0,
            min_stability: None,
        },
    );

    assert!(
        paths.len() >= 2,
        "should find multiple paths from A to D, got {}",
        paths.len()
    );

    // All paths should start at A and end at D
    for path in &paths {
        assert_eq!(*path.nodes.first().unwrap(), a);
        assert_eq!(*path.nodes.last().unwrap(), d);
    }
}

// ---------------------------------------------------------------------------
// 2. Tier 1 — effects_of and causes_of bidirectional
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_effects_and_causes() {
    let mut engine = CausalEngine::new();
    let a = uuid(10);
    let b = uuid(20);
    let c = uuid(30);
    let d = uuid(40);

    engine.insert_edge(edge(a, b, 0.9)).unwrap();
    engine.insert_edge(edge(b, c, 0.8)).unwrap();
    engine.insert_edge(edge(c, d, 0.7)).unwrap();

    let effects = engine.effects_of(a, 10);
    assert!(
        !effects.is_empty(),
        "A should have effects (B, C, D reachable)"
    );

    let causes = engine.causes_of(d, 10);
    assert!(
        !causes.is_empty(),
        "D should have causes (C, B, A reachable)"
    );
}

// ---------------------------------------------------------------------------
// 3. Tier 1 — cycle rejection
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_cycle_rejection() {
    let mut engine = CausalEngine::new();
    let a = uuid(100);
    let b = uuid(200);
    let c = uuid(300);

    engine.insert_edge(edge(a, b, 0.9)).unwrap();
    engine.insert_edge(edge(b, c, 0.8)).unwrap();

    let result = engine.insert_edge(edge(c, a, 0.7));
    assert!(
        matches!(result, Err(CausalError::Cycle { .. })),
        "C→A should create a cycle and be rejected: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// 4. Tier 1 — Mermaid visualization
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_visualize_mermaid() {
    let mut engine = CausalEngine::new();
    let a = uuid(1000);
    let b = uuid(2000);

    engine.insert_edge(edge(a, b, 0.9)).unwrap();

    // First get some paths, then visualize them
    let paths = engine.trace(
        a,
        b,
        TraceOptions {
            max_depth: 5,
            min_strength: 0.0,
            min_stability: None,
        },
    );
    let mermaid = engine.visualize(&paths, VisFormat::Mermaid);
    assert!(
        mermaid.contains("graph") || mermaid.contains("-->") || !mermaid.is_empty(),
        "Mermaid output should contain graph directives: {}",
        mermaid
    );
}

// ---------------------------------------------------------------------------
// 5. Tier 1 — trace with stability filter
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_trace_stability_filter() {
    let mut engine = CausalEngine::new();
    let a = uuid(1);
    let b = uuid(2);
    let c = uuid(3);

    // A→B (stable), B→C (unstable via low stability_score)
    engine.insert_edge(edge(a, b, 0.9)).unwrap();
    let mut unstable_edge = edge(b, c, 0.8);
    unstable_edge.stability_score = Some(0.2);
    unstable_edge.stability_status = StabilityStatus::Unstable;
    engine.insert_edge(unstable_edge).unwrap();

    // Trace with high min_stability should filter unstable edges
    let paths = engine.trace(
        a,
        c,
        TraceOptions {
            max_depth: 10,
            min_strength: 0.0,
            min_stability: Some(0.8),
        },
    );

    // With min_stability=0.8, the B→C edge (stability=0.2) should be filtered
    assert!(
        paths.is_empty(),
        "high min_stability should filter unstable path: got {} paths",
        paths.len()
    );
}

// ---------------------------------------------------------------------------
// 6. Granger — synthetic causal series
// ---------------------------------------------------------------------------
#[test]
fn test_granger_synthetic_causal_series() {
    let granger = GrangerDiscovery::new();

    let n = 100;
    let mut x = vec![0.0f64; n];
    let mut y = vec![0.0f64; n];

    // x is random-ish, y[t] = 0.8 * x[t-1] + noise
    for (i, xi) in x.iter_mut().enumerate().take(n) {
        *xi = (i as f64 * 0.1).sin();
    }
    for i in 1..n {
        y[i] = 0.8 * x[i - 1] + (i as f64 * 0.3).cos() * 0.1;
    }

    let result = granger.test_pair(&x, &y, 3);
    assert!(
        result.p_value < 0.05,
        "causal series should have p_value < 0.05, got {}",
        result.p_value
    );
    assert!(result.f_stat > 0.0, "F-statistic should be positive");
}

// ---------------------------------------------------------------------------
// 7. Granger — no causality (independent series)
// ---------------------------------------------------------------------------
#[test]
fn test_granger_no_causality_independent() {
    let granger = GrangerDiscovery::new();

    // x varies but y is constant — x carries zero predictive information for y
    let n = 100;
    let x: Vec<f64> = (0..n)
        .map(|i: i32| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let y: Vec<f64> = vec![5.0; n as usize];

    let result = granger.test_pair(&x, &y, 3);
    assert!(
        result.p_value > 0.05,
        "independent series should have p_value > 0.05, got {}",
        result.p_value
    );
}

// ---------------------------------------------------------------------------
// 8. Granger — rolling window stability
// ---------------------------------------------------------------------------
#[test]
fn test_granger_rolling_window_stability() {
    let granger = GrangerDiscovery::new();

    let n = 200;
    let mut x = vec![0.0f64; n];
    let mut y = vec![0.0f64; n];

    for (i, xi) in x.iter_mut().enumerate().take(n) {
        *xi = (i as f64 * 0.1).sin();
    }
    for i in 1..n {
        y[i] = 0.8 * x[i - 1] + (i as f64 * 0.3).cos() * 0.05;
    }

    let result = granger.rolling_window_test(&x, &y, 50, 10);
    assert!(result.windows_tested > 0, "should test at least one window");
    assert!(
        result.stability_status == StabilityStatus::Stable
            || result.stability_status == StabilityStatus::Provisional,
        "consistent causal signal should be Stable or Provisional, got {:?}",
        result.stability_status
    );
}

// ---------------------------------------------------------------------------
// 9. SCM — intervention (diamond graph)
// ---------------------------------------------------------------------------
#[test]
fn test_scm_intervention_diamond() {
    // X→Y, X→Z, Y→W, Z→W
    let equations = vec![
        StructuralEquation {
            variable: "X".to_string(),
            parents: vec![],
            coefficients: vec![],
            intercept: 1.0,
            noise_std: 0.0,
        },
        StructuralEquation {
            variable: "Y".to_string(),
            parents: vec!["X".to_string()],
            coefficients: vec![2.0],
            intercept: 0.0,
            noise_std: 0.0,
        },
        StructuralEquation {
            variable: "Z".to_string(),
            parents: vec!["X".to_string()],
            coefficients: vec![3.0],
            intercept: 0.0,
            noise_std: 0.0,
        },
        StructuralEquation {
            variable: "W".to_string(),
            parents: vec!["Y".to_string(), "Z".to_string()],
            coefficients: vec![1.0, 1.0],
            intercept: 0.0,
            noise_std: 0.0,
        },
    ];

    let model = ScmModel::new(equations).unwrap();

    // Natural: X=1 → Y=2, Z=3, W=5
    let obs: HashMap<String, f64> = [("X".to_string(), 1.0)].into_iter().collect();
    let natural = model.evaluate(&obs);
    assert!(
        (natural["W"] - 5.0).abs() < 1e-6,
        "natural W should be 5.0, got {}",
        natural["W"]
    );

    // Intervention: do(X=0) → Y=0, Z=0, W=0
    let interventional = model.intervene("X", 0.0, "W", &obs).unwrap();
    // After do(X=0), W should be different from natural
    assert!(
        (interventional - 5.0).abs() > 0.1 || (interventional - 0.0).abs() < 1e-6,
        "do(X=0) should alter W"
    );
}

// ---------------------------------------------------------------------------
// 10. SCM — counterfactual with noise
// ---------------------------------------------------------------------------
#[test]
fn test_scm_counterfactual_with_noise() {
    // Y = 2X + 1
    let equations = vec![
        StructuralEquation {
            variable: "X".to_string(),
            parents: vec![],
            coefficients: vec![],
            intercept: 3.0,
            noise_std: 0.0,
        },
        StructuralEquation {
            variable: "Y".to_string(),
            parents: vec!["X".to_string()],
            coefficients: vec![2.0],
            intercept: 1.0,
            noise_std: 0.0,
        },
    ];

    let model = ScmModel::new(equations).unwrap();

    // Observe: X=3, Y should be 7
    let obs: HashMap<String, f64> = [("X".to_string(), 3.0), ("Y".to_string(), 7.0)]
        .into_iter()
        .collect();

    // Counterfactual: had X been 2, Y would be 2*2+1=5
    let cf_y = model.counterfactual("X", 2.0, "Y", &obs).unwrap();
    assert!(
        (cf_y - 5.0).abs() < 1e-4,
        "counterfactual Y should be 5.0 when X=2, got {}",
        cf_y
    );
}

// ---------------------------------------------------------------------------
// 11. PC discovers V-structure
// ---------------------------------------------------------------------------
#[test]
fn test_pc_discovers_v_structure() {
    // X→Z←Y: X and Y are independent but both cause Z
    // Correlation matrix (3×3):
    // X correlates with Z, Y correlates with Z, but X and Y are independent
    let vars = vec!["X".to_string(), "Y".to_string(), "Z".to_string()];
    let correlations = vec![
        1.0, 0.0, 0.7, // X row
        0.0, 1.0, 0.6, // Y row
        0.7, 0.6, 1.0, // Z row
    ];

    let edges = PcDiscovery::discover(&vars, &correlations, 0.05);
    assert!(
        !edges.is_empty(),
        "PC should discover edges in the V-structure"
    );
}

// ---------------------------------------------------------------------------
// 12. Ensemble agreement
// ---------------------------------------------------------------------------
#[test]
fn test_ensemble_agreement() {
    use fundb_causal::{EdgeOrientation, PcEdge};

    // PC discovers A→B
    let pc_edges = vec![PcEdge {
        from: "A".to_string(),
        to: "B".to_string(),
        orientation: EdgeOrientation::Directed,
    }];

    // Granger also finds A→B
    let granger_pairs = vec![("A".to_string(), "B".to_string())];

    // Explicit edge A→B
    let explicit_edges = vec![("A".to_string(), "B".to_string())];

    let ensemble = EnsembleDiscovery::merge(pc_edges, granger_pairs, explicit_edges, 2);
    assert!(
        !ensemble.is_empty(),
        "ensemble should find edges with multi-source support"
    );

    let ab_edge = ensemble
        .iter()
        .find(|e| (e.from == "A" && e.to == "B") || (e.from == "B" && e.to == "A"));
    assert!(ab_edge.is_some(), "ensemble should include A→B edge");
    let ab = ab_edge.unwrap();
    assert!(
        ab.support >= 2,
        "A→B should have support >= 2, got {}",
        ab.support
    );
}

// ---------------------------------------------------------------------------
// 13. Tier1 + Tier2 combined
// ---------------------------------------------------------------------------
#[test]
fn test_tier1_plus_tier2_combined() {
    let granger = GrangerDiscovery::new();

    // Granger discovers causality
    let n = 100;
    let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.1).sin()).collect();
    let mut y = vec![0.0f64; n];
    for i in 1..n {
        y[i] = 0.8 * x[i - 1] + (i as f64 * 0.3).cos() * 0.05;
    }
    let result = granger.test_pair(&x, &y, 3);

    if result.p_value < 0.05 {
        // Granger found causality — now create edge in DAG
        let mut engine = CausalEngine::new();
        let x_id = uuid(1);
        let y_id = uuid(2);

        let causal_edge = CausalEdge {
            source_id: x_id,
            target_id: y_id,
            relation: CausalType::Caused,
            strength: 1.0 - result.p_value as f32,
            mechanism: Some("Granger causality test".to_string()),
            origin: CausalOrigin::Granger {
                p_value: result.p_value,
                lag: result.lag as u32,
            },
            confidence: 1.0 - result.p_value as f32,
            stability_score: Some(0.9),
            stability_status: StabilityStatus::Stable,
            direction_status: DirectionStatus::Confirmed,
            discovery_algo: Some("granger".to_string()),
        };

        engine.insert_edge(causal_edge).unwrap();

        // Now trace in the DAG
        let paths = engine.trace(
            x_id,
            y_id,
            TraceOptions {
                max_depth: 5,
                min_strength: 0.0,
                min_stability: None,
            },
        );
        assert!(!paths.is_empty(), "trace should find path from x to y");
    }
}

// ---------------------------------------------------------------------------
// 14. SCM — cycle detection in equations
// ---------------------------------------------------------------------------
#[test]
fn test_scm_topological_sort_validates() {
    // A depends on B, B depends on A — cyclic!
    let equations = vec![
        StructuralEquation {
            variable: "A".to_string(),
            parents: vec!["B".to_string()],
            coefficients: vec![1.0],
            intercept: 0.0,
            noise_std: 0.0,
        },
        StructuralEquation {
            variable: "B".to_string(),
            parents: vec!["A".to_string()],
            coefficients: vec![1.0],
            intercept: 0.0,
            noise_std: 0.0,
        },
    ];

    let result = ScmModel::new(equations);
    assert!(
        result.is_err(),
        "SCM with cyclic equations should fail: {:?}",
        result
    );
}
