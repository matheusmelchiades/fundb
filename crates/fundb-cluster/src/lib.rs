// fundb-cluster — shard management and distributed query execution for FunDB.

pub mod dist_query;
pub mod sharding;

pub use dist_query::{
    AggOp, DistQuery, DistQueryConfig, DistQueryCoordinator, DistQueryError, MergedResult,
    MockShardExecutor, ShardRecord, ShardResult,
};
pub use sharding::{NodeId, ReplicationGroup, ShardId, ShardMap, ShardMigration};
