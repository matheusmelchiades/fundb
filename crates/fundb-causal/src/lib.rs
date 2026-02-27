// fundb-causal — causal engine: explicit causality (tier 1), statistical discovery (tier 2).

pub mod tier1;

pub use tier1::{CausalEngine, CausalError, CausalPath, TraceOptions, VisFormat};
