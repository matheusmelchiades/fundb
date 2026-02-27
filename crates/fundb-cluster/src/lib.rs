// fundb-cluster — shard management and distributed query execution for FunDB.

pub mod sharding;
pub mod dist_query;

pub use sharding::{ShardId, NodeId, ShardMap, ShardMigration, ReplicationGroup};
pub use dist_query::{
    AggOp, DistQuery, DistQueryConfig, DistQueryCoordinator, DistQueryError,
    MergedResult, MockShardExecutor, ShardRecord, ShardResult,
};
