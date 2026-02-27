// fundb-core — FunRecord data model (STORY-1-1)

pub mod types;
pub mod record;
pub mod builder;
pub mod id;

pub use types::*;
pub use record::*;
pub use builder::FunRecordBuilder;
pub use id::new_record_id;
