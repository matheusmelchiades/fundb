// fundb-cognitive — cognitive layer: confidence, context, contradiction, memory.

pub mod confidence;
pub mod context;
pub mod contradiction;
pub mod memory;

pub use confidence::ConfidencePropagator;
pub use context::{ContextMetadata, ContextOptimizer, ContextOptions, SortKey};
pub use contradiction::{
    ContradictionCandidate, ContradictionDetector, ContradictionEvent, Polarity,
};
pub use memory::{
    AgentMemory, MemoryResult, MemoryType, RecallComponents, RecallWeights, RememberOptions,
};
