// fundb-core — FunRecord data model (STORY-1-1)

pub mod builder;
pub mod codec;
pub mod id;
pub mod page;
pub mod record;
pub mod types;

pub use builder::FunRecordBuilder;
pub use id::new_record_id;
pub use record::*;
pub use types::*;
