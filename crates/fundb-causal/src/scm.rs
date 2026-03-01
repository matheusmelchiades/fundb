// crates/fundb-causal/src/scm.rs
//
// STORY-9-1: Structural Causal Models — Interventions + Counterfactuals + Ensemble Discovery
//
// Implements:
//   - ScmModel: structural causal model with do-calculus interventions and counterfactuals
//   - PcDiscovery: PC algorithm skeleton discovery via partial correlation + Fisher Z-test
//   - EnsembleDiscovery: consensus DAG from PC + Granger + explicit edges

use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ScmError {
    #[error("cycle detected involving variable '{0}'. Causal models must be acyclic (DAG). Use MODE EQUILIBRIUM for feedback loops")]
    CycleDetected(String),
    #[error("unknown variable '{0}'. Check the variable name exists in the causal model")]
    UnknownVariable(String),
    #[error("dimension mismatch: expected {expected} variables, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("no causal path from '{from}' to '{to}'. These variables may be independent in the model")]
    NoPath { from: String, to: String },
    #[error("causal model did not converge: {0}. Try increasing max iterations or check for numerical instability")]
    NonConvergence(String),
}

// ---------------------------------------------------------------------------
// Structural Equation
// ---------------------------------------------------------------------------

/// A linear structural equation: Y = Σ(coeff_i × X_i) + noise
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructuralEquation {
    pub variable: String,
    pub parents: Vec<String>,
    pub coefficients: Vec<f64>, // len == parents.len()
    pub intercept: f64,
    pub noise_std: f64,
}

// ---------------------------------------------------------------------------
// ScmModel
// ---------------------------------------------------------------------------

/// A complete Structural Causal Model — directed acyclic graph of equations
#[derive(Debug, Clone)]
pub struct ScmModel {
    equations: HashMap<String, StructuralEquation>,
    topo_order: Vec<String>, // topological sort for forward evaluation
}

impl ScmModel {
    /// Build from a list of equations. Returns Err if cycles detected.
    pub fn new(equations: Vec<StructuralEquation>) -> Result<Self, ScmError> {
        // Validate dimension consistency: coefficients.len() must equal parents.len()
        for eq in &equations {
            if eq.coefficients.len() != eq.parents.len() {
                return Err(ScmError::DimensionMismatch {
                    expected: eq.parents.len(),
                    got: eq.coefficients.len(),
                });
            }
        }

        let eq_map: HashMap<String, StructuralEquation> = equations
            .into_iter()
            .map(|eq| (eq.variable.clone(), eq))
            .collect();

        // Kahn's algorithm for topological sort + cycle detection
        // Build in-degree map and adjacency list (parent -> children)
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();

        for (var, eq) in &eq_map {
            in_degree.entry(var.clone()).or_insert(0);
            for parent in &eq.parents {
                in_degree.entry(parent.clone()).or_insert(0);
                children
                    .entry(parent.clone())
                    .or_default()
                    .push(var.clone());
                *in_degree.entry(var.clone()).or_insert(0) += 1;
            }
        }

        // Collect all known variables (both defined and referenced as parents)
        let all_vars: HashSet<String> = in_degree.keys().cloned().collect();

        // Initialize queue with zero-in-degree nodes
        let mut queue: VecDeque<String> = VecDeque::new();
        for var in &all_vars {
            if in_degree[var] == 0 {
                queue.push_back(var.clone());
            }
        }

        // Sort queue entries for deterministic ordering
        let mut sorted_queue: Vec<String> = queue.into_iter().collect();
        sorted_queue.sort();
        let mut queue: VecDeque<String> = sorted_queue.into_iter().collect();

        let mut topo_order: Vec<String> = Vec::new();

        while let Some(var) = queue.pop_front() {
            topo_order.push(var.clone());

            if let Some(child_list) = children.get(&var) {
                let mut new_zero: Vec<String> = Vec::new();
                for child in child_list {
                    let deg = in_degree.get_mut(child).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        new_zero.push(child.clone());
                    }
                }
                new_zero.sort();
                for c in new_zero {
                    queue.push_back(c);
                }
            }
        }

        // If we didn't visit all nodes, there's a cycle
        if topo_order.len() != all_vars.len() {
            // Find a variable involved in the cycle
            let visited: HashSet<&String> = topo_order.iter().collect();
            let cycle_var = all_vars
                .iter()
                .find(|v| !visited.contains(v))
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            return Err(ScmError::CycleDetected(cycle_var));
        }

        Ok(ScmModel {
            equations: eq_map,
            topo_order,
        })
    }

    /// Evaluate all variables in topological order given root node values.
    /// `observations`: known variable → value pairs (roots / exogenous)
    pub fn evaluate(&self, observations: &HashMap<String, f64>) -> HashMap<String, f64> {
        let mut values: HashMap<String, f64> = observations.clone();

        for var in &self.topo_order {
            if values.contains_key(var) {
                // Already known (observation or prior computation)
                continue;
            }

            if let Some(eq) = self.equations.get(var) {
                let computed = self.compute_equation(eq, &values);
                values.insert(var.clone(), computed);
            }
        }

        values
    }

    /// do-calculus intervention: set variable to value, re-evaluate downstream.
    /// Returns estimated value of `target` after intervention.
    pub fn intervene(
        &self,
        intervention: &str,
        value: f64,
        target: &str,
        observations: &HashMap<String, f64>,
    ) -> Result<f64, ScmError> {
        // Validate that intervention and target are known variables
        if !self.equations.contains_key(intervention) && !observations.contains_key(intervention) {
            return Err(ScmError::UnknownVariable(intervention.to_string()));
        }
        if !self.equations.contains_key(target) && !observations.contains_key(target) {
            return Err(ScmError::UnknownVariable(target.to_string()));
        }

        // Clone observations and override intervention variable (do-calculus: cut incoming edges)
        let mut mutilated_obs = observations.clone();
        mutilated_obs.insert(intervention.to_string(), value);

        // Evaluate with mutilated model (intervened variable is fixed, no equation used)
        let result = self.evaluate_mutilated(intervention, &mutilated_obs);

        result
            .get(target)
            .copied()
            .ok_or_else(|| ScmError::UnknownVariable(target.to_string()))
    }

    /// Counterfactual: given observed world, "what would `target` have been if `antecedent` had been `value`?"
    pub fn counterfactual(
        &self,
        antecedent: &str,
        antecedent_value: f64,
        target: &str,
        observations: &HashMap<String, f64>,
    ) -> Result<f64, ScmError> {
        // Validate variables
        if !self.equations.contains_key(antecedent) && !observations.contains_key(antecedent) {
            return Err(ScmError::UnknownVariable(antecedent.to_string()));
        }
        if !self.equations.contains_key(target) && !observations.contains_key(target) {
            return Err(ScmError::UnknownVariable(target.to_string()));
        }

        // Step 1: Abduction — compute exogenous noise terms from observed values
        // For each variable with an equation, compute: noise = observed - (intercept + Σ coeff_i * parent_i)
        let factual_values = self.evaluate(observations);
        let mut noise_terms: HashMap<String, f64> = HashMap::new();

        for (var, eq) in &self.equations {
            if let Some(&obs_val) = factual_values.get(var) {
                let structural_val = self.compute_equation(eq, &factual_values);
                let noise = obs_val - structural_val;
                noise_terms.insert(var.clone(), noise);
            }
        }

        // Step 2: Action — perform intervention on antecedent
        // Step 3: Prediction — re-evaluate with noise terms carried over.
        // cf_obs must only contain exogenous variables (those without equations),
        // because endogenous variables must be recomputed with the counterfactual X.
        // Keeping observed values of endogenous vars would prevent recomputation.
        let mut cf_obs: HashMap<String, f64> = HashMap::new();
        for (k, v) in observations {
            if !self.equations.contains_key(k) {
                cf_obs.insert(k.clone(), *v);
            }
        }
        cf_obs.insert(antecedent.to_string(), antecedent_value);

        let result = self.evaluate_counterfactual_with_noise(antecedent, &cf_obs, &noise_terms);

        result
            .get(target)
            .copied()
            .ok_or_else(|| ScmError::UnknownVariable(target.to_string()))
    }

    /// List all variables in the model.
    pub fn variables(&self) -> Vec<&str> {
        self.topo_order.iter().map(|s| s.as_str()).collect()
    }

    /// Return direct parents of a variable.
    pub fn parents_of(&self, variable: &str) -> Vec<&str> {
        self.equations
            .get(variable)
            .map(|eq| eq.parents.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Compute the structural equation value (without noise) given current values.
    fn compute_equation(&self, eq: &StructuralEquation, values: &HashMap<String, f64>) -> f64 {
        let mut result = eq.intercept;
        for (parent, &coeff) in eq.parents.iter().zip(eq.coefficients.iter()) {
            let parent_val = values.get(parent).copied().unwrap_or(0.0);
            result += coeff * parent_val;
        }
        result
    }

    /// Evaluate with mutilated model: intervention variable is fixed (incoming edges severed).
    /// The intervention value must already be present in `observations` before calling this.
    fn evaluate_mutilated(
        &self,
        _intervention: &str,
        observations: &HashMap<String, f64>,
    ) -> HashMap<String, f64> {
        let mut values: HashMap<String, f64> = observations.clone();

        for var in &self.topo_order {
            if values.contains_key(var) {
                // Fixed (either observation or intervention — equation severed by do-calculus)
                continue;
            }

            if let Some(eq) = self.equations.get(var) {
                let computed = self.compute_equation(eq, &values);
                values.insert(var.clone(), computed);
            }
        }

        values
    }

    /// Evaluate with noise terms added back (counterfactual prediction step).
    fn evaluate_counterfactual_with_noise(
        &self,
        antecedent: &str,
        observations: &HashMap<String, f64>,
        noise_terms: &HashMap<String, f64>,
    ) -> HashMap<String, f64> {
        let mut values: HashMap<String, f64> = observations.clone();

        for var in &self.topo_order {
            if values.contains_key(var) {
                // Fixed (either observation or antecedent intervention)
                continue;
            }

            if let Some(eq) = self.equations.get(var) {
                let mut computed = self.compute_equation(eq, &values);
                // Add back the abducted noise for this variable
                if var != antecedent {
                    if let Some(&noise) = noise_terms.get(var) {
                        computed += noise;
                    }
                }
                values.insert(var.clone(), computed);
            }
        }

        values
    }
}

// ---------------------------------------------------------------------------
// PC Algorithm
// ---------------------------------------------------------------------------

/// Edge orientation in a discovered causal graph
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeOrientation {
    Directed,
    Undirected,
    DirectionUncertain,
}

/// Result of PC algorithm edge discovery
#[derive(Debug, Clone)]
pub struct PcEdge {
    pub from: String,
    pub to: String,
    pub orientation: EdgeOrientation,
}

pub struct PcDiscovery;

impl PcDiscovery {
    /// Run PC algorithm on a correlation matrix.
    /// `variables`: ordered variable names
    /// `correlations`: N×N correlation matrix (flattened row-major)
    /// `alpha`: significance level for independence tests (e.g. 0.05)
    pub fn discover(variables: &[String], correlations: &[f64], alpha: f64) -> Vec<PcEdge> {
        let n = variables.len();
        if n < 2 || correlations.len() != n * n {
            return vec![];
        }

        // We need a sample size estimate; use a heuristic based on correlation matrix size.
        // In practice this would come from the data, but here we use n_samples = 100 as default
        // for significance testing when not explicitly provided.
        let n_samples = 100usize;

        // Step 1: Start with complete undirected graph — track adjacency as a set of edges
        let mut adjacency: Vec<Vec<bool>> = vec![vec![true; n]; n];
        // Remove self-loops
        for i in 0..n {
            adjacency[i][i] = false;
        }

        // Step 2: Test unconditional independence (conditioning set = ∅)
        for i in 0..n {
            for j in (i + 1)..n {
                let r = correlations[i * n + j];
                let p = Self::fisher_z_test(r, n_samples, 0);
                if p > alpha {
                    // X ⊥ Y — remove edge
                    adjacency[i][j] = false;
                    adjacency[j][i] = false;
                }
            }
        }

        // Step 3: Test conditional independence with conditioning sets of size 1
        for i in 0..n {
            for j in (i + 1)..n {
                if !adjacency[i][j] {
                    continue;
                }
                // Try each possible conditioning variable Z ≠ i, j
                let mut remove = false;
                for k in 0..n {
                    if k == i || k == j {
                        continue;
                    }
                    let partial_r =
                        Self::partial_correlation(correlations, n, i, j, &[k]);
                    let p = Self::fisher_z_test(partial_r, n_samples, 1);
                    if p > alpha {
                        remove = true;
                        break;
                    }
                }
                if remove {
                    adjacency[i][j] = false;
                    adjacency[j][i] = false;
                }
            }
        }

        // Step 4: Orient v-structures: X → Z ← Y when X–Z–Y skeleton and X not adjacent to Y
        // We'll track directed edges: oriented[i][j] = true means i→j is directed
        let mut directed: Vec<Vec<Option<bool>>> = vec![vec![None; n]; n];
        // None = undirected, Some(true) = directed i→j, Some(false) = directed j→i

        for z in 0..n {
            for i in 0..n {
                if i == z || !adjacency[i][z] {
                    continue;
                }
                for j in (i + 1)..n {
                    if j == z || !adjacency[j][z] {
                        continue;
                    }
                    // X=i, Z=z, Y=j: check if i–z–j path with i not adjacent to j
                    if !adjacency[i][j] {
                        // V-structure: i → z ← j
                        directed[i][z] = Some(true); // i → z
                        directed[z][i] = Some(false);
                        directed[j][z] = Some(true); // j → z
                        directed[z][j] = Some(false);
                    }
                }
            }
        }

        // Step 5: Collect edges
        let mut edges: Vec<PcEdge> = Vec::new();
        let mut seen: HashSet<(usize, usize)> = HashSet::new();

        for i in 0..n {
            for j in (i + 1)..n {
                if !adjacency[i][j] {
                    continue;
                }
                if seen.contains(&(i, j)) || seen.contains(&(j, i)) {
                    continue;
                }
                seen.insert((i, j));

                // Determine orientation
                let orientation = match (directed[i][j], directed[j][i]) {
                    (Some(true), Some(false)) => {
                        // i → j
                        edges.push(PcEdge {
                            from: variables[i].clone(),
                            to: variables[j].clone(),
                            orientation: EdgeOrientation::Directed,
                        });
                        continue;
                    }
                    (Some(false), Some(true)) => {
                        // j → i
                        edges.push(PcEdge {
                            from: variables[j].clone(),
                            to: variables[i].clone(),
                            orientation: EdgeOrientation::Directed,
                        });
                        continue;
                    }
                    _ => EdgeOrientation::Undirected,
                };

                edges.push(PcEdge {
                    from: variables[i].clone(),
                    to: variables[j].clone(),
                    orientation,
                });
            }
        }

        edges
    }

    /// Partial correlation: r_{XY.Z} from full correlation matrix
    /// Uses the recursive formula for conditioning on a single variable first,
    /// then iterating for larger conditioning sets.
    pub fn partial_correlation(
        corr: &[f64],
        n_vars: usize,
        x: usize,
        y: usize,
        conditioning_set: &[usize],
    ) -> f64 {
        if conditioning_set.is_empty() {
            return corr[x * n_vars + y];
        }

        if conditioning_set.len() == 1 {
            let z = conditioning_set[0];
            let r_xy = corr[x * n_vars + y];
            let r_xz = corr[x * n_vars + z];
            let r_yz = corr[y * n_vars + z];

            let denom = ((1.0 - r_xz * r_xz) * (1.0 - r_yz * r_yz)).sqrt();
            if denom.abs() < 1e-10 {
                return 0.0;
            }
            return (r_xy - r_xz * r_yz) / denom;
        }

        // For larger conditioning sets, use recursive approach:
        // r_{XY|Z1,...,Zk} = r_{XY|Z2,...,Zk} - r_{XZ1|Z2,...,Zk} * r_{YZ1|Z2,...,Zk}
        //                    / sqrt((1 - r_{XZ1|Z2,...,Zk}^2) * (1 - r_{YZ1|Z2,...,Zk}^2))
        let z = conditioning_set[0];
        let rest = &conditioning_set[1..];

        let r_xy_rest = Self::partial_correlation(corr, n_vars, x, y, rest);
        let r_xz_rest = Self::partial_correlation(corr, n_vars, x, z, rest);
        let r_yz_rest = Self::partial_correlation(corr, n_vars, y, z, rest);

        let denom = ((1.0 - r_xz_rest * r_xz_rest) * (1.0 - r_yz_rest * r_yz_rest)).sqrt();
        if denom.abs() < 1e-10 {
            return 0.0;
        }

        (r_xy_rest - r_xz_rest * r_yz_rest) / denom
    }

    /// Fisher's Z-test for independence: returns p-value
    /// H0: the partial correlation is zero (variables are conditionally independent)
    pub fn fisher_z_test(partial_corr: f64, n_samples: usize, k: usize) -> f64 {
        // Fisher Z-transformation: Z = 0.5 * ln((1+r)/(1-r))
        let r = partial_corr.clamp(-0.9999, 0.9999);
        let z = 0.5 * ((1.0 + r) / (1.0 - r)).ln();

        // Standard error: SE = 1 / sqrt(n - k - 3)
        let df = n_samples as isize - k as isize - 3;
        if df <= 0 {
            // Not enough samples — cannot reject independence
            return 1.0;
        }

        let se = 1.0 / (df as f64).sqrt();
        let test_stat = z / se; // z-statistic

        // Two-tailed p-value from standard normal distribution
        // Approximate using complementary error function
        p_value_from_z(test_stat.abs())
    }
}

/// Approximate two-tailed p-value from a z-statistic using the standard normal CDF.
/// P(|Z| > z) = 2 * (1 - Φ(z)) ≈ 2 * erfc(z / sqrt(2)) / 2
fn p_value_from_z(z_abs: f64) -> f64 {
    // Use a rational approximation for the complementary normal CDF
    // Abramowitz & Stegun approximation for erfc
    let x = z_abs / std::f64::consts::SQRT_2;
    let erfc_val = erfc_approx(x);
    erfc_val // This is already 2 * (1 - Phi(z_abs)) = 2-tailed p-value
}

/// Approximate erfc(x) = 2 * (1 - Phi(x * sqrt(2)))
/// Using Abramowitz & Stegun formula 7.1.26
fn erfc_approx(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc_approx(-x);
    }

    // For large x, erfc is essentially 0
    if x > 5.0 {
        return 1e-12;
    }

    // Approximation valid for x >= 0
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erfc = poly * (-x * x).exp();
    erfc.max(0.0)
}

// ---------------------------------------------------------------------------
// Ensemble Discovery
// ---------------------------------------------------------------------------

/// Combines PC + Granger + Tier1 explicit edges into a consensus DAG
pub struct EnsembleDiscovery;

#[derive(Debug, Clone)]
pub struct EnsembleEdge {
    pub from: String,
    pub to: String,
    /// agreement count out of methods used
    pub support: u8,
    /// "PC", "Granger", "Explicit"
    pub sources: Vec<String>,
    pub orientation: EdgeOrientation,
}

impl EnsembleDiscovery {
    /// Merge edges from multiple discovery methods.
    /// Edges with support >= min_support are included.
    pub fn merge(
        pc_edges: Vec<PcEdge>,
        granger_pairs: Vec<(String, String)>,
        explicit_edges: Vec<(String, String)>,
        min_support: u8,
    ) -> Vec<EnsembleEdge> {
        // Canonical key: (from, to) — normalize to alphabetical order for counting
        // but preserve directionality information per source

        // Map from canonical edge key to (sources, directed_sources, reverse_directed_sources)
        // Key is always (lexicographic_first, lexicographic_second)
        let mut edge_info: HashMap<(String, String), EdgeAccumulator> = HashMap::new();

        // Helper closure to get canonical key
        let canonical = |a: &str, b: &str| -> (String, String) {
            if a <= b {
                (a.to_string(), b.to_string())
            } else {
                (b.to_string(), a.to_string())
            }
        };

        // Process PC edges
        for edge in &pc_edges {
            let key = canonical(&edge.from, &edge.to);
            let acc = edge_info.entry(key.clone()).or_default();
            acc.sources.insert("PC".to_string());
            match &edge.orientation {
                EdgeOrientation::Directed => {
                    // Record which direction this directed edge goes
                    acc.directed_from.insert(edge.from.clone());
                }
                _ => {
                    acc.has_undirected = true;
                }
            }
        }

        // Process Granger pairs (cause → effect, so directed)
        for (cause, effect) in &granger_pairs {
            let key = canonical(cause, effect);
            let acc = edge_info.entry(key.clone()).or_default();
            acc.sources.insert("Granger".to_string());
            acc.directed_from.insert(cause.clone());
        }

        // Process explicit edges (directed)
        for (from, to) in &explicit_edges {
            let key = canonical(from, to);
            let acc = edge_info.entry(key.clone()).or_default();
            acc.sources.insert("Explicit".to_string());
            acc.directed_from.insert(from.clone());
        }

        // Build ensemble edges, filtering by min_support
        let mut result: Vec<EnsembleEdge> = Vec::new();

        for ((a, b), acc) in edge_info {
            let support = acc.sources.len() as u8;
            if support < min_support {
                continue;
            }

            // Determine orientation
            let (from, to, orientation) = if acc.directed_from.len() == 1 {
                // All directed sources agree on direction
                let directed_from = acc.directed_from.iter().next().unwrap().clone();
                let directed_to = if directed_from == a {
                    b.clone()
                } else {
                    a.clone()
                };
                (directed_from, directed_to, EdgeOrientation::Directed)
            } else if acc.directed_from.is_empty() {
                // All undirected
                (a, b, EdgeOrientation::Undirected)
            } else {
                // Mixed or conflicting directions
                (a, b, EdgeOrientation::DirectionUncertain)
            };

            let mut sources: Vec<String> = acc.sources.into_iter().collect();
            sources.sort();

            result.push(EnsembleEdge {
                from,
                to,
                support,
                sources,
                orientation,
            });
        }

        // Sort for deterministic output
        result.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then(a.to.cmp(&b.to))
        });

        result
    }

    /// When NOTEARS fails to converge (>3 restarts simulated), fall back to ensemble.
    /// Returns edges with a warning flag.
    pub fn notears_fallback(
        pc_edges: Vec<PcEdge>,
        granger_pairs: Vec<(String, String)>,
    ) -> (Vec<EnsembleEdge>, String) {
        let warning = "NOTEARS failed to converge after 3 restarts; falling back to ensemble discovery (PC + Granger)".to_string();
        let edges = Self::merge(pc_edges, granger_pairs, vec![], 1);
        (edges, warning)
    }
}

// Helper accumulator for ensemble merging
#[derive(Default)]
struct EdgeAccumulator {
    sources: HashSet<String>,
    directed_from: HashSet<String>,
    #[allow(dead_code)]
    has_undirected: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to make a simple equation
    fn eq(variable: &str, parents: Vec<&str>, coefficients: Vec<f64>, intercept: f64) -> StructuralEquation {
        StructuralEquation {
            variable: variable.to_string(),
            parents: parents.into_iter().map(|s| s.to_string()).collect(),
            coefficients,
            intercept,
            noise_std: 0.0,
        }
    }

    // -----------------------------------------------------------------------
    // 1. test_scm_new_simple — 3-variable chain X→Y→Z builds correctly
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_new_simple() {
        let equations = vec![
            eq("X", vec![], vec![], 0.0),
            eq("Y", vec!["X"], vec![2.0], 1.0),
            eq("Z", vec!["Y"], vec![3.0], 0.0),
        ];

        let model = ScmModel::new(equations).expect("should build without error");

        // All three variables should be present
        let vars: Vec<&str> = model.variables();
        assert!(vars.contains(&"X"), "model should contain X");
        assert!(vars.contains(&"Y"), "model should contain Y");
        assert!(vars.contains(&"Z"), "model should contain Z");

        // Parents should be correct
        assert!(model.parents_of("X").is_empty(), "X has no parents");
        assert_eq!(model.parents_of("Y"), vec!["X"], "Y's parent is X");
        assert_eq!(model.parents_of("Z"), vec!["Y"], "Z's parent is Y");
    }

    // -----------------------------------------------------------------------
    // 2. test_scm_cycle_detection — cycle X→Y→X returns CycleDetected
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_cycle_detection() {
        // X depends on Y, Y depends on X — cycle
        let equations = vec![
            eq("X", vec!["Y"], vec![1.0], 0.0),
            eq("Y", vec!["X"], vec![1.0], 0.0),
        ];

        let result = ScmModel::new(equations);
        assert!(
            matches!(result, Err(ScmError::CycleDetected(_))),
            "expected CycleDetected, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // 3. test_scm_evaluate_chain — X=1.0, Y=2X+1, Z=3Y → correct values
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_evaluate_chain() {
        let equations = vec![
            eq("X", vec![], vec![], 0.0),
            eq("Y", vec!["X"], vec![2.0], 1.0),  // Y = 2X + 1
            eq("Z", vec!["Y"], vec![3.0], 0.0),  // Z = 3Y
        ];

        let model = ScmModel::new(equations).unwrap();

        let mut obs = HashMap::new();
        obs.insert("X".to_string(), 1.0);

        let result = model.evaluate(&obs);

        let y = result["Y"];
        let z = result["Z"];

        // Y = 2*1 + 1 = 3
        assert!(
            (y - 3.0).abs() < 1e-9,
            "expected Y=3.0, got {}",
            y
        );
        // Z = 3*3 = 9
        assert!(
            (z - 9.0).abs() < 1e-9,
            "expected Z=9.0, got {}",
            z
        );
    }

    // -----------------------------------------------------------------------
    // 4. test_scm_intervene — do(X=0) cuts incoming edges, downstream recalculated
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_intervene() {
        // Model: X → Y → Z, Y = 2X + 1, Z = 3Y
        let equations = vec![
            eq("X", vec![], vec![], 0.0),
            eq("Y", vec!["X"], vec![2.0], 1.0),
            eq("Z", vec!["Y"], vec![3.0], 0.0),
        ];

        let model = ScmModel::new(equations).unwrap();

        let mut obs = HashMap::new();
        obs.insert("X".to_string(), 5.0); // Original X = 5

        // do(X=0): intervene setting X=0
        let z_after = model
            .intervene("X", 0.0, "Z", &obs)
            .expect("intervention should succeed");

        // With X=0: Y = 2*0 + 1 = 1, Z = 3*1 = 3
        assert!(
            (z_after - 3.0).abs() < 1e-9,
            "after do(X=0), expected Z=3.0, got {}",
            z_after
        );
    }

    // -----------------------------------------------------------------------
    // 5. test_scm_counterfactual — counterfactual "what if X had been 2?" returns correct estimate
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_counterfactual() {
        // Model: X → Y, Y = 2X + 1 (no noise)
        let equations = vec![
            eq("X", vec![], vec![], 0.0),
            eq("Y", vec!["X"], vec![2.0], 1.0),
        ];

        let model = ScmModel::new(equations).unwrap();

        // Observed: X=3, Y=7 (which is correct: 2*3+1=7)
        let mut obs = HashMap::new();
        obs.insert("X".to_string(), 3.0);
        obs.insert("Y".to_string(), 7.0);

        // Counterfactual: what if X had been 2?
        let y_cf = model
            .counterfactual("X", 2.0, "Y", &obs)
            .expect("counterfactual should succeed");

        // With X=2: Y = 2*2 + 1 = 5
        assert!(
            (y_cf - 5.0).abs() < 1e-9,
            "counterfactual Y given X=2 should be 5.0, got {}",
            y_cf
        );
    }

    // -----------------------------------------------------------------------
    // 6. test_pc_partial_correlation — known 3×3 matrix, partial corr matches formula
    // -----------------------------------------------------------------------
    #[test]
    fn test_pc_partial_correlation() {
        // 3×3 correlation matrix
        // Variables: 0, 1, 2
        // r_01 = 0.8, r_02 = 0.6, r_12 = 0.5
        let n = 3;
        let mut corr = vec![0.0f64; n * n];
        // Diagonal = 1
        corr[0 * n + 0] = 1.0;
        corr[1 * n + 1] = 1.0;
        corr[2 * n + 2] = 1.0;
        corr[0 * n + 1] = 0.8;
        corr[1 * n + 0] = 0.8;
        corr[0 * n + 2] = 0.6;
        corr[2 * n + 0] = 0.6;
        corr[1 * n + 2] = 0.5;
        corr[2 * n + 1] = 0.5;

        // Partial correlation r_{01|2}
        // Formula: r_{01.2} = (r_01 - r_02 * r_12) / sqrt((1 - r_02^2)(1 - r_12^2))
        let expected = (0.8 - 0.6 * 0.5) / ((1.0 - 0.6_f64.powi(2)) * (1.0 - 0.5_f64.powi(2))).sqrt();

        let result = PcDiscovery::partial_correlation(&corr, n, 0, 1, &[2]);

        assert!(
            (result - expected).abs() < 1e-9,
            "partial correlation r_{{01|2}} expected {:.6}, got {:.6}",
            expected,
            result
        );
    }

    // -----------------------------------------------------------------------
    // 7. test_pc_fisher_z_independent — r=0.01 with n=100, p > 0.05
    // -----------------------------------------------------------------------
    #[test]
    fn test_pc_fisher_z_independent() {
        // Near-zero correlation should produce high p-value (not significant)
        let p = PcDiscovery::fisher_z_test(0.01, 100, 0);
        assert!(
            p > 0.05,
            "r=0.01 with n=100 should not be significant (p > 0.05), got p={}",
            p
        );
    }

    // -----------------------------------------------------------------------
    // 8. test_pc_fisher_z_dependent — r=0.8 with n=100, p < 0.001
    // -----------------------------------------------------------------------
    #[test]
    fn test_pc_fisher_z_dependent() {
        // High correlation should produce very small p-value (highly significant)
        let p = PcDiscovery::fisher_z_test(0.8, 100, 0);
        assert!(
            p < 0.001,
            "r=0.8 with n=100 should be highly significant (p < 0.001), got p={}",
            p
        );
    }

    // -----------------------------------------------------------------------
    // 9. test_pc_discover_chain — 3 variables with chain correlation → discovers edges
    // -----------------------------------------------------------------------
    #[test]
    fn test_pc_discover_chain() {
        // Simulate correlations consistent with a chain X → Y → Z
        // r_XY = 0.9 (strong), r_YZ = 0.9 (strong), r_XZ = 0.81 (moderate, due to Y)
        let vars = vec!["X".to_string(), "Y".to_string(), "Z".to_string()];
        let n = 3;
        let mut corr = vec![0.0f64; n * n];
        corr[0 * n + 0] = 1.0;
        corr[1 * n + 1] = 1.0;
        corr[2 * n + 2] = 1.0;
        // X-Y strong correlation
        corr[0 * n + 1] = 0.9;
        corr[1 * n + 0] = 0.9;
        // Y-Z strong correlation
        corr[1 * n + 2] = 0.9;
        corr[2 * n + 1] = 0.9;
        // X-Z weaker (through Y)
        corr[0 * n + 2] = 0.81;
        corr[2 * n + 0] = 0.81;

        let edges = PcDiscovery::discover(&vars, &corr, 0.05);

        // Should discover at least 2 edges (X-Y and Y-Z)
        // X-Z should be removed when conditioning on Y
        let edge_pairs: Vec<(String, String)> = edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();

        assert!(
            !edges.is_empty(),
            "expected to discover edges for chain X→Y→Z, got none"
        );

        // Should have X-Y edge
        let has_xy = edge_pairs.iter().any(|(f, t)| {
            (f == "X" && t == "Y") || (f == "Y" && t == "X")
        });
        assert!(has_xy, "expected X-Y edge, edges: {:?}", edge_pairs);

        // Should have Y-Z edge
        let has_yz = edge_pairs.iter().any(|(f, t)| {
            (f == "Y" && t == "Z") || (f == "Z" && t == "Y")
        });
        assert!(has_yz, "expected Y-Z edge, edges: {:?}", edge_pairs);
    }

    // -----------------------------------------------------------------------
    // 10. test_ensemble_merge — PC + Granger agree on edge → support=2, included
    // -----------------------------------------------------------------------
    #[test]
    fn test_ensemble_merge() {
        let pc_edges = vec![PcEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            orientation: EdgeOrientation::Directed,
        }];
        let granger_pairs = vec![("A".to_string(), "B".to_string())];
        let explicit_edges = vec![];

        let result = EnsembleDiscovery::merge(pc_edges, granger_pairs, explicit_edges, 1);

        assert_eq!(result.len(), 1, "expected 1 merged edge");
        let edge = &result[0];
        assert_eq!(edge.support, 2, "support should be 2 (PC + Granger)");
        assert!(
            edge.sources.contains(&"PC".to_string()),
            "sources should include PC"
        );
        assert!(
            edge.sources.contains(&"Granger".to_string()),
            "sources should include Granger"
        );
        assert_eq!(
            edge.orientation,
            EdgeOrientation::Directed,
            "both sources agree on direction → Directed"
        );
    }

    // -----------------------------------------------------------------------
    // 11. test_ensemble_min_support_filter — edge with support=1 filtered when min_support=2
    // -----------------------------------------------------------------------
    #[test]
    fn test_ensemble_min_support_filter() {
        // Only PC has A→B; Granger has C→D; no overlap
        let pc_edges = vec![PcEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            orientation: EdgeOrientation::Directed,
        }];
        let granger_pairs = vec![("C".to_string(), "D".to_string())];
        let explicit_edges = vec![];

        // With min_support=2, neither edge passes (each only has support=1)
        let result = EnsembleDiscovery::merge(pc_edges, granger_pairs, explicit_edges, 2);

        assert!(
            result.is_empty(),
            "expected no edges with min_support=2 when each edge has support=1, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // 12. test_notears_fallback_warning — returns warning message containing "NOTEARS"
    // -----------------------------------------------------------------------
    #[test]
    fn test_notears_fallback_warning() {
        let pc_edges = vec![PcEdge {
            from: "X".to_string(),
            to: "Y".to_string(),
            orientation: EdgeOrientation::Directed,
        }];
        let granger_pairs = vec![("X".to_string(), "Y".to_string())];

        let (edges, warning) = EnsembleDiscovery::notears_fallback(pc_edges, granger_pairs);

        assert!(
            warning.contains("NOTEARS"),
            "warning message should contain 'NOTEARS', got: {}",
            warning
        );
        assert!(
            !edges.is_empty(),
            "fallback should return at least one edge"
        );
    }

    // -----------------------------------------------------------------------
    // Bonus: test_scm_unknown_variable_error
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_unknown_variable_error() {
        let equations = vec![
            eq("X", vec![], vec![], 0.0),
            eq("Y", vec!["X"], vec![1.0], 0.0),
        ];

        let model = ScmModel::new(equations).unwrap();
        let obs = HashMap::new();

        let result = model.intervene("NONEXISTENT", 1.0, "Y", &obs);
        assert!(
            matches!(result, Err(ScmError::UnknownVariable(_))),
            "expected UnknownVariable error, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Bonus: test_scm_evaluate_with_observations
    // -----------------------------------------------------------------------
    #[test]
    fn test_scm_evaluate_with_observations() {
        // Model with a fork: W → X, W → Y; verify both X and Y computed correctly
        let equations = vec![
            eq("W", vec![], vec![], 0.0),
            eq("X", vec!["W"], vec![2.0], 0.0), // X = 2W
            eq("Y", vec!["W"], vec![-1.0], 3.0), // Y = -W + 3
        ];

        let model = ScmModel::new(equations).unwrap();

        let mut obs = HashMap::new();
        obs.insert("W".to_string(), 4.0);

        let result = model.evaluate(&obs);

        assert!((result["X"] - 8.0).abs() < 1e-9, "X = 2*4 = 8, got {}", result["X"]);
        assert!((result["Y"] - (-1.0)).abs() < 1e-9, "Y = -4 + 3 = -1, got {}", result["Y"]);
    }
}
